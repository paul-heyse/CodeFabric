"""Validate and select the append-only WP38 production-evidence transaction.

The WP33 issuance is the independent source of decoded expectations and semantic
fault fixtures.  This module binds that issuance to live successor-only recipes;
it does not calculate expected rows, import production behavior, or execute a
predecessor.  The Just recipes execute the production observations before asking
this module to validate the corresponding transaction projection.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

import blake3
import rfc8785

from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    SuccessorEvidenceError,
    validate_transaction_integrity,
)

ROOT = Path(__file__).resolve().parents[2]
TRANSACTION_PATH = Path(
    "contracts/acceptance/relational-fabric-v3/production-evidence-transaction.jsonl"
)
JUSTFILE_PATH = Path("justfile")
CLAIM_001 = "RFV3-CLAIM-001"
TRANSACTION_ID = "wp38:first-principles-production-evidence-v1"
SUITE = "codefabric-relational-data-fabric@2.1.0"

ORACLES = (
    "production-evidence-input-integrity-check",
    "first-principles-production-behavior-check",
    "causal-fault-discrimination-check",
    "production-evidence-recovery-operations-check",
)
INPUT_ORACLE, BEHAVIOR_ORACLE, CAUSAL_ORACLE, OPERATIONS_ORACLE = ORACLES

EXPECTED_ENTRY_KINDS = (
    "transaction_opened",
    "claim_oracle_mapping",
    "production_execution_contract",
    "limitations_recorded",
)
EXPECTED_OPERATION_CLAIMS = frozenset(
    {
        "RFV3-CLAIM-012",
        "RFV3-CLAIM-013",
        "RFV3-CLAIM-015",
        "RFV3-CLAIM-016",
        "RFV3-CLAIM-018",
    }
)
EXPECTED_ACCEPTANCE_INPUTS = (
    {"kind": "reviewed_expectations", "path": str(EXPECTATIONS_PATH)},
    {"kind": "semantic_fault_fixtures", "path": str(FIXTURES_PATH)},
    {"kind": "independent_issuance", "path": str(ISSUANCE_PATH)},
)
EXPECTED_RECIPE_DEPENDENCIES = {
    INPUT_ORACLE: (
        "successor-evidence-transaction-integrity-check",
        "remaining-legacy-zero-state-check",
    ),
    BEHAVIOR_ORACLE: (
        INPUT_ORACLE,
        "exact-provider-batch-check",
        "provider-ipc-contract-integrity-check",
        "datafusion-contract-matrix-integrity-check",
        "analysis-producer-semantic-check",
        "semantic-request-program-check",
        "delta-exact-reconstruction-v3-check",
        "lifecycle-production-vertical-check",
        "public-lifecycle-wire-contract-integrity-check",
        "wp38-artifact-bound-positive-execution-check",
    ),
    CAUSAL_ORACLE: (
        INPUT_ORACLE,
        "provider-admission-exclusivity-check",
        "authorized-child-schema-rejection-check",
        "analysis-causal-fault-check",
        "query-unknown-negative-proof-check",
        "activation-receipt-nonauthority-check",
        "fastmcp-presentation-boundary-check",
        "wp38-artifact-bound-causal-execution-check",
        "wp38-artifact-bound-negative-execution-check",
    ),
    OPERATIONS_ORACLE: (
        INPUT_ORACLE,
        "provider-trust-coverage-remainder-check",
        "datafusion-cache-resource-operations-check",
        "candidate-free-recovery-check",
        "graph-query-resource-operations-check",
        "resource-cancellation-recovery-check",
        "wp38-claim-018-production-check",
    ),
}
CLAIM_018_TESTS = (
    "wp38_claim_018_clean_incremental_equivalence_executes_successor_arrow_datafusion",
    "wp38_claim_018_causal_source_change_is_discriminated_by_successor_execution",
    "wp38_claim_018_missing_delete_fault_is_rejected_by_successor_execution",
)
ARTIFACT_BOUND_CLAIM_TESTS = {
    "RFV3-CLAIM-001": (
        "wp38_claim_001_positive_executes_frozen_pyrefly_provider_observation",
        "wp38_claim_001_causal_source_mutation_changes_production_pyrefly_target",
        "wp38_claim_001_negative_rejects_open_provider_coverage",
    ),
    "RFV3-CLAIM-002": (
        "wp38_claim_002_positive_executes_frozen_typed_datafusion_transformation",
        "wp38_claim_002_causal_fixture_changes_real_datafusion_rows",
        "wp38_claim_002_negative_fixture_rejects_undeclared_typed_column",
    ),
    "RFV3-CLAIM-003": (
        "wp38_claim_003_positive_executes_candidate_preserving_common_call_graph",
        "wp38_claim_003_causal_provider_target_changes_common_call_graph",
        "wp38_claim_003_negative_preserves_known_fact_and_typed_unknown",
    ),
    **{
        f"RFV3-CLAIM-{number:03d}": (
            f"wp38_claim_{number:03d}_positive_production_execution",
            f"wp38_claim_{number:03d}_causal_production_execution",
            f"wp38_claim_{number:03d}_negative_production_execution",
        )
        for number in (*range(4, 12), 14)
    },
    "RFV3-CLAIM-012": (
        "wp38_claim_012_positive_executes_frozen_exact_delta_and_cdf_semantics",
        "wp38_claim_012_causal_exact_version_changes_the_decoded_snapshot",
        "wp38_claim_012_negative_rejects_frozen_unsupported_writer_feature",
    ),
    "RFV3-CLAIM-013": (
        "wp38_claim_013_positive_recovers_the_artifact_bound_exact_epoch",
        "wp38_claim_013_causal_new_head_changes_the_recovered_exact_epoch",
        "wp38_claim_013_negative_transaction_mismatch_keeps_admission_closed",
    ),
    "RFV3-CLAIM-015": (
        "wp38_claim_015_positive_executes_typed_arrow_ipc_and_canonical_artifact_identity",
        "wp38_claim_015_causal_row_budget_rejects_before_resource_publication",
        "wp38_claim_015_negative_cancellation_releases_without_publication",
    ),
    "RFV3-CLAIM-016": (
        "wp38_claim_016_positive_executes_fail_closed_production_preflight",
        "wp38_claim_016_causal_authorization_executes_degraded_trusted_local_plan",
        "wp38_claim_016_negative_rejects_seccomp_requirement_weakening",
    ),
    "RFV3-CLAIM-017": (
        "test_wp38_claim_017_positive_executes_frozen_released_response_projection",
        "test_wp38_claim_017_causal_terminal_selects_frozen_cancelled_response",
        "test_wp38_claim_017_negative_rejects_frozen_candidate_public_projection",
    ),
    "RFV3-CLAIM-018": CLAIM_018_TESTS,
}
ARTIFACT_BOUND_HELPER_TESTS = {
    helper: tuple(tests[index] for tests in ARTIFACT_BOUND_CLAIM_TESTS.values())
    for helper, index in (
        ("wp38-artifact-bound-positive-execution-check", 0),
        ("wp38-artifact-bound-causal-execution-check", 1),
        ("wp38-artifact-bound-negative-execution-check", 2),
    )
}
EXPECTED_LIMITATIONS = {
    "HOST-UNTRUSTED-CONTAINMENT": (
        "unavailable",
        "untrusted_profile_unavailable",
    ),
    "PERFORMANCE-EVIDENCE-NOT-CLAIMED": (
        "not_claimed",
        "none",
    ),
    "SCHEDULED-DEEP-ASSURANCE-DEFERRED": (
        "deferred",
        "scheduled_assurance_not_executed",
    ),
    "SUPPORTED-PLATFORM-COVERAGE": (
        "local_workstation_only",
        "platform_coverage_limited",
    ),
}
FORBIDDEN_DEPENDENCY_FRAGMENTS = (
    "bootstrap_model",
    "comparator",
    "legacy-execution",
    "model-digest",
    "old-binary",
    "predecessor-execution",
    "replay-agreement",
)
ENTRY_DIGEST = re.compile(r"b3:[0-9a-f]{64}\Z")


class ProductionEvidenceError(ValueError):
    """A typed, fail-closed WP38 evidence prerequisite or invariant failure."""

    def __init__(
        self, code: str, message: str, *, details: Mapping[str, object] | None = None
    ) -> None:
        super().__init__(message)
        self.code = code
        self.details = dict(details or {})


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_DUPLICATE_JSON_MEMBER",
                f"duplicate JSON member {key!r}",
            )
        value[key] = item
    return value


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID", f"{context} must be an object"
        )
    return value


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    if set(value) != expected:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            f"{context} keys differ: expected={sorted(expected)} observed={sorted(value)}",
        )


def _nonempty_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            f"{context} must be a non-empty string",
        )
    return value


def _load_json(path: Path, context: str) -> Mapping[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, json.JSONDecodeError) as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE", f"cannot load {context}: {error}"
        ) from error
    return _mapping(value, context)


def _load_jsonl(path: Path, context: str) -> list[Mapping[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE", f"cannot load {context}: {error}"
        ) from error
    if not lines or any(not line.strip() for line in lines):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            f"{context} must be non-empty JSONL without blank rows",
        )
    result: list[Mapping[str, Any]] = []
    for line_number, line in enumerate(lines, 1):
        try:
            value = json.loads(line, object_pairs_hook=_reject_duplicates)
        except json.JSONDecodeError as error:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
                f"cannot decode {context} row {line_number}: {error}",
            ) from error
        result.append(_mapping(value, f"{context} row {line_number}"))
    return result


def _canonical_b3(value: Mapping[str, Any]) -> str:
    return f"b3:{blake3.blake3(rfc8785.dumps(dict(value))).hexdigest()}"


def _bytes_b3(path: Path) -> str:
    return f"b3:{blake3.blake3(path.read_bytes()).hexdigest()}"


def validate_claim_001_live_binding(root: Path = ROOT) -> None:
    """Reject live Claim 001 input drift before any production evidence executes."""

    expectations = _load_jsonl(root / EXPECTATIONS_PATH, "WP33 expectations")
    matches = [row for row in expectations if row.get("claim_id") == CLAIM_001]
    if len(matches) != 1:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_CLAIM_001_MISSING",
            f"expected exactly one {CLAIM_001} row, observed {len(matches)}",
        )
    claim = matches[0]
    universe = _mapping(
        claim.get("complete_input_universe"), "Claim 001 input universe"
    )
    inputs = _mapping(universe.get("inputs"), "Claim 001 inputs")
    protocol = _mapping(
        inputs.get("protocol_schema_identity"), "Claim 001 protocol schema identity"
    )
    schema_path = Path(
        _nonempty_string(protocol.get("control_schema"), "control schema path")
    )
    if schema_path.is_absolute() or ".." in schema_path.parts:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            "Claim 001 control schema path must be repository-relative",
        )
    expected = _nonempty_string(
        protocol.get("control_schema_b3"), "Claim 001 control schema BLAKE3"
    )
    try:
        observed = _bytes_b3(root / schema_path)
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot hash Claim 001 control schema: {error}",
        ) from error
    if expected != observed:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_REISSUANCE_REQUIRED",
            "Claim 001 reviewed input identity differs from the live provider control schema",
            details={
                "claim_id": CLAIM_001,
                "path": str(schema_path),
                "reviewed_b3": expected,
                "live_b3": observed,
                "required_action": (
                    "independent review and reissuance of Claim 001 and every derived "
                    "expectation, review, artifact, content, and issuance identity"
                ),
            },
        )


def _load_issuance_binding(root: Path) -> Mapping[str, Any]:
    issuance = _load_json(root / ISSUANCE_PATH, "WP33 evidence issuance")
    for key in ("issuance_id", "reviewed_content_id", "author", "reviewer"):
        if key not in issuance:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_SCHEMA_INVALID", f"issuance lacks {key}"
            )
    return issuance


def _expectation_index(root: Path) -> dict[str, Mapping[str, Any]]:
    rows = _load_jsonl(root / EXPECTATIONS_PATH, "WP33 expectations")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        claim_id = _nonempty_string(row.get("claim_id"), "claim id")
        if claim_id in result:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_DUPLICATE_CLAIM", f"duplicate claim {claim_id}"
            )
        result[claim_id] = row
    expected = {f"RFV3-CLAIM-{number:03d}" for number in range(1, 19)}
    if set(result) != expected:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_CLAIM_CLOSURE_INVALID",
            f"claim closure differs: missing={sorted(expected - set(result))} "
            f"extra={sorted(set(result) - expected)}",
        )
    return result


def _validate_chain(entries: Sequence[Mapping[str, Any]]) -> None:
    previous: str | None = None
    for sequence, entry in enumerate(entries, 1):
        context = f"production evidence entry {sequence}"
        _strict_keys(
            entry,
            {
                "schema_version",
                "sequence",
                "transaction_id",
                "entry_kind",
                "recorded_by",
                "previous_entry_b3",
                "payload",
                "entry_b3",
            },
            context,
        )
        if entry["schema_version"] != 1 or entry["sequence"] != sequence:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
                f"{context} schema version or sequence differs",
            )
        if entry["transaction_id"] != TRANSACTION_ID:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
                f"{context} transaction identity differs",
            )
        _nonempty_string(entry["entry_kind"], f"{context} kind")
        _nonempty_string(entry["recorded_by"], f"{context} recorder")
        if entry["previous_entry_b3"] != previous:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
                f"{context} previous digest differs",
            )
        digest = _nonempty_string(entry["entry_b3"], f"{context} digest")
        if ENTRY_DIGEST.fullmatch(digest) is None:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
                f"{context} digest framing is invalid",
            )
        unsigned = {key: value for key, value in entry.items() if key != "entry_b3"}
        expected_digest = _canonical_b3(unsigned)
        if digest != expected_digest:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
                f"{context} canonical digest differs",
            )
        previous = digest


def _validate_opened(payload: Mapping[str, Any], issuance: Mapping[str, Any]) -> None:
    _strict_keys(
        payload,
        {
            "suite",
            "packet",
            "oracles",
            "acceptance_inputs",
            "diagnostic_inputs",
            "issuance_binding",
        },
        "transaction-open payload",
    )
    if payload["suite"] != SUITE or payload["packet"] != "WP38":
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_AUTHORITY_BINDING_INVALID",
            "transaction suite or packet differs",
        )
    if tuple(payload["oracles"]) != ORACLES:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID", "WP38 oracle names differ"
        )
    if tuple(payload["acceptance_inputs"]) != EXPECTED_ACCEPTANCE_INPUTS:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_AUTHORITY_BINDING_INVALID",
            "acceptance inputs differ from the frozen successor-only set",
        )
    if payload["diagnostic_inputs"] != []:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_FORBIDDEN_ACCEPTANCE_EDGE",
            "diagnostic or historical inputs entered the acceptance transaction",
        )
    binding = _mapping(payload["issuance_binding"], "issuance binding")
    _strict_keys(
        binding,
        {
            "issuance_id",
            "reviewed_content_id",
            "expectation_author",
            "independent_reviewer",
        },
        "issuance binding",
    )
    author = _mapping(issuance["author"], "issuance author")
    reviewer = _mapping(issuance["reviewer"], "issuance reviewer")
    expected_binding = {
        "issuance_id": issuance["issuance_id"],
        "reviewed_content_id": issuance["reviewed_content_id"],
        "expectation_author": author.get("identity"),
        "independent_reviewer": reviewer.get("identity"),
    }
    if dict(binding) != expected_binding:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_AUTHORITY_BINDING_INVALID",
            "transaction does not bind the current independent issuance",
        )
    if (
        author.get("implementation_owner") is not False
        or reviewer.get("implementation_owner") is not False
        or reviewer.get("expectation_author") is not False
        or author.get("identity") == reviewer.get("identity")
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_REVIEW_INDEPENDENCE_INVALID",
            "bound issuance does not establish author/reviewer independence",
        )


def _validate_claim_mapping(
    payload: Mapping[str, Any], expectations: Mapping[str, Mapping[str, Any]]
) -> None:
    _strict_keys(payload, {"claims"}, "claim-oracle mapping payload")
    rows = payload["claims"]
    if not isinstance(rows, list):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID", "claim mapping must be a list"
        )
    observed: set[str] = set()
    for row_value in rows:
        row = _mapping(row_value, "claim-oracle mapping row")
        _strict_keys(
            row,
            {
                "claim_id",
                "claim_family",
                "issued_observation_recipe",
                "input_oracle",
                "positive_oracle",
                "causal_oracle",
                "operations_oracle",
            },
            "claim-oracle mapping row",
        )
        claim_id = _nonempty_string(row["claim_id"], "mapped claim id")
        if claim_id in observed or claim_id not in expectations:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_CLAIM_CLOSURE_INVALID",
                f"duplicate or unknown mapped claim {claim_id}",
            )
        observed.add(claim_id)
        expectation = expectations[claim_id]
        consumer = _mapping(expectation.get("future_consumer"), f"{claim_id} consumer")
        expected_operations = (
            OPERATIONS_ORACLE if claim_id in EXPECTED_OPERATION_CLAIMS else None
        )
        expected_row = {
            "claim_id": claim_id,
            "claim_family": expectation.get("claim_family"),
            "issued_observation_recipe": consumer.get("oracle"),
            "input_oracle": INPUT_ORACLE,
            "positive_oracle": BEHAVIOR_ORACLE,
            "causal_oracle": CAUSAL_ORACLE,
            "operations_oracle": expected_operations,
        }
        if dict(row) != expected_row:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_ORACLE_MAPPING_INVALID",
                f"{claim_id} oracle mapping differs from its reviewed issuance",
            )
    if observed != set(expectations):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_CLAIM_CLOSURE_INVALID",
            f"transaction claim mapping selected {len(observed)} of {len(expectations)} claims",
        )


def _recipe_dependencies(justfile: str, recipe: str) -> tuple[str, ...]:
    match = re.search(rf"(?m)^{re.escape(recipe)}:(?P<dependencies>[^\n]*)$", justfile)
    if match is None:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID",
            f"Just recipe {recipe} is absent",
        )
    return tuple(match.group("dependencies").split())


def _recipe_body(justfile: str, recipe: str) -> str:
    match = re.search(
        rf"(?ms)^{re.escape(recipe)}:[^\n]*\n(?P<body>(?:[ \t].*\n|\n)*)",
        justfile,
    )
    if match is None:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID",
            f"Just recipe body {recipe} is absent",
        )
    return match.group("body")


def _validate_execution_contract(payload: Mapping[str, Any], justfile: str) -> None:
    _strict_keys(
        payload,
        {
            "recipe_dependencies",
            "claim_018_successor_tests",
            "production_observation_mode",
            "historical_executables_required",
        },
        "production execution contract payload",
    )
    dependencies = _mapping(payload["recipe_dependencies"], "recipe dependencies")
    if set(dependencies) != set(ORACLES):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID",
            "recipe dependency oracle set differs",
        )
    for oracle, expected in EXPECTED_RECIPE_DEPENDENCIES.items():
        observed_value = dependencies[oracle]
        if not isinstance(observed_value, list) or tuple(observed_value) != expected:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID",
                f"{oracle} production dependency closure differs",
            )
        observed_live = _recipe_dependencies(justfile, oracle)
        if observed_live != expected:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID",
                f"{oracle} live Just dependency closure differs",
            )
        for dependency in observed_live:
            lowered = dependency.lower()
            if any(fragment in lowered for fragment in FORBIDDEN_DEPENDENCY_FRAGMENTS):
                raise ProductionEvidenceError(
                    "PRODUCTION_EVIDENCE_FORBIDDEN_ACCEPTANCE_EDGE",
                    f"{oracle} contains forbidden acceptance dependency {dependency}",
                )
    if tuple(payload["claim_018_successor_tests"]) != CLAIM_018_TESTS:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_CLAIM_018_EXECUTION_INVALID",
            "Claim 018 successor test closure differs",
        )
    helper_body = _recipe_body(justfile, "wp38-claim-018-production-check")
    if not all(test in helper_body for test in CLAIM_018_TESTS):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_CLAIM_018_EXECUTION_INVALID",
            "Claim 018 Just helper does not select every real successor test",
        )
    for helper, selectors in ARTIFACT_BOUND_HELPER_TESTS.items():
        helper_body = _recipe_body(justfile, helper)
        if not all(selector in helper_body for selector in selectors):
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_ARTIFACT_EXECUTION_INVALID",
                f"{helper} does not select every recorded artifact-bound test",
            )
    if (
        payload["production_observation_mode"] != "live_successor_recipe_execution"
        or payload["historical_executables_required"] is not False
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_FORBIDDEN_ACCEPTANCE_EDGE",
            "production observations are not successor-only live recipe executions",
        )


def _validate_limitations(payload: Mapping[str, Any]) -> None:
    _strict_keys(
        payload,
        {"development_readiness", "release_certification", "limitations"},
        "limitations payload",
    )
    if (
        payload["development_readiness"] != "eligible_after_live_oracles"
        or payload["release_certification"] != "not_decided_by_wp38"
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_LIMITATION_INVALID",
            "WP38 evidence attempted to decide release certification",
        )
    limitations = payload["limitations"]
    if not isinstance(limitations, list):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID", "limitations must be a list"
        )
    observed: dict[str, tuple[str, str]] = {}
    for value in limitations:
        limitation = _mapping(value, "limitation")
        _strict_keys(
            limitation,
            {"id", "state", "release_effect", "evidence"},
            "limitation",
        )
        limitation_id = _nonempty_string(limitation["id"], "limitation id")
        if limitation_id in observed:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_LIMITATION_INVALID",
                f"duplicate limitation {limitation_id}",
            )
        evidence = limitation["evidence"]
        if (
            not isinstance(evidence, list)
            or not evidence
            or not all(isinstance(item, str) and item for item in evidence)
        ):
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_LIMITATION_INVALID",
                f"{limitation_id} lacks explicit evidence",
            )
        observed[limitation_id] = (
            _nonempty_string(limitation["state"], f"{limitation_id} state"),
            _nonempty_string(
                limitation["release_effect"], f"{limitation_id} release effect"
            ),
        )
    if observed != EXPECTED_LIMITATIONS:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_LIMITATION_INVALID",
            "required platform, performance, and deep-assurance limitations differ",
        )


def _validate_review(
    entries: Sequence[Mapping[str, Any]],
    issuance: Mapping[str, Any],
    *,
    require_review: bool,
) -> None:
    if len(entries) == len(EXPECTED_ENTRY_KINDS):
        if require_review:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_REQUIRED",
                "the append-only WP38 transaction awaits independent review",
                details={
                    "required_action": (
                        "append one independently authored review_accepted entry binding the "
                        "current chain tip after all frozen input identities are valid"
                    )
                },
            )
        return
    if len(entries) != len(EXPECTED_ENTRY_KINDS) + 1:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
            "transaction contains unexpected rows after the required execution entries",
        )
    review = entries[-1]
    if review["entry_kind"] != "review_accepted":
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "final transaction entry is not an independent acceptance review",
        )
    payload = _mapping(review["payload"], "transaction review payload")
    _strict_keys(
        payload,
        {
            "reviewer_identity",
            "reviewed_through_entry_b3",
            "implementation_owner",
            "expectation_author",
            "verdict",
            "scope",
        },
        "transaction review payload",
    )
    reviewer_identity = _nonempty_string(
        payload["reviewer_identity"], "transaction reviewer"
    )
    issuance_author = _mapping(issuance["author"], "issuance author").get("identity")
    execution_recorders = {str(entry["recorded_by"]) for entry in entries[:-1]}
    if (
        review["recorded_by"] != reviewer_identity
        or reviewer_identity == issuance_author
        or reviewer_identity in execution_recorders
        or payload["reviewed_through_entry_b3"] != entries[-2]["entry_b3"]
        or payload["implementation_owner"] is not False
        or payload["expectation_author"] is not False
        or payload["verdict"] != "accepted"
        or payload["scope"]
        != [
            "append_only_chain",
            "successor_only_acceptance_edges",
            "claim_oracle_mapping",
            "production_recipe_closure",
            "limitations",
        ]
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "independent transaction review is incomplete or does not bind the chain tip",
        )


def validate_append_only_transaction(
    root: Path = ROOT, *, require_review: bool = True
) -> int:
    """Validate immutable chain, authority, claim mapping, recipes, and limitations."""

    entries = _load_jsonl(root / TRANSACTION_PATH, "WP38 production transaction")
    _validate_chain(entries)
    if tuple(entry["entry_kind"] for entry in entries[:4]) != EXPECTED_ENTRY_KINDS:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
            "required transaction entry kinds or order differ",
        )
    expectations = _expectation_index(root)
    issuance = _load_issuance_binding(root)
    _validate_opened(
        _mapping(entries[0]["payload"], "transaction-open payload"), issuance
    )
    _validate_claim_mapping(
        _mapping(entries[1]["payload"], "claim mapping payload"), expectations
    )
    try:
        justfile = (root / JUSTFILE_PATH).read_text(encoding="utf-8")
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE", f"cannot load justfile: {error}"
        ) from error
    _validate_execution_contract(
        _mapping(entries[2]["payload"], "execution contract payload"), justfile
    )
    _validate_limitations(_mapping(entries[3]["payload"], "limitations payload"))
    _validate_review(entries, issuance, require_review=require_review)
    return len(expectations)


def _select_claims(
    expectations: Mapping[str, Mapping[str, Any]], selected_claims: Sequence[str]
) -> list[Mapping[str, Any]]:
    if not selected_claims:
        return list(expectations.values())
    selected = set(selected_claims)
    result = [row for claim_id, row in expectations.items() if claim_id in selected]
    unknown = selected - set(expectations)
    if unknown or not result:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_ZERO_SELECTION",
            f"claim selector selected zero rows or unknown claims: {sorted(unknown)}",
        )
    return result


def _validate_frozen_inputs(root: Path) -> None:
    validate_claim_001_live_binding(root)
    try:
        validate_transaction_integrity(root)
    except SuccessorEvidenceError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_WP33_ISSUANCE_INVALID", str(error)
        ) from error


def validate_mode(
    mode: str,
    root: Path = ROOT,
    *,
    selected_claims: Sequence[str] = (),
    require_review: bool = True,
) -> int:
    """Validate one WP38 oracle projection after its Just dependencies execute."""

    if mode not in {
        "input-integrity",
        "behavior",
        "causal-faults",
        "recovery-operations",
    }:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_MODE_INVALID", f"unknown evidence mode {mode}"
        )
    _validate_frozen_inputs(root)
    validate_append_only_transaction(root, require_review=require_review)
    selected = _select_claims(_expectation_index(root), selected_claims)
    if mode == "recovery-operations":
        selected = [
            row for row in selected if row["claim_id"] in EXPECTED_OPERATION_CLAIMS
        ]
        if not selected:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_ZERO_SELECTION",
                "recovery-operations selected zero operations claims",
            )
    return len(selected)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode",
        choices=("input-integrity", "behavior", "causal-faults", "recovery-operations"),
    )
    parser.add_argument(
        "--claim",
        action="append",
        default=[],
        help="select an exact reviewed claim id; zero or unknown selection fails closed",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        count = validate_mode(arguments.mode, selected_claims=arguments.claim)
    except ProductionEvidenceError as error:
        print(
            json.dumps(
                {
                    "status": "blocked",
                    "error_code": error.code,
                    "message": str(error),
                    "details": error.details,
                },
                sort_keys=True,
                separators=(",", ":"),
            ),
            file=sys.stderr,
        )
        return 1
    print(
        json.dumps(
            {
                "status": "valid",
                "mode": arguments.mode,
                "selected_claims": count,
                "transaction": TRANSACTION_ID,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
