"""Fail closed if the FastMCP package acquires semantic or mutable CPG authority."""

from __future__ import annotations

import ast
import re
import sys
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]
ADAPTER = ROOT / "codefabric-cpg-mcp"
PACKAGE = ADAPTER / "src" / "codefabric_cpg_mcp"

SEMANTIC_RUNTIME_PACKAGES = {
    "datafusion",
    "deltalake",
    "duckdb",
    "pandas",
    "polars",
    "pyarrow",
    "shelve",
    "sqlite3",
}
RETIRED_AUTHORITY_MODULES = {
    "contracts/fingerprints.py",
    "contracts/model_registries.py",
    "contracts/query_forms.py",
    "contracts/schemas.py",
}
MUTABLE_SESSION_CALLS = {
    "get_state",
    "set_state",
    "delete_state",
    "session_state_store",
}


def dependency_name(requirement: str) -> str:
    match = re.match(r"[A-Za-z0-9_.-]+", requirement)
    if match is None:
        raise ValueError(f"invalid dependency requirement: {requirement!r}")
    return match.group(0).lower().replace("_", "-")


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def main() -> int:
    errors: list[str] = []
    project = tomllib.loads((ADAPTER / "pyproject.toml").read_text(encoding="utf-8"))
    dependencies = {
        dependency_name(requirement)
        for requirement in project["project"].get("dependencies", [])
    }
    forbidden_dependencies = dependencies & SEMANTIC_RUNTIME_PACKAGES
    if forbidden_dependencies:
        fail(
            errors,
            "semantic runtime dependencies are forbidden: "
            + ", ".join(sorted(forbidden_dependencies)),
        )

    source_paths = sorted(
        path
        for path in PACKAGE.rglob("*.py")
        if "generated" not in path.relative_to(PACKAGE).parts
    )
    relative_sources = {path.relative_to(PACKAGE).as_posix() for path in source_paths}
    surviving_authority = relative_sources & RETIRED_AUTHORITY_MODULES
    if surviving_authority:
        fail(
            errors,
            "retired Python authority modules survive: "
            + ", ".join(sorted(surviving_authority)),
        )

    fastmcp_constructors = 0
    for path in source_paths:
        source = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT).as_posix()
        if "tcp://" in source:
            fail(errors, f"{relative}: production TCP daemon target survives")
        tree = ast.parse(source, filename=str(path))
        for node in ast.walk(tree):
            imported: str | None = None
            if isinstance(node, ast.Import):
                for alias in node.names:
                    root = alias.name.partition(".")[0]
                    if root in SEMANTIC_RUNTIME_PACKAGES:
                        fail(
                            errors, f"{relative}:{node.lineno}: forbidden import {root}"
                        )
            elif isinstance(node, ast.ImportFrom) and node.module:
                imported = node.module.partition(".")[0]
            if imported in SEMANTIC_RUNTIME_PACKAGES:
                fail(errors, f"{relative}:{node.lineno}: forbidden import {imported}")
            if isinstance(node, ast.Call):
                if isinstance(node.func, ast.Name) and node.func.id == "FastMCP":
                    fastmcp_constructors += 1
                if (
                    isinstance(node.func, ast.Attribute)
                    and node.func.attr in MUTABLE_SESSION_CALLS
                ):
                    fail(
                        errors,
                        f"{relative}:{node.lineno}: mutable FastMCP session state call "
                        f"{node.func.attr}",
                    )

    if fastmcp_constructors != 1:
        fail(
            errors,
            f"expected exactly one presentation server construction, found {fastmcp_constructors}",
        )

    if errors:
        for error in errors:
            print(f"fastmcp-presentation-boundary: {error}", file=sys.stderr)
        return 1
    print(
        "fastmcp-presentation-boundary: UDS-only presentation package has no Python semantic "
        "runtime, retired authority module, or mutable session-state seam"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
