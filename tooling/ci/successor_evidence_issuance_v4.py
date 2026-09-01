"""Validate the independent relational-fabric v4 expectation issuance.

The v4 issuance is an input to production implementation, never an output of it.
This module therefore depends only on repository governance artifacts and the
three committed JSON/JSONL inputs under ``contracts/acceptance``.  It does not
import production Rust/Python code, generated Protobuf modules, or predecessor
evidence validators.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from tooling.ci.artifact_contracts import ArtifactContractError, parse_frontmatter
from tooling.ci.successor_evidence_contracts_v4 import (
    V4ContractError,
    validate_evidence_contracts,
)

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_ROOT = Path("contracts/acceptance/relational-fabric-v4")
EXPECTATIONS_PATH = EVIDENCE_ROOT / "expectations.jsonl"
FIXTURES_PATH = EVIDENCE_ROOT / "negative-fixtures.jsonl"
ISSUANCE_PATH = EVIDENCE_ROOT / "evidence-issuance.json"
ACTIVE_PLAN_POINTER = Path("docs/plans/active-plan.json")
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md"
)
PRINCIPLES_PATH = Path("docs/library_ref/full_data_fabric_design_principles_v2.md")
AUTHORITY_ROOT = Path("docs/authoritative_design")
DESIGN_PATH = Path(
    "docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md"
)
DESIGN_V4_PATH = Path(
    "docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md"
)
DESIGN_V3_PATH = Path(
    "docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-08-31_v3.md"
)

SUITE_ID = "codefabric-relational-data-fabric"
SUITE_VERSION = "2.2.0"
SUITE_IDENTITY = f"{SUITE_ID}@{SUITE_VERSION}"
EVIDENCE_RELEASE = "wp33-v4-r6"
REQUIRED_TAGS = frozenset({"SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"})

EXPECTATION_SCHEMA = "codefabric.relational-fabric-v4.expectation.v1"
FIXTURE_SCHEMA = "codefabric.relational-fabric-v4.fixture.v1"
ISSUANCE_SCHEMA = "codefabric.relational-fabric-v4.evidence-issuance.v1"
CLAIM_ID = re.compile(r"RFV4-CLAIM-(\d{3})\Z")
FIXTURE_ID = re.compile(r"RFV4-FIX-(\d{3})-([CN])\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
IDENTITY = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:@-]{2,127}\Z")
SECTION_CITATION = re.compile(r"§(\d+(?:\.\d+)*)\Z")
SECTION_HEADING = re.compile(r"^#{2,6}\s+(\d+(?:\.\d+)*)(?:\.)?(?=\s|$)", re.MULTILINE)

ORACLES = (
    "successor-authority-expectation-integrity-check",
    "independent-expected-relation-review-check",
    "negative-fixture-independence-check",
    "expectation-drift-selector-sensitivity-check",
)

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
EXPECTED_CLAIM_IDS = tuple(
    f"RFV4-CLAIM-{number:03d}" for number in range(1, len(EXPECTED_FAMILIES) + 1)
)
EXPECTED_FIXTURE_IDS = tuple(
    f"RFV4-FIX-{number:03d}-{suffix}"
    for number in range(1, len(EXPECTED_FAMILIES) + 1)
    for suffix in ("C", "N")
)

EXPECTATION_KEYS = {
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
FIXTURE_KEYS = {
    "schema",
    "fixture_id",
    "claim_id",
    "fixture_kind",
    "fixture_input",
    "expected_decoded",
    "distinguishes",
}
ISSUANCE_KEYS = {
    "schema",
    "suite_identity",
    "evidence_release",
    "status",
    "source_provenance",
    "artifact_digests",
    "counts",
    "id_ranges",
    "authoring_constraints",
    "limitations",
    "independent_review",
    "selectors",
}

FORBIDDEN_SOURCE_PREFIXES = (
    "src/",
    "target/",
    "rustc-extractor/src/",
    "pyrefly-sidecar/src/",
    "codefabric-cpg-mcp/src/",
    "contracts/acceptance/relational-fabric-v1/",
    "contracts/acceptance/relational-fabric-v3/",
    "contracts/generated/",
)
FORBIDDEN_SOURCE_FRAGMENTS = (
    "generated cpgd.v2 output",
    "relational-fabric-v3 evidence values",
    "v1 runtime fixtures",
)


class V4EvidenceError(ValueError):
    """A fail-closed v4 evidence schema, provenance, or review failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class AuthorityMaster:
    """One terminal authoritative-suite role."""

    path: Path
    tag: str
    version: str
    predecessor: Path


@dataclass(frozen=True)
class V4Issuance:
    """Validated v4 evidence rows and their issuance record."""

    expectations: tuple[dict[str, Any], ...]
    fixtures: tuple[dict[str, Any], ...]
    issuance: dict[str, Any]
    terminal_suite: Mapping[str, AuthorityMaster]


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise V4EvidenceError(
                "V4_DUPLICATE_JSON_MEMBER", f"duplicate JSON member {key!r}"
            )
        value[key] = item
    return value


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise V4EvidenceError(
            "V4_INPUT_UNREADABLE", f"cannot decode {context}: {error}"
        ) from error
    return dict(_mapping(value, context))


def _load_jsonl(path: Path, context: str) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise V4EvidenceError(
            "V4_INPUT_UNREADABLE", f"cannot read {context}: {error}"
        ) from error
    if not lines or any(not line.strip() for line in lines):
        raise V4EvidenceError(
            "V4_JSONL_FRAMING_INVALID", f"{context} must contain nonblank JSONL rows"
        )
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, 1):
        try:
            value = json.loads(line, object_pairs_hook=_reject_duplicates)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise V4EvidenceError(
                "V4_INPUT_UNREADABLE",
                f"cannot decode {context} line {line_number}: {error}",
            ) from error
        rows.append(dict(_mapping(value, f"{context} line {line_number}")))
    return rows


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} must be an object")
    return value


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    if set(value) != expected:
        raise V4EvidenceError(
            "V4_SCHEMA_INVALID",
            f"{context} keys differ: expected={sorted(expected)} observed={sorted(value)}",
        )


def _nonempty_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise V4EvidenceError(
            "V4_SCHEMA_INVALID", f"{context} must be a nonempty string"
        )
    return value


def _identity(value: object, context: str) -> str:
    text = _nonempty_string(value, context)
    if text != text.strip() or IDENTITY.fullmatch(text) is None:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", f"{context} is not a canonical identity"
        )
    return text


def _review_timestamp(value: object, context: str) -> str:
    text = _nonempty_string(value, context)
    try:
        parsed = datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=UTC)
    except ValueError as error:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", f"{context} is not a valid UTC timestamp"
        ) from error
    if parsed.strftime("%Y-%m-%dT%H:%M:%SZ") != text:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", f"{context} is not a canonical UTC timestamp"
        )
    return text


def _string_list(value: object, context: str) -> list[str]:
    if not isinstance(value, list):
        raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} must be an array")
    result = [_nonempty_string(item, context) for item in value]
    if len(result) != len(set(result)):
        raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} contains duplicates")
    return result


def _nonempty_object(value: object, context: str) -> Mapping[str, Any]:
    result = _mapping(value, context)
    if not result:
        raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} must not be empty")
    return result


def _nonempty_structured(value: object, context: str) -> object:
    if isinstance(value, (Mapping, list)):
        if not value:
            raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} must not be empty")
    elif isinstance(value, str):
        _nonempty_string(value, context)
    else:
        raise V4EvidenceError(
            "V4_SCHEMA_INVALID", f"{context} must be structured or textual"
        )
    return value


def _relative_path(value: object, context: str) -> Path:
    text = _nonempty_string(value, context)
    path = Path(text)
    if path.is_absolute() or ".." in path.parts or path.as_posix() != text:
        raise V4EvidenceError(
            "V4_PATH_INVALID", f"{context} is not repository-relative"
        )
    return path


def _sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise V4EvidenceError(
            "V4_INPUT_UNREADABLE", f"cannot hash {path}: {error}"
        ) from error


def _canonical_sha256(value: object) -> str:
    payload = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def _apply_json_merge_patch(target: object, patch: object) -> object:
    """Apply RFC 7396 merge-patch semantics to JSON-compatible values."""

    if not isinstance(patch, Mapping):
        return copy.deepcopy(patch)
    result: dict[str, Any] = (
        copy.deepcopy(dict(target)) if isinstance(target, Mapping) else {}
    )
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = _apply_json_merge_patch(result.get(key), value)
    return result


def _validate_closed_merge_patch(
    target: Mapping[str, Any], patch: Mapping[str, Any], *, context: str
) -> None:
    """Reject top-level causal knobs absent from the controlled input.

    Nested domain-map keys are validated by the per-family contract layer;
    generic RFC 7396 cannot distinguish an object field from a map entry.
    """

    for key in patch:
        if key not in target:
            raise V4EvidenceError(
                "V4_FIXTURE_NOT_DISCRIMINATING",
                f"{context} introduces unbound controlled-input path {key!r}",
            )


def _issuance_review_projection(issuance: Mapping[str, Any]) -> dict[str, Any]:
    return {
        key: issuance[key]
        for key in (
            "schema",
            "suite_identity",
            "evidence_release",
            "source_provenance",
            "artifact_digests",
            "counts",
            "id_ranges",
            "authoring_constraints",
            "limitations",
            "selectors",
        )
    }


def _frontmatter(path: Path) -> Mapping[str, Any] | None:
    try:
        if not path.read_bytes().startswith(b"---\n"):
            return None
        return parse_frontmatter(path)
    except (OSError, ArtifactContractError) as error:
        raise V4EvidenceError(
            "V4_AUTHORITY_INVALID", f"cannot read authority metadata {path}: {error}"
        ) from error


def discover_terminal_suite(root: Path = ROOT) -> dict[str, AuthorityMaster]:
    """Derive the unique synchronized terminal suite from predecessor edges."""

    authority = root / AUTHORITY_ROOT
    if not authority.is_dir():
        raise V4EvidenceError(
            "V4_AUTHORITY_INVALID", "authoritative-design root is absent"
        )
    records: dict[Path, tuple[Mapping[str, Any], Path]] = {}
    for absolute in sorted(authority.glob("*.md")):
        metadata = _frontmatter(absolute)
        if metadata is None or metadata.get("artifact") != "authoritative-design":
            continue
        if metadata.get("suite_id") != SUITE_ID:
            continue
        relative = absolute.relative_to(root)
        predecessor = _relative_path(
            metadata.get("predecessor_path"), f"{relative} predecessor_path"
        )
        records[relative] = (metadata, predecessor)
    if not records:
        raise V4EvidenceError(
            "V4_AUTHORITY_INVALID", "suite discovery selected zero masters"
        )

    predecessor_paths = {predecessor for _, predecessor in records.values()}
    terminal_paths = set(records) - predecessor_paths
    terminals: dict[str, AuthorityMaster] = {}
    for path in sorted(terminal_paths):
        metadata, predecessor = records[path]
        tag = _nonempty_string(metadata.get("artifact_tag"), f"{path} artifact_tag")
        version = _nonempty_string(
            metadata.get("suite_version"), f"{path} suite_version"
        )
        _nonempty_string(metadata.get("artifact_version"), f"{path} artifact_version")
        if version != SUITE_VERSION:
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID",
                f"terminal role {tag} is not synchronized at {SUITE_VERSION}: {path}",
            )
        if metadata.get("authority_status") != "current":
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID", f"terminal role {tag} is not current: {path}"
            )
        if tag in terminals:
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID", f"duplicate terminal authority role {tag}"
            )
        predecessor_record = records.get(predecessor)
        if predecessor_record is None:
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID",
                f"terminal predecessor is unresolved: {predecessor}",
            )
        predecessor_metadata, _ = predecessor_record
        if predecessor_metadata.get("artifact_tag") != tag:
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID", f"terminal predecessor changes role for {tag}"
            )
        allocated_name = "_2.2_" in path.name if tag == "RM" else "v2.2" in path.name
        if not allocated_name:
            raise V4EvidenceError(
                "V4_AUTHORITY_INVALID",
                f"terminal {tag} lacks the v2.2 filename allocation",
            )
        terminals[tag] = AuthorityMaster(path, tag, version, predecessor)
    if set(terminals) != REQUIRED_TAGS:
        raise V4EvidenceError(
            "V4_AUTHORITY_INVALID",
            "terminal suite roles differ: "
            f"missing={sorted(REQUIRED_TAGS - set(terminals))}, "
            f"extra={sorted(set(terminals) - REQUIRED_TAGS)}",
        )
    return terminals


def _active_plan(root: Path) -> tuple[Path, Path]:
    pointer = _load_json(root / ACTIVE_PLAN_POINTER, "active plan pointer")
    _strict_keys(pointer, {"schema_version", "plan_path"}, "active plan pointer")
    if pointer["schema_version"] != 1:
        raise V4EvidenceError(
            "V4_ACTIVE_PLAN_INVALID", "active plan pointer schema differs"
        )
    selected = _relative_path(pointer["plan_path"], "active plan pointer plan_path")
    if selected != PLAN_PATH:
        raise V4EvidenceError(
            "V4_ACTIVE_PLAN_INVALID",
            f"v4 evidence is not bound to the active plan: {selected}",
        )
    try:
        metadata = parse_frontmatter(root / selected)
    except ArtifactContractError as error:
        raise V4EvidenceError("V4_ACTIVE_PLAN_INVALID", str(error)) from error
    if (
        metadata.get("artifact") != "implementation-plan"
        or metadata.get("plan_id")
        != "codefabric-execution-proved-relational-data-fabric"
        or metadata.get("version") != "v4"
        or metadata.get("status") != "approved"
        or metadata.get("design_version") != "v5"
    ):
        raise V4EvidenceError(
            "V4_ACTIVE_PLAN_INVALID", "active v4 plan approval/design identity differs"
        )
    design = _relative_path(metadata.get("design_path"), "active plan design_path")
    if design != DESIGN_PATH or not (root / design).is_file():
        raise V4EvidenceError(
            "V4_ACTIVE_PLAN_INVALID", f"active design identity differs: {design}"
        )
    design_metadata = _frontmatter(root / design)
    if design_metadata is None or any(
        (
            design_metadata.get("artifact") != "interface-design-review",
            design_metadata.get("version") != "v5",
            design_metadata.get("status") != "complete",
            design_metadata.get("verdict") != "aligned",
            design_metadata.get("plan_path") != PLAN_PATH.as_posix(),
        )
    ):
        raise V4EvidenceError(
            "V4_ACTIVE_PLAN_INVALID", "accepted design frontmatter identity differs"
        )
    plan_text = (root / selected).read_text(encoding="utf-8")
    for oracle in ORACLES:
        if f"Executable oracle: `{oracle}`" not in plan_text:
            raise V4EvidenceError(
                "V4_ACTIVE_PLAN_INVALID",
                f"WP33 oracle is absent from active plan: {oracle}",
            )
    return selected, design


def _validate_source_provenance(
    root: Path,
    value: object,
    *,
    plan_path: Path,
    design_path: Path,
    terminal_suite: Mapping[str, AuthorityMaster],
) -> set[Path]:
    if not isinstance(value, list) or not value:
        raise V4EvidenceError("V4_PROVENANCE_INVALID", "source_provenance is empty")
    observed: dict[Path, str] = {}
    for index, row_value in enumerate(value, 1):
        row = _mapping(row_value, f"source provenance row {index}")
        _strict_keys(row, {"path", "sha256", "role"}, f"source provenance row {index}")
        path = _relative_path(row["path"], f"source provenance row {index} path")
        digest = _nonempty_string(
            row["sha256"], f"source provenance row {index} sha256"
        )
        role = _nonempty_string(row["role"], f"source provenance row {index} role")
        if path in observed:
            raise V4EvidenceError(
                "V4_PROVENANCE_INVALID", f"duplicate source path: {path}"
            )
        if any(
            path.as_posix().startswith(prefix) for prefix in FORBIDDEN_SOURCE_PREFIXES
        ):
            raise V4EvidenceError(
                "V4_PROVENANCE_FORBIDDEN_SOURCE", f"forbidden evidence input: {path}"
            )
        if SHA256.fullmatch(digest) is None or digest != _sha256(root / path):
            raise V4EvidenceError(
                "V4_PROVENANCE_DRIFT", f"source provenance digest differs: {path}"
            )
        observed[path] = role
    expected = {
        plan_path: "active-implementation-plan",
        design_path: "accepted-operational-design",
        DESIGN_V4_PATH: "incorporated-forward-interface-design",
        DESIGN_V3_PATH: "incorporated-relational-interface-design",
        PRINCIPLES_PATH: "data-fabric-doctrine",
        **{master.path: tag for tag, master in terminal_suite.items()},
    }
    if observed != expected:
        raise V4EvidenceError(
            "V4_PROVENANCE_INVALID",
            "source provenance path/role closure differs: "
            f"missing={sorted(set(expected) - set(observed))}, "
            f"extra={sorted(set(observed) - set(expected))}, "
            f"role_mismatch={sorted(path for path in set(expected) & set(observed) if expected[path] != observed[path])}",
        )
    return set(observed)


def _document_sections(root: Path, path: Path) -> set[str]:
    try:
        text = (root / path).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise V4EvidenceError(
            "V4_INPUT_UNREADABLE", f"cannot read cited authority {path}: {error}"
        ) from error
    sections = set(SECTION_HEADING.findall(text))
    if not sections:
        raise V4EvidenceError(
            "V4_PROVENANCE_INVALID", f"cited authority has no numbered sections: {path}"
        )
    return sections


def _validate_review(value: object, context: str) -> None:
    """Validate the immutable author-to-reviewer handoff marker."""

    review = _mapping(value, context)
    _strict_keys(review, {"status", "reviewer"}, context)
    if review != {"status": "pending-independent-review", "reviewer": None}:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID",
            f"{context} must remain the immutable pending-review handoff",
        )


def validate_expectations(
    root: Path,
    provenance_paths: set[Path],
) -> list[dict[str, Any]]:
    rows = _load_jsonl(root / EXPECTATIONS_PATH, "v4 expectations")
    if len(rows) != len(EXPECTED_CLAIM_IDS):
        raise V4EvidenceError(
            "V4_EXPECTATION_CLOSURE_INVALID",
            f"expected {len(EXPECTED_CLAIM_IDS)} claims, observed {len(rows)}",
        )
    seen_titles: set[str] = set()
    seen_faults: set[str] = set()
    section_cache: dict[Path, set[str]] = {}
    for index, row in enumerate(rows, 1):
        context = f"expectation row {index}"
        _strict_keys(row, EXPECTATION_KEYS, context)
        if row["schema"] != EXPECTATION_SCHEMA:
            raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} schema differs")
        claim_id = _nonempty_string(row["claim_id"], f"{context} claim_id")
        if (
            CLAIM_ID.fullmatch(claim_id) is None
            or claim_id != EXPECTED_CLAIM_IDS[index - 1]
        ):
            raise V4EvidenceError(
                "V4_EXPECTATION_CLOSURE_INVALID", f"{context} id differs"
            )
        family = _nonempty_string(row["family"], f"{context} family")
        if family != EXPECTED_FAMILIES[index - 1]:
            raise V4EvidenceError(
                "V4_EXPECTATION_CLOSURE_INVALID",
                f"{claim_id} family differs: expected {EXPECTED_FAMILIES[index - 1]!r}",
            )
        title = _nonempty_string(row["title"], f"{context} title")
        if title in seen_titles:
            raise V4EvidenceError(
                "V4_EXPECTATION_CLOSURE_INVALID", "claim titles are duplicated"
            )
        seen_titles.add(title)
        basis = row["design_basis"]
        if not isinstance(basis, list) or not basis:
            raise V4EvidenceError(
                "V4_PROVENANCE_INVALID", f"{claim_id} has no design basis"
            )
        for basis_index, basis_value in enumerate(basis, 1):
            basis_row = _mapping(basis_value, f"{claim_id} design basis {basis_index}")
            _strict_keys(
                basis_row,
                {"path", "sections"},
                f"{claim_id} design basis {basis_index}",
            )
            path = _relative_path(
                basis_row["path"], f"{claim_id} design basis {basis_index} path"
            )
            if path not in provenance_paths:
                raise V4EvidenceError(
                    "V4_PROVENANCE_INVALID", f"{claim_id} cites unbound source {path}"
                )
            citations = _string_list(
                basis_row["sections"], f"{claim_id} design basis {basis_index} sections"
            )
            if not citations:
                raise V4EvidenceError(
                    "V4_PROVENANCE_INVALID", f"{claim_id} has empty sections"
                )
            available = section_cache.setdefault(path, _document_sections(root, path))
            for citation in citations:
                match = SECTION_CITATION.fullmatch(citation)
                if match is None or match.group(1) not in available:
                    raise V4EvidenceError(
                        "V4_PROVENANCE_INVALID",
                        f"{claim_id} cites unresolved section {citation!r} in {path}",
                    )
        controlled = _nonempty_object(
            row["controlled_input"], f"{claim_id} controlled_input"
        )
        _nonempty_string(
            controlled.get("case_id"), f"{claim_id} controlled_input case_id"
        )
        decoded = _nonempty_structured(
            row["expected_decoded"], f"{claim_id} expected_decoded"
        )
        if controlled == decoded:
            raise V4EvidenceError(
                "V4_EXPECTATION_NOT_DECODED",
                f"{claim_id} copies input as expected output",
            )
        independence = _mapping(row["independence"], f"{claim_id} independence")
        _strict_keys(
            independence,
            {"authored_from", "forbidden_sources"},
            f"{claim_id} independence",
        )
        if independence["authored_from"] != "accepted-design-and-authoritative-specs":
            raise V4EvidenceError(
                "V4_EXPECTATION_INDEPENDENCE_INVALID",
                f"{claim_id} was not authored from accepted authority",
            )
        forbidden = _string_list(
            independence["forbidden_sources"], f"{claim_id} forbidden_sources"
        )
        lowered = " ".join(forbidden).lower()
        if any(fragment not in lowered for fragment in FORBIDDEN_SOURCE_FRAGMENTS):
            raise V4EvidenceError(
                "V4_EXPECTATION_INDEPENDENCE_INVALID",
                f"{claim_id} does not close production/generated/v1/v3 sources",
            )
        fault = _mapping(
            row["discriminating_fault"], f"{claim_id} discriminating_fault"
        )
        _strict_keys(
            fault,
            {"mutation", "required_observation"},
            f"{claim_id} discriminating_fault",
        )
        _nonempty_structured(fault["mutation"], f"{claim_id} fault mutation")
        _nonempty_structured(
            fault["required_observation"], f"{claim_id} required observation"
        )
        fault_identity = _canonical_sha256(fault)
        if fault_identity in seen_faults:
            raise V4EvidenceError(
                "V4_EXPECTATION_CLOSURE_INVALID",
                f"{claim_id} reuses another claim's discriminating fault",
            )
        seen_faults.add(fault_identity)
        _validate_review(row["review"], f"{claim_id} review")
    return rows


def validate_fixtures(
    root: Path, expectations: Sequence[Mapping[str, Any]]
) -> list[dict[str, Any]]:
    rows = _load_jsonl(root / FIXTURES_PATH, "v4 semantic fixtures")
    if len(rows) != len(EXPECTED_FIXTURE_IDS):
        raise V4EvidenceError(
            "V4_FIXTURE_CLOSURE_INVALID",
            f"expected {len(EXPECTED_FIXTURE_IDS)} fixtures, observed {len(rows)}",
        )
    claims = {str(row["claim_id"]): row for row in expectations}
    kinds_by_claim: dict[str, set[str]] = {}
    for index, row in enumerate(rows, 1):
        context = f"fixture row {index}"
        _strict_keys(row, FIXTURE_KEYS, context)
        if row["schema"] != FIXTURE_SCHEMA:
            raise V4EvidenceError("V4_SCHEMA_INVALID", f"{context} schema differs")
        fixture_id = _nonempty_string(row["fixture_id"], f"{context} fixture_id")
        if (
            FIXTURE_ID.fullmatch(fixture_id) is None
            or fixture_id != EXPECTED_FIXTURE_IDS[index - 1]
        ):
            raise V4EvidenceError("V4_FIXTURE_CLOSURE_INVALID", f"{context} id differs")
        claim_id = _nonempty_string(row["claim_id"], f"{fixture_id} claim_id")
        match = FIXTURE_ID.fullmatch(fixture_id)
        assert match is not None
        if claim_id != f"RFV4-CLAIM-{match.group(1)}" or claim_id not in claims:
            raise V4EvidenceError(
                "V4_FIXTURE_CLOSURE_INVALID", f"{fixture_id} claim binding differs"
            )
        kind = _nonempty_string(row["fixture_kind"], f"{fixture_id} fixture_kind")
        expected_kind = "causal" if match.group(2) == "C" else "negative"
        if kind != expected_kind:
            raise V4EvidenceError(
                "V4_FIXTURE_CLOSURE_INVALID", f"{fixture_id} kind differs"
            )
        kinds_by_claim.setdefault(claim_id, set()).add(kind)
        fixture_input = _nonempty_object(
            row["fixture_input"], f"{fixture_id} fixture_input"
        )
        decoded = _nonempty_structured(
            row["expected_decoded"], f"{fixture_id} expected_decoded"
        )
        claim = claims[claim_id]
        controlled_input = _mapping(
            claim["controlled_input"], f"{claim_id} controlled_input"
        )
        base_case_id = _nonempty_string(
            fixture_input.get("base_case_id"), f"{fixture_id} base_case_id"
        )
        if base_case_id != controlled_input["case_id"]:
            raise V4EvidenceError(
                "V4_FIXTURE_NOT_DISCRIMINATING",
                f"{fixture_id} does not identify its controlled base case",
            )
        if kind == "causal":
            _strict_keys(
                fixture_input,
                {"base_case_id", "patch_semantics", "merge_patch"},
                f"{fixture_id} fixture_input",
            )
            if fixture_input["patch_semantics"] != "json_merge_patch_rfc7396":
                raise V4EvidenceError(
                    "V4_FIXTURE_NOT_DISCRIMINATING",
                    f"{fixture_id} has unknown patch semantics",
                )
            merge_patch = _nonempty_object(
                fixture_input["merge_patch"], f"{fixture_id} merge_patch"
            )
            _validate_closed_merge_patch(
                controlled_input, merge_patch, context=f"{fixture_id} merge_patch"
            )
            if (
                _apply_json_merge_patch(controlled_input, merge_patch)
                == controlled_input
            ):
                raise V4EvidenceError(
                    "V4_FIXTURE_NOT_DISCRIMINATING",
                    f"{fixture_id} merge patch does not change its controlled input",
                )
        else:
            _strict_keys(
                fixture_input,
                {"base_case_id", "invalid_change"},
                f"{fixture_id} fixture_input",
            )
            _nonempty_structured(
                fixture_input["invalid_change"], f"{fixture_id} invalid_change"
            )
        if decoded == claim["expected_decoded"]:
            raise V4EvidenceError(
                "V4_FIXTURE_NOT_DISCRIMINATING",
                f"{fixture_id} does not change observation",
            )
        distinguishes = _mapping(row["distinguishes"], f"{fixture_id} distinguishes")
        _strict_keys(
            distinguishes,
            {"mutation", "from_expected"},
            f"{fixture_id} distinguishes",
        )
        _nonempty_structured(distinguishes["mutation"], f"{fixture_id} mutation")
        _nonempty_structured(
            distinguishes["from_expected"], f"{fixture_id} from_expected"
        )
        if (
            kind == "causal"
            and distinguishes["mutation"] != claim["discriminating_fault"]["mutation"]
        ):
            raise V4EvidenceError(
                "V4_FIXTURE_NOT_DISCRIMINATING",
                f"{fixture_id} does not commit the claim's discriminating fault",
            )
        if (
            kind == "causal"
            and distinguishes["from_expected"]
            != claim["discriminating_fault"]["required_observation"]
        ):
            raise V4EvidenceError(
                "V4_FIXTURE_NOT_DISCRIMINATING",
                f"{fixture_id} does not bind the required fault observation",
            )
    expected_kinds = {"causal", "negative"}
    if any(
        kinds_by_claim.get(claim_id) != expected_kinds
        for claim_id in EXPECTED_CLAIM_IDS
    ):
        raise V4EvidenceError(
            "V4_FIXTURE_CLOSURE_INVALID",
            "every claim requires one causal and one negative fixture",
        )
    return rows


def _validate_artifact_digests(
    root: Path,
    issuance: Mapping[str, Any],
    expectations: Sequence[object],
    fixtures: Sequence[object],
) -> None:
    digests = _mapping(issuance["artifact_digests"], "artifact_digests")
    _strict_keys(
        digests,
        {"expectations_sha256", "negative_fixtures_sha256"},
        "artifact_digests",
    )
    expected_digests = {
        "expectations_sha256": _sha256(root / EXPECTATIONS_PATH),
        "negative_fixtures_sha256": _sha256(root / FIXTURES_PATH),
    }
    if dict(digests) != expected_digests:
        raise V4EvidenceError(
            "V4_ISSUANCE_DRIFT",
            "issued expectation or fixture bytes changed after freeze",
        )
    counts = _mapping(issuance["counts"], "counts")
    _strict_keys(counts, {"expectations", "negative_fixtures"}, "counts")
    if counts != {
        "expectations": len(expectations),
        "negative_fixtures": len(fixtures),
    }:
        raise V4EvidenceError("V4_ISSUANCE_DRIFT", "issued row counts differ")
    ranges = _mapping(issuance["id_ranges"], "id_ranges")
    _strict_keys(ranges, {"claims", "fixtures"}, "id_ranges")
    expected_ranges = {
        "claims": {"first": EXPECTED_CLAIM_IDS[0], "last": EXPECTED_CLAIM_IDS[-1]},
        "fixtures": {
            "first": EXPECTED_FIXTURE_IDS[0],
            "last": EXPECTED_FIXTURE_IDS[-1],
        },
    }
    if ranges != expected_ranges:
        raise V4EvidenceError("V4_ISSUANCE_DRIFT", "issued id ranges differ")


def _validate_authoring_constraints(value: object) -> str:
    constraints = _mapping(value, "authoring_constraints")
    expected_keys = {
        "author_identity",
        "target_execution_used",
        "predecessor_evidence_used",
        "production_expected_value_code_used",
        "imports_production_modules",
        "forbidden_inputs",
    }
    _strict_keys(constraints, expected_keys, "authoring_constraints")
    author = _identity(constraints["author_identity"], "author_identity")
    for key in (
        "target_execution_used",
        "predecessor_evidence_used",
        "production_expected_value_code_used",
        "imports_production_modules",
    ):
        if constraints[key] is not False:
            raise V4EvidenceError(
                "V4_EXPECTATION_INDEPENDENCE_INVALID",
                f"authoring constraint violated: {key}",
            )
    forbidden = _string_list(constraints["forbidden_inputs"], "forbidden_inputs")
    lowered = " ".join(forbidden).lower()
    if any(fragment not in lowered for fragment in FORBIDDEN_SOURCE_FRAGMENTS):
        raise V4EvidenceError(
            "V4_EXPECTATION_INDEPENDENCE_INVALID",
            "forbidden evidence inputs are incomplete",
        )
    return author


def _claim_review_bindings(
    expectations: Sequence[Mapping[str, Any]],
    fixtures: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, object]]:
    fixtures_by_claim: dict[str, dict[str, Mapping[str, Any]]] = {}
    for fixture in fixtures:
        fixtures_by_claim.setdefault(str(fixture["claim_id"]), {})[
            str(fixture["fixture_kind"])
        ] = fixture
    return {
        str(expectation["claim_id"]): {
            "expectation_sha256": _canonical_sha256(expectation),
            "fixture_sha256": {
                kind: _canonical_sha256(
                    fixtures_by_claim[str(expectation["claim_id"])][kind]
                )
                for kind in ("causal", "negative")
            },
        }
        for expectation in expectations
    }


def _validate_independent_review(
    value: object,
    *,
    issuance: Mapping[str, Any],
    author: str,
    expectations: Sequence[Mapping[str, Any]],
    fixtures: Sequence[Mapping[str, Any]],
    require_review: bool,
) -> str:
    review = _mapping(value, "independent_review")
    common_keys = {
        "status",
        "reviewer",
        "reviewed_at",
        "notes",
        "reviewed_artifact_digests",
        "reviewed_issuance_projection_sha256",
    }
    if review.get("status") == "pending-independent-review":
        _strict_keys(review, common_keys, "independent_review")
        notes = _string_list(review["notes"], "independent review notes")
        if issuance["status"] != "pending-independent-review" or any(
            review[key] is not None
            for key in (
                "reviewer",
                "reviewed_at",
                "reviewed_artifact_digests",
                "reviewed_issuance_projection_sha256",
            )
        ):
            raise V4EvidenceError("V4_REVIEW_INVALID", "pending review fields differ")
        if require_review:
            raise V4EvidenceError("V4_REVIEW_REQUIRED", "issuance review is pending")
        return "pending-independent-review"
    _strict_keys(review, common_keys | {"claim_reviews"}, "independent_review")
    notes = _string_list(review["notes"], "independent review notes")
    reviewer = review.get("reviewer")
    reviewed_at = review.get("reviewed_at")
    status = review.get("status")
    canonical_reviewer = _identity(reviewer, "independent reviewer")
    canonical_reviewed_at = _review_timestamp(reviewed_at, "reviewed_at")
    if (
        status not in {"accepted", "rejected", "not-accepted"}
        or issuance["status"] != status
        or canonical_reviewer.casefold() == author.casefold()
        or not notes
        or review["reviewed_artifact_digests"] != issuance["artifact_digests"]
        or review["reviewed_issuance_projection_sha256"]
        != _canonical_sha256(_issuance_review_projection(issuance))
    ):
        raise V4EvidenceError("V4_REVIEW_INVALID", "independent review is invalid")
    reviewer = canonical_reviewer
    reviewed_at = canonical_reviewed_at

    bindings = _claim_review_bindings(expectations, fixtures)
    claim_reviews = review["claim_reviews"]
    if not isinstance(claim_reviews, list) or len(claim_reviews) != len(bindings):
        raise V4EvidenceError("V4_REVIEW_INVALID", "claim review closure differs")
    dispositions: dict[str, str] = {}
    for index, row_value in enumerate(claim_reviews, 1):
        row = _mapping(row_value, f"claim review {index}")
        _strict_keys(
            row,
            {
                "claim_id",
                "expectation_sha256",
                "fixture_sha256",
                "disposition",
                "reviewer",
                "rationale",
            },
            f"claim review {index}",
        )
        claim_id = _nonempty_string(row["claim_id"], f"claim review {index} claim_id")
        if claim_id not in bindings or claim_id in dispositions:
            raise V4EvidenceError(
                "V4_REVIEW_INVALID",
                "claim review references unknown or duplicate claim",
            )
        disposition = _nonempty_string(
            row["disposition"], f"{claim_id} review disposition"
        )
        if disposition not in {"accepted", "rejected", "not-accepted"}:
            raise V4EvidenceError(
                "V4_REVIEW_INVALID", f"{claim_id} review disposition differs"
            )
        if (
            row["reviewer"] != reviewer
            or row["expectation_sha256"] != bindings[claim_id]["expectation_sha256"]
            or row["fixture_sha256"] != bindings[claim_id]["fixture_sha256"]
        ):
            raise V4EvidenceError(
                "V4_REVIEW_INVALID",
                f"{claim_id} review does not bind exact authored bytes",
            )
        rationale = _nonempty_string(row["rationale"], f"{claim_id} rationale")
        if claim_id not in rationale or str(row["expectation_sha256"]) not in rationale:
            raise V4EvidenceError(
                "V4_REVIEW_INVALID",
                f"{claim_id} review rationale is not claim-specific",
            )
        dispositions[claim_id] = disposition
    if set(dispositions) != set(bindings):
        raise V4EvidenceError("V4_REVIEW_INVALID", "claim review closure is incomplete")
    expected_status = (
        "accepted"
        if set(dispositions.values()) == {"accepted"}
        else "rejected"
        if "rejected" in dispositions.values()
        else "not-accepted"
    )
    if status != expected_status:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", "issuance status differs from per-claim dispositions"
        )
    if require_review and status != "accepted":
        raise V4EvidenceError(
            "V4_REVIEW_NOT_ACCEPTED", f"independent review disposition is {status}"
        )
    return status


def _selected_claims(
    selector: Mapping[str, Any],
    expectations: Sequence[Mapping[str, Any]],
    *,
    context: str,
) -> list[Mapping[str, Any]]:
    _strict_keys(selector, {"claim_ids", "families", "fixture_kinds"}, context)
    claim_ids = _string_list(selector["claim_ids"], f"{context} claim_ids")
    families = _string_list(selector["families"], f"{context} families")
    fixture_kinds = _string_list(selector["fixture_kinds"], f"{context} fixture_kinds")
    known_claims = set(EXPECTED_CLAIM_IDS)
    known_families = set(EXPECTED_FAMILIES)
    if set(claim_ids) - known_claims or set(families) - known_families:
        raise V4EvidenceError("V4_SELECTOR_INVALID", f"{context} names unknown rows")
    if set(fixture_kinds) - {"causal", "negative"}:
        raise V4EvidenceError("V4_SELECTOR_INVALID", f"{context} fixture kinds differ")
    if not claim_ids and not families:
        raise V4EvidenceError(
            "V4_SELECTOR_ZERO_SELECTION", f"{context} has no selector"
        )
    selected = [
        row
        for row in expectations
        if (not claim_ids or row["claim_id"] in claim_ids)
        and (not families or row["family"] in families)
    ]
    if not selected:
        raise V4EvidenceError(
            "V4_SELECTOR_ZERO_SELECTION", f"{context} selected zero claims"
        )
    return selected


def _validate_selectors(
    value: object,
    expectations: Sequence[Mapping[str, Any]],
    fixtures: Sequence[Mapping[str, Any]],
) -> dict[str, tuple[int, int]]:
    selectors = _mapping(value, "selectors")
    if set(selectors) != set(ORACLES):
        raise V4EvidenceError("V4_SELECTOR_INVALID", "oracle selector closure differs")
    counts: dict[str, tuple[int, int]] = {}
    for oracle in ORACLES:
        selector = _mapping(selectors[oracle], f"selector {oracle}")
        claims = _selected_claims(selector, expectations, context=f"selector {oracle}")
        kinds = set(selector["fixture_kinds"])
        claim_ids = {str(row["claim_id"]) for row in claims}
        selected_fixtures = [
            row
            for row in fixtures
            if row["claim_id"] in claim_ids
            and (not kinds or row["fixture_kind"] in kinds)
        ]
        if not selected_fixtures:
            raise V4EvidenceError(
                "V4_SELECTOR_ZERO_SELECTION",
                f"selector {oracle} selected zero fixtures",
            )
        if oracle == "negative-fixture-independence-check" and not any(
            row["fixture_kind"] == "negative" for row in selected_fixtures
        ):
            raise V4EvidenceError(
                "V4_SELECTOR_INVALID",
                "negative-fixture selector excludes negative fixtures",
            )
        if not all(row["discriminating_fault"]["mutation"] for row in claims):
            raise V4EvidenceError(
                "V4_SELECTOR_INVALID",
                f"selector {oracle} selected an uncommitted fault",
            )
        counts[oracle] = (len(claims), len(selected_fixtures))
    return counts


def validate_issuance(root: Path = ROOT, *, require_review: bool = True) -> V4Issuance:
    """Validate authority, rows, fixtures, freeze, selectors, and review closure."""

    root = root.resolve()
    plan_path, design_path = _active_plan(root)
    terminal_suite = discover_terminal_suite(root)
    issuance = _load_json(root / ISSUANCE_PATH, "v4 evidence issuance")
    _strict_keys(issuance, ISSUANCE_KEYS, "v4 evidence issuance")
    if issuance["schema"] != ISSUANCE_SCHEMA:
        raise V4EvidenceError("V4_SCHEMA_INVALID", "issuance schema differs")
    if issuance["suite_identity"] != SUITE_IDENTITY:
        raise V4EvidenceError("V4_AUTHORITY_INVALID", "issuance suite identity differs")
    if issuance["evidence_release"] != EVIDENCE_RELEASE:
        raise V4EvidenceError("V4_ISSUANCE_DRIFT", "evidence release identity differs")
    provenance_paths = _validate_source_provenance(
        root,
        issuance["source_provenance"],
        plan_path=plan_path,
        design_path=design_path,
        terminal_suite=terminal_suite,
    )
    author = _validate_authoring_constraints(issuance["authoring_constraints"])
    limitations = _string_list(issuance["limitations"], "limitations")
    if not limitations:
        raise V4EvidenceError("V4_SCHEMA_INVALID", "issuance limitations are absent")
    expectations = validate_expectations(root, provenance_paths)
    fixtures = validate_fixtures(root, expectations)
    _validate_artifact_digests(root, issuance, expectations, fixtures)
    _validate_selectors(issuance["selectors"], expectations, fixtures)
    _validate_independent_review(
        issuance["independent_review"],
        issuance=issuance,
        author=author,
        expectations=expectations,
        fixtures=fixtures,
        require_review=require_review,
    )
    return V4Issuance(tuple(expectations), tuple(fixtures), issuance, terminal_suite)


def _selector_counts(validated: V4Issuance, oracle: str) -> tuple[int, int]:
    selector = _mapping(validated.issuance["selectors"][oracle], f"selector {oracle}")
    claims = _selected_claims(
        selector, validated.expectations, context=f"selector {oracle}"
    )
    claim_ids = {str(row["claim_id"]) for row in claims}
    kinds = set(selector["fixture_kinds"])
    fixtures = [
        row
        for row in validated.fixtures
        if row["claim_id"] in claim_ids and (not kinds or row["fixture_kind"] in kinds)
    ]
    return len(claims), len(fixtures)


def validate_oracle(root: Path, oracle: str) -> dict[str, object]:
    """Run one final WP33 oracle and return nonzero selection/fault counts."""

    if oracle not in ORACLES:
        raise V4EvidenceError("V4_SELECTOR_INVALID", f"unknown oracle: {oracle}")
    try:
        validate_evidence_contracts(root.resolve())
    except V4ContractError as error:
        code = "V4_ISSUANCE_DRIFT" if error.code == "V4_R4_FREEZE_DRIFT" else error.code
        raise V4EvidenceError(code, str(error)) from error
    validated = validate_issuance(root, require_review=True)
    claims, fixtures = _selector_counts(validated, oracle)
    selector = _mapping(validated.issuance["selectors"][oracle], f"selector {oracle}")
    selected_claims = _selected_claims(
        selector, validated.expectations, context=f"selector {oracle}"
    )
    selected_ids = {str(row["claim_id"]) for row in selected_claims}
    fixture_kinds = set(selector["fixture_kinds"])
    selected_fixtures = [
        row
        for row in validated.fixtures
        if row["claim_id"] in selected_ids
        and (not fixture_kinds or row["fixture_kind"] in fixture_kinds)
    ]
    claim_by_id = {str(row["claim_id"]): row for row in selected_claims}
    committed_faults = []
    for fixture in selected_fixtures:
        claim_id = str(fixture["claim_id"])
        claim = claim_by_id[claim_id]
        fixture_input = _mapping(
            fixture["fixture_input"], f"{fixture['fixture_id']} input"
        )
        descriptor = {
            "claim_id": claim_id,
            "fixture_id": fixture["fixture_id"],
            "fixture_kind": fixture["fixture_kind"],
            "fault_sha256": _canonical_sha256(claim["discriminating_fault"]),
            "fixture_input_sha256": _canonical_sha256(fixture_input),
            "observation_sha256": _canonical_sha256(fixture["expected_decoded"]),
            "distinguishes_sha256": _canonical_sha256(fixture["distinguishes"]),
        }
        if fixture["fixture_kind"] == "causal":
            descriptor["applied_input_sha256"] = _canonical_sha256(
                _apply_json_merge_patch(
                    claim["controlled_input"], fixture_input["merge_patch"]
                )
            )
        committed_faults.append(descriptor)
    return {
        "oracle": oracle,
        "status": "accepted",
        "selected_claims": claims,
        "selected_fixtures": fixtures,
        "committed_discriminating_fault_count": len(committed_faults),
        "committed_discriminating_faults": committed_faults,
        "suite": SUITE_IDENTITY,
        "evidence_release": EVIDENCE_RELEASE,
    }


def review_candidate(
    root: Path,
    *,
    reviewer: str,
    reviewed_at: str,
    notes: Sequence[str],
    disposition: str,
) -> dict[str, Any]:
    """Return an issuance-only review transaction for exact authored bytes."""

    validated = validate_issuance(root, require_review=False)
    if validated.issuance["status"] != "pending-independent-review":
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", "only a pending issuance can be accepted"
        )
    reviewer = _identity(reviewer, "reviewer")
    reviewed_at = _review_timestamp(reviewed_at, "reviewed_at")
    notes_list = [_nonempty_string(note, "review note") for note in notes]
    if not notes_list:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", "independent review notes are required"
        )
    author = validated.issuance["authoring_constraints"]["author_identity"]
    if reviewer.casefold() == author.casefold():
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", "expectation author cannot review issuance"
        )

    if disposition not in {"accepted", "rejected", "not-accepted"}:
        raise V4EvidenceError(
            "V4_REVIEW_INVALID", f"unsupported review disposition: {disposition}"
        )
    bindings = _claim_review_bindings(validated.expectations, validated.fixtures)
    note_text = " ".join(notes_list)
    issuance = copy.deepcopy(validated.issuance)
    issuance["status"] = disposition
    issuance["independent_review"] = {
        "status": disposition,
        "reviewer": reviewer,
        "reviewed_at": reviewed_at,
        "notes": notes_list,
        "reviewed_artifact_digests": copy.deepcopy(issuance["artifact_digests"]),
        "reviewed_issuance_projection_sha256": _canonical_sha256(
            _issuance_review_projection(issuance)
        ),
        "claim_reviews": [
            {
                "claim_id": claim_id,
                "expectation_sha256": binding["expectation_sha256"],
                "fixture_sha256": binding["fixture_sha256"],
                "disposition": disposition,
                "reviewer": reviewer,
                "rationale": (
                    f"{claim_id} {disposition}; exact expectation "
                    f"{binding['expectation_sha256']}; {note_text}"
                ),
            }
            for claim_id, binding in bindings.items()
        ],
    }
    return issuance


def pending_acceptance_candidate(
    root: Path,
    *,
    reviewer: str,
    reviewed_at: str,
    notes: Sequence[str],
) -> dict[str, Any]:
    """Compatibility wrapper for an all-claims accepted review transaction."""

    return review_candidate(
        root,
        reviewer=reviewer,
        reviewed_at=reviewed_at,
        notes=notes,
        disposition="accepted",
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle", choices=ORACLES)
    parser.add_argument("--root", type=Path, default=ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        report = validate_oracle(arguments.root, arguments.oracle)
    except V4EvidenceError as error:
        print(
            json.dumps(
                {"status": "blocked", "code": error.code, "message": str(error)},
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
