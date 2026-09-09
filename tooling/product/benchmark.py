"""Repeat real product scenarios with attributable wall-time observations."""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import sys
from pathlib import Path

from tooling.product.golden import CASES, ROOT
from tooling.product.process import run


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=CASES, default="startup")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument(
        "--output", type=Path, default=ROOT / "target/benchmarks/product.json"
    )
    args = parser.parse_args(argv)
    if args.samples < 1:
        parser.error("--samples must be positive")
    samples = []
    ok = True
    for number in range(args.samples):
        child_report = args.output.parent / f"{args.case}-{number}.json"
        outcome = run(
            [
                sys.executable,
                "-m",
                "tooling.product.golden",
                "--case",
                args.case,
                "--timeout",
                str(args.timeout),
                "--output",
                str(child_report),
            ],
            cwd=ROOT,
            timeout=args.timeout + 10,
        )
        samples.append(
            {
                "wall_seconds_including_runner_and_build": outcome.elapsed_seconds,
                "returncode": outcome.returncode,
                "case_report": str(child_report),
            }
        )
        if outcome.returncode:
            ok = False
            break
    report = {
        "kind": "product-scenario-timing",
        "case": args.case,
        "machine": platform.platform(),
        "settings": vars(args) | {"output": str(args.output)},
        "samples": samples,
        "passed": ok,
        "median_wall_seconds": statistics.median(
            s["wall_seconds_including_runner_and_build"] for s in samples
        )
        if ok
        else None,
        "unavailable_metrics": [
            "edit-to-syntax including debounce",
            "semantic convergence",
            "backlog",
            "steady-state query latency",
            "daemon-only peak RSS",
            "retained disk",
            "recovery time breakdown",
        ],
        "note": "No latency threshold is implied. Integration-test timing includes startup, build and harness costs; use production telemetry for individual phases.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
