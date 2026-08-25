"""Independent model-driver tests for the production Proto compilation unit."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest
from google.protobuf import descriptor_pb2, descriptor_pool, message_factory

from tooling.model.proto_driver import (
    apply_wire_enums,
    compile_once,
    output_plan,
    source_model,
)


def proto_source(
    path: str,
    package: str,
    *,
    body: str = "message Example { optional string value = 1; }",
) -> dict[str, str]:
    from blake3 import blake3

    contents = (
        "// artifact_id: codefabric.rpc.example\n"
        "// artifact_kind: protobuf-schema\n"
        "// version: 1.0\n"
        "// compatible_suite_major: 1\n"
        "// status: released\n"
        f"// canonical_digest: b3:{'1' * 64}\n"
        "// digest_projection: proto-descriptor-v1\n"
        'syntax = "proto3";\n'
        f"package {package};\n"
        f"{body}\n"
    )
    return {
        "path": path,
        "contents": contents,
        "source_digest": f"b3:{blake3(contents.encode()).hexdigest()}",
    }


def request(*sources: dict[str, str]) -> dict[str, object]:
    return {"sources": list(sources)}


def test_wp56_negative_zero_state() -> None:
    from blake3 import blake3

    path = "contracts/rpc/cpg_query_service.proto"
    contents = Path(path).read_text(encoding="utf-8")
    source = {
        "path": path,
        "contents": contents,
        "source_digest": f"b3:{blake3(contents.encode()).hexdigest()}",
    }
    projection = {
        "registry_domain": "QUERY_EXECUTION_STATE",
        "proto_path": path,
        "enum_name": "QueryExecutionState",
        "values": [
            {"number": number, "name": name}
            for number, name in enumerate(
                (
                    "QUERY_EXECUTION_STATE_UNSPECIFIED",
                    "QUERY_EXECUTION_STATE_ACCEPTED",
                    "QUERY_EXECUTION_STATE_WAITING_FOR_FRESHNESS",
                    "QUERY_EXECUTION_STATE_RUNNING",
                    "QUERY_EXECUTION_STATE_SUCCEEDED",
                    "QUERY_EXECUTION_STATE_FAILED",
                    "QUERY_EXECUTION_STATE_CANCELLED",
                    "QUERY_EXECUTION_STATE_LOST",
                )
            )
        ],
    }
    assert apply_wire_enums(source_model(request(source)), [projection])
    mutated = deepcopy(projection)
    mutated["values"][-1]["number"] = 8
    with pytest.raises(RuntimeError, match="diverge"):
        apply_wire_enums(source_model(request(source)), [mutated])


def test_model_proto_plan_is_source_and_package_derived() -> None:
    sources = source_model(
        request(proto_source("contracts/rpc/example.proto", "codefabric.example.v1"))
    )
    outputs = output_plan(sources)

    assert {output["path"] for output in outputs} >= {
        "tooling/proto/production-descriptor.pb",
        "src/generated/codefabric.example.v1.rs",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2.py",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2.pyi",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2_grpc.py",
    }


@pytest.mark.parametrize(
    "mutation",
    [
        "import_escape",
        "missing_import",
        "package_collision",
        "module_collision",
        "unknown_field",
    ],
)
def test_model_proto_rejects_unclosed_units(mutation: str) -> None:
    first = proto_source("contracts/rpc/one.proto", "codefabric.one.v1")
    second = proto_source("contracts/rpc/two.proto", "codefabric.two.v1")
    if mutation == "import_escape":
        first = proto_source(
            "contracts/rpc/one.proto",
            "codefabric.one.v1",
            body='import "../outside.proto"; message Example {}',
        )
    elif mutation == "missing_import":
        first = proto_source(
            "contracts/rpc/one.proto",
            "codefabric.one.v1",
            body='import "contracts/rpc/missing.proto"; message Example {}',
        )
    elif mutation == "package_collision":
        second = proto_source("contracts/rpc/two.proto", "codefabric.one.v1")
    elif mutation == "module_collision":
        second = proto_source("contracts/rpc/one.proto", "codefabric.two.v1")
    else:
        first["unexpected"] = "not allowed"

    with pytest.raises(RuntimeError):
        source_model(request(first, second))


def test_model_proto_one_fds_drives_python_and_rust_equivalently() -> None:
    sources = source_model(
        request(proto_source("contracts/rpc/example.proto", "codefabric.example.v1"))
    )
    descriptor, census, outputs = compile_once(sources, enforce_baseline=False)
    descriptor_set = descriptor_pb2.FileDescriptorSet.FromString(descriptor)

    assert [file.package for file in descriptor_set.file] == ["codefabric.example.v1"]
    assert b'"full_name": "codefabric.example.v1.Example"' in census
    assert set(outputs) == {
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2.py",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2.pyi",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/example_pb2_grpc.py",
    }


def test_model_proto_semantic_mutation_changes_descriptor() -> None:
    first = proto_source("contracts/rpc/example.proto", "codefabric.example.v1")
    second = deepcopy(first)
    second["contents"] = second["contents"].replace("value = 1", "other = 1")
    from blake3 import blake3

    second["source_digest"] = f"b3:{blake3(second['contents'].encode()).hexdigest()}"
    first_descriptor, _, _ = compile_once(
        source_model(request(first)), enforce_baseline=False
    )
    second_descriptor, _, _ = compile_once(
        source_model(request(second)), enforce_baseline=False
    )

    assert first_descriptor != second_descriptor


def test_model_proto_descriptor_census_covers_all_semantic_compatibility_dimensions() -> (
    None
):
    import json
    from pathlib import Path

    census = json.loads(Path("tooling/proto/descriptor-census.json").read_bytes())
    assert census["schema"] == 1
    assert census["files"]
    for file in census["files"]:
        assert {
            "name",
            "package",
            "syntax",
            "edition",
            "dependencies",
            "public_dependencies",
            "weak_dependencies",
            "options",
            "messages",
            "enums",
            "services",
        } <= file.keys()
        for message in file["messages"]:
            assert {
                "fields",
                "oneofs",
                "reserved_names",
                "reserved_ranges",
                "options",
            } <= message.keys()
            for field in message["fields"]:
                assert {
                    "number",
                    "label",
                    "type",
                    "type_name",
                    "oneof",
                    "proto3_optional",
                    "has_presence",
                    "options",
                } <= field.keys()
        for service in file["services"]:
            for method in service["methods"]:
                assert {
                    "input_type",
                    "output_type",
                    "client_streaming",
                    "server_streaming",
                    "options",
                } <= method.keys()


def test_model_proto_cross_language_round_trip_preserves_presence_oneofs_and_unknowns() -> (
    None
):
    from pathlib import Path

    descriptor_set = descriptor_pb2.FileDescriptorSet.FromString(
        Path("tooling/proto/production-descriptor.pb").read_bytes()
    )
    pool = descriptor_pool.DescriptorPool()
    remaining = {file.name: file for file in descriptor_set.file}
    while remaining:
        for name, file in list(remaining.items()):
            if all(dependency not in remaining for dependency in file.dependency):
                pool.AddSerializedFile(file.SerializeToString())
                del remaining[name]
                break
        else:  # pragma: no cover - an explicit failure branch
            raise AssertionError("cyclic descriptor import graph")
    message_type = message_factory.GetMessageClass(
        pool.FindMessageTypeByName("codefabric.cpgd.v1.StartQueryRequest")
    )
    message = message_type(workspace_id="ws:test", semantic_request_id="request:test")
    assert message.HasField("semantic_request_id")
    unknown_field_99_varint_42 = b"\x98\x06\x2a"
    wire = message.SerializeToString(deterministic=True) + unknown_field_99_varint_42
    decoded = message_type.FromString(wire)
    assert decoded.HasField("semantic_request_id")
    assert decoded.SerializeToString(deterministic=True).endswith(
        unknown_field_99_varint_42
    )
