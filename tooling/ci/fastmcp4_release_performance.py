"""Validate and summarize the target-only WP65 resource/performance evidence."""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any, NoReturn

from tooling.benchmarks.fastmcp4_release_benchmark import (
    METHOD_ID,
    METHOD_PATH,
    METHOD_REVISION,
    REPORT_SCHEMA,
    ROOT,
    BenchmarkError,
    Method,
    Workload,
    build_summaries,
    load_json,
    load_method,
    nearest_rank,
)

DEFAULT_RAW_REPORT_PATH = Path(
    "contracts/evidence/relational-fabric-v7/wp65-raw-performance-v2.json"
)
DEFAULT_SUMMARY_PATH = Path(
    "contracts/evidence/relational-fabric-v7/wp65-performance-review-v2.json"
)
SUMMARY_SCHEMA = "codefabric.compiled-release-performance.review.v2"
HEX40 = frozenset("0123456789abcdef")
FROZEN_IMPLEMENTATION_PATHS = (
    "src",
    "rustc-extractor/src",
    "pyrefly-sidecar/src",
    "codefabric-cpg-mcp/src",
    "codefabric-cpg-mcp/pyproject.toml",
    "codefabric-cpg-mcp/uv.lock",
    "Cargo.toml",
    "Cargo.lock",
    "contracts/rpc",
    "tests/integration",
    "tests/fixtures/fastmcp4_performance",
    "tooling/benchmarks/fastmcp4_release_benchmark.py",
    "tooling/ci/fastmcp4_release_performance.py",
    "tooling/ci/test_fastmcp4_release_performance.py",
)


class ReleasePerformanceError(ValueError):
    """Fail-closed evidence error with a stable diagnostic code."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _fail(code: str, message: str) -> NoReturn:
    raise ReleasePerformanceError(code, message)


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        _fail(code, message)


def _rows(value: object, context: str) -> list[Mapping[str, Any]]:
    _require(
        isinstance(value, list) and all(isinstance(item, dict) for item in value),
        "WP65_REPORT_SCHEMA_INVALID",
        context,
    )
    assert isinstance(value, list)
    return value


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(isinstance(value, dict), "WP65_REPORT_SCHEMA_INVALID", context)
    assert isinstance(value, dict)
    return value


def _number(value: object, context: str) -> float:
    _require(
        isinstance(value, int | float)
        and not isinstance(value, bool)
        and math.isfinite(float(value))
        and float(value) >= 0,
        "WP65_REPORT_VALUE_INVALID",
        context,
    )
    return float(value)


def _commit(value: object, context: str) -> str:
    _require(
        isinstance(value, str) and len(value) == 40 and set(value).issubset(HEX40),
        "WP65_CANDIDATE_INVALID",
        context,
    )
    return value


def _git(
    root: Path, *arguments: str, check: bool = True
) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=check,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ReleasePerformanceError("WP65_GIT_FAILED", " ".join(arguments)) from error


def _validate_expectation(actual: object, expected: object, context: str) -> None:
    if isinstance(expected, dict):
        _require(
            set(expected).issubset({"minimum", "maximum"}) and bool(expected),
            "WP65_METHOD_EXPECTATION_INVALID",
            context,
        )
        number = _number(actual, context)
        if "minimum" in expected:
            _require(
                number >= _number(expected["minimum"], context),
                "WP65_OBSERVATION_BOUND_EXCEEDED",
                context,
            )
        if "maximum" in expected:
            _require(
                number <= _number(expected["maximum"], context),
                "WP65_OBSERVATION_BOUND_EXCEEDED",
                context,
            )
        return
    _require(actual == expected, "WP65_SEMANTIC_OBSERVATION_DRIFT", context)


def _validate_sample(sample: Mapping[str, Any], workload: Workload) -> None:
    expected_keys = {
        "workload_id",
        "phase",
        "sample_index",
        "state_classification",
        "data_scale",
        "command",
        "start_monotonic_ns",
        "stop_monotonic_ns",
        "elapsed_millis",
        "peak_process_tree_rss_bytes",
        "maximum_processes",
        "stdout_bytes",
        "stderr_bytes",
        "exit_code",
        "observation",
    }
    _require(
        set(sample) == expected_keys
        and sample.get("workload_id") == workload.workload_id
        and sample.get("phase") == "measured"
        and sample.get("state_classification") == workload.state_classification
        and sample.get("data_scale") == workload.data_scale
        and sample.get("command") == list(workload.command)
        and sample.get("exit_code") == 0,
        "WP65_SAMPLE_BINDING_INVALID",
        workload.workload_id,
    )
    start = int(_number(sample.get("start_monotonic_ns"), "start"))
    stop = int(_number(sample.get("stop_monotonic_ns"), "stop"))
    elapsed = _number(sample.get("elapsed_millis"), "elapsed")
    _require(
        stop >= start
        and math.isclose(elapsed, (stop - start) / 1_000_000, rel_tol=0, abs_tol=1e-9),
        "WP65_SAMPLE_CLOCK_INVALID",
        workload.workload_id,
    )
    for field in (
        "peak_process_tree_rss_bytes",
        "maximum_processes",
        "stdout_bytes",
        "stderr_bytes",
    ):
        _number(sample.get(field), f"{workload.workload_id}.{field}")
    observation = _mapping(sample.get("observation"), "observation")
    _require(
        set(observation)
        == (
            {"workload_id", *workload.required_observation_fields}
            if workload.required_observation_fields
            else set()
        ),
        "WP65_OBSERVATION_CENSUS_INVALID",
        workload.workload_id,
    )
    for field, expected in workload.observation_expectations.items():
        _validate_expectation(
            observation.get(field), expected, f"{workload.workload_id}.{field}"
        )


def _validate_workload_bounds(
    samples: Sequence[Mapping[str, Any]], workload: Workload
) -> None:
    ceilings = workload.ceilings
    _require(
        nearest_rank([float(row["elapsed_millis"]) for row in samples], 0.95)
        <= ceilings["elapsed_p95_millis"],
        "WP65_RESOURCE_ENVELOPE_EXCEEDED",
        f"{workload.workload_id}.elapsed",
    )
    for field, ceiling in (
        ("peak_process_tree_rss_bytes", ceilings["peak_process_tree_rss_bytes"]),
        ("maximum_processes", ceilings["maximum_processes"]),
        ("stdout_bytes", ceilings["stdout_bytes"]),
        ("stderr_bytes", ceilings["stderr_bytes"]),
    ):
        _require(
            max(float(row[field]) for row in samples) <= ceiling,
            "WP65_RESOURCE_ENVELOPE_EXCEEDED",
            f"{workload.workload_id}.{field}",
        )


def validate_report_document(report: Mapping[str, Any], method: Method) -> int:
    _require(
        report.get("schema") == REPORT_SCHEMA
        and report.get("method_id") == METHOD_ID
        and report.get("method_revision") == METHOD_REVISION
        and report.get("samples_deleted_after_observation") == 0
        and report.get("failed_samples") == 0
        and report.get("skipped_samples") == 0,
        "WP65_REPORT_IDENTITY_INVALID",
        "root",
    )
    candidate = _commit(report.get("candidate_commit"), "candidate")
    tree = _commit(report.get("candidate_tree"), "tree")
    method_commit = _commit(report.get("method_commit"), "method")
    environment = _mapping(report.get("environment"), "environment")
    _require(
        environment.get("repository_head") == candidate
        and environment.get("repository_tree") == tree
        and environment.get("method_commit") == method_commit
        and environment.get("python_version") == "3.14.7"
        and environment.get("fastmcp_version") == "4.0.0"
        and environment.get("mcp_version") == "2.1.1"
        and environment.get("pydantic_version") == "2.13.4"
        and environment.get("grpcio_version") == "1.83.0"
        and environment.get("host_profile") == "local-workstation-v1"
        and isinstance(environment.get("release_artifacts"), dict),
        "WP65_ENVIRONMENT_INVALID",
        "environment",
    )
    samples = _rows(report.get("samples"), "samples")
    expected_total = sum(workload.samples for workload in method.workloads)
    _require(
        len(samples) == expected_total, "WP65_SAMPLE_COUNT_INVALID", str(len(samples))
    )
    offset = 0
    for workload in method.workloads:
        selected = samples[offset : offset + workload.samples]
        _require(
            [row.get("sample_index") for row in selected]
            == list(range(workload.samples)),
            "WP65_SAMPLE_SEQUENCE_INVALID",
            workload.workload_id,
        )
        for sample in selected:
            _validate_sample(sample, workload)
        _validate_workload_bounds(selected, workload)
        offset += workload.samples
    _require(
        report.get("summaries") == build_summaries(samples, method),
        "WP65_SUMMARY_DRIFT",
        "raw report summaries",
    )
    structural = _rows(report.get("structural_runs"), "structural runs")
    _require(
        [(row.get("contract_id"), row.get("recipe")) for row in structural]
        == list(method.structural_contracts)
        and all(row.get("exit_code") == 0 for row in structural),
        "WP65_STRUCTURAL_PROOF_INCOMPLETE",
        "structural runs",
    )
    return len(samples)


def validate_git_lineage(report: Mapping[str, Any], root: Path = ROOT) -> None:
    candidate = _commit(report.get("candidate_commit"), "candidate")
    method_commit = _commit(report.get("method_commit"), "method")
    tree = _commit(report.get("candidate_tree"), "tree")
    _require(
        _git(root, "rev-parse", f"{candidate}^{{tree}}").stdout.strip() == tree,
        "WP65_CANDIDATE_TREE_DRIFT",
        candidate,
    )
    _require(
        _git(
            root, "merge-base", "--is-ancestor", method_commit, candidate, check=False
        ).returncode
        == 0,
        "WP65_METHOD_NOT_PREREGISTERED",
        method_commit,
    )
    _require(
        _git(
            root, "merge-base", "--is-ancestor", candidate, "HEAD", check=False
        ).returncode
        == 0,
        "WP65_CANDIDATE_NOT_ANCESTOR",
        candidate,
    )
    try:
        candidate_method = subprocess.run(
            ["git", "show", f"{candidate}:{METHOD_PATH.as_posix()}"],
            cwd=root,
            check=True,
            capture_output=True,
            timeout=30,
        ).stdout
        current_method = (root / METHOD_PATH).read_bytes()
    except (OSError, subprocess.SubprocessError) as error:
        raise ReleasePerformanceError(
            "WP65_METHOD_READ_FAILED", str(METHOD_PATH)
        ) from error
    _require(
        candidate_method == current_method,
        "WP65_EXPECTATION_DRIFT",
        METHOD_PATH.as_posix(),
    )
    changed = _git(
        root,
        "diff",
        "--name-only",
        candidate,
        "HEAD",
        "--",
        *FROZEN_IMPLEMENTATION_PATHS,
    ).stdout.splitlines()
    _require(not changed, "WP65_MEASURED_IMPLEMENTATION_DRIFT", repr(changed))


def summary_document(report: Mapping[str, Any], method: Method) -> dict[str, Any]:
    validate_report_document(report, method)
    workload_rows = []
    summaries = _mapping(report.get("summaries"), "summaries")
    for workload in method.workloads:
        summary = _mapping(summaries.get(workload.workload_id), workload.workload_id)
        workload_rows.append(
            {
                "workload_id": workload.workload_id,
                "state_classification": workload.state_classification,
                "data_scale": workload.data_scale,
                "sample_count": workload.samples,
                "elapsed_millis": summary["elapsed_millis"],
                "peak_process_tree_rss_bytes": summary["peak_process_tree_rss_bytes"],
                "verdict": "passed",
            }
        )
    return {
        "schema": SUMMARY_SCHEMA,
        "verdict": "passed_without_bound_relaxation",
        "candidate_commit": report["candidate_commit"],
        "candidate_tree": report["candidate_tree"],
        "method_commit": report["method_commit"],
        "method_id": METHOD_ID,
        "method_revision": METHOD_REVISION,
        "comparison_scope": "target-only-explicit-bounds-no-predecessor-baseline",
        "coverage": sorted(method.required_coverage),
        "measured_sample_count": len(_rows(report.get("samples"), "samples")),
        "structural_contract_count": len(method.structural_contracts),
        "samples_deleted_after_observation": 0,
        "workloads": workload_rows,
        "limitations": [
            "compile is target-cold with the current shared sccache rather than a cache-cold dependency rebuild",
            "deterministic assurance scales validate the supported local topology and are not a capacity forecast for arbitrary repositories",
            "single-sample destructive setup scenarios report no bootstrap confidence interval",
        ],
    }


def validate_summary(
    report: Mapping[str, Any], summary: Mapping[str, Any], method: Method
) -> None:
    _require(
        summary == summary_document(report, method),
        "WP65_REVIEW_SUMMARY_DRIFT",
        "review",
    )


def validate_evidence(
    *,
    root: Path = ROOT,
    raw_path: Path = DEFAULT_RAW_REPORT_PATH,
    summary_path: Path = DEFAULT_SUMMARY_PATH,
    check_git: bool = True,
) -> int:
    method = load_method(root)
    report = load_json(raw_path if raw_path.is_absolute() else root / raw_path)
    summary = load_json(
        summary_path if summary_path.is_absolute() else root / summary_path
    )
    count = validate_report_document(report, method)
    validate_summary(report, summary, method)
    if check_git:
        validate_git_lineage(report, root)
    return count


def write_summary(
    *, root: Path, raw_path: Path, summary_path: Path
) -> Mapping[str, Any]:
    method = load_method(root)
    report = load_json(raw_path if raw_path.is_absolute() else root / raw_path)
    validate_report_document(report, method)
    validate_git_lineage(report, root)
    summary = summary_document(report, method)
    destination = summary_path if summary_path.is_absolute() else root / summary_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(summary, stream, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
    except OSError as error:
        raise ReleasePerformanceError(
            "WP65_REVIEW_ALREADY_EXISTS", str(destination)
        ) from error
    return summary


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("method-integrity")
    raw = subparsers.add_parser("validate-raw")
    raw.add_argument("--raw", type=Path, default=DEFAULT_RAW_REPORT_PATH)
    raw.add_argument("--no-git-check", action="store_true")
    summarize = subparsers.add_parser("summarize")
    summarize.add_argument("--raw", type=Path, default=DEFAULT_RAW_REPORT_PATH)
    summarize.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY_PATH)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--raw", type=Path, default=DEFAULT_RAW_REPORT_PATH)
    validate.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY_PATH)
    validate.add_argument("--no-git-check", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "method-integrity":
            method = load_method(ROOT)
            print(
                json.dumps(
                    {
                        "workload_count": len(method.workloads),
                        "coverage_count": len(method.required_coverage),
                        "structural_contract_count": len(method.structural_contracts),
                    },
                    sort_keys=True,
                )
            )
        elif arguments.command == "validate-raw":
            method = load_method(ROOT)
            report = load_json(
                arguments.raw if arguments.raw.is_absolute() else ROOT / arguments.raw
            )
            count = validate_report_document(report, method)
            if not arguments.no_git_check:
                validate_git_lineage(report, ROOT)
            print(count)
        elif arguments.command == "summarize":
            summary = write_summary(
                root=ROOT, raw_path=arguments.raw, summary_path=arguments.summary
            )
            print(json.dumps({"verdict": summary["verdict"]}, sort_keys=True))
        else:
            print(
                validate_evidence(
                    root=ROOT,
                    raw_path=arguments.raw,
                    summary_path=arguments.summary,
                    check_git=not arguments.no_git_check,
                )
            )
        return 0
    except (ReleasePerformanceError, BenchmarkError) as error:
        code = getattr(error, "code", "WP65_VALIDATION_FAILED")
        print(f"{code}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
