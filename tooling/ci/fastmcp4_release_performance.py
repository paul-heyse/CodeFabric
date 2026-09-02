"""Validate and record target-only WP50 release/performance evidence.

The evidence transaction is append-only, binds one candidate commit and tree,
and admits only current v5 target inputs.  Raw benchmark samples remain the
authority for statistics; summaries, budget verdicts, and entry hashes are
recomputed.  No predecessor comparator, historical result, or cache state is
accepted as release evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, NoReturn

from tooling.benchmarks.fastmcp4_release_benchmark import (
    METHOD_PATH,
    REGISTERED_METHOD_PATH,
    BenchmarkError,
    Method,
    Workload,
    canonical_json,
    distribution_summary,
    load_method,
    sha256_file,
    validate_report_document,
)

ROOT = Path(__file__).resolve().parents[2]
BENCHMARK_PATH = Path("tooling/benchmarks/fastmcp4_release_benchmark.py")
CONTROL_PATH = Path("tooling/benchmarks/fastmcp4_minimal_control.py")
VALIDATOR_PATH = Path("tooling/ci/fastmcp4_release_performance.py")
VALIDATOR_TEST_PATH = Path("tooling/ci/test_fastmcp4_release_performance.py")
DEFAULT_RAW_REPORT_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp50-raw-performance-v1.json"
)
DEFAULT_TRANSACTION_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp50-release-performance-v1.jsonl"
)
DEFAULT_REVIEW_PATH = Path(
    "contracts/evidence/relational-fabric-v5/wp50-release-performance-review-v1.json"
)

TRANSACTION_ID = "relational-fabric-v5-wp50-release-performance-r1"
ENTRY_SCHEMA = "codefabric.fastmcp4-release-performance.entry.v1"
REVIEW_SCHEMA = "codefabric.fastmcp4-release-performance.review.v1"
ENTRY_KINDS = (
    "transaction_opened",
    "post_purge_release_matrix",
    "raw_performance_observation",
    "history_independence",
    "resource_and_budget_verdict",
    "independent_review",
)
INPUT_PATHS = (
    METHOD_PATH,
    REGISTERED_METHOD_PATH,
    BENCHMARK_PATH,
    CONTROL_PATH,
    VALIDATOR_PATH,
    VALIDATOR_TEST_PATH,
    Path("Cargo.lock"),
    Path("codefabric-cpg-mcp/uv.lock"),
    Path("contracts/rpc/cpg_query_service.proto"),
    Path("tooling/proto/production-descriptor.pb"),
    Path(
        "docs/authoritative_design/"
        "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md"
    ),
    Path(
        "docs/plans/"
        "codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md"
    ),
)
REQUIRED_RELEASE_RECIPES = (
    "root-check",
    "root-test",
    "features-each",
    "extractor-ci-fast",
    "sidecar-ci-fast",
    "adapter-ci-fast",
    "proto-check",
    "stable-graph-check",
    "governance-scan",
    "fastmcp4-live-surface-integrity-check",
    "fastmcp4-post-purge-behavior-check",
    "fastmcp4-decommission-zero-state-check",
    "fastmcp4-package-build-check",
)
FORBIDDEN_INPUT_COMPONENTS = frozenset(
    {
        "relational-fabric-v3",
        "relational-fabric-v4",
        "data_fabric_upgrade",
        "successor_evidence_contracts_v4.py",
        "successor_evidence_issuance_v4.py",
        "relational_fabric_release.py",
        "data_fabric_revision_benchmark.rs",
    }
)
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
MAX_JSON_BYTES = 64 * 1024 * 1024
MAX_RELEASE_STDOUT_BYTES = 16 * 1024 * 1024
MAX_RELEASE_STDERR_BYTES = 512 * 1024


class ReleasePerformanceError(ValueError):
    """Fail-closed release/performance evidence error with a stable code."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _fail(code: str, message: str) -> NoReturn:
    raise ReleasePerformanceError(code, message)


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        _fail(code, message)


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("WP50_JSON_DUPLICATE_MEMBER", key)
        result[key] = value
    return result


def _reject_nonfinite(value: str) -> NoReturn:
    _fail("WP50_JSON_NONFINITE", value)


def _load_json(path: Path, *, maximum_bytes: int = MAX_JSON_BYTES) -> dict[str, Any]:
    try:
        metadata = path.stat()
        _require(
            path.is_file() and metadata.st_size <= maximum_bytes,
            "WP50_JSON_SIZE_INVALID",
            str(path),
        )
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicates,
            parse_constant=_reject_nonfinite,
        )
    except ReleasePerformanceError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ReleasePerformanceError("WP50_JSON_INVALID", str(path)) from error
    _require(isinstance(value, dict), "WP50_JSON_ROOT_INVALID", str(path))
    return value


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        metadata = path.stat()
        _require(
            path.is_file() and metadata.st_size <= MAX_JSON_BYTES,
            "WP50_JSON_SIZE_INVALID",
            str(path),
        )
        lines = path.read_text(encoding="utf-8").splitlines()
    except ReleasePerformanceError:
        raise
    except (OSError, UnicodeError) as error:
        raise ReleasePerformanceError("WP50_JSON_INVALID", str(path)) from error
    _require(
        bool(lines) and all(line.strip() for line in lines),
        "WP50_TRANSACTION_INVALID",
        "lines",
    )
    entries: list[dict[str, Any]] = []
    for index, line in enumerate(lines, 1):
        try:
            value = json.loads(
                line,
                object_pairs_hook=_reject_duplicates,
                parse_constant=_reject_nonfinite,
            )
        except (json.JSONDecodeError, ReleasePerformanceError) as error:
            raise ReleasePerformanceError(
                "WP50_JSON_INVALID", f"line {index}"
            ) from error
        _require(isinstance(value, dict), "WP50_TRANSACTION_INVALID", f"line {index}")
        entries.append(value)
    return entries


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(isinstance(value, Mapping), "WP50_EVIDENCE_SCHEMA_INVALID", context)
    assert isinstance(value, Mapping)
    return value


def _rows(value: object, context: str) -> list[Mapping[str, Any]]:
    _require(isinstance(value, list), "WP50_EVIDENCE_SCHEMA_INVALID", context)
    assert isinstance(value, list)
    return [_mapping(item, f"{context}[{index}]") for index, item in enumerate(value)]


def _canonical_sha256(value: Mapping[str, Any]) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def _entry(
    kind: str, payload: Mapping[str, Any], sequence: int, previous: str | None
) -> dict[str, Any]:
    entry: dict[str, Any] = {
        "schema": ENTRY_SCHEMA,
        "transaction_id": TRANSACTION_ID,
        "sequence": sequence,
        "kind": kind,
        "previous_entry_sha256": previous,
        "payload": dict(payload),
    }
    entry["entry_sha256"] = _canonical_sha256(entry)
    return entry


def _append(
    entries: list[dict[str, Any]], kind: str, payload: Mapping[str, Any]
) -> None:
    _require(
        len(entries) < len(ENTRY_KINDS) and kind == ENTRY_KINDS[len(entries)],
        "WP50_TRANSACTION_ORDER_INVALID",
        kind,
    )
    previous = str(entries[-1]["entry_sha256"]) if entries else None
    entries.append(_entry(kind, payload, len(entries) + 1, previous))


def validate_chain(
    entries: Sequence[Mapping[str, Any]], *, require_review: bool
) -> str:
    expected = ENTRY_KINDS if require_review else ENTRY_KINDS[:-1]
    _require(
        len(entries) == len(expected),
        "WP50_TRANSACTION_ENTRY_COUNT_INVALID",
        str(len(entries)),
    )
    previous: str | None = None
    for sequence, (entry, kind) in enumerate(zip(entries, expected), 1):
        _require(
            entry.get("schema") == ENTRY_SCHEMA
            and entry.get("transaction_id") == TRANSACTION_ID
            and entry.get("sequence") == sequence
            and entry.get("kind") == kind
            and entry.get("previous_entry_sha256") == previous,
            "WP50_TRANSACTION_CHAIN_INVALID",
            kind,
        )
        actual = entry.get("entry_sha256")
        unsigned = {key: value for key, value in entry.items() if key != "entry_sha256"}
        _require(
            isinstance(actual, str)
            and SHA256.fullmatch(actual) is not None
            and actual == _canonical_sha256(unsigned),
            "WP50_TRANSACTION_CHAIN_INVALID",
            f"{kind} digest",
        )
        _mapping(entry.get("payload"), f"{kind}.payload")
        previous = actual
    assert previous is not None
    return previous


def _git(root: Path, *arguments: str) -> str:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError) as error:
        raise ReleasePerformanceError(
            "WP50_GIT_QUERY_FAILED", " ".join(arguments)
        ) from error


def _input_bindings(root: Path, candidate: str) -> list[dict[str, str]]:
    bindings: list[dict[str, str]] = []
    for path in INPUT_PATHS:
        path_text = path.as_posix()
        _require(
            not any(component in path_text for component in FORBIDDEN_INPUT_COMPONENTS),
            "WP50_HISTORY_DEPENDENCY",
            path_text,
        )
        blob = _git(root, "rev-parse", f"{candidate}:{path_text}")
        _require(
            SHA256.fullmatch(sha256_file(root / path)) is not None,
            "WP50_INPUT_UNREADABLE",
            path_text,
        )
        bindings.append(
            {
                "path": path_text,
                "git_blob": blob,
                "sha256": sha256_file(root / path),
            }
        )
    return bindings


def _validate_candidate(
    root: Path, candidate: str, tree: str, *, check_head: bool
) -> None:
    _require(
        HEX40.fullmatch(candidate) is not None and HEX40.fullmatch(tree) is not None,
        "WP50_CANDIDATE_BINDING_INVALID",
        "candidate/tree syntax",
    )
    _require(
        _git(root, "rev-parse", f"{candidate}^{{tree}}") == tree,
        "WP50_CANDIDATE_BINDING_INVALID",
        "tree does not belong to candidate",
    )
    if check_head:
        _require(
            _git(root, "rev-parse", "HEAD") == candidate,
            "WP50_CANDIDATE_BINDING_INVALID",
            "candidate is not HEAD",
        )


def validate_environment(report: Mapping[str, Any], method: Method) -> None:
    environment = _mapping(report.get("environment"), "environment")
    required_fields = method.document.get("environment_required_fields")
    required_values = _mapping(
        method.document.get("environment_required_values"),
        "required environment values",
    )
    _require(
        isinstance(required_fields, list)
        and set(environment) == set(required_fields)
        and all(environment.get(field) not in {None, ""} for field in required_fields),
        "WP50_ENVIRONMENT_INCOMPLETE",
        "environment fields",
    )
    _require(
        all(environment.get(key) == value for key, value in required_values.items())
        and environment.get("repository_head") == report.get("candidate_commit")
        and environment.get("repository_tree") == report.get("candidate_tree")
        and all(
            isinstance(environment.get(field), str)
            and SHA256.fullmatch(str(environment[field])) is not None
            for field in (
                "dirty_tree_digest",
                "adapter_wheel_sha256",
                "daemon_executable_sha256",
                "supervisor_executable_sha256",
            )
        ),
        "WP50_ENVIRONMENT_IDENTITY_INVALID",
        "environment identity/version",
    )


def _workload(method: Method, workload_id: str) -> Workload:
    return next(item for item in method.workloads if item.workload_id == workload_id)


def _metric_summary(
    report: Mapping[str, Any],
    workload: str,
    case: str,
    concurrency: int,
    group: str,
    metric: str,
) -> Mapping[str, Any]:
    try:
        value = report["distribution_summary"][workload][case][str(concurrency)][group][
            metric
        ]
    except (KeyError, TypeError) as error:
        raise ReleasePerformanceError(
            "WP50_DISTRIBUTION_SUMMARY_DRIFT",
            f"{workload}/{case}/{concurrency}/{group}/{metric}",
        ) from error
    return _mapping(value, "metric summary")


def _assert_upper(value: float, bound: float, context: str) -> None:
    _require(value <= bound, "WP50_PERFORMANCE_BUDGET_EXCEEDED", context)


def _assert_lower(value: float, bound: float, context: str) -> None:
    _require(value >= bound, "WP50_PERFORMANCE_BUDGET_EXCEEDED", context)


def _candidate_samples(
    report: Mapping[str, Any], workload: str, case: str, concurrency: int
) -> list[Mapping[str, Any]]:
    return [
        sample
        for sample in _rows(report.get("samples"), "samples")
        if sample.get("workload_id") == workload
        and sample.get("case_id") == case
        and sample.get("concurrency") == concurrency
        and sample.get("phase") == "measured"
        and sample.get("arm") == "candidate"
    ]


def validate_budgets(report: Mapping[str, Any], method: Method) -> int:
    """Apply every frozen budget to summaries recomputed from raw samples."""

    startup = _workload(method, "startup_to_protocol_ready")
    case = startup.cases[0]
    _assert_upper(
        float(
            _metric_summary(
                report, startup.workload_id, case, 1, "candidate", "latency_ms"
            )["p95"]
        ),
        2000,
        "startup p95",
    )
    _assert_upper(
        float(
            _metric_summary(
                report,
                startup.workload_id,
                case,
                1,
                "candidate_minus_control",
                "latency_ms",
            )["p95"]
        ),
        750,
        "startup candidate-control p95",
    )

    rss = _workload(method, "idle_and_active_rss")
    idle_case = "idle_after_discovery"
    for concurrency in rss.concurrency_levels:
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    rss.workload_id,
                    idle_case,
                    concurrency,
                    "candidate",
                    "adapter_rss_bytes",
                )["p95"]
            ),
            201_326_592,
            f"idle adapter RSS concurrency {concurrency}",
        )
        idle = {
            int(sample["block_index"]): float(
                sample["measurements"]["adapter_rss_bytes"]
            )
            for sample in _candidate_samples(
                report, rss.workload_id, idle_case, concurrency
            )
        }
        for active_case in rss.cases[1:]:
            active = _candidate_samples(
                report, rss.workload_id, active_case, concurrency
            )
            increments = [
                float(sample["measurements"]["adapter_rss_bytes"])
                - idle[int(sample["block_index"])]
                for sample in active
            ]
            summary = distribution_summary(
                increments,
                method,
                f"{rss.workload_id}:{active_case}:{concurrency}:active-increment",
            )
            _assert_upper(
                float(summary["p95"]),
                67_108_864,
                f"active RSS increment {active_case}/{concurrency}",
            )
    for state in rss.cases:
        _assert_upper(
            float(
                _metric_summary(
                    report, rss.workload_id, state, 8, "candidate", "topology_rss_bytes"
                )["p95"]
            ),
            1_073_741_824,
            f"eight-agent total topology RSS {state}",
        )

    for workload_id, bound in (
        ("status_presentation_overhead", 25),
        ("query_acceptance_presentation_overhead", 50),
    ):
        workload = _workload(method, workload_id)
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    workload_id,
                    workload.cases[0],
                    1,
                    "candidate_minus_control",
                    "latency_ms",
                )["p95"]
            ),
            bound,
            workload_id,
        )

    guard = _workload(method, "guard_one_and_max_round_overhead")
    for guard_case, bound in zip(guard.cases, (75, 300)):
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    guard.workload_id,
                    guard_case,
                    1,
                    "candidate_minus_control",
                    "latency_ms",
                )["p95"]
            ),
            bound,
            f"guard {guard_case}",
        )
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    guard.workload_id,
                    guard_case,
                    1,
                    "candidate",
                    "guard_rounds",
                )["maximum"]
            ),
            3,
            f"guard rounds {guard_case}",
        )

    completion = _workload(method, "completion_latency_and_cardinality")
    for completion_case in completion.cases:
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    completion.workload_id,
                    completion_case,
                    1,
                    "candidate",
                    "latency_ms",
                )["p95"]
            ),
            100,
            f"completion latency {completion_case}",
        )
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    completion.workload_id,
                    completion_case,
                    1,
                    "candidate",
                    "candidate_count",
                )["maximum"]
            ),
            100,
            f"completion cardinality {completion_case}",
        )

    resource = _workload(method, "resource_first_byte_and_throughput")
    for resource_case in resource.cases:
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    resource.workload_id,
                    resource_case,
                    1,
                    "candidate",
                    "first_byte_ms",
                )["p95"]
            ),
            250,
            f"resource first byte {resource_case}",
        )
        _assert_lower(
            float(
                _metric_summary(
                    report,
                    resource.workload_id,
                    resource_case,
                    1,
                    "candidate",
                    "throughput_bytes_per_second",
                )["minimum"]
            ),
            16_777_216,
            f"resource throughput {resource_case}",
        )
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    resource.workload_id,
                    resource_case,
                    1,
                    "candidate",
                    "materialized_pages",
                )["maximum"]
            ),
            1,
            f"resource pages {resource_case}",
        )

    cancellation = _workload(method, "cancellation_ack_and_cleanup")
    for cancellation_case in cancellation.cases:
        for metric, bound in (("acknowledgement_ms", 250), ("cleanup_ms", 1000)):
            _assert_upper(
                float(
                    _metric_summary(
                        report,
                        cancellation.workload_id,
                        cancellation_case,
                        1,
                        "candidate",
                        metric,
                    )["p95"]
                ),
                bound,
                f"cancellation {cancellation_case}/{metric}",
            )

    reconnect = _workload(method, "reconnect_and_resume")
    for reconnect_case in reconnect.cases:
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    reconnect.workload_id,
                    reconnect_case,
                    1,
                    "candidate",
                    "latency_ms",
                )["p95"]
            ),
            1500,
            f"reconnect {reconnect_case}",
        )
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    reconnect.workload_id,
                    reconnect_case,
                    1,
                    "candidate",
                    "start_query_resubmissions",
                )["maximum"]
            ),
            0,
            f"reconnect resubmission {reconnect_case}",
        )

    agents = _workload(method, "n_agent_fairness_and_aggregate_memory")
    agent_case = agents.cases[0]
    _require(
        agents.concurrency_levels == (1, 2, 4, 8),
        "WP50_CONCURRENCY_METHOD_DRIFT",
        "agent concurrency levels",
    )
    for concurrency in agents.concurrency_levels:
        _assert_upper(
            float(
                _metric_summary(
                    report,
                    agents.workload_id,
                    agent_case,
                    concurrency,
                    "candidate",
                    "fairness_ratio",
                )["maximum"]
            ),
            2.0,
            f"fairness concurrency {concurrency}",
        )
    _assert_upper(
        float(
            _metric_summary(
                report,
                agents.workload_id,
                agent_case,
                8,
                "candidate",
                "topology_rss_bytes",
            )["p95"]
        ),
        1_073_741_824,
        "eight-agent total topology RSS mixed load",
    )
    return len(method.workloads)


def validate_structural_observation(report: Mapping[str, Any], method: Method) -> int:
    observed = _mapping(report.get("structural_observation"), "structural observation")
    bounds = _mapping(method.document.get("structural_bounds"), "structural bounds")
    _require(
        set(observed) == set(bounds), "WP50_STRUCTURAL_OBSERVATION_INCOMPLETE", "fields"
    )
    checked = 0
    for field, bound in bounds.items():
        actual = observed.get(field)
        if field.endswith("_required"):
            _require(actual is bound is True, "WP50_STRUCTURAL_RESOURCE_FAULT", field)
        else:
            _require(
                isinstance(actual, int | float)
                and not isinstance(actual, bool)
                and math.isfinite(float(actual))
                and actual <= bound,
                "WP50_STRUCTURAL_RESOURCE_FAULT",
                field,
            )
        checked += 1
    return checked


def validate_history_independence(
    report: Mapping[str, Any], method: Method, bindings: Sequence[Mapping[str, Any]]
) -> int:
    comparison = _mapping(method.document.get("comparison"), "comparison")
    _require(
        comparison.get("predecessor_comparison_permitted") is False
        and {str(row.get("path")) for row in bindings}
        == {path.as_posix() for path in INPUT_PATHS}
        and not any(
            forbidden in str(row.get("path"))
            for row in bindings
            for forbidden in FORBIDDEN_INPUT_COMPONENTS
        )
        and report.get("outlier_policy") == "report-all-no-post-hoc-removal",
        "WP50_HISTORY_DEPENDENCY",
        "target-only input closure",
    )
    return len(bindings)


@dataclass(frozen=True)
class ReleaseRun:
    recipe: str
    exit_code: int
    stdout_sha256: str
    stdout_bytes: int
    stderr_sha256: str
    stderr_bytes: int

    def as_dict(self) -> dict[str, Any]:
        return {
            "recipe": self.recipe,
            "exit_code": self.exit_code,
            "stdout_sha256": self.stdout_sha256,
            "stdout_bytes": self.stdout_bytes,
            "stderr_sha256": self.stderr_sha256,
            "stderr_bytes": self.stderr_bytes,
        }


def _execute_release_recipe(root: Path, recipe: str) -> ReleaseRun:
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            completed = subprocess.run(
                ["just", recipe],
                cwd=root,
                check=False,
                stdout=stdout,
                stderr=stderr,
            )
        except OSError as error:
            raise ReleasePerformanceError(
                "WP50_RELEASE_RECIPE_FAILED", recipe
            ) from error
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        _require(
            stdout_size <= MAX_RELEASE_STDOUT_BYTES
            and stderr_size <= MAX_RELEASE_STDERR_BYTES,
            "WP50_RELEASE_RECIPE_OUTPUT_UNBOUNDED",
            recipe,
        )
        stdout.seek(0)
        stderr.seek(0)
        stdout_bytes = stdout.read()
        stderr_bytes = stderr.read()
    return ReleaseRun(
        recipe=recipe,
        exit_code=completed.returncode,
        stdout_sha256=hashlib.sha256(stdout_bytes).hexdigest(),
        stdout_bytes=len(stdout_bytes),
        stderr_sha256=hashlib.sha256(stderr_bytes).hexdigest(),
        stderr_bytes=len(stderr_bytes),
    )


def validate_release_matrix(value: object) -> int:
    rows = _rows(value, "release matrix")
    _require(
        [row.get("recipe") for row in rows] == list(REQUIRED_RELEASE_RECIPES),
        "WP50_RELEASE_MATRIX_INCOMPLETE",
        "recipe order/census",
    )
    for row in rows:
        _require(
            row.get("exit_code") == 0
            and isinstance(row.get("stdout_sha256"), str)
            and SHA256.fullmatch(str(row["stdout_sha256"])) is not None
            and isinstance(row.get("stderr_sha256"), str)
            and SHA256.fullmatch(str(row["stderr_sha256"])) is not None
            and isinstance(row.get("stdout_bytes"), int)
            and isinstance(row.get("stderr_bytes"), int),
            "WP50_RELEASE_RECIPE_FAILED",
            str(row.get("recipe")),
        )
    return len(rows)


def _opened_payload(
    root: Path, report_path: Path, report: Mapping[str, Any], method: Method
) -> dict[str, Any]:
    candidate = str(report["candidate_commit"])
    tree = str(report["candidate_tree"])
    _validate_candidate(root, candidate, tree, check_head=True)
    bindings = _input_bindings(root, candidate)
    return {
        "captured_at_utc": datetime.now(UTC).isoformat(),
        "candidate_commit": candidate,
        "candidate_tree": tree,
        "method_sha256": method.digest,
        "registered_method_sha256": sha256_file(root / REGISTERED_METHOD_PATH),
        "raw_report_path": report_path.as_posix(),
        "raw_report_sha256": sha256_file(root / report_path),
        "input_bindings": bindings,
    }


def _write_entries_exclusive(path: Path, entries: Sequence[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = b"".join(canonical_json(entry) + b"\n" for entry in entries)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
    except OSError as error:
        raise ReleasePerformanceError(
            "WP50_TRANSACTION_ALREADY_EXISTS", str(path)
        ) from error


def capture_transaction(
    *,
    root: Path = ROOT,
    report_path: Path = DEFAULT_RAW_REPORT_PATH,
    transaction_path: Path = DEFAULT_TRANSACTION_PATH,
) -> str:
    """Run release recipes and create the immutable pre-review transaction."""

    method = load_method(root)
    report = _load_json(root / report_path)
    validate_report_document(report, method)
    validate_environment(report, method)
    validate_budgets(report, method)
    structural_count = validate_structural_observation(report, method)
    opened = _opened_payload(root, report_path, report, method)
    bindings = _rows(opened["input_bindings"], "input bindings")
    validate_history_independence(report, method, bindings)
    release_runs = [
        _execute_release_recipe(root, recipe) for recipe in REQUIRED_RELEASE_RECIPES
    ]
    validate_release_matrix([run.as_dict() for run in release_runs])

    entries: list[dict[str, Any]] = []
    _append(entries, "transaction_opened", opened)
    _append(
        entries,
        "post_purge_release_matrix",
        {"runs": [run.as_dict() for run in release_runs]},
    )
    _append(
        entries,
        "raw_performance_observation",
        {
            "raw_report_sha256": opened["raw_report_sha256"],
            "sample_count": len(_rows(report.get("samples"), "samples")),
            "semantic_equality": True,
            "skipped_samples": 0,
            "failed_samples": 0,
        },
    )
    _append(
        entries,
        "history_independence",
        {
            "input_paths": [row["path"] for row in bindings],
            "predecessor_inputs": [],
            "history_comparators": [],
            "cache_used_for_verdict": False,
        },
    )
    _append(
        entries,
        "resource_and_budget_verdict",
        {
            "budget_source_id": "wp43-preimplementation-operator-budget-v1",
            "budget_disposition": "passed_without_local_relaxation",
            "locally_relaxed_budget": False,
            "samples_deleted_after_observation": 0,
            "structural_bounds_checked": structural_count,
            "aggregate_memory_interpretation": "total-topology-one-gibibyte",
        },
    )
    tip = validate_chain(entries, require_review=False)
    _write_entries_exclusive(root / transaction_path, entries)
    return tip


def append_independent_review(
    *,
    root: Path = ROOT,
    transaction_path: Path = DEFAULT_TRANSACTION_PATH,
    review_path: Path = DEFAULT_REVIEW_PATH,
) -> str:
    """Append one accepted independent review without rewriting prior bytes."""

    path = root / transaction_path
    entries = _load_jsonl(path)
    tip = validate_chain(entries, require_review=False)
    review = _load_json(root / review_path)
    _require(
        review.get("schema") == REVIEW_SCHEMA
        and review.get("transaction_id") == TRANSACTION_ID
        and review.get("reviewed_through_entry_sha256") == tip
        and review.get("reviewer_is_implementation_owner") is False
        and review.get("reviewer_is_benchmark_operator") is False
        and review.get("verdict") == "accepted"
        and review.get("findings") == [],
        "WP50_INDEPENDENT_REVIEW_INVALID",
        str(review_path),
    )
    payload = {
        "review_path": review_path.as_posix(),
        "review_sha256": sha256_file(root / review_path),
        "reviewer_identity": review.get("reviewer_identity"),
        "reviewed_through_entry_sha256": tip,
        "verdict": "accepted",
    }
    _append(entries, "independent_review", payload)
    new_entry = entries[-1]
    try:
        with path.open("ab") as output:
            output.write(canonical_json(new_entry) + b"\n")
    except OSError as error:
        raise ReleasePerformanceError(
            "WP50_TRANSACTION_APPEND_FAILED", str(path)
        ) from error
    return str(new_entry["entry_sha256"])


def validate_transaction(
    *,
    root: Path = ROOT,
    transaction_path: Path = DEFAULT_TRANSACTION_PATH,
    check_git: bool = True,
) -> int:
    entries = _load_jsonl(root / transaction_path)
    validate_chain(entries, require_review=True)
    opened = _mapping(entries[0].get("payload"), "opened")
    candidate = str(opened.get("candidate_commit"))
    tree = str(opened.get("candidate_tree"))
    _validate_candidate(root, candidate, tree, check_head=check_git)
    report_path = Path(str(opened.get("raw_report_path")))
    _require(
        sha256_file(root / report_path) == opened.get("raw_report_sha256"),
        "WP50_RAW_REPORT_DRIFT",
        str(report_path),
    )
    method = load_method(root)
    report = _load_json(root / report_path)
    validate_report_document(report, method)
    validate_environment(report, method)
    _require(
        report.get("candidate_commit") == candidate
        and report.get("candidate_tree") == tree,
        "WP50_CANDIDATE_BINDING_INVALID",
        "raw report",
    )
    bindings = _rows(opened.get("input_bindings"), "input bindings")
    expected_bindings = _input_bindings(root, candidate)
    _require(
        bindings == expected_bindings, "WP50_INPUT_BINDING_DRIFT", "candidate inputs"
    )
    validate_release_matrix(
        _mapping(entries[1].get("payload"), "release payload").get("runs")
    )
    validate_history_independence(report, method, bindings)
    validate_budgets(report, method)
    validate_structural_observation(report, method)
    performance = _mapping(entries[4].get("payload"), "performance verdict")
    _require(
        performance.get("budget_disposition") == "passed_without_local_relaxation"
        and performance.get("locally_relaxed_budget") is False
        and performance.get("samples_deleted_after_observation") == 0
        and performance.get("aggregate_memory_interpretation")
        == "total-topology-one-gibibyte",
        "WP50_PERFORMANCE_VERDICT_INVALID",
        "performance entry",
    )
    review = _mapping(entries[5].get("payload"), "review entry")
    review_path = Path(str(review.get("review_path")))
    review_document = _load_json(root / review_path)
    _require(
        sha256_file(root / review_path) == review.get("review_sha256")
        and review_document.get("verdict") == "accepted"
        and review_document.get("reviewed_through_entry_sha256")
        == entries[4].get("entry_sha256"),
        "WP50_INDEPENDENT_REVIEW_INVALID",
        str(review_path),
    )
    return len(_rows(report.get("samples"), "samples"))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("method-integrity")
    capture = subparsers.add_parser("capture")
    capture.add_argument("--raw-report", type=Path, default=DEFAULT_RAW_REPORT_PATH)
    capture.add_argument("--transaction", type=Path, default=DEFAULT_TRANSACTION_PATH)
    finalize = subparsers.add_parser("append-review")
    finalize.add_argument("--transaction", type=Path, default=DEFAULT_TRANSACTION_PATH)
    finalize.add_argument("--review", type=Path, default=DEFAULT_REVIEW_PATH)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--transaction", type=Path, default=DEFAULT_TRANSACTION_PATH)
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
                        "method_sha256": method.digest,
                        "workload_count": len(method.workloads),
                        "input_count": len(INPUT_PATHS),
                    },
                    sort_keys=True,
                )
            )
            return 0
        if arguments.command == "capture":
            print(
                capture_transaction(
                    root=ROOT,
                    report_path=arguments.raw_report,
                    transaction_path=arguments.transaction,
                )
            )
            return 0
        if arguments.command == "append-review":
            print(
                append_independent_review(
                    root=ROOT,
                    transaction_path=arguments.transaction,
                    review_path=arguments.review,
                )
            )
            return 0
        print(
            validate_transaction(
                root=ROOT,
                transaction_path=arguments.transaction,
                check_git=not arguments.no_git_check,
            )
        )
        return 0
    except (ReleasePerformanceError, BenchmarkError) as error:
        code = getattr(error, "code", "WP50_VALIDATION_FAILED")
        print(f"{code}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
