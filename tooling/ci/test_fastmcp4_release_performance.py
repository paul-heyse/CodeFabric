"""Falsification tests for the frozen WP65 method and measured evidence."""

from __future__ import annotations

import copy
import math

import pytest

from tooling.benchmarks.fastmcp4_release_benchmark import (
    DEFAULT_REPORT_PATH,
    METHOD_PATH,
    ROOT,
    BenchmarkError,
    bootstrap_interval,
    load_json,
    load_method,
    nearest_rank,
)
from tooling.ci.fastmcp4_release_performance import (
    DEFAULT_SUMMARY_PATH,
    ReleasePerformanceError,
    summary_document,
    validate_report_document,
    validate_summary,
)


def test_resource_envelope_method_integrity() -> None:
    method = load_method()
    authority = method.document["authority"]
    assert authority["registered_before_candidate_results"] is True
    assert authority["candidate_results_used"] is False
    assert authority["predecessor_comparison_permitted"] is False
    assert authority["local_bound_relaxation_permitted"] is False
    assert len(method.workloads) == 10
    assert len(method.required_coverage) == 32
    assert len(method.structural_contracts) == 7
    assert sum(workload.samples for workload in method.workloads) == 30
    assert sum(workload.warmups for workload in method.workloads) == 6
    assert {workload.command[0] for workload in method.workloads} == {"just"}
    assert sum(workload.isolated_cargo_target for workload in method.workloads) == 1
    text = METHOD_PATH if METHOD_PATH.is_absolute() else ROOT / METHOD_PATH
    assert text.name == "workloads.json"
    encoded = str(method.document).lower()
    assert "minimal-fastmcp" not in encoded
    assert ("relational-fabric-" + "v5") not in encoded


def test_performance_optimization_semantic_equivalence() -> None:
    method = load_method()
    report = load_json(DEFAULT_REPORT_PATH)
    validate_report_document(report, method)
    observations = {
        sample["workload_id"]: sample["observation"] for sample in report["samples"]
    }
    assert (
        observations["inprocess_tree_sitter_ruff"]["incremental_equals_clean"] is True
    )
    assert observations["retained_pyrefly"]["final_equal_clean"] is True
    assert observations["datafusion_stream"]["drop_cancels_stream"] is True
    assert (
        observations["installed_fastmcp_vertical"]["semantic_change_observed"] is True
    )
    assert observations["installed_fastmcp_vertical"]["baseline_rows"] == 1
    assert observations["installed_fastmcp_vertical"]["mutated_rows"] == 2
    assert any(
        row["recipe"] == "semantic-release-vertical-check" and row["exit_code"] == 0
        for row in report["structural_runs"]
    )


def test_unbounded_resource_and_method_drift_faults() -> None:
    method = load_method()
    report = load_json(DEFAULT_REPORT_PATH)

    missing = copy.deepcopy(report)
    missing["samples"].pop()
    with pytest.raises(ReleasePerformanceError) as missing_error:
        validate_report_document(missing, method)
    assert missing_error.value.code == "WP65_SAMPLE_COUNT_INVALID"

    unbounded = copy.deepcopy(report)
    unbounded["samples"][0]["peak_process_tree_rss_bytes"] = 1 << 62
    with pytest.raises(ReleasePerformanceError) as resource_error:
        validate_report_document(unbounded, method)
    assert resource_error.value.code == "WP65_RESOURCE_ENVELOPE_EXCEEDED"

    semantic = copy.deepcopy(report)
    sample = next(
        row
        for row in semantic["samples"]
        if row["workload_id"] == "inprocess_tree_sitter_ruff"
    )
    sample["observation"]["incremental_equals_clean"] = False
    with pytest.raises(ReleasePerformanceError) as semantic_error:
        validate_report_document(semantic, method)
    assert semantic_error.value.code == "WP65_SEMANTIC_OBSERVATION_DRIFT"

    structural = copy.deepcopy(report)
    structural["structural_runs"][0]["exit_code"] = 1
    with pytest.raises(ReleasePerformanceError) as structural_error:
        validate_report_document(structural, method)
    assert structural_error.value.code == "WP65_STRUCTURAL_PROOF_INCOMPLETE"


def test_review_summary_is_a_total_derivation_of_raw_samples() -> None:
    method = load_method()
    report = load_json(DEFAULT_REPORT_PATH)
    summary = load_json(DEFAULT_SUMMARY_PATH)
    validate_summary(report, summary, method)
    drifted = copy.deepcopy(summary)
    drifted["verdict"] = "passed_with_relaxed_bound"
    with pytest.raises(ReleasePerformanceError) as error:
        validate_summary(report, drifted, method)
    assert error.value.code == "WP65_REVIEW_SUMMARY_DRIFT"
    assert summary == summary_document(report, method)


def test_nearest_rank_and_bootstrap_uncertainty_are_deterministic() -> None:
    values = [1.0, 2.0, 3.0, 4.0, 5.0]
    assert nearest_rank(values, 0) == 1
    assert nearest_rank(values, 0.5) == 3
    assert nearest_rank(values, 0.95) == 5
    first = bootstrap_interval(
        values, statistic="p95", seed=17, resamples=2000, confidence=95
    )
    second = bootstrap_interval(
        values, statistic="p95", seed=17, resamples=2000, confidence=95
    )
    assert first == second
    assert first is not None and all(math.isfinite(item) for item in first)
    assert (
        bootstrap_interval(
            values[:2], statistic="median", seed=17, resamples=2000, confidence=95
        )
        is None
    )


def test_duplicate_json_members_fail_closed(tmp_path) -> None:
    path = tmp_path / "duplicate.json"
    path.write_text('{"schema":"one","schema":"two"}', encoding="utf-8")
    with pytest.raises(BenchmarkError) as error:
        load_json(path)
    assert error.value.code == "WP65_JSON_DUPLICATE_MEMBER"
