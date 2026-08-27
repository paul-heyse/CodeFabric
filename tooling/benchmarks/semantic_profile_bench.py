"""Reproducible, non-normative WP36 semantic substrate micro-workloads."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "tooling/benchmarks/semantic-profile-baseline-v1.json"


def fingerprint(parts: list[str]) -> str:
    digest = hashlib.sha256()
    for part in parts:
        encoded = part.encode()
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return "sha256:" + digest.hexdigest()


def workload(size: int, repetitions: int) -> tuple[str, float]:
    fixture = bytes((index * 31 + 17) % 251 for index in range(size))
    started = time.perf_counter_ns()
    digest = b""
    for _ in range(repetitions):
        digest = hashlib.sha256(fixture).digest()
    elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000
    return digest.hex(), elapsed_ms


def main() -> None:
    baseline = json.loads(BASELINE.read_text(encoding="utf-8"))
    rustc = subprocess.run(
        ["rustc", "--version"], check=True, text=True, capture_output=True
    ).stdout.strip()
    machine = fingerprint(
        [platform.system(), platform.release(), platform.machine(), str(os.cpu_count())]
    )
    context = fingerprint(
        [
            rustc,
            hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
            baseline["version"],
            "WP36-semantic-substrate-v1",
        ]
    )
    results: dict[str, dict[str, object]] = {}
    for name, definition in sorted(baseline["workloads"].items()):
        size = int(definition["bytes"])
        cold_digest, cold = workload(size, 1)
        warm_samples = [workload(size, 1)[1] for _ in range(5)]
        warm_digest, _ = workload(size, 1)
        if warm_digest != cold_digest:
            raise RuntimeError(f"{name} workload digest drifted")
        results[name] = {
            "bytes": size,
            "fixture_digest": "sha256:" + cold_digest,
            "cold_milliseconds": round(cold, 4),
            "warm_milliseconds_median": round(statistics.median(warm_samples), 4),
            "baseline": definition,
            "baseline_normative": False,
        }
    print(
        json.dumps(
            {
                "artifact": "semantic-profile-bench-v1",
                "machine_fingerprint": machine,
                "context_fingerprint": context,
                "warm_cold_distinguished": True,
                "results": results,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
