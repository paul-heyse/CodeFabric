"""Production Protobuf interoperability, descriptors, and channel contracts."""

import importlib.metadata
import json
from collections.abc import Iterable
from pathlib import Path
from unittest.mock import patch

from google.protobuf import descriptor_pb2, descriptor_pool
from google.protobuf.message import Message
from jsonschema import Draft202012Validator

from codefabric_cpg_mcp.contracts.query_forms import (
    QUERY_FORM_CONTRACT_DIGEST,
    QUERY_FORM_CONTRACT_ID,
    QUERY_FORM_NODE_KINDS,
    QUERY_FORMS,
)
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


def clear_derived_json_names(
    messages: Iterable[descriptor_pb2.DescriptorProto],
) -> None:
    for message in messages:
        for field in message.field:
            field.ClearField("json_name")
        clear_derived_json_names(message.nested_type)


def production_messages() -> dict[str, Message]:
    return {
        "cpg_start_query": cpg.StartQueryRequest(
            agent_instance_id="agent:test",
            workspace_id="ws:test",
            semantic_query_version="1.3",
            canonical_request_json=b'{"kind":"lookup"}',
            request_checksum="b3:test",
            idempotency_key="idem:test",
        ),
        "provider_job": provider.ProviderJobSpec(
            provider_run_id="run:test",
            workspace_id="ws:test",
            analysis_context_id="context:source",
            source_generation=7,
            resource_profile_id="in-process-syntax-standard",
        ),
        "pyrefly_hello": pyrefly.Hello(
            protocol_major=1,
            maximum_arrow_chunk_bytes=1_048_576,
        ),
        "rustc_accepted": rustc.CompilationAccepted(
            provider_run_id="run:test",
            compilation_unit_id="unit:test",
            accepted_generation=7,
        ),
    }


def test_wp10_behavioral_acceptance_all_packages_match_wire_kats() -> None:
    fixture = json.loads(
        (ROOT / "contracts/fixtures/proto/production_wire.json").read_text(encoding="utf-8")
    )
    expected = {case["name"]: case["wire_hex"] for case in fixture["cases"]}
    messages = production_messages()

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
    assert set(files) == set(modules)

    for name, module in modules.items():
        source = descriptor_pb2.FileDescriptorProto()
        source.CopyFrom(files[name])
        clear_derived_json_names(source.message_type)
        generated = descriptor_pb2.FileDescriptorProto()
        module.DESCRIPTOR.CopyToProto(generated)
        assert generated == source

    pool = descriptor_pool.DescriptorPool()
    for name in (
        "contracts/rpc/provider_control.proto",
        "contracts/rpc/cpg_query_service.proto",
        "contracts/rpc/pyrefly_sidecar.proto",
        "contracts/rpc/rustc_extractor.proto",
    ):
        pool.AddSerializedFile(files[name].SerializeToString())
    assert set(pool.FindServiceByName("codefabric.cpgd.v1.CpgQueryService").methods_by_name) == {
        "Handshake",
        "GetStatus",
        "ValidateQuery",
        "StartQuery",
        "StreamQuery",
        "AttachQuery",
        "CancelQuery",
        "ReadResult",
        "ReleaseResult",
    }


def test_wp10_structural_acceptance_is_closed_and_registry_aligned() -> None:
    service = cpg.DESCRIPTOR.services_by_name["CpgQueryService"]
    assert len(service.methods) == 9
    assert "ExecuteQuery" not in service.methods_by_name
    event = cpg.QueryEvent.DESCRIPTOR.oneofs_by_name["event"]
    assert [field.name for field in event.fields] == [
        "snapshot_pinned",
        "progress",
        "response_chunk",
        "artifact_ready",
        "terminal",
    ]
    sequence = cpg.QueryEventHeader.DESCRIPTOR.fields_by_name["sequence"]
    assert sequence.type == sequence.TYPE_UINT64
    assert cpg.FRESHNESS_POLICY_REQUIRE_CURRENT_FOR_TARGETS == 3
    assert provider.PROVIDER_RUN_STATE_PROTOCOL_ERROR == 100
    assert provider.CREDIT_CONTROL_LIMIT_MAX_OUTSTANDING_CHUNKS == 4
    assert provider.CREDIT_CONTROL_LIMIT_MAX_UNACKNOWLEDGED_MIB == 16
    resource_profile_id = provider.ProviderJobSpec.DESCRIPTOR.fields_by_name["resource_profile_id"]
    assert resource_profile_id.number == 15
    assert resource_profile_id.type == resource_profile_id.TYPE_STRING


def test_relation_ipc_frame_contract_has_exact_cross_process_directions() -> None:
    relation_frame = provider.RelationIpcFrame.DESCRIPTOR.oneofs_by_name["frame"]
    assert [(field.name, field.number) for field in relation_frame.fields] == [
        ("open", 1),
        ("payload", 2),
        ("flow_control_ack", 3),
        ("ipc_end", 4),
        ("coverage_trailer", 5),
        ("terminal", 6),
    ]
    assert provider.ARROW_IPC_METADATA_VERSION_V5 == 5
    assert provider.RELATION_IPC_TERMINAL_STATUS_COMPLETE == 1

    pyrefly_command = pyrefly.AnalyzeCommand.DESCRIPTOR.oneofs_by_name["command"]
    pyrefly_event = pyrefly.AnalyzeEvent.DESCRIPTOR.oneofs_by_name["event"]
    pyrefly_command_fields = {field.name: field for field in pyrefly_command.fields}
    pyrefly_event_fields = {field.name: field for field in pyrefly_event.fields}
    assert pyrefly_command_fields["relation_ipc_ack"].number == 5
    assert pyrefly_event_fields["relation_ipc_frame"].number == 7
    assert (
        pyrefly_command_fields["relation_ipc_ack"].message_type.full_name
        == "codefabric.provider.v1.RelationIpcFrame"
    )

    rustc_command = rustc.ExtractorCommand.DESCRIPTOR.oneofs_by_name["command"]
    rustc_event = rustc.ExtractionEvent.DESCRIPTOR.oneofs_by_name["event"]
    rustc_command_fields = {field.name: field for field in rustc_command.fields}
    rustc_event_fields = {field.name: field for field in rustc_event.fields}
    assert rustc_command_fields["relation_ipc_ack"].number == 5
    assert rustc_event_fields["owner_relation_ipc_frame"].number == 7
    assert (
        rustc_command_fields["relation_ipc_ack"].message_type.full_name
        == "codefabric.provider.v1.RelationIpcFrame"
    )


def test_relation_ipc_binary_round_trip_keeps_arrow_bytes_payload_only() -> None:
    identity = provider.RelationIpcStreamIdentity(
        relation_id=b"r" * 16,
        stream_id=b"s" * 16,
        schema_fingerprint=b"f" * 32,
        source_pin=b"i" * 32,
        context_pin=b"c" * 32,
    )
    header = provider.RelationIpcFrameHeader(
        protocol_version=1,
        identity=identity,
        sequence=1,
    )
    arrow_fragment = b"ARROW1\x00typed-semantic-bytes"
    original = provider.RelationIpcFrame(
        payload=provider.RelationIpcPayload(
            header=header,
            arrow_ipc_fragment=arrow_fragment,
        )
    )

    encoded = original.SerializeToString(deterministic=True)
    decoded = provider.RelationIpcFrame.FromString(encoded)
    assert decoded.WhichOneof("frame") == "payload"
    assert decoded.payload.arrow_ipc_fragment == arrow_fragment
    assert decoded.payload.header.identity == identity
    assert type(original).FromString(encoded).SerializeToString(deterministic=True) == encoded

    decoded.open.CopyFrom(
        provider.RelationIpcOpen(
            header=header,
            requested_units=1,
            arrow_type_universe=(
                "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5"
            ),
            metadata_version=provider.ARROW_IPC_METADATA_VERSION_V5,
            semantic_encoding="typed-arrow-relation-stream",
        )
    )
    assert decoded.WhichOneof("frame") == "open"
    assert not decoded.HasField("payload")


def test_wp67_structural_acceptance_admin_protocol_schema_examples() -> None:
    schema = json.loads(
        (ROOT / "contracts/rpc/admin-line-protocol.schema.json").read_text(encoding="utf-8")
    )
    examples = json.loads(
        (ROOT / "contracts/fixtures/admin-line-protocol-examples.json").read_text(encoding="utf-8")
    )
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    assert all(validator.is_valid(value) for value in examples["valid"])
    assert all(not validator.is_valid(value) for value in examples["invalid"])


def test_query_form_python_projection_parity() -> None:
    contract = json.loads(
        (
            ROOT / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query-form-contract.json"
        ).read_text(encoding="utf-8")
    )
    schema = json.loads(
        (ROOT / "contracts/schema/cpg-semantic-query-request.schema.json").read_text(
            encoding="utf-8"
        )
    )
    Draft202012Validator.check_schema(schema)
    assert contract["artifact_id"] == QUERY_FORM_CONTRACT_ID
    assert contract["canonical_digest"] == QUERY_FORM_CONTRACT_DIGEST
    assert tuple(form["slug"] for form in contract["forms"]) == QUERY_FORMS
    assert {form["slug"]: form["node_kind"] for form in contract["forms"]} == QUERY_FORM_NODE_KINDS
    schema_slugs = tuple(
        variant["properties"]["request"]["const"]
        for variant in schema["properties"]["queries"]["items"]["oneOf"]
    )
    assert schema_slugs == QUERY_FORMS


def test_pre_profile_provider_job_wire_remains_decodable() -> None:
    pre_profile_wire = bytes.fromhex(
        "0a0872756e3a74657374120777733a746573741a0e636f6e746578743a736f757263652007"
    )
    decoded = provider.ProviderJobSpec.FromString(pre_profile_wire)
    assert decoded.provider_run_id == "run:test"
    assert decoded.resource_profile_id == ""


def test_python_binary_round_trip_preserves_unknown_fields() -> None:
    known = cpg.StartQueryRequest(workspace_id="ws:test").SerializeToString()
    unknown_field_99_varint_42 = b"\x98\x06\x2a"
    older_reader = cpg.StartQueryRequest.FromString(known + unknown_field_99_varint_42)
    assert older_reader.SerializeToString(deterministic=True) == (
        known + unknown_field_99_varint_42
    )


def test_exact_python_protobuf_runtime_versions_are_installed() -> None:
    assert importlib.metadata.version("grpcio") == "1.83.0"
    assert importlib.metadata.version("protobuf") == "7.36.0"


def test_python_channel_applies_symmetric_four_mib_limits() -> None:
    assert GRPC_MESSAGE_OPTIONS == (
        ("grpc.max_send_message_length", MAX_CONTROL_MESSAGE_BYTES),
        ("grpc.max_receive_message_length", MAX_CONTROL_MESSAGE_BYTES),
        ("grpc.default_authority", GRPC_DEFAULT_AUTHORITY),
    )
    assert MAX_CONTROL_MESSAGE_BYTES == 4 * 1024 * 1024

    with patch("codefabric_cpg_mcp.daemon.channel.grpc.aio.insecure_channel") as create:
        channel = create_local_channel("unix:///tmp/codefabric.sock")

    create.assert_called_once_with(
        "unix:///tmp/codefabric.sock",
        options=GRPC_MESSAGE_OPTIONS,
    )
    assert channel is create.return_value
