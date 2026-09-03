"""Fault-seeded tests for the WP64 deployment census and DB23 zero state."""

from __future__ import annotations

from pathlib import Path

import pytest

from tooling.ci.fresh_activation_assurance import (
    DORMANT_AUTHORITY_TOKENS,
    FreshActivationAssuranceError,
    deployment_predecessor_census,
    validate_dormant_authority_zero_state,
)


def _repository(root: Path) -> None:
    for directory in ("src", "tests", "scripts", "tooling/ci", ".github"):
        (root / directory).mkdir(parents=True, exist_ok=True)
    (root / "justfile").write_text("target-check:\n    true\n", encoding="utf-8")
    deployment = root / "contracts/deployment"
    deployment.mkdir(parents=True)
    (deployment / "local-workstation-v1.yaml").write_text(
        "profile_id: local-workstation-v1\n", encoding="utf-8"
    )


def _census(root: Path, **kwargs: object) -> object:
    return deployment_predecessor_census(
        root,
        home=root / "home",
        service_roots=kwargs.get("service_roots", ()),
        product_roots=kwargs.get("product_roots", ()),
        proc_root=kwargs.get("proc_root", root / "missing-proc"),
    )


def test_int_empty_supported_deployment_has_no_predecessor(tmp_path: Path) -> None:
    _repository(tmp_path)
    report = _census(tmp_path)
    assert report == {
        "repository_deployment_contracts": 1,
        "service_candidates": 0,
        "product_state_entries": 0,
        "running_product_processes": 0,
        "predecessor_found": False,
    }


@pytest.mark.parametrize(
    ("category", "token"), sorted(DORMANT_AUTHORITY_TOKENS.items())
)
def test_neg_every_dormant_authority_class_is_detected(
    tmp_path: Path, category: str, token: str
) -> None:
    _repository(tmp_path)
    (tmp_path / "src/legacy.rs").write_text(
        f"// {category}: {token}\n", encoding="utf-8"
    )
    with pytest.raises(FreshActivationAssuranceError) as captured:
        validate_dormant_authority_zero_state(tmp_path)
    assert captured.value.code == "CFV7_DORMANT_AUTHORITY_REACHABLE"


def test_neg_product_service_is_a_design_reopen_trigger(tmp_path: Path) -> None:
    _repository(tmp_path)
    services = tmp_path / "services"
    services.mkdir()
    (services / "codefabric.service").write_text("ExecStart=/old/codefabricd\n")
    with pytest.raises(FreshActivationAssuranceError) as captured:
        _census(tmp_path, service_roots=(services,))
    assert captured.value.code == "CFV7_DEPLOYED_PREDECESSOR_FOUND"


def test_neg_generically_named_product_service_is_detected_from_content(
    tmp_path: Path,
) -> None:
    _repository(tmp_path)
    services = tmp_path / "services"
    services.mkdir()
    (services / "workspace.service").write_text("ExecStart=/old/codefabricd\n")
    with pytest.raises(FreshActivationAssuranceError) as captured:
        _census(tmp_path, service_roots=(services,))
    assert captured.value.code == "CFV7_DEPLOYED_PREDECESSOR_FOUND"


def test_neg_product_state_is_a_design_reopen_trigger(tmp_path: Path) -> None:
    _repository(tmp_path)
    state = tmp_path / "state/codefabric"
    state.mkdir(parents=True)
    (state / "workspace.sqlite3").write_bytes(b"old")
    with pytest.raises(FreshActivationAssuranceError) as captured:
        _census(tmp_path, product_roots=(state,))
    assert captured.value.code == "CFV7_DEPLOYED_PREDECESSOR_FOUND"


def test_neg_running_product_process_is_a_design_reopen_trigger(tmp_path: Path) -> None:
    _repository(tmp_path)
    proc = tmp_path / "proc/41"
    proc.mkdir(parents=True)
    (proc / "comm").write_text("codefabricd\n", encoding="utf-8")
    with pytest.raises(FreshActivationAssuranceError) as captured:
        _census(tmp_path, proc_root=tmp_path / "proc")
    assert captured.value.code == "CFV7_DEPLOYED_PREDECESSOR_FOUND"


def test_neg_python_module_product_process_is_detected(tmp_path: Path) -> None:
    _repository(tmp_path)
    proc = tmp_path / "proc/42"
    proc.mkdir(parents=True)
    (proc / "comm").write_text("python\n", encoding="utf-8")
    (proc / "cmdline").write_bytes(b"python\0-m\0codefabric_cpg_mcp.server\0")
    with pytest.raises(FreshActivationAssuranceError) as captured:
        _census(tmp_path, proc_root=tmp_path / "proc")
    assert captured.value.code == "CFV7_DEPLOYED_PREDECESSOR_FOUND"


def test_int_unrelated_command_argument_does_not_impersonate_product(
    tmp_path: Path,
) -> None:
    _repository(tmp_path)
    proc = tmp_path / "proc/43"
    proc.mkdir(parents=True)
    (proc / "comm").write_text("uv\n", encoding="utf-8")
    (proc / "cmdline").write_bytes(
        b"uv\0run\0--project\0/work/codefabric-cpg-mcp\0pytest\0"
    )
    assert _census(tmp_path, proc_root=tmp_path / "proc") == {
        "repository_deployment_contracts": 1,
        "service_candidates": 0,
        "product_state_entries": 0,
        "running_product_processes": 0,
        "predecessor_found": False,
    }


def test_neg_empty_zero_state_selection_is_not_proof(tmp_path: Path) -> None:
    with pytest.raises(FreshActivationAssuranceError) as captured:
        validate_dormant_authority_zero_state(tmp_path)
    assert captured.value.code == "CFV7_FRESH_ZERO_SELECTION"
