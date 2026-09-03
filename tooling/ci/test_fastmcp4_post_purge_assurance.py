"""Fault-seeded tests for the WP62 live-surface purge census."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from tooling.ci.fastmcp4_post_purge_assurance import (
    CURRENT_NEGATIVE_ASSURANCE_PATHS,
    EXPECTED_RUNTIME_DEPENDENCIES,
    FORBIDDEN_PATHS,
    RETIRED_RECIPE_NAMES,
    RETIRED_TOKENS,
    ROOT,
    PostPurgeAssuranceError,
    validate_coverage,
    validate_package_contract,
    validate_zero_state,
)


def _write(root: Path, relative: str, text: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _minimal_root(tmp_path: Path) -> Path:
    root = tmp_path / "repo"
    dependencies = ",\n  ".join(f'"{value}"' for value in EXPECTED_RUNTIME_DEPENDENCIES)
    _write(
        root,
        "Cargo.toml",
        """[package]
name = "codefabric"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "codefabric-proto-gen"
path = "tooling/proto/generate.rs"
required-features = ["proto-tooling"]

[[bin]]
name = "codefabricd"
path = "src/bin/codefabricd.rs"
required-features = ["daemon"]

[[bin]]
name = "codefabric"
path = "src/bin/codefabric.rs"
required-features = ["daemon"]
""",
    )
    _write(
        root,
        "codefabric-cpg-mcp/pyproject.toml",
        f"""[project]
name = "codefabric-cpg-mcp"
version = "0.1.0"
dependencies = [
  {dependencies}
]
""",
    )
    _write(root, "codefabric-cpg-mcp/uv.lock", "version = 1\nrevision = 3\n")
    _write(
        root,
        "src/bin/codefabric.rs",
        "fn main() { let _ = CodefabricProcessSettings::parse(std::env::args_os()); }\n",
    )
    _write(
        root,
        "src/bin/codefabricd.rs",
        "fn main() { let _ = FabricDaemonProcessSettings::parse(std::env::args_os()); }\n",
    )
    for filename in (
        "codefabric.cpgd.v2.rs",
        "codefabric.provider.v1.rs",
        "codefabric.pyrefly.v1.rs",
        "codefabric.rustc.v1.rs",
    ):
        _write(root, f"src/generated/{filename}", "// generated target\n")
    _write(root, "src/lib.rs", "pub fn target() {}\n")
    _write(root, "tests/integration.rs", "#[test] fn target() {}\n")
    _write(
        root, "codefabric-cpg-mcp/src/codefabric_cpg_mcp/__init__.py", "__all__ = []\n"
    )
    _write(
        root,
        "codefabric-cpg-mcp/tests/test_target.py",
        "def test_target():\n    assert True\n",
    )
    for filename in (
        "__init__.py",
        "cpg_query_service_pb2.py",
        "cpg_query_service_pb2_grpc.py",
    ):
        _write(
            root,
            f"codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/{filename}",
            "# generated v2 target\n",
        )
    _write(
        root,
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/cpg_query_service_pb2.pyi",
        "# generated v2 target\n",
    )
    for filename in (
        "cpg_query_service.proto",
        "provider_control.proto",
        "pyrefly_sidecar.proto",
        "rustc_extractor.proto",
    ):
        _write(root, f"contracts/rpc/{filename}", 'syntax = "proto3";\n')
    _write(root, "contracts/schema/target.json", "{}\n")
    _write(root, "scripts/target.sh", "#!/bin/sh\nexit 0\n")
    _write(root, "tooling/ci/target.py", "VALUE = 1\n")
    _write(root, "tooling/benchmarks/target.py", "VALUE = 1\n")
    _write(root, "tooling/fastmcp4_modern_client_driver.py", "VALUE = 1\n")
    _write(root, "tooling/proto/target.py", "VALUE = 1\n")
    _write(root, "tooling/proto/generate.rs", "fn main() {}\n")
    _write(root, "tooling/proto/production-descriptor.pb", "descriptor\n")
    _write(root, "rules/target.yml", "id: target\n")
    _write(root, "rule-tests/target.yml", "id: target\n")
    _write(root, ".github/workflows/target.yml", "name: target\n")
    _write(root, "justfile", "target-check:\n    true\n")
    _write(root, "README.md", "# Target\n")
    _write(root, "AGENTS.md", "# Target\n")
    return root


def test_int_live_repository_surface_is_fully_covered() -> None:
    report = validate_coverage(ROOT)
    assert int(report["live_files"]) > 0
    assert int(report["parsed_python"]) > 0
    assert report["unreadable"] == report["unparsed"] == 0


def test_int_unparsed_python_is_rejected(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "tooling/ci/broken.py", "def broken(:\n")
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_coverage(root)
    assert failure.value.code == "CFV7_PURGE_UNPARSED"


def test_beh_retained_package_contract_is_exact(tmp_path: Path) -> None:
    report = validate_package_contract(_minimal_root(tmp_path))
    assert report == {
        "runtime_dependencies": 8,
        "root_binaries": 3,
        "operational_binaries": 2,
        "proto_files": 4,
        "descriptor_sets": 1,
        "rust_generated_bindings": 4,
        "python_generated_bindings": 4,
    }


def test_beh_extra_binary_is_rejected(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "src/bin/schema_generator.rs", "fn main() {}\n")
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_package_contract(root)
    assert failure.value.code == "CFV7_PURGE_BINARY_SURFACE"


@pytest.mark.parametrize(
    ("relative", "contents"),
    (
        ("contracts/rpc/retired_service.proto", 'syntax = "proto3";\n'),
        ("src/generated/codefabric.retired.v1.rs", "// retired binding\n"),
        (
            "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/retired_pb2.py",
            "# retired binding\n",
        ),
    ),
)
def test_beh_extra_generated_surface_is_rejected(
    tmp_path: Path, relative: str, contents: str
) -> None:
    root = _minimal_root(tmp_path)
    _write(root, relative, contents)
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_package_contract(root)
    assert failure.value.code == "CFV7_PURGE_GENERATED_BINDING"


def test_beh_missing_production_descriptor_is_rejected(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    (root / "tooling/proto/production-descriptor.pb").unlink()
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_package_contract(root)
    assert failure.value.code == "CFV7_PURGE_GENERATED_BINDING"


def test_beh_direct_dependency_drift_is_rejected(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    pyproject = root / "codefabric-cpg-mcp/pyproject.toml"
    pyproject.write_text(
        pyproject.read_text(encoding="utf-8").replace(
            '  "rfc8785==0.1.4"',
            '  "rfc8785==0.1.4",\n  "pydantic-settings==2.15.0"',
        ),
        encoding="utf-8",
    )
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_package_contract(root)
    assert failure.value.code == "CFV7_PURGE_PACKAGE_INVALID"


@pytest.mark.parametrize(("category", "token"), sorted(RETIRED_TOKENS.items()))
def test_neg_every_retired_token_class_is_detected(
    tmp_path: Path, category: str, token: str
) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "tooling/ci/seed.py", f"# {category}: {token}\n")
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_zero_state(root)
    assert failure.value.code == "CFV7_PURGE_RETIRED_TOKEN"


def test_neg_transitive_fastmcp_lock_entries_are_not_application_adoption(
    tmp_path: Path,
) -> None:
    root = _minimal_root(tmp_path)
    lock = root / "codefabric-cpg-mcp/uv.lock"
    lock.write_text(
        lock.read_text(encoding="utf-8")
        + 'name = "pydantic-settings"\n'
        + 'url = "https://example.invalid/pydantic_settings.whl"\n'
        + 'name = "fastmcp-slim"\n'
        + 'url = "https://example.invalid/fastmcp_slim.whl"\n',
        encoding="utf-8",
    )
    assert validate_zero_state(root)["live_matches"] == 0


def test_neg_retired_token_guard_recipe_is_classified_without_hiding_others(
    tmp_path: Path,
) -> None:
    root = _minimal_root(tmp_path)
    guard_body = "|".join(RETIRED_TOKENS.values())
    _write(
        root,
        "justfile",
        "fastmcp4-adapter-authority-zero-state-check:\n"
        f"    @if rg '{guard_body}' src; then exit 1; fi\n",
    )
    assert validate_zero_state(root)["live_matches"] == 0
    _write(
        root,
        "justfile",
        "target-check:\n"
        f"    @echo '{RETIRED_TOKENS['mcp_call_identity']}'\n"
        "fastmcp4-adapter-authority-zero-state-check:\n"
        f"    @if rg '{guard_body}' src; then exit 1; fi\n",
    )
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_zero_state(root)
    assert failure.value.code == "CFV7_PURGE_RETIRED_TOKEN"


@pytest.mark.parametrize("path", sorted(CURRENT_NEGATIVE_ASSURANCE_PATHS))
def test_neg_current_fault_runner_tokens_are_classified_not_runtime_authority(
    tmp_path: Path, path: Path
) -> None:
    root = _minimal_root(tmp_path)
    _write(
        root,
        str(path),
        "\n".join(RETIRED_TOKENS.values()) + "\n",
    )
    assert validate_zero_state(root)["live_matches"] == 0


def test_int_governed_proto_history_is_covered_but_not_live_authority(
    tmp_path: Path,
) -> None:
    root = _minimal_root(tmp_path)
    _write(
        root,
        "tooling/proto/history/descriptor-history.md",
        "immutable history: " + RETIRED_TOKENS["compat_environment"] + "\n",
    )
    coverage = validate_coverage(root)
    assert int(coverage["live_files"]) > 0
    assert validate_zero_state(root)["live_matches"] == 0


@pytest.mark.parametrize("forbidden_path", sorted(FORBIDDEN_PATHS))
def test_neg_every_forbidden_path_class_is_detected(
    tmp_path: Path, forbidden_path: str
) -> None:
    root = _minimal_root(tmp_path)
    path = Path(forbidden_path)
    if path.suffix:
        _write(root, forbidden_path, "# retired\n")
    else:
        (root / path).mkdir(parents=True, exist_ok=True)
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_zero_state(root)
    assert failure.value.code == "CFV7_PURGE_FORBIDDEN_PATH"


@pytest.mark.parametrize("recipe", sorted(RETIRED_RECIPE_NAMES))
def test_neg_every_retired_recipe_is_detected(tmp_path: Path, recipe: str) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "justfile", f"{recipe}:\n    true\n")
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_zero_state(root)
    assert failure.value.code == "CFV7_PURGE_RETIRED_RECIPE"


def test_ops_live_directory_symlink_is_not_silently_skipped(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    target = root / "outside"
    target.mkdir()
    os.symlink(target, root / "src/substituted")
    with pytest.raises(PostPurgeAssuranceError) as failure:
        validate_coverage(root)
    assert failure.value.code == "CFV7_PURGE_SYMLINK_CANDIDATE"


def test_ops_clean_minimal_target_has_zero_retired_matches(tmp_path: Path) -> None:
    report = validate_zero_state(_minimal_root(tmp_path))
    assert report["live_matches"] == 0
    assert int(report["retired_token_classes"]) == len(RETIRED_TOKENS)


def test_purged_target_feature_and_type_integrity() -> None:
    coverage = validate_coverage(ROOT)
    zero_state = validate_zero_state(ROOT)
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    assert int(coverage["live_files"]) > 0
    assert zero_state["live_matches"] == 0
    assert "repository-input = [" in cargo and "operational-state = [" in cargo


def test_post_purge_target_behavior() -> None:
    package = validate_package_contract(ROOT)
    server = (ROOT / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py").read_text(
        encoding="utf-8"
    )
    daemon_client = (
        ROOT / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py"
    ).read_text(encoding="utf-8")
    assert package["runtime_dependencies"] == len(EXPECTED_RUNTIME_DEPENDENCIES)
    assert "FastMCP(" in server
    assert "class DaemonPort(Protocol)" in daemon_client


def test_post_purge_package_feature_operations() -> None:
    coverage = validate_coverage(ROOT)
    package = validate_package_contract(ROOT)
    assert coverage["unreadable"] == coverage["unparsed"] == 0
    assert package["operational_binaries"] == 2
    assert package["proto_files"] == package["rust_generated_bindings"] == 4
    assert package["python_generated_bindings"] == 4
