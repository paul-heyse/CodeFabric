"""Validate staged adapter projections through real FastMCP, Pytest, and wheel consumers."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PACKAGE_PREFIX = Path("codefabric-cpg-mcp/src/codefabric_cpg_mcp")


def _run(
    arguments: list[str], *, cwd: Path, environment: dict[str, str] | None = None
) -> None:
    subprocess.run(
        arguments,
        cwd=cwd,
        env=environment,
        check=True,
        text=True,
    )


def _copy_project(destination: Path) -> Path:
    project = destination / "codefabric-cpg-mcp"
    shutil.copytree(
        ROOT / "codefabric-cpg-mcp",
        project,
        ignore=shutil.ignore_patterns(".venv", "__pycache__", ".pytest_cache", "*.pyc"),
    )
    return project


def _overlay(stage: Path, project: Path, package_outputs: list[dict[str, str]]) -> None:
    for record in package_outputs:
        relative = Path(record["path"])
        source = stage / PACKAGE_PREFIX / relative
        destination = project / "src/codefabric_cpg_mcp" / relative
        if not source.is_file():
            raise RuntimeError(f"staged package output is absent: {source}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)


def _wheel_paths(wheel: Path) -> set[str]:
    with zipfile.ZipFile(wheel) as archive:
        return set(archive.namelist())


def validate(stage: Path) -> None:
    package_manifest_path = (
        stage / PACKAGE_PREFIX / "contracts/adapter-package-data.json"
    )
    manifest = json.loads(package_manifest_path.read_text(encoding="utf-8"))
    if manifest["package"] != "codefabric_cpg_mcp":
        raise RuntimeError("adapter package manifest names the wrong package")
    outputs = manifest["outputs"]
    if not isinstance(outputs, list) or not outputs:
        raise RuntimeError("adapter package output census is empty")
    paths = [record["path"] for record in outputs]
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        raise RuntimeError("adapter package output paths are not unique and sorted")
    required_kinds = {
        "pydantic-model-source",
        "pydantic-schema-manifest",
        "pydantic-fingerprint-manifest",
        "fastmcp-fingerprint-module",
        "python-package-manifest",
    }
    if {record["projection_kind"] for record in outputs} != required_kinds:
        raise RuntimeError("adapter package projection kinds differ")

    with tempfile.TemporaryDirectory(
        prefix="adapter-consumer.", dir=stage
    ) as directory:
        consumer_root = Path(directory)
        project = _copy_project(consumer_root)
        _overlay(stage, project, outputs)
        environment = os.environ.copy()
        environment.pop("VIRTUAL_ENV", None)
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        environment["PYTHONPATH"] = str(project / "src")
        ruff = ROOT / "codefabric-cpg-mcp/.venv/bin/ruff"
        pyrefly = ROOT / "codefabric-cpg-mcp/.venv/bin/pyrefly"
        pytest = ROOT / "codefabric-cpg-mcp/.venv/bin/pytest"
        generated_python = [
            project / "src/codefabric_cpg_mcp/contracts/wire_models.py",
            project / "src/codefabric_cpg_mcp/contracts/fingerprints.py",
        ]
        _run([str(ruff), "format", "--check", *map(str, generated_python)], cwd=ROOT)
        _run([str(ruff), "check", *map(str, generated_python)], cwd=ROOT)
        python_files = sorted((project / "src").rglob("*.py")) + sorted(
            (project / "tests").rglob("*.py")
        )
        _run(
            [str(pyrefly), "check", *map(str, python_files)],
            cwd=project,
            environment=environment,
        )
        _run(
            [str(pytest), str(project / "tests/test_adapter_contracts.py"), "-q"],
            cwd=consumer_root,
            environment=environment,
        )

        distribution = consumer_root / "dist"
        _run(
            [
                "uv",
                "build",
                "--project",
                str(project),
                "--wheel",
                "--out-dir",
                str(distribution),
            ],
            cwd=consumer_root,
            environment=environment,
        )
        wheels = list(distribution.glob("*.whl"))
        if len(wheels) != 1:
            raise RuntimeError("staged adapter build did not emit exactly one wheel")
        wheel = wheels[0]
        wheel_paths = _wheel_paths(wheel)
        expected_package_paths = {
            f"codefabric_cpg_mcp/{path}"
            for path in paths
            if path.endswith((".py", ".json"))
        }
        if not expected_package_paths <= wheel_paths:
            missing = sorted(expected_package_paths - wheel_paths)
            raise RuntimeError(
                f"staged adapter wheel omitted package outputs: {missing}"
            )

        virtual_environment = consumer_root / "venv"
        install_environment = environment.copy()
        install_environment.pop("PYTHONPATH", None)
        _run(
            [
                "uv",
                "venv",
                "--python",
                sys.executable,
                str(virtual_environment),
            ],
            cwd=consumer_root,
            environment=install_environment,
        )
        python = virtual_environment / "bin/python"
        _run(
            ["uv", "pip", "install", "--python", str(python), str(wheel)],
            cwd=consumer_root,
            environment=install_environment,
        )
        probe = """
from pathlib import Path
from codefabric_cpg_mcp.contracts.schemas import schema_fingerprints, schema_manifest
from codefabric_cpg_mcp.contracts.wire_models import MODEL_ADAPTERS, QueryCounts
import codefabric_cpg_mcp
origin = Path(codefabric_cpg_mcp.__file__).resolve()
assert 'venv' in origin.parts
assert MODEL_ADAPTERS['QueryCounts'].validate_python(
    {'fact_count': 1, 'result_count': 1, 'truncated': False}
) == QueryCounts(fact_count=1, result_count=1, truncated=False)
assert schema_manifest()['validation']
assert schema_fingerprints()['serialization']
print(origin)
"""
        _run(
            [str(python), "-c", probe],
            cwd=consumer_root,
            environment=install_environment,
        )

    print(f"validated staged adapter with {len(outputs)} packaged outputs")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("stage", type=Path)
    arguments = parser.parse_args()
    validate(arguments.stage.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
