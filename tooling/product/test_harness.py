"""Harness tests use controlled children; they do not claim product correctness."""

import json
import sys
from pathlib import Path

import pytest

from tooling.product.corpus import apply_edit, clean_incremental, compare
from tooling.product.golden import select
from tooling.product.process import run


def test_failure_timeout_and_bounded_output(tmp_path):
    failed = run(
        [sys.executable, "-c", 'import sys; print("failure"); sys.exit(7)'],
        cwd=tmp_path,
    )
    assert failed.returncode == 7 and "failure" in failed.stdout
    timeout = run(
        [sys.executable, "-c", "import time; time.sleep(10)"], cwd=tmp_path, timeout=0.1
    )
    assert timeout.returncode == 124 and timeout.timed_out
    noisy = run(
        [sys.executable, "-c", 'print("x" * 100000)'], cwd=tmp_path, maximum_bytes=128
    )
    assert noisy.returncode == 125 and len(noisy.stdout) <= 128


def test_empty_unknown_selection_and_semantic_mismatch():
    for value in ([], ["unknown"]):
        with pytest.raises(ValueError):
            select(value)
    with pytest.raises(AssertionError):
        compare(
            {"facts": [], "coverage": "unavailable"},
            {"facts": [], "coverage": "complete"},
        )
    with pytest.raises(AssertionError):
        compare({"path": ["b", "a"]}, {"path": ["a", "b"]})


def test_edit_paths_and_differential_failure(tmp_path):
    with pytest.raises(ValueError):
        apply_edit(tmp_path, {"operation": "write", "path": "../escape", "text": ""})
    observations = iter(
        [{"facts": [1], "coverage": "complete"}, {"facts": [], "coverage": "complete"}]
    )
    with pytest.raises(AssertionError, match="clean/incremental"):
        clean_incremental(
            tmp_path,
            [{"operation": "write", "path": "sample.py", "text": "x = 1"}],
            lambda: next(observations),
            lambda: None,
            lambda: None,
        )


def test_fixture_sequence_is_applicable(tmp_path):
    import shutil

    root = Path(__file__).resolve().parents[2] / "tests/fixtures/pragmatic_cpg"
    shutil.copytree(root / "workspace", tmp_path, dirs_exist_ok=True)
    for edit in json.loads((root / "edits.json").read_text()):
        apply_edit(tmp_path, edit)
    assert not (tmp_path / "renamed_helpers.py").exists()
    assert "repaired" in (tmp_path / "src/lib.rs").read_text()


def test_benchmark_failure_has_no_successful_latency(tmp_path, monkeypatch):
    from tooling.product import benchmark
    from tooling.product.process import Outcome

    monkeypatch.setattr(
        benchmark,
        "run",
        lambda *args, **kwargs: Outcome(
            ["product"], 7, 0.1, "", "failed", False, False
        ),
    )
    output = tmp_path / "measure.json"
    assert benchmark.main(["--samples", "2", "--output", str(output)]) == 1
    report = json.loads(output.read_text())
    assert report["passed"] is False and report["median_wall_seconds"] is None
    assert len(report["samples"]) == 1


def test_golden_stops_and_records_not_run_after_failure(tmp_path, monkeypatch):
    from tooling.product import golden
    from tooling.product.process import Outcome

    monkeypatch.setattr(
        golden,
        "run",
        lambda *args, **kwargs: Outcome(
            ["product"], 7, 0.1, "", "startup failed", False, False
        ),
    )
    output = tmp_path / "golden.json"
    assert golden.main(["--output", str(output)]) == 1
    report = json.loads(output.read_text())
    assert report["passed"] is False and report["not_run"] == [
        "python-serving",
        "reopen",
        "cancellation",
    ]
