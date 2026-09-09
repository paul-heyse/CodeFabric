"""Harness tests use controlled children; they do not claim product correctness."""

import asyncio
import json
import sys
from pathlib import Path

import pytest

from tooling.product.corpus import apply_edit, clean_incremental, compare
from tooling.product.golden import select
from tooling.product.process import run


def test_modern_client_barrier_observes_controller_and_has_a_deadline(
    tmp_path, monkeypatch
):
    from tooling.fastmcp4_modern_client_driver import (
        DriverError,
        _execute_step,
        _validate_steps,
    )

    monkeypatch.chdir(tmp_path)
    step = {"id": "checkpoint", "operation": "barrier", "name": "policy"}
    _validate_steps([step])
    with pytest.raises(DriverError, match="SCENARIO_BARRIER_INVALID"):
        _validate_steps([{**step, "name": "../escape"}])

    async def exercise():
        task = asyncio.create_task(_execute_step(None, step, {}, 1.0))
        async with asyncio.timeout(1):
            while not Path("policy.ready").exists():
                await asyncio.sleep(0.01)
        assert not task.done()
        Path("policy.resume").write_text("resume\n")
        assert await task == {"resumed": True}
        with pytest.raises(TimeoutError):
            await _execute_step(None, {**step, "name": "no-controller"}, {}, 0.02)

    asyncio.run(exercise())


def test_guard_choice_uses_live_label_and_rejects_missing_or_ambiguous_choices():
    from tooling.fastmcp4_modern_client_driver import (
        DriverError,
        _materialize_guard_content,
    )

    marker = {
        "value": {"$requested_schema_presentation": "Python function declarations"}
    }
    choices = {
        "choice:other": "Rust constants",
        "choice:function": "Python function declarations",
    }
    field = {"enum": list(choices), "x-codefabric-choice-presentations": choices}
    schema = {"properties": {"value": field}}
    assert _materialize_guard_content(marker, schema) == {"value": "choice:function"}
    field["enum"].reverse()
    assert _materialize_guard_content(marker, schema) == {"value": "choice:function"}
    choices["choice:other"] = choices["choice:function"]
    with pytest.raises(DriverError, match="AMBIGUOUS_OR_MISSING"):
        _materialize_guard_content(marker, schema)
    with pytest.raises(DriverError, match="SCHEMA_MISSING"):
        _materialize_guard_content(marker, {})


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
        "python-live",
        "mixed-clean-live",
        "staged-live",
        "python-context-live",
        "processing-pages",
    ]


@pytest.mark.parametrize(
    "case",
    ["mixed-clean-live", "python-context-live", "staged-live", "processing-pages"],
)
@pytest.mark.parametrize(("override", "expected"), [(None, 600), (0.25, 0.25)])
def test_clean_live_case_has_a_bounded_default_and_honors_explicit_deadline(
    tmp_path, monkeypatch, override, expected, case
):
    from tooling.product import golden
    from tooling.product.process import Outcome

    observed = []

    def execute(command, *, cwd, timeout):
        observed.append((command, timeout))
        return Outcome(command, 0, 0.1, "", "", False, False)

    monkeypatch.setattr(golden, "run", execute)
    args = ["--case", case, "--output", str(tmp_path / "result.json")]
    if override is not None:
        args.extend(["--timeout", str(override)])
    assert golden.main(args) == 0
    assert len(observed) == 1 and observed[0][1] == expected
    assert f"test(=integration::daemon::{golden.CASES[case]})" in observed[0][0]
