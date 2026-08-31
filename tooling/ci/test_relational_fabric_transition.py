"""Focused negative fixtures for the v2 transition-governance relations."""

from __future__ import annotations

import json
import subprocess
import zipfile
from dataclasses import replace
from pathlib import Path

import pytest

from tooling.ci.relational_fabric_transition import (
    CARGO_MANIFESTS,
    CURRENT_SUITE_ID,
    CURRENT_SUITE_VERSION,
    EXPECTED_DECISIONS,
    LEGACY_SELECTORS,
    REQUIRED_FILE_PATHS,
    ROOT,
    TARGET_PRINCIPLES,
    CoverageReport,
    InventoryIssue,
    InventoryReport,
    InventorySurface,
    SelectorProgram,
    SurfaceSelector,
    TransitionGovernanceError,
    _sha256_bytes,
    _wheel_surfaces,
    collect_cargo_surfaces,
    collect_outline_surfaces,
    design_legacy_dispositions,
    enumerate_repository_files,
    evaluate_disposition_coverage,
    load_selector_program,
    validate_authority_selection,
    validate_disposition_coverage,
    validate_inventory_universe,
    validate_legacy_authority_freeze,
)

ZERO_DIGEST = "0" * 64


def _git(root: Path, *arguments: str) -> str:
    return subprocess.run(
        ("git", *arguments),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _init_git(root: Path) -> None:
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "Transition Test")
    _git(root, "config", "user.email", "transition@example.invalid")


def _authority_fixture(tmp_path: Path) -> Path:
    _init_git(tmp_path)
    authority_root = tmp_path / "docs/authoritative_design"
    authority_root.mkdir(parents=True)
    tags = ("SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM")
    predecessor_payloads: dict[str, str] = {}
    for tag in tags:
        name = f"predecessor-{tag.lower()}.md"
        payload = f"# Immutable predecessor {tag}\n"
        predecessor_payloads[name] = payload
        (authority_root / name).write_text(payload, encoding="utf-8")
    _git(tmp_path, "add", "docs/authoritative_design")
    _git(tmp_path, "commit", "-q", "-m", "predecessor suite")
    baseline = _git(tmp_path, "rev-parse", "HEAD")

    for tag in tags:
        artifact_id = f"fixture-{tag.lower()}"
        predecessor = f"docs/authoritative_design/predecessor-{tag.lower()}.md"
        body = (
            "---\n"
            "artifact: authoritative-design\n"
            f"artifact_id: {artifact_id}\n"
            f"suite_id: {CURRENT_SUITE_ID}\n"
            f"suite_version: {CURRENT_SUITE_VERSION}\n"
            f"artifact_tag: {tag}\n"
            "artifact_version: 2.0.0\n"
            "authority_status: current\n"
            f"predecessor_path: {predecessor}\n"
            "---\n\n"
            f"# Current {tag}\n\nStable identity `{artifact_id}`.\n"
        )
        if tag == "SUITE":
            body += (
                f"\nDoctrine `{TARGET_PRINCIPLES.as_posix()}`. "
                "There is no generated manifest authority.\n"
            )
        (authority_root / f"current-{tag.lower()}.md").write_text(
            body, encoding="utf-8"
        )
    principles = tmp_path / TARGET_PRINCIPLES
    principles.parent.mkdir(parents=True, exist_ok=True)
    principles.write_text("# v2 principles\n", encoding="utf-8")
    plan = (
        tmp_path
        / "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md"
    )
    plan.parent.mkdir(parents=True)
    plan.write_text(
        "---\n"
        "artifact: implementation-plan\n"
        f"baseline_commit: {baseline}\n"
        "---\n\n# Fixture plan\n",
        encoding="utf-8",
    )
    return tmp_path


def _surface(
    surface_id: str,
    path: str,
    *,
    kind: str = "file",
    sources: frozenset[str] = frozenset({"filesystem"}),
    symbol: str | None = None,
    package: str | None = None,
    legacy: bool = False,
    digest: str | None = None,
) -> InventorySurface:
    return InventorySurface(
        surface_id=surface_id,
        path=path,
        kind=kind,
        sources=sources,
        content_digest=digest or _sha256_bytes(surface_id.encode()),
        symbol=symbol,
        package=package,
        legacy_candidate=legacy,
    )


def _report(legacy_surfaces: list[InventorySurface]) -> InventoryReport:
    required_paths = set(REQUIRED_FILE_PATHS) | {"fuzz/fuzz_targets/sample.rs"}
    files = [
        _surface(
            f"file:{path}",
            path,
            sources=frozenset({"git", "filesystem"}),
        )
        for path in sorted(required_paths)
    ]
    support = [
        _surface(
            "symbol:src/lib.rs#public",
            "src/lib.rs",
            kind="symbol",
            sources=frozenset({"ast-grep"}),
            symbol="public",
        ),
        _surface(
            "cargo-package:Cargo.toml#codefabric",
            "Cargo.toml",
            kind="cargo-package",
            sources=frozenset({"cargo"}),
            package="codefabric",
        ),
        _surface(
            "installed:adapter!module.py",
            "codefabric-cpg-mcp/module.py",
            kind="installed",
            sources=frozenset({"installed"}),
            package="codefabric-cpg-mcp",
        ),
        _surface(
            "wheel:adapter!module.py",
            "codefabric-cpg-mcp/module.py",
            kind="wheel",
            sources=frozenset({"wheel"}),
            package="codefabric-cpg-mcp",
        ),
        _surface(
            "sdist:adapter!module.py",
            "codefabric-cpg-mcp/module.py",
            kind="sdist",
            sources=frozenset({"sdist"}),
            package="codefabric-cpg-mcp",
        ),
    ]
    parse_paths = {
        path
        for path in required_paths
        if path.endswith((".rs", ".py"))
        and path.startswith(("src/", "fuzz/fuzz_targets/"))
    }
    repository_paths = frozenset(required_paths)
    return InventoryReport(
        surfaces=(*files, *support, *legacy_surfaces),
        git_paths=repository_paths,
        filesystem_paths=repository_paths,
        parsed_paths=frozenset(parse_paths),
        cargo_manifests=frozenset(path.as_posix() for path in CARGO_MANIFESTS),
        excluded=(
            InventoryIssue("filesystem", ".envrc.local", "secret-file:.envrc.local"),
        ),
        skipped=(),
        unknowns=(),
    )


def _selector(
    selector_id: str,
    path_glob: str,
    *,
    decision_id: str | None = None,
    disposition: str | None = None,
    kinds: frozenset[str] = frozenset({"file"}),
    symbol_regex: str | None = None,
) -> SurfaceSelector:
    import re

    return SurfaceSelector(
        selector_id=selector_id,
        path_glob=path_glob,
        surface_kinds=kinds,
        symbol_regex=re.compile(symbol_regex) if symbol_regex else None,
        package_regex=None,
        decision_id=decision_id,
        disposition=disposition,
    )


def _program(*, extra_l20: bool = False) -> tuple[InventoryReport, SelectorProgram]:
    legacy = [
        _surface(
            f"file:legacy/{decision}.rs",
            f"legacy/{decision}.rs",
            legacy=True,
        )
        for decision in sorted(EXPECTED_DECISIONS)
    ]
    if extra_l20:
        legacy.append(
            _surface(
                "file:legacy/L-20-extra.rs",
                "legacy/L-20-extra.rs",
                legacy=True,
            )
        )
    dispositions = []
    for decision in sorted(EXPECTED_DECISIONS):
        path = (
            f"legacy/{decision}*.rs" if decision == "L-20" else f"legacy/{decision}.rs"
        )
        dispositions.append(
            _selector(
                f"disposition:{decision}",
                path,
                decision_id=decision,
                disposition="delete" if decision != "L-43" else "preserve",
            )
        )
    return (
        _report(legacy),
        SelectorProgram(
            candidates=(_selector("candidate:legacy", "legacy/*.rs"),),
            dispositions=tuple(dispositions),
            source_digest="1" * 64,
        ),
    )


def test_authority_selection_proves_one_current_and_byte_identical_history(
    tmp_path: Path,
) -> None:
    root = _authority_fixture(tmp_path)
    report = validate_authority_selection(root)
    assert report == {
        "current_suite_id": CURRENT_SUITE_ID,
        "current_suite_version": CURRENT_SUITE_VERSION,
        "current_master_count": 8,
        "historical_suite_count": 1,
        "historical_predecessor_count": 1,
        "unrouted_master_count": 0,
        "generated_authority_selector_count": 0,
    }


def test_authority_selection_rejects_coequal_unrouted_master(tmp_path: Path) -> None:
    root = _authority_fixture(tmp_path)
    (root / "docs/authoritative_design/rogue.md").write_text(
        "# Coequal authority\n", encoding="utf-8"
    )
    with pytest.raises(TransitionGovernanceError, match="predecessor closure"):
        validate_authority_selection(root)


def test_authority_selection_rejects_rewritten_predecessor(tmp_path: Path) -> None:
    root = _authority_fixture(tmp_path)
    historical = root / "docs/authoritative_design/predecessor-suite.md"
    historical.write_text("# rewritten\n", encoding="utf-8")
    with pytest.raises(TransitionGovernanceError, match="byte-identical"):
        validate_authority_selection(root)


def test_live_transition_inputs_fail_closed_when_not_published(tmp_path: Path) -> None:
    with pytest.raises(
        TransitionGovernanceError, match="principles document is absent"
    ):
        validate_authority_selection(tmp_path)
    with pytest.raises(TransitionGovernanceError, match="missing or invalid"):
        load_selector_program(tmp_path, LEGACY_SELECTORS)


def test_accepted_design_has_exact_l20_through_l55_dispositions() -> None:
    dispositions = design_legacy_dispositions(
        ROOT
        / "docs/designs/codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md"
    )
    assert set(dispositions) == EXPECTED_DECISIONS
    assert dispositions["L-22"] == "encapsulate-temporarily"
    assert dispositions["L-43"] == "preserve"
    assert dispositions["L-55"] == "replace"


def test_selector_program_binds_exact_design_and_plan_and_rejects_conflict(
    tmp_path: Path,
) -> None:
    design_target = (
        tmp_path
        / "docs/designs/codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md"
    )
    plan_target = (
        tmp_path
        / "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md"
    )
    design_target.parent.mkdir(parents=True)
    plan_target.parent.mkdir(parents=True)
    design_target.write_bytes((ROOT / design_target.relative_to(tmp_path)).read_bytes())
    plan_target.write_bytes((ROOT / plan_target.relative_to(tmp_path)).read_bytes())
    dispositions = design_legacy_dispositions(design_target)

    def row(decision: str) -> dict[str, object]:
        return {
            "selector_id": f"disposition:{decision}",
            "path_glob": f"legacy/{decision}.rs",
            "surface_kinds": ["file"],
            "symbol_regex": None,
            "package_regex": None,
            "decision_id": decision,
            "disposition": dispositions[decision],
        }

    document: dict[str, object] = {
        "schema_version": 1,
        "design_path": design_target.relative_to(tmp_path).as_posix(),
        "design_sha256": _sha256_bytes(design_target.read_bytes()),
        "plan_path": plan_target.relative_to(tmp_path).as_posix(),
        "plan_sha256": _sha256_bytes(plan_target.read_bytes()),
        "candidate_selectors": [
            {
                "selector_id": "candidate:legacy",
                "path_glob": "legacy/*.rs",
                "surface_kinds": ["file"],
                "symbol_regex": None,
                "package_regex": None,
            }
        ],
        "disposition_selectors": [row(decision) for decision in sorted(dispositions)],
    }
    selector_path = tmp_path / LEGACY_SELECTORS
    selector_path.parent.mkdir(parents=True)
    selector_path.write_text(json.dumps(document), encoding="utf-8")
    program = load_selector_program(tmp_path)
    assert len(program.dispositions) == 36

    document["disposition_selectors"][0]["disposition"] = "preserve"
    selector_path.write_text(json.dumps(document), encoding="utf-8")
    with pytest.raises(TransitionGovernanceError, match="contradict"):
        load_selector_program(tmp_path)


def test_hidden_no_ignore_inventory_excludes_secret_and_build_outputs(
    tmp_path: Path,
) -> None:
    _init_git(tmp_path)
    (tmp_path / ".gitignore").write_text(".hidden/\n", encoding="utf-8")
    (tmp_path / "tracked.txt").write_text("tracked\n", encoding="utf-8")
    (tmp_path / "shared").mkdir()
    (tmp_path / "shared/value.txt").write_text("shared\n", encoding="utf-8")
    (tmp_path / ".linked").symlink_to("shared", target_is_directory=True)
    _git(tmp_path, "add", ".gitignore", "tracked.txt", "shared", ".linked")
    _git(tmp_path, "commit", "-q", "-m", "fixture")
    (tmp_path / ".hidden").mkdir()
    (tmp_path / ".hidden/ignored.txt").write_text("hidden\n", encoding="utf-8")
    (tmp_path / ".envrc.local").write_text("TOKEN=secret\n", encoding="utf-8")
    (tmp_path / "target").mkdir()
    (tmp_path / "target/output").write_text("build\n", encoding="utf-8")
    (tmp_path / ".venv").mkdir()
    (tmp_path / ".venv/package").write_text("build\n", encoding="utf-8")

    surfaces, git_paths, filesystem_paths, excluded, issues = (
        enumerate_repository_files(tmp_path)
    )
    assert "tracked.txt" in git_paths
    assert ".linked" in filesystem_paths
    assert ".hidden/ignored.txt" in filesystem_paths
    assert all(surface.path != ".envrc.local" for surface in surfaces)
    assert {issue.subject for issue in excluded} >= {
        ".git",
        ".envrc.local",
        "target",
        ".venv",
    }
    assert issues == ()


def test_outline_inventory_accounts_for_imports_exports_and_files(
    tmp_path: Path,
) -> None:
    (tmp_path / "src").mkdir()
    (tmp_path / "src/lib.rs").write_text(
        "pub use std::path::Path;\npub fn exposed() {}\n", encoding="utf-8"
    )
    (tmp_path / "tooling/ci").mkdir(parents=True)
    (tmp_path / "tooling/ci/check.py").write_text(
        "from pathlib import Path\n\ndef exposed():\n    return Path('.')\n",
        encoding="utf-8",
    )
    paths = {"src/lib.rs", "tooling/ci/check.py"}
    surfaces, parsed, issues = collect_outline_surfaces(tmp_path, paths)
    assert parsed == paths
    assert issues == ()
    assert {surface.kind for surface in surfaces} >= {"import", "symbol"}


def test_current_cargo_inventory_covers_all_four_roots() -> None:
    surfaces, manifests, issues = collect_cargo_surfaces(ROOT)
    assert issues == ()
    assert manifests == frozenset(path.as_posix() for path in CARGO_MANIFESTS)
    assert {surface.kind for surface in surfaces} >= {
        "cargo-package",
        "cargo-feature",
        "cargo-target",
    }


def test_archive_inventory_rejects_parent_traversal(tmp_path: Path) -> None:
    wheel = tmp_path / "unsafe.whl"
    with zipfile.ZipFile(wheel, "w") as archive:
        archive.writestr("../secret", "not allowed")
    with pytest.raises(TransitionGovernanceError, match="repository-relative"):
        _wheel_surfaces(wheel)


def test_inventory_validation_rejects_skipped_input() -> None:
    report, _ = _program()
    invalid = replace(
        report,
        skipped=(InventoryIssue("ast-grep", "src/lib.rs", "not parsed"),),
    )
    with pytest.raises(TransitionGovernanceError, match="skipped or unknown"):
        validate_inventory_universe(invalid)


def test_disposition_coverage_accepts_all_l20_through_l55_once() -> None:
    report, program = _program()
    result = validate_disposition_coverage(report, program)
    assert result["candidate_surface_count"] == len(EXPECTED_DECISIONS)
    assert result["decision_count"] == 36


def test_selector_symbol_regex_matches_identity_independently_of_signature() -> None:
    selector = _selector(
        "candidate:module",
        "src/lib.rs",
        kinds=frozenset({"symbol"}),
        symbol_regex="^legacy_module$",
    )
    surface = replace(
        _surface(
            "symbol:src/lib.rs#legacy_module",
            "src/lib.rs",
            kind="symbol",
            symbol="legacy_module",
        ),
        signature="pub mod legacy_module",
    )
    assert selector.matches(surface)


def test_disposition_coverage_rejects_uncovered_overlap_and_no_match() -> None:
    report, program = _program()
    uncovered_program = replace(program, dispositions=program.dispositions[1:])
    with pytest.raises(TransitionGovernanceError, match="uncovered"):
        validate_disposition_coverage(report, uncovered_program)

    overlapping_program = replace(
        program,
        dispositions=(
            *program.dispositions,
            _selector(
                "disposition:overlap",
                "legacy/L-20.rs",
                decision_id="L-21",
                disposition="replace",
            ),
        ),
    )
    with pytest.raises(TransitionGovernanceError, match="overlapping"):
        validate_disposition_coverage(report, overlapping_program)

    no_match_program = replace(
        program,
        candidates=(*program.candidates, _selector("candidate:absent", "absent/**")),
    )
    with pytest.raises(TransitionGovernanceError, match="no_match_selectors"):
        validate_disposition_coverage(report, no_match_program)


def test_mixed_file_requires_non_overlapping_symbol_selectors() -> None:
    base_report, base_program = _program()
    retained = [
        surface
        for surface in base_report.surfaces
        if surface.path not in {"legacy/L-20.rs", "legacy/L-21.rs"}
    ]
    mixed = [
        _surface(
            "symbol:legacy/mixed.rs#alpha",
            "legacy/mixed.rs",
            kind="symbol",
            symbol="alpha",
            legacy=True,
        ),
        _surface(
            "symbol:legacy/mixed.rs#beta",
            "legacy/mixed.rs",
            kind="symbol",
            symbol="beta",
            legacy=True,
        ),
    ]
    report = replace(base_report, surfaces=(*retained, *mixed))
    remaining = tuple(
        selector
        for selector in base_program.dispositions
        if selector.decision_id not in {"L-20", "L-21"}
    )
    valid = replace(
        base_program,
        candidates=(
            _selector(
                "candidate:mixed", "legacy/*", kinds=frozenset({"file", "symbol"})
            ),
        ),
        dispositions=(
            *remaining,
            _selector(
                "disposition:mixed-alpha",
                "legacy/mixed.rs",
                decision_id="L-20",
                disposition="delete",
                kinds=frozenset({"symbol"}),
                symbol_regex="^alpha$",
            ),
            _selector(
                "disposition:mixed-beta",
                "legacy/mixed.rs",
                decision_id="L-21",
                disposition="replace",
                kinds=frozenset({"symbol"}),
                symbol_regex="^beta$",
            ),
        ),
    )
    validate_disposition_coverage(report, valid)

    invalid = replace(
        valid,
        dispositions=(
            *remaining,
            _selector(
                "disposition:mixed-alpha",
                "legacy/mixed.rs",
                decision_id="L-20",
                disposition="delete",
                kinds=frozenset({"symbol"}),
            ),
            _selector(
                "disposition:mixed-beta",
                "legacy/mixed.rs",
                decision_id="L-21",
                disposition="replace",
                kinds=frozenset({"symbol"}),
            ),
        ),
    )
    coverage: CoverageReport = evaluate_disposition_coverage(report, invalid)
    assert coverage.unresolved_mixed_files == ("legacy/mixed.rs",)
    with pytest.raises(TransitionGovernanceError, match="unresolved_mixed_files"):
        validate_disposition_coverage(report, invalid)


def test_legacy_authority_freeze_allows_deletion_but_rejects_new_or_changed() -> None:
    report, program = _program(extra_l20=True)
    candidate_ids = evaluate_disposition_coverage(report, program).candidate_surfaces
    candidate_by_id = {
        surface.surface_id: surface
        for surface in report.surfaces
        if surface.surface_id in candidate_ids
    }
    freeze: dict[str, object] = {
        "schema_version": 1,
        "selector_sha256": program.source_digest,
        "frozen_at_commit": "2" * 40,
        "surfaces": [
            {"surface_id": surface_id, "content_digest": surface.content_digest}
            for surface_id, surface in sorted(candidate_by_id.items())
        ],
    }

    deleted_report = replace(
        report,
        surfaces=tuple(
            surface
            for surface in report.surfaces
            if surface.surface_id != "file:legacy/L-20-extra.rs"
        ),
    )
    result = validate_legacy_authority_freeze(deleted_report, program, freeze)
    assert result["deleted_surface_count"] == 1

    changed_report = replace(
        report,
        surfaces=tuple(
            replace(surface, content_digest="f" * 64)
            if surface.surface_id == "file:legacy/L-20.rs"
            else surface
            for surface in report.surfaces
        ),
    )
    with pytest.raises(TransitionGovernanceError, match="changed"):
        validate_legacy_authority_freeze(changed_report, program, freeze)

    introduced_report = replace(
        report,
        surfaces=(
            *report.surfaces,
            _surface(
                "file:legacy/L-20-new.rs",
                "legacy/L-20-new.rs",
                legacy=True,
            ),
        ),
    )
    with pytest.raises(TransitionGovernanceError, match="introduced"):
        validate_legacy_authority_freeze(introduced_report, program, freeze)
