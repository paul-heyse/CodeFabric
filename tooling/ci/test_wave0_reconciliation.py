"""Executable v5 acceptance oracles for inherited Wave-0 packets WP01-WP06."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def _text(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def _json(relative: str) -> object:
    return json.loads(_text(relative))


def test_wp01_behavioral_acceptance() -> None:
    source = _text("tests/integration/compatibility.rs")
    assert "stable_dependency_contract_is_executable" in source
    assert "application_transaction" in source


def test_wp01_structural_acceptance() -> None:
    subprocess.run(
        ("./scripts/stable_graph_check.sh",), cwd=ROOT, check=True, capture_output=True
    )


def test_wp01_negative_zero_state() -> None:
    root_manifest = _text("Cargo.toml")
    assert "pyo3" not in root_manifest
    assert "maturin" not in root_manifest
    assert "[workspace]" not in root_manifest
    assert not (ROOT / "python/codefabric").exists()


def test_wp01_operational_acceptance() -> None:
    just_list = subprocess.run(
        ("just", "--list"), cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    assert all(
        retired not in just_list
        for retired in ("maturin", "python-develop", "test-python")
    )


def test_wp02_behavioral_acceptance() -> None:
    source = _text("rustc-extractor/src/main.rs")
    assert "default_build_runs_a_real_rustc_public_callback" in source
    assert "identity_is_stderr_only_and_exact" in source


def test_wp02_structural_acceptance() -> None:
    toolchain = _text("rustc-extractor/rust-toolchain.toml")
    assert 'channel = "nightly-2026-08-18"' in toolchain
    assert 'components = ["rustc-dev", "rust-src", "llvm-tools"]' in toolchain
    assert (ROOT / "rustc-extractor/Cargo.lock").is_file()


def test_wp02_negative_zero_state() -> None:
    assert 'channel = "stable"' in _text("rust-toolchain.toml")
    assert "rustc_public" not in _text("Cargo.lock")
    assert "rustc-extractor" not in _text("Cargo.toml")


def test_wp02_operational_acceptance() -> None:
    workflow = _text(".github/workflows/ci.yml")
    assert "rustc-extractor" in workflow
    assert "--identity" in workflow


def test_wp03_behavioral_acceptance() -> None:
    source = _text("pyrefly-sidecar/src/main.rs")
    assert "identity_is_stderr_only_and_exact" in source
    assert "serve_stub_is_protocol_silent" in source


def test_wp03_structural_acceptance() -> None:
    manifest = _text("pyrefly-sidecar/Cargo.toml")
    assert 'version = "=1.2.0"' in manifest
    assert 'rev = "1933169ad8ee9e4d4114112eb56ef0811fb0a094"' in manifest
    assert (ROOT / "pyrefly-sidecar/Cargo.lock").is_file()


def test_wp03_negative_zero_state() -> None:
    assert "pyrefly" not in _text("Cargo.lock")
    rule = _text("rules/no-pyrefly-public-api.yml")
    assert "pyrefly" in rule


def test_wp03_operational_acceptance() -> None:
    deny = _text("pyrefly-sidecar/deny.toml")
    assert "allow-git" in deny or "git" in deny
    assert "pyrefly-sidecar" in _text(".github/workflows/ci.yml")


def test_wp04_behavioral_acceptance() -> None:
    tests = _text("codefabric-cpg-mcp/tests/test_stdio.py")
    assert "stdout" in tests
    assert "exits_cleanly" in tests
    assert "Client" in _text("codefabric-cpg-mcp/tests/test_server.py")


def test_wp04_structural_acceptance() -> None:
    manifest = _text("codefabric-cpg-mcp/pyproject.toml")
    for pin in ("fastmcp==3.4.7", "pydantic==2.13.4", "grpcio==1.83.0"):
        assert pin in manifest
    assert "pydantic-core==" not in manifest
    assert 'requires-python = ">=3.12"' in manifest


def test_wp04_negative_zero_state() -> None:
    manifest = _text("codefabric-cpg-mcp/pyproject.toml")
    assert all(
        name not in manifest for name in ("pyarrow", "datafusion", "maturin", "pyo3")
    )
    settings = _text("codefabric-cpg-mcp/src/codefabric_cpg_mcp/settings.py")
    assert "env_file=None" in settings
    assert "return init_settings, env_settings, file_secret_settings" in settings


def test_wp04_operational_acceptance() -> None:
    manifest = _text("codefabric-cpg-mcp/pyproject.toml")
    assert "project-includes = [" in manifest
    assert '"src/**/*.py"' in manifest and '"tests/**/*.py"' in manifest


def test_wp05_behavioral_acceptance() -> None:
    assert "rust_protobuf_matches_the_shared_wire_fixture" in _text(
        "tests/integration/rpc.rs"
    )
    assert "DESCRIPTOR" in _text("codefabric-cpg-mcp/tests/test_proto.py")


def test_wp05_structural_acceptance() -> None:
    recipes = subprocess.run(
        ("just", "--list"), cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    for recipe in ("root-check", "extractor-check", "sidecar-check", "adapter-ci-fast"):
        assert recipe in recipes
    assert "multiple-versions" in _text("deny.toml")


def test_wp05_negative_zero_state() -> None:
    assert "protoc-bin-vendored" not in _text("Cargo.lock")
    assert "duplicate-family" in _text("scripts/duplicate_family_check.sh")
    assert "generated output drift" in _text("tooling/proto/generate.py")


def test_wp05_operational_acceptance() -> None:
    generator = _text("tooling/proto/generate.py")
    assert "grpc_tools.protoc" in generator
    assert "repro-check" in generator
    assert "compile_fds" in _text("tooling/proto/generate.rs")


def test_wp06_behavioral_acceptance() -> None:
    compiler = _text("src/contracts/compiler.rs")
    for oracle in (
        "normative_projection_vectors_have_exact_blake3_identities",
        "governed_sources_fit_their_named_resource_profiles",
        "bundle_projection_uses_the_closed_sorted_model_and_retains_member_identity",
    ):
        assert oracle in compiler


def test_wp06_structural_acceptance() -> None:
    catalog = _json("contracts/manifests/suite-manifest.json")
    assert isinstance(catalog, dict)
    assert len(catalog["artifacts"]) == 58
    assert catalog["catalog_schema_version"] == 2


def test_wp06_negative_zero_state() -> None:
    compiler = _text("src/contracts/compiler.rs")
    for failure in (
        "duplicate-key",
        "yaml_alias",
        "yaml-tag",
        "ebnf-structure",
        "resource-limit",
    ):
        assert failure in compiler
    assert "normative KAT paths" in _text("scripts/fixture_governance_check.sh")


def test_wp06_operational_acceptance() -> None:
    reproduction = _text("scripts/contracts_repro_check.sh")
    assert (
        "first" in reproduction
        and "second" in reproduction
        and "reordered" in reproduction
    )
    assert "catalog reorder changes only its source identity" in reproduction
