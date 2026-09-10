"""Run selected real daemon/MCP scenarios. This command never certifies all CPG scope."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path

from tooling.product.corpus import compare
from tooling.product.process import run

ROOT = Path(__file__).resolve().parents[2]
CASES = {
    "startup": "wp44_beh_real_supervisor_ready_requires_durable_fresh_activation",
    "python-serving": "wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent",
    "reopen": "wp63_ops_installed_restart_reconstructs_only_exact_activation_authority",
    "cancellation": "wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation",
    "rust-failure": "pragmatic_rust_target_failure_retains_other_targets",
    "python-live": "pragmatic_live_python_edits_converge_without_restart",
    "mixed-clean-live": "live_updates::mixed_live_updates_equal_independent_clean_public_queries",
    "staged-live": "live_updates::source_current_publication_fences_delayed_semantics_and_resumes_after_restart",
    "python-context-live": "live_updates::live_python_context_and_negative_imports_equal_independent_clean_queries",
    "python-stubs-live": "live_updates::live_python_namespace_stub_precedence_equals_independent_clean_queries",
    "python-roots-live": "live_updates::live_python_search_paths_preserve_all_sources_and_equal_independent_clean_queries",
    "python-paths-live": "live_updates::live_python_raw_paths_and_root_initializer_keep_exact_source_identity",
    "decoded-source-live": "live_updates::live_mixed_decoded_sources_equal_original_bytes_and_independent_clean_queries",
    "rust-paths-live": "live_updates::mixed_raw_path_inventory_keeps_rust_calls_across_updates_and_clean_reopen",
    "cargo-build-live": "live_updates::custom_cargo_build_input_changes_context_and_matches_clean_public_results",
    "cargo-platforms-live": "live_updates::cargo_configured_platforms_and_flags_converge_with_clean_public_queries",
    "cargo-linkage-live": "live_updates::cargo_library_linkage_kinds_survive_live_queries_and_clean_reopen",
    "cargo-selections-live": "live_updates::cargo_feature_and_profile_selections_keep_partial_contexts_and_equal_clean_queries",
    "source-lines-live": "live_updates::source_line_windows_and_hard_limits_survive_public_delivery_and_reopen",
    "function-source-live": "live_updates::live_mixed_function_definitions_and_bodies_equal_exact_clean_source",
    "processing-pages": "live_updates::processing_remainder_pages_keep_exact_scope_across_reopen_and_updates",
}


def select(names: list[str] | None) -> list[str]:
    result = list(CASES) if names is None else names
    if not result or any(name not in CASES for name in result):
        raise ValueError("select at least one known product case")
    return list(dict.fromkeys(result))


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", action="append", choices=CASES)
    parser.add_argument("--list", action="store_true")
    parser.add_argument(
        "--timeout",
        type=float,
        help="per-case deadline; defaults to 240s, or 600s for mixed/context clean/live, staged and processing-page builds",
    )
    parser.add_argument(
        "--output", type=Path, default=ROOT / "target/product/golden.json"
    )
    parser.add_argument(
        "--scenario",
        type=Path,
        help="existing modern client driver scenario against a configured real supervisor",
    )
    parser.add_argument(
        "--expect",
        type=Path,
        help="independently authored expected driver-report fragments, required with --scenario",
    )
    args = parser.parse_args(argv)
    if args.list:
        print("\n".join(CASES))
        return 0
    if bool(args.scenario) != bool(args.expect):
        parser.error("--scenario and --expect must be supplied together")
    observations = []
    success = True
    if args.scenario:
        commands = [
            (
                "public-scenario",
                [
                    sys.executable,
                    str(ROOT / "tooling/fastmcp4_modern_client_driver.py"),
                    str(args.scenario.resolve()),
                ],
            )
        ]
    else:
        commands = [
            (
                name,
                [
                    "cargo",
                    "nextest",
                    "run",
                    "--locked",
                    "--test",
                    "integration",
                    "-E",
                    f"test(=integration::daemon::{CASES[name]})",
                    "--no-tests=fail",
                ],
            )
            for name in select(args.case)
        ]
    for name, command in commands:
        print(f"product case: {name}", flush=True)
        try:
            timeout = (
                args.timeout
                if args.timeout is not None
                else (
                    600
                    if name
                    in {
                        "mixed-clean-live",
                        "python-context-live",
                        "python-stubs-live",
                        "python-roots-live",
                        "python-paths-live",
                        "function-source-live",
                        "source-lines-live",
                        "rust-paths-live",
                        "cargo-build-live",
                        "cargo-platforms-live",
                        "cargo-linkage-live",
                        "cargo-selections-live",
                        "decoded-source-live",
                        "staged-live",
                        "processing-pages",
                    }
                    else 240
                )
            )
            outcome = run(command, cwd=ROOT, timeout=timeout)
            observation = {"case": name, **asdict(outcome)}
            success = outcome.returncode == 0
            if success and args.expect:
                expected = json.loads(args.expect.read_text())
                if not expected:
                    raise ValueError("expected fragments cannot be empty")
                compare(json.loads(outcome.stdout), expected)
        except (OSError, ValueError, AssertionError, RuntimeError) as error:
            observation = {"case": name, "error": str(error), "returncode": 1}
            success = False
        observations.append(observation)
        print(f"{name}: {'passed' if success else 'failed'}", flush=True)
        if not success:
            print(
                str(observation.get("stderr", observation.get("error", "")))[-6000:],
                file=sys.stderr,
            )
            break
    report = {
        "kind": "real-product-scenarios",
        "revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "working_tree": subprocess.check_output(
            ["git", "status", "--short"], cwd=ROOT, text=True
        ),
        "passed": success and len(observations) == len(commands),
        "selected": [name for name, _ in commands],
        "not_run": [name for name, _ in commands[len(observations) :]],
        "observations": observations,
        "remaining_target": "All-family semantics, complete query meanings/composition, fine-grained remainder, broader context/edit coverage and sustained operation remain open; selected static/live/clean comparisons do not certify the entire product.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
