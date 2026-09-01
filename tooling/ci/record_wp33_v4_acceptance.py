"""Record an issuance-only review of the exact pending WP33 v4 evidence.

The recorder never changes the authored expectations or fixtures.  It writes
claim-specific digest-bound dispositions only to ``evidence-issuance.json``.
Acceptance requires every claim to be accepted; rejection and not-acceptance
remain valid review records but cannot satisfy a final acceptance oracle.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import shutil
import tempfile
from collections.abc import Iterator, Sequence
from contextlib import contextmanager
from pathlib import Path

from tooling.ci.successor_evidence_issuance_v4 import (
    ACTIVE_PLAN_POINTER,
    AUTHORITY_ROOT,
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    PRINCIPLES_PATH,
    ROOT,
    V4EvidenceError,
    review_candidate,
    validate_issuance,
)


def _json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()


def _write_file(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


@contextmanager
def _exclusive_review_lock(root: Path) -> Iterator[None]:
    """Serialize issuance reviewers without placing a lock in the contract tree."""

    lock_path = root / "target/governance-locks/wp33-v4-evidence-review.lock"
    lock_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def _stage_candidate(
    root: Path,
    destination: Path,
    *,
    issuance_bytes: bytes,
) -> None:
    """Copy only authority inputs required to validate the candidate."""

    shutil.copytree(root / AUTHORITY_ROOT, destination / AUTHORITY_ROOT)
    for relative in (
        ACTIVE_PLAN_POINTER,
        PRINCIPLES_PATH,
        EXPECTATIONS_PATH,
        FIXTURES_PATH,
    ):
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / relative, target)

    pointer = json.loads((root / ACTIVE_PLAN_POINTER).read_text(encoding="utf-8"))
    plan_path = Path(pointer["plan_path"])
    plan_target = destination / plan_path
    plan_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / plan_path, plan_target)
    plan = (root / plan_path).read_text(encoding="utf-8")
    design_line = next(
        line for line in plan.splitlines() if line.startswith("design_path:")
    )
    design_path = Path(design_line.partition(":")[2].strip())
    design_target = destination / design_path
    design_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / design_path, design_target)

    issuance = json.loads(issuance_bytes)
    for row in issuance["source_provenance"]:
        relative = Path(row["path"])
        target = destination / relative
        if target.exists():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / relative, target)

    (destination / ISSUANCE_PATH).write_bytes(issuance_bytes)


def record_review(
    *,
    root: Path,
    reviewer: str,
    reviewed_at: str,
    notes: Sequence[str],
    disposition: str,
) -> int:
    """Install only a digest-bound issuance review, returning claim count."""

    root = root.resolve()
    with _exclusive_review_lock(root):
        return _record_review_locked(
            root=root,
            reviewer=reviewer,
            reviewed_at=reviewed_at,
            notes=notes,
            disposition=disposition,
        )


def _record_review_locked(
    *,
    root: Path,
    reviewer: str,
    reviewed_at: str,
    notes: Sequence[str],
    disposition: str,
) -> int:
    """Validate and publish one review while holding the per-root lock."""

    expectation_path = root / EXPECTATIONS_PATH
    fixture_path = root / FIXTURES_PATH
    issuance_path = root / ISSUANCE_PATH
    initial_issuance = issuance_path.read_bytes()
    authored_before = {
        EXPECTATIONS_PATH: expectation_path.read_bytes(),
        FIXTURES_PATH: fixture_path.read_bytes(),
    }
    issuance = review_candidate(
        root,
        reviewer=reviewer,
        reviewed_at=reviewed_at,
        notes=notes,
        disposition=disposition,
    )
    issuance_bytes = _json_bytes(issuance)

    with tempfile.TemporaryDirectory(prefix="codefabric-wp33-v4-review-") as directory:
        staged = Path(directory)
        _stage_candidate(
            root,
            staged,
            issuance_bytes=issuance_bytes,
        )
        validate_issuance(staged, require_review=disposition == "accepted")

    if issuance_path.read_bytes() != initial_issuance:
        raise V4EvidenceError(
            "V4_REVIEW_CONFLICT",
            "issuance changed while the review candidate was being staged",
        )
    try:
        _write_file(issuance_path, issuance_bytes)
        validate_issuance(root, require_review=disposition == "accepted")
        authored_after = {
            EXPECTATIONS_PATH: expectation_path.read_bytes(),
            FIXTURES_PATH: fixture_path.read_bytes(),
        }
        if authored_after != authored_before:
            raise V4EvidenceError(
                "V4_REVIEW_MUTATED_AUTHORED_INPUT",
                "review transaction changed authored expectation or fixture bytes",
            )
    except BaseException:
        if issuance_path.read_bytes() == issuance_bytes:
            _write_file(issuance_path, initial_issuance)
        raise
    return len(issuance["independent_review"]["claim_reviews"])


def record_acceptance(
    *,
    root: Path,
    reviewer: str,
    reviewed_at: str,
    notes: Sequence[str],
) -> int:
    """Compatibility wrapper recording an all-claims acceptance."""

    return record_review(
        root=root,
        reviewer=reviewer,
        reviewed_at=reviewed_at,
        notes=notes,
        disposition="accepted",
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--reviewer", required=True)
    parser.add_argument("--reviewed-at", required=True)
    parser.add_argument("--note", action="append", default=[])
    parser.add_argument(
        "--disposition",
        choices=("accepted", "rejected", "not-accepted"),
        default="accepted",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        count = record_review(
            root=arguments.root,
            reviewer=arguments.reviewer,
            reviewed_at=arguments.reviewed_at,
            notes=arguments.note,
            disposition=arguments.disposition,
        )
    except (OSError, ValueError, V4EvidenceError) as error:
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
            {
                "status": arguments.disposition,
                "claims": count,
                "reviewer": arguments.reviewer,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
