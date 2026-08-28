#!/usr/bin/env python3
"""Run the seven ontology-fabric pin probes and emit target-only content-addressed reports."""

from __future__ import annotations

import argparse
import json
import subprocess
import tomllib
from pathlib import Path

from blake3 import blake3

ROOT = Path(__file__).resolve().parents[1]
REPORT_ROOT = ROOT / "target/ontology-fabric-probes"

PROBES = {
    "PR-1": {
        "branch": "domain-typed-fsb-literals",
        "fallback": "storage-typed-literal-rewrite",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(odf_domain_conformant_plans_execute) | test(odf_cross_domain_plan_rejection)",
            "--no-tests=fail",
        ],
    },
    "PR-2": {
        "branch": "delta-binary-with-scan-schema-reattachment",
        "fallback": "per-scan-reattachment-cast",
        "command": [
            "cargo", "nextest", "run", "--locked", "--test", "integration", "-E",
            "test(delta_43a0cf10_provider_pruning_contract_wp04_structural_delta_provider_path)",
            "--no-tests=fail",
        ],
    },
    "PR-3a": {
        "branch": "flat-source-span",
        "fallback": "flat-source-span",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(odf_span_decision_conformance) | test(odf_span_incoherence_rejection)",
            "--no-tests=fail",
        ],
    },
    "PR-3b": {
        "branch": "flat-source-span-pruning",
        "fallback": "flat-source-span-pruning",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(odf_span_pruning_parity)", "--no-tests=fail",
        ],
    },
    "PR-4": {
        "branch": "row-count-and-null-statistics-only",
        "fallback": "row-count-and-null-statistics-only",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(datafusion_55_effective_provider_statistics_contract)",
            "--no-tests=fail",
        ],
    },
    "PR-5": {
        "branch": "parquet-arrow-schema-extension-metadata",
        "fallback": "manifest-carried-extension-metadata",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(odf_id_domain_lowering_conformance)", "--no-tests=fail",
        ],
    },
    "PR-6": {
        "branch": "projection-driven-decoration",
        "fallback": "narrow-decoration-per-view",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(odf_decoration_plan_shape)", "--no-tests=fail",
        ],
    },
    "PR-7": {
        "branch": "view-types-disabled",
        "fallback": "view-types-disabled",
        "performance_posture": "owner-waived-no-measurement",
        "command": [
            "cargo", "nextest", "run", "--locked", "--lib", "-E",
            "test(datafusion_55_serving_equivalence)", "--no-tests=fail",
        ],
    },
}


def _run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def worktree_fingerprint() -> str:
    status = _run(["git", "status", "--porcelain=v1", "-z"])
    diff = _run(["git", "diff", "--binary", "--no-ext-diff"])
    cached = _run(["git", "diff", "--cached", "--binary", "--no-ext-diff"])
    if status.returncode or diff.returncode or cached.returncode:
        raise RuntimeError("cannot fingerprint the worktree")
    payload = status.stdout.encode() + diff.stdout.encode() + cached.stdout.encode()
    for record in status.stdout.split("\0"):
        if record.startswith("?? "):
            path = ROOT / record[3:]
            if path.is_file() and not path.is_relative_to(REPORT_ROOT):
                payload += record.encode() + path.read_bytes()
    return f"b3:{blake3(payload).hexdigest()}"


def stack_identity() -> dict[str, str]:
    cargo_bytes = (ROOT / "Cargo.lock").read_bytes()
    cargo = tomllib.loads(cargo_bytes.decode())
    fabric = (ROOT / "docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md").read_text(encoding="utf-8")
    packages = {package["name"]: package for package in cargo["package"]}
    delta_source = packages["deltalake"]["source"]
    delta_revision = delta_source.rsplit("#", maxsplit=1)[-1]
    return {
        "cargo_lock_digest": f"b3:{blake3(cargo_bytes).hexdigest()}",
        "fabric_spec_digest": f"b3:{blake3(fabric.encode()).hexdigest()}",
        "datafusion": packages["datafusion"]["version"],
        "arrow": packages["arrow"]["version"],
        "delta_revision": delta_revision,
    }


def run_suite() -> list[Path]:
    before = worktree_fingerprint()
    REPORT_ROOT.mkdir(parents=True, exist_ok=True)
    outputs = []
    identity = stack_identity()
    for probe_id, contract in PROBES.items():
        result = _run(contract["command"])
        report = {
            "contract": "codefabric.ontology-fabric-probe-observation.v1",
            "probe_id": probe_id,
            "resolved_stack": identity,
            "environment_digest": f"b3:{blake3(str(ROOT).encode()).hexdigest()}",
            "session_digest": f"b3:{blake3(json.dumps(contract, sort_keys=True).encode()).hexdigest()}",
            "workload_digest": f"b3:{blake3(' '.join(contract['command']).encode()).hexdigest()}",
            "fixture_digest": identity["cargo_lock_digest"],
            "command": contract["command"],
            "selected_branch": contract["branch"],
            "fallback": contract["fallback"],
            "performance_posture": contract.get("performance_posture", "not-applicable"),
            "verdict": "pass" if result.returncode == 0 else "fail",
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
        payload = json.dumps(report, sort_keys=True, separators=(",", ":")).encode()
        digest = blake3(payload).hexdigest()
        path = REPORT_ROOT / f"{probe_id.lower()}-{digest}.json"
        path.write_bytes(payload + b"\n")
        outputs.append(path)
        if result.returncode:
            raise RuntimeError(f"{probe_id} failed; see {path}")
    after = worktree_fingerprint()
    if after != before:
        raise RuntimeError("probe suite changed tracked or pre-existing untracked worktree content")
    return outputs


def record_reviewed_decision(reports: list[Path]) -> Path:
    """Bind one accepted design branch to every fresh probe observation."""
    identity = stack_identity()
    decisions = []
    for path in reports:
        payload = path.read_bytes().rstrip(b"\n")
        report = json.loads(payload)
        probe_id = report["probe_id"]
        contract = PROBES[probe_id]
        if report["verdict"] != "pass" or report["resolved_stack"] != identity:
            raise RuntimeError(f"{probe_id} is not eligible for decision recording")
        decisions.append(
            {
                "probe_id": probe_id,
                "report_digest": f"b3:{blake3(payload).hexdigest()}",
                "pin_config_digest": identity["cargo_lock_digest"],
                "selected_branch": contract["branch"],
                "fallback": contract["fallback"],
                "rationale": "accepted v2 design branch confirmed by the pinned correctness probe",
            }
        )
    decision = {
        "contract": "codefabric.ontology-fabric-probe-decision.v1",
        "reviewer": "plan-owner-v2-implementation-authorization",
        "reviewed_at": "2026-08-28",
        "resolved_stack": identity,
        "decisions": sorted(decisions, key=lambda item: item["probe_id"]),
    }
    payload = json.dumps(decision, sort_keys=True, separators=(",", ":")).encode()
    path = REPORT_ROOT / f"decision-{blake3(payload).hexdigest()}.json"
    path.write_bytes(payload + b"\n")
    return path


def validate_reviewed_decision(path: Path) -> dict[str, object]:
    decision = json.loads(path.read_text(encoding="utf-8"))
    if decision.get("contract") != "codefabric.ontology-fabric-probe-decision.v1":
        raise RuntimeError("probe decision contract is invalid")
    if decision.get("resolved_stack") != stack_identity():
        raise RuntimeError("probe decision pin/config identity drifted")
    records = decision.get("decisions")
    if not isinstance(records, list) or len(records) != len(PROBES):
        raise RuntimeError("probe decision census is incomplete")
    for record in records:
        probe_id = record["probe_id"]
        contract = PROBES.get(probe_id)
        if contract is None:
            raise RuntimeError(f"unknown reviewed probe {probe_id}")
        if (
            record.get("selected_branch") != contract["branch"]
            or record.get("fallback") != contract["fallback"]
            or record.get("pin_config_digest") != stack_identity()["cargo_lock_digest"]
        ):
            raise RuntimeError(f"reviewed probe {probe_id} drifted")
        digest = record.get("report_digest", "")
        candidates = list(REPORT_ROOT.glob(f"{probe_id.lower()}-*.json"))
        if not any(
            f"b3:{blake3(candidate.read_bytes().rstrip(b'\n')).hexdigest()}" == digest
            for candidate in candidates
        ):
            raise RuntimeError(f"reviewed probe report {probe_id} is missing or changed")
    return decision


def validate_contract() -> None:
    assert set(PROBES) == {"PR-1", "PR-2", "PR-3a", "PR-3b", "PR-4", "PR-5", "PR-6", "PR-7"}
    assert PROBES["PR-7"]["performance_posture"] == "owner-waived-no-measurement"
    assert all(contract["command"][-1] == "--no-tests=fail" for contract in PROBES.values())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "run"))
    args = parser.parse_args()
    validate_contract()
    if args.command == "run":
        reports = run_suite()
        for path in reports:
            print(path.relative_to(ROOT))
        print(record_reviewed_decision(reports).relative_to(ROOT))


if __name__ == "__main__":
    main()
