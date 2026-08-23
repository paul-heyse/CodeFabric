"""Validate staged Proto outputs through descriptor, Python, Rust, and parity consumers."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from google.protobuf import descriptor_pb2, descriptor_pool

ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    stage = Path(sys.argv[1]).resolve()
    descriptor_path = stage / "tooling/proto/production-descriptor.pb"
    census_path = stage / "tooling/proto/descriptor-census.json"
    descriptors = descriptor_pb2.FileDescriptorSet.FromString(
        descriptor_path.read_bytes()
    )
    census = json.loads(census_path.read_bytes())
    names = [file.name for file in descriptors.file]
    packages = [file.package for file in descriptors.file]
    assert len(names) == len(set(names)) == len(census["files"])
    assert len(packages) == len(set(packages))

    pool = descriptor_pool.DescriptorPool()
    remaining = {file.name: file for file in descriptors.file}
    while remaining:
        progressed = False
        for name, file in list(remaining.items()):
            if all(dependency not in remaining for dependency in file.dependency):
                pool.AddSerializedFile(file.SerializeToString())
                del remaining[name]
                progressed = True
        assert progressed, "descriptor import graph is cyclic or incomplete"

    for file in descriptors.file:
        assert pool.FindFileByName(file.name).package == file.package
        stem = Path(file.name).stem
        for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
            staged = (
                stage
                / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated"
                / f"{stem}{suffix}"
            )
            committed = (
                ROOT
                / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated"
                / f"{stem}{suffix}"
            )
            assert staged.read_bytes() == committed.read_bytes()
        staged_rust = stage / "src/generated" / f"{file.package}.rs"
        committed_rust = ROOT / "src/generated" / f"{file.package}.rs"
        assert staged_rust.read_bytes() == committed_rust.read_bytes()

    assert (
        descriptor_path.read_bytes()
        == (ROOT / "tooling/proto/production-descriptor.pb").read_bytes()
    )
    assert (
        census_path.read_bytes()
        == (ROOT / "tooling/proto/descriptor-census.json").read_bytes()
    )
    print(f"validated {len(names)} staged Proto sources from one FDS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
