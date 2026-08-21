"""Negative compatibility proofs for the descriptor-set compiler."""

from __future__ import annotations

import json
from copy import deepcopy

import pytest
from google.protobuf import descriptor_pb2

from tooling.proto.generate import (
    BASELINE,
    CENSUS_DESTINATION,
    COMPILER_SOURCES,
    EXACT_PYTHON_PACKAGES,
    assert_compatible,
    assert_descriptor_profile,
    assert_exact_python_versions,
)


@pytest.fixture
def baseline() -> dict[str, object]:
    return json.loads(BASELINE.read_bytes())


def project_file(census: dict[str, object]) -> dict[str, object]:
    files = census["files"]
    assert isinstance(files, list)
    return next(file for file in files if file["package"] == "codefabric.cpgd.v1")


def message(census: dict[str, object], name: str) -> dict[str, object]:
    messages = project_file(census)["messages"]
    assert isinstance(messages, list)
    return next(item for item in messages if item["full_name"] == name)


def test_reviewed_baseline_is_self_compatible(baseline: dict[str, object]) -> None:
    assert_compatible(baseline, deepcopy(baseline))


def test_descriptor_census_covers_every_compiler_source() -> None:
    census = json.loads(CENSUS_DESTINATION.read_bytes())
    names = {file["name"] for file in census["files"]}
    expected = {relative.as_posix() for relative, _ in COMPILER_SOURCES}

    assert expected <= names


def test_wp10_structural_acceptance_covers_four_production_packages() -> None:
    census = json.loads(CENSUS_DESTINATION.read_bytes())
    packages = {file["package"] for file in census["files"]}
    assert packages == {
        "codefabric.cpgd.v1",
        "codefabric.provider.v1",
        "codefabric.pyrefly.v1",
        "codefabric.rustc.v1",
    }
    cpg = next(
        file for file in census["files"] if file["package"] == "codefabric.cpgd.v1"
    )
    service = next(
        item
        for item in cpg["services"]
        if item["full_name"] == "codefabric.cpgd.v1.CpgQueryService"
    )
    assert [method["name"] for method in service["methods"]] == sorted(
        [
            "AttachQuery",
            "CancelQuery",
            "GetStatus",
            "Handshake",
            "ReadResult",
            "ReleaseResult",
            "StartQuery",
            "StreamQuery",
            "ValidateQuery",
        ]
    )
    event = message(census, "codefabric.cpgd.v1.QueryEvent")
    assert [field["name"] for field in event["fields"]] == [
        "snapshot_pinned",
        "progress",
        "response_chunk",
        "artifact_ready",
        "terminal",
    ]


@pytest.mark.parametrize(
    "mutation",
    [
        "field_number_reuse",
        "removal_without_reservation",
        "presence_drift",
        "oneof_drift",
        "cardinality_drift",
        "enum_number_drift",
        "unknown_required_feature",
    ],
)
def test_incompatible_descriptor_changes_fail(
    baseline: dict[str, object], mutation: str
) -> None:
    current = deepcopy(baseline)
    envelope = message(current, "codefabric.cpgd.v1.StartQueryRequest")
    fields = envelope["fields"]
    assert isinstance(fields, list)
    if mutation == "field_number_reuse":
        fields[0]["name"] = "replacement_payload"
    elif mutation == "removal_without_reservation":
        del fields[0]
    elif mutation == "presence_drift":
        semantic_request_id = next(
            field for field in fields if field["name"] == "semantic_request_id"
        )
        semantic_request_id["proto3_optional"] = False
        semantic_request_id["has_presence"] = False
        semantic_request_id["oneof"] = None
    elif mutation == "oneof_drift":
        semantic_request_id = next(
            field for field in fields if field["name"] == "semantic_request_id"
        )
        semantic_request_id["oneof"] = None
    elif mutation == "cardinality_drift":
        method = project_file(current)["services"][0]["methods"][0]
        method["client_streaming"] = True
    elif mutation == "enum_number_drift":
        value = project_file(current)["enums"][0]["values"][1]
        value["number"] = 3
    else:
        project_file(current)["syntax"] = "proto2"

    with pytest.raises(RuntimeError):
        assert_compatible(baseline, current)


def test_removed_field_requires_both_name_and_number_reservation(
    baseline: dict[str, object],
) -> None:
    current = deepcopy(baseline)
    envelope = message(current, "codefabric.cpgd.v1.StartQueryRequest")
    fields = envelope["fields"]
    assert isinstance(fields, list)
    payload = fields.pop(0)
    envelope["reserved_names"].append(payload["name"])
    envelope["reserved_ranges"].append(
        {"start": payload["number"], "end_exclusive": payload["number"] + 1}
    )

    assert_compatible(baseline, current)


def test_compiler_runtime_mismatch_fails_before_generation() -> None:
    mismatched = dict(EXACT_PYTHON_PACKAGES)
    mismatched["protobuf"] = "7.35.0"

    with pytest.raises(RuntimeError, match="version mismatch"):
        assert_exact_python_versions(mismatched)


def test_semantic_descriptor_rejects_source_info_and_incomplete_imports() -> None:
    descriptors = descriptor_pb2.FileDescriptorSet(
        file=[
            descriptor_pb2.FileDescriptorProto(
                name="example.proto",
                dependency=["missing.proto"],
                source_code_info=descriptor_pb2.SourceCodeInfo(
                    location=[descriptor_pb2.SourceCodeInfo.Location(path=[1])]
                ),
            )
        ]
    )

    with pytest.raises(RuntimeError):
        assert_descriptor_profile(descriptors)
