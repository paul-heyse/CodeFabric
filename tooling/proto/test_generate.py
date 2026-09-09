"""Negative compatibility proofs for the released descriptor generator."""

from __future__ import annotations

import json
from copy import deepcopy

import pytest
from google.protobuf import descriptor_pb2

from tooling.proto.generate import (
    BASELINE,
    CENSUS_DESTINATION,
    COMPILER_SOURCES,
    CPGD_V2_HISTORY_CENSUS,
    CPGD_V2_HISTORY_DESCRIPTOR,
    EXACT_PYTHON_PACKAGES,
    HISTORY_INDEX_DESTINATION,
    PYTHON_DESTINATIONS,
    RUST_DESTINATIONS,
    UNRELEASED_PACKAGES,
    assert_compatible,
    assert_declared_descriptor_identities,
    assert_descriptor_profile,
    assert_exact_python_versions,
    descriptor_set,
    normalized_census,
    validate_history,
)


@pytest.fixture
def baseline() -> dict[str, object]:
    return json.loads(BASELINE.read_bytes())


@pytest.fixture
def current_census() -> dict[str, object]:
    return json.loads(CENSUS_DESTINATION.read_bytes())


def project_file(census: dict[str, object]) -> dict[str, object]:
    files = census["files"]
    assert isinstance(files, list)
    return next(file for file in files if file["package"] == "codefabric.cpgd.v2")


def message(census: dict[str, object], name: str) -> dict[str, object]:
    messages = project_file(census)["messages"]
    assert isinstance(messages, list)
    return next(item for item in messages if item["full_name"] == name)


def test_reviewed_baseline_is_self_compatible(baseline: dict[str, object]) -> None:
    assert_compatible(baseline, deepcopy(baseline))


def test_unreleased_v2_is_history_not_compatibility_authority(
    baseline: dict[str, object], current_census: dict[str, object]
) -> None:
    baseline_packages = {file["package"] for file in baseline["files"]}
    assert not baseline_packages.intersection(UNRELEASED_PACKAGES)
    assert {file["package"] for file in current_census["files"]}.intersection(
        UNRELEASED_PACKAGES
    ) == {"codefabric.cpgd.v2"}

    validate_history()
    index = json.loads(HISTORY_INDEX_DESTINATION.read_bytes())
    entry = index["entries"][0]
    assert entry["status"] == "unreleased-displaced-non-live"
    assert entry["live_runtime"] is False
    assert entry["compatibility_baseline"] is False
    historical = descriptor_set(CPGD_V2_HISTORY_DESCRIPTOR)
    assert normalized_census(historical) == json.loads(
        CPGD_V2_HISTORY_CENSUS.read_bytes()
    )
    current = descriptor_set(CENSUS_DESTINATION.parent / "production-descriptor.pb")
    historical_cpg = next(
        file for file in historical.file if file.package == "codefabric.cpgd.v2"
    )
    current_cpg = next(
        file for file in current.file if file.package == "codefabric.cpgd.v2"
    )
    assert historical_cpg.SerializeToString(
        deterministic=True
    ) != current_cpg.SerializeToString(deterministic=True)


def test_v2_declared_identity_is_the_exact_descriptor_projection() -> None:
    assert_declared_descriptor_identities(
        CENSUS_DESTINATION.parent / "production-descriptor.pb"
    )


def test_descriptor_census_covers_every_released_source() -> None:
    census = json.loads(CENSUS_DESTINATION.read_bytes())
    names = {file["name"] for file in census["files"]}
    expected = {relative.as_posix() for relative, _ in COMPILER_SOURCES}

    assert expected <= names


def test_runtime_binding_inventory_keeps_four_rust_families_and_one_python_service() -> (
    None
):
    assert set(RUST_DESTINATIONS) == {
        "codefabric.cpgd.v2.rs",
        "codefabric.provider.v1.rs",
        "codefabric.pyrefly.v1.rs",
        "codefabric.rustc.v1.rs",
    }
    assert set(PYTHON_DESTINATIONS) == {
        "cpg_query_service_pb2.py",
        "cpg_query_service_pb2.pyi",
        "cpg_query_service_pb2_grpc.py",
    }


def test_descriptor_census_covers_four_production_packages_and_well_known_dependency() -> (
    None
):
    census = json.loads(CENSUS_DESTINATION.read_bytes())
    packages = {file["package"] for file in census["files"]}
    assert packages == {
        "codefabric.cpgd.v2",
        "codefabric.provider.v1",
        "codefabric.pyrefly.v1",
        "codefabric.rustc.v1",
        "google.protobuf",
    }
    cpg = next(
        file for file in census["files"] if file["package"] == "codefabric.cpgd.v2"
    )
    service = next(
        item
        for item in cpg["services"]
        if item["full_name"] == "codefabric.cpgd.v2.CpgQueryService"
    )
    assert [method["name"] for method in service["methods"]] == sorted(
        [
            "CancelQuery",
            "GetReference",
            "GetStatus",
            "Handshake",
            "ReadResource",
            "ReadProcessingRemainder",
            "ReleaseResource",
            "StartQuery",
            "ValidateQuery",
            "WatchQuery",
        ]
    )


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
    current_census: dict[str, object], mutation: str
) -> None:
    baseline = deepcopy(current_census)
    current = deepcopy(current_census)
    envelope = message(current, "codefabric.cpgd.v2.StartQueryRequest")
    fields = envelope["fields"]
    assert isinstance(fields, list)
    if mutation == "field_number_reuse":
        fields[0]["name"] = "replacement_payload"
    elif mutation == "removal_without_reservation":
        del fields[0]
    elif mutation == "presence_drift":
        initial = next(field for field in fields if field["name"] == "initial")
        initial["has_presence"] = False
    elif mutation == "oneof_drift":
        initial = next(field for field in fields if field["name"] == "initial")
        initial["oneof"] = None
    elif mutation == "cardinality_drift":
        project_file(current)["services"][0]["methods"][0]["client_streaming"] = True
    elif mutation == "enum_number_drift":
        project_file(current)["enums"][0]["values"][1]["number"] = 3
    else:
        project_file(current)["syntax"] = "proto2"

    with pytest.raises(RuntimeError):
        assert_compatible(baseline, current)


def test_removed_field_requires_both_name_and_number_reservation(
    current_census: dict[str, object],
) -> None:
    baseline = deepcopy(current_census)
    current = deepcopy(baseline)
    envelope = message(current, "codefabric.cpgd.v2.StartQueryRequest")
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
