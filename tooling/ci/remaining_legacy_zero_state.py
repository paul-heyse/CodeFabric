"""Prove that executable predecessor authority is absent from the live tree.

Historical design/plan/review material and released allocation evidence are retained
deliberately.  Everything else is discovered from the current filesystem, manifests,
package sources, and recipe registry; there is no frozen predecessor inventory.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
from collections.abc import Callable, Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

import tomllib

ROOT = Path(__file__).resolve().parents[2]
MAX_TEXT_BYTES = 16 * 1024 * 1024

# These are evidence, not executable/package/build authority.  Keep the exclusions narrow and
# named so adding a new historical hiding place changes this executable contract.
RETAINED_HISTORY_GLOBS = (
    "docs/designs/**",
    "docs/plans/**",
    "docs/reviews/**",
    "docs/authoritative_design/*_v1.3.md",
    "docs/authoritative_design/*_v2.0.md",
)
RELEASED_EVIDENCE_GLOBS = ("contracts/acceptance/released-artifact-census-v1.json",)
NON_EXECUTABLE_REFERENCE_GLOBS = ("docs/library_ref/**",)

# Negative-oracle definitions necessarily spell the things they reject.  They are executable
# guards, not retained implementations or compatibility aliases.
ORACLE_DEFINITION_GLOBS = (
    "scripts/model_zero_state_check.sh",
    "scripts/stable_graph_check.sh",
    "tooling/ci/remaining_legacy_zero_state.py",
    "tooling/ci/test_remaining_legacy_zero_state.py",
    "tooling/ci/successor_evidence_issuance.py",
    "tooling/ci/test_successor_evidence_issuance.py",
)

LIVE_SCAN_PATHS = (
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "Cargo.toml",
    "justfile",
    ".cargo",
    ".claude",
    ".codex",
    ".agents",
    ".config",
    ".github",
    "bacon.toml",
    "deny.toml",
    "src",
    "tests",
    "scripts",
    "tooling",
    "rules",
    "rule-tests",
    "contracts",
    "fuzz",
    "rustc-extractor",
    "pyrefly-sidecar",
    "codefabric-cpg-mcp",
    "docs/spec_index",
)

IGNORED_SOURCE_ROOTS = (
    "src",
    "tests",
    "scripts",
    "tooling",
    "rules",
    "rule-tests",
    "contracts",
    "fuzz",
    "rustc-extractor/src",
    "pyrefly-sidecar/src",
    "codefabric-cpg-mcp/src",
    "codefabric-cpg-mcp/tests",
    ".github",
)

FORBIDDEN_PATH_GLOBS = (
    "contracts/generated/model/**",
    "contracts/acceptance/relational-fabric-v1/**",
    "contracts/bundles/*-bundle.json",
    "contracts/governance/relational-fabric-legacy-selectors.json",
    "contracts/governance/relational-fabric-legacy-freeze.json",
    "tooling/ci/relational_fabric_transition.py",
    "tooling/ci/test_relational_fabric_transition.py",
    "tests/golden/**",
    "contracts/governance/semantic-provider-legacy-candidates.yaml",
    "scripts/semantic_provider_legacy_zero_state.sh",
    "tooling/ci/semantic_provider_legacy_zero_state.py",
    "tooling/ci/test_semantic_provider_legacy_zero_state.py",
    "tooling/ast-grep/semantic-provider-legacy/**",
    "rules/serving-projections-generated-only.yml",
    "rule-tests/serving-projections-generated-only-test.yml",
    "rule-tests/__snapshots__/serving-projections-generated-only-snapshot.yml",
    "contracts/identity/fingerprint-domain-registry.yaml",
    "contracts/comparison/comparison-ignore-registry.yaml",
    "contracts/fixtures/rebuild-comparison-manifest-v1.json",
    "tooling/ci/digest_domain_contracts.py",
    "tooling/ci/test_digest_domain_contracts.py",
    "contracts/semantic-fragments/**",
    "contracts/query/query-form-contract.json",
    "contracts/fixtures/registries/enum-flag-v1-vectors.json",
)

FORBIDDEN_TEXT = (
    "relational_fabric_transition",
    "relational-fabric-legacy-selectors",
    "relational-fabric-legacy-freeze",
    "contracts/acceptance/relational-fabric-v1",
    "contracts/generated/model",
    "v2-authority-cutover-check",
    "legacy-suite-current-authority-zero-state-check",
    "legacy-inventory-universe-check",
    "legacy-disposition-coverage-check",
    "legacy-authority-freeze-check",
    "MODEL_PACK_INCOMPATIBLE",
    "ONTOLOGY_GATE_",
    "ONTOLOGY_PROGRAM_",
    "ONTOLOGY_CANDIDATE_CLOSURE_INVALID",
    "ONTOLOGY_ACTIVATION_TRANSACTION_INVALID",
    'relation_id("model.',
    "semantic-provider-legacy-zero-state-check",
    "semantic_provider_legacy_zero_state",
    "semantic-provider-legacy-candidates",
    "provider-observations",
    "type_table_json",
    "callees_json",
    "diagnostics_json",
    "statement_kinds",
    "terminator_kinds",
    "digest-domain-contract-check",
    "provider-protocol-check",
    "wp59_",
    "fingerprint-domain-registry",
    "comparison-ignore-registry",
    "semantic-lane-fragment",
)

FORBIDDEN_RECIPES = frozenset(
    {
        "v2-authority-cutover-check",
        "legacy-suite-current-authority-zero-state-check",
        "legacy-inventory-universe-check",
        "legacy-disposition-coverage-check",
        "legacy-authority-freeze-check",
        "semantic-provider-legacy-zero-state-check",
        "digest-domain-contract-check",
        "provider-protocol-check",
    }
)
FORBIDDEN_CARGO_FEATURES = frozenset({"model-compiler", "provider-inventory-tooling"})
FORBIDDEN_CARGO_TARGETS = frozenset(
    {
        "codefabric-gate-b-candidate",
        "codefabric-model",
        "codefabric-model-schema-consumer",
        "codefabric-provider-inventory",
    }
)
CARGO_MANIFESTS = (
    "Cargo.toml",
    "rustc-extractor/Cargo.toml",
    "pyrefly-sidecar/Cargo.toml",
    "fuzz/Cargo.toml",
)

STRUCTURAL_QUERIES = (
    ("python", "from tooling.ci.relational_fabric_transition import $$$N"),
    ("python", "import tooling.ci.relational_fabric_transition"),
    ("python", "from codefabric_cpg_mcp.contracts.model_registries import $$$N"),
    ("rust", "mod ontology_candidate;"),
    ("rust", "mod ontology_program;"),
    ("rust", "mod functional_golden;"),
    ("rust", "mod gate_b_candidate;"),
    ("rust", "run_pyrefly($$$A)"),
    ("rust", "run_rustc($$$A)"),
)

COMPOSED_ZERO_STATE_COMMANDS = (("bash", "scripts/model_zero_state_check.sh"),)


class RemainingLegacyError(ValueError):
    """The live tree still contains predecessor authority or coverage is incomplete."""


def _matches(path: str, globs: Iterable[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in globs)


def _git_paths(root: Path, *arguments: str) -> list[str]:
    completed = subprocess.run(
        ("git", "ls-files", "-z", *arguments),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return sorted(
        item.decode("utf-8") for item in completed.stdout.split(b"\0") if item
    )


def current_files(root: Path) -> list[str]:
    """Return existing tracked and non-ignored untracked files, including dotfiles."""
    candidates = _git_paths(root, "--cached", "--others", "--exclude-standard")
    return [path for path in candidates if (root / path).is_file()]


def _ignored_source_issues(root: Path) -> tuple[list[str], int]:
    ignored = _git_paths(
        root,
        "--others",
        "--ignored",
        "--exclude-standard",
        "--",
        *IGNORED_SOURCE_ROOTS,
    )
    issues: list[str] = []
    cache_count = 0
    for path in ignored:
        if "/__pycache__/" in f"/{path}" or path.endswith((".pyc", ".pyo")):
            cache_count += 1
        else:
            issues.append(f"unclassified ignored live-source file: {path}")
    return issues, cache_count


def classify_paths(paths: Iterable[str]) -> tuple[list[str], list[str], list[str]]:
    """Classify current paths as live, retained history, or forbidden residue."""
    live: list[str] = []
    retained: list[str] = []
    issues: list[str] = []
    for path in sorted(set(paths)):
        if _matches(path, (*RETAINED_HISTORY_GLOBS, *RELEASED_EVIDENCE_GLOBS)):
            retained.append(path)
        elif _matches(path, FORBIDDEN_PATH_GLOBS):
            issues.append(f"forbidden predecessor path: {path}")
        else:
            live.append(path)
    return live, retained, issues


def _existing_scan_paths(root: Path, paths: Iterable[str]) -> list[str]:
    return [path for path in paths if (root / path).exists()]


def _negative_process_result(
    completed: subprocess.CompletedProcess[str], *, label: str
) -> list[str]:
    if completed.returncode == 1:
        return []
    if completed.returncode == 0:
        return [f"{label} matched forbidden live residue:\n{completed.stdout.strip()}"]
    raise RemainingLegacyError(
        f"{label} failed with exit {completed.returncode}: {completed.stderr.strip()}"
    )


def text_probe(root: Path, *, scan_paths: Sequence[str] = LIVE_SCAN_PATHS) -> list[str]:
    """Run a hidden-aware fixed-string negative search over live control surfaces."""
    command = [
        "rg",
        "--hidden",
        "--fixed-strings",
        "--no-heading",
        "--line-number",
        "--color=never",
    ]
    for token in FORBIDDEN_TEXT:
        command.extend(("-e", token))
    for pattern in (
        *RETAINED_HISTORY_GLOBS,
        *RELEASED_EVIDENCE_GLOBS,
        *NON_EXECUTABLE_REFERENCE_GLOBS,
        *ORACLE_DEFINITION_GLOBS,
    ):
        command.extend(("-g", f"!{pattern}"))
    command.extend(_existing_scan_paths(root, scan_paths))
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    return _negative_process_result(completed, label="fixed-string live-tree probe")


def structural_probe(root: Path) -> tuple[list[str], list[str]]:
    """Run syntax-aware probes for surviving imports and module registrations."""
    issues: list[str] = []
    coverage: list[str] = []
    roots_by_language = {
        "python": _existing_scan_paths(
            root,
            (
                "tooling",
                "scripts",
                "codefabric-cpg-mcp/src",
                "codefabric-cpg-mcp/tests",
            ),
        ),
        "rust": _existing_scan_paths(
            root, ("src", "tests", "rustc-extractor/src", "pyrefly-sidecar/src")
        ),
    }
    for language, pattern in STRUCTURAL_QUERIES:
        roots = roots_by_language[language]
        completed = subprocess.run(
            (
                "ast-grep",
                "run",
                "--lang",
                language,
                "--pattern",
                pattern,
                "--json=compact",
                "--inspect",
                "summary",
                "--globs",
                f"*.{'py' if language == 'python' else 'rs'}",
                *roots,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        coverage.append(completed.stderr.strip())
        issues.extend(
            _negative_process_result(
                completed, label=f"ast-grep {language} pattern {pattern!r}"
            )
        )
    return issues, coverage


def cargo_payload_issues(
    payloads: Iterable[Mapping[str, Any]],
) -> tuple[list[str], int]:
    """Reject predecessor features and targets from discovered Cargo metadata."""
    issues: list[str] = []
    package_count = 0
    for payload in payloads:
        for package in payload.get("packages", []):
            package_count += 1
            name = str(package.get("name", "<unnamed>"))
            features = set(package.get("features", {}))
            for feature in sorted(features & FORBIDDEN_CARGO_FEATURES):
                issues.append(f"forbidden Cargo feature {name}#{feature}")
            for target in package.get("targets", []):
                target_name = str(target.get("name", ""))
                if target_name in FORBIDDEN_CARGO_TARGETS:
                    issues.append(f"forbidden Cargo target {name}#{target_name}")
    return issues, package_count


def cargo_inventory(root: Path) -> tuple[list[str], int]:
    payloads: list[Mapping[str, Any]] = []
    for manifest in CARGO_MANIFESTS:
        completed = subprocess.run(
            (
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
                manifest,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            raise RemainingLegacyError(
                f"Cargo metadata failed for {manifest}: {completed.stderr.strip()}"
            )
        payloads.append(json.loads(completed.stdout))
    return cargo_payload_issues(payloads)


def recipe_issues(root: Path) -> tuple[list[str], int]:
    completed = subprocess.run(
        ("just", "--dump", "--dump-format", "json"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RemainingLegacyError(
            f"recipe enumeration failed: {completed.stderr.strip()}"
        )
    recipes = set(json.loads(completed.stdout).get("recipes", {}))
    issues = [
        f"forbidden predecessor recipe: {name}"
        for name in sorted(recipes & FORBIDDEN_RECIPES)
    ]
    return issues, len(recipes)


def python_package_issues(
    root: Path, live_paths: Iterable[str]
) -> tuple[list[str], int]:
    """Validate the Python distribution declaration and discover its current source payload."""
    pyproject = root / "codefabric-cpg-mcp/pyproject.toml"
    payload = tomllib.loads(pyproject.read_text(encoding="utf-8"))
    if payload.get("project", {}).get("name") != "codefabric-cpg-mcp":
        return ["unexpected Python package identity"], 0
    prefix = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/"
    package_paths = [path for path in live_paths if path.startswith(prefix)]
    issues = [
        f"forbidden predecessor Python package path: {path}"
        for path in package_paths
        if _matches(path, FORBIDDEN_PATH_GLOBS)
    ]
    return issues, len(package_paths)


def run_composed_zero_state(
    root: Path,
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> None:
    """Delegate the remaining model-authority class to its permanent specialist guard."""
    for command in COMPOSED_ZERO_STATE_COMMANDS:
        completed = runner(
            command,
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            raise RemainingLegacyError(
                f"composed zero-state check failed ({' '.join(command)}):\n"
                f"{completed.stdout}{completed.stderr}"
            )


def validate_remaining_legacy(
    root: Path = ROOT, *, run_composed: bool = True
) -> dict[str, object]:
    """Validate current live surfaces without a frozen predecessor census."""
    files = current_files(root)
    live, retained, issues = classify_paths(files)
    ignored_issues, ignored_cache_count = _ignored_source_issues(root)
    issues.extend(ignored_issues)
    issues.extend(text_probe(root))
    structural_issues, structural_coverage = structural_probe(root)
    issues.extend(structural_issues)
    cargo_issues, cargo_package_count = cargo_inventory(root)
    issues.extend(cargo_issues)
    current_recipe_issues, recipe_count = recipe_issues(root)
    issues.extend(current_recipe_issues)
    package_issues, python_package_file_count = python_package_issues(root, live)
    issues.extend(package_issues)

    if issues:
        raise RemainingLegacyError("\n".join(issues))
    if run_composed:
        run_composed_zero_state(root)
    return {
        "live_file_count": len(live),
        "retained_history_or_release_evidence_file_count": len(retained),
        "ignored_cache_file_count": ignored_cache_count,
        "cargo_package_count": cargo_package_count,
        "python_package_file_count": python_package_file_count,
        "recipe_count": recipe_count,
        "structural_probes": len(structural_coverage),
        "composed_zero_state_checks": len(COMPOSED_ZERO_STATE_COMMANDS)
        if run_composed
        else 0,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    arguments = parser.parse_args(argv)
    try:
        report = validate_remaining_legacy(arguments.root.resolve())
    except (
        OSError,
        RemainingLegacyError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
    ) as error:
        print(f"remaining legacy zero-state check failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
