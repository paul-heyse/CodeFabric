"""Run the pre-registered WP65 target-only resource/performance method.

The runner executes real product assurance scenarios through the repository's
``just`` command boundary.  It never compares to a predecessor.  Every sample
uses monotonic time and polls the complete Linux process tree for aggregate RSS
and process count.  Component tests may emit one bounded
``CODEFABRIC_WP65_OBSERVATION=`` JSON record to expose phase-local measurements.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import random
import signal
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from importlib.metadata import version
from pathlib import Path
from typing import Any, NoReturn

ROOT = Path(__file__).resolve().parents[2]
METHOD_PATH = Path("tests/fixtures/fastmcp4_performance/workloads.json")
DEFAULT_REPORT_PATH = Path(
    "contracts/evidence/relational-fabric-v7/wp65-raw-performance-v2.json"
)
METHOD_SCHEMA = "codefabric.compiled-release-performance.method.v2"
REPORT_SCHEMA = "codefabric.compiled-release-performance.raw-report.v2"
METHOD_ID = "relational-fabric-v7-final-target-v1"
METHOD_REVISION = "wp65-preregistered-v4"
OBSERVATION_PREFIX = b"CODEFABRIC_WP65_OBSERVATION="
MAX_CAPTURE_BYTES = 67_108_864
MAX_FAILURE_DIAGNOSTIC_BYTES = 4_096


class BenchmarkError(ValueError):
    """Fail-closed benchmark error with a stable diagnostic code."""

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
        _require(key not in result, "WP65_JSON_DUPLICATE_MEMBER", key)
        result[key] = value
    return result


def load_json(path: Path, *, maximum_bytes: int = MAX_CAPTURE_BYTES) -> dict[str, Any]:
    try:
        size = path.stat().st_size
        _require(size <= maximum_bytes, "WP65_JSON_TOO_LARGE", str(path))
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicates,
            parse_constant=lambda item: _fail("WP65_JSON_NONFINITE", item),
        )
    except BenchmarkError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BenchmarkError("WP65_JSON_INVALID", str(path)) from error
    _require(isinstance(value, dict), "WP65_JSON_ROOT_INVALID", str(path))
    return value


def _closed(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    _require(set(value) == expected, "WP65_METHOD_SCHEMA_INVALID", context)


def _positive_int(value: object, context: str, *, allow_zero: bool = False) -> int:
    minimum = 0 if allow_zero else 1
    _require(
        isinstance(value, int) and not isinstance(value, bool) and value >= minimum,
        "WP65_METHOD_VALUE_INVALID",
        context,
    )
    return int(value)


def _strings(value: object, context: str) -> tuple[str, ...]:
    _require(
        isinstance(value, list)
        and bool(value)
        and all(isinstance(item, str) and item for item in value),
        "WP65_METHOD_VALUE_INVALID",
        context,
    )
    return tuple(value)


@dataclass(frozen=True)
class Workload:
    workload_id: str
    coverage: tuple[str, ...]
    command: tuple[str, ...]
    state_classification: str
    data_scale: str
    warmups: int
    samples: int
    timeout_seconds: int
    isolated_cargo_target: bool
    required_observation_fields: tuple[str, ...]
    observation_expectations: Mapping[str, Any]
    ceilings: Mapping[str, int]


@dataclass(frozen=True)
class Method:
    document: Mapping[str, Any]
    workloads: tuple[Workload, ...]
    structural_contracts: tuple[tuple[str, str], ...]
    required_coverage: frozenset[str]
    sampling_interval_millis: int
    bootstrap_seed: int
    bootstrap_resamples: int
    confidence_percent: int


def load_method(root: Path = ROOT, path: Path = METHOD_PATH) -> Method:
    document = load_json(path if path.is_absolute() else root / path)
    _closed(
        document,
        {
            "schema",
            "method_id",
            "method_revision",
            "authority",
            "statistics",
            "environment",
            "structural_contracts",
            "required_coverage",
            "workloads",
        },
        "root",
    )
    _require(
        document.get("schema") == METHOD_SCHEMA
        and document.get("method_id") == METHOD_ID
        and document.get("method_revision") == METHOD_REVISION,
        "WP65_METHOD_IDENTITY_DRIFT",
        "identity",
    )
    authority = document.get("authority")
    _require(isinstance(authority, dict), "WP65_METHOD_SCHEMA_INVALID", "authority")
    assert isinstance(authority, dict)
    _require(
        authority.get("packet") == "WP65"
        and authority.get("registered_before_candidate_results") is True
        and authority.get("candidate_results_used") is False
        and authority.get("predecessor_comparison_permitted") is False
        and authority.get("local_bound_relaxation_permitted") is False,
        "WP65_METHOD_AUTHORITY_DRIFT",
        "authority",
    )
    statistics = document.get("statistics")
    _require(isinstance(statistics, dict), "WP65_METHOD_SCHEMA_INVALID", "statistics")
    assert isinstance(statistics, dict)
    _require(
        statistics.get("clock") == "monotonic_ns"
        and statistics.get("memory_source") == "linux-proc-process-tree-rss"
        and statistics.get("percentile_policy") == "nearest-rank"
        and statistics.get("reported_percentiles") == [0, 50, 95, 100]
        and statistics.get("outlier_policy") == "report-all-no-post-hoc-removal",
        "WP65_STATISTICAL_METHOD_DRIFT",
        "statistics",
    )
    required_coverage = frozenset(
        _strings(document.get("required_coverage"), "coverage")
    )
    raw_structural = document.get("structural_contracts")
    _require(
        isinstance(raw_structural, list) and bool(raw_structural),
        "WP65_METHOD_SCHEMA_INVALID",
        "structural contracts",
    )
    structural: list[tuple[str, str]] = []
    for row in raw_structural:
        _require(isinstance(row, dict), "WP65_METHOD_SCHEMA_INVALID", "structural row")
        assert isinstance(row, dict)
        _closed(row, {"contract_id", "recipe"}, "structural row")
        contract_id = row.get("contract_id")
        recipe = row.get("recipe")
        _require(
            isinstance(contract_id, str)
            and bool(contract_id)
            and isinstance(recipe, str)
            and bool(recipe),
            "WP65_METHOD_VALUE_INVALID",
            "structural row",
        )
        structural.append((contract_id, recipe))
    _require(
        len({item[0] for item in structural}) == len(structural)
        and len({item[1] for item in structural}) == len(structural),
        "WP65_METHOD_SCHEMA_INVALID",
        "duplicate structural contract",
    )
    raw_workloads = document.get("workloads")
    _require(
        isinstance(raw_workloads, list) and bool(raw_workloads),
        "WP65_METHOD_SCHEMA_INVALID",
        "workloads",
    )
    workloads: list[Workload] = []
    for raw in raw_workloads:
        _require(isinstance(raw, dict), "WP65_METHOD_SCHEMA_INVALID", "workload")
        assert isinstance(raw, dict)
        _closed(
            raw,
            {
                "workload_id",
                "coverage",
                "command",
                "state_classification",
                "data_scale",
                "warmups",
                "samples",
                "timeout_seconds",
                "isolated_cargo_target",
                "required_observation_fields",
                "observation_expectations",
                "ceilings",
            },
            "workload",
        )
        workload_id = raw.get("workload_id")
        command = _strings(raw.get("command"), f"{workload_id}.command")
        _require(
            isinstance(workload_id, str)
            and bool(workload_id)
            and len(command) == 2
            and command[0] == "just"
            and command[1].startswith("_wp65-measure-"),
            "WP65_COMMAND_METHOD_DRIFT",
            str(workload_id),
        )
        expectations = raw.get("observation_expectations")
        ceilings = raw.get("ceilings")
        _require(
            isinstance(expectations, dict) and isinstance(ceilings, dict),
            "WP65_METHOD_SCHEMA_INVALID",
            str(workload_id),
        )
        assert isinstance(expectations, dict) and isinstance(ceilings, dict)
        _require(
            set(ceilings)
            == {
                "elapsed_p95_millis",
                "peak_process_tree_rss_bytes",
                "maximum_processes",
                "stdout_bytes",
                "stderr_bytes",
            }
            and all(
                isinstance(item, int) and not isinstance(item, bool) and item > 0
                for item in ceilings.values()
            ),
            "WP65_BOUND_METHOD_DRIFT",
            str(workload_id),
        )
        required_fields_value = raw.get("required_observation_fields")
        _require(
            isinstance(required_fields_value, list)
            and all(isinstance(item, str) and item for item in required_fields_value),
            "WP65_METHOD_VALUE_INVALID",
            f"{workload_id}.required observation fields",
        )
        assert isinstance(required_fields_value, list)
        required_fields = tuple(required_fields_value)
        _require(
            set(expectations).issubset(required_fields),
            "WP65_METHOD_SCHEMA_INVALID",
            f"{workload_id}.expectations",
        )
        state = raw.get("state_classification")
        scale = raw.get("data_scale")
        isolated = raw.get("isolated_cargo_target")
        _require(
            isinstance(state, str)
            and bool(state)
            and isinstance(scale, str)
            and bool(scale)
            and isinstance(isolated, bool),
            "WP65_METHOD_VALUE_INVALID",
            str(workload_id),
        )
        workloads.append(
            Workload(
                workload_id=workload_id,
                coverage=_strings(raw.get("coverage"), f"{workload_id}.coverage"),
                command=command,
                state_classification=state,
                data_scale=scale,
                warmups=_positive_int(raw.get("warmups"), "warmups", allow_zero=True),
                samples=_positive_int(raw.get("samples"), "samples"),
                timeout_seconds=_positive_int(raw.get("timeout_seconds"), "timeout"),
                isolated_cargo_target=isolated,
                required_observation_fields=required_fields,
                observation_expectations=expectations,
                ceilings={str(key): int(value) for key, value in ceilings.items()},
            )
        )
    observed_coverage = {item for workload in workloads for item in workload.coverage}
    _require(
        observed_coverage == required_coverage,
        "WP65_COVERAGE_METHOD_DRIFT",
        "workload coverage",
    )
    _require(
        len({item.workload_id for item in workloads}) == len(workloads)
        and sum(item.isolated_cargo_target for item in workloads) == 1,
        "WP65_METHOD_SCHEMA_INVALID",
        "workload identity or isolated compile census",
    )
    method_text = json.dumps(document, sort_keys=True).lower()
    _require(
        "minimal-fastmcp" not in method_text
        and "direct-daemon-control" not in method_text
        and ("relational-fabric-" + "v5") not in method_text,
        "WP65_PREDECESSOR_METHOD_INPUT",
        "method",
    )
    return Method(
        document=document,
        workloads=tuple(workloads),
        structural_contracts=tuple(structural),
        required_coverage=required_coverage,
        sampling_interval_millis=_positive_int(
            statistics.get("sampling_interval_millis"), "sampling interval"
        ),
        bootstrap_seed=_positive_int(
            statistics.get("bootstrap_seed"), "bootstrap seed"
        ),
        bootstrap_resamples=_positive_int(
            statistics.get("bootstrap_resamples"), "bootstrap resamples"
        ),
        confidence_percent=_positive_int(
            statistics.get("confidence_percent"), "confidence percent"
        ),
    )


def nearest_rank(values: Sequence[float], percentile: float) -> float:
    _require(bool(values), "WP65_DISTRIBUTION_EMPTY", "nearest rank")
    _require(0 <= percentile <= 1, "WP65_PERCENTILE_INVALID", str(percentile))
    ordered = sorted(float(item) for item in values)
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def bootstrap_interval(
    values: Sequence[float],
    *,
    statistic: str,
    seed: int,
    resamples: int,
    confidence: int,
) -> list[float] | None:
    if len(values) < 3:
        return None
    _require(statistic in {"median", "p95"}, "WP65_STATISTIC_INVALID", statistic)
    rng = random.Random(seed)
    observed = [float(item) for item in values]
    estimates: list[float] = []
    for _ in range(resamples):
        sample = [rng.choice(observed) for _ in observed]
        estimates.append(nearest_rank(sample, 0.5 if statistic == "median" else 0.95))
    tail = (100 - confidence) / 200
    return [nearest_rank(estimates, tail), nearest_rank(estimates, 1 - tail)]


def distribution(
    values: Sequence[float], method: Method, identity: str
) -> dict[str, Any]:
    _require(bool(values), "WP65_DISTRIBUTION_EMPTY", identity)
    seed = method.bootstrap_seed + sum(identity.encode("utf-8"))
    return {
        "count": len(values),
        "minimum": nearest_rank(values, 0),
        "p50": nearest_rank(values, 0.5),
        "p95": nearest_rank(values, 0.95),
        "maximum": nearest_rank(values, 1),
        "median_confidence_interval": bootstrap_interval(
            values,
            statistic="median",
            seed=seed,
            resamples=method.bootstrap_resamples,
            confidence=method.confidence_percent,
        ),
        "p95_confidence_interval": bootstrap_interval(
            values,
            statistic="p95",
            seed=seed + 1,
            resamples=method.bootstrap_resamples,
            confidence=method.confidence_percent,
        ),
    }


def _git(root: Path, *arguments: str) -> str:
    try:
        completed = subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise BenchmarkError("WP65_GIT_FAILED", " ".join(arguments)) from error
    return completed.stdout.strip()


def _process_table() -> dict[int, tuple[int, int]]:
    result: dict[int, tuple[int, int]] = {}
    page_size = os.sysconf("SC_PAGE_SIZE")
    for path in Path("/proc").iterdir():
        if not path.name.isdigit():
            continue
        try:
            stat = (path / "stat").read_text(encoding="utf-8")
            suffix = stat[stat.rfind(")") + 2 :].split()
            parent = int(suffix[1])
            resident_pages = int(
                (path / "statm").read_text(encoding="ascii").split()[1]
            )
        except (OSError, UnicodeError, ValueError, IndexError):
            continue
        result[int(path.name)] = (parent, resident_pages * page_size)
    return result


def _tree_resource_sample(root_pid: int) -> tuple[int, int]:
    table = _process_table()
    selected = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, (parent, _) in table.items():
            if parent in selected and pid not in selected:
                selected.add(pid)
                changed = True
    resident = sum(table.get(pid, (0, 0))[1] for pid in selected)
    return resident, len(selected & table.keys())


def _parse_observation(stdout: bytes, stderr: bytes) -> Mapping[str, Any] | None:
    records: list[dict[str, Any]] = []
    for line in (*stdout.splitlines(), *stderr.splitlines()):
        marker = line.find(OBSERVATION_PREFIX)
        if marker < 0:
            continue
        payload = line[marker + len(OBSERVATION_PREFIX) :]
        try:
            value = json.loads(payload, object_pairs_hook=_reject_duplicates)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise BenchmarkError(
                "WP65_OBSERVATION_INVALID", payload[:200].decode(errors="replace")
            ) from error
        _require(isinstance(value, dict), "WP65_OBSERVATION_INVALID", "root")
        records.append(value)
    _require(len(records) <= 1, "WP65_OBSERVATION_DUPLICATE", str(len(records)))
    return records[0] if records else None


def _failure_tail(output: bytes) -> str:
    clipped = output[-MAX_FAILURE_DIAGNOSTIC_BYTES:]
    prefix = "<earlier output omitted>\n" if len(output) > len(clipped) else ""
    return prefix + clipped.decode("utf-8", errors="replace")


def _terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)


def execute_workload(
    workload: Workload,
    *,
    phase: str,
    sample_index: int,
    root: Path,
    cargo_target: Path | None,
    sampling_interval_millis: int,
) -> dict[str, Any]:
    environment = os.environ.copy()
    environment["CODEFABRIC_WP65_MEASURE"] = "1"
    command = workload.command
    if cargo_target is not None:
        _require(
            workload.isolated_cargo_target,
            "WP65_ISOLATED_TARGET_BINDING_INVALID",
            workload.workload_id,
        )
        try:
            target_argument = cargo_target.relative_to(root).as_posix()
        except ValueError as error:
            raise BenchmarkError(
                "WP65_ISOLATED_TARGET_BINDING_INVALID", str(cargo_target)
            ) from error
        command = (*workload.command, target_argument)
    start = time.monotonic_ns()
    peak_rss = 0
    maximum_processes = 0
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            process = subprocess.Popen(
                command,
                cwd=root,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
        except OSError as error:
            raise BenchmarkError(
                "WP65_WORKLOAD_START_FAILED", workload.workload_id
            ) from error
        deadline = time.monotonic() + workload.timeout_seconds
        try:
            while process.poll() is None:
                rss, count = _tree_resource_sample(process.pid)
                peak_rss = max(peak_rss, rss)
                maximum_processes = max(maximum_processes, count)
                if time.monotonic() >= deadline:
                    _terminate_process_group(process)
                    _fail("WP65_WORKLOAD_TIMEOUT", workload.workload_id)
                time.sleep(sampling_interval_millis / 1_000)
        except BaseException:
            if process.poll() is None:
                _terminate_process_group(process)
            raise
        stop = time.monotonic_ns()
        rss, count = _tree_resource_sample(process.pid)
        peak_rss = max(peak_rss, rss)
        maximum_processes = max(maximum_processes, count, 1)
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        _require(
            stdout_size <= MAX_CAPTURE_BYTES and stderr_size <= MAX_CAPTURE_BYTES,
            "WP65_WORKLOAD_OUTPUT_UNBOUNDED",
            workload.workload_id,
        )
        stdout.seek(0)
        stderr.seek(0)
        stdout_bytes = stdout.read()
        stderr_bytes = stderr.read()
    if process.returncode != 0:
        _fail(
            "WP65_WORKLOAD_FAILED",
            (
                f"{workload.workload_id} exited {process.returncode}; "
                f"stdout_tail={_failure_tail(stdout_bytes)!r}; "
                f"stderr_tail={_failure_tail(stderr_bytes)!r}"
            ),
        )
    observation = _parse_observation(stdout_bytes, stderr_bytes)
    if workload.required_observation_fields:
        _require(
            observation is not None, "WP65_OBSERVATION_MISSING", workload.workload_id
        )
        assert observation is not None
        _require(
            observation.get("workload_id") == workload.workload_id
            and set(observation)
            == {"workload_id", *workload.required_observation_fields},
            "WP65_OBSERVATION_CENSUS_INVALID",
            workload.workload_id,
        )
    return {
        "workload_id": workload.workload_id,
        "phase": phase,
        "sample_index": sample_index,
        "state_classification": workload.state_classification,
        "data_scale": workload.data_scale,
        "command": list(command),
        "start_monotonic_ns": start,
        "stop_monotonic_ns": stop,
        "elapsed_millis": (stop - start) / 1_000_000,
        "peak_process_tree_rss_bytes": peak_rss,
        "maximum_processes": maximum_processes,
        "stdout_bytes": stdout_size,
        "stderr_bytes": stderr_size,
        "exit_code": process.returncode,
        "observation": dict(observation) if observation is not None else {},
    }


def _structural_run(
    contract_id: str, recipe: str, root: Path, sampling_interval_millis: int
) -> dict[str, Any]:
    workload = Workload(
        workload_id=f"structural:{contract_id}",
        coverage=(contract_id,),
        command=("just", recipe),
        state_classification="structural-independent-of-timing",
        data_scale="recipe-owned-discriminating-fixtures",
        warmups=0,
        samples=1,
        timeout_seconds=900,
        isolated_cargo_target=False,
        required_observation_fields=(),
        observation_expectations={},
        ceilings={
            "elapsed_p95_millis": 900_000,
            "peak_process_tree_rss_bytes": 17_179_869_184,
            "maximum_processes": 1_024,
            "stdout_bytes": MAX_CAPTURE_BYTES,
            "stderr_bytes": MAX_CAPTURE_BYTES,
        },
    )
    observed = execute_workload(
        workload,
        phase="structural",
        sample_index=0,
        root=root,
        cargo_target=None,
        sampling_interval_millis=sampling_interval_millis,
    )
    return {"contract_id": contract_id, "recipe": recipe, **observed}


def _cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unavailable"


def _memory_bytes() -> int:
    try:
        for line in Path("/proc/meminfo").read_text(encoding="ascii").splitlines():
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    return 0


def _power_profile() -> str:
    path = Path("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
    try:
        return path.read_text(encoding="ascii").strip() or "unavailable"
    except OSError:
        return "unavailable"


def _environment(root: Path, method_commit: str) -> dict[str, Any]:
    status_lines = _git(
        root, "status", "--porcelain=v1", "--untracked-files=all"
    ).splitlines()
    tracked = [line for line in status_lines if not line.startswith("?? ")]
    _require(not tracked, "WP65_TRACKED_TREE_DIRTY", repr(tracked))
    return {
        "recorded_at_utc": datetime.now(UTC).isoformat(),
        "repository_head": _git(root, "rev-parse", "HEAD"),
        "repository_tree": _git(root, "rev-parse", "HEAD^{tree}"),
        "method_commit": method_commit,
        "untracked_paths": sorted(
            line[3:] for line in status_lines if line.startswith("?? ")
        ),
        "operating_system": platform.platform(),
        "kernel_version": platform.release(),
        "architecture": platform.machine(),
        "cpu_model": _cpu_model(),
        "logical_cpu_count": os.cpu_count() or 0,
        "memory_bytes": _memory_bytes(),
        "power_profile": _power_profile(),
        "load_average": list(os.getloadavg()),
        "process_count": sum(path.name.isdigit() for path in Path("/proc").iterdir()),
        "python_version": platform.python_version(),
        "fastmcp_version": version("fastmcp"),
        "mcp_version": version("mcp"),
        "pydantic_version": version("pydantic"),
        "grpcio_version": version("grpcio"),
        "host_profile": "local-workstation-v1",
    }


def build_summaries(
    samples: Sequence[Mapping[str, Any]], method: Method
) -> dict[str, Any]:
    summaries: dict[str, Any] = {}
    for workload in method.workloads:
        selected = [
            sample
            for sample in samples
            if sample.get("workload_id") == workload.workload_id
            and sample.get("phase") == "measured"
        ]
        summaries[workload.workload_id] = {
            field: distribution(
                [float(sample[field]) for sample in selected],
                method,
                f"{workload.workload_id}:{field}",
            )
            for field in (
                "elapsed_millis",
                "peak_process_tree_rss_bytes",
                "maximum_processes",
                "stdout_bytes",
                "stderr_bytes",
            )
        }
    return summaries


def run_method(method: Method, *, root: Path, output: Path) -> dict[str, Any]:
    method_commit = _git(root, "log", "-1", "--format=%H", "--", str(METHOD_PATH))
    head = _git(root, "rev-parse", "HEAD")
    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", method_commit, head],
        cwd=root,
        check=False,
    )
    _require(ancestor.returncode == 0, "WP65_METHOD_NOT_PREREGISTERED", method_commit)
    environment = _environment(root, method_commit)
    samples: list[dict[str, Any]] = []
    release_root: Path | None = None
    for workload in method.workloads:
        cargo_target: Path | None = None
        if workload.isolated_cargo_target:
            release_root = root / "target" / "wp65-release" / head[:16]
            _require(
                not release_root.exists(),
                "WP65_ISOLATED_TARGET_EXISTS",
                str(release_root),
            )
            release_root.mkdir(parents=True)
            cargo_target = release_root
        for index in range(workload.warmups):
            execute_workload(
                workload,
                phase="warmup",
                sample_index=index,
                root=root,
                cargo_target=cargo_target,
                sampling_interval_millis=method.sampling_interval_millis,
            )
        for index in range(workload.samples):
            samples.append(
                execute_workload(
                    workload,
                    phase="measured",
                    sample_index=index,
                    root=root,
                    cargo_target=cargo_target,
                    sampling_interval_millis=method.sampling_interval_millis,
                )
            )
    if release_root is not None:
        artifacts = {}
        for name in ("codefabric", "codefabricd"):
            path = release_root / "release" / name
            _require(path.is_file(), "WP65_RELEASE_ARTIFACT_MISSING", str(path))
            artifacts[name] = {
                "bytes": path.stat().st_size,
                "path": str(path.relative_to(root)),
            }
        environment["release_artifacts"] = artifacts
    structural_runs = [
        _structural_run(
            contract_id, recipe, root, method.sampling_interval_millis
        )
        for contract_id, recipe in method.structural_contracts
    ]
    report = {
        "schema": REPORT_SCHEMA,
        "method_id": METHOD_ID,
        "method_revision": METHOD_REVISION,
        "candidate_commit": environment["repository_head"],
        "candidate_tree": environment["repository_tree"],
        "method_commit": method_commit,
        "environment": environment,
        "samples_deleted_after_observation": 0,
        "failed_samples": 0,
        "skipped_samples": 0,
        "samples": samples,
        "summaries": build_summaries(samples, method),
        "structural_runs": structural_runs,
    }
    destination = output if output.is_absolute() else root / output
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(report, stream, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
    except OSError as error:
        raise BenchmarkError("WP65_REPORT_ALREADY_EXISTS", str(destination)) from error
    return report


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("method")
    run = subparsers.add_parser("run")
    run.add_argument("--output", type=Path, default=DEFAULT_REPORT_PATH)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        method = load_method()
        if arguments.command == "method":
            print(
                json.dumps(
                    {
                        "method_id": METHOD_ID,
                        "method_revision": METHOD_REVISION,
                        "workload_count": len(method.workloads),
                        "measured_sample_count": sum(
                            item.samples for item in method.workloads
                        ),
                        "coverage_count": len(method.required_coverage),
                    },
                    sort_keys=True,
                )
            )
        else:
            report = run_method(method, root=ROOT, output=arguments.output)
            print(
                json.dumps(
                    {
                        "candidate_commit": report["candidate_commit"],
                        "sample_count": len(report["samples"]),
                        "structural_contract_count": len(report["structural_runs"]),
                    },
                    sort_keys=True,
                )
            )
        return 0
    except BenchmarkError as error:
        print(f"{error.code}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
