"""Fail-closed WP42 certification for the relational-fabric v3 successor.

The immutable plan remains the oracle-name authority.  This module narrows the owner-adjusted
certification scope to WP29-WP42, derives the 56 oracle/criterion pairs, executes live Just recipes,
and writes a machine-readable candidate record only after provenance and review prerequisites pass.
WP28 and M01 are explicit exclusions and can never be promoted by this surface.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import resource
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from tooling.ci import artifact_contracts, plan_assurance

ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = Path(
    "contracts/acceptance/relational-fabric-v3/successor-certification-contract.json"
)
EXPECTED_PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
)
EXPECTED_STATE_PATH = Path(
    "docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json"
)
EXPECTED_PACKETS = tuple(f"WP{number:02d}" for number in range(29, 43))
EXPECTED_CATEGORIES = ("INT", "BEH", "NEG", "OPS")
EXPECTED_MILESTONES = tuple(f"M{number:02d}" for number in range(2, 7))
EXPECTED_BATCHES = tuple(f"DB{number:02d}" for number in range(9, 15))
EXPECTED_DOMAINS = (
    "stable-root",
    "rustc-extractor",
    "pyrefly-sidecar",
    "python-adapter",
)
EXPECTED_TERMINAL_GATES = {
    "INT": "successor-provenance-state-integrity-check",
    "BEH": "relational-fabric-v3-certification",
    "NEG": "successor-final-zero-state-check",
    "OPS": "successor-four-domain-release-check",
}
HEAD = re.compile(r"[0-9a-f]{40}\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
SELECTED_TESTS = (
    re.compile(r"Starting\s+(\d+)\s+tests?\b"),
    re.compile(r"\b(\d+)\s+passed\b"),
)
FORBIDDEN_SEMANTIC_ACCEPTANCE_KEYS = {
    "expected_file_count",
    "legacy_digest",
    "legacy_hash",
    "predecessor_digest",
    "predecessor_hash",
    "semantic_digest_agreement",
}


class SuccessorCertificationError(ValueError):
    """The candidate cannot truthfully satisfy one WP42 certification boundary."""


@dataclass(frozen=True)
class OracleContract:
    packet: str
    category: str
    oracle: str
    criterion: str


@dataclass(frozen=True)
class CommandObservation:
    command: tuple[str, ...]
    exit_code: int
    selected_test_count: int | None
    elapsed_ms: int
    resource_summary: Mapping[str, int | str]
    output_sha256: str


@dataclass(frozen=True)
class HostProfileObservation:
    profile: str
    availability: str
    admission: str
    architecture_effect: str
    detail: str


RecipeRunner = Callable[[str], CommandObservation]
TrustResolver = Callable[[str | None], Mapping[str, Any]]


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise SuccessorCertificationError(f"duplicate JSON member {key!r}")
        result[key] = value
    return result


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, json.JSONDecodeError) as error:
        raise SuccessorCertificationError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise SuccessorCertificationError(f"{path} must contain one JSON object")
    return value


def _expect_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    if set(value) != expected:
        raise SuccessorCertificationError(
            f"{context} fields differ: expected={sorted(expected)}, "
            f"observed={sorted(value)}"
        )


def _string_list(value: Any, context: str, *, nonempty: bool = True) -> list[str]:
    if (
        not isinstance(value, list)
        or (nonempty and not value)
        or not all(isinstance(item, str) and item for item in value)
    ):
        raise SuccessorCertificationError(f"{context} must be a string list")
    if len(value) != len(set(value)):
        raise SuccessorCertificationError(f"{context} contains duplicates")
    return value


def load_contract(root: Path = ROOT) -> dict[str, Any]:
    """Load and close the owner-adjusted certification contract."""

    contract = _load_json(root / CONTRACT_PATH)
    _expect_keys(
        contract,
        {
            "schema_version",
            "contract_id",
            "suite",
            "plan_path",
            "state_path",
            "certification_record_schema_path",
            "scope",
            "terminal_gate_recipes",
            "four_domain_release",
            "cross_domain_recipes",
            "zero_state_recipes",
            "host_capability_policy",
        },
        "certification contract",
    )
    if (
        contract["schema_version"] != 1
        or contract["contract_id"]
        != "codefabric.relational-fabric-v3.successor-certification.v1"
        or contract["suite"] != "codefabric-relational-data-fabric@2.1.0"
        or contract["plan_path"] != EXPECTED_PLAN_PATH.as_posix()
        or contract["state_path"] != EXPECTED_STATE_PATH.as_posix()
    ):
        raise SuccessorCertificationError("certification root identity differs")

    scope = contract.get("scope")
    if not isinstance(scope, dict):
        raise SuccessorCertificationError("certification scope must be an object")
    _expect_keys(
        scope,
        {
            "packets",
            "excluded_packets",
            "oracle_categories",
            "oracle_count",
            "required_milestones",
            "excluded_milestones",
            "required_decommission_batches",
        },
        "certification scope",
    )
    if tuple(_string_list(scope["packets"], "scope.packets")) != EXPECTED_PACKETS:
        raise SuccessorCertificationError(
            "certification packets must be exactly WP29-WP42"
        )
    if _string_list(scope["excluded_packets"], "scope.excluded_packets") != ["WP28"]:
        raise SuccessorCertificationError("WP28 must remain the sole excluded packet")
    if tuple(_string_list(scope["oracle_categories"], "scope.oracle_categories")) != (
        EXPECTED_CATEGORIES
    ):
        raise SuccessorCertificationError("oracle categories differ")
    if scope["oracle_count"] != len(EXPECTED_PACKETS) * len(EXPECTED_CATEGORIES):
        raise SuccessorCertificationError(
            "certification must derive exactly 56 oracles"
        )
    if (
        tuple(_string_list(scope["required_milestones"], "scope.required_milestones"))
        != EXPECTED_MILESTONES
    ):
        raise SuccessorCertificationError("required milestones must be exactly M02-M06")
    if _string_list(scope["excluded_milestones"], "scope.excluded_milestones") != [
        "M01"
    ]:
        raise SuccessorCertificationError("M01 must remain excluded")
    if (
        tuple(
            _string_list(
                scope["required_decommission_batches"],
                "scope.required_decommission_batches",
            )
        )
        != EXPECTED_BATCHES
    ):
        raise SuccessorCertificationError(
            "decommission scope must be exactly DB09-DB14"
        )

    terminal = contract.get("terminal_gate_recipes")
    if terminal != EXPECTED_TERMINAL_GATES:
        raise SuccessorCertificationError("WP42 terminal gate mapping differs")
    _validate_domain_contract(contract)
    _validate_host_policy(contract)
    _validate_schema_definition(root, contract)
    _reject_forbidden_semantic_acceptance(contract)
    return contract


def _validate_domain_contract(contract: Mapping[str, Any]) -> None:
    entries = contract.get("four_domain_release")
    if not isinstance(entries, list) or not all(
        isinstance(item, dict) for item in entries
    ):
        raise SuccessorCertificationError("four_domain_release must be an object list")
    domains: list[str] = []
    all_recipes: list[str] = []
    for entry in entries:
        _expect_keys(entry, {"domain", "recipes"}, "four-domain entry")
        domain = entry.get("domain")
        if not isinstance(domain, str):
            raise SuccessorCertificationError("four-domain identity must be a string")
        domains.append(domain)
        all_recipes.extend(_string_list(entry.get("recipes"), f"{domain}.recipes"))
    if tuple(domains) != EXPECTED_DOMAINS:
        raise SuccessorCertificationError(
            "release composition must contain exactly four domains"
        )
    cross = _string_list(contract.get("cross_domain_recipes"), "cross_domain_recipes")
    zero = _string_list(contract.get("zero_state_recipes"), "zero_state_recipes")
    if len(all_recipes) != len(set(all_recipes)):
        raise SuccessorCertificationError("four-domain recipes must be uniquely owned")
    if set(all_recipes) & set(cross):
        raise SuccessorCertificationError("domain and cross-domain recipes overlap")
    if "post-purge-package-build-operations-check" not in zero:
        raise SuccessorCertificationError(
            "zero state lacks package/compiler reachability proof"
        )
    if "predecessor-restart-revocation-check" not in zero:
        raise SuccessorCertificationError("zero state lacks restart revocation proof")


def _validate_host_policy(contract: Mapping[str, Any]) -> None:
    policy = contract.get("host_capability_policy")
    if not isinstance(policy, dict):
        raise SuccessorCertificationError("host capability policy must be an object")
    expected = {
        "profile": "untrusted-provider-execution",
        "observation_recipe": "semantic-sandbox-host-matrix-check",
        "unavailable_terminal": "UNAVAILABLE",
        "fallback": "forbidden",
        "certification_effect": "profile_unavailable",
        "architecture_effect": "not_global_failure_if_fail_closed",
    }
    if policy != expected:
        raise SuccessorCertificationError(
            "unavailable host capability must remain explicit and fail closed"
        )


def _validate_schema_definition(root: Path, contract: Mapping[str, Any]) -> None:
    schema_path = contract.get("certification_record_schema_path")
    if not isinstance(schema_path, str):
        raise SuccessorCertificationError("record schema path must be a string")
    schema = _load_json(root / schema_path)
    if (
        schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema"
        or schema.get("type") != "object"
        or schema.get("additionalProperties") is not False
    ):
        raise SuccessorCertificationError("certification record schema is not closed")
    required = _string_list(schema.get("required"), "record schema required")
    properties = schema.get("properties")
    if not isinstance(properties, dict) or set(required) != set(properties):
        raise SuccessorCertificationError("record schema required/properties differ")
    results = properties.get("oracle_results")
    if (
        not isinstance(results, dict)
        or results.get("minItems") != 56
        or results.get("maxItems") != 56
    ):
        raise SuccessorCertificationError(
            "record schema must require exactly 56 results"
        )


def _reject_forbidden_semantic_acceptance(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_SEMANTIC_ACCEPTANCE_KEYS:
                raise SuccessorCertificationError(
                    f"{path}.{key} reintroduces legacy/static semantic acceptance"
                )
            _reject_forbidden_semantic_acceptance(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_forbidden_semantic_acceptance(child, f"{path}[{index}]")


def derive_oracle_catalog(
    root: Path = ROOT, contract: Mapping[str, Any] | None = None
) -> list[OracleContract]:
    """Derive the exact 56 owner-adjusted oracle mappings from the plan."""

    contract = load_contract(root) if contract is None else contract
    plan_path = root / str(contract["plan_path"])
    pairs = plan_assurance._oracle_contracts(
        plan_path, selected_packets=set(EXPECTED_PACKETS)
    )
    if tuple(sorted(pairs, key=lambda item: int(item[2:]))) != EXPECTED_PACKETS:
        raise SuccessorCertificationError(
            "plan-derived packet scope differs from WP29-WP42"
        )
    result = [
        OracleContract(packet, category, oracle, criterion)
        for packet in EXPECTED_PACKETS
        for category, (oracle, criterion) in zip(
            EXPECTED_CATEGORIES, pairs[packet], strict=True
        )
    ]
    if len(result) != 56 or len({item.oracle for item in result}) != 56:
        raise SuccessorCertificationError(
            "plan does not derive 56 unique successor oracles"
        )
    wp42 = {item.category: item.oracle for item in result if item.packet == "WP42"}
    if wp42 != contract["terminal_gate_recipes"]:
        raise SuccessorCertificationError("WP42 recipe mapping differs from the plan")
    if any(
        item.packet == "WP28" or item.criterion.startswith("PC-WP28-")
        for item in result
    ):
        raise SuccessorCertificationError(
            "WP28 entered the successor certification catalog"
        )
    return result


def validate_definition_scope(
    root: Path,
    catalog: Sequence[OracleContract],
    *,
    definitions: Sequence[plan_assurance.OracleDefinition] | None = None,
) -> None:
    """Require one substantive live definition for every and only scoped oracle."""

    wanted = {item.oracle for item in catalog}
    observed = (
        plan_assurance.oracle_definitions(root, wanted)
        if definitions is None
        else list(definitions)
    )
    try:
        plan_assurance._require_exact_definitions(
            wanted, observed, context="WP29-WP42 certification"
        )
    except plan_assurance.PlanAssuranceError as error:
        raise SuccessorCertificationError(str(error)) from error


def _all_supporting_recipes(contract: Mapping[str, Any]) -> set[str]:
    recipes = set(contract["terminal_gate_recipes"].values())
    for entry in contract["four_domain_release"]:
        recipes.update(entry["recipes"])
    recipes.update(contract["cross_domain_recipes"])
    recipes.update(contract["zero_state_recipes"])
    recipes.add("packet-oracle-check")
    return recipes


def validate_live_recipe_inventory(root: Path, contract: Mapping[str, Any]) -> None:
    try:
        recipes = artifact_contracts.load_just_recipes(root)
    except (subprocess.CalledProcessError, json.JSONDecodeError) as error:
        raise SuccessorCertificationError(
            f"cannot derive Just recipe graph: {error}"
        ) from error
    missing = sorted(_all_supporting_recipes(contract) - recipes.keys())
    if missing:
        raise SuccessorCertificationError(
            f"certification recipes are absent: {missing}"
        )


def validate_contract_integrity(root: Path = ROOT) -> list[OracleContract]:
    contract = load_contract(root)
    catalog = derive_oracle_catalog(root, contract)
    validate_live_recipe_inventory(root, contract)
    validate_definition_scope(root, catalog)
    return catalog


def _entry_complete_and_trusted(
    entry: Mapping[str, Any], identifier: str, trust_resolver: TrustResolver
) -> None:
    if entry.get("status") != "complete":
        raise SuccessorCertificationError(f"{identifier} is not complete")
    commit = entry.get("proving_commit")
    trust = trust_resolver(commit if isinstance(commit, str) else None)
    if not trust.get("exists") or not trust.get("ancestor"):
        raise SuccessorCertificationError(
            f"{identifier} proving commit is not ancestral"
        )


def validate_state_provenance(
    state: Mapping[str, Any],
    *,
    phase: str,
    trust_resolver: TrustResolver,
) -> None:
    """Validate candidate/final proof lineage without inventing completion."""

    if phase not in {"candidate", "final"}:
        raise SuccessorCertificationError(f"unknown certification phase {phase}")
    packets = state.get("packets")
    milestones = state.get("milestones")
    batches = state.get("decommission_batches")
    if not all(isinstance(group, dict) for group in (packets, milestones, batches)):
        raise SuccessorCertificationError("execution state groups are absent")
    assert isinstance(packets, dict)
    assert isinstance(milestones, dict)
    assert isinstance(batches, dict)
    for group, identifier in ((packets, "WP28"), (milestones, "M01")):
        entry = group.get(identifier)
        if not isinstance(entry, dict) or entry.get("status") != "invalidated":
            raise SuccessorCertificationError(f"{identifier} must remain invalidated")
        if entry.get("proving_commit") is not None:
            raise SuccessorCertificationError(
                f"{identifier} cannot acquire proving lineage"
            )

    packet_limit = 43 if phase == "final" else 42
    milestone_limit = 7 if phase == "final" else 6
    batch_limit = 15 if phase == "final" else 14
    for packet in EXPECTED_PACKETS[: packet_limit - 29]:
        entry = packets.get(packet)
        if not isinstance(entry, dict):
            raise SuccessorCertificationError(f"state lacks {packet}")
        _entry_complete_and_trusted(entry, packet, trust_resolver)
    for milestone in EXPECTED_MILESTONES[: milestone_limit - 2]:
        entry = milestones.get(milestone)
        if not isinstance(entry, dict):
            raise SuccessorCertificationError(f"state lacks {milestone}")
        _entry_complete_and_trusted(entry, milestone, trust_resolver)
    for batch in EXPECTED_BATCHES[: batch_limit - 9]:
        entry = batches.get(batch)
        if not isinstance(entry, dict):
            raise SuccessorCertificationError(f"state lacks {batch}")
        _entry_complete_and_trusted(entry, batch, trust_resolver)

    if phase == "candidate":
        for group, identifier in (
            (packets, "WP42"),
            (milestones, "M06"),
            (batches, "DB14"),
        ):
            entry = group.get(identifier)
            if not isinstance(entry, dict) or entry.get("status") not in {
                "in_progress",
                "complete",
            }:
                raise SuccessorCertificationError(
                    f"{identifier} must be in progress or complete at candidate certification"
                )
            if entry.get("status") == "complete":
                _entry_complete_and_trusted(entry, identifier, trust_resolver)


def _git(root: Path, *args: str) -> str:
    return subprocess.run(
        ("git", *args), cwd=root, check=True, capture_output=True, text=True
    ).stdout.strip()


def _require_clean_trusted_head(root: Path) -> str:
    head = _git(root, "rev-parse", "HEAD")
    if not HEAD.fullmatch(head):
        raise SuccessorCertificationError("candidate HEAD is invalid")
    dirty = _git(root, "status", "--porcelain=v1", "--untracked-files=all")
    if dirty:
        raise SuccessorCertificationError(
            "terminal certification requires a clean, explicitly trusted HEAD"
        )
    return head


def _accepted_review(root: Path, plan_path: str) -> tuple[str, str]:
    accepted: list[tuple[str, str]] = []
    for path in sorted((root / "docs/reviews").glob("*.md")):
        try:
            values = artifact_contracts.parse_frontmatter(path)
        except artifact_contracts.ArtifactContractError:
            continue
        if (
            values.get("artifact") == "implementation-review"
            and values.get("plan_path") == plan_path
            and values.get("status") == "complete"
            and values.get("verdict") in {"approved", "approved-with-minor-findings"}
        ):
            accepted.append((path.relative_to(root).as_posix(), str(values["verdict"])))
    if not accepted:
        raise SuccessorCertificationError(
            "no accepted independent implementation review exists for the v3 plan"
        )
    return accepted[-1]


def validate_provenance_state(
    root: Path = ROOT, *, phase: str = "candidate", require_clean: bool = True
) -> tuple[str, tuple[str, str]]:
    contract = load_contract(root)
    plan_path = root / str(contract["plan_path"])
    try:
        plan = artifact_contracts.validate_plan(root, plan_path)
        state = artifact_contracts.validate_state(
            root,
            root / str(contract["state_path"]),
            expected_ids=plan["ids"],
        )
    except artifact_contracts.ArtifactContractError as error:
        raise SuccessorCertificationError(str(error)) from error
    validate_state_provenance(
        state,
        phase=phase,
        trust_resolver=lambda commit: artifact_contracts.commit_trust(root, commit),
    )
    head = (
        _require_clean_trusted_head(root)
        if require_clean
        else _git(root, "rev-parse", "HEAD")
    )
    review = _accepted_review(root, str(contract["plan_path"]))
    return head, review


def _selected_test_count(output: str) -> int | None:
    counts: list[int] = []
    for pattern in SELECTED_TESTS:
        counts.extend(int(value) for value in pattern.findall(output))
    return sum(counts) if counts else None


def run_recipe(root: Path, recipe: str) -> CommandObservation:
    """Execute one live recipe and retain only reproducible identity/summary evidence."""

    command = ("just", recipe)
    started = time.monotonic()
    usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    with tempfile.TemporaryFile() as output:
        completed = subprocess.run(
            command,
            cwd=root,
            check=False,
            stdout=output,
            stderr=subprocess.STDOUT,
        )
        elapsed_ms = round((time.monotonic() - started) * 1000)
        output.seek(0)
        body = output.read()
    usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    text = body.decode("utf-8", errors="replace")
    if completed.returncode != 0:
        tail = "\n".join(text.splitlines()[-40:])
        raise SuccessorCertificationError(
            f"just {recipe} exited {completed.returncode}\n{tail}"
        )
    return CommandObservation(
        command=command,
        exit_code=completed.returncode,
        selected_test_count=_selected_test_count(text),
        elapsed_ms=elapsed_ms,
        resource_summary={
            "child_user_cpu_ms": round(
                (usage_after.ru_utime - usage_before.ru_utime) * 1000
            ),
            "child_system_cpu_ms": round(
                (usage_after.ru_stime - usage_before.ru_stime) * 1000
            ),
            "children_ru_maxrss": round(usage_after.ru_maxrss),
            "ru_maxrss_unit": "platform_native",
        },
        output_sha256=hashlib.sha256(body).hexdigest(),
    )


def _summarize_internal(label: str, values: Any) -> CommandObservation:
    body = json.dumps(values, sort_keys=True, separators=(",", ":")).encode()
    return CommandObservation(
        command=("internal", label),
        exit_code=0,
        selected_test_count=None,
        elapsed_ms=0,
        resource_summary={
            "child_user_cpu_ms": 0,
            "child_system_cpu_ms": 0,
            "children_ru_maxrss": 0,
            "ru_maxrss_unit": "internal_observation",
        },
        output_sha256=hashlib.sha256(body).hexdigest(),
    )


def observe_host_profile(root: Path = ROOT) -> HostProfileObservation:
    """Observe external containment conservatively; exact executable identity is insufficient."""

    executable = Path("/usr/bin/bwrap")
    observed = "absent"
    if executable.is_file():
        completed = subprocess.run(
            (str(executable), "--version"),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        observed = (completed.stdout or completed.stderr).strip() or "unreadable"
    return HostProfileObservation(
        profile="untrusted-provider-execution",
        availability="unavailable",
        admission="fail_closed",
        architecture_effect="profile_unavailable",
        detail=(
            f"observed bubblewrap identity: {observed}; executable identity alone cannot prove "
            "compiled seccomp, kernel accounting, or the escape matrix"
        ),
    )


def execute_recipe_set(
    recipes: Sequence[str], runner: RecipeRunner
) -> dict[str, CommandObservation]:
    result: dict[str, CommandObservation] = {}
    for recipe in recipes:
        if recipe in result:
            raise SuccessorCertificationError(f"duplicate recipe execution {recipe}")
        result[recipe] = runner(recipe)
    return result


def four_domain_recipes(contract: Mapping[str, Any]) -> list[str]:
    return [
        recipe
        for entry in contract["four_domain_release"]
        for recipe in entry["recipes"]
    ] + list(contract["cross_domain_recipes"])


def execute_four_domain_release(
    root: Path = ROOT, *, runner: RecipeRunner | None = None
) -> tuple[dict[str, CommandObservation], HostProfileObservation]:
    contract = load_contract(root)
    validate_live_recipe_inventory(root, contract)
    actual_runner = (
        (lambda recipe: run_recipe(root, recipe)) if runner is None else runner
    )
    observations = execute_recipe_set(four_domain_recipes(contract), actual_runner)
    return observations, observe_host_profile(root)


def execute_final_zero_state(
    root: Path = ROOT, *, runner: RecipeRunner | None = None
) -> dict[str, CommandObservation]:
    contract = load_contract(root)
    validate_live_recipe_inventory(root, contract)
    actual_runner = (
        (lambda recipe: run_recipe(root, recipe)) if runner is None else runner
    )
    return execute_recipe_set(contract["zero_state_recipes"], actual_runner)


def _immutable_inputs(root: Path, contract: Mapping[str, Any]) -> list[dict[str, str]]:
    paths = {
        Path(str(contract["plan_path"])),
        Path(str(contract["state_path"])),
        CONTRACT_PATH,
        Path(str(contract["certification_record_schema_path"])),
        Path("Cargo.lock"),
        Path("rustc-extractor/Cargo.lock"),
        Path("pyrefly-sidecar/Cargo.lock"),
        Path("codefabric-cpg-mcp/uv.lock"),
        Path("tooling/proto/production-descriptor.pb"),
    }
    return [
        {
            "path": path.as_posix(),
            "sha256": hashlib.sha256((root / path).read_bytes()).hexdigest(),
        }
        for path in sorted(paths)
    ]


def _oracle_result(
    contract: OracleContract, observation: CommandObservation
) -> dict[str, Any]:
    return {
        "packet": contract.packet,
        "category": contract.category,
        "criterion": contract.criterion,
        "oracle": contract.oracle,
        **asdict(observation),
        "command": list(observation.command),
    }


def validate_certification_record(
    record: Mapping[str, Any],
    contract: Mapping[str, Any],
    catalog: Sequence[OracleContract],
) -> None:
    required = {
        "schema_version",
        "record_id",
        "suite",
        "plan_path",
        "state_path",
        "trusted_head",
        "generated_at",
        "certification_state",
        "release_state",
        "scope",
        "immutable_inputs",
        "environment",
        "oracle_results",
        "limitations",
        "independent_review",
    }
    _expect_keys(record, required, "certification record")
    if (
        record.get("schema_version") != 1
        or record.get("suite") != contract["suite"]
        or record.get("plan_path") != contract["plan_path"]
        or record.get("state_path") != contract["state_path"]
        or not HEAD.fullmatch(str(record.get("trusted_head", "")))
    ):
        raise SuccessorCertificationError("certification record identity differs")
    certification_state = record.get("certification_state")
    release_state = record.get("release_state")
    if (certification_state, release_state) not in {
        ("architecture_certified", "eligible"),
        (
            "architecture_certified_profile_limited",
            "blocked_by_unavailable_profile",
        ),
    }:
        raise SuccessorCertificationError("architecture and release states disagree")
    scope = record.get("scope")
    if not isinstance(scope, dict) or scope != {
        "packets": list(EXPECTED_PACKETS),
        "excluded_packets": ["WP28"],
        "oracle_count": 56,
        "required_milestones": list(EXPECTED_MILESTONES),
        "excluded_milestones": ["M01"],
        "required_decommission_batches": list(EXPECTED_BATCHES),
    }:
        raise SuccessorCertificationError("certification record scope differs")
    results = record.get("oracle_results")
    if not isinstance(results, list) or len(results) != 56:
        raise SuccessorCertificationError(
            "certification record must contain 56 results"
        )
    immutable_inputs = record.get("immutable_inputs")
    if not isinstance(immutable_inputs, list) or len(immutable_inputs) < 4:
        raise SuccessorCertificationError("immutable input identities are incomplete")
    input_paths: set[str] = set()
    for item in immutable_inputs:
        if not isinstance(item, dict) or set(item) != {"path", "sha256"}:
            raise SuccessorCertificationError("immutable input identity is malformed")
        path = item.get("path")
        if (
            not isinstance(path, str)
            or not path
            or path in input_paths
            or not SHA256.fullmatch(str(item.get("sha256", "")))
        ):
            raise SuccessorCertificationError("immutable input identity is malformed")
        input_paths.add(path)
    environment = record.get("environment")
    if not isinstance(environment, dict) or set(environment) != {
        "platform",
        "domains",
        "host_profiles",
    }:
        raise SuccessorCertificationError("certification environment is malformed")
    if environment.get("domains") != list(EXPECTED_DOMAINS):
        raise SuccessorCertificationError("certification environment domains differ")
    host_profiles = environment.get("host_profiles")
    if not isinstance(host_profiles, list) or not host_profiles:
        raise SuccessorCertificationError("host profile observations are absent")
    for profile in host_profiles:
        if not isinstance(profile, dict) or set(profile) != {
            "profile",
            "availability",
            "admission",
            "architecture_effect",
        }:
            raise SuccessorCertificationError("host profile observation is malformed")
        if profile.get("admission") != "fail_closed" or profile.get(
            "availability"
        ) not in {"available", "unavailable"}:
            raise SuccessorCertificationError("host profile does not fail closed")
    expected = {
        (item.packet, item.category, item.criterion, item.oracle) for item in catalog
    }
    observed: set[tuple[str, str, str, str]] = set()
    for result in results:
        if not isinstance(result, dict):
            raise SuccessorCertificationError("oracle result must be an object")
        if set(result) != {
            "packet",
            "category",
            "criterion",
            "oracle",
            "command",
            "exit_code",
            "selected_test_count",
            "elapsed_ms",
            "resource_summary",
            "output_sha256",
        }:
            raise SuccessorCertificationError("oracle result fields differ")
        identity = tuple(
            str(result.get(key))
            for key in ("packet", "category", "criterion", "oracle")
        )
        observed.add(identity)  # type: ignore[arg-type]
        if result.get("exit_code") != 0:
            raise SuccessorCertificationError(f"nonzero oracle result: {identity}")
        command = result.get("command")
        if (
            not isinstance(command, list)
            or not command
            or not all(isinstance(value, str) and value for value in command)
        ):
            raise SuccessorCertificationError("oracle command is malformed")
        elapsed = result.get("elapsed_ms")
        selected = result.get("selected_test_count")
        if (
            not isinstance(elapsed, int)
            or elapsed < 0
            or (
                selected is not None and (not isinstance(selected, int) or selected < 0)
            )
        ):
            raise SuccessorCertificationError("oracle execution summary is malformed")
        resources = result.get("resource_summary")
        if not isinstance(resources, dict) or set(resources) != {
            "child_user_cpu_ms",
            "child_system_cpu_ms",
            "children_ru_maxrss",
            "ru_maxrss_unit",
        }:
            raise SuccessorCertificationError("oracle resource summary is malformed")
        if not SHA256.fullmatch(str(result.get("output_sha256", ""))):
            raise SuccessorCertificationError("oracle output identity is malformed")
    if observed != expected or len(observed) != 56:
        raise SuccessorCertificationError(
            "oracle result identities differ from the plan"
        )
    review = record.get("independent_review")
    if not isinstance(review, dict) or review.get("verdict") not in {
        "approved",
        "approved-with-minor-findings",
    }:
        raise SuccessorCertificationError("independent review is not accepted")
    _reject_forbidden_semantic_acceptance(record)


def execute_certification(
    root: Path = ROOT,
    *,
    runner: RecipeRunner | None = None,
    require_prerequisites: bool = True,
) -> dict[str, Any]:
    """Execute 52 predecessor oracles plus four non-recursive WP42 surfaces."""

    contract = load_contract(root)
    catalog = derive_oracle_catalog(root, contract)
    validate_live_recipe_inventory(root, contract)
    validate_definition_scope(root, catalog)
    if require_prerequisites:
        head, review = validate_provenance_state(root, phase="candidate")
    else:
        head = _git(root, "rev-parse", "HEAD")
        review = ("synthetic-review", "approved")
    actual_runner = (
        (lambda recipe: run_recipe(root, recipe)) if runner is None else runner
    )
    observations: dict[str, CommandObservation] = {}
    for item in catalog:
        if item.packet != "WP42":
            observations[item.oracle] = actual_runner(item.oracle)

    zero, (domain, host) = (
        execute_final_zero_state(root, runner=actual_runner),
        execute_four_domain_release(root, runner=actual_runner),
    )
    observations[EXPECTED_TERMINAL_GATES["INT"]] = _summarize_internal(
        "provenance-state-integrity", {"head": head, "review": review}
    )
    observations[EXPECTED_TERMINAL_GATES["NEG"]] = _summarize_internal(
        "successor-final-zero-state",
        {key: asdict(value) for key, value in zero.items()},
    )
    observations[EXPECTED_TERMINAL_GATES["OPS"]] = _summarize_internal(
        "successor-four-domain-release",
        {
            "commands": {key: asdict(value) for key, value in domain.items()},
            "host": asdict(host),
        },
    )
    observations[EXPECTED_TERMINAL_GATES["BEH"]] = _summarize_internal(
        "relational-fabric-v3-certification",
        {"oracle_count": 56, "trusted_head": head},
    )
    results = [_oracle_result(item, observations[item.oracle]) for item in catalog]
    limited = host.availability == "unavailable"
    record = {
        "schema_version": 1,
        "record_id": f"relational-fabric-v3:{head}",
        "suite": contract["suite"],
        "plan_path": contract["plan_path"],
        "state_path": contract["state_path"],
        "trusted_head": head,
        "generated_at": datetime.now(UTC).isoformat(),
        "certification_state": (
            "architecture_certified_profile_limited"
            if limited
            else "architecture_certified"
        ),
        "release_state": "blocked_by_unavailable_profile" if limited else "eligible",
        "scope": {
            "packets": list(EXPECTED_PACKETS),
            "excluded_packets": ["WP28"],
            "oracle_count": 56,
            "required_milestones": list(EXPECTED_MILESTONES),
            "excluded_milestones": ["M01"],
            "required_decommission_batches": list(EXPECTED_BATCHES),
        },
        "immutable_inputs": _immutable_inputs(root, contract),
        "environment": {
            "platform": platform.platform(),
            "domains": list(EXPECTED_DOMAINS),
            "host_profiles": [
                {
                    "profile": host.profile,
                    "availability": host.availability,
                    "admission": host.admission,
                    "architecture_effect": host.architecture_effect,
                }
            ],
        },
        "oracle_results": results,
        "limitations": (
            [
                {
                    "limitation_id": "HOST-UNTRUSTED-CONTAINMENT",
                    "state": "unavailable",
                    "release_effect": "profile_unavailable",
                    "detail": host.detail,
                }
            ]
            if limited
            else []
        ),
        "independent_review": {"path": review[0], "verdict": review[1]},
    }
    validate_certification_record(record, contract, catalog)
    return record


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("contract-integrity")
    provenance = subparsers.add_parser("provenance-state-integrity")
    provenance.add_argument(
        "--phase", choices=("candidate", "final"), default="candidate"
    )
    subparsers.add_parser("final-zero-state")
    subparsers.add_parser("four-domain-release")
    certify = subparsers.add_parser("certify")
    certify.add_argument(
        "--output",
        type=Path,
        default=Path("target/wp42/relational-fabric-v3-certification.json"),
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "contract-integrity":
            catalog = validate_contract_integrity(ROOT)
            print(
                "successor certification contract: "
                f"{len(EXPECTED_PACKETS)} packets, {len(catalog)} oracles, WP28/M01 excluded"
            )
        elif args.command == "provenance-state-integrity":
            head, review = validate_provenance_state(ROOT, phase=args.phase)
            print(
                f"successor provenance/state: {args.phase} at {head}, review={review[1]}"
            )
        elif args.command == "final-zero-state":
            observations = execute_final_zero_state(ROOT)
            print(f"successor final zero state: {len(observations)} live gates")
        elif args.command == "four-domain-release":
            observations, host = execute_four_domain_release(ROOT)
            print(
                "successor four-domain release: "
                f"{len(observations)} live gates, host_profile={host.availability}"
            )
        else:
            record = execute_certification(ROOT)
            output = args.output if args.output.is_absolute() else ROOT / args.output
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
            print(
                f"relational fabric v3 certification: {len(record['oracle_results'])} "
                f"oracles -> {output.relative_to(ROOT)}"
            )
    except (
        OSError,
        SuccessorCertificationError,
        subprocess.CalledProcessError,
        plan_assurance.PlanAssuranceError,
    ) as error:
        print(f"successor certification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
