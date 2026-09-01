"""Author and independently review the append-only WP38 evidence transaction.

This utility is intentionally separate from the WP33 expectation transaction.  It
consumes only an independently accepted WP33 issuance and the live WP38 contract
constants, validates every candidate with :mod:`tooling.ci.production_evidence`,
and only then writes canonical JSON Lines.  Drafting never implies acceptance: an
independent reviewer must append the fifth, chain-tip-bound entry explicitly.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import shutil
import sys
import tempfile
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

import rfc8785

from tooling.ci import production_evidence
from tooling.ci.production_evidence import ProductionEvidenceError
from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    SuccessorEvidenceError,
    validate_transaction_integrity,
)

EXECUTOR_IDENTITY = "wp38-production-evidence-executor"
REVIEW_SCOPE = (
    "append_only_chain",
    "successor_only_acceptance_edges",
    "claim_oracle_mapping",
    "production_recipe_closure",
    "limitations",
)

# These are capability observations, not launcher-name or launcher-version gates.
# In particular, a version string may aid diagnosis but can never establish hostile
# containment or authorize untrusted execution.
LIMITATION_EVIDENCE: Mapping[str, tuple[str, ...]] = {
    "HOST-UNTRUSTED-CONTAINMENT": (
        (
            "delegated cgroup-v2 cpu, memory, and pids bounds plus process-group and "
            "cgroup kill/reap are observed behavioral capabilities"
        ),
        (
            "application-owned compiled seccomp authorization is absent and the network, "
            "credential, live-workspace, inherited-file-descriptor, cleanup, and escape "
            "matrix remains unproved; untrusted execution is unavailable"
        ),
        (
            "launcher identity and launcher version are diagnostic-only metadata and are "
            "never admission or acceptance gates"
        ),
    ),
    "PERFORMANCE-EVIDENCE-NOT-CLAIMED": (
        (
            "No representative production workload or regression baseline is accepted; "
            "WP38 makes no performance or regression claim."
        ),
    ),
    "SCHEDULED-DEEP-ASSURANCE-DEFERRED": (
        (
            "scheduled mutation, fuzz, coverage, and supported-host assurance have not been "
            "executed as part of this transaction"
        ),
    ),
    "SUPPORTED-PLATFORM-COVERAGE": (
        (
            "current behavioral observations cover one Linux local-workstation development "
            "profile and do not establish another supported host"
        ),
    ),
}


def _strict_json_bytes(data: bytes, context: str) -> Mapping[str, Any]:
    try:
        value = json.loads(
            data, object_pairs_hook=production_evidence._reject_duplicates
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot decode {context}: {error}",
        ) from error
    return production_evidence._mapping(value, context)


def _strict_jsonl_bytes(data: bytes, context: str) -> list[Mapping[str, Any]]:
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot decode {context}: {error}",
        ) from error
    if not lines or any(not line.strip() for line in lines):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            f"{context} must be non-empty JSONL without blank rows",
        )
    rows: list[Mapping[str, Any]] = []
    for line_number, line in enumerate(lines, 1):
        try:
            value = json.loads(
                line, object_pairs_hook=production_evidence._reject_duplicates
            )
        except json.JSONDecodeError as error:
            raise ProductionEvidenceError(
                "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
                f"cannot decode {context} row {line_number}: {error}",
            ) from error
        rows.append(production_evidence._mapping(value, f"{context} row {line_number}"))
    return rows


def _accepted_authority(
    root: Path,
) -> tuple[Mapping[str, Any], list[Mapping[str, Any]]]:
    """Return a stable snapshot only after the live WP33 validator accepts it."""

    paths = (EXPECTATIONS_PATH, FIXTURES_PATH, ISSUANCE_PATH)
    try:
        before = {path: (root / path).read_bytes() for path in paths}
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot snapshot WP33 authority: {error}",
        ) from error
    try:
        validate_transaction_integrity(root)
    except SuccessorEvidenceError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_WP33_ISSUANCE_INVALID", str(error)
        ) from error
    try:
        after = {path: (root / path).read_bytes() for path in paths}
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot confirm WP33 authority snapshot: {error}",
        ) from error
    if before != after:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_AUTHORITY_CHANGED_DURING_AUTHORING",
            "WP33 authority changed while the candidate transaction was being authored",
        )

    issuance = _strict_json_bytes(after[ISSUANCE_PATH], "WP33 evidence issuance")
    if issuance.get("schema_version") != 1 or issuance.get("status") != "accepted":
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_WP33_ISSUANCE_INVALID",
            "WP38 authoring requires an independently accepted WP33 schema-version-1 issuance",
        )
    expectations = _strict_jsonl_bytes(after[EXPECTATIONS_PATH], "WP33 expectations")
    return issuance, expectations


def _append_entry(
    entries: list[dict[str, Any]],
    entry_kind: str,
    payload: Mapping[str, Any],
    *,
    recorded_by: str,
) -> None:
    if not recorded_by:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_SCHEMA_INVALID",
            "transaction recorder identity must be non-empty",
        )
    entry: dict[str, Any] = {
        "schema_version": 1,
        "sequence": len(entries) + 1,
        "transaction_id": production_evidence.TRANSACTION_ID,
        "entry_kind": entry_kind,
        "recorded_by": recorded_by,
        "previous_entry_b3": entries[-1]["entry_b3"] if entries else None,
        "payload": dict(payload),
    }
    entry["entry_b3"] = production_evidence._canonical_b3(entry)
    entries.append(entry)


def draft_transaction(
    root: Path = production_evidence.ROOT,
    *,
    executor_identity: str = EXECUTOR_IDENTITY,
) -> tuple[dict[str, Any], ...]:
    """Draft the four execution entries from accepted WP33 and live WP38 constants."""

    issuance, expectations = _accepted_authority(root)
    author = production_evidence._mapping(issuance.get("author"), "issuance author")
    reviewer = production_evidence._mapping(
        issuance.get("reviewer"), "issuance reviewer"
    )

    entries: list[dict[str, Any]] = []
    _append_entry(
        entries,
        "transaction_opened",
        {
            "suite": production_evidence.SUITE,
            "packet": "WP38",
            "oracles": list(production_evidence.ORACLES),
            "acceptance_inputs": list(production_evidence.EXPECTED_ACCEPTANCE_INPUTS),
            "diagnostic_inputs": [],
            "issuance_binding": {
                "issuance_id": issuance.get("issuance_id"),
                "reviewed_content_id": issuance.get("reviewed_content_id"),
                "expectation_author": author.get("identity"),
                "independent_reviewer": reviewer.get("identity"),
            },
        },
        recorded_by=executor_identity,
    )

    claim_rows: list[dict[str, Any]] = []
    for expectation in sorted(expectations, key=lambda row: str(row.get("claim_id"))):
        claim_id = production_evidence._nonempty_string(
            expectation.get("claim_id"), "claim id"
        )
        consumer = production_evidence._mapping(
            expectation.get("future_consumer"), f"{claim_id} future consumer"
        )
        claim_rows.append(
            {
                "claim_id": claim_id,
                "claim_family": expectation.get("claim_family"),
                "issued_observation_recipe": consumer.get("oracle"),
                "input_oracle": production_evidence.INPUT_ORACLE,
                "positive_oracle": production_evidence.BEHAVIOR_ORACLE,
                "causal_oracle": production_evidence.CAUSAL_ORACLE,
                "operations_oracle": (
                    production_evidence.OPERATIONS_ORACLE
                    if claim_id in production_evidence.EXPECTED_OPERATION_CLAIMS
                    else None
                ),
            }
        )
    _append_entry(
        entries,
        "claim_oracle_mapping",
        {"claims": claim_rows},
        recorded_by=executor_identity,
    )
    _append_entry(
        entries,
        "production_execution_contract",
        {
            "recipe_dependencies": {
                oracle: list(dependencies)
                for oracle, dependencies in production_evidence.EXPECTED_RECIPE_DEPENDENCIES.items()
            },
            "claim_018_successor_tests": list(production_evidence.CLAIM_018_TESTS),
            "production_observation_mode": "live_successor_recipe_execution",
            "historical_executables_required": False,
        },
        recorded_by=executor_identity,
    )
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
                    "evidence": list(LIMITATION_EVIDENCE[limitation_id]),
                }
                for limitation_id, (
                    state,
                    release_effect,
                ) in production_evidence.EXPECTED_LIMITATIONS.items()
            ],
        },
        recorded_by=executor_identity,
    )
    if tuple(entry["entry_kind"] for entry in entries) != (
        production_evidence.EXPECTED_ENTRY_KINDS
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID",
            "draft did not produce the exact four-entry WP38 execution chain",
        )
    return tuple(entries)


def _jsonl_bytes(entries: Sequence[Mapping[str, Any]]) -> bytes:
    return b"".join(rfc8785.dumps(dict(entry)) + b"\n" for entry in entries)


def _stage_and_validate(
    root: Path,
    entries: Sequence[Mapping[str, Any]],
    *,
    require_review: bool,
) -> int:
    """Validate a candidate with the production validator before any destination write."""

    # This proves that the source authority is currently accepted.  The production
    # validator then proves the exact candidate against a byte-for-byte snapshot.
    _accepted_authority(root)
    copied_paths = (
        EXPECTATIONS_PATH,
        FIXTURES_PATH,
        ISSUANCE_PATH,
        production_evidence.JUSTFILE_PATH,
    )
    with tempfile.TemporaryDirectory(prefix="codefabric-wp38-candidate-") as directory:
        candidate_root = Path(directory)
        for relative_path in copied_paths:
            source = root / relative_path
            destination = candidate_root / relative_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            try:
                shutil.copyfile(source, destination)
            except OSError as error:
                raise ProductionEvidenceError(
                    "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
                    f"cannot stage {relative_path}: {error}",
                ) from error
        transaction_path = candidate_root / production_evidence.TRANSACTION_PATH
        transaction_path.parent.mkdir(parents=True, exist_ok=True)
        transaction_path.write_bytes(_jsonl_bytes(entries))
        return production_evidence.validate_append_only_transaction(
            candidate_root, require_review=require_review
        )


def append_accepted_review(
    entries: Sequence[Mapping[str, Any]],
    *,
    reviewer_identity: str,
    root: Path = production_evidence.ROOT,
) -> tuple[dict[str, Any], ...]:
    """Append one independent accepted review bound to the current chain tip."""

    issuance, _ = _accepted_authority(root)
    author = production_evidence._mapping(issuance.get("author"), "issuance author")
    if not reviewer_identity:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "independent reviewer identity must be non-empty",
        )
    execution_recorders = {str(entry.get("recorded_by")) for entry in entries}
    if reviewer_identity == author.get("identity"):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "the WP33 expectation author cannot accept the WP38 transaction",
        )
    if reviewer_identity in execution_recorders:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "the WP38 executor cannot self-review the transaction",
        )
    if tuple(entry.get("entry_kind") for entry in entries) != (
        production_evidence.EXPECTED_ENTRY_KINDS
    ):
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID",
            "an accepted review may only append to the exact four-entry execution chain",
        )
    _stage_and_validate(root, entries, require_review=False)

    reviewed = copy.deepcopy([dict(entry) for entry in entries])
    reviewed_tip = production_evidence._nonempty_string(
        reviewed[-1].get("entry_b3"), "reviewed chain tip"
    )
    _append_entry(
        reviewed,
        "review_accepted",
        {
            "reviewer_identity": reviewer_identity,
            "reviewed_through_entry_b3": reviewed_tip,
            "implementation_owner": False,
            "expectation_author": False,
            "verdict": "accepted",
            "scope": list(REVIEW_SCOPE),
        },
        recorded_by=reviewer_identity,
    )
    _stage_and_validate(root, reviewed, require_review=True)
    return tuple(reviewed)


def load_transaction(path: Path) -> tuple[Mapping[str, Any], ...]:
    """Strictly load an existing draft without collapsing duplicate members."""

    try:
        data = path.read_bytes()
    except OSError as error:
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_INPUT_UNREADABLE",
            f"cannot load transaction draft: {error}",
        ) from error
    return tuple(_strict_jsonl_bytes(data, "WP38 transaction draft"))


def write_validated_transaction(
    output: Path,
    entries: Sequence[Mapping[str, Any]],
    *,
    root: Path = production_evidence.ROOT,
    require_review: bool,
) -> int:
    """Validate with the production contract and atomically write canonical JSONL."""

    count = _stage_and_validate(root, entries, require_review=require_review)
    data = _jsonl_bytes(entries)
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.", suffix=".tmp", dir=output.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, output)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    return count


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    draft = subparsers.add_parser("draft", help="author the four execution entries")
    draft.add_argument("--root", type=Path, default=production_evidence.ROOT)
    draft.add_argument("--output", type=Path, required=True)
    draft.add_argument("--executor", default=EXECUTOR_IDENTITY)

    review = subparsers.add_parser(
        "review", help="append an independent accepted review to an existing draft"
    )
    review.add_argument("--root", type=Path, default=production_evidence.ROOT)
    review.add_argument("--input", type=Path, required=True)
    review.add_argument("--output", type=Path, required=True)
    review.add_argument("--reviewer", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "draft":
            entries = draft_transaction(
                arguments.root, executor_identity=arguments.executor
            )
            count = write_validated_transaction(
                arguments.output,
                entries,
                root=arguments.root,
                require_review=False,
            )
        else:
            entries = load_transaction(arguments.input)
            reviewed = append_accepted_review(
                entries,
                reviewer_identity=arguments.reviewer,
                root=arguments.root,
            )
            count = write_validated_transaction(
                arguments.output,
                reviewed,
                root=arguments.root,
                require_review=True,
            )
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
                "command": arguments.command,
                "claims": count,
                "output": str(arguments.output),
                "transaction": production_evidence.TRANSACTION_ID,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
