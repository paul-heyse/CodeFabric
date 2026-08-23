"""Closed external driver for one model-derived production Proto compilation unit."""

from __future__ import annotations

import hashlib
import importlib.metadata
import importlib.resources
import json
import re
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Never

from blake3 import blake3
from grpc_tools import protoc

from tooling.proto.generate import (
    BASELINE,
    GRPC_TOOLS_PROTOC,
    assert_compatible,
    assert_descriptor_profile,
    assert_exact_python_versions,
    descriptor_set,
    encoded_json,
    normalized_census,
)

ROOT = Path(__file__).resolve().parents[2]
PROTOCOL_VERSION = "codefabric-external-proto-driver-v1"
PYTHON_OUTPUT_ROOT = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated"
RUST_OUTPUT_ROOT = "src/generated"
MAX_SOURCE_COUNT = 64
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_OUTPUT_BYTES = 64 * 1024 * 1024


def fail(message: str) -> Never:
    raise RuntimeError(message)


def exact_object(value: Any, keys: set[str], context: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{context} must contain exactly {sorted(keys)}")
    return value


def digest_bytes(value: bytes) -> str:
    return f"b3:{blake3(value).hexdigest()}"


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def source_model(request: dict[str, Any]) -> list[dict[str, str]]:
    raw_sources = request["sources"]
    if (
        not isinstance(raw_sources, list)
        or not 0 < len(raw_sources) <= MAX_SOURCE_COUNT
    ):
        fail("Proto source set is empty or exceeds its declared bound")
    sources: list[dict[str, str]] = []
    paths: set[str] = set()
    packages: set[str] = set()
    stems: set[str] = set()
    total = 0
    for index, raw in enumerate(raw_sources):
        source = exact_object(
            raw, {"path", "contents", "source_digest"}, f"source[{index}]"
        )
        path = source["path"]
        contents = source["contents"]
        claimed_digest = source["source_digest"]
        if not all(isinstance(item, str) for item in (path, contents, claimed_digest)):
            fail("Proto source fields must be strings")
        pure = PurePosixPath(path)
        if (
            pure.is_absolute()
            or ".." in pure.parts
            or len(pure.parts) != 3
            or pure.parts[:2] != ("contracts", "rpc")
            or pure.suffix != ".proto"
            or "\\" in path
            or path in paths
        ):
            fail(f"unsafe or duplicate Proto source path: {path}")
        encoded = contents.encode("utf-8")
        total += len(encoded)
        if total > MAX_SOURCE_BYTES or digest_bytes(encoded) != claimed_digest:
            fail(f"Proto source digest or resource bound failed: {path}")
        package_matches = re.findall(
            r"(?m)^\s*package\s+([A-Za-z_][A-Za-z0-9_.]*)\s*;", contents
        )
        if len(package_matches) != 1 or package_matches[0] in packages:
            fail(f"Proto package is absent, repeated, or colliding: {path}")
        semantic_matches = re.findall(
            r"(?m)^//\s*canonical_digest:\s*(b3:[0-9a-f]{64})\s*$", contents
        )
        if len(semantic_matches) != 1 or pure.stem in stems:
            fail(f"Proto semantic identity or generated module name collides: {path}")
        paths.add(path)
        packages.add(package_matches[0])
        stems.add(pure.stem)
        sources.append(
            {
                "path": path,
                "contents": contents,
                "source_digest": claimed_digest,
                "canonical_digest": semantic_matches[0],
                "package": package_matches[0],
                "stem": pure.stem,
            }
        )
    sources.sort(key=lambda item: item["path"])
    for source in sources:
        imports = re.findall(
            r'(?m)^\s*import(?:\s+(?:public|weak))?\s+"([^"]+)"\s*;', source["contents"]
        )
        for imported in imports:
            pure = PurePosixPath(imported)
            if (
                pure.is_absolute()
                or ".." in pure.parts
                or "\\" in imported
                or imported not in paths
            ):
                fail(
                    f"Proto import escapes or is missing from the unit: {source['path']} -> {imported}"
                )
    return sources


def output_plan(sources: list[dict[str, str]]) -> list[dict[str, str]]:
    outputs = [
        {
            "output_id": "output:proto-descriptor-set",
            "path": "tooling/proto/production-descriptor.pb",
            "role": "proto-descriptor",
        },
        {
            "output_id": "output:proto-descriptor-census",
            "path": "tooling/proto/descriptor-census.json",
            "role": "descriptor-census",
        },
        {
            "output_id": "output:proto-toolchain-identity",
            "path": "tooling/proto/toolchain-identity.json",
            "role": "toolchain-identity",
        },
    ]
    for source in sources:
        stem = source["stem"]
        for suffix, kind in (
            ("_pb2.py", "module"),
            ("_pb2.pyi", "stub"),
            ("_pb2_grpc.py", "grpc"),
        ):
            outputs.append(
                {
                    "output_id": f"output:proto-python:{stem}:{kind}",
                    "path": f"{PYTHON_OUTPUT_ROOT}/{stem}{suffix}",
                    "role": "python-binding",
                }
            )
        outputs.append(
            {
                "output_id": f"output:proto-rust:{source['package']}",
                "path": f"{RUST_OUTPUT_ROOT}/{source['package']}.rs",
                "role": "rust-binding",
            }
        )
    return sorted(outputs, key=lambda item: (item["path"], item["output_id"]))


def tool_identity() -> dict[str, Any]:
    versions = assert_exact_python_versions()
    executable = Path(sys.executable).resolve()
    return {
        "python_path": str(executable),
        "python_digest": digest_file(executable),
        "python_version": sys.version.split()[0],
        "script_digest": digest_file(Path(__file__).resolve()),
        "lock_digest": digest_file(ROOT / "codefabric-cpg-mcp" / "uv.lock"),
        "project_digest": digest_file(ROOT / "codefabric-cpg-mcp" / "pyproject.toml"),
        **versions,
        "protoc": GRPC_TOOLS_PROTOC,
    }


def normalize_imports(contents: str) -> str:
    return re.sub(
        r"(?m)^from\s+[A-Za-z_][A-Za-z0-9_.]*\s+import\s+([A-Za-z_][A-Za-z0-9_]*_pb2)\s+as\s+",
        r"from . import \1 as ",
        contents,
    )


def generated_header(sources: list[dict[str, str]], comment: str) -> str:
    identities = ",".join(source["canonical_digest"] for source in sources)
    return (
        f"{comment} @generated from catalog primary semantic identity "
        f"{identities}; do not edit.\n"
    )


def compile_once(
    sources: list[dict[str, str]],
    *,
    enforce_baseline: bool = True,
) -> tuple[bytes, bytes, dict[str, bytes]]:
    with tempfile.TemporaryDirectory(prefix="codefabric-model-proto-") as raw:
        root = Path(raw)
        python_out = root / "python"
        python_out.mkdir()
        for source in sources:
            destination = root / source["path"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(source["contents"], encoding="utf-8")
        descriptor = root / "production-descriptor.pb"
        bundled_include = importlib.resources.files("grpc_tools").joinpath("_proto")
        arguments = [
            "grpc_tools.protoc",
            f"-I{root}",
            f"-I{bundled_include}",
            f"--python_out={python_out}",
            f"--pyi_out={python_out}",
            f"--grpc_python_out={python_out}",
            f"--descriptor_set_out={descriptor}",
            "--include_imports",
            *(source["path"] for source in sources),
        ]
        if protoc.main(arguments) != 0:
            fail("grpc_tools.protoc failed")
        descriptors = descriptor_set(descriptor)
        assert_descriptor_profile(descriptors)
        census = normalized_census(descriptors)
        if enforce_baseline:
            assert_compatible(json.loads(BASELINE.read_bytes()), census)
        rendered: dict[str, bytes] = {}
        header = generated_header(sources, "#")
        for source in sources:
            stem = source["stem"]
            for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
                matches = list(python_out.rglob(f"{stem}{suffix}"))
                if len(matches) != 1:
                    fail(f"compiler output is absent or ambiguous: {stem}{suffix}")
                contents = normalize_imports(matches[0].read_text(encoding="utf-8"))
                rendered[f"{PYTHON_OUTPUT_ROOT}/{stem}{suffix}"] = (
                    header + contents
                ).encode()
        descriptor_bytes = descriptor.read_bytes()
        census_bytes = encoded_json(census)
        if (
            sum(map(len, rendered.values())) + len(descriptor_bytes) + len(census_bytes)
            > MAX_OUTPUT_BYTES
        ):
            fail("Proto driver output budget exceeded")
        return descriptor_bytes, census_bytes, rendered


def main() -> int:
    request = exact_object(
        json.load(sys.stdin),
        {"protocol_version", "operation", "sources", "planned_outputs"},
        "request",
    )
    if request["protocol_version"] != PROTOCOL_VERSION or request["operation"] not in {
        "plan",
        "render",
    }:
        fail("unsupported Proto driver protocol or operation")
    sources = source_model(request)
    planned = output_plan(sources)
    supplied = request["planned_outputs"]
    if request["operation"] == "plan":
        if supplied != []:
            fail("plan cannot accept caller-selected outputs")
        response = {
            "protocol_version": PROTOCOL_VERSION,
            "tool_identity": tool_identity(),
            "outputs": planned,
            "compiler_invocations": 0,
        }
    else:
        if supplied != planned:
            fail("render outputs differ from the model-derived plan")
        descriptor, census, python_outputs = compile_once(sources)
        rendered = [
            {
                "output_id": next(
                    item["output_id"] for item in planned if item["path"] == path
                ),
                "path": path,
                "role": "python-binding",
                "contents_hex": contents.hex(),
            }
            for path, contents in sorted(python_outputs.items())
        ]
        rendered.extend(
            [
                {
                    "output_id": "output:proto-descriptor-census",
                    "path": "tooling/proto/descriptor-census.json",
                    "role": "descriptor-census",
                    "contents_hex": census.hex(),
                },
                {
                    "output_id": "output:proto-descriptor-set",
                    "path": "tooling/proto/production-descriptor.pb",
                    "role": "proto-descriptor",
                    "contents_hex": descriptor.hex(),
                },
            ]
        )
        response = {
            "protocol_version": PROTOCOL_VERSION,
            "tool_identity": tool_identity(),
            "outputs": sorted(
                rendered, key=lambda item: (item["path"], item["output_id"])
            ),
            "compiler_invocations": 1,
            "descriptor_sha256": hashlib.sha256(descriptor).hexdigest(),
        }
    sys.stdout.write(json.dumps(response, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
