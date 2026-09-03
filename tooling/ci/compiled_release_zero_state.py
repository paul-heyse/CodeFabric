"""Target-v7 zero-state assurance for displaced release/runtime architecture.

The candidate set is derived from the same complete live-surface census as the
post-purge package oracle. Historical designs, plans, reviews, and frozen
acceptance artifacts are intentionally not runtime authority and are outside
that census. Every selected Python and TOML file is parsed by the shared
coverage pass before token/structure checks run.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from tooling.ci.fastmcp4_post_purge_assurance import (
    ROOT,
    _iter_live_files,
    _read_live_text,
    validate_coverage,
)


class CompiledReleaseZeroStateError(ValueError):
    """A typed displaced-architecture or incomplete-proof failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


# Assemble forbidden spellings so the oracle remains in its own candidate
# census without self-matching. The test file is excluded because it must seed
# every class independently.
LEGACY_TOKEN_CLASSES: Mapping[str, str] = {
    "provider_marker": "CompiledProvider" + "Authority",
    "transformation_marker": "CompiledTransformation" + "Authority",
    "query_marker": "CompiledQuery" + "Authority",
    "proof_marker": "CompiledProof" + "Authority",
    "policy_marker": "CompiledPolicy" + "Authority",
    "global_release_lookup": "CompiledSemanticRelease::" + "current",
    "mixed_provider_profile": "CompiledProviderExecution" + "Profile",
    "disposable_pyrefly": "DisposablePyreflySidecar" + "Process",
    "v5_error_namespace": "RF" + "V5_",
    "v5_live_artifact_selector": "relational-fabric-" + "v5",
    "v5_plan_selector": "implementation_plan_" + "v5_",
    "old_performance_error_namespace": "WP" + "50_",
    "stale_suite_literal": "2." + "2.0",
    "old_production_evidence": "fastmcp4_" + "production_evidence",
    "old_successor_expectations": "fastmcp4_" + "successor_expectations",
    "reverse_state_feature": "repository-" + "state",
}
OLD_PROVIDER_LANE = re.compile(r"\bCompiledProviderLane\b")

SELF_PATHS = {
    Path("tooling/ci/compiled_release_zero_state.py"),
    Path("tooling/ci/test_compiled_release_zero_state.py"),
}
NEGATIVE_GUARD_PATHS = {
    Path("tooling/ci/remaining_legacy_zero_state.py"),
    Path("tooling/ci/test_remaining_legacy_zero_state.py"),
    Path("tooling/ci/fastmcp4_post_purge_assurance.py"),
    Path("tooling/ci/test_fastmcp4_post_purge_assurance.py"),
    Path("tooling/ci/feature_architecture.py"),
    Path("scripts/stable_graph_check.sh"),
}
TRANSITIVE_VERSION_PATHS = {
    Path("codefabric-cpg-mcp/uv.lock"),
}


def validate_compiled_release_zero_state(root: Path = ROOT) -> Mapping[str, object]:
    """Prove complete live coverage and reject every displaced release class."""

    coverage = validate_coverage(root)
    files, skipped, classified_symlinks = _iter_live_files(root)
    matches: list[str] = []
    scanned = 0
    for path in files:
        if path in SELF_PATHS or path in NEGATIVE_GUARD_PATHS:
            continue
        text = _read_live_text(root, path)
        scanned += 1
        for category, token in LEGACY_TOKEN_CLASSES.items():
            if category == "stale_suite_literal" and path in TRANSITIVE_VERSION_PATHS:
                continue
            if token in text:
                matches.append(f"{category}:{path}")
        if OLD_PROVIDER_LANE.search(text):
            matches.append(f"old_provider_lane:{path}")

    if scanned == 0:
        raise CompiledReleaseZeroStateError(
            "CFV7_RELEASE_ZERO_SELECTION",
            "compiled-release census selected no live files",
        )
    if matches:
        raise CompiledReleaseZeroStateError(
            "CFV7_RELEASE_LEGACY",
            "displaced compiled-release architecture remains: " + ", ".join(matches),
        )
    return {
        "scanned_live_files": scanned,
        "legacy_token_classes": len(LEGACY_TOKEN_CLASSES) + 1,
        "live_matches": 0,
        "unreadable": coverage["unreadable"],
        "unparsed": coverage["unparsed"],
        "skipped_directory_classes": skipped,
        "classified_symlinks": classified_symlinks,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        report = validate_compiled_release_zero_state(args.root)
    except CompiledReleaseZeroStateError as error:
        print(
            json.dumps(
                {"status": "failed", "code": error.code, "message": str(error)},
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps({"status": "passed", "report": report}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
