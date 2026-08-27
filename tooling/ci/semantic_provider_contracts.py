"""Validate WP36 semantic-provider fault, telemetry, sandbox, and dispatch contracts."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "src/provider_runtime.rs"
SANDBOX = ROOT / "src/provider_sandbox.rs"
FAULTS = ROOT / "contracts/faults/fault-point-registry.yaml"
TELEMETRY = ROOT / "contracts/observability/semantic-provider-telemetry-contract.yaml"
PROFILES = ROOT / "contracts/registry/provider-resource-profile-registry.yaml"

FAULT_CODES = (
    "PROVIDER_ADMISSION",
    "PROVIDER_CHILD_LAUNCH",
    "PROVIDER_HANDSHAKE",
    "PROVIDER_STAGE_CREATION",
    "PROVIDER_CHUNK_WRITE",
    "PROVIDER_CHUNK_ACCEPT",
    "PROVIDER_CHUNK_REJECT",
    "PROVIDER_TERMINAL_VERIFY",
    "PROVIDER_CANCELLATION",
    "PROVIDER_KILL",
    "PROVIDER_CLEANUP",
    "PROVIDER_JOURNAL_TRANSITION",
)

TELEMETRY_FIELDS = (
    ("provider_phase", "code", "provider-run"),
    ("input_bytes", "bytes", "provider-run"),
    ("output_bytes", "bytes", "provider-run"),
    ("memory_high_water", "bytes", "provider-run"),
    ("queue_depth", "jobs", "runtime-sample"),
    ("chunk_count", "chunks", "provider-run"),
    ("cache_hits", "entries", "provider-run"),
    ("cancellation_count", "requests", "provider-run"),
    ("visited_node_count", "nodes", "provider-run"),
    ("scope_count", "scopes", "provider-run"),
    ("binding_count", "bindings", "provider-run"),
    ("reference_count", "references", "provider-run"),
    ("unresolved_reference_count", "references", "provider-run"),
    ("failure_count", "failures", "provider-run"),
    ("wall_time", "microseconds", "provider-run"),
)


def load(path: Path) -> dict:
    value = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"{path.relative_to(ROOT)} must contain a mapping")
    return value


def rust_constant_strings(source: str, name: str) -> tuple[str, ...]:
    match = re.search(
        rf"pub const {name}: \[&str; \d+\] = \[(.*?)\];",
        source,
        re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"Rust constant {name} is absent")
    return tuple(re.findall(r'"([A-Z][A-Z0-9_-]+)"', match.group(1)))


def check_faults() -> None:
    runtime = RUNTIME.read_text(encoding="utf-8")
    runtime_codes = rust_constant_strings(
        runtime, "SEMANTIC_PROVIDER_FAULT_POINT_CODES"
    )
    records = load(FAULTS).get("records", [])
    released = tuple(
        record["code"] for record in records if record["code"].startswith("PROVIDER_")
    )
    if runtime_codes != FAULT_CODES or released != FAULT_CODES:
        raise AssertionError(
            f"semantic fault census differs: runtime={runtime_codes}, registry={released}"
        )
    for record in records:
        if record["code"] in FAULT_CODES:
            if record.get("production_exposable") is not False:
                raise AssertionError(
                    f"{record['code']} must not be production exposable"
                )
            if not record.get("expected_invariants") or not record.get("scenarios"):
                raise AssertionError(f"{record['code']} lacks invariant/scenario proof")


def check_observability() -> None:
    contract = load(TELEMETRY)
    metrics = tuple(
        (metric["name"], metric["unit"], metric["lifecycle"])
        for metric in contract.get("metrics", [])
    )
    if metrics != TELEMETRY_FIELDS:
        raise AssertionError(f"telemetry fields differ: {metrics}")
    labels = contract.get("label_policy", {})
    forbidden = set(labels.get("forbidden_labels", []))
    if (
        not {"workspace_id", "provider_run_id", "source_path", "diagnostic_text"}
        <= forbidden
    ):
        raise AssertionError("high-cardinality semantic labels are not closed")
    profiles = load(PROFILES).get("records", [])
    bindings = {
        provider: profile["profile_id"]
        for profile in profiles
        for provider in profile.get("provider_ids", [])
        if provider in {"ruff-python", "pyrefly-python", "rustc-mir"}
    }
    if bindings != contract.get("resource_profiles"):
        raise AssertionError(f"resource profile bindings differ: {bindings}")
    sandbox = SANDBOX.read_text(encoding="utf-8")
    for token in ("UntrustedSandboxed", "TrustedLocal", "ParsingOnly"):
        if token not in sandbox:
            raise AssertionError(f"sandbox trust profile {token} is absent")

    direct = []
    for path in (ROOT / "src").rglob("*.rs"):
        relative = path.relative_to(ROOT).as_posix()
        if relative == "src/provider_runtime/fixture.rs":
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"\brun_(?:pyrefly|rustc)\s*\(", text):
            direct.append(relative)
    if direct:
        raise AssertionError(
            f"direct semantic provider invocation escaped ProviderRuntime: {direct}"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("faults", "observability"))
    args = parser.parse_args()
    if args.mode == "faults":
        check_faults()
        print(f"semantic provider fault-point check passed: {len(FAULT_CODES)} seams")
    else:
        check_observability()
        print(
            f"semantic provider observability check passed: {len(TELEMETRY_FIELDS)} fields"
        )


if __name__ == "__main__":
    main()
