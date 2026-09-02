"""Clean cpgd.v2 descriptors, generated bindings, wire KATs, and UDS channel bounds."""

from __future__ import annotations

import importlib.metadata
import json
from collections.abc import Iterable
from pathlib import Path
from unittest.mock import patch

import pytest
from google.protobuf import descriptor_pb2, descriptor_pool
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message

from codefabric_cpg_mcp.daemon.channel import (
    GRPC_DEFAULT_AUTHORITY,
    GRPC_MESSAGE_OPTIONS,
    MAX_CONTROL_MESSAGE_BYTES,
    create_local_channel,
)
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as cpg
from codefabric_cpg_mcp.daemon.generated import provider_control_pb2 as provider
from codefabric_cpg_mcp.daemon.generated import pyrefly_sidecar_pb2 as pyrefly
from codefabric_cpg_mcp.daemon.generated import rustc_extractor_pb2 as rustc

ROOT = Path(__file__).resolve().parents[2]
DESCRIPTOR_PATH = ROOT / "tooling/proto/production-descriptor.pb"


def _clear_derived_json_names(messages: Iterable[descriptor_pb2.DescriptorProto]) -> None:
    for message in messages:
        for field in message.field:
            field.ClearField("json_name")
        _clear_derived_json_names(message.nested_type)


def _messages() -> dict[str, Message]:
    return {
        "cpg_start_query": cpg.StartQueryRequest(
            context=cpg.RequestContext(
                correlation_id="mcp:test",
                remaining_budget=Duration(seconds=30),
            ),
            initial=cpg.InitialQueryStart(
                query=cpg.QuerySubmission(
                    canonical_request_json=b'{"kind":"lookup"}',
                    request_checksum="b3:test",
                    semantic_request_id="semantic:test",
                    semantic_profile="codefabric.semantic-query.v2",
                    result_limits=cpg.ResultLimits(
                        maximum_result_bytes=1_048_576,
                        maximum_result_pages=64,
                    ),
                ),
            ),
        ),
        "provider_job": provider.ProviderJobSpec(
            provider_run_id="run:test",
            workspace_id="ws:test",
            analysis_context_id="context:source",
            source_generation=7,
            resource_profile_id="in-process-syntax-standard",
        ),
        "pyrefly_hello": pyrefly.Hello(protocol_major=1, maximum_arrow_ipc_bytes=1_048_576),
        "rustc_accepted": rustc.CompilationAccepted(
            provider_run_id="run:test",
            compilation_unit_id="unit:test",
            accepted_generation=7,
        ),
    }


def test_all_production_packages_match_independent_wire_kats() -> None:
    fixture = json.loads(
        (ROOT / "contracts/fixtures/proto/production_wire.json").read_text(encoding="utf-8")
    )
    expected = {case["name"]: case["wire_hex"] for case in fixture["cases"]}
    messages = _messages()
    assert messages.keys() == expected.keys()
    for name, message in messages.items():
        encoded = message.SerializeToString(deterministic=True)
        assert encoded.hex() == expected[name]
        assert type(message).FromString(encoded) == message


def test_generated_descriptors_match_the_one_committed_fds() -> None:
    descriptor_set = descriptor_pb2.FileDescriptorSet.FromString(DESCRIPTOR_PATH.read_bytes())
    files = {file.name: file for file in descriptor_set.file}
    modules = {
        "contracts/rpc/cpg_query_service.proto": cpg,
        "contracts/rpc/provider_control.proto": provider,
        "contracts/rpc/pyrefly_sidecar.proto": pyrefly,
        "contracts/rpc/rustc_extractor.proto": rustc,
    }
    assert set(files) == {*modules, "google/protobuf/duration.proto"}
    for name, module in modules.items():
        source = descriptor_pb2.FileDescriptorProto()
        source.CopyFrom(files[name])
        _clear_derived_json_names(source.message_type)
        generated = descriptor_pb2.FileDescriptorProto()
        module.DESCRIPTOR.CopyToProto(generated)
        assert generated == source

    pool = descriptor_pool.DescriptorPool()
    for file in descriptor_set.file:
        pool.AddSerializedFile(file.SerializeToString())
    service = pool.FindServiceByName("codefabric.cpgd.v2.CpgQueryService")
    assert tuple(method.name for method in service.methods) == (
        "Handshake",
        "GetStatus",
        "GetReference",
        "ValidateQuery",
        "StartQuery",
        "WatchQuery",
        "CancelQuery",
        "ReadResource",
        "ReleaseResource",
    )
    with pytest.raises(KeyError):
        pool.FindServiceByName("codefabric.cpgd.v1.CpgQueryService")


def test_v2_atomic_start_and_event_contracts_are_closed() -> None:
    service = cpg.DESCRIPTOR.services_by_name["CpgQueryService"]
    assert len(service.methods) == 9
    assert not {
        "StreamQuery",
        "AttachQuery",
        "ReadResult",
        "ReleaseResult",
        "ExecuteQuery",
    }.intersection(service.methods_by_name)
    event = cpg.QueryEvent.DESCRIPTOR.oneofs_by_name["event"]
    assert [field.name for field in event.fields] == [
        "snapshot_pinned",
        "progress",
        "result_ready",
        "terminal",
    ]
    start_leg = cpg.StartQueryRequest.DESCRIPTOR.oneofs_by_name["leg"]
    assert [(field.name, field.number) for field in start_leg.fields] == [
        ("initial", 2),
        ("continuation", 3),
    ]
    outcome = cpg.StartQueryResponse.DESCRIPTOR.oneofs_by_name["outcome"]
    assert [(field.name, field.number) for field in outcome.fields] == [
        ("accepted", 1),
        ("input_challenge", 2),
        ("validation_rejection", 3),
    ]
    assert cpg.QueryEventHeader.DESCRIPTOR.fields_by_name["cursor"].number == 5
    initial = cpg.InitialQueryStart.DESCRIPTOR
    assert "start_idempotency_id" not in initial.fields_by_name
    proto = descriptor_pb2.DescriptorProto()
    initial.CopyToProto(proto)
    assert list(proto.reserved_name) == ["start_idempotency_id"]
    assert [(item.start, item.end) for item in proto.reserved_range] == [(2, 3)]


def test_binary_round_trip_preserves_unknown_fields() -> None:
    known = cpg.StartQueryRequest(initial=cpg.InitialQueryStart()).SerializeToString()
    unknown_field_99_varint_42 = b"\x98\x06\x2a"
    older_reader = cpg.StartQueryRequest.FromString(known + unknown_field_99_varint_42)
    assert older_reader.SerializeToString(deterministic=True) == known + unknown_field_99_varint_42


def test_atomic_start_oneofs_replace_previous_variants() -> None:
    request = cpg.StartQueryRequest(initial=cpg.InitialQueryStart())
    assert request.WhichOneof("leg") == "initial"
    request.continuation.CopyFrom(
        cpg.QueryChallengeContinuation(
            daemon_continuation=b"opaque",
            challenge_id="challenge:test",
            round=2,
        )
    )
    assert request.WhichOneof("leg") == "continuation"
    assert not request.HasField("initial")

    response = cpg.StartQueryResponse(accepted=cpg.AcceptedQuery(daemon_query_id="query:test"))
    response.input_challenge.CopyFrom(
        cpg.InputChallenge(challenge_id="challenge:test", daemon_continuation=b"opaque")
    )
    assert response.WhichOneof("outcome") == "input_challenge"
    assert not response.HasField("accepted")


def test_independent_atomic_start_wire_fixtures_decode_exact_outcomes() -> None:
    fixture = json.loads(
        (ROOT / "contracts/fixtures/proto/cpgd_v2_fastmcp4_wire.json").read_text(encoding="utf-8")
    )
    assert fixture["package"] == "codefabric.cpgd.v2"
    assert fixture["message"] == "StartQueryResponse"
    assert {case["outcome"] for case in fixture["cases"]} == {
        "accepted",
        "input_challenge",
        "validation_rejection",
    }
    for case in fixture["cases"]:
        wire = bytes.fromhex(case["wire_hex"])
        response = cpg.StartQueryResponse.FromString(wire)
        assert response.WhichOneof("outcome") == case["outcome"]
        assert response.SerializeToString(deterministic=True) == wire


def test_challenge_contract_is_typed_bounded_and_contains_no_opaque_json() -> None:
    challenge_messages = (
        cpg.InputChallenge,
        cpg.InputRequirement,
        cpg.ChallengeConstraints,
        cpg.ChallengeStringConstraints,
        cpg.ChallengeIntegerConstraints,
        cpg.ChallengeEnumConstraints,
        cpg.ChallengeCollectionConstraints,
        cpg.AuthorizedChoice,
        cpg.QueryChallengeContinuation,
        cpg.InputAnswer,
    )
    challenge_fields = {
        field.name
        for message_type in challenge_messages
        for field in message_type.DESCRIPTOR.fields
    }
    assert not {name for name in challenge_fields if "json" in name or "any" in name}
    assert challenge_fields.intersection({"maximum_length", "maximum_items"}) == {
        "maximum_length",
        "maximum_items",
    }
    assert {
        "semantic_field_id",
        "input_kind",
        "presentation_key",
        "authorized_choices",
        "remaining_rounds",
        "maximum_answer_bytes",
        "daemon_continuation",
    } <= challenge_fields
    assert [
        field.name
        for field in cpg.ChallengeConstraints.DESCRIPTOR.oneofs_by_name["constraint"].fields
    ] == [
        "string_constraints",
        "integer_constraints",
        "enum_constraints",
        "collection_constraints",
    ]
    assert [field.name for field in cpg.InputAnswer.DESCRIPTOR.oneofs_by_name["value"].fields] == [
        "string_value",
        "integer_value",
        "boolean_value",
        "choice_id",
        "string_collection",
        "integer_collection",
        "boolean_collection",
        "choice_collection",
    ]


def test_reference_resource_cancel_and_error_contracts_are_authority_safe() -> None:
    reference_operation = cpg.GetReferenceRequest.DESCRIPTOR.oneofs_by_name["operation"]
    reference_result = cpg.GetReferenceResponse.DESCRIPTOR.oneofs_by_name["result"]
    assert [field.name for field in reference_operation.fields] == ["read", "completion"]
    assert [field.name for field in reference_result.fields] == ["reference", "completion"]
    reference_document = cpg.ReferenceDocument.DESCRIPTOR.fields_by_name
    assert set(reference_document) == {"reference_id", "resource"}
    reference_resource_type = reference_document["resource"].message_type
    assert reference_resource_type is not None
    assert reference_resource_type.full_name == "codefabric.cpgd.v2.ResourceDescriptor"
    assert not set(reference_document).intersection(
        {"content", "content_checksum", "media_type", "byte_length", "public_handle"}
    )
    assert set(cpg.ReferenceCompletion.DESCRIPTOR.fields_by_name) == {
        "candidates",
        "total",
        "has_more",
    }
    assert (
        cpg.ReferenceCompletionRequest.DESCRIPTOR.fields_by_name["maximum_candidates"].number == 5
    )

    resource_fields = set(cpg.ResourceDescriptor.DESCRIPTOR.fields_by_name)
    assert "public_handle" in resource_fields
    assert not resource_fields.intersection(
        {"lease", "lease_id", "lease_token", "path", "object_store_url"}
    )
    for request_type in (cpg.ReadResourceRequest, cpg.ReleaseResourceRequest):
        fields = set(request_type.DESCRIPTOR.fields_by_name)
        assert {"context", "public_handle"} <= fields
        assert not fields.intersection({"lease_token", "path"})

    assert set(cpg.ResourceKind.keys()) == {
        "RESOURCE_KIND_UNSPECIFIED",
        "RESOURCE_KIND_RESULT_MANIFEST",
        "RESOURCE_KIND_RESULT_PAGE",
        "RESOURCE_KIND_REFERENCE",
    }
    resource_kind_proto = descriptor_pb2.EnumDescriptorProto()
    cpg.ResourceKind.DESCRIPTOR.CopyToProto(resource_kind_proto)
    assert list(resource_kind_proto.reserved_name) == ["RESOURCE_KIND_PROJECTION"]
    assert [(item.start, item.end) for item in resource_kind_proto.reserved_range] == [(4, 4)]
    selector = cpg.ResourceSelector.DESCRIPTOR
    assert [field.name for field in selector.oneofs_by_name["selector"].fields] == [
        "manifest",
        "page",
        "reference",
    ]
    selector_proto = descriptor_pb2.DescriptorProto()
    selector.CopyToProto(selector_proto)
    assert list(selector_proto.reserved_name) == ["projection"]
    assert [(item.start, item.end) for item in selector_proto.reserved_range] == [(4, 5)]
    assert not hasattr(cpg, "ProjectionSelector")

    assert set(cpg.CancelQueryRequest.DESCRIPTOR.fields_by_name) == {
        "context",
        "daemon_query_id",
        "cancellation_id",
    }
    assert {
        "authority",
        "cancellation_id",
        "acknowledgement",
        "terminal",
        "idempotent_replay",
    } == set(cpg.CancelQueryResponse.DESCRIPTOR.fields_by_name)
    assert set(cpg.SafeErrorMetadata.DESCRIPTOR.fields_by_name) == {
        "code",
        "layer",
        "retryable",
        "retry_after_ms",
        "diagnostic_reference",
        "correlation_id",
    }


def test_relative_budget_generations_and_reserved_control_are_explicit() -> None:
    context = cpg.RequestContext.DESCRIPTOR.fields_by_name
    assert set(context) == {"correlation_id", "remaining_budget"}
    context_budget_type = context["remaining_budget"].message_type
    assert context_budget_type is not None
    assert context_budget_type.full_name == "google.protobuf.Duration"
    assert not set(context).intersection(
        {
            "deadline",
            "deadline_unix_ms",
            "timeout_ms",
            "session_id",
            "session_generation",
            "daemon_generation",
            "expected_daemon_generation",
            "expected_session_generation",
        }
    )
    context_proto = descriptor_pb2.DescriptorProto()
    cpg.RequestContext.DESCRIPTOR.CopyToProto(context_proto)
    assert [(item.start, item.end) for item in context_proto.reserved_range] == [
        (2, 3),
        (3, 4),
        (4, 5),
    ]
    assert set(cpg.AuthorityGeneration.DESCRIPTOR.fields_by_name) == {
        "session_id",
        "session_generation",
        "daemon_generation",
        "supervisor_generation",
        "policy_generation",
        "revocation_generation",
    }
    handshake_budget_type = cpg.HandshakeRequest.DESCRIPTOR.fields_by_name[
        "remaining_budget"
    ].message_type
    assert handshake_budget_type is not None
    assert handshake_budget_type.full_name == "google.protobuf.Duration"
    assert [
        value.name for value in cpg.ReservedControlOperation.DESCRIPTOR.values if value.number
    ] == [
        "RESERVED_CONTROL_OPERATION_HANDSHAKE",
        "RESERVED_CONTROL_OPERATION_GET_STATUS",
        "RESERVED_CONTROL_OPERATION_CANCEL_QUERY",
        "RESERVED_CONTROL_OPERATION_RELEASE_RESOURCE",
    ]
    assert set(cpg.ReservedControlContract.DESCRIPTOR.fields_by_name) == {
        "reserved_capacity",
        "operations",
    }


def test_v2_has_no_duplicate_freshness_attempt_secret_or_v3_surface() -> None:
    descriptor_set = descriptor_pb2.FileDescriptorSet.FromString(DESCRIPTOR_PATH.read_bytes())
    assert not any(file.package.startswith("codefabric.cpgd.v3") for file in descriptor_set.file)
    cpg_file = next(file for file in descriptor_set.file if file.package == "codefabric.cpgd.v2")
    field_names = {field.name for message in cpg_file.message_type for field in message.field}
    assert not field_names.intersection(
        {
            "freshness_policy",
            "mcp_call_id",
            "rpc_attempt_id",
            "lease_token",
            "filesystem_path",
        }
    )
    assert "google/protobuf/any.proto" not in cpg_file.dependency


def test_exact_python_transport_versions_and_channel_bounds() -> None:
    assert importlib.metadata.version("grpcio") == "1.83.0"
    assert importlib.metadata.version("protobuf") == "7.36.0"
    assert GRPC_MESSAGE_OPTIONS == (
        ("grpc.max_send_message_length", MAX_CONTROL_MESSAGE_BYTES),
        ("grpc.max_receive_message_length", MAX_CONTROL_MESSAGE_BYTES),
        ("grpc.default_authority", GRPC_DEFAULT_AUTHORITY),
    )
    assert MAX_CONTROL_MESSAGE_BYTES == 4 * 1024 * 1024
    with patch("codefabric_cpg_mcp.daemon.channel.grpc.aio.insecure_channel") as create:
        channel = create_local_channel("unix:///tmp/codefabric.sock")
    create.assert_called_once_with("unix:///tmp/codefabric.sock", options=GRPC_MESSAGE_OPTIONS)
    assert channel is create.return_value
    for forbidden in ("tcp://127.0.0.1:50051", "unix://relative.sock", "unix:///"):
        with pytest.raises(ValueError, match="absolute Unix socket"):
            create_local_channel(forbidden)
