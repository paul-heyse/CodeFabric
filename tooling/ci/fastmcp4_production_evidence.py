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
import hashlib
import importlib.metadata
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from tooling.ci.fastmcp4_successor_expectations import (
    ALLOWED_DESIGN_PATHS,
    RELEASE_PATH,
    Bundle,
    load_bundle,
    validate_independent_review,
    validate_issuance,
    validate_negative_fixtures,
)

ROOT = Path(__file__).resolve().parents[2]
TRANSACTION_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp48-production-evidence-v1.jsonl"
)
REVIEW_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp48-independent-review-v1.json"
)
REVIEW_REPORT_PATH = Path(
    "docs/reviews/implementation_review_codefabric_relational_data_fabric_v5_wp48_"
    "2026-09-02_v1.md"
)
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md"
)
STATE_PATH = Path(
    "docs/plans/state/codefabric-execution-proved-relational-data-fabric_v5_state.json"
)
JUSTFILE_PATH = Path("justfile")
RUNNER_PATH = Path("tooling/ci/fastmcp4_production_evidence.py")
RUNNER_TEST_PATH = Path("tooling/ci/test_fastmcp4_production_evidence.py")
EXPECTATION_VALIDATOR_PATH = Path("tooling/ci/fastmcp4_successor_expectations.py")
EXPECTATION_VALIDATOR_TEST_PATH = Path(
    "tooling/ci/test_fastmcp4_successor_expectations.py"
)
POST_PURGE_RUNNER_PATH = Path("tooling/ci/fastmcp4_post_purge_assurance.py")
POST_PURGE_RUNNER_TEST_PATH = Path("tooling/ci/test_fastmcp4_post_purge_assurance.py")
TRANSACTION_ID = "relational-fabric-v5-wp48-production-evidence-r1"
SUITE = "codefabric-relational-data-fabric@2.3.0"
ENTRY_SCHEMA = "codefabric.fastmcp4-production-evidence.entry.v1"
REVIEW_SCHEMA = "codefabric.fastmcp4-production-evidence.review.v1"

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
EXPECTED_INPUT_PATHS = {
    RELEASE_PATH / "issuance.yaml",
    RELEASE_PATH / "expectations.yaml",
    RELEASE_PATH / "causal-fixtures.yaml",
    RELEASE_PATH / "negative-fixtures.yaml",
    RELEASE_PATH / "independent-review.yaml",
    RELEASE_PATH / "performance-method.yaml",
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
    *(Path(path) for path in ALLOWED_DESIGN_PATHS),
}


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
    "RFV5-FM4-001": ("authority-pins",),
    "RFV5-FM4-002": ("modern-contract", "security-boundaries"),
    "RFV5-FM4-003": ("modern-contract",),
    "RFV5-FM4-004": ("modern-contract", "authority-pins"),
    "RFV5-FM4-005": ("guard-query-resources", "guard-roundtrip"),
    "RFV5-FM4-006": ("atomic-start",),
    "RFV5-FM4-007": ("guard-query-resources", "completion-authority"),
    "RFV5-FM4-008": (
        "guard-query-resources",
        "resource-authority",
        "security-boundaries",
    ),
    "RFV5-FM4-009": ("cancel-reconnect-isolation", "daemon-security-recovery"),
    "RFV5-FM4-010": ("cancel-reconnect-isolation", "security-boundaries"),
    "RFV5-FM4-011": ("security-boundaries", "daemon-security-recovery"),
    "RFV5-FM4-012": ("security-boundaries",),
    "RFV5-FM4-013": ("adapter-zero-state", "modern-contract"),
    "RFV5-FM4-014": ("adapter-zero-state", "authority-pins"),
    "RFV5-FM4-015": ("release-drift",),
    "RFV5-FM4-016": ("release-drift",),
}
NEGATIVE_FIXTURE_RUN_IDS = {
    "RFV5-FM4-001-N": ("authority-pins", "adapter-zero-state"),
    "RFV5-FM4-002-N": ("fault-mcp-projection",),
    "RFV5-FM4-003-N": ("modern-contract", "adapter-zero-state"),
    "RFV5-FM4-004-N": ("modern-contract",),
    "RFV5-FM4-005-N": ("fault-guard-token",),
    "RFV5-FM4-006-N": ("fault-start-variant",),
    "RFV5-FM4-007-N": ("fault-completion-filtering",),
    "RFV5-FM4-008-N": ("fault-resource-authorization",),
    "RFV5-FM4-009-N": ("fault-cancellation",),
    "RFV5-FM4-010-N": ("fault-cancellation", "fault-mcp-projection"),
    "RFV5-FM4-011-N": ("daemon-security-recovery", "fault-mcp-projection"),
    "RFV5-FM4-012-N": ("fault-mcp-projection",),
    "RFV5-FM4-013-N": ("adapter-zero-state",),
    "RFV5-FM4-014-N": ("adapter-zero-state", "release-drift"),
    "RFV5-FM4-015-N": ("release-drift",),
    "RFV5-FM4-016-N": ("release-drift",),
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
WP48_REVIEW_PREFIX = (
    "docs/reviews/implementation_review_codefabric_relational_data_fabric_v5_wp48_"
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


def _validate_recipe_specs(root: Path, specs: Sequence[RunSpec]) -> None:
    justfile = (root / JUSTFILE_PATH).read_text(encoding="utf-8")
    catalog = _just_recipe_catalog(root)
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
    exact_allowed = {
        "Untitled",
        str(JUSTFILE_PATH),
        str(STATE_PATH),
        ".github/workflows/ci.yml",
        str(RUNNER_PATH),
        str(RUNNER_TEST_PATH),
        str(POST_PURGE_RUNNER_PATH),
        str(POST_PURGE_RUNNER_TEST_PATH),
    }
    return path in exact_allowed or path.startswith(
        (
            "contracts/evidence/relational-fabric-v5/wp48-",
            WP48_REVIEW_PREFIX,
        )
    )


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


def _input_bindings(root: Path) -> list[dict[str, str]]:
    return [
        {"path": str(path), "sha256": _file_sha256(root / path)}
        for path in sorted(EXPECTED_INPUT_PATHS, key=str)
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


def _runner_bindings(root: Path) -> dict[str, object]:
    catalog = _just_recipe_catalog(root)
    recipe_roots = tuple(
        spec.recipe
        for spec in (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
        if spec.recipe is not None
    )
    recipes = _recipe_closure(catalog, recipe_roots)
    return {
        "runner_sha256": _file_sha256(root / RUNNER_PATH),
        "runner_test_sha256": _file_sha256(root / RUNNER_TEST_PATH),
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
                    tuple(spec for spec in FAULT_RUN_SPECS if spec.recipe is None)
                    + CLEAN_RUN_SPECS
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
    root: Path, candidate: str, candidate_tree: str
) -> dict[str, object]:
    return {
        "suite": SUITE,
        "packet": "WP48",
        "candidate_commit": candidate,
        "candidate_tree": candidate_tree,
        "oracles": list(ORACLES),
        "criteria": list(CRITERIA),
        "input_bindings": _input_bindings(root),
        "runner_bindings": _runner_bindings(root),
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


def _claim_payload(
    bundle: Bundle, runs: Sequence[Mapping[str, object]]
) -> dict[str, object]:
    expectations = _expectation_index(bundle)
    claims = []
    for claim_id, run_ids in CLAIM_RUN_IDS.items():
        expectation = expectations[claim_id]
        expected = _mapping(expectation["expected_observation"], "expected_observation")
        claims.append(
            {
                "claim_id": claim_id,
                "family": expectation["family"],
                "expected_observation_sha256": canonical_sha256(expected),
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
) -> dict[str, object]:
    fixtures = {str(row["fixture_id"]): row for row in bundle.negative}
    return {
        "runs": list(fault_runs),
        "faults": [
            {
                "layer": layer,
                "run_id": run_id,
                "distinguished": True,
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
                "expected_error": fixtures[fixture_id]["expected_error"],
                "expected_mismatch_paths_sha256": canonical_sha256(
                    {"paths": fixtures[fixture_id]["expected_mismatch_paths"]}
                ),
                "run_ids": list(run_ids),
                "distinguished": True,
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


def _record_failed_attempt(
    root: Path,
    candidate_commit: str,
    candidate_tree: str,
    error: ProductionEvidenceError,
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
        _opened_payload(root, candidate_commit, candidate_tree),
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


def capture_transaction(
    root: Path,
    candidate_commit: str,
    output_path: Path,
    *,
    executor: Executor = _default_executor,
    check_git: bool = True,
) -> int:
    """Execute every WP48 evidence leg and create an unreviewed transaction."""

    output = _under_root(root, output_path, "transaction output")
    _require(
        not output.exists(),
        "RFV5_EVIDENCE_APPEND_ONLY",
        f"transaction already exists: {output}",
    )
    validate_issuance(root=root, require_review=True)
    bundle = load_bundle(root)
    _require(
        validate_independent_review(bundle) == 16
        and validate_negative_fixtures(bundle) == 16,
        "RFV5_EVIDENCE_EXPECTATION_DRIFT",
        "WP43 expectation release is not closed",
    )
    _validate_recipe_specs(
        root, (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
    )
    candidate_tree = (
        _validate_capture_candidate(root, candidate_commit) if check_git else "2" * 40
    )
    environment = dict(os.environ)
    environment["CARGO_INCREMENTAL"] = "0"
    try:
        behavior_runs = _execute_all(BEHAVIOR_RUN_SPECS, root, environment, executor)
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
        _record_failed_attempt(root, candidate_commit, candidate_tree, error)
        raise

    entries: list[dict[str, Any]] = []
    recorder = "wp48-production-evidence-executor"
    _append(
        entries,
        "transaction_opened",
        _opened_payload(root, candidate_commit, candidate_tree),
        recorder=recorder,
    )
    _append(
        entries,
        "claim_observation_map",
        _claim_payload(bundle, behavior_runs),
        recorder=recorder,
    )
    _append(
        entries,
        "causal_fault_map",
        _fault_payload(bundle, fault_runs, behavior_runs),
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
) -> None:
    _require(
        set(payload)
        == {
            "suite",
            "packet",
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
    _require(
        HEX40.fullmatch(commit) is not None and HEX40.fullmatch(tree) is not None,
        "RFV5_EVIDENCE_CANDIDATE_INVALID",
        "candidate commit or tree identity differs",
    )
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
            and _file_sha256(root / path) == digest,
            "RFV5_EVIDENCE_INPUT_DRIFT",
            f"bound input drifted: {path}",
        )
    _require(
        observed_paths == EXPECTED_INPUT_PATHS,
        "RFV5_EVIDENCE_INPUT_CLOSURE",
        "bound input path closure differs",
    )
    _require(
        payload["runner_bindings"] == _runner_bindings(root),
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
        _require(
            set(row)
            == {
                "claim_id",
                "family",
                "expected_observation_sha256",
                "run_ids",
                "selected_count",
            }
            and row["claim_id"] == claim_id
            and row["family"] == expectation["family"]
            and row["expected_observation_sha256"] == canonical_sha256(expected)
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
            set(row) == {"layer", "run_id", "distinguished", "selected_count"}
            and row["layer"] == layer
            and row["run_id"] == run_id
            and row["distinguished"] is True
            and row["selected_count"] == 1
            and run_id in run_ids,
            "RFV5_EVIDENCE_FAULT_SURVIVED",
            f"fault {layer} was not distinguished",
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
        _require(
            set(row)
            == {
                "fixture_id",
                "claim_id",
                "expected_error",
                "expected_mismatch_paths_sha256",
                "run_ids",
                "distinguished",
            }
            and row["fixture_id"] == fixture_id
            and row["claim_id"] == fixture["claim_id"]
            and row["expected_error"] == fixture["expected_error"]
            and row["expected_mismatch_paths_sha256"]
            == canonical_sha256({"paths": fixture["expected_mismatch_paths"]})
            and tuple(row["run_ids"]) == expected_runs
            and row["distinguished"] is True
            and set(expected_runs).issubset(production_run_ids),
            "RFV5_EVIDENCE_FIXTURE_DRIFT",
            f"negative fixture production mapping differs for {fixture_id}",
        )


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

    validate_issuance(root=root, require_review=True)
    bundle = load_bundle(root)
    _require(
        validate_independent_review(bundle) == 16
        and validate_negative_fixtures(bundle) == 16,
        "RFV5_EVIDENCE_EXPECTATION_DRIFT",
        "WP43 independent expectation closure differs",
    )
    transaction = _under_root(root, transaction_path, "transaction")
    entries = _load_jsonl(transaction)
    _validate_chain(entries, reviewed=False)
    payloads = [
        _mapping(entry["payload"], f"entry {index} payload")
        for index, entry in enumerate(entries, 1)
    ]
    _validate_opened(payloads[0], root, check_git=check_git)
    _validate_claim_map(payloads[1], bundle)
    _validate_fault_map(payloads[2], bundle)
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
        root, (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
    )
    return len(CLAIM_RUN_IDS)


def validate_transaction(root: Path = ROOT, *, check_git: bool = True) -> int:
    """Validate the complete reviewed transaction and return its claim count."""

    validate_issuance(root=root, require_review=True)
    bundle = load_bundle(root)
    _require(
        validate_independent_review(bundle) == 16
        and validate_negative_fixtures(bundle) == 16,
        "RFV5_EVIDENCE_EXPECTATION_DRIFT",
        "WP43 independent expectation closure differs",
    )
    entries = _load_jsonl(root / TRANSACTION_PATH)
    _validate_chain(entries)
    payloads = [
        _mapping(entry["payload"], f"entry {index} payload")
        for index, entry in enumerate(entries, 1)
    ]
    _validate_opened(payloads[0], root, check_git=check_git)
    _validate_claim_map(payloads[1], bundle)
    _validate_fault_map(payloads[2], bundle)
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
        root, (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
    )
    return len(CLAIM_RUN_IDS)


def _run(command: str, root: Path, *, check_git: bool = True) -> Mapping[str, object]:
    _require(
        command in COMMANDS,
        "RFV5_EVIDENCE_SELECTOR_UNKNOWN",
        f"unknown selector {command}",
    )
    selected = validate_transaction(root, check_git=check_git)
    oracle, criterion = COMMANDS[command]
    category_count = {
        "integrity": len(EXPECTED_INPUT_PATHS),
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
    capture.add_argument("--root", type=Path, default=ROOT)
    review = subparsers.add_parser("finalize-review")
    review.add_argument("--transaction", type=Path, default=TRANSACTION_PATH)
    review.add_argument("--review", type=Path, default=REVIEW_PATH)
    review.add_argument("--root", type=Path, default=ROOT)
    candidate = subparsers.add_parser("review-candidate")
    candidate.add_argument("--transaction", type=Path, default=TRANSACTION_PATH)
    candidate.add_argument("--root", type=Path, default=ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "capture":
            selected = capture_transaction(
                args.root, args.candidate_commit, args.output
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
