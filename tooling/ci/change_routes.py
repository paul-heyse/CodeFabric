"""Conservative CI routing by real build consumers; unknown paths run all domains."""

from __future__ import annotations

import argparse
import subprocess

DOMAINS = ("stable-root", "adapter", "contracts", "governance", "extractor", "sidecar")


def routes(paths: list[str], *, full: bool = False) -> dict[str, bool]:
    result = dict.fromkeys(DOMAINS, full)
    result["tooling"] = True
    for path in paths:
        if path.startswith(("docs/", ".claude/", ".codex/", ".agents/")) or path in {
            "AGENTS.md",
            "CLAUDE.md",
            "README.md",
            "STATUS.md",
        }:
            continue
        if path.startswith(("tooling/ci/", "tooling/product/", "tooling/benchmarks/")):
            continue
        if path.startswith("rustc-extractor/"):
            affected = ("extractor", "contracts")
        elif path.startswith("pyrefly-sidecar/"):
            affected = ("sidecar", "contracts")
        elif path.startswith("codefabric-cpg-mcp/"):
            affected = ("adapter", "contracts")
        elif path.startswith(("src/", "tests/")):
            affected = ("stable-root", "contracts", "governance")
        else:
            # Shared Cargo, native, shell, CI, wire, rules and unclassified inputs.
            affected = DOMAINS
        for domain in affected:
            result[domain] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--full", action="store_true")
    args = parser.parse_args()
    paths = (
        []
        if args.full
        else subprocess.check_output(
            ["git", "diff", "--name-only", "-z", f"{args.base}...{args.head}"]
        )
        .decode()
        .strip("\0")
        .split("\0")
    )
    for key, value in routes(paths, full=args.full).items():
        print(f"{key}={str(value).lower()}")


if __name__ == "__main__":
    main()
