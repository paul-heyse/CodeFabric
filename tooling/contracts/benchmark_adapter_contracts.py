"""Controlled microbenchmarks for the generated adapter contract substrate."""

import json
import statistics
import subprocess
import sys
import time
from collections.abc import Callable
from importlib.metadata import version
from typing import Any

from codefabric_cpg_mcp.contracts.json import canonicalize_value
from codefabric_cpg_mcp.contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    QueryCounts,
    StatusToolOutput,
)
from fastmcp import FastMCP


def _per_operation(call: Callable[[], Any], *, operations: int, rounds: int = 9) -> int:
    samples = []
    for _ in range(rounds):
        started = time.perf_counter_ns()
        for _ in range(operations):
            call()
        samples.append((time.perf_counter_ns() - started) // operations)
    return int(statistics.median(samples))


def _import_startup_ns() -> int:
    samples = []
    statement = (
        "from codefabric_cpg_mcp.contracts.wire_models import MODEL_TYPES; "
        "assert MODEL_TYPES"
    )
    for _ in range(7):
        started = time.perf_counter_ns()
        subprocess.run(
            [sys.executable, "-c", statement],
            check=True,
            capture_output=True,
        )
        samples.append(time.perf_counter_ns() - started)
    return int(statistics.median(samples))


def main() -> int:
    counts_input = {"fact_count": 12, "result_count": 3, "truncated": False}
    counts = QueryCounts(**counts_input)
    status = StatusToolOutput(
        ready=True,
        workspace_id="workspace-benchmark",
        agent_instance_id="agent-benchmark",
        snapshot=None,
        versions={"adapter": "1.0"},
        supported_languages=("python", "rust"),
        supported_request_forms=("probe",),
        capability_statuses=(),
        freshness_state="CURRENT",
        service_limits={},
        notices=(),
    )
    measurements = {
        "python_import_startup_ns": _import_startup_ns(),
        "model_validate_ns_per_op": _per_operation(
            lambda: QueryCounts.model_validate(counts_input), operations=10_000
        ),
        "model_serialize_ns_per_op": _per_operation(
            lambda: counts.model_dump(mode="json"), operations=10_000
        ),
        "json_adapter_validate_ns_per_op": _per_operation(
            lambda: JSON_OBJECT_ADAPTER.validate_python({"kind": "probe", "limit": 10}),
            operations=10_000,
        ),
        "canonical_request_ns_per_op": _per_operation(
            lambda: canonicalize_value({"kind": "probe", "limit": 10}),
            operations=10_000,
        ),
        "public_envelope_serialize_ns_per_op": _per_operation(
            lambda: status.model_dump(mode="json"), operations=10_000
        ),
        "schema_build_ns_per_op": _per_operation(
            lambda: StatusToolOutput.model_json_schema(mode="serialization"),
            operations=50,
        ),
        "fastmcp_construction_ns_per_op": _per_operation(
            lambda: FastMCP(name="contract-benchmark", tasks=False), operations=100
        ),
    }
    evidence = {
        "profile": "codefabric-adapter-contract-benchmark-v1",
        "python": sys.version.split()[0],
        "fastmcp": version("fastmcp"),
        "pydantic": version("pydantic"),
        "decision": "eager module-scope model and TypeAdapter build; defer_build not justified",
        "measurements": measurements,
    }
    print(json.dumps(evidence, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
