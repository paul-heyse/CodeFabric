"""Capture and validate WP48 first-principles FastMCP 4 evidence.

WP43 owns the expected values. This module never imports the adapter, daemon,
generated Protobuf modules, or predecessor evidence. Its mutating ``capture``
command executes a closed set of production and fault probes, records bounded
command metadata, and emits the first five entries of an append-only
transaction. A distinct reviewer supplies the sixth entry with
``finalize-review``. The four read-only selectors validate the frozen result.
"""

from __future__ import annotations

import argparse
import ast
import asyncio
import copy
import functools
import hashlib
import importlib.metadata
import inspect
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Iterator, Mapping, Sequence
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import tomllib
import yaml

from tooling.ci.fastmcp4_successor_expectations import (
    EXPECTED_FILES,
    RELEASE_ID,
    RELEASE_PATH,
    Bundle,
    ExpectationReleaseError,
    load_bundle,
    validate_independent_review,
    validate_issuance,
    validate_negative_fixtures,
    validate_observation,
)

ROOT = Path(__file__).resolve().parents[2]
ACTIVE_RELEASE_PATH = RELEASE_PATH
ACTIVE_RELEASE_ID = RELEASE_ID
TRANSACTION_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp48-production-evidence-v2.jsonl"
)
REVIEW_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp48-independent-review-v2.json"
)
REVIEW_REPORT_PATH = Path(
    "docs/reviews/implementation_review_codefabric_relational_data_fabric_v5_wp48_"
    "2026-09-02_v2.md"
)
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md"
)
JUSTFILE_PATH = Path("justfile")
RUNNER_PATH = Path("tooling/ci/fastmcp4_production_evidence.py")
RUNNER_TEST_PATH = Path("tooling/ci/test_fastmcp4_production_evidence.py")
REAL_OBSERVER_MODULE_PATH = Path("tests/integration/daemon/wp48_observer.rs")
REAL_OBSERVER_REGISTRATION_PATH = Path("tests/integration/daemon.rs")
REAL_COMPONENT_PROBE_PATH = Path("tests/integration/daemon/wp48_component_probe.py")
EXPECTATION_VALIDATOR_PATH = Path("tooling/ci/fastmcp4_successor_expectations.py")
EXPECTATION_VALIDATOR_TEST_PATH = Path(
    "tooling/ci/test_fastmcp4_successor_expectations.py"
)
TRANSACTION_ID = "relational-fabric-v5-wp48-production-evidence-r2"
SUITE = "codefabric-relational-data-fabric@2.3.0"
ENTRY_SCHEMA = "codefabric.fastmcp4-production-evidence.entry.v2"
REVIEW_SCHEMA = "codefabric.fastmcp4-production-evidence.review.v2"
OBSERVATION_REQUEST_SCHEMA = "codefabric.wp48-observation-request.v1"
REAL_OBSERVATION_SCHEMA = "codefabric.wp48-real-topology-observations.v1"
REAL_OBSERVATION_PRODUCER = "rust-integration-real-installed-topology"
NORMAL_REAL_SOURCE = "real-installed-supervisor-daemon-launcher-wheel"
FAULT_REAL_SOURCE = f"{NORMAL_REAL_SOURCE}-fault-mode"
NORMAL_LOCAL_SOURCE = "installed-presentation-and-repository-candidate"
FAULT_LOCAL_SOURCE = f"{NORMAL_LOCAL_SOURCE}-fault-mode"
LOCAL_PROBE_SCHEMA = "codefabric.wp48-installed-presentation-observation.v1"
LOCAL_OBSERVATION_SCHEMA = "codefabric.wp48-local-observations.v1"
FIELD_SOURCE_KINDS = {
    "installed-fastmcp-client",
    "generated-tonic-client",
    "durable-state-readback",
    "focused-production-component-probe",
    "os-process-observation",
    "installed-python-introspection",
    "repository-candidate-census",
    "frozen-contract-execution",
    "git-candidate-history",
}
FIELD_EXECUTION_CLASSES = {
    "real-installed-topology",
    "production-tonic-authority",
    "production-durable-state",
    "production-component-behavior",
    "operating-system-process-census",
    "installed-presentation-introspection",
    "isolated-repository-candidate",
    "frozen-release-validation",
    "candidate-history-comparison",
}
REAL_FIELD_SOURCE_PAIRS = {
    ("installed-fastmcp-client", "real-installed-topology"),
    ("generated-tonic-client", "production-tonic-authority"),
    ("durable-state-readback", "production-durable-state"),
    ("focused-production-component-probe", "production-component-behavior"),
    ("os-process-observation", "operating-system-process-census"),
}
LOCAL_FIELD_SOURCE_PAIRS = {
    ("installed-fastmcp-client", "installed-presentation-introspection"),
    ("installed-python-introspection", "installed-presentation-introspection"),
    ("repository-candidate-census", "isolated-repository-candidate"),
    ("frozen-contract-execution", "frozen-release-validation"),
    ("git-candidate-history", "candidate-history-comparison"),
}
SOURCE_IDENTIFIER = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)+\Z")

ORACLES = (
    "fastmcp4-production-evidence-integrity-check",
    "fastmcp4-production-behavior-check",
    "fastmcp4-causal-fault-check",
    "fastmcp4-clean-reconstruction-check",
)
CRITERIA = (
    "PC-WP48-INT",
    "PC-WP48-BEH",
    "PC-WP48-NEG",
    "PC-WP48-OPS",
)
COMMANDS = {
    "integrity": (ORACLES[0], CRITERIA[0]),
    "behavior": (ORACLES[1], CRITERIA[1]),
    "causal-faults": (ORACLES[2], CRITERIA[2]),
    "clean-reconstruction": (ORACLES[3], CRITERIA[3]),
}

EXPECTED_ENTRY_KINDS = (
    "transaction_opened",
    "claim_observation_map",
    "causal_fault_map",
    "clean_reconstruction_contract",
    "limitations_recorded",
    "review_accepted",
)
BASE_INPUT_PATHS = {
    PLAN_PATH,
    Path("Cargo.toml"),
    Path("Cargo.lock"),
    Path("rust-toolchain.toml"),
    Path("tooling/rust-tool-versions.env"),
    Path("codefabric-cpg-mcp/.python-version"),
    Path("codefabric-cpg-mcp/pyproject.toml"),
    Path("codefabric-cpg-mcp/uv.lock"),
    Path("tooling/proto/production-descriptor.pb"),
    EXPECTATION_VALIDATOR_PATH,
    EXPECTATION_VALIDATOR_TEST_PATH,
    REAL_OBSERVER_MODULE_PATH,
    REAL_OBSERVER_REGISTRATION_PATH,
    REAL_COMPONENT_PROBE_PATH,
}
RELEASE_FILENAMES = tuple(sorted(EXPECTED_FILES))


@dataclass(frozen=True)
class RunSpec:
    """One closed evidence command and the selectors it must exercise."""

    run_id: str
    argv: tuple[str, ...]
    selected_count: int
    selectors: tuple[str, ...]
    execution_class: str
    recipe: str | None = None


def _just_spec(
    run_id: str,
    recipe: str,
    selectors: tuple[str, ...],
    execution_class: str,
    *,
    selected_count: int = 1,
) -> RunSpec:
    return RunSpec(
        run_id,
        ("just", recipe),
        selected_count,
        selectors,
        execution_class,
        recipe,
    )


REAL_TOPOLOGY_CLAIMS = (
    "RFV5-FM4-002",
    "RFV5-FM4-005",
    "RFV5-FM4-006",
    "RFV5-FM4-007",
    "RFV5-FM4-008",
    "RFV5-FM4-009",
    "RFV5-FM4-010",
    "RFV5-FM4-011",
    "RFV5-FM4-012",
)
LOCAL_OBSERVATION_CLAIMS = (
    "RFV5-FM4-001",
    "RFV5-FM4-003",
    "RFV5-FM4-004",
    "RFV5-FM4-013",
    "RFV5-FM4-014",
    "RFV5-FM4-015",
    "RFV5-FM4-016",
)
CLAIM_PROBE_IDS = {
    "RFV5-FM4-001": "exact-successor-release",
    "RFV5-FM4-002": "modern-and-legacy-admission",
    "RFV5-FM4-003": "bare-target-catalog",
    "RFV5-FM4-004": "strict-tool-schema-observation",
    "RFV5-FM4-005": "two-leg-guard",
    "RFV5-FM4-006": "atomic-start-outcomes",
    "RFV5-FM4-007": "authorized-reference-completion",
    "RFV5-FM4-008": "bounded-result-page",
    "RFV5-FM4-009": "accepted-query-cancel-reconnect",
    "RFV5-FM4-010": "two-agent-one-workspace",
    "RFV5-FM4-011": "authority-denial-matrix",
    "RFV5-FM4-012": "secret-bearing-failure",
    "RFV5-FM4-013": "presentation-authority-census",
    "RFV5-FM4-014": "post-cutover-live-census",
    "RFV5-FM4-015": "preregistered-performance-run",
    "RFV5-FM4-016": "frozen-release-drift",
}
CLAIM_FAULT_MODES = {
    "RFV5-FM4-001": "predecessor-suite-and-pins-restored",
    "RFV5-FM4-002": "legacy-business-dispatch",
    "RFV5-FM4-003": "forbidden-component-registration",
    "RFV5-FM4-004": "public-schema-authority-leak",
    "RFV5-FM4-005": "first-leg-acceptance-and-cross-authority-replay",
    "RFV5-FM4-006": "validate-before-start-restored",
    "RFV5-FM4-007": "denied-completion-enumeration",
    "RFV5-FM4-008": "handle-only-resource-read",
    "RFV5-FM4-009": "reconnect-resubmits-query",
    "RFV5-FM4-010": "shared-agent-presentation-authority",
    "RFV5-FM4-011": "authority-denial-hole",
    "RFV5-FM4-012": "secret-and-stdout-leak",
    "RFV5-FM4-013": "python-authority-registration",
    "RFV5-FM4-014": "predecessor-surface-restored",
    "RFV5-FM4-015": "candidate-shaped-performance-method",
    "RFV5-FM4-016": "silent-restamp",
}
REAL_FIELD_SOURCE_CONTRACT = {
    "RFV5-FM4-002": frozenset(
        {
            (
                "legacy-initialize-admission",
                "installed-jsonrpc-admission",
                "installed-fastmcp-client",
                "real-installed-topology",
            )
        }
    ),
    "RFV5-FM4-005": frozenset(
        {
            (
                "guarded-start-authority",
                "generated-client-guard-ledger",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "guarded-input-presentation",
                "installed-fastmcp-input-required",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
            (
                "guarded-input-presentation",
                "guard-challenge-presentation-intervention",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
            (
                "guarded-input-presentation",
                "installed-fastmcp-guard-state-roundtrip",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
        }
    ),
    "RFV5-FM4-006": frozenset(
        {
            (
                "atomic-start-journal",
                "generated-client-atomic-start",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "atomic-start-journal",
                "query-coordinator-sqlite-readback",
                "durable-state-readback",
                "production-durable-state",
            ),
        }
    ),
    "RFV5-FM4-007": frozenset(
        {
            (
                "reference-completion-authority",
                "installed-fastmcp-reference-completion",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
            (
                "reference-completion-authority",
                "generated-client-completion-denial",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "reference-completion-authority",
                "completion-enumeration-intervention",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
        }
    ),
    "RFV5-FM4-008": frozenset(
        {
            (
                "resource-read-authorization",
                "generated-client-resource-read",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "resource-read-authorization",
                "handle-only-resource-intervention",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
        }
    ),
    "RFV5-FM4-009": frozenset(
        {
            (
                "query-reconnect",
                "generated-client-query-reconnect",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "cancellation-cleanup",
                "installed-fastmcp-cancellation-cleanup",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
            (
                "query-reconnect",
                "installed-fastmcp-fresh-session-pair",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
        }
    ),
    "RFV5-FM4-010": frozenset(
        {
            (
                "agent-process-census",
                "installed-two-agent-process-census",
                "os-process-observation",
                "operating-system-process-census",
            ),
            (
                "agent-presentation-isolation",
                "generated-client-cross-agent-authority",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "agent-presentation-isolation",
                "installed-fastmcp-session-isolation",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
        }
    ),
    "RFV5-FM4-011": frozenset(
        {
            (
                "session-and-request-authority",
                "production-session-authority-component",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
            (
                "session-and-request-authority",
                "generated-client-authority-denial-matrix",
                "generated-tonic-client",
                "production-tonic-authority",
            ),
            (
                "session-and-request-authority",
                "installed-cross-principal-denial",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
        }
    ),
    "RFV5-FM4-012": frozenset(
        {
            (
                "adapter-sink-redaction",
                "installed-fastmcp-safe-error-sinks",
                "focused-production-component-probe",
                "production-component-behavior",
            ),
            (
                "adapter-sink-redaction",
                "installed-fastmcp-stderr-capture",
                "installed-fastmcp-client",
                "real-installed-topology",
            ),
        }
    ),
}

REAL_OBSERVATION_RUN_SPEC = RunSpec(
    "real-semantic-observations",
    (
        "cargo",
        "nextest",
        "run",
        "--locked",
        "--test",
        "integration",
        "-E",
        "test(wp48_evidence_real_topology_observations)",
        "--test-threads=1",
        "--no-tests=fail",
    ),
    len(REAL_TOPOLOGY_CLAIMS) * 2,
    ("wp48_evidence_real_topology_observations",),
    "real-installed-topology-semantic-observer",
)
LOCAL_OBSERVATION_RUN_SPEC = RunSpec(
    "local-semantic-observations",
    (
        "uv",
        "run",
        "--frozen",
        "--project",
        "codefabric-cpg-mcp",
        "python",
        str(RUNNER_PATH),
        "emit-local-observations",
    ),
    len(LOCAL_OBSERVATION_CLAIMS) * 2,
    ("emit-local-observations",),
    "installed-and-isolated-candidate-semantic-observer",
)


BEHAVIOR_RUN_SPECS = (
    _just_spec(
        "authority-pins",
        "fastmcp4-dependency-contract-check",
        ("fastmcp==4.0.0", "mcp==2.1.1", "pydantic==2.13.4"),
        "installed-package-identity",
        selected_count=3,
    ),
    _just_spec(
        "provider-batches",
        "exact-provider-batch-check",
        ("wp34_beh_",),
        "provider-native-arrow",
    ),
    _just_spec(
        "transformations",
        "analysis-producer-semantic-check",
        ("wp35_beh_",),
        "typed-transformation",
    ),
    _just_spec(
        "query-forms",
        "semantic-request-program-check",
        ("all_eight_released_forms_compile", "all_eight_epoch_bound_forms_execute"),
        "datafusion-programmatic-query",
        selected_count=8,
    ),
    _just_spec(
        "delta-selection",
        "delta-exact-reconstruction-v3-check",
        ("exact_process_reopen", "snapshot_recipe_reconstructs_the_exact_pin"),
        "exact-delta-selection",
    ),
    _just_spec(
        "activation-readback",
        "lifecycle-production-vertical-check",
        ("wp44_beh_real_supervisor_ready_requires_durable_fresh_activation",),
        "real-supervisor-activation",
    ),
    _just_spec(
        "modern-protocol-exact",
        "fastmcp4-modern-protocol-check",
        ("legacy_initialize_is_rejected_before_business_dispatch",),
        "exact-modern-protocol-oracle",
        selected_count=10,
    ),
    _just_spec(
        "public-surface-exact",
        "fastmcp4-public-surface-check",
        ("test_fastmcp_registers_exact_modern_target_surface",),
        "exact-public-surface-oracle",
        selected_count=3,
    ),
    REAL_OBSERVATION_RUN_SPEC,
    LOCAL_OBSERVATION_RUN_SPEC,
    _just_spec(
        "modern-contract",
        "fastmcp4-contract-observation-check",
        ("wp47_int_real_installed_wheel_modern_contract_observation",),
        "real-installed-topology",
    ),
    _just_spec(
        "guard-query-resources",
        "fastmcp4-stdio-vertical-check",
        ("wp47_beh_real_installed_wheel_guard_query_resource_and_completion",),
        "real-installed-topology",
    ),
    _just_spec(
        "security-boundaries",
        "fastmcp4-security-negative-check",
        ("wp47_neg_real_agent_scope_legacy_framing_and_secret_denial",),
        "real-installed-topology",
    ),
    _just_spec(
        "cancel-reconnect-isolation",
        "fastmcp4-cancellation-recovery-check",
        ("wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation",),
        "real-installed-topology",
    ),
    _just_spec(
        "atomic-start",
        "fastmcp4-atomic-start-check",
        ("wp45_explicit_validation_is_pure", "wp45_start_replay_precedes"),
        "daemon-atomic-start",
    ),
    _just_spec(
        "guard-roundtrip",
        "fastmcp4-guard-roundtrip-check",
        ("guard_three_round_ledger", "guard_invalid_answer"),
        "daemon-guard-authority",
    ),
    _just_spec(
        "completion-authority",
        "fastmcp4-completion-authorization-check",
        ("wp45_live_reference_completion", "wp45_reference_denials"),
        "daemon-completion-authority",
    ),
    _just_spec(
        "resource-authority",
        "fastmcp4-resource-authority-check",
        ("wp45_resource_scoped_manifest_pages", "reference_handle_reauthorizes"),
        "daemon-resource-authority",
    ),
    _just_spec(
        "daemon-security-recovery",
        "fastmcp4-daemon-security-recovery-check",
        ("neg_policy_revocation", "relative_budget_is_consumed"),
        "daemon-security-recovery",
    ),
    _just_spec(
        "adapter-zero-state",
        "fastmcp4-adapter-authority-zero-state-check",
        ("source_has_no_displaced_authority", "maps_closed_v2_contract"),
        "installed-adapter-zero-state",
    ),
    _just_spec(
        "release-drift",
        "fastmcp4-expectation-drift-check",
        ("ops_", "expectation-drift"),
        "independent-release-drift",
        selected_count=16,
    ),
)

CLAIM_RUN_IDS = {
    "RFV5-FM4-001": ("local-semantic-observations", "authority-pins"),
    "RFV5-FM4-002": (
        "real-semantic-observations",
        "modern-protocol-exact",
        "modern-contract",
        "security-boundaries",
    ),
    "RFV5-FM4-003": (
        "local-semantic-observations",
        "public-surface-exact",
        "modern-contract",
    ),
    "RFV5-FM4-004": (
        "local-semantic-observations",
        "public-surface-exact",
        "modern-contract",
        "authority-pins",
    ),
    "RFV5-FM4-005": (
        "real-semantic-observations",
        "guard-query-resources",
        "guard-roundtrip",
    ),
    "RFV5-FM4-006": ("real-semantic-observations", "atomic-start"),
    "RFV5-FM4-007": (
        "real-semantic-observations",
        "guard-query-resources",
        "completion-authority",
    ),
    "RFV5-FM4-008": (
        "real-semantic-observations",
        "guard-query-resources",
        "resource-authority",
        "security-boundaries",
    ),
    "RFV5-FM4-009": (
        "real-semantic-observations",
        "cancel-reconnect-isolation",
        "daemon-security-recovery",
    ),
    "RFV5-FM4-010": (
        "real-semantic-observations",
        "cancel-reconnect-isolation",
        "security-boundaries",
    ),
    "RFV5-FM4-011": (
        "real-semantic-observations",
        "security-boundaries",
        "daemon-security-recovery",
    ),
    "RFV5-FM4-012": ("real-semantic-observations", "security-boundaries"),
    "RFV5-FM4-013": (
        "local-semantic-observations",
        "adapter-zero-state",
        "modern-contract",
    ),
    "RFV5-FM4-014": (
        "local-semantic-observations",
        "adapter-zero-state",
        "authority-pins",
    ),
    "RFV5-FM4-015": ("local-semantic-observations", "release-drift"),
    "RFV5-FM4-016": ("local-semantic-observations", "release-drift"),
}
NEGATIVE_FIXTURE_RUN_IDS = {
    "RFV5-FM4-001-N": (
        "local-semantic-observations",
        "authority-pins",
        "adapter-zero-state",
    ),
    "RFV5-FM4-002-N": ("real-semantic-observations", "fault-mcp-projection"),
    "RFV5-FM4-003-N": (
        "local-semantic-observations",
        "public-surface-exact",
        "adapter-zero-state",
    ),
    "RFV5-FM4-004-N": (
        "local-semantic-observations",
        "public-surface-exact",
        "modern-contract",
    ),
    "RFV5-FM4-005-N": ("real-semantic-observations", "fault-guard-token"),
    "RFV5-FM4-006-N": ("real-semantic-observations", "fault-start-variant"),
    "RFV5-FM4-007-N": (
        "real-semantic-observations",
        "fault-completion-filtering",
    ),
    "RFV5-FM4-008-N": (
        "real-semantic-observations",
        "fault-resource-authorization",
    ),
    "RFV5-FM4-009-N": ("real-semantic-observations", "fault-cancellation"),
    "RFV5-FM4-010-N": (
        "real-semantic-observations",
        "fault-cancellation",
        "fault-mcp-projection",
    ),
    "RFV5-FM4-011-N": (
        "real-semantic-observations",
        "daemon-security-recovery",
        "fault-mcp-projection",
    ),
    "RFV5-FM4-012-N": ("real-semantic-observations", "fault-mcp-projection"),
    "RFV5-FM4-013-N": ("local-semantic-observations", "adapter-zero-state"),
    "RFV5-FM4-014-N": (
        "local-semantic-observations",
        "adapter-zero-state",
        "release-drift",
    ),
    "RFV5-FM4-015-N": ("local-semantic-observations", "release-drift"),
    "RFV5-FM4-016-N": ("local-semantic-observations", "release-drift"),
}
SUBSTRATE_RUN_IDS = (
    "provider-batches",
    "transformations",
    "query-forms",
    "delta-selection",
    "activation-readback",
)


def _cargo_test(test_name: str, *, integration: bool = False) -> tuple[str, ...]:
    target = ("--test", "integration") if integration else ("--lib",)
    return (
        "cargo",
        "nextest",
        "run",
        "--locked",
        *target,
        "-E",
        f"test({test_name})",
        "--no-tests=fail",
    )


FAULT_RUN_SPECS = (
    RunSpec(
        "fault-provider-row",
        _cargo_test("changed_row_without_a_new_compiler_pin_is_causally_rejected"),
        1,
        ("changed_row_without_a_new_compiler_pin_is_causally_rejected",),
        "causal-fault-provider-row",
    ),
    RunSpec(
        "fault-transformation",
        _cargo_test("changed_catalog_inputs_causally_change_real_producer_outputs"),
        1,
        ("changed_catalog_inputs_causally_change_real_producer_outputs",),
        "causal-fault-transformation",
    ),
    RunSpec(
        "fault-datafusion-plan-schema",
        _cargo_test("unresolved_relation_and_function_name_authority_fail_closed"),
        1,
        ("unresolved_relation_and_function_name_authority_fail_closed",),
        "causal-fault-datafusion-plan-schema",
    ),
    RunSpec(
        "fault-delta-selection",
        _cargo_test(
            "unknown_reader_feature_is_rejected_while_reopening_the_exact_delta_version"
        ),
        1,
        ("unknown_reader_feature_is_rejected_while_reopening_the_exact_delta_version",),
        "causal-fault-delta-selection",
    ),
    RunSpec(
        "fault-activation-readback",
        _cargo_test(
            "wp32_neg_recovery_rejects_a_reversible_version_vector_substitution"
        ),
        1,
        ("wp32_neg_recovery_rejects_a_reversible_version_vector_substitution",),
        "causal-fault-activation-readback",
    ),
    RunSpec(
        "fault-start-variant",
        _cargo_test("wp45_start_replay_precedes_changed_catalog_preparation"),
        1,
        ("wp45_start_replay_precedes_changed_catalog_preparation",),
        "causal-fault-start-variant",
    ),
    RunSpec(
        "fault-guard-token",
        _cargo_test("wp45_guard_invalid_answer_closes_and_replays_stable_rejection"),
        1,
        ("wp45_guard_invalid_answer_closes_and_replays_stable_rejection",),
        "causal-fault-guard-token",
    ),
    RunSpec(
        "fault-resource-authorization",
        _cargo_test("wp45_restart_retains_locator_and_release_denies_reissue_durably"),
        1,
        ("wp45_restart_retains_locator_and_release_denies_reissue_durably",),
        "causal-fault-resource-authorization",
    ),
    RunSpec(
        "fault-completion-filtering",
        _cargo_test("wp45_reference_denials_do_not_disclose_selector_existence"),
        1,
        ("wp45_reference_denials_do_not_disclose_selector_existence",),
        "causal-fault-completion-filtering",
    ),
    RunSpec(
        "fault-cancellation",
        _cargo_test(
            "wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation",
            integration=True,
        ),
        1,
        ("wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation",),
        "causal-fault-cancellation",
    ),
    RunSpec(
        "fault-mcp-projection",
        _cargo_test(
            "wp47_neg_real_agent_scope_legacy_framing_and_secret_denial",
            integration=True,
        ),
        1,
        ("wp47_neg_real_agent_scope_legacy_framing_and_secret_denial",),
        "causal-fault-mcp-projection",
    ),
    _just_spec(
        "independent-negative-fixtures",
        "fastmcp4-negative-fixture-independence-check",
        ("neg_", "negative-fixture-independence"),
        "independent-fixture-discrimination",
        selected_count=16,
    ),
)

FAULT_RUN_IDS = {
    "provider_row": "fault-provider-row",
    "transformation": "fault-transformation",
    "datafusion_plan_schema": "fault-datafusion-plan-schema",
    "delta_selection": "fault-delta-selection",
    "activation_readback": "fault-activation-readback",
    "start_variant": "fault-start-variant",
    "guard_token": "fault-guard-token",
    "resource_authorization": "fault-resource-authorization",
    "completion_filtering": "fault-completion-filtering",
    "cancellation": "fault-cancellation",
    "mcp_projection": "fault-mcp-projection",
}

CLEAN_RUN_SPECS = (
    RunSpec(
        "clean-real-topology",
        (
            "cargo",
            "nextest",
            "run",
            "--locked",
            "--test",
            "integration",
            "-E",
            (
                "test(/(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation|"
                "wp47_int_real_installed_wheel_modern_contract_observation|"
                "wp47_beh_real_installed_wheel_guard_query_resource_and_completion|"
                "wp47_neg_real_agent_scope_legacy_framing_and_secret_denial|"
                "wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation)/)"
            ),
            "--test-threads=1",
            "--no-tests=fail",
        ),
        5,
        (
            "fresh-activation",
            "installed-wheel-contract",
            "guard-query-resource-completion",
            "security-denial",
            "restart-reconnect-two-agent",
        ),
        "fresh-build-real-topology",
    ),
    RunSpec(
        "clean-source-mutation",
        _cargo_test("wp34_beh_incremental_run_emits_structural_changed_ranges"),
        1,
        ("wp34_beh_incremental_run_emits_structural_changed_ranges",),
        "fresh-build-source-mutation",
    ),
    _just_spec(
        "clean-descriptor",
        "proto-repro-check",
        ("repro-check",),
        "fresh-runtime-descriptor-identity",
    ),
)

FORBIDDEN_LIVE_FRAGMENTS = (
    "test_production_evidence_claim017",
    "wp38-artifact-bound",
    "fastmcp3",
    "fastmcp-3",
    "predecessor-comparator",
    "source-tree-adapter-import",
)
REVIEW_SCOPE = (
    "causality",
    "fixture_independence",
    "real_process_coverage",
    "exclusions",
    "fault_discrimination",
    "limitations",
)
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
MAX_STDOUT_BYTES = 16_777_216
MAX_STDERR_BYTES = 131_072


class ProductionEvidenceError(ValueError):
    """Typed fail-closed WP48 transaction error."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        details: Mapping[str, object] | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.details = dict(details or {})


Executor = Callable[
    [RunSpec, Path, Mapping[str, str]], subprocess.CompletedProcess[bytes]
]


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise ProductionEvidenceError(code, message)


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(
        isinstance(value, Mapping),
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        f"{context} must be an object",
    )
    assert isinstance(value, Mapping)
    return value


def _rows(value: object, context: str) -> list[Mapping[str, Any]]:
    _require(
        isinstance(value, list),
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        f"{context} must be a list",
    )
    assert isinstance(value, list)
    return [_mapping(row, f"{context}[{index}]") for index, row in enumerate(value)]


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        _require(
            key not in result,
            "RFV5_EVIDENCE_DUPLICATE_MEMBER",
            f"duplicate JSON member {key!r}",
        )
        result[key] = value
    return result


def _load_json(path: Path) -> Mapping[str, Any]:
    try:
        return _mapping(
            json.loads(
                path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
            ),
            str(path),
        )
    except (OSError, json.JSONDecodeError) as error:
        raise ProductionEvidenceError("RFV5_EVIDENCE_UNREADABLE", str(error)) from error


def _load_jsonl(path: Path) -> list[Mapping[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ProductionEvidenceError("RFV5_EVIDENCE_UNREADABLE", str(error)) from error
    _require(
        bool(lines) and all(line.strip() for line in lines),
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "transaction must be non-empty JSONL without blank rows",
    )
    result: list[Mapping[str, Any]] = []
    for number, line in enumerate(lines, 1):
        try:
            value = json.loads(line, object_pairs_hook=_reject_duplicates)
        except json.JSONDecodeError as error:
            raise ProductionEvidenceError(
                "RFV5_EVIDENCE_UNREADABLE", f"row {number}: {error}"
            ) from error
        result.append(_mapping(value, f"transaction row {number}"))
    return result


def canonical_sha256(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def _file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _bytes_sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _recipe_body(justfile: str, recipe: str) -> str:
    match = re.search(
        rf"(?m)^{re.escape(recipe)}:[^\n]*\n(?P<body>(?:[ \t][^\n]*\n|\n)*)",
        justfile,
    )
    _require(
        match is not None,
        "RFV5_EVIDENCE_RECIPE_MISSING",
        f"recipe {recipe} is absent",
    )
    assert match is not None
    body = match.group("body")
    _require(
        bool(body.strip()),
        "RFV5_EVIDENCE_RECIPE_EMPTY",
        f"recipe {recipe} is empty",
    )
    return body


def _recipe_definition(justfile: str, recipe: str) -> str:
    body = _recipe_body(justfile, recipe)
    header = re.search(rf"(?m)^{re.escape(recipe)}:[^\n]*$", justfile)
    assert header is not None
    return f"{header.group()}\n{body}"


def _just_recipe_catalog(root: Path) -> Mapping[str, Mapping[str, Any]]:
    result = subprocess.run(
        ["just", "--dump", "--dump-format", "json"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    _require(
        result.returncode == 0,
        "RFV5_EVIDENCE_RECIPE_INVALID",
        "just could not parse the repository command contract",
    )
    try:
        document = _mapping(
            json.loads(result.stdout, object_pairs_hook=_reject_duplicates),
            "just dump",
        )
    except json.JSONDecodeError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_RECIPE_INVALID", "just emitted invalid JSON"
        ) from error
    recipes = _mapping(document.get("recipes"), "just recipes")
    return {
        str(name): _mapping(recipe, f"just recipe {name}")
        for name, recipe in recipes.items()
    }


def _just_recipe_catalog_from_text(
    justfile: str,
) -> Mapping[str, Mapping[str, Any]]:
    with tempfile.TemporaryDirectory(prefix="wp48-just-snapshot-") as temporary:
        snapshot = Path(temporary)
        (snapshot / "justfile").write_text(justfile, encoding="utf-8")
        return _just_recipe_catalog(snapshot)


def _git_blob(root: Path, candidate: str, path: Path) -> bytes:
    result = subprocess.run(
        ["git", "show", f"{candidate}:{path.as_posix()}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    _require(
        result.returncode == 0,
        "RFV5_EVIDENCE_CANDIDATE_BLOB_MISSING",
        f"candidate {candidate} does not contain {path}",
    )
    return result.stdout


def _snapshot_bytes(root: Path, path: Path, *, candidate: str | None) -> bytes:
    return (
        (root / path).read_bytes()
        if candidate is None
        else _git_blob(root, candidate, path)
    )


def _snapshot_text(root: Path, path: Path, *, candidate: str | None) -> str:
    try:
        return _snapshot_bytes(root, path, candidate=candidate).decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_CANDIDATE_BLOB_INVALID", f"{path} is not UTF-8"
        ) from error


def _immutable_source_bindings(
    root: Path,
    release_path: Path,
    *,
    candidate: str | None = None,
    verify_hashes: bool = True,
) -> tuple[tuple[Path, str], ...]:
    """Read and verify immutable source bindings from the selected release snapshot."""

    _require(
        not release_path.is_absolute()
        and ".." not in release_path.parts
        and release_path != Path("."),
        "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
        "expectation release path must be repository-relative",
    )
    try:
        value = yaml.safe_load(
            _snapshot_text(
                root,
                release_path / "issuance.yaml",
                candidate=candidate,
            )
        )
    except yaml.YAMLError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
            f"expectation issuance is not valid YAML: {error}",
        ) from error
    issuance = _mapping(value, "expectation issuance")
    rows = _rows(issuance.get("immutable_source_inputs"), "immutable sources")
    bindings: list[tuple[Path, str]] = []
    observed: set[Path] = set()
    for number, row in enumerate(rows):
        path = Path(str(row.get("path", "")))
        digest = str(row.get("sha256", ""))
        _require(
            set(row) == {"path", "sha256"}
            and path != Path(".")
            and not path.is_absolute()
            and ".." not in path.parts
            and path not in observed
            and SHA256.fullmatch(digest) is not None,
            "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
            f"immutable source binding {number} is invalid",
        )
        if verify_hashes:
            try:
                source_bytes = _snapshot_bytes(root, path, candidate=candidate)
            except OSError as error:
                raise ProductionEvidenceError(
                    "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
                    f"immutable source input is absent: {path}",
                ) from error
            _require(
                _bytes_sha256(source_bytes) == digest,
                "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
                f"immutable source input hash differs: {path}",
            )
        observed.add(path)
        bindings.append((path, digest))
    _require(
        bool(bindings),
        "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
        "expectation release declares no immutable source inputs",
    )
    return tuple(bindings)


def _expected_input_paths(
    root: Path, release_path: Path, *, candidate: str | None = None
) -> set[Path]:
    source_paths = {
        path
        for path, _digest in _immutable_source_bindings(
            root, release_path, candidate=candidate
        )
    }
    return (
        BASE_INPUT_PATHS
        | source_paths
        | {release_path / name for name in RELEASE_FILENAMES}
    )


def _recipe_closure(
    catalog: Mapping[str, Mapping[str, Any]], roots: Sequence[str]
) -> tuple[str, ...]:
    pending = list(roots)
    closed: set[str] = set()
    while pending:
        recipe = pending.pop()
        if recipe in closed:
            continue
        _require(
            recipe in catalog,
            "RFV5_EVIDENCE_RECIPE_MISSING",
            f"recipe {recipe} is absent from the parsed command contract",
        )
        closed.add(recipe)
        dependencies = _rows(
            catalog[recipe].get("dependencies"), f"{recipe} dependencies"
        )
        for dependency in dependencies:
            name = dependency.get("recipe")
            _require(
                isinstance(name, str) and bool(name),
                "RFV5_EVIDENCE_RECIPE_INVALID",
                f"recipe {recipe} has an invalid dependency",
            )
            pending.append(name)
    return tuple(sorted(closed))


def _validate_recipe_specs(
    root: Path, specs: Sequence[RunSpec], *, candidate: str | None = None
) -> None:
    justfile = _snapshot_text(root, JUSTFILE_PATH, candidate=candidate)
    catalog = (
        _just_recipe_catalog(root)
        if candidate is None
        else _just_recipe_catalog_from_text(justfile)
    )
    recipe_roots = tuple(spec.recipe for spec in specs if spec.recipe is not None)
    for recipe in _recipe_closure(catalog, recipe_roots):
        normalized = json.dumps(catalog[recipe], separators=(",", ":"), sort_keys=True)
        _require(
            not any(
                fragment in normalized.lower() for fragment in FORBIDDEN_LIVE_FRAGMENTS
            ),
            "RFV5_EVIDENCE_HISTORY_EDGE",
            f"recipe dependency closure {recipe} uses a forbidden live edge",
        )
    for spec in specs:
        if spec.recipe is None:
            continue
        definition = _recipe_definition(justfile, spec.recipe)
        body = _recipe_body(justfile, spec.recipe)
        lowered = definition.lower()
        _require(
            not any(fragment in lowered for fragment in FORBIDDEN_LIVE_FRAGMENTS),
            "RFV5_EVIDENCE_HISTORY_EDGE",
            f"run {spec.run_id} uses a forbidden live edge",
        )
        for selector in spec.selectors:
            _require(
                selector in body,
                "RFV5_EVIDENCE_SELECTOR_MISSING",
                f"run {spec.run_id} selector {selector!r} is absent from {spec.recipe}",
            )


def _append(
    entries: list[dict[str, Any]],
    kind: str,
    payload: Mapping[str, Any],
    *,
    recorder: str,
) -> None:
    entry: dict[str, Any] = {
        "schema": ENTRY_SCHEMA,
        "sequence": len(entries) + 1,
        "transaction_id": TRANSACTION_ID,
        "entry_kind": kind,
        "recorded_by": recorder,
        "previous_entry_sha256": entries[-1]["entry_sha256"] if entries else None,
        "payload": dict(payload),
    }
    entry["entry_sha256"] = canonical_sha256(entry)
    entries.append(entry)


def _write_entries(
    path: Path, entries: Sequence[Mapping[str, Any]], *, exclusive: bool
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = "x" if exclusive else "w"
    with path.open(mode, encoding="utf-8", newline="\n") as output:
        for entry in entries:
            output.write(json.dumps(entry, separators=(",", ":"), sort_keys=True))
            output.write("\n")


def _under_root(root: Path, path: Path, context: str) -> Path:
    resolved_root = root.resolve()
    resolved = (
        path.resolve() if path.is_absolute() else (resolved_root / path).resolve()
    )
    _require(
        resolved.is_relative_to(resolved_root),
        "RFV5_EVIDENCE_PATH_INVALID",
        f"{context} must remain under the repository root",
    )
    return resolved


def _validate_chain(
    entries: Sequence[Mapping[str, Any]], *, reviewed: bool = True
) -> None:
    expected = EXPECTED_ENTRY_KINDS if reviewed else EXPECTED_ENTRY_KINDS[:-1]
    _require(
        tuple(entry.get("entry_kind") for entry in entries) == expected,
        "RFV5_EVIDENCE_ENTRY_CLOSURE",
        "transaction entry kinds differ",
    )
    previous: str | None = None
    for sequence, entry in enumerate(entries, 1):
        _require(
            set(entry)
            == {
                "schema",
                "sequence",
                "transaction_id",
                "entry_kind",
                "recorded_by",
                "previous_entry_sha256",
                "payload",
                "entry_sha256",
            },
            "RFV5_EVIDENCE_SCHEMA_INVALID",
            f"entry {sequence} keys differ",
        )
        _require(
            entry["schema"] == ENTRY_SCHEMA
            and entry["sequence"] == sequence
            and entry["transaction_id"] == TRANSACTION_ID
            and entry["previous_entry_sha256"] == previous,
            "RFV5_EVIDENCE_CHAIN_INVALID",
            f"entry {sequence} framing differs",
        )
        digest = entry["entry_sha256"]
        unsigned = {key: value for key, value in entry.items() if key != "entry_sha256"}
        _require(
            isinstance(digest, str)
            and SHA256.fullmatch(digest) is not None
            and digest == canonical_sha256(unsigned),
            "RFV5_EVIDENCE_CHAIN_INVALID",
            f"entry {sequence} digest differs",
        )
        previous = str(digest)


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    _require(
        result.returncode == 0,
        "RFV5_EVIDENCE_GIT_INVALID",
        f"git {' '.join(args)} failed",
    )
    return result.stdout.strip()


def _allowed_capture_path(path: str) -> bool:
    """Allow only the explicitly unrelated user-owned scratch file during capture."""

    return path == "Untitled"


def _validate_capture_candidate(root: Path, candidate: str) -> str:
    _require(
        HEX40.fullmatch(candidate) is not None,
        "RFV5_EVIDENCE_CANDIDATE_INVALID",
        "candidate commit must be a full Git object id",
    )
    head = _git(root, "rev-parse", "HEAD")
    _require(
        candidate == head,
        "RFV5_EVIDENCE_CANDIDATE_NOT_HEAD",
        "capture must execute from the exact candidate HEAD",
    )
    tracked = set(
        filter(None, _git(root, "diff", "--name-only", candidate).splitlines())
    )
    tracked.update(
        filter(
            None, _git(root, "diff", "--cached", "--name-only", candidate).splitlines()
        )
    )
    untracked = set(
        filter(
            None, _git(root, "ls-files", "--others", "--exclude-standard").splitlines()
        )
    )
    unexpected = sorted(
        path for path in tracked | untracked if not _allowed_capture_path(path)
    )
    _require(
        not unexpected,
        "RFV5_EVIDENCE_CANDIDATE_DIRTY",
        "capture candidate has out-of-scope changes: " + ", ".join(unexpected),
    )
    return _git(root, "rev-parse", f"{candidate}^{{tree}}")


def _input_bindings(
    root: Path,
    *,
    candidate: str | None = None,
    release_path: Path = RELEASE_PATH,
) -> list[dict[str, str]]:
    return [
        {
            "path": str(path),
            "sha256": _bytes_sha256(_snapshot_bytes(root, path, candidate=candidate)),
        }
        for path in sorted(
            _expected_input_paths(root, release_path, candidate=candidate), key=str
        )
    ]


def _serialized_specs(specs: Sequence[RunSpec]) -> list[dict[str, object]]:
    return [
        {
            "run_id": spec.run_id,
            "argv": list(spec.argv),
            "selected_count": spec.selected_count,
            "selectors": list(spec.selectors),
            "execution_class": spec.execution_class,
            "recipe": spec.recipe,
        }
        for spec in specs
    ]


def _runner_bindings(root: Path, *, candidate: str | None = None) -> dict[str, object]:
    justfile = _snapshot_text(root, JUSTFILE_PATH, candidate=candidate)
    catalog = (
        _just_recipe_catalog(root)
        if candidate is None
        else _just_recipe_catalog_from_text(justfile)
    )
    recipe_roots = tuple(
        spec.recipe
        for spec in (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
        if spec.recipe is not None
    )
    recipes = _recipe_closure(catalog, recipe_roots)
    return {
        "runner_sha256": _bytes_sha256(
            _snapshot_bytes(root, RUNNER_PATH, candidate=candidate)
        ),
        "runner_test_sha256": _bytes_sha256(
            _snapshot_bytes(root, RUNNER_TEST_PATH, candidate=candidate)
        ),
        "recipe_dependency_closure": [
            {
                "recipe": recipe,
                "sha256": canonical_sha256(catalog[recipe]),
            }
            for recipe in recipes
        ],
        "direct_run_specs_sha256": canonical_sha256(
            {
                "specs": _serialized_specs(
                    tuple(
                        spec
                        for spec in (
                            *BEHAVIOR_RUN_SPECS,
                            *FAULT_RUN_SPECS,
                            *CLEAN_RUN_SPECS,
                        )
                        if spec.recipe is None
                    )
                )
            }
        ),
    }


def _package_identities() -> dict[str, str]:
    packages = (
        "codefabric-cpg-mcp",
        "fastmcp",
        "mcp",
        "pydantic",
        "grpcio",
        "protobuf",
        "opentelemetry-api",
    )
    return {package: importlib.metadata.version(package) for package in packages}


def _version_line(argv: Sequence[str]) -> str:
    result = subprocess.run(argv, check=False, capture_output=True, text=True)
    _require(
        result.returncode == 0 and bool(result.stdout.strip()),
        "RFV5_EVIDENCE_TOOL_IDENTITY_INVALID",
        f"tool identity command failed: {' '.join(argv)}",
    )
    return result.stdout.splitlines()[0].strip()


def _tool_identities() -> dict[str, str]:
    return {
        "rustc": _version_line(("rustc", "--version")),
        "cargo": _version_line(("cargo", "--version")),
        "cargo-nextest": _version_line(("cargo", "nextest", "--version")),
        "just": _version_line(("just", "--version")),
        "uv": _version_line(("uv", "--version")),
        "sccache": _version_line(("sccache", "--version")),
    }


def _opened_payload(
    root: Path,
    candidate: str,
    candidate_tree: str,
    *,
    snapshot_candidate: bool = True,
    release_path: Path = ACTIVE_RELEASE_PATH,
    release_id: str = ACTIVE_RELEASE_ID,
) -> dict[str, object]:
    snapshot = candidate if snapshot_candidate else None
    return {
        "suite": SUITE,
        "packet": "WP48",
        "expectation_release_path": str(release_path),
        "expectation_release_id": release_id,
        "candidate_commit": candidate,
        "candidate_tree": candidate_tree,
        "oracles": list(ORACLES),
        "criteria": list(CRITERIA),
        "input_bindings": _input_bindings(
            root, candidate=snapshot, release_path=release_path
        ),
        "runner_bindings": _runner_bindings(root, candidate=snapshot),
        "observation_mode": "real-installed-supervisor-daemon-fastmcp4",
        "historical_acceptance_inputs": [],
        "captured_at_utc": datetime.now(UTC).isoformat(),
        "environment": {
            "operating_system": platform.system().lower(),
            "architecture": platform.machine(),
            "kernel_release": platform.release(),
            "python_version": platform.python_version(),
            "scope": "local-workstation",
        },
        "package_identities": _package_identities(),
        "tool_identities": _tool_identities(),
        "resource_context": {
            "workspace_daemons": 1,
            "adapter_processes": 2,
            "grpc_channels": 2,
            "stderr_capture_max_bytes": 131_072,
        },
    }


def _default_executor(
    spec: RunSpec, root: Path, environment: Mapping[str, str]
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        spec.argv,
        cwd=root,
        env=dict(environment),
        check=False,
        capture_output=True,
        timeout=3_600,
    )


def _execute(
    spec: RunSpec,
    root: Path,
    environment: Mapping[str, str],
    executor: Executor,
) -> dict[str, object]:
    print(
        json.dumps(
            {
                "event": "wp48-run-started",
                "run_id": spec.run_id,
                "execution_class": spec.execution_class,
            },
            sort_keys=True,
        ),
        file=sys.stderr,
        flush=True,
    )
    try:
        result = executor(spec, root, environment)
    except (OSError, subprocess.SubprocessError) as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_COMMAND_FAILED",
            f"evidence run {spec.run_id} could not complete: {type(error).__name__}",
            details={
                "run_id": spec.run_id,
                "argv": list(spec.argv),
                "execution_class": spec.execution_class,
                "exception_type": type(error).__name__,
            },
        ) from error
    observation = {
        "run_id": spec.run_id,
        "argv": list(spec.argv),
        "exit_code": result.returncode,
        "stdout_sha256": _bytes_sha256(result.stdout),
        "stderr_sha256": _bytes_sha256(result.stderr),
        "stdout_bytes": len(result.stdout),
        "stderr_bytes": len(result.stderr),
        "selectors": list(spec.selectors),
        "execution_class": spec.execution_class,
    }
    if result.returncode != 0:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_COMMAND_FAILED",
            f"evidence run {spec.run_id} exited {result.returncode}",
            details=observation,
        )
    if len(result.stdout) > MAX_STDOUT_BYTES or len(result.stderr) > MAX_STDERR_BYTES:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_COMMAND_OUTPUT_UNBOUNDED",
            f"evidence run {spec.run_id} exceeded its output budget",
            details=observation,
        )
    print(
        json.dumps(
            {
                "event": "wp48-run-passed",
                "run_id": spec.run_id,
                "stdout_bytes": len(result.stdout),
                "stderr_bytes": len(result.stderr),
            },
            sort_keys=True,
        ),
        file=sys.stderr,
        flush=True,
    )
    return {
        "run_id": spec.run_id,
        "argv": list(spec.argv),
        "exit_code": result.returncode,
        "stdout_sha256": _bytes_sha256(result.stdout),
        "stderr_sha256": _bytes_sha256(result.stderr),
        "stdout_bytes": len(result.stdout),
        "stderr_bytes": len(result.stderr),
        "selected_count": spec.selected_count,
        "selectors": list(spec.selectors),
        "execution_class": spec.execution_class,
    }


def _execute_all(
    specs: Sequence[RunSpec],
    root: Path,
    environment: Mapping[str, str],
    executor: Executor,
) -> list[dict[str, object]]:
    return [_execute(spec, root, environment, executor) for spec in specs]


def _expectation_index(bundle: Bundle) -> dict[str, Mapping[str, Any]]:
    return {str(row["claim_id"]): row for row in bundle.expectations}


def _json_pointer(path: tuple[str, ...]) -> str:
    if not path:
        return "/"
    return "/" + "/".join(part.replace("~", "~0").replace("/", "~1") for part in path)


def _observation_diff(
    expected: object, actual: object, path: tuple[str, ...] = ()
) -> set[str]:
    if isinstance(expected, Mapping) and isinstance(actual, Mapping):
        differences: set[str] = set()
        for key in set(expected) | set(actual):
            nested = (*path, str(key))
            if key not in expected or key not in actual:
                differences.add(_json_pointer(nested))
            else:
                differences.update(
                    _observation_diff(expected[key], actual[key], nested)
                )
        return differences
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            return {_json_pointer(path)}
        differences = set()
        for index, (left, right) in enumerate(zip(expected, actual, strict=True)):
            differences.update(_observation_diff(left, right, (*path, str(index))))
        return differences
    if expected != actual:
        return {_json_pointer(path)}
    return set()


def _leaf_paths(value: object, path: tuple[str, ...] = ()) -> set[str]:
    if isinstance(value, Mapping):
        if not value:
            return {_json_pointer(path)}
        result: set[str] = set()
        for key, nested in value.items():
            result.update(_leaf_paths(nested, (*path, str(key))))
        return result
    if isinstance(value, list):
        if not value:
            return {_json_pointer(path)}
        result = set()
        for index, nested in enumerate(value):
            result.update(_leaf_paths(nested, (*path, str(index))))
        return result
    return {_json_pointer(path)}


def _expected_fault_index(bundle: Bundle) -> dict[str, Mapping[str, Any]]:
    return {str(row["fixture_id"]): row for row in bundle.negative}


def _source_row(
    *,
    claim_id: str,
    mode: str,
    actual: Mapping[str, Any],
    source_kind: str,
    field_sources: Mapping[str, Any] | None = None,
) -> dict[str, object]:
    fault = mode == "fault"
    if field_sources is None:
        seam_id = CLAIM_PROBE_IDS[claim_id]
        source_probe_id = CLAIM_PROBE_IDS[claim_id]
        if claim_id in REAL_TOPOLOGY_CLAIMS:
            seam_id, source_probe_id, probe_kind, execution_class = min(
                REAL_FIELD_SOURCE_CONTRACT[claim_id]
            )
        elif claim_id in {"RFV5-FM4-003", "RFV5-FM4-004"}:
            probe_kind = "installed-fastmcp-client"
            execution_class = "installed-presentation-introspection"
        elif claim_id == "RFV5-FM4-013":
            probe_kind = "installed-python-introspection"
            execution_class = "installed-presentation-introspection"
        elif claim_id in {"RFV5-FM4-015", "RFV5-FM4-016"}:
            probe_kind = "frozen-contract-execution"
            execution_class = "frozen-release-validation"
        else:
            probe_kind = "repository-candidate-census"
            execution_class = "isolated-repository-candidate"
        field_sources = {
            path: {
                "seam_id": seam_id,
                "probe_id": source_probe_id,
                "probe_kind": (
                    "git-candidate-history"
                    if claim_id == "RFV5-FM4-014" and path == "/history_bytes_mutated"
                    else probe_kind
                ),
                "execution_class": (
                    "candidate-history-comparison"
                    if claim_id == "RFV5-FM4-014" and path == "/history_bytes_mutated"
                    else execution_class
                ),
                "run_id": (
                    "real-semantic-observations"
                    if claim_id in REAL_TOPOLOGY_CLAIMS
                    else "local-semantic-observations"
                ),
            }
            for path in sorted(_leaf_paths(actual))
        }
    return {
        "claim_id": claim_id,
        "mode": mode,
        "fixture_id": f"{claim_id}-N" if fault else None,
        "probe_id": CLAIM_PROBE_IDS[claim_id],
        "fault_mode": CLAIM_FAULT_MODES[claim_id] if fault else None,
        "source_kind": source_kind,
        "actual_observation": copy.deepcopy(dict(actual)),
        "field_sources": copy.deepcopy(dict(field_sources)),
    }


def _local_field_source_pair(claim_id: str, pointer: str) -> tuple[str, str]:
    if claim_id in {"RFV5-FM4-003", "RFV5-FM4-004"}:
        return "installed-fastmcp-client", "installed-presentation-introspection"
    if claim_id == "RFV5-FM4-013":
        return "installed-python-introspection", "installed-presentation-introspection"
    if claim_id in {"RFV5-FM4-015", "RFV5-FM4-016"}:
        return "frozen-contract-execution", "frozen-release-validation"
    if claim_id == "RFV5-FM4-014" and pointer == "/history_bytes_mutated":
        return "git-candidate-history", "candidate-history-comparison"
    return "repository-candidate-census", "isolated-repository-candidate"


def _validate_source_rows(
    rows: Sequence[Mapping[str, Any]],
    *,
    expected_claims: Sequence[str] | None = None,
) -> dict[tuple[str, str], Mapping[str, Any]]:
    _require(
        set(REAL_FIELD_SOURCE_CONTRACT) == set(REAL_TOPOLOGY_CLAIMS),
        "RFV5_EVIDENCE_FIELD_SOURCE_INVALID",
        "real field-source claim contract differs",
    )
    claims = tuple(CLAIM_RUN_IDS) if expected_claims is None else tuple(expected_claims)
    expected_order = tuple(
        (claim_id, mode) for claim_id in claims for mode in ("normal", "fault")
    )
    _require(
        len(rows) == len(expected_order),
        "RFV5_EVIDENCE_OBSERVATION_CLOSURE",
        "normal/fault observation count differs",
    )
    index: dict[tuple[str, str], Mapping[str, Any]] = {}
    observed_order: list[tuple[str, str]] = []
    for number, row in enumerate(rows):
        _require(
            set(row)
            == {
                "claim_id",
                "mode",
                "fixture_id",
                "probe_id",
                "fault_mode",
                "source_kind",
                "actual_observation",
                "field_sources",
            },
            "RFV5_EVIDENCE_OBSERVATION_SCHEMA",
            f"observation source row {number} keys differ",
        )
        claim_id = str(row["claim_id"])
        mode = str(row["mode"])
        key = (claim_id, mode)
        _require(
            claim_id in claims and mode in {"normal", "fault"} and key not in index,
            "RFV5_EVIDENCE_OBSERVATION_CLOSURE",
            f"duplicate or unknown observation source {claim_id}/{mode}",
        )
        real = claim_id in REAL_TOPOLOGY_CLAIMS
        expected_source = (
            NORMAL_REAL_SOURCE
            if real and mode == "normal"
            else FAULT_REAL_SOURCE
            if real
            else NORMAL_LOCAL_SOURCE
            if mode == "normal"
            else FAULT_LOCAL_SOURCE
        )
        _require(
            row["probe_id"] == CLAIM_PROBE_IDS[claim_id]
            and row["source_kind"] == expected_source
            and (row["fixture_id"], row["fault_mode"])
            == (
                (None, None)
                if mode == "normal"
                else (f"{claim_id}-N", CLAIM_FAULT_MODES[claim_id])
            )
            and isinstance(row["actual_observation"], Mapping)
            and isinstance(row["field_sources"], Mapping),
            "RFV5_EVIDENCE_OBSERVATION_SCHEMA",
            f"observation source metadata differs for {claim_id}/{mode}",
        )
        actual = _mapping(row["actual_observation"], f"{claim_id}/{mode}.actual")
        field_sources = _mapping(
            row["field_sources"], f"{claim_id}/{mode}.field_sources"
        )
        _require(
            set(field_sources) == _leaf_paths(actual),
            "RFV5_EVIDENCE_FIELD_SOURCE_CLOSURE",
            f"field-source closure differs for {claim_id}/{mode}",
        )
        expected_run_id = (
            "real-semantic-observations"
            if claim_id in REAL_TOPOLOGY_CLAIMS
            else "local-semantic-observations"
        )
        allowed_pairs = REAL_FIELD_SOURCE_PAIRS if real else LOCAL_FIELD_SOURCE_PAIRS
        for pointer, raw_descriptor in field_sources.items():
            descriptor = _mapping(raw_descriptor, f"{claim_id}/{mode}{pointer}")
            source_pair = (
                descriptor.get("probe_kind"),
                descriptor.get("execution_class"),
            )
            _require(
                set(descriptor)
                == {
                    "seam_id",
                    "probe_id",
                    "probe_kind",
                    "execution_class",
                    "run_id",
                }
                and isinstance(descriptor["seam_id"], str)
                and SOURCE_IDENTIFIER.fullmatch(descriptor["seam_id"]) is not None
                and isinstance(descriptor["probe_id"], str)
                and SOURCE_IDENTIFIER.fullmatch(descriptor["probe_id"]) is not None
                and descriptor["probe_kind"] in FIELD_SOURCE_KINDS
                and descriptor["execution_class"] in FIELD_EXECUTION_CLASSES
                and source_pair in allowed_pairs
                and descriptor["run_id"] == expected_run_id,
                "RFV5_EVIDENCE_FIELD_SOURCE_INVALID",
                f"invalid field source for {claim_id}/{mode}{pointer}",
            )
            if real:
                exact_source = (
                    descriptor["seam_id"],
                    descriptor["probe_id"],
                    descriptor["probe_kind"],
                    descriptor["execution_class"],
                )
                _require(
                    exact_source in REAL_FIELD_SOURCE_CONTRACT[claim_id],
                    "RFV5_EVIDENCE_FIELD_SOURCE_INVALID",
                    f"real field source is not an allowed executed claim seam for {claim_id}/{mode}{pointer}",
                )
            else:
                expected_kind, expected_class = _local_field_source_pair(
                    claim_id, pointer
                )
                _require(
                    descriptor["seam_id"] == CLAIM_PROBE_IDS[claim_id]
                    and descriptor["probe_id"] == CLAIM_PROBE_IDS[claim_id]
                    and descriptor["probe_kind"] == expected_kind
                    and descriptor["execution_class"] == expected_class,
                    "RFV5_EVIDENCE_FIELD_SOURCE_INVALID",
                    f"local field source is not the executed claim seam for {claim_id}/{mode}{pointer}",
                )
        index[key] = row
        observed_order.append(key)
    _require(
        tuple(observed_order) == expected_order,
        "RFV5_EVIDENCE_OBSERVATION_CLOSURE",
        "normal/fault observation order differs",
    )
    for claim_id in claims:
        normal = _mapping(
            index[(claim_id, "normal")]["field_sources"], "normal sources"
        )
        fault = _mapping(index[(claim_id, "fault")]["field_sources"], "fault sources")
        for pointer in set(normal) & set(fault):
            _require(
                normal[pointer] == fault[pointer],
                "RFV5_EVIDENCE_FIELD_SOURCE_PARITY",
                f"normal/fault source seam differs for {claim_id}{pointer}",
            )
    return index


def _matched_comparison(
    expectation: Mapping[str, Any], actual: Mapping[str, Any]
) -> dict[str, object]:
    expected = _mapping(expectation["expected_observation"], "expected_observation")
    expected_leaves = sorted(_leaf_paths(expected))
    actual_leaves = sorted(_leaf_paths(actual))
    _require(
        actual_leaves == expected_leaves,
        "RFV5_EVIDENCE_OBSERVATION_LEAF_CLOSURE",
        "actual observation omitted or added leaves",
    )
    try:
        validate_observation(expectation, actual)
    except ExpectationReleaseError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_OBSERVATION_MISMATCH",
            str(error),
            details={
                "typed_error": error.code,
                "mismatch_paths": sorted(_observation_diff(expected, actual)),
            },
        ) from error
    return {
        "comparator": "validate_observation",
        "status": "matched",
        "typed_error": None,
        "mismatch_paths": [],
        "expected_leaf_count": len(expected_leaves),
        "actual_leaf_count": len(actual_leaves),
        "leaf_paths_sha256": canonical_sha256({"paths": expected_leaves}),
    }


def _rejected_comparison(
    expectation: Mapping[str, Any],
    fixture: Mapping[str, Any],
    normal: Mapping[str, Any],
    actual: Mapping[str, Any],
    field_sources: Mapping[str, Any] | None = None,
) -> dict[str, object]:
    expected = _mapping(expectation["expected_observation"], "expected_observation")
    paths = sorted(_observation_diff(expected, actual))
    _require(
        actual != normal and paths,
        "RFV5_EVIDENCE_FAULT_NOT_DISCRIMINATING",
        f"{fixture['fixture_id']} fault observation survived",
    )
    try:
        validate_observation(expectation, actual)
    except ExpectationReleaseError as error:
        typed_error = error.code
    else:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_FAULT_NOT_CAUGHT",
            str(fixture["fixture_id"]),
        )
    declared = sorted(str(path) for path in fixture["expected_mismatch_paths"])
    if field_sources is not None:
        sourced = set(field_sources)
        unsupported = [
            path
            for path in declared
            if not any(
                pointer == path
                or pointer.startswith(f"{path}/")
                or path.startswith(f"{pointer}/")
                for pointer in sourced
            )
        ]
        _require(
            not unsupported,
            "RFV5_EVIDENCE_FAULT_SOURCE_MISSING",
            f"{fixture['fixture_id']} mismatch paths lack a fault source",
        )
    _require(
        paths == declared and typed_error == fixture["expected_error"],
        "RFV5_EVIDENCE_FAULT_MISMATCH",
        f"{fixture['fixture_id']} paths or typed error differ",
    )
    return {
        "comparator": "validate_observation",
        "status": "rejected",
        "typed_error": typed_error,
        "mismatch_paths": paths,
        "normal_observation_sha256": canonical_sha256(normal),
        "fault_observation_sha256": canonical_sha256(actual),
    }


def _claim_payload(
    bundle: Bundle,
    runs: Sequence[Mapping[str, object]],
    observations: Sequence[Mapping[str, Any]],
) -> dict[str, object]:
    expectations = _expectation_index(bundle)
    sources = _validate_source_rows(observations)
    claims = []
    for claim_id, run_ids in CLAIM_RUN_IDS.items():
        expectation = expectations[claim_id]
        expected = _mapping(expectation["expected_observation"], "expected_observation")
        source = sources[(claim_id, "normal")]
        actual = _mapping(
            source["actual_observation"], f"{claim_id}.actual_observation"
        )
        claims.append(
            {
                "claim_id": claim_id,
                "family": expectation["family"],
                "source_kind": source["source_kind"],
                "probe_id": source["probe_id"],
                "expected_observation": copy.deepcopy(dict(expected)),
                "actual_observation": copy.deepcopy(dict(actual)),
                "field_sources": copy.deepcopy(dict(source["field_sources"])),
                "comparison": _matched_comparison(expectation, actual),
                "run_ids": list(run_ids),
                "selected_count": len(run_ids),
            }
        )
    return {
        "runs": list(runs),
        "claims": claims,
        "substrate_run_ids": list(SUBSTRATE_RUN_IDS),
    }


def _fault_payload(
    bundle: Bundle,
    fault_runs: Sequence[Mapping[str, object]],
    behavior_runs: Sequence[Mapping[str, object]],
    observations: Sequence[Mapping[str, Any]],
) -> dict[str, object]:
    fixtures = _expected_fault_index(bundle)
    expectations = _expectation_index(bundle)
    sources = _validate_source_rows(observations)
    return {
        "runs": list(fault_runs),
        "faults": [
            {
                "layer": layer,
                "run_id": run_id,
                "evidence_kind": "executed-causal-fault-run",
                "selected_count": 1,
            }
            for layer, run_id in FAULT_RUN_IDS.items()
        ],
        "independent_fixture_run_id": "independent-negative-fixtures",
        "independent_fixture_count": 16,
        "negative_fixture_production_map": [
            {
                "fixture_id": fixture_id,
                "claim_id": fixtures[fixture_id]["claim_id"],
                "source_kind": sources[
                    (str(fixtures[fixture_id]["claim_id"]), "fault")
                ]["source_kind"],
                "probe_id": sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                    "probe_id"
                ],
                "fault_mode": sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                    "fault_mode"
                ],
                "actual_observation": copy.deepcopy(
                    dict(
                        _mapping(
                            sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                                "actual_observation"
                            ],
                            f"{fixture_id}.actual_observation",
                        )
                    )
                ),
                "normal_observation": copy.deepcopy(
                    dict(
                        _mapping(
                            sources[(str(fixtures[fixture_id]["claim_id"]), "normal")][
                                "actual_observation"
                            ],
                            f"{fixture_id}.normal_observation",
                        )
                    )
                ),
                "normal_field_sources": copy.deepcopy(
                    dict(
                        _mapping(
                            sources[(str(fixtures[fixture_id]["claim_id"]), "normal")][
                                "field_sources"
                            ],
                            f"{fixture_id}.normal_field_sources",
                        )
                    )
                ),
                "field_sources": copy.deepcopy(
                    dict(
                        _mapping(
                            sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                                "field_sources"
                            ],
                            f"{fixture_id}.field_sources",
                        )
                    )
                ),
                "comparison": _rejected_comparison(
                    expectations[str(fixtures[fixture_id]["claim_id"])],
                    fixtures[fixture_id],
                    _mapping(
                        sources[(str(fixtures[fixture_id]["claim_id"]), "normal")][
                            "actual_observation"
                        ],
                        f"{fixture_id}.normal_observation",
                    ),
                    _mapping(
                        sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                            "actual_observation"
                        ],
                        f"{fixture_id}.fault_observation",
                    ),
                    _mapping(
                        sources[(str(fixtures[fixture_id]["claim_id"]), "fault")][
                            "field_sources"
                        ],
                        f"{fixture_id}.field_sources",
                    ),
                ),
                "run_ids": list(run_ids),
            }
            for fixture_id, run_ids in NEGATIVE_FIXTURE_RUN_IDS.items()
        ],
        "production_run_ids": sorted(
            {
                str(row["run_id"])
                for row in (*behavior_runs, *fault_runs)
                if str(row["run_id"]) != "independent-negative-fixtures"
            }
        ),
    }


def _clean_payload(runs: Sequence[Mapping[str, object]]) -> dict[str, object]:
    return {
        "runs": list(runs),
        "fresh_build_root": True,
        "fresh_runtime_root": True,
        "installed_wheel_only": True,
        "predecessor_model_present": False,
        "static_adapter_schema_present": False,
        "stale_descriptor_present": False,
        "cached_epoch_present": False,
        "representative_source_mutation_repeated": True,
        "process_restart_repeated": True,
        "ephemeral_root_removed": True,
        "compiler_cache_reads_disabled": True,
    }


def _limitations_payload() -> dict[str, object]:
    return {
        "limitations": [
            "One Linux local-workstation environment is observed by WP48.",
            "WP49 owns physical predecessor purge after this transaction passes.",
            "WP50 owns representative resource and performance evidence.",
        ],
        "skipped_candidates": [],
        "unparsed_candidates": [],
        "verdict_scope": "WP48-development-evidence-not-final-release",
    }


def _real_observation_request(
    candidate_commit: str, candidate_tree: str, evidence_root: Path
) -> dict[str, object]:
    return {
        "schema": OBSERVATION_REQUEST_SCHEMA,
        "candidate_commit": candidate_commit,
        "candidate_tree": candidate_tree,
        "evidence_root": str(evidence_root),
        "cases": [
            {
                "claim_id": claim_id,
                "mode": mode,
                "fixture_id": f"{claim_id}-N" if mode == "fault" else None,
            }
            for claim_id in REAL_TOPOLOGY_CLAIMS
            for mode in ("normal", "fault")
        ],
    }


def _write_private_json(path: Path, value: Mapping[str, object]) -> None:
    encoded = json.dumps(value, separators=(",", ":"), sort_keys=True).encode() + b"\n"
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        os.write(descriptor, encoded)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _load_real_observation_report(
    path: Path, candidate_commit: str, candidate_tree: str
) -> list[Mapping[str, Any]]:
    report = _load_json(path)
    _require(
        set(report)
        == {
            "schema",
            "candidate_commit",
            "candidate_tree",
            "producer",
            "observations",
        }
        and report["schema"] == REAL_OBSERVATION_SCHEMA
        and report["candidate_commit"] == candidate_commit
        and report["candidate_tree"] == candidate_tree
        and report["producer"] == REAL_OBSERVATION_PRODUCER,
        "RFV5_EVIDENCE_OBSERVATION_SCHEMA",
        "real-topology observation envelope differs",
    )
    rows = _rows(report["observations"], "real-topology observations")
    _validate_source_rows(rows, expected_claims=REAL_TOPOLOGY_CLAIMS)
    return rows


def _schema_field(schema: Mapping[str, Any]) -> dict[str, object]:
    alternatives = schema.get("anyOf")
    selected: Mapping[str, Any] = schema
    if isinstance(alternatives, list):
        selected = next(
            (
                _mapping(value, "schema alternative")
                for value in alternatives
                if isinstance(value, Mapping) and value.get("type") != "null"
            ),
            schema,
        )
    return {
        key: copy.deepcopy(selected[key] if key in selected else schema[key])
        for key in ("type", "enum", "default", "pattern")
        if key in selected or key in schema
    }


def _installed_presentation_observation(claim_id: str, mode: str) -> Mapping[str, Any]:
    """Observe a live FastMCP server; fault modes mutate that server before census."""

    if claim_id not in {"RFV5-FM4-003", "RFV5-FM4-004"} or mode not in {
        "normal",
        "fault",
    }:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_UNKNOWN", f"{claim_id}/{mode}"
        )
    import codefabric_cpg_mcp.server as server_module
    import fastmcp
    from codefabric_cpg_mcp.daemon import (
        AuthorityGeneration,
        SafeError,
        ValidationRejection,
    )
    from codefabric_cpg_mcp.server import create_server
    from codefabric_cpg_mcp.settings import Settings
    from fastmcp import Client
    from pydantic import SecretStr

    settings = Settings(
        format="codefabric.adapter-launch.v1",
        query_socket=Path("/tmp/codefabric-wp48-observation.sock"),
        launch_grant_hex=SecretStr("ab" * 32),
        adapter_program=Path(sys.executable).resolve(),
        adapter_arguments=("-m", "codefabric_cpg_mcp"),
        daemon_generation=7,
        supervisor_generation=11,
        session_expires_at_unix_ms=int(time.time() * 1000) + 120_000,
        maximum_request_state_ttl_seconds=1,
    )

    class ObservationPort:
        def __init__(self) -> None:
            self.settings = settings
            self.seen_requests: list[Mapping[str, Any]] = []

        def current_settings(self) -> Any:
            return self.settings

        async def connect(self, *, correlation_id: str) -> None:
            _ = correlation_id

        async def close(self) -> None:
            return None

        async def start_query(
            self, value: Any, *, correlation_id: str
        ) -> ValidationRejection:
            _ = correlation_id
            self.seen_requests.append(copy.deepcopy(dict(value.request)))
            return ValidationRejection(
                authority=AuthorityGeneration(
                    session_id="wp48-observation-session",
                    session_generation=1,
                    daemon_generation=settings.daemon_generation,
                    supervisor_generation=settings.supervisor_generation,
                    policy_generation=1,
                    revocation_generation=0,
                ),
                semantic_request_id="semantic:wp48-observation",
                issues=(),
                error=SafeError(
                    code="VALIDATION_REJECTED",
                    layer="VALIDATION",
                    retryable=False,
                    correlation_id="wp48-observation",
                ),
            )

    observation_port = ObservationPort()
    server = create_server(settings, lambda _settings: observation_port)

    async def observe() -> Mapping[str, Any]:
        if claim_id == "RFV5-FM4-003" and mode == "fault":
            from fastmcp.server.extensions import ServerExtension

            class UiFaultExtension(ServerExtension):
                identifier = "io.modelcontextprotocol/ui"

                def settings(self) -> dict[str, Any]:
                    return {"enabled": True}

            class CustomFaultExtension(ServerExtension):
                identifier = "dev.codefabric/custom"

            @server.prompt(name="author_code_graph_query")
            def author_code_graph_query() -> str:
                return "fault-mode prompt"

            server._support_tasks_by_default = True
            server.add_extension(UiFaultExtension())
            server.add_extension(CustomFaultExtension())
            query_tool = await server._local_provider.get_tool("query_code_graph")
            _require(
                query_tool is not None,
                "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
                "query tool missing before UI fault injection",
            )
            query_tool.meta = {"ui": {"component": "query-dashboard"}}
        if claim_id == "RFV5-FM4-004" and mode == "fault":
            server.strict_input_validation = False
            Settings.model_config["frozen"] = False
            Settings.model_config["extra"] = "allow"
            fastmcp.settings.mcp_camelcase_compat = True
            query_tool = await server._local_provider.get_tool("query_code_graph")
            _require(
                query_tool is not None,
                "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
                "query tool missing before schema fault injection",
            )
            query_tool.parameters["properties"]["freshness"] = {"type": "string"}
            query_tool.parameters["properties"]["daemon_port"] = {"type": "string"}
            query_tool.parameters["additionalProperties"] = True
            original_query = query_tool.fn

            @functools.wraps(original_query)
            async def adapter_rewriting_query(*args: Any, **kwargs: Any) -> Any:
                if "request" in kwargs:
                    rewritten = dict(kwargs["request"])
                    rewritten["wp48_adapter_rewrite"] = True
                    kwargs["request"] = rewritten
                elif args:
                    rewritten = dict(args[0])
                    rewritten["wp48_adapter_rewrite"] = True
                    args = (rewritten, *args[1:])
                return await original_query(*args, **kwargs)

            query_tool.fn = adapter_rewriting_query

        async with Client(server, mode="auto", cache=False) as client:
            tools = await client.list_tools()
            templates = await client.list_resource_templates()
            prompts = await client.list_prompts()
            discovery = client.session.discover_result
            _require(
                discovery is not None,
                "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
                "installed server discovery is absent",
            )
            if claim_id == "RFV5-FM4-003":
                framework = discovery.capabilities.extensions or {}
                ui_components = sorted(
                    {
                        str(tool.meta["ui"]["component"])
                        for tool in tools
                        if isinstance(tool.meta, Mapping)
                        and isinstance(tool.meta.get("ui"), Mapping)
                        and isinstance(tool.meta["ui"].get("component"), str)
                    }
                )
                return {
                    "tools": sorted(tool.name for tool in tools),
                    "resource_families": sorted(
                        template.name for template in templates
                    ),
                    "completion_handlers": (
                        [server._completion_handler.__name__]
                        if server._completion_handler is not None
                        else []
                    ),
                    "prompts": sorted(prompt.name for prompt in prompts),
                    "application_extensions": sorted(
                        identifier
                        for identifier in server._extensions
                        if identifier != "io.modelcontextprotocol/ui"
                    ),
                    "framework_extensions": copy.deepcopy(dict(framework)),
                    "ui_components": ui_components,
                    "providers": sorted(
                        type(provider).__name__
                        for provider in server.providers
                        if type(provider).__name__ != "LocalProvider"
                    ),
                    "transforms": sorted(
                        type(value).__name__ for value in server.transforms
                    ),
                    "sessions": getattr(server, "_session_manager", None) is not None,
                    "task_default": (
                        "optional" if server._support_tasks_by_default else "forbidden"
                    ),
                }

            sentinel_request = {"form": "symbol_lookup", "symbol": "wp48"}
            await client.call_tool(
                "query_code_graph",
                {"request": sentinel_request},
            )
            _require(
                len(observation_port.seen_requests) == 1,
                "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
                "query schema probe did not dispatch exactly once to the daemon port",
            )
            adapter_rewrote_request = (
                observation_port.seen_requests[0] != sentinel_request
            )

            schema_tools: dict[str, object] = {}
            for tool in tools:
                schema = _mapping(tool.input_schema, f"{tool.name}.input_schema")
                properties = _mapping(
                    schema.get("properties"), f"{tool.name}.properties"
                )
                required = [str(value) for value in schema.get("required", [])]
                fields: dict[str, object] = {}
                for name, raw_field in properties.items():
                    field = _schema_field(_mapping(raw_field, f"{tool.name}.{name}"))
                    fields[str(name)] = field
                schema_tools[tool.name] = {
                    "required": required,
                    "optional": [name for name in properties if name not in required],
                    "additional_properties": schema.get("additionalProperties") is True,
                    "fields": fields,
                }
            source = inspect.getsource(server_module)
            hidden = []
            if "Context" in source and "_CURRENT_CONTEXT" in source:
                hidden.append("context")
            if "Depends(daemon_port)" in source:
                hidden.append("daemon_port")
            if "channel/session reference" in source and "DaemonPort" in source:
                hidden.append("daemon_session")
            if "_correlation_id(" in source:
                hidden.append("request_correlation")
            public_fields = {
                name
                for tool in schema_tools.values()
                for name in _mapping(tool, "tool schema")["fields"]
            }
            return {
                "model_policy": {
                    "strict": server.strict_input_validation,
                    "frozen": bool(Settings.model_config.get("frozen")),
                    "extra": str(Settings.model_config.get("extra")),
                    "compatibility_bridge": bool(fastmcp.settings.mcp_camelcase_compat),
                },
                "tools": schema_tools,
                "hidden_dependencies": hidden,
                "hidden_dependencies_in_schema": bool(public_fields & set(hidden)),
                "semantic_request_authority": (
                    "python-adapter" if adapter_rewrote_request else "rust-daemon"
                ),
                "adapter_semantic_request_rewrites": adapter_rewrote_request,
                "wire_alias_style": "camelCase",
                "python_attribute_style": "camelCase"
                if mode == "fault"
                else "snake_case",
            }

    return asyncio.run(observe())


def _run_installed_probe(root: Path, claim_id: str, mode: str) -> Mapping[str, Any]:
    result = subprocess.run(
        [
            "uv",
            "run",
            "--frozen",
            "--project",
            str(root / "codefabric-cpg-mcp"),
            "python",
            str(root / RUNNER_PATH),
            "local-source-probe",
            "--claim-id",
            claim_id,
            "--mode",
            mode,
        ],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        timeout=300,
    )
    _require(
        result.returncode == 0 and not result.stderr and result.stdout.count("\n") == 1,
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_FAILED",
        f"installed presentation probe failed for {claim_id}/{mode}",
    )
    try:
        report = _mapping(json.loads(result.stdout), "installed presentation probe")
    except json.JSONDecodeError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_FAILED",
            f"installed presentation probe emitted invalid JSON: {error}",
        ) from error
    _require(
        set(report) == {"schema", "claim_id", "mode", "actual_observation"}
        and report["schema"] == LOCAL_PROBE_SCHEMA
        and report["claim_id"] == claim_id
        and report["mode"] == mode,
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_FAILED",
        "installed presentation probe envelope differs",
    )
    return _mapping(report["actual_observation"], "installed actual observation")


def _frontmatter(path: Path) -> Mapping[str, Any]:
    text = path.read_text(encoding="utf-8")
    _require(
        text.startswith("---\n") and "\n---\n" in text[4:],
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
        f"{path} lacks closed frontmatter",
    )
    document = yaml.safe_load(text.split("\n---\n", 1)[0][4:])
    return _mapping(document, f"{path} frontmatter")


def _dependency_pins(root: Path) -> dict[str, str]:
    project = tomllib.loads(
        (root / "codefabric-cpg-mcp/pyproject.toml").read_text(encoding="utf-8")
    )
    dependencies = _mapping(_mapping(project, "pyproject")["project"], "project")[
        "dependencies"
    ]
    pins = {
        str(value).split("==", 1)[0]: str(value).split("==", 1)[1]
        for value in dependencies
        if isinstance(value, str) and "==" in value
    }
    pins["python"] = (
        (root / "codefabric-cpg-mcp/.python-version")
        .read_text(encoding="utf-8")
        .strip()
    )
    return pins


def _observe_claim_001(root: Path) -> Mapping[str, Any]:
    design_root = root / "docs/authoritative_design"
    documents = [
        (path, _frontmatter(path))
        for path in sorted(design_root.glob("*.md"))
        if path.read_text(encoding="utf-8").startswith("---\n")
    ]
    suite_docs = [
        (path, meta)
        for path, meta in documents
        if meta.get("artifact_tag") == "SUITE"
        and meta.get("suite_id") == "codefabric-relational-data-fabric"
    ]
    referenced = {
        str(meta["predecessor_path"])
        for _path, meta in suite_docs
        if isinstance(meta.get("predecessor_path"), str)
    }
    terminals = [
        (path, meta)
        for path, meta in suite_docs
        if str(path.relative_to(root)) not in referenced
    ]
    _require(
        bool(terminals),
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
        "suite graph has no terminal",
    )
    selected_path, selected = max(
        terminals,
        key=lambda item: tuple(
            int(part) for part in str(item[1]["suite_version"]).split(".")
        ),
    )
    predecessor = _frontmatter(root / str(selected["predecessor_path"]))
    selected_files = [
        meta
        for path, meta in documents
        if (
            str(meta.get("suite_version")) == str(selected["suite_version"])
            or path.name.endswith("_v2.3.md")
        )
        and meta.get("artifact_tag")
    ]
    tag_order = {
        tag: index
        for index, tag in enumerate(
            ["SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"]
        )
    }
    member_tags = sorted(
        {str(meta["artifact_tag"]) for meta in selected_files},
        key=lambda tag: tag_order.get(tag, len(tag_order)),
    )
    pins = _dependency_pins(root)
    server_source = (
        root / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py"
    ).read_text(encoding="utf-8")
    protocol = re.search(r'(?m)^MODERN_PROTOCOL_VERSION\s*=\s*"([^"]+)"', server_source)
    _require(
        protocol is not None,
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
        "modern protocol constant is absent",
    )
    bridge_enabled = bool(re.search(r"mcp_camelcase_compat\s*=\s*True", server_source))
    predecessor_runtime = pins.get("fastmcp", "").startswith("3.") or pins.get(
        "mcp", ""
    ).startswith("1.")
    _ = selected_path
    return {
        "selected_suite": {
            "suite_id": selected["suite_id"],
            "suite_version": selected["suite_version"],
            "member_tags": member_tags,
            "predecessor_version": predecessor["suite_version"],
            "sole_terminal": len(terminals) == 1,
        },
        "runtime_pins": {
            "python": pins["python"],
            "fastmcp": pins["fastmcp"],
            "mcp": pins["mcp"],
            "pydantic": pins["pydantic"],
        },
        "protocol_version": protocol.group(1),
        "compatibility_bridge": bridge_enabled,
        "predecessor_runtime_authority": predecessor_runtime,
    }


AUTHORITY_CENSUS_TOKENS = {
    "application_extensions": r"\badd_extension\s*\(",
    "ui_components": r"\bui_component(?:s)?\b",
    "semantic_registries": r"\bsemantic_registry\b",
    "session_registries": r"\bsession_registry\b",
    "task_registries": r"\btask_registry\b",
    "response_caches": r"\bresponse_cache\b",
    "resource_lease_maps": r"\bresource_lease_map\b|\b_resource_leases\b",
    "arrow_decoders": r"\b(?:pyarrow|polars)\b",
    "datafusion_planners": r"\bdatafusion(?:_planner)?\b",
    "delta_readers": r"\b(?:deltalake|delta_reader)\b",
    "canonical_request_rewriters": r"\bcanonical_request_rewriter\b",
    "mutable_cpg_state": r"\bmutable_cpg_state\b",
}


def _python_source_corpus(root: Path) -> str:
    source_root = root / "codefabric-cpg-mcp/src"
    paths = sorted(source_root.rglob("*.py"))
    _require(
        bool(paths),
        "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
        "adapter source census selected no Python files",
    )
    for path in paths:
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    return "\n".join(path.read_text(encoding="utf-8") for path in paths)


def _observe_claim_013(root: Path) -> Mapping[str, Any]:
    corpus = _python_source_corpus(root)
    server = (root / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py").read_text(
        encoding="utf-8"
    )
    result: dict[str, Any] = {
        "lifespan_channels_per_adapter": len(
            re.findall(r"\bport\s*=\s*daemon_factory\(settings\)", server)
        )
    }
    result.update(
        {
            name: len(re.findall(pattern, corpus))
            for name, pattern in AUTHORITY_CENSUS_TOKENS.items()
        }
    )
    return result


DECOMMISSION_TOKENS = {
    "live_fastmcp_3_pins": r"fastmcp==3[.]",
    "live_mcp_1_pins": r"(?<!fast)mcp==1[.]",
    "camelcase_bridge_usage": r"FASTMCP_MCP_CAMELCASE_COMPAT[^\n]*true|mcp_camelcase_compat\s*=\s*True",
    "import_time_global_servers": r"(?m)^server\s*=\s*create_server\(",
    "normal_path_validate_then_start": r"validate_query[\s\S]{0,160}start_query",
    "duplicate_freshness_fields": r"\bduplicate_freshness\b",
    "python_resource_lease_maps": r"\b_resource_leases\b|\bresource_lease_map\b",
    "python_generated_public_handles": r"\bpython_generated_public_handle\b",
    "random_mcp_call_ids": r"\brandom_mcp_call_id\b",
    "random_rpc_attempt_ids": r"\brandom_rpc_attempt_id\b",
    "phantom_prompts": r"@(?:server|mcp)[.]prompt\b|[.]add_prompt\(",
    "pydantic_settings_dependency": r"\bpydantic_settings\b|pydantic-settings",
    "dead_arrow_resource_gate_references": r"\barrow_resource_gate\b",
    "predecessor_runtime_fallbacks": r"\bpredecessor_runtime_fallback\b",
}


def _live_runtime_corpus(root: Path) -> str:
    roots = (
        Path("Cargo.toml"),
        Path("codefabric-cpg-mcp/pyproject.toml"),
        Path("codefabric-cpg-mcp/src"),
        Path("src"),
        Path("scripts/run_fastmcp4_modern_client.sh"),
    )
    texts: list[str] = []
    for relative in roots:
        path = root / relative
        if path.is_file():
            texts.append(path.read_text(encoding="utf-8"))
        elif path.is_dir():
            texts.extend(
                candidate.read_text(encoding="utf-8")
                for candidate in sorted(path.rglob("*"))
                if candidate.is_file()
                and candidate.suffix in {".py", ".rs", ".toml", ".sh"}
            )
    return "\n".join(texts)


def _observe_claim_014(
    root: Path, *, history_bytes_mutated: int = 0
) -> Mapping[str, Any]:
    corpus = _live_runtime_corpus(root)
    result = {
        name: len(re.findall(pattern, corpus, flags=re.IGNORECASE))
        for name, pattern in DECOMMISSION_TOKENS.items()
    }
    result["live_static_adapter_schemas"] = sum(
        1 for path in (root / "contracts/adapter").glob("**/*") if path.is_file()
    )
    result["history_bytes_mutated"] = history_bytes_mutated
    return result


def _historical_acceptance_snapshot(root: Path) -> dict[str, bytes]:
    history_roots = (
        Path("contracts/acceptance/relational-fabric-v5"),
        Path("contracts/evidence/relational-fabric-v5"),
    )
    return {
        str(path.relative_to(root)): path.read_bytes()
        for relative in history_roots
        for path in sorted((root / relative).glob("**/*"))
        if path.is_file()
    }


def _mutated_history_byte_count(
    before: Mapping[str, bytes], after: Mapping[str, bytes]
) -> int:
    mutated = 0
    for path in set(before) | set(after):
        left = before.get(path, b"")
        right = after.get(path, b"")
        shared = min(len(left), len(right))
        mutated += sum(left[index] != right[index] for index in range(shared))
        mutated += abs(len(left) - len(right))
    return mutated


def _observe_claim_015(root: Path, release_path: Path) -> Mapping[str, Any]:
    document = _mapping(
        yaml.safe_load(
            (root / release_path / "performance-method.yaml").read_text(
                encoding="utf-8"
            )
        ),
        "performance method",
    )
    registration = _mapping(document["registration"], "performance registration")
    environment = _mapping(document["environment_record"], "environment record")
    raw_control = document.get("minimal_control")
    control = raw_control if isinstance(raw_control, Mapping) else {}
    method = _mapping(document["execution_method"], "execution method")
    budget = _mapping(document["budget_source"], "budget source")
    workloads = _rows(document["workloads"], "performance workloads")
    return {
        "performance_contract_path": str(release_path / "performance-method.yaml"),
        "environment_record_required": bool(environment.get("required_fields")),
        "minimal_fastmcp4_control_required": bool(control),
        "same_host_interleaving_required": (
            "interleaved" in str(method.get("ordering", ""))
            and "candidate_and_control_use_different_hosts"
            in environment.get("invalid_when", [])
        ),
        "warmups_per_case": method["warmups_per_case"],
        "samples_per_case": method["samples_per_case"],
        "report_distribution": list(method["distributions"]),
        "candidate_neutral_budget_source": budget["source_id"],
        "local_relaxation_permitted": registration["local_relaxation_permitted"],
        "measured_dimensions": [str(row["workload_id"]) for row in workloads],
    }


def _observe_claim_016(root: Path, release_path: Path) -> Mapping[str, Any]:
    release = root / release_path
    issuance = _mapping(
        yaml.safe_load((release / "issuance.yaml").read_text(encoding="utf-8")),
        "issuance",
    )
    actual_files = sorted(path.name for path in release.iterdir() if path.is_file())
    source_bindings = _immutable_source_bindings(
        root, release_path, verify_hashes=False
    )
    bundle = load_bundle(root, release_path)
    source_verified = sum(
        _file_sha256(root / path) == digest for path, digest in source_bindings
    )
    artifact_verified = sum(
        (release / name).is_file() and _file_sha256(release / name) == digest
        for name, digest in bundle.spec.frozen_bytes_sha256.items()
    )
    validator_source = (root / EXPECTATION_VALIDATOR_PATH).read_text(encoding="utf-8")
    runner_source = (root / RUNNER_PATH).read_text(encoding="utf-8")
    capture_source = runner_source.split("\ndef capture_transaction(", 1)[-1].split(
        "\ndef ", 1
    )[0]
    validation_position = capture_source.find("validate_issuance(")
    execution_position = capture_source.find("_execute_all(")
    return {
        "frozen_files": sorted(EXPECTED_FILES),
        "selector_names": sorted(_mapping(issuance["selectors"], "selectors")),
        "source_input_hashes_verified": source_verified,
        "artifact_hashes_verified": artifact_verified,
        "extra_release_files": len(set(actual_files) - set(EXPECTED_FILES)),
        "restamp_on_failure": "RESTAMP_ON_FAILURE = True" in validator_source,
        "dependent_execution_stops_on_drift": (
            validation_position >= 0
            and execution_position >= 0
            and validation_position < execution_position
        ),
    }


def _copy_candidate_paths(root: Path, destination: Path, paths: Sequence[Path]) -> None:
    for relative in paths:
        source = root / relative
        target = destination / relative
        _require(
            source.exists(),
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
            f"local candidate input is absent: {relative}",
        )
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(
                source,
                target,
                ignore=shutil.ignore_patterns("__pycache__", "*.pyc", ".pytest_cache"),
            )
        else:
            shutil.copy2(source, target)


def _fault_candidate(root: Path, claim_id: str, release_path: Path) -> tuple[Path, Any]:
    temporary = tempfile.TemporaryDirectory(prefix=f"wp48-{claim_id.lower()}-")
    candidate = Path(temporary.name)
    common = (
        Path("codefabric-cpg-mcp/pyproject.toml"),
        Path("codefabric-cpg-mcp/.python-version"),
        Path("codefabric-cpg-mcp/src"),
    )
    immutable_source_paths: tuple[Path, ...] = ()
    if claim_id == "RFV5-FM4-001":
        paths = (*common, Path("docs/authoritative_design"))
    elif claim_id == "RFV5-FM4-013":
        paths = common
    elif claim_id == "RFV5-FM4-014":
        paths = (
            *common,
            Path("Cargo.toml"),
            Path("src"),
            Path("scripts/run_fastmcp4_modern_client.sh"),
        )
    elif claim_id == "RFV5-FM4-015":
        paths = (release_path / "performance-method.yaml",)
    elif claim_id == "RFV5-FM4-016":
        immutable_source_paths = tuple(
            path for path, _digest in _immutable_source_bindings(root, release_path)
        )
        paths = (
            release_path,
            EXPECTATION_VALIDATOR_PATH,
            RUNNER_PATH,
            *immutable_source_paths,
        )
    else:
        temporary.cleanup()
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_UNKNOWN", claim_id
        )
    _copy_candidate_paths(root, candidate, paths)

    if claim_id == "RFV5-FM4-001":
        suite = candidate / (
            "docs/authoritative_design/"
            "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md"
        )
        text = suite.read_text(encoding="utf-8")
        text = text.replace("suite_version: 2.3.0", "suite_version: 2.2.0", 1)
        text = text.replace(
            "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.2.md",
            "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.1.md",
            1,
        )
        suite.write_text(text, encoding="utf-8")
        duplicate = (
            candidate / "docs/authoritative_design/wp48_fault_second_terminal.md"
        )
        duplicate.write_text(text, encoding="utf-8")
        project = candidate / "codefabric-cpg-mcp/pyproject.toml"
        project.write_text(
            project.read_text(encoding="utf-8")
            .replace("fastmcp==4.0.0", "fastmcp==3.4.7")
            .replace("mcp==2.1.1", "mcp==1.29.0"),
            encoding="utf-8",
        )
    elif claim_id == "RFV5-FM4-013":
        seed = candidate / "codefabric-cpg-mcp/src/wp48_authority_fault.py"
        seed.write_text(
            """def seed(server):
    server.add_extension(object())
ui_component = {}
semantic_registry = {}
session_registry = {}
task_registry = {}
response_cache = {}
resource_lease_map = {}
arrow_decoder = __import__('pyarrow')
canonical_request_rewriter = object()
mutable_cpg_state = {}
""",
            encoding="utf-8",
        )
    elif claim_id == "RFV5-FM4-014":
        project = candidate / "codefabric-cpg-mcp/pyproject.toml"
        project.write_text(
            project.read_text(encoding="utf-8")
            .replace("fastmcp==4.0.0", "fastmcp==3.4.7")
            .replace("mcp==2.1.1", "mcp==1.29.0"),
            encoding="utf-8",
        )
        seed = candidate / "codefabric-cpg-mcp/src/wp48_predecessor_fault.py"
        seed.write_text(
            """FASTMCP_MCP_CAMELCASE_COMPAT = 'true'
_resource_leases = {}
predecessor_runtime_fallback = True
def old_path():
    validate_query()
    start_query()
@server.prompt()
def old_prompt_one(): ...
@server.prompt()
def old_prompt_two(): ...
""",
            encoding="utf-8",
        )
        schema = candidate / "contracts/adapter/stale.schema.json"
        schema.parent.mkdir(parents=True, exist_ok=True)
        schema.write_text("{}\n", encoding="utf-8")
    elif claim_id == "RFV5-FM4-015":
        path = candidate / release_path / "performance-method.yaml"
        document = _mapping(
            yaml.safe_load(path.read_text(encoding="utf-8")), "performance"
        )
        mutable = copy.deepcopy(dict(document))
        mutable.pop("minimal_control", None)
        mutable["registration"]["local_relaxation_permitted"] = True
        mutable["execution_method"]["ordering"] = "candidate-only-sequential"
        mutable["execution_method"]["warmups_per_case"] = 0
        mutable["execution_method"]["samples_per_case"] = 3
        mutable["budget_source"]["source_id"] = "candidate-result-local-threshold"
        path.write_text(yaml.safe_dump(mutable, sort_keys=False), encoding="utf-8")
    elif claim_id == "RFV5-FM4-016":
        release = candidate / release_path
        source = candidate / min(immutable_source_paths)
        source.write_text(
            source.read_text(encoding="utf-8") + "\nwp48 fault\n", encoding="utf-8"
        )
        artifact = release / "causal-fixtures.yaml"
        artifact.write_text(
            artifact.read_text(encoding="utf-8") + "\n# wp48 fault\n", encoding="utf-8"
        )
        (release / "extra.yaml").write_text("fault: true\n", encoding="utf-8")
        validator = candidate / EXPECTATION_VALIDATOR_PATH
        validator.write_text(
            validator.read_text(encoding="utf-8") + "\nRESTAMP_ON_FAILURE = True\n",
            encoding="utf-8",
        )
        runner = candidate / RUNNER_PATH
        runner_text = runner.read_text(encoding="utf-8")
        prefix, separator, capture_source = runner_text.partition(
            "\ndef capture_transaction("
        )
        _require(
            bool(separator),
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
            "capture function is absent from the fault candidate",
        )
        issuance_call = re.search(r"(?m)^    validate_issuance\(", capture_source)
        _require(
            issuance_call is not None,
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_INVALID",
            "capture function has no issuance gate to reorder",
        )
        assert issuance_call is not None
        capture_source = (
            capture_source[: issuance_call.start()]
            + "    _execute_all((), root, {}, _default_executor)\n"
            + capture_source[issuance_call.start() :]
        )
        runner.write_text(
            prefix + separator + capture_source,
            encoding="utf-8",
        )
    return candidate, temporary


def _local_actual_observation(
    root: Path, claim_id: str, mode: str, release_path: Path
) -> Mapping[str, Any]:
    if claim_id in {"RFV5-FM4-003", "RFV5-FM4-004"}:
        return _run_installed_probe(root, claim_id, mode)
    candidate = root
    temporary: Any | None = None
    history_before = _historical_acceptance_snapshot(root)
    if mode == "fault":
        candidate, temporary = _fault_candidate(root, claim_id, release_path)
    try:
        if claim_id == "RFV5-FM4-001":
            return _observe_claim_001(candidate)
        if claim_id == "RFV5-FM4-013":
            return _observe_claim_013(candidate)
        if claim_id == "RFV5-FM4-014":
            return _observe_claim_014(
                candidate,
                history_bytes_mutated=_mutated_history_byte_count(
                    history_before,
                    _historical_acceptance_snapshot(root),
                ),
            )
        if claim_id == "RFV5-FM4-015":
            return _observe_claim_015(candidate, release_path)
        if claim_id == "RFV5-FM4-016":
            return _observe_claim_016(candidate, release_path)
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_OBSERVATION_SOURCE_UNKNOWN", claim_id
        )
    finally:
        if temporary is not None:
            temporary.cleanup()


def _emit_local_observations(
    root: Path,
    output_path: Path,
    release_path: Path,
    candidate_commit: str,
    candidate_tree: str,
) -> int:
    rows = [
        _source_row(
            claim_id=claim_id,
            mode=mode,
            actual=_local_actual_observation(root, claim_id, mode, release_path),
            source_kind=(
                NORMAL_LOCAL_SOURCE if mode == "normal" else FAULT_LOCAL_SOURCE
            ),
        )
        for claim_id in LOCAL_OBSERVATION_CLAIMS
        for mode in ("normal", "fault")
    ]
    _validate_source_rows(rows, expected_claims=LOCAL_OBSERVATION_CLAIMS)
    _write_private_json(
        output_path,
        {
            "schema": LOCAL_OBSERVATION_SCHEMA,
            "candidate_commit": candidate_commit,
            "candidate_tree": candidate_tree,
            "producer": "python-installed-and-repository-candidate-observer",
            "observations": rows,
        },
    )
    return len(rows)


def _collect_local_observations(
    root: Path,
    candidate_commit: str,
    candidate_tree: str,
    release_path: Path,
    environment: Mapping[str, str],
    executor: Executor,
) -> tuple[list[Mapping[str, Any]], dict[str, object]]:
    parent = root / "target" / "wp48-local-observations"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=parent, prefix="source-") as temporary:
        evidence_root = Path(temporary).resolve()
        output_path = evidence_root / "observations.json"
        source_environment = dict(environment)
        source_environment["CODEFABRIC_WP48_LOCAL_OBSERVATION_OUTPUT_PATH"] = str(
            output_path
        )
        source_environment["CODEFABRIC_WP48_CANDIDATE_COMMIT"] = candidate_commit
        source_environment["CODEFABRIC_WP48_CANDIDATE_TREE"] = candidate_tree
        source_environment["CODEFABRIC_WP48_EXPECTATION_RELEASE_PATH"] = str(
            release_path
        )
        run = _execute(LOCAL_OBSERVATION_RUN_SPEC, root, source_environment, executor)
        report = _load_json(output_path)
        _require(
            set(report)
            == {
                "schema",
                "candidate_commit",
                "candidate_tree",
                "producer",
                "observations",
            }
            and report["schema"] == LOCAL_OBSERVATION_SCHEMA
            and report["candidate_commit"] == candidate_commit
            and report["candidate_tree"] == candidate_tree
            and report["producer"]
            == "python-installed-and-repository-candidate-observer",
            "RFV5_EVIDENCE_OBSERVATION_SCHEMA",
            "local observation envelope differs",
        )
        rows = _rows(report["observations"], "local observations")
        _validate_source_rows(rows, expected_claims=LOCAL_OBSERVATION_CLAIMS)
    return rows, run


ObservationCollector = Callable[
    [
        Path,
        str,
        str,
        Path,
        Mapping[str, str],
        Executor,
    ],
    tuple[list[Mapping[str, Any]], Mapping[str, Mapping[str, object]]],
]


def _collect_observations(
    root: Path,
    candidate_commit: str,
    candidate_tree: str,
    release_path: Path,
    environment: Mapping[str, str],
    executor: Executor,
) -> tuple[list[Mapping[str, Any]], Mapping[str, Mapping[str, object]]]:
    real_rows, real_run = _collect_real_observations(
        root, candidate_commit, candidate_tree, environment, executor
    )
    local_rows, local_run = _collect_local_observations(
        root,
        candidate_commit,
        candidate_tree,
        release_path,
        environment,
        executor,
    )
    source_index = {
        (str(row["claim_id"]), str(row["mode"])): row
        for row in (*real_rows, *local_rows)
    }
    rows = [
        source_index[(claim_id, mode)]
        for claim_id in CLAIM_RUN_IDS
        for mode in ("normal", "fault")
    ]
    _validate_source_rows(rows)
    return rows, {
        REAL_OBSERVATION_RUN_SPEC.run_id: real_run,
        LOCAL_OBSERVATION_RUN_SPEC.run_id: local_run,
    }


def _collect_real_observations(
    root: Path,
    candidate_commit: str,
    candidate_tree: str,
    environment: Mapping[str, str],
    executor: Executor,
) -> tuple[list[Mapping[str, Any]], dict[str, object]]:
    parent = root / "target" / "wp48-production-observations"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=parent, prefix="source-") as temporary:
        evidence_root = Path(temporary).resolve()
        os.chmod(evidence_root, 0o700)
        request_path = evidence_root / "request.json"
        output_path = evidence_root / "observations.json"
        _write_private_json(
            request_path,
            _real_observation_request(candidate_commit, candidate_tree, evidence_root),
        )
        source_environment = dict(environment)
        source_environment["CODEFABRIC_WP48_OBSERVATION_REQUEST_PATH"] = str(
            request_path
        )
        source_environment["CODEFABRIC_WP48_OBSERVATION_OUTPUT_PATH"] = str(output_path)
        run = _execute(REAL_OBSERVATION_RUN_SPEC, root, source_environment, executor)
        _require(
            output_path.is_file()
            and not output_path.is_symlink()
            and output_path.stat().st_mode & 0o777 == 0o600,
            "RFV5_EVIDENCE_OBSERVATION_OUTPUT_INVALID",
            "real-topology observer did not create one private regular report",
        )
        rows = _load_real_observation_report(
            output_path, candidate_commit, candidate_tree
        )
    _require(
        not evidence_root.exists(),
        "RFV5_EVIDENCE_OBSERVATION_OUTPUT_INVALID",
        "real-topology observation root was not removed",
    )
    return rows, run


def _record_failed_attempt(
    root: Path,
    candidate_commit: str,
    candidate_tree: str,
    error: ProductionEvidenceError,
    *,
    release_path: Path,
    release_id: str,
    snapshot_candidate: bool,
) -> Path:
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S%fZ")
    path = (
        root
        / "contracts/evidence/relational-fabric-v5"
        / f"wp48-production-evidence-failed-{timestamp}.jsonl"
    )
    entries: list[dict[str, Any]] = []
    _append(
        entries,
        "transaction_opened",
        _opened_payload(
            root,
            candidate_commit,
            candidate_tree,
            snapshot_candidate=snapshot_candidate,
            release_path=release_path,
            release_id=release_id,
        ),
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "attempt_failed",
        {
            "code": error.code,
            "message": str(error),
            "accepted_as_success": False,
            "failure_observation": error.details,
            "limitations": [
                "This append-only attempt is retained as unfavorable evidence.",
                "A new attempt must rerun every closed command from the candidate.",
            ],
        },
        recorder="wp48-production-evidence-executor",
    )
    _write_entries(path, entries, exclusive=True)
    return path


def _snapshot_release_identity(
    root: Path, release_path: Path, *, candidate: str | None
) -> str:
    """Read the selected release identity from the same snapshot as its bindings."""

    _expected_input_paths(root, release_path, candidate=candidate)
    try:
        value = yaml.safe_load(
            _snapshot_text(
                root,
                release_path / "issuance.yaml",
                candidate=candidate,
            )
        )
    except yaml.YAMLError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
            f"expectation issuance is not valid YAML: {error}",
        ) from error
    issuance = _mapping(value, "expectation issuance")
    release_id = issuance.get("release_id")
    _require(
        issuance.get("schema") == "codefabric.fastmcp4-successor.issuance.v1"
        and isinstance(release_id, str)
        and bool(release_id),
        "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
        "expectation issuance schema or release identity differs",
    )
    assert isinstance(release_id, str)
    return release_id


@contextmanager
def _bound_expectation_bundle(
    root: Path,
    opened: Mapping[str, Any],
    *,
    check_git: bool,
) -> Iterator[Bundle]:
    """Validate and expose WP43 bytes from the bound candidate, not the descendant."""

    release_path = Path(str(opened["expectation_release_path"]))
    commit = str(opened["candidate_commit"])
    try:
        if not check_git:
            yield validate_issuance(
                root=root,
                require_review=True,
                release_path=release_path,
            )
            return

        with tempfile.TemporaryDirectory(prefix="wp48-candidate-release-") as temporary:
            snapshot_root = Path(temporary)
            for relative in sorted(
                _expected_input_paths(root, release_path, candidate=commit), key=str
            ):
                destination = snapshot_root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(_git_blob(root, commit, relative))
            yield validate_issuance(
                root=snapshot_root,
                require_review=True,
                release_path=release_path,
            )
    except ExpectationReleaseError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_EXPECTATION_DRIFT",
            str(error),
            details={"typed_error": error.code},
        ) from error


def capture_transaction(
    root: Path,
    candidate_commit: str,
    output_path: Path,
    *,
    executor: Executor = _default_executor,
    observation_collector: ObservationCollector = _collect_observations,
    release_path: Path = ACTIVE_RELEASE_PATH,
    check_git: bool = True,
) -> int:
    """Execute every WP48 evidence leg and create an unreviewed transaction."""

    output = _under_root(root, output_path, "transaction output")
    _require(
        not output.exists(),
        "RFV5_EVIDENCE_APPEND_ONLY",
        f"transaction already exists: {output}",
    )
    validate_issuance(root=root, require_review=True, release_path=release_path)
    bundle = load_bundle(root, release_path)
    release_id = str(bundle.issuance.get("release_id", ""))
    _require(
        bool(release_id),
        "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
        "WP43 expectation release has no identity",
    )
    _require(
        validate_independent_review(bundle) == 16
        and validate_negative_fixtures(bundle) == 16,
        "RFV5_EVIDENCE_EXPECTATION_DRIFT",
        "WP43 expectation release is not closed",
    )
    candidate_tree = (
        _validate_capture_candidate(root, candidate_commit) if check_git else "2" * 40
    )
    _validate_recipe_specs(
        root,
        (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS),
        candidate=candidate_commit if check_git else None,
    )
    environment = dict(os.environ)
    environment["CARGO_INCREMENTAL"] = "0"
    try:
        observations, semantic_runs = observation_collector(
            root,
            candidate_commit,
            candidate_tree,
            release_path,
            environment,
            executor,
        )
        behavior_runs = [
            dict(semantic_runs[spec.run_id])
            if spec.run_id in semantic_runs
            else _execute(spec, root, environment, executor)
            for spec in BEHAVIOR_RUN_SPECS
        ]
        _require(
            set(semantic_runs)
            == {
                REAL_OBSERVATION_RUN_SPEC.run_id,
                LOCAL_OBSERVATION_RUN_SPEC.run_id,
            },
            "RFV5_EVIDENCE_OBSERVATION_RUN_CLOSURE",
            "semantic observation run closure differs",
        )
        fault_runs = _execute_all(FAULT_RUN_SPECS, root, environment, executor)

        clean_parent = root / "target" / "wp48-clean-reconstruction"
        clean_parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            dir=clean_parent, prefix="attempt-"
        ) as temporary:
            clean_root = Path(temporary)
            clean_environment = dict(environment)
            clean_environment["CARGO_TARGET_DIR"] = str(clean_root / "cargo-target")
            clean_environment["UV_CACHE_DIR"] = str(clean_root / "uv-cache")
            clean_environment["SCCACHE_RECACHE"] = "1"
            clean_runs = _execute_all(
                CLEAN_RUN_SPECS, root, clean_environment, executor
            )
            _require(
                (clean_root / "cargo-target").exists(),
                "RFV5_EVIDENCE_CLEAN_RECONSTRUCTION_INVALID",
                "fresh Cargo target was not constructed",
            )
        _require(
            not clean_root.exists(),
            "RFV5_EVIDENCE_CLEAN_RECONSTRUCTION_INVALID",
            "ephemeral clean reconstruction root was not removed",
        )
    except ProductionEvidenceError as error:
        _record_failed_attempt(
            root,
            candidate_commit,
            candidate_tree,
            error,
            release_path=release_path,
            release_id=release_id,
            snapshot_candidate=check_git,
        )
        raise

    entries: list[dict[str, Any]] = []
    recorder = "wp48-production-evidence-executor"
    _append(
        entries,
        "transaction_opened",
        _opened_payload(
            root,
            candidate_commit,
            candidate_tree,
            snapshot_candidate=check_git,
            release_path=release_path,
            release_id=release_id,
        ),
        recorder=recorder,
    )
    _append(
        entries,
        "claim_observation_map",
        _claim_payload(bundle, behavior_runs, observations),
        recorder=recorder,
    )
    _append(
        entries,
        "causal_fault_map",
        _fault_payload(bundle, fault_runs, behavior_runs, observations),
        recorder=recorder,
    )
    _append(
        entries,
        "clean_reconstruction_contract",
        _clean_payload(clean_runs),
        recorder=recorder,
    )
    _append(entries, "limitations_recorded", _limitations_payload(), recorder=recorder)
    try:
        _write_entries(output, entries, exclusive=True)
    except FileExistsError as error:
        raise ProductionEvidenceError(
            "RFV5_EVIDENCE_APPEND_ONLY", f"transaction already exists: {output}"
        ) from error
    return len(CLAIM_RUN_IDS)


def _validate_opened(
    payload: Mapping[str, Any], root: Path, *, check_git: bool
) -> Path:
    _require(
        set(payload)
        == {
            "suite",
            "packet",
            "expectation_release_path",
            "expectation_release_id",
            "candidate_commit",
            "candidate_tree",
            "oracles",
            "criteria",
            "input_bindings",
            "runner_bindings",
            "observation_mode",
            "historical_acceptance_inputs",
            "captured_at_utc",
            "environment",
            "package_identities",
            "tool_identities",
            "resource_context",
        },
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "transaction-open payload keys differ",
    )
    _require(
        payload["suite"] == SUITE
        and payload["packet"] == "WP48"
        and tuple(payload["oracles"]) == ORACLES
        and tuple(payload["criteria"]) == CRITERIA,
        "RFV5_EVIDENCE_AUTHORITY_DRIFT",
        "suite, packet, oracle, or criterion closure differs",
    )
    commit = str(payload["candidate_commit"])
    tree = str(payload["candidate_tree"])
    release_path = Path(str(payload["expectation_release_path"]))
    _require(
        HEX40.fullmatch(commit) is not None and HEX40.fullmatch(tree) is not None,
        "RFV5_EVIDENCE_CANDIDATE_INVALID",
        "candidate commit or tree identity differs",
    )
    if check_git:
        _require(
            _git(root, "rev-parse", f"{commit}^{{tree}}") == tree,
            "RFV5_EVIDENCE_CANDIDATE_INVALID",
            "candidate tree does not match its Git commit",
        )
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", commit, "HEAD"],
            cwd=root,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        _require(
            result.returncode == 0,
            "RFV5_EVIDENCE_CANDIDATE_NOT_ANCESTRAL",
            "bound candidate is not ancestral to HEAD",
        )
    snapshot = commit if check_git else None
    _require(
        payload["expectation_release_id"]
        == _snapshot_release_identity(root, release_path, candidate=snapshot),
        "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID",
        "bound expectation release identity differs from its candidate issuance",
    )
    expected_bindings = {
        row["path"]: row["sha256"]
        for row in _input_bindings(
            root,
            candidate=snapshot,
            release_path=release_path,
        )
    }
    bindings = _rows(payload["input_bindings"], "input_bindings")
    observed_paths: set[Path] = set()
    for binding in bindings:
        _require(
            set(binding) == {"path", "sha256"},
            "RFV5_EVIDENCE_SCHEMA_INVALID",
            "input binding keys differ",
        )
        path = Path(str(binding["path"]))
        _require(
            not path.is_absolute()
            and ".." not in path.parts
            and path not in observed_paths,
            "RFV5_EVIDENCE_INPUT_CLOSURE",
            f"invalid or duplicate input path {path}",
        )
        observed_paths.add(path)
        digest = str(binding["sha256"])
        _require(
            SHA256.fullmatch(digest) is not None
            and expected_bindings.get(str(path)) == digest,
            "RFV5_EVIDENCE_INPUT_DRIFT",
            f"bound input drifted: {path}",
        )
    _require(
        observed_paths == _expected_input_paths(root, release_path, candidate=snapshot),
        "RFV5_EVIDENCE_INPUT_CLOSURE",
        "bound input path closure differs",
    )
    _require(
        payload["runner_bindings"] == _runner_bindings(root, candidate=snapshot),
        "RFV5_EVIDENCE_RUNNER_DRIFT",
        "runner, test, recipe, or direct-selector binding differs",
    )
    _require(
        payload["observation_mode"] == "real-installed-supervisor-daemon-fastmcp4"
        and payload["historical_acceptance_inputs"] == [],
        "RFV5_EVIDENCE_FAKE_TOPOLOGY",
        "evidence admitted fake or historical authority",
    )
    environment = _mapping(payload["environment"], "environment")
    packages = _mapping(payload["package_identities"], "package_identities")
    tools = _mapping(payload["tool_identities"], "tool_identities")
    resources = _mapping(payload["resource_context"], "resource_context")
    _require(
        set(environment)
        == {
            "operating_system",
            "architecture",
            "kernel_release",
            "python_version",
            "scope",
        }
        and environment.get("operating_system") == "linux"
        and environment.get("python_version") == "3.14.7"
        and environment.get("scope") == "local-workstation",
        "RFV5_EVIDENCE_ENVIRONMENT_INVALID",
        "environment differs from the supported evidence profile",
    )
    _require(
        packages
        == {
            "codefabric-cpg-mcp": "0.1.0",
            "fastmcp": "4.0.0",
            "mcp": "2.1.1",
            "pydantic": "2.13.4",
            "grpcio": "1.83.0",
            "protobuf": "7.36.0",
            "opentelemetry-api": "1.44.0",
        },
        "RFV5_EVIDENCE_PACKAGE_IDENTITY_INVALID",
        "installed package identity differs",
    )
    _require(
        tools
        == {
            "rustc": "rustc 1.98.0 (88d9e12ae 2026-08-18)",
            "cargo": "cargo 1.98.0 (797e8a9bc 2026-08-05)",
            "cargo-nextest": "cargo-nextest 0.9.143 (60fa45f63 2026-08-04)",
            "just": "just 1.58.0",
            "uv": "uv 0.12.7 (x86_64-unknown-linux-gnu)",
            "sccache": "sccache 0.17.0",
        },
        "RFV5_EVIDENCE_TOOL_IDENTITY_INVALID",
        "repository tool identity differs",
    )
    _require(
        resources
        == {
            "workspace_daemons": 1,
            "adapter_processes": 2,
            "grpc_channels": 2,
            "stderr_capture_max_bytes": 131_072,
        },
        "RFV5_EVIDENCE_RESOURCE_CONTEXT_INVALID",
        "process/resource topology differs",
    )
    return release_path


def _validate_run(row: Mapping[str, Any], spec: RunSpec) -> None:
    _require(
        set(row)
        == {
            "run_id",
            "argv",
            "exit_code",
            "stdout_sha256",
            "stderr_sha256",
            "stdout_bytes",
            "stderr_bytes",
            "selected_count",
            "selectors",
            "execution_class",
        },
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        f"run {spec.run_id} keys differ",
    )
    _require(
        row["run_id"] == spec.run_id
        and tuple(row["argv"]) == spec.argv
        and row["exit_code"] == 0
        and row["selected_count"] == spec.selected_count
        and tuple(row["selectors"]) == spec.selectors
        and row["execution_class"] == spec.execution_class,
        "RFV5_EVIDENCE_RUN_DRIFT",
        f"run {spec.run_id} contract differs",
    )
    _require(
        SHA256.fullmatch(str(row["stdout_sha256"])) is not None
        and SHA256.fullmatch(str(row["stderr_sha256"])) is not None
        and isinstance(row["stdout_bytes"], int)
        and 0 <= row["stdout_bytes"] <= MAX_STDOUT_BYTES
        and isinstance(row["stderr_bytes"], int)
        and 0 <= row["stderr_bytes"] <= MAX_STDERR_BYTES,
        "RFV5_EVIDENCE_RUN_OUTPUT_INVALID",
        f"run {spec.run_id} output metadata differs",
    )


def _validate_runs(value: object, specs: Sequence[RunSpec], context: str) -> set[str]:
    rows = _rows(value, context)
    _require(
        len(rows) == len(specs),
        "RFV5_EVIDENCE_RUN_CLOSURE",
        f"{context} count differs",
    )
    for row, spec in zip(rows, specs, strict=True):
        _validate_run(row, spec)
    return {spec.run_id for spec in specs}


def _validate_claim_map(payload: Mapping[str, Any], bundle: Bundle) -> None:
    _require(
        set(payload) == {"runs", "claims", "substrate_run_ids"},
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "claim-map keys differ",
    )
    run_ids = _validate_runs(payload["runs"], BEHAVIOR_RUN_SPECS, "behavior runs")
    expectations = _expectation_index(bundle)
    rows = _rows(payload["claims"], "claims")
    _require(
        len(rows) == len(CLAIM_RUN_IDS),
        "RFV5_EVIDENCE_CLAIM_CLOSURE",
        "claim count differs",
    )
    for row, (claim_id, expected_runs) in zip(rows, CLAIM_RUN_IDS.items(), strict=True):
        expectation = expectations[claim_id]
        expected = _mapping(expectation["expected_observation"], "expected_observation")
        actual = _mapping(
            row.get("actual_observation"), f"{claim_id}.actual_observation"
        )
        comparison = _mapping(row.get("comparison"), f"{claim_id}.comparison")
        recomputed = _matched_comparison(expectation, actual)
        source_kind = (
            NORMAL_REAL_SOURCE
            if claim_id in REAL_TOPOLOGY_CLAIMS
            else NORMAL_LOCAL_SOURCE
        )
        _require(
            set(row)
            == {
                "claim_id",
                "family",
                "source_kind",
                "probe_id",
                "expected_observation",
                "actual_observation",
                "field_sources",
                "comparison",
                "run_ids",
                "selected_count",
            }
            and row["claim_id"] == claim_id
            and row["family"] == expectation["family"]
            and row["source_kind"] == source_kind
            and row["probe_id"] == CLAIM_PROBE_IDS[claim_id]
            and row["expected_observation"] == expected
            and comparison == recomputed
            and tuple(row["run_ids"]) == expected_runs
            and row["selected_count"] == len(expected_runs)
            and set(expected_runs).issubset(run_ids),
            "RFV5_EVIDENCE_CLAIM_CLOSURE",
            f"claim mapping differs for {claim_id}",
        )
    _require(
        tuple(payload["substrate_run_ids"]) == SUBSTRATE_RUN_IDS
        and set(SUBSTRATE_RUN_IDS).issubset(run_ids),
        "RFV5_EVIDENCE_SUBSTRATE_CLOSURE",
        "source-to-presentation substrate run closure differs",
    )


def _validate_fault_map(payload: Mapping[str, Any], bundle: Bundle) -> None:
    _require(
        set(payload)
        == {
            "runs",
            "faults",
            "independent_fixture_run_id",
            "independent_fixture_count",
            "negative_fixture_production_map",
            "production_run_ids",
        },
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "fault-map keys differ",
    )
    run_ids = _validate_runs(payload["runs"], FAULT_RUN_SPECS, "fault runs")
    rows = _rows(payload["faults"], "faults")
    _require(
        len(rows) == len(FAULT_RUN_IDS),
        "RFV5_EVIDENCE_FAULT_CLOSURE",
        "fault count differs",
    )
    for row, (layer, run_id) in zip(rows, FAULT_RUN_IDS.items(), strict=True):
        _require(
            set(row) == {"layer", "run_id", "evidence_kind", "selected_count"}
            and row["layer"] == layer
            and row["run_id"] == run_id
            and row["evidence_kind"] == "executed-causal-fault-run"
            and row["selected_count"] == 1
            and run_id in run_ids,
            "RFV5_EVIDENCE_FAULT_CLOSURE",
            f"fault run metadata differs for {layer}",
        )
    _require(
        payload["independent_fixture_run_id"] == "independent-negative-fixtures"
        and payload["independent_fixture_count"] == 16
        and "independent-negative-fixtures" in run_ids,
        "RFV5_EVIDENCE_FIXTURE_DRIFT",
        "independent negative-fixture execution differs",
    )
    behavior_run_ids = {spec.run_id for spec in BEHAVIOR_RUN_SPECS}
    production_run_ids = (run_ids | behavior_run_ids) - {
        "independent-negative-fixtures"
    }
    _require(
        payload["production_run_ids"] == sorted(production_run_ids),
        "RFV5_EVIDENCE_FIXTURE_DRIFT",
        "negative-fixture production run closure differs",
    )
    fixtures = {str(row["fixture_id"]): row for row in bundle.negative}
    rows = _rows(
        payload["negative_fixture_production_map"],
        "negative_fixture_production_map",
    )
    _require(
        len(rows) == len(NEGATIVE_FIXTURE_RUN_IDS) == 16,
        "RFV5_EVIDENCE_FIXTURE_DRIFT",
        "negative-fixture production map count differs",
    )
    for row, (fixture_id, expected_runs) in zip(
        rows, NEGATIVE_FIXTURE_RUN_IDS.items(), strict=True
    ):
        fixture = fixtures[fixture_id]
        claim_id = str(fixture["claim_id"])
        expectation = _expectation_index(bundle)[claim_id]
        normal = _mapping(
            row.get("normal_observation"),
            f"{claim_id}.normal_observation",
        )
        actual = _mapping(row.get("actual_observation"), f"{fixture_id}.actual")
        comparison = _mapping(row.get("comparison"), f"{fixture_id}.comparison")
        recomputed = _rejected_comparison(
            expectation,
            fixture,
            normal,
            actual,
            _mapping(row.get("field_sources"), f"{fixture_id}.field_sources"),
        )
        source_kind = (
            FAULT_REAL_SOURCE
            if claim_id in REAL_TOPOLOGY_CLAIMS
            else FAULT_LOCAL_SOURCE
        )
        _require(
            set(row)
            == {
                "fixture_id",
                "claim_id",
                "source_kind",
                "probe_id",
                "fault_mode",
                "normal_observation",
                "normal_field_sources",
                "actual_observation",
                "field_sources",
                "comparison",
                "run_ids",
            }
            and row["fixture_id"] == fixture_id
            and row["claim_id"] == claim_id
            and row["source_kind"] == source_kind
            and row["probe_id"] == CLAIM_PROBE_IDS[claim_id]
            and row["fault_mode"] == CLAIM_FAULT_MODES[claim_id]
            and comparison == recomputed
            and tuple(row["run_ids"]) == expected_runs
            and set(expected_runs).issubset(production_run_ids),
            "RFV5_EVIDENCE_FIXTURE_DRIFT",
            f"negative fixture production mapping differs for {fixture_id}",
        )


def _validate_recorded_observation_sources(
    claim_payload: Mapping[str, Any], fault_payload: Mapping[str, Any]
) -> None:
    claims = {
        str(row["claim_id"]): row
        for row in _rows(claim_payload["claims"], "recorded claims")
    }
    faults = {
        str(row["claim_id"]): row
        for row in _rows(
            fault_payload["negative_fixture_production_map"],
            "recorded fixture observations",
        )
    }
    rows: list[Mapping[str, Any]] = []
    for claim_id in CLAIM_RUN_IDS:
        claim = claims[claim_id]
        fault = faults[claim_id]
        rows.append(
            {
                "claim_id": claim_id,
                "mode": "normal",
                "fixture_id": None,
                "probe_id": claim["probe_id"],
                "fault_mode": None,
                "source_kind": claim["source_kind"],
                "actual_observation": claim["actual_observation"],
                "field_sources": claim["field_sources"],
            }
        )
        rows.append(
            {
                "claim_id": claim_id,
                "mode": "fault",
                "fixture_id": fault["fixture_id"],
                "probe_id": fault["probe_id"],
                "fault_mode": fault["fault_mode"],
                "source_kind": fault["source_kind"],
                "actual_observation": fault["actual_observation"],
                "field_sources": fault["field_sources"],
            }
        )
        _require(
            fault["normal_observation"] == claim["actual_observation"]
            and fault["normal_field_sources"] == claim["field_sources"],
            "RFV5_EVIDENCE_OBSERVATION_PARITY",
            f"fault record is not paired to the normal source for {claim_id}",
        )
    _validate_source_rows(rows)


def _validate_clean_contract(payload: Mapping[str, Any]) -> None:
    expected_keys = {
        "runs",
        "fresh_build_root",
        "fresh_runtime_root",
        "installed_wheel_only",
        "predecessor_model_present",
        "static_adapter_schema_present",
        "stale_descriptor_present",
        "cached_epoch_present",
        "representative_source_mutation_repeated",
        "process_restart_repeated",
        "ephemeral_root_removed",
        "compiler_cache_reads_disabled",
    }
    _require(
        set(payload) == expected_keys,
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "clean reconstruction keys differ",
    )
    _validate_runs(payload["runs"], CLEAN_RUN_SPECS, "clean runs")
    _require(
        payload["fresh_build_root"] is True
        and payload["fresh_runtime_root"] is True
        and payload["installed_wheel_only"] is True
        and payload["predecessor_model_present"] is False
        and payload["static_adapter_schema_present"] is False
        and payload["stale_descriptor_present"] is False
        and payload["cached_epoch_present"] is False
        and payload["representative_source_mutation_repeated"] is True
        and payload["process_restart_repeated"] is True
        and payload["ephemeral_root_removed"] is True
        and payload["compiler_cache_reads_disabled"] is True,
        "RFV5_EVIDENCE_CLEAN_RECONSTRUCTION_INVALID",
        "clean reconstruction admitted stale, predecessor, cache, or source-import state",
    )


def _validate_limitations(payload: Mapping[str, Any]) -> None:
    _require(
        set(payload)
        == {"limitations", "skipped_candidates", "unparsed_candidates", "verdict_scope"}
        and isinstance(payload["limitations"], list)
        and payload["skipped_candidates"] == []
        and payload["unparsed_candidates"] == []
        and payload["verdict_scope"] == "WP48-development-evidence-not-final-release",
        "RFV5_EVIDENCE_COVERAGE_GAP",
        "limitations, skipped candidates, or verdict scope differs",
    )


def _review_payload(
    review: Mapping[str, Any], review_path: Path, digest: str
) -> dict[str, object]:
    return {
        "review_artifact_path": str(review_path),
        "review_artifact_sha256": digest,
        "review_report_path": review["review_report_path"],
        "review_report_sha256": review["review_report_sha256"],
        "reviewer_identity": review["reviewer_identity"],
        "reviewed_through_entry_sha256": review["reviewed_through_entry_sha256"],
        "implementation_owner": review["reviewer_is_implementation_owner"],
        "expectation_author": review["reviewer_is_expectation_author"],
        "verdict": review["verdict"],
        "scope": review["scope"],
        "findings": review["findings"],
    }


def _validate_review_document(
    review: Mapping[str, Any], reviewed_tip: str, root: Path
) -> None:
    reviewer_identity = review.get("reviewer_identity")
    _require(
        set(review)
        == {
            "schema",
            "transaction_id",
            "reviewer_identity",
            "reviewed_through_entry_sha256",
            "review_report_path",
            "review_report_sha256",
            "reviewer_is_implementation_owner",
            "reviewer_is_expectation_author",
            "verdict",
            "scope",
            "findings",
        }
        and review["schema"] == REVIEW_SCHEMA
        and review["transaction_id"] == TRANSACTION_ID
        and isinstance(reviewer_identity, str)
        and bool(reviewer_identity)
        and reviewer_identity
        not in {
            "wp48-production-evidence-executor",
            "codex-wp43-independent-expectation-author",
        }
        and review["reviewed_through_entry_sha256"] == reviewed_tip
        and review["review_report_path"] == str(REVIEW_REPORT_PATH)
        and SHA256.fullmatch(str(review["review_report_sha256"])) is not None
        and review["reviewer_is_implementation_owner"] is False
        and review["reviewer_is_expectation_author"] is False
        and review["verdict"] == "accepted"
        and tuple(review["scope"]) == REVIEW_SCOPE
        and review["findings"] == [],
        "RFV5_EVIDENCE_REVIEW_REJECTED",
        "independent review is incomplete, drifted, or not accepted",
    )
    report_path = root / REVIEW_REPORT_PATH
    _require(
        report_path.is_file()
        and _file_sha256(report_path) == review["review_report_sha256"],
        "RFV5_EVIDENCE_REVIEW_DRIFT",
        "independent Markdown review is absent or drifted",
    )
    report = report_path.read_text(encoding="utf-8")
    _require(
        reviewed_tip in report
        and all(scope in report for scope in REVIEW_SCOPE)
        and "ACCEPTED" in report,
        "RFV5_EVIDENCE_REVIEW_REJECTED",
        "independent Markdown review lacks its exact tip, scope, or accepted verdict",
    )


def _validate_review(payload: Mapping[str, Any], reviewed_tip: str, root: Path) -> None:
    expected_keys = {
        "review_artifact_path",
        "review_artifact_sha256",
        "review_report_path",
        "review_report_sha256",
        "reviewer_identity",
        "reviewed_through_entry_sha256",
        "implementation_owner",
        "expectation_author",
        "verdict",
        "scope",
        "findings",
    }
    _require(
        set(payload) == expected_keys,
        "RFV5_EVIDENCE_SCHEMA_INVALID",
        "review entry keys differ",
    )
    path = Path(str(payload["review_artifact_path"]))
    _require(
        not path.is_absolute() and ".." not in path.parts,
        "RFV5_EVIDENCE_REVIEW_DRIFT",
        "review artifact path is not repository-relative",
    )
    digest = _file_sha256(root / path)
    _require(
        payload["review_artifact_sha256"] == digest,
        "RFV5_EVIDENCE_REVIEW_DRIFT",
        "review artifact digest differs",
    )
    review = _load_json(root / path)
    _validate_review_document(review, reviewed_tip, root)
    _require(
        dict(payload) == _review_payload(review, path, digest),
        "RFV5_EVIDENCE_REVIEW_DRIFT",
        "review entry differs from its independent artifact",
    )


def finalize_review(
    root: Path,
    transaction_path: Path,
    review_path: Path,
    *,
    check_git: bool = True,
) -> int:
    """Append an independently authored accepted review to a five-entry transaction."""

    transaction = _under_root(root, transaction_path, "transaction")
    review_file = _under_root(root, review_path, "review")
    validate_capture(root, transaction_path=transaction, check_git=check_git)
    entries = [dict(entry) for entry in _load_jsonl(transaction)]
    _validate_chain(entries, reviewed=False)
    reviewed_tip = str(entries[-1]["entry_sha256"])
    review = _load_json(review_file)
    _validate_review_document(review, reviewed_tip, root)
    relative_review = review_file.relative_to(root.resolve())
    _append(
        entries,
        "review_accepted",
        _review_payload(review, relative_review, _file_sha256(review_file)),
        recorder=str(review["reviewer_identity"]),
    )
    with transaction.open("a", encoding="utf-8", newline="\n") as output:
        output.write(json.dumps(entries[-1], separators=(",", ":"), sort_keys=True))
        output.write("\n")
    return len(CLAIM_RUN_IDS)


def validate_capture(
    root: Path = ROOT,
    *,
    transaction_path: Path = TRANSACTION_PATH,
    check_git: bool = True,
) -> int:
    """Validate all five execution entries before an independent review is appended."""

    transaction = _under_root(root, transaction_path, "transaction")
    entries = _load_jsonl(transaction)
    _validate_chain(entries, reviewed=False)
    payloads = [
        _mapping(entry["payload"], f"entry {index} payload")
        for index, entry in enumerate(entries, 1)
    ]
    _validate_opened(payloads[0], root, check_git=check_git)
    with _bound_expectation_bundle(root, payloads[0], check_git=check_git) as bundle:
        _require(
            validate_independent_review(bundle) == 16
            and validate_negative_fixtures(bundle) == 16,
            "RFV5_EVIDENCE_EXPECTATION_DRIFT",
            "WP43 independent expectation closure differs",
        )
        _validate_claim_map(payloads[1], bundle)
        _validate_fault_map(payloads[2], bundle)
    _validate_recorded_observation_sources(payloads[1], payloads[2])
    _validate_clean_contract(payloads[3])
    _validate_limitations(payloads[4])
    _require(
        all(
            entry["recorded_by"] == "wp48-production-evidence-executor"
            for entry in entries
        ),
        "RFV5_EVIDENCE_RECORDER_DRIFT",
        "capture recorder ownership differs",
    )
    _validate_recipe_specs(
        root,
        (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS),
        candidate=str(payloads[0]["candidate_commit"]) if check_git else None,
    )
    return len(CLAIM_RUN_IDS)


def validate_transaction(root: Path = ROOT, *, check_git: bool = True) -> int:
    """Validate the complete reviewed transaction and return its claim count."""

    entries = _load_jsonl(root / TRANSACTION_PATH)
    _validate_chain(entries)
    payloads = [
        _mapping(entry["payload"], f"entry {index} payload")
        for index, entry in enumerate(entries, 1)
    ]
    _validate_opened(payloads[0], root, check_git=check_git)
    with _bound_expectation_bundle(root, payloads[0], check_git=check_git) as bundle:
        _require(
            validate_independent_review(bundle) == 16
            and validate_negative_fixtures(bundle) == 16,
            "RFV5_EVIDENCE_EXPECTATION_DRIFT",
            "WP43 independent expectation closure differs",
        )
        _validate_claim_map(payloads[1], bundle)
        _validate_fault_map(payloads[2], bundle)
    _validate_recorded_observation_sources(payloads[1], payloads[2])
    _validate_clean_contract(payloads[3])
    _validate_limitations(payloads[4])
    _validate_review(payloads[5], str(entries[4]["entry_sha256"]), root)
    _require(
        all(
            entry["recorded_by"] == "wp48-production-evidence-executor"
            for entry in entries[:5]
        )
        and entries[5]["recorded_by"] == payloads[5]["reviewer_identity"],
        "RFV5_EVIDENCE_REVIEW_NOT_INDEPENDENT",
        "transaction recorder ownership differs",
    )
    _validate_recipe_specs(
        root,
        (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS),
        candidate=str(payloads[0]["candidate_commit"]) if check_git else None,
    )
    return len(CLAIM_RUN_IDS)


def _run(command: str, root: Path, *, check_git: bool = True) -> Mapping[str, object]:
    _require(
        command in COMMANDS,
        "RFV5_EVIDENCE_SELECTOR_UNKNOWN",
        f"unknown selector {command}",
    )
    selected = validate_transaction(root, check_git=check_git)
    opened = _mapping(
        _load_jsonl(root / TRANSACTION_PATH)[0]["payload"],
        "transaction-open payload",
    )
    integrity_count = len(_rows(opened["input_bindings"], "input_bindings"))
    oracle, criterion = COMMANDS[command]
    category_count = {
        "integrity": integrity_count,
        "behavior": selected,
        "causal-faults": len(FAULT_RUN_IDS),
        "clean-reconstruction": sum(spec.selected_count for spec in CLEAN_RUN_SPECS),
    }[command]
    return {
        "status": "passed",
        "oracle": oracle,
        "criterion": criterion,
        "selected_count": category_count,
        "transaction": str(TRANSACTION_PATH),
        "suite": SUITE,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in sorted(COMMANDS):
        selected = subparsers.add_parser(command)
        selected.add_argument("--root", type=Path, default=ROOT)
    capture = subparsers.add_parser("capture")
    capture.add_argument("--candidate-commit", required=True)
    capture.add_argument("--output", type=Path, default=TRANSACTION_PATH)
    capture.add_argument("--release-path", type=Path, default=ACTIVE_RELEASE_PATH)
    capture.add_argument("--root", type=Path, default=ROOT)
    review = subparsers.add_parser("finalize-review")
    review.add_argument("--transaction", type=Path, default=TRANSACTION_PATH)
    review.add_argument("--review", type=Path, default=REVIEW_PATH)
    review.add_argument("--root", type=Path, default=ROOT)
    candidate = subparsers.add_parser("review-candidate")
    candidate.add_argument("--transaction", type=Path, default=TRANSACTION_PATH)
    candidate.add_argument("--root", type=Path, default=ROOT)
    local_probe = subparsers.add_parser("local-source-probe")
    local_probe.add_argument("--claim-id", required=True)
    local_probe.add_argument("--mode", choices=("normal", "fault"), required=True)
    local_emit = subparsers.add_parser("emit-local-observations")
    local_emit.add_argument("--root", type=Path, default=ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "local-source-probe":
            actual = _installed_presentation_observation(args.claim_id, args.mode)
            print(
                json.dumps(
                    {
                        "schema": LOCAL_PROBE_SCHEMA,
                        "claim_id": args.claim_id,
                        "mode": args.mode,
                        "actual_observation": actual,
                    },
                    separators=(",", ":"),
                    sort_keys=True,
                )
            )
            return 0
        if args.command == "emit-local-observations":
            output_value = os.environ.get(
                "CODEFABRIC_WP48_LOCAL_OBSERVATION_OUTPUT_PATH"
            )
            commit = os.environ.get("CODEFABRIC_WP48_CANDIDATE_COMMIT", "")
            tree = os.environ.get("CODEFABRIC_WP48_CANDIDATE_TREE", "")
            release_value = os.environ.get(
                "CODEFABRIC_WP48_EXPECTATION_RELEASE_PATH", str(RELEASE_PATH)
            )
            _require(
                output_value is not None
                and HEX40.fullmatch(commit) is not None
                and HEX40.fullmatch(tree) is not None,
                "RFV5_EVIDENCE_OBSERVATION_OUTPUT_INVALID",
                "local observation environment is incomplete",
            )
            output = _under_root(
                args.root, Path(output_value), "local observation output"
            )
            selected = _emit_local_observations(
                args.root.resolve(), output, Path(release_value), commit, tree
            )
            print(
                json.dumps(
                    {
                        "status": "local-observations-emitted",
                        "selected_count": selected,
                    },
                    sort_keys=True,
                )
            )
            return 0
        if args.command == "capture":
            selected = capture_transaction(
                args.root,
                args.candidate_commit,
                args.output,
                release_path=args.release_path,
            )
            report: Mapping[str, object] = {
                "status": "captured",
                "selected_count": selected,
                "transaction": str(args.output),
                "review_required": True,
            }
        elif args.command == "finalize-review":
            selected = finalize_review(args.root, args.transaction, args.review)
            report = {
                "status": "reviewed",
                "selected_count": selected,
                "transaction": str(args.transaction),
                "review": str(args.review),
            }
        elif args.command == "review-candidate":
            selected = validate_capture(args.root, transaction_path=args.transaction)
            entries = _load_jsonl(
                _under_root(args.root, args.transaction, "transaction")
            )
            report = {
                "status": "review-candidate-valid",
                "selected_count": selected,
                "transaction": str(args.transaction),
                "reviewed_through_entry_sha256": entries[-1]["entry_sha256"],
                "review_artifact": str(REVIEW_PATH),
                "review_report": str(REVIEW_REPORT_PATH),
                "required_scope": list(REVIEW_SCOPE),
            }
        else:
            report = _run(args.command, args.root)
    except (ProductionEvidenceError, OSError, subprocess.SubprocessError) as error:
        code = getattr(error, "code", "RFV5_EVIDENCE_UNREADABLE")
        print(
            json.dumps(
                {"status": "failed", "code": code, "message": str(error)},
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
