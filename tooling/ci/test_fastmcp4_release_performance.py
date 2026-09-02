"""Deterministic falsification tests for the frozen WP50 method and evidence."""

from __future__ import annotations

import copy
import math
from typing import Any

import pytest

from tooling.benchmarks.fastmcp4_minimal_control import description
from tooling.benchmarks.fastmcp4_release_benchmark import (
    SAMPLE_RESULT_SCHEMA,
    BenchmarkError,
    bootstrap_ci,
    build_schedule,
    load_method,
    nearest_rank,
    schedule_sha256,
    validate_samples,
)
from tooling.ci.fastmcp4_release_performance import (
    INPUT_PATHS,
    ReleasePerformanceError,
    validate_history_independence,
    validate_structural_observation,
)


def _sample(request: dict[str, Any]) -> dict[str, Any]:
    workload = request["workload_id"]
    case = request["case_id"]
    concurrency = int(request["concurrency"])
    metrics: dict[str, int | float] = {}
    adapter_rss = 50_000_000
    if workload == "idle_and_active_rss" and case != "idle_after_discovery":
        adapter_rss = 60_000_000
    processes = [
        {"pid": 10_000 + index, "role": "adapter", "rss_bytes": adapter_rss}
        for index in range(concurrency)
    ]
    processes.extend(
        [
            {"pid": 20_000, "role": "daemon", "rss_bytes": 50_000_000},
            {"pid": 20_001, "role": "supervisor", "rss_bytes": 10_000_000},
        ]
    )
    topology_rss = sum(int(item["rss_bytes"]) for item in processes)
    elapsed_ms = 10.0 if request["arm"] == "candidate" else 5.0
    for metric in request["required_metrics"]:
        values: dict[str, int | float] = {
            "latency_ms": elapsed_ms,
            "adapter_rss_bytes": adapter_rss,
            "topology_rss_bytes": topology_rss,
            "guard_rounds": 1 if case == "one-input-required-leg" else 3,
            "candidate_count": 100 if case == "one-hundred-candidates" else 1,
            "first_byte_ms": 10,
            "throughput_bytes_per_second": 20_000_000,
            "materialized_pages": 1,
            "acknowledgement_ms": 10,
            "cleanup_ms": 20,
            "start_query_resubmissions": 0,
            "fairness_ratio": 1.0,
        }
        metrics[metric] = values[metric]
    start = 1_000_000_000 + int(request["schedule_index"]) * 20_000_000
    return {
        "schema": SAMPLE_RESULT_SCHEMA,
        **{
            key: request[key]
            for key in (
                "schedule_index",
                "workload_id",
                "case_id",
                "concurrency",
                "phase",
                "block_index",
                "arm",
                "control_kind",
            )
        },
        "status": "passed",
        "skipped": False,
        "errors": [],
        "timing": {
            "clock": "monotonic_ns",
            "start_monotonic_ns": start,
            "stop_monotonic_ns": start + int(elapsed_ms * 1_000_000),
            "elapsed_ms": elapsed_ms,
        },
        "rss": {
            "source": "linux-proc-statm",
            "processes": processes,
            "topology_rss_bytes": topology_rss,
        },
        "measurements": metrics,
        "semantic_observation": {
            "workload_id": workload,
            "case_id": case,
            "concurrency": concurrency,
        },
    }


def _structural(method: Any) -> dict[str, Any]:
    bounds = dict(method.document["structural_bounds"])
    return {
        key: (True if key.endswith("_required") else value)
        for key, value in bounds.items()
    }


def test_int_method_freezes_statistics_bounds_and_target_only_inputs() -> None:
    method = load_method()
    assert method.warmups == 3
    assert method.samples == 30
    assert method.bootstrap_resamples == 2000
    assert method.confidence_percent == 95
    assert method.workloads[-1].concurrency_levels == (1, 2, 4, 8)
    assert len(method.workloads) == 10
    assert not any("relational-fabric-v3" in path.as_posix() for path in INPUT_PATHS)


def test_int_schedule_is_deterministic_randomized_and_block_interleaved() -> None:
    method = load_method()
    first = build_schedule(method)
    second = build_schedule(method)
    assert schedule_sha256(first) == schedule_sha256(second)
    assert len(first) == 2244
    assert all(
        {first[index]["arm"], first[index + 1]["arm"]} == {"candidate", "control"}
        for index in range(0, len(first), 2)
    )
    assert {
        tuple(row["arm"] for row in first[index : index + 2])
        for index in range(0, len(first), 2)
    } == {
        ("candidate", "control"),
        ("control", "candidate"),
    }


def test_int_nearest_rank_and_bootstrap_are_frozen_and_deterministic() -> None:
    values = list(range(1, 31))
    assert nearest_rank(values, 0.95) == 29
    first = bootstrap_ci(
        values, statistic="p95", seed=17, resamples=2000, confidence_percent=95
    )
    second = bootstrap_ci(
        values, statistic="p95", seed=17, resamples=2000, confidence_percent=95
    )
    assert first == second
    assert all(math.isfinite(value) for value in first)


def test_beh_minimal_control_is_modern_stdio_and_contains_no_product_authority() -> (
    None
):
    assert description() == {
        "control_id": "minimal-fastmcp4-stdio-control-v1",
        "control_version": "1.0.0",
        "protocol_version": "2026-07-28",
        "transport": "direct-stdio",
        "tasks": False,
        "tools": ["no_op"],
        "resources": [],
        "prompts": [],
        "completion_handlers": [],
        "application_extensions": [],
        "daemon_channel": False,
    }


@pytest.mark.parametrize(
    "field,bad_value",
    [
        ("per_call_channel_construction_max", 1),
        ("per_call_server_construction_max", 1),
        ("python_sync_blocking_semantic_operations_max", 1),
        ("python_arrow_decodes_max", 1),
        ("python_threadpool_semantic_operations_max", 1),
        ("completion_candidates_max", 101),
        ("materialized_resource_pages_max", 2),
        ("queued_queries_total_max", 129),
        ("captured_log_bytes_max", 131073),
        ("orphan_processes_max", 1),
        ("cleanup_joined_required", False),
        ("stdout_protocol_only_required", False),
    ],
)
def test_ops_structural_faults_fail_without_timing_noise(
    field: str, bad_value: Any
) -> None:
    method = load_method()
    report = {"structural_observation": _structural(method)}
    report["structural_observation"][field] = bad_value
    with pytest.raises(ReleasePerformanceError) as captured:
        validate_structural_observation(report, method)
    assert captured.value.code == "WP50_STRUCTURAL_RESOURCE_FAULT"


def test_ops_missing_skipped_and_semantically_different_samples_fail_closed() -> None:
    method = load_method()
    schedule = build_schedule(method)
    samples = [_sample(request) for request in schedule]
    with pytest.raises(BenchmarkError) as missing:
        validate_samples(samples[:-1], method)
    assert missing.value.code == "WP50_SAMPLE_COUNT_INVALID"

    skipped = copy.deepcopy(samples)
    skipped[0]["skipped"] = True
    skipped[0]["status"] = "skipped"
    with pytest.raises(BenchmarkError) as skipped_error:
        validate_samples(skipped, method)
    assert skipped_error.value.code == "WP50_SAMPLE_NOT_SUCCESSFUL"

    different = copy.deepcopy(samples)
    different[1]["semantic_observation"]["fabricated"] = True
    with pytest.raises(BenchmarkError) as semantic:
        validate_samples(different, method)
    assert semantic.value.code == "WP50_SEMANTIC_DIFFERENTIAL"


def test_neg_history_independence_rejects_a_predecessor_binding() -> None:
    method = load_method()
    report = {"outlier_policy": "report-all-no-post-hoc-removal"}
    bindings = [{"path": path.as_posix()} for path in INPUT_PATHS]
    bindings[0]["path"] = "contracts/acceptance/relational-fabric-v3/evidence.json"
    with pytest.raises(ReleasePerformanceError) as captured:
        validate_history_independence(report, method, bindings)
    assert captured.value.code == "WP50_HISTORY_DEPENDENCY"
