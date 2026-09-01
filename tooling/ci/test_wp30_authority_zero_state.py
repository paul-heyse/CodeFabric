"""Independent causal tests for the WP30 multidimensional live-tree inventory."""

from __future__ import annotations

from pathlib import Path

from tooling.ci.wp30_authority_zero_state import (
    FORBIDDEN_TEXT,
    NEGATIVE_FIXTURE,
    ROOT,
    archive_member_violations,
    cargo_payload_inventory,
    classify_files,
    disposed_activation_residue_violations,
    retired_text_violations,
    validate_activation_residue,
    validate_negative_fixture,
)


def test_neg_controlled_live_route_is_outside_history_and_fails_detector() -> None:
    assert not NEGATIVE_FIXTURE.startswith(
        ("docs/designs/", "docs/plans/", "docs/reviews/")
    )
    text = (ROOT / NEGATIVE_FIXTURE).read_text(encoding="utf-8")
    assert "serve_programmatic" in text
    assert "serve_programmatic" in FORBIDDEN_TEXT
    assert retired_text_violations(NEGATIVE_FIXTURE, text) == [
        (
            "text: tooling/ci/fixtures/wp30-live-legacy-route.rs contains retired token "
            "'serve_programmatic'"
        )
    ]
    assert validate_negative_fixture(ROOT) > 0


def test_int_history_and_oracle_exclusions_do_not_hide_live_source() -> None:
    live, excluded = classify_files(
        (
            "docs/plans/retained.md",
            "tooling/ci/fixtures/wp30-live-legacy-route.rs",
            "src/live.rs",
        )
    )
    assert live == ["src/live.rs"]
    assert excluded == [
        "docs/plans/retained.md",
        "tooling/ci/fixtures/wp30-live-legacy-route.rs",
    ]


def test_neg_cargo_payload_rejects_retired_feature_and_target() -> None:
    count, violations = cargo_payload_inventory(
        (
            {
                "packages": [
                    {
                        "name": "codefabric",
                        "features": {"model-compiler": []},
                        "targets": [{"name": "codefabric-model"}],
                        "dependencies": [],
                    }
                ]
            },
        )
    )
    assert count == 3
    assert violations == [
        "cargo: forbidden feature codefabric#model-compiler",
        "cargo: forbidden target codefabric#codefabric-model",
    ]


def test_neg_distribution_member_inventory_rejects_retired_package_data() -> None:
    count, violations = archive_member_violations(
        (
            "codefabric_cpg_mcp/__init__.py",
            "codefabric_cpg_mcp/contracts/model_registries.py",
        )
    )
    assert count == 2
    assert violations == [
        (
            "installed_artifact: retired distribution member "
            "codefabric_cpg_mcp/contracts/model_registries.py"
        )
    ]


def test_neg_fixture_path_remains_a_regular_non_imported_file() -> None:
    path = ROOT / NEGATIVE_FIXTURE
    assert path == Path(ROOT, "tooling", "ci", "fixtures", "wp30-live-legacy-route.rs")
    assert path.is_file()


def test_int_activation_residue_has_exact_retained_and_disposed_wp32_state() -> None:
    assert validate_activation_residue(ROOT) == 8


def test_neg_disposed_activation_authority_cannot_reappear_in_live_rust(
    tmp_path: Path,
) -> None:
    source = tmp_path / "src"
    source.mkdir()
    live = source / "live.rs"
    live.write_text("struct ProgrammaticWorkspaceReleasePins;\n", encoding="utf-8")
    assert disposed_activation_residue_violations(
        tmp_path, {"ProgrammaticWorkspaceReleasePins"}, {"src/live.rs"}
    ) == [
        (
            "disposed activation residue ProgrammaticWorkspaceReleasePins remains live in "
            "src/live.rs"
        )
    ]
