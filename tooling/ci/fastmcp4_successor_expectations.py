"""Validate the independent WP43 FastMCP 4 expectation release.

This module deliberately imports no adapter, daemon, generated wire, or prior
acceptance implementation.  Expected values live only in the frozen v5 YAML
release and are compared structurally by this generic validator.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[2]
RELEASE_PATH = Path("contracts/acceptance/relational-fabric-v5")
RELEASE_ID = "relational-fabric-v5-wp43-r1"
EXPECTED_FILES = {
    "causal-fixtures.yaml",
    "expectations.yaml",
    "independent-review.yaml",
    "issuance.yaml",
    "negative-fixtures.yaml",
    "performance-method.yaml",
}
FROZEN_BYTES_SHA256 = {
    "causal-fixtures.yaml": "aa8f3c8eff736171f04d51d6e4c9ccef2f6cc149be0460026043d48a63a5329f",
    "expectations.yaml": "c9e2fa028343914dac6bb8b1a3266c10f8538cea62ac5dc77f86ae9155865cdc",
    "independent-review.yaml": "0fdb22c5509545bd8f330a805a15223337676ae99a93e1df6d9c82459d64b0f9",
    "issuance.yaml": "d8cfdac48d2d79b7b4faefcc3bc58a777b762528346b01207ece9d62f4738875",
    "negative-fixtures.yaml": "12ccb79370b729552926ad69118c444ffdbbaf92db77f4172bf4481df0e387c2",
    "performance-method.yaml": "ceb48efae08732a452bbbafa9642f1130eb81cffefcd4e7b2869925d2be5c6df",
}
REQUIRED_FAMILIES = {
    "successor_identity_and_pins",
    "modern_protocol_admission",
    "exact_catalog_and_extensions",
    "exact_public_schemas",
    "guarded_input_roundtrip",
    "atomic_start",
    "completion_authorization",
    "resource_authority",
    "cancellation_and_reconnect",
    "two_agent_isolation",
    "security_denial_matrix",
    "redaction_and_stdout_purity",
    "adapter_authority_zero_state",
    "predecessor_decommission",
    "performance_method_registration",
    "release_drift_control",
}
SUBCOMMANDS = {
    "successor-authority-integrity": (
        "fastmcp4-successor-authority-integrity-check",
        "PC-WP43-INT",
    ),
    "independent-expectation-review": (
        "fastmcp4-independent-expectation-review-check",
        "PC-WP43-BEH",
    ),
    "negative-fixture-independence": (
        "fastmcp4-negative-fixture-independence-check",
        "PC-WP43-NEG",
    ),
    "expectation-drift": ("fastmcp4-expectation-drift-check", "PC-WP43-OPS"),
}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
CLAIM_ID = re.compile(r"RFV5-FM4-(\d{3})\Z")
ALLOWED_DESIGN_PATHS = {
    "docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v1.md",
    "docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v2.md",
    "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md",
}


class ExpectationReleaseError(ValueError):
    """Fail-closed release validation error with a stable code."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class Bundle:
    root: Path
    release_dir: Path
    issuance: Mapping[str, Any]
    expectations: list[Mapping[str, Any]]
    causal: list[Mapping[str, Any]]
    negative: list[Mapping[str, Any]]
    review: Mapping[str, Any]
    performance: Mapping[str, Any]


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise ExpectationReleaseError(code, message)


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(
        isinstance(value, Mapping),
        "RFV5_SCHEMA_INVALID",
        f"{context} must be an object",
    )
    assert isinstance(value, Mapping)
    return value


def _rows(value: object, context: str) -> list[Mapping[str, Any]]:
    _require(
        isinstance(value, list), "RFV5_SCHEMA_INVALID", f"{context} must be a list"
    )
    assert isinstance(value, list)
    result: list[Mapping[str, Any]] = []
    for index, row in enumerate(value):
        result.append(_mapping(row, f"{context}[{index}]"))
    return result


def _load_yaml(path: Path, schema: str) -> Mapping[str, Any]:
    _require(
        path.is_file(), "RFV5_RELEASE_FILE_MISSING", f"missing release file: {path}"
    )
    try:
        value = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as error:
        raise ExpectationReleaseError(
            "RFV5_YAML_INVALID", f"{path}: {error}"
        ) from error
    document = _mapping(value, str(path))
    _require(
        document.get("schema") == schema, "RFV5_SCHEMA_INVALID", f"{path}: wrong schema"
    )
    _require(
        document.get("release_id") == RELEASE_ID,
        "RFV5_RELEASE_ID_DRIFT",
        f"{path}: wrong release_id",
    )
    return document


def load_bundle(root: Path = ROOT, release_path: Path = RELEASE_PATH) -> Bundle:
    release_dir = release_path if release_path.is_absolute() else root / release_path
    issuance = _load_yaml(
        release_dir / "issuance.yaml", "codefabric.fastmcp4-successor.issuance.v1"
    )
    expectations_doc = _load_yaml(
        release_dir / "expectations.yaml",
        "codefabric.fastmcp4-successor.expectations.v1",
    )
    causal_doc = _load_yaml(
        release_dir / "causal-fixtures.yaml",
        "codefabric.fastmcp4-successor.causal-fixtures.v1",
    )
    negative_doc = _load_yaml(
        release_dir / "negative-fixtures.yaml",
        "codefabric.fastmcp4-successor.negative-fixtures.v1",
    )
    review = _load_yaml(
        release_dir / "independent-review.yaml",
        "codefabric.fastmcp4-successor.independent-review.v1",
    )
    performance = _load_yaml(
        release_dir / "performance-method.yaml",
        "codefabric.fastmcp4-successor.performance-method.v1",
    )
    return Bundle(
        root=root,
        release_dir=release_dir,
        issuance=issuance,
        expectations=_rows(expectations_doc.get("expectations"), "expectations"),
        causal=_rows(causal_doc.get("fixtures"), "causal fixtures"),
        negative=_rows(negative_doc.get("fixtures"), "negative fixtures"),
        review=review,
        performance=performance,
    )


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _pointer(path: tuple[str, ...]) -> str:
    if not path:
        return "/"
    return "/" + "/".join(part.replace("~", "~0").replace("/", "~1") for part in path)


def _diff(left: object, right: object, path: tuple[str, ...] = ()) -> set[str]:
    if isinstance(left, Mapping) and isinstance(right, Mapping):
        differences: set[str] = set()
        for key in set(left) | set(right):
            key_path = (*path, str(key))
            if key not in left or key not in right:
                differences.add(_pointer(key_path))
            else:
                differences.update(_diff(left[key], right[key], key_path))
        return differences
    if left != right:
        return {_pointer(path)}
    return set()


def apply_merge_patch(target: object, patch: object) -> object:
    """Apply RFC 7396 JSON Merge Patch without importing target code."""

    if not isinstance(patch, Mapping):
        return copy.deepcopy(patch)
    result: dict[str, object] = dict(target) if isinstance(target, Mapping) else {}
    for key, value in patch.items():
        if value is None:
            result.pop(str(key), None)
        else:
            result[str(key)] = apply_merge_patch(result.get(str(key)), value)
    return result


def validate_observation(expectation: Mapping[str, Any], observation: object) -> None:
    expected = _mapping(expectation.get("expected_observation"), "expected_observation")
    differences = sorted(_diff(expected, observation))
    _require(
        not differences,
        "RFV5_OBSERVATION_DRIFT",
        "observation differs at " + ", ".join(differences),
    )


def _expectation_index(bundle: Bundle) -> dict[str, Mapping[str, Any]]:
    index: dict[str, Mapping[str, Any]] = {}
    families: set[str] = set()
    categories: set[str] = set()
    for row in bundle.expectations:
        claim_id = row.get("claim_id")
        match = CLAIM_ID.fullmatch(str(claim_id))
        _require(
            match is not None, "RFV5_CLAIM_ID_INVALID", f"invalid claim_id: {claim_id}"
        )
        _require(
            claim_id not in index,
            "RFV5_CLAIM_DUPLICATE",
            f"duplicate claim_id: {claim_id}",
        )
        family = row.get("family")
        _require(
            isinstance(family, str),
            "RFV5_SCHEMA_INVALID",
            f"{claim_id}: family missing",
        )
        _require(
            family not in families,
            "RFV5_FAMILY_DUPLICATE",
            f"duplicate family: {family}",
        )
        provenance = _mapping(row.get("provenance"), f"{claim_id}.provenance")
        _require(
            provenance.get("authored_from") == "accepted-design-manual-derivation"
            and provenance.get("expected_value_origin") == "independent-manual"
            and provenance.get("imports") == []
            and provenance.get("generated") is False
            and provenance.get("target_execution_used") is False
            and provenance.get("predecessor_expected_values_used") is False,
            "RFV5_EXPECTATION_NOT_INDEPENDENT",
            f"{claim_id}: generated, self-imported, target-derived, or predecessor-derived expectation",
        )
        basis = _rows(row.get("design_basis"), f"{claim_id}.design_basis")
        _require(
            all(item.get("path") in ALLOWED_DESIGN_PATHS for item in basis),
            "RFV5_FORBIDDEN_EXPECTATION_SOURCE",
            f"{claim_id}: design basis includes production/generated/history authority",
        )
        controlled = _mapping(
            row.get("controlled_input"), f"{claim_id}.controlled_input"
        )
        _require(
            isinstance(controlled.get("case_id"), str),
            "RFV5_SCHEMA_INVALID",
            f"{claim_id}: case_id",
        )
        _mapping(row.get("expected_observation"), f"{claim_id}.expected_observation")
        fault = _mapping(
            row.get("discriminating_fault"), f"{claim_id}.discriminating_fault"
        )
        _require(
            fault.get("fixture_id") == f"{claim_id}-N",
            "RFV5_FAULT_BINDING_INVALID",
            f"{claim_id}: discriminating fault is not bound",
        )
        row_categories = row.get("oracle_categories")
        _require(
            isinstance(row_categories, list) and row_categories,
            "RFV5_SCHEMA_INVALID",
            f"{claim_id}: categories",
        )
        categories.update(str(value) for value in row_categories)
        index[str(claim_id)] = row
        families.add(family)
    _require(
        set(index) == {f"RFV5-FM4-{n:03d}" for n in range(1, 17)},
        "RFV5_CLAIM_COVERAGE",
        "claim range is incomplete",
    )
    _require(
        families == REQUIRED_FAMILIES,
        "RFV5_FAMILY_COVERAGE",
        "required family coverage drifted",
    )
    _require(
        categories == {"INT", "BEH", "NEG", "OPS"},
        "RFV5_CATEGORY_COVERAGE",
        "oracle category coverage drifted",
    )
    return index


def _validate_issuance(bundle: Bundle) -> None:
    issuance = bundle.issuance
    _require(
        issuance.get("issued_for_packet") == "WP43",
        "RFV5_ISSUANCE_INVALID",
        "wrong packet",
    )
    suite = _mapping(issuance.get("suite"), "issuance.suite")
    _require(
        suite.get("suite_id") == "codefabric-relational-data-fabric"
        and suite.get("suite_version") == "2.3.0"
        and suite.get("predecessor_version") == "2.2.0",
        "RFV5_SUITE_IDENTITY_DRIFT",
        "successor suite identity drifted",
    )
    members = _rows(suite.get("members"), "issuance.suite.members")
    _require(
        {row.get("tag") for row in members}
        == {"SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"},
        "RFV5_SUITE_MEMBERSHIP_DRIFT",
        "suite membership is not the exact eight-role set",
    )
    selectors = _mapping(issuance.get("selectors"), "issuance.selectors")
    _require(
        set(selectors) == set(SUBCOMMANDS),
        "RFV5_SELECTOR_DRIFT",
        "four oracle selectors drifted",
    )
    counts = _mapping(issuance.get("counts"), "issuance.counts")
    _require(
        counts.get("expectations") == 16
        and counts.get("causal_fixtures") == 16
        and counts.get("negative_fixtures") == 16
        and counts.get("reviewed_claims") == 16,
        "RFV5_COUNT_DRIFT",
        "issuance counts drifted",
    )
    authoring = _mapping(issuance.get("authoring_constraints"), "authoring_constraints")
    _require(
        authoring.get("imports_production_modules") is False
        and authoring.get("generated_observations_used") is False
        and authoring.get("target_execution_used") is False
        and authoring.get("predecessor_expected_values_used") is False,
        "RFV5_EXPECTATION_NOT_INDEPENDENT",
        "issuance permits generated/self-imported expectations",
    )
    artifact_hashes = _mapping(issuance.get("artifact_sha256"), "artifact_sha256")
    expected_hashes = {
        name: digest
        for name, digest in FROZEN_BYTES_SHA256.items()
        if name != "issuance.yaml"
    }
    _require(
        {name: artifact_hashes.get(name) for name in expected_hashes} == expected_hashes
        and artifact_hashes.get("issuance.yaml") == "self-excluded-use-sha256sums",
        "RFV5_ISSUANCE_HASH_BINDING_DRIFT",
        "issuance artifact hash bindings drifted",
    )


def _parse_frontmatter(path: Path) -> Mapping[str, Any]:
    text = path.read_text(encoding="utf-8")
    _require(
        text.startswith("---\n"),
        "RFV5_SUITE_FRONTMATTER_INVALID",
        f"{path}: no frontmatter",
    )
    try:
        _, frontmatter, _ = text.split("---", 2)
    except ValueError as error:
        raise ExpectationReleaseError(
            "RFV5_SUITE_FRONTMATTER_INVALID", str(path)
        ) from error
    return _mapping(yaml.safe_load(frontmatter), f"{path}.frontmatter")


def validate_successor_authority(bundle: Bundle) -> int:
    _validate_issuance(bundle)
    _expectation_index(bundle)
    suite = _mapping(bundle.issuance.get("suite"), "issuance.suite")
    combined: list[str] = []
    for member in _rows(suite.get("members"), "issuance.suite.members"):
        relative = Path(str(member.get("path")))
        path = bundle.root / relative
        _require(
            path.is_file(),
            "RFV5_SUITE_MEMBER_MISSING",
            f"missing suite member: {relative}",
        )
        frontmatter = _parse_frontmatter(path)
        _require(
            frontmatter.get("artifact") == "authoritative-design"
            and frontmatter.get("suite_id") == "codefabric-relational-data-fabric"
            and frontmatter.get("suite_version") == "2.3.0"
            and frontmatter.get("artifact_tag") == member.get("tag")
            and str(frontmatter.get("artifact_version"))
            == str(member.get("artifact_version"))
            and frontmatter.get("authority_status") == "current"
            and frontmatter.get("predecessor_path") == member.get("predecessor_path"),
            "RFV5_SUITE_FRONTMATTER_DRIFT",
            f"{relative}: successor identity or predecessor linkage drifted",
        )
        combined.append(path.read_text(encoding="utf-8"))
    corpus = "\n".join(combined)
    for token in suite.get("required_authority_tokens", []):
        _require(
            str(token) in corpus,
            "RFV5_PIN_OR_PROTOCOL_CLAUSE_MISSING",
            f"missing authority token: {token}",
        )
    return len(combined)


def _validate_fixture_set(
    bundle: Bundle, fixtures: list[Mapping[str, Any]], *, negative: bool
) -> int:
    expectations = _expectation_index(bundle)
    suffix = "N" if negative else "C"
    seen: set[str] = set()
    for row in fixtures:
        claim_id = str(row.get("claim_id"))
        fixture_id = str(row.get("fixture_id"))
        _require(claim_id in expectations, "RFV5_FIXTURE_CLAIM_UNKNOWN", fixture_id)
        _require(
            fixture_id == f"{claim_id}-{suffix}", "RFV5_FIXTURE_ID_INVALID", fixture_id
        )
        _require(claim_id not in seen, "RFV5_FIXTURE_DUPLICATE", claim_id)
        expectation = expectations[claim_id]
        if negative:
            patch = _mapping(row.get("fault_patch"), f"{fixture_id}.fault_patch")
            expected = _mapping(
                expectation.get("expected_observation"), "expected_observation"
            )
            faulty = apply_merge_patch(expected, patch)
            differences = _diff(expected, faulty)
            declared = {str(value) for value in row.get("expected_mismatch_paths", [])}
            _require(
                differences and differences == declared,
                "RFV5_FAULT_NOT_DISCRIMINATING",
                f"{fixture_id}: mismatch paths drifted",
            )
            try:
                validate_observation(expectation, faulty)
            except ExpectationReleaseError as error:
                _require(
                    error.code == row.get("expected_error"),
                    "RFV5_FAULT_ERROR_DRIFT",
                    fixture_id,
                )
            else:
                raise ExpectationReleaseError("RFV5_FAULT_NOT_CAUGHT", fixture_id)
        else:
            controlled = _mapping(
                expectation.get("controlled_input"), "controlled_input"
            )
            _require(
                row.get("base_case_id") == controlled.get("case_id"),
                "RFV5_CAUSAL_BINDING_INVALID",
                fixture_id,
            )
            input_patch = _mapping(row.get("input_patch"), f"{fixture_id}.input_patch")
            output_patch = _mapping(
                row.get("expected_patch"), f"{fixture_id}.expected_patch"
            )
            input_differences = _diff(
                controlled, apply_merge_patch(controlled, input_patch)
            )
            expected = _mapping(
                expectation.get("expected_observation"), "expected_observation"
            )
            output_differences = _diff(
                expected, apply_merge_patch(expected, output_patch)
            )
            _require(
                input_differences
                == {str(value) for value in row.get("changed_input_paths", [])}
                and output_differences
                == {str(value) for value in row.get("changed_output_paths", [])}
                and input_differences
                and output_differences,
                "RFV5_CAUSAL_FIXTURE_NOT_DISCRIMINATING",
                fixture_id,
            )
        seen.add(claim_id)
    _require(
        seen == set(expectations),
        "RFV5_FIXTURE_COVERAGE",
        f"{suffix} fixture coverage incomplete",
    )
    return len(seen)


def _validate_performance(bundle: Bundle) -> None:
    value = bundle.performance
    registration = _mapping(value.get("registration"), "performance.registration")
    method = _mapping(value.get("execution_method"), "performance.execution_method")
    control = _mapping(value.get("minimal_control"), "performance.minimal_control")
    budget = _mapping(value.get("budget_source"), "performance.budget_source")
    workloads = _rows(value.get("workloads"), "performance.workloads")
    _require(
        value.get("method_id") == "fastmcp4-local-stdio-v1"
        and registration.get("registered_before_candidate_results") is True
        and registration.get("candidate_results_used") is False
        and registration.get("local_relaxation_permitted") is False
        and method.get("warmups_per_case") == 3
        and method.get("samples_per_case") == 30
        and control.get("control_id") == "minimal-fastmcp4-stdio-control-v1"
        and budget.get("source_id") == "wp43-preimplementation-operator-budget-v1"
        and budget.get("candidate_neutral") is True
        and budget.get("local_override_permitted") is False
        and len(workloads) == 10,
        "RFV5_PERFORMANCE_METHOD_DRIFT",
        "performance registration, control, samples, workloads, or budget source drifted",
    )


def validate_independent_review(bundle: Bundle) -> int:
    _validate_issuance(bundle)
    expectations = _expectation_index(bundle)
    causal_count = _validate_fixture_set(bundle, bundle.causal, negative=False)
    _validate_performance(bundle)
    review = _mapping(bundle.review.get("review"), "independent review")
    _require(
        review.get("status") == "accepted"
        and review.get("reviewer_is_author") is False
        and review.get("production_imports_used") is False
        and review.get("target_execution_used") is False
        and review.get("predecessor_expected_values_used") is False,
        "RFV5_REVIEW_NOT_INDEPENDENT",
        "independent review posture drifted",
    )
    _require(
        set(review.get("reviewed_claim_ids", [])) == set(expectations),
        "RFV5_REVIEW_COVERAGE",
        "reviewed claim set is incomplete",
    )
    _require(
        review.get("reviewed_expectations_sha256")
        == FROZEN_BYTES_SHA256["expectations.yaml"]
        and review.get("reviewed_causal_fixtures_sha256")
        == FROZEN_BYTES_SHA256["causal-fixtures.yaml"]
        and review.get("reviewed_negative_fixtures_sha256")
        == FROZEN_BYTES_SHA256["negative-fixtures.yaml"]
        and review.get("reviewed_performance_method_sha256")
        == FROZEN_BYTES_SHA256["performance-method.yaml"],
        "RFV5_REVIEW_HASH_BINDING_DRIFT",
        "independent review does not bind the frozen expectation release bytes",
    )
    return causal_count


def validate_negative_fixtures(bundle: Bundle) -> int:
    _validate_issuance(bundle)
    return _validate_fixture_set(bundle, bundle.negative, negative=True)


def validate_drift(bundle: Bundle) -> int:
    _validate_issuance(bundle)
    actual_files = {
        path.name for path in bundle.release_dir.iterdir() if path.is_file()
    }
    _require(
        actual_files == EXPECTED_FILES,
        "RFV5_RELEASE_FILESET_DRIFT",
        "release file set drifted",
    )
    for name, expected in FROZEN_BYTES_SHA256.items():
        actual = _sha256(bundle.release_dir / name)
        _require(
            actual == expected,
            "RFV5_ARTIFACT_HASH_DRIFT",
            f"{name}: {actual} != {expected}",
        )
    sources = _rows(
        bundle.issuance.get("immutable_source_inputs"), "immutable_source_inputs"
    )
    _require(
        len(sources) == 3,
        "RFV5_SOURCE_HASH_COVERAGE",
        "expected three immutable sources",
    )
    for source in sources:
        relative = str(source.get("path"))
        digest = str(source.get("sha256"))
        _require(
            relative in ALLOWED_DESIGN_PATHS and SHA256.fullmatch(digest) is not None,
            "RFV5_SOURCE_HASH_INVALID",
            relative,
        )
        _require(
            _sha256(bundle.root / relative) == digest,
            "RFV5_SOURCE_INPUT_DRIFT",
            relative,
        )
    _validate_performance(bundle)
    return len(FROZEN_BYTES_SHA256) + len(sources)


def validate_issuance(
    root: Path = ROOT,
    require_review: bool = True,
    release_path: Path = RELEASE_PATH,
) -> Bundle:
    """Validate and return the frozen issuance for artifact-contract consumers."""

    bundle = load_bundle(root, release_path)
    _validate_issuance(bundle)
    _expectation_index(bundle)
    if require_review:
        validate_independent_review(bundle)
        validate_negative_fixtures(bundle)
        validate_drift(bundle)
    return bundle


def _run(command: str, root: Path, release_path: Path) -> dict[str, object]:
    bundle = load_bundle(root, release_path)
    if command == "successor-authority-integrity":
        selected = validate_successor_authority(bundle)
    elif command == "independent-expectation-review":
        selected = validate_independent_review(bundle)
    elif command == "negative-fixture-independence":
        selected = validate_negative_fixtures(bundle)
    else:
        selected = validate_drift(bundle)
    _require(selected > 0, "RFV5_ZERO_SELECTION", f"{command}: selected no cases")
    oracle, criterion = SUBCOMMANDS[command]
    return {
        "criterion": criterion,
        "oracle": oracle,
        "release_id": RELEASE_ID,
        "selected_count": selected,
        "status": "passed",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=sorted(SUBCOMMANDS))
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--release-path", type=Path, default=RELEASE_PATH)
    args = parser.parse_args(argv)
    try:
        report = _run(args.command, args.root.resolve(), args.release_path)
    except (ExpectationReleaseError, OSError) as error:
        code = (
            error.code
            if isinstance(error, ExpectationReleaseError)
            else "RFV5_IO_ERROR"
        )
        print(
            json.dumps(
                {"code": code, "error": str(error), "status": "failed"}, sort_keys=True
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
