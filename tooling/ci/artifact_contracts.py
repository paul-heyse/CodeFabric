"""Validate planning artifacts and derive execution trust from repository truth.

The committed artifacts retain human judgment.  This module recomputes file
identity, Git ancestry, recipe availability, and tracked-output zero state so
those derived facts never need to be copied into an execution-state ledger.
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
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
ACTIVE_PLAN_POINTER = Path("docs/plans/active-plan.json")


def load_just_recipes(root: Path = ROOT) -> dict[str, Any]:
    """Read the live recipe graph directly from Just's structured interface."""
    completed = subprocess.run(
        ("just", "--dump", "--dump-format", "json"),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    dump = json.loads(completed.stdout)
    recipes = dump.get("recipes")
    if not isinstance(recipes, dict):
        raise ArtifactContractError("just JSON did not contain a recipe map")
    return recipes


PLAN_REQUIRED_KEYS = {
    "artifact",
    "plan_id",
    "version",
    "date",
    "status",
    "design_path",
    "design_version",
    "baseline_commit",
    "state_path",
    "cutover",
}
STATE_KEYS = {
    "schema_version",
    "plan_path",
    "design_path",
    "baseline_commit",
    "status",
    "current_packet",
    "packets",
    "milestones",
    "decommission_batches",
    "baseline_failures",
    "discovered_obligations",
    "plan_deviations",
    "next_action",
    "updated_at",
}
ENTRY_KEYS = {
    "status",
    "proving_commit",
    "deviations",
    "failed_approaches",
    "blockers",
}
ENTRY_STATUSES = {
    "not_started",
    "ready",
    "in_progress",
    "blocked",
    "complete",
    "stale",
    "invalidated",
}
OVERALL_STATUSES = {
    "not_started",
    "executing",
    "blocked",
    "complete",
    "superseded",
    "invalidated",
}
DERIVED_STATE_KEYS = {
    "acceptance",
    "changed_files",
    "check_results",
    "checks",
    "current_head",
    "design_input_digests",
    "evidence",
    "evidence_refs",
    "exit_code",
    "expected_change_surface",
    "output",
    "plan_digest",
    "state_digest",
    "working_tree_digest",
}
REVIEW_REQUIREMENTS = {
    "plan-audit": {"plan_path", "verdict"},
    "implementation-review": {"plan_path", "verdict"},
    "implementation-status": {"plan_path", "state_path"},
    "library-capability-research": {"topic"},
    "lib-leverage": {"library"},
    "skill-eval": set(),
    "design-principles-conformance": {
        "principles_path",
        "principles_digest",
        "baseline_commit",
        "verdict",
    },
    "design-principles-remediation-proposal": {
        "principles_path",
        "principles_digest",
        "conformance_review_path",
        "conformance_review_digest",
        "baseline_commit",
    },
}
REVIEW_VERDICTS = {
    "plan-audit": {
        "ready",
        "ready-with-corrections",
        "needs-revision",
        "needs-redesign",
    },
    "implementation-review": {
        "approved",
        "approved-with-minor-findings",
        "changes-required",
        "design-invalidated",
    },
    "design-principles-conformance": {
        "conformant",
        "conformant-with-findings",
        "divergent",
        "framework-unowned",
    },
}
REVIEW_ARTIFACT_ROW = re.compile(r"^\|\s*`([^`]+)`\s*\|", re.MULTILINE)
TARGET_PATH = re.compile(rb"(?:^|/)target(?:/|$)")
STABLE_HEADING_ID = re.compile(
    r"^#{2,6}\s+(?P<id>WP\d+[a-z]?|M\d+|DB\d+|D-\d+|I-\d+|"
    r"L-\d+|LD-\d+|F-\d+|IR-\d+)\b",
    re.MULTILINE,
)
PLAN_HEADING = re.compile(
    r"^###\s+(?P<id>WP\d+[a-z]?|M\d+|DB\d+)\s+—",
    re.MULTILINE,
)
INPUT_ROW = re.compile(
    r"^\|\s*(?P<path>[^|`][^|]*?)\s*\|\s*(?P<digest>[0-9a-f]{64})\s*\|$"
)
ORACLE = re.compile(r"Executable oracle:\s*`([^`]+)`")
JUST_RECIPE = re.compile(r"\bjust\s+([a-z][a-z0-9-]*)")


class ArtifactContractError(ValueError):
    """An artifact or a derived trust assertion is invalid."""


def documented_review_artifacts(root: Path = ROOT) -> set[str]:
    """Return the review artifact vocabulary documented by schema section 7."""
    path = root / ".claude/skills/_shared/artifact-schemas.md"
    try:
        source = path.read_text(encoding="utf-8")
    except OSError as error:
        raise ArtifactContractError(f"cannot read {path}: {error}") from error
    section = source.partition("## 7. Review-artifact frontmatter")[2].partition(
        "\n## "
    )[0]
    return set(REVIEW_ARTIFACT_ROW.findall(section)) - {"artifact"}


def active_plan_path(root: Path = ROOT) -> Path:
    """Resolve the repository's one reviewable active-plan pointer."""
    pointer_path = root / ACTIVE_PLAN_POINTER
    try:
        pointer = json.loads(pointer_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ArtifactContractError(
            f"{ACTIVE_PLAN_POINTER.as_posix()}: invalid active-plan pointer"
        ) from error
    if not isinstance(pointer, dict) or set(pointer) != {
        "schema_version",
        "plan_path",
    }:
        raise ArtifactContractError(
            f"{ACTIVE_PLAN_POINTER.as_posix()}: expected schema_version and plan_path"
        )
    if pointer["schema_version"] != 1 or not isinstance(pointer["plan_path"], str):
        raise ArtifactContractError(
            f"{ACTIVE_PLAN_POINTER.as_posix()}: unsupported active-plan pointer"
        )
    relative = Path(pointer["plan_path"])
    if relative.is_absolute() or ".." in relative.parts:
        raise ArtifactContractError(
            f"{ACTIVE_PLAN_POINTER.as_posix()}: plan_path must be repository-relative"
        )
    resolved = root / relative
    if not resolved.is_file():
        raise ArtifactContractError(
            f"{ACTIVE_PLAN_POINTER.as_posix()}: active plan does not exist"
        )
    return resolved


DEFAULT_PLAN = active_plan_path()


@dataclass(frozen=True)
class DeclaredInput:
    """One immutable plan input and its planning-time SHA-256 identity."""

    path: str
    digest: str


def _relative(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path)


def _run_git(
    root: Path,
    *args: str,
    check: bool = True,
    text: bool = True,
) -> subprocess.CompletedProcess[Any]:
    return subprocess.run(
        ("git", *args),
        cwd=root,
        check=check,
        capture_output=True,
        text=text,
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_frontmatter(path: Path) -> dict[str, Any]:
    """Parse the top-level scalar subset used by repository frontmatter."""
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != "---":
        raise ArtifactContractError(f"{_relative(path, ROOT)}: missing frontmatter")
    try:
        end = lines.index("---", 1)
    except ValueError as error:
        raise ArtifactContractError(
            f"{_relative(path, ROOT)}: unterminated frontmatter"
        ) from error

    values: dict[str, Any] = {}
    for number, line in enumerate(lines[1:end], start=2):
        if not line or line[0].isspace() or line.startswith("#"):
            continue
        if ":" not in line:
            raise ArtifactContractError(
                f"{_relative(path, ROOT)}:{number}: invalid frontmatter scalar"
            )
        key, raw_value = line.split(":", 1)
        key = key.strip()
        value: Any = raw_value.strip()
        if value in {"true", "false"}:
            value = value == "true"
        elif len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
            value = value[1:-1]
        if key in values:
            raise ArtifactContractError(
                f"{_relative(path, ROOT)}:{number}: duplicate frontmatter key {key}"
            )
        values[key] = value
    return values


def _validate_paths(
    root: Path,
    artifact_path: Path,
    values: Mapping[str, Any],
    *,
    allow_missing: frozenset[str] = frozenset(),
) -> None:
    for key, value in values.items():
        if not key.endswith("_path") or not isinstance(value, str):
            continue
        referenced = Path(value)
        if referenced.is_absolute() or ".." in referenced.parts:
            raise ArtifactContractError(
                f"{_relative(artifact_path, root)}: {key} must be repository-relative"
            )
        if key not in allow_missing and not (root / referenced).is_file():
            raise ArtifactContractError(
                f"{_relative(artifact_path, root)}: unresolved {key}={value}"
            )


def _validate_digest_pairs(
    root: Path, artifact_path: Path, values: Mapping[str, Any]
) -> None:
    for key, value in values.items():
        if not key.endswith("_digest") or not isinstance(value, str):
            continue
        path_key = f"{key.removesuffix('_digest')}_path"
        paired_path = values.get(path_key)
        if not isinstance(paired_path, str):
            continue
        observed = _sha256(root / paired_path)
        if value != observed:
            raise ArtifactContractError(
                f"{_relative(artifact_path, root)}: {key} mismatch for {paired_path}"
            )


def validate_review(root: Path, path: Path) -> dict[str, Any]:
    """Validate current review-artifact frontmatter."""
    values = parse_frontmatter(path)
    artifact = values.get("artifact")
    if artifact not in REVIEW_REQUIREMENTS:
        raise ArtifactContractError(
            f"{_relative(path, root)}: unknown review artifact {artifact!r}"
        )
    required = {
        "artifact",
        "date",
        "version",
        "status",
        *REVIEW_REQUIREMENTS[str(artifact)],
    }
    missing = required - values.keys()
    if missing:
        raise ArtifactContractError(
            f"{_relative(path, root)}: missing frontmatter keys {sorted(missing)}"
        )
    if not re.fullmatch(r"v\d+", str(values["version"])):
        raise ArtifactContractError(f"{_relative(path, root)}: invalid version")
    if values["status"] not in {"draft", "complete", "superseded"}:
        raise ArtifactContractError(f"{_relative(path, root)}: invalid status")
    allowed_verdicts = REVIEW_VERDICTS.get(str(artifact))
    if allowed_verdicts is not None and values.get("verdict") not in allowed_verdicts:
        raise ArtifactContractError(f"{_relative(path, root)}: invalid verdict")
    _validate_paths(root, path, values)
    _validate_digest_pairs(root, path, values)
    return values


def declared_inputs(plan_path: Path) -> list[DeclaredInput]:
    """Read the single declared-input table from implementation-plan section 2."""
    in_section = False
    inputs: list[DeclaredInput] = []
    for line in plan_path.read_text(encoding="utf-8").splitlines():
        if line.startswith("## 2."):
            in_section = True
            continue
        if in_section and line.startswith("## "):
            break
        if not in_section:
            continue
        match = INPUT_ROW.fullmatch(line)
        if match:
            inputs.append(
                DeclaredInput(match.group("path").strip(), match.group("digest"))
            )
    if not inputs:
        raise ArtifactContractError(
            f"{_relative(plan_path, ROOT)}: declared-input table is empty"
        )
    paths = [item.path for item in inputs]
    if len(paths) != len(set(paths)):
        raise ArtifactContractError(
            f"{_relative(plan_path, ROOT)}: duplicate declared-input path"
        )
    return inputs


def plan_ids(plan_path: Path) -> dict[str, list[str]]:
    """Return the closed packet, milestone, and decommission ID sets."""
    text = plan_path.read_text(encoding="utf-8")
    groups = {"packets": [], "milestones": [], "decommission_batches": []}
    all_heading_ids = [match.group("id") for match in STABLE_HEADING_ID.finditer(text)]
    duplicates = sorted(
        identifier
        for identifier in set(all_heading_ids)
        if all_heading_ids.count(identifier) > 1
    )
    if duplicates:
        raise ArtifactContractError(
            f"{_relative(plan_path, ROOT)}: duplicate stable heading IDs {duplicates}"
        )
    for match in PLAN_HEADING.finditer(text):
        identifier = match.group("id")
        if identifier.startswith("WP"):
            groups["packets"].append(identifier)
        elif identifier.startswith("DB"):
            groups["decommission_batches"].append(identifier)
        else:
            groups["milestones"].append(identifier)
    for name, identifiers in groups.items():
        if not identifiers:
            raise ArtifactContractError(
                f"{_relative(plan_path, ROOT)}: no {name.replace('_', ' ')} found"
            )
    return groups


def _packet_blocks(plan_path: Path) -> dict[str, str]:
    text = plan_path.read_text(encoding="utf-8")
    matches = list(re.finditer(r"^###\s+(WP\d+[a-z]?)\s+—", text, re.MULTILINE))
    blocks: dict[str, str] = {}
    for match in matches:
        next_heading = re.search(r"^(?:##|###)\s+", text[match.end() :], re.MULTILINE)
        end = (
            match.end() + next_heading.start()
            if next_heading is not None
            else len(text)
        )
        blocks[match.group(1)] = text[match.start() : end]
    return blocks


def _validate_oracle_catalog(plan_path: Path) -> dict[str, list[str]]:
    catalog: dict[str, list[str]] = {}
    for packet, block in _packet_blocks(plan_path).items():
        names = ORACLE.findall(block)
        if len(names) != 4 or len(set(names)) != 4:
            raise ArtifactContractError(
                f"{_relative(plan_path, ROOT)}: {packet} must declare four unique oracles"
            )
        catalog[packet] = names
    return catalog


def validate_plan(
    root: Path,
    plan_path: Path,
    *,
    verify_declared_inputs: bool = True,
    _allow_missing_state: bool = False,
) -> dict[str, Any]:
    """Validate implementation-plan structure and immutable inputs."""
    values = parse_frontmatter(plan_path)
    missing = PLAN_REQUIRED_KEYS - values.keys()
    if missing:
        raise ArtifactContractError(
            f"{_relative(plan_path, root)}: missing frontmatter keys {sorted(missing)}"
        )
    if values["artifact"] != "implementation-plan":
        raise ArtifactContractError(
            f"{_relative(plan_path, root)}: artifact must be implementation-plan"
        )
    if values["status"] not in {"draft", "audited", "approved", "superseded"}:
        raise ArtifactContractError(f"{_relative(plan_path, root)}: invalid status")
    if not re.fullmatch(r"v\d+", str(values["version"])):
        raise ArtifactContractError(f"{_relative(plan_path, root)}: invalid version")
    if not isinstance(values["cutover"], bool):
        raise ArtifactContractError(
            f"{_relative(plan_path, root)}: cutover must be bool"
        )
    try:
        inactive = active_plan_path(root).resolve() != plan_path.resolve()
    except ArtifactContractError:
        inactive = True
    future_state_is_valid = inactive and values["status"] in {"draft", "audited"}
    allow_missing = (
        frozenset({"state_path"})
        if _allow_missing_state or future_state_is_valid
        else frozenset()
    )
    _validate_paths(root, plan_path, values, allow_missing=allow_missing)
    _validate_digest_pairs(root, plan_path, values)
    identifiers = plan_ids(plan_path)
    _validate_oracle_catalog(plan_path)
    if verify_declared_inputs:
        _validate_declared_inputs(root, plan_path, set())
    return {**values, "ids": identifiers}


def _initial_state(
    plan_path: Path, plan: Mapping[str, Any], root: Path
) -> dict[str, Any]:
    """Construct the judgment-only state created by an activation transaction."""

    def entry() -> dict[str, Any]:
        return {
            "status": "not_started",
            "proving_commit": None,
            "deviations": [],
            "failed_approaches": [],
            "blockers": [],
        }

    identifiers = plan["ids"]
    return {
        "schema_version": 2,
        "plan_path": _relative(plan_path, root),
        "design_path": plan["design_path"],
        "baseline_commit": plan["baseline_commit"],
        "status": "not_started",
        "current_packet": None,
        "packets": {identifier: entry() for identifier in identifiers["packets"]},
        "milestones": {identifier: entry() for identifier in identifiers["milestones"]},
        "decommission_batches": {
            identifier: entry() for identifier in identifiers["decommission_batches"]
        },
        "baseline_failures": [],
        "discovered_obligations": [],
        "plan_deviations": [],
        "next_action": "Reconcile dependency readiness before the first packet edit.",
        "updated_at": datetime.now(UTC).isoformat(),
    }


def _stage_json(path: Path, value: Mapping[str, Any]) -> Path:
    """Write and fsync JSON beside its destination without publishing it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as handle:
        json.dump(value, handle, indent=2)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
        return Path(handle.name)


def activate_plan(root: Path, plan_path: Path) -> dict[str, Any]:
    """Create validated state before atomically switching the active-plan pointer."""
    plan_path = plan_path if plan_path.is_absolute() else root / plan_path
    try:
        plan_path.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise ArtifactContractError(
            "activation plan must be inside the repository"
        ) from error
    plan = validate_plan(
        root,
        plan_path,
        verify_declared_inputs=True,
        _allow_missing_state=True,
    )
    if plan["status"] != "approved":
        raise ArtifactContractError("only an approved plan can become active")

    state_path = root / str(plan["state_path"])
    if state_path.exists():
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: activation refuses to overwrite state"
        )

    state = _initial_state(plan_path, plan, root)
    staged_state = _stage_json(state_path, state)
    pointer_path = root / ACTIVE_PLAN_POINTER
    staged_pointer = _stage_json(
        pointer_path,
        {"schema_version": 1, "plan_path": _relative(plan_path, root)},
    )
    try:
        validate_state(root, staged_state, expected_ids=plan["ids"])
        os.replace(staged_state, state_path)
        os.replace(staged_pointer, pointer_path)
    finally:
        staged_state.unlink(missing_ok=True)
        staged_pointer.unlink(missing_ok=True)

    return {
        "plan": _relative(plan_path, root),
        "state": _relative(state_path, root),
        "active_plan_pointer": ACTIVE_PLAN_POINTER.as_posix(),
    }


def _accepted_input_evolution_paths(root: Path, state: Mapping[str, Any]) -> set[str]:
    accepted: set[str] = set()
    for deviation in state.get("plan_deviations", []):
        if (
            not isinstance(deviation, dict)
            or deviation.get("kind") != "planned_design_input_evolution"
            or not isinstance(deviation.get("packet"), str)
            or not isinstance(deviation.get("paths"), list)
        ):
            continue
        packet = deviation["packet"]
        entry = state.get("packets", {}).get(packet)
        if (
            isinstance(entry, dict)
            and entry.get("status") == "complete"
            and commit_trust(root, entry.get("proving_commit"))["ancestor"]
        ):
            accepted.update(
                path for path in deviation["paths"] if isinstance(path, str)
            )
    return accepted


def _accepted_gate_substitutions(
    state: Mapping[str, Any],
) -> dict[str, str]:
    """Return explicit historical-packet to current replacement-proof judgments."""
    substitutions: dict[str, str] = {}
    packets = state.get("packets", {})
    for deviation in state.get("plan_deviations", []):
        if (
            not isinstance(deviation, dict)
            or deviation.get("kind") != "accepted_gate_substitution"
            or "superseded_packets" not in deviation
        ):
            continue
        replacement = deviation.get("replacement_packet")
        superseded = deviation.get("superseded_packets")
        if not isinstance(replacement, str) or replacement not in packets:
            raise ArtifactContractError(
                "accepted_gate_substitution requires a known replacement_packet"
            )
        if not isinstance(superseded, list) or not superseded:
            raise ArtifactContractError(
                "accepted_gate_substitution requires nonempty superseded_packets"
            )
        for packet in superseded:
            if not isinstance(packet, str) or packet not in packets:
                raise ArtifactContractError(
                    "accepted_gate_substitution names an unknown superseded packet"
                )
            if packet == replacement:
                raise ArtifactContractError(
                    "accepted_gate_substitution cannot replace a packet with itself"
                )
            prior = substitutions.setdefault(packet, replacement)
            if prior != replacement:
                raise ArtifactContractError(
                    f"accepted_gate_substitution gives {packet} multiple replacements"
                )
    return substitutions


def _validate_declared_inputs(
    root: Path,
    plan_path: Path,
    accepted_paths: set[str],
) -> None:
    for item in declared_inputs(plan_path):
        input_path = root / item.path
        if not input_path.is_file():
            raise ArtifactContractError(
                f"{_relative(plan_path, root)}: missing declared input {item.path}"
            )
        if _sha256(input_path) != item.digest and item.path not in accepted_paths:
            raise ArtifactContractError(
                f"{_relative(plan_path, root)}: stale declared input {item.path}"
            )


def _reject_derived_keys(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in DERIVED_STATE_KEYS:
                raise ArtifactContractError(
                    f"{path}.{key}: derived state field is forbidden"
                )
            _reject_derived_keys(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_derived_keys(child, f"{path}[{index}]")


def _validate_entry(entry: Any, path: str) -> None:
    if not isinstance(entry, dict):
        raise ArtifactContractError(f"{path}: entry must be an object")
    keys = set(entry)
    if keys != ENTRY_KEYS:
        raise ArtifactContractError(
            f"{path}: expected keys {sorted(ENTRY_KEYS)}, observed {sorted(keys)}"
        )
    if entry["status"] not in ENTRY_STATUSES:
        raise ArtifactContractError(f"{path}.status: invalid status")
    for key in ("deviations", "failed_approaches", "blockers"):
        if not isinstance(entry[key], list):
            raise ArtifactContractError(f"{path}.{key}: must be an array")
    proving_commit = entry["proving_commit"]
    if proving_commit is not None and not isinstance(proving_commit, str):
        raise ArtifactContractError(f"{path}.proving_commit: must be string or null")
    if entry["status"] == "complete" and not proving_commit:
        raise ArtifactContractError(
            f"{path}.proving_commit: complete entry requires a proving commit"
        )


def load_state(path: Path) -> dict[str, Any]:
    """Load an execution state as a JSON object."""
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ArtifactContractError(f"{_relative(path, ROOT)}: invalid JSON") from error
    if not isinstance(value, dict):
        raise ArtifactContractError(
            f"{_relative(path, ROOT)}: state root must be object"
        )
    return value


def validate_state(
    root: Path,
    state_path: Path,
    *,
    expected_ids: Mapping[str, Sequence[str]] | None = None,
) -> dict[str, Any]:
    """Validate the exact schema-2, judgment-only execution-state shape."""
    state = load_state(state_path)
    if state.get("schema_version") != 2:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: only schema version 2 is writable/current"
        )
    keys = set(state)
    if keys != STATE_KEYS:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: expected keys {sorted(STATE_KEYS)}, "
            f"observed {sorted(keys)}"
        )
    _reject_derived_keys(state)
    if state["status"] not in OVERALL_STATUSES:
        raise ArtifactContractError(f"{_relative(state_path, root)}: invalid status")
    for key in (
        "baseline_failures",
        "discovered_obligations",
        "plan_deviations",
    ):
        if not isinstance(state[key], list):
            raise ArtifactContractError(
                f"{_relative(state_path, root)}: {key} must be an array"
            )
    for group in ("packets", "milestones", "decommission_batches"):
        entries = state[group]
        if not isinstance(entries, dict):
            raise ArtifactContractError(
                f"{_relative(state_path, root)}: {group} must be an object"
            )
        if expected_ids is not None and set(entries) != set(expected_ids[group]):
            missing = sorted(set(expected_ids[group]) - set(entries))
            extra = sorted(set(entries) - set(expected_ids[group]))
            raise ArtifactContractError(
                f"{_relative(state_path, root)}: {group} ID mismatch; "
                f"missing={missing}, extra={extra}"
            )
        for identifier, entry in entries.items():
            _validate_entry(entry, f"$.{group}.{identifier}")
    current = state["current_packet"]
    if current is not None and current not in state["packets"]:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: current_packet is not in packets"
        )
    return state


def validate_artifacts(
    root: Path = ROOT, plan_path: Path = DEFAULT_PLAN
) -> dict[str, Any]:
    """Validate the active plan, its current reviews, and its schema-2 state."""
    plan_path = plan_path if plan_path.is_absolute() else root / plan_path
    plan = validate_plan(root, plan_path, verify_declared_inputs=False)
    state_path = root / str(plan["state_path"])
    state = validate_state(root, state_path, expected_ids=plan["ids"])
    _validate_declared_inputs(
        root,
        plan_path,
        _accepted_input_evolution_paths(root, state),
    )
    expected_plan_path = _relative(plan_path, root)
    if state["plan_path"] != expected_plan_path:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: plan_path does not identify active plan"
        )
    if state["design_path"] != plan["design_path"]:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: design_path differs from active plan"
        )
    if state["baseline_commit"] != plan["baseline_commit"]:
        raise ArtifactContractError(
            f"{_relative(state_path, root)}: baseline_commit differs from active plan"
        )
    reviews: list[str] = []
    for key in ("status_review_path", "implementation_review_path"):
        value = plan.get(key)
        if isinstance(value, str):
            validate_review(root, root / value)
            reviews.append(value)
    from tooling.ci import released_fixture_verifier

    fixture_report = released_fixture_verifier.verify_released_assurance(root)
    return {
        "plan": expected_plan_path,
        "state": _relative(state_path, root),
        "reviews": reviews,
        "declared_input_count": len(declared_inputs(plan_path)),
        "packet_count": len(plan["ids"]["packets"]),
        "released_fixture_count": fixture_report["fixtures"]["fixture_count"],
    }


def commit_trust(root: Path, commit: str | None) -> dict[str, Any]:
    if not commit:
        return {"exists": False, "ancestor": False}
    exists = (
        _run_git(root, "cat-file", "-e", f"{commit}^{{commit}}", check=False).returncode
        == 0
    )
    ancestor = exists and (
        _run_git(
            root, "merge-base", "--is-ancestor", commit, "HEAD", check=False
        ).returncode
        == 0
    )
    return {"exists": exists, "ancestor": ancestor}


def _implemented_oracle(root: Path, oracle: str) -> bool:
    search_roots = [
        path
        for name in (
            "src",
            "tests",
            "tooling",
            "scripts",
            "codefabric-cpg-mcp",
            "rustc-extractor",
            "pyrefly-sidecar",
        )
        if (path := root / name).exists()
    ]
    return (
        subprocess.run(
            ("rg", "-F", "-l", oracle, *(str(path) for path in search_roots)),
            cwd=root,
            check=False,
            capture_output=True,
        ).returncode
        == 0
    )


def derive_plan_status(
    root: Path = ROOT, plan_path: Path = DEFAULT_PLAN
) -> dict[str, Any]:
    """Derive plan freshness and packet trust without mutating state."""
    plan_path = plan_path if plan_path.is_absolute() else root / plan_path
    plan = validate_plan(root, plan_path, verify_declared_inputs=False)
    state = validate_state(
        root,
        root / str(plan["state_path"]),
        expected_ids=plan["ids"],
    )
    baseline = commit_trust(root, str(plan["baseline_commit"]))
    accepted_paths = _accepted_input_evolution_paths(root, state)
    inputs = []
    for item in declared_inputs(plan_path):
        fresh = (root / item.path).is_file() and _sha256(
            root / item.path
        ) == item.digest
        inputs.append(
            {
                "path": item.path,
                "fresh": fresh,
                "accepted_evolution": not fresh and item.path in accepted_paths,
            }
        )
    oracle_catalog = _validate_oracle_catalog(plan_path)
    just_recipes = load_just_recipes()
    blocks = _packet_blocks(plan_path)
    substitutions = _accepted_gate_substitutions(state)
    current_proofs: dict[str, tuple[dict[str, bool | None], list[str], bool]] = {}
    for packet, entry in state["packets"].items():
        declared_oracles = oracle_catalog[packet]
        if entry["status"] == "complete":
            implemented = {
                oracle: (
                    oracle.removeprefix("just ") in just_recipes
                    if oracle.startswith("just ")
                    else _implemented_oracle(root, oracle)
                )
                for oracle in declared_oracles
            }
        else:
            implemented = dict.fromkeys(declared_oracles)
        required_recipes = sorted(set(JUST_RECIPE.findall(blocks[packet])))
        recipes_resolve = all(recipe in just_recipes for recipe in required_recipes)
        current_proofs[packet] = (implemented, required_recipes, recipes_resolve)

    packet_status: dict[str, Any] = {}
    untrusted: list[str] = []
    for packet, entry in state["packets"].items():
        commit = commit_trust(root, entry["proving_commit"])
        implemented, required_recipes, recipes_resolve = current_proofs[packet]
        replacement = substitutions.get(packet)
        replacement_trusted = False
        if replacement is not None:
            replacement_entry = state["packets"][replacement]
            replacement_commit = commit_trust(root, replacement_entry["proving_commit"])
            replacement_implemented, _, replacement_recipes_resolve = current_proofs[
                replacement
            ]
            replacement_trusted = (
                replacement_entry["status"] == "complete"
                and replacement_commit["ancestor"]
                and all(replacement_implemented.values())
                and replacement_recipes_resolve
            )
        trusted = (
            entry["status"] == "complete"
            and commit["ancestor"]
            and ((all(implemented.values()) and recipes_resolve) or replacement_trusted)
        )
        if entry["status"] == "complete" and not trusted:
            untrusted.append(packet)
        packet_status[packet] = {
            "status": entry["status"],
            "proving_commit": entry["proving_commit"],
            "commit": commit,
            "named_oracles": implemented,
            "required_recipes": required_recipes,
            "recipes_resolve": recipes_resolve,
            "assurance_substitute": replacement if replacement_trusted else None,
            "trusted": trusted,
        }
    completion_groups: dict[str, dict[str, Any]] = {}
    untrusted_entries: list[str] = []
    for group in ("milestones", "decommission_batches"):
        completion_groups[group] = {}
        for identifier, entry in state[group].items():
            commit = commit_trust(root, entry["proving_commit"])
            trusted = entry["status"] == "complete" and commit["ancestor"]
            qualified = f"{group}.{identifier}"
            if entry["status"] == "complete" and not trusted:
                untrusted_entries.append(qualified)
            completion_groups[group][identifier] = {
                "status": entry["status"],
                "proving_commit": entry["proving_commit"],
                "commit": commit,
                "trusted": trusted,
            }
    healthy = (
        baseline["ancestor"]
        and all(item["fresh"] or item["accepted_evolution"] for item in inputs)
        and not untrusted
        and not untrusted_entries
    )
    return {
        "schema_version": 1,
        "plan_path": _relative(plan_path, root),
        "baseline": {"commit": plan["baseline_commit"], **baseline},
        "declared_inputs": inputs,
        "packets": packet_status,
        **completion_groups,
        "untrusted_complete_packets": untrusted,
        "untrusted_complete_entries": untrusted_entries,
        "healthy": healthy,
    }


def _target_paths(values: bytes) -> list[str]:
    paths: list[str] = []
    for value in values.split(b"\0"):
        if value and TARGET_PATH.search(value):
            paths.append(value.decode("utf-8", errors="backslashreplace"))
    return paths


def check_tracked_target_zero_state(root: Path = ROOT) -> dict[str, Any]:
    """Prove Cargo target roots are ignored and absent from index and HEAD history."""
    root = root.resolve()
    if _run_git(root, "rev-parse", "--show-toplevel", check=False).returncode != 0:
        raise ArtifactContractError(f"{root}: not a Git repository")
    probes = (
        "target/.codefabric-ignore-probe",
        "fuzz/target/.codefabric-ignore-probe",
        "rustc-extractor/target/.codefabric-ignore-probe",
        "pyrefly-sidecar/target/.codefabric-ignore-probe",
    )
    uncovered = [
        probe
        for probe in probes
        if _run_git(
            root, "check-ignore", "--no-index", "-q", probe, check=False
        ).returncode
        != 0
    ]
    tracked_output = _run_git(root, "ls-files", "-z", text=False).stdout
    tracked = _target_paths(tracked_output)
    history_process = _run_git(
        root, "rev-list", "--objects", "HEAD", "-z", check=False, text=False
    )
    if history_process.returncode != 0:
        raise ArtifactContractError(f"{root}: HEAD is not a readable commit")
    historical = _target_paths(history_process.stdout)
    if uncovered or tracked or historical:
        raise ArtifactContractError(
            "tracked target zero state failed: "
            f"uncovered_ignore_roots={uncovered}, tracked={tracked[:8]}, "
            f"historical={historical[:8]}"
        )
    return {
        "ignore_probes": list(probes),
        "tracked_target_paths": 0,
        "reachable_history_target_paths": 0,
    }


def _print_report(report: Mapping[str, Any]) -> None:
    print(json.dumps(report, indent=2, sort_keys=True))


def _status_summary(report: Mapping[str, Any]) -> dict[str, Any]:
    packets = report["packets"]
    assert isinstance(packets, dict)
    declared_inputs_report = report["declared_inputs"]
    assert isinstance(declared_inputs_report, list)
    return {
        "plan_path": report["plan_path"],
        "baseline": report["baseline"],
        "declared_input_count": len(declared_inputs_report),
        "stale_inputs": [
            item["path"]
            for item in declared_inputs_report
            if not item["fresh"] and not item["accepted_evolution"]
        ],
        "accepted_input_evolutions": [
            item["path"]
            for item in declared_inputs_report
            if item["accepted_evolution"]
        ],
        "complete_packets": [
            packet
            for packet, status in packets.items()
            if status["status"] == "complete"
        ],
        "complete_milestones": [
            identifier
            for identifier, status in report["milestones"].items()
            if status["status"] == "complete"
        ],
        "complete_decommission_batches": [
            identifier
            for identifier, status in report["decommission_batches"].items()
            if status["status"] == "complete"
        ],
        "untrusted_complete_packets": report["untrusted_complete_packets"],
        "untrusted_complete_entries": report["untrusted_complete_entries"],
        "healthy": report["healthy"],
    }


def _plan_argument(value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else ROOT / path


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=(
            "activate-plan",
            "artifacts-check",
            "plan-status",
            "tracked-target-zero-state-check",
        ),
    )
    parser.add_argument("--plan", type=_plan_argument, default=DEFAULT_PLAN)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.command == "activate-plan":
            report = activate_plan(args.root, args.plan)
        elif args.command == "artifacts-check":
            report = validate_artifacts(args.root, args.plan)
        elif args.command == "plan-status":
            report = derive_plan_status(args.root, args.plan)
            if not report["healthy"]:
                _print_report(report if args.verbose else _status_summary(report))
                return 1
        else:
            report = check_tracked_target_zero_state(args.root)
    except (ArtifactContractError, subprocess.CalledProcessError) as error:
        print(f"artifact contract error: {error}", file=sys.stderr)
        return 1
    _print_report(
        report
        if args.command != "plan-status" or args.verbose
        else _status_summary(report)
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
