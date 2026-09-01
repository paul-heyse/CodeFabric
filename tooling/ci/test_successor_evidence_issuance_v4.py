"""Focused positive and falsification tests for the WP33 v4 evidence issuance."""

from __future__ import annotations

import ast
import copy
import hashlib
import json
import shutil
from pathlib import Path

import pytest

from tooling.ci.record_wp33_v4_acceptance import record_acceptance, record_review
from tooling.ci.successor_evidence_issuance_v4 import (
    ACTIVE_PLAN_POINTER,
    AUTHORITY_ROOT,
    EVIDENCE_ROOT,
    EXPECTATIONS_PATH,
    EXPECTED_CLAIM_IDS,
    EXPECTED_FAMILIES,
    EXPECTED_FIXTURE_IDS,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    ORACLES,
    PLAN_PATH,
    PRINCIPLES_PATH,
    REQUIRED_TAGS,
    ROOT,
    SUITE_IDENTITY,
    SUITE_VERSION,
    V4EvidenceError,
    _apply_json_merge_patch,
    _canonical_sha256,
    _issuance_review_projection,
    _selector_counts,
    discover_terminal_suite,
    validate_issuance,
    validate_oracle,
)


def _load_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def _load_jsonl(path: Path) -> list[dict[str, object]]:
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    assert all(isinstance(row, dict) for row in rows)
    return rows


def _write_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(
            json.dumps(row, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
            + "\n"
            for row in rows
        ),
        encoding="utf-8",
    )


def _copy_candidate(destination: Path) -> Path:
    shutil.copytree(ROOT / AUTHORITY_ROOT, destination / AUTHORITY_ROOT)
    shutil.copytree(ROOT / EVIDENCE_ROOT, destination / EVIDENCE_ROOT)
    issuance = _load_json(ROOT / ISSUANCE_PATH)
    source_rows = issuance["source_provenance"]
    assert isinstance(source_rows, list)
    paths = {ACTIVE_PLAN_POINTER, PLAN_PATH, PRINCIPLES_PATH}
    for row in source_rows:
        assert isinstance(row, dict)
        paths.add(Path(str(row["path"])))
    for relative in paths:
        source = ROOT / relative
        target = destination / relative
        if target.exists():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def _refresh_freeze(root: Path, *, bind_review: bool = True) -> None:
    issuance_path = root / ISSUANCE_PATH
    issuance = _load_json(issuance_path)
    artifact_digests = issuance["artifact_digests"]
    assert isinstance(artifact_digests, dict)
    artifact_digests["expectations_sha256"] = hashlib.sha256(
        (root / EXPECTATIONS_PATH).read_bytes()
    ).hexdigest()
    artifact_digests["negative_fixtures_sha256"] = hashlib.sha256(
        (root / FIXTURES_PATH).read_bytes()
    ).hexdigest()
    if bind_review and issuance["status"] == "accepted":
        review = issuance["independent_review"]
        assert isinstance(review, dict)
        review["reviewed_artifact_digests"] = copy.deepcopy(artifact_digests)
        review["reviewed_issuance_projection_sha256"] = _canonical_sha256(
            _issuance_review_projection(issuance)
        )
    _write_json(issuance_path, issuance)


def _make_pending(root: Path) -> None:
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    for row in expectations:
        row["review"] = {"status": "pending-independent-review", "reviewer": None}
    _write_jsonl(root / EXPECTATIONS_PATH, expectations)
    issuance = _load_json(root / ISSUANCE_PATH)
    issuance["status"] = "pending-independent-review"
    issuance["independent_review"] = {
        "status": "pending-independent-review",
        "reviewer": None,
        "reviewed_at": None,
        "notes": [],
        "reviewed_artifact_digests": None,
        "reviewed_issuance_projection_sha256": None,
    }
    _write_json(root / ISSUANCE_PATH, issuance)
    _refresh_freeze(root, bind_review=False)


def _ensure_accepted(root: Path) -> None:
    issuance = _load_json(root / ISSUANCE_PATH)
    if issuance["status"] == "accepted":
        return
    record_acceptance(
        root=root,
        reviewer="wp33-independent-v4-reviewer-fixture",
        reviewed_at="2026-09-01T12:00:00Z",
        notes=["Accepted exact claim closure and both fixtures per claim."],
    )


def test_int_repository_issuance_binds_active_plan_and_terminal_suite() -> None:
    validated = validate_issuance(ROOT, require_review=False)
    assert len(validated.expectations) == len(EXPECTED_CLAIM_IDS)
    assert len(validated.fixtures) == len(EXPECTED_FIXTURE_IDS)
    assert set(validated.terminal_suite) == REQUIRED_TAGS
    assert {master.version for master in validated.terminal_suite.values()} == {
        SUITE_VERSION
    }
    assert validated.issuance["suite_identity"] == SUITE_IDENTITY


def test_int_claim_and_fixture_allocations_are_closed() -> None:
    validated = validate_issuance(ROOT, require_review=False)
    assert (
        tuple(row["claim_id"] for row in validated.expectations) == EXPECTED_CLAIM_IDS
    )
    assert tuple(row["family"] for row in validated.expectations) == EXPECTED_FAMILIES
    assert (
        tuple(row["fixture_id"] for row in validated.fixtures) == EXPECTED_FIXTURE_IDS
    )


def test_int_pending_candidate_is_valid_but_cannot_pass_a_final_gate(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    assert len(validate_issuance(root, require_review=False).expectations) == len(
        EXPECTED_CLAIM_IDS
    )
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_REVIEW_REQUIRED"


def test_int_wrong_active_plan_fails_closed(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    pointer = _load_json(root / ACTIVE_PLAN_POINTER)
    pointer["plan_path"] = "docs/plans/not-the-v4-plan.md"
    _write_json(root / ACTIVE_PLAN_POINTER, pointer)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_ACTIVE_PLAN_INVALID"


def test_int_active_design_frontmatter_is_exact_authority(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    pointer = _load_json(root / ACTIVE_PLAN_POINTER)
    plan = (root / str(pointer["plan_path"])).read_text(encoding="utf-8")
    design_line = next(
        line for line in plan.splitlines() if line.startswith("design_path:")
    )
    design = root / design_line.partition(":")[2].strip()
    design.write_text(
        design.read_text(encoding="utf-8").replace("version: v5", "version: v999", 1),
        encoding="utf-8",
    )
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == "V4_ACTIVE_PLAN_INVALID"


def test_int_terminal_suite_rejects_an_unsynchronized_role(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    terminal = discover_terminal_suite(root)["QRY"]
    path = root / terminal.path
    text = path.read_text(encoding="utf-8")
    path.write_text(text.replace("suite_version: 2.2.0", "suite_version: 9.9.9", 1))
    with pytest.raises(V4EvidenceError) as failure:
        discover_terminal_suite(root)
    assert failure.value.code == "V4_AUTHORITY_INVALID"


def test_beh_every_decoded_expectation_has_one_independent_reviewer_and_fault() -> None:
    validated = validate_issuance(ROOT, require_review=False)
    faults = []
    for row in validated.expectations:
        assert row["review"] == {
            "status": "pending-independent-review",
            "reviewer": None,
        }
        faults.append(_canonical_sha256(row["discriminating_fault"]))
    assert len(faults) == len(set(faults)) == len(EXPECTED_CLAIM_IDS)
    if validated.issuance["status"] != "accepted":
        with pytest.raises(V4EvidenceError) as failure:
            validate_issuance(ROOT)
        assert failure.value.code in {"V4_REVIEW_REQUIRED", "V4_REVIEW_NOT_ACCEPTED"}


def test_beh_changed_expected_value_invalidates_exact_review_binding(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    expectations[0]["expected_decoded"] = {"outcome": "self-consistent-but-unreviewed"}
    _write_jsonl(root / EXPECTATIONS_PATH, expectations)
    _refresh_freeze(root, bind_review=False)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_REVIEW_INVALID"


def test_beh_claim_reviewer_must_equal_issuance_reviewer(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    review = expectations[0]["review"]
    assert isinstance(review, dict)
    review["reviewer"] = "different-reviewer"
    _write_jsonl(root / EXPECTATIONS_PATH, expectations)
    _refresh_freeze(root)
    with pytest.raises(V4EvidenceError, match="immutable pending-review handoff"):
        validate_issuance(root)


@pytest.mark.parametrize(
    "reviewer_transform",
    [lambda author: f" {author} ", lambda author: author.upper()],
    ids=("surrounding-whitespace", "casefold-alias"),
)
def test_beh_author_cannot_alias_as_independent_reviewer(
    tmp_path: Path, reviewer_transform: object
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    issuance = _load_json(root / ISSUANCE_PATH)
    constraints = issuance["authoring_constraints"]
    assert isinstance(constraints, dict)
    author = str(constraints["author_identity"])
    assert callable(reviewer_transform)
    with pytest.raises(V4EvidenceError) as failure:
        record_acceptance(
            root=root,
            reviewer=reviewer_transform(author),
            reviewed_at="2026-09-01T12:00:00Z",
            notes=["This identity must not satisfy independent review."],
        )
    assert failure.value.code == "V4_REVIEW_INVALID"


def test_beh_review_timestamp_must_be_a_real_utc_instant(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    with pytest.raises(V4EvidenceError) as failure:
        record_acceptance(
            root=root,
            reviewer="wp33-independent-v4-reviewer-fixture",
            reviewed_at="2026-02-30T12:00:00Z",
            notes=["An impossible timestamp must fail closed."],
        )
    assert failure.value.code == "V4_REVIEW_INVALID"


def test_beh_malformed_review_returns_a_structured_schema_error(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    issuance = _load_json(root / ISSUANCE_PATH)
    review = issuance["independent_review"]
    assert isinstance(review, dict)
    review.pop("notes")
    _write_json(root / ISSUANCE_PATH, issuance)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == "V4_SCHEMA_INVALID"


def test_beh_recorder_preserves_a_concurrent_issuance_write(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    from tooling.ci import record_wp33_v4_acceptance as recorder

    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    concurrent = _load_json(root / ISSUANCE_PATH)
    limitations = concurrent["limitations"]
    assert isinstance(limitations, list)
    limitations.append("Concurrent reviewer marker.")
    concurrent_bytes = (
        json.dumps(concurrent, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    original_stage = recorder._stage_candidate

    def interleaved_stage(
        staged_root: Path, destination: Path, *, issuance_bytes: bytes
    ) -> None:
        original_stage(staged_root, destination, issuance_bytes=issuance_bytes)
        (staged_root / ISSUANCE_PATH).write_bytes(concurrent_bytes)

    monkeypatch.setattr(recorder, "_stage_candidate", interleaved_stage)
    with pytest.raises(V4EvidenceError) as failure:
        record_acceptance(
            root=root,
            reviewer="wp33-independent-v4-reviewer-fixture",
            reviewed_at="2026-09-01T12:00:00Z",
            notes=["A stale reviewer must not overwrite another review."],
        )
    assert failure.value.code == "V4_REVIEW_CONFLICT"
    assert (root / ISSUANCE_PATH).read_bytes() == concurrent_bytes


def test_beh_reviewer_only_recorder_closes_exact_pending_bytes(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    authored_before = {
        EXPECTATIONS_PATH: (root / EXPECTATIONS_PATH).read_bytes(),
        FIXTURES_PATH: (root / FIXTURES_PATH).read_bytes(),
    }
    assert record_acceptance(
        root=root,
        reviewer="wp33-independent-v4-reviewer-fixture",
        reviewed_at="2026-09-01T12:00:00Z",
        notes=["Accepted exact claim closure and both fixtures per claim."],
    ) == len(EXPECTED_CLAIM_IDS)
    validated = validate_issuance(root)
    assert validated.issuance["status"] == "accepted"
    assert {
        row["disposition"]
        for row in validated.issuance["independent_review"]["claim_reviews"]
    } == {"accepted"}
    assert all(
        row["review"] == {"status": "pending-independent-review", "reviewer": None}
        for row in validated.expectations
    )
    assert {
        EXPECTATIONS_PATH: (root / EXPECTATIONS_PATH).read_bytes(),
        FIXTURES_PATH: (root / FIXTURES_PATH).read_bytes(),
    } == authored_before


@pytest.mark.parametrize("disposition", ["rejected", "not-accepted"])
def test_beh_nonacceptance_is_issuance_only_and_cannot_pass_final_review(
    tmp_path: Path, disposition: str
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    authored_before = {
        EXPECTATIONS_PATH: (root / EXPECTATIONS_PATH).read_bytes(),
        FIXTURES_PATH: (root / FIXTURES_PATH).read_bytes(),
    }
    assert record_review(
        root=root,
        reviewer="wp33-independent-v4-reviewer-fixture",
        reviewed_at="2026-09-01T12:00:00Z",
        notes=[f"The exact issuance was {disposition}."],
        disposition=disposition,
    ) == len(EXPECTED_CLAIM_IDS)
    reviewed = validate_issuance(root, require_review=False)
    assert reviewed.issuance["status"] == disposition
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_REVIEW_NOT_ACCEPTED"
    assert {
        EXPECTATIONS_PATH: (root / EXPECTATIONS_PATH).read_bytes(),
        FIXTURES_PATH: (root / FIXTURES_PATH).read_bytes(),
    } == authored_before


def test_neg_every_claim_has_changed_causal_and_rejection_observations() -> None:
    validated = validate_issuance(ROOT, require_review=False)
    fixtures_by_claim: dict[str, dict[str, object]] = {}
    for fixture in validated.fixtures:
        fixtures_by_claim.setdefault(str(fixture["claim_id"]), {})[
            str(fixture["fixture_kind"])
        ] = fixture
    for claim in validated.expectations:
        fixtures = fixtures_by_claim[str(claim["claim_id"])]
        assert set(fixtures) == {"causal", "negative"}
        for fixture in fixtures.values():
            assert fixture["expected_decoded"] != claim["expected_decoded"]
        causal_input = fixtures["causal"]["fixture_input"]
        assert isinstance(causal_input, dict)
        assert causal_input["base_case_id"] == claim["controlled_input"]["case_id"]
        assert (
            _apply_json_merge_patch(
                claim["controlled_input"], causal_input["merge_patch"]
            )
            != claim["controlled_input"]
        )


def test_neg_json_merge_patch_has_rfc7396_object_array_and_null_semantics() -> None:
    target = {
        "object": {"keep": 1, "drop": 2},
        "array": [1, 2],
        "scalar": "old",
    }
    patch = {
        "object": {"drop": None, "add": 3},
        "array": [9],
        "scalar": {"nested": True},
        "absent": None,
    }
    assert _apply_json_merge_patch(target, patch) == {
        "object": {"keep": 1, "add": 3},
        "array": [9],
        "scalar": {"nested": True},
    }


def test_neg_causal_patch_cannot_invent_an_unbound_control_knob(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    fixtures = _load_jsonl(root / FIXTURES_PATH)
    fixture_input = fixtures[0]["fixture_input"]
    assert isinstance(fixture_input, dict)
    fixture_input["merge_patch"] = {"unrelated_magic_result": "accepted"}
    _write_jsonl(root / FIXTURES_PATH, fixtures)
    _refresh_freeze(root, bind_review=False)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == "V4_FIXTURE_NOT_DISCRIMINATING"


@pytest.mark.parametrize(
    ("field", "value", "code"),
    (
        ("base_case_id", "wrong-case", "V4_FIXTURE_NOT_DISCRIMINATING"),
        ("patch_semantics", "undefined", "V4_FIXTURE_NOT_DISCRIMINATING"),
        ("merge_patch", None, "V4_SCHEMA_INVALID"),
    ),
)
def test_neg_causal_fixture_envelope_is_executable_and_exact(
    tmp_path: Path, field: str, value: object, code: str
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    fixtures = _load_jsonl(root / FIXTURES_PATH)
    fixture_input = fixtures[0]["fixture_input"]
    assert isinstance(fixture_input, dict)
    fixture_input[field] = value
    _write_jsonl(root / FIXTURES_PATH, fixtures)
    _refresh_freeze(root, bind_review=False)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == code


def test_neg_causal_fixture_must_commit_the_claim_fault(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    fixtures = _load_jsonl(root / FIXTURES_PATH)
    distinguishes = fixtures[0]["distinguishes"]
    assert isinstance(distinguishes, dict)
    distinguishes["mutation"] = "a different unreviewed mutation"
    _write_jsonl(root / FIXTURES_PATH, fixtures)
    _refresh_freeze(root)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_FIXTURE_NOT_DISCRIMINATING"


def test_neg_production_or_predecessor_evidence_cannot_enter_provenance(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    issuance = _load_json(root / ISSUANCE_PATH)
    provenance = issuance["source_provenance"]
    assert isinstance(provenance, list)
    predecessor = Path("contracts/acceptance/relational-fabric-v3/expectations.jsonl")
    target = root / predecessor
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / predecessor, target)
    provenance.append(
        {
            "path": predecessor.as_posix(),
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
            "role": "forbidden-predecessor-expected-values",
        }
    )
    _write_json(root / ISSUANCE_PATH, issuance)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_PROVENANCE_FORBIDDEN_SOURCE"


def test_neg_provenance_rejects_nonterminal_authority_even_with_exact_digest(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    issuance = _load_json(root / ISSUANCE_PATH)
    provenance = issuance["source_provenance"]
    assert isinstance(provenance, list)
    historical = Path(
        "docs/authoritative_design/"
        "code_property_graph_present_state_fact_ontology_specification_v1.3.md"
    )
    provenance.append(
        {
            "path": historical.as_posix(),
            "sha256": hashlib.sha256((root / historical).read_bytes()).hexdigest(),
            "role": "historical-ontology",
        }
    )
    _write_json(root / ISSUANCE_PATH, issuance)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == "V4_PROVENANCE_INVALID"


def test_neg_design_basis_sections_resolve_to_real_headings(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _make_pending(root)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    basis = expectations[0]["design_basis"]
    assert isinstance(basis, list) and isinstance(basis[0], dict)
    basis[0]["sections"] = ["§999"]
    _write_jsonl(root / EXPECTATIONS_PATH, expectations)
    _refresh_freeze(root, bind_review=False)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root, require_review=False)
    assert failure.value.code == "V4_PROVENANCE_INVALID"


def test_neg_v4_validator_has_no_production_or_v3_validator_import() -> None:
    forbidden = {
        "src",
        "codefabric_cpg_mcp",
        "tooling.ci.successor_evidence_issuance",
        "tooling.ci.production_evidence",
    }
    for relative in (
        Path("tooling/ci/successor_evidence_issuance_v4.py"),
        Path("tooling/ci/record_wp33_v4_acceptance.py"),
    ):
        tree = ast.parse((ROOT / relative).read_text(encoding="utf-8"))
        imported = {
            alias.name
            for node in ast.walk(tree)
            if isinstance(node, ast.Import)
            for alias in node.names
        } | {
            node.module
            for node in ast.walk(tree)
            if isinstance(node, ast.ImportFrom) and node.module is not None
        }
        assert not any(
            module == prefix or module.startswith(f"{prefix}.")
            for module in imported
            for prefix in forbidden
        )


def test_ops_every_oracle_selects_nonzero_cases_and_committed_faults() -> None:
    validated = validate_issuance(ROOT, require_review=False)
    for oracle in ORACLES:
        claims, fixtures = _selector_counts(validated, oracle)
        assert claims > 0
        assert fixtures > 0
        if validated.issuance["status"] == "accepted":
            report = validate_oracle(ROOT, oracle)
            committed = report["committed_discriminating_faults"]
            assert isinstance(committed, list)
            assert report["committed_discriminating_fault_count"] == len(committed)
            assert len(committed) == fixtures
            assert {row["claim_id"] for row in committed} <= set(EXPECTED_CLAIM_IDS)
            selector = validated.issuance["selectors"][oracle]
            assert isinstance(selector, dict)
            selected_kinds = set(selector["fixture_kinds"])
            assert {row["fixture_kind"] for row in committed} <= selected_kinds
            selected_claim_ids = set(selector["claim_ids"])
            selected_families = set(selector["families"])
            selected_ids = {
                str(row["claim_id"])
                for row in validated.expectations
                if (not selected_claim_ids or row["claim_id"] in selected_claim_ids)
                and (not selected_families or row["family"] in selected_families)
            }
            expected_fixture_ids = {
                str(row["fixture_id"])
                for row in validated.fixtures
                if row["claim_id"] in selected_ids
                and row["fixture_kind"] in selected_kinds
            }
            assert {row["fixture_id"] for row in committed} == expected_fixture_ids
            assert all(
                ("applied_input_sha256" in row) == (row["fixture_kind"] == "causal")
                for row in committed
            )
        else:
            with pytest.raises(V4EvidenceError) as failure:
                validate_oracle(ROOT, oracle)
            assert failure.value.code == "V4_REVIEW_REQUIRED"


def test_ops_zero_selector_fails_even_if_review_binding_is_reissued(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    issuance = _load_json(root / ISSUANCE_PATH)
    selectors = issuance["selectors"]
    assert isinstance(selectors, dict)
    selector = selectors["expectation-drift-selector-sensitivity-check"]
    assert isinstance(selector, dict)
    selector["claim_ids"] = []
    selector["families"] = []
    review = issuance["independent_review"]
    assert isinstance(review, dict)
    review["reviewed_issuance_projection_sha256"] = _canonical_sha256(
        _issuance_review_projection(issuance)
    )
    _write_json(root / ISSUANCE_PATH, issuance)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_SELECTOR_ZERO_SELECTION"


def test_ops_expectation_byte_drift_stops_before_selector_execution(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    with (root / EXPECTATIONS_PATH).open("a", encoding="utf-8") as stream:
        stream.write(" ")
    with pytest.raises(V4EvidenceError) as failure:
        validate_oracle(root, "expectation-drift-selector-sensitivity-check")
    assert failure.value.code in {"V4_JSONL_FRAMING_INVALID", "V4_ISSUANCE_DRIFT"}


def test_ops_reviewed_selector_projection_detects_silent_refresh(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    _ensure_accepted(root)
    issuance = _load_json(root / ISSUANCE_PATH)
    selectors = issuance["selectors"]
    assert isinstance(selectors, dict)
    selector = selectors["expectation-drift-selector-sensitivity-check"]
    assert isinstance(selector, dict)
    claim_ids = selector["claim_ids"]
    assert isinstance(claim_ids, list) and len(claim_ids) > 1
    claim_ids.pop()
    _write_json(root / ISSUANCE_PATH, issuance)
    with pytest.raises(V4EvidenceError) as failure:
        validate_issuance(root)
    assert failure.value.code == "V4_REVIEW_INVALID"
