"""First-principles typed contracts for the relational-fabric v4 evidence.

The issuance validator proves allocation, provenance, review, and freeze integrity.  This
module proves a different property: every issued decoded expectation is a consequence of
its controlled input and every negative fixture closes the authority it claims to close.
It deliberately imports neither production code nor predecessor evidence tooling.
"""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import re
from collections import deque
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_ROOT = Path("contracts/acceptance/relational-fabric-v4")
EXPECTATIONS_PATH = EVIDENCE_ROOT / "expectations.jsonl"
FIXTURES_PATH = EVIDENCE_ROOT / "negative-fixtures.jsonl"
ISSUANCE_PATH = EVIDENCE_ROOT / "evidence-issuance.json"

EVIDENCE_RELEASE = "wp33-v4-r6"
FROZEN_SHA256 = {
    EXPECTATIONS_PATH: "d9cd74a9cbd4a78f43117b191ef14ddc86957445fc9eb3016db63cf3f5608e7f",
    FIXTURES_PATH: "cce359c558a988ffa104ce4ca463617a79dc08774dc630eec8c8d66613d02d29",
}

EXPECTATION_SCHEMA = "codefabric.relational-fabric-v4.expectation.v1"
FIXTURE_SCHEMA = "codefabric.relational-fabric-v4.fixture.v1"

EXPECTED_FAMILIES = (
    "provider_rows",
    "provider_gaps",
    "producer_remainders",
    "transformations",
    "analyses",
    "query_find_code_entities",
    "query_retrieve_facts",
    "query_follow_relationships",
    "query_connecting_paths",
    "query_match_pattern",
    "query_combine_results",
    "query_summarize_facts",
    "query_source_context",
    "genesis",
    "activation_readback",
    "lifecycle",
    "recovery_pre_append",
    "recovery_uncertain_append",
    "supervisor_policy",
    "supervisor_singleton_multi_agent",
    "supervisor_control",
    "supervisor_fd3",
    "supervisor_restart_revocation",
    "rpc_handshake",
    "rpc_get_status",
    "rpc_get_reference",
    "rpc_validate_query",
    "rpc_start_query",
    "rpc_watch_query",
    "rpc_cancel_query",
    "rpc_read_resource",
    "rpc_release_resource",
    "wire_session_budget_cursor",
    "wire_errors",
    "mcp_query",
    "mcp_validate",
    "mcp_status",
    "mcp_reference",
    "mcp_lifespan_resources",
    "recovery_resource_bounds",
    "forward_only_zero_state",
)

_CAUSAL_PATHS: dict[str, frozenset[str]] = {
    "RFV4-CLAIM-001": frozenset(
        {
            "identity_anchors[]",
            "normalization_program.mappings.NamedExpr",
            "normalization_program.mappings.call",
            "normalization_program.transformation_id",
            "provider_native_rows[]",
        }
    ),
    "RFV4-CLAIM-002": frozenset(
        {
            "family",
            "provider",
            "provider_outcome",
            "requested_family",
            "scope",
            "source_image_id",
        }
    ),
    "RFV4-CLAIM-003": frozenset({"authority_rule"}),
    "RFV4-CLAIM-004": frozenset({"mapping.Call"}),
    "RFV4-CLAIM-005": frozenset({"edges[]"}),
    "RFV4-CLAIM-006": frozenset({"looking_for"}),
    "RFV4-CLAIM-007": frozenset({"available.target", "available.type", "unknown.type"}),
    "RFV4-CLAIM-008": frozenset({"distance"}),
    "RFV4-CLAIM-009": frozenset({"path_policy"}),
    "RFV4-CLAIM-010": frozenset(
        {"coverage.complete", "coverage.family", "coverage.owner"}
    ),
    "RFV4-CLAIM-011": frozenset({"operation"}),
    "RFV4-CLAIM-012": frozenset(
        {"about[]", "groups[]", "provenance_edges[]", "supporting_fact_ids[]"}
    ),
    "RFV4-CLAIM-013": frozenset({"byte_limit"}),
    "RFV4-CLAIM-014": frozenset({"deliveries", "same_command_id"}),
    "RFV4-CLAIM-015": frozenset(
        {
            "appended_event.control_horizon",
            "appended_event.versions.entities",
            "appended_event.versions.facts",
        }
    ),
    "RFV4-CLAIM-016": frozenset({"stop_at"}),
    "RFV4-CLAIM-017": frozenset(
        {"candidate_stage", "fault_point", "proof_receipt_present"}
    ),
    "RFV4-CLAIM-018": frozenset(
        {"observed_horizon.candidate", "observed_horizon.head"}
    ),
    "RFV4-CLAIM-019": frozenset(
        {
            "anti_replay_registry.identity",
            "anti_replay_registry.seen",
            "launcher_request.anti_replay_identity",
            "launcher_request.peer_pid",
            "launcher_request.peer_start_time_ticks",
            "launcher_request.peer_uid",
            "launcher_request.policy_id",
            "launcher_request.presentation",
            "launcher_request.request_at_unix_ms",
            "launcher_request.requested_operation",
            "launcher_request.requested_workspace",
            "launcher_request.supervisor_generation",
            "observed_adapter.distribution",
            "observed_adapter.distribution_version",
            "observed_adapter.executable_sha256",
            "policy.adapter_identity.distribution",
            "policy.adapter_identity.distribution_version",
            "policy.adapter_identity.executable_observation_supported",
            "policy.adapter_identity.executable_sha256",
            "policy.deadline_bounds.max_cleanup_budget_ms",
            "policy.deadline_bounds.max_execution_budget_ms",
            "policy.expires_at_unix_ms",
            "policy.issued_at_unix_ms",
            "policy.max_launches",
            "policy.mcp_host_bounds.max_inline_response_bytes",
            "policy.mcp_host_bounds.max_progress_events",
            "policy.mcp_host_bounds.max_request_bytes",
            "policy.not_before_unix_ms",
            "policy.operations[]",
            "policy.policy_id",
            "policy.principal",
            "policy.profiles[]",
            "policy.resource_bounds.max_live_resource_pages",
            "policy.resource_bounds.max_result_bytes",
            "policy.resource_bounds.max_running_queries",
            "policy.revision",
            "policy.revocation_generation",
            "policy.workspaces[]",
            "policy_file.authorized_root",
            "policy_file.device",
            "policy_file.file_type",
            "policy_file.inode",
            "policy_file.mode",
            "policy_file.opened_no_follow",
            "policy_file.owner_uid",
            "policy_file.path",
            "policy_file.strict_schema",
        }
    ),
    "RFV4-CLAIM-020": frozenset({"agent_exit"}),
    "RFV4-CLAIM-021": frozenset({"records[]"}),
    "RFV4-CLAIM-022": frozenset({"daemon_generation", "replacement_capability"}),
    "RFV4-CLAIM-023": frozenset(
        {
            "daemon_restart",
            "event",
            "new_daemon_generation",
            "new_supervisor_generation",
            "replacement_daemon_pid",
            "restart_policy",
            "revocation_targets[]",
            "supervisor_restart",
        }
    ),
    "RFV4-CLAIM-024": frozenset({"lifecycle"}),
    "RFV4-CLAIM-025": frozenset({"queue.queued", "queue.running"}),
    "RFV4-CLAIM-026": frozenset({"daemon_revision", "selector.id", "selector.kind"}),
    "RFV4-CLAIM-027": frozenset({"canonical_request.queries[]"}),
    "RFV4-CLAIM-028": frozenset({"delivery"}),
    "RFV4-CLAIM-029": frozenset(
        {
            "drop_after_sequence",
            "reconnect_cursor",
            "reconnect_cursor_fixture_oracle.daemon_generation",
            "reconnect_cursor_fixture_oracle.expires_at_unix_ms",
            "reconnect_cursor_fixture_oracle.next_sequence",
            "reconnect_cursor_fixture_oracle.non_public",
            "reconnect_cursor_fixture_oracle.preceding_event_content_sha256",
            "reconnect_cursor_fixture_oracle.principal_session_class",
            "reconnect_cursor_fixture_oracle.profile",
            "reconnect_cursor_fixture_oracle.query_id",
        }
    ),
    "RFV4-CLAIM-030": frozenset({"delivery"}),
    "RFV4-CLAIM-031": frozenset(
        {
            "authorization_observations[]",
            "range.end_offset_exclusive",
            "range.length",
            "range.offset",
            "read_cursor",
            "read_cursor_fixture_oracle.daemon_generation",
            "read_cursor_fixture_oracle.end_offset_exclusive",
            "read_cursor_fixture_oracle.expires_at_unix_ms",
            "read_cursor_fixture_oracle.lease_id",
            "read_cursor_fixture_oracle.next_offset",
            "read_cursor_fixture_oracle.owner_principal",
            "read_cursor_fixture_oracle.owner_session_id",
            "read_cursor_fixture_oracle.owner_workspace",
            "read_cursor_fixture_oracle.preceding_chunk_content_sha256",
            "read_cursor_fixture_oracle.resource_checksum_sha256",
            "read_cursor_fixture_oracle.resource_uri",
            "read_cursor_fixture_oracle.source_disclosure_policy",
            "read_cursor_fixture_oracle.supervisor_generation",
            "resource.artifact_lease.expires_at_unix_ms",
            "resource.artifact_lease.lease_id",
            "resource.artifact_lease.not_before_unix_ms",
            "resource.byte_length",
            "resource.checksum_sha256",
            "resource.descriptor.descriptor_policy_revision",
            "resource.owner_daemon_generation",
            "resource.owner_principal",
            "resource.owner_session_id",
            "resource.owner_supervisor_generation",
            "resource.owner_workspace",
            "resource.query_id",
            "resource.selector.page_index",
            "resource.uri",
            "session_fixture_oracle.daemon_generation",
            "session_fixture_oracle.expires_at_unix_ms",
            "session_fixture_oracle.not_before_unix_ms",
            "session_fixture_oracle.operation_grant_id",
            "session_fixture_oracle.principal",
            "session_fixture_oracle.principal_session_class",
            "session_fixture_oracle.revocation_generation",
            "session_fixture_oracle.session_id",
            "session_fixture_oracle.source_disclosure_policy",
            "session_fixture_oracle.supervisor_generation",
            "session_fixture_oracle.workspace",
            "session_metadata.session-bin",
        }
    ),
    "RFV4-CLAIM-032": frozenset({"delivery"}),
    "RFV4-CLAIM-033": frozenset({"execution_budget.nanos", "execution_budget.seconds"}),
    "RFV4-CLAIM-034": frozenset(
        {
            "semantic_case.availability",
            "semantic_case.condition",
            "semantic_case.grpc_status",
            "semantic_case.remainder_count",
            "semantic_case.unknown_count",
        }
    ),
    "RFV4-CLAIM-035": frozenset({"daemon_events[]"}),
    "RFV4-CLAIM-036": frozenset(
        {"daemon_validation.typed_code", "daemon_validation.valid", "request.queries[]"}
    ),
    "RFV4-CLAIM-037": frozenset(
        {
            "daemon_status.lifecycle",
            "daemon_status.queue.queued",
            "daemon_status.queue.running",
            "daemon_status.ready",
        }
    ),
    "RFV4-CLAIM-038": frozenset(
        {
            "daemon_reference.availability",
            "daemon_reference.remainder",
            "daemon_reference.revision",
        }
    ),
    "RFV4-CLAIM-039": frozenset(
        {
            "progress_bound",
            "progress_events",
            "resource.byte_length",
            "resource.requested_live_pages",
            "resource.selector.kind",
            "resource.selector.page_index",
            "resource.uri",
        }
    ),
    "RFV4-CLAIM-040": frozenset(
        {
            "limits.adapter_processes",
            "limits.journal_events",
            "limits.lease_ms",
            "limits.page_bytes",
            "limits.resident_page_buffers",
            "limits.running_queries",
            "limits.tombstone_ms",
            "load.adapter_exits",
            "load.cancel_query",
            "load.expired_leases",
            "load.leased_resources",
            "load.queries",
            "load.released_resources",
            "load.repeat_releases",
            "load.slow_consumer",
        }
    ),
    "RFV4-CLAIM-041": frozenset({"coverage_dimensions.installed_artifact"}),
}

_INPUT_KEYS: dict[str, frozenset[str]] = {
    "provider_rows": frozenset(
        {
            "case_id",
            "identity_anchors",
            "identity_release",
            "normalization_program",
            "provider_native_rows",
            "requested_family",
            "source_image_id",
        }
    ),
    "provider_gaps": frozenset(
        {
            "case_id",
            "family",
            "provider",
            "provider_outcome",
            "requested_family",
            "scope",
            "source_image_id",
        }
    ),
    "producer_remainders": frozenset(
        {"authority_rule", "case_id", "eligible_producers", "required_family"}
    ),
    "transformations": frozenset({"case_id", "mapping", "rows", "transformation"}),
    "analyses": frozenset(
        {
            "analysis",
            "analysis_algorithm_release",
            "case_id",
            "completeness",
            "edges",
            "entry",
            "fabric_epoch_id",
            "identity_anchors",
            "identity_release",
            "limit",
            "precision",
            "producer_id",
            "producer_release",
            "projection_id",
        }
    ),
    "query_find_code_entities": frozenset(
        {"case_id", "entities", "looking_for", "request", "within"}
    ),
    "query_retrieve_facts": frozenset(
        {"about", "available", "case_id", "facts", "request", "unknown"}
    ),
    "query_follow_relationships": frozenset(
        {
            "case_id",
            "direction",
            "distance",
            "edges",
            "relationship",
            "request",
            "starting_from",
        }
    ),
    "query_connecting_paths": frozenset(
        {"case_id", "edges", "from", "path_policy", "request", "to", "using"}
    ),
    "query_match_pattern": frozenset({"case_id", "coverage", "pattern", "request"}),
    "query_combine_results": frozenset({"case_id", "inputs", "operation", "request"}),
    "query_summarize_facts": frozenset(
        {
            "about",
            "case_id",
            "completeness",
            "fabric_epoch_id",
            "group_by",
            "groups",
            "measure",
            "precision",
            "producer_id",
            "producer_release",
            "provenance_edges",
            "request",
            "supporting_fact_ids",
        }
    ),
    "query_source_context": frozenset(
        {
            "about",
            "authorized",
            "byte_limit",
            "case_id",
            "context",
            "request",
            "source_bytes",
        }
    ),
    "genesis": frozenset(
        {
            "activation_head",
            "candidate",
            "case_id",
            "command_id",
            "deliveries",
            "profile",
            "same_command_id",
            "writer_generation",
        }
    ),
    "activation_readback": frozenset(
        {"appended_event", "candidate", "case_id", "expected_head", "readback"}
    ),
    "lifecycle": frozenset({"case_id", "installed_epoch", "stop_at", "transitions"}),
    "recovery_pre_append": frozenset(
        {
            "candidate",
            "candidate_stage",
            "case_id",
            "fault_point",
            "private_table_versions_written",
            "proof_receipt_present",
            "selected_head",
        }
    ),
    "recovery_uncertain_append": frozenset(
        {"append_outcome", "candidate", "case_id", "expected_head", "observed_horizon"}
    ),
    "supervisor_policy": frozenset(
        {
            "anti_replay_registry",
            "case_id",
            "current_revocation_generation",
            "launch_capacity_available",
            "launcher_peer",
            "launcher_request",
            "observed_adapter",
            "policy",
            "policy_file",
            "supervisor_generation",
        }
    ),
    "supervisor_singleton_multi_agent": frozenset(
        {
            "agent_exit",
            "attach_requests",
            "capacity",
            "case_id",
            "live_supervisor",
            "runtime_root",
            "singleton_lease",
            "socket_recovery_policy",
            "supervisor_socket",
            "workspace",
        }
    ),
    "supervisor_control": frozenset(
        {
            "case_id",
            "channel",
            "daemon_generation",
            "governed_max_record_bytes",
            "records",
            "supervisor_generation",
            "workspace",
        }
    ),
    "supervisor_fd3": frozenset(
        {
            "adapter_spawn",
            "argv_contains_capability",
            "capability",
            "capability_length_bytes",
            "capability_max_bytes",
            "case_id",
            "child_descriptors",
            "daemon_generation",
            "descriptor_policy",
            "environment_contains_capability",
            "issued_random_bits",
            "one_shot_fallback",
            "platform_descriptor_capability",
            "replacement_capability",
            "stderr_policy",
            "stdio_policy",
        }
    ),
    "supervisor_restart_revocation": frozenset(
        {
            "accepted_query",
            "case_id",
            "daemon_restart",
            "event",
            "fresh_grant",
            "new_daemon_generation",
            "new_supervisor_generation",
            "old_cursor",
            "old_daemon_generation",
            "old_daemon_process",
            "old_grant",
            "old_session",
            "old_supervisor_generation",
            "policy_id",
            "principal",
            "replacement_daemon_pid",
            "restart_policy",
            "retained_terminal",
            "revocation_targets",
            "supervisor_restart",
            "workspace",
        }
    ),
    "rpc_handshake": frozenset(
        {
            "case_id",
            "current_revocation_generation",
            "daemon_generation",
            "grant",
            "grant_anti_replay_identity",
            "grant_expires_at_unix_ms",
            "grant_issued_at_unix_ms",
            "grant_length_bytes",
            "grant_not_before_unix_ms",
            "grant_replayed",
            "grant_revocation_generation",
            "handshake_at_unix_ms",
            "handshake_policy",
            "lifecycle",
            "peer_uid",
            "profile",
            "registered",
        }
    ),
    "rpc_get_status": frozenset(
        {
            "case_id",
            "current_epoch",
            "lifecycle",
            "observation_contract",
            "queue",
            "session",
        }
    ),
    "rpc_get_reference": frozenset(
        {"authorized", "case_id", "daemon_revision", "selector", "session"}
    ),
    "rpc_validate_query": frozenset({"canonical_request", "case_id", "session"}),
    "rpc_start_query": frozenset(
        {
            "canonical_original_query_id",
            "capacity",
            "case_id",
            "delivery",
            "execution_budget_ms",
            "idempotency_key",
            "request_identity",
            "session",
        }
    ),
    "rpc_watch_query": frozenset(
        {
            "case_id",
            "cursor_fixture_oracle",
            "drop_after_sequence",
            "events",
            "query_id",
            "reconnect_cursor",
            "reconnect_cursor_fixture_oracle",
            "resume_cursor",
            "session",
        }
    ),
    "rpc_cancel_query": frozenset(
        {"case_id", "cleanup_budget_ms", "delivery", "query_id", "session", "state"}
    ),
    "rpc_read_resource": frozenset(
        {
            "authorization_observations",
            "case_id",
            "chunk_bytes",
            "consumer_credit_bytes",
            "consumer_delay_ms",
            "range",
            "read_cursor",
            "read_cursor_fixture_oracle",
            "resident_buffer_limit_bytes",
            "resource",
            "session_fixture_oracle",
            "session_metadata",
            "transport_window_bytes",
        }
    ),
    "rpc_release_resource": frozenset(
        {
            "case_id",
            "delivery",
            "lease_owner",
            "resource_id",
            "session",
            "tombstone_window_ms",
        }
    ),
    "wire_session_budget_cursor": frozenset(
        {
            "body_authority_fields",
            "case_id",
            "cursor_bindings",
            "execution_budget",
            "metadata",
            "package",
        }
    ),
    "wire_errors": frozenset({"case_id", "outer_cases", "semantic_case"}),
    "mcp_query": frozenset(
        {"case_id", "daemon_events", "request", "result_resource", "tool"}
    ),
    "mcp_validate": frozenset({"case_id", "daemon_validation", "request", "tool"}),
    "mcp_status": frozenset({"case_id", "daemon_status", "tool"}),
    "mcp_reference": frozenset({"case_id", "daemon_reference", "selector", "tool"}),
    "mcp_lifespan_resources": frozenset(
        {
            "case_id",
            "channel_ready",
            "handshake",
            "profile_reference_valid",
            "progress_bound",
            "progress_events",
            "resource",
        }
    ),
    "recovery_resource_bounds": frozenset({"case_id", "limits", "load"}),
    "forward_only_zero_state": frozenset(
        {
            "candidate_inventory",
            "case_id",
            "coverage_dimensions",
            "prohibited_live_authority_classes",
            "required_coverage_dimensions",
            "retained_history",
            "target_surfaces",
        }
    ),
}

# Exact nested object members for the released controlled-input grammar.  Maps whose
# keys are domain data (normalization maps, available/unknown families) are validated
# by their family deriver instead of being treated as open schema extension points.
_NESTED_KEYS: dict[str, dict[str, frozenset[str]]] = {
    "provider_rows": {
        "provider_native_rows[]": frozenset(
            {
                "provider",
                "fact_id",
                "occurrence_id",
                "raw_kind",
                "start_byte",
                "end_byte",
                "authority_class",
            }
        ),
        "normalization_program": frozenset({"transformation_id", "mappings"}),
        "identity_anchors[]": frozenset({"identity_inputs", "fact_id"}),
        "identity_anchors[].identity_inputs": frozenset(
            {"transformation_id", "input_fact_id", "normalized_kind"}
        ),
    },
    "transformations": {
        "rows[]": frozenset({"occurrence_id", "raw_kind", "language"}),
    },
    "analyses": {
        "edges[]": frozenset({"fact_id", "from", "to"}),
        "identity_anchors[]": frozenset({"identity_inputs", "fact_id"}),
        "identity_anchors[].identity_inputs": frozenset(
            {
                "analysis_algorithm_release",
                "from",
                "to",
                "distance",
                "supporting_fact_ids",
            }
        ),
    },
    "query_find_code_entities": {
        "entities[]": frozenset({"id", "role"}),
    },
    "query_connecting_paths": {
        "edges[]": frozenset({"fact_id", "from", "to"}),
    },
    "query_match_pattern": {
        "pattern": frozenset({"node", "not"}),
        "pattern.not": frozenset({"relationship"}),
        "coverage": frozenset({"family", "owner", "complete"}),
    },
    "query_combine_results": {
        "inputs[]": frozenset({"workspace", "role", "ids"}),
    },
    "query_summarize_facts": {
        "groups[]": frozenset({"module", "members"}),
        "provenance_edges[]": frozenset({"kind", "fact_id"}),
    },
    "activation_readback": {
        "appended_event": frozenset(
            {
                "activation_id",
                "predecessor",
                "versions",
                "writer_generation",
                "control_horizon",
            }
        ),
        "appended_event.versions": frozenset({"entities", "facts"}),
    },
    "recovery_uncertain_append": {
        "observed_horizon": frozenset({"head", "candidate"}),
    },
    "supervisor_policy": {
        "policy": frozenset(
            {
                "policy_id",
                "principal",
                "workspaces",
                "operations",
                "profiles",
                "max_launches",
                "revision",
                "issued_at_unix_ms",
                "not_before_unix_ms",
                "expires_at_unix_ms",
                "revocation_generation",
                "mcp_host_bounds",
                "resource_bounds",
                "deadline_bounds",
                "adapter_identity",
            }
        ),
        "policy.mcp_host_bounds": frozenset(
            {"max_request_bytes", "max_inline_response_bytes", "max_progress_events"}
        ),
        "policy.resource_bounds": frozenset(
            {"max_running_queries", "max_result_bytes", "max_live_resource_pages"}
        ),
        "policy.deadline_bounds": frozenset(
            {"max_execution_budget_ms", "max_cleanup_budget_ms"}
        ),
        "policy.adapter_identity": frozenset(
            {
                "distribution",
                "distribution_version",
                "executable_observation_supported",
                "executable_sha256",
            }
        ),
        "launcher_request": frozenset(
            {
                "policy_id",
                "presentation",
                "peer_uid",
                "peer_pid",
                "peer_start_time_ticks",
                "supervisor_generation",
                "requested_workspace",
                "requested_operation",
                "request_at_unix_ms",
                "anti_replay_identity",
            }
        ),
        "policy_file": frozenset(
            {
                "path",
                "authorized_root",
                "opened_no_follow",
                "owner_uid",
                "mode",
                "strict_schema",
                "file_type",
                "device",
                "inode",
            }
        ),
        "launcher_peer": frozenset({"uid", "pid", "start_time_ticks"}),
        "anti_replay_registry": frozenset({"identity", "seen"}),
        "observed_adapter": frozenset(
            {"distribution", "distribution_version", "executable_sha256"}
        ),
    },
    "supervisor_singleton_multi_agent": {
        "live_supervisor": frozenset({"generation", "daemon_pid"}),
        "attach_requests[]": frozenset({"agent", "policy_id"}),
        "runtime_root": frozenset(
            {
                "path",
                "authorized_root",
                "no_symlink_components",
                "owner_uid",
                "mode",
                "type",
                "device",
            }
        ),
        "singleton_lease": frozenset(
            {"owned", "winner_pid", "winner_start_time_ticks", "capacity_reserved"}
        ),
        "supervisor_socket": frozenset(
            {
                "path",
                "type",
                "owner_uid",
                "mode",
                "device",
                "inode",
                "generation",
                "live_probe",
                "cleanup_requires_exact_identity",
            }
        ),
        "socket_recovery_policy": frozenset(
            {
                "requires_singleton_lease",
                "requires_failed_live_probe",
                "requires_exact_type_owner_mode_device_inode_generation",
                "replacement_inode_safe_cleanup",
            }
        ),
    },
    "supervisor_control": {
        "records[]": frozenset(
            {
                "sequence",
                "workspace",
                "daemon_generation",
                "supervisor_generation",
                "operation",
                "operation_identity",
                "expires_at_unix_ms",
                "declared_length_bytes",
                "content_sha256",
            }
        ),
        "channel": frozenset({"transport", "channel_generation", "state"}),
    },
    "supervisor_fd3": {
        "child_descriptors": frozenset({"stdin", "stdout", "stderr", "fd3"}),
        "descriptor_policy": frozenset(
            {
                "capability_fd",
                "allowlisted_inherited_fds",
                "unrelated_descriptors_close_on_exec",
                "fd3_direction",
                "frame_contract",
                "fd3_channel_open_after_delivery",
                "parent_writer_open_for_replacement",
                "child_fd3_open_for_replacement",
                "terminal_eof_observed",
            }
        ),
        "one_shot_fallback": frozenset(
            {
                "enabled",
                "path",
                "authorized_root",
                "opened_no_follow",
                "owner_uid",
                "mode",
                "path_device",
                "path_inode",
                "opened_device",
                "opened_inode",
                "unlinked_after_open",
                "max_reads",
                "unlinked_immediately_after_open",
                "capability_logged",
                "cleanup_complete",
            }
        ),
        "adapter_spawn": frozenset(
            {
                "stage",
                "child_pid",
                "process_group_owned",
                "grant_registered",
                "diagnostic_tasks_joined",
                "descriptor_cleanup_complete",
            }
        ),
        "stdio_policy": frozenset({"stdin", "stdout", "launcher_proxy_copies"}),
        "stderr_policy": frozenset(
            {
                "byte_limit",
                "emitted_bytes",
                "forwarded_bytes",
                "truncated_bytes",
                "accounting_complete",
            }
        ),
        "platform_descriptor_capability": frozenset(
            {"fixed_fd3_inheritance_available", "probe_status", "selected_transport"}
        ),
    },
    "supervisor_restart_revocation": {
        "old_daemon_process": frozenset({"pid", "process_group_owned", "state"}),
        "restart_policy": frozenset(
            {
                "drain_timeout_ms",
                "kill_timeout_ms",
                "join_lifecycle_tasks",
                "reap_owned_children",
                "cleanup_owned_socket_by_exact_identity",
                "reacquire_singleton_lease",
            }
        ),
    },
    "rpc_handshake": {
        "handshake_policy": frozenset(
            {
                "session_ttl_ms",
                "session_minimum_bytes",
                "grant_maximum_bytes",
                "required_peer_uid",
            }
        ),
    },
    "rpc_get_status": {
        "queue": frozenset({"running", "queued"}),
        "observation_contract": frozenset(
            {
                "query_execution_forbidden",
                "catalog_traversal_forbidden",
                "provider_read_forbidden",
            }
        ),
    },
    "rpc_get_reference": {"selector": frozenset({"kind", "id"})},
    "rpc_validate_query": {
        "canonical_request": frozenset(
            {"specification", "version", "scope", "freshness", "queries"}
        ),
        "canonical_request.scope": frozenset({"workspace_id"}),
        "canonical_request.freshness": frozenset({"policy"}),
        "canonical_request.queries[]": frozenset(
            {
                "query_id",
                "request",
                "from",
                "to",
                "using",
                "path_policy",
                "bound",
                "return",
            }
        ),
        "canonical_request.queries[].from[]": frozenset({"entity_id"}),
        "canonical_request.queries[].to[]": frozenset({"entity_id"}),
        "canonical_request.queries[].return": frozenset(),
    },
    "rpc_start_query": {
        "capacity": frozenset(
            {"coordinator", "journal_bytes", "result_bytes", "retention_ms"}
        ),
    },
    "rpc_watch_query": {
        "events[]": frozenset({"sequence", "kind"}),
        "cursor_fixture_oracle": frozenset(
            {
                "non_public",
                "query_id",
                "principal_session_class",
                "daemon_generation",
                "profile",
                "next_sequence",
                "preceding_event_content_sha256",
                "expires_at_unix_ms",
            }
        ),
    },
    "rpc_read_resource": {
        "authorization_observations[]": frozenset(
            {
                "artifact_lease",
                "current_revocation_generation",
                "cursor",
                "descriptor",
                "offset",
                "operation",
                "owner",
                "range",
                "read_at_unix_ms",
                "read_index",
                "session_valid",
                "source_disclosure",
                "workspace",
            }
        ),
        "authorization_observations[].artifact_lease": frozenset(
            {
                "active",
                "expires_at_unix_ms",
                "lease_id",
                "not_before_unix_ms",
            }
        ),
        "authorization_observations[].cursor": frozenset(
            {
                "content_bound",
                "end_offset_exclusive",
                "next_offset",
                "preceding_chunk_content_sha256",
                "resource_checksum_sha256",
                "resource_uri",
            }
        ),
        "authorization_observations[].descriptor": frozenset(
            {
                "descriptor_policy_revision",
                "filesystem_descriptor",
                "kind",
                "object_store_descriptor",
                "path_descriptor",
            }
        ),
        "authorization_observations[].operation": frozenset(
            {"authorized", "grant_id", "requested"}
        ),
        "authorization_observations[].owner": frozenset(
            {
                "daemon_generation",
                "principal",
                "session_id",
                "supervisor_generation",
            }
        ),
        "authorization_observations[].range": frozenset(
            {"authorized", "current_offset", "end_offset_exclusive", "start_offset"}
        ),
        "authorization_observations[].source_disclosure": frozenset(
            {"authorized", "policy", "source_bearing"}
        ),
        "authorization_observations[].workspace": frozenset(
            {"authorized_workspace", "requested_workspace"}
        ),
        "range": frozenset({"offset", "length", "end_offset_exclusive"}),
        "read_cursor_fixture_oracle": frozenset(
            {
                "non_public",
                "resource_uri",
                "owner_principal",
                "owner_session_id",
                "owner_workspace",
                "daemon_generation",
                "supervisor_generation",
                "operation",
                "lease_id",
                "next_offset",
                "end_offset_exclusive",
                "preceding_chunk_content_sha256",
                "resource_checksum_sha256",
                "source_disclosure_policy",
                "expires_at_unix_ms",
            }
        ),
        "resource": frozenset(
            {
                "uri",
                "selector",
                "byte_length",
                "checksum_sha256",
                "query_id",
                "owner_workspace",
                "owner_principal",
                "owner_session_id",
                "owner_daemon_generation",
                "owner_supervisor_generation",
                "artifact_lease",
                "descriptor",
                "source_bearing",
            }
        ),
        "resource.artifact_lease": frozenset(
            {
                "active",
                "expires_at_unix_ms",
                "lease_id",
                "not_before_unix_ms",
            }
        ),
        "resource.descriptor": frozenset(
            {
                "descriptor_policy_revision",
                "filesystem_descriptor",
                "kind",
                "object_store_descriptor",
                "path_descriptor",
            }
        ),
        "resource.selector": frozenset({"kind", "page_index"}),
        "session_fixture_oracle": frozenset(
            {
                "non_public",
                "session_id",
                "principal",
                "principal_session_class",
                "workspace",
                "daemon_generation",
                "supervisor_generation",
                "revocation_generation",
                "operations",
                "operation_grant_id",
                "not_before_unix_ms",
                "expires_at_unix_ms",
                "source_disclosure_policy",
            }
        ),
        "session_metadata": frozenset({"session-bin"}),
    },
    "wire_session_budget_cursor": {
        "metadata": frozenset({"session-bin"}),
        "execution_budget": frozenset({"seconds", "nanos"}),
    },
    "wire_errors": {
        "outer_cases[]": frozenset({"condition", "grpc_status", "typed_code"}),
        "semantic_case": frozenset(
            {
                "condition",
                "grpc_status",
                "availability",
                "unknown_count",
                "remainder_count",
            }
        ),
    },
    "mcp_query": {
        "request": frozenset({"request", "looking_for"}),
        "result_resource": frozenset({"uri", "selector"}),
        "result_resource.selector": frozenset({"kind"}),
    },
    "mcp_validate": {
        "request": frozenset(
            {"specification", "version", "scope", "freshness", "queries"}
        ),
        "request.scope": frozenset({"workspace_id"}),
        "request.freshness": frozenset({"policy"}),
        "request.queries[]": frozenset(
            {
                "query_id",
                "request",
                "starting_from",
                "relationship",
                "direction",
                "distance",
                "return",
            }
        ),
        "request.queries[].starting_from[]": frozenset({"entity_id"}),
        "request.queries[].return": frozenset(),
        "daemon_validation": frozenset({"valid", "typed_code"}),
    },
    "mcp_status": {
        "daemon_status": frozenset({"lifecycle", "ready", "queue"}),
        "daemon_status.queue": frozenset({"running", "queued"}),
    },
    "mcp_reference": {
        "selector": frozenset({"kind", "id"}),
        "daemon_reference": frozenset({"revision", "availability", "remainder"}),
    },
    "mcp_lifespan_resources": {
        "resource": frozenset(
            {"uri", "selector", "byte_length", "requested_live_pages"}
        ),
        "resource.selector": frozenset({"kind", "page_index"}),
    },
    "recovery_resource_bounds": {
        "limits": frozenset(
            {
                "running_queries",
                "journal_events",
                "page_bytes",
                "resident_page_buffers",
                "lease_ms",
                "tombstone_ms",
                "adapter_processes",
            }
        ),
        "load": frozenset(
            {
                "queries",
                "slow_consumer",
                "cancel_query",
                "leased_resources",
                "released_resources",
                "expired_leases",
                "repeat_releases",
                "adapter_exits",
            }
        ),
    },
    "forward_only_zero_state": {
        "candidate_inventory[]": frozenset(
            {"candidate_id", "classification", "dimension", "disposition", "subject"}
        ),
        "target_surfaces": frozenset(
            {"proto_packages", "daemon_targets", "supervisors", "mcp_catalog"}
        ),
        "retained_history[]": frozenset(
            {
                "artifact_id",
                "candidate_id",
                "classification",
                "retention",
                "selectable",
                "live_reader_count",
            }
        ),
    },
}


class V4ContractError(ValueError):
    """A typed expectation or fixture violates the independently encoded contract."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class ContractReport:
    """Validated family and fixture counts."""

    families: int
    expectations: int
    causal_fixtures: int
    negative_fixtures: int


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", f"{context} must be an object")
    return value


def _sequence(value: object, context: str) -> Sequence[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", f"{context} must be an array")
    return value


def _strict_keys(
    value: Mapping[str, Any], expected: frozenset[str], context: str
) -> None:
    actual = frozenset(value)
    if actual != expected:
        added = sorted(actual - expected)
        missing = sorted(expected - actual)
        raise V4ContractError(
            "V4_INVENTED_CONTROL_KNOB",
            f"{context} keys drifted; added={added}, missing={missing}",
        )


_BOOL_FIELDS = frozenset(
    {
        "authorized",
        "complete",
        "same_command_id",
        "proof_receipt_present",
        "current",
        "stale_current_facts",
        "opened_no_follow",
        "strict_schema",
        "seen",
        "authorized_root",
        "no_symlink_components",
        "owned",
        "capacity_reserved",
        "cleanup_requires_exact_identity",
        "requires_singleton_lease",
        "requires_failed_live_probe",
        "requires_exact_type_owner_mode_device_inode_generation",
        "replacement_inode_safe_cleanup",
        "unrelated_descriptors_close_on_exec",
        "fd3_eof_after_frame",
        "parent_writer_closed_after_delivery",
        "child_fd3_closed_after_read",
        "enabled",
        "unlinked_after_open",
        "unlinked_immediately_after_open",
        "capability_logged",
        "cleanup_complete",
        "process_group_owned",
        "grant_registered",
        "diagnostic_tasks_joined",
        "descriptor_cleanup_complete",
        "accounting_complete",
        "argv_contains_capability",
        "environment_contains_capability",
        "registered",
        "grant_replayed",
        "query_execution_forbidden",
        "catalog_traversal_forbidden",
        "provider_read_forbidden",
        "non_public",
        "slow_consumer",
        "channel_ready",
        "profile_reference_valid",
        "retained_terminal",
        "daemon_restart",
        "valid",
        "executable_observation_supported",
        "fd3_channel_open_after_delivery",
        "parent_writer_open_for_replacement",
        "child_fd3_open_for_replacement",
        "terminal_eof_observed",
        "fixed_fd3_inheritance_available",
        "supervisor_restart",
        "join_lifecycle_tasks",
        "reap_owned_children",
        "cleanup_owned_socket_by_exact_identity",
        "reacquire_singleton_lease",
        "ready",
        "cursor_match",
        "lease_active",
        "operation_authorized",
        "range_authorized",
        "resource_owner_match",
        "session_valid",
        "active",
        "content_bound",
        "filesystem_descriptor",
        "object_store_descriptor",
        "path_descriptor",
        "source_bearing",
        "source_disclosure_allowed",
        "principal_revoked",
        "selectable",
    }
)
_INT_FIELDS = frozenset(
    {
        "start_byte",
        "end_byte",
        "distance",
        "limit",
        "deliveries",
        "writer_generation",
        "control_horizon",
        "entities",
        "facts",
        "private_table_versions_written",
        "max_launches",
        "revision",
        "issued_at_unix_ms",
        "not_before_unix_ms",
        "expires_at_unix_ms",
        "revocation_generation",
        "peer_uid",
        "peer_pid",
        "peer_start_time_ticks",
        "supervisor_generation",
        "request_at_unix_ms",
        "owner_uid",
        "uid",
        "pid",
        "start_time_ticks",
        "device",
        "inode",
        "launch_capacity_available",
        "generation",
        "daemon_pid",
        "capacity",
        "winner_pid",
        "winner_start_time_ticks",
        "sequence",
        "daemon_generation",
        "declared_length_bytes",
        "channel_generation",
        "governed_max_record_bytes",
        "capability_fd",
        "path_device",
        "path_inode",
        "opened_device",
        "opened_inode",
        "max_reads",
        "child_pid",
        "launcher_proxy_copies",
        "byte_limit",
        "emitted_bytes",
        "forwarded_bytes",
        "truncated_bytes",
        "capability_length_bytes",
        "capability_max_bytes",
        "issued_random_bits",
        "old_generation",
        "new_generation",
        "grant_issued_at_unix_ms",
        "grant_not_before_unix_ms",
        "grant_expires_at_unix_ms",
        "handshake_at_unix_ms",
        "grant_length_bytes",
        "grant_revocation_generation",
        "current_revocation_generation",
        "session_ttl_ms",
        "session_minimum_bytes",
        "grant_maximum_bytes",
        "required_peer_uid",
        "running",
        "queued",
        "coordinator",
        "journal_bytes",
        "result_bytes",
        "retention_ms",
        "execution_budget_ms",
        "next_sequence",
        "cleanup_budget_ms",
        "page_index",
        "byte_length",
        "read_bound",
        "transport_window_bytes",
        "consumer_credit_bytes",
        "chunk_bytes",
        "resident_buffer_limit_bytes",
        "consumer_delay_ms",
        "tombstone_window_ms",
        "seconds",
        "nanos",
        "unknown_count",
        "remainder_count",
        "progress_events",
        "progress_bound",
        "requested_live_pages",
        "running_queries",
        "journal_events",
        "page_bytes",
        "resident_page_buffers",
        "lease_ms",
        "tombstone_ms",
        "adapter_processes",
        "queries",
        "leased_resources",
        "released_resources",
        "expired_leases",
        "repeat_releases",
        "adapter_exits",
        "historical_live_readers",
        "max_request_bytes",
        "max_inline_response_bytes",
        "max_progress_events",
        "max_running_queries",
        "max_result_bytes",
        "max_live_resource_pages",
        "max_execution_budget_ms",
        "max_cleanup_budget_ms",
        "drain_timeout_ms",
        "kill_timeout_ms",
        "replacement_daemon_pid",
        "old_supervisor_generation",
        "new_supervisor_generation",
        "old_daemon_generation",
        "new_daemon_generation",
        "daemon_revision",
        "bound",
        "drop_after_sequence",
        "offset",
        "start_offset",
        "current_offset",
        "length",
        "end_offset_exclusive",
        "next_offset",
        "read_index",
        "read_at_unix_ms",
        "descriptor_policy_revision",
        "classified_count",
        "unreadable_count",
        "unparsed_count",
        "overlapping_count",
        "count_incoherent_count",
        "retain_target_count",
        "retain_history_count",
        "exclude_non_authority_count",
        "candidate_count",
        "parse_error_count",
        "skipped_count",
        "unmatched_count",
        "live_count",
        "unclassified_count",
        "live_reader_count",
        "owner_daemon_generation",
        "owner_supervisor_generation",
        "completed_units",
        "total_units",
    }
)
_NULLABLE_FIELDS = frozenset(
    {
        "agent_exit",
        "replacement_capability",
        "drop_after_sequence",
        "reconnect_cursor",
        "reconnect_cursor_fixture_oracle",
        "candidate",
        "bound",
        "typed_code",
        "remainder",
    }
)


def _field_name(path: str) -> str:
    return path.rsplit(".", 1)[-1].removesuffix("[]")


def _validate_scalar_type(value: object, path: str) -> None:
    name = _field_name(path)
    if value is None:
        if name not in _NULLABLE_FIELDS:
            raise V4ContractError("V4_TYPED_SCHEMA_INVALID", f"{path} may not be null")
        return
    if path.endswith("policy_file.authorized_root") or (
        path.endswith("[]") and name in {"facts", "entities"}
    ):
        valid = isinstance(value, str)
    elif name in _BOOL_FIELDS:
        valid = type(value) is bool
    elif name in _INT_FIELDS or name == "allowlisted_inherited_fds":
        valid = type(value) is int
    else:
        valid = isinstance(value, str)
    if not valid:
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID",
            f"{path} has invalid type {type(value).__name__}",
        )


def _validate_nested_input(family: str, value: Mapping[str, Any]) -> None:
    schemas = _NESTED_KEYS.get(family, {})

    def walk(item: object, path: str) -> None:
        if isinstance(item, Mapping):
            expected = schemas.get(path)
            if (
                family == "supervisor_restart_revocation"
                and path == ""
                and item.get("event") == "principal_policy_revocation"
                and expected is not None
            ):
                expected = expected - {"replacement_daemon_pid", "restart_policy"}
            if (
                family == "recovery_uncertain_append"
                and path == "observed_horizon"
                and "candidate" not in item
                and expected is not None
            ):
                expected = expected - {"candidate"}
            if (
                family == "mcp_validate"
                and path == "request.queries[]"
                and "relationship" not in item
                and expected is not None
            ):
                expected = expected - {"relationship"}
            if (
                family == "mcp_reference"
                and path == "daemon_reference"
                and "remainder" not in item
                and expected is not None
            ):
                expected = expected - {"remainder"}
            if family == "supervisor_control" and path == "records[]":
                common = frozenset(
                    {
                        "sequence",
                        "workspace",
                        "daemon_generation",
                        "supervisor_generation",
                        "operation",
                        "operation_identity",
                        "expires_at_unix_ms",
                        "declared_length_bytes",
                        "content_sha256",
                    }
                )
                operation = item.get("operation")
                extra = {
                    "RegisterLaunchGrant": frozenset({"grant_digest"}),
                    "RevokePrincipal": frozenset({"principal"}),
                }.get(str(operation), frozenset())
                expected = common | extra
            if family == "mcp_query" and path == "daemon_events[]":
                expected = {
                    "SnapshotPinned": frozenset({"kind", "epoch_id"}),
                    "Progress": frozenset(
                        {
                            "kind",
                            "current_query_id",
                            "phase",
                            "completed_units",
                            "total_units",
                            "safe_message",
                        }
                    ),
                    "ResultReady": frozenset({"kind", "resource_uri"}),
                    "Terminal": frozenset({"kind", "terminal_state"}),
                }.get(str(item.get("kind")))
            if family == "forward_only_zero_state" and path.startswith(
                "coverage_dimensions."
            ):
                expected = frozenset(
                    {
                        "candidate_ids",
                        "candidate_count",
                        "classified_count",
                        "count_incoherent_count",
                        "disposition_counts",
                        "overlapping_count",
                        "parse_error_count",
                        "skipped_count",
                        "unparsed_count",
                        "unreadable_count",
                        "unknown_count",
                        "unmatched_count",
                    }
                )
            if family == "forward_only_zero_state" and path.endswith(
                ".disposition_counts"
            ):
                expected = frozenset(
                    {
                        "exclude_non_authority_count",
                        "retain_history_count",
                        "retain_target_count",
                    }
                )
            if family == "forward_only_zero_state" and path.startswith(
                "prohibited_live_authority_classes."
            ):
                expected = frozenset({"live_count", "unclassified_count"})
            if expected is not None:
                _strict_keys(item, expected, f"{family}.{path}")
            for key, child in item.items():
                walk(child, f"{path}.{key}" if path else key)
            return
        if isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
            for child in item:
                walk(child, f"{path}[]")
            return
        _validate_scalar_type(item, path)

    walk(value, "")
    if family == "provider_rows":
        native_kinds = {
            _mapping(row, "provider row")["raw_kind"]
            for row in _sequence(value["provider_native_rows"], "provider_native_rows")
        }
        mappings = set(
            _mapping(
                _mapping(value["normalization_program"], "normalization_program")[
                    "mappings"
                ],
                "normalization mappings",
            )
        )
        allowed_mappings = {"call", "Call", "NamedExpr"}
        if not native_kinds <= mappings or not mappings <= allowed_mappings:
            raise V4ContractError(
                "V4_INVENTED_CONTROL_KNOB",
                "normalization mappings escape the released provider-kind vocabulary",
            )
        mapping_values = set(
            _mapping(
                _mapping(value["normalization_program"], "normalization_program")[
                    "mappings"
                ],
                "normalization mappings",
            ).values()
        )
        if not mapping_values <= {"call_expression", "assignment_expression"}:
            raise V4ContractError(
                "V4_TYPED_ENUM_INVALID",
                "normalization mapping targets escape the released vocabulary",
            )
    if family == "transformations":
        row_kinds = {
            _mapping(row, "transformation row")["raw_kind"]
            for row in _sequence(value["rows"], "rows")
        }
        if set(_mapping(value["mapping"], "mapping")) != row_kinds:
            raise V4ContractError(
                "V4_INVENTED_CONTROL_KNOB",
                "transformation mapping must be exactly the consumed raw kinds",
            )
    if family == "query_retrieve_facts":
        requested = set(_sequence(value["facts"], "facts"))
        available = set(_mapping(value["available"], "available"))
        unknown = set(_mapping(value["unknown"], "unknown"))
        if available & unknown or available | unknown != requested:
            raise V4ContractError(
                "V4_INVENTED_CONTROL_KNOB",
                "requested fact families need one exact available-or-unknown owner",
            )
    if family == "query_follow_relationships" and any(
        len(_sequence(edge, "relationship edge")) != 2
        for edge in _sequence(value["edges"], "edges")
    ):
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID",
            "relationship edges must contain exactly from and to",
        )


_ENUMS: dict[tuple[str, str], frozenset[object]] = {
    ("provider_rows", "requested_family"): frozenset({"syntax.call"}),
    ("provider_gaps", "provider_outcome"): frozenset(
        {"compile_failed", "sidecar_unavailable"}
    ),
    ("transformations", "transformation"): frozenset({"normalize_syntax_kind"}),
    ("analyses", "analysis"): frozenset({"cfg_reachability"}),
    ("query_follow_relationships", "direction"): frozenset({"outgoing", "incoming"}),
    ("query_connecting_paths", "path_policy"): frozenset({"shortest", "all shortest"}),
    ("query_combine_results", "operation"): frozenset({"intersection", "union"}),
    ("query_source_context", "context"): frozenset({"surrounding lines"}),
    ("genesis", "profile"): frozenset({"FreshActivation"}),
    ("genesis", "activation_head"): frozenset({"Empty"}),
    ("activation_readback", "readback"): frozenset({"same_event"}),
    ("lifecycle", "stop_at"): frozenset({"Ready", "EndpointsBoundBootstrapping"}),
    ("recovery_pre_append", "candidate_stage"): frozenset(
        {"proved", "tables_written_unproved"}
    ),
    ("recovery_uncertain_append", "append_outcome"): frozenset({"unknown"}),
    ("supervisor_policy", "launcher_request.presentation"): frozenset({"stdio"}),
    ("supervisor_policy", "policy_file.file_type"): frozenset({"regular_file"}),
    ("supervisor_control", "channel.transport"): frozenset({"unnamed_socketpair"}),
    ("supervisor_control", "channel.state"): frozenset({"healthy"}),
    ("supervisor_fd3", "descriptor_policy.fd3_direction"): frozenset(
        {"supervisor_to_adapter_only"}
    ),
    ("rpc_handshake", "profile"): frozenset({"cpg.v2"}),
    ("rpc_handshake", "lifecycle"): frozenset({"BOOTSTRAPPING", "READY"}),
    ("rpc_get_status", "lifecycle"): frozenset({"READY", "BOOTSTRAPPING"}),
    ("wire_session_budget_cursor", "package"): frozenset({"codefabric.cpgd.v2"}),
    ("mcp_query", "tool"): frozenset({"query_code_graph"}),
    ("mcp_validate", "tool"): frozenset({"validate_code_graph_query"}),
    ("mcp_status", "tool"): frozenset({"get_code_graph_status"}),
    ("mcp_reference", "tool"): frozenset({"get_code_graph_reference"}),
}


def _lookup_path(value: Mapping[str, Any], path: str) -> object:
    current: object = value
    for part in path.split("."):
        current = _mapping(current, path)[part]
    return current


def _validate_enums(family: str, value: Mapping[str, Any]) -> None:
    for (enum_family, path), allowed in _ENUMS.items():
        if enum_family == family and _lookup_path(value, path) not in allowed:
            raise V4ContractError(
                "V4_TYPED_ENUM_INVALID",
                f"{family}.{path} is outside {sorted(allowed)!r}",
            )


_ENTITY_ID = re.compile(r"entity:[a-z][a-z0-9-]*:[0-9a-f]{32}\Z")
_FACT_ID = re.compile(r"fact:[a-z][a-z0-9-]*:[0-9a-f]{32}\Z")
_WORKSPACE_ID = re.compile(r"workspace:[0-9a-f]{32}\Z")
_EPOCH_ID = re.compile(r"fabric-epoch:[0-9a-f]{32}\Z")
_RESOURCE_URI = re.compile(r"codefabric-result://[0-9a-f]{32}/[0-9a-f]{32}\Z")
_SHA256_ID = re.compile(r"sha256:[0-9a-f]{64}\Z")
_QUERY_HANDLE = re.compile(r"[0-9a-f]{32}\Z")
_SESSION_ID = re.compile(r"session:[0-9a-f]{32}\Z")
_LEASE_ID = re.compile(r"lease:[0-9a-f]{32}\Z")
_EPOCH_HANDLE = re.compile(r"epoch:[0-9a-f]{32}\Z")
_CLAIM_ALLOCATION = re.compile(r"RFV4-CLAIM-(\d{3})\Z")
_FIXTURE_ALLOCATION = re.compile(r"RFV4-FIX-(\d{3})-([CN])\Z")


def _validate_public_ids(value: object, path: str = "") -> None:
    if isinstance(value, Mapping):
        for key, child in value.items():
            _validate_public_ids(child, f"{path}.{key}" if path else key)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for child in value:
            _validate_public_ids(child, f"{path}[]")
        return
    if not isinstance(value, str):
        return
    checks = (
        ("entity:", _ENTITY_ID),
        ("fact:", _FACT_ID),
        ("workspace:", _WORKSPACE_ID),
        ("fabric-epoch:", _EPOCH_ID),
        ("codefabric-result://", _RESOURCE_URI),
        ("sha256:", _SHA256_ID),
        ("session:", _SESSION_ID),
        ("lease:", _LEASE_ID),
        ("epoch:", _EPOCH_HANDLE),
    )
    for prefix, pattern in checks:
        if value.startswith(prefix) and not pattern.fullmatch(value):
            raise V4ContractError(
                "V4_PUBLIC_ID_INVALID", f"{path} is not a canonical public ID"
            )
    if (
        _field_name(path)
        in {
            "accepted_query",
            "canonical_original_query_id",
            "cancel_query",
            "query_id",
        }
        and ".queries[]" not in path
        and not _QUERY_HANDLE.fullmatch(value)
    ):
        raise V4ContractError(
            "V4_PUBLIC_ID_INVALID", f"{path} is not a canonical query handle"
        )


def _decode_base64(
    value: object, context: str, *, exact_bytes: int | None = None
) -> bytes:
    if not isinstance(value, str):
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", f"{context} must be base64")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, TypeError) as error:
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID", f"{context} is not base64"
        ) from error
    if exact_bytes is not None and len(decoded) != exact_bytes:
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID", f"{context} must decode to {exact_bytes} bytes"
        )
    return decoded


_CURSOR_ORACLE_KEYS = frozenset(
    {
        "non_public",
        "query_id",
        "principal_session_class",
        "daemon_generation",
        "profile",
        "next_sequence",
        "preceding_event_content_sha256",
        "expires_at_unix_ms",
    }
)
_CURSOR_ORACLE_KATS = {
    "AQIDBAUGBwgJCgsMDQ4PEA==": "adecfe37223c196fa420a0647d55a61c38a0ff54c9d9ed79925e3f387f761959",
    "AQIDBAUGBwgJCgsMDQ4PEQ==": "b074901abbe46bad516cb9fb53308f89a6f849f641526556d407ead342f72e6d",
}


def _validate_cursor_oracle(
    cursor: object, oracle_value: object, context: str
) -> Mapping[str, Any]:
    """Bind an opaque cursor token to every separately authored oracle field."""

    _decode_base64(cursor, context, exact_bytes=16)
    oracle = _mapping(oracle_value, f"{context} oracle")
    _strict_keys(oracle, _CURSOR_ORACLE_KEYS, f"{context} oracle")
    if (
        oracle["non_public"] is not True
        or not _QUERY_HANDLE.fullmatch(str(oracle["query_id"]))
        or not isinstance(oracle["principal_session_class"], str)
        or not oracle["principal_session_class"]
        or type(oracle["daemon_generation"]) is not int
        or oracle["daemon_generation"] < 1
        or oracle["profile"] != "cpg.v2"
        or type(oracle["next_sequence"]) is not int
        or oracle["next_sequence"] < 1
        or not re.fullmatch(
            r"[0-9a-f]{64}", str(oracle["preceding_event_content_sha256"])
        )
        or type(oracle["expires_at_unix_ms"]) is not int
        or oracle["expires_at_unix_ms"] <= 0
    ):
        raise V4ContractError(
            "V4_CURSOR_ORACLE_INVALID", f"{context} oracle is not authoritative"
        )
    payload = json.dumps(
        {"cursor": cursor, "oracle": oracle},
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
    actual = hashlib.sha256(payload).hexdigest()
    expected = _CURSOR_ORACLE_KATS.get(str(cursor))
    if expected is None or actual != expected:
        raise V4ContractError(
            "V4_CURSOR_ORACLE_BINDING_DRIFT",
            f"{context} cursor bytes are not bound to every oracle authority field",
        )
    return oracle


_READ_CURSOR_ORACLE_KATS = {
    "IiMkJSYnKCkqKywtLi8wMQ==": "40bcd4a628d05e43bb4b380e8b1bfb84bfecfaa6346ff1f085001150034b1248",
    "MzQ1Njc4OTo7PD0+P0BBQg==": "b07f246daa538964220675a58f86bc3fcd1049ea34f32395a0d156fbc2b46a45",
}


def _validate_read_cursor_oracle(
    cursor: object, oracle_value: object, context: str
) -> Mapping[str, Any]:
    _decode_base64(cursor, context, exact_bytes=16)
    oracle = _mapping(oracle_value, f"{context} oracle")
    expected_keys = _NESTED_KEYS["rpc_read_resource"]["read_cursor_fixture_oracle"]
    _strict_keys(oracle, expected_keys, f"{context} oracle")
    _validate_public_ids(oracle, f"{context}.oracle")
    if (
        oracle["non_public"] is not True
        or type(oracle["daemon_generation"]) is not int
        or type(oracle["supervisor_generation"]) is not int
        or type(oracle["next_offset"]) is not int
        or type(oracle["end_offset_exclusive"]) is not int
        or oracle["next_offset"] < 0
        or oracle["end_offset_exclusive"] <= oracle["next_offset"]
        or type(oracle["expires_at_unix_ms"]) is not int
        or oracle["expires_at_unix_ms"] <= 0
    ):
        raise V4ContractError(
            "V4_CURSOR_ORACLE_INVALID", f"{context} oracle is not authoritative"
        )
    payload = json.dumps(
        {"cursor": cursor, "oracle": oracle},
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
    actual = hashlib.sha256(payload).hexdigest()
    if _READ_CURSOR_ORACLE_KATS.get(str(cursor)) != actual:
        raise V4ContractError(
            "V4_CURSOR_ORACLE_BINDING_DRIFT",
            f"{context} cursor bytes are not bound to every read authority field",
        )
    return oracle


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise V4ContractError(
                "V4_TYPED_SCHEMA_INVALID", f"duplicate JSON member {key!r}"
            )
        result[key] = value
    return result


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    """Load a nonblank JSONL artifact while rejecting duplicate members."""

    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise V4ContractError("V4_TYPED_INPUT_UNREADABLE", str(error)) from error
    if not lines or any(not line.strip() for line in lines):
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID", f"invalid JSONL framing: {path}"
        )
    rows: list[dict[str, Any]] = []
    for number, line in enumerate(lines, 1):
        try:
            value = json.loads(line, object_pairs_hook=_reject_duplicates)
        except (json.JSONDecodeError, UnicodeError) as error:
            raise V4ContractError(
                "V4_TYPED_INPUT_UNREADABLE", f"{path}:{number}: {error}"
            ) from error
        rows.append(dict(_mapping(value, f"{path}:{number}")))
    return rows


def _verify_frozen_release(root: Path) -> None:
    for relative_path, expected in FROZEN_SHA256.items():
        path = root / relative_path
        try:
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError as error:
            raise V4ContractError("V4_TYPED_INPUT_UNREADABLE", str(error)) from error
        if actual != expected:
            raise V4ContractError(
                "V4_R4_FREEZE_DRIFT",
                f"{relative_path} is not the immutable {EVIDENCE_RELEASE} artifact",
            )
    try:
        issuance = json.loads(
            (root / ISSUANCE_PATH).read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicates,
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise V4ContractError("V4_TYPED_INPUT_UNREADABLE", str(error)) from error
    issuance_map = _mapping(issuance, "evidence issuance")
    if issuance_map.get("evidence_release") != EVIDENCE_RELEASE:
        raise V4ContractError(
            "V4_R4_FREEZE_DRIFT", "evidence issuance release identity drifted"
        )
    digests = _mapping(issuance_map.get("artifact_digests"), "artifact_digests")
    if digests != {
        "expectations_sha256": FROZEN_SHA256[EXPECTATIONS_PATH],
        "negative_fixtures_sha256": FROZEN_SHA256[FIXTURES_PATH],
    }:
        raise V4ContractError(
            "V4_R4_FREEZE_DRIFT",
            "mutable review issuance no longer projects the frozen authored artifacts",
        )
    review = _mapping(issuance_map.get("independent_review"), "independent_review")
    if review.get("status") not in {
        "pending-independent-review",
        "accepted",
        "rejected",
        "not-accepted",
    }:
        raise V4ContractError(
            "V4_R4_FREEZE_DRIFT", "issuance review projection has an unknown status"
        )


def apply_json_merge_patch(target: object, patch: object) -> object:
    """Apply RFC 7396 JSON Merge Patch without mutating either argument."""

    if not isinstance(patch, Mapping):
        return copy.deepcopy(patch)
    result: dict[str, Any]
    if isinstance(target, Mapping):
        result = copy.deepcopy(dict(target))
    else:
        result = {}
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = apply_json_merge_patch(result.get(key), value)
    return result


class _AnchorResolver:
    """Consume a one-to-one, conflict-free identity-anchor allocation."""

    def __init__(self, inputs: Mapping[str, Any]) -> None:
        self._by_inputs: dict[str, str] = {}
        fact_ids: set[str] = set()
        for raw_anchor in _sequence(inputs["identity_anchors"], "identity_anchors"):
            anchor = _mapping(raw_anchor, "identity anchor")
            identity_inputs = _mapping(anchor.get("identity_inputs"), "identity_inputs")
            fact_id = anchor.get("fact_id")
            if not isinstance(fact_id, str) or not _FACT_ID.fullmatch(fact_id):
                raise V4ContractError(
                    "V4_PUBLIC_ID_INVALID", "identity anchor fact_id is not canonical"
                )
            key = json.dumps(
                identity_inputs,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=False,
            )
            if key in self._by_inputs or fact_id in fact_ids:
                raise V4ContractError(
                    "V4_IDENTITY_ANCHOR_CONFLICT",
                    "identity anchors must be one-to-one and conflict-free",
                )
            self._by_inputs[key] = fact_id
            fact_ids.add(fact_id)
        self._unused = set(self._by_inputs)

    def consume(self, identity_inputs: Mapping[str, Any]) -> str:
        key = json.dumps(
            identity_inputs, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        )
        if key not in self._by_inputs:
            raise V4ContractError(
                "V4_OUTPUT_ONLY_IDENTITY",
                f"no controlled identity anchor for {dict(identity_inputs)!r}",
            )
        if key not in self._unused:
            raise V4ContractError(
                "V4_IDENTITY_ANCHOR_CONFLICT", "identity anchor was consumed twice"
            )
        self._unused.remove(key)
        return self._by_inputs[key]

    def finish(self) -> None:
        if self._unused:
            raise V4ContractError(
                "V4_UNUSED_IDENTITY_ANCHOR",
                f"{len(self._unused)} controlled identity anchor(s) were unused",
            )


def _provider_rows(inputs: Mapping[str, Any]) -> dict[str, Any]:
    program = _mapping(inputs["normalization_program"], "normalization_program")
    mappings = _mapping(program["mappings"], "normalization mappings")
    transformation_id = str(program["transformation_id"])
    anchors = _AnchorResolver(inputs)
    native = []
    normalized = []
    provider_fact_ids: set[str] = set()
    for raw in _sequence(inputs["provider_native_rows"], "provider_native_rows"):
        row = _mapping(raw, "provider row")
        if row["fact_id"] in provider_fact_ids:
            raise V4ContractError(
                "V4_IDENTITY_ANCHOR_CONFLICT", "provider fact identity is duplicated"
            )
        provider_fact_ids.add(str(row["fact_id"]))
        normalized_kind = mappings.get(row.get("raw_kind"))
        if not isinstance(normalized_kind, str):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "provider raw kind has no mapping"
            )
        identity_inputs = {
            "transformation_id": transformation_id,
            "input_fact_id": row["fact_id"],
            "normalized_kind": normalized_kind,
        }
        native.append(
            {
                key: row[key]
                for key in (
                    "provider",
                    "fact_id",
                    "occurrence_id",
                    "raw_kind",
                    "authority_class",
                )
            }
        )
        normalized.append(
            {
                "fact_id": anchors.consume(identity_inputs),
                "input_fact_id": row["fact_id"],
                "occurrence_id": row["occurrence_id"],
                "raw_kind": row["raw_kind"],
                "normalized_kind": normalized_kind,
                "authority_class": "normalized",
                "transformation_id": transformation_id,
                "provenance_edges": [
                    {"kind": "NORMALIZED_FROM", "fact_id": row["fact_id"]}
                ],
            }
        )
    native.sort(key=lambda row: str(row["provider"]))
    anchors.finish()
    return {
        "outcome": "complete",
        "provider_native_rows": native,
        "normalized_rows": normalized,
        "coverage": "closed",
    }


def _provider_gaps(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if (
        inputs["family"] != inputs["requested_family"]
        or inputs["scope"] != inputs["source_image_id"]
        or inputs["provider_outcome"] not in {"compile_failed", "sidecar_unavailable"}
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "provider gap family, source-image scope, and outcome are not coupled",
        )
    return {
        "outcome": "partial",
        "facts": [],
        "provider_gaps": [
            {
                "provider": inputs["provider"],
                "family": inputs["family"],
                "scope": inputs["scope"],
                "reason": inputs["provider_outcome"],
                "reason_source": "provider_outcome",
                "current": True,
            }
        ],
        "stale_current_facts": False,
    }


def _producer_remainders(inputs: Mapping[str, Any]) -> dict[str, Any]:
    producers = list(_sequence(inputs["eligible_producers"], "eligible_producers"))
    if str(inputs["authority_rule"]).startswith("pyrefly_precedes"):
        return {
            "outcome": "complete",
            "selected_producer": "pyrefly",
            "conflicting_evidence_retained": True,
        }
    return {
        "outcome": "partial",
        "selected_producer": None,
        "remainders": [
            {
                "family": inputs["required_family"],
                "reason": "ambiguous_producer",
                "candidates": producers,
            }
        ],
        "conflicting_evidence_retained": True,
    }


def _transformations(inputs: Mapping[str, Any]) -> dict[str, Any]:
    mapping = _mapping(inputs["mapping"], "mapping")
    rows = []
    for raw in _sequence(inputs["rows"], "rows"):
        row = dict(_mapping(raw, "transformation row"))
        normalized = mapping.get(row.get("raw_kind"))
        if not isinstance(normalized, str):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "transformation input is uncovered"
            )
        row["normalized_kind"] = normalized
        rows.append(row)
    return {
        "outcome": "complete",
        "relation": "program.syntax_occurrence",
        "rows": rows,
    }


def _shortest_witnesses(
    inputs: Mapping[str, Any], anchors: _AnchorResolver
) -> list[dict[str, Any]]:
    entry = str(inputs["entry"])
    limit = int(inputs["limit"])
    adjacency: dict[str, list[tuple[str, str]]] = {}
    for raw in _sequence(inputs["edges"], "edges"):
        edge = _mapping(raw, "analysis edge")
        adjacency.setdefault(str(edge["from"]), []).append(
            (str(edge["to"]), str(edge["fact_id"]))
        )
    queue: deque[tuple[str, list[str], list[str]]] = deque([(entry, [entry], [])])
    seen = {entry}
    result = []
    while queue:
        node, entities, facts = queue.popleft()
        if len(facts) >= limit:
            continue
        for target, fact_id in adjacency.get(node, []):
            if target in seen:
                continue
            seen.add(target)
            next_entities = [*entities, target]
            next_facts = [*facts, fact_id]
            identity_inputs = {
                "analysis_algorithm_release": inputs["analysis_algorithm_release"],
                "from": entry,
                "to": target,
                "distance": len(next_facts),
                "supporting_fact_ids": next_facts,
            }
            result.append(
                {
                    "fact_id": anchors.consume(identity_inputs),
                    "from": entry,
                    "to": target,
                    "distance": len(next_facts),
                    "supporting_fact_ids": next_facts,
                    "witness_entity_ids": next_entities,
                    "provenance_edges": [
                        {"kind": "DERIVED_FROM", "fact_id": item} for item in next_facts
                    ],
                }
            )
            queue.append((target, next_entities, next_facts))
    return result


def _analyses(
    inputs: Mapping[str, Any], *, require_all_anchors: bool = True
) -> dict[str, Any]:
    anchors = _AnchorResolver(inputs)
    rows = _shortest_witnesses(inputs, anchors)
    if require_all_anchors:
        anchors.finish()
    return {
        "outcome": "complete",
        "relation": "derived.cfg_reachable",
        "analysis_algorithm_release": inputs["analysis_algorithm_release"],
        "producer_id": inputs["producer_id"],
        "producer_release": inputs["producer_release"],
        "fabric_epoch_id": inputs["fabric_epoch_id"],
        "projection_id": inputs["projection_id"],
        "precision": inputs["precision"],
        "completeness": inputs["completeness"],
        "rows": rows,
        "judgment_labels": [],
    }


def _query_find(inputs: Mapping[str, Any]) -> dict[str, Any]:
    syntax = "syntax" in str(inputs["looking_for"])
    role = "syntax_occurrence" if syntax else "semantic_declaration"
    entities = [
        str(_mapping(item, "entity")["id"])
        for item in _sequence(inputs["entities"], "entities")
        if _mapping(item, "entity").get("role") == role
    ]
    return {
        "outcome": "complete",
        "resolved_interpretation": "syntax occurrence"
        if syntax
        else "semantic declaration",
        "entity_ids": entities,
        "coverage": "closed",
    }


def _query_retrieve(inputs: Mapping[str, Any]) -> dict[str, Any]:
    subjects = _sequence(inputs["about"], "about")
    available = _mapping(inputs["available"], "available")
    unknown = _mapping(inputs["unknown"], "unknown")
    facts = []
    unknowns = []
    for subject in subjects:
        for family in _sequence(inputs["facts"], "facts"):
            if family in available:
                facts.append(
                    {"subject": subject, "family": family, "object": available[family]}
                )
            elif family in unknown:
                unknowns.append(
                    {"subject": subject, "family": family, "reason": unknown[family]}
                )
            else:
                raise V4ContractError(
                    "V4_TYPED_DERIVATION_FAILED",
                    f"unaccounted requested family {family}",
                )
    return {
        "outcome": "partial" if unknowns else "complete",
        "facts": facts,
        "unknowns": unknowns,
        "expanded_families": list(_sequence(inputs["facts"], "facts")),
    }


def _query_follow(inputs: Mapping[str, Any]) -> dict[str, Any]:
    distance = inputs["distance"]
    if not isinstance(distance, int) or isinstance(distance, bool) or distance < 1:
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "distance must be bounded")
    adjacency: dict[str, list[str]] = {}
    for raw in _sequence(inputs["edges"], "edges"):
        edge = _sequence(raw, "relationship edge")
        adjacency.setdefault(str(edge[0]), []).append(str(edge[1]))
    queue: deque[tuple[str, int]] = deque(
        (str(item), 0) for item in _sequence(inputs["starting_from"], "starting_from")
    )
    seen = {node for node, _ in queue}
    entities = []
    while queue:
        node, depth = queue.popleft()
        if depth >= distance:
            continue
        for target in adjacency.get(node, []):
            if target in seen:
                continue
            seen.add(target)
            entities.append({"id": target, "distance": depth + 1})
            queue.append((target, depth + 1))
    return {"outcome": "complete", "entities": entities, "truncated": False}


def _all_shortest_paths(inputs: Mapping[str, Any]) -> list[dict[str, Any]]:
    starts = [str(item) for item in _sequence(inputs["from"], "from")]
    targets = {str(item) for item in _sequence(inputs["to"], "to")}
    adjacency: dict[str, list[tuple[str, str]]] = {}
    for raw in _sequence(inputs["edges"], "edges"):
        edge = _mapping(raw, "path edge")
        adjacency.setdefault(str(edge["from"]), []).append(
            (str(edge["to"]), str(edge["fact_id"]))
        )
    queue: deque[tuple[list[str], list[str]]] = deque(
        [([start], []) for start in starts]
    )
    shortest: int | None = None
    paths: list[dict[str, Any]] = []
    while queue:
        entities, facts = queue.popleft()
        if shortest is not None and len(facts) > shortest:
            continue
        node = entities[-1]
        if node in targets:
            shortest = len(facts)
            paths.append({"entities": entities, "facts": facts})
            continue
        for target, fact_id in adjacency.get(node, []):
            if target not in entities:
                queue.append(([*entities, target], [*facts, fact_id]))
    paths.sort(key=lambda item: (item["entities"], item["facts"]))
    return paths


def _query_paths(inputs: Mapping[str, Any]) -> dict[str, Any]:
    paths = _all_shortest_paths(inputs)
    if inputs["path_policy"] == "shortest":
        paths = paths[:1]
    return {"outcome": "complete", "paths": paths, "ordering": "canonical_identity"}


def _query_pattern(inputs: Mapping[str, Any]) -> dict[str, Any]:
    coverage = _mapping(inputs["coverage"], "coverage")
    if not coverage.get("complete"):
        return {
            "outcome": "partial",
            "binding_status": "indeterminate",
            "reason": "incomplete_negation_universe",
        }
    pattern = _mapping(inputs["pattern"], "pattern")
    return {
        "outcome": "complete",
        "bindings": [{"node": pattern["node"], "negated_clause": "satisfied"}],
        "branch": "base",
    }


def _query_combine(inputs: Mapping[str, Any]) -> dict[str, Any]:
    raw_inputs = _sequence(inputs["inputs"], "inputs")
    relations = [_mapping(item, "result reference") for item in raw_inputs]
    workspaces = {item["workspace"] for item in relations}
    roles = {item["role"] for item in relations}
    if len(workspaces) != 1 or len(roles) != 1:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "result references have incompatible identities",
        )
    sets = [set(_sequence(item["ids"], "ids")) for item in relations]
    if inputs["operation"] == "intersection":
        ids = set.intersection(*sets)
    elif inputs["operation"] == "union":
        ids = set.union(*sets)
    else:
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "unknown combine operation")
    return {
        "outcome": "complete",
        "workspace": next(iter(workspaces)),
        "role": next(iter(roles)),
        "ids": sorted(ids),
    }


def _query_summary(inputs: Mapping[str, Any]) -> dict[str, Any]:
    groups = []
    for raw in _sequence(inputs["groups"], "groups"):
        group = _mapping(raw, "summary group")
        groups.append({"module": group["module"], "count": len(group["members"])})
    supporting = list(_sequence(inputs["supporting_fact_ids"], "supporting_fact_ids"))
    return {
        "outcome": "complete",
        "groups": groups,
        "supporting_fact_count": len(supporting),
        "judgment_labels": [],
        "producer_id": inputs["producer_id"],
        "producer_release": inputs["producer_release"],
        "precision": inputs["precision"],
        "completeness": inputs["completeness"],
        "fabric_epoch_id": inputs["fabric_epoch_id"],
        "supporting_fact_ids": supporting,
        "provenance_edges": copy.deepcopy(inputs["provenance_edges"]),
    }


def _source_context(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if inputs["authorized"] is not True:
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "source is not authorized")
    source = base64.b64decode(str(inputs["source_bytes"]), validate=True)
    limit = int(inputs["byte_limit"])
    selected = source[:limit]
    return {
        "outcome": "complete" if len(selected) == len(source) else "partial",
        "encoding": "utf-8",
        "text": selected.decode("utf-8"),
        "omitted_bytes": len(source) - len(selected),
        "source_authorized": True,
    }


def _genesis(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if inputs["profile"] != "FreshActivation" or inputs["activation_head"] != "Empty":
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "genesis must start fresh")
    return {
        "outcome": "activated",
        "command": "ActivateGenesis",
        "expected_head": "Empty",
        "append_count": 1,
        "selected_epoch": inputs["candidate"],
        "admission": "closed_until_install",
    }


def _activation_readback(inputs: Mapping[str, Any]) -> dict[str, Any]:
    event = _mapping(inputs["appended_event"], "appended_event")
    if (
        inputs["readback"] != "same_event"
        or event.get("predecessor") != inputs["expected_head"]
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "activation readback is incoherent"
        )
    return {
        "outcome": "activated",
        "selected_record": {
            "activation_id": event["activation_id"],
            "versions": copy.deepcopy(event["versions"]),
            "writer_generation": event["writer_generation"],
            "control_horizon": event["control_horizon"],
        },
        "selection_source": "exact_readback",
        "latest_guess": False,
    }


def _lifecycle(inputs: Mapping[str, Any]) -> dict[str, Any]:
    transitions = list(_sequence(inputs["transitions"], "transitions"))
    stop_at = inputs["stop_at"]
    if stop_at not in transitions:
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "stop phase is unreachable")
    if stop_at == "EndpointsBoundBootstrapping":
        return {
            "outcome": "bootstrapping",
            "status_available": True,
            "reference_available": True,
            "query_admission": False,
        }
    if stop_at != "Ready" or transitions[-1] != "Ready":
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "unsupported lifecycle phase"
        )
    return {
        "outcome": "ready",
        "active_workspace": inputs["installed_epoch"],
        "atomic_install_count": 1,
        "query_admission": [
            {"phase": "EndpointsBoundBootstrapping", "open": False},
            {"phase": "Ready", "open": True},
        ],
    }


def _recovery_pre_append(inputs: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "outcome": "recovered",
        "selected_head": inputs["selected_head"],
        "candidate_visibility": "private_discarded",
        "append_count": 0,
        "admission": "closed_during_recovery",
        "discarded_private_table_versions": inputs["private_table_versions_written"],
        "proof_receipt_observed": inputs["proof_receipt_present"],
    }


def _recovery_uncertain_append(inputs: Mapping[str, Any]) -> dict[str, Any]:
    horizon = _mapping(inputs["observed_horizon"], "observed_horizon")
    if horizon.get("candidate") is None:
        return {
            "outcome": "reconciled_predecessor_current",
            "selected_head": horizon["head"],
            "append_retry_count": 0,
            "candidate_visibility": "orphan_private",
        }
    if horizon.get("candidate") != inputs["candidate"]:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "observed candidate drifted"
        )
    return {
        "outcome": "reconciled",
        "selected_head": horizon["head"],
        "append_retry_count": 0,
        "repair": "none",
        "predecessor_restore": False,
    }


def _supervisor_policy(inputs: Mapping[str, Any]) -> dict[str, Any]:
    policy = _mapping(inputs["policy"], "policy")
    request = _mapping(inputs["launcher_request"], "launcher_request")
    policy_file = _mapping(inputs["policy_file"], "policy_file")
    peer = _mapping(inputs["launcher_peer"], "launcher_peer")
    replay = _mapping(inputs["anti_replay_registry"], "anti_replay_registry")
    observed_adapter = _mapping(inputs["observed_adapter"], "observed_adapter")
    adapter_identity = _mapping(policy["adapter_identity"], "adapter_identity")
    host_bounds = _mapping(policy["mcp_host_bounds"], "mcp_host_bounds")
    resource_bounds = _mapping(policy["resource_bounds"], "resource_bounds")
    deadline_bounds = _mapping(policy["deadline_bounds"], "deadline_bounds")
    request_time = int(request["request_at_unix_ms"])
    authorized = (
        request.get("policy_id") == policy.get("policy_id")
        and request.get("requested_workspace") in policy.get("workspaces", [])
        and request.get("requested_operation") in policy.get("operations", [])
        and request.get("supervisor_generation") == inputs["supervisor_generation"]
        and request.get("peer_uid") == peer.get("uid")
        and request.get("peer_pid") == peer.get("pid")
        and request.get("peer_start_time_ticks") == peer.get("start_time_ticks")
        and policy_file.get("opened_no_follow") is True
        and policy_file.get("owner_uid") == 0
        and policy_file.get("mode") == "0600"
        and policy_file.get("strict_schema") is True
        and policy_file.get("file_type") == "regular_file"
        and Path(str(policy_file.get("path"))).parent
        == Path(str(policy_file.get("authorized_root")))
        and int(policy_file.get("device", -1)) >= 0
        and int(policy_file.get("inode", -1)) > 0
        and int(policy["not_before_unix_ms"])
        <= request_time
        <= int(policy["expires_at_unix_ms"])
        and int(policy["issued_at_unix_ms"]) <= int(policy["not_before_unix_ms"])
        and policy.get("revocation_generation")
        == inputs["current_revocation_generation"]
        and replay.get("identity") == request.get("anti_replay_identity")
        and replay.get("seen") is False
        and adapter_identity.get("executable_observation_supported") is True
        and all(
            observed_adapter.get(key) == adapter_identity.get(key)
            for key in ("distribution", "distribution_version", "executable_sha256")
        )
        and all(type(value) is int and value > 0 for value in host_bounds.values())
        and all(type(value) is int and value > 0 for value in resource_bounds.values())
        and all(type(value) is int and value > 0 for value in deadline_bounds.values())
        and type(policy.get("max_launches")) is int
        and int(policy["max_launches"]) > 0
        and type(policy.get("revision")) is int
        and int(policy["revision"]) > 0
        and int(inputs["launch_capacity_available"]) > 0
    )
    if not authorized:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "launch policy is not authorized"
        )
    return {
        "outcome": "authorized",
        "principal": policy["principal"],
        "workspaces": copy.deepcopy(policy["workspaces"]),
        "operations": copy.deepcopy(policy["operations"]),
        "policy_revision": policy["revision"],
        "adapter_claims_authoritative": False,
        "profiles": copy.deepcopy(policy["profiles"]),
        "max_launches": policy["max_launches"],
        "grant_registered": True,
        "policy_file_verified": True,
        "peer_identity_verified": True,
        "launch_capacity_reserved": 1,
        "policy_time_window_verified": True,
        "revocation_generation": policy["revocation_generation"],
        "policy_file_identity": {
            "file_type": policy_file["file_type"],
            "device": policy_file["device"],
            "inode": policy_file["inode"],
        },
        "anti_replay_identity_accepted": True,
        "mcp_host_bounds": copy.deepcopy(host_bounds),
        "resource_bounds": copy.deepcopy(resource_bounds),
        "deadline_bounds": copy.deepcopy(deadline_bounds),
        "adapter_identity": copy.deepcopy(adapter_identity),
        "adapter_identity_verified": True,
        "strict_schema_verified": True,
    }


def _supervisor_singleton(inputs: Mapping[str, Any]) -> dict[str, Any]:
    root = _mapping(inputs["runtime_root"], "runtime_root")
    lease = _mapping(inputs["singleton_lease"], "singleton_lease")
    socket = _mapping(inputs["supervisor_socket"], "supervisor_socket")
    recovery = _mapping(inputs["socket_recovery_policy"], "socket_recovery_policy")
    attaches = _sequence(inputs["attach_requests"], "attach_requests")
    safe = (
        root.get("authorized_root") is True
        and root.get("no_symlink_components") is True
        and root.get("mode") == "0700"
        and root.get("type") == "directory"
        and int(root.get("device", -1)) >= 0
        and int(root.get("owner_uid", -1)) >= 0
        and root.get("device") == socket.get("device")
        and root.get("owner_uid") == socket.get("owner_uid")
        and lease.get("owned") is True
        and lease.get("capacity_reserved") is True
        and lease.get("winner_pid")
        == _mapping(inputs["live_supervisor"], "live_supervisor").get("daemon_pid")
        and socket.get("type") == "unix_socket"
        and socket.get("mode") == "0600"
        and socket.get("live_probe") == "authenticated"
        and socket.get("cleanup_requires_exact_identity") is True
        and socket.get("generation")
        == _mapping(inputs["live_supervisor"], "live_supervisor").get("generation")
        and int(socket.get("inode", -1)) > 0
        and Path(str(socket.get("path"))).parent == Path(str(root.get("path")))
        and all(
            recovery.get(key) is True
            for key in (
                "requires_singleton_lease",
                "requires_failed_live_probe",
                "requires_exact_type_owner_mode_device_inode_generation",
                "replacement_inode_safe_cleanup",
            )
        )
    )
    if not safe or len(attaches) > int(inputs["capacity"]):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "singleton boundary is unsafe"
        )
    return {
        "outcome": "attached",
        "supervisor_count": 1,
        "daemon_count": 1,
        "adapter_count": len(attaches),
        "grant_count": len(attaches),
        "semantic_state_copies": 1,
        "runtime_root_safe": True,
        "singleton_lease_count": 1,
        "owned_socket_count": 1,
        "losing_racer_mutations": 0,
        "owned_stale_socket_recovery_supported": True,
        "replacement_inode_safe_cleanup": True,
    }


def _supervisor_control(inputs: Mapping[str, Any]) -> dict[str, Any]:
    records = [
        _mapping(item, "control record")
        for item in _sequence(inputs["records"], "records")
    ]
    sequences = [item.get("sequence") for item in records]
    if sequences != list(range(1, len(records) + 1)):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "control sequence is not contiguous"
        )
    operation_identities = {
        "AdvanceSupervisorGeneration": "advance-supervisor-generation@1",
        "RegisterLaunchGrant": "register-launch-grant@1",
        "Acknowledgement": "acknowledgement@1",
        "RevokePrincipal": "revoke-principal@1",
    }
    for record in records:
        if (
            record.get("workspace") != inputs["workspace"]
            or record.get("daemon_generation") != inputs["daemon_generation"]
            or record.get("supervisor_generation") != inputs["supervisor_generation"]
            or operation_identities.get(str(record.get("operation")))
            != record.get("operation_identity")
            or int(record.get("declared_length_bytes", -1))
            > int(inputs["governed_max_record_bytes"])
            or int(record.get("declared_length_bytes", -1)) <= 0
            or int(record.get("expires_at_unix_ms", -1)) <= 0
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "control record binding is invalid"
            )
    channel = _mapping(inputs["channel"], "channel")
    if (
        channel.get("transport") != "unnamed_socketpair"
        or channel.get("channel_generation") != inputs["supervisor_generation"]
        or channel.get("state") != "healthy"
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "control channel binding is invalid"
        )
    grants = [
        item["grant_digest"]
        for item in records
        if item.get("operation") == "RegisterLaunchGrant"
    ]
    semantic = sum(
        1
        for item in records
        if any(key in item for key in ("query", "result", "semantic_payload"))
    )
    return {
        "outcome": "accepted",
        "next_sequence": len(records) + 1,
        "registered_grants": grants,
        "semantic_payload_records": semantic,
        "record_bindings_verified": True,
        "max_record_bytes_observed": max(
            int(item["declared_length_bytes"]) for item in records
        ),
        "channel_state": channel["state"],
    }


def _supervisor_fd3(inputs: Mapping[str, Any]) -> dict[str, Any]:
    descriptors = _mapping(inputs["child_descriptors"], "child_descriptors")
    policy = _mapping(inputs["descriptor_policy"], "descriptor_policy")
    fallback = _mapping(inputs["one_shot_fallback"], "one_shot_fallback")
    spawn = _mapping(inputs["adapter_spawn"], "adapter_spawn")
    stdio = _mapping(inputs["stdio_policy"], "stdio_policy")
    stderr = _mapping(inputs["stderr_policy"], "stderr_policy")
    platform = _mapping(
        inputs["platform_descriptor_capability"], "platform_descriptor_capability"
    )
    capability = base64.b64decode(str(inputs["capability"]), validate=True)
    replacement = inputs["replacement_capability"]
    active_capability = (
        base64.b64decode(str(replacement), validate=True)
        if replacement is not None
        else capability
    )
    fallback_safe = (
        fallback.get("authorized_root") is True
        and fallback.get("opened_no_follow") is True
        and fallback.get("owner_uid") == 1000
        and fallback.get("mode") == "0600"
        and fallback.get("path_device") == fallback.get("opened_device")
        and fallback.get("path_inode") == fallback.get("opened_inode")
        and fallback.get("unlinked_after_open") is True
        and fallback.get("unlinked_immediately_after_open") is True
        and fallback.get("max_reads") == 1
        and fallback.get("capability_logged") is False
        and fallback.get("cleanup_complete") is True
    )
    fixed_fd3 = platform.get("fixed_fd3_inheritance_available") is True
    selected_transport = platform.get("selected_transport")
    transport_safe = (
        fixed_fd3
        and platform.get("probe_status") == "passed"
        and selected_transport == "fixed_fd3"
        and fallback.get("enabled") is False
    ) or (
        not fixed_fd3
        and platform.get("probe_status") == "unavailable"
        and selected_transport == "one_shot_file"
        and fallback.get("enabled") is True
        and fallback_safe
    )
    safe = (
        descriptors.get("fd3") == "capability-channel"
        and policy.get("capability_fd") == 3
        and policy.get("allowlisted_inherited_fds") == [0, 1, 2, 3]
        and policy.get("unrelated_descriptors_close_on_exec") is True
        and inputs["argv_contains_capability"] is False
        and inputs["environment_contains_capability"] is False
        and transport_safe
        and fallback_safe
        and spawn.get("grant_registered") is True
        and spawn.get("diagnostic_tasks_joined") is True
        and spawn.get("descriptor_cleanup_complete") is True
        and spawn.get("process_group_owned") is True
        and spawn.get("stage") == "ready"
        and int(spawn.get("child_pid", -1)) > 0
        and len(active_capability) == int(inputs["capability_length_bytes"])
        and len(active_capability) <= int(inputs["capability_max_bytes"])
        and int(inputs["issued_random_bits"]) >= 256
        and policy.get("fd3_direction") == "supervisor_to_adapter_only"
        and policy.get("frame_contract") == "length_bounded_generation_labelled"
        and policy.get("fd3_channel_open_after_delivery") is True
        and policy.get("parent_writer_open_for_replacement") is True
        and policy.get("child_fd3_open_for_replacement") is True
        and policy.get("terminal_eof_observed") is False
        and stdio.get("stdin") == "direct_host"
        and stdio.get("stdout") == "direct_host"
        and stdio.get("launcher_proxy_copies") == 0
        and descriptors.get("stdin") == "host-stdin"
        and descriptors.get("stdout") == "host-stdout"
        and descriptors.get("stderr") == "bounded-pipe"
        and int(stderr["forwarded_bytes"]) == int(stderr["byte_limit"])
        and int(stderr["truncated_bytes"])
        == int(stderr["emitted_bytes"]) - int(stderr["forwarded_bytes"])
        and stderr.get("accounting_complete") is True
        and (
            replacement is None
            or (
                active_capability != capability and int(inputs["daemon_generation"]) > 4
            )
        )
    )
    if not safe:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "capability delivery is ambient or unsafe"
        )
    return {
        "outcome": "delivered",
        "read_count": 1,
        "fd": 3,
        "reusable": False,
        "unrelated_inherited_fds": [],
        "stdout_proxy_copies": 0,
        "fallback_used": selected_transport == "one_shot_file",
        "adapter_processes": 1,
        "grant_registered_before_delivery": True,
        "capability_bytes": len(active_capability),
        "capability_max_bytes": inputs["capability_max_bytes"],
        "fd3_direction": policy["fd3_direction"],
        "direct_host_stdio": True,
        "stderr_forwarded_bytes": stderr["forwarded_bytes"],
        "stderr_truncated_bytes": stderr["truncated_bytes"],
        "stderr_accounting_complete": True,
        "capability_logged": False,
        "descriptor_cleanup_complete": True,
        "frame_contract": policy["frame_contract"],
        "fd3_channel_open": True,
        "parent_writer_open_for_replacement": True,
        "child_fd3_open_for_replacement": True,
        "terminal_eof_observed": False,
        "further_handshake_authority_available": True,
        "selected_transport": selected_transport,
    }


def _supervisor_restart(inputs: Mapping[str, Any]) -> dict[str, Any]:
    invalidated = [inputs["old_grant"], inputs["old_session"], inputs["old_cursor"]]
    if inputs["event"] == "principal_policy_revocation":
        revoked = [
            _mapping(item, "revocation target")["id"]
            for item in _sequence(inputs["revocation_targets"], "revocation_targets")
        ]
        return {
            "outcome": "authority_revoked",
            "revoked": revoked,
            "invalidated": invalidated,
            "replacement_authority_required": True,
            "fresh_grant_required": True,
            "accepted_query": inputs["accepted_query"],
            "accepted_query_survives": True,
            "query_cancelled": False,
            "start_query_calls": 0,
            "watch_resume_requires_fresh_authority": True,
            "daemon_restart": False,
            "supervisor_restart": False,
            "old_daemon_pid": _mapping(
                inputs["old_daemon_process"], "old_daemon_process"
            )["pid"],
            "old_daemon_remains_owned": True,
            "fresh_grant": inputs["fresh_grant"],
            "restart_lifecycle_actions": 0,
        }
    old_process = _mapping(inputs["old_daemon_process"], "old_daemon_process")
    policy = _mapping(inputs["restart_policy"], "restart_policy")
    if (
        inputs["event"] != "supervisor_restart"
        or inputs["supervisor_restart"] is not True
        or inputs["daemon_restart"] is not False
        or int(inputs["new_supervisor_generation"])
        <= int(inputs["old_supervisor_generation"])
        or int(inputs["new_daemon_generation"]) <= int(inputs["old_daemon_generation"])
        or inputs["retained_terminal"] is not True
        or old_process.get("state") != "running"
        or old_process.get("process_group_owned") is not True
        or any(
            policy.get(key) is not True
            for key in (
                "join_lifecycle_tasks",
                "reap_owned_children",
                "cleanup_owned_socket_by_exact_identity",
                "reacquire_singleton_lease",
            )
        )
        or int(policy["drain_timeout_ms"]) <= 0
        or int(policy["kill_timeout_ms"]) <= 0
        or int(inputs["replacement_daemon_pid"]) == int(old_process["pid"])
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "supervisor restart contract is not closed"
        )
    return {
        "outcome": "supervisor_restarted",
        "old_supervisor_generation": inputs["old_supervisor_generation"],
        "new_supervisor_generation": inputs["new_supervisor_generation"],
        "old_daemon_generation": inputs["old_daemon_generation"],
        "new_daemon_generation": inputs["new_daemon_generation"],
        "invalidated": invalidated,
        "old_daemon_pid": old_process["pid"],
        "replacement_daemon_pid": inputs["replacement_daemon_pid"],
        "old_daemon_child_joined": True,
        "old_daemon_child_reaped": True,
        "old_process_group_remaining": 0,
        "lifecycle_tasks_joined": True,
        "owned_socket_cleanup_complete": True,
        "singleton_lease_reacquired": True,
        "orphan_children": 0,
        "query_resubmitted": False,
        "reconnect_action": "fresh_grant_session_then_watch_query",
        "retained_query": inputs["accepted_query"],
        "fresh_grant_required": True,
        "fresh_grant": inputs["fresh_grant"],
        "accepted_query_survives": True,
        "start_query_calls": 0,
        "watch_resume_only": True,
    }


def _rpc_handshake(inputs: Mapping[str, Any]) -> dict[str, Any]:
    issued = int(inputs["handshake_at_unix_ms"])
    policy = _mapping(inputs["handshake_policy"], "handshake_policy")
    grant = _decode_base64(inputs["grant"], "grant")
    _decode_base64(
        inputs["grant_anti_replay_identity"],
        "grant_anti_replay_identity",
        exact_bytes=32,
    )
    valid = (
        inputs["registered"] is True
        and int(inputs["grant_issued_at_unix_ms"])
        <= int(inputs["grant_not_before_unix_ms"])
        and int(inputs["grant_not_before_unix_ms"])
        <= issued
        < int(inputs["grant_expires_at_unix_ms"])
        and inputs["peer_uid"] == policy["required_peer_uid"]
        and int(inputs["grant_length_bytes"]) <= int(policy["grant_maximum_bytes"])
        and len(grant) == int(inputs["grant_length_bytes"])
        and int(policy["grant_maximum_bytes"]) > 0
        and int(policy["session_minimum_bytes"]) > 0
        and int(policy["session_ttl_ms"]) > 0
        and inputs["grant_revocation_generation"]
        == inputs["current_revocation_generation"]
        and inputs["grant_replayed"] is False
    )
    if not valid:
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "launch grant is not valid")
    result = {
        "outcome": "accepted",
        "method": "Handshake",
        "grant_consumed": True,
        "daemon_generation": inputs["daemon_generation"],
        "profile": inputs["profile"],
        "lifecycle": inputs["lifecycle"],
        "session_issued_at_unix_ms": issued,
        "session_expires_at_unix_ms": min(
            int(inputs["grant_expires_at_unix_ms"]),
            issued + int(policy["session_ttl_ms"]),
        ),
        "session_contract": {
            "wire": "opaque_binary_metadata",
            "minimum_bytes_from": "handshake_policy.session_minimum_bytes",
            "issued_new": True,
            "not_equal_to_grant": True,
        },
        "session_ttl_ms": policy["session_ttl_ms"],
        "session_minimum_bytes": policy["session_minimum_bytes"],
        "grant_revocation_generation": inputs["grant_revocation_generation"],
        "anti_replay_identity_consumed": True,
    }
    if inputs["lifecycle"] == "READY":
        result["semantic_admission"] = True
    return result


def _rpc_status(inputs: Mapping[str, Any]) -> dict[str, Any]:
    queue = _mapping(inputs["queue"], "queue")
    observation = _mapping(inputs["observation_contract"], "observation_contract")
    if any(observation.get(key) is not True for key in observation):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "status observation contract permits semantic work",
        )
    return {
        "outcome": "success",
        "method": "GetStatus",
        "lifecycle": inputs["lifecycle"],
        "running": queue["running"],
        "queued": queue["queued"],
        "current_epoch": inputs["current_epoch"],
        "semantic_registry_source": "daemon",
        "query_plans_created": 0,
        "catalog_traversals": 0,
        "provider_reads": 0,
        "lifecycle_projection_reads": 1,
        "coordinator_projection_reads": 1,
    }


def _rpc_reference(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if inputs["authorized"] is not True:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "reference is not authorized"
        )
    selector = _mapping(inputs["selector"], "selector")
    return {
        "outcome": "success",
        "method": "GetReference",
        "selector_kind": selector["kind"],
        "id": selector["id"],
        "authority_revision": inputs["daemon_revision"],
        "source": "daemon-live-reference",
    }


def _request_is_bounded(request: Mapping[str, Any]) -> bool:
    queries = _sequence(request.get("queries"), "queries")
    for raw in queries:
        query = _mapping(raw, "query")
        if query.get("path_policy") == "all paths" and query.get("bound") is None:
            return False
    return True


def _rpc_validate(inputs: Mapping[str, Any]) -> dict[str, Any]:
    request = _mapping(inputs["canonical_request"], "canonical_request")
    _decode_base64(inputs["session"], "session", exact_bytes=16)
    if (
        request["specification"] != "composable semantic CPG fact query"
        or request["version"] != "2.0"
        or _mapping(request["freshness"], "freshness")["policy"]
        != "require_current_for_targets"
        or not _mapping(request["scope"], "scope")["workspace_id"]
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "canonical query envelope is invalid"
        )
    if not _request_is_bounded(request):
        return {
            "outcome": "invalid",
            "typed_code": "UNBOUNDED_QUERY",
            "query_started": False,
            "capacity_reserved": False,
        }
    return {
        "outcome": "valid",
        "method": "ValidateQuery",
        "query_started": False,
        "capacity_reserved": False,
    }


def _rpc_start(inputs: Mapping[str, Any]) -> dict[str, Any]:
    _decode_base64(inputs["session"], "session", exact_bytes=16)
    _decode_base64(inputs["idempotency_key"], "idempotency_key", exact_bytes=16)
    if (
        not re.fullmatch(r"[0-9a-f]{32}", str(inputs["request_identity"]))
        or not _QUERY_HANDLE.fullmatch(str(inputs["canonical_original_query_id"]))
        or type(inputs["execution_budget_ms"]) is not int
        or inputs["execution_budget_ms"] <= 0
        or inputs["delivery"]
        not in {"initial", "repeat_same_key_same_normalized_operation"}
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "StartQuery authority envelope is invalid"
        )
    if inputs["delivery"] == "repeat_same_key_same_normalized_operation":
        return {
            "outcome": "accepted",
            "query_id": inputs["canonical_original_query_id"],
            "new_query_count": 0,
            "response": "original_acceptance",
            "query_id_reference": "original_acceptance.query_id",
            "new_reservation_count": 0,
        }
    capacity = _mapping(inputs["capacity"], "capacity")
    if any(int(value) <= 0 for value in capacity.values()):
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "capacity is incomplete")
    return {
        "outcome": "accepted",
        "method": "StartQuery",
        "reservation_complete": True,
        "repeat_same_operation": "original_acceptance",
        "changed_operation": "typed_conflict",
        "reserved_classes": [
            "coordinator",
            "journal",
            "idempotency",
            "task",
            "result",
            "retention",
        ],
        "query_id": inputs["canonical_original_query_id"],
    }


def _rpc_watch(inputs: Mapping[str, Any]) -> dict[str, Any]:
    _validate_cursor_oracle(
        inputs["resume_cursor"], inputs["cursor_fixture_oracle"], "resume_cursor"
    )
    if inputs["reconnect_cursor"] is not None:
        _validate_cursor_oracle(
            inputs["reconnect_cursor"],
            inputs["reconnect_cursor_fixture_oracle"],
            "reconnect_cursor",
        )
    oracle_name = (
        "reconnect_cursor_fixture_oracle"
        if inputs["reconnect_cursor"]
        else "cursor_fixture_oracle"
    )
    oracle = _mapping(inputs[oracle_name], oracle_name)
    if oracle.get("query_id") != inputs["query_id"]:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "cursor query identity drifted"
        )
    start = int(oracle["next_sequence"])
    delivered = [
        int(_mapping(item, "event")["sequence"])
        for item in _sequence(inputs["events"], "events")
        if int(_mapping(item, "event")["sequence"]) >= start
    ]
    if inputs["reconnect_cursor"]:
        return {
            "outcome": "stream",
            "delivered_sequences_after_reconnect": delivered,
            "query_restart_count": 0,
        }
    return {
        "outcome": "stream",
        "method": "WatchQuery",
        "delivered_sequences": delivered,
        "query_restart_count": 0,
        "result_bytes_in_events": 0,
        "delivery": "ordered_at_least_once",
    }


def _rpc_cancel(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if inputs["delivery"] == "repeat_after_terminal":
        return {
            "outcome": "cancelled",
            "repeat": "same_terminal",
            "additional_cancel_tasks": 0,
        }
    return {
        "outcome": "cancelled",
        "method": "CancelQuery",
        "query_id": inputs["query_id"],
        "idempotent_repeat": "same_terminal",
        "work_joined": True,
        "permits_released": True,
    }


def _rpc_read_resource(inputs: Mapping[str, Any]) -> dict[str, Any]:
    resource = _mapping(inputs["resource"], "resource")
    lease = _mapping(resource["artifact_lease"], "resource artifact lease")
    descriptor = _mapping(resource["descriptor"], "resource descriptor")
    range_authority = _mapping(inputs["range"], "range")
    session = _mapping(inputs["session_fixture_oracle"], "session fixture oracle")
    metadata = _mapping(inputs["session_metadata"], "session metadata")
    _decode_base64(metadata["session-bin"], "ReadResource session-bin", exact_bytes=16)
    cursor = _validate_read_cursor_oracle(
        inputs["read_cursor"],
        inputs["read_cursor_fixture_oracle"],
        "ReadResource read_cursor",
    )
    observations = [
        _mapping(value, "authorization observation")
        for value in _sequence(
            inputs["authorization_observations"], "authorization_observations"
        )
    ]
    start = int(range_authority["offset"])
    length = int(range_authority["length"])
    end = int(range_authority["end_offset_exclusive"])
    session_bound = (
        session["non_public"] is True
        and int(session["not_before_unix_ms"]) < int(session["expires_at_unix_ms"])
        and session["session_id"] == resource["owner_session_id"]
        and session["principal"] == resource["owner_principal"]
        and session["workspace"] == resource["owner_workspace"]
        and session["daemon_generation"] == resource["owner_daemon_generation"]
        and session["supervisor_generation"] == resource["owner_supervisor_generation"]
        and session["operations"] == ["ReadResource"]
        and isinstance(session["operation_grant_id"], str)
        and bool(session["operation_grant_id"])
        and isinstance(session["source_disclosure_policy"], str)
        and bool(session["source_disclosure_policy"])
        and cursor["resource_uri"] == resource["uri"]
        and cursor["owner_principal"] == resource["owner_principal"]
        and cursor["owner_session_id"] == resource["owner_session_id"]
        and cursor["owner_workspace"] == resource["owner_workspace"]
        and cursor["daemon_generation"] == resource["owner_daemon_generation"]
        and cursor["supervisor_generation"] == resource["owner_supervisor_generation"]
        and cursor["operation"] == "ReadResource"
        and cursor["lease_id"] == lease["lease_id"]
        and cursor["next_offset"] == start
        and cursor["end_offset_exclusive"] == end
        and cursor["resource_checksum_sha256"] == resource["checksum_sha256"]
        and cursor["source_disclosure_policy"] == session["source_disclosure_policy"]
        and lease["active"] is True
        and int(lease["not_before_unix_ms"]) < int(lease["expires_at_unix_ms"])
        and int(lease["expires_at_unix_ms"]) <= int(cursor["expires_at_unix_ms"])
        and resource["source_bearing"] is True
        and descriptor
        == {
            "kind": "OPAQUE_RESULT_RESOURCE",
            "descriptor_policy_revision": descriptor["descriptor_policy_revision"],
            "filesystem_descriptor": False,
            "object_store_descriptor": False,
            "path_descriptor": False,
        }
        and type(descriptor["descriptor_policy_revision"]) is int
        and descriptor["descriptor_policy_revision"] > 0
        and length == end - start
        and 0 <= start < end <= int(resource["byte_length"])
    )
    if not session_bound or not observations:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "ReadResource authority is not fully bound"
        )
    delivered_end = start
    previous_read_at = -1
    for expected_index, observation in enumerate(observations, 1):
        expected_offset = start + (expected_index - 1) * int(inputs["chunk_bytes"])
        if (
            observation["read_index"] != expected_index
            or observation["offset"] != expected_offset
            or observation["offset"] >= end
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED",
                "per-read authorization observation is not cursor ordered",
            )
        operation = _mapping(observation["operation"], "per-read operation")
        workspace = _mapping(observation["workspace"], "per-read workspace")
        owner = _mapping(observation["owner"], "per-read owner")
        observed_lease = _mapping(
            observation["artifact_lease"], "per-read artifact lease"
        )
        observed_cursor = _mapping(observation["cursor"], "per-read cursor")
        observed_range = _mapping(observation["range"], "per-read range")
        disclosure = _mapping(
            observation["source_disclosure"], "per-read source disclosure"
        )
        observed_descriptor = _mapping(observation["descriptor"], "per-read descriptor")
        read_at = int(observation["read_at_unix_ms"])
        if (
            observation["session_valid"] is not True
            or not previous_read_at < read_at
            or not int(session["not_before_unix_ms"])
            <= read_at
            < int(session["expires_at_unix_ms"])
            or operation
            != {
                "requested": "ReadResource",
                "grant_id": session["operation_grant_id"],
                "authorized": True,
            }
            or workspace
            != {
                "requested_workspace": session["workspace"],
                "authorized_workspace": resource["owner_workspace"],
            }
            or owner
            != {
                "principal": session["principal"],
                "session_id": session["session_id"],
                "daemon_generation": session["daemon_generation"],
                "supervisor_generation": session["supervisor_generation"],
            }
            or observed_lease != lease
            or observed_lease["active"] is not True
            or not int(observed_lease["not_before_unix_ms"])
            <= read_at
            < int(observed_lease["expires_at_unix_ms"])
            or observed_cursor["content_bound"] is not True
            or observed_cursor["resource_uri"] != resource["uri"]
            or observed_cursor["next_offset"] != observation["offset"]
            or observed_cursor["end_offset_exclusive"] != end
            or observed_cursor["resource_checksum_sha256"]
            != resource["checksum_sha256"]
            or _SHA256_ID.fullmatch(
                str(observed_cursor["preceding_chunk_content_sha256"])
            )
            is None
            or observed_range
            != {
                "start_offset": start,
                "end_offset_exclusive": end,
                "current_offset": observation["offset"],
                "authorized": True,
            }
            or disclosure
            != {
                "policy": session["source_disclosure_policy"],
                "source_bearing": resource["source_bearing"],
                "authorized": True,
            }
            or observed_descriptor != descriptor
            or observation["current_revocation_generation"]
            != session["revocation_generation"]
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED",
                "per-read authorization observation is not fully and exactly bound",
            )
        previous_read_at = read_at
        delivered_end = min(end, observation["offset"] + int(inputs["chunk_bytes"]))
    if delivered_end != end:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "authorized observations do not cover the range",
        )
    return {
        "outcome": "stream",
        "method": "ReadResource",
        "resource": {
            "uri": resource["uri"],
            "selector": copy.deepcopy(resource["selector"]),
        },
        "range": copy.deepcopy(range_authority),
        "bytes_delivered": length,
        "authorization_checks": len(observations),
        "authorized_operation": "ReadResource",
        "operation_grant_id": session["operation_grant_id"],
        "authorized_workspace": session["workspace"],
        "owner_binding": {
            "principal": session["principal"],
            "session_id": session["session_id"],
            "daemon_generation": session["daemon_generation"],
            "supervisor_generation": session["supervisor_generation"],
        },
        "artifact_lease": copy.deepcopy(lease),
        "content_binding": {
            "resource_checksum_sha256": resource["checksum_sha256"],
            "cursor_preceding_chunk_content_sha256": cursor[
                "preceding_chunk_content_sha256"
            ],
            "range_start_offset": start,
            "range_end_offset_exclusive": end,
        },
        "source_disclosure_policy": session["source_disclosure_policy"],
        "descriptor_policy": copy.deepcopy(descriptor),
        "per_read_authorization": copy.deepcopy(observations),
        "verified_bindings": [
            "operation",
            "principal",
            "session",
            "workspace",
            "daemon_generation",
            "supervisor_generation",
            "resource_owner",
            "resource_identity",
            "cursor_content",
            "range",
            "artifact_lease",
            "source_disclosure_policy",
        ],
        "final_cursor_offset": end,
        "independently_decodable": True,
        "filesystem_location_exposed": False,
        "object_store_location_exposed": False,
        "path_descriptor_exposed": False,
        "whole_relation_materialized": False,
        "max_in_flight_bytes": min(
            int(inputs["transport_window_bytes"]), int(inputs["consumer_credit_bytes"])
        ),
        "max_resident_bytes": inputs["resident_buffer_limit_bytes"],
        "producer_intervention": "pause_until_credit",
        "transport_backpressure_observed": True,
    }


def _rpc_release_resource(inputs: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "outcome": "released",
        "method": "ReleaseResource",
        "resource_id": inputs["resource_id"],
        "lease_count": 0,
        "repeat": "same_released_result",
        "tombstone_ms": inputs["tombstone_window_ms"],
    }


def _wire_common(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if inputs["package"] != "codefabric.cpgd.v2" or set(inputs["metadata"]) != {
        "session-bin"
    }:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "wire authority is not v2 metadata"
        )
    metadata = _mapping(inputs["metadata"], "metadata")
    _decode_base64(metadata["session-bin"], "session-bin", exact_bytes=16)
    budget = _mapping(inputs["execution_budget"], "execution_budget")
    if (
        list(_sequence(inputs["body_authority_fields"], "body_authority_fields"))
        or type(budget["seconds"]) is not int
        or budget["seconds"] < 0
        or type(budget["nanos"]) is not int
        or not 0 <= budget["nanos"] < 1_000_000_000
        or budget["seconds"] == budget["nanos"] == 0
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "wire authority is duplicated or unbounded"
        )
    required = {
        "query",
        "principal_session_class",
        "daemon_generation",
        "profile",
        "next_sequence",
        "preceding_event_content",
        "expiry",
    }
    if set(_sequence(inputs["cursor_bindings"], "cursor_bindings")) != required:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "cursor is not content bound"
        )
    return {
        "outcome": "valid_wire",
        "method_count": 9,
        "session_location": "binary_metadata",
        "absolute_deadline_authority": False,
        "cursor_content_bound": True,
        "compatibility_prose_fields": 0,
    }


def _wire_errors(inputs: Mapping[str, Any]) -> dict[str, Any]:
    outer_contract = {
        "expired_session": ("UNAUTHENTICATED", "SESSION_EXPIRED"),
    }
    for raw_case in _sequence(inputs["outer_cases"], "outer_cases"):
        case = _mapping(raw_case, "outer error case")
        if outer_contract.get(str(case["condition"])) != (
            case["grpc_status"],
            case["typed_code"],
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "outer error classification is invalid"
            )
    semantic = _mapping(inputs["semantic_case"], "semantic_case")
    semantic_contract = {
        "provider_gap": ("OK", "PARTIAL", 1, 0),
        "ambiguous_producer": ("OK", "PARTIAL", 0, 1),
    }
    if semantic_contract.get(str(semantic.get("condition"))) != (
        semantic.get("grpc_status"),
        semantic.get("availability"),
        semantic.get("unknown_count"),
        semantic.get("remainder_count"),
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "semantic error authority does not match its exact typed classification",
        )
    return {
        "outcome": "classified",
        "python_branch_source": "grpc_status_plus_typed_code",
        "prose_branching": False,
        "semantic_gap_transport_success": semantic.get("grpc_status") == "OK",
        "unknown_count": semantic.get("unknown_count", 0),
        "grpc_status": semantic["grpc_status"],
        "availability": semantic["availability"],
        "remainder_count": semantic.get("remainder_count", 0),
    }


def _mcp_query(inputs: Mapping[str, Any]) -> dict[str, Any]:
    resource = _mapping(inputs["result_resource"], "result_resource")
    progress = []
    for raw_event in _sequence(inputs["daemon_events"], "daemon_events"):
        event = _mapping(raw_event, "daemon event")
        if event["kind"] == "Progress":
            progress.append(
                {
                    key: event[key]
                    for key in (
                        "current_query_id",
                        "phase",
                        "completed_units",
                        "total_units",
                        "safe_message",
                    )
                }
            )
    return {
        "outcome": "tool_success",
        "tool": inputs["tool"],
        "rpc_sequence": ["StartQuery", "WatchQuery"],
        "resource_links": [copy.deepcopy(resource)],
        "python_semantic_execution": False,
        "progress_observations": progress,
        "progress_report_count": len(progress),
        "python_progress_synthesis": False,
    }


def _mcp_validate(inputs: Mapping[str, Any]) -> dict[str, Any]:
    validation = _mapping(inputs["daemon_validation"], "daemon_validation")
    request = _mapping(inputs["request"], "request")
    if (
        request["specification"] != "composable semantic CPG fact query"
        or request["version"] != "2.0"
        or _mapping(request["freshness"], "freshness")["policy"]
        != "require_current_for_targets"
        or not _WORKSPACE_ID.fullmatch(
            str(_mapping(request["scope"], "scope")["workspace_id"])
        )
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "MCP validation request envelope is invalid"
        )
    structurally_valid = all(
        "relationship" in _mapping(query, "query")
        for query in _sequence(request.get("queries"), "queries")
    )
    if validation.get("valid") is not structurally_valid:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "daemon validation does not correspond to the controlled request",
        )
    if structurally_valid and validation.get("typed_code") is not None:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "valid daemon result carries an error code"
        )
    if (
        not structurally_valid
        and validation.get("typed_code") != "INVALID_REQUEST_SCHEMA"
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "invalid daemon result lacks its typed code"
        )
    if validation.get("valid") is False:
        return {
            "outcome": "tool_result",
            "valid": False,
            "typed_code": validation["typed_code"],
            "start_query_calls": 0,
        }
    return {
        "outcome": "tool_success",
        "tool": inputs["tool"],
        "rpc_sequence": ["ValidateQuery"],
        "start_query_calls": 0,
        "strict_model": "frozen_extra_forbid",
    }


def _mcp_status(inputs: Mapping[str, Any]) -> dict[str, Any]:
    status = _mapping(inputs["daemon_status"], "daemon_status")
    result = {
        "outcome": "tool_success",
        "tool": inputs["tool"],
        "lifecycle": status["lifecycle"],
        "ready": status["ready"],
        "source": "GetStatus",
        "local_ready_default": False,
    }
    if status["lifecycle"] == "READY":
        result = {
            "outcome": "tool_success",
            "lifecycle": "READY",
            "ready": True,
            "running": _mapping(status["queue"], "queue")["running"],
        }
    return result


def _mcp_reference(inputs: Mapping[str, Any]) -> dict[str, Any]:
    reference = _mapping(inputs["daemon_reference"], "daemon_reference")
    return {
        "outcome": "tool_success",
        "tool": inputs["tool"],
        "rpc_sequence": ["GetReference"],
        "revision": reference["revision"],
        "availability": reference["availability"],
        "remainder": reference.get("remainder"),
        "python_static_source": False,
    }


def _mcp_lifespan(inputs: Mapping[str, Any]) -> dict[str, Any]:
    if not (
        inputs["channel_ready"] is True
        and inputs["handshake"] == "accepted"
        and inputs["profile_reference_valid"] is True
    ):
        raise V4ContractError("V4_TYPED_DERIVATION_FAILED", "lifespan is not ready")
    resource = _mapping(inputs["resource"], "resource")
    pages = int(resource["requested_live_pages"])
    if pages != 1:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "host may materialize one page"
        )
    return {
        "outcome": "available",
        "lifespan_yield_after_handshake": True,
        "materialized_pages": 1,
        "materialized_bytes": resource["byte_length"],
        "arrow_interpreted_by_python": False,
        "reported_progress_events": min(
            int(inputs["progress_events"]), int(inputs["progress_bound"])
        ),
        "stdout": "protocol_only",
        "resource": {
            "uri": resource["uri"],
            "selector": copy.deepcopy(resource["selector"]),
        },
        "requested_live_pages": pages,
    }


def _recovery_bounds(inputs: Mapping[str, Any]) -> dict[str, Any]:
    limits = _mapping(inputs["limits"], "limits")
    load = _mapping(inputs["load"], "load")
    running = min(int(load["queries"]), int(limits["running_queries"]))
    active = max(
        0,
        int(load["leased_resources"])
        - int(load["released_resources"])
        - int(load["expired_leases"]),
    )
    result = {
        "outcome": "bounded",
        "running_queries": running,
        "queued_queries": int(load["queries"]) - running,
        "journal_events_max": limits["journal_events"],
        "resident_page_bytes_max": int(limits["page_bytes"])
        * int(limits["resident_page_buffers"]),
        "active_leases": active,
        "released_leases": load["released_resources"],
        "expired_leases": load["expired_leases"],
        "tombstones": load["repeat_releases"],
        "adapter_processes_max": limits["adapter_processes"],
        "adapter_children_reaped": load["adapter_exits"],
        "cancelled_work_joined": True,
        "permits_released": True,
        "orphan_children": 0,
    }
    if int(load["repeat_releases"]):
        result["tombstone_window_ms"] = limits["tombstone_ms"]
    return result


def _zero_state(inputs: Mapping[str, Any]) -> dict[str, Any]:
    surfaces = _mapping(inputs["target_surfaces"], "target_surfaces")
    expected_surfaces = {
        "proto_packages": ["codefabric.cpgd.v2"],
        "daemon_targets": ["codefabricd"],
        "supervisors": ["WorkspaceSupervisor"],
        "mcp_catalog": [
            "query_code_graph",
            "validate_code_graph_query",
            "get_code_graph_status",
            "get_code_graph_reference",
        ],
    }
    required = list(
        _sequence(inputs["required_coverage_dimensions"], "required dimensions")
    )
    expected_dimensions = [
        "path",
        "text",
        "syntax",
        "cargo",
        "python_package",
        "generated_include",
        "recipe",
        "workflow",
        "service",
        "rule",
        "fixture",
        "ignored_source",
        "installed_artifact",
    ]
    coverage = _mapping(inputs["coverage_dimensions"], "coverage_dimensions")
    if required != expected_dimensions or dict(surfaces) != expected_surfaces:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "zero-state census allocation drifted"
        )
    inventory = [
        _mapping(item, "census candidate")
        for item in _sequence(inputs["candidate_inventory"], "candidate_inventory")
    ]
    inventory_by_id = {str(item["candidate_id"]): item for item in inventory}
    allowed_dispositions = {
        "v2_target_authority": "retain_target_authority",
        "authorized_non_authority_exclusion": "exclude_non_authority",
        "released_wire_allocation_history": "retain_immutable_history",
        "superseded_suite_history": "retain_immutable_history",
    }
    if (
        len(inventory) != 19
        or len(inventory_by_id) != len(inventory)
        or any(item["dimension"] not in required for item in inventory)
        or any(
            allowed_dispositions.get(str(item["classification"])) != item["disposition"]
            for item in inventory
        )
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "zero-state candidate inventory is duplicate, unclassified, or undisposed",
        )
    missing = [dimension for dimension in required if dimension not in coverage]
    if missing:
        covered_ids = {
            str(candidate_id)
            for dimension, observation_value in coverage.items()
            if dimension in required
            for candidate_id in _sequence(
                _mapping(observation_value, f"coverage {dimension}")["candidate_ids"],
                f"coverage {dimension} candidate_ids",
            )
        }
        return {
            "outcome": "incomplete_census",
            "coverage_complete": False,
            "missing_dimensions": missing,
            "observed_dimension_count": len(coverage),
            "covered_candidate_ids": sorted(covered_ids),
            "uncovered_candidate_ids": sorted(set(inventory_by_id) - covered_ids),
            "zero_outcomes_issued": False,
            "sole_target_authority": False,
        }
    if set(coverage) != set(required):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "zero-state census has unknown dimensions"
        )
    totals = {
        "candidate_count": 0,
        "classified_count": 0,
        "retain_target_count": 0,
        "retain_history_count": 0,
        "exclude_non_authority_count": 0,
        "parse_error_count": 0,
        "skipped_count": 0,
        "unreadable_count": 0,
        "unparsed_count": 0,
        "overlapping_count": 0,
        "count_incoherent_count": 0,
        "unknown_count": 0,
        "unmatched_count": 0,
    }
    observed_candidate_ids: list[str] = []
    dimension_disposition_counts: dict[str, dict[str, int]] = {}
    for dimension in required:
        observation = _mapping(coverage[dimension], f"coverage {dimension}")
        candidate_ids = [
            str(candidate_id)
            for candidate_id in _sequence(
                observation["candidate_ids"], f"coverage {dimension} candidate_ids"
            )
        ]
        disposition_counts = _mapping(
            observation["disposition_counts"],
            f"coverage {dimension} disposition_counts",
        )
        if any(
            type(observation[key]) is not int or observation[key] < 0
            for key in totals
            if key
            not in {
                "retain_target_count",
                "retain_history_count",
                "exclude_non_authority_count",
            }
        ) or any(
            type(disposition_counts[key]) is not int or disposition_counts[key] < 0
            for key in (
                "retain_target_count",
                "retain_history_count",
                "exclude_non_authority_count",
            )
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "coverage counters are not nonnegative"
            )
        if (
            len(candidate_ids) != len(set(candidate_ids))
            or any(
                candidate_id not in inventory_by_id for candidate_id in candidate_ids
            )
            or any(
                inventory_by_id[candidate_id]["dimension"] != dimension
                for candidate_id in candidate_ids
            )
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED",
                "coverage candidate allocation is unknown or overlapping",
            )
        derived_disposition_counts = {
            "retain_target_count": sum(
                inventory_by_id[candidate_id]["disposition"]
                == "retain_target_authority"
                for candidate_id in candidate_ids
            ),
            "retain_history_count": sum(
                inventory_by_id[candidate_id]["disposition"]
                == "retain_immutable_history"
                for candidate_id in candidate_ids
            ),
            "exclude_non_authority_count": sum(
                inventory_by_id[candidate_id]["disposition"] == "exclude_non_authority"
                for candidate_id in candidate_ids
            ),
        }
        if (
            observation["candidate_count"] != len(candidate_ids)
            or observation["classified_count"] != len(candidate_ids)
            or dict(disposition_counts) != derived_disposition_counts
            or sum(disposition_counts.values()) != len(candidate_ids)
        ):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED",
                "coverage candidate and disposition accounting is incoherent",
            )
        bad_counters = (
            "parse_error_count",
            "skipped_count",
            "unreadable_count",
            "unparsed_count",
            "overlapping_count",
            "count_incoherent_count",
            "unknown_count",
            "unmatched_count",
        )
        if any(observation[key] != 0 for key in bad_counters):
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", "zero-state census is not exhaustive"
            )
        observed_candidate_ids.extend(candidate_ids)
        dimension_disposition_counts[dimension] = dict(disposition_counts)
        totals["candidate_count"] += observation["candidate_count"]
        totals["classified_count"] += observation["classified_count"]
        for key in derived_disposition_counts:
            totals[key] += disposition_counts[key]
        for key in bad_counters:
            totals[key] += observation[key]
    if len(observed_candidate_ids) != len(set(observed_candidate_ids)) or set(
        observed_candidate_ids
    ) != set(inventory_by_id):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED",
            "candidate inventory is not classified exactly once across the census",
        )
    prohibited = _mapping(
        inputs["prohibited_live_authority_classes"], "prohibited authority classes"
    )
    expected_classes = {
        "bootstrap_backend",
        "cutover_controller",
        "ontology_backend",
        "translator",
        "v1_route",
    }
    if set(prohibited) != expected_classes:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "prohibited authority allocation drifted"
        )
    if any(
        _mapping(value, "prohibited authority")["live_count"] != 0
        or _mapping(value, "prohibited authority")["unclassified_count"] != 0
        for value in prohibited.values()
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "predecessor runtime authority remains live"
        )
    history = [
        _mapping(item, "retained history")
        for item in _sequence(inputs["retained_history"], "retained_history")
    ]
    expected_history = {
        "codefabric.cpgd.v1.proto-source-allocation": (
            "released_wire_allocation_history",
            "census:path:history-v1-proto-source",
        ),
        "codefabric.cpgd.v1.descriptor-allocation": (
            "released_wire_allocation_history",
            "census:path:history-v1-descriptor",
        ),
        "codefabric.cpgd.v1.fixture-allocation": (
            "released_wire_allocation_history",
            "census:path:history-v1-fixture",
        ),
        "codefabric-relational-data-fabric@2.1.0": (
            "superseded_suite_history",
            "census:path:history-suite-2-1",
        ),
        "codefabric-relational-data-fabric@2.0.0": (
            "superseded_suite_history",
            "census:path:history-suite-2-0",
        ),
        "codefabric-relational-data-fabric@1.3.0": (
            "superseded_suite_history",
            "census:path:history-suite-1-3",
        ),
    }
    if (
        len(history) != 6
        or len({item["artifact_id"] for item in history}) != len(history)
        or {item["artifact_id"] for item in history} != set(expected_history)
        or any(
            (
                item["classification"],
                item["candidate_id"],
            )
            != expected_history[item["artifact_id"]]
            or item["retention"] != "immutable_non_live"
            or item["selectable"] is not False
            or item["live_reader_count"] != 0
            or inventory_by_id[item["candidate_id"]]["subject"] != item["artifact_id"]
            or inventory_by_id[item["candidate_id"]]["classification"]
            != item["classification"]
            or inventory_by_id[item["candidate_id"]]["disposition"]
            != "retain_immutable_history"
            for item in history
        )
    ):
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", "retained history is live or unclassified"
        )
    classification_counts = {
        classification: sum(
            item["classification"] == classification for item in history
        )
        for classification in sorted(
            {
                classification
                for classification, _candidate_id in expected_history.values()
            }
        )
    }
    return {
        "outcome": "sole_target_authority",
        "sole_target_authority": True,
        "coverage_complete": True,
        "observed_coverage_dimensions": required,
        "candidate_dispositions": copy.deepcopy(inventory),
        "dimension_disposition_counts": dimension_disposition_counts,
        "coverage_totals": totals,
        "prohibited_live_authority_classes": copy.deepcopy(dict(prohibited)),
        "zero_outcomes": {
            "live_v1_routes": prohibited["v1_route"]["live_count"],
            "translators": prohibited["translator"]["live_count"],
            "bootstrap_backends": prohibited["bootstrap_backend"]["live_count"],
            "ontology_backends": prohibited["ontology_backend"]["live_count"],
            "cutover_controllers": prohibited["cutover_controller"]["live_count"],
            "unclassified_live_authority": sum(
                value["unclassified_count"] for value in prohibited.values()
            ),
        },
        "retained_history_count": len(history),
        "retained_history": copy.deepcopy(history),
        "retained_history_live_readers": sum(
            item["live_reader_count"] for item in history
        ),
        "retained_history_classification_counts": classification_counts,
        "forward_repair_only": True,
    }


_DERIVERS: dict[str, Callable[[Mapping[str, Any]], dict[str, Any]]] = {
    "provider_rows": _provider_rows,
    "provider_gaps": _provider_gaps,
    "producer_remainders": _producer_remainders,
    "transformations": _transformations,
    "analyses": _analyses,
    "query_find_code_entities": _query_find,
    "query_retrieve_facts": _query_retrieve,
    "query_follow_relationships": _query_follow,
    "query_connecting_paths": _query_paths,
    "query_match_pattern": _query_pattern,
    "query_combine_results": _query_combine,
    "query_summarize_facts": _query_summary,
    "query_source_context": _source_context,
    "genesis": _genesis,
    "activation_readback": _activation_readback,
    "lifecycle": _lifecycle,
    "recovery_pre_append": _recovery_pre_append,
    "recovery_uncertain_append": _recovery_uncertain_append,
    "supervisor_policy": _supervisor_policy,
    "supervisor_singleton_multi_agent": _supervisor_singleton,
    "supervisor_control": _supervisor_control,
    "supervisor_fd3": _supervisor_fd3,
    "supervisor_restart_revocation": _supervisor_restart,
    "rpc_handshake": _rpc_handshake,
    "rpc_get_status": _rpc_status,
    "rpc_get_reference": _rpc_reference,
    "rpc_validate_query": _rpc_validate,
    "rpc_start_query": _rpc_start,
    "rpc_watch_query": _rpc_watch,
    "rpc_cancel_query": _rpc_cancel,
    "rpc_read_resource": _rpc_read_resource,
    "rpc_release_resource": _rpc_release_resource,
    "wire_session_budget_cursor": _wire_common,
    "wire_errors": _wire_errors,
    "mcp_query": _mcp_query,
    "mcp_validate": _mcp_validate,
    "mcp_status": _mcp_status,
    "mcp_reference": _mcp_reference,
    "mcp_lifespan_resources": _mcp_lifespan,
    "recovery_resource_bounds": _recovery_bounds,
    "forward_only_zero_state": _zero_state,
}


def _causal_decoded(
    family: str,
    base: Mapping[str, Any],
    patched: Mapping[str, Any],
) -> dict[str, Any]:
    """Derive the deliberately small distinguishing observation for one family."""

    derived = (
        _analyses(patched, require_all_anchors=False)
        if family == "analyses"
        else _DERIVERS[family](patched)
    )
    if family == "provider_rows":
        base_ids = {
            _mapping(item, "provider row")["fact_id"]
            for item in _sequence(base["provider_native_rows"], "provider_native_rows")
        }
        changed = next(
            row
            for row in _sequence(
                derived["provider_native_rows"], "provider_native_rows"
            )
            if _mapping(row, "provider row")["fact_id"] not in base_ids
        )
        changed_row = _mapping(changed, "changed provider row")
        normalized = next(
            row
            for row in _sequence(derived["normalized_rows"], "normalized_rows")
            if _mapping(row, "normalized row")["input_fact_id"]
            == changed_row["fact_id"]
        )
        normalized_row = _mapping(normalized, "changed normalized row")
        unchanged_native = next(
            row
            for row in _sequence(
                derived["provider_native_rows"], "provider_native_rows"
            )
            if _mapping(row, "provider row")["fact_id"] in base_ids
        )
        unchanged_native_row = _mapping(unchanged_native, "unchanged provider row")
        unchanged_normalized = next(
            row
            for row in _sequence(derived["normalized_rows"], "normalized_rows")
            if _mapping(row, "normalized row")["input_fact_id"]
            == unchanged_native_row["fact_id"]
        )
        return {
            "outcome": "complete",
            "changed_provider_native_row": {
                key: changed_row[key]
                for key in ("provider", "fact_id", "raw_kind", "authority_class")
            },
            "changed_normalized_row": {
                key: normalized_row[key]
                for key in (
                    "fact_id",
                    "input_fact_id",
                    "raw_kind",
                    "normalized_kind",
                    "authority_class",
                    "transformation_id",
                    "provenance_edges",
                )
            },
            "unchanged_provider_native_fact_id": unchanged_native_row["fact_id"],
            "unchanged_normalized_fact_id": _mapping(
                unchanged_normalized, "unchanged normalized row"
            )["fact_id"],
            "coverage": "closed",
        }
    if family == "provider_gaps":
        return derived
    if family == "transformations":
        row = _mapping(_sequence(derived["rows"], "rows")[0], "row")
        return {
            "outcome": "complete",
            "row": {
                "occurrence_id": row["occurrence_id"],
                "normalized_kind": row["normalized_kind"],
            },
        }
    if family == "query_find_code_entities":
        return {
            key: derived[key]
            for key in ("outcome", "resolved_interpretation", "entity_ids")
        }
    if family == "query_retrieve_facts":
        return derived
    if family == "query_follow_relationships":
        return {"outcome": derived["outcome"], "entities": derived["entities"]}
    if family == "query_source_context":
        return {key: derived[key] for key in ("outcome", "text", "omitted_bytes")}
    if family == "genesis":
        return {
            "outcome": derived["outcome"],
            "append_count": derived["append_count"],
            "selected_epoch": derived["selected_epoch"],
            "duplicate_result": "original_acknowledgement",
        }
    if family == "activation_readback":
        return {
            key: derived[key]
            for key in ("outcome", "selected_record", "selection_source")
        }
    if family == "recovery_pre_append":
        return derived
    if family == "supervisor_singleton_multi_agent":
        exit_agent = patched["agent_exit"]
        agents = [
            _mapping(item, "attach request")["agent"]
            for item in _sequence(patched["attach_requests"], "attach_requests")
        ]
        surviving = next(agent for agent in agents if agent != exit_agent)
        return {
            "outcome": "running",
            "supervisor_count": 1,
            "daemon_count": 1,
            "adapter_count": len(agents) - 1,
            "surviving_agent": surviving,
            "singleton_lease_count": 1,
            "owned_socket_count": 1,
            "owned_socket_retained": True,
            "owned_stale_socket_recovery_supported": derived[
                "owned_stale_socket_recovery_supported"
            ],
            "replacement_inode_safe_cleanup": derived["replacement_inode_safe_cleanup"],
        }
    if family == "supervisor_control":
        records = [
            _mapping(item, "control record")
            for item in _sequence(patched["records"], "records")
        ]
        return {
            "outcome": "accepted",
            "next_sequence": len(records) + 1,
            "revoked": [
                item["principal"]
                for item in records
                if item.get("operation") == "RevokePrincipal"
            ],
            "registered_grants": [
                item["grant_digest"]
                for item in records
                if item.get("operation") == "RegisterLaunchGrant"
            ],
            "semantic_payload_records": 0,
            "record_bindings_verified": derived["record_bindings_verified"],
            "max_record_bytes_observed": derived["max_record_bytes_observed"],
            "channel_state": derived["channel_state"],
        }
    if family == "supervisor_fd3":
        replacement = _decode_base64(
            patched["replacement_capability"], "replacement_capability"
        )
        return {
            "outcome": "delivered",
            "replacement_frame_daemon_generation": patched["daemon_generation"],
            "replacement_capability_bytes": len(replacement),
            "replacement_read_count": 1,
            "old_capability_valid": False,
            "fd": derived["fd"],
            "direct_host_stdio": derived["direct_host_stdio"],
            "stderr_forwarded_bytes": derived["stderr_forwarded_bytes"],
            "stderr_truncated_bytes": derived["stderr_truncated_bytes"],
            "stderr_accounting_complete": derived["stderr_accounting_complete"],
            "capability_logged": derived["capability_logged"],
            "descriptor_cleanup_complete": derived["descriptor_cleanup_complete"],
            "frame_contract": derived["frame_contract"],
            "fd3_channel_open": derived["fd3_channel_open"],
            "parent_writer_open_for_replacement": derived[
                "parent_writer_open_for_replacement"
            ],
            "child_fd3_open_for_replacement": derived["child_fd3_open_for_replacement"],
            "terminal_eof_observed": derived["terminal_eof_observed"],
            "further_handshake_authority_available": derived[
                "further_handshake_authority_available"
            ],
            "selected_transport": derived["selected_transport"],
        }
    if family == "rpc_start_query":
        return {
            key: derived[key]
            for key in (
                "outcome",
                "query_id",
                "new_query_count",
                "response",
                "new_reservation_count",
            )
        }
    if family == "rpc_get_status":
        return {
            "outcome": derived["outcome"],
            "running": derived["running"],
            "queued": derived["queued"],
            "source": "QueryCoordinator",
            "query_plans_created": 0,
            "catalog_traversals": 0,
            "provider_reads": 0,
            "lifecycle_projection_reads": 1,
            "coordinator_projection_reads": 1,
        }
    if family == "rpc_get_reference":
        return {
            key: derived[key]
            for key in ("outcome", "selector_kind", "id", "authority_revision")
        }
    if family == "rpc_read_resource":
        return derived
    if family == "rpc_release_resource":
        return {key: derived[key] for key in ("outcome", "repeat", "lease_count")}
    if family == "wire_session_budget_cursor":
        budget = _mapping(patched["execution_budget"], "execution_budget")
        return {
            "outcome": "valid_wire",
            "relative_budget_ms": int(budget["seconds"]) * 1000
            + int(budget["nanos"]) // 1_000_000,
            "absolute_deadline_authority": False,
        }
    if family == "wire_errors":
        semantic = _mapping(patched["semantic_case"], "semantic_case")
        return {
            "outcome": "classified",
            "grpc_status": semantic["grpc_status"],
            "availability": semantic["availability"],
            "remainder_count": semantic["remainder_count"],
            "semantic_condition": semantic["condition"],
            "unknown_count": semantic["unknown_count"],
        }
    if family == "mcp_query":
        return {
            "outcome": "tool_success",
            "resource_links": derived["resource_links"],
            "progress_observations": derived["progress_observations"],
            "progress_report_count": derived["progress_report_count"],
            "rpc_sequence": ["StartQuery", "WatchQuery"],
            "python_semantic_execution": False,
            "python_progress_synthesis": False,
        }
    if family == "mcp_reference":
        return {
            key: derived[key]
            for key in ("outcome", "revision", "availability", "remainder")
        }
    if family == "mcp_lifespan_resources":
        return {
            key: derived[key]
            for key in (
                "outcome",
                "resource",
                "materialized_pages",
                "materialized_bytes",
                "reported_progress_events",
                "lifespan_yield_after_handshake",
            )
        }
    if family == "forward_only_zero_state":
        return derived
    return derived


_SIMPLE_NEGATIVE_CODES = {
    "RFV4-CLAIM-001": "PROVIDER_BATCH_CONFLICT",
    "RFV4-CLAIM-002": "FALSE_CLOSED_COVERAGE",
    "RFV4-CLAIM-003": "AMBIGUOUS_PRODUCER",
    "RFV4-CLAIM-004": "TRANSFORMATION_INPUT_UNCOVERED",
    "RFV4-CLAIM-005": "NOT_OBJECTIVE_FACT_REQUEST",
    "RFV4-CLAIM-006": "NOT_OBJECTIVE_FACT_REQUEST",
    "RFV4-CLAIM-007": "REQUESTED_FAMILY_UNACCOUNTED",
    "RFV4-CLAIM-008": "INVALID_REQUEST_SCHEMA",
    "RFV4-CLAIM-009": "UNBOUNDED_QUERY",
    "RFV4-CLAIM-010": "NEGATIVE_PROOF_INDETERMINATE",
    "RFV4-CLAIM-011": "INCOMPATIBLE_RESULT_REFERENCE",
    "RFV4-CLAIM-012": "NOT_OBJECTIVE_FACT_REQUEST",
    "RFV4-CLAIM-013": "SOURCE_ACCESS_DENIED",
    "RFV4-CLAIM-014": "SOLE_MUTATION_AUTHORITY_REQUIRED",
    "RFV4-CLAIM-015": "INCOHERENT_ACTIVATION_HORIZON",
    "RFV4-CLAIM-016": "INVALID_LIFECYCLE_TRANSITION",
    "RFV4-CLAIM-017": "UNACTIVATED_CANDIDATE_NOT_CURRENT",
    "RFV4-CLAIM-018": "UNKNOWN_OUTCOME_REQUIRES_READBACK",
    "RFV4-CLAIM-023": "SESSION_GENERATION_MISMATCH",
    "RFV4-CLAIM-025": "CONTRADICTORY_LIFECYCLE_PROJECTION",
    "RFV4-CLAIM-026": "REFERENCE_NOT_AUTHORIZED",
    "RFV4-CLAIM-027": "VALIDATION_MUST_NOT_EXECUTE",
    "RFV4-CLAIM-029": "CURSOR_CONTENT_MISMATCH",
    "RFV4-CLAIM-030": "QUERY_OWNER_MISMATCH",
    "RFV4-CLAIM-032": "RESOURCE_OWNER_MISMATCH",
    "RFV4-CLAIM-033": "REPEATED_BODY_AUTHORITY_FORBIDDEN",
    "RFV4-CLAIM-034": "PROSE_ERROR_BRANCH_FORBIDDEN",
    "RFV4-CLAIM-035": "PYTHON_SEMANTIC_AUTHORITY_FORBIDDEN",
    "RFV4-CLAIM-036": "VALIDATION_TOOL_STARTED_WORK",
    "RFV4-CLAIM-037": "LOCAL_READINESS_AUTHORITY_FORBIDDEN",
    "RFV4-CLAIM-038": "STATIC_REFERENCE_AUTHORITY_FORBIDDEN",
    "RFV4-CLAIM-040": "UNBOUNDED_RESOURCE_POLICY",
}

_SECURITY_NEGATIVE_CASES: dict[str, list[dict[str, Any]]] = {
    "RFV4-CLAIM-019": [
        {"reason": "launcher_claim_override", "code": "POLICY_CLAIM_OVERRIDE"},
        {"reason": "policy_symlink", "code": "POLICY_PATH_UNSAFE"},
        {"reason": "policy_wrong_owner", "code": "POLICY_OWNER_MISMATCH"},
        {"reason": "policy_wrong_mode", "code": "POLICY_MODE_UNSAFE"},
        {
            "reason": "policy_outside_authorized_root",
            "code": "POLICY_ROOT_UNAUTHORIZED",
        },
        {
            "reason": "peer_pid_start_mismatch",
            "code": "LAUNCHER_PEER_IDENTITY_MISMATCH",
        },
        {"reason": "wrong_generation", "code": "SUPERVISOR_GENERATION_MISMATCH"},
        {"reason": "wrong_workspace", "code": "WORKSPACE_NOT_AUTHORIZED"},
        {"reason": "wrong_operation", "code": "OPERATION_NOT_AUTHORIZED"},
        {"reason": "capacity_exhausted", "code": "LAUNCH_CAPACITY_EXHAUSTED"},
    ],
    "RFV4-CLAIM-020": [
        {
            "reason": "socket_parent_symlink",
            "code": "UNSAFE_SOCKET_PATH",
            "daemon_spawn_count": 0,
        },
        {
            "reason": "socket_wrong_owner",
            "code": "UNSAFE_SOCKET_OWNER",
            "unlinked_paths": 0,
        },
        {
            "reason": "socket_wrong_mode",
            "code": "UNSAFE_SOCKET_MODE",
            "unlinked_paths": 0,
        },
        {
            "reason": "live_foreign_socket",
            "code": "LIVE_SOCKET_OWNERSHIP_CONFLICT",
            "unlinked_paths": 0,
            "signalled_processes": 0,
        },
        {
            "reason": "losing_singleton_racer",
            "code": "SUPERVISOR_SINGLETON_LOST",
            "winner_mutations": 0,
        },
        {
            "reason": "partial_spawn_before_control_ack",
            "code": "PARTIAL_DAEMON_SPAWN_ROLLED_BACK",
            "child_reaped": True,
            "owned_socket_cleaned": True,
            "singleton_lease_released": True,
            "partial_state_count": 0,
        },
        {
            "reason": "attach_without_supervisor",
            "code": "SUPERVISOR_UNAVAILABLE",
            "daemon_spawn_count": 0,
        },
    ],
    "RFV4-CLAIM-021": [
        {"reason": "changed_replay", "code": "CHANGED_CONTROL_REPLAY"},
        {
            "reason": "semantic_payload_forbidden",
            "code": "CONTROL_RECORD_SEMANTIC_PAYLOAD_FORBIDDEN",
        },
        {"reason": "record_too_large", "code": "CONTROL_RECORD_TOO_LARGE"},
    ],
    "RFV4-CLAIM-022": [
        {
            "reason": "ambient_environment",
            "code": "AMBIENT_CAPABILITY_FORBIDDEN",
            "adapter_spawned": False,
        },
        {
            "reason": "ambient_argv",
            "code": "AMBIENT_CAPABILITY_FORBIDDEN",
            "adapter_spawned": False,
        },
        {
            "reason": "non_allowlisted_descriptor",
            "code": "INHERITED_DESCRIPTOR_NOT_ALLOWLISTED",
            "adapter_spawned": False,
        },
        {
            "reason": "wrong_fixed_fd",
            "code": "CAPABILITY_DESCRIPTOR_MISMATCH",
            "adapter_spawned": False,
        },
        {
            "reason": "fallback_symlink",
            "code": "UNSAFE_CAPABILITY_FALLBACK_PATH",
            "adapter_spawned": False,
        },
        {
            "reason": "fallback_wrong_owner",
            "code": "UNSAFE_CAPABILITY_FALLBACK_OWNER",
            "adapter_spawned": False,
        },
        {
            "reason": "fallback_wrong_mode",
            "code": "UNSAFE_CAPABILITY_FALLBACK_MODE",
            "adapter_spawned": False,
        },
        {
            "reason": "fallback_reread",
            "code": "CAPABILITY_FALLBACK_NOT_SINGLE_USE",
            "second_read_bytes": 0,
        },
        {
            "reason": "fallback_substituted_path",
            "code": "CAPABILITY_FALLBACK_PATH_REPLACED",
            "capability_delivered": False,
        },
        {
            "reason": "partial_adapter_spawn",
            "code": "PARTIAL_ADAPTER_SPAWN_ROLLED_BACK",
            "grant_revoked": True,
            "child_reaped": True,
            "lifecycle_tasks_joined": True,
            "adapter_processes": 0,
        },
    ],
    "RFV4-CLAIM-024": [
        {
            "grpc_status": "UNAUTHENTICATED",
            "typed_code": "LAUNCH_GRANT_UNKNOWN",
            "session_minted": False,
        },
        {
            "grpc_status": "UNAUTHENTICATED",
            "typed_code": "SESSION_EXPIRED",
            "rpc_authorized": False,
        },
    ],
    "RFV4-CLAIM-028": [
        {
            "grpc_status": "ALREADY_EXISTS",
            "typed_code": "IDEMPOTENCY_CONFLICT",
            "new_query_count": 0,
            "partial_reservations": 0,
        },
        {
            "grpc_status": "RESOURCE_EXHAUSTED",
            "typed_code": "QUERY_CAPACITY_UNAVAILABLE",
            "accepted": False,
            "partial_reservations": 0,
        },
    ],
    "RFV4-CLAIM-039": [
        {"code": "LIFESPAN_HANDSHAKE_REQUIRED", "tools_available": False},
        {"code": "RESULT_TOO_LARGE_FOR_HOST", "materialized_pages": 0},
    ],
}

_SIMPLE_NEGATIVE_CLOSURE: dict[str, dict[str, Any]] = {
    "RFV4-CLAIM-001": {"closed_coverage": False},
    "RFV4-CLAIM-003": {"candidate_count": 2},
    "RFV4-CLAIM-004": {"raw_kind": "Call"},
    "RFV4-CLAIM-007": {"family": "type"},
    "RFV4-CLAIM-013": {"returned_bytes": 0},
    "RFV4-CLAIM-014": {"append_count": 0},
    "RFV4-CLAIM-015": {"installed": False},
    "RFV4-CLAIM-016": {"query_admission": False},
    "RFV4-CLAIM-018": {"admission": "closed"},
    "RFV4-CLAIM-023": {"query_resubmitted": False},
    "RFV4-CLAIM-027": {"query_started": False},
    "RFV4-CLAIM-030": {"query_state": "RUNNING"},
    "RFV4-CLAIM-032": {"lease_count": 1},
    "RFV4-CLAIM-040": {"orphan_children": 0},
}


def _invalid_object(
    value: object, expected: frozenset[str], context: str
) -> Mapping[str, Any]:
    result = _mapping(value, context)
    _strict_keys(result, expected, context)
    return result


def _require_invalid_literal(value: object, expected: str, context: str) -> None:
    if value != expected:
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID",
            f"{context} is not the released invalid operation",
        )


def _derive_simple_negative(
    claim_id: str, base: Mapping[str, Any], invalid: object
) -> dict[str, Any] | None:
    literals: dict[str, tuple[str, dict[str, Any]]] = {
        "RFV4-CLAIM-001": (
            "duplicate batch identity with different row bytes",
            {
                "outcome": "rejected",
                "code": "PROVIDER_BATCH_CONFLICT",
                "closed_coverage": False,
            },
        ),
        "RFV4-CLAIM-002": (
            "report empty complete facts after compile failure",
            {"outcome": "rejected", "code": "FALSE_CLOSED_COVERAGE"},
        ),
        "RFV4-CLAIM-004": (
            "remove mapping for raw kind Call",
            {
                "outcome": "rejected",
                "code": "TRANSFORMATION_INPUT_UNCOVERED",
                "raw_kind": "Call",
            },
        ),
        "RFV4-CLAIM-007": (
            "omit unavailable type from facts and coverage",
            {
                "outcome": "rejected",
                "code": "REQUESTED_FAMILY_UNACCOUNTED",
                "family": "type",
            },
        ),
        "RFV4-CLAIM-010": (
            "treat incomplete negation as positive match",
            {"outcome": "rejected", "code": "NEGATIVE_PROOF_INDETERMINATE"},
        ),
        "RFV4-CLAIM-014": (
            "direct seed activation outside FabricCommand actor",
            {
                "outcome": "rejected",
                "code": "SOLE_MUTATION_AUTHORITY_REQUIRED",
                "append_count": 0,
            },
        ),
        "RFV4-CLAIM-016": (
            "skip SoleTargetAuthorityCommitted before Ready",
            {
                "outcome": "rejected",
                "code": "STATE_TRANSITION_VIOLATION",
                "query_admission": False,
            },
        ),
        "RFV4-CLAIM-017": (
            "make orphan candidate tables query-visible",
            {"outcome": "rejected", "code": "UNACTIVATED_CANDIDATE_NOT_CURRENT"},
        ),
        "RFV4-CLAIM-018": (
            "blind append retry",
            {
                "outcome": "rejected",
                "code": "UNKNOWN_OUTCOME_REQUIRES_READBACK",
                "admission": "closed",
            },
        ),
    }
    literal = literals.get(claim_id)
    if literal is not None:
        _require_invalid_literal(invalid, literal[0], claim_id)
        return copy.deepcopy(literal[1])
    if claim_id == "RFV4-CLAIM-003":
        change = _invalid_object(invalid, frozenset({"cases"}), claim_id)
        cases = [
            _mapping(case, "producer case")
            for case in _sequence(change["cases"], "cases")
        ]
        if [case.get("reason") for case in cases] != [
            "multiple_eligible_producers",
            "zero_eligible_producers",
        ]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "producer cases are incomplete"
            )
        multiple, zero = cases
        _strict_keys(
            multiple,
            frozenset(
                {"reason", "eligible_producers", "authority_rule", "invalid_action"}
            ),
            "multiple producer case",
        )
        _strict_keys(
            zero,
            frozenset({"reason", "eligible_producers", "invalid_action"}),
            "zero producer case",
        )
        candidates = list(
            _sequence(multiple["eligible_producers"], "eligible_producers")
        )
        if (
            candidates != list(base["eligible_producers"])
            or multiple["authority_rule"] is not None
            or multiple["invalid_action"] != "select_first"
            or list(zero["eligible_producers"]) != []
            or zero["invalid_action"] != "report_complete"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "producer case is not causal"
            )
        family = base["required_family"]
        return {
            "outcome": "accounted_partial",
            "cases": [
                {
                    "reason": "multiple_eligible_producers",
                    "selected_producer": None,
                    "remainder": {
                        "family": family,
                        "reason": "ambiguous_producer",
                        "candidates": candidates,
                    },
                },
                {
                    "reason": "zero_eligible_producers",
                    "selected_producer": None,
                    "remainder": {
                        "family": family,
                        "reason": "unsupported",
                        "candidates": [],
                    },
                },
            ],
        }
    if claim_id == "RFV4-CLAIM-005":
        change = _invalid_object(invalid, frozenset({"derived_label"}), claim_id)
        if change["derived_label"] not in {
            "HIGH_RISK",
            "SAFE_TO_REFACTOR",
            "SHOULD_CHANGE",
        }:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "label is not evaluative"
            )
        return {"outcome": "rejected", "code": "NOT_OBJECTIVE_FACT_REQUEST"}
    if claim_id == "RFV4-CLAIM-006":
        change = _invalid_object(invalid, frozenset({"looking_for"}), claim_id)
        if "refactor" not in str(change["looking_for"]):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "request is not evaluative"
            )
        return {
            "outcome": "rejected",
            "code": "NOT_OBJECTIVE_FACT_REQUEST",
            "fact_equivalent_rewrite": "find function declarations matching explicit facts",
        }
    if claim_id == "RFV4-CLAIM-008":
        change = _invalid_object(invalid, frozenset({"distance"}), claim_id)
        if change["distance"] is not None:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "distance remains bounded"
            )
        return {"outcome": "rejected", "code": "INVALID_REQUEST_SCHEMA"}
    if claim_id == "RFV4-CLAIM-009":
        change = _invalid_object(
            invalid, frozenset({"path_policy", "graph", "bound"}), claim_id
        )
        if change != {"path_policy": "all paths", "graph": "cyclic", "bound": None}:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "path case is not unbounded"
            )
        return {
            "outcome": "rejected",
            "code": "UNBOUNDED_QUERY",
            "alternatives": ["shortest", "all shortest", "simple with explicit bound"],
        }
    if claim_id == "RFV4-CLAIM-011":
        change = _invalid_object(
            invalid, frozenset({"second_workspace", "second_role"}), claim_id
        )
        first = _mapping(_sequence(base["inputs"], "inputs")[0], "first input")
        if (
            change["second_workspace"] == first["workspace"]
            or change["second_role"] == first["role"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "combined result remains compatible"
            )
        return {
            "outcome": "rejected",
            "code": "INCOMPATIBLE_RESULT_REFERENCE",
            "workspaces": [first["workspace"], change["second_workspace"]],
            "roles": [first["role"], change["second_role"]],
        }
    if claim_id == "RFV4-CLAIM-012":
        change = _invalid_object(invalid, frozenset({"label"}), claim_id)
        if change["label"] != "SHOULD_CHANGE":
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID",
                "summary label is not the forbidden judgment",
            )
        return {"outcome": "rejected", "code": "NOT_OBJECTIVE_FACT_REQUEST"}
    if claim_id == "RFV4-CLAIM-013":
        change = _invalid_object(invalid, frozenset({"authorized"}), claim_id)
        if change["authorized"] is not False:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "source remains authorized"
            )
        return {
            "outcome": "rejected",
            "code": "SOURCE_ACCESS_DENIED",
            "returned_bytes": 0,
        }
    if claim_id == "RFV4-CLAIM-015":
        change = _invalid_object(
            invalid, frozenset({"read_versions_independently"}), claim_id
        )
        versions = _invalid_object(
            change["read_versions_independently"],
            frozenset({"entities", "facts"}),
            claim_id,
        )
        event = _mapping(base["appended_event"], "appended_event")
        if (
            any(type(value) is not int or value < 0 for value in versions.values())
            or versions == event["versions"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "versions remain coherent"
            )
        return {
            "outcome": "rejected",
            "code": "INCOHERENT_ACTIVATION_HORIZON",
            "installed": False,
        }
    object_contracts: dict[str, tuple[frozenset[str], dict[str, Any]]] = {
        "RFV4-CLAIM-025": (
            frozenset({"lifecycle_authority", "response_lifecycle"}),
            {"outcome": "rejected", "code": "CONTRADICTORY_LIFECYCLE_PROJECTION"},
        ),
        "RFV4-CLAIM-026": (
            frozenset({"authorized"}),
            {
                "outcome": "rejected",
                "grpc_status": "PERMISSION_DENIED",
                "typed_code": "REFERENCE_NOT_AUTHORIZED",
            },
        ),
        "RFV4-CLAIM-027": (
            frozenset({"implementation_action"}),
            {
                "outcome": "rejected",
                "code": "VALIDATION_MUST_NOT_EXECUTE",
                "query_started": False,
            },
        ),
        "RFV4-CLAIM-030": (
            frozenset({"session_owner"}),
            {
                "outcome": "rejected",
                "grpc_status": "PERMISSION_DENIED",
                "typed_code": "QUERY_OWNER_MISMATCH",
                "query_state": "RUNNING",
            },
        ),
        "RFV4-CLAIM-032": (
            frozenset({"session_owner"}),
            {
                "outcome": "rejected",
                "grpc_status": "PERMISSION_DENIED",
                "typed_code": "RESOURCE_OWNER_MISMATCH",
                "lease_count": 1,
            },
        ),
        "RFV4-CLAIM-033": (
            frozenset({"body_authority_fields"}),
            {"outcome": "rejected", "code": "REPEATED_BODY_AUTHORITY_FORBIDDEN"},
        ),
        "RFV4-CLAIM-034": (
            frozenset({"python_branch_on"}),
            {"outcome": "rejected", "code": "PROSE_ERROR_BRANCH_FORBIDDEN"},
        ),
        "RFV4-CLAIM-035": (
            frozenset({"python_action"}),
            {"outcome": "rejected", "code": "PYTHON_SEMANTIC_AUTHORITY_FORBIDDEN"},
        ),
        "RFV4-CLAIM-036": (
            frozenset({"rpc_sequence"}),
            {"outcome": "rejected", "code": "VALIDATION_TOOL_STARTED_WORK"},
        ),
        "RFV4-CLAIM-037": (
            frozenset({"python_default_ready"}),
            {"outcome": "rejected", "code": "LOCAL_READINESS_AUTHORITY_FORBIDDEN"},
        ),
        "RFV4-CLAIM-038": (
            frozenset({"source", "revision"}),
            {"outcome": "rejected", "code": "STATIC_REFERENCE_AUTHORITY_FORBIDDEN"},
        ),
        "RFV4-CLAIM-040": (
            frozenset({"journal_policy", "result_policy"}),
            {
                "outcome": "rejected",
                "code": "UNBOUNDED_RESOURCE_POLICY",
                "orphan_children": 0,
            },
        ),
        "RFV4-CLAIM-041": (
            frozenset({"selectable_feature", "live_route"}),
            {
                "outcome": "rejected",
                "code": "PREDECESSOR_RUNTIME_REACHABLE",
                "sole_target_authority": False,
            },
        ),
    }
    contract = object_contracts.get(claim_id)
    if contract is not None:
        change = _invalid_object(invalid, contract[0], claim_id)
        if (
            claim_id == "RFV4-CLAIM-025"
            and change["lifecycle_authority"] == change["response_lifecycle"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "lifecycle projections agree"
            )
        if claim_id == "RFV4-CLAIM-025" and change != {
            "lifecycle_authority": "BOOTSTRAPPING",
            "response_lifecycle": "READY",
        }:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "lifecycle contradiction is not typed"
            )
        if claim_id == "RFV4-CLAIM-026" and change["authorized"] is not False:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "reference remains authorized"
            )
        if (
            claim_id == "RFV4-CLAIM-027"
            and change["implementation_action"] != "start_query"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "validation did not start work"
            )
        if claim_id in {"RFV4-CLAIM-030", "RFV4-CLAIM-032"} and change[
            "session_owner"
        ] == base.get("lease_owner"):
            raise V4ContractError("V4_NEGATIVE_INPUT_INVALID", "owner did not change")
        if claim_id == "RFV4-CLAIM-030" and change["session_owner"] != "principal:b":
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "cancel owner mismatch is not released"
            )
        if claim_id == "RFV4-CLAIM-033" and not change["body_authority_fields"]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "body repeats no authority"
            )
        if claim_id == "RFV4-CLAIM-033" and change["body_authority_fields"] != [
            "principal",
            "workspace_id",
            "session_id",
        ]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "body authority repetition is not exact"
            )
        if (
            claim_id == "RFV4-CLAIM-034"
            and change["python_branch_on"] != "error_message_prose"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "Python branch source is not prose"
            )
        if (
            claim_id == "RFV4-CLAIM-035"
            and change["python_action"] != "rewrite request to a different query form"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "Python action is not semantic rewriting"
            )
        if claim_id == "RFV4-CLAIM-036" and change["rpc_sequence"] != [
            "ValidateQuery",
            "StartQuery",
        ]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "validation sequence did not start work"
            )
        if claim_id == "RFV4-CLAIM-037" and change["python_default_ready"] is not True:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "Python did not claim readiness"
            )
        if claim_id == "RFV4-CLAIM-038":
            reference = _mapping(base["daemon_reference"], "daemon_reference")
            if (
                change["source"] != "python_static_registry"
                or type(change["revision"]) is not int
                or change["revision"] == reference["revision"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "reference remains daemon-live"
                )
        if claim_id == "RFV4-CLAIM-040" and change != {
            "journal_policy": "retain_unbounded_progress",
            "result_policy": "collect_whole_relation",
        }:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "resource policy is not unbounded"
            )
        if claim_id == "RFV4-CLAIM-041" and not str(change["live_route"]).startswith(
            "codefabric.cpgd.v1/"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "predecessor route is not live"
            )
        if (
            claim_id == "RFV4-CLAIM-041"
            and change["selectable_feature"] != "cpgd-v1-compat"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "predecessor feature is not selectable"
            )
        return copy.deepcopy(contract[1])
    if claim_id == "RFV4-CLAIM-029":
        change = _invalid_object(
            invalid,
            frozenset(
                {
                    "cursor",
                    "cursor_fixture_oracle",
                    "actual_preceding_event_content_sha256",
                }
            ),
            claim_id,
        )
        oracle = _mapping(change["cursor_fixture_oracle"], "cursor_fixture_oracle")
        _validate_cursor_oracle(
            change["cursor"], oracle, "negative cursor_fixture_oracle"
        )
        if (
            oracle.get("preceding_event_content_sha256")
            == change["actual_preceding_event_content_sha256"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "cursor content still matches"
            )
        return {
            "outcome": "rejected",
            "grpc_status": "INVALID_ARGUMENT",
            "typed_code": "CURSOR_CONTENT_MISMATCH",
        }
    if claim_id == "RFV4-CLAIM-039":
        change = _invalid_object(invalid, frozenset({"cases"}), claim_id)
        cases = [
            _mapping(case, f"{claim_id} case")
            for case in _sequence(change["cases"], "cases")
        ]
        if len(cases) != 2:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "lifespan case matrix is incomplete"
            )
        _strict_keys(cases[0], frozenset({"lifespan_yield_before_handshake"}), claim_id)
        _strict_keys(cases[1], frozenset({"resource"}), claim_id)
        resource = _invalid_object(
            cases[1]["resource"],
            frozenset({"requested_live_pages", "selector", "uri"}),
            claim_id,
        )
        selector = _invalid_object(
            resource["selector"],
            frozenset({"kind", "page_count", "start_page"}),
            claim_id,
        )
        base_resource = _mapping(base["resource"], "resource")
        if (
            cases[0]["lifespan_yield_before_handshake"] is not True
            or type(resource["requested_live_pages"]) is not int
            or resource["requested_live_pages"] <= base_resource["requested_live_pages"]
            or resource["uri"] != base_resource["uri"]
            or selector["kind"] != "PAGE_RANGE"
            or selector["page_count"] != resource["requested_live_pages"]
            or type(selector["start_page"]) is not int
            or selector["start_page"] < 0
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "lifespan matrix is not fail-closed"
            )
        return {
            "outcome": "rejected",
            "cases": [
                {"code": "LIFESPAN_HANDSHAKE_REQUIRED", "tools_available": False},
                {"code": "RESULT_TOO_LARGE_FOR_HOST", "materialized_pages": 0},
            ],
        }
    return None


def _negative_cases(invalid: object, claim_id: str) -> list[Mapping[str, Any]]:
    """Return a closed, uniquely named invalid-case matrix."""

    change = _invalid_object(invalid, frozenset({"cases"}), claim_id)
    cases = [
        _mapping(case, f"{claim_id} invalid_change.cases[{index}]")
        for index, case in enumerate(_sequence(change["cases"], "cases"))
    ]
    reasons = [case.get("reason") for case in cases]
    if not all(isinstance(reason, str) and reason for reason in reasons):
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID", f"{claim_id} has an unnamed invalid case"
        )
    if len(set(reasons)) != len(reasons):
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID", f"{claim_id} repeats an invalid case"
        )
    return cases


def _require_reason_order(
    cases: Sequence[Mapping[str, Any]], reasons: Sequence[str], claim_id: str
) -> None:
    actual = [case.get("reason") for case in cases]
    if actual != list(reasons):
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID",
            f"{claim_id} invalid case allocation is incomplete or reordered",
        )


def _closed_launch_case(reason: str, code: str) -> dict[str, Any]:
    return {
        "reason": reason,
        "code": code,
        "grant_registered": False,
        "session_minted": False,
        "adapter_spawned": False,
        "partial_reservations": 0,
    }


def _derive_negative_019(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-019"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "launcher_claim_override",
        "policy_symlink",
        "policy_wrong_owner",
        "policy_wrong_mode",
        "policy_outside_authorized_root",
        "peer_pid_start_mismatch",
        "wrong_uid",
        "wrong_policy",
        "wrong_generation",
        "wrong_workspace",
        "wrong_operation",
        "capacity_exhausted",
        "policy_wrong_type",
        "policy_wrong_device",
        "policy_wrong_inode",
        "policy_not_yet_valid",
        "policy_expired",
        "policy_revoked",
        "request_replayed",
        "policy_schema_not_strict",
        "adapter_distribution_mismatch",
        "adapter_executable_identity_mismatch",
    )
    _require_reason_order(cases, reasons, claim_id)
    policy = _mapping(base["policy"], "policy")
    policy_file = _mapping(base["policy_file"], "policy_file")
    request = _mapping(base["launcher_request"], "launcher_request")
    peer = _mapping(base["launcher_peer"], "launcher_peer")
    observed = _mapping(base["observed_adapter"], "observed_adapter")
    code_by_reason = {
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
    }
    for case in cases:
        reason = str(case["reason"])
        if reason == "launcher_claim_override":
            _strict_keys(case, frozenset({"reason", "launcher_override"}), reason)
            override = _invalid_object(
                case["launcher_override"],
                frozenset(
                    {
                        "principal",
                        "profiles",
                        "operations",
                        "resource_bounds",
                        "mcp_host_bounds",
                        "deadline_bounds",
                    }
                ),
                reason,
            )
            if all(override[key] == policy[key] for key in override):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "launcher override changes no policy claim",
                )
        elif reason.startswith("policy_") and reason not in {
            "policy_not_yet_valid",
            "policy_expired",
            "policy_revoked",
        }:
            _strict_keys(case, frozenset({"reason", "policy_file"}), reason)
            allowed = {
                "policy_symlink": frozenset({"is_symlink", "opened_no_follow"}),
                "policy_wrong_owner": frozenset({"owner_uid"}),
                "policy_wrong_mode": frozenset({"mode"}),
                "policy_outside_authorized_root": frozenset(
                    {"authorized_root", "path"}
                ),
                "policy_wrong_type": frozenset({"file_type"}),
                "policy_wrong_device": frozenset({"device", "inode"}),
                "policy_wrong_inode": frozenset({"device", "inode"}),
                "policy_schema_not_strict": frozenset({"strict_schema"}),
            }[reason]
            changed = _invalid_object(case["policy_file"], allowed, reason)
            if all(changed[key] == policy_file.get(key) for key in changed):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    f"{reason} does not violate policy file authority",
                )
        elif reason == "peer_pid_start_mismatch":
            _strict_keys(case, frozenset({"reason", "launcher_peer"}), reason)
            changed = _invalid_object(
                case["launcher_peer"],
                frozenset({"uid", "pid", "start_time_ticks"}),
                reason,
            )
            if (
                changed["uid"] != peer["uid"]
                or changed["pid"] != request["peer_pid"]
                or changed["start_time_ticks"] == request["peer_start_time_ticks"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "peer mismatch is not isolated to process start",
                )
        elif reason == "wrong_uid":
            _strict_keys(
                case, frozenset({"reason", "launcher_peer", "launcher_request"}), reason
            )
            changed_peer = _invalid_object(
                case["launcher_peer"],
                frozenset({"uid", "pid", "start_time_ticks"}),
                reason,
            )
            changed_request = _invalid_object(
                case["launcher_request"], frozenset({"peer_uid"}), reason
            )
            if (
                changed_peer["uid"] == peer["uid"]
                or changed_peer["uid"] != changed_request["peer_uid"]
                or changed_peer["pid"] != peer["pid"]
                or changed_peer["start_time_ticks"] != peer["start_time_ticks"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "wrong uid case is not peer-bound"
                )
        elif reason in {
            "wrong_policy",
            "wrong_generation",
            "wrong_workspace",
            "wrong_operation",
        }:
            _strict_keys(case, frozenset({"reason", "launcher_request"}), reason)
            key = {
                "wrong_policy": "policy_id",
                "wrong_generation": "supervisor_generation",
                "wrong_workspace": "requested_workspace",
                "wrong_operation": "requested_operation",
            }[reason]
            changed = _invalid_object(
                case["launcher_request"], frozenset({key}), reason
            )
            authorized = {
                "policy_id": policy["policy_id"],
                "supervisor_generation": base["supervisor_generation"],
                "requested_workspace": list(policy["workspaces"]),
                "requested_operation": list(policy["operations"]),
            }[key]
            value = changed[key]
            if value == authorized or (
                isinstance(authorized, list) and value in authorized
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains authorized"
                )
        elif reason == "capacity_exhausted":
            _strict_keys(
                case, frozenset({"reason", "launch_capacity_available"}), reason
            )
            if (
                case["launch_capacity_available"] != 0
                or base["launch_capacity_available"] <= 0
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "capacity case is not an exhausted transition",
                )
        elif reason in {"policy_not_yet_valid", "policy_expired"}:
            _strict_keys(case, frozenset({"reason", "launcher_request"}), reason)
            changed = _invalid_object(
                case["launcher_request"], frozenset({"request_at_unix_ms"}), reason
            )
            at = changed["request_at_unix_ms"]
            violates = (
                at < policy["not_before_unix_ms"]
                if reason == "policy_not_yet_valid"
                else at > policy["expires_at_unix_ms"]
            )
            if not violates:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", f"{reason} is inside the policy window"
                )
        elif reason == "policy_revoked":
            _strict_keys(
                case, frozenset({"reason", "current_revocation_generation"}), reason
            )
            if case["current_revocation_generation"] <= policy["revocation_generation"]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "policy revocation generation did not advance",
                )
        elif reason == "request_replayed":
            _strict_keys(case, frozenset({"reason", "anti_replay_registry"}), reason)
            registry = _invalid_object(
                case["anti_replay_registry"], frozenset({"identity", "seen"}), reason
            )
            if registry != {
                "identity": request["anti_replay_identity"],
                "seen": True,
            }:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "replay is not bound to the request identity",
                )
        elif reason.startswith("adapter_"):
            _strict_keys(case, frozenset({"reason", "observed_adapter"}), reason)
            changed = _invalid_object(
                case["observed_adapter"],
                frozenset(
                    {"distribution", "distribution_version", "executable_sha256"}
                ),
                reason,
            )
            differing = {key for key in changed if changed[key] != observed[key]}
            required = (
                {"distribution"}
                if reason == "adapter_distribution_mismatch"
                else {"executable_sha256"}
            )
            if differing != required:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    f"{reason} changes the wrong identity field",
                )
    return {
        "outcome": "rejected",
        "cases": [
            _closed_launch_case(reason, code_by_reason[reason]) for reason in reasons
        ],
    }


def _derive_negative_020(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-020"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "socket_parent_symlink",
        "runtime_root_wrong_type",
        "socket_wrong_type",
        "socket_wrong_owner",
        "socket_wrong_mode",
        "cross_device_replacement",
        "live_foreign_socket",
        "losing_singleton_racer",
        "stale_replacement_inode_cleanup",
        "owned_stale_socket_recovery",
        "partial_spawn_before_control_ack",
        "attach_without_supervisor",
    )
    _require_reason_order(cases, reasons, claim_id)
    runtime_root = _mapping(base["runtime_root"], "runtime_root")
    socket = _mapping(base["supervisor_socket"], "supervisor_socket")
    derived: list[dict[str, Any]] = []
    for case in cases:
        reason = str(case["reason"])
        if reason in {"socket_parent_symlink", "runtime_root_wrong_type"}:
            _strict_keys(case, frozenset({"reason", "runtime_root"}), reason)
            key = (
                "no_symlink_components" if reason == "socket_parent_symlink" else "type"
            )
            changed = _invalid_object(case["runtime_root"], frozenset({key}), reason)
            expected = False if key == "no_symlink_components" else "regular_file"
            if changed[key] != expected or changed[key] == runtime_root[key]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains safe"
                )
            code = (
                "UNSAFE_SOCKET_PATH"
                if reason == "socket_parent_symlink"
                else "UNSAFE_RUNTIME_ROOT_TYPE"
            )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": code,
                    "daemon_spawn_count": 0,
                }
            )
        elif reason in {"socket_wrong_type", "socket_wrong_owner", "socket_wrong_mode"}:
            _strict_keys(case, frozenset({"reason", "supervisor_socket"}), reason)
            key = {
                "socket_wrong_type": "type",
                "socket_wrong_owner": "owner_uid",
                "socket_wrong_mode": "mode",
            }[reason]
            changed = _invalid_object(
                case["supervisor_socket"], frozenset({key}), reason
            )
            if changed[key] == socket[key]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains safe"
                )
            code = {
                "socket_wrong_type": "UNSAFE_SOCKET_TYPE",
                "socket_wrong_owner": "UNSAFE_SOCKET_OWNER",
                "socket_wrong_mode": "UNSAFE_SOCKET_MODE",
            }[reason]
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": code,
                    "unlinked_paths": 0,
                }
            )
        elif reason == "cross_device_replacement":
            _strict_keys(
                case,
                frozenset({"reason", "recorded_socket", "observed_socket"}),
                reason,
            )
            recorded = _invalid_object(
                case["recorded_socket"], frozenset({"device", "inode"}), reason
            )
            observed = _invalid_object(
                case["observed_socket"], frozenset({"device", "inode"}), reason
            )
            if (
                recorded["inode"] != observed["inode"]
                or recorded["device"] == observed["device"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "replacement is not cross-device"
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "SOCKET_CROSS_DEVICE_REPLACEMENT",
                    "unlinked_paths": 0,
                    "signalled_processes": 0,
                }
            )
        elif reason == "live_foreign_socket":
            _strict_keys(case, frozenset({"reason", "supervisor_socket"}), reason)
            changed = _invalid_object(
                case["supervisor_socket"],
                frozenset({"device", "inode", "live_probe"}),
                reason,
            )
            if (
                changed["live_probe"] != "authenticated_foreign"
                or changed["device"] == socket["device"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "socket is not authenticated foreign authority",
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "LIVE_SOCKET_OWNERSHIP_CONFLICT",
                    "unlinked_paths": 0,
                    "signalled_processes": 0,
                }
            )
        elif reason == "losing_singleton_racer":
            _strict_keys(case, frozenset({"reason", "singleton_lease"}), reason)
            lease = _invalid_object(
                case["singleton_lease"],
                frozenset({"owned", "winner_pid", "winner_start_time_ticks"}),
                reason,
            )
            if lease["owned"] is not False:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "losing racer still owns lease"
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "SUPERVISOR_SINGLETON_LOST",
                    "winner_mutations": 0,
                    "unlinked_paths": 0,
                    "signalled_processes": 0,
                }
            )
        elif reason == "stale_replacement_inode_cleanup":
            _strict_keys(
                case,
                frozenset({"reason", "recorded_socket", "shutdown_observed_socket"}),
                reason,
            )
            recorded = _invalid_object(
                case["recorded_socket"],
                frozenset({"device", "inode", "generation"}),
                reason,
            )
            observed = _invalid_object(
                case["shutdown_observed_socket"],
                frozenset({"device", "inode", "generation"}),
                reason,
            )
            if recorded == observed or recorded["device"] != observed["device"]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "replacement identity did not change"
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "preserved",
                    "code": "REPLACEMENT_SOCKET_IDENTITY_MISMATCH",
                    "unlinked_paths": 0,
                    "observed_replacement_preserved": True,
                }
            )
        elif reason == "owned_stale_socket_recovery":
            _strict_keys(
                case,
                frozenset(
                    {"reason", "singleton_lease", "stale_socket", "replacement_socket"}
                ),
                reason,
            )
            lease = _invalid_object(
                case["singleton_lease"], frozenset({"owned"}), reason
            )
            stale = _invalid_object(
                case["stale_socket"],
                frozenset(
                    {
                        "device",
                        "generation",
                        "inode",
                        "live_probe",
                        "mode",
                        "owner_uid",
                        "type",
                    }
                ),
                reason,
            )
            replacement = _invalid_object(
                case["replacement_socket"],
                frozenset(
                    {"device", "generation", "inode", "mode", "owner_uid", "type"}
                ),
                reason,
            )
            if (
                lease["owned"] is not True
                or stale["live_probe"] != "failed"
                or stale["device"] != replacement["device"]
                or stale["inode"] == replacement["inode"]
                or stale["generation"] >= replacement["generation"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "stale recovery lacks safe identity transition",
                )

            def identity(value: Mapping[str, Any]) -> dict[str, Any]:
                return {key: value[key] for key in ("device", "generation", "inode")}

            derived.append(
                {
                    "reason": reason,
                    "outcome": "recovered",
                    "unlinked_paths": 1,
                    "unlinked_socket_identity": identity(stale),
                    "new_socket_identity": identity(replacement),
                }
            )
        elif reason == "partial_spawn_before_control_ack":
            _strict_keys(case, frozenset({"reason", "spawn_stage"}), reason)
            if case["spawn_stage"] != "child_started_control_unacknowledged":
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "spawn is not partial before control acknowledgement",
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "PARTIAL_DAEMON_SPAWN_ROLLED_BACK",
                    "child_reaped": True,
                    "owned_socket_cleaned": True,
                    "singleton_lease_released": True,
                    "partial_state_count": 0,
                }
            )
        else:
            _strict_keys(
                case,
                frozenset({"reason", "launcher_action", "supervisor_rendezvous"}),
                reason,
            )
            if (
                case["launcher_action"] != "spawn_daemon"
                or case["supervisor_rendezvous"] != "absent"
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "attach-only launcher did not try forbidden spawn",
                )
            derived.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "SUPERVISOR_UNAVAILABLE",
                    "daemon_spawn_count": 0,
                }
            )
    return {"outcome": "case_matrix", "cases": derived}


def _record_case(case: Mapping[str, Any], reason: str) -> Mapping[str, Any]:
    allowed = {"reason", "record"}
    if reason == "expired_record":
        allowed.add("received_at_unix_ms")
    if reason == "content_integrity_mismatch":
        allowed.add("observed_content_sha256")
    _strict_keys(case, frozenset(allowed), reason)
    record = _mapping(case["record"], f"{reason}.record")
    required = {
        "content_sha256",
        "daemon_generation",
        "declared_length_bytes",
        "expires_at_unix_ms",
        "operation",
        "operation_identity",
        "sequence",
        "supervisor_generation",
        "workspace",
    }
    if record.get("operation") == "RegisterLaunchGrant":
        required.add("grant_digest")
    _strict_keys(record, frozenset(required), f"{reason}.record")
    return record


def _derive_negative_021(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-021"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "sequence_gap",
        "exact_replay",
        "changed_replay",
        "unknown_record",
        "wrong_workspace",
        "wrong_daemon_generation",
        "wrong_supervisor_generation",
        "wrong_operation_identity",
        "expired_record",
        "content_integrity_mismatch",
        "record_too_large",
        "semantic_payload_forbidden",
        "channel_replacement",
        "channel_loss",
    )
    _require_reason_order(cases, reasons, claim_id)
    records = [
        _mapping(record, "base record")
        for record in _sequence(base["records"], "records")
    ]
    by_sequence = {record["sequence"]: record for record in records}
    next_sequence = max(by_sequence) + 1
    operation_identity = {
        "AdvanceSupervisorGeneration": "advance-supervisor-generation@1",
        "RegisterLaunchGrant": "register-launch-grant@1",
        "Acknowledgement": "acknowledgement@1",
    }
    code_by_reason = {
        "sequence_gap": "CONTROL_SEQUENCE_GAP",
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
    }
    derived: list[dict[str, Any]] = []
    for case in cases:
        reason = str(case["reason"])
        if reason.startswith("channel_"):
            _strict_keys(case, frozenset({"reason", "channel"}), reason)
            channel = _invalid_object(
                case["channel"],
                frozenset({"channel_generation", "state", "transport"}),
                reason,
            )
            if channel["transport"] != base["channel"]["transport"]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "channel transport drift is not a generation event",
                )
            if reason == "channel_replacement":
                if (
                    channel["state"] != "replacement"
                    or channel["channel_generation"]
                    <= base["channel"]["channel_generation"]
                ):
                    raise V4ContractError(
                        "V4_NEGATIVE_INPUT_INVALID",
                        "replacement channel does not advance generation",
                    )
                derived.append(
                    {
                        "reason": reason,
                        "outcome": "rejected",
                        "code": "CONTROL_CHANNEL_REPLACED",
                    }
                )
            else:
                if (
                    channel["state"] != "lost"
                    or channel["channel_generation"]
                    != base["channel"]["channel_generation"]
                ):
                    raise V4ContractError(
                        "V4_NEGATIVE_INPUT_INVALID",
                        "loss is not bound to active channel",
                    )
                derived.append(
                    {
                        "reason": reason,
                        "outcome": "degraded_draining",
                        "code": "CONTROL_CHANNEL_LOST",
                        "new_handshakes": "closed",
                        "grant_renewals": "closed",
                        "accepted_pinned_work": "survives",
                        "implicit_cancellation": False,
                    }
                )
            continue
        record = _record_case(case, reason)
        if reason == "sequence_gap" and record["sequence"] <= next_sequence:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "sequence is not a forward gap"
            )
        elif reason == "exact_replay" and dict(record) != dict(
            by_sequence[record["sequence"]]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "replay is not byte-identical authority"
            )
        elif reason == "changed_replay":
            prior = by_sequence.get(record["sequence"])
            if (
                prior is None
                or dict(record) == dict(prior)
                or record["content_sha256"] == prior["content_sha256"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "changed replay is not a conflicting prior sequence",
                )
        elif reason == "unknown_record" and record["operation"] in operation_identity:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record operation is known"
            )
        elif reason == "wrong_workspace" and record["workspace"] == base["workspace"]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record workspace remains bound"
            )
        elif (
            reason == "wrong_daemon_generation"
            and record["daemon_generation"] == base["daemon_generation"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record daemon generation remains bound"
            )
        elif (
            reason == "wrong_supervisor_generation"
            and record["supervisor_generation"] == base["supervisor_generation"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID",
                "record supervisor generation remains bound",
            )
        elif reason == "wrong_operation_identity" and record[
            "operation_identity"
        ] == operation_identity.get(record["operation"]):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record operation identity remains bound"
            )
        elif (
            reason == "expired_record"
            and not case["received_at_unix_ms"] > record["expires_at_unix_ms"]
        ):
            raise V4ContractError("V4_NEGATIVE_INPUT_INVALID", "record is not expired")
        elif (
            reason == "content_integrity_mismatch"
            and case["observed_content_sha256"] == record["content_sha256"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record content still matches"
            )
        elif (
            reason == "record_too_large"
            and record["declared_length_bytes"] <= base["governed_max_record_bytes"]
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record remains within governed bound"
            )
        elif (
            reason == "semantic_payload_forbidden"
            and record["operation"] != "SemanticQueryPayload"
        ):
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "record has no forbidden semantic payload"
            )
        if reason == "exact_replay":
            derived.append(
                {
                    "reason": reason,
                    "outcome": "replayed",
                    "code": "CONTROL_EXACT_REPLAY",
                    "new_mutations": 0,
                    "next_sequence": next_sequence,
                }
            )
        else:
            result: dict[str, Any] = {
                "reason": reason,
                "outcome": "rejected",
                "code": code_by_reason[reason],
            }
            if reason == "record_too_large":
                result["governed_max_record_bytes"] = base["governed_max_record_bytes"]
            derived.append(result)
    return {
        "outcome": "case_matrix",
        "cases": derived,
        "failed_cases_new_handshakes": "closed",
    }


def _derive_negative_022(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-022"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "capability_oversized",
        "ambient_environment",
        "ambient_argv",
        "non_allowlisted_descriptor",
        "wrong_fixed_fd",
        "fd3_not_one_way",
        "replacement_after_terminal_eof",
        "fixed_fd3_unavailable_safe_fallback",
        "fallback_when_fixed_fd3_available",
        "fallback_symlink",
        "fallback_wrong_owner",
        "fallback_wrong_mode",
        "fallback_reread",
        "fallback_substituted_path",
        "fallback_not_immediately_unlinked",
        "fallback_capability_logged",
        "fallback_cleanup_leak",
        "partial_adapter_spawn",
        "adapter_early_exit",
        "adapter_signal",
        "adapter_timeout",
    )
    _require_reason_order(cases, reasons, claim_id)
    descriptor = _mapping(base["descriptor_policy"], "descriptor_policy")
    fallback = _mapping(base["one_shot_fallback"], "one_shot_fallback")
    platform = _mapping(base["platform_descriptor_capability"], "platform descriptor")
    outputs: list[dict[str, Any]] = []
    for case in cases:
        reason = str(case["reason"])
        if reason == "capability_oversized":
            _strict_keys(case, frozenset({"reason", "capability_length_bytes"}), reason)
            if case["capability_length_bytes"] <= base["capability_max_bytes"]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "capability remains in bounds"
                )
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "CAPABILITY_FRAME_TOO_LARGE",
                    "capability_delivered": False,
                }
            )
        elif reason in {"ambient_environment", "ambient_argv"}:
            key = (
                "environment_contains_capability"
                if reason == "ambient_environment"
                else "argv_contains_capability"
            )
            _strict_keys(case, frozenset({"reason", key}), reason)
            if case[key] is not True:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "ambient capability is absent"
                )
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "AMBIENT_CAPABILITY_FORBIDDEN",
                    "adapter_spawned": False,
                }
            )
        elif reason in {
            "non_allowlisted_descriptor",
            "wrong_fixed_fd",
            "fd3_not_one_way",
        }:
            _strict_keys(case, frozenset({"reason", "descriptor_policy"}), reason)
            key = {
                "non_allowlisted_descriptor": "allowlisted_inherited_fds",
                "wrong_fixed_fd": "capability_fd",
                "fd3_not_one_way": "fd3_direction",
            }[reason]
            changed = _invalid_object(
                case["descriptor_policy"], frozenset({key}), reason
            )
            if changed[key] == descriptor[key]:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains compliant"
                )
            if reason == "non_allowlisted_descriptor":
                extra = set(changed[key]) - set(descriptor[key])
                if not extra:
                    raise V4ContractError(
                        "V4_NEGATIVE_INPUT_INVALID",
                        "no non-allowlisted descriptor was inherited",
                    )
                outputs.append(
                    {
                        "reason": reason,
                        "outcome": "rejected",
                        "code": "INHERITED_DESCRIPTOR_NOT_ALLOWLISTED",
                        "adapter_spawned": False,
                    }
                )
            elif reason == "wrong_fixed_fd":
                outputs.append(
                    {
                        "reason": reason,
                        "outcome": "rejected",
                        "code": "CAPABILITY_DESCRIPTOR_MISMATCH",
                        "adapter_spawned": False,
                    }
                )
            else:
                outputs.append(
                    {
                        "reason": reason,
                        "outcome": "rejected",
                        "code": "CAPABILITY_DESCRIPTOR_DIRECTION_INVALID",
                        "capability_delivered": False,
                    }
                )
        elif reason == "replacement_after_terminal_eof":
            _strict_keys(
                case,
                frozenset(
                    {
                        "reason",
                        "daemon_generation",
                        "replacement_capability",
                        "descriptor_policy",
                    }
                ),
                reason,
            )
            changed = _invalid_object(
                case["descriptor_policy"],
                frozenset(
                    {
                        "child_fd3_open_for_replacement",
                        "fd3_channel_open_after_delivery",
                        "parent_writer_open_for_replacement",
                        "terminal_eof_observed",
                    }
                ),
                reason,
            )
            if (
                case["daemon_generation"] <= base["daemon_generation"]
                or changed
                != {
                    "child_fd3_open_for_replacement": False,
                    "fd3_channel_open_after_delivery": False,
                    "parent_writer_open_for_replacement": False,
                    "terminal_eof_observed": True,
                }
                or case["replacement_capability"] == base["capability"]
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "replacement is not attempted after terminal EOF",
                )
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "CAPABILITY_CHANNEL_EOF",
                    "replacement_capability_delivered": False,
                    "further_handshake_authority_available": False,
                    "parent_writer_closed": True,
                    "child_fd3_closed": True,
                }
            )
        elif reason == "fixed_fd3_unavailable_safe_fallback":
            _strict_keys(
                case,
                frozenset(
                    {
                        "reason",
                        "platform_descriptor_capability",
                        "child_descriptors",
                        "one_shot_fallback",
                    }
                ),
                reason,
            )
            changed_platform = _invalid_object(
                case["platform_descriptor_capability"],
                frozenset(
                    {
                        "fixed_fd3_inheritance_available",
                        "probe_status",
                        "selected_transport",
                    }
                ),
                reason,
            )
            child = _invalid_object(
                case["child_descriptors"],
                frozenset({"stdin", "stdout", "stderr", "fd3"}),
                reason,
            )
            changed = _invalid_object(
                case["one_shot_fallback"],
                frozenset(
                    {
                        "enabled",
                        "opened_no_follow",
                        "owner_uid",
                        "mode",
                        "max_reads",
                        "opened_device",
                        "opened_inode",
                        "path_device",
                        "path_inode",
                        "unlinked_immediately_after_open",
                        "capability_logged",
                        "cleanup_complete",
                    }
                ),
                reason,
            )
            if (
                changed_platform
                != {
                    "fixed_fd3_inheritance_available": False,
                    "probe_status": "unavailable",
                    "selected_transport": "one_shot_file",
                }
                or child["fd3"] is not None
                or child["stdin"] != "host-stdin"
                or child["stdout"] != "host-stdout"
                or changed["enabled"] is not True
                or changed["opened_no_follow"] is not True
                or changed["owner_uid"] != fallback["owner_uid"]
                or changed["mode"] != "0600"
                or changed["max_reads"] != 1
                or (changed["opened_device"], changed["opened_inode"])
                != (changed["path_device"], changed["path_inode"])
                or changed["unlinked_immediately_after_open"] is not True
                or changed["capability_logged"] is not False
                or changed["cleanup_complete"] is not True
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "fallback is not the lawful conditional authority path",
                )
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "delivered_via_fallback",
                    "selected_transport": "one_shot_file",
                    "fallback_used": True,
                    "fd3_inherited": False,
                    "inherited_extra_fds": [],
                    "direct_host_stdio": True,
                    "capability_read_count": 1,
                    "second_read_bytes": 0,
                    "opened_identity": {
                        "device": changed["opened_device"],
                        "inode": changed["opened_inode"],
                    },
                    "unlinked_immediately_after_open": True,
                    "path_visible_after_open": False,
                    "capability_logged": False,
                    "cleanup_complete": True,
                    "adapter_spawned": True,
                }
            )
        elif reason == "fallback_when_fixed_fd3_available":
            _strict_keys(case, frozenset({"reason", "one_shot_fallback"}), reason)
            changed = _invalid_object(
                case["one_shot_fallback"], frozenset({"enabled"}), reason
            )
            if (
                changed["enabled"] is not True
                or platform["fixed_fd3_inheritance_available"] is not True
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "fallback is not selected while fd3 is available",
                )
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": "CAPABILITY_FALLBACK_NOT_PERMITTED",
                    "adapter_spawned": False,
                }
            )
        elif reason.startswith("fallback_"):
            _strict_keys(
                case,
                frozenset(
                    {"reason", "platform_descriptor_capability", "one_shot_fallback"}
                ),
                reason,
            )
            changed_platform = _invalid_object(
                case["platform_descriptor_capability"],
                frozenset(
                    {
                        "fixed_fd3_inheritance_available",
                        "probe_status",
                        "selected_transport",
                    }
                ),
                reason,
            )
            if (
                changed_platform["fixed_fd3_inheritance_available"] is not False
                or changed_platform["selected_transport"] != "one_shot_file"
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    "unsafe fallback is not on the conditional path",
                )
            keys = {
                "fallback_symlink": frozenset(
                    {"enabled", "is_symlink", "opened_no_follow"}
                ),
                "fallback_wrong_owner": frozenset({"enabled", "owner_uid"}),
                "fallback_wrong_mode": frozenset({"enabled", "mode"}),
                "fallback_reread": frozenset({"enabled", "max_reads"}),
                "fallback_substituted_path": frozenset(
                    {
                        "enabled",
                        "opened_device",
                        "opened_inode",
                        "path_device",
                        "path_inode",
                    }
                ),
                "fallback_not_immediately_unlinked": frozenset(
                    {"enabled", "unlinked_immediately_after_open"}
                ),
                "fallback_capability_logged": frozenset(
                    {"enabled", "capability_logged"}
                ),
                "fallback_cleanup_leak": frozenset({"enabled", "cleanup_complete"}),
            }[reason]
            changed = _invalid_object(case["one_shot_fallback"], keys, reason)
            if changed["enabled"] is not True:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "fallback is disabled"
                )
            result = {
                "fallback_symlink": {
                    "code": "UNSAFE_CAPABILITY_FALLBACK_PATH",
                    "adapter_spawned": False,
                },
                "fallback_wrong_owner": {
                    "code": "UNSAFE_CAPABILITY_FALLBACK_OWNER",
                    "adapter_spawned": False,
                },
                "fallback_wrong_mode": {
                    "code": "UNSAFE_CAPABILITY_FALLBACK_MODE",
                    "adapter_spawned": False,
                },
                "fallback_reread": {
                    "code": "CAPABILITY_FALLBACK_NOT_SINGLE_USE",
                    "second_read_bytes": 0,
                },
                "fallback_substituted_path": {
                    "code": "CAPABILITY_FALLBACK_PATH_REPLACED",
                    "capability_delivered": False,
                },
                "fallback_not_immediately_unlinked": {
                    "code": "CAPABILITY_FALLBACK_UNLINK_REQUIRED",
                    "path_visible": False,
                },
                "fallback_capability_logged": {
                    "code": "CAPABILITY_LOGGING_FORBIDDEN",
                    "logged_capability_bytes": 0,
                },
                "fallback_cleanup_leak": {
                    "code": "CAPABILITY_FALLBACK_CLEANUP_INCOMPLETE",
                    "cleanup_completed": True,
                },
            }[reason]
            predicate = {
                "fallback_symlink": changed.get("is_symlink") is True
                and changed.get("opened_no_follow") is False,
                "fallback_wrong_owner": changed.get("owner_uid")
                != fallback["owner_uid"],
                "fallback_wrong_mode": changed.get("mode") != "0600",
                "fallback_reread": changed.get("max_reads", 0) > 1,
                "fallback_substituted_path": (
                    changed.get("opened_device"),
                    changed.get("opened_inode"),
                )
                != (changed.get("path_device"), changed.get("path_inode")),
                "fallback_not_immediately_unlinked": changed.get(
                    "unlinked_immediately_after_open"
                )
                is False,
                "fallback_capability_logged": changed.get("capability_logged") is True,
                "fallback_cleanup_leak": changed.get("cleanup_complete") is False,
            }[reason]
            if not predicate:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    f"{reason} does not violate fallback authority",
                )
            outputs.append({"reason": reason, "outcome": "rejected", **result})
        else:
            _strict_keys(case, frozenset({"reason", "adapter_spawn"}), reason)
            spawn = _invalid_object(
                case["adapter_spawn"],
                frozenset(
                    {"child_pid", "grant_registered", "process_group_owned", "stage"}
                ),
                reason,
            )
            expected_stage = {
                "partial_adapter_spawn": "child_started_before_grant_delivery",
                "adapter_early_exit": "early_exit",
                "adapter_signal": "signalled",
                "adapter_timeout": "timeout",
            }[reason]
            if (
                spawn["stage"] != expected_stage
                or spawn["grant_registered"] is not True
                or spawn["process_group_owned"] is not True
            ):
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID",
                    f"{reason} is not an owned partial process",
                )
            code = {
                "partial_adapter_spawn": "PARTIAL_ADAPTER_SPAWN_ROLLED_BACK",
                "adapter_early_exit": "ADAPTER_EARLY_EXIT",
                "adapter_signal": "ADAPTER_SIGNALLED",
                "adapter_timeout": "ADAPTER_TIMEOUT",
            }[reason]
            outputs.append(
                {
                    "reason": reason,
                    "outcome": "rejected",
                    "code": code,
                    "grant_revoked": True,
                    "child_reaped": True,
                    "lifecycle_tasks_joined": True,
                    "parent_writer_closed": True,
                    "child_fd3_closed": True,
                    "adapter_processes": 0,
                }
            )
    return {
        "outcome": "case_matrix",
        "cases": outputs,
        "ambient_inherited_fds": [],
        "direct_host_stdio_preserved": True,
        "safe_fallback_required": True,
        "fixed_fd3_preferred_when_available": True,
    }


def _derive_negative_023(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-023"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "old_session_after_supervisor_restart",
        "old_cursor_after_supervisor_restart",
        "implicit_start_after_restart",
        "principal_revocation_ignored",
        "policy_revocation_ignored",
        "old_daemon_child_unreaped",
        "restart_without_fresh_authority",
        "accepted_query_cancelled_on_restart",
    )
    _require_reason_order(cases, reasons, claim_id)
    codes = {
        "old_session_after_supervisor_restart": "SESSION_GENERATION_MISMATCH",
        "old_cursor_after_supervisor_restart": "CURSOR_GENERATION_MISMATCH",
        "implicit_start_after_restart": "IMPLICIT_QUERY_RESUBMISSION_FORBIDDEN",
        "principal_revocation_ignored": "PRINCIPAL_REVOKED",
        "policy_revocation_ignored": "POLICY_REVOKED",
        "old_daemon_child_unreaped": "SUPERVISOR_RESTART_CHILD_NOT_REAPED",
        "restart_without_fresh_authority": "FRESH_GRANT_REQUIRED",
        "accepted_query_cancelled_on_restart": "ACCEPTED_QUERY_IMPLICIT_CANCELLATION_FORBIDDEN",
    }
    outputs: list[dict[str, Any]] = []
    for case in cases:
        reason = str(case["reason"])
        if reason == "old_session_after_supervisor_restart":
            _strict_keys(
                case,
                frozenset({"reason", "present_session", "supervisor_generation"}),
                reason,
            )
            valid = (
                case["present_session"] == base["old_session"]
                and case["supervisor_generation"] == base["new_supervisor_generation"]
            )
        elif reason == "old_cursor_after_supervisor_restart":
            _strict_keys(
                case,
                frozenset({"reason", "present_cursor", "supervisor_generation"}),
                reason,
            )
            valid = (
                case["present_cursor"] == base["old_cursor"]
                and case["supervisor_generation"] == base["new_supervisor_generation"]
            )
        elif reason in {
            "implicit_start_after_restart",
            "accepted_query_cancelled_on_restart",
        }:
            _strict_keys(case, frozenset({"reason", "implementation_action"}), reason)
            valid = case["implementation_action"] == (
                "StartQuery" if reason.startswith("implicit") else "CancelQuery"
            )
        elif reason in {"principal_revocation_ignored", "policy_revocation_ignored"}:
            target = "principal" if reason.startswith("principal") else "policy"
            key = f"revoked_{target}"
            _strict_keys(case, frozenset({"reason", "present_session", key}), reason)
            valid = (
                case["present_session"] == base["old_session"]
                and case[key] == base[f"{target}_id" if target == "policy" else target]
            )
        elif reason == "old_daemon_child_unreaped":
            _strict_keys(case, frozenset({"reason", "restart_observation"}), reason)
            observation = _invalid_object(
                case["restart_observation"],
                frozenset(
                    {
                        "old_daemon_child_joined",
                        "old_daemon_child_reaped",
                        "orphan_children",
                    }
                ),
                reason,
            )
            valid = (
                observation["old_daemon_child_joined"] is False
                and observation["old_daemon_child_reaped"] is False
                and observation["orphan_children"] > 0
            )
        else:
            _strict_keys(case, frozenset({"reason", "fresh_grant_available"}), reason)
            valid = case["fresh_grant_available"] is False
        if not valid:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID",
                f"{reason} is not the released restart violation",
            )
        output: dict[str, Any] = {
            "reason": reason,
            "code": codes[reason],
            "accepted_query_survives": True,
            "start_query_calls": 0,
        }
        if reason == "old_daemon_child_unreaped":
            output.update(
                {
                    "old_daemon_child_joined": True,
                    "old_daemon_child_reaped": True,
                    "orphan_children": 0,
                }
            )
        else:
            output["implicit_cancellation"] = False
            if reason == "restart_without_fresh_authority":
                output["session_minted"] = False
        outputs.append(output)
    return {"outcome": "rejected", "cases": outputs}


def _derive_negative_024(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-024"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "grant_unknown",
        "grant_not_yet_valid",
        "grant_expired",
        "wrong_peer_uid",
        "grant_revoked",
        "grant_replayed",
        "session_expired",
    )
    _require_reason_order(cases, reasons, claim_id)
    codes = {
        "grant_unknown": "LAUNCH_GRANT_UNKNOWN",
        "grant_not_yet_valid": "LAUNCH_GRANT_NOT_YET_VALID",
        "grant_expired": "LAUNCH_GRANT_EXPIRED",
        "wrong_peer_uid": "PEER_UID_MISMATCH",
        "grant_revoked": "LAUNCH_GRANT_REVOKED",
        "grant_replayed": "LAUNCH_GRANT_REPLAY",
        "session_expired": "SESSION_EXPIRED",
    }
    outputs: list[dict[str, Any]] = []
    for case in cases:
        reason = str(case["reason"])
        if reason == "grant_unknown":
            _strict_keys(
                case,
                frozenset({"reason", "registered", "handshake_at_unix_ms"}),
                reason,
            )
            valid = case["registered"] is False
        elif reason in {"grant_not_yet_valid", "grant_expired"}:
            _strict_keys(case, frozenset({"reason", "handshake_at_unix_ms"}), reason)
            valid = (
                case["handshake_at_unix_ms"] < base["grant_not_before_unix_ms"]
                if reason.endswith("not_yet_valid")
                else case["handshake_at_unix_ms"] > base["grant_expires_at_unix_ms"]
            )
        elif reason == "wrong_peer_uid":
            _strict_keys(case, frozenset({"reason", "peer_uid"}), reason)
            valid = case["peer_uid"] != base["handshake_policy"]["required_peer_uid"]
        elif reason == "grant_revoked":
            _strict_keys(
                case, frozenset({"reason", "current_revocation_generation"}), reason
            )
            valid = (
                case["current_revocation_generation"]
                > base["grant_revocation_generation"]
            )
        elif reason == "grant_replayed":
            _strict_keys(case, frozenset({"reason", "grant_replayed"}), reason)
            valid = case["grant_replayed"] is True
        else:
            _strict_keys(
                case,
                frozenset(
                    {
                        "reason",
                        "session",
                        "rpc_at_unix_ms",
                        "session_expires_at_unix_ms",
                    }
                ),
                reason,
            )
            valid = (
                case["session"] != base["grant"]
                and case["rpc_at_unix_ms"] > case["session_expires_at_unix_ms"]
            )
        if not valid:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains authorized"
            )
        output = {
            "reason": reason,
            "grpc_status": "UNAUTHENTICATED",
            "typed_code": codes[reason],
        }
        output[
            "rpc_authorized" if reason == "session_expired" else "session_minted"
        ] = False
        outputs.append(output)
    return {"outcome": "rejected", "cases": outputs}


def _derive_negative_028(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-028"
    change = _invalid_object(invalid, frozenset({"cases"}), claim_id)
    cases = [
        _mapping(case, f"{claim_id} invalid_change.cases[{index}]")
        for index, case in enumerate(_sequence(change["cases"], "cases"))
    ]
    if len(cases) != 2:
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID",
            "StartQuery matrix must have conflict and capacity cases",
        )
    first, second = cases
    _strict_keys(
        first,
        frozenset({"same_idempotency_key", "execution_budget_ms"}),
        "idempotency conflict",
    )
    _strict_keys(second, frozenset({"capacity_available"}), "capacity exhaustion")
    capacity = _invalid_object(
        second["capacity_available"],
        frozenset(
            {
                "coordinator",
                "idempotency",
                "journal_bytes",
                "result_bytes",
                "retention",
                "tasks",
            }
        ),
        "capacity_available",
    )
    if (
        first["same_idempotency_key"] is not True
        or first["execution_budget_ms"] == base["execution_budget_ms"]
    ):
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID", "idempotency operation did not change"
        )
    if (
        capacity["result_bytes"] >= base["capacity"]["result_bytes"]
        or capacity["result_bytes"] != 0
    ):
        raise V4ContractError(
            "V4_NEGATIVE_INPUT_INVALID", "capacity case retains result reservation"
        )
    return {
        "outcome": "rejected",
        "cases": [
            {
                "grpc_status": "ALREADY_EXISTS",
                "typed_code": "IDEMPOTENCY_CONFLICT",
                "new_query_count": 0,
                "partial_reservations": 0,
            },
            {
                "grpc_status": "RESOURCE_EXHAUSTED",
                "typed_code": "QUERY_CAPACITY_UNAVAILABLE",
                "accepted": False,
                "partial_reservations": 0,
            },
        ],
    }


def _derive_negative_031(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-031"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "wrong_workspace",
        "unauthorized_operation",
        "inactive_lease",
        "expired_lease",
        "source_disclosure_denied",
        "wrong_owner",
        "wrong_principal",
        "wrong_session",
        "wrong_daemon_generation",
        "wrong_supervisor_generation",
        "wrong_resource",
        "wrong_cursor_resource",
        "wrong_cursor_content",
        "wrong_cursor_range",
        "range_out_of_bounds",
        "unsafe_filesystem_descriptor",
        "unsafe_object_store_descriptor",
        "unsafe_path_descriptor",
    )
    _require_reason_order(cases, reasons, claim_id)
    resource = _mapping(base["resource"], "resource")
    lease = _mapping(resource["artifact_lease"], "resource artifact lease")
    session = _mapping(base["session_fixture_oracle"], "session fixture oracle")
    base_cursor = _mapping(
        base["read_cursor_fixture_oracle"], "read cursor fixture oracle"
    )
    codes = {
        "wrong_workspace": ("PERMISSION_DENIED", "WORKSPACE_NOT_AUTHORIZED"),
        "unauthorized_operation": (
            "PERMISSION_DENIED",
            "OPERATION_NOT_AUTHORIZED",
        ),
        "inactive_lease": ("FAILED_PRECONDITION", "RESOURCE_LEASE_INACTIVE"),
        "expired_lease": ("FAILED_PRECONDITION", "RESOURCE_EXPIRED"),
        "source_disclosure_denied": (
            "PERMISSION_DENIED",
            "SOURCE_ACCESS_DENIED",
        ),
        "wrong_owner": ("PERMISSION_DENIED", "RESOURCE_OWNER_MISMATCH"),
        "wrong_principal": ("PERMISSION_DENIED", "SESSION_PRINCIPAL_MISMATCH"),
        "wrong_session": ("UNAUTHENTICATED", "SESSION_RESOURCE_MISMATCH"),
        "wrong_daemon_generation": (
            "UNAUTHENTICATED",
            "SESSION_GENERATION_MISMATCH",
        ),
        "wrong_supervisor_generation": (
            "UNAUTHENTICATED",
            "SUPERVISOR_GENERATION_MISMATCH",
        ),
        "wrong_resource": ("PERMISSION_DENIED", "RESOURCE_ID_MISMATCH"),
        "wrong_cursor_resource": ("INVALID_ARGUMENT", "CURSOR_RESOURCE_MISMATCH"),
        "wrong_cursor_content": ("INVALID_ARGUMENT", "CURSOR_CONTENT_MISMATCH"),
        "wrong_cursor_range": ("INVALID_ARGUMENT", "CURSOR_RANGE_MISMATCH"),
        "range_out_of_bounds": ("OUT_OF_RANGE", "RESOURCE_RANGE_OUT_OF_BOUNDS"),
        "unsafe_filesystem_descriptor": (
            "INVALID_ARGUMENT",
            "RESOURCE_DESCRIPTOR_KIND_FORBIDDEN",
        ),
        "unsafe_object_store_descriptor": (
            "INVALID_ARGUMENT",
            "RESOURCE_DESCRIPTOR_KIND_FORBIDDEN",
        ),
        "unsafe_path_descriptor": (
            "INVALID_ARGUMENT",
            "RESOURCE_DESCRIPTOR_KIND_FORBIDDEN",
        ),
    }
    outputs = []
    for case in cases:
        reason = str(case["reason"])
        if reason == "wrong_workspace":
            _strict_keys(case, frozenset({"reason", "present_workspace"}), reason)
            valid = case["present_workspace"] != session["workspace"]
        elif reason == "unauthorized_operation":
            _strict_keys(case, frozenset({"reason", "requested_operation"}), reason)
            valid = case["requested_operation"] not in session["operations"]
        elif reason == "inactive_lease":
            _strict_keys(case, frozenset({"reason", "lease_active"}), reason)
            valid = case["lease_active"] is False and lease["active"] is True
        elif reason == "expired_lease":
            _strict_keys(case, frozenset({"reason", "read_at_unix_ms"}), reason)
            valid = case["read_at_unix_ms"] >= lease["expires_at_unix_ms"]
        elif reason == "source_disclosure_denied":
            _strict_keys(
                case, frozenset({"reason", "source_disclosure_allowed"}), reason
            )
            valid = (
                case["source_disclosure_allowed"] is False
                and resource["source_bearing"] is True
            )
        elif reason == "wrong_owner":
            _strict_keys(
                case, frozenset({"reason", "resource_owner_principal"}), reason
            )
            valid = case["resource_owner_principal"] != resource["owner_principal"]
        elif reason == "wrong_principal":
            _strict_keys(case, frozenset({"reason", "present_principal"}), reason)
            valid = case["present_principal"] != session["principal"]
        elif reason == "wrong_session":
            _strict_keys(case, frozenset({"reason", "present_session_id"}), reason)
            valid = case["present_session_id"] != session["session_id"]
        elif reason == "wrong_daemon_generation":
            _strict_keys(
                case, frozenset({"reason", "present_daemon_generation"}), reason
            )
            valid = case["present_daemon_generation"] != session["daemon_generation"]
        elif reason == "wrong_supervisor_generation":
            _strict_keys(
                case,
                frozenset({"reason", "present_supervisor_generation"}),
                reason,
            )
            valid = (
                case["present_supervisor_generation"]
                != session["supervisor_generation"]
            )
        elif reason == "wrong_resource":
            _strict_keys(case, frozenset({"reason", "requested_resource_uri"}), reason)
            valid = case["requested_resource_uri"] != resource["uri"]
        elif reason == "wrong_cursor_resource":
            _strict_keys(case, frozenset({"reason", "cursor_resource_uri"}), reason)
            valid = case["cursor_resource_uri"] != base_cursor["resource_uri"]
        elif reason == "wrong_cursor_content":
            _strict_keys(
                case, frozenset({"reason", "cursor_resource_checksum_sha256"}), reason
            )
            valid = (
                _SHA256_ID.fullmatch(str(case["cursor_resource_checksum_sha256"]))
                is not None
                and case["cursor_resource_checksum_sha256"]
                != base_cursor["resource_checksum_sha256"]
            )
        elif reason == "wrong_cursor_range":
            _strict_keys(case, frozenset({"reason", "cursor_next_offset"}), reason)
            valid = case["cursor_next_offset"] != base_cursor["next_offset"]
        elif reason == "range_out_of_bounds":
            _strict_keys(case, frozenset({"reason", "range"}), reason)
            changed = _invalid_object(
                case["range"],
                frozenset({"offset", "length", "end_offset_exclusive"}),
                reason,
            )
            valid = (
                changed["length"] == changed["end_offset_exclusive"] - changed["offset"]
                and changed["end_offset_exclusive"] > resource["byte_length"]
            )
        else:
            _strict_keys(case, frozenset({"reason", "descriptor"}), reason)
            changed = _invalid_object(
                case["descriptor"],
                frozenset(
                    {
                        "descriptor_policy_revision",
                        "filesystem_descriptor",
                        "kind",
                        "object_store_descriptor",
                        "path_descriptor",
                    }
                ),
                reason,
            )
            descriptor_contract = {
                "unsafe_filesystem_descriptor": (
                    "FILESYSTEM_PATH",
                    "filesystem_descriptor",
                ),
                "unsafe_object_store_descriptor": (
                    "OBJECT_STORE_LOCATION",
                    "object_store_descriptor",
                ),
                "unsafe_path_descriptor": ("PATH_DESCRIPTOR", "path_descriptor"),
            }
            expected_kind, marker = descriptor_contract[reason]
            valid = (
                changed["kind"] == expected_kind
                and changed[marker] is True
                and sum(
                    changed[key] is True
                    for key in (
                        "filesystem_descriptor",
                        "object_store_descriptor",
                        "path_descriptor",
                    )
                )
                == 1
            )
        if not valid:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", f"{reason} remains authorized"
            )
        grpc_status, typed_code = codes[reason]
        outputs.append(
            {
                "reason": reason,
                "grpc_status": grpc_status,
                "typed_code": typed_code,
                "bytes_delivered": 0,
            }
        )
    return {
        "outcome": "rejected",
        "cases": outputs,
        "filesystem_location_exposed": False,
        "object_store_location_exposed": False,
        "path_descriptor_exposed": False,
    }


def _derive_negative_035(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    del base
    claim_id = "RFV4-CLAIM-035"
    cases = _negative_cases(invalid, claim_id)
    _require_reason_order(
        cases,
        ("python_rewrites_semantics", "python_synthesizes_progress"),
        claim_id,
    )
    expected_actions = {
        "python_rewrites_semantics": "rewrite request to a different query form",
        "python_synthesizes_progress": "emit a progress report absent from daemon events",
    }
    for case in cases:
        _strict_keys(case, frozenset({"reason", "python_action"}), str(case["reason"]))
        if case["python_action"] != expected_actions[str(case["reason"])]:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", "Python authority case is not released"
            )
    return {
        "outcome": "rejected",
        "cases": [
            {
                "reason": "python_rewrites_semantics",
                "code": "PYTHON_SEMANTIC_AUTHORITY_FORBIDDEN",
            },
            {
                "reason": "python_synthesizes_progress",
                "code": "PYTHON_PROGRESS_AUTHORITY_FORBIDDEN",
                "progress_report_count": 0,
            },
        ],
    }


def _derive_negative_041(base: Mapping[str, Any], invalid: object) -> dict[str, Any]:
    claim_id = "RFV4-CLAIM-041"
    cases = _negative_cases(invalid, claim_id)
    reasons = (
        "skipped_coverage",
        "unreadable_coverage",
        "unparsed_coverage",
        "parse_failure",
        "overlapping_coverage",
        "unmatched_coverage",
        "unknown_coverage",
        "count_incoherent_coverage",
        "candidate_unclassified",
        "disposition_count_mismatch",
        "live_v1_route",
        "live_translator",
        "live_bootstrap_backend",
        "live_ontology_backend",
        "live_cutover_controller",
        "unclassified_live_authority",
        "retained_history_unclassified",
    )
    _require_reason_order(cases, reasons, claim_id)
    coverage = _mapping(base["coverage_dimensions"], "coverage_dimensions")
    inventory = {
        str(_mapping(item, "census candidate")["candidate_id"]): _mapping(
            item, "census candidate"
        )
        for item in _sequence(base["candidate_inventory"], "candidate_inventory")
    }
    prohibited = _mapping(
        base["prohibited_live_authority_classes"], "prohibited authorities"
    )
    history = {
        _mapping(item, "retained history")["artifact_id"]: _mapping(
            item, "retained history"
        )
        for item in _sequence(base["retained_history"], "retained_history")
    }
    codes = {
        "skipped_coverage": "ZERO_STATE_COVERAGE_INCOMPLETE",
        "unreadable_coverage": "ZERO_STATE_COVERAGE_INCOMPLETE",
        "unparsed_coverage": "ZERO_STATE_COVERAGE_INCOMPLETE",
        "parse_failure": "ZERO_STATE_COVERAGE_INCOMPLETE",
        "overlapping_coverage": "ZERO_STATE_COVERAGE_OVERLAP",
        "unmatched_coverage": "ZERO_STATE_COVERAGE_UNCLASSIFIED",
        "unknown_coverage": "ZERO_STATE_COVERAGE_UNCLASSIFIED",
        "count_incoherent_coverage": "ZERO_STATE_COVERAGE_COUNT_MISMATCH",
        "candidate_unclassified": "ZERO_STATE_CANDIDATE_UNCLASSIFIED",
        "disposition_count_mismatch": "ZERO_STATE_DISPOSITION_COUNT_MISMATCH",
        "live_v1_route": "PREDECESSOR_RUNTIME_REACHABLE",
        "live_translator": "PREDECESSOR_RUNTIME_REACHABLE",
        "live_bootstrap_backend": "PREDECESSOR_RUNTIME_REACHABLE",
        "live_ontology_backend": "PREDECESSOR_RUNTIME_REACHABLE",
        "live_cutover_controller": "PREDECESSOR_RUNTIME_REACHABLE",
        "unclassified_live_authority": "PREDECESSOR_RUNTIME_UNCLASSIFIED",
        "retained_history_unclassified": "HISTORICAL_ARTIFACT_CLASSIFICATION_INVALID",
    }
    counter_cases = {
        "skipped_coverage": "skipped_count",
        "unreadable_coverage": "unreadable_count",
        "unparsed_coverage": "unparsed_count",
        "parse_failure": "parse_error_count",
        "overlapping_coverage": "overlapping_count",
        "unmatched_coverage": "unmatched_count",
        "unknown_coverage": "unknown_count",
        "count_incoherent_coverage": "count_incoherent_count",
    }
    live_cases = {
        "live_v1_route": "v1_route",
        "live_translator": "translator",
        "live_bootstrap_backend": "bootstrap_backend",
        "live_ontology_backend": "ontology_backend",
        "live_cutover_controller": "cutover_controller",
    }
    for case in cases:
        reason = str(case["reason"])
        if reason in counter_cases:
            _strict_keys(
                case,
                frozenset(
                    {"reason", "coverage_dimension", "counter", "observed_value"}
                ),
                reason,
            )
            dimension = str(case["coverage_dimension"])
            if dimension not in coverage:
                raise V4ContractError(
                    "V4_NEGATIVE_INPUT_INVALID", "coverage dimension is unknown"
                )
            expected_counter = counter_cases[reason]
            valid = (
                case["counter"] == expected_counter
                and coverage[dimension][expected_counter] == 0
                and type(case["observed_value"]) is int
                and case["observed_value"] > 0
            )
        elif reason == "candidate_unclassified":
            _strict_keys(
                case,
                frozenset({"reason", "candidate_id", "classification", "disposition"}),
                reason,
            )
            candidate_id = str(case["candidate_id"])
            valid = (
                candidate_id in inventory
                and case["classification"] == "unclassified"
                and case["disposition"] == "unclassified"
            )
        elif reason == "disposition_count_mismatch":
            _strict_keys(
                case,
                frozenset(
                    {
                        "reason",
                        "coverage_dimension",
                        "disposition_count",
                        "observed_value",
                    }
                ),
                reason,
            )
            dimension = str(case["coverage_dimension"])
            disposition_count = str(case["disposition_count"])
            valid = (
                dimension in coverage
                and disposition_count
                in {
                    "retain_target_count",
                    "retain_history_count",
                    "exclude_non_authority_count",
                }
                and type(case["observed_value"]) is int
                and case["observed_value"]
                != coverage[dimension]["disposition_counts"][disposition_count]
            )
        elif reason in live_cases:
            _strict_keys(
                case,
                frozenset({"reason", "authority_class", "live_count", "surface"}),
                reason,
            )
            authority_class = str(case["authority_class"])
            valid = (
                authority_class == live_cases[reason]
                and authority_class in prohibited
                and prohibited[authority_class]["live_count"] == 0
                and case["live_count"] > 0
                and bool(case["surface"])
            )
        elif reason == "unclassified_live_authority":
            _strict_keys(
                case,
                frozenset(
                    {"reason", "authority_class", "unclassified_count", "surface"}
                ),
                reason,
            )
            authority_class = str(case["authority_class"])
            valid = (
                authority_class in prohibited
                and prohibited[authority_class]["unclassified_count"] == 0
                and case["unclassified_count"] > 0
                and bool(case["surface"])
            )
        else:
            _strict_keys(
                case,
                frozenset({"reason", "artifact_id", "candidate_id", "classification"}),
                reason,
            )
            artifact_id = str(case["artifact_id"])
            valid = (
                artifact_id in history
                and case["candidate_id"] == history[artifact_id]["candidate_id"]
                and case["classification"] == "unclassified_history"
            )
        if not valid:
            raise V4ContractError(
                "V4_NEGATIVE_INPUT_INVALID", f"{reason} is not discriminating"
            )
    return {
        "outcome": "case_matrix",
        "cases": [
            {
                "reason": reason,
                "outcome": "rejected",
                "code": codes[reason],
                "zero_outcomes_issued": False,
                "sole_target_authority": False,
            }
            for reason in reasons
        ],
    }


def _derive_negative(
    claim_id: str, base: Mapping[str, Any], invalid: object
) -> dict[str, Any]:
    special: dict[str, Callable[[Mapping[str, Any], object], dict[str, Any]]] = {
        "RFV4-CLAIM-019": _derive_negative_019,
        "RFV4-CLAIM-020": _derive_negative_020,
        "RFV4-CLAIM-021": _derive_negative_021,
        "RFV4-CLAIM-022": _derive_negative_022,
        "RFV4-CLAIM-023": _derive_negative_023,
        "RFV4-CLAIM-024": _derive_negative_024,
        "RFV4-CLAIM-028": _derive_negative_028,
        "RFV4-CLAIM-031": _derive_negative_031,
        "RFV4-CLAIM-035": _derive_negative_035,
        "RFV4-CLAIM-041": _derive_negative_041,
    }
    if claim_id in special:
        return special[claim_id](base, invalid)
    derived = _derive_simple_negative(claim_id, base, invalid)
    if derived is None:
        raise V4ContractError(
            "V4_TYPED_FAMILY_UNKNOWN", f"no negative derivation for {claim_id}"
        )
    return derived


_EXPECTATION_KEYS = frozenset(
    {
        "schema",
        "claim_id",
        "family",
        "title",
        "design_basis",
        "controlled_input",
        "expected_decoded",
        "independence",
        "discriminating_fault",
        "review",
    }
)
_FIXTURE_KEYS = frozenset(
    {
        "schema",
        "fixture_id",
        "claim_id",
        "fixture_kind",
        "fixture_input",
        "expected_decoded",
        "distinguishes",
    }
)


def _identity_drift(
    actual: object,
    derived: object,
    *,
    path: str = "expected_decoded",
) -> str | None:
    if isinstance(actual, Mapping) and isinstance(derived, Mapping):
        identity_names = {
            "id",
            "ids",
            "fact_id",
            "fact_ids",
            "entity_id",
            "entity_ids",
            "occurrence_id",
            "input_fact_id",
            "query_id",
            "resource_id",
            "selected_epoch",
            "selected_head",
            "retained_query",
            "current_epoch",
            "workspace",
            "workspaces",
            "principal",
        }
        for key, value in actual.items():
            next_path = f"{path}.{key}"
            if key in identity_names and derived.get(key) != value:
                return next_path
            if key in derived:
                drift = _identity_drift(value, derived[key], path=next_path)
                if drift:
                    return drift
        return None
    if isinstance(actual, Sequence) and not isinstance(actual, (str, bytes, bytearray)):
        if not isinstance(derived, Sequence) or isinstance(
            derived, (str, bytes, bytearray)
        ):
            return None
        for index, (left, right) in enumerate(zip(actual, derived, strict=False)):
            drift = _identity_drift(left, right, path=f"{path}[{index}]")
            if drift:
                return drift
    return None


def _validate_expectation_row(row: Mapping[str, Any]) -> dict[str, Any]:
    """Validate one expectation by deriving its decoded result from controlled input."""

    _strict_keys(row, _EXPECTATION_KEYS, "expectation")
    if row.get("schema") != EXPECTATION_SCHEMA:
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "expectation schema drifted")
    family = row.get("family")
    if not isinstance(family, str) or family not in _DERIVERS:
        raise V4ContractError("V4_TYPED_FAMILY_UNKNOWN", f"unknown family {family!r}")
    controlled = _mapping(row.get("controlled_input"), f"{family} controlled_input")
    _strict_keys(controlled, _INPUT_KEYS[family], f"{family} controlled_input")
    _validate_nested_input(family, controlled)
    _validate_enums(family, controlled)
    _validate_public_ids(controlled)
    expected = dict(_mapping(row.get("expected_decoded"), f"{family} expected_decoded"))
    try:
        derived = _DERIVERS[family](controlled)
    except V4ContractError:
        raise
    except (KeyError, TypeError, ValueError) as error:
        raise V4ContractError(
            "V4_TYPED_DERIVATION_FAILED", f"{family}: {error}"
        ) from error
    if expected != derived:
        drift = _identity_drift(expected, derived)
        if drift:
            raise V4ContractError(
                "V4_OUTPUT_ONLY_IDENTITY",
                f"{row.get('claim_id')} invents or drifts identity at {drift}",
            )
        raise V4ContractError(
            "V4_TYPED_EXPECTATION_MISMATCH",
            f"{row.get('claim_id')} decoded expectation is not derived from its controlled input",
        )
    return derived


def _validate_distinguishing_fault(
    fixture: Mapping[str, Any], expectation: Mapping[str, Any], *, bind_claim: bool
) -> None:
    distinguishes = _mapping(fixture.get("distinguishes"), "distinguishes")
    fault = _mapping(expectation.get("discriminating_fault"), "discriminating_fault")
    if set(distinguishes) != {"mutation", "from_expected"}:
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "fixture distinction drifted")
    if not all(
        isinstance(distinguishes.get(key), str) and distinguishes.get(key)
        for key in ("mutation", "from_expected")
    ):
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "fixture distinction is empty")
    if not bind_claim:
        return
    if distinguishes.get("mutation") != fault.get("mutation"):
        raise V4ContractError(
            "V4_CAUSAL_MUTATION_DRIFT", "fixture mutation does not bind its claim"
        )
    if distinguishes.get("from_expected") != fault.get("required_observation"):
        raise V4ContractError(
            "V4_CAUSAL_OBSERVATION_DRIFT", "fixture observation does not bind its claim"
        )


def _validate_negative_decoded(claim_id: str, decoded: Mapping[str, Any]) -> None:
    if decoded.get("outcome") != "rejected":
        raise V4ContractError("V4_NEGATIVE_FAIL_OPEN", f"{claim_id} is not rejected")
    security_cases = _SECURITY_NEGATIVE_CASES.get(claim_id)
    if security_cases is not None:
        if decoded.get("cases") != security_cases:
            raise V4ContractError(
                "V4_SECURITY_NEGATIVE_CLOSURE_DRIFT",
                f"{claim_id} does not close every supervisor/security case",
            )
        if claim_id == "RFV4-CLAIM-019":
            closure = {
                "grant_registered": False,
                "session_minted": False,
                "adapter_spawned": False,
                "partial_reservations": 0,
            }
            if any(decoded.get(key) != value for key, value in closure.items()):
                raise V4ContractError(
                    "V4_SECURITY_NEGATIVE_CLOSURE_DRIFT",
                    "policy denial leaked partial launch authority",
                )
        if claim_id == "RFV4-CLAIM-021" and (
            decoded.get("next_sequence") != 4
            or decoded.get("new_handshakes") != "closed"
        ):
            raise V4ContractError(
                "V4_SECURITY_NEGATIVE_CLOSURE_DRIFT",
                "control rejection did not preserve the horizon and close handshakes",
            )
        return
    required_code = _SIMPLE_NEGATIVE_CODES.get(claim_id)
    if required_code is None:
        raise V4ContractError(
            "V4_TYPED_FAMILY_UNKNOWN", f"no negative contract for {claim_id}"
        )
    actual_code = decoded.get("code", decoded.get("typed_code"))
    if actual_code != required_code:
        raise V4ContractError(
            "V4_NEGATIVE_CODE_DRIFT",
            f"{claim_id} expected {required_code}, found {actual_code!r}",
        )
    closure = _SIMPLE_NEGATIVE_CLOSURE.get(claim_id, {})
    if any(decoded.get(key) != value for key, value in closure.items()):
        raise V4ContractError(
            "V4_NEGATIVE_FAIL_OPEN", f"{claim_id} does not retain fail-closed state"
        )


def _validate_fixture_row(
    fixture: Mapping[str, Any], expectation: Mapping[str, Any]
) -> dict[str, Any]:
    """Validate one causal or negative fixture against its bound expectation."""

    _strict_keys(fixture, _FIXTURE_KEYS, "fixture")
    if fixture.get("schema") != FIXTURE_SCHEMA:
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "fixture schema drifted")
    if fixture.get("claim_id") != expectation.get("claim_id"):
        raise V4ContractError(
            "V4_TYPED_SCHEMA_INVALID", "fixture claim binding drifted"
        )
    fixture_match = _FIXTURE_ALLOCATION.fullmatch(str(fixture.get("fixture_id")))
    claim_match = _CLAIM_ALLOCATION.fullmatch(str(fixture.get("claim_id")))
    kind_suffix = {"causal": "C", "negative": "N"}.get(str(fixture.get("fixture_kind")))
    if (
        fixture_match is None
        or claim_match is None
        or fixture_match.group(1) != claim_match.group(1)
        or fixture_match.group(2) != kind_suffix
    ):
        raise V4ContractError(
            "V4_TYPED_FIXTURE_TRIPLE_INVALID",
            "fixture_id, claim_id, and fixture_kind are not the same allocation",
        )
    controlled = _mapping(expectation.get("controlled_input"), "controlled_input")
    fixture_input = _mapping(fixture.get("fixture_input"), "fixture_input")
    if fixture_input.get("base_case_id") != controlled.get("case_id"):
        raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "fixture base case drifted")
    decoded = dict(
        _mapping(fixture.get("expected_decoded"), "fixture expected_decoded")
    )
    kind = fixture.get("fixture_kind")
    _validate_distinguishing_fault(fixture, expectation, bind_claim=kind == "causal")
    if kind == "causal":
        _strict_keys(
            fixture_input,
            frozenset({"base_case_id", "patch_semantics", "merge_patch"}),
            "causal fixture_input",
        )
        if fixture_input.get("patch_semantics") != "json_merge_patch_rfc7396":
            raise V4ContractError(
                "V4_TYPED_SCHEMA_INVALID", "causal patch semantics drifted"
            )
        patch = _mapping(fixture_input.get("merge_patch"), "merge_patch")
        if not patch:
            raise V4ContractError("V4_CAUSAL_MUTATION_DRIFT", "causal patch is empty")
        patch_paths: set[str] = set()

        def collect_paths(value: object, path: str = "") -> None:
            if isinstance(value, Mapping):
                if not value:
                    patch_paths.add(path)
                for key, child in value.items():
                    collect_paths(child, f"{path}.{key}" if path else key)
                return
            if isinstance(value, Sequence) and not isinstance(
                value, (str, bytes, bytearray)
            ):
                patch_paths.add(f"{path}[]")
                return
            patch_paths.add(path)

        collect_paths(patch)
        if not patch_paths or patch_paths <= {"case_id"}:
            raise V4ContractError(
                "V4_CAUSAL_NON_SEMANTIC_PATCH",
                "causal patch must change a declared semantic input path",
            )
        family = str(expectation["family"])
        allocated_paths = _CAUSAL_PATHS.get(str(expectation["claim_id"]))
        if allocated_paths is None or patch_paths != allocated_paths:
            raise V4ContractError(
                "V4_CAUSAL_PATCH_PATH_INVALID",
                "causal patch paths differ from the released semantic allocation",
            )
        if any(
            path.split(".", 1)[0].removesuffix("[]") not in _INPUT_KEYS[family]
            for path in patch_paths
        ):
            raise V4ContractError(
                "V4_CAUSAL_PATCH_PATH_INVALID",
                "causal patch targets an undeclared path",
            )
        patched = _mapping(apply_json_merge_patch(controlled, patch), "patched input")
        patched_keys = _INPUT_KEYS[family]
        if (
            family == "supervisor_restart_revocation"
            and patched.get("event") == "principal_policy_revocation"
        ):
            patched_keys = patched_keys - {
                "replacement_daemon_pid",
                "restart_policy",
            }
        _strict_keys(patched, patched_keys, f"{family} patched input")
        _validate_nested_input(family, patched)
        _validate_enums(family, patched)
        _validate_public_ids(patched)
        try:
            base_full = _DERIVERS[family](controlled)
            patched_full = (
                _analyses(patched, require_all_anchors=False)
                if family == "analyses"
                else _DERIVERS[family](patched)
            )
            base_semantic = {
                key: value for key, value in controlled.items() if key != "case_id"
            }
            patched_semantic = {
                key: value for key, value in patched.items() if key != "case_id"
            }
            if (base_full, base_semantic) == (patched_full, patched_semantic):
                raise V4ContractError(
                    "V4_CAUSAL_NO_OBSERVABLE_CHANGE",
                    "base and patched first-principles observations are equal",
                )
            derived = _causal_decoded(family, controlled, patched)
        except V4ContractError:
            raise
        except (KeyError, TypeError, ValueError, StopIteration) as error:
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED", f"{family} causal fixture: {error}"
            ) from error
        if decoded != derived:
            drift = _identity_drift(decoded, derived, path="causal.expected_decoded")
            if drift:
                raise V4ContractError(
                    "V4_OUTPUT_ONLY_IDENTITY",
                    f"causal fixture invents identity at {drift}",
                )
            raise V4ContractError(
                "V4_CAUSAL_OBSERVATION_DRIFT",
                f"{fixture.get('fixture_id')} does not observe its controlled mutation",
            )
        return derived
    if kind == "negative":
        _strict_keys(
            fixture_input,
            frozenset({"base_case_id", "invalid_change"}),
            "negative fixture_input",
        )
        invalid_change = fixture_input.get("invalid_change")
        if invalid_change in (None, "", {}, []):
            raise V4ContractError("V4_NEGATIVE_FAIL_OPEN", "invalid change is empty")
        _validate_public_ids(invalid_change)
        try:
            derived = _derive_negative(
                str(expectation["claim_id"]), controlled, invalid_change
            )
        except V4ContractError:
            raise
        except (KeyError, TypeError, ValueError, StopIteration) as error:
            raise V4ContractError(
                "V4_TYPED_DERIVATION_FAILED",
                f"{expectation['family']} negative fixture: {error}",
            ) from error
        if decoded != derived:
            drift = _identity_drift(decoded, derived, path="negative.expected_decoded")
            if drift:
                raise V4ContractError(
                    "V4_OUTPUT_ONLY_IDENTITY",
                    f"negative fixture invents identity at {drift}",
                )
            raise V4ContractError(
                "V4_NEGATIVE_OBSERVATION_DRIFT",
                f"{fixture.get('fixture_id')} does not derive from invalid_change",
            )
        return derived
    raise V4ContractError("V4_TYPED_SCHEMA_INVALID", f"unknown fixture kind {kind!r}")


def validate_expectation_row(row: Mapping[str, Any]) -> dict[str, Any]:
    """Validate one row and attach its exact claim identity to every diagnostic."""

    claim_id = str(row.get("claim_id", "claim:<missing>"))
    try:
        return _validate_expectation_row(row)
    except V4ContractError as error:
        if str(error).startswith(f"{claim_id}:"):
            raise
        raise V4ContractError(error.code, f"{claim_id}: {error}") from error


def validate_fixture_row(
    fixture: Mapping[str, Any], expectation: Mapping[str, Any]
) -> dict[str, Any]:
    """Validate one fixture and attach both fixture and claim IDs to diagnostics."""

    fixture_id = str(fixture.get("fixture_id", "fixture:<missing>"))
    claim_id = str(fixture.get("claim_id", "claim:<missing>"))
    identity = f"{fixture_id}/{claim_id}"
    try:
        return _validate_fixture_row(fixture, expectation)
    except V4ContractError as error:
        if str(error).startswith(f"{identity}:"):
            raise
        raise V4ContractError(error.code, f"{identity}: {error}") from error


def validate_evidence_contracts(root: Path = ROOT) -> ContractReport:
    """Validate the closed 41-family, two-fixture-per-family evidence release."""

    _verify_frozen_release(root)
    expectations = load_jsonl(root / EXPECTATIONS_PATH)
    fixtures = load_jsonl(root / FIXTURES_PATH)
    if tuple(row.get("family") for row in expectations) != EXPECTED_FAMILIES:
        raise V4ContractError(
            "V4_TYPED_FAMILY_ALLOCATION_DRIFT", "41-family allocation/order drifted"
        )
    expected_ids = tuple(
        f"RFV4-CLAIM-{number:03d}" for number in range(1, len(EXPECTED_FAMILIES) + 1)
    )
    if tuple(row.get("claim_id") for row in expectations) != expected_ids:
        raise V4ContractError(
            "V4_TYPED_FAMILY_ALLOCATION_DRIFT", "claim allocation drifted"
        )
    by_claim = {str(row["claim_id"]): row for row in expectations}
    for row in expectations:
        validate_expectation_row(row)
    expected_fixture_ids = tuple(
        f"RFV4-FIX-{number:03d}-{suffix}"
        for number in range(1, len(EXPECTED_FAMILIES) + 1)
        for suffix in ("C", "N")
    )
    if tuple(row.get("fixture_id") for row in fixtures) != expected_fixture_ids:
        raise V4ContractError(
            "V4_TYPED_FIXTURE_ALLOCATION_DRIFT", "fixture allocation/order drifted"
        )
    causal = 0
    negative = 0
    for fixture in fixtures:
        expectation = by_claim.get(str(fixture.get("claim_id")))
        if expectation is None:
            raise V4ContractError("V4_TYPED_SCHEMA_INVALID", "orphan fixture")
        validate_fixture_row(fixture, expectation)
        if fixture["fixture_kind"] == "causal":
            causal += 1
        else:
            negative += 1
    if causal != len(EXPECTED_FAMILIES) or negative != len(EXPECTED_FAMILIES):
        raise V4ContractError(
            "V4_TYPED_FIXTURE_ALLOCATION_DRIFT",
            "each family needs one C and one N fixture",
        )
    return ContractReport(
        families=len(EXPECTED_FAMILIES),
        expectations=len(expectations),
        causal_fixtures=causal,
        negative_fixtures=negative,
    )


def main() -> int:
    """Validate repository evidence and emit a stable one-line summary."""

    try:
        report = validate_evidence_contracts()
    except V4ContractError as error:
        print(f"{error.code}: {error}")
        return 1
    print(
        "v4 typed evidence contracts valid: "
        f"families={report.families} expectations={report.expectations} "
        f"causal={report.causal_fixtures} negative={report.negative_fixtures}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
