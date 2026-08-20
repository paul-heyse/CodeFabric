"""Generate and verify the committed Wave 0 Rust/Python Protobuf outputs."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = ROOT / "tooling" / "proto" / "source"
SOURCE_RELATIVE = Path("codefabric_cpg_mcp/daemon/generated/wave0_probe.proto")
SOURCE = SOURCE_ROOT / SOURCE_RELATIVE
RUST_DESTINATION = ROOT / "src" / "generated"
PYTHON_DESTINATION = (
    ROOT / "codefabric-cpg-mcp" / "src" / "codefabric_cpg_mcp" / "daemon" / "generated"
)
IDENTITY_DESTINATION = ROOT / "tooling" / "proto" / "toolchain-identity.json"
RUST_OUTPUT = "codefabric.wave0.v1.rs"
PYTHON_OUTPUTS = (
    "wave0_probe_pb2.py",
    "wave0_probe_pb2.pyi",
    "wave0_probe_pb2_grpc.py",
)


def clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.pop("VIRTUAL_ENV", None)
    environment.pop("UV_PROJECT_ENVIRONMENT", None)
    return environment


def run(command: list[str], *, capture: bool = False) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=clean_environment(),
        check=True,
        capture_output=capture,
        text=True,
    )
    return completed.stderr.strip() if capture else ""


def source_digest() -> str:
    return hashlib.sha256(SOURCE.read_bytes()).hexdigest()


def generated_header(comment: str) -> bytes:
    return (
        f"{comment} @generated from {SOURCE_RELATIVE.as_posix()} "
        f"sha256:{source_digest()}; do not edit.\n"
    ).encode()


def prepend_header(path: Path, comment: str) -> None:
    body = path.read_bytes()
    path.write_bytes(generated_header(comment) + body)


def cargo_package_versions() -> dict[str, str]:
    completed = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--features",
            "proto-tooling",
        ],
        cwd=ROOT,
        env=clean_environment(),
        check=True,
        capture_output=True,
        text=True,
    )
    packages = json.loads(completed.stdout)["packages"]
    wanted = {
        "prost",
        "prost-build",
        "protoc-bin-vendored",
        "tonic",
        "tonic-build",
        "tonic-prost",
        "tonic-prost-build",
    }
    resolved = {
        package["name"]: package["version"]
        for package in packages
        if package["name"] in wanted
    }
    missing = wanted - resolved.keys()
    if missing:
        raise RuntimeError(f"missing generator packages: {sorted(missing)}")
    return dict(sorted(resolved.items()))


def generate_into(root: Path) -> dict[str, Path]:
    rust_output = root / "rust"
    python_output = root / "python"
    rust_output.mkdir(parents=True)
    python_output.mkdir(parents=True)

    run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--features",
            "proto-tooling",
            "--bin",
            "codefabric-proto-gen",
            "--",
            "--rust-out",
            str(rust_output),
        ]
    )
    run(
        [
            sys.executable,
            "-m",
            "grpc_tools.protoc",
            f"-I{SOURCE_ROOT}",
            f"--python_out={python_output}",
            f"--pyi_out={python_output}",
            f"--grpc_python_out={python_output}",
            SOURCE_RELATIVE.as_posix(),
        ]
    )

    rust_file = rust_output / RUST_OUTPUT
    python_package = python_output / SOURCE_RELATIVE.parent
    files = {"rust": rust_file}
    for output in PYTHON_OUTPUTS:
        files[f"python/{output}"] = python_package / output

    prepend_header(rust_file, "//")
    for key, path in files.items():
        if key.startswith("python/"):
            prepend_header(path, "#")

    run(["rustfmt", "--edition", "2024", str(rust_file)])
    run(["rustfmt", "--edition", "2024", "--check", str(rust_file)])
    return files


def output_digest(files: dict[str, Path]) -> str:
    digest = hashlib.sha256()
    for name, path in sorted(files.items()):
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def identity(files: dict[str, Path]) -> dict[str, Any]:
    protoc_version = run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--features",
            "proto-tooling",
            "--bin",
            "codefabric-proto-gen",
            "--",
            "--protoc-version",
        ],
        capture=True,
    )
    python_protoc = subprocess.run(
        [sys.executable, "-m", "grpc_tools.protoc", "--version"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return {
        "schema": 1,
        "source": SOURCE_RELATIVE.as_posix(),
        "source_sha256": source_digest(),
        "generated_sha256": output_digest(files),
        "rust": {
            "packages": cargo_package_versions(),
            "protoc": protoc_version,
            "toolchain_policy": "stable root; declared MSRV 1.94.1",
        },
        "python": {
            "grpcio-tools": importlib.metadata.version("grpcio-tools"),
            "protobuf": importlib.metadata.version("protobuf"),
            "protoc": python_protoc,
            "runtime": sys.version.split()[0],
        },
    }


def destination_files() -> dict[str, Path]:
    files = {"rust": RUST_DESTINATION / RUST_OUTPUT}
    files.update(
        {f"python/{name}": PYTHON_DESTINATION / name for name in PYTHON_OUTPUTS}
    )
    return files


def encoded_identity(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def assert_equal(expected: dict[str, Path], actual: dict[str, Path]) -> None:
    for name in sorted(expected):
        if not actual[name].is_file():
            raise RuntimeError(f"missing committed generated output: {actual[name]}")
        if expected[name].read_bytes() != actual[name].read_bytes():
            raise RuntimeError(f"generated output drift: {name}")


def write_outputs(files: dict[str, Path], identity_value: dict[str, Any]) -> None:
    RUST_DESTINATION.mkdir(parents=True, exist_ok=True)
    PYTHON_DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, source in files.items():
        destination = destination_files()[name]
        shutil.copyfile(source, destination)
    IDENTITY_DESTINATION.write_bytes(encoded_identity(identity_value))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("write", "check", "repro-check"))
    arguments = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="codefabric-proto-a-") as first_raw:
        first = Path(first_raw)
        first_files = generate_into(first)
        first_identity = identity(first_files)

        if arguments.mode == "write":
            write_outputs(first_files, first_identity)
            print(first_identity["generated_sha256"])
            return 0

        assert_equal(first_files, destination_files())
        if IDENTITY_DESTINATION.read_bytes() != encoded_identity(first_identity):
            raise RuntimeError("generator identity drift")

        if arguments.mode == "repro-check":
            with tempfile.TemporaryDirectory(
                prefix="codefabric-proto-b-"
            ) as second_raw:
                second_files = generate_into(Path(second_raw))
                if output_digest(first_files) != output_digest(second_files):
                    raise RuntimeError(
                        "isolated Protobuf generations were not byte-identical"
                    )

        print(first_identity["generated_sha256"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
