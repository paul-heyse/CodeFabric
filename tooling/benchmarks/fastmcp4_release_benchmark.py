"""Execute the preregistered WP65 FastMCP 4 release benchmark.

The runner is deliberately candidate-neutral.  The frozen JSON fixture owns
the cases, order, statistics, resource bounds, and budgets; probe processes
only return raw monotonic/RSS observations.  A probe may be the installed
CodeFabric topology, the minimal FastMCP 4 STDIO control, or the same
candidate's direct-daemon control.  No predecessor result is an input.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import random
import re
import statistics
import subprocess
import sys
import tempfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from typing import Any, NoReturn

ROOT = Path(__file__).resolve().parents[2]
METHOD_PATH = Path("tests/fixtures/fastmcp4_performance/workloads.json")

METHOD_SCHEMA = "codefabric.compiled-release-performance.method.v1"
RAW_REPORT_SCHEMA = "codefabric.compiled-release-performance.raw-report.v1"
SAMPLE_REQUEST_SCHEMA = "codefabric.fastmcp4-release-performance.sample-request.v1"
SAMPLE_RESULT_SCHEMA = "codefabric.fastmcp4-release-performance.sample-result.v1"
METHOD_ID = "fastmcp4-local-stdio-v1"
METHOD_REVISION = "wp65-v1"
METHOD_PLAN = (
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02.md"
)
METHOD_SUITE = "codefabric-relational-data-fabric@2.3.0"
EXPECTED_WORKLOAD_IDS = (
    "startup_to_protocol_ready",
    "idle_and_active_rss",
    "status_presentation_overhead",
    "query_acceptance_presentation_overhead",
    "guard_one_and_max_round_overhead",
    "completion_latency_and_cardinality",
    "resource_first_byte_and_throughput",
    "cancellation_ack_and_cleanup",
    "reconnect_and_resume",
    "n_agent_fairness_and_aggregate_memory",
)
ARMS = ("candidate", "control")
PHASES = ("warmup", "measured")
OS_RSS_SOURCES = frozenset({"linux-proc-statm", "posix-ps-rss-kib"})
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
MAX_METHOD_BYTES = 1_048_576
MAX_PROBE_STDOUT_BYTES = 8_388_608
MAX_PROBE_STDERR_BYTES = 131_072
DEFAULT_PROBE_TIMEOUT_SECONDS = 300.0


class BenchmarkError(ValueError):
    """Fail-closed benchmark error with a stable machine code."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _fail(code: str, message: str) -> NoReturn:
    raise BenchmarkError(code, message)


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        _fail(code, message)


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("WP65_JSON_DUPLICATE_MEMBER", key)
        result[key] = value
    return result


def _reject_nonfinite(value: str) -> NoReturn:
    _fail("WP65_JSON_NONFINITE", value)


def _load_json(path: Path, *, maximum_bytes: int = MAX_METHOD_BYTES) -> dict[str, Any]:
    try:
        metadata = path.stat()
        _require(
            path.is_file() and metadata.st_size <= maximum_bytes,
            "WP65_JSON_SIZE_INVALID",
            str(path),
        )
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicates,
            parse_constant=_reject_nonfinite,
        )
    except BenchmarkError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BenchmarkError("WP65_JSON_INVALID", str(path)) from error
    _require(isinstance(value, dict), "WP65_JSON_ROOT_INVALID", str(path))
    return value


def canonical_json(value: object) -> bytes:
    """Return deterministic JSON bytes for integrity/equality, not semantic proof."""

    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BenchmarkError("WP65_JSON_VALUE_INVALID", "non-JSON value") from error


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as error:
        raise BenchmarkError("WP65_INPUT_UNREADABLE", str(path)) from error


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(isinstance(value, Mapping), "WP65_METHOD_SCHEMA_INVALID", context)
    assert isinstance(value, Mapping)
    return value


def _string_list(value: object, context: str) -> list[str]:
    _require(
        isinstance(value, list)
        and bool(value)
        and all(isinstance(item, str) and item for item in value),
        "WP65_METHOD_SCHEMA_INVALID",
        context,
    )
    assert isinstance(value, list)
    return list(value)


def _integer(value: object, context: str, *, minimum: int = 0) -> int:
    _require(
        isinstance(value, int) and not isinstance(value, bool) and value >= minimum,
        "WP65_METHOD_SCHEMA_INVALID",
        context,
    )
    assert isinstance(value, int)
    return value


def _closed(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    _require(set(value) == expected, "WP65_METHOD_SCHEMA_INVALID", context)


@dataclass(frozen=True)
class Workload:
    workload_id: str
    control_kind: str
    concurrency_levels: tuple[int, ...]
    cases: tuple[str, ...]
    metrics: tuple[str, ...]
    budgets: Mapping[str, int | float]


@dataclass(frozen=True)
class Method:
    document: Mapping[str, Any]
    digest: str
    warmups: int
    samples: int
    bootstrap_seed: int
    bootstrap_resamples: int
    confidence_percent: int
    ordering_seed: int
    workloads: tuple[Workload, ...]


def _validate_method_document(document: Mapping[str, Any], root: Path) -> Method:
    _closed(
        document,
        {
            "schema",
            "method_id",
            "method_revision",
            "authority",
            "statistics",
            "comparison",
            "topology",
            "structural_bounds",
            "environment_required_fields",
            "environment_required_values",
            "workloads",
        },
        "method root",
    )
    _require(
        document.get("schema") == METHOD_SCHEMA
        and document.get("method_id") == METHOD_ID
        and document.get("method_revision") == METHOD_REVISION,
        "WP65_METHOD_IDENTITY_DRIFT",
        "method identity",
    )
    authority = _mapping(document.get("authority"), "authority")
    _require(
        authority.get("plan_path") == METHOD_PLAN
        and authority.get("packet") == "WP65"
        and authority.get("suite") == METHOD_SUITE
        and authority.get("registered_before_candidate_results") is True
        and authority.get("candidate_results_used") is False
        and authority.get("local_relaxation_permitted") is False,
        "WP65_METHOD_AUTHORITY_DRIFT",
        "preregistration",
    )

    statistics_value = _mapping(document.get("statistics"), "statistics")
    percentile = _mapping(statistics_value.get("percentile_policy"), "percentile")
    bootstrap = _mapping(statistics_value.get("bootstrap"), "bootstrap")
    ordering = _mapping(statistics_value.get("ordering"), "ordering")
    _require(
        statistics_value.get("warmups_per_case") == 3
        and statistics_value.get("samples_per_case") == 30
        and statistics_value.get("clock") == "monotonic_ns"
        and statistics_value.get("latency_unit") == "milliseconds"
        and statistics_value.get("memory_source")
        == "operating-system-resident-set-bytes"
        and statistics_value.get("outlier_policy") == "report-all-no-post-hoc-removal"
        and percentile.get("name") == "nearest-rank"
        and percentile.get("rank_expression") == "max(1,ceil(percentile*sample_count))"
        and percentile.get("reported_percentiles") == [0, 50, 95, 100]
        and bootstrap.get("sampling") == "with-replacement-same-cardinality"
        and bootstrap.get("confidence_percent") == 95
        and bootstrap.get("statistics") == ["median", "p95"]
        and ordering.get("name")
        == "deterministic-randomized-interleaved-candidate-control-blocks"
        and ordering.get("arms") == list(ARMS),
        "WP65_STATISTICAL_METHOD_DRIFT",
        "statistics",
    )

    comparison = _mapping(document.get("comparison"), "comparison")
    topology = _mapping(document.get("topology"), "topology")
    _require(
        comparison.get("predecessor_comparison_permitted") is False
        and comparison.get("same_host_required") is True
        and comparison.get("same_power_profile_required") is True
        and comparison.get("semantic_equality")
        == "canonical-json-decoded-observations-must-match-per-block"
        and comparison.get("different_semantics_permitted") is False
        and topology.get("aggregate_rss_scope")
        == "sum-unique-os-rss-for-every-live-process-in-the-measured-topology"
        and topology.get("eight_agent_aggregate_ceiling_bytes") == 1_073_741_824
        and topology.get("eight_agent_ceiling_interpretation")
        == "one-total-topology-ceiling-not-a-per-process-or-per-agent-ceiling"
        and topology.get("concurrency_levels") == [1, 2, 4, 8],
        "WP65_COMPARISON_METHOD_DRIFT",
        "comparison/topology",
    )

    raw_workloads = document.get("workloads")
    _require(isinstance(raw_workloads, list), "WP65_METHOD_SCHEMA_INVALID", "workloads")
    assert isinstance(raw_workloads, list)
    workloads: list[Workload] = []
    for index, raw in enumerate(raw_workloads):
        value = _mapping(raw, f"workloads[{index}]")
        _closed(
            value,
            {
                "workload_id",
                "control_kind",
                "concurrency_levels",
                "cases",
                "metrics",
                "budgets",
            },
            f"workloads[{index}]",
        )
        workload_id = value.get("workload_id")
        _require(
            isinstance(workload_id, str), "WP65_METHOD_SCHEMA_INVALID", "workload id"
        )
        levels_raw = value.get("concurrency_levels")
        _require(
            isinstance(levels_raw, list)
            and bool(levels_raw)
            and all(
                isinstance(level, int)
                and not isinstance(level, bool)
                and level in {1, 2, 4, 8}
                for level in levels_raw
            ),
            "WP65_CONCURRENCY_METHOD_DRIFT",
            str(workload_id),
        )
        budgets = _mapping(value.get("budgets"), f"{workload_id}.budgets")
        _require(
            bool(budgets)
            and all(
                isinstance(item, int | float)
                and not isinstance(item, bool)
                and math.isfinite(float(item))
                and item >= 0
                for item in budgets.values()
            ),
            "WP65_BUDGET_METHOD_DRIFT",
            str(workload_id),
        )
        control_kind = value.get("control_kind")
        _require(
            control_kind
            in {
                "minimal-fastmcp4-stdio-control-v1",
                "same-candidate-direct-daemon-rpc-v1",
            },
            "WP65_CONTROL_METHOD_DRIFT",
            str(workload_id),
        )
        assert isinstance(workload_id, str)
        assert isinstance(control_kind, str)
        workloads.append(
            Workload(
                workload_id=workload_id,
                control_kind=control_kind,
                concurrency_levels=tuple(int(level) for level in levels_raw),
                cases=tuple(_string_list(value.get("cases"), f"{workload_id}.cases")),
                metrics=tuple(
                    _string_list(value.get("metrics"), f"{workload_id}.metrics")
                ),
                budgets={str(key): item for key, item in budgets.items()},
            )
        )
    _require(
        tuple(item.workload_id for item in workloads) == EXPECTED_WORKLOAD_IDS,
        "WP65_WORKLOAD_CENSUS_DRIFT",
        "workload order/census",
    )
    _require(
        next(
            item for item in workloads if item.workload_id.startswith("n_agent")
        ).concurrency_levels
        == (1, 2, 4, 8),
        "WP65_CONCURRENCY_METHOD_DRIFT",
        "N-agent levels",
    )
    return Method(
        document=document,
        digest=sha256_bytes(canonical_json(document)),
        warmups=_integer(
            statistics_value.get("warmups_per_case"), "warmups", minimum=1
        ),
        samples=_integer(
            statistics_value.get("samples_per_case"), "samples", minimum=1
        ),
        bootstrap_seed=_integer(bootstrap.get("seed"), "bootstrap seed", minimum=1),
        bootstrap_resamples=_integer(
            bootstrap.get("resamples"), "bootstrap resamples", minimum=100
        ),
        confidence_percent=_integer(
            bootstrap.get("confidence_percent"), "confidence", minimum=1
        ),
        ordering_seed=_integer(ordering.get("seed"), "ordering seed", minimum=1),
        workloads=tuple(workloads),
    )


def load_method(root: Path = ROOT, path: Path = METHOD_PATH) -> Method:
    resolved = path if path.is_absolute() else root / path
    document = _load_json(resolved)
    return _validate_method_document(document, root)


def nearest_rank(values: Sequence[float], percentile: float) -> float:
    """Return the frozen one-based nearest-rank percentile."""

    _require(bool(values), "WP65_EMPTY_DISTRIBUTION", "nearest rank")
    _require(0 <= percentile <= 1, "WP65_PERCENTILE_INVALID", str(percentile))
    ordered = sorted(float(value) for value in values)
    _require(
        all(math.isfinite(value) for value in ordered),
        "WP65_NONFINITE_SAMPLE",
        "nearest rank",
    )
    if percentile == 0:
        return ordered[0]
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def _statistic(values: Sequence[float], name: str) -> float:
    if name == "median":
        return float(statistics.median(values))
    if name == "p95":
        return nearest_rank(values, 0.95)
    _fail("WP65_STATISTIC_INVALID", name)


def _derived_seed(base_seed: int, identity: str) -> int:
    digest = hashlib.sha256(f"{base_seed}\0{identity}".encode()).digest()
    return int.from_bytes(digest[:8], "big")


def bootstrap_ci(
    values: Sequence[float],
    *,
    statistic: str,
    seed: int,
    resamples: int,
    confidence_percent: int,
) -> tuple[float, float]:
    """Return the deterministic percentile bootstrap interval."""

    _require(bool(values), "WP65_EMPTY_DISTRIBUTION", statistic)
    _require(resamples >= 100, "WP65_BOOTSTRAP_INVALID", "resamples")
    _require(1 <= confidence_percent < 100, "WP65_BOOTSTRAP_INVALID", "confidence")
    source = [float(value) for value in values]
    rng = random.Random(seed)
    estimates = [
        _statistic(
            [source[rng.randrange(len(source))] for _ in range(len(source))],
            statistic,
        )
        for _ in range(resamples)
    ]
    tail = (1 - confidence_percent / 100) / 2
    return nearest_rank(estimates, tail), nearest_rank(estimates, 1 - tail)


def distribution_summary(
    values: Sequence[float], method: Method, identity: str
) -> dict[str, Any]:
    _require(len(values) == method.samples, "WP65_SAMPLE_COUNT_INVALID", identity)
    normalized = [float(value) for value in values]
    return {
        "count": len(normalized),
        "minimum": nearest_rank(normalized, 0),
        "median": float(statistics.median(normalized)),
        "p95": nearest_rank(normalized, 0.95),
        "maximum": nearest_rank(normalized, 1),
        "bootstrap_ci95": {
            statistic: list(
                bootstrap_ci(
                    normalized,
                    statistic=statistic,
                    seed=_derived_seed(
                        method.bootstrap_seed, f"{identity}:{statistic}"
                    ),
                    resamples=method.bootstrap_resamples,
                    confidence_percent=method.confidence_percent,
                )
            )
            for statistic in ("median", "p95")
        },
    }


def arm_order(seed: int, identity: str) -> tuple[str, str]:
    digest = hashlib.sha256(f"{seed}\0{identity}".encode()).digest()
    return ARMS if digest[0] & 1 == 0 else tuple(reversed(ARMS))


def build_schedule(method: Method) -> list[dict[str, Any]]:
    schedule: list[dict[str, Any]] = []
    for workload in method.workloads:
        for case_id in workload.cases:
            for concurrency in workload.concurrency_levels:
                for phase, blocks in (
                    ("warmup", method.warmups),
                    ("measured", method.samples),
                ):
                    for block_index in range(blocks):
                        identity = (
                            f"{workload.workload_id}:{case_id}:{concurrency}:"
                            f"{phase}:{block_index}"
                        )
                        for arm in arm_order(method.ordering_seed, identity):
                            schedule.append(
                                {
                                    "schema": SAMPLE_REQUEST_SCHEMA,
                                    "schedule_index": len(schedule),
                                    "workload_id": workload.workload_id,
                                    "case_id": case_id,
                                    "concurrency": concurrency,
                                    "phase": phase,
                                    "block_index": block_index,
                                    "arm": arm,
                                    "control_kind": workload.control_kind,
                                    "required_metrics": list(workload.metrics),
                                }
                            )
    return schedule


def schedule_sha256(schedule: Sequence[Mapping[str, Any]]) -> str:
    return sha256_bytes(canonical_json(list(schedule)))


def process_rss_bytes(pid: int) -> tuple[str, int]:
    """Read one process's resident set from an operating-system interface."""

    _require(pid > 0, "WP65_RSS_PID_INVALID", str(pid))
    statm = Path(f"/proc/{pid}/statm")
    if statm.is_file():
        try:
            resident_pages = int(statm.read_text(encoding="ascii").split()[1])
            page_bytes = int(os.sysconf("SC_PAGE_SIZE"))
        except (OSError, ValueError, IndexError) as error:
            raise BenchmarkError("WP65_RSS_READ_FAILED", str(pid)) from error
        return "linux-proc-statm", resident_pages * page_bytes
    try:
        result = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)],
            check=True,
            capture_output=True,
            text=True,
            timeout=5,
        )
        kibibytes = int(result.stdout.strip())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise BenchmarkError("WP65_RSS_READ_FAILED", str(pid)) from error
    return "posix-ps-rss-kib", kibibytes * 1024


def _numeric(value: object, context: str) -> float:
    _require(
        isinstance(value, int | float)
        and not isinstance(value, bool)
        and math.isfinite(float(value))
        and value >= 0,
        "WP65_SAMPLE_MEASUREMENT_INVALID",
        context,
    )
    return float(value)


def _sample_key(value: Mapping[str, Any]) -> tuple[Any, ...]:
    return (
        value.get("workload_id"),
        value.get("case_id"),
        value.get("concurrency"),
        value.get("phase"),
        value.get("block_index"),
        value.get("arm"),
    )


def validate_sample(
    sample: Mapping[str, Any], request: Mapping[str, Any]
) -> dict[str, Any]:
    _require(
        sample.get("schema") == SAMPLE_RESULT_SCHEMA,
        "WP65_SAMPLE_SCHEMA_INVALID",
        "schema",
    )
    for field in (
        "schedule_index",
        "workload_id",
        "case_id",
        "concurrency",
        "phase",
        "block_index",
        "arm",
        "control_kind",
    ):
        _require(
            sample.get(field) == request.get(field),
            "WP65_SAMPLE_BINDING_INVALID",
            field,
        )
    _require(
        sample.get("status") == "passed"
        and sample.get("skipped") is False
        and sample.get("errors") == [],
        "WP65_SAMPLE_NOT_SUCCESSFUL",
        str(_sample_key(sample)),
    )
    timing = _mapping(sample.get("timing"), "sample timing")
    start = _integer(timing.get("start_monotonic_ns"), "start monotonic", minimum=1)
    stop = _integer(timing.get("stop_monotonic_ns"), "stop monotonic", minimum=1)
    _require(
        timing.get("clock") == "monotonic_ns" and stop >= start,
        "WP65_SAMPLE_TIMING_INVALID",
        str(_sample_key(sample)),
    )
    latency_ms = _numeric(timing.get("elapsed_ms"), "elapsed_ms")
    _require(
        math.isclose(latency_ms, (stop - start) / 1_000_000, rel_tol=0, abs_tol=1e-9),
        "WP65_SAMPLE_TIMING_INVALID",
        "elapsed does not derive from monotonic endpoints",
    )

    rss = _mapping(sample.get("rss"), "sample rss")
    processes = rss.get("processes")
    _require(
        rss.get("source") in OS_RSS_SOURCES
        and isinstance(processes, list)
        and bool(processes),
        "WP65_SAMPLE_RSS_INVALID",
        str(_sample_key(sample)),
    )
    assert isinstance(processes, list)
    seen_pids: set[int] = set()
    total = 0
    adapter_values: list[int] = []
    for process in processes:
        row = _mapping(process, "rss process")
        pid = _integer(row.get("pid"), "rss pid", minimum=1)
        role = row.get("role")
        rss_bytes = _integer(row.get("rss_bytes"), "rss bytes", minimum=1)
        _require(
            pid not in seen_pids and isinstance(role, str) and bool(role),
            "WP65_SAMPLE_RSS_INVALID",
            "duplicate pid or empty role",
        )
        seen_pids.add(pid)
        total += rss_bytes
        if role == "adapter":
            adapter_values.append(rss_bytes)
    _require(
        rss.get("topology_rss_bytes") == total,
        "WP65_SAMPLE_RSS_INVALID",
        "topology RSS is not the unique-process sum",
    )

    measurements = _mapping(sample.get("measurements"), "sample measurements")
    required_metrics = request.get("required_metrics")
    _require(
        isinstance(required_metrics, list)
        and set(measurements) == set(required_metrics),
        "WP65_SAMPLE_METRIC_CENSUS_INVALID",
        str(_sample_key(sample)),
    )
    for metric, value in measurements.items():
        _numeric(value, f"measurement {metric}")
    if "latency_ms" in measurements:
        _require(
            math.isclose(
                float(measurements["latency_ms"]), latency_ms, rel_tol=0, abs_tol=1e-9
            ),
            "WP65_SAMPLE_TIMING_INVALID",
            "latency metric differs from monotonic timing",
        )
    if "topology_rss_bytes" in measurements:
        _require(
            measurements["topology_rss_bytes"] == total,
            "WP65_SAMPLE_RSS_INVALID",
            "topology metric differs from OS snapshot",
        )
    if "adapter_rss_bytes" in measurements:
        _require(
            bool(adapter_values)
            and measurements["adapter_rss_bytes"] == max(adapter_values),
            "WP65_SAMPLE_RSS_INVALID",
            "adapter metric is not maximum per-adapter OS RSS",
        )
    canonical_json(sample.get("semantic_observation"))
    result = dict(sample)
    result["measurements"] = dict(measurements)
    return result


def _summaries(samples: Sequence[Mapping[str, Any]], method: Method) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for workload in method.workloads:
        workload_summary: dict[str, Any] = {}
        for case_id in workload.cases:
            case_summary: dict[str, Any] = {}
            for concurrency in workload.concurrency_levels:
                level_summary: dict[str, Any] = {}
                selected = [
                    item
                    for item in samples
                    if item["workload_id"] == workload.workload_id
                    and item["case_id"] == case_id
                    and item["concurrency"] == concurrency
                    and item["phase"] == "measured"
                ]
                for arm in ARMS:
                    arm_samples = [item for item in selected if item["arm"] == arm]
                    level_summary[arm] = {
                        metric: distribution_summary(
                            [
                                float(item["measurements"][metric])
                                for item in arm_samples
                            ],
                            method,
                            f"{workload.workload_id}:{case_id}:{concurrency}:{arm}:{metric}",
                        )
                        for metric in workload.metrics
                    }
                paired: dict[str, Any] = {}
                for metric in workload.metrics:
                    by_block = {
                        (item["block_index"], item["arm"]): float(
                            item["measurements"][metric]
                        )
                        for item in selected
                    }
                    deltas = [
                        by_block[(block, "candidate")] - by_block[(block, "control")]
                        for block in range(method.samples)
                    ]
                    paired[metric] = distribution_summary(
                        deltas,
                        method,
                        f"{workload.workload_id}:{case_id}:{concurrency}:paired:{metric}",
                    )
                level_summary["candidate_minus_control"] = paired
                case_summary[str(concurrency)] = level_summary
            workload_summary[case_id] = case_summary
        result[workload.workload_id] = workload_summary
    return result


def validate_samples(
    samples: Sequence[Mapping[str, Any]], method: Method
) -> tuple[list[dict[str, Any]], bool]:
    schedule = build_schedule(method)
    _require(
        len(samples) == len(schedule),
        "WP65_SAMPLE_COUNT_INVALID",
        f"expected {len(schedule)}, observed {len(samples)}",
    )
    checked = [
        validate_sample(sample, request) for sample, request in zip(samples, schedule)
    ]
    keys = [_sample_key(item) for item in checked]
    _require(len(keys) == len(set(keys)), "WP65_SAMPLE_DUPLICATE", "sample key")
    semantic_equal = True
    for index in range(0, len(checked), 2):
        first, second = checked[index : index + 2]
        _require(
            first["arm"] != second["arm"]
            and _sample_key(first)[:-1] == _sample_key(second)[:-1],
            "WP65_INTERLEAVING_INVALID",
            str(index),
        )
        if canonical_json(first.get("semantic_observation")) != canonical_json(
            second.get("semantic_observation")
        ):
            semantic_equal = False
    _require(semantic_equal, "WP65_SEMANTIC_DIFFERENTIAL", "candidate/control output")
    return checked, semantic_equal


def build_report(
    *,
    method: Method,
    candidate_commit: str,
    candidate_tree: str,
    environment: Mapping[str, Any],
    samples: Sequence[Mapping[str, Any]],
    structural_observation: Mapping[str, Any],
) -> dict[str, Any]:
    checked, semantic_equal = validate_samples(samples, method)
    schedule = build_schedule(method)
    return {
        "schema": RAW_REPORT_SCHEMA,
        "method_id": METHOD_ID,
        "method_revision": METHOD_REVISION,
        "method_sha256": method.digest,
        "candidate_commit": candidate_commit,
        "candidate_tree": candidate_tree,
        "environment": dict(environment),
        "schedule_sha256": schedule_sha256(schedule),
        "warmups_per_case": method.warmups,
        "samples_per_case": method.samples,
        "outlier_policy": "report-all-no-post-hoc-removal",
        "samples_deleted_after_observation": 0,
        "skipped_samples": 0,
        "failed_samples": 0,
        "semantic_equality": semantic_equal,
        "samples": checked,
        "distribution_summary": _summaries(checked, method),
        "structural_observation": dict(structural_observation),
    }


def validate_report_document(report: Mapping[str, Any], method: Method) -> int:
    _require(
        report.get("schema") == RAW_REPORT_SCHEMA
        and report.get("method_id") == METHOD_ID
        and report.get("method_revision") == METHOD_REVISION
        and report.get("method_sha256") == method.digest,
        "WP65_REPORT_METHOD_DRIFT",
        "report method",
    )
    _require(
        isinstance(report.get("candidate_commit"), str)
        and HEX40.fullmatch(str(report["candidate_commit"])) is not None
        and isinstance(report.get("candidate_tree"), str)
        and HEX40.fullmatch(str(report["candidate_tree"])) is not None,
        "WP65_REPORT_CANDIDATE_INVALID",
        "candidate binding",
    )
    _require(
        report.get("schedule_sha256") == schedule_sha256(build_schedule(method))
        and report.get("warmups_per_case") == 3
        and report.get("samples_per_case") == 30
        and report.get("outlier_policy") == "report-all-no-post-hoc-removal"
        and report.get("samples_deleted_after_observation") == 0
        and report.get("skipped_samples") == 0
        and report.get("failed_samples") == 0
        and report.get("semantic_equality") is True,
        "WP65_REPORT_COMPLETENESS_INVALID",
        "sample policy",
    )
    raw_samples = report.get("samples")
    _require(isinstance(raw_samples, list), "WP65_SAMPLE_COUNT_INVALID", "samples")
    assert isinstance(raw_samples, list)
    checked, _ = validate_samples(raw_samples, method)
    _require(
        report.get("distribution_summary") == _summaries(checked, method),
        "WP65_DISTRIBUTION_SUMMARY_DRIFT",
        "summary does not derive from raw samples",
    )
    _mapping(report.get("environment"), "environment")
    _mapping(report.get("structural_observation"), "structural observation")
    return len(checked)


def _probe_command(value: Sequence[str], context: str) -> tuple[str, ...]:
    _require(bool(value), "WP65_PROBE_COMMAND_INVALID", context)
    executable = Path(value[0])
    _require(
        executable.is_absolute()
        and executable.is_file()
        and os.access(executable, os.X_OK),
        "WP65_PROBE_COMMAND_INVALID",
        context,
    )
    return tuple(value)


ProbeExecutor = Callable[
    [Sequence[str], Mapping[str, Any], float, Path], Mapping[str, Any]
]


def execute_probe(
    command: Sequence[str], request: Mapping[str, Any], timeout: float, root: Path
) -> Mapping[str, Any]:
    """Execute one bounded probe without retaining unbounded child output."""

    checked = _probe_command(command, "probe")
    payload = canonical_json(request) + b"\n"
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            completed = subprocess.run(
                checked,
                input=payload,
                stdout=stdout,
                stderr=stderr,
                cwd=root,
                timeout=timeout,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise BenchmarkError(
                "WP65_PROBE_EXECUTION_FAILED", str(request["schedule_index"])
            ) from error
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        _require(
            completed.returncode == 0
            and stdout_size <= MAX_PROBE_STDOUT_BYTES
            and stderr_size <= MAX_PROBE_STDERR_BYTES,
            "WP65_PROBE_EXECUTION_FAILED",
            str(request["schedule_index"]),
        )
        stdout.seek(0)
        try:
            result = json.loads(
                stdout.read().decode("utf-8"),
                object_pairs_hook=_reject_duplicates,
                parse_constant=_reject_nonfinite,
            )
        except (UnicodeError, json.JSONDecodeError) as error:
            raise BenchmarkError(
                "WP65_PROBE_RESULT_INVALID", str(request["schedule_index"])
            ) from error
    _require(isinstance(result, Mapping), "WP65_PROBE_RESULT_INVALID", "root")
    assert isinstance(result, Mapping)
    return result


def execute_method(
    method: Method,
    *,
    candidate_probe: Sequence[str],
    minimal_control_probe: Sequence[str],
    daemon_control_probe: Sequence[str],
    timeout: float = DEFAULT_PROBE_TIMEOUT_SECONDS,
    root: Path = ROOT,
    executor: ProbeExecutor = execute_probe,
) -> list[dict[str, Any]]:
    commands = {
        "candidate": _probe_command(candidate_probe, "candidate probe"),
        "minimal-fastmcp4-stdio-control-v1": _probe_command(
            minimal_control_probe, "minimal control probe"
        ),
        "same-candidate-direct-daemon-rpc-v1": _probe_command(
            daemon_control_probe, "daemon control probe"
        ),
    }
    results: list[dict[str, Any]] = []
    for request in build_schedule(method):
        command = (
            commands["candidate"]
            if request["arm"] == "candidate"
            else commands[str(request["control_kind"])]
        )
        result = executor(command, request, timeout, root)
        results.append(validate_sample(result, request))
    return results


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
        raise BenchmarkError("WP65_GIT_QUERY_FAILED", " ".join(arguments)) from error


def environment_record(
    *,
    root: Path,
    adapter_wheel: Path,
    daemon_executable: Path,
    supervisor_executable: Path,
    power_profile: str,
    competing_load_description: str,
) -> dict[str, Any]:
    """Capture the preregistered environment fields without interpreting results."""

    uname = platform.uname()
    cpu_model = platform.processor().strip() or "unknown"
    if Path("/proc/cpuinfo").is_file():
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.lower().startswith("model name"):
                cpu_model = line.partition(":")[2].strip()
                break
    memory_bytes = 0
    if Path("/proc/meminfo").is_file():
        first = Path("/proc/meminfo").read_text(encoding="ascii").splitlines()[0]
        memory_bytes = int(first.split()[1]) * 1024
    logical = os.cpu_count() or 1
    physical = logical
    try:
        physical = len(
            {
                line
                for line in subprocess.run(
                    ["lscpu", "-p=socket,core"],
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=5,
                ).stdout.splitlines()
                if line and not line.startswith("#")
            }
        )
    except (OSError, subprocess.SubprocessError):
        pass

    def package(package: str) -> str:
        try:
            return version(package)
        except PackageNotFoundError as error:
            raise BenchmarkError("WP65_PACKAGE_IDENTITY_MISSING", package) from error

    status = _git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all")
    head = _git(root, "rev-parse", "HEAD")
    tree = _git(root, "rev-parse", "HEAD^{tree}")
    return {
        "recorded_at_utc": datetime.now(UTC).isoformat(),
        "repository_head": head,
        "repository_tree": tree,
        "dirty_tree_digest": sha256_bytes(status.encode()),
        "operating_system": uname.system,
        "kernel_version": uname.release,
        "architecture": uname.machine,
        "cpu_model": cpu_model,
        "physical_core_count": physical,
        "logical_cpu_count": logical,
        "memory_bytes": memory_bytes,
        "python_version": platform.python_version(),
        "fastmcp_version": package("fastmcp"),
        "mcp_version": package("mcp"),
        "pydantic_version": package("pydantic"),
        "grpcio_version": package("grpcio"),
        "adapter_wheel_sha256": sha256_file(adapter_wheel),
        "daemon_executable_sha256": sha256_file(daemon_executable),
        "supervisor_executable_sha256": sha256_file(supervisor_executable),
        "host_profile": "local-workstation-v1",
        "power_profile": power_profile,
        "competing_load_description": competing_load_description,
    }


def _parse_command(value: str) -> tuple[str, ...]:
    try:
        decoded = json.loads(value, object_pairs_hook=_reject_duplicates)
    except (json.JSONDecodeError, BenchmarkError) as error:
        raise argparse.ArgumentTypeError(
            "command must be a JSON string array"
        ) from error
    if not isinstance(decoded, list) or not all(
        isinstance(item, str) for item in decoded
    ):
        raise argparse.ArgumentTypeError("command must be a JSON string array")
    return tuple(decoded)


def _write_exclusive(path: Path, value: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = canonical_json(value) + b"\n"
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    try:
        descriptor = os.open(path, flags, 0o600)
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
    except OSError as error:
        raise BenchmarkError("WP65_OUTPUT_NOT_EXCLUSIVE", str(path)) from error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("method", help="validate and describe the frozen method")
    run = subparsers.add_parser("run", help="execute every frozen probe block")
    run.add_argument("--candidate-probe", type=_parse_command, required=True)
    run.add_argument("--minimal-control-probe", type=_parse_command, required=True)
    run.add_argument("--daemon-control-probe", type=_parse_command, required=True)
    run.add_argument("--adapter-wheel", type=Path, required=True)
    run.add_argument("--daemon-executable", type=Path, required=True)
    run.add_argument("--supervisor-executable", type=Path, required=True)
    run.add_argument("--power-profile", required=True)
    run.add_argument("--competing-load-description", required=True)
    run.add_argument("--structural-observation", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument(
        "--timeout-seconds", type=float, default=DEFAULT_PROBE_TIMEOUT_SECONDS
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        method = load_method(ROOT)
        if arguments.command == "method":
            print(
                json.dumps(
                    {
                        "method_id": METHOD_ID,
                        "method_revision": METHOD_REVISION,
                        "method_sha256": method.digest,
                        "schedule_sha256": schedule_sha256(build_schedule(method)),
                        "scheduled_samples": len(build_schedule(method)),
                    },
                    sort_keys=True,
                )
            )
            return 0
        environment = environment_record(
            root=ROOT,
            adapter_wheel=arguments.adapter_wheel,
            daemon_executable=arguments.daemon_executable,
            supervisor_executable=arguments.supervisor_executable,
            power_profile=arguments.power_profile,
            competing_load_description=arguments.competing_load_description,
        )
        samples = execute_method(
            method,
            candidate_probe=arguments.candidate_probe,
            minimal_control_probe=arguments.minimal_control_probe,
            daemon_control_probe=arguments.daemon_control_probe,
            timeout=arguments.timeout_seconds,
        )
        structural = _load_json(arguments.structural_observation)
        report = build_report(
            method=method,
            candidate_commit=str(environment["repository_head"]),
            candidate_tree=str(environment["repository_tree"]),
            environment=environment,
            samples=samples,
            structural_observation=structural,
        )
        _write_exclusive(arguments.output, report)
        return 0
    except BenchmarkError as error:
        print(f"{error.code}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
