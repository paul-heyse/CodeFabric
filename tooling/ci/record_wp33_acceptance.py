"""Atomically record an independent acceptance of the exact frozen WP33 issuance.

The expectation authoring utility always emits a pending transaction.  This separate
recorder changes only reviewer-owned disposition fields after an independent reviewer
has accepted every exact claim/fixture binding.  It stages the candidate and runs the
production issuance validator before replacing the versioned record.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import shutil
import tempfile
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from tooling.ci.reissue_wp33_r3 import claim_review_rationale
from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    ROOT,
    SuccessorEvidenceError,
    validate_expectations,
    validate_fixtures,
    validate_issuance,
)


def _strict_json(path: Path) -> dict[str, Any]:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise SuccessorEvidenceError(
                    f"duplicate member in WP33 issuance: {key}"
                )
            value[key] = item
        return value

    try:
        value = json.loads(path.read_bytes(), object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise SuccessorEvidenceError(f"cannot decode WP33 issuance: {error}") from error
    if not isinstance(value, dict):
        raise SuccessorEvidenceError("WP33 issuance must be an object")
    return value


def accepted_candidate(
    *, root: Path, reviewer_identity: str, accepted_claims: Sequence[str]
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    """Return a validator-ready accepted candidate bound to the exact current files."""

    expectations = validate_expectations(root)
    fixtures = validate_fixtures(root, expectations)
    issuance = _strict_json(root / ISSUANCE_PATH)
    reviewer = issuance.get("reviewer")
    if not isinstance(reviewer, dict) or reviewer.get("identity") != reviewer_identity:
        raise SuccessorEvidenceError(
            "acceptance recorder identity differs from the designated independent reviewer"
        )
    if issuance.get("status") != "pending_independent_review":
        raise SuccessorEvidenceError(
            "only an exact pending WP33 issuance can receive independent acceptance"
        )
    expected_claims = {str(row["claim_id"]) for row in expectations}
    accepted = list(accepted_claims)
    if len(accepted) != len(set(accepted)) or set(accepted) != expected_claims:
        missing = sorted(expected_claims - set(accepted))
        extra = sorted(set(accepted) - expected_claims)
        raise SuccessorEvidenceError(
            f"independent review claim closure differs; missing={missing}, extra={extra}"
        )
    claims_by_id = {str(row["claim_id"]): row for row in expectations}
    reviews = issuance.get("claim_reviews")
    if not isinstance(reviews, list) or len(reviews) != len(expectations):
        raise SuccessorEvidenceError(
            "pending WP33 issuance review closure is incomplete"
        )
    candidate = copy.deepcopy(issuance)
    candidate["status"] = "accepted"
    for review in candidate["claim_reviews"]:
        if not isinstance(review, dict):
            raise SuccessorEvidenceError("WP33 claim review must be an object")
        claim_id = str(review.get("claim_id"))
        if (
            claim_id not in expected_claims
            or review.get("reviewer_id") != reviewer_identity
        ):
            raise SuccessorEvidenceError(
                "WP33 claim review is outside the accepted reviewer/claim closure"
            )
        basis = review.get("review_basis")
        if not isinstance(basis, dict):
            raise SuccessorEvidenceError(f"{claim_id} review basis is absent")
        review["disposition"] = "accepted"
        review["rationale"] = claim_review_rationale(
            claims_by_id[claim_id], basis, accepted=True
        )
    return candidate, expectations, fixtures


def record_acceptance(
    *, root: Path, reviewer_identity: str, accepted_claims: Sequence[str]
) -> int:
    """Validate and atomically install the independently accepted issuance."""

    candidate, expectations, fixtures = accepted_candidate(
        root=root,
        reviewer_identity=reviewer_identity,
        accepted_claims=accepted_claims,
    )
    with tempfile.TemporaryDirectory(prefix="codefabric-wp33-review-") as directory:
        staged_root = Path(directory)
        for relative_path in (EXPECTATIONS_PATH, FIXTURES_PATH):
            destination = staged_root / relative_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / relative_path, destination)
        staged_issuance = staged_root / ISSUANCE_PATH
        staged_issuance.parent.mkdir(parents=True, exist_ok=True)
        staged_issuance.write_text(
            json.dumps(candidate, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        validate_issuance(staged_root, expectations, fixtures)

    destination = root / ISSUANCE_PATH
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(candidate, stream, indent=2, ensure_ascii=False)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, destination)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    return len(expectations)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--reviewer", required=True)
    parser.add_argument("--accept-claim", action="append", default=[])
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        count = record_acceptance(
            root=arguments.root,
            reviewer_identity=arguments.reviewer,
            accepted_claims=arguments.accept_claim,
        )
    except SuccessorEvidenceError as error:
        print(
            json.dumps(
                {"status": "blocked", "message": str(error)},
                sort_keys=True,
                separators=(",", ":"),
            ),
            file=os.sys.stderr,
        )
        return 1
    print(
        json.dumps(
            {"status": "accepted", "claims": count, "reviewer": arguments.reviewer},
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
