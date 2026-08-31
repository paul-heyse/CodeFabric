"""Validate WP22's independent relational-fabric evidence boundary.

The validators are intentionally read-only.  They validate decoded expectations,
content-addressed review, plan ordering, producer/oracle independence, and the frozen
legacy comparator contract.  They never generate, accept, or repair evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_ROOT = Path("contracts/acceptance/relational-fabric-v1")
CORPUS_PATH = EVIDENCE_ROOT / "expectations.json"
SCHEMA_PATH = EVIDENCE_ROOT / "expectation.schema.json"
TRANSACTION_PATH = EVIDENCE_ROOT / "acceptance-transaction.json"
COMPARATOR_PATH = EVIDENCE_ROOT / "comparator-manifest.json"
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md"
)

SHA256 = re.compile(r"[0-9a-f]{64}")
GIT_OBJECT = re.compile(r"[0-9a-f]{40}")
PACKET_HEADING = re.compile(r"^### (WP\d{2}) — .+$", re.MULTILINE)
PACKET_ID = re.compile(r"WP\d{2}")

REQUIRED_CATEGORIES = frozenset(
    {
        "bootstrap_model_semantics",
        "tree_sitter_exact_api",
        "ruff_exact_api",
        "pyrefly_exact_api",
        "rustc_public_exact_api",
        "rustc_private_exact_api",
        "provider_failure_remainder",
        "normalization",
        "python_derived_analysis",
        "rust_derived_analysis",
        "common_derived_analysis",
        "query_form",
        "public_lifecycle",
        "authorization_redaction",
        "hostile_rust_compilation",
        "activation_recovery",
        "legacy_revocation",
        "resource_performance",
    }
)
QUERY_FORMS = frozenset(
    {
        "FIND_ENTITIES",
        "RETRIEVE_FACTS",
        "FOLLOW_RELATIONSHIPS",
        "FIND_PATHS",
        "MATCH_PATTERN",
        "COMBINE_RESULTS",
        "SUMMARIZE_FACTS",
        "RETRIEVE_SOURCE_CONTEXT",
    }
)
REQUIRED_RESULT_ROLES = frozenset(
    {
        "entities",
        "facts",
        "paths",
        "pattern_bindings",
        "groups",
        "summary",
        "source_contexts",
    }
)
REQUIRED_ANALYSIS_FAMILIES = frozenset(
    {
        "python.cfg",
        "python.evaluation_order",
        "python.reaching_definitions",
        "python.liveness",
        "python.value_flow",
        "python.alias_points_to",
        "python.exceptional_flow",
        "python.effects",
        "python.resources",
        "python.async_flow",
        "python.pattern_flow",
        "rust.mir_access",
        "rust.ownership_state",
        "rust.reaching_definitions",
        "rust.liveness",
        "rust.alias_points_to",
        "rust.drop_resource",
        "rust.async_lowering",
        "rust.unsafe_ffi",
        "rust.call_resolution",
        "common.dominance",
        "common.post_dominance",
        "common.reachability",
        "common.scc",
        "common.call_graph",
        "common.control_dependence",
        "common.summaries",
        "common.interprocedural_effects",
        "common.interprocedural_resources",
        "common.metrics",
    }
)
PROVIDER_CATEGORIES = frozenset(
    {
        "tree_sitter_exact_api",
        "ruff_exact_api",
        "pyrefly_exact_api",
        "rustc_public_exact_api",
        "rustc_private_exact_api",
    }
)
PUBLIC_CATEGORIES = frozenset(
    {"query_form", "public_lifecycle", "authorization_redaction"}
)
ALLOWED_SOURCE_ROOTS = (
    "docs/designs/",
    "docs/authoritative_design/",
    "docs/library_ref/",
    "contracts/query/",
    "contracts/rpc/",
    "contracts/schema/",
    "contracts/security/",
    "contracts/faults/",
    "contracts/fixtures/",
    "contracts/toolchain/",
)
FORBIDDEN_PROVENANCE_ROOTS = (
    "src/",
    "target/",
    "tests/golden/",
    "contracts/generated/",
    "codefabric-cpg-mcp/src/",
    "pyrefly-sidecar/src/",
    "rustc-extractor/src/",
)
AUTHORING_SCAN_ROOTS = (
    Path("src"),
    Path("codefabric-cpg-mcp/src"),
    Path("pyrefly-sidecar/src"),
    Path("rustc-extractor/src"),
    Path("scripts"),
    Path(".github"),
    Path("tooling"),
)
SCAN_SUFFIXES = frozenset(
    {".py", ".rs", ".sh", ".toml", ".yaml", ".yml", ".json", ".md"}
)


class EvidenceContractError(ValueError):
    """The independent-evidence boundary is incomplete, stale, or unsafe."""


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceContractError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceContractError(f"missing or invalid {context}: {path}") from error
    if not isinstance(value, dict):
        raise EvidenceContractError(f"{context} must be a JSON object: {path}")
    return value


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    actual = set(value)
    if actual != expected:
        raise EvidenceContractError(
            f"{context} keys differ: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )


def _nonempty_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceContractError(f"{context} must be a non-empty string")
    return value


def _string_list(value: object, context: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise EvidenceContractError(f"{context} must be a non-empty list")
    result = [_nonempty_string(item, f"{context} item") for item in value]
    if len(result) != len(set(result)):
        raise EvidenceContractError(f"{context} contains duplicates")
    return result


def _safe_relative(value: object, context: str) -> str:
    text = _nonempty_string(value, context)
    path = PurePosixPath(text)
    if (
        path.is_absolute()
        or ".." in path.parts
        or "\\" in text
        or "\x00" in text
        or str(path) != text
    ):
        raise EvidenceContractError(f"{context} is not repository-relative: {text}")
    return text


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise EvidenceContractError(f"cannot read bound input: {path}") from error
    return digest.hexdigest()


def _canonical_sha256(value: object) -> str:
    payload = json.dumps(
        value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return _sha256_bytes(payload)


def _git(root: Path, *arguments: str, check: bool = True) -> str:
    completed = subprocess.run(
        ("git", *arguments),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if check and completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise EvidenceContractError(
            f"git {' '.join(arguments)} failed: {detail or completed.returncode}"
        )
    return completed.stdout.strip()


def _validate_schema(root: Path) -> None:
    schema = _load_json(root / SCHEMA_PATH, "expectation schema")
    for key in ("$schema", "$id", "type", "additionalProperties", "required"):
        if key not in schema:
            raise EvidenceContractError(f"expectation schema lacks {key}")
    if schema["type"] != "object" or schema["additionalProperties"] is not False:
        raise EvidenceContractError("expectation schema root is not strict")
    definitions = schema.get("$defs")
    if not isinstance(definitions, dict) or not definitions:
        raise EvidenceContractError("expectation schema has no strict definitions")
    for name, definition in definitions.items():
        if not isinstance(definition, dict):
            raise EvidenceContractError(f"schema definition {name} is not an object")
        if (
            definition.get("type") == "object"
            and definition.get("additionalProperties") is not False
        ):
            raise EvidenceContractError(f"schema definition {name} is not strict")


def _validate_sources(
    root: Path, sources: object
) -> tuple[dict[str, dict[str, Any]], set[str]]:
    if not isinstance(sources, list) or not sources:
        raise EvidenceContractError("sources must be a non-empty list")
    by_id: dict[str, dict[str, Any]] = {}
    kinds: set[str] = set()
    for index, item in enumerate(sources):
        if not isinstance(item, dict):
            raise EvidenceContractError(f"source[{index}] must be an object")
        _strict_keys(
            item,
            {"source_id", "kind", "path", "sha256", "locator", "identity"},
            f"source[{index}]",
        )
        source_id = _nonempty_string(item["source_id"], "source_id")
        if source_id in by_id:
            raise EvidenceContractError(f"duplicate source_id: {source_id}")
        kind = _nonempty_string(item["kind"], f"{source_id}.kind")
        if kind not in {
            "normative_design",
            "upstream_api",
            "released_public_contract",
            "independent_fixture",
            "security_contract",
            "toolchain_contract",
        }:
            raise EvidenceContractError(f"{source_id}: unsupported source kind {kind}")
        relative = _safe_relative(item["path"], f"{source_id}.path")
        if relative.startswith(FORBIDDEN_PROVENANCE_ROOTS):
            raise EvidenceContractError(
                f"{source_id}: producer or generated output is forbidden provenance: {relative}"
            )
        if not relative.startswith(ALLOWED_SOURCE_ROOTS):
            raise EvidenceContractError(
                f"{source_id}: source path is outside the evidence allowlist: {relative}"
            )
        digest = _nonempty_string(item["sha256"], f"{source_id}.sha256")
        if SHA256.fullmatch(digest) is None:
            raise EvidenceContractError(f"{source_id}: sha256 is malformed")
        actual = _sha256_file(root / relative)
        if actual != digest:
            raise EvidenceContractError(
                f"{source_id}: source digest drift: expected {digest}, found {actual}"
            )
        _nonempty_string(item["locator"], f"{source_id}.locator")
        _nonempty_string(item["identity"], f"{source_id}.identity")
        by_id[source_id] = item
        kinds.add(kind)
    return by_id, kinds


def _validate_owner(item: object, index: int) -> tuple[str, str]:
    if not isinstance(item, dict):
        raise EvidenceContractError(f"owner[{index}] must be an object")
    _strict_keys(
        item,
        {"owner_id", "role", "independence_class", "accountability"},
        f"owner[{index}]",
    )
    owner_id = _nonempty_string(item["owner_id"], f"owner[{index}].owner_id")
    independence = _nonempty_string(
        item["independence_class"], f"owner[{index}].independence_class"
    )
    if independence != "independent-from-production-producers":
        raise EvidenceContractError(f"{owner_id}: owner is not independent")
    _nonempty_string(item["role"], f"{owner_id}.role")
    _nonempty_string(item["accountability"], f"{owner_id}.accountability")
    return owner_id, independence


def _validate_relation(relation: object, context: str) -> None:
    if not isinstance(relation, dict):
        raise EvidenceContractError(f"{context} must be an object")
    _strict_keys(relation, {"relation", "columns", "rows", "ordering"}, context)
    _nonempty_string(relation["relation"], f"{context}.relation")
    columns = _string_list(relation["columns"], f"{context}.columns")
    ordering = _string_list(relation["ordering"], f"{context}.ordering")
    if not set(ordering).issubset(columns):
        raise EvidenceContractError(f"{context}: ordering references unknown columns")
    rows = relation["rows"]
    if not isinstance(rows, list) or not rows:
        raise EvidenceContractError(f"{context}.rows must be non-empty")
    for row_index, row in enumerate(rows):
        if not isinstance(row, dict) or set(row) != set(columns):
            raise EvidenceContractError(
                f"{context}.rows[{row_index}] keys must exactly match columns"
            )
        for value in row.values():
            if isinstance(value, (dict, list)):
                raise EvidenceContractError(
                    f"{context}.rows[{row_index}] must contain decoded scalar cells"
                )


def _validate_control(control: object, context: str) -> str:
    if not isinstance(control, dict):
        raise EvidenceContractError(f"{context} must be an object")
    _strict_keys(
        control,
        {"control_id", "intervention", "expected_discrimination", "unknown_passes"},
        context,
    )
    control_id = _nonempty_string(control["control_id"], f"{context}.control_id")
    _nonempty_string(control["intervention"], f"{context}.intervention")
    _nonempty_string(
        control["expected_discrimination"], f"{context}.expected_discrimination"
    )
    if control["unknown_passes"] is not False:
        raise EvidenceContractError(f"{control_id}: unknown may not satisfy evidence")
    return control_id


def validate_corpus(root: Path = ROOT) -> dict[str, Any]:
    """Validate the decoded corpus and every currently bound authority input."""

    _validate_schema(root)
    corpus = _load_json(root / CORPUS_PATH, "expectation corpus")
    _strict_keys(
        corpus,
        {
            "schema_version",
            "corpus_id",
            "corpus_version",
            "status",
            "purpose",
            "owners",
            "sources",
            "global_limitations",
            "expectations",
        },
        "expectation corpus",
    )
    if corpus["schema_version"] != 1:
        raise EvidenceContractError("unsupported expectation schema version")
    for field in ("corpus_id", "corpus_version", "purpose"):
        _nonempty_string(corpus[field], field)
    if corpus["status"] != "review-candidate":
        raise EvidenceContractError(
            "corpus status must remain review-candidate until accepted"
        )
    _string_list(corpus["global_limitations"], "global_limitations")

    owners = corpus["owners"]
    if not isinstance(owners, list) or not owners:
        raise EvidenceContractError("owners must be a non-empty list")
    owner_ids = {_validate_owner(owner, index)[0] for index, owner in enumerate(owners)}
    if len(owner_ids) != len(owners):
        raise EvidenceContractError("duplicate owner_id")

    sources, _ = _validate_sources(root, corpus["sources"])
    expectations = corpus["expectations"]
    if not isinstance(expectations, list) or not expectations:
        raise EvidenceContractError("expectations must be a non-empty list")

    expectation_ids: set[str] = set()
    control_ids: set[str] = set()
    categories: set[str] = set()
    query_forms: set[str] = set()
    result_roles: set[str] = set()
    analysis_families: set[str] = set()
    for index, item in enumerate(expectations):
        context = f"expectation[{index}]"
        if not isinstance(item, dict):
            raise EvidenceContractError(f"{context} must be an object")
        _strict_keys(
            item,
            {
                "expectation_id",
                "category",
                "subject",
                "query_form",
                "analysis_families",
                "owner_id",
                "rationale",
                "limitations",
                "input",
                "expected",
                "causal_controls",
                "source_refs",
            },
            context,
        )
        expectation_id = _nonempty_string(
            item["expectation_id"], f"{context}.expectation_id"
        )
        if expectation_id in expectation_ids:
            raise EvidenceContractError(f"duplicate expectation_id: {expectation_id}")
        expectation_ids.add(expectation_id)
        category = _nonempty_string(item["category"], f"{context}.category")
        if category not in REQUIRED_CATEGORIES:
            raise EvidenceContractError(
                f"{expectation_id}: unknown category {category}"
            )
        categories.add(category)
        _nonempty_string(item["subject"], f"{expectation_id}.subject")
        if item["owner_id"] not in owner_ids:
            raise EvidenceContractError(f"{expectation_id}: unknown owner")
        _nonempty_string(item["rationale"], f"{expectation_id}.rationale")
        _string_list(item["limitations"], f"{expectation_id}.limitations")

        query_form = item["query_form"]
        if category == "query_form":
            if query_form not in QUERY_FORMS:
                raise EvidenceContractError(
                    f"{expectation_id}: missing or invalid query form"
                )
            query_forms.add(query_form)
        elif query_form is not None:
            raise EvidenceContractError(f"{expectation_id}: unexpected query_form")

        families = item["analysis_families"]
        if not isinstance(families, list):
            raise EvidenceContractError(
                f"{expectation_id}.analysis_families must be a list"
            )
        for family in families:
            analysis_families.add(
                _nonempty_string(family, f"{expectation_id}.analysis_family")
            )
        if category.endswith("derived_analysis") and not families:
            raise EvidenceContractError(
                f"{expectation_id}: derived families are absent"
            )
        if not category.endswith("derived_analysis") and families:
            raise EvidenceContractError(
                f"{expectation_id}: unexpected analysis families"
            )

        input_value = item["input"]
        if not isinstance(input_value, dict):
            raise EvidenceContractError(f"{expectation_id}.input must be decoded JSON")
        _strict_keys(input_value, {"fixture_id", "decoded"}, f"{expectation_id}.input")
        _nonempty_string(input_value["fixture_id"], f"{expectation_id}.fixture_id")
        if not isinstance(input_value["decoded"], dict) or not input_value["decoded"]:
            raise EvidenceContractError(
                f"{expectation_id}.decoded input must be non-empty"
            )

        expected = item["expected"]
        if not isinstance(expected, dict):
            raise EvidenceContractError(f"{expectation_id}.expected must be an object")
        _strict_keys(expected, {"terminal", "relations"}, f"{expectation_id}.expected")
        if expected["terminal"] not in {"pass", "reject", "unknown"}:
            raise EvidenceContractError(f"{expectation_id}: invalid terminal")
        relations = expected["relations"]
        if not isinstance(relations, list) or not relations:
            raise EvidenceContractError(
                f"{expectation_id}: expected relations are absent"
            )
        for relation_index, relation in enumerate(relations):
            _validate_relation(
                relation, f"{expectation_id}.relations[{relation_index}]"
            )
            if category == "query_form" and "output_role" in relation["columns"]:
                result_roles.update(str(row["output_role"]) for row in relation["rows"])

        controls = item["causal_controls"]
        if not isinstance(controls, list) or not controls:
            raise EvidenceContractError(f"{expectation_id}: causal controls are absent")
        for control_index, control in enumerate(controls):
            control_id = _validate_control(
                control, f"{expectation_id}.controls[{control_index}]"
            )
            if control_id in control_ids:
                raise EvidenceContractError(f"duplicate causal control: {control_id}")
            control_ids.add(control_id)

        refs = _string_list(item["source_refs"], f"{expectation_id}.source_refs")
        missing = set(refs) - set(sources)
        if missing:
            raise EvidenceContractError(
                f"{expectation_id}: unknown source refs {sorted(missing)}"
            )
        kinds = {sources[ref]["kind"] for ref in refs}
        if "normative_design" not in kinds:
            raise EvidenceContractError(
                f"{expectation_id}: no normative design provenance"
            )
        if category in PROVIDER_CATEGORIES and "upstream_api" not in kinds:
            raise EvidenceContractError(
                f"{expectation_id}: no exact upstream API provenance"
            )
        if category in PUBLIC_CATEGORIES and "released_public_contract" not in kinds:
            raise EvidenceContractError(
                f"{expectation_id}: no released public provenance"
            )

    missing_categories = REQUIRED_CATEGORIES - categories
    if missing_categories:
        raise EvidenceContractError(
            f"expectation categories are incomplete: {sorted(missing_categories)}"
        )
    if query_forms != QUERY_FORMS:
        raise EvidenceContractError(
            f"query form coverage differs: missing={sorted(QUERY_FORMS - query_forms)}, "
            f"extra={sorted(query_forms - QUERY_FORMS)}"
        )
    if result_roles != REQUIRED_RESULT_ROLES:
        raise EvidenceContractError(
            "query composition/result roles differ: "
            f"missing={sorted(REQUIRED_RESULT_ROLES - result_roles)}, "
            f"extra={sorted(result_roles - REQUIRED_RESULT_ROLES)}"
        )
    if analysis_families != REQUIRED_ANALYSIS_FAMILIES:
        raise EvidenceContractError(
            "derived-analysis coverage differs: "
            f"missing={sorted(REQUIRED_ANALYSIS_FAMILIES - analysis_families)}, "
            f"extra={sorted(analysis_families - REQUIRED_ANALYSIS_FAMILIES)}"
        )
    return corpus


def validate_review_transaction(
    root: Path = ROOT, *, require_accepted: bool = True
) -> dict[str, Any]:
    """Validate the immutable content-addressed review transaction."""

    transaction = _load_json(root / TRANSACTION_PATH, "acceptance transaction")
    _strict_keys(
        transaction,
        {
            "schema_version",
            "transaction_id",
            "evidence_set_id",
            "candidate",
            "decision",
            "reviewer",
            "reviewed_at",
            "review_basis",
            "independence_attestation",
            "authoring_boundary",
            "successor_of",
            "invalidation_policy",
            "blocking_conditions",
        },
        "acceptance transaction",
    )
    if transaction["schema_version"] != 1:
        raise EvidenceContractError("unsupported acceptance transaction schema")
    candidate = transaction["candidate"]
    if not isinstance(candidate, dict):
        raise EvidenceContractError("candidate binding must be an object")
    _strict_keys(
        candidate,
        {
            "corpus_path",
            "corpus_sha256",
            "schema_path",
            "schema_sha256",
            "comparator_path",
            "comparator_sha256",
        },
        "candidate binding",
    )
    for name in ("corpus", "schema", "comparator"):
        relative = _safe_relative(candidate[f"{name}_path"], f"{name}_path")
        expected = _nonempty_string(candidate[f"{name}_sha256"], f"{name}_sha256")
        if SHA256.fullmatch(expected) is None:
            raise EvidenceContractError(f"{name} digest is malformed")
        actual = _sha256_file(root / relative)
        if actual != expected:
            raise EvidenceContractError(
                f"{name} candidate drift: expected {expected}, found {actual}"
            )
    evidence_set_id = _nonempty_string(
        transaction["evidence_set_id"], "evidence_set_id"
    )
    candidate_identity = _canonical_sha256(candidate)
    if evidence_set_id != f"sha256:{candidate_identity}":
        raise EvidenceContractError("evidence_set_id does not bind candidate files")
    transaction_projection = dict(transaction)
    transaction_id = _nonempty_string(
        transaction_projection.pop("transaction_id"), "transaction_id"
    )
    actual_transaction_id = _canonical_sha256(transaction_projection)
    if transaction_id != f"sha256:{actual_transaction_id}":
        raise EvidenceContractError(
            "transaction_id does not bind the review transaction"
        )
    if transaction["decision"] not in {"accepted", "rejected"}:
        raise EvidenceContractError("review decision must be accepted or rejected")
    reviewer = transaction["reviewer"]
    if not isinstance(reviewer, dict):
        raise EvidenceContractError("reviewer must be an object")
    _strict_keys(
        reviewer,
        {"reviewer_id", "role", "independence_class", "accountability"},
        "reviewer",
    )
    for field in ("reviewer_id", "role", "accountability"):
        _nonempty_string(reviewer[field], f"reviewer.{field}")
    if reviewer["independence_class"] != "independent-from-production-producers":
        raise EvidenceContractError("reviewer is not independent")
    _nonempty_string(transaction["reviewed_at"], "reviewed_at")
    _string_list(transaction["review_basis"], "review_basis")
    _nonempty_string(
        transaction["independence_attestation"], "independence_attestation"
    )
    _nonempty_string(transaction["invalidation_policy"], "invalidation_policy")
    if transaction["successor_of"] is not None:
        _nonempty_string(transaction["successor_of"], "successor_of")
    if not isinstance(transaction["blocking_conditions"], list):
        raise EvidenceContractError("blocking_conditions must be a list")
    for condition in transaction["blocking_conditions"]:
        _nonempty_string(condition, "blocking condition")
    _validate_authoring_boundary(root, transaction, require_commits=False)
    if require_accepted and transaction["decision"] != "accepted":
        conditions = "; ".join(transaction["blocking_conditions"])
        raise EvidenceContractError(
            f"evidence review is {transaction['decision']}: {conditions}"
        )
    if require_accepted and transaction["blocking_conditions"]:
        raise EvidenceContractError("accepted evidence retains blocking conditions")
    return transaction


def _parse_packet_dependencies(plan_text: str) -> dict[str, set[str]]:
    matches = list(PACKET_HEADING.finditer(plan_text))
    if not matches:
        raise EvidenceContractError("plan contains no WP packet headings")
    dependencies: dict[str, set[str]] = {}
    for index, match in enumerate(matches):
        packet = match.group(1)
        end = matches[index + 1].start() if index + 1 < len(matches) else len(plan_text)
        body = plan_text[match.end() : end]
        dependency_match = re.search(
            r"\*\*Dependencies\.\*\*\s*\n\n(?P<body>.*?)(?=\n\n\*\*)",
            body,
            re.DOTALL,
        )
        if dependency_match is None:
            raise EvidenceContractError(f"{packet}: Dependencies section is absent")
        # The plan places the normative dependency expression in the first
        # sentence.  Later sentences may discuss packet ownership or explicitly
        # say that something is *not* a dependency (WP01 does this), so scanning
        # the whole paragraph would manufacture edges.
        first_sentence = dependency_match.group("body").split(".", maxsplit=1)[0]
        found = set(PACKET_ID.findall(first_sentence))
        if packet in found:
            raise EvidenceContractError(f"{packet}: self dependency")
        dependencies[packet] = found
    unknown = {
        dependency
        for packet_dependencies in dependencies.values()
        for dependency in packet_dependencies
        if dependency not in dependencies
    }
    if unknown:
        raise EvidenceContractError(f"plan has unknown dependencies: {sorted(unknown)}")
    return dependencies


def _ancestors(
    packet: str,
    dependencies: Mapping[str, set[str]],
    visiting: set[str] | None = None,
) -> set[str]:
    path = set() if visiting is None else set(visiting)
    if packet in path:
        raise EvidenceContractError(f"dependency cycle reaches {packet}")
    path.add(packet)
    result: set[str] = set()
    for dependency in dependencies[packet]:
        result.add(dependency)
        result.update(_ancestors(dependency, dependencies, path))
    return result


def validate_independent_evidence_dag(plan_path: Path) -> None:
    """Prove WP22 precedes every implementation consumer in the parsed plan DAG."""

    try:
        text = plan_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise EvidenceContractError(f"cannot read plan: {plan_path}") from error
    dependencies = _parse_packet_dependencies(text)
    if dependencies.get("WP22") != {"WP01"}:
        raise EvidenceContractError("WP22 must depend on WP01 and no later packet")
    consumers = set(dependencies) - {"WP01", "WP22"}
    missing = sorted(
        packet for packet in consumers if "WP22" not in _ancestors(packet, dependencies)
    )
    if missing:
        raise EvidenceContractError(
            f"implementation consumers lack transitive WP22 dependency: {missing}"
        )


def _iter_authoring_files(root: Path) -> Iterable[Path]:
    for relative_root in AUTHORING_SCAN_ROOTS:
        base = root / relative_root
        if not base.exists():
            continue
        for directory, names, files in os.walk(base):
            names[:] = sorted(
                name
                for name in names
                if name not in {".git", "target", ".venv", "__pycache__"}
            )
            for name in sorted(files):
                path = Path(directory) / name
                if path.suffix in SCAN_SUFFIXES or name == "justfile":
                    yield path


def validate_expectation_independence(root: Path = ROOT) -> None:
    """Reject producer provenance and production-to-expectation authoring edges."""

    validate_corpus(root)
    literal = EVIDENCE_ROOT.as_posix()
    module = "tooling.ci.relational_fabric_evidence"
    allowed = {
        (root / "tooling/ci/relational_fabric_evidence.py").resolve(),
        (root / "tooling/ci/test_relational_fabric_evidence.py").resolve(),
    }
    violations: list[str] = []
    for path in _iter_authoring_files(root):
        if path.resolve() in allowed:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            continue
        if literal in text or module in text:
            violations.append(path.relative_to(root).as_posix())
    if violations:
        raise EvidenceContractError(
            "production or general tooling references the expectation ingress: "
            f"{violations}"
        )


def _validate_authoring_boundary(
    root: Path, transaction: Mapping[str, Any], *, require_commits: bool
) -> None:
    boundary = transaction["authoring_boundary"]
    if not isinstance(boundary, dict):
        raise EvidenceContractError("authoring_boundary must be an object")
    _strict_keys(
        boundary,
        {
            "required_predecessor_packet",
            "required_before_packet",
            "evidence_freeze_commit",
            "consumer_state_path",
            "status",
        },
        "authoring_boundary",
    )
    if boundary["required_predecessor_packet"] != "WP01":
        raise EvidenceContractError("evidence predecessor boundary is not WP01")
    if boundary["required_before_packet"] != "WP02":
        raise EvidenceContractError("evidence consumer boundary is not WP02")
    if boundary["status"] not in {"proved-early", "unproved-late"}:
        raise EvidenceContractError("invalid authoring boundary status")
    freeze = boundary["evidence_freeze_commit"]
    state_path = boundary["consumer_state_path"]
    if not require_commits and freeze is None:
        return
    if not isinstance(freeze, str) or GIT_OBJECT.fullmatch(freeze) is None:
        raise EvidenceContractError("evidence_freeze_commit is absent or malformed")
    relative_state = _safe_relative(state_path, "consumer_state_path")
    if boundary["status"] != "proved-early":
        raise EvidenceContractError("evidence is explicitly marked late/unproved")
    for candidate_key in ("corpus_path", "schema_path", "comparator_path"):
        relative = transaction["candidate"][candidate_key]
        payload = subprocess.run(
            ("git", "show", f"{freeze}:{relative}"),
            cwd=root,
            check=False,
            capture_output=True,
        )
        if payload.returncode != 0:
            raise EvidenceContractError(
                f"{relative} did not exist at the evidence freeze commit"
            )
        expected = transaction["candidate"][candidate_key.replace("path", "sha256")]
        if _sha256_bytes(payload.stdout) != expected:
            raise EvidenceContractError(
                f"{relative} differs from the accepted freeze bytes"
            )

    transaction_digest = _sha256_file(root / TRANSACTION_PATH)
    acceptance_commit: str | None = None
    for commit in _git(
        root, "log", "--format=%H", "--", TRANSACTION_PATH.as_posix()
    ).splitlines():
        payload = subprocess.run(
            ("git", "show", f"{commit}:{TRANSACTION_PATH.as_posix()}"),
            cwd=root,
            check=False,
            capture_output=True,
        )
        if (
            payload.returncode == 0
            and _sha256_bytes(payload.stdout) == transaction_digest
        ):
            acceptance_commit = commit
    if acceptance_commit is None:
        raise EvidenceContractError("accepted transaction bytes have no proving commit")

    state = _load_json(root / relative_state, "consumer execution state")
    packets = state.get("packets")
    if not isinstance(packets, dict) or not isinstance(packets.get("WP02"), dict):
        raise EvidenceContractError("consumer state has no WP02 judgment")
    consumer = packets["WP02"].get("proving_commit")
    if not isinstance(consumer, str) or GIT_OBJECT.fullmatch(consumer) is None:
        raise EvidenceContractError("WP02 has no proving commit for early-order proof")
    for predecessor, successor, context in (
        (freeze, acceptance_commit, "evidence freeze does not precede acceptance"),
        (acceptance_commit, consumer, "evidence acceptance does not precede WP02"),
    ):
        completed = subprocess.run(
            ("git", "merge-base", "--is-ancestor", predecessor, successor),
            cwd=root,
            check=False,
            capture_output=True,
        )
        if completed.returncode != 0:
            raise EvidenceContractError(context)


def validate_late_authoring_zero_state(root: Path = ROOT) -> None:
    """Prove accepted bytes existed in Git before the first implementation consumer."""

    transaction = validate_review_transaction(root, require_accepted=True)
    _validate_authoring_boundary(root, transaction, require_commits=True)


def validate_comparator_manifest(
    root: Path = ROOT, *, require_available: bool = True
) -> dict[str, Any]:
    """Validate exact comparator source identity and fail closed on missing capture."""

    manifest = _load_json(root / COMPARATOR_PATH, "comparator manifest")
    _strict_keys(
        manifest,
        {
            "schema_version",
            "comparator_id",
            "status",
            "historical_source",
            "toolchain",
            "frozen_inputs",
            "build",
            "artifact",
            "comparison_contract",
            "isolation",
            "limitations",
        },
        "comparator manifest",
    )
    if manifest["schema_version"] != 1:
        raise EvidenceContractError("unsupported comparator manifest schema")
    _nonempty_string(manifest["comparator_id"], "comparator_id")
    if manifest["status"] not in {"captured", "blocked-unavailable"}:
        raise EvidenceContractError("invalid comparator status")
    source = manifest["historical_source"]
    if not isinstance(source, dict):
        raise EvidenceContractError("historical_source must be an object")
    _strict_keys(
        source,
        {"repository_commit", "tree_oid", "entrypoint", "objects"},
        "historical_source",
    )
    commit = _nonempty_string(source["repository_commit"], "repository_commit")
    tree_oid = _nonempty_string(source["tree_oid"], "tree_oid")
    if GIT_OBJECT.fullmatch(commit) is None or GIT_OBJECT.fullmatch(tree_oid) is None:
        raise EvidenceContractError("historical Git identity is malformed")
    actual_tree = _git(root, "rev-parse", f"{commit}^{{tree}}")
    if actual_tree != tree_oid:
        raise EvidenceContractError("historical comparator tree identity drifted")
    _safe_relative(source["entrypoint"], "comparator entrypoint")
    objects = source["objects"]
    if not isinstance(objects, list) or not objects:
        raise EvidenceContractError("historical comparator objects are absent")
    for index, item in enumerate(objects):
        if not isinstance(item, dict):
            raise EvidenceContractError(f"historical object[{index}] must be an object")
        _strict_keys(item, {"path", "object_id"}, f"historical object[{index}]")
        relative = _safe_relative(item["path"], f"historical object[{index}].path")
        object_id = _nonempty_string(item["object_id"], f"{relative}.object_id")
        if GIT_OBJECT.fullmatch(object_id) is None:
            raise EvidenceContractError(f"{relative}: malformed Git object id")
        actual = _git(root, "rev-parse", f"{commit}:{relative}")
        if actual != object_id:
            raise EvidenceContractError(f"{relative}: frozen Git object drift")

    toolchain = manifest["toolchain"]
    if not isinstance(toolchain, dict):
        raise EvidenceContractError("toolchain must be an object")
    _strict_keys(
        toolchain,
        {
            "status",
            "rustc_verbose_version",
            "rustc_binary_sha256",
            "cargo_verbose_version",
            "cargo_binary_sha256",
            "host_triple",
            "lockfile_sha256",
        },
        "toolchain",
    )
    if toolchain["status"] not in {"exact", "unresolved"}:
        raise EvidenceContractError("invalid toolchain status")
    lock_digest = _nonempty_string(toolchain["lockfile_sha256"], "lockfile_sha256")
    if SHA256.fullmatch(lock_digest) is None:
        raise EvidenceContractError("lockfile digest is malformed")
    lock_payload = subprocess.run(
        ("git", "show", f"{commit}:Cargo.lock"),
        cwd=root,
        check=False,
        capture_output=True,
    )
    if (
        lock_payload.returncode != 0
        or _sha256_bytes(lock_payload.stdout) != lock_digest
    ):
        raise EvidenceContractError("historical Cargo.lock binding is invalid")
    if toolchain["status"] == "exact":
        for field in (
            "rustc_verbose_version",
            "cargo_verbose_version",
            "host_triple",
        ):
            _nonempty_string(toolchain[field], f"toolchain.{field}")
        for field in ("rustc_binary_sha256", "cargo_binary_sha256"):
            digest = _nonempty_string(toolchain[field], f"toolchain.{field}")
            if SHA256.fullmatch(digest) is None:
                raise EvidenceContractError(f"toolchain.{field} is malformed")

    inputs = manifest["frozen_inputs"]
    if not isinstance(inputs, list) or not inputs:
        raise EvidenceContractError("frozen comparator inputs are absent")
    for index, item in enumerate(inputs):
        if not isinstance(item, dict):
            raise EvidenceContractError(f"frozen input[{index}] must be an object")
        _strict_keys(item, {"path", "object_id", "purpose"}, f"frozen input[{index}]")
        relative = _safe_relative(item["path"], f"frozen input[{index}].path")
        _nonempty_string(item["purpose"], f"{relative}.purpose")
        expected = _nonempty_string(item["object_id"], f"{relative}.object_id")
        actual = _git(root, "rev-parse", f"{commit}:{relative}")
        if actual != expected:
            raise EvidenceContractError(f"{relative}: frozen input object drift")

    build = manifest["build"]
    if not isinstance(build, dict):
        raise EvidenceContractError("build contract must be an object")
    _strict_keys(build, {"argv", "environment", "network"}, "build")
    _string_list(build["argv"], "build.argv")
    if not isinstance(build["environment"], dict):
        raise EvidenceContractError("build.environment must be an object")
    if build["network"] != "deny":
        raise EvidenceContractError("comparator reconstruction permits network")

    artifact = manifest["artifact"]
    if not isinstance(artifact, dict):
        raise EvidenceContractError("artifact contract must be an object")
    _strict_keys(artifact, {"status", "path", "sha256"}, "artifact")
    if artifact["status"] not in {"captured", "missing"}:
        raise EvidenceContractError("invalid comparator artifact status")
    artifact_path = _safe_relative(artifact["path"], "artifact.path")
    if artifact["status"] == "captured":
        digest = _nonempty_string(artifact["sha256"], "artifact.sha256")
        if SHA256.fullmatch(digest) is None:
            raise EvidenceContractError("artifact digest is malformed")
        path = root / artifact_path
        if _sha256_file(path) != digest:
            raise EvidenceContractError("captured comparator artifact digest drift")
        if path.stat().st_mode & (stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH):
            raise EvidenceContractError("captured comparator artifact is writeable")
    elif artifact["sha256"] is not None:
        raise EvidenceContractError("missing comparator artifact has a digest")

    comparison = manifest["comparison_contract"]
    if not isinstance(comparison, dict):
        raise EvidenceContractError("comparison contract must be an object")
    _strict_keys(
        comparison,
        {
            "oracle_authority",
            "legacy_role",
            "row_semantics",
            "ordering",
            "unknown_policy",
        },
        "comparison contract",
    )
    if comparison["oracle_authority"] != "independent-expectations-only":
        raise EvidenceContractError("legacy output is incorrectly authoritative")
    if comparison["legacy_role"] != "comparison-evidence-only":
        raise EvidenceContractError("legacy comparator is incorrectly authoritative")
    for field in ("row_semantics", "ordering", "unknown_policy"):
        _nonempty_string(comparison[field], f"comparison.{field}")

    isolation = manifest["isolation"]
    if not isinstance(isolation, dict):
        raise EvidenceContractError("isolation contract must be an object")
    _strict_keys(
        isolation,
        {
            "status",
            "backend",
            "network",
            "filesystem",
            "write_allowlist",
            "read_only_inputs",
            "environment_allowlist",
            "stdout_contract",
        },
        "isolation",
    )
    if isolation["status"] not in {"enforced", "unresolved"}:
        raise EvidenceContractError("invalid isolation status")
    if isolation["network"] != "deny" or isolation["filesystem"] != "read-only":
        raise EvidenceContractError("comparator isolation is not no-network/read-only")
    if isolation["write_allowlist"] != []:
        raise EvidenceContractError(
            "comparator isolation has filesystem write authority"
        )
    _string_list(isolation["read_only_inputs"], "isolation.read_only_inputs")
    _string_list(isolation["environment_allowlist"], "isolation.environment_allowlist")
    _nonempty_string(isolation["stdout_contract"], "isolation.stdout_contract")
    _string_list(manifest["limitations"], "comparator limitations")

    if require_available:
        missing: list[str] = []
        if manifest["status"] != "captured":
            missing.append("manifest status is blocked-unavailable")
        if toolchain["status"] != "exact":
            missing.append("exact rustc/cargo bytes were not captured")
        if artifact["status"] != "captured":
            missing.append("exact comparator executable bytes were not captured")
        if isolation["status"] != "enforced" or isolation["backend"] is None:
            missing.append("no enforcing read-only/no-network backend was captured")
        if missing:
            raise EvidenceContractError("; ".join(missing))
    return manifest


def validate_comparison_engine_isolation(root: Path = ROOT) -> None:
    """Require exact immutable comparator bytes and an enforcing isolation backend."""

    validate_comparator_manifest(root, require_available=True)


def validate_legacy_comparator_reconstruction(root: Path = ROOT) -> None:
    """Require an exact reconstructible worktree, toolchain, artifact, and isolation."""

    validate_comparator_manifest(root, require_available=True)


def validate_early_evidence_acceptance(root: Path = ROOT) -> None:
    """Require complete decoded evidence, accepted review, and proved early authoring."""

    validate_corpus(root)
    validate_expectation_independence(root)
    validate_late_authoring_zero_state(root)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("early-evidence-acceptance")
    dag = subparsers.add_parser("independent-evidence-dag")
    dag.add_argument("plan", nargs="?", type=Path, default=PLAN_PATH)
    subparsers.add_parser("expectation-independence")
    subparsers.add_parser("comparison-engine-isolation")
    subparsers.add_parser("late-authoring-zero-state")
    subparsers.add_parser("legacy-comparator-reconstruction")
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(arguments)
    root = args.root.resolve()
    try:
        if args.command == "early-evidence-acceptance":
            validate_early_evidence_acceptance(root)
        elif args.command == "independent-evidence-dag":
            plan = args.plan
            if not plan.is_absolute():
                plan = root / plan
            validate_independent_evidence_dag(plan)
        elif args.command == "expectation-independence":
            validate_expectation_independence(root)
        elif args.command == "comparison-engine-isolation":
            validate_comparison_engine_isolation(root)
        elif args.command == "late-authoring-zero-state":
            validate_late_authoring_zero_state(root)
        elif args.command == "legacy-comparator-reconstruction":
            validate_legacy_comparator_reconstruction(root)
        else:  # pragma: no cover - argparse owns command closure.
            raise EvidenceContractError(f"unsupported command: {args.command}")
    except EvidenceContractError as error:
        print(f"relational-fabric evidence check failed: {error}", file=sys.stderr)
        return 1
    print(f"relational-fabric evidence check passed: {args.command}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
