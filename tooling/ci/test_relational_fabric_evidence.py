"""Focused positive and fail-closed tests for WP22 evidence contracts."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

from tooling.ci.relational_fabric_evidence import (
    COMPARATOR_PATH,
    CORPUS_PATH,
    EVIDENCE_ROOT,
    PLAN_PATH,
    ROOT,
    TRANSACTION_PATH,
    EvidenceContractError,
    _canonical_sha256,
    main,
    validate_comparator_manifest,
    validate_corpus,
    validate_expectation_independence,
    validate_independent_evidence_dag,
    validate_late_authoring_zero_state,
    validate_review_transaction,
)


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
    _git(root, "config", "user.name", "WP22 Test")
    _git(root, "config", "user.email", "wp22@example.invalid")


def _copy_evidence_with_sources(destination: Path) -> Path:
    shutil.copytree(ROOT / EVIDENCE_ROOT, destination / EVIDENCE_ROOT)
    corpus = json.loads((destination / CORPUS_PATH).read_text(encoding="utf-8"))
    for source in corpus["sources"]:
        target = destination / source["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / source["path"], target)
    return destination


def _rebind_transaction(root: Path, *, accepted: bool) -> dict[str, object]:
    transaction_path = root / TRANSACTION_PATH
    transaction = json.loads(transaction_path.read_text(encoding="utf-8"))
    candidate = transaction["candidate"]
    for name in ("corpus", "schema", "comparator"):
        candidate[f"{name}_sha256"] = _sha256(root / candidate[f"{name}_path"])
    transaction["evidence_set_id"] = f"sha256:{_canonical_sha256(candidate)}"
    transaction["decision"] = "accepted" if accepted else "rejected"
    transaction["blocking_conditions"] = [] if accepted else ["fixture rejection"]
    projection = dict(transaction)
    projection.pop("transaction_id")
    transaction["transaction_id"] = f"sha256:{_canonical_sha256(projection)}"
    _write_json(transaction_path, transaction)
    return transaction


def test_repository_corpus_is_strict_complete_and_source_bound() -> None:
    corpus = validate_corpus(ROOT)
    assert len(corpus["expectations"]) == 25
    assert len(corpus["sources"]) == 20


def test_repository_review_is_immutable_but_rejected() -> None:
    transaction = validate_review_transaction(ROOT, require_accepted=False)
    assert transaction["decision"] == "rejected"
    with pytest.raises(EvidenceContractError, match="evidence review is rejected"):
        validate_review_transaction(ROOT, require_accepted=True)


def test_strict_corpus_rejects_target_output_provenance(tmp_path: Path) -> None:
    root = _copy_evidence_with_sources(tmp_path / "repo")
    corpus_path = root / CORPUS_PATH
    corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
    source = corpus["sources"][0]
    source["path"] = "src/target_output.json"
    target = root / source["path"]
    target.parent.mkdir(parents=True)
    target.write_text("{}\n", encoding="utf-8")
    source["sha256"] = _sha256(target)
    _write_json(corpus_path, corpus)
    with pytest.raises(EvidenceContractError, match="forbidden provenance"):
        validate_corpus(root)


def test_strict_corpus_rejects_missing_query_form(tmp_path: Path) -> None:
    root = _copy_evidence_with_sources(tmp_path / "repo")
    corpus_path = root / CORPUS_PATH
    corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
    corpus["expectations"] = [
        item for item in corpus["expectations"] if item["query_form"] != "FIND_PATHS"
    ]
    _write_json(corpus_path, corpus)
    with pytest.raises(EvidenceContractError, match="query form coverage differs"):
        validate_corpus(root)


def test_strict_corpus_rejects_relation_row_shape_drift(tmp_path: Path) -> None:
    root = _copy_evidence_with_sources(tmp_path / "repo")
    corpus_path = root / CORPUS_PATH
    corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
    row = corpus["expectations"][0]["expected"]["relations"][0]["rows"][0]
    row["producer_generated_extra"] = True
    _write_json(corpus_path, corpus)
    with pytest.raises(EvidenceContractError, match="keys must exactly match columns"):
        validate_corpus(root)


def test_plan_dag_places_wp22_before_every_consumer(tmp_path: Path) -> None:
    validate_independent_evidence_dag(ROOT / PLAN_PATH)
    plan = (ROOT / PLAN_PATH).read_text(encoding="utf-8")
    broken = plan.replace(
        "WP01 and WP22. The frozen comparator",
        "WP01. The frozen comparator",
        1,
    )
    broken_path = tmp_path / "broken-plan.md"
    broken_path.write_text(broken, encoding="utf-8")
    with pytest.raises(EvidenceContractError, match="lack transitive WP22"):
        validate_independent_evidence_dag(broken_path)


def test_independence_rejects_production_ingress_reference(tmp_path: Path) -> None:
    root = _copy_evidence_with_sources(tmp_path / "repo")
    producer = root / "src/producer.rs"
    producer.parent.mkdir(parents=True)
    producer.write_text(
        'const EXPECTATIONS: &str = "contracts/acceptance/relational-fabric-v1";\n',
        encoding="utf-8",
    )
    with pytest.raises(
        EvidenceContractError, match="references the expectation ingress"
    ):
        validate_expectation_independence(root)


def test_late_authoring_requires_committed_early_bytes(tmp_path: Path) -> None:
    root = _copy_evidence_with_sources(tmp_path / "repo")
    transaction_path = root / TRANSACTION_PATH
    transaction_template = json.loads(transaction_path.read_text(encoding="utf-8"))
    transaction_path.unlink()
    _init_git(root)
    _git(root, "add", ".")
    _git(root, "commit", "-q", "-m", "freeze evidence")
    freeze = _git(root, "rev-parse", "HEAD")

    _write_json(transaction_path, transaction_template)
    transaction = _rebind_transaction(root, accepted=True)
    transaction["authoring_boundary"] = {
        "required_predecessor_packet": "WP01",
        "required_before_packet": "WP02",
        "evidence_freeze_commit": freeze,
        "consumer_state_path": "docs/plans/state/fixture-state.json",
        "status": "proved-early",
    }
    projection = dict(transaction)
    projection.pop("transaction_id")
    transaction["transaction_id"] = f"sha256:{_canonical_sha256(projection)}"
    _write_json(transaction_path, transaction)
    _git(root, "add", TRANSACTION_PATH.as_posix())
    _git(root, "commit", "-q", "-m", "accept evidence")

    consumer = root / "src/consumer.rs"
    consumer.parent.mkdir(parents=True)
    consumer.write_text("pub fn consumer() {}\n", encoding="utf-8")
    _git(root, "add", "src/consumer.rs")
    _git(root, "commit", "-q", "-m", "first consumer")
    consumer_commit = _git(root, "rev-parse", "HEAD")
    _write_json(
        root / "docs/plans/state/fixture-state.json",
        {"packets": {"WP02": {"proving_commit": consumer_commit}}},
    )
    validate_late_authoring_zero_state(root)

    transaction = json.loads(transaction_path.read_text(encoding="utf-8"))
    transaction["authoring_boundary"] = {
        **transaction["authoring_boundary"],
        "status": "unproved-late",
    }
    projection = dict(transaction)
    projection.pop("transaction_id")
    transaction["transaction_id"] = f"sha256:{_canonical_sha256(projection)}"
    _write_json(transaction_path, transaction)
    with pytest.raises(EvidenceContractError, match="late/unproved"):
        validate_late_authoring_zero_state(root)


def test_comparator_reconstruction_contract_can_be_exact(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    _init_git(root)
    (root / "Cargo.lock").write_text("# exact lock\n", encoding="utf-8")
    (root / "Cargo.toml").write_text(
        "[package]\nname='legacy'\nversion='0.1.0'\n", encoding="utf-8"
    )
    entrypoint = root / "tooling/comparator.rs"
    entrypoint.parent.mkdir(parents=True)
    entrypoint.write_text("fn main() {}\n", encoding="utf-8")
    frozen = root / "fixtures/input.json"
    frozen.parent.mkdir(parents=True)
    frozen.write_text("{}\n", encoding="utf-8")
    _git(root, "add", ".")
    _git(root, "commit", "-q", "-m", "historical comparator")
    commit = _git(root, "rev-parse", "HEAD")
    tree = _git(root, "rev-parse", "HEAD^{tree}")

    artifact = root / EVIDENCE_ROOT / "archive/comparator"
    artifact.parent.mkdir(parents=True)
    artifact.write_bytes(b"exact comparator bytes")
    os.chmod(artifact, 0o444)
    manifest = {
        "schema_version": 1,
        "comparator_id": "fixture-comparator",
        "status": "captured",
        "historical_source": {
            "repository_commit": commit,
            "tree_oid": tree,
            "entrypoint": "tooling/comparator.rs",
            "objects": [
                {
                    "path": "Cargo.toml",
                    "object_id": _git(root, "rev-parse", "HEAD:Cargo.toml"),
                },
                {
                    "path": "Cargo.lock",
                    "object_id": _git(root, "rev-parse", "HEAD:Cargo.lock"),
                },
                {
                    "path": "tooling/comparator.rs",
                    "object_id": _git(root, "rev-parse", "HEAD:tooling/comparator.rs"),
                },
            ],
        },
        "toolchain": {
            "status": "exact",
            "rustc_verbose_version": "rustc 1.95.0 fixture",
            "rustc_binary_sha256": "1" * 64,
            "cargo_verbose_version": "cargo 1.95.0 fixture",
            "cargo_binary_sha256": "2" * 64,
            "host_triple": "fixture-host",
            "lockfile_sha256": _sha256(root / "Cargo.lock"),
        },
        "frozen_inputs": [
            {
                "path": "fixtures/input.json",
                "object_id": _git(root, "rev-parse", "HEAD:fixtures/input.json"),
                "purpose": "fixture",
            }
        ],
        "build": {
            "argv": ["cargo", "build", "--offline"],
            "environment": {},
            "network": "deny",
        },
        "artifact": {
            "status": "captured",
            "path": artifact.relative_to(root).as_posix(),
            "sha256": _sha256(artifact),
        },
        "comparison_contract": {
            "oracle_authority": "independent-expectations-only",
            "legacy_role": "comparison-evidence-only",
            "row_semantics": "decoded rows",
            "ordering": "canonical",
            "unknown_policy": "never passes",
        },
        "isolation": {
            "status": "enforced",
            "backend": "fixture-read-only-sandbox",
            "network": "deny",
            "filesystem": "read-only",
            "write_allowlist": [],
            "read_only_inputs": ["artifact"],
            "environment_allowlist": ["PATH"],
            "stdout_contract": "decoded rows",
        },
        "limitations": ["test fixture"],
    }
    _write_json(root / COMPARATOR_PATH, manifest)
    validate_comparator_manifest(root, require_available=True)


def test_repository_cli_has_two_green_and_four_fail_closed_commands() -> None:
    assert main(["--root", str(ROOT), "independent-evidence-dag", str(PLAN_PATH)]) == 0
    assert main(["--root", str(ROOT), "expectation-independence"]) == 0
    assert main(["--root", str(ROOT), "early-evidence-acceptance"]) == 1
    assert main(["--root", str(ROOT), "comparison-engine-isolation"]) == 1
    assert main(["--root", str(ROOT), "late-authoring-zero-state"]) == 1
    assert main(["--root", str(ROOT), "legacy-comparator-reconstruction"]) == 1
