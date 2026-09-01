"""Author and validate the immutable, successor-only WP40 release record.

The record binds an ancestral implementation evidence base; it never tries to name the commit that
contains itself. Live Just recipes, not embedded green flags, provide behavioral evidence, and WP42
retains final release authority. Performance is explicitly not claimed when no representative
workload exists, so the missing comparison is disclosed without becoming a release blocker. This
module rejects fake pass records, unreviewed WP38 transactions, and attempts to promote an
unmeasured performance result into a claim.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import rfc8785

from tooling.ci.production_evidence import (
    ProductionEvidenceError,
    validate_append_only_transaction,
)

ROOT = Path(__file__).resolve().parents[2]
RECORD_PATH = Path("contracts/acceptance/relational-fabric-v3/release-evidence.json")
JUSTFILE_PATH = Path("justfile")
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
HEAD = re.compile(r"[0-9a-f]{40}\Z")
RECIPE = re.compile(r"^([a-zA-Z0-9_-]+)(?:\s+[^:]*)?:.*$")

REQUIRED_DIMENSIONS = frozenset(
    {
        "provider",
        "transformation",
        "analysis",
        "query",
        "delta",
        "activation",
        "recovery",
        "authorization",
        "security",
        "resource",
        "lifecycle",
        "public_wire",
        "provenance",
        "legacy_absence",
        "independent_expectations",
    }
)
REQUIRED_REJECTIONS = frozenset(
    {
        "authorization",
        "credential",
        "network",
        "unsafe_source",
        "resource_exhaustion",
        "flow_control",
        "cancellation",
        "partial_or_corrupt_provider",
        "uncertain_delta_commit",
        "stale_fence",
        "split_activation_chain",
        "retention_guard",
        "adapter_protocol",
        "provenance_gap",
    }
)
REQUIRED_VARIATIONS = frozenset(
    {
        "clean",
        "incremental",
        "partition_layout",
        "batch_boundaries",
        "restart",
        "cdf_gap",
        "cache_state",
        "resource_pressure",
    }
)
CONDITIONAL_BLOCKER_POLICY: Mapping[str, tuple[str, str]] = {
    "WORKTREE-NOT-PROVING-COMMIT": ("blocking", "non_waivable"),
    "EXECUTION-STATE-UNAVAILABLE": ("blocking", "non_waivable"),
}
REQUIRED_PROFILE_LIMITATION_POLICY: Mapping[str, tuple[str, str]] = {
    "UNTRUSTED-PROVIDER-PROFILE-UNAVAILABLE": (
        "profile_limited",
        "non_waivable",
    ),
}
PERFORMANCE_DISCLOSURE_ID = "PERFORMANCE-EVIDENCE-NOT-CLAIMED"
LEGACY_PERFORMANCE_BLOCKER_ID = "PERFORMANCE-BASELINE-UNAVAILABLE"
PERFORMANCE_DISCLOSURE = {
    "limitation_id": PERFORMANCE_DISCLOSURE_ID,
    "severity": "informational",
    "waivability": "not_applicable",
    "detail": (
        "No representative production workload or regression baseline is accepted; "
        "WP40 makes no performance or regression claim."
    ),
}
REQUIRED_UNTRUSTED_CAPABILITIES = frozenset(
    {
        "compiled-seccomp-policy-authorized",
        "credential-read-denied",
        "inherited-fd-read-denied",
        "live-workspace-read-denied",
        "network-denied",
        "seccomp-active",
    }
)
REQUIRED_FROZEN_INPUT_ROLES: tuple[tuple[str, str], ...] = (
    ("Cargo.lock", "exact Rust dependency graph"),
    (
        "codefabric-cpg-mcp/uv.lock",
        "exact Python presentation dependency graph",
    ),
    ("rustc-extractor/Cargo.lock", "exact dated-nightly extractor dependency graph"),
    (
        "pyrefly-sidecar/Cargo.lock",
        "exact pinned Pyrefly sidecar dependency graph",
    ),
    (
        (
            "docs/authoritative_design/"
            "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.1.md"
        ),
        "current authoritative suite identity and governance",
    ),
    (
        (
            "docs/plans/"
            "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
        ),
        "current successor implementation plan",
    ),
    (
        "contracts/acceptance/relational-fabric-v3/evidence-issuance.json",
        "independent expectation issuance",
    ),
    (
        "contracts/acceptance/relational-fabric-v3/expectations.jsonl",
        "independently decoded successor expectations",
    ),
    (
        "contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl",
        "independent causal and negative fixtures",
    ),
    (
        (
            "contracts/acceptance/relational-fabric-v3/"
            "production-evidence-transaction.jsonl"
        ),
        "independently reviewed WP38 production evidence transaction",
    ),
    (
        "contracts/rpc/cpg_query_service.proto",
        "released lifecycle and query wire contract",
    ),
    (
        "tooling/proto/production-descriptor.pb",
        "hermetic production Protobuf descriptor",
    ),
)
REQUIRED_FROZEN_INPUTS = frozenset(path for path, _ in REQUIRED_FROZEN_INPUT_ROLES)
FORBIDDEN_RECIPE_FRAGMENTS = (
    "bootstrap",
    "comparator",
    "model-",
    "ontology",
    "predecessor",
)
FAKE_PASS_STATES = frozenset({"green", "pass", "passed", "success", "succeeded"})


class ReleaseEvidenceError(ValueError):
    """The release record is incomplete, mutable, or overclaims its evidence."""


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ReleaseEvidenceError(f"duplicate JSON member {key!r}")
        value[key] = item
    return value


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseEvidenceError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise ReleaseEvidenceError(f"{path} must contain one JSON object")
    return value


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _content_id(record: Mapping[str, Any]) -> str:
    """Return an integrity identity over RFC 8785 bytes, never semantic proof."""

    projection = {key: value for key, value in record.items() if key != "content_id"}
    return f"sha256:{hashlib.sha256(rfc8785.dumps(projection)).hexdigest()}"


def _items(value: Any, field: str) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        raise ReleaseEvidenceError(f"{field} must be a list of objects")
    return value


def _strings(value: Any, field: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item for item in value)
    ):
        raise ReleaseEvidenceError(f"{field} must be a non-empty string list")
    return value


def _just_recipes(root: Path) -> frozenset[str]:
    recipes = {
        match.group(1)
        for line in (root / JUSTFILE_PATH).read_text(encoding="utf-8").splitlines()
        if (match := RECIPE.fullmatch(line)) is not None
    }
    if not recipes:
        raise ReleaseEvidenceError("Just recipe inventory is empty")
    return frozenset(recipes)


def _validate_live_recipes(record: Mapping[str, Any], root: Path) -> None:
    recipes = _just_recipes(root)
    referenced: set[str] = set()
    for item in _items(record.get("matrix"), "matrix"):
        referenced.update(_strings(item.get("live_recipes"), "matrix.live_recipes"))
    for item in _items(record.get("rejection_matrix"), "rejection_matrix"):
        referenced.update(
            _strings(item.get("live_recipes"), "rejection_matrix.live_recipes")
        )
    for item in _items(record.get("operations_matrix"), "operations_matrix"):
        referenced.update(
            _strings(item.get("live_recipes"), "operations_matrix.live_recipes")
        )
    absent = sorted(referenced - recipes)
    if absent:
        raise ReleaseEvidenceError(
            f"release record references absent live recipes: {absent}"
        )
    forbidden = sorted(
        recipe
        for recipe in referenced
        if any(fragment in recipe for fragment in FORBIDDEN_RECIPE_FRAGMENTS)
    )
    if forbidden:
        raise ReleaseEvidenceError(
            f"retired authority recipe entered release evidence: {forbidden}"
        )


def _validate_selected_tests(record: Mapping[str, Any], root: Path) -> None:
    inventory: set[str] = set()
    for source_root, suffix in (
        (root / "src", "*.rs"),
        (root / "tests", "*.rs"),
        (root / "rustc-extractor" / "src", "*.rs"),
        (root / "pyrefly-sidecar" / "src", "*.rs"),
        (root / "codefabric-cpg-mcp" / "tests", "*.py"),
        (root / "tooling" / "ci", "test_*.py"),
    ):
        if not source_root.is_dir():
            continue
        for path in source_root.rglob(suffix):
            inventory.update(
                re.findall(
                    r"^\s*(?:async\s+)?(?:fn|def)\s+([A-Za-z0-9_]+)",
                    path.read_text(encoding="utf-8"),
                    re.MULTILINE,
                )
            )
    selected: set[str] = set()
    for field in ("matrix", "rejection_matrix", "operations_matrix"):
        for item in _items(record.get(field), field):
            selected.update(
                _strings(item.get("selected_tests"), f"{field}.selected_tests")
            )
    absent = sorted(selected - inventory)
    if absent:
        raise ReleaseEvidenceError(
            f"release record names non-existent selected tests: {absent}"
        )


def _validate_provenance(record: Mapping[str, Any]) -> None:
    graph = record.get("provenance_graph")
    if not isinstance(graph, dict):
        raise ReleaseEvidenceError("provenance_graph must be an object")
    root_node = graph.get("served_row_root")
    source_node = graph.get("exact_source_terminal")
    edges = _items(graph.get("edges"), "provenance_graph.edges")
    adjacency: dict[str, set[str]] = {}
    for edge in edges:
        subject = edge.get("subject")
        dependency = edge.get("depends_on")
        if (
            not isinstance(subject, str)
            or not subject
            or not isinstance(dependency, str)
            or not dependency
        ):
            raise ReleaseEvidenceError(
                "provenance edges require explicit subject and depends_on"
            )
        adjacency.setdefault(subject, set()).add(dependency)
        adjacency.setdefault(dependency, set())
    if root_node not in adjacency or source_node not in adjacency:
        raise ReleaseEvidenceError("provenance terminal identities are absent")

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visiting:
            raise ReleaseEvidenceError("provenance graph contains a cycle")
        if node in visited:
            return
        visiting.add(node)
        for dependency in adjacency[node]:
            visit(dependency)
        visiting.remove(node)
        visited.add(node)

    visit(str(root_node))
    if source_node not in visited:
        raise ReleaseEvidenceError(
            "served-row provenance does not reach the exact source terminal"
        )


def _validate_reviewed_wp38_transaction(root: Path) -> None:
    """Require the exact frozen WP38 input to carry an independent accepted review."""

    try:
        validate_append_only_transaction(root, require_review=True)
    except ProductionEvidenceError as error:
        raise ReleaseEvidenceError(
            f"WP38 production evidence is not independently reviewed: {error}"
        ) from error


def _validate_limitation_policy(record: Mapping[str, Any]) -> None:
    limitations = _items(record.get("limitations"), "limitations")
    by_id: dict[str, dict[str, Any]] = {}
    for limitation in limitations:
        limitation_id = limitation.get("limitation_id")
        if not isinstance(limitation_id, str) or not limitation_id:
            raise ReleaseEvidenceError("limitation identity is malformed")
        if limitation_id in by_id:
            raise ReleaseEvidenceError("limitation identities are duplicated")
        by_id[limitation_id] = limitation

    if LEGACY_PERFORMANCE_BLOCKER_ID in by_id:
        raise ReleaseEvidenceError(
            "the retired performance-baseline blocker was reintroduced"
        )
    for limitation_id, (
        severity,
        waivability,
    ) in REQUIRED_PROFILE_LIMITATION_POLICY.items():
        limitation = by_id.get(limitation_id)
        if limitation is None or (
            limitation.get("severity"),
            limitation.get("waivability"),
        ) != (severity, waivability):
            raise ReleaseEvidenceError(
                f"required fail-closed limitation differs: {limitation_id}"
            )

    proving = record.get("proving_state")
    if not isinstance(proving, dict):
        raise ReleaseEvidenceError("proving_state must be an object")
    expected_conditional = {
        "WORKTREE-NOT-PROVING-COMMIT": proving.get("worktree_state")
        != "clean_evidence_base",
        "EXECUTION-STATE-UNAVAILABLE": proving.get("release_certification")
        == "blocked",
    }
    for limitation_id, required in expected_conditional.items():
        limitation = by_id.get(limitation_id)
        if required:
            severity, waivability = CONDITIONAL_BLOCKER_POLICY[limitation_id]
            if limitation is None or (
                limitation.get("severity"),
                limitation.get("waivability"),
            ) != (severity, waivability):
                raise ReleaseEvidenceError(
                    f"required fail-closed limitation differs: {limitation_id}"
                )
        elif limitation is not None:
            raise ReleaseEvidenceError(
                f"resolved limitation remains in the release record: {limitation_id}"
            )

    performance_disclosure = by_id.get(PERFORMANCE_DISCLOSURE_ID)
    if performance_disclosure != PERFORMANCE_DISCLOSURE:
        raise ReleaseEvidenceError(
            "the non-blocking performance non-claim disclosure differs"
        )


def _apply_current_performance_policy(record: dict[str, Any]) -> None:
    """Migrate the stale baseline blocker to an explicit non-claim disclosure."""

    limitations = _items(record.get("limitations"), "limitations")
    migrated: list[dict[str, Any]] = []
    inserted = False
    for limitation in limitations:
        limitation_id = limitation.get("limitation_id")
        if limitation_id in {
            LEGACY_PERFORMANCE_BLOCKER_ID,
            PERFORMANCE_DISCLOSURE_ID,
        }:
            if not inserted:
                migrated.append(dict(PERFORMANCE_DISCLOSURE))
                inserted = True
            continue
        migrated.append(dict(limitation))
    if not inserted:
        migrated.append(dict(PERFORMANCE_DISCLOSURE))
    record["limitations"] = migrated

    performance = record.get("performance")
    if not isinstance(performance, dict):
        raise ReleaseEvidenceError("performance must be an object")
    record["performance"] = {
        "claim_status": "not_claimed",
        "benchmark_comparison": "not_performed",
        "supported_kernel_metrics": performance.get("supported_kernel_metrics"),
        "observation": performance.get("observation"),
        "uncertainty": performance.get("uncertainty"),
    }


def _git(
    root: Path, *args: str, check: bool = True
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ("git", *args),
        cwd=root,
        check=check,
        capture_output=True,
        text=True,
    )


def _candidate_state_ready(root: Path) -> bool:
    state_path = root / (
        "docs/plans/state/"
        "codefabric-execution-proved-relational-data-fabric_v3_state.json"
    )
    if not state_path.is_file():
        return False
    try:
        state = _load_json(state_path)
    except (OSError, ReleaseEvidenceError):
        return False
    packets = state.get("packets")
    milestones = state.get("milestones")
    batches = state.get("decommission_batches")
    if not all(isinstance(group, dict) for group in (packets, milestones, batches)):
        return False
    assert isinstance(packets, dict)
    assert isinstance(milestones, dict)
    assert isinstance(batches, dict)
    completed = (
        all(
            isinstance(packets.get(f"WP{number:02d}"), dict)
            and packets[f"WP{number:02d}"].get("status") == "complete"
            for number in range(29, 42)
        )
        and all(
            isinstance(milestones.get(f"M{number:02d}"), dict)
            and milestones[f"M{number:02d}"].get("status") == "complete"
            for number in range(2, 6)
        )
        and all(
            isinstance(batches.get(f"DB{number:02d}"), dict)
            and batches[f"DB{number:02d}"].get("status") == "complete"
            for number in range(9, 14)
        )
    )
    pending_final = all(
        isinstance(group.get(identifier), dict)
        and group[identifier].get("status") in {"in_progress", "complete"}
        for group, identifier in (
            (packets, "WP42"),
            (milestones, "M06"),
            (batches, "DB14"),
        )
    )
    return completed and pending_final


def _refresh_proving_state(record: dict[str, Any], root: Path) -> None:
    """Bind WP40 to a real ancestral evidence base without a self-referential HEAD."""

    if not (root / ".git").exists():
        return
    head = _git(root, "rev-parse", "HEAD").stdout.strip()
    clean = not _git(root, "status", "--porcelain=v1", "--untracked-files=all").stdout
    state_ready = _candidate_state_ready(root)
    record["proving_state"] = {
        "base_head": head,
        "head_role": ("ancestral_evidence_base" if clean else "development_base_only"),
        "worktree_state": (
            "clean_evidence_base" if clean else "dirty_concurrent_implementation"
        ),
        "release_certification": (
            "deferred_to_wp42" if clean and state_ready else "blocked"
        ),
        "reason": (
            "WP40 is frozen at an ancestral clean evidence base; final release authority belongs "
            "to WP42 and its independently reviewed certification."
            if clean and state_ready
            else "WP40 remains development evidence until a clean ancestral implementation base "
            "and candidate-ready execution state exist."
        ),
    }
    limitations = _items(record.get("limitations"), "limitations")
    retained = [
        dict(item)
        for item in limitations
        if item.get("limitation_id")
        not in {"WORKTREE-NOT-PROVING-COMMIT", "EXECUTION-STATE-UNAVAILABLE"}
    ]
    existing = {
        str(item.get("limitation_id"))
        for item in limitations
        if isinstance(item.get("limitation_id"), str)
    }
    for limitation_id, required in (
        ("WORKTREE-NOT-PROVING-COMMIT", not clean),
        ("EXECUTION-STATE-UNAVAILABLE", not state_ready),
    ):
        if required:
            source = next(
                (
                    item
                    for item in limitations
                    if item.get("limitation_id") == limitation_id
                ),
                None,
            )
            if source is None or limitation_id not in existing:
                raise ReleaseEvidenceError(
                    f"cannot refresh absent fail-closed limitation: {limitation_id}"
                )
            retained.append(dict(source))
    record["limitations"] = retained


def _refreshed_frozen_inputs(root: Path) -> list[dict[str, str]]:
    frozen: list[dict[str, str]] = []
    for path, role in REQUIRED_FROZEN_INPUT_ROLES:
        candidate = root / path
        if not candidate.is_file():
            raise ReleaseEvidenceError(f"required frozen input is absent: {path}")
        frozen.append({"path": path, "sha256": _sha256(candidate), "role": role})
    return frozen


def _validate_record_integrity(record: Mapping[str, Any], root: Path) -> int:
    """Validate immutable identities and an ancestral WP40 evidence base."""

    expected_root = {
        "schema_version",
        "record_id",
        "content_id",
        "evidence_class",
        "suite",
        "created_at",
        "proving_state",
        "environment",
        "frozen_inputs",
        "matrix",
        "rejection_matrix",
        "operations_matrix",
        "development_observations",
        "provenance_graph",
        "performance",
        "limitations",
    }
    if set(record) != expected_root:
        raise ReleaseEvidenceError(
            "release record root fields differ from the closed schema"
        )
    if (
        record.get("schema_version") != 1
        or record.get("evidence_class") != "development"
    ):
        raise ReleaseEvidenceError("WP40 record must remain development evidence")
    if record.get("content_id") != _content_id(record):
        raise ReleaseEvidenceError("release record immutable content identity differs")

    proving = record.get("proving_state")
    if not isinstance(proving, dict):
        raise ReleaseEvidenceError("proving_state must be an object")
    if not HEAD.fullmatch(str(proving.get("base_head", ""))):
        raise ReleaseEvidenceError("development base HEAD is invalid")
    posture = (
        proving.get("head_role"),
        proving.get("worktree_state"),
        proving.get("release_certification"),
    )
    if posture not in {
        ("development_base_only", "dirty_concurrent_implementation", "blocked"),
        ("ancestral_evidence_base", "clean_evidence_base", "blocked"),
        (
            "ancestral_evidence_base",
            "clean_evidence_base",
            "deferred_to_wp42",
        ),
    }:
        raise ReleaseEvidenceError("WP40 proving-state posture is invalid")
    if (root / ".git").exists():
        current_head = _git(root, "rev-parse", "HEAD").stdout.strip()
        exists = _git(
            root,
            "cat-file",
            "-e",
            f"{proving['base_head']}^{{commit}}",
            check=False,
        )
        ancestor = _git(
            root,
            "merge-base",
            "--is-ancestor",
            str(proving["base_head"]),
            current_head,
            check=False,
        )
        if exists.returncode != 0 or ancestor.returncode != 0:
            raise ReleaseEvidenceError(
                "recorded WP40 evidence base is not an ancestral commit"
            )

    frozen_inputs = _items(record.get("frozen_inputs"), "frozen_inputs")
    paths = [item.get("path") for item in frozen_inputs]
    if len(paths) != len(set(paths)) or set(paths) != REQUIRED_FROZEN_INPUTS:
        raise ReleaseEvidenceError("frozen release input set differs")
    expected_roles = dict(REQUIRED_FROZEN_INPUT_ROLES)
    for item in frozen_inputs:
        path = item.get("path")
        digest = item.get("sha256")
        if (
            not isinstance(path, str)
            or not isinstance(digest, str)
            or not SHA256.fullmatch(f"sha256:{digest}")
        ):
            raise ReleaseEvidenceError("frozen input identity is malformed")
        if item.get("role") != expected_roles[path]:
            raise ReleaseEvidenceError(f"frozen input role differs: {path}")
        candidate = root / path
        if not candidate.is_file() or _sha256(candidate) != digest:
            raise ReleaseEvidenceError(f"frozen input digest differs: {path}")
    _validate_reviewed_wp38_transaction(root)

    observations = _items(
        record.get("development_observations"), "development_observations"
    )
    for observation in observations:
        if any(
            isinstance(value, str) and value.lower() in FAKE_PASS_STATES
            for key, value in observation.items()
            if key in {"outcome", "result", "status"}
        ):
            raise ReleaseEvidenceError(
                "embedded pass records cannot replace live recipe evidence"
            )
        if observation.get("certification_role") != "development_only":
            raise ReleaseEvidenceError(
                "development observation overstates certification role"
            )

    _validate_limitation_policy(record)

    _validate_live_recipes(record, root)
    _validate_selected_tests(record, root)
    _validate_provenance(record)
    return len(frozen_inputs)


def validate_record_integrity(root: Path = ROOT) -> int:
    """Validate the repository's current WP40 release-evidence record."""

    return _validate_record_integrity(_load_json(root / RECORD_PATH), root)


def build_refreshed_record(
    root: Path = ROOT, *, source: Path = RECORD_PATH
) -> dict[str, Any]:
    """Derive a candidate record from exact inputs after WP38 independent review."""

    source_path = source if source.is_absolute() else root / source
    record = _load_json(source_path)
    _apply_current_performance_policy(record)
    _refresh_proving_state(record, root)
    record["frozen_inputs"] = _refreshed_frozen_inputs(root)
    record["content_id"] = _content_id(record)
    _validate_record_integrity(record, root)
    _validate_matrix_v3_record(record)
    _validate_security_resource_rejection_record(record)
    _validate_clean_incremental_recovery_performance_record(record)
    return record


def _write_record_atomic(path: Path, record: Mapping[str, Any]) -> None:
    data = (json.dumps(record, indent=2, ensure_ascii=False) + "\n").encode("utf-8")
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def refresh_release_record(
    root: Path = ROOT,
    *,
    source: Path = RECORD_PATH,
    output: Path,
) -> int:
    """Atomically write one validated candidate; an unreviewed WP38 input writes nothing."""

    record = build_refreshed_record(root, source=source)
    output_path = output if output.is_absolute() else root / output
    _write_record_atomic(output_path, record)
    return len(REQUIRED_FROZEN_INPUTS)


def _validate_matrix_v3_record(record: Mapping[str, Any]) -> int:
    matrix = _items(record["matrix"], "matrix")
    dimensions = {item.get("dimension") for item in matrix}
    if dimensions != REQUIRED_DIMENSIONS or len(matrix) != len(dimensions):
        raise ReleaseEvidenceError("post-purge release matrix dimensions differ")
    for item in matrix:
        _strings(item.get("selected_tests"), "matrix.selected_tests")
        _strings(item.get("exact_inputs"), "matrix.exact_inputs")
        _strings(item.get("expected_outputs"), "matrix.expected_outputs")
        _strings(item.get("provenance_nodes"), "matrix.provenance_nodes")
        if item.get("observation_mode") != "live_gate_execution":
            raise ReleaseEvidenceError("release matrix contains a non-live observation")
    return len(matrix)


def validate_matrix_v3(root: Path = ROOT) -> int:
    """Validate complete successor behavior dimensions and independent frozen expectations."""

    validate_record_integrity(root)
    return _validate_matrix_v3_record(_load_json(root / RECORD_PATH))


def _validate_security_resource_rejection_record(record: Mapping[str, Any]) -> int:
    matrix = _items(record["rejection_matrix"], "rejection_matrix")
    classes = {item.get("fault_class") for item in matrix}
    if classes != REQUIRED_REJECTIONS or len(matrix) != len(classes):
        raise ReleaseEvidenceError("security/resource rejection matrix differs")
    for item in matrix:
        if item.get("required_terminal") != "rejected_without_publication":
            raise ReleaseEvidenceError(
                "negative evidence permits a non-rejection terminal"
            )
        _strings(item.get("selected_tests"), "rejection_matrix.selected_tests")

    environment = record.get("environment")
    if not isinstance(environment, dict):
        raise ReleaseEvidenceError("environment must be an object")
    containment = environment.get("host_containment")
    if not isinstance(containment, dict):
        raise ReleaseEvidenceError("host containment observation is absent")
    if containment.get("untrusted_execution") != "unavailable":
        raise ReleaseEvidenceError("unsupported host cannot claim untrusted execution")
    launcher = containment.get("launcher")
    if not isinstance(launcher, dict):
        raise ReleaseEvidenceError("host launcher observation is absent")
    if set(launcher) != {
        "kind",
        "path",
        "observed_version",
        "version_role",
        "observed_root_owned",
        "observed_mode",
        "observed_setuid",
    }:
        raise ReleaseEvidenceError("host launcher diagnostic fields differ")
    for field in ("kind", "path", "observed_version"):
        value = launcher.get(field)
        if not isinstance(value, str) or not value:
            raise ReleaseEvidenceError(
                f"host launcher diagnostic {field} must be a non-empty string"
            )
    if (
        launcher.get("version_role") != "diagnostic_only"
        or not isinstance(launcher.get("observed_root_owned"), bool)
        or not isinstance(launcher.get("observed_mode"), str)
        or re.fullmatch(r"[0-7]{4}", launcher["observed_mode"]) is None
        or not isinstance(launcher.get("observed_setuid"), bool)
    ):
        raise ReleaseEvidenceError("host launcher diagnostics are malformed")
    if containment.get("admission_authority") != "capability_matrix":
        raise ReleaseEvidenceError(
            "launcher identity cannot replace executable capability proof"
        )
    unmet = set(
        _strings(
            containment.get("unmet_requirements"),
            "environment.host_containment.unmet_requirements",
        )
    )
    if not REQUIRED_UNTRUSTED_CAPABILITIES <= unmet:
        raise ReleaseEvidenceError(
            "untrusted profile lacks the declared capability remainders"
        )
    if containment.get("trusted_local") != "authorization_and_receipt_required":
        raise ReleaseEvidenceError(
            "trusted-local mode cannot substitute for containment"
        )
    return len(matrix)


def validate_security_resource_rejection(root: Path = ROOT) -> int:
    """Validate the closed negative matrix and capability-derived host profile."""

    validate_record_integrity(root)
    return _validate_security_resource_rejection_record(_load_json(root / RECORD_PATH))


def _validate_clean_incremental_recovery_performance_record(
    record: Mapping[str, Any],
) -> int:
    matrix = _items(record["operations_matrix"], "operations_matrix")
    variations = {item.get("variation") for item in matrix}
    if variations != REQUIRED_VARIATIONS or len(matrix) != len(variations):
        raise ReleaseEvidenceError(
            "clean/incremental/recovery variation matrix differs"
        )
    for item in matrix:
        _strings(item.get("selected_tests"), "operations_matrix.selected_tests")
        if item.get("comparison") != "typed_successor_outputs_only":
            raise ReleaseEvidenceError(
                "operational evidence attempts a retired comparison"
            )

    performance = record.get("performance")
    if not isinstance(performance, dict):
        raise ReleaseEvidenceError("performance must be an object")
    if set(performance) != {
        "claim_status",
        "benchmark_comparison",
        "supported_kernel_metrics",
        "observation",
        "uncertainty",
    }:
        raise ReleaseEvidenceError("performance non-claim fields differ")
    if performance.get("claim_status") != "not_claimed":
        raise ReleaseEvidenceError(
            "unmeasured performance cannot become a passing claim"
        )
    if performance.get("benchmark_comparison") != "not_performed":
        raise ReleaseEvidenceError("an unperformed benchmark comparison was promoted")
    metrics = performance.get("supported_kernel_metrics")
    if set(_strings(metrics, "performance.supported_kernel_metrics")) != {
        "cpu",
        "memory",
        "pids",
        "swap",
    }:
        raise ReleaseEvidenceError("supported cgroup metric set differs")
    for field in ("observation", "uncertainty"):
        value = performance.get(field)
        if not isinstance(value, str) or not value:
            raise ReleaseEvidenceError(f"performance {field} must be disclosed")
    return len(matrix)


def validate_clean_incremental_recovery_performance(root: Path = ROOT) -> int:
    """Validate operational variations without inventing a performance claim."""

    validate_record_integrity(root)
    return _validate_clean_incremental_recovery_performance_record(
        _load_json(root / RECORD_PATH)
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=(
            "record-integrity",
            "matrix-v3",
            "security-resource-rejection",
            "clean-incremental-recovery-performance",
            "refresh",
        ),
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=RECORD_PATH,
        help="existing record template, relative to the repository root by default",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="required refresh destination; validation commands ignore this option",
    )
    args = parser.parse_args()
    validators = {
        "record-integrity": validate_record_integrity,
        "matrix-v3": validate_matrix_v3,
        "security-resource-rejection": validate_security_resource_rejection,
        "clean-incremental-recovery-performance": validate_clean_incremental_recovery_performance,
    }
    try:
        if args.command == "refresh":
            if args.output is None:
                raise ReleaseEvidenceError("refresh requires an explicit --output")
            count = refresh_release_record(
                ROOT,
                source=args.source,
                output=args.output,
            )
        else:
            count = validators[args.command](ROOT)
    except (OSError, ReleaseEvidenceError, subprocess.CalledProcessError) as error:
        print(f"relational-fabric-release: {error}", file=sys.stderr)
        return 1
    print(f"relational-fabric-release {args.command}: {count} closed entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
