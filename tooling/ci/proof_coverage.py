"""Validate and materialize the Tier-A command proof graph."""

from __future__ import annotations

import argparse
import json
import subprocess
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "tooling/ci/proof-coverage.json"
DEFAULT_REPORT = ROOT / "target/proof-coverage-current.json"


class ProofCoverageError(ValueError):
    """The declared and executable proof graphs differ."""


def _run(*args: str) -> str:
    return subprocess.run(
        args,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def load_manifest(path: Path = MANIFEST) -> dict[str, Any]:
    """Load the committed proof model."""
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ProofCoverageError("proof manifest root must be an object")
    return value


def load_just_recipes() -> dict[str, Any]:
    """Read the exact current recipe graph from Just's JSON interface."""
    dump = json.loads(_run("just", "--dump", "--dump-format", "json"))
    recipes = dump.get("recipes")
    if not isinstance(recipes, dict):
        raise ProofCoverageError("just JSON did not contain a recipe map")
    return recipes


def recipe_dependencies(recipe: Mapping[str, Any]) -> list[str]:
    """Return dependency names in execution order."""
    return [dependency["recipe"] for dependency in recipe.get("dependencies", [])]


def recipe_commands(recipe: Mapping[str, Any]) -> list[str]:
    """Return exact command lines from a Just recipe body."""
    return [line for body_line in recipe.get("body", []) for line in body_line]


def proof_closure(
    recipe: str,
    declarations: Mapping[str, Mapping[str, Any]],
    dependencies: Mapping[str, Sequence[str]],
    active: frozenset[str] = frozenset(),
) -> set[str]:
    """Resolve the transitive proof atoms supplied by one recipe."""
    if recipe in active:
        raise ProofCoverageError(f"recipe dependency cycle at {recipe}")
    if recipe not in declarations:
        raise ProofCoverageError(f"recipe lacks proof declaration: {recipe}")
    proofs = set(declarations[recipe].get("proofs", []))
    for dependency in dependencies.get(recipe, []):
        proofs.update(
            proof_closure(dependency, declarations, dependencies, active | {recipe})
        )
    return proofs


def compare_candidate(
    candidate: Mapping[str, Any],
    declarations: Mapping[str, Mapping[str, Any]],
    dependencies: Mapping[str, Sequence[str]],
) -> dict[str, Any]:
    """Prove a before/after aggregate edit preserves all proof atoms."""
    before = set()
    for recipe in candidate["before_dependencies"]:
        before.update(proof_closure(recipe, declarations, dependencies))
    after = set()
    for recipe in candidate["after_dependencies"]:
        after.update(proof_closure(recipe, declarations, dependencies))
    if before != after:
        raise ProofCoverageError(
            f"{candidate['id']} changes proof coverage: "
            f"removed={sorted(before - after)}, added={sorted(after - before)}"
        )
    return {"before": sorted(before), "after": sorted(after)}


def _pytest_node_ids(selector: str) -> set[str]:
    output = _run(
        "uv",
        "run",
        "--frozen",
        "--project",
        "codefabric-cpg-mcp",
        "pytest",
        "--collect-only",
        "-q",
        selector,
    )
    return {line.strip() for line in output.splitlines() if "::" in line}


def _assert_no_destructive_commands(commands: Iterable[str]) -> None:
    forbidden = ("cargo clean", "sccache --stop-server", "sccache --zero-stats")
    for command in commands:
        if any(token in command for token in forbidden):
            raise ProofCoverageError(
                f"Tier-A graph contains destructive cache step: {command}"
            )


def validate(
    manifest: Mapping[str, Any],
    just_recipes: Mapping[str, Mapping[str, Any]],
    *,
    collect_pytest: bool,
) -> dict[str, Any]:
    """Validate declarations and return an exact executable proof report."""
    if manifest.get("schema_version") != 1:
        raise ProofCoverageError("unsupported proof manifest schema version")
    declarations = manifest.get("recipes")
    if not isinstance(declarations, dict):
        raise ProofCoverageError("proof manifest recipes must be an object")

    live_dependencies = {
        name: recipe_dependencies(recipe) for name, recipe in just_recipes.items()
    }
    for name, declaration in declarations.items():
        if name not in just_recipes:
            raise ProofCoverageError(f"declared recipe is absent from Just: {name}")
        expected = declaration.get("dependencies")
        if expected is not None and expected != live_dependencies[name]:
            raise ProofCoverageError(
                f"dependency drift for {name}: expected {expected}, "
                f"observed {live_dependencies[name]}"
            )

    aggregate = str(manifest["aggregate"])
    aggregate_proofs = proof_closure(aggregate, declarations, live_dependencies)
    candidates = []
    for candidate in manifest.get("optimization_candidates", []):
        if candidate["after_dependencies"] != live_dependencies[candidate["aggregate"]]:
            raise ProofCoverageError(
                f"optimized dependency graph drifted: {candidate['id']}"
            )
        if candidate["retained_independent_recipe"] not in just_recipes:
            raise ProofCoverageError(
                f"independent diagnostic recipe was removed: {candidate['id']}"
            )
        comparison = compare_candidate(candidate, declarations, live_dependencies)
        selection: dict[str, Any] = {"verified": False}
        if collect_pytest:
            full = _pytest_node_ids(candidate["full_pytest_selector"])
            targeted = _pytest_node_ids(candidate["targeted_pytest_selector"])
            if not targeted or not targeted <= full:
                raise ProofCoverageError(
                    f"targeted pytest selection is not covered by the full suite: {candidate['id']}"
                )
            selection = {
                "verified": True,
                "full_count": len(full),
                "targeted_count": len(targeted),
                "targeted_node_ids": sorted(targeted),
            }
        candidates.append({"id": candidate["id"], **comparison, "selection": selection})

    declared_names = set(declarations)
    reachable = set()
    pending = [aggregate]
    while pending:
        name = pending.pop()
        if name in reachable:
            continue
        reachable.add(name)
        pending.extend(live_dependencies[name])
    missing = reachable - declared_names
    if missing:
        raise ProofCoverageError(
            f"reachable Tier-A recipes lack declarations: {sorted(missing)}"
        )

    exact_recipes = {
        name: {
            "dependencies": live_dependencies[name],
            "commands": recipe_commands(just_recipes[name]),
            "proofs": sorted(proof_closure(name, declarations, live_dependencies)),
        }
        for name in sorted(reachable)
    }
    _assert_no_destructive_commands(
        command for recipe in exact_recipes.values() for command in recipe["commands"]
    )
    return {
        "schema_version": 1,
        "profile": manifest["profile"],
        "aggregate": aggregate,
        "proof_count": len(aggregate_proofs),
        "proofs": sorted(aggregate_proofs),
        "recipes": exact_recipes,
        "optimization_candidates": candidates,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--skip-pytest-collection", action="store_true")
    args = parser.parse_args()
    report = validate(
        load_manifest(),
        load_just_recipes(),
        collect_pytest=not args.skip_pytest_collection,
    )
    report_path = args.report if args.report.is_absolute() else ROOT / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        f"validated {report['proof_count']} Tier-A proof atoms; "
        f"wrote {report_path.relative_to(ROOT)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
