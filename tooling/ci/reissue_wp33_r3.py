"""One-shot authoring utility for the WP33 successor-only r3 evidence reissue.

This file encodes independently authored expectations from the accepted v2.1
specifications.  It does not import or execute production behavior.  The generated
artifacts remain pending independent review.  The ten materially changed claims force
one new issuance, so the unchanged claims are adopted under the same author identity
and reviewed again as part of the indivisible transaction.
"""

from __future__ import annotations

import ast
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

import blake3
import rfc8785

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / "contracts/acceptance/relational-fabric-v3"
EXPECTATIONS = EVIDENCE / "expectations.jsonl"
FIXTURES = EVIDENCE / "negative-fixtures.jsonl"
ISSUANCE = EVIDENCE / "evidence-issuance.json"

AUTHOR = "wp33-successor-expectation-author-r3"
ISSUANCE_ID = "wp33:fixture-r3"
REVIEWER = "wp33-independent-successor-reissue-reviewer-r2"
REISSUED = {
    f"RFV3-CLAIM-{number:03d}"
    for number in (1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18)
}

CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE = 4
CBEF_SOURCE_FILE_DOMAIN_CODE = 6
CBEF_ENTITY_DOMAIN_CODE = 8
CBEF_RELATION_FACT_DOMAIN_CODE = 9
CBEF_PATH_RESULT_DOMAIN_CODE = 18
CBEF_QUERY_SOURCE_CONTEXT_DOMAIN_CODE = 21
PYTHON_FUNCTION_KIND_CODE = 120
PYTHON_CALL_SITE_KIND_CODE = 130
CALLS_RELATION_KIND_CODE = 50


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def canonical_b3(value: object) -> str:
    return f"b3:{blake3.blake3(rfc8785.dumps(value)).hexdigest()}"


def bytes_b3(value: bytes) -> str:
    return f"b3:{blake3.blake3(value).hexdigest()}"


def file_b3(path: str) -> str:
    return bytes_b3((ROOT / path).read_bytes())


def explicit_typed_fixture_bytes(fill: int, width: int) -> str:
    """Return visible nonzero fixture bytes without pretending labels are identity."""

    if not 0 < fill <= 0xFF or width not in {16, 32}:
        raise ValueError(
            "typed fixture bytes require a nonzero byte and governed width"
        )
    return (bytes([fill]) * width).hex()


def identity_recipe(
    *, domain: str, prefix: str, preimage: dict[str, Any], excluded: list[str]
) -> dict[str, Any]:
    envelope = {
        "domain": domain,
        "recipe_version": "codefabric.canonical-public-id.v1",
        "preimage": preimage,
    }
    canonical = rfc8785.dumps(envelope)
    output_id = f"{prefix}:{blake3.blake3(canonical).digest()[:16].hex()}"
    return {
        "recipe_version": "codefabric.canonical-public-id.v1",
        "strict_ingress": "reject duplicate members and non-I-JSON values before mapping",
        "canonicalization": "RFC8785 JCS UTF-8 bytes",
        "framing": "one canonical object with domain, recipe_version, and preimage members; no byte concatenation",
        "digest": {
            "algorithm": "BLAKE3",
            "mode": "unkeyed",
            "full_output_bytes": 32,
            "truncation": "first 16 digest bytes",
            "text_encoding": "lowercase hexadecimal",
        },
        "envelope": envelope,
        "canonical_jcs_utf8": canonical.decode("utf-8"),
        "excluded": excluded,
        "output_id": output_id,
    }


def cbef_field(tag: int, type_code: int, payload: bytes) -> bytes:
    return (
        tag.to_bytes(2, "big")
        + type_code.to_bytes(1, "big")
        + len(payload).to_bytes(4, "big")
        + payload
    )


def cbef_record(domain_code: int, fields: list[tuple[int, int, bytes]]) -> bytes:
    if (
        not fields
        or fields[0][0] == 0
        or [field[0] for field in fields] != sorted({field[0] for field in fields})
    ):
        raise ValueError("CBEF-v1 fields must have unique ascending nonzero tags")
    return (
        b"CFID"
        + b"\x01"
        + domain_code.to_bytes(2, "big")
        + len(fields).to_bytes(2, "big")
        + b"".join(cbef_field(*field) for field in fields)
    )


def public_id_bytes(value: object, prefix: str) -> bytes:
    text = str(value)
    expected = f"{prefix}:"
    encoded = text.removeprefix(expected)
    if not text.startswith(expected) or len(encoded) != 32:
        raise ValueError(f"{prefix} identity must contain exactly 16 lowercase bytes")
    try:
        decoded = bytes.fromhex(encoded)
    except ValueError as error:
        raise ValueError(f"{prefix} identity is not lowercase hexadecimal") from error
    if encoded != encoded.lower():
        raise ValueError(f"{prefix} identity is not lowercase hexadecimal")
    return decoded


def analysis_context_id_bytes(value: object) -> bytes:
    text = str(value)
    if text == "context:source":
        return b"\xff" * 16
    if text.startswith("context:"):
        return public_id_bytes(text, "context")
    if len(text) == 64 and text == text.lower():
        try:
            # Provider relations retain the full 256-bit context digest; CBEF ID
            # fields carry its governed 128-bit public identity.
            return bytes.fromhex(text)[:16]
        except ValueError as error:
            raise ValueError("analysis context digest is not hexadecimal") from error
    raise ValueError(
        "analysis context identity is neither context:source nor a CBEF ID"
    )


def digest_bytes(value: object, prefix: str) -> bytes:
    text = str(value)
    expected = f"{prefix}:"
    encoded = text.removeprefix(expected)
    if not text.startswith(expected) or len(encoded) != 64:
        raise ValueError(f"{prefix} digest must contain exactly 32 lowercase bytes")
    try:
        decoded = bytes.fromhex(encoded)
    except ValueError as error:
        raise ValueError(f"{prefix} digest is not lowercase hexadecimal") from error
    if encoded != encoded.lower():
        raise ValueError(f"{prefix} digest is not lowercase hexadecimal")
    return decoded


def cbef_analysis_context_id(
    *, workspace_id: object, language_slug: str, environment_digest: object
) -> str:
    """Build the closed CBEF-v1 ANALYSIS_CONTEXT recipe."""

    if not language_slug.isascii():
        raise ValueError("analysis-context language slug must be ASCII")
    preimage = cbef_record(
        CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
        [
            (1, 7, public_id_bytes(workspace_id, "workspace")),
            (2, 2, language_slug.lower().encode("utf-8")),
            (3, 8, digest_bytes(environment_digest, "b3")),
        ],
    )
    return f"context:{blake3.blake3(preimage).digest()[:16].hex()}"


def cbef_analysis_context_digest(
    *, workspace_id: object, language_slug: str, environment_digest: object
) -> str:
    """Return the provider relation's full CBEF analysis-context digest."""

    if not language_slug.isascii():
        raise ValueError("analysis-context language slug must be ASCII")
    preimage = cbef_record(
        CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
        [
            (1, 7, public_id_bytes(workspace_id, "workspace")),
            (2, 2, language_slug.lower().encode("utf-8")),
            (3, 8, digest_bytes(environment_digest, "b3")),
        ],
    )
    return blake3.blake3(preimage).hexdigest()


def cbef_source_file_id(*, workspace_id: object, comparison_key: bytes) -> str:
    """Build the closed CBEF-v1 SOURCE_FILE identity as raw 16-byte Arrow hex."""

    if not comparison_key:
        raise ValueError("source-file comparison key must be non-empty")
    preimage = cbef_record(
        CBEF_SOURCE_FILE_DOMAIN_CODE,
        [
            (1, 7, public_id_bytes(workspace_id, "workspace")),
            (2, 7, analysis_context_id_bytes("context:source")),
            (3, 1, comparison_key),
        ],
    )
    return blake3.blake3(preimage).digest()[:16].hex()


def semantic_environment_digest(
    *,
    provider_id: str,
    provider_release: str,
    relation_id: str,
    crate_inputs: dict[str, Any] | None,
    trust_inputs: dict[str, Any] | None,
) -> str:
    """Bind a semantic-environment pin to its exact declared typed inputs."""

    return canonical_b3(
        {
            "provider_id": provider_id,
            "provider_release": provider_release,
            "relation_id": relation_id,
            "crate_manifest_target_inputs": crate_inputs,
            "trust_inputs": trust_inputs,
        }
    )


def cbef_entity_preimage(
    *,
    workspace_id: object,
    analysis_context_id: object,
    kind_code: int,
    owner_id: object,
    owner_prefix: str,
    semantic_key: bytes,
) -> bytes:
    """Build the closed CBEF-v1 ENTITY recipe; callers cannot author tags."""

    return cbef_record(
        CBEF_ENTITY_DOMAIN_CODE,
        [
            (1, 7, public_id_bytes(workspace_id, "workspace")),
            (2, 7, analysis_context_id_bytes(analysis_context_id)),
            (3, 4, kind_code.to_bytes(2, "big")),
            (4, 7, public_id_bytes(owner_id, owner_prefix)),
            (5, 1, semantic_key),
        ],
    )


def cbef_relation_fact_preimage(
    *,
    workspace_id: object,
    analysis_context_id: object,
    relation_kind_code: int,
    subject_id: object,
    object_id: object,
    role: str,
) -> bytes:
    """Build the closed CBEF-v1 RELATION_FACT recipe; callers cannot author tags."""

    role_payload = role.encode("utf-8")
    typed_role = b"\x02" + len(role_payload).to_bytes(4, "big") + role_payload
    tagged_role = b"\x00\x01" + len(typed_role).to_bytes(4, "big") + typed_role
    return cbef_record(
        CBEF_RELATION_FACT_DOMAIN_CODE,
        [
            (1, 7, public_id_bytes(workspace_id, "workspace")),
            (2, 7, analysis_context_id_bytes(analysis_context_id)),
            (3, 4, relation_kind_code.to_bytes(2, "big")),
            (4, 7, public_id_bytes(subject_id, "entity:function")),
            (5, 7, public_id_bytes(object_id, "entity:function")),
            (6, 12, tagged_role),
        ],
    )


def python_function_id(
    identity_context: dict[str, Any], qualified_lexical_path: list[str]
) -> str:
    semantic_key = rfc8785.dumps(
        {
            "schema_version": 1,
            "module_id": identity_context["module_id"],
            "qualified_lexical_path": qualified_lexical_path,
            "kind": "function",
        }
    )
    preimage = cbef_entity_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        kind_code=PYTHON_FUNCTION_KIND_CODE,
        owner_id=identity_context["module_id"],
        owner_prefix="entity:module",
        semantic_key=semantic_key,
    )
    return f"entity:function:{blake3.blake3(preimage).digest()[:16].hex()}"


def python_call_site_id(
    identity_context: dict[str, Any],
    *,
    owner_id: str,
    owner_relative_role: str,
    owner_relative_ordinal: int,
    start_byte: int,
    end_byte: int,
) -> str:
    semantic_key = rfc8785.dumps(
        {
            "module_id": identity_context["module_id"],
            "owner_relative_role": owner_relative_role,
            "owner_relative_ordinal": owner_relative_ordinal,
            "file_id": identity_context["file_id"],
            "content_digest": identity_context["content_digest"],
            "source_range": {"start_byte": start_byte, "end_byte": end_byte},
            "kind": "call_site",
            "schema_version": 1,
        },
    )
    preimage = cbef_entity_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        kind_code=PYTHON_CALL_SITE_KIND_CODE,
        owner_id=owner_id,
        owner_prefix="entity:function",
        semantic_key=semantic_key,
    )
    return f"entity:call-site:{blake3.blake3(preimage).digest()[:16].hex()}"


def calls_fact_id(
    identity_context: dict[str, Any],
    *,
    caller_id: str,
    callee_id: str,
    call_site_id: str,
) -> str:
    role = rfc8785.dumps(
        {
            "module_id": identity_context["module_id"],
            "call_site_id": call_site_id,
            "schema_version": 1,
        }
    ).decode("utf-8")
    preimage = cbef_relation_fact_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        relation_kind_code=CALLS_RELATION_KIND_CODE,
        subject_id=caller_id,
        object_id=callee_id,
        role=role,
    )
    return f"fact:calls:{blake3.blake3(preimage).digest()[:16].hex()}"


def node_byte_range(source: str, node: ast.AST) -> tuple[int, int]:
    if not all(
        hasattr(node, name)
        for name in ("lineno", "col_offset", "end_lineno", "end_col_offset")
    ):
        raise ValueError("Python identity fixture node lacks an exact source range")
    lines = source.splitlines(keepends=True)
    start_line = int(node.lineno) - 1
    end_line = int(node.end_lineno) - 1
    start = sum(len(line.encode("utf-8")) for line in lines[:start_line]) + int(
        node.col_offset
    )
    end = sum(len(line.encode("utf-8")) for line in lines[:end_line]) + int(
        node.end_col_offset
    )
    return start, end


def field(name: str, data_type: str, nullable: bool, meaning: str) -> dict[str, Any]:
    return {
        "name": name,
        "data_type": data_type,
        "nullable": nullable,
        "metadata": {
            "codefabric.meaning": meaning,
            "codefabric.semantic_representation": "typed-arrow-field",
        },
    }


NATIVE_COMMON = [
    field("provider_run_id", "fixed_size_binary[16]", False, "provider-run-id"),
    field("provider_id", "utf8", False, "provider-id"),
    field("provider_release", "utf8", False, "provider-release"),
    field("analysis_context_id", "fixed_size_binary[32]", False, "analysis-context-id"),
    field(
        "semantic_environment_id",
        "fixed_size_binary[32]",
        False,
        "semantic-environment-id",
    ),
    field("file_id", "fixed_size_binary[16]", False, "file-id"),
    field("content_digest", "fixed_size_binary[32]", False, "content-digest"),
    field("source_generation", "uint64", False, "source-generation"),
]


def relation_contract(
    relation_id: str,
    release: str,
    common: list[dict[str, Any]],
    specific: list[dict[str, Any]],
) -> dict[str, Any]:
    fields = [*copy.deepcopy(common), *specific]
    contract = {
        "relation_id": relation_id,
        "schema_contract_version": 1,
        "provider_release": release,
        "arrow_type_universe": "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5",
        "semantic_encoding": "typed-arrow-relation-stream",
        "fields": fields,
    }
    contract["schema_identity"] = canonical_b3(contract)
    return contract


def pyrefly_relation_schema_identity(contract: dict[str, Any]) -> str:
    """Derive the sidecar's application-owned Arrow schema identity."""

    provider_release = str(contract["provider_release"])
    release, revision = provider_release.removeprefix("pyrefly@").split("#", 1)
    fields = ";".join(
        f"{field_value['name']}:{field_value['data_type']}:"
        f"{'nullable' if field_value['nullable'] else 'required'}"
        for field_value in contract["fields"]
    )
    descriptor = (
        f"{contract['relation_id']}|protocol=1|"
        f"arrow={contract['arrow_type_universe']}|"
        f"provider={release}@{revision}|{fields}"
    )
    return bytes_b3(descriptor.encode("utf-8"))


TREE_CONTRACT = relation_contract(
    "provider.tree_sitter.cst_node",
    "tree-sitter@0.26.12|tree-sitter-python@0.25.0",
    NATIVE_COMMON,
    [
        field("provider_local_node_id", "uint64", False, "provider-local-id"),
        field("parent_provider_local_node_id", "uint64", True, "provider-local-id"),
        field("raw_kind_id", "uint16", False, "provider-native-kind-id"),
        field("raw_kind", "utf8", False, "provider-native-kind"),
        field("field_name", "utf8", True, "provider-native-field"),
        field("start_byte", "uint64", False, "source-byte-start"),
        field("end_byte", "uint64", False, "source-byte-end"),
        field("named", "boolean", False, "provider-native-flag"),
        field("extra", "boolean", False, "provider-native-flag"),
        field("error", "boolean", False, "provider-native-flag"),
        field("missing", "boolean", False, "provider-native-flag"),
        field("ordinal", "uint32", False, "provider-local-ordinal"),
        field("depth", "uint16", False, "provider-local-depth"),
        field("raw_kind_disposition", "utf8", False, "raw-kind-disposition"),
    ],
)

RUFF_CONTRACT = relation_contract(
    "provider.ruff.ast_node",
    "ruff@0.0.7",
    NATIVE_COMMON,
    [
        field("provider_local_ast_id", "uint64", False, "provider-local-id"),
        field("parent_provider_local_ast_id", "uint64", True, "provider-local-id"),
        field("raw_kind_id", "uint16", False, "provider-native-kind-id"),
        field("raw_kind", "utf8", False, "provider-native-kind"),
        field("ast_category", "utf8", False, "typed-ast-category"),
        field("child_role", "utf8", True, "typed-ast-child-role"),
        field("start_byte", "uint64", False, "source-byte-start"),
        field("end_byte", "uint64", False, "source-byte-end"),
        field("line", "uint32", False, "provider-native-coordinate"),
        field("column", "uint32", False, "provider-native-coordinate"),
        field("child_ordinal", "uint32", False, "provider-local-ordinal"),
        field("source_ordinal", "uint32", False, "provider-local-ordinal"),
        field("evaluation_ordinal", "uint32", True, "provider-local-ordinal"),
        field("explicit_parenthesized", "boolean", False, "provider-native-flag"),
        field("raw_kind_disposition", "utf8", False, "raw-kind-disposition"),
    ],
)

PYREFLY_COMMON = [
    field("provider_run_id", "utf8", False, "provider-run-id"),
    field("analysis_context_id", "utf8", False, "analysis-context-id"),
    field("module_id", "utf8", False, "module-id"),
    field("file_id", "utf8", False, "file-id"),
    field("content_digest", "fixed_size_binary[32]", False, "content-digest"),
    field(
        "semantic_environment_id",
        "fixed_size_binary[32]",
        False,
        "semantic-environment-id",
    ),
    field("source_generation", "uint64", False, "source-generation"),
]
PYREFLY_CONTRACT = relation_contract(
    "provider.pyrefly.call_target.v1",
    "pyrefly@1.2.0#1933169ad8ee9e4d4114112eb56ef0811fb0a094",
    PYREFLY_COMMON,
    [
        field(
            "call_occurrence_ordinal", "uint64", False, "provider-local-call-occurrence"
        ),
        field("start_byte", "uint64", False, "source-byte-start"),
        field("end_byte", "uint64", False, "source-byte-end"),
        field("target_ordinal", "uint64", False, "provider-local-target-ordinal"),
        field("callee_kind", "utf8", False, "provider-native-callee-kind"),
        field("qualified_target", "utf8", False, "provider-native-qualified-target"),
        field("class_name", "utf8", True, "provider-native-class-name"),
        field("resolution_state", "utf8", False, "provider-completeness-state"),
    ],
)
PYREFLY_CONTRACT["schema_identity"] = pyrefly_relation_schema_identity(PYREFLY_CONTRACT)

RUSTC_COMMON = [
    field("provider_run_id", "utf8", False, "provider-run-id"),
    field("compilation_unit_id", "utf8", False, "compilation-unit-id"),
    field("owner_id", "utf8", False, "owner-id"),
    field("source_generation", "uint64", False, "source-generation"),
    field("source_file_id", "utf8", False, "source-file-id"),
    field("source_content_digest", "fixed_size_binary[32]", False, "content-digest"),
    field("stable_crate_id", "uint64", True, "private-stable-crate-id"),
    field("def_path_hash", "fixed_size_binary[16]", True, "private-def-path-hash"),
]
RUSTC_CONTRACT = relation_contract(
    "provider.rustc.public_item.v1",
    "rustc@1.100.0-nightly#8fa1c96cfd489e4c27654c144ae871ce2c4db6c6",
    RUSTC_COMMON,
    [
        field("qualified_name", "utf8", False, "compiler-qualified-name"),
        field("item_kind", "utf8", False, "typed-public-item-kind"),
        field("has_body", "boolean", False, "typed-public-item-property"),
        field("is_foreign_item", "boolean", False, "typed-public-item-property"),
        field(
            "requires_monomorphization", "boolean", False, "typed-public-item-property"
        ),
        field("type_key", "fixed_size_binary[32]", False, "provider-local-typed-key"),
        field("span_file", "utf8", False, "compiler-source-file"),
        field("span_start", "uint64", False, "source-byte-start"),
        field("span_end", "uint64", False, "source-byte-end"),
        field("span_start_line", "uint64", False, "provider-source-line"),
        field("span_end_line", "uint64", False, "provider-source-line"),
        field("span_start_column", "uint64", False, "provider-source-column"),
        field("span_end_column", "uint64", False, "provider-source-column"),
        field("expansion_kind", "utf8", False, "compiler-expansion-kind"),
        field("in_external_macro", "boolean", False, "compiler-expansion-property"),
    ],
)


def source_image(
    source_id: str, language: str, path: str, content: str
) -> dict[str, Any]:
    workspace_id = "workspace:00000000000000000000000000000000"
    comparison_key = path.encode("utf-8")
    return {
        "source_id": source_id,
        "workspace_id": workspace_id,
        "file_id": cbef_source_file_id(
            workspace_id=workspace_id, comparison_key=comparison_key
        ),
        "canonical_path_bytes_hex": comparison_key.hex(),
        "language": language,
        "encoding": "utf-8",
        "source_kind": "source_file",
        "source_generation": 2,
        "access_authorization_id": "authorization:wp33-source-fixture",
        "bytes_utf8": content,
        "content_digest": bytes_b3(content.encode("utf-8")),
    }


def provider_request(
    provider_id: str,
    provider_run_id: str,
    language_slug: str,
    source_id: str,
    contract: dict[str, Any],
    release: str,
    *,
    crate_inputs: dict[str, Any] | None = None,
    trust_inputs: dict[str, Any] | None = None,
) -> dict[str, Any]:
    environment_digest = semantic_environment_digest(
        provider_id=provider_id,
        provider_release=release,
        relation_id=contract["relation_id"],
        crate_inputs=crate_inputs,
        trust_inputs=trust_inputs,
    )
    return {
        "provider_id": provider_id,
        "provider_run_id": provider_run_id,
        "provider_release": release,
        "source_id": source_id,
        "relation_id": contract["relation_id"],
        "requested_scope": {"scope_kind": "source_file", "scope_id": source_id},
        "analysis_context_id": cbef_analysis_context_digest(
            workspace_id="workspace:00000000000000000000000000000000",
            language_slug=language_slug,
            environment_digest=environment_digest,
        ),
        "semantic_environment_id": environment_digest.removeprefix("b3:"),
        "schema_contract": copy.deepcopy(contract),
        "crate_manifest_target_inputs": crate_inputs,
        "trust_inputs": trust_inputs,
    }


def claim_001(base: dict[str, Any]) -> dict[str, Any]:
    py_syntax = source_image(
        "py:syntax",
        "python",
        "fixture_syntax.py",
        "def f(x):\n    return x\n",
    )
    py_typed = source_image(
        "py:typed",
        "python",
        "fixture_typed.py",
        "def f(x: int) -> int:\n    return abs(x)\n",
    )
    rust_source = source_image(
        "rs:fixture",
        "rust",
        "fixture.rs",
        "pub fn identity(value: u32) -> u32 { value }\n",
    )
    crate_inputs = {
        "package": {
            "name": "wp33-rust-fixture",
            "version": "0.0.0",
            "edition": "2024",
            "publish": False,
            "dependencies": [],
        },
        "target": {
            "name": "wp33_rust_fixture",
            "kind": "lib",
            "crate_types": ["rlib"],
            "source_id": "rs:fixture",
            "target_triple": "x86_64-unknown-linux-gnu",
            "requested_features": [],
        },
        "toolchain": {
            "channel": "nightly-2026-08-18",
            "rustc_release": "1.100.0-nightly",
            "rustc_commit": "8fa1c96cfd489e4c27654c144ae871ce2c4db6c6",
        },
    }
    rust_trust = {
        "requested_profile": "untrusted",
        "direct_host_cargo_allowed": False,
        "required_launcher": "capability-proved immutable workspace, network and credential denial, resource accounting, seccomp, and descendant cleanup",
        "observed_host": {
            "launcher_kind": "bubblewrap",
            "launcher_version": "0.9.0",
            "launcher_version_role": "diagnostic_only",
            "compiled_seccomp_policy_authorized": False,
            "full_escape_matrix_proved": False,
        },
        "admission": "unavailable",
        "reason": "HOST_CONTAINMENT_PREREQUISITES_UNAVAILABLE",
    }
    requests = [
        provider_request(
            "tree-sitter-python",
            explicit_typed_fixture_bytes(0x31, 16),
            "python",
            "py:syntax",
            TREE_CONTRACT,
            "tree-sitter@0.26.12|tree-sitter-python@0.25.0",
        ),
        provider_request(
            "ruff",
            explicit_typed_fixture_bytes(0x32, 16),
            "python",
            "py:syntax",
            RUFF_CONTRACT,
            "ruff@0.0.7",
        ),
        provider_request(
            "pyrefly",
            explicit_typed_fixture_bytes(0x33, 16),
            "python",
            "py:typed",
            PYREFLY_CONTRACT,
            "pyrefly@1.2.0#1933169ad8ee9e4d4114112eb56ef0811fb0a094",
        ),
        provider_request(
            "rustc",
            explicit_typed_fixture_bytes(0x34, 16),
            "rust",
            "rs:fixture",
            RUSTC_CONTRACT,
            "rustc@1.100.0-nightly#8fa1c96cfd489e4c27654c144ae871ce2c4db6c6",
            crate_inputs=crate_inputs,
            trust_inputs=rust_trust,
        ),
    ]
    request_by_provider = {request["provider_id"]: request for request in requests}
    terminals = [
        {
            "provider_id": provider,
            "provider_run_id": request_by_provider[provider]["provider_run_id"],
            "relation_id": request_by_provider[provider]["relation_id"],
            "requested_units": 1,
            "completed_units": 0 if provider == "rustc" else 1,
            "remainders": (
                [
                    {
                        "units": 1,
                        "reason": "TRUST_SUBSTRATE_UNAVAILABLE",
                        "retryable": True,
                    }
                ]
                if provider == "rustc"
                else []
            ),
            "state": "unavailable" if provider == "rustc" else "complete",
        }
        for provider in ("tree-sitter-python", "ruff", "pyrefly", "rustc")
    ]
    rows = [
        [
            "pyrefly",
            request_by_provider["pyrefly"]["provider_run_id"],
            PYREFLY_CONTRACT["relation_id"],
            PYREFLY_CONTRACT["schema_identity"],
            "py:typed",
            py_typed["content_digest"],
            {
                "call_occurrence_ordinal": 0,
                "start_byte": 33,
                "end_byte": 36,
                "target_ordinal": 0,
                "callee_kind": "function",
                "qualified_target": "builtins.abs",
                "class_name": None,
                "resolution_state": "resolved",
            },
            "complete",
        ],
        [
            "ruff",
            request_by_provider["ruff"]["provider_run_id"],
            RUFF_CONTRACT["relation_id"],
            RUFF_CONTRACT["schema_identity"],
            "py:syntax",
            py_syntax["content_digest"],
            {
                "raw_kind": "FunctionDef",
                "ast_category": "statement",
                "start_byte": 0,
                "end_byte": len(
                    py_syntax["bytes_utf8"].removesuffix("\n").encode("utf-8")
                ),
                "raw_kind_disposition": "known",
            },
            "complete",
        ],
        [
            "rustc",
            request_by_provider["rustc"]["provider_run_id"],
            RUSTC_CONTRACT["relation_id"],
            RUSTC_CONTRACT["schema_identity"],
            "rs:fixture",
            rust_source["content_digest"],
            {
                "typed_remainder": "HOST_CONTAINMENT_PREREQUISITES_UNAVAILABLE",
                "crate_manifest_target_inputs": crate_inputs,
                "trust_inputs": rust_trust,
            },
            "unavailable",
        ],
        [
            "tree-sitter-python",
            request_by_provider["tree-sitter-python"]["provider_run_id"],
            TREE_CONTRACT["relation_id"],
            TREE_CONTRACT["schema_identity"],
            "py:syntax",
            py_syntax["content_digest"],
            {
                "provider_local_node_id": 2,
                "parent_provider_local_node_id": 1,
                "raw_kind": "function_definition",
                "start_byte": 0,
                "end_byte": len(
                    py_syntax["bytes_utf8"].removesuffix("\n").encode("utf-8")
                ),
                "named": True,
                "error": False,
                "missing": False,
                "raw_kind_disposition": "known",
            },
            "complete",
        ],
    ]
    base.update(
        {
            "subject": "Application-owned provider-native Arrow contracts bind exact source, release, context, and trust inputs; unavailable untrusted Rust compilation is an explicit remainder",
            "author_id": AUTHOR,
            "source_anchor": "GEN §4 Provider responsibility model; GEN §8 Immutable source-image contract; GEN §10 Provider-observation metadata; GEN §11 Relation-scoped Arrow IPC boundary; GEN AC-G-31 rustc extractor protocol and public/private seam",
            "governing_clauses": [
                "SUITE §5.1 Proof relations",
                "GEN §90 Provider job interfaces",
                "GEN AC-G-35 Provider sandbox and Rust compilation trust model",
            ],
            "complete_input_universe": {
                "closed": True,
                "inputs": {
                    "source_images": [py_syntax, py_typed, rust_source],
                    "provider_requests": requests,
                    "semantic_context": {
                        "workspace_id": "workspace:00000000000000000000000000000000",
                        "analysis_context_ids": {
                            request["provider_id"]: request["analysis_context_id"]
                            for request in requests
                        },
                        "semantic_environment_ids": {
                            request["provider_id"]: request["semantic_environment_id"]
                            for request in requests
                        },
                        "source_generation": 2,
                        "authority": "exact immutable source image plus application-owned relation contract and declared semantic-environment inputs",
                    },
                    "provider_release_pins": {
                        "tree_sitter_runtime": "0.26.12",
                        "tree_sitter_python": "0.25.0",
                        "ruff_stable_root": "0.0.7",
                        "pyrefly": "1.2.0@1933169ad8ee9e4d4114112eb56ef0811fb0a094",
                        "rustc_extractor_toolchain": "nightly-2026-08-18",
                        "rustc_compiler": "1.100.0-nightly@8fa1c96cfd489e4c27654c144ae871ce2c4db6c6",
                    },
                    "requested_family_set": [
                        request["relation_id"] for request in requests
                    ],
                    "coverage_terminals": terminals,
                    "execution_policy": {
                        "deadline_ms": 1000,
                        "cancellation": "enabled",
                        "resource_profile": "fixture-bounded",
                        "rust_trust_profile": "untrusted",
                        "fail_closed": True,
                    },
                    "protocol_schema_identity": {
                        "control_schema": "contracts/rpc/provider_control.proto",
                        "control_schema_b3": file_b3(
                            "contracts/rpc/provider_control.proto"
                        ),
                        "payload_encoding": "Arrow IPC stream",
                        "metadata_version": "V5",
                        "protocol_id": "provider-relation-ipc-v1",
                        "schema_contract": {
                            "request_message": "codefabric.provider.v1.ProviderJobSpec",
                            "chunk_message": "codefabric.provider.v1.ProviderObservationChunkEvent",
                            "terminal_message": "codefabric.provider.v1.ProviderTerminalEvent",
                            "semantic_payload_field": "arrow_ipc",
                            "stream_schema_authority": "schema_digest",
                            "dictionary_scope": "one IPC stream",
                        },
                    },
                },
            },
            "decoded_expectation": {
                "terminal": "pass_with_explicit_trust_remainder",
                "relation": "proof.provider_native_contract_expectation",
                "columns": [
                    "provider_id",
                    "provider_run_id",
                    "relation_id",
                    "schema_identity",
                    "source_id",
                    "content_digest",
                    "native_fields_or_remainder",
                    "coverage_state",
                ],
                "rows": rows,
                "coverage": "one independently selected typed observation exercises each available provider contract; source-file completion is represented only by the separate coverage terminals, while the Rust contract remains an explicit trust remainder",
            },
            "semantics": {
                "ordering": "provider_id ascending, then relation_id ascending",
                "nulls": "only fields declared nullable by the selected Arrow relation contract may be null",
                "unknowns": "the unavailable hostile Rust trust substrate is a typed remainder and never an empty or fabricated fact row",
                "provenance": "each row binds provider run, exact release, source BLAKE3 digest, analysis and semantic-environment identities, relation schema identity, and terminal coverage",
            },
            "limitations": [
                "The current workstation cannot execute the untrusted rustc provider contract because an authorized application-owned compiled seccomp policy and hostile containment proof are absent; no executable version alone is treated as security proof and no hostile-run success is claimed."
            ],
        }
    )
    return base


DERIVED_FAMILIES: list[tuple[str, str]] = [
    ("python.cfg_node", "GEN §24"),
    ("python.cfg_edge", "GEN §24"),
    ("python.evaluation_order", "GEN §24"),
    ("python.def_use", "GEN §25"),
    ("python.reaching_definition", "GEN §25"),
    ("python.liveness", "GEN §25"),
    ("python.value_flow", "GEN §25"),
    ("python.memory_location", "GEN §26"),
    ("python.alias_points_to", "GEN §26"),
    ("python.effect", "GEN §27"),
    ("python.exceptional_flow", "GEN §28"),
    ("python.resource_lifecycle", "GEN §29"),
    ("python.async_suspension", "GEN §30"),
    ("python.closure_capture", "GEN §31"),
    ("python.unknown", "GEN §33"),
    ("rust_mir.ownership_state", "GEN §44"),
    ("rust_mir.def_use", "GEN §45"),
    ("rust_mir.reaching_definition", "GEN §45"),
    ("rust_mir.liveness", "GEN §45"),
    ("rust_mir.value_flow", "GEN §45"),
    ("rust_mir.alias_points_to", "GEN §46"),
    ("rust_mir.resource_lifecycle", "GEN §47"),
    ("rust_mir.async_lowering", "GEN §48"),
    ("rust_mir.unsafe_ffi_candidate", "GEN §50"),
    ("rust_mir.control_dependence", "GEN §57"),
    ("rust_mir.unknown", "GEN §51"),
    ("common.reachability", "GEN §54"),
    ("common.scc_membership", "GEN §55"),
    ("common.dominator", "GEN §56"),
    ("common.post_dominator", "GEN §56"),
    ("common.control_dependence", "GEN §57"),
    ("common.loop", "GEN §58"),
    ("common.reaching_definition", "GEN §59"),
    ("common.liveness", "GEN §60"),
    ("common.alias_points_to", "GEN §61"),
    ("common.shortest_distance", "GEN §62"),
    ("common.connected_component", "GEN §63"),
    ("common.transitive_closure", "GEN §64"),
    ("common.transitive_reduction", "GEN §64"),
    ("common.structural_metric", "GEN §65"),
    ("common.callable_summary", "GEN §66"),
    ("common.call_graph", "GEN §72"),
    ("common.callable_effect", "GEN §77"),
    ("common.callable_resource", "GEN §77B"),
    ("common.unknown", "GEN §84"),
    ("common.completeness", "GEN §85"),
    ("common.invalidation", "GEN §87"),
]
ALGORITHM_PROPERTIES = [
    "independently authored examples",
    "row-order permutation",
    "addition deletion and change",
    "exceptional dynamic and partial inputs",
    "clean incremental equivalence",
    "causal input mutations",
    "convergence and resource bounds",
    "exact provenance closure",
]


def derived_call_target_fixture(
    direct_target: str = "fixture.alpha", *, partial: bool = False
) -> dict[str, dict[str, Any]]:
    source = (
        "def alpha() -> None:\n    pass\n\n"
        "def beta() -> None:\n    pass\n\n"
        "def gamma() -> None:\n    pass\n\n"
        "def caller(flag: bool) -> None:\n"
        "    alpha()\n"
        "    target = beta if flag else gamma\n"
        "    target()\n"
    )
    source_digest = bytes_b3(source.encode("utf-8"))
    workspace_id = "workspace:03030303030303030303030303030303"
    comparison_key = b"fixture_derived_call_graph.py"
    environment_inputs = {
        "provider_id": "pyrefly",
        "provider_release": "pyrefly@1.2.0#1933169ad8ee9e4d4114112eb56ef0811fb0a094",
        "relation_id": "provider.pyrefly.call_target.v1",
        "language": "python",
    }
    environment_digest = canonical_b3(environment_inputs)
    identity_context = {
        "workspace_id": workspace_id,
        "module_id": "entity:module:03030303030303030303030303030303",
        "analysis_context_id": cbef_analysis_context_digest(
            workspace_id=workspace_id,
            language_slug="python",
            environment_digest=environment_digest,
        ),
        "file_id": cbef_source_file_id(
            workspace_id=workspace_id, comparison_key=comparison_key
        ),
        "content_digest": source_digest,
    }
    names = ("alpha", "beta", "caller", "gamma")
    callable_rows = [
        [
            f"fixture.{name}",
            python_function_id(identity_context, [name]),
            "function",
            [name],
        ]
        for name in names
    ]
    callable_by_name = {
        row[0].removeprefix("fixture."): row[1] for row in callable_rows
    }

    tree = ast.parse(source)
    occurrence_drafts: list[tuple[int, int, str, int, str]] = []
    for function in tree.body:
        if not isinstance(function, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        calls = sorted(
            (node for node in ast.walk(function) if isinstance(node, ast.Call)),
            key=lambda node: node_byte_range(source, node.func),
        )
        for owner_ordinal, call in enumerate(calls):
            if not isinstance(call.func, ast.Name):
                raise TypeError("derived call fixture requires named callees")
            start_byte, end_byte = node_byte_range(source, call.func)
            occurrence_drafts.append(
                (start_byte, end_byte, function.name, owner_ordinal, call.func.id)
            )
    occurrence_rows: list[list[Any]] = []
    for occurrence_ordinal, draft in enumerate(sorted(occurrence_drafts)):
        start_byte, end_byte, owner_name, owner_ordinal, syntactic_callee = draft
        owner_id = callable_by_name[owner_name]
        call_site_id = python_call_site_id(
            identity_context,
            owner_id=owner_id,
            owner_relative_role="body.call.callee",
            owner_relative_ordinal=owner_ordinal,
            start_byte=start_byte,
            end_byte=end_byte,
        )
        occurrence_rows.append(
            [
                occurrence_ordinal,
                call_site_id,
                owner_id,
                "body.call.callee",
                owner_ordinal,
                identity_context["file_id"],
                source_digest,
                start_byte,
                end_byte,
                syntactic_callee,
            ]
        )
    occurrence_by_callee = {row[9]: row for row in occurrence_rows}
    direct_occurrence = occurrence_by_callee["alpha"]
    dynamic_occurrence = occurrence_by_callee["target"]
    rows = [
        [
            direct_occurrence[0],
            direct_occurrence[7],
            direct_occurrence[8],
            0,
            "function",
            direct_target,
            None,
            "resolved",
        ]
    ]
    if not partial:
        rows.extend(
            [
                [
                    dynamic_occurrence[0],
                    dynamic_occurrence[7],
                    dynamic_occurrence[8],
                    0,
                    "function",
                    "fixture.beta",
                    None,
                    "candidate",
                ],
                [
                    dynamic_occurrence[0],
                    dynamic_occurrence[7],
                    dynamic_occurrence[8],
                    1,
                    "function",
                    "fixture.gamma",
                    None,
                    "candidate",
                ],
            ]
        )
    return {
        "provider_call_targets": {
            "relation": "provider.pyrefly.call_target.v1",
            "columns": [
                "call_occurrence_ordinal",
                "start_byte",
                "end_byte",
                "target_ordinal",
                "callee_kind",
                "qualified_target",
                "class_name",
                "resolution_state",
            ],
            "rows": rows,
            "source_image": {
                "source_id": "py:derived-call-graph",
                **identity_context,
                "canonical_path_bytes_hex": comparison_key.hex(),
                "semantic_environment_inputs": environment_inputs,
                "semantic_environment_id": environment_digest.removeprefix("b3:"),
                "source_generation": 1,
                "bytes_utf8": source,
            },
            "provider_run_id": explicit_typed_fixture_bytes(0x35, 16),
            "coverage_terminal": {
                "requested_call_sites": 2,
                "completed_call_sites": 1 if partial else 2,
                "remainders": (
                    [
                        {
                            "call_site_id": dynamic_occurrence[1],
                            "reason": "PROVIDER_TARGET_SET_INCOMPLETE",
                            "retryable": True,
                        }
                    ]
                    if partial
                    else []
                ),
                "state": "partial" if partial else "complete",
            },
        },
        "canonical_call_occurrences": {
            "relation": "canonical.python_call_site.v1",
            "columns": [
                "call_occurrence_ordinal",
                "call_site_id",
                "owner_id",
                "owner_relative_role",
                "owner_relative_ordinal",
                "file_id",
                "content_digest",
                "start_byte",
                "end_byte",
                "syntactic_callee",
            ],
            "rows": occurrence_rows,
        },
        "canonical_callable_lookup": {
            "relation": "canonical.python_callable_lookup.v1",
            "columns": [
                "qualified_target",
                "callable_id",
                "kind",
                "qualified_lexical_path",
            ],
            "rows": callable_rows,
        },
    }


def derived_call_graph_rows(fixture: dict[str, dict[str, Any]]) -> list[list[Any]]:
    provider_targets = fixture["provider_call_targets"]
    occurrences = fixture["canonical_call_occurrences"]
    callables = fixture["canonical_callable_lookup"]
    occurrence_by_join = {(row[0], row[7], row[8]): row for row in occurrences["rows"]}
    callable_by_target = {row[0]: row[1] for row in callables["rows"]}
    source_image = provider_targets["source_image"]
    provenance = {
        "input_relation": provider_targets["relation"],
        "provider_run_id": provider_targets["provider_run_id"],
        "analysis_context_id": source_image["analysis_context_id"],
        "source_content_digest": source_image["content_digest"],
        "analysis_semantic_id": "analysis.common.call-graph.candidate-preserving.v1",
    }
    rows = []
    for native in provider_targets["rows"]:
        occurrence = occurrence_by_join[(native[0], native[1], native[2])]
        rows.append(
            [
                occurrence[1],
                occurrence[2],
                callable_by_target[native[5]],
                native[7],
                native[3],
                provenance,
            ]
        )
    return sorted(
        rows,
        key=lambda row: (row[0], row[4], row[2]),
    )


def derived_partial_call_graph_expectation(
    fixture: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    """Retain every known edge while representing only the uncovered tail as unknown."""

    provider_targets = fixture["provider_call_targets"]
    terminal = provider_targets["coverage_terminal"]
    remainders = terminal["remainders"]
    if (
        terminal["state"] != "partial"
        or terminal["requested_call_sites"] != 2
        or terminal["completed_call_sites"] != 1
        or len(remainders) != 1
    ):
        raise ValueError(
            "partial call-graph fixture does not close its coverage terminal"
        )
    remainder = remainders[0]
    source_image = provider_targets["source_image"]
    provenance = {
        "input_relation": provider_targets["relation"],
        "provider_run_id": provider_targets["provider_run_id"],
        "analysis_context_id": source_image["analysis_context_id"],
        "source_content_digest": source_image["content_digest"],
        "analysis_semantic_id": "analysis.common.call-graph.candidate-preserving.v1",
    }
    return {
        "known_facts": {
            "relation": "analysis.common_call_graph.v1",
            "columns": [
                "call_site_id",
                "caller_id",
                "callee_id",
                "resolution_state",
                "target_ordinal",
                "provenance",
            ],
            "rows": derived_call_graph_rows(fixture),
        },
        "unknown_remainder": {
            "relation": "analysis.common_unknown.v1",
            "columns": [
                "call_site_id",
                "family_id",
                "reason",
                "retryable",
                "provenance",
            ],
            "rows": [
                [
                    remainder["call_site_id"],
                    "common.call_graph",
                    remainder["reason"],
                    remainder["retryable"],
                    provenance,
                ]
            ],
        },
        "published_false_edges": [],
    }


def claim_003(base: dict[str, Any]) -> dict[str, Any]:
    census = [
        {
            "family_id": family_id,
            "normative_owner": source,
            "authority": "application_owned",
            "expected_disposition": "independent_expectation_required",
            "required_properties": ALGORITHM_PROPERTIES,
        }
        for family_id, source in DERIVED_FAMILIES
    ]
    definitions = [
        {
            "family_id": family_id,
            "normative_owner": source,
            "authority": "application_owned",
            "typed_input_contract": (
                "provider.pyrefly.call_target.v1 joined by exact source range to canonical call occurrences/owners and by qualified_target to canonical callable entities"
                if family_id == "common.call_graph"
                else "accepted immutable provider/canonical relations plus explicit typed inputs"
            ),
            "precision": (
                "candidate-preserving; partial target sets materialize common.unknown"
                if family_id == "common.call_graph"
                else "named and explicit; partial inputs propagate typed unknowns"
            ),
            "proof_contract": ALGORITHM_PROPERTIES,
        }
        for family_id, source in DERIVED_FAMILIES
    ]
    call_graph_fixture = derived_call_target_fixture()
    provider_targets = call_graph_fixture["provider_call_targets"]
    decoded_rows = derived_call_graph_rows(call_graph_fixture)
    base.update(
        {
            "subject": "A concrete application-owned call-graph analysis derives one typed row per admitted Pyrefly target candidate, preserves multi-candidate evidence, and materializes incomplete target coverage as an explicit unknown",
            "author_id": AUTHOR,
            "source_anchor": "GEN §72 Call graph; GEN §84 Explicit unknown-materialization rules; GEN §95 Algorithm validation",
            "governing_clauses": [
                "SUITE §5.1 Proof relations",
                "GEN §79 Derived graph relationship generation",
                "P27 Separate provider evidence from application meaning",
                "P30 Make derivation algorithms first-class",
            ],
            "complete_input_universe": {
                "closed": True,
                "inputs": {
                    "accepted_family_census": census,
                    "python_cfg_inputs": {
                        "normative_relations": [
                            "python.cfg_node",
                            "python.cfg_edge",
                            "python.evaluation_order",
                        ],
                        "authority": "application_owned",
                        "provider_inputs": [
                            "provider.ruff.ast_node",
                            "provider.ruff.semantic_edge",
                        ],
                    },
                    "rust_mir_cfg_inputs": {
                        "provider_native_relation": "provider.rustc.cfg_edge.v1",
                        "application_output_families": ["rust_mir.control_dependence"],
                        "authority_split": "provider-native CFG is input and never relabeled as application-derived output",
                    },
                    "rust_control_native_inputs": {
                        "relations": [
                            "provider.rustc.mir_block.v1",
                            "provider.rustc.mir_operand.v1",
                            "provider.rustc.mir_terminator.v1",
                            "provider.rustc.cfg_edge.v1",
                        ]
                    },
                    "provider_call_targets": provider_targets,
                    "canonical_call_occurrences": call_graph_fixture[
                        "canonical_call_occurrences"
                    ],
                    "canonical_callable_lookup": call_graph_fixture[
                        "canonical_callable_lookup"
                    ],
                    "analysis_definitions": definitions,
                    "precision_profiles": {
                        "required": "named direction lattice transfer convergence resource and unknown policy per family",
                        "partial_input": "typed unknown or explicit remainder",
                        "forbidden": "provider-authoritative labeling of application-derived output",
                    },
                    "authority_context": {
                        "suite": "codefabric-relational-data-fabric@2.1.0",
                        "derivation_rule": "GEN family owner headings identify the complete expectation scope; decoded rows are mechanically derived from exact typed inputs",
                    },
                    "coverage_terminals": [
                        {
                            "scope": "accepted_family_expectations",
                            "family_ids": [
                                family_id for family_id, _ in DERIVED_FAMILIES
                            ],
                            "state": "closed",
                        },
                        {
                            "family_id": "common.call_graph",
                            **provider_targets["coverage_terminal"],
                        },
                    ],
                },
            },
            "decoded_expectation": {
                "terminal": "pass",
                "relation": "analysis.common_call_graph.v1",
                "columns": [
                    "call_site_id",
                    "caller_id",
                    "callee_id",
                    "resolution_state",
                    "target_ordinal",
                    "provenance",
                ],
                "rows": decoded_rows,
                "coverage": "the decoded rows are derived by exact joins across one complete two-call-site Pyrefly-native fixture, canonical call occurrences/owners, and canonical callable lookup; the separate family census retains every accepted derived-family expectation without pretending all families executed",
            },
            "semantics": {
                "ordering": "call_site_id, target_ordinal, then callee_id ascending",
                "nulls": "resolved and candidate call-graph rows require caller, call-site, callee, ordinal, and provenance",
                "unknowns": "partial provider target coverage emits common.unknown for the affected call site and never implies no callee",
                "provenance": "each application row repeats the exact provider run, context, immutable source content, input relation, and analysis semantic identity; provider-native rows never author caller, callee, or call-site IDs",
            },
            "limitations": [
                "One bounded candidate-preserving call-graph example proves concrete analysis semantics; the complete family census records the remaining independent-expectation obligations without fixed producer counts or fabricated outputs."
            ],
        }
    )
    return base


def equivalence_source_image(generation: str, callee_name: str) -> dict[str, Any]:
    source = f"def {callee_name}():\n    pass\n\ndef e1():\n    {callee_name}()\n"
    workspace_id = "workspace:18181818181818181818181818181818"
    language = "python"
    semantic_environment_digest = bytes_b3(
        b"codefabric.wp33.python-equivalence-environment.v1"
    )
    return {
        "source_id": "source:python-equivalence-fixture",
        "workspace_id": workspace_id,
        "module_id": "entity:module:18181818181818181818181818181818",
        "file_id": "18181818181818181818181818181818",
        "language": language,
        "semantic_environment_digest": semantic_environment_digest,
        "analysis_context_id": cbef_analysis_context_id(
            workspace_id=workspace_id,
            language_slug=language,
            environment_digest=semantic_environment_digest,
        ),
        "source_generation": generation,
        "bytes_utf8": source,
        "content_digest": bytes_b3(source.encode("utf-8")),
    }


def equivalence_semantic_rows(image: dict[str, Any]) -> list[list[Any]]:
    source = image["bytes_utf8"]
    tree = ast.parse(source)
    functions = [
        node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    ]
    identity_context = {
        "workspace_id": image["workspace_id"],
        "module_id": image["module_id"],
        "analysis_context_id": image["analysis_context_id"],
        "file_id": image["file_id"],
        "content_digest": image["content_digest"],
    }
    function_ids = {
        function.name: python_function_id(identity_context, [function.name])
        for function in functions
    }
    rows: list[list[Any]] = [
        [entity_id, "function", name, None, None, None]
        for name, entity_id in function_ids.items()
    ]
    for function in functions:
        calls = sorted(
            (node for node in ast.walk(function) if isinstance(node, ast.Call)),
            key=lambda node: node_byte_range(source, node.func),
        )
        for owner_ordinal, call in enumerate(calls):
            if not isinstance(call.func, ast.Name) or call.func.id not in function_ids:
                raise ValueError("equivalence fixture requires a defined named callee")
            start_byte, end_byte = node_byte_range(source, call.func)
            caller_id = function_ids[function.name]
            callee_id = function_ids[call.func.id]
            call_site_id = python_call_site_id(
                identity_context,
                owner_id=caller_id,
                owner_relative_role="body.call.callee",
                owner_relative_ordinal=owner_ordinal,
                start_byte=start_byte,
                end_byte=end_byte,
            )
            fact_id = calls_fact_id(
                identity_context,
                caller_id=caller_id,
                callee_id=callee_id,
                call_site_id=call_site_id,
            )
            rows.append(
                [
                    call_site_id,
                    "call_site",
                    f"{start_byte}:{end_byte}",
                    caller_id,
                    None,
                    call_site_id,
                ]
            )
            rows.append(
                [
                    fact_id,
                    "calls",
                    None,
                    caller_id,
                    callee_id,
                    call_site_id,
                ]
            )
    return sorted(rows, key=lambda row: row[0])


def equivalence_provenance(inputs: dict[str, Any]) -> dict[str, Any]:
    pins = inputs["policy_proof_pins"]
    image = inputs["source_images"]["generation_g2"]
    derivation = inputs["change_derivation"]
    return {
        "source_generation": inputs["coverage_proof_inputs"]["source_generation"],
        "provider_release_vector": inputs["provider_release_vector"],
        "transformation_analysis_release": inputs["transformation_analysis_release"],
        "table_version_vector": inputs["exact_table_vector"],
        "policy_release": pins["policy_release"],
        "expectation_issuance": pins["expectation_issuance"],
        "proof_set": pins["proof_set"],
        "proof_state": pins["proof_state"],
        "source_content_digest": image["content_digest"],
        "analysis_context_id": image["analysis_context_id"],
        "provider_program_identity": derivation["provider_program_identity"],
        "transformation_program_identity": derivation[
            "transformation_program_identity"
        ],
    }


def equivalence_fixture_routes(inputs: dict[str, Any]) -> list[dict[str, Any]]:
    rows = [
        [*row, "current"]
        for row in equivalence_semantic_rows(inputs["source_images"]["generation_g2"])
    ]
    provenance = equivalence_provenance(inputs)
    return [
        {
            "route": route,
            "canonical_rows": rows,
            "public_rows": rows,
            "unknown_rows": [],
            "diagnostic_rows": [],
            "capability_state": "advertised",
            "provenance_closure": provenance,
        }
        for route in ("clean", "incremental")
    ]


def claim_018(base: dict[str, Any]) -> dict[str, Any]:
    inputs = base["complete_input_universe"]["inputs"]
    generation_g1 = equivalence_source_image("g1", "e2")
    generation_g2 = equivalence_source_image("g2", "e3")
    inputs["source_images"] = {
        "generation_g1": generation_g1,
        "generation_g2": generation_g2,
    }
    inputs["incremental_base_state"] = {
        "generation": "g1",
        "source_content_digest": generation_g1["content_digest"],
        "analysis_context_id": generation_g1["analysis_context_id"],
        "rows": [[*row, "current"] for row in equivalence_semantic_rows(generation_g1)],
    }
    inputs["change_derivation"] = {
        "authority": "application-owned source difference",
        "provider_program_identity": "independent.python-ast-provider-fixture.v2",
        "transformation_program_identity": "independent.canonical-entity-fixture.v2",
        "identity_key": "canonical_row_identity",
        "operations": ["insert", "delete", "replace"],
        "identity_contract": {
            "format": "CBEF-v1",
            "format_version": 1,
            "magic": "CFID",
            "byte_order": "big-endian",
            "digest": {
                "algorithm": "BLAKE3-256",
                "public_identity": "first 16 digest bytes as lowercase hexadecimal",
                "collision_diagnostic": "retain the full 32-byte digest",
            },
            "analysis_context": {
                "recipe": "ANALYSIS_CONTEXT",
                "domain_code": CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
                "fields": [
                    ["workspace_id", "ID"],
                    ["language_slug", "UTF8/ASCII_LOWER"],
                    ["environment_digest", "DIGEST"],
                ],
            },
            "function": {
                "recipe": "ENTITY",
                "domain_code": CBEF_ENTITY_DOMAIN_CODE,
                "kind_code": PYTHON_FUNCTION_KIND_CODE,
                "kind_slug": "function",
                "owner_field": "module_id",
                "semantic_key_encoding": "RFC8785 JCS UTF-8 bytes",
                "semantic_key_fields": [
                    "schema_version",
                    "module_id",
                    "qualified_lexical_path",
                    "kind",
                ],
            },
            "call_site": {
                "recipe": "ENTITY",
                "domain_code": CBEF_ENTITY_DOMAIN_CODE,
                "kind_code": PYTHON_CALL_SITE_KIND_CODE,
                "kind_slug": "call-site",
                "owner_field": "owner_id",
                "semantic_key_encoding": "RFC8785 JCS UTF-8 bytes",
                "semantic_key_fields": [
                    "schema_version",
                    "module_id",
                    "owner_relative_role",
                    "owner_relative_ordinal",
                    "file_id",
                    "content_digest",
                    "source_range",
                    "kind",
                ],
            },
            "calls_fact": {
                "recipe": "RELATION_FACT",
                "domain_code": CBEF_RELATION_FACT_DOMAIN_CODE,
                "relation_kind_code": CALLS_RELATION_KIND_CODE,
                "kind_slug": "calls",
                "subject_field": "caller_id",
                "object_field": "callee_id",
                "role": {
                    "type": "TAGGED_UNION",
                    "variant": 1,
                    "member_type": "UTF8",
                    "encoding": "RFC8785 JCS UTF-8 string",
                    "fields": ["schema_version", "module_id", "call_site_id"],
                },
                "outer_context_fields": [
                    "workspace_id",
                    "analysis_context_id",
                ],
            },
        },
    }
    for route in inputs["route_definitions"].values():
        typed_inputs = route["typed_inputs"]
        if "change_derivation.identity_contract" not in typed_inputs:
            typed_inputs.append("change_derivation.identity_contract")

    rows = [[*row, "current"] for row in equivalence_semantic_rows(generation_g2)]
    provenance = equivalence_provenance(inputs)
    base["decoded_expectation"] = {
        "terminal": "pass",
        "relation": "proof.rebuild_equivalence",
        "columns": [
            "route",
            "canonical_rows",
            "public_rows",
            "unknown_rows",
            "diagnostic_rows",
            "capability_state",
            "provenance_closure",
        ],
        "rows": [
            ["clean", rows, rows, [], [], "advertised", provenance],
            ["incremental", rows, rows, [], [], "advertised", provenance],
        ],
        "coverage": "both independently defined routes consume the same immutable Python source bytes through explicit workspace/module/context/provider/program inputs and cover identical function entities, content-bound call-site occurrences, call-site-bound calls facts, canonical/public rows, unknowns, diagnostics, capability, and exact provenance",
    }
    base["semantics"] = {
        "ordering": "route name ascending; within each route canonical primary key ascending after bag comparison",
        "nulls": "identity, generation, capability, policy, proof, and provenance pins are required in both routes; calls facts require call_site_id",
        "unknowns": "missing provider/analysis/coverage/policy/proof closure materializes unknown route state and blocks equivalence",
        "provenance": "function IDs bind workspace/module/analysis context/qualified lexical path/kind; call-site IDs additionally bind owner-relative role/ordinal and exact file/content/range; calls facts expose and bind call_site_id",
    }
    base["limitations"] = [
        "The two-generation Python corpus discriminates source parsing, deletion, insertion, replacement, and anonymous-occurrence identity but is not a performance workload or a claim about every language construct.",
        "This bounded direct-call oracle rejects unresolved callees instead of conflating call syntax with a semantic target entity.",
    ]
    return base


def replace_identity_everywhere(value: Any, old: str, new: str) -> None:
    if isinstance(value, dict):
        for key, item in list(value.items()):
            if key == old:
                value[new] = value.pop(key)
                item = value[new]
            if item == old:
                value[key if key != old else new] = new
            else:
                replace_identity_everywhere(item, old, new)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            if item == old:
                value[index] = new
            else:
                replace_identity_everywhere(item, old, new)


def _query_provenance(response: dict[str, Any]) -> dict[str, Any]:
    return copy.deepcopy(response["query_results"][0]["provenance"])


def _public_entity_record(row: dict[str, Any]) -> dict[str, Any]:
    """Project one admitted query row into its public entity record."""

    record = copy.deepcopy(row)
    record.pop("alias", None)
    return record


def _find_entity_selection(
    inputs: dict[str, Any], block: dict[str, Any]
) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any]]:
    """Independently author the exact entity selection from declared inputs."""

    program = inputs["program_binding"]
    looking_for = block["looking_for"]
    semantic_kind = program["description_resolution"][looking_for]
    rows = inputs["admitted_relations"]["entity_rows"]
    selected = [
        _public_entity_record(row)
        for row in rows
        if row["workspace_id"] in block["within"]
        and row["semantic_kind"] == semantic_kind
    ]
    selected.sort(
        key=lambda row: (
            row.get("source_reference", {}).get("byte_safe_path", ""),
            row.get("source_reference", {}).get("start_byte", -1),
            row["semantic_kind"],
            row.get("qualified_name", ""),
            row["entity_id"],
        )
    )
    representations = {row["representation"] for row in selected}
    if len(representations) != 1:
        raise ValueError("entity description does not resolve to one representation")
    coverage = inputs["producer_coverage"]
    if set(coverage["covered_entity_ids"]) != {row["entity_id"] for row in rows}:
        raise ValueError("entity selection coverage does not close admitted rows")
    resolved = {
        "looking_for": looking_for,
        "representation": representations.pop(),
        "semantic_kind": semantic_kind,
    }
    result_coverage = {
        "state": coverage["state"],
        "family": coverage["family"],
        "scope": coverage["scope"],
        "completed_inputs": len(coverage["covered_entity_ids"]),
    }
    return selected, resolved, result_coverage


def _selected_follow_edges(
    inputs: dict[str, Any], block: dict[str, Any]
) -> list[dict[str, Any]]:
    selected = [
        copy.deepcopy(edge)
        for edge in inputs["admitted_relations"]["call_edges"]
        if edge["statement"]["subject"] in block["starting_from"]
        and edge["statement"]["predicate"] == block["relationship"]
    ]
    selected.sort(key=lambda edge: (edge["statement"]["object"], edge["fact_id"]))
    return selected


def _canonical_shortest_witness(
    edges: list[dict[str, Any]],
    *,
    start: str,
    target: str,
    families: list[str],
    maximum_length: int,
) -> tuple[list[str], list[str]]:
    """Author the shortest path and deterministic equal-length tie break."""

    adjacency: dict[str, list[tuple[str, str]]] = {}
    for edge in edges:
        statement = edge["statement"]
        if statement["predicate"] in families:
            adjacency.setdefault(statement["subject"], []).append(
                (statement["object"], edge["fact_id"])
            )
    for values in adjacency.values():
        values.sort()

    frontier: list[tuple[list[str], list[str]]] = [([start], [])]
    for _ in range(maximum_length + 1):
        matches = [value for value in frontier if value[0][-1] == target]
        if matches:
            matches.sort(key=lambda value: (value[1], value[0]))
            return matches[0]
        next_frontier: list[tuple[list[str], list[str]]] = []
        for entity_ids, fact_ids in frontier:
            for successor, fact_id in adjacency.get(entity_ids[-1], []):
                if successor not in entity_ids:
                    next_frontier.append(
                        ([*entity_ids, successor], [*fact_ids, fact_id])
                    )
        frontier = next_frontier
        if not frontier:
            break
    raise ValueError("the bounded admitted graph has no connecting path")


def _typed_pattern_matches(
    inputs: dict[str, Any], block: dict[str, Any], *, indeterminate: bool = False
) -> list[dict[str, Any]]:
    """Execute the bounded typed-node, positive-edge, and negation fixture."""

    pattern = block["pattern"]
    relations = inputs["admitted_relations"]
    rows = relations["entities"]["rows"]
    edges = relations["call_edges"]
    candidates: dict[str, list[dict[str, Any]]] = {}
    for node in pattern["nodes"]:
        candidates[node["binding"]] = [
            row
            for row in rows
            if row.get("module_id") == node["module_id"]
            and row["semantic_kind"] == node["semantic_kind"]
            and row.get("qualified_name", "").rsplit(".", 1)[-1] == node["name"]
        ]

    bindings: list[dict[str, dict[str, Any]]] = [{}]
    for variable in sorted(candidates):
        bindings = [
            {**binding, variable: candidate}
            for binding in bindings
            for candidate in candidates[variable]
        ]
    for fact in pattern["facts"]:
        bindings = [
            binding
            for binding in bindings
            if any(
                edge["statement"]
                == {
                    "subject": binding[fact["subject_binding"]]["entity_id"],
                    "predicate": fact["relationship"],
                    "object": binding[fact["object_binding"]]["entity_id"],
                }
                for edge in edges
            )
        ]

    results: list[dict[str, Any]] = []
    for binding in bindings:
        support_ids = sorted(
            edge["fact_id"]
            for fact in pattern["facts"]
            for edge in edges
            if edge["statement"]
            == {
                "subject": binding[fact["subject_binding"]]["entity_id"],
                "predicate": fact["relationship"],
                "object": binding[fact["object_binding"]]["entity_id"],
            }
        )
        negation_evidence: list[dict[str, Any]] = []
        rejected = False
        for negation in pattern["scoped_negation"]:
            subject_id = binding[negation["subject_binding"]]["entity_id"]
            has_edge = any(
                edge["analysis_context_id"] == negation["analysis_context_id"]
                and edge["statement"]["subject"] == subject_id
                and edge["statement"]["predicate"] == negation["relationship"]
                for edge in edges
            )
            if has_edge and not indeterminate:
                rejected = True
                break
            negation_evidence.append(
                {
                    "subject_binding": negation["subject_binding"],
                    "subject_entity_id": subject_id,
                    "relationship": negation["relationship"],
                    "direction": negation["direction"],
                    "owner_scope": negation["owner_scope"],
                    "analysis_context_id": negation["analysis_context_id"],
                    "coverage_witness": inputs["producer_coverage"][
                        "negative_proof_universe_id"
                    ],
                    "state": ("INDETERMINATE" if indeterminate else "PROVED_ABSENT"),
                }
            )
        if rejected:
            continue
        results.append(
            {
                "matched_branch": "primary",
                "binding_state": "INDETERMINATE" if indeterminate else "MATCH",
                "bindings": {
                    variable: {
                        "binding_type": "entity:function",
                        "entity_id": record["entity_id"],
                        "semantic_kind": record["semantic_kind"],
                    }
                    for variable, record in sorted(binding.items())
                },
                "supporting_fact_ids": support_ids,
                "scoped_negation": negation_evidence,
            }
        )
    results.sort(
        key=lambda result: tuple(
            value["entity_id"] for value in result["bindings"].values()
        )
    )
    return results


def _producer_result_envelope(
    *,
    query_id: str,
    input_relation: str,
    entity_ids: list[str],
    compatibility: dict[str, Any],
    provenance: dict[str, Any],
) -> dict[str, Any]:
    return {
        "query_id": query_id,
        "request": "find code entities",
        "execution_state": "COMPLETE",
        "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE",
        "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED",
        "dependency_state": "READY",
        "resolved_semantics": {
            "looking_for": "function declarations",
            "producer_input_relation": input_relation,
            "compatibility_dimensions": copy.deepcopy(compatibility),
        },
        "result_role": "entities",
        "entity_ids": sorted(entity_ids),
        "fact_ids": [],
        "path_ids": [],
        "group_ids": [],
        "source_context_ids": [],
        "bindings": [],
        "coverage": {
            "state": "COMPLETE",
            "producer_query_id": query_id,
            "producer_input_relation": input_relation,
            "completed_entities": len(entity_ids),
        },
        "provenance": copy.deepcopy(provenance),
        "errors": [],
        "notices": [],
    }


def _producer_results_from_inputs(
    producer_inputs: dict[str, dict[str, Any]], provenance: dict[str, Any]
) -> dict[str, dict[str, Any]]:
    results: dict[str, dict[str, Any]] = {}
    for query_id, source in sorted(producer_inputs.items()):
        compatibility = {
            key: source[key]
            for key in (
                "workspace_id",
                "analysis_context_id",
                "representation_layer",
                "certainty_class",
                "semantic_role",
            )
        }
        results[query_id] = _producer_result_envelope(
            query_id=query_id,
            input_relation=source["relation_id"],
            entity_ids=source["rows"],
            compatibility=compatibility,
            provenance=provenance,
        )
    return results


def _cbef_typed(type_code: int, payload: bytes) -> bytes:
    return type_code.to_bytes(1, "big") + len(payload).to_bytes(4, "big") + payload


def _public_domain_id_bytes(value: object, domain: str) -> bytes:
    text = str(value)
    prefix, separator, _ = text.rpartition(":")
    if not separator or not prefix.startswith(f"{domain}:"):
        raise ValueError(f"identity is outside the {domain} domain")
    return public_id_bytes(text, prefix)


def _cbef_sequence_payload(
    members: list[tuple[int, bytes]], *, canonical_set: bool
) -> bytes:
    encoded = [_cbef_typed(type_code, payload) for type_code, payload in members]
    if canonical_set:
        encoded.sort()
        if len(set(encoded)) != len(encoded):
            raise ValueError("CBEF-v1 SET members must be unique")
    return len(encoded).to_bytes(4, "big") + b"".join(
        len(member).to_bytes(4, "big") + member for member in encoded
    )


def _cbef_map_payload(
    entries: list[tuple[tuple[int, bytes], tuple[int, bytes]]],
) -> bytes:
    encoded = [(_cbef_typed(*key), _cbef_typed(*value)) for key, value in entries]
    encoded.sort(key=lambda entry: entry[0])
    if len({key for key, _ in encoded}) != len(encoded):
        raise ValueError("CBEF-v1 MAP keys must be unique")
    return len(encoded).to_bytes(4, "big") + b"".join(
        len(key).to_bytes(4, "big") + key + len(value).to_bytes(4, "big") + value
        for key, value in encoded
    )


def _cbef_utf8(value: object, *, ascii_lower: bool = False) -> bytes:
    text = str(value)
    if ascii_lower:
        if not text.isascii():
            raise ValueError("CBEF-v1 ASCII_LOWER field must be ASCII")
        text = text.lower()
    return text.encode("utf-8")


def _cbef_scalar_union(value: Any) -> tuple[bytes, dict[str, Any]]:
    if isinstance(value, bool):
        variant = 6
        member_type = "BOOLEAN"
        payload = bytes([int(value)])
    elif isinstance(value, int):
        if value >= 0:
            variant = 4
            member_type = "UNSIGNED"
            payload = value.to_bytes(8, "big")
        else:
            variant = 5
            member_type = "SIGNED"
            payload = value.to_bytes(8, "big", signed=True)
    elif isinstance(value, str) and value:
        variant = 2
        member_type = "UTF8"
        payload = value.encode("utf-8")
    else:
        raise ValueError("objective group key is not a supported scalar")
    typed = _cbef_typed(variant, payload)
    tagged = variant.to_bytes(2, "big") + len(typed).to_bytes(4, "big") + typed
    return tagged, {"variant": variant, "member_type": member_type, "value": value}


def _cbef_recipe_evidence(
    *,
    domain_code: int,
    domain_name: str,
    output_prefix: str,
    fields: list[tuple[int, str, int, str, bytes, Any]],
    excluded: list[str],
) -> dict[str, Any]:
    preimage = cbef_record(
        domain_code,
        [(tag, type_code, payload) for tag, _, type_code, _, payload, _ in fields],
    )
    digest = blake3.blake3(preimage).digest()
    return {
        "recipe_version": "CBEF-v1",
        "contract": {
            "artifact_id": "codefabric.identity.cbef-v1",
            "version": "1.1",
        },
        "record_domain": {"code": domain_code, "name": domain_name},
        "fields": [
            {
                "tag": tag,
                "name": name,
                "type_code": {"code": type_code, "name": type_name},
                "value": copy.deepcopy(value),
                "payload_hex": payload.hex(),
            }
            for tag, name, type_code, type_name, payload, value in fields
        ],
        "canonical_preimage_hex": preimage.hex(),
        "digest": {
            "algorithm": "BLAKE3-256",
            "mode": "unkeyed",
            "full_digest_hex": digest.hex(),
            "id_derivation": "first 16 digest bytes",
            "text_encoding": "lowercase hexadecimal",
        },
        "excluded": excluded,
        "output_id": f"{output_prefix}:{digest[:16].hex()}",
    }


def _path_result_recipe(
    *,
    workspace_id: object,
    analysis_context_id: object,
    fabric_epoch_id: object,
    policy_identity: object,
    ordered_entity_ids: list[str],
    ordered_fact_ids: list[str],
) -> dict[str, Any]:
    """Author the closed CBEF-v1.1 PATH_RESULT domain-18 identity evidence."""

    entity_payload = _cbef_sequence_payload(
        [(7, _public_domain_id_bytes(value, "entity")) for value in ordered_entity_ids],
        canonical_set=False,
    )
    fact_payload = _cbef_sequence_payload(
        [(7, _public_domain_id_bytes(value, "fact")) for value in ordered_fact_ids],
        canonical_set=False,
    )
    return _cbef_recipe_evidence(
        domain_code=CBEF_PATH_RESULT_DOMAIN_CODE,
        domain_name="PATH_RESULT",
        output_prefix="path",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(workspace_id, "workspace"),
                workspace_id,
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                analysis_context_id_bytes(analysis_context_id),
                analysis_context_id,
            ),
            (
                3,
                "fabric_epoch_id",
                7,
                "ID",
                public_id_bytes(fabric_epoch_id, "fabric-epoch"),
                fabric_epoch_id,
            ),
            (
                4,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8(policy_identity),
                policy_identity,
            ),
            (
                5,
                "ordered_entity_ids",
                9,
                "ORDERED_LIST",
                entity_payload,
                ordered_entity_ids,
            ),
            (
                6,
                "ordered_fact_ids",
                9,
                "ORDERED_LIST",
                fact_payload,
                ordered_fact_ids,
            ),
        ],
        excluded=["path length", "witness provenance", "certainty summary"],
    )


def _query_source_context_recipe(
    *,
    workspace_id: object,
    analysis_context_id: object,
    snapshot_id: object,
    entity_id: object,
    source_file_id: object,
    source_generation: int,
    source_content_digest: object,
    delivered_start_byte: int,
    delivered_end_byte: int,
    delivered_content_digest: object,
    disclosure_scope_id: object,
    policy_identity: object,
    context_kind: object,
) -> dict[str, Any]:
    """Author the closed CBEF-v1.1 QUERY_SOURCE_CONTEXT identity evidence."""

    if (
        not isinstance(source_generation, int)
        or isinstance(source_generation, bool)
        or not 0 <= source_generation <= 0xFFFF_FFFF_FFFF_FFFF
        or not isinstance(delivered_start_byte, int)
        or isinstance(delivered_start_byte, bool)
        or not isinstance(delivered_end_byte, int)
        or isinstance(delivered_end_byte, bool)
        or not 0 <= delivered_start_byte <= delivered_end_byte <= 0xFFFF_FFFF_FFFF_FFFF
    ):
        raise ValueError("source-context unsigned fields are invalid")
    canonical_context_kind = str(context_kind)
    if not canonical_context_kind.isascii():
        raise ValueError("source-context kind must be ASCII")
    canonical_context_kind = canonical_context_kind.lower()
    return _cbef_recipe_evidence(
        domain_code=CBEF_QUERY_SOURCE_CONTEXT_DOMAIN_CODE,
        domain_name="QUERY_SOURCE_CONTEXT",
        output_prefix="context",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(workspace_id, "workspace"),
                workspace_id,
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                analysis_context_id_bytes(analysis_context_id),
                analysis_context_id,
            ),
            (
                3,
                "snapshot_id",
                7,
                "ID",
                public_id_bytes(snapshot_id, "snapshot"),
                snapshot_id,
            ),
            (
                4,
                "entity_id",
                7,
                "ID",
                _public_domain_id_bytes(entity_id, "entity"),
                entity_id,
            ),
            (
                5,
                "source_file_id",
                7,
                "ID",
                public_id_bytes(source_file_id, "file"),
                source_file_id,
            ),
            (
                6,
                "source_generation",
                4,
                "UNSIGNED",
                source_generation.to_bytes(8, "big"),
                source_generation,
            ),
            (
                7,
                "source_content_digest",
                8,
                "DIGEST",
                digest_bytes(source_content_digest, "b3"),
                source_content_digest,
            ),
            (
                8,
                "delivered_start_byte",
                4,
                "UNSIGNED",
                delivered_start_byte.to_bytes(8, "big"),
                delivered_start_byte,
            ),
            (
                9,
                "delivered_end_byte",
                4,
                "UNSIGNED",
                delivered_end_byte.to_bytes(8, "big"),
                delivered_end_byte,
            ),
            (
                10,
                "delivered_content_digest",
                8,
                "DIGEST",
                digest_bytes(delivered_content_digest, "b3"),
                delivered_content_digest,
            ),
            (
                11,
                "disclosure_scope_id",
                7,
                "ID",
                public_id_bytes(disclosure_scope_id, "access-scope"),
                disclosure_scope_id,
            ),
            (
                12,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8(policy_identity),
                policy_identity,
            ),
            (
                13,
                "context_kind",
                2,
                "UTF8",
                _cbef_utf8(canonical_context_kind, ascii_lower=True),
                canonical_context_kind,
            ),
        ],
        excluded=["omitted byte count", "truncation state"],
    )


def _property_kind_code(relations: dict[str, Any], property_kind: str) -> int:
    registry = relations["property_kind_registry"]
    if set(registry) != {"relation_id", "closed_universe", "rows"} or (
        registry["relation_id"] != "input.property_kind"
        or registry["closed_universe"] is not True
    ):
        raise ValueError("input.property_kind registry is not closed")
    rows = registry["rows"]
    names: set[str] = set()
    codes: set[int] = set()
    allocation: dict[str, int] = {}
    for row in rows:
        if set(row) != {"property_kind", "property_kind_code"}:
            raise ValueError("input.property_kind row shape differs")
        name = str(row["property_kind"])
        code = row["property_kind_code"]
        if (
            not name
            or not isinstance(code, int)
            or isinstance(code, bool)
            or not 0 < code <= 0xFFFF
            or name in names
            or code in codes
        ):
            raise ValueError("input.property_kind allocation is invalid or duplicate")
        names.add(name)
        codes.add(code)
        allocation[name] = code
    if property_kind not in allocation:
        raise ValueError("property kind is outside the closed input registry")
    return allocation[property_kind]


def _native_kind_fact_recipe(
    fact: dict[str, Any], property_kind_code: int
) -> dict[str, Any]:
    native_kind = str(fact["statement"]["object"])
    value_payload = _cbef_utf8(native_kind)
    typed_value = _cbef_typed(2, value_payload)
    tagged_value = (
        (50).to_bytes(2, "big") + len(typed_value).to_bytes(4, "big") + typed_value
    )
    return _cbef_recipe_evidence(
        domain_code=10,
        domain_name="PROPERTY_FACT",
        output_prefix="fact:native-kind",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(fact["workspace_id"], "workspace"),
                fact["workspace_id"],
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                analysis_context_id_bytes(fact["analysis_context_id"]),
                fact["analysis_context_id"],
            ),
            (
                3,
                "property_kind_code",
                4,
                "UNSIGNED",
                property_kind_code.to_bytes(2, "big"),
                property_kind_code,
            ),
            (
                4,
                "subject_entity_id",
                7,
                "ID",
                _public_domain_id_bytes(fact["statement"]["subject"], "entity"),
                fact["statement"]["subject"],
            ),
            (
                5,
                "canonical_value",
                12,
                "TAGGED_UNION",
                tagged_value,
                {"variant": 50, "member_type": "UTF8", "value": native_kind},
            ),
        ],
        excluded=[
            "input-set identity",
            "support ids",
            "source and producer provenance",
            "policy identity",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
    )


def _objective_input_set_recipe(
    rows: list[dict[str, Any]], policy_identity: str, coverage_state: str
) -> dict[str, Any]:
    workspace_ids = {str(row["workspace_id"]) for row in rows}
    context_ids = sorted({str(row["analysis_context_id"]) for row in rows})
    producer_ids = sorted({str(row["producer"]["producer_id"]) for row in rows})
    if len(workspace_ids) != 1 or not context_ids or not producer_ids:
        raise ValueError("objective input set is empty or crosses workspaces")
    if coverage_state not in {
        "complete",
        "partial",
        "indeterminate",
        "unavailable",
    }:
        raise ValueError("objective coverage state is outside its closed vocabulary")
    fact_ids = sorted(str(row["fact_id"]) for row in rows)
    context_set_payload = _cbef_sequence_payload(
        [(7, analysis_context_id_bytes(value)) for value in context_ids],
        canonical_set=True,
    )
    fact_set_payload = _cbef_sequence_payload(
        [(7, public_id_bytes(fact_id, "fact:native-kind")) for fact_id in fact_ids],
        canonical_set=True,
    )
    producer_set_payload = _cbef_sequence_payload(
        [(2, _cbef_utf8(value)) for value in producer_ids], canonical_set=True
    )
    return _cbef_recipe_evidence(
        domain_code=19,
        domain_name="OBJECTIVE_INPUT_SET",
        output_prefix="input-set",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(rows[0]["workspace_id"], "workspace"),
                rows[0]["workspace_id"],
            ),
            (
                2,
                "analysis_context_ids",
                10,
                "SET",
                context_set_payload,
                context_ids,
            ),
            (3, "fact_ids", 10, "SET", fact_set_payload, fact_ids),
            (
                4,
                "producer_identities",
                10,
                "SET",
                producer_set_payload,
                producer_ids,
            ),
            (
                5,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8(policy_identity),
                policy_identity,
            ),
            (
                6,
                "coverage_state",
                2,
                "UTF8",
                _cbef_utf8(coverage_state, ascii_lower=True),
                coverage_state,
            ),
        ],
        excluded=[
            "fact ordering",
            "support ids",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
    )


def _retrieve_fact_input_set_recipe(
    relations: dict[str, Any], policy_identity: str
) -> dict[str, Any]:
    """Bind the exact known retrieval inputs and incomplete-family producers."""

    rows = relations["fact_rows"]
    coverage_rows = relations["coverage_rows"]
    if not rows or not coverage_rows:
        raise ValueError("retrieve-facts input set is empty")
    workspace_ids = {str(row["workspace_id"]) for row in rows}
    context_ids = sorted({str(row["analysis_context_id"]) for row in rows})
    if len(workspace_ids) != 1 or not context_ids:
        raise ValueError("retrieve-facts input set crosses a CBEF identity boundary")
    states = {str(row["state"]) for row in coverage_rows}
    if not states <= {"COMPLETE", "PARTIAL", "UNAVAILABLE"}:
        raise ValueError(
            "retrieve-facts coverage state is outside its closed vocabulary"
        )
    if states == {"COMPLETE"}:
        coverage_state = "complete"
    elif "UNAVAILABLE" in states:
        coverage_state = "indeterminate"
    else:
        coverage_state = "partial"
    fact_ids = sorted(str(row["fact_id"]) for row in rows)
    producer_ids = sorted(
        {
            *(str(row["producer"]["producer_id"]) for row in rows),
            *(
                f"coverage:{row['family']}"
                for row in coverage_rows
                if row["state"] != "COMPLETE"
            ),
        }
    )
    context_set_payload = _cbef_sequence_payload(
        [(7, analysis_context_id_bytes(value)) for value in context_ids],
        canonical_set=True,
    )
    fact_set_payload = _cbef_sequence_payload(
        [(7, _public_domain_id_bytes(value, "fact")) for value in fact_ids],
        canonical_set=True,
    )
    producer_set_payload = _cbef_sequence_payload(
        [(2, _cbef_utf8(value)) for value in producer_ids], canonical_set=True
    )
    return _cbef_recipe_evidence(
        domain_code=19,
        domain_name="OBJECTIVE_INPUT_SET",
        output_prefix="input-set",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(rows[0]["workspace_id"], "workspace"),
                rows[0]["workspace_id"],
            ),
            (
                2,
                "analysis_context_ids",
                10,
                "SET",
                context_set_payload,
                context_ids,
            ),
            (3, "fact_ids", 10, "SET", fact_set_payload, fact_ids),
            (
                4,
                "producer_identities",
                10,
                "SET",
                producer_set_payload,
                producer_ids,
            ),
            (
                5,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8(policy_identity),
                policy_identity,
            ),
            (
                6,
                "coverage_state",
                2,
                "UTF8",
                _cbef_utf8(coverage_state, ascii_lower=True),
                coverage_state,
            ),
        ],
        excluded=[
            "fact ordering",
            "support ids",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
    )


def _normalize_objective_relations(
    relations: dict[str, Any], policy_identity: str
) -> None:
    rows = relations["syntax_rows"]
    for row in rows:
        property_kind_code = _property_kind_code(
            relations, str(row["statement"]["predicate"])
        )
        row["property_kind_code"] = property_kind_code
        recipe = _native_kind_fact_recipe(row, property_kind_code)
        row["fact_id"] = recipe["output_id"]
        row["identity_recipe"] = recipe
    input_set_recipe = _objective_input_set_recipe(
        rows, policy_identity, str(relations["coverage_state"])
    )
    input_set_id = input_set_recipe["output_id"]
    for row in rows:
        row["direct_provenance"]["input_set_id"] = input_set_id
    relations["input_set_identity"] = input_set_recipe


def _objective_groups(relations: dict[str, Any]) -> dict[str, dict[str, Any]]:
    rows = relations["syntax_rows"]
    input_set_id = relations["input_set_identity"]["output_id"]
    grouped: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        grouped.setdefault(str(row["statement"]["object"]), []).append(row)
    result: dict[str, dict[str, Any]] = {}
    for native_kind, members in sorted(grouped.items()):
        if any(
            member["workspace_id"] != members[0]["workspace_id"]
            or member["analysis_context_id"] != members[0]["analysis_context_id"]
            or member["producer"]["producer_id"]
            != members[0]["producer"]["producer_id"]
            for member in members
        ):
            raise ValueError("objective group crosses a CBEF identity boundary")
        group = {
            "group_key": {"native_kind": native_kind},
            "objective_value": {"measure": "count", "value": len(members)},
            "input_set_id": input_set_id,
            "grouping": ["native_kind"],
            "aggregation": "count",
            "producer_id": members[0]["producer"]["producer_id"],
            "precision": "source-syntax-exact",
            "completeness": "COMPLETE",
            "support_fact_ids": sorted(member["fact_id"] for member in members),
        }
        grouping_payload = _cbef_sequence_payload(
            [(2, _cbef_utf8(value)) for value in group["grouping"]],
            canonical_set=False,
        )
        tagged_key = {
            key: _cbef_scalar_union(value) for key, value in group["group_key"].items()
        }
        key_payload = _cbef_map_payload(
            [
                ((2, _cbef_utf8(key)), (12, payload))
                for key, (payload, _) in tagged_key.items()
            ]
        )
        group_key_evidence = {
            key: evidence for key, (_, evidence) in tagged_key.items()
        }
        recipe = _cbef_recipe_evidence(
            domain_code=20,
            domain_name="OBJECTIVE_GROUP",
            output_prefix="group",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    public_id_bytes(members[0]["workspace_id"], "workspace"),
                    members[0]["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    analysis_context_id_bytes(members[0]["analysis_context_id"]),
                    members[0]["analysis_context_id"],
                ),
                (
                    3,
                    "input_set_id",
                    7,
                    "ID",
                    public_id_bytes(input_set_id, "input-set"),
                    input_set_id,
                ),
                (
                    4,
                    "grouping_dimensions",
                    9,
                    "ORDERED_LIST",
                    grouping_payload,
                    group["grouping"],
                ),
                (
                    5,
                    "canonical_group_key",
                    11,
                    "MAP",
                    key_payload,
                    group_key_evidence,
                ),
                (
                    6,
                    "aggregate_function",
                    2,
                    "UTF8",
                    _cbef_utf8(group["aggregation"], ascii_lower=True),
                    group["aggregation"],
                ),
                (
                    7,
                    "measure",
                    2,
                    "UTF8",
                    _cbef_utf8(group["objective_value"]["measure"]),
                    group["objective_value"]["measure"],
                ),
                (
                    8,
                    "producer_identity",
                    2,
                    "UTF8",
                    _cbef_utf8(group["producer_id"]),
                    group["producer_id"],
                ),
            ],
            excluded=[
                "support_fact_ids",
                "group members",
                "member count",
                "objective count value",
                "mutable coverage counters",
            ],
        )
        group["group_id"] = recipe["output_id"]
        group["identity_recipe"] = recipe
        result[group["group_id"]] = group
    return result


def _query_property_fact_recipe(
    fact: dict[str, Any],
    *,
    property_kind_code: int,
    output_prefix: str,
    canonical_value: Any,
    excluded: list[str],
) -> dict[str, Any]:
    """Build one CBEF-v1.1 PROPERTY_FACT from an admitted typed proposition."""

    scalar = (
        canonical_value
        if isinstance(canonical_value, str)
        else rfc8785.dumps(canonical_value).decode("utf-8")
    )
    if not scalar:
        raise ValueError("query property fact canonical scalar is empty")
    typed_value = _cbef_typed(2, scalar.encode("utf-8"))
    tagged_value = (
        (50).to_bytes(2, "big") + len(typed_value).to_bytes(4, "big") + typed_value
    )
    return _cbef_recipe_evidence(
        domain_code=10,
        domain_name="PROPERTY_FACT",
        output_prefix=output_prefix,
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(fact["workspace_id"], "workspace"),
                fact["workspace_id"],
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                analysis_context_id_bytes(fact["analysis_context_id"]),
                fact["analysis_context_id"],
            ),
            (
                3,
                "property_kind_code",
                4,
                "UNSIGNED",
                property_kind_code.to_bytes(2, "big"),
                property_kind_code,
            ),
            (
                4,
                "subject_entity_id",
                7,
                "ID",
                _public_domain_id_bytes(fact["statement"]["subject"], "entity"),
                fact["statement"]["subject"],
            ),
            (
                5,
                "canonical_value",
                12,
                "TAGGED_UNION",
                tagged_value,
                {"variant": 50, "member_type": "UTF8", "value": scalar},
            ),
        ],
        excluded=excluded,
    )


def update_query_claim(base: dict[str, Any]) -> dict[str, Any]:
    base["author_id"] = AUTHOR
    inputs = base["complete_input_universe"]["inputs"]
    epoch = inputs["pinned_epoch"]
    epoch["expectation_issuance"] = ISSUANCE_ID
    epoch["public_snapshot_projection"]["proof_receipt"] = ISSUANCE_ID
    response = base["decoded_expectation"]["rows"][0][0]
    response["snapshot"]["proof_receipt"] = ISSUANCE_ID
    response["query_results"][0]["provenance"]["expectation_issuance"] = ISSUANCE_ID
    claim_id = base["claim_id"]
    if claim_id == "RFV3-CLAIM-004":
        block = inputs["request_envelope"]["decoded"]["queries"][0]
        rows = inputs["admitted_relations"]["entity_rows"]
        inputs["program_binding"].pop("phrase_resolution", None)
        inputs["producer_coverage"] = {
            "state": "COMPLETE",
            "family": "entity_kind",
            "scope": inputs["request_envelope"]["decoded"]["scope"]["workspace_id"],
            "covered_entity_ids": sorted(row["entity_id"] for row in rows),
        }
        selected, resolved, coverage = _find_entity_selection(inputs, block)
        selected_by_id = {row["entity_id"]: row for row in selected}
        admitted_by_id = {row["entity_id"]: _public_entity_record(row) for row in rows}
        inputs["admitted_relations"]["entity_dictionary"] = copy.deepcopy(
            admitted_by_id
        )
        response["entities"] = copy.deepcopy(selected_by_id)
        result = response["query_results"][0]
        result["entity_ids"] = [row["entity_id"] for row in selected]
        result["resolved_semantics"] = resolved
        result["coverage"] = coverage
        base["decoded_expectation"]["coverage"] = (
            "the typed description program consumes both admitted candidate rows under "
            "exact catalog/access authority and complete entity-kind coverage, then emits "
            "only the canonical occurrence selected by the request"
        )
        base["subject"] = (
            "Find-code-entities compiles the requested description through the typed query "
            "program and selects exact admitted entity rows without collapsing syntax "
            "occurrences into semantic callables"
        )
        base["source_anchor"] = (
            "QRY §4.1 `find code entities`; QRY §5 Composition DAG and execution semantics; "
            "QRY §6 Resolution, authorization, and bound authority"
        )
    elif claim_id == "RFV3-CLAIM-005":
        relations = inputs["admitted_relations"]
        relations["property_kind_registry"] = {
            "relation_id": "input.property_kind",
            "closed_universe": True,
            "rows": [
                {"property_kind": "type", "property_kind_code": 1},
                {"property_kind": "UNKNOWN_EFFECT", "property_kind_code": 2},
            ],
        }
        coverage = next(
            row for row in relations["coverage_rows"] if row["family"] == "effects"
        )
        source_text = "def f(x: int) -> int:\n    return int(x)\n"
        comparison_key = b"fixture.py"
        coverage["source_identity"] = {
            "file_id": (
                "file:"
                + cbef_source_file_id(
                    workspace_id=inputs["request_envelope"]["decoded"]["scope"][
                        "workspace_id"
                    ],
                    comparison_key=comparison_key,
                )
            ),
            "canonical_path_bytes_hex": comparison_key.hex(),
            "content_utf8": source_text,
            "content_digest": bytes_b3(source_text.encode("utf-8")),
        }
        admitted_type = next(
            record for record in relations["fact_rows"] if record["fact_kind"] == "type"
        )
        type_code = _property_kind_code(relations, "type")
        admitted_type["property_kind_code"] = type_code
        type_recipe = _query_property_fact_recipe(
            admitted_type,
            property_kind_code=type_code,
            output_prefix="fact:type",
            canonical_value=admitted_type["statement"]["object"],
            excluded=[
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
        )
        old_type_id = admitted_type["fact_id"]
        admitted_type["fact_id"] = type_recipe["output_id"]
        admitted_type["identity_recipe"] = type_recipe
        replace_identity_everywhere(base, old_type_id, type_recipe["output_id"])
        old_input_set_ids = {
            str(row["direct_provenance"]["input_set_id"])
            for row in relations["fact_rows"]
        }
        input_set_recipe = _retrieve_fact_input_set_recipe(
            relations, str(epoch["policy_release"])
        )
        input_set_id = input_set_recipe["output_id"]
        relations["input_set_identity"] = input_set_recipe
        for row in relations["fact_rows"]:
            row["direct_provenance"]["input_set_id"] = input_set_id
        for old_input_set_id in old_input_set_ids:
            replace_identity_everywhere(base, old_input_set_id, input_set_id)
        known_type = copy.deepcopy(admitted_type)
        known_type.pop("alias", None)
        prior_unknown = next(
            record
            for record in response["facts"].values()
            if record["fact_kind"] == "unknown"
        )
        unknown = {
            "fact_id": prior_unknown["fact_id"],
            "fact_form": "property",
            "fact_kind": "unknown",
            "fact_class": "semantic",
            "workspace_id": admitted_type["workspace_id"],
            "analysis_context_id": admitted_type["analysis_context_id"],
            "owner_id": admitted_type["owner_id"],
            "statement": {
                "subject": admitted_type["statement"]["subject"],
                "predicate": "UNKNOWN_EFFECT",
                "object": coverage["family"],
            },
            "certainty": "unresolved",
            "resolution": "unavailable",
            "directness": "direct",
            "producer": {"producer_id": "coverage:effects", "release": "r1"},
            "direct_provenance": {
                "source_generation": admitted_type["direct_provenance"][
                    "source_generation"
                ],
                "input_set_id": admitted_type["direct_provenance"]["input_set_id"],
                "support_ids": [],
                "coverage": {
                    "state": coverage["state"],
                    "reason": coverage["reason"],
                    "retryable": False,
                },
            },
        }
        unknown_code = _property_kind_code(relations, "UNKNOWN_EFFECT")
        unknown["property_kind_code"] = unknown_code
        recipe = _query_property_fact_recipe(
            unknown,
            property_kind_code=unknown_code,
            output_prefix="fact:unknown-effect",
            canonical_value=coverage["family"],
            excluded=[
                "coverage state",
                "coverage reason",
                "retryability",
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
        )
        old = unknown["fact_id"]
        unknown["fact_id"] = recipe["output_id"]
        unknown["identity_recipe"] = recipe
        replace_identity_everywhere(base, old, recipe["output_id"])
        response["facts"] = {
            known_type["fact_id"]: known_type,
            unknown["fact_id"]: unknown,
        }
        result = response["query_results"][0]
        result["fact_ids"] = [known_type["fact_id"], unknown["fact_id"]]
        result["coverage"]["unknown_fact_id"] = unknown["fact_id"]
        base["subject"] = (
            "Known type and UNKNOWN_EFFECT property facts are derived from admitted rows and explicit coverage using closed property-kind allocations and CBEF-v1.1 PROPERTY_FACT identities"
        )
        base["source_anchor"] = (
            "QRY §4.2 `retrieve facts about code`; QRY §7 Evidence, unknowns, absence, and provenance; QRY §8 Canonical identity and ordering; ONT §64 Canonical identity; ONT §66 Mandatory unknown semantics"
        )
    elif claim_id == "RFV3-CLAIM-006":
        block = inputs["request_envelope"]["decoded"]["queries"][0]
        selected = _selected_follow_edges(inputs, block)
        result = response["query_results"][0]
        response["facts"] = {edge["fact_id"]: edge for edge in selected}
        result["fact_ids"] = [edge["fact_id"] for edge in selected]
        result["resolved_semantics"] = {
            "starting_from": block["starting_from"],
            "relationship": block["relationship"],
            "direction": block["direction"],
            "distance": block["distance"],
        }
        coverage = inputs["producer_coverage"]
        result["coverage"] = {
            "state": coverage["state"],
            "owner": coverage["owner"],
            "analysis_context_id": coverage["analysis_context_id"],
            "distance": block["distance"],
            "completed_family": coverage["family"],
        }
        base["decoded_expectation"]["coverage"] = (
            "the exact one-step outgoing call facts are selected from admitted typed edges "
            "and equal the complete producer-coverage fact set for the requested owner"
        )
    elif claim_id == "RFV3-CLAIM-007":
        block = inputs["request_envelope"]["decoded"]["queries"][0]
        edges = inputs["admitted_relations"]["edges"]
        entity_ids, fact_ids = _canonical_shortest_witness(
            edges,
            start=block["from"][0],
            target=block["to"][0],
            families=block["using"],
            maximum_length=inputs["resource_limits"]["max_path_length"],
        )
        path = next(iter(response["paths"].values()))
        path["ordered_entity_ids"] = entity_ids
        path["ordered_fact_ids"] = fact_ids
        path["length"] = len(fact_ids)
        path["path_policy"] = block["path_policy"]
        path["certainty_summary"] = "exact"
        path["supporting_provenance"] = {
            "analysis_context_id": inputs["producer_coverage"]["analysis_context_id"],
            "coverage_state": inputs["producer_coverage"]["state"],
            "producer_releases": sorted(
                {
                    f"{edge['producer']['producer_id']}:{edge['producer']['release']}"
                    for edge in edges
                }
            ),
        }
        response["entities"] = copy.deepcopy(
            inputs["admitted_relations"]["entity_dictionary"]
        )
        response["facts"] = {edge["fact_id"]: copy.deepcopy(edge) for edge in edges}
        result = response["query_results"][0]
        result["coverage"] = {
            "state": inputs["producer_coverage"]["state"],
            "graph_projection": (
                f"{inputs['producer_coverage']['family']}@"
                f"{inputs['producer_coverage']['analysis_context_id']}"
            ),
            "searched_entity_count": len(inputs["producer_coverage"]["entity_ids"]),
            "searched_fact_count": len(inputs["producer_coverage"]["fact_ids"]),
        }
        result["resolved_semantics"] = {
            "from": block["from"],
            "to": block["to"],
            "relationship_families": block["using"],
            "path_policy": block["path_policy"],
            "maximum_path_length": inputs["resource_limits"]["max_path_length"],
        }
        recipe = _path_result_recipe(
            workspace_id=response["snapshot"]["workspace_id"],
            analysis_context_id=path["supporting_provenance"]["analysis_context_id"],
            fabric_epoch_id=epoch["fabric_epoch_id"],
            policy_identity=epoch["policy_release"],
            ordered_entity_ids=path["ordered_entity_ids"],
            ordered_fact_ids=path["ordered_fact_ids"],
        )
        old = path["path_id"]
        new = recipe["output_id"]
        path["path_id"] = new
        path["identity_recipe"] = recipe
        replace_identity_everywhere(base, old, new)
        base["subject"] = (
            "Path identity binds the authorized workspace, analysis context, exact epoch and policy, and the ordered entity/fact witness"
        )
        base["source_anchor"] = (
            "QRY §4.4 `find connecting fact paths`; QRY §8 Canonical identity and ordering"
        )
        base["decoded_expectation"]["coverage"] = (
            "the exact canonical shortest witness is derived from all admitted call edges "
            "under the complete entity/fact coverage projection and bounded path program"
        )
    elif claim_id == "RFV3-CLAIM-008":
        block = inputs["request_envelope"]["decoded"]["queries"][0]
        pattern = block["pattern"]
        if not any(node.get("binding") == "g" for node in pattern["nodes"]):
            pattern["nodes"].append(
                {
                    "binding": "g",
                    "module_id": pattern["nodes"][0]["module_id"],
                    "name": "g",
                    "semantic_kind": "function_declaration",
                }
            )
        pattern["facts"] = [
            {
                "subject_binding": "g",
                "relationship": "calls",
                "object_binding": "f",
                "direction": "outgoing",
            }
        ]
        inputs["request_envelope"]["canonical_json"] = rfc8785.dumps(
            inputs["request_envelope"]["decoded"]
        ).decode("utf-8")
        inputs["program_binding"]["pattern_contract"] = copy.deepcopy(pattern)
        rows = inputs["admitted_relations"]["entities"]["rows"]
        dictionary = {row["entity_id"]: _public_entity_record(row) for row in rows}
        inputs["admitted_relations"]["entity_dictionary"] = copy.deepcopy(dictionary)
        coverage = inputs["producer_coverage"]
        coverage["covered_fact_ids"] = sorted(
            edge["fact_id"] for edge in inputs["admitted_relations"]["call_edges"]
        )
        matches = _typed_pattern_matches(inputs, block)
        response["entities"] = copy.deepcopy(dictionary)
        response["facts"] = {
            edge["fact_id"]: copy.deepcopy(edge)
            for edge in inputs["admitted_relations"]["call_edges"]
        }
        result = response["query_results"][0]
        result["bindings"] = matches
        result["entity_ids"] = sorted(
            {
                value["entity_id"]
                for match in matches
                for value in match["bindings"].values()
            }
        )
        result["fact_ids"] = sorted(
            {fact_id for match in matches for fact_id in match["supporting_fact_ids"]}
        )
        result["coverage"] = {
            "state": coverage["state"],
            "outcome": "MATCH" if matches else "NO_MATCH_AFTER_FILTERS",
            "owner_scope": coverage["owner_scope"],
            "analysis_context_id": coverage["analysis_context_id"],
            "family": coverage["family"],
            "covered_subject_ids": coverage["covered_subject_ids"],
            "covered_fact_ids": coverage["covered_fact_ids"],
            "negative_proof_universe_id": coverage["negative_proof_universe_id"],
        }
        result["resolved_semantics"] = {
            "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
            "typed_bindings": {
                node["binding"]: node["semantic_kind"] for node in pattern["nodes"]
            },
            "positive_fact_count": len(pattern["facts"]),
            "scoped_negation_universe": coverage["negative_proof_universe_id"],
        }
        base["decoded_expectation"]["coverage"] = (
            "typed f/g nodes and the admitted g-calls-f fact bind first; complete "
            "module/context call coverage then proves f has no outgoing call"
        )
        base["subject"] = (
            "A typed pattern joins admitted node and relationship facts before applying "
            "scoped negation, whose absence result requires exact owner, family, context, "
            "subject, and fact coverage"
        )
        base["source_anchor"] = (
            "QRY §4.5 `match a code fact pattern`; QRY §7 Evidence, unknowns, absence, "
            "and provenance"
        )
    elif claim_id == "RFV3-CLAIM-009":
        envelope = inputs["request_envelope"]["decoded"]
        combine_block = copy.deepcopy(
            next(
                block
                for block in envelope["queries"]
                if block.get("request") == "combine result sets"
            )
        )
        workspace_id = envelope["scope"]["workspace_id"]
        relations = inputs["admitted_relations"]
        provenance = _query_provenance(response)
        existing_inputs = relations.get("producer_inputs")
        producer_inputs: dict[str, dict[str, Any]] = (
            copy.deepcopy(existing_inputs) if isinstance(existing_inputs, dict) else {}
        )
        producer_blocks: list[dict[str, Any]] = []
        for query_id in ("left", "right"):
            input_relation = f"input.query.entities.{query_id}"
            if query_id not in producer_inputs:
                source = relations.pop(query_id)
                source.pop("query_id")
                source["relation_id"] = input_relation
                producer_inputs[query_id] = source
            producer_blocks.append(
                {
                    "query_id": query_id,
                    "request": "find code entities",
                    "looking_for": "function declarations",
                    "within": [workspace_id],
                    "where": [
                        {
                            "relation": input_relation,
                            "predicate": "member",
                        }
                    ],
                    "return": copy.deepcopy(combine_block["return"]),
                }
            )
        relations["producer_inputs"] = producer_inputs
        producer_results = _producer_results_from_inputs(producer_inputs, provenance)
        envelope["queries"] = [*producer_blocks, combine_block]
        inputs["request_envelope"]["canonical_json"] = rfc8785.dumps(envelope).decode(
            "utf-8"
        )
        combine_result = next(
            result
            for result in response["query_results"]
            if result.get("query_id") == combine_block["query_id"]
        )
        response["query_results"] = [
            copy.deepcopy(producer_results["left"]),
            copy.deepcopy(producer_results["right"]),
            combine_result,
        ]
        response["successful_query_count"] = 3
        for result in response["query_results"]:
            result["provenance"]["expectation_issuance"] = ISSUANCE_ID
        base["subject"] = (
            "A topologically closed request DAG executes two typed producer query blocks and combines their real result envelopes only after exact semantic-role and compatibility checks"
        )
        base["source_anchor"] = (
            "QRY §4.6 `combine result sets`; QRY §5 Composition and dependency graphs; QRY §8 Canonical identity and ordering"
        )
    elif claim_id == "RFV3-CLAIM-010":
        relations = inputs["admitted_relations"]
        relations["property_kind_registry"] = {
            "relation_id": "input.property_kind",
            "closed_universe": True,
            "rows": [{"property_kind": "native_kind", "property_kind_code": 1}],
        }
        relations["coverage_state"] = str(inputs["producer_coverage"]["syntax"])
        first = relations["syntax_rows"][0]
        first_entity = relations["entity_dictionary"][first["statement"]["subject"]]
        if (
            first["statement"]["object"] == "identifier"
            and first_entity["semantic_kind"] == "function_syntax"
        ):
            first["statement"]["object"] = "function_definition"
        _normalize_objective_relations(relations, epoch["policy_release"])
        response["entities"] = copy.deepcopy(relations["entity_dictionary"])
        response["facts"] = {
            row["fact_id"]: copy.deepcopy(row) for row in relations["syntax_rows"]
        }
        response["groups"] = _objective_groups(relations)
        result = response["query_results"][0]
        input_set_id = relations["input_set_identity"]["output_id"]
        group_ids = list(response["groups"])
        result["group_ids"] = group_ids
        result["resolved_semantics"]["input_set_id"] = input_set_id
        result["coverage"].update(
            {
                "input_set_id": input_set_id,
                "input_count": len(relations["syntax_rows"]),
                "group_count": len(group_ids),
            }
        )
        base["subject"] = (
            "Objective fact, input-set, and group identities are independently derived from typed syntax occurrences, exact property values, canonical membership, grouping dimensions, aggregate definition, producer, and policy"
        )
        base["source_anchor"] = (
            "QRY §4.7 `summarize objective facts`; QRY §8 Canonical identity and ordering"
        )
    elif claim_id == "RFV3-CLAIM-011":
        request_envelope = inputs["request_envelope"]["decoded"]
        request = request_envelope["queries"][0]
        prior_context = request["context"]
        if isinstance(prior_context, dict):
            # Earlier r3 drafts incorrectly extended the released v2 request field.
            # Recover its semantic values once, then emit the governed scalar ingress.
            context_option = prior_context
            while isinstance(context_option.get("kind"), dict):
                context_option = context_option["kind"]
            context_kind = context_option["kind"]
            source_byte_limit = context_option["maximum_source_bytes"]
        else:
            context_kind = prior_context
            source_byte_limit = inputs["resource_limits"].get(
                "max_source_bytes", request["return"]["limit"]["maximum_results"]
            )
        request["context"] = context_kind
        request["return"]["limit"].update({"maximum_results": 1, "per": "query block"})
        inputs["resource_limits"]["max_source_bytes"] = source_byte_limit
        inputs["request_envelope"]["canonical_json"] = rfc8785.dumps(
            request_envelope
        ).decode("utf-8")
        source = inputs["admitted_relations"]["source_bytes"]
        digest = bytes_b3(source["value"].encode("utf-8"))
        span = inputs["admitted_relations"]["entity_span"]
        old_source_file_id = span["source_file_id"]
        source_file_id = f"file:{cbef_source_file_id(workspace_id=span['workspace_id'], comparison_key=span['byte_safe_path'].encode('utf-8'))}"
        replace_identity_everywhere(base, old_source_file_id, source_file_id)
        span["source_file_id"] = source_file_id
        span["content_digest"] = digest
        entity = next(iter(inputs["admitted_relations"]["entity_dictionary"].values()))
        entity["source_reference"].update(
            {
                "source_file_id": source_file_id,
                "content_digest": digest,
                "start_byte": span["start_byte"],
                "end_byte": span["end_byte"],
            }
        )
        response_entity = response["entities"][entity["entity_id"]]
        response_entity["source_reference"] = copy.deepcopy(entity["source_reference"])
        context = next(iter(response["source_contexts"].values()))
        context["context_kind"] = context_kind
        context["source_reference"] = copy.deepcopy(entity["source_reference"])
        access_scope = {
            "workspace": span["workspace_id"],
            "principal_id": "principal:11111111111111111111111111111111",
            "agent_id": "agent:11111111111111111111111111111111",
            "credential_digest": bytes_b3(b"wp33-c011-source-context-credential"),
            "role": "reader",
            "operation": "query",
            "allowed_relations": [
                "canonical.entity_span",
                "source.authorized_bytes",
            ],
            "allowed_columns": {
                "canonical.entity_span": [
                    "byte_safe_path",
                    "content_digest",
                    "end_byte",
                    "entity_id",
                    "source_file_id",
                    "source_generation",
                    "start_byte",
                    "workspace_id",
                ],
                "source.authorized_bytes": ["byte_length", "encoding", "value"],
            },
            "allowed_functions": [],
            "allowed_extensions": [],
            "allowed_variables": [],
            "allowed_object_stores": [],
            "allowed_metadata": [],
            "row_policies": [],
            "execution_posture": ["bounded", "read_only"],
            "source_access": True,
            "source_file_ids": [source_file_id],
            "authorized_ranges": [
                [source_file_id, span["start_byte"], span["end_byte"]]
            ],
        }
        scope_recipe = authorization_scope_identity(
            access_scope, {"policy_id": epoch["policy_release"]}
        )
        access_scope["scope_id"] = scope_recipe["output_id"]
        access_scope["identity_recipe"] = scope_recipe
        inputs["access_scope"] = access_scope
        context["authorization_scope"] = access_scope["scope_id"]
        delivered = context["content"]["text"].encode("utf-8")
        recipe = _query_source_context_recipe(
            workspace_id=span["workspace_id"],
            analysis_context_id=entity["analysis_context_id"],
            snapshot_id=response["snapshot"]["snapshot_id"],
            entity_id=context["entity_id"],
            source_file_id=span["source_file_id"],
            source_generation=span["source_generation"],
            source_content_digest=digest,
            delivered_start_byte=span["start_byte"],
            delivered_end_byte=span["start_byte"] + context["returned_bytes"],
            delivered_content_digest=bytes_b3(delivered),
            disclosure_scope_id=access_scope["scope_id"],
            policy_identity=epoch["policy_release"],
            context_kind=context_kind,
        )
        old = context["source_context_id"]
        context["source_context_id"] = recipe["output_id"]
        context["identity_recipe"] = recipe
        replace_identity_everywhere(base, old, recipe["output_id"])
        result = response["query_results"][0]
        result["resolved_semantics"].update(
            {
                "context": context_kind,
                "explicit_source_byte_limit": source_byte_limit,
            }
        )
        base["subject"] = (
            "Source-context identity binds entity, workspace, snapshot, file generation, source digest, exact delivered range and bytes, disclosure scope, and policy"
        )
        base["source_anchor"] = (
            "QRY §4.8 `retrieve source and syntax context`; QRY §8 Canonical identity and ordering"
        )
    return base


def delta_inputs() -> dict[str, Any]:
    protocol = {
        "min_reader_version": 1,
        "min_writer_version": 4,
        "reader_features": [],
        "writer_features": [],
        "table_properties": {"delta.enableChangeDataFeed": "true"},
    }
    versions = [
        {
            "version": 0,
            "operation": "CREATE",
            "input_rows": [],
            "expected_snapshot": [],
            "protocol": copy.deepcopy(protocol),
        },
        {
            "version": 1,
            "operation": "WRITE_APPEND",
            "input_rows": [["e1", "baseline"]],
            "expected_snapshot": [["e1", "baseline"]],
            "protocol": copy.deepcopy(protocol),
        },
        {
            "version": 2,
            "operation": "WRITE_APPEND",
            "input_rows": [["e2", "version-two"]],
            "expected_snapshot": [
                ["e1", "baseline"],
                ["e2", "version-two"],
            ],
            "protocol": copy.deepcopy(protocol),
        },
        {
            "version": 3,
            "operation": "WRITE_APPEND",
            "input_rows": [["e3", "version-three"]],
            "expected_snapshot": [
                ["e1", "baseline"],
                ["e2", "version-two"],
                ["e3", "version-three"],
            ],
            "protocol": copy.deepcopy(protocol),
        },
    ]
    return {
        "delta_table_history": {
            "table": "fact.entity",
            "materialization": {
                "root_binding": "runtime-created private local-filesystem URL",
                "creation_api": "deltalake::operations::create::CreateBuilder",
                "write_api": "DeltaTable::write",
                "authority": "Delta transaction log interpreted by delta-rs",
                "frozen_physical_uri": False,
            },
            "schema": [
                {"name": "entity_id", "delta_type": "string", "nullable": False},
                {"name": "value", "delta_type": "string", "nullable": False},
            ],
            "versions": versions,
        },
        "selected_version_vector": {
            "table_versions": {"fact.entity": 2},
            "selection": "exact",
        },
        "protocol_support": {
            "delta_rs_revision": "43a0cf10a313e5077c48637ad786a05359136bbb",
            "supported_reader_features": [
                "timestampNtz",
                "deletionVectors",
                "variantType",
                "variantType-preview",
                "v2Checkpoint",
                "columnMapping",
            ],
            "supported_writer_features": [
                "appendOnly",
                "timestampNtz",
                "variantType",
                "variantType-preview",
                "v2Checkpoint",
                "changeDataFeed",
                "invariants",
                "checkConstraints",
                "generatedColumns",
                "columnMapping",
                "deletionVectors",
            ],
            "unsupported_writer_features": ["rowTracking"],
        },
        "table_root_identity": {
            "binding": "runtime-created local-filesystem URL",
            "canonicalization": "ExactDeltaPin canonical URL after materialization",
            "frozen_uri": False,
        },
        "runtime_configuration": {
            "datafusion": "55.0.0",
            "arrow": "59.2.0",
            "object_store": "0.13.2",
            "deltalake": "git:43a0cf10a313e5077c48637ad786a05359136bbb",
        },
        "proof_input": {
            "exact_snapshot_read": {
                "api": "DeltaTableBuilder::with_version",
                "version": 2,
            },
            "latest_snapshot_read": {
                "api": "DeltaTable::update_state",
                "expected_version": 3,
            },
            "cdf_read": {
                "api": "DeltaTable::scan_cdf",
                "starting_version": 3,
                "ending_version": 3,
                "inclusive_bounds": True,
                "allow_out_of_range": False,
                "required_metadata_columns": [
                    "_change_type",
                    "_commit_version",
                    "_commit_timestamp",
                ],
            },
            "raw_object_listing_authority": False,
        },
    }


def delta_decoded(inputs: dict[str, Any], selected: int) -> dict[str, Any]:
    versions = inputs["delta_table_history"]["versions"]
    version = next(item for item in versions if item["version"] == selected)
    latest = versions[-1]["version"]
    cdf = inputs["proof_input"]["cdf_read"]
    cdf_rows = [
        [*row, "insert", candidate["version"]]
        for candidate in versions
        if cdf["starting_version"] <= candidate["version"] <= cdf["ending_version"]
        for row in candidate["input_rows"]
    ]
    return {
        "selected_version": selected,
        "latest_version": latest,
        "protocol": version["protocol"],
        "snapshot_rows": version["expected_snapshot"],
        "cdf_window": {
            "starting_version": cdf["starting_version"],
            "ending_version": cdf["ending_version"],
            "inclusive": True,
        },
        "cdf_columns": ["entity_id", "value", "_change_type", "_commit_version"],
        "cdf_rows": cdf_rows,
    }


def claim_012(base: dict[str, Any]) -> dict[str, Any]:
    inputs = delta_inputs()
    expected = delta_decoded(inputs, 2)
    base.update(
        {
            "subject": "The exact delta-rs revision materializes a real local Delta table, selects an explicit non-latest version, enforces protocol features, and reads one inclusive CDF range",
            "author_id": AUTHOR,
            "source_anchor": "FAB §9 Durable Delta relations; FAB §9.2 Exact reads and the single-selector rule; FAB §9.4 Durability classification and exact reconstruction",
            "governing_clauses": [
                "FAB §15 Executable acceptance obligations",
                "SUITE §5.1 Proof relations",
                "P11 Immutable snapshots and explicit transitions",
            ],
            "complete_input_universe": {"closed": True, "inputs": inputs},
            "decoded_expectation": {
                "terminal": "pass",
                "relation": "proof.delta_exact_version_and_cdf",
                "columns": ["table", "exact_observation"],
                "rows": [["fact.entity", expected]],
                "coverage": "a runtime-created local Delta table must expose an empty CREATE at version 0 followed by WRITE commits 1..3; exact version 2 differs from latest version 3; CDF [3,3] is inclusive and returns only the version-3 insert",
            },
            "semantics": {
                "ordering": "snapshot rows by entity_id; CDF rows by _commit_version, _change_type, entity_id",
                "nulls": "entity_id, value, _change_type, and _commit_version are required",
                "unknowns": "missing retained log versions, absent CDF activation, or unsupported protocol features fail closed and require exact snapshot reconstruction or admission denial",
                "provenance": "the observation binds the exact delta-rs revision, canonical runtime table root, selected version, protocol, table properties, inclusive CDF bounds, and commit-version metadata",
            },
            "limitations": [
                "WP33 authors logical Delta observations and APIs; the physical table URI, Parquet filenames, checkpoint bytes, and commit-file hashes are runtime observations and are deliberately not fabricated here."
            ],
        }
    )
    return base


ACTIVATION_FIELDS = [
    "control_root",
    "control_predecessor_version",
    "control_commit_version",
    "control_session_id",
    "control_provider_binding_id",
    "control_binding_fingerprint",
    "logical_schema_digest",
    "storage_schema_digest",
    "event_id",
    "workspace_id",
    "operation_id",
    "predecessor_event_id",
    "predecessor_epoch",
    "ordinal",
    "lease_id",
    "writer_generation",
    "epoch_id",
    "input_release",
    "program_release",
    "application_release",
    "source_authority",
    "source_generation",
    "provider_release",
    "provider_set",
    "table_versions",
    "table_version_components",
    "overlay_segments",
    "policy_set",
    "resource_envelope",
    "proof_receipt",
    "compatibility_class",
    "retention_policy",
    "operation_selection",
    "transaction",
    "row_digest",
]

PROGRAMMATIC_OBSERVATION_TABLE_VERSION_RELATIONS = [
    "system.programmatic_dependency_observation",
    "system.programmatic_field_observation",
    "system.programmatic_provenance_observation",
    "system.programmatic_relation_observation",
    "system.programmatic_schema_observation",
]


def activation_table_version_binding() -> dict[str, Any]:
    """Describe the production-only binding for one sealed epoch's Delta vector.

    WP33 cannot author a table-version reference, root, or exact version: all three
    exist only after the five observation histories have been committed and sealed.
    The Rust consumer resolves these bindings from the publication and derives the
    reference with the same `TableVersionSet::try_new` path used by production.
    """

    return {
        "kind": "runtime_derived_table_version_set",
        "source": "sealed_programmatic_observation_delta_publication",
        "constructor": "TableVersionSet::try_new",
        "reference_projection": "TableVersionSet::reference",
        "components": [
            {
                "relation_id": relation_id,
                "exact_delta_pin": {
                    "root": "publication_runtime_root",
                    "version": "publication_exact_version",
                },
            }
            for relation_id in PROGRAMMATIC_OBSERVATION_TABLE_VERSION_RELATIONS
        ],
    }


def activation_typed_input(seed: int, width: int) -> str:
    """Encode an explicit nonzero fixture value for one production byte-identity type."""

    if seed <= 0 or seed.bit_length() > width * 8:
        raise ValueError("activation typed fixture input is outside its declared width")
    return seed.to_bytes(width, "big").hex()


def activation_event(
    label: str, ordinal: int, predecessor: dict[str, Any] | None
) -> dict[str, Any]:
    del label
    event_id = activation_typed_input(0x1000 + ordinal, 32)
    workspace = activation_typed_input(0x2000, 16)
    operation = activation_typed_input(0x3000 + ordinal, 16)
    epoch = activation_typed_input(0x4000 + ordinal, 16)
    pins = {
        "epoch": epoch,
        "input_release": activation_typed_input(0x5000 + ordinal, 32),
        "program_release": activation_typed_input(0x6000, 32),
        "application_release": activation_typed_input(0x7000, 32),
        "source_authority": activation_typed_input(0x8000 + ordinal, 32),
        "source_generation": ordinal,
        "provider_release": activation_typed_input(0x9000, 32),
        "provider_set": activation_typed_input(0xA000 + ordinal, 32),
        "table_versions": activation_table_version_binding(),
        "overlay_segments": activation_typed_input(0xC000 + ordinal, 32),
        "policy_set": activation_typed_input(0xD000, 32),
        "resource_envelope": activation_typed_input(0xE000, 32),
        "proof_receipt": activation_typed_input(0xF000 + ordinal, 32),
    }
    fence = {
        "lease_id": activation_typed_input(0x1_0000, 16),
        "generation": ordinal,
    }
    transaction = activation_typed_input(0x1_1000 + ordinal, 32)
    control_predecessor = ordinal - 1
    command = {
        "identity": {
            "operation_id": operation,
            "idempotency_key": activation_typed_input(0x1_2000 + ordinal, 32),
        },
        "ownership": {
            "workspace_id": workspace,
            "principal_id": activation_typed_input(0x1_3000, 16),
            "authorization": activation_typed_input(0x1_4000, 32),
        },
        "expected_head": (
            {"kind": "empty"}
            if predecessor is None
            else {"kind": "epoch", "epoch": predecessor["pins"]["epoch"]}
        ),
        "pins": {
            key: value
            for key, value in pins.items()
            if key
            not in {
                "epoch",
                "table_versions",
                "overlay_segments",
                "policy_set",
                "proof_receipt",
            }
        },
        "resources": pins["resource_envelope"],
        "payload": {
            "kind": "ActivateEpoch",
            "candidate_epoch": epoch,
            "proof_receipt": pins["proof_receipt"],
        },
    }
    return {
        "event_id": event_id,
        "workspace_id": workspace,
        "operation_id": operation,
        "predecessor_event_id": None
        if predecessor is None
        else predecessor["event_id"],
        "predecessor_epoch": command["expected_head"],
        "ordinal": ordinal,
        "execution_fence": fence,
        "pins": pins,
        "compatibility_class": activation_typed_input(0x1_5000, 32),
        "retention_policy": activation_typed_input(0x1_6000, 32),
        "command": command,
        "durable_commit": {
            "operation_selection": activation_typed_input(0x1_7000 + ordinal, 32),
            "transaction": transaction,
        },
        "backend_observation": {
            "control_root_binding": "runtime-created private activation-control URL",
            "control_predecessor_version": control_predecessor,
            "control_commit_version": ordinal,
            "marker_observed_version": ordinal,
            "operation_id": operation,
            "transaction": transaction,
            "writer_fence": fence,
            "read_version": {"kind": "exact", "version": control_predecessor},
            "num_retries": 0,
        },
        "readback": {
            "relation_id": "control.activation_event.v3",
            "storage_relation_id": "storage.delta.activation_event.v3",
            "control_commit_version": ordinal,
            "operation_id": operation,
            "transaction": transaction,
            "writer_fence": fence,
            "row_event_id": event_id,
        },
    }


def activation_chain(count: int = 1) -> dict[str, Any]:
    events: list[dict[str, Any]] = []
    for ordinal in range(1, count + 1):
        events.append(
            activation_event(f"e{ordinal}", ordinal, events[-1] if events else None)
        )
    return {
        "relation_contract": {
            "relation_id": "control.activation_event.v3",
            "storage_relation_id": "storage.delta.activation_event.v3",
            "schema_identity": "programmatic:control.activation_event.v3:arrow59-delta1",
            "provider_binding_id": "binding.delta.exact-snapshot.activation-event.v3",
            "schema_version": 3,
            "arrow_type_universe": "arrow-array@59.2.0|arrow-schema@59.2.0|datafusion@55.0.0|deltalake@43a0cf10",
            "append_only": True,
            "change_data_feed": True,
            "fields": ACTIVATION_FIELDS,
        },
        "typed_identity_contract": {
            "contract_id": "codefabric.activation.typed-fixture-inputs.v1",
            "authority": "explicit canonical bytes supplied to the production Rust byte-identity types",
            "widths": {
                "WorkspaceId": 16,
                "OperationId": 16,
                "PrincipalId": 16,
                "LeaseId": 16,
                "EpochId": 16,
                "ActivationEventId": 32,
                "IdempotencyKey": 32,
                "AuthorizationRef": 32,
                "release_and_policy_refs": 32,
                "OperationSelectionRef": 32,
                "TransactionRef": 32,
            },
            "classification": {
                "command_and_control_ids": "opaque typed inputs, not content hashes",
                "release_and_policy_refs": "exact fixture release inputs, not acceptance checksums",
                "row_and_readback_integrity": "computed and verified by the production activation-control codec; deliberately not authored by WP33",
            },
            "programmatic_readback": "exact operation, transaction, fence, commit version, and row event equality; no digest surrogate",
        },
        "events": events,
    }


def activation_outcome(inputs: dict[str, Any]) -> dict[str, Any]:
    head = inputs["activation_chain"]["events"][-1]
    return {
        "selected_event_id": head["event_id"],
        "selected_epoch": head["pins"]["epoch"],
        "predecessor_event_id": head["predecessor_event_id"],
        "command": head["command"],
        "fabric_epoch_pins": head["pins"],
        "durable_commit": head["durable_commit"],
        "backend_observation": head["backend_observation"],
        "readback": head["readback"],
        "installation": {
            "state": "installed",
            "epoch": head["pins"]["epoch"],
            "readback_event_id": head["readback"]["row_event_id"],
            "control_commit_version": head["readback"]["control_commit_version"],
        },
        "receipt_cache_reconciliation": inputs["receipt_cache_observation"][
            "reconciliation"
        ],
        "acknowledgement": {
            "state": "acknowledged",
            "event_id": head["event_id"],
            "control_commit_version": head["readback"]["control_commit_version"],
        },
        "admission_state": "open",
        "candidate_present_during_reconcile": inputs["candidate_memory_observation"][
            "candidate_present"
        ],
        "receipt_cache_authoritative": inputs["receipt_cache_observation"][
            "authoritative"
        ],
    }


def claim_013(base: dict[str, Any]) -> dict[str, Any]:
    inputs = {
        "activation_chain": activation_chain(1),
        "recovery_policy": {
            "admission_during_recovery": "closed",
            "admission_after_install_readback": "open",
            "acknowledgement_after_install_readback": "required",
            "candidate_epoch_allowed": False,
            "selection": "unique predecessor-event-linked complete head",
            "receipt_cache_authority": False,
            "required_relation": "control.activation_event.v3",
        },
        "receipt_cache_observation": {
            "present": False,
            "authoritative": False,
            "reconciliation": "complete_non_authoritative",
        },
        "candidate_memory_observation": {"candidate_present": False},
    }
    base.update(
        {
            "subject": "Candidate-free recovery selects and installs the unique control.activation_event.v3 head, reconciles non-authoritative receipt state, reopens admission, and acknowledges only after exact readback",
            "author_id": AUTHOR,
            "source_anchor": "FAB §11 `FabricCommand`, fencing, publication, and activation; FAB §11.3 Ordered activation; FAB §14 Lifecycle, reconstruction, and cutover",
            "governing_clauses": [
                "SUITE §5.1 Proof relations",
                "P16 Lifecycle phases first-class",
                "P34 One mutation path; idempotent commands",
            ],
            "complete_input_universe": {"closed": True, "inputs": inputs},
            "decoded_expectation": {
                "terminal": "selected_epoch_installed_admission_reopened_acknowledged",
                "relation": "control.activation_recovery_outcome.v3",
                "columns": ["outcome"],
                "rows": [[activation_outcome(inputs)]],
                "coverage": "the live v3 activation relation contract, all FabricEpochPins, the runtime-derived TableVersionSet from exactly the five sealed ProgrammaticObservationDeltaPublication histories, exact command ownership/pins/payload, writer fence, operation selection, transaction marker, separate activation-control backend commit observation, exact row readback, candidate-free reopen, installation, receipt reconciliation, admission reopen, and acknowledgement agree",
            },
            "semantics": {
                "ordering": "strict predecessor event identity and nonzero activation ordinal; Delta commit version is physical evidence, not semantic timestamp ordering",
                "nulls": "only a genesis predecessor may be absent; every selected event closes command, pins, transaction, backend, and readback inputs",
                "unknowns": "any missing or contradictory command, pin, transaction, backend, readback, predecessor, or fence observation keeps admission closed",
                "provenance": "the selected outcome retains the exact live control.activation_event.v3 contract, the separately bound activation-control root/version, and the complete five-relation runtime TableVersionSet plus durable and post-commit evidence chain",
            },
            "limitations": [
                "Operational IDs are explicit width-checked inputs to the production Rust identity types, not hashes of fixture labels. WP33 does not fabricate a TableVersionSetRef, Delta URI/version, commit-file hash, row checksum, readback digest, or process-local candidate; production derives the reversible observation-history vector and its reference from actual sealed Arrow/Delta observations."
            ],
        }
    )
    return base


def authorization_scope_id(
    access_scope: dict[str, Any], authorization_policy: dict[str, Any]
) -> str:
    return authorization_scope_identity(access_scope, authorization_policy)["output_id"]


def authorization_scope_identity(
    access_scope: dict[str, Any], authorization_policy: dict[str, Any]
) -> dict[str, Any]:
    def canonical_strings(name: str) -> list[str]:
        values = [str(value) for value in access_scope[name]]
        if any(not value for value in values) or len(values) != len(set(values)):
            raise ValueError(f"access-scope {name} must be a unique string set")
        return sorted(values)

    allowed_relations = canonical_strings("allowed_relations")
    allowed_columns = {
        str(relation): sorted(str(column) for column in columns)
        for relation, columns in sorted(access_scope["allowed_columns"].items())
    }
    if set(allowed_columns) != set(allowed_relations) or any(
        not columns or len(columns) != len(set(columns))
        for columns in allowed_columns.values()
    ):
        raise ValueError("access-scope relation and column grants differ")
    relation_payload = _cbef_sequence_payload(
        [(2, _cbef_utf8(relation)) for relation in allowed_relations],
        canonical_set=True,
    )
    columns_payload = _cbef_map_payload(
        [
            (
                (2, _cbef_utf8(relation)),
                (
                    10,
                    _cbef_sequence_payload(
                        [(2, _cbef_utf8(column)) for column in columns],
                        canonical_set=True,
                    ),
                ),
            )
            for relation, columns in allowed_columns.items()
        ]
    )
    grant_sets = {
        name: canonical_strings(name)
        for name in (
            "allowed_functions",
            "allowed_extensions",
            "allowed_variables",
            "allowed_object_stores",
            "allowed_metadata",
            "row_policies",
            "execution_posture",
        )
    }
    grant_payloads = {
        name: _cbef_sequence_payload(
            [(2, _cbef_utf8(value)) for value in values], canonical_set=True
        )
        for name, values in grant_sets.items()
    }
    source_file_ids = canonical_strings("source_file_ids")
    source_files_payload = _cbef_sequence_payload(
        [(7, public_id_bytes(value, "file")) for value in source_file_ids],
        canonical_set=True,
    )
    authorized_ranges = access_scope["authorized_ranges"]
    range_members: list[tuple[int, bytes]] = []
    canonical_ranges: list[list[Any]] = []
    for value in authorized_ranges:
        if (
            not isinstance(value, list)
            or len(value) != 3
            or not isinstance(value[1], int)
            or isinstance(value[1], bool)
            or not isinstance(value[2], int)
            or isinstance(value[2], bool)
            or not 0 <= value[1] < value[2] <= 0xFFFF_FFFF_FFFF_FFFF
        ):
            raise ValueError("access-scope authorized range is invalid")
        canonical_ranges.append([str(value[0]), value[1], value[2]])
        range_members.append(
            (
                9,
                _cbef_sequence_payload(
                    [
                        (7, public_id_bytes(value[0], "file")),
                        (4, value[1].to_bytes(8, "big")),
                        (4, value[2].to_bytes(8, "big")),
                    ],
                    canonical_set=False,
                ),
            )
        )
    ranges_payload = _cbef_sequence_payload(range_members, canonical_set=True)
    return _cbef_recipe_evidence(
        domain_code=22,
        domain_name="ACCESS_SCOPE",
        output_prefix="access-scope",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(access_scope["workspace"], "workspace"),
                access_scope["workspace"],
            ),
            (
                2,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8(authorization_policy["policy_id"]),
                authorization_policy["policy_id"],
            ),
            (
                3,
                "principal_id",
                7,
                "ID",
                public_id_bytes(access_scope["principal_id"], "principal"),
                access_scope["principal_id"],
            ),
            (
                4,
                "agent_id",
                7,
                "ID",
                public_id_bytes(access_scope["agent_id"], "agent"),
                access_scope["agent_id"],
            ),
            (
                5,
                "credential_digest",
                8,
                "DIGEST",
                digest_bytes(access_scope["credential_digest"], "b3"),
                access_scope["credential_digest"],
            ),
            (
                6,
                "role",
                2,
                "UTF8",
                _cbef_utf8(access_scope["role"], ascii_lower=True),
                access_scope["role"],
            ),
            (
                7,
                "operation",
                2,
                "UTF8",
                _cbef_utf8(access_scope["operation"], ascii_lower=True),
                access_scope["operation"],
            ),
            (8, "allowed_relations", 10, "SET", relation_payload, allowed_relations),
            (9, "allowed_columns", 11, "MAP", columns_payload, allowed_columns),
            (
                10,
                "allowed_functions",
                10,
                "SET",
                grant_payloads["allowed_functions"],
                grant_sets["allowed_functions"],
            ),
            (
                11,
                "allowed_extensions",
                10,
                "SET",
                grant_payloads["allowed_extensions"],
                grant_sets["allowed_extensions"],
            ),
            (
                12,
                "allowed_variables",
                10,
                "SET",
                grant_payloads["allowed_variables"],
                grant_sets["allowed_variables"],
            ),
            (
                13,
                "allowed_object_stores",
                10,
                "SET",
                grant_payloads["allowed_object_stores"],
                grant_sets["allowed_object_stores"],
            ),
            (
                14,
                "allowed_metadata",
                10,
                "SET",
                grant_payloads["allowed_metadata"],
                grant_sets["allowed_metadata"],
            ),
            (
                15,
                "row_policies",
                10,
                "SET",
                grant_payloads["row_policies"],
                grant_sets["row_policies"],
            ),
            (
                16,
                "execution_posture",
                10,
                "SET",
                grant_payloads["execution_posture"],
                grant_sets["execution_posture"],
            ),
            (
                17,
                "source_access",
                6,
                "BOOLEAN",
                bytes([int(access_scope["source_access"])]),
                access_scope["source_access"],
            ),
            (
                18,
                "source_file_ids",
                10,
                "SET",
                source_files_payload,
                source_file_ids,
            ),
            (
                19,
                "authorized_ranges",
                10,
                "SET",
                ranges_payload,
                canonical_ranges,
            ),
        ],
        excluded=[
            "derived child catalog",
            "bound physical plan",
            "resource limits",
            "mutable request counters",
        ],
    )


def claim_014(base: dict[str, Any]) -> dict[str, Any]:
    base["author_id"] = AUTHOR
    inputs = base["complete_input_universe"]["inputs"]
    scope = inputs["access_scope"]
    scope.update(
        {
            "principal_id": "principal:14141414141414141414141414141414",
            "agent_id": "agent:24242424242424242424242424242424",
            "credential_digest": bytes_b3(b"wp33-c014-credential-identity"),
            "role": "reader",
            "operation": "query",
            "allowed_functions": [],
            "allowed_extensions": [],
            "allowed_variables": [],
            "allowed_object_stores": [],
            "allowed_metadata": [],
            "row_policies": [],
            "execution_posture": ["bounded", "read_only"],
            "source_access": False,
            "source_file_ids": [],
            "authorized_ranges": [],
        }
    )
    recipe = authorization_scope_identity(scope, inputs["authorization_policy"])
    scope["scope_id"] = recipe["output_id"]
    scope["identity_recipe"] = recipe
    base["subject"] = (
        "A content-bound access-scope identity constructs a reduced child catalog that exposes only its exact authorized relation and column grants"
    )
    return base


def claim_017(base: dict[str, Any]) -> dict[str, Any]:
    base["author_id"] = AUTHOR
    inputs = base["complete_input_universe"]["inputs"]
    public_response = base["decoded_expectation"]["rows"][0][0]
    inputs["candidate_released_projection"] = copy.deepcopy(public_response)
    base["subject"] = (
        "The presentation boundary releases only the exact daemon-authored public projection; private diagnostics remain separate authority and any candidate projection containing a forbidden physical field is rejected"
    )
    base["source_anchor"] = (
        "SRV §11 Public status, errors, and redaction; SRV §13 Lifespan, middleware, and STDIO purity"
    )
    return base


def arrow_schema_contract() -> dict[str, Any]:
    contract = {
        "relation_id": "query.result.ordinals.v1",
        "arrow_type_universe": "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5",
        "fields": [
            {
                "name": "ordinal",
                "data_type": "uint64",
                "nullable": False,
                "metadata": {
                    "codefabric.field_id": "query.result.ordinals.v1.ordinal",
                    "codefabric.field_ordinal": "0",
                    "codefabric.semantic_role": "deterministic_result_ordinal",
                },
            }
        ],
        "metadata": {
            "codefabric.relation_id": "query.result.ordinals.v1",
            "codefabric.semantic_encoding": "typed-arrow-result-resource",
            "codefabric.schema_contract_version": "1",
        },
    }
    contract["canonical_schema_identity"] = canonical_b3(contract)
    return contract


def result_artifact_identity(
    *,
    workspace_id: str,
    owning_agent_id: str,
    fabric_epoch_id: str,
    snapshot_id: str,
    canonical_response_checksum: str,
    format_name: str,
    format_version: str,
) -> dict[str, Any]:
    return _cbef_recipe_evidence(
        domain_code=23,
        domain_name="RESULT_ARTIFACT_V2",
        output_prefix="artifact",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                public_id_bytes(workspace_id, "workspace"),
                workspace_id,
            ),
            (
                2,
                "owning_agent_id",
                2,
                "UTF8",
                _cbef_utf8(owning_agent_id),
                owning_agent_id,
            ),
            (
                3,
                "fabric_epoch_id",
                7,
                "ID",
                public_id_bytes(fabric_epoch_id, "fabric-epoch"),
                fabric_epoch_id,
            ),
            (
                4,
                "snapshot_id",
                7,
                "ID",
                public_id_bytes(snapshot_id, "snapshot"),
                snapshot_id,
            ),
            (
                5,
                "canonical_response_checksum",
                8,
                "DIGEST",
                digest_bytes(canonical_response_checksum, "b3"),
                canonical_response_checksum,
            ),
            (
                6,
                "format",
                2,
                "UTF8",
                _cbef_utf8(format_name, ascii_lower=True),
                format_name,
            ),
            (
                7,
                "format_version",
                2,
                "UTF8",
                _cbef_utf8(format_version),
                format_version,
            ),
        ],
        excluded=[
            "resource URI",
            "physical storage path",
            "publication generation",
            "lease identity and expiry",
            "mutable access counters",
            "transport-specific emitted byte checksum",
        ],
    )


def resource_terminal(
    inputs: dict[str, Any], state: str, published: bool
) -> dict[str, Any]:
    batch = inputs["actual_output_batch"]
    if state == "complete":
        terminal_state = "complete"
        public_error = None
    elif state == "hard_limit_exceeded":
        terminal_state = "failed"
        public_error = "QUERY_HARD_LIMIT_EXCEEDED"
    elif state == "cancelled":
        terminal_state = "cancelled"
        public_error = "CANCELLED"
    else:
        raise ValueError(f"unsupported resource terminal state: {state}")
    return {
        "query_id": inputs["query_identity"]["query_id"],
        "state": terminal_state,
        "public_error": public_error,
        "published_rows": batch["row_count"] if published else 0,
        "byte_observation": "required_at_execution",
        "published_resources": 1 if published else 0,
        "resource_uri": inputs["registry_state"]["resource_uri"] if published else None,
        "complete_partial_list": False,
        "terminal_provenance": {
            "schema_contract": batch["schema_contract"],
            "ipc_contract": batch["ipc_contract"],
            "artifact_identity": batch["artifact_identity"],
            "reservation_state": "held" if published else "released",
            "cancellation": inputs["cancellation_state"],
            "lease_state": "active" if published else "released",
            "publication_state": "published" if published else "not_published",
        },
    }


def claim_015(base: dict[str, Any]) -> dict[str, Any]:
    schema = arrow_schema_contract()
    rows = [[1], [2], [3]]
    inputs = copy.deepcopy(base["complete_input_universe"]["inputs"])
    query_identity = inputs["query_identity"]
    query_identity.update(
        {
            "owning_agent_id": "wp33-production-oracle",
            "snapshot_id": "snapshot:11111111111111111111111111111111",
        }
    )
    canonical_response = {
        "query_id": query_identity["query_id"],
        "workspace_id": query_identity["workspace"],
        "fabric_epoch_id": query_identity["epoch"],
        "snapshot_id": query_identity["snapshot_id"],
        "relation_id": schema["relation_id"],
        "columns": ["ordinal"],
        "rows": rows,
        "coverage": {"state": "COMPLETE"},
        "errors": [],
    }
    canonical_response_checksum = canonical_b3(canonical_response)
    ipc_contract = {
        "format": "Arrow IPC stream",
        "metadata_version": "V5",
        "schema_message_count": 1,
        "dictionary_scope": "one stream",
        "physical_end_of_stream_required": True,
        "identity_input": "exact emitted IPC bytes observed at execution",
    }
    artifact_recipe = result_artifact_identity(
        workspace_id=query_identity["workspace"],
        owning_agent_id=query_identity["owning_agent_id"],
        fabric_epoch_id=query_identity["epoch"],
        snapshot_id=query_identity["snapshot_id"],
        canonical_response_checksum=canonical_response_checksum,
        format_name="arrow-ipc",
        format_version="V5",
    )
    artifact_id = artifact_recipe["output_id"]
    resource_uri = (
        "codefabric-result://"
        f"{query_identity['workspace'].removeprefix('workspace:')}/"
        f"{artifact_id.removeprefix('artifact:')}"
    )
    artifact_identity = {
        "artifact_id": artifact_id,
        "identity_recipe": artifact_recipe,
        "resource_uri": resource_uri,
        "workspace_id": query_identity["workspace"],
        "owning_agent_id": query_identity["owning_agent_id"],
        "fabric_epoch_id": query_identity["epoch"],
        "snapshot_id": query_identity["snapshot_id"],
        "canonical_schema_identity": schema["canonical_schema_identity"],
        "canonical_response_checksum": canonical_response_checksum,
        "format": "arrow-ipc",
        "format_version": "V5",
        "expected_ipc_identity_recipe": {
            "algorithm": "unkeyed BLAKE3",
            "input": "exact Arrow IPC V5 stream bytes",
            "output": "b3 lowercase hex",
            "frozen_without_bytes": False,
        },
        "immutable": True,
    }
    inputs["bound_plan"] = {
        "output_schema_contract": schema,
        "plan_schema_authority": "DataFusion LogicalPlan::schema",
        "estimated_rows": 3,
    }
    inputs["resource_budget"]["bytes"] = 4096
    inputs["reservation"] = {
        "memory_bytes": 4096,
        "spill_bytes": 0,
        "concurrency_slots": 1,
    }
    inputs["registry_state"].update(
        {
            "artifact_id": artifact_id,
            "resource_uri": resource_uri,
            "owning_agent_id": query_identity["owning_agent_id"],
            "workspace_id": query_identity["workspace"],
            "fabric_epoch_id": query_identity["epoch"],
            "snapshot_id": query_identity["snapshot_id"],
            "canonical_response_checksum": canonical_response_checksum,
            "format": "arrow-ipc",
            "format_version": "V5",
        }
    )
    inputs["lease_policy"]["artifact_id"] = artifact_id
    inputs["actual_output_batch"] = {
        "schema_contract": schema,
        "canonical_response": canonical_response,
        "rows": rows,
        "row_count": len(rows),
        "ipc_contract": ipc_contract,
        "measured_ipc_bytes": None,
        "artifact_identity": artifact_identity,
    }
    expected = resource_terminal(inputs, "complete", True)
    base.update(
        {
            "subject": "Resource publication binds an explicit canonical Arrow schema contract and exact Arrow IPC V5 identity recipe before lease/publication state can become visible",
            "author_id": AUTHOR,
            "source_anchor": "SRV §9 One logical response and delivery policy; SRV §10 Immutable result resources; AC-G-63 Immutable result artifact store",
            "governing_clauses": [
                "FAB §15 Executable acceptance obligations",
                "LIFE §15 Executable acceptance obligations",
                "P23 Local explicit state ownership",
            ],
            "complete_input_universe": {"closed": True, "inputs": inputs},
            "decoded_expectation": {
                "terminal": "complete",
                "relation": "resource.query_terminal",
                "columns": ["terminal"],
                "rows": [[expected]],
                "coverage": "the exact Arrow schema, canonical semantic response/checksum, CBEF-v1 owner/epoch/snapshot/format-bound artifact identity, stable result URI, IPC V5 stream contract, logical rows, lease, reservation, and publication identities are closed; actual IPC byte count and digest must be observed by execution",
            },
            "semantics": {
                "ordering": "one terminal per query identity after the immutable Arrow artifact is sealed",
                "nulls": "ordinal is non-null UInt64; measured IPC bytes remain unasserted until actual serialization",
                "unknowns": "missing schema, actual IPC bytes, cancellation, cleanup, lease, or publication observation prevents capability proof",
                "provenance": "terminal retains the explicit Arrow schema contract, canonical semantic response checksum, CBEF artifact recipe, IPC codec profile, stable URI, and exact lease/publication states",
            },
            "limitations": [
                "WP33 does not invent an IPC byte count or checksum; the later production oracle must serialize the declared Arrow schema and rows and bind the exact emitted IPC bytes."
            ],
        }
    )
    return base


HOST_REQUIRED = {
    "containment_substrate_available": True,
    "compiled_seccomp_policy_authorized": True,
    "no_new_privileges": True,
    "network_namespace_isolated": True,
    "credentials_stripped": True,
    "workspace_read_only": True,
    "unrelated_file_descriptors_closed": True,
    "process_group_and_cgroup_cleanup": True,
    "hostile_escape_matrix_executed": True,
}


def security_inputs(base: dict[str, Any]) -> dict[str, Any]:
    inputs = copy.deepcopy(base["complete_input_universe"]["inputs"])
    inputs["explicit_authorization"] = {
        "job_id": "trusted-local-request-1",
        "trusted_local": False,
        "authorization_id": "authorization:none",
    }
    inputs["trust_policy"] = {
        "policy_id": "rust-compilation-trust-v1",
        "default_profile": "untrusted",
        "trusted_local_requires_distinct_authorization": True,
        "trusted_local_is_degraded": True,
        "untrusted_requires_all_host_prerequisites": True,
        "fail_closed": True,
    }
    inputs["launcher_evidence_contract"] = {
        "contract_id": "rust-launcher-evidence-v2",
        "required_prerequisites": list(HOST_REQUIRED),
        "receipt_required_before_execution": True,
        "proof_required_for_capability": True,
        "absence_terminal": "SANDBOX_UNAVAILABLE",
        "hostile_actions_are_not_assumed_executed": True,
    }
    inputs["launcher_constraints"] = {
        "required": copy.deepcopy(HOST_REQUIRED),
        "observed_host": {
            "containment_substrate_available": True,
            "compiled_seccomp_policy_authorized": False,
            "no_new_privileges": "unproved_for_application_launcher",
            "network_namespace_isolated": "unproved",
            "credentials_stripped": "unproved",
            "workspace_read_only": "unproved",
            "unrelated_file_descriptors_closed": "unproved",
            "process_group_and_cgroup_cleanup": "cgroup substrate supported; full hostile cleanup matrix unproved",
            "hostile_escape_matrix_executed": False,
            "launcher_diagnostics": {
                "kind": "bubblewrap",
                "path": "/usr/bin/bwrap",
                "version": "0.9.0",
                "authority": "diagnostic_only",
            },
            "cgroup_v2": {
                "delegated_controllers": ["cpu", "memory", "pids"],
                "pre_exec_self_placement": True,
                "cpu_memory_pids_swap_bounds": True,
                "aggregate_kernel_accounting": True,
            },
        },
        "untrusted_admission": "unavailable",
    }
    return inputs


def security_unavailable_rows(inputs: dict[str, Any]) -> list[list[Any]]:
    policy = inputs["trust_policy"]["policy_id"]
    trusted = next(
        job
        for job in inputs["provider_jobs"]
        if job["requested_profile"] == "trusted_local"
    )
    hostile = next(
        job
        for job in inputs["provider_jobs"]
        if job["requested_profile"] == "untrusted"
    )
    preflight = {
        "policy_id": policy,
        "job_id": trusted["job_id"],
        "authorization": inputs["explicit_authorization"],
        "state": "denied",
    }
    unavailable = {
        "policy_id": policy,
        "job_id": hostile["job_id"],
        "required": inputs["launcher_constraints"]["required"],
        "observed": inputs["launcher_constraints"]["observed_host"],
        "state": "sandbox_unavailable",
    }
    return [
        [
            trusted["job_id"],
            "trusted_local_preflight",
            "denied",
            "CAPABILITY_UNAVAILABLE",
            None,
            None,
            canonical_b3(preflight),
            "not_advertised",
            0,
            [],
            "unknown",
        ],
        [
            hostile["job_id"],
            "untrusted_preflight",
            "sandbox_unavailable",
            "SANDBOX_UNAVAILABLE",
            None,
            None,
            canonical_b3(unavailable),
            "not_advertised",
            0,
            [
                {
                    "action_id": action["action_id"],
                    "attempted": False,
                    "contained": "unknown",
                    "expected_terminal": action["expected_terminal"],
                }
                for action in inputs["hostile_actions"]
            ],
            "unknown",
        ],
    ]


def claim_016(base: dict[str, Any]) -> dict[str, Any]:
    inputs = security_inputs(base)
    base.update(
        {
            "subject": "Untrusted provider execution is unavailable and not advertised when any host-containment prerequisite is absent; trusted-local remains separately authorized and explicitly degraded",
            "author_id": AUTHOR,
            "source_anchor": "GEN AC-G-35 Provider sandbox and Rust compilation trust model; GEN §7.5 Semantic compilation trust; GEN §85 Capability reporting",
            "governing_clauses": [
                "SUITE §5.1 Proof relations",
                "GEN §93 Provider fixture requirements",
                "P36 Governance is executable",
            ],
            "complete_input_universe": {"closed": True, "inputs": inputs},
            "decoded_expectation": {
                "terminal": "pass_with_security_denials",
                "relation": "security.provider_terminal",
                "columns": [
                    "job_id",
                    "scenario",
                    "state",
                    "public_error",
                    "launcher_receipt_id",
                    "launcher_proof_id",
                    "provenance_id",
                    "capability_state",
                    "attempted_action_count",
                    "hostile_action_closure",
                    "surviving_children",
                ],
                "rows": security_unavailable_rows(inputs),
                "coverage": "trusted-local is denied without distinct authorization; untrusted execution is rejected before launch because an authorized compiled seccomp policy and the full hostile escape matrix are absent; zero hostile actions are reported attempted",
            },
            "semantics": {
                "ordering": "trusted-local authorization preflight before untrusted host-capability preflight",
                "nulls": "launcher receipt and proof identities are null because no conforming hostile execution occurred",
                "unknowns": "containment and surviving-child outcomes remain unknown when execution is not admitted",
                "provenance": "the denial retains policy, job, complete required-prerequisite vector, exact observed host limitations, typed absence of launcher proof, and zero attempted actions",
            },
            "limitations": [
                "A containment substrate is observed, but no application-owned compiled seccomp policy is authorized and the network, credential, live-workspace, inherited-fd, and cleanup escape matrix is unproved; launcher kind, path, and version are diagnostic only and never gate capability.",
                "The cgroup-v2 substrate supports delegated cpu, memory, and pids control with kernel accounting and kill/reap, but this does not substitute for the absent sandbox and hostile-matrix proof.",
                "Trusted-local is degraded and never an untrusted substitute; authorization proves only launch admission, while execution receipts and terminals require separate runtime observations.",
            ],
        }
    )
    return base


def fixture_base(fixture: dict[str, Any]) -> dict[str, Any]:
    fixture["author_id"] = AUTHOR
    fixture["imports"] = []
    fixture["semantic"] = True
    fixture["integrity_only"] = False
    return fixture


def replace_fixture(
    fixture: dict[str, Any],
    *,
    source_anchor: str,
    fault_dimension: str,
    change: str,
    terminal: str,
    expected: dict[str, Any],
    role: str,
    pointer: str,
    before: Any,
    after: Any,
    basis: str,
) -> dict[str, Any]:
    fixture_base(fixture)
    fixture.update(
        {
            "source_anchor": source_anchor,
            "fault_dimension": fault_dimension,
            "authoritative_change": change,
            "expected_terminal": terminal,
            "expected_decoded": expected,
            "semantic_basis": basis,
            "mutation": {
                "input_role": role,
                "json_pointer": pointer,
                "before": before,
                "after": after,
            },
        }
    )
    return fixture


def rewrite_query_oracle_fixtures(
    fixtures: list[dict[str, Any]], claims: dict[str, dict[str, Any]]
) -> None:
    """Re-author the non-identity query fixtures from their mutated inputs."""

    by_id = {fixture["fixture_id"]: fixture for fixture in fixtures}

    c4 = claims["RFV3-CLAIM-004"]
    c4_inputs = c4["complete_input_universe"]["inputs"]
    request_before = c4_inputs["request_envelope"]
    request_after = copy.deepcopy(request_before)
    request_after["decoded"]["queries"][0]["looking_for"] = "function"
    request_after["canonical_json"] = rfc8785.dumps(request_after["decoded"]).decode(
        "utf-8"
    )
    causal_inputs = copy.deepcopy(c4_inputs)
    causal_inputs["request_envelope"] = request_after
    causal_block = request_after["decoded"]["queries"][0]
    selected, resolved, coverage = _find_entity_selection(causal_inputs, causal_block)
    replace_fixture(
        by_id["RFV3-FIX-004-C"],
        source_anchor="QRY §4.1 `find code entities`; QRY §5 Composition DAG and execution semantics",
        fault_dimension="authoritative_input",
        change="Change only the requested typed description from function syntax to function; the pinned program, admitted candidate rows, authorized catalog, and complete coverage remain fixed.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": resolved,
            "query_result": {
                "query_id": causal_block["query_id"],
                "result_role": "entities",
                "entity_ids": [row["entity_id"] for row in selected],
                "entities": selected,
                "coverage": coverage,
                "errors": [],
                "notices": [],
            },
            "errors": [],
        },
        role="request_envelope",
        pointer="",
        before=request_before,
        after=request_after,
        basis="The typed description mapping selects the semantic callable row rather than the syntax occurrence, while every other executable input remains identical.",
    )

    excluded_request = copy.deepcopy(request_before)
    excluded_request["decoded"]["queries"][0]["looking_for"] = (
        "runtime-covered function"
    )
    excluded_request["canonical_json"] = rfc8785.dumps(
        excluded_request["decoded"]
    ).decode("utf-8")
    excluded_error = {
        "code": "NOT_OBJECTIVE_FACT_REQUEST",
        "layer": "semantic_resolution",
        "safe_message": "Runtime observation and coverage are outside the present-state fact substrate.",
        "field": "looking_for",
        "semantic_phrase": "runtime-covered function",
        "candidate_interpretations": [
            "find function declarations",
            "retrieve static source/semantic facts",
        ],
        "retryable": False,
        "failed_dependency_query_id": None,
        "diagnostic_id": None,
    }
    replace_fixture(
        by_id["RFV3-FIX-004-N"],
        source_anchor="QRY §4.1 `find code entities`; QRY §6 Resolution, authorization, and bound authority",
        fault_dimension="objectivity",
        change="Replace the admitted typed entity description with an excluded runtime-coverage judgment while leaving program, catalog, access, rows, and coverage fixed.",
        terminal="reject",
        expected={
            "execution_state": "FAILED",
            "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "rejected_phrase": "runtime-covered function",
                "fact_equivalent_rewrites": excluded_error["candidate_interpretations"],
            },
            "query_result": {
                "query_id": causal_block["query_id"],
                "result_role": "entities",
                "entity_ids": [],
                "coverage": {"state": "NOT_APPLICABLE", "reason": "excluded domain"},
                "errors": [excluded_error],
                "notices": [],
            },
            "errors": [excluded_error],
        },
        role="request_envelope",
        pointer="",
        before=request_before,
        after=excluded_request,
        basis="The excluded phrase is absent from the pinned description program and is rejected before any admitted row can be misread as runtime-observation evidence.",
    )

    c6 = claims["RFV3-CLAIM-006"]
    c6_inputs = c6["complete_input_universe"]["inputs"]
    c6_block = c6_inputs["request_envelope"]["decoded"]["queries"][0]
    c6_edges = _selected_follow_edges(c6_inputs, c6_block)
    c6_coverage_before = c6_inputs["producer_coverage"]
    c6_causal = by_id["RFV3-FIX-006-C"]
    c6_mutation_after = c6_causal["mutation"]["after"]
    c6_relations_after = copy.deepcopy(
        c6_mutation_after.get("admitted_relations", c6_mutation_after)
    )
    c6_causal_inputs = copy.deepcopy(c6_inputs)
    c6_causal_inputs["admitted_relations"] = c6_relations_after
    c6_causal_edges = _selected_follow_edges(c6_causal_inputs, c6_block)
    baseline_fact_ids = {edge["fact_id"] for edge in c6_edges}
    added_edges = [
        edge for edge in c6_causal_edges if edge["fact_id"] not in baseline_fact_ids
    ]
    if len(added_edges) != 1:
        raise ValueError("follow causal fixture must add exactly one selected edge")
    added_edge = added_edges[0]
    added_entity = c6_relations_after["entity_dictionary"][
        added_edge["statement"]["object"]
    ]
    c6_causal_coverage = copy.deepcopy(c6_coverage_before)
    c6_causal_coverage["covered_fact_ids"] = sorted(
        edge["fact_id"] for edge in c6_causal_edges
    )
    replace_fixture(
        c6_causal,
        source_anchor="QRY §4.3 `follow code relationships`; QRY §5 Composition DAG and execution semantics",
        fault_dimension="authoritative_input",
        change="Atomically add one admitted owner-scoped call edge, its target entity, and the corresponding complete-coverage membership.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "starting_from": c6_block["starting_from"],
                "relationship": c6_block["relationship"],
                "direction": c6_block["direction"],
                "distance": c6_block["distance"],
            },
            "query_result": {
                "query_id": c6_block["query_id"],
                "result_role": "facts",
                "fact_ids": [edge["fact_id"] for edge in c6_causal_edges],
                "added_fact": added_edge,
                "added_entity": added_entity,
                "coverage": {
                    "state": c6_causal_coverage["state"],
                    "owner": c6_causal_coverage["owner"],
                    "analysis_context_id": c6_causal_coverage["analysis_context_id"],
                    "distance": c6_block["distance"],
                    "completed_family": c6_causal_coverage["family"],
                },
                "errors": [],
                "notices": [],
            },
            "errors": [],
        },
        role="$input_universe",
        pointer="",
        before={
            "admitted_relations": c6_inputs["admitted_relations"],
            "producer_coverage": c6_coverage_before,
        },
        after={
            "admitted_relations": c6_relations_after,
            "producer_coverage": c6_causal_coverage,
        },
        basis="The exact one-step result grows only because the admitted relationship relation and its complete owner/family/context coverage terminal grow atomically.",
    )
    c6_remainder = {
        "kind": "unknown_relationship_remainder",
        "owner_id": c6_coverage_before["owner"],
        "analysis_context_id": c6_coverage_before["analysis_context_id"],
        "family": c6_coverage_before["family"],
        "direction": c6_block["direction"],
        "distance": c6_block["distance"],
        "reason": "PRODUCER_COVERAGE_PARTIAL",
        "retryable": True,
    }
    c6_coverage_after = copy.deepcopy(c6_coverage_before)
    c6_coverage_after["state"] = "PARTIAL"
    c6_coverage_after["remainders"] = [c6_remainder]
    replace_fixture(
        by_id["RFV3-FIX-006-N"],
        source_anchor="QRY §4.3 `follow code relationships`; QRY §7 Evidence, unknowns, absence, and provenance",
        fault_dimension="coverage",
        change="Retain every admitted one-step call fact but replace the complete owner/family/context terminal with a typed partial terminal and explicit unknown relationship remainder.",
        terminal="PARTIAL",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "PARTIAL",
            "completeness_state": "PARTIAL",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "starting_from": c6_block["starting_from"],
                "relationship": c6_block["relationship"],
                "direction": c6_block["direction"],
                "distance": c6_block["distance"],
            },
            "query_result": {
                "query_id": c6_block["query_id"],
                "result_role": "facts",
                "fact_ids": [edge["fact_id"] for edge in c6_edges],
                "facts": {edge["fact_id"]: edge for edge in c6_edges},
                "remainders": [c6_remainder],
                "coverage": {
                    "state": "PARTIAL",
                    "owner": c6_coverage_after["owner"],
                    "analysis_context_id": c6_coverage_after["analysis_context_id"],
                    "completed_family": c6_coverage_after["family"],
                    "distance": c6_block["distance"],
                    "covered_fact_ids": c6_coverage_after["covered_fact_ids"],
                    "remainders": [c6_remainder],
                },
                "errors": [],
                "notices": [],
            },
            "errors": [],
        },
        role="producer_coverage",
        pointer="",
        before=c6_coverage_before,
        after=c6_coverage_after,
        basis="Known admitted edges remain objective facts; partial coverage adds an explicit typed remainder and prevents those known facts from being mistaken for an exhaustive traversal.",
    )

    c7 = claims["RFV3-CLAIM-007"]
    c7_inputs = c7["complete_input_universe"]["inputs"]
    c7_block = c7_inputs["request_envelope"]["decoded"]["queries"][0]
    c7_fixture = by_id["RFV3-FIX-007-C"]
    c7_prior_mutation = c7_fixture["mutation"]
    c7_after_value = c7_prior_mutation["after"]
    c7_after_edges = (
        c7_after_value["admitted_relations"]["edges"]
        if c7_prior_mutation.get("input_role") == "$input_universe"
        else c7_after_value
    )
    c7_relations_before = c7_inputs["admitted_relations"]
    c7_relations_after = copy.deepcopy(c7_relations_before)
    c7_relations_after["edges"] = copy.deepcopy(c7_after_edges)
    c7_coverage_before = c7_inputs["producer_coverage"]
    c7_coverage_after = copy.deepcopy(c7_coverage_before)
    c7_coverage_after["fact_ids"] = sorted(edge["fact_id"] for edge in c7_after_edges)
    c7_fixture["mutation"] = {
        "input_role": "$input_universe",
        "json_pointer": "",
        "before": {
            "admitted_relations": c7_relations_before,
            "producer_coverage": c7_coverage_before,
        },
        "after": {
            "admitted_relations": c7_relations_after,
            "producer_coverage": c7_coverage_after,
        },
    }
    c7_fixture["authoritative_change"] = (
        "Atomically remove the baseline short call edge and its exact coverage membership "
        "while retaining the alternate admitted route and all covered entities."
    )
    c7_entities, c7_facts = _canonical_shortest_witness(
        c7_after_edges,
        start=c7_block["from"][0],
        target=c7_block["to"][0],
        families=c7_block["using"],
        maximum_length=c7_inputs["resource_limits"]["max_path_length"],
    )
    c7_result = c7_fixture["expected_decoded"]["query_result"]
    c7_path = c7_result["paths"][0]
    c7_path.update(
        {
            "ordered_entity_ids": c7_entities,
            "ordered_fact_ids": c7_facts,
            "length": len(c7_facts),
            "path_policy": c7_block["path_policy"],
            "certainty_summary": "exact",
        }
    )
    c7_recipe = _path_result_recipe(
        workspace_id=c7_inputs["request_envelope"]["decoded"]["scope"]["workspace_id"],
        analysis_context_id=c7_coverage_after["analysis_context_id"],
        fabric_epoch_id=c7_inputs["pinned_epoch"]["fabric_epoch_id"],
        policy_identity=c7_inputs["pinned_epoch"]["policy_release"],
        ordered_entity_ids=c7_entities,
        ordered_fact_ids=c7_facts,
    )
    replace_identity_everywhere(
        c7_fixture["expected_decoded"], c7_path["path_id"], c7_recipe["output_id"]
    )
    c7_path["path_id"] = c7_recipe["output_id"]
    c7_result["path_ids"] = [c7_recipe["output_id"]]
    c7_result["identity_contract"] = {
        "path_id": c7_recipe["output_id"],
        "identity_recipe": c7_recipe,
        "witness_bound": True,
    }
    c7_result["coverage"] = {
        "state": "COMPLETE",
        "searched_fact_count": len(c7_after_edges),
    }
    c7_fixture["expected_decoded"]["resolved_semantics"] = {
        "from": c7_block["from"],
        "to": c7_block["to"],
        "relationship_families": c7_block["using"],
        "path_policy": c7_block["path_policy"],
        "maximum_path_length": c7_inputs["resource_limits"]["max_path_length"],
    }
    c7_fixture["semantic_basis"] = (
        "Removing the baseline short edge forces the independently derived canonical "
        "shortest witness to change; its identity contract remains owned by the separate "
        "CBEF query-identity work."
    )

    c8 = claims["RFV3-CLAIM-008"]
    c8_inputs = c8["complete_input_universe"]["inputs"]
    c8_block = c8_inputs["request_envelope"]["decoded"]["queries"][0]
    c8_relations_before = c8_inputs["admitted_relations"]
    old_causal = by_id["RFV3-FIX-008-C"]
    c8_prior_mutation = old_causal["mutation"]
    c8_after_value = c8_prior_mutation["after"]
    old_after_edges = (
        c8_after_value["admitted_relations"]["call_edges"]
        if c8_prior_mutation.get("input_role") == "$input_universe"
        else c8_after_value
    )
    before_fact_ids = {edge["fact_id"] for edge in c8_relations_before["call_edges"]}
    added_edge = next(
        edge for edge in old_after_edges if edge["fact_id"] not in before_fact_ids
    )
    c8_relations_after = copy.deepcopy(c8_relations_before)
    c8_relations_after["call_edges"].append(copy.deepcopy(added_edge))
    c8_coverage_before = c8_inputs["producer_coverage"]
    c8_coverage_after = copy.deepcopy(c8_coverage_before)
    c8_coverage_after["covered_fact_ids"] = sorted(
        [*c8_coverage_after["covered_fact_ids"], added_edge["fact_id"]]
    )
    c8_causal_inputs = copy.deepcopy(c8_inputs)
    c8_causal_inputs["admitted_relations"] = c8_relations_after
    c8_causal_inputs["producer_coverage"] = c8_coverage_after
    causal_matches = _typed_pattern_matches(c8_causal_inputs, c8_block)
    if causal_matches:
        raise ValueError("the causal outgoing edge did not invalidate scoped negation")
    replace_fixture(
        old_causal,
        source_anchor="QRY §4.5 `match a code fact pattern`; QRY §7 Evidence, unknowns, absence, and provenance",
        fault_dimension="authoritative_input",
        change="Atomically add one admitted f-calls-g fact and its exact coverage membership while retaining the typed g-calls-f positive edge and every node binding input.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
                "typed_bindings": {
                    node["binding"]: node["semantic_kind"]
                    for node in c8_block["pattern"]["nodes"]
                },
                "positive_fact_count": len(c8_block["pattern"]["facts"]),
                "scoped_negation_universe": c8_coverage_after[
                    "negative_proof_universe_id"
                ],
            },
            "query_result": {
                "query_id": c8_block["query_id"],
                "result_role": "pattern_bindings",
                "bindings": [],
                "entity_ids": [],
                "fact_ids": [],
                "evaluated_fact_ids": c8_coverage_after["covered_fact_ids"],
                "coverage": {
                    "state": "COMPLETE",
                    "outcome": "NO_MATCH_AFTER_FILTERS",
                    "owner_scope": c8_coverage_after["owner_scope"],
                    "analysis_context_id": c8_coverage_after["analysis_context_id"],
                    "family": c8_coverage_after["family"],
                    "covered_subject_ids": c8_coverage_after["covered_subject_ids"],
                    "covered_fact_ids": c8_coverage_after["covered_fact_ids"],
                    "negative_proof_universe_id": c8_coverage_after[
                        "negative_proof_universe_id"
                    ],
                },
                "errors": [],
                "notices": [],
            },
            "errors": [],
        },
        role="$input_universe",
        pointer="",
        before={
            "admitted_relations": c8_relations_before,
            "producer_coverage": c8_coverage_before,
        },
        after={
            "admitted_relations": c8_relations_after,
            "producer_coverage": c8_coverage_after,
        },
        basis="The positive typed edge still binds g to f, but the newly admitted outgoing fact for f makes the complete scoped-negation clause false and removes the final match.",
    )

    c8_remainder = {
        "kind": "unknown_scoped_negation_remainder",
        "owner_scope": c8_coverage_before["owner_scope"],
        "analysis_context_id": c8_coverage_before["analysis_context_id"],
        "family": c8_coverage_before["family"],
        "covered_subject_ids": c8_coverage_before["covered_subject_ids"],
        "reason": "NEGATIVE_PROOF_UNIVERSE_PARTIAL",
        "retryable": True,
    }
    c8_partial = copy.deepcopy(c8_coverage_before)
    c8_partial["state"] = "PARTIAL"
    c8_partial["remainders"] = [c8_remainder]
    c8_partial_inputs = copy.deepcopy(c8_inputs)
    c8_partial_inputs["producer_coverage"] = c8_partial
    partial_matches = _typed_pattern_matches(
        c8_partial_inputs, c8_block, indeterminate=True
    )
    positive_fact_ids = sorted(
        {
            fact_id
            for match in partial_matches
            for fact_id in match["supporting_fact_ids"]
        }
    )
    positive_entity_ids = sorted(
        {
            value["entity_id"]
            for match in partial_matches
            for value in match["bindings"].values()
        }
    )
    indeterminate_error = {
        "code": "NEGATIVE_PROOF_INDETERMINATE",
        "layer": "coverage",
        "safe_message": "Scoped negation requires complete owner, family, context, subject, and fact coverage.",
        "field": "pattern.scoped_negation",
        "semantic_phrase": None,
        "candidate_interpretations": [],
        "retryable": True,
        "failed_dependency_query_id": None,
        "diagnostic_id": None,
    }
    replace_fixture(
        by_id["RFV3-FIX-008-N"],
        source_anchor="QRY §4.5 `match a code fact pattern`; QRY §7 Evidence, unknowns, absence, and provenance",
        fault_dimension="coverage",
        change="Retain the admitted nodes and positive g-calls-f fact but replace the complete scoped negative universe with a typed partial terminal and explicit remainder.",
        terminal="INDETERMINATE",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "PARTIAL",
            "completeness_state": "INDETERMINATE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
                "typed_bindings": {
                    node["binding"]: node["semantic_kind"]
                    for node in c8_block["pattern"]["nodes"]
                },
                "positive_fact_count": len(c8_block["pattern"]["facts"]),
                "scoped_negation_universe": c8_partial["negative_proof_universe_id"],
            },
            "query_result": {
                "query_id": c8_block["query_id"],
                "result_role": "pattern_bindings",
                "bindings": partial_matches,
                "entity_ids": positive_entity_ids,
                "fact_ids": positive_fact_ids,
                "entity_records": {
                    entity_id: c8_relations_before["entity_dictionary"][entity_id]
                    for entity_id in positive_entity_ids
                },
                "facts": {
                    fact_id: next(
                        edge
                        for edge in c8_relations_before["call_edges"]
                        if edge["fact_id"] == fact_id
                    )
                    for fact_id in positive_fact_ids
                },
                "remainders": [c8_remainder],
                "coverage": {
                    "state": "PARTIAL",
                    "owner_scope": c8_partial["owner_scope"],
                    "analysis_context_id": c8_partial["analysis_context_id"],
                    "family": c8_partial["family"],
                    "covered_subject_ids": c8_partial["covered_subject_ids"],
                    "covered_fact_ids": c8_partial["covered_fact_ids"],
                    "negative_proof_universe_id": c8_partial[
                        "negative_proof_universe_id"
                    ],
                    "remainders": [c8_remainder],
                },
                "errors": [indeterminate_error],
                "notices": [],
            },
            "errors": [indeterminate_error],
        },
        role="producer_coverage",
        pointer="",
        before=c8_coverage_before,
        after=c8_partial,
        basis="The positive typed bindings and supporting fact remain known, while only the scoped negation is explicitly indeterminate under the partial owner/family/context universe.",
    )


def rewrite_fixtures(
    fixtures: list[dict[str, Any]], claims: dict[str, dict[str, Any]]
) -> None:
    by_id = {fixture["fixture_id"]: fixture for fixture in fixtures}
    c1 = claims["RFV3-CLAIM-001"]
    c1_inputs = c1["complete_input_universe"]["inputs"]
    typed_before = c1_inputs["source_images"][1]
    typed_after = copy.deepcopy(typed_before)
    typed_after["bytes_utf8"] = "def f(x: int) -> int:\n    return len([x])\n"
    typed_after["content_digest"] = bytes_b3(typed_after["bytes_utf8"].encode())
    pyrefly_request = next(
        r for r in c1_inputs["provider_requests"] if r["provider_id"] == "pyrefly"
    )
    replace_fixture(
        by_id["RFV3-FIX-001-C"],
        source_anchor="GEN §8 Immutable source-image contract; GEN §20 Python type generation",
        fault_dimension="provider_batch",
        change="Replace only the immutable typed Python source image and matching BLAKE3 content digest; the Pyrefly relation contract and run identity remain fixed.",
        terminal="changed",
        expected={
            "provider_id": "pyrefly",
            "provider_run_id": pyrefly_request["provider_run_id"],
            "relation_id": PYREFLY_CONTRACT["relation_id"],
            "source_digest": typed_after["content_digest"],
            "native_fields": {
                "call_occurrence_ordinal": 0,
                "start_byte": 33,
                "end_byte": 36,
                "target_ordinal": 0,
                "callee_kind": "function",
                "qualified_target": "len",
                "class_name": None,
                "resolution_state": "resolved",
            },
        },
        role="source_images",
        pointer="/1",
        before=typed_before,
        after=typed_after,
        basis="Provider observations must change with the exact admitted source bytes while the application-owned relation contract remains stable.",
    )
    terminal_before = next(
        t for t in c1_inputs["coverage_terminals"] if t["provider_id"] == "ruff"
    )
    terminal_after = copy.deepcopy(terminal_before)
    terminal_after.update({"state": "open", "completed_units": 0, "remainders": []})
    replace_fixture(
        by_id["RFV3-FIX-001-N"],
        source_anchor="GEN §10 Provider-observation metadata",
        fault_dimension="coverage",
        change="Replace the Ruff terminal with an open terminal that completes no unit and declares no remainder.",
        terminal="reject",
        expected={
            "error": "PROVIDER_REQUESTED_COVERAGE_OPEN",
            "relation_id": RUFF_CONTRACT["relation_id"],
        },
        role="coverage_terminals",
        pointer=f"/{c1_inputs['coverage_terminals'].index(terminal_before)}",
        before=terminal_before,
        after=terminal_after,
        basis="Requested minus completed minus intentional remainder must be empty before coverage can close.",
    )

    c2 = claims["RFV3-CLAIM-002"]
    c2_definition = c2["complete_input_universe"]["inputs"]["transformation_definition"]
    predicate_value = c2_definition["plan_building_function"]["operations"][0][
        "predicate"
    ]["right"]["value"]
    replace_fixture(
        by_id["RFV3-FIX-002-C"],
        source_anchor="GEN §92 Programmatic relation interface; GEN AC-G-38 Programmatic transformation, matching, and trust",
        fault_dimension="transformation",
        change="Change the typed filter literal from function_definition to identifier while keeping the admitted relation and remaining typed plan fixed.",
        terminal="changed",
        expected={
            "terminal": "pass",
            "relation": "canonical.function_syntax",
            "columns": ["subject", "ordinal"],
            "rows": [["n2", 1]],
            "coverage": "all three admitted input rows consumed",
        },
        role="transformation_definition",
        pointer="/plan_building_function/operations/0/predicate/right/value",
        before=predicate_value,
        after="identifier",
        basis="The author-declared typed predicate is executable authority: changing its literal must change the DataFusion result without changing the admitted rows.",
    )
    fixture_base(by_id["RFV3-FIX-002-N"])

    c3 = claims["RFV3-CLAIM-003"]
    c3_inputs = c3["complete_input_universe"]["inputs"]
    provider_before = c3_inputs["provider_call_targets"]
    causal_bundle = derived_call_target_fixture("fixture.beta")
    replace_fixture(
        by_id["RFV3-FIX-003-C"],
        source_anchor="GEN §72 Call graph; GEN §95 Algorithm validation",
        fault_dimension="provider_batch",
        change="Change only the Pyrefly-native qualified_target for the direct observation from fixture.alpha to fixture.beta; preserve its exact source coordinates, the separate canonical call occurrence/owner relation, the canonical callable lookup, and the dynamic candidate set.",
        terminal="changed",
        expected={
            "relation": "analysis.common_call_graph.v1",
            "columns": c3["decoded_expectation"]["columns"],
            "rows": derived_call_graph_rows(causal_bundle),
        },
        role="provider_call_targets",
        pointer="/rows/0/5",
        before=provider_before["rows"][0][5],
        after="fixture.beta",
        basis="The application-owned call graph changes its callee only after the provider-native qualified target joins to the separately canonicalized callable; no provider row authors caller, callee, or call-site identity.",
    )
    partial_bundle = derived_call_target_fixture(partial=True)
    provider_partial = partial_bundle["provider_call_targets"]
    replace_fixture(
        by_id["RFV3-FIX-003-N"],
        source_anchor="GEN §72 Call graph; GEN §84 Explicit unknown-materialization rules",
        fault_dimension="coverage",
        change="Preserve the known direct alpha edge while replacing only the uncovered dynamic target set with a typed partial terminal and an explicit provider remainder.",
        terminal="explicit_unknown",
        expected=derived_partial_call_graph_expectation(partial_bundle),
        role="provider_call_targets",
        pointer="",
        before=provider_before,
        after=provider_partial,
        basis="Partial candidate coverage preserves every derivable known edge and adds an unknown only for the uncovered call site; it never erases the direct alpha fact or publishes a false negative edge.",
    )
    c5 = claims["RFV3-CLAIM-005"]
    c5_relations = c5["complete_input_universe"]["inputs"]["admitted_relations"]
    admitted_type = next(
        r for r in c5_relations["fact_rows"] if r["fact_kind"] == "type"
    )
    known_type = next(
        r
        for r in c5["decoded_expectation"]["rows"][0][0]["facts"].values()
        if r["fact_kind"] == "type"
    )
    unknown = next(
        r
        for r in c5["decoded_expectation"]["rows"][0][0]["facts"].values()
        if r["fact_kind"] == "unknown"
    )
    for fixture_id in ("RFV3-FIX-005-C", "RFV3-FIX-005-N"):
        fixture = fixture_base(by_id[fixture_id])
        result = fixture["expected_decoded"]["query_result"]
        prior_ids = list(result["fact_ids"])
        prior_type_id = next(
            value for value in prior_ids if value.startswith("fact:type:")
        )
        prior_unknown_id = next(value for value in prior_ids if value != prior_type_id)
        replace_identity_everywhere(fixture, prior_type_id, known_type["fact_id"])
        replace_identity_everywhere(fixture, prior_unknown_id, unknown["fact_id"])
        result["identity_contract"] = {
            "known_type_fact_id": known_type["fact_id"],
            "known_type_identity_recipe": copy.deepcopy(known_type["identity_recipe"]),
            "unknown_fact_id": unknown["fact_id"],
            "identity_recipe": copy.deepcopy(unknown["identity_recipe"]),
            "input_set_id": c5_relations["input_set_identity"]["output_id"],
            "input_set_identity_recipe": copy.deepcopy(
                c5_relations["input_set_identity"]
            ),
            "source_file_id": next(
                row
                for row in c5_relations["coverage_rows"]
                if row["family"] == "effects"
            )["source_identity"]["file_id"],
        }
    causal_type = copy.deepcopy(admitted_type)
    causal_type["statement"]["object"]["return"]["name"] = "str"
    causal_type_recipe = _query_property_fact_recipe(
        causal_type,
        property_kind_code=causal_type["property_kind_code"],
        output_prefix="fact:type",
        canonical_value=causal_type["statement"]["object"],
        excluded=[
            "source and producer provenance",
            "input-set and policy identity",
            "diagnostic evidence",
            "mutable coverage counters",
        ],
    )
    causal_type["fact_id"] = causal_type_recipe["output_id"]
    causal_type["identity_recipe"] = causal_type_recipe
    causal_relations = copy.deepcopy(c5_relations)
    causal_relations["fact_rows"] = [causal_type]
    causal_input_set = _retrieve_fact_input_set_recipe(
        causal_relations,
        str(c5["complete_input_universe"]["inputs"]["pinned_epoch"]["policy_release"]),
    )
    causal_relations["input_set_identity"] = causal_input_set
    causal_type["direct_provenance"]["input_set_id"] = causal_input_set["output_id"]
    causal_relations["fact_rows"] = [causal_type]
    replace_identity_everywhere(
        by_id["RFV3-FIX-005-C"],
        known_type["fact_id"],
        causal_type_recipe["output_id"],
    )
    causal_fixture = by_id["RFV3-FIX-005-C"]
    causal_fixture["authoritative_change"] = (
        "Replace the one admitted type PROPERTY_FACT row with the same proposition carrying return type str and its rederived CBEF-v1.1 identity; effects coverage remains unavailable."
    )
    causal_fixture["semantic_basis"] = (
        "Changing the admitted canonical type value rederives that fact identity, its CBEF objective input set, and the returned known fact; the separately materialized UNKNOWN_EFFECT proposition is unchanged."
    )
    causal_fixture["mutation"] = {
        "input_role": "admitted_relations",
        "json_pointer": "",
        "before": c5_relations,
        "after": causal_relations,
    }
    causal_identity = by_id["RFV3-FIX-005-C"]["expected_decoded"]["query_result"][
        "identity_contract"
    ]
    causal_identity["known_type_fact_id"] = causal_type_recipe["output_id"]
    causal_identity["known_type_identity_recipe"] = causal_type_recipe
    causal_identity["input_set_id"] = causal_input_set["output_id"]
    causal_identity["input_set_identity_recipe"] = causal_input_set

    negative_fixture = by_id["RFV3-FIX-005-N"]
    negative_relations = copy.deepcopy(c5_relations)
    negative_effects = next(
        row for row in negative_relations["coverage_rows"] if row["family"] == "effects"
    )
    negative_effects["state"] = "PARTIAL"
    negative_input_set = _retrieve_fact_input_set_recipe(
        negative_relations,
        str(c5["complete_input_universe"]["inputs"]["pinned_epoch"]["policy_release"]),
    )
    negative_relations["input_set_identity"] = negative_input_set
    for row in negative_relations["fact_rows"]:
        row["direct_provenance"]["input_set_id"] = negative_input_set["output_id"]
    negative_identity = negative_fixture["expected_decoded"]["query_result"][
        "identity_contract"
    ]
    negative_identity["input_set_id"] = negative_input_set["output_id"]
    negative_identity["input_set_identity_recipe"] = negative_input_set
    negative_fixture["authoritative_change"] = (
        "Atomically change effects coverage from UNAVAILABLE to PARTIAL and reissue the exact CBEF objective input-set identity; the UNKNOWN_EFFECT proposition remains stable because mutable coverage is excluded from its identity."
    )
    negative_fixture["semantic_basis"] = (
        "The typed unknown keeps its proposition identity while its separately bound coverage provenance and objective input-set identity change with the admitted partial remainder."
    )
    negative_fixture["mutation"] = {
        "input_role": "admitted_relations",
        "json_pointer": "",
        "before": c5_relations,
        "after": negative_relations,
    }

    c6 = claims["RFV3-CLAIM-006"]
    c6_inputs = c6["complete_input_universe"]["inputs"]
    c6_fixture = by_id["RFV3-FIX-006-C"]
    c6_expected = copy.deepcopy(c6_fixture["expected_decoded"])
    added_fact = copy.deepcopy(c6_expected["query_result"]["added_fact"])
    added_entity = copy.deepcopy(c6_expected["query_result"]["added_entity"])
    relations_before = c6_inputs["admitted_relations"]
    relations_after = copy.deepcopy(relations_before)
    relations_after["call_edges"].append(added_fact)
    relations_after["entity_dictionary"][added_entity["entity_id"]] = added_entity
    coverage_before = c6_inputs["producer_coverage"]
    coverage_after = copy.deepcopy(coverage_before)
    coverage_after["covered_fact_ids"] = sorted(
        [*coverage_after["covered_fact_ids"], added_fact["fact_id"]]
    )
    selected_edges = sorted(
        [
            edge
            for edge in relations_after["call_edges"]
            if edge["statement"]["subject"] == coverage_after["owner"]
            and edge["statement"]["predicate"] == coverage_after["family"]
        ],
        key=lambda edge: (edge["statement"]["object"], edge["fact_id"]),
    )
    c6_expected["resolved_semantics"] = {
        "starting_from": [coverage_after["owner"]],
        "relationship": coverage_after["family"],
        "direction": "outgoing",
        "distance": 1,
    }
    c6_expected["query_result"]["fact_ids"] = [
        edge["fact_id"] for edge in selected_edges
    ]
    c6_expected["query_result"]["coverage"] = {
        "state": coverage_after["state"],
        "owner": coverage_after["owner"],
        "analysis_context_id": coverage_after["analysis_context_id"],
        "distance": 1,
        "completed_family": coverage_after["family"],
    }
    replace_fixture(
        c6_fixture,
        source_anchor="QRY §4.3 `follow code relationships`; QRY §7 Evidence, unknowns, absence, and provenance",
        fault_dimension="authoritative_input",
        change="Atomically add one admitted call relationship, its canonical target entity, and the producer-coverage membership proving that exact new fact is complete.",
        terminal="changed",
        expected=c6_expected,
        role="$input_universe",
        pointer="",
        before={
            "admitted_relations": relations_before,
            "producer_coverage": coverage_before,
        },
        after={
            "admitted_relations": relations_after,
            "producer_coverage": coverage_after,
        },
        basis="The one-step traversal may emit the new fact only when the typed edge, target dictionary record, and exact producer-coverage set change in one authoritative input transaction.",
    )
    fixture_base(by_id["RFV3-FIX-006-N"])

    c7 = claims["RFV3-CLAIM-007"]
    path = next(iter(c7["decoded_expectation"]["rows"][0][0]["paths"].values()))
    for fixture_id in ("RFV3-FIX-007-C", "RFV3-FIX-007-N"):
        fixture_base(by_id[fixture_id])
        replace_identity_everywhere(
            by_id[fixture_id], "path:77777777777777777777777777777777", path["path_id"]
        )
    causal_path = by_id["RFV3-FIX-007-C"]["expected_decoded"]["query_result"]["paths"][
        0
    ]
    c7_response = c7["decoded_expectation"]["rows"][0][0]
    c7_epoch = c7["complete_input_universe"]["inputs"]["pinned_epoch"]
    causal_path_recipe = _path_result_recipe(
        workspace_id=c7_response["snapshot"]["workspace_id"],
        analysis_context_id=path["supporting_provenance"]["analysis_context_id"],
        fabric_epoch_id=c7_epoch["fabric_epoch_id"],
        policy_identity=c7_epoch["policy_release"],
        ordered_entity_ids=causal_path["ordered_entity_ids"],
        ordered_fact_ids=causal_path["ordered_fact_ids"],
    )
    replace_identity_everywhere(
        by_id["RFV3-FIX-007-C"], path["path_id"], causal_path_recipe["output_id"]
    )
    causal_path["path_id"] = causal_path_recipe["output_id"]
    by_id["RFV3-FIX-007-C"]["expected_decoded"]["query_result"]["identity_contract"] = {
        "path_id": causal_path_recipe["output_id"],
        "identity_recipe": causal_path_recipe,
        "witness_bound": True,
    }
    by_id["RFV3-FIX-007-N"]["expected_decoded"]["query_result"].pop(
        "identity_contract", None
    )

    c9 = claims["RFV3-CLAIM-009"]
    c9_inputs = c9["complete_input_universe"]["inputs"]
    producer_inputs = c9_inputs["admitted_relations"]["producer_inputs"]
    provenance = c9["decoded_expectation"]["rows"][0][0]["query_results"][0][
        "provenance"
    ]
    right_rows_before = producer_inputs["right"]["rows"]
    right_rows_after = sorted(
        [
            *right_rows_before,
            "entity:function:cccccccccccccccccccccccccccccccc",
        ]
    )
    causal_producer_inputs = copy.deepcopy(producer_inputs)
    causal_producer_inputs["right"]["rows"] = right_rows_after
    causal_producer_results = _producer_results_from_inputs(
        causal_producer_inputs, provenance
    )
    intersection = sorted(
        set(causal_producer_results["left"]["entity_ids"])
        & set(causal_producer_results["right"]["entity_ids"])
    )
    dictionary = c9_inputs["admitted_relations"]["entity_dictionary"]
    replace_fixture(
        by_id["RFV3-FIX-009-C"],
        source_anchor="QRY §4.6 `combine result sets`; QRY §5 Composition and dependency graphs",
        fault_dimension="authoritative_input",
        change="Change the complete typed right producer result envelope by adding one canonical entity; keep both producer query blocks and the dependency DAG fixed.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "SATISFIED",
            "resolved_semantics": {
                "operation": "intersection",
                "producer_query_ids": ["left", "right"],
                "compatibility": "equal",
            },
            "query_result": {
                "query_id": "q-combine",
                "result_role": "entities",
                "entity_ids": intersection,
                "upstream_query_ids": ["left", "right"],
                "producer_results": causal_producer_results,
                "coverage": {"state": "COMPLETE"},
                "errors": [],
                "notices": [],
                "entity_records": {
                    entity_id: copy.deepcopy(dictionary[entity_id])
                    for entity_id in intersection
                },
            },
            "errors": [],
        },
        role="admitted_relations",
        pointer="/producer_inputs/right/rows",
        before=right_rows_before,
        after=right_rows_after,
        basis="Each producer result is derived from its independent admitted base relation before set composition; changing the right base rows must rederive that result and the canonical intersection.",
    )
    inputs_without_right = {"left": copy.deepcopy(producer_inputs["left"])}
    old_error = copy.deepcopy(by_id["RFV3-FIX-009-N"]["expected_decoded"]["errors"][0])
    old_error.update(
        {
            "code": "DANGLING_RESULT_REFERENCE",
            "safe_message": "The referenced prior query result is absent.",
            "failed_dependency_query_id": "right",
        }
    )
    replace_fixture(
        by_id["RFV3-FIX-009-N"],
        source_anchor="QRY §4.6 `combine result sets`; QRY §5 Composition and dependency graphs",
        fault_dimension="authoritative_input",
        change="Remove the independent right producer input relation while retaining its producer block and q-combine's explicit results_of=right dependency.",
        terminal="reject",
        expected={
            "execution_state": "NOT_EXECUTED_DEPENDENCY",
            "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "FAILED_DEPENDENCY",
            "resolved_semantics": {
                "operation": "intersection",
                "dangling_result_reference": "right",
            },
            "query_result": {
                "query_id": "q-combine",
                "result_role": "entities",
                "entity_ids": [],
                "coverage": {"state": "NOT_APPLICABLE"},
                "errors": [copy.deepcopy(old_error)],
                "notices": [],
            },
            "errors": [old_error],
        },
        role="admitted_relations",
        pointer="/producer_inputs",
        before=producer_inputs,
        after=inputs_without_right,
        basis="Every producer block must derive its result from an independent admitted base relation before results_of can resolve; a missing producer input fails closed without treating a preauthored output as authority.",
    )

    c10 = claims["RFV3-CLAIM-010"]
    baseline_relations = c10["complete_input_universe"]["inputs"]["admitted_relations"]
    causal_relations = copy.deepcopy(baseline_relations)
    call_row = next(
        row
        for row in causal_relations["syntax_rows"]
        if row["statement"]["object"] == "call"
    )
    old_entity_id = call_row["statement"]["subject"]
    new_entity_id = old_entity_id.replace("entity:call-site:", "entity:identifier:")
    entity = causal_relations["entity_dictionary"].pop(old_entity_id)
    entity["entity_id"] = new_entity_id
    entity["semantic_kind"] = "identifier_syntax"
    causal_relations["entity_dictionary"][new_entity_id] = entity
    call_row["owner_id"] = new_entity_id
    call_row["statement"]["subject"] = new_entity_id
    call_row["statement"]["object"] = "identifier"
    call_row.pop("identity_recipe", None)
    _normalize_objective_relations(
        causal_relations,
        c10["complete_input_universe"]["inputs"]["pinned_epoch"]["policy_release"],
    )
    causal_groups_by_id = _objective_groups(causal_relations)
    causal_groups = list(causal_groups_by_id.values())
    changed_fact = next(
        row
        for row in causal_relations["syntax_rows"]
        if row["statement"]["subject"] == new_entity_id
    )
    replace_fixture(
        by_id["RFV3-FIX-010-C"],
        source_anchor="QRY §4.7 `summarize objective facts`; QRY §8 Canonical identity and ordering",
        fault_dimension="authoritative_input",
        change="Replace the typed call-site occurrence with an identifier occurrence and rederive its native-kind fact identity, the complete objective input-set identity, and every dependent group identity.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "measure": "count",
                "group_by": ["native_kind"],
                "input_set_id": causal_relations["input_set_identity"]["output_id"],
            },
            "query_result": {
                "query_id": "q-summary",
                "result_role": "groups",
                "group_ids": list(causal_groups_by_id),
                "groups": causal_groups,
                "coverage": {
                    "state": "COMPLETE",
                    "input_set_id": causal_relations["input_set_identity"]["output_id"],
                    "input_count": len(causal_relations["syntax_rows"]),
                    "group_count": len(causal_groups),
                },
                "errors": [],
                "notices": [],
                "identity_contract": {
                    "changed_fact_id": changed_fact["fact_id"],
                    "changed_fact_identity_recipe": changed_fact["identity_recipe"],
                    "input_set_id": causal_relations["input_set_identity"]["output_id"],
                    "input_set_identity_recipe": causal_relations["input_set_identity"],
                    "group_ids_by_native_kind": {
                        group["group_key"]["native_kind"]: group["group_id"]
                        for group in causal_groups
                    },
                },
            },
            "errors": [],
        },
        role="admitted_relations",
        pointer="",
        before=baseline_relations,
        after=causal_relations,
        basis="A typed property change is an identity-bearing source fact change: stale fact, input-set, or group identities would allow incompatible summaries to collide.",
    )
    fixture_base(by_id["RFV3-FIX-010-N"])

    c11 = claims["RFV3-CLAIM-011"]
    source_context = next(
        iter(c11["decoded_expectation"]["rows"][0][0]["source_contexts"].values())
    )
    for fixture_id in ("RFV3-FIX-011-C", "RFV3-FIX-011-N"):
        fixture_base(by_id[fixture_id])
        replace_identity_everywhere(
            by_id[fixture_id],
            "context:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
            source_context["source_context_id"],
        )
    c11_inputs = c11["complete_input_universe"]["inputs"]
    c11_span = c11_inputs["admitted_relations"]["entity_span"]
    c11_entity = c11_inputs["admitted_relations"]["entity_dictionary"][
        c11_span["entity_id"]
    ]
    c11_source = c11_inputs["admitted_relations"]["source_bytes"]
    c11_context_kind = c11_inputs["request_envelope"]["decoded"]["queries"][0][
        "context"
    ]
    limits_before = c11_inputs["resource_limits"]
    causal_limit = c11_source["byte_length"]
    causal_text = c11_source["value"].encode("utf-8")
    causal_context_recipe = _query_source_context_recipe(
        workspace_id=c11_span["workspace_id"],
        analysis_context_id=c11_entity["analysis_context_id"],
        snapshot_id=c11_inputs["pinned_epoch"]["snapshot_id"],
        entity_id=c11_span["entity_id"],
        source_file_id=c11_span["source_file_id"],
        source_generation=c11_span["source_generation"],
        source_content_digest=c11_span["content_digest"],
        delivered_start_byte=c11_span["start_byte"],
        delivered_end_byte=c11_span["start_byte"] + causal_limit,
        delivered_content_digest=bytes_b3(causal_text),
        disclosure_scope_id=c11_inputs["access_scope"]["scope_id"],
        policy_identity=c11_inputs["pinned_epoch"]["policy_release"],
        context_kind=c11_context_kind,
    )
    replace_fixture(
        by_id["RFV3-FIX-011-C"],
        source_anchor="QRY §4.8 `retrieve source and syntax context`; QRY §8 Canonical identity and ordering",
        fault_dimension="resource",
        change="Change only the explicit source-disclosure byte bound; keep the released scalar context selector, one-record return limit, and hard service byte budget fixed.",
        terminal="changed",
        expected={
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "context": c11_context_kind,
                "explicit_source_byte_limit": causal_limit,
            },
            "query_result": {
                "query_id": "q-source",
                "result_role": "source_contexts",
                "source_context_ids": [causal_context_recipe["output_id"]],
                "source_contexts": [
                    {
                        "source_context_id": causal_context_recipe["output_id"],
                        "content": {"variant": "text", "text": c11_source["value"]},
                        "returned_bytes": causal_limit,
                        "omitted_bytes": 0,
                        "complete": True,
                    }
                ],
                "coverage": {
                    "state": "COMPLETE",
                    "authorized_span_bytes": c11_span["end_byte"]
                    - c11_span["start_byte"],
                    "returned_bytes": causal_limit,
                    "omitted_bytes": 0,
                },
                "errors": [],
                "notices": [],
                "identity_contract": {
                    "source_context_id": causal_context_recipe["output_id"],
                    "identity_recipe": causal_context_recipe,
                    "delivered_bytes_bound": True,
                },
            },
            "errors": [],
        },
        role="resource_limits",
        pointer="/max_source_bytes",
        before=limits_before["max_source_bytes"],
        after=causal_limit,
        basis="The independent semantic source-disclosure bound causally controls exact returned and omitted bytes; the released request selector, result-record limit, and hard output envelope remain separate authorities.",
    )
    denied_scope = copy.deepcopy(c11_inputs["access_scope"])
    denied_scope["source_access"] = False
    denied_scope["allowed_relations"] = ["canonical.entity_span"]
    denied_scope["allowed_columns"] = {
        "canonical.entity_span": denied_scope["allowed_columns"][
            "canonical.entity_span"
        ]
    }
    denied_scope["source_file_ids"] = []
    denied_scope["authorized_ranges"] = []
    denied_recipe = authorization_scope_identity(
        denied_scope, {"policy_id": c11_inputs["pinned_epoch"]["policy_release"]}
    )
    denied_scope["scope_id"] = denied_recipe["output_id"]
    denied_scope["identity_recipe"] = denied_recipe
    fault = {
        "code": "SOURCE_ACCESS_DENIED",
        "layer": "authorization",
        "retryable": False,
        "safe_message": "Source disclosure is not authorized for this request.",
        "field": "source_access",
        "semantic_phrase": None,
        "candidate_interpretations": [],
        "failed_dependency_query_id": None,
        "diagnostic_id": None,
    }
    replace_fixture(
        by_id["RFV3-FIX-011-N"],
        source_anchor="QRY §4.8 `retrieve source and syntax context`; FAB §12 Authorization boundaries",
        fault_dimension="authorization",
        change="Replace the exact CBEF access scope with a separately derived scope that denies source disclosure and grants no source files or byte ranges.",
        terminal="FAILED",
        expected={
            "execution_state": "FAILED",
            "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "about": [c11_span["entity_id"]],
                "context": c11_context_kind,
            },
            "query_result": {
                "query_id": "q-source",
                "result_role": "source_contexts",
                "source_context_ids": [],
                "source_bytes_disclosed": 0,
                "text_disclosed": False,
                "coverage": {
                    "state": "NOT_APPLICABLE",
                    "reason": "source authorization denied",
                },
                "errors": [fault],
                "notices": [],
            },
            "errors": [fault],
        },
        role="access_scope",
        pointer="",
        before=c11_inputs["access_scope"],
        after=denied_scope,
        basis="Fact visibility does not imply source-byte visibility; denial is carried by a valid, independently derived authorization identity rather than an identity mismatch.",
    )

    c12 = claims["RFV3-CLAIM-012"]
    delta = c12["complete_input_universe"]["inputs"]
    selected_before = delta["selected_version_vector"]["table_versions"]["fact.entity"]
    replace_fixture(
        by_id["RFV3-FIX-012-C"],
        source_anchor="FAB §9.2 Exact reads and the single-selector rule; FAB §9.4 Durability classification and exact reconstruction",
        fault_dimension="exact_version_feature",
        change="Change only the exact selected version from 2 to 3 while preserving the runtime-created Delta root binding and CDF request.",
        terminal="changed",
        expected=delta_decoded(delta, 3),
        role="selected_version_vector",
        pointer="/table_versions/fact.entity",
        before=selected_before,
        after=3,
        basis="Exact version selection changes the snapshot without changing physical root identity or substituting latest implicitly.",
    )
    protocol_before = delta["delta_table_history"]["versions"][3]["protocol"]
    protocol_after = copy.deepcopy(protocol_before)
    protocol_after.update({"min_writer_version": 7, "writer_features": ["rowTracking"]})
    replace_fixture(
        by_id["RFV3-FIX-012-N"],
        source_anchor="FAB §9 Durable Delta relations",
        fault_dimension="exact_version_feature",
        change="Require the unsupported rowTracking writer feature at exact version 3.",
        terminal="reject",
        expected={
            "error": "DELTA_WRITER_FEATURE_UNSUPPORTED",
            "feature": "rowTracking",
            "table_version": 3,
        },
        role="delta_table_history",
        pointer="/versions/3/protocol",
        before=protocol_before,
        after=protocol_after,
        basis="delta-rs protocol features are compatibility gates and unsupported writer features fail before write or activation.",
    )

    c13 = claims["RFV3-CLAIM-013"]
    chain_before = c13["complete_input_universe"]["inputs"]["activation_chain"]
    chain_after = activation_chain(2)
    causal_inputs = copy.deepcopy(c13["complete_input_universe"]["inputs"])
    causal_inputs["activation_chain"] = chain_after
    replace_fixture(
        by_id["RFV3-FIX-013-C"],
        source_anchor="FAB §11.3 Ordered activation; FAB §14 Lifecycle, reconstruction, and cutover",
        fault_dimension="authoritative_input",
        change="Append one fully bound v3 event whose predecessor event, predecessor epoch, writer fence, command, FabricEpochPins, transaction, backend observation, and readback all agree.",
        terminal="changed",
        expected=activation_outcome(causal_inputs),
        role="activation_chain",
        pointer="",
        before=chain_before,
        after=chain_after,
        basis="Candidate-free recovery must select the new unique exact event head only from its durable and read-back evidence.",
    )
    readback_before = chain_before["events"][0]["readback"]["transaction"]
    readback_after = activation_typed_input(0xDEAD, 32)
    replace_fixture(
        by_id["RFV3-FIX-013-N"],
        source_anchor="FAB §11.3 Ordered activation",
        fault_dimension="authoritative_input",
        change="Change only the readback transaction reference so it contradicts the durable activation row and backend observation.",
        terminal="reject",
        expected={
            "error": "ACTIVATION_TRANSACTION_READBACK_MISMATCH",
            "admission_state": "closed",
        },
        role="activation_chain",
        pointer="/events/0/readback/transaction",
        before=readback_before,
        after=readback_after,
        basis="No activation may be selected when command, durable transaction, backend observation, and readback disagree.",
    )

    c14 = claims["RFV3-CLAIM-014"]
    scope_before = c14["complete_input_universe"]["inputs"]["access_scope"]
    scope_after = copy.deepcopy(scope_before)
    scope_after["allowed_relations"].append("public.location")
    scope_after["allowed_columns"]["public.location"] = [
        "entity_id",
        "source_file_id",
        "start_byte",
        "end_byte",
    ]
    scope_after_recipe = authorization_scope_identity(
        scope_after,
        c14["complete_input_universe"]["inputs"]["authorization_policy"],
    )
    scope_after["scope_id"] = scope_after_recipe["output_id"]
    scope_after["identity_recipe"] = scope_after_recipe
    replace_fixture(
        by_id["RFV3-FIX-014-C"],
        source_anchor="FAB §12 Authorization boundaries",
        fault_dimension="authorization",
        change="Add public.location and its column grant, deriving a distinct access-scope identity before constructing the expanded child catalog.",
        terminal="changed",
        expected={
            "scope_id": scope_after["scope_id"],
            "previous_scope_id": scope_before["scope_id"],
            "visible_relations": ["public.entity", "public.location"],
            "rebuilt_installed_relations": ["public.entity", "public.location"],
            "previous_installed_relations": ["public.entity"],
            "bound_plan_providers_unchanged": ["public.entity"],
        },
        role="access_scope",
        pointer="",
        before=scope_before,
        after=scope_after,
        basis="The access-scope identity binds the exact workspace, policy, relation, and column grants, so a grant-vector change cannot collide in authorization or plan caches.",
    )
    stale_scope = copy.deepcopy(scope_before)
    stale_scope["allowed_columns"]["public.entity"] = [
        *stale_scope["allowed_columns"]["public.entity"],
        "qualified_name",
    ]
    derived_scope_id = authorization_scope_id(
        stale_scope,
        c14["complete_input_universe"]["inputs"]["authorization_policy"],
    )
    replace_fixture(
        by_id["RFV3-FIX-014-N"],
        source_anchor="FAB §12 Authorization and bound-plan closure",
        fault_dimension="authorization",
        change="Add one column grant while retaining the previous content-bound access-scope identity.",
        terminal="reject",
        expected={
            "error": "ACCESS_SCOPE_IDENTITY_MISMATCH",
            "supplied_scope_id": scope_before["scope_id"],
            "derived_scope_id": derived_scope_id,
        },
        role="access_scope",
        pointer="",
        before=scope_before,
        after=stale_scope,
        basis="An access-scope identity is a commitment to the complete workspace, policy, relation, and column grant vector; any grant mutation with a stale identity must fail before catalog or plan reuse.",
    )

    c15 = claims["RFV3-CLAIM-015"]
    c15_inputs = c15["complete_input_universe"]["inputs"]
    budget_before = c15_inputs["resource_budget"]["rows"]
    budget_after_inputs = copy.deepcopy(c15_inputs)
    budget_after_inputs["resource_budget"]["rows"] = 2
    replace_fixture(
        by_id["RFV3-FIX-015-C"],
        source_anchor="FAB §13 Resources, observability, and proof",
        fault_dimension="resource",
        change="Lower only the row budget below the declared three-row Arrow batch.",
        terminal="changed",
        expected=resource_terminal(budget_after_inputs, "hard_limit_exceeded", False),
        role="resource_budget",
        pointer="/rows",
        before=budget_before,
        after=2,
        basis="A hard row budget below the actual Arrow row count must terminate with QUERY_HARD_LIMIT_EXCEEDED, publish zero rows and no resource, and release reservations and leases.",
    )
    cancel_before = c15_inputs["cancellation_state"]
    cancel_after = {"cancelled": True, "cancellation_ordinal": 2}
    cancel_inputs = copy.deepcopy(c15_inputs)
    cancel_inputs["cancellation_state"] = cancel_after
    replace_fixture(
        by_id["RFV3-FIX-015-N"],
        source_anchor="FAB §13 Resources, observability, and proof",
        fault_dimension="resource",
        change="Cancel the in-flight Arrow result at ordinal 2 before immutable publication.",
        terminal="cancelled",
        expected=resource_terminal(cancel_inputs, "cancelled", False),
        role="cancellation_state",
        pointer="",
        before=cancel_before,
        after=cancel_after,
        basis="Cancellation must discard unpublished IPC state and release reservations and leases without publishing a partial resource.",
    )

    c16 = claims["RFV3-CLAIM-016"]
    sec = c16["complete_input_universe"]["inputs"]
    auth_before = sec["explicit_authorization"]
    auth_after = {
        "job_id": auth_before["job_id"],
        "trusted_local": True,
        "authorization_id": "authorization:trusted-1",
    }
    replace_fixture(
        by_id["RFV3-FIX-016-C"],
        source_anchor="GEN AC-G-35 Provider sandbox and Rust compilation trust model; GEN §85 Capability reporting",
        fault_dimension="authorization",
        change="Grant the distinct trusted-local authorization while leaving every untrusted host limitation unchanged; no runtime observation is authored by the authorization input.",
        terminal="changed",
        expected={
            "job_id": auth_before["job_id"],
            "authorization_state": "accepted",
            "effective_trust_profile": "trusted_local",
            "capability_state": "degraded_trusted_local",
            "launch_admission": "authorized_for_trusted_local_launch",
            "untrusted_admission": "unavailable",
            "hostile_actions_attempted": 0,
        },
        role="explicit_authorization",
        pointer="",
        before=auth_before,
        after=auth_after,
        basis="Distinct authorization changes only trusted-local launch admission; a later runtime observation must independently establish any receipt, terminal, or provenance, and authorization cannot create untrusted-containment proof.",
    )
    required_before = sec["launcher_constraints"]["required"][
        "compiled_seccomp_policy_authorized"
    ]
    replace_fixture(
        by_id["RFV3-FIX-016-N"],
        source_anchor="GEN AC-G-35 Provider sandbox and Rust compilation trust model",
        fault_dimension="security",
        change="Attempt to waive the mandatory application-owned compiled seccomp authorization prerequisite.",
        terminal="reject",
        expected={
            "error": "HOST_CONTAINMENT_POLICY_WEAKENING_REJECTED",
            "missing_prerequisite": "compiled_seccomp_policy_authorized",
            "untrusted_admission": "unavailable",
            "hostile_actions_attempted": 0,
        },
        role="launcher_constraints",
        pointer="/required/compiled_seccomp_policy_authorized",
        before=required_before,
        after=False,
        basis="The accepted trust policy is fail closed; author input cannot waive a mandatory kernel-enforcement prerequisite.",
    )

    c17 = claims["RFV3-CLAIM-017"]
    c17_inputs = c17["complete_input_universe"]["inputs"]
    fixture_base(by_id["RFV3-FIX-017-C"])
    projection_before = c17_inputs["candidate_released_projection"]
    projection_after = copy.deepcopy(projection_before)
    projection_after["internal_table"] = c17_inputs["private_diagnostics"][
        "internal_table"
    ]
    replace_fixture(
        by_id["RFV3-FIX-017-N"],
        source_anchor="SRV §11 Public status, errors, and redaction; SRV §13 Lifespan, middleware, and STDIO purity",
        fault_dimension="public_output",
        change="Inject the private internal_table physical name directly into the candidate released projection while preserving the deny policy and private diagnostic boundary.",
        terminal="reject",
        expected={
            "error": "RELEASED_PROJECTION_FORBIDDEN_FIELD",
            "forbidden_fields": ["internal_table"],
            "admission_state": "rejected",
        },
        role="candidate_released_projection",
        pointer="",
        before=projection_before,
        after=projection_after,
        basis="Leakage is established only by a forbidden field present in the candidate public projection; the existence of the same field in a separately authorized private diagnostic resource is not itself a leak.",
    )

    c18 = claims["RFV3-CLAIM-018"]
    c18_inputs = c18["complete_input_universe"]["inputs"]
    causal_before = c18_inputs["source_images"]["generation_g2"]
    causal_after = equivalence_source_image("g2", "e4")
    causal_inputs = copy.deepcopy(c18_inputs)
    causal_inputs["source_images"]["generation_g2"] = causal_after
    causal_routes = equivalence_fixture_routes(causal_inputs)
    causal_provenance = equivalence_provenance(causal_inputs)
    replace_fixture(
        by_id["RFV3-FIX-018-C"],
        source_anchor="FAB §14",
        fault_dimension="authoritative_input",
        change="Change only generation_g2 immutable Python source bytes and their content digest by replacing the defined e3 function and its direct call with defined e4; derive all function, call-site, and calls-fact identities again from the frozen identity contract.",
        terminal="changed",
        expected={
            "routes": causal_routes,
            "clean_incremental_equivalent": True,
            "policy_release": causal_provenance["policy_release"],
            "proof_set": causal_provenance["proof_set"],
            "expectation_issuance": causal_provenance["expectation_issuance"],
        },
        role="source_images",
        pointer="/generation_g2",
        before=causal_before,
        after=causal_after,
        basis="Both routes independently parse and transform the same changed immutable source bytes; function identities remain semantic, call-site identities change with content/range, and every calls fact exposes the recomputed call_site_id.",
    )

    clean_rows = [
        [*row, "current"]
        for row in equivalence_semantic_rows(
            c18_inputs["source_images"]["generation_g2"]
        )
    ]
    base_rows = c18_inputs["incremental_base_state"]["rows"]
    clean_ids = {row[0] for row in clean_rows}
    incremental_extra = [row for row in base_rows if row[0] not in clean_ids]
    delete_before = next(
        operation["enabled"]
        for operation in c18_inputs["route_definitions"]["incremental"]["operations"]
        if operation["operator"] == "apply_deletes"
    )
    replace_fixture(
        by_id["RFV3-FIX-018-N"],
        source_anchor="LIFE §15",
        fault_dimension="coverage",
        change="Disable only the typed incremental apply-deletes operation while clean reconstruction and the canonical identity contract remain unchanged.",
        terminal="fail",
        expected={
            "error": "CLEAN_INCREMENTAL_DIVERGENCE",
            "clean_rows": clean_rows,
            "incremental_extra": incremental_extra,
            "capability_state": "not_advertised",
        },
        role="route_definitions",
        pointer="/incremental/operations/2/enabled",
        before=delete_before,
        after=False,
        basis="Disabling typed deletes preserves the superseded function, its content-bound call-site occurrence, and its call-site-bound calls fact as stale current semantic state.",
    )

    # These four families are re-authored last so retained legacy-shaped fixture
    # payloads cannot overwrite their executable successor semantics.
    rewrite_query_oracle_fixtures(fixtures, claims)


def _review_row_sha256(value: dict[str, Any]) -> str:
    """Bind an immutable review input row without treating its digest as semantics."""

    return hashlib.sha256(
        json.dumps(
            value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode()
    ).hexdigest()


def claim_review_basis(
    claim: dict[str, Any], fixture_digests: dict[str, str]
) -> dict[str, Any]:
    """Build the exact claim/fixture/authority envelope an independent review consumes."""

    claim_id = str(claim["claim_id"])
    governing_clauses = [str(value) for value in claim["governing_clauses"]]
    payload = {
        "claim_id": claim_id,
        "expectation_sha256": _review_row_sha256(claim),
        "semantic_fixture_sha256": {
            kind: fixture_digests[kind] for kind in ("causal", "negative")
        },
        "subject": str(claim["subject"]),
        "source_authority": {
            "source_anchor": str(claim["source_anchor"]),
            "governing_clauses": governing_clauses,
        },
        "review_dimensions": [
            "decoded_expectation_semantics",
            "causal_fixture_discrimination",
            "negative_fixture_fail_closed_behavior",
            "governing_clause_and_source_authority_conformance",
        ],
    }
    return {
        **payload,
        "review_binding_id": f"sha256:{_review_row_sha256(payload)}",
    }


def claim_review_rationale(
    claim: dict[str, Any], review_basis: dict[str, Any], *, accepted: bool
) -> str:
    """Emit claim-specific reviewer prose carrying the exact immutable binding."""

    disposition = "Accepted" if accepted else "Pending independent review for"
    authority = review_basis["source_authority"]
    clauses = " | ".join(authority["governing_clauses"])
    suffix = (
        "Both semantic fixtures were independently discriminated."
        if accepted
        else "No acceptance has been issued for this binding."
    )
    return (
        f"{disposition} {claim['claim_id']} under "
        f"{review_basis['review_binding_id']}. Subject: {claim['subject']} "
        f"Source authority: {authority['source_anchor']}. "
        f"Governing clauses: {clauses}. {suffix}"
    )


def refresh_issuance(
    expectations: list[dict[str, Any]], fixtures: list[dict[str, Any]]
) -> dict[str, Any]:
    expectation_bytes = "".join(
        json.dumps(row, separators=(",", ":"), ensure_ascii=False) + "\n"
        for row in expectations
    ).encode()
    fixture_bytes = "".join(
        json.dumps(row, separators=(",", ":"), ensure_ascii=False) + "\n"
        for row in fixtures
    ).encode()
    expectation_sha = hashlib.sha256(expectation_bytes).hexdigest()
    fixture_sha = hashlib.sha256(fixture_bytes).hexdigest()
    projection = {
        "expectations_sha256": expectation_sha,
        "negative_fixtures_sha256": fixture_sha,
    }
    issuance = json.loads(ISSUANCE.read_text(encoding="utf-8"))
    issuance.update(
        {
            "issuance_id": ISSUANCE_ID,
            "reviewed_content_id": f"sha256:{hashlib.sha256(json.dumps(projection, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()}",
            "status": "pending_independent_review",
            "author": {
                "identity": AUTHOR,
                "role": "successor-only expectation transaction reissue author",
                "implementation_owner": False,
            },
            "reviewer": {
                "identity": REVIEWER,
                "role": "independent successor-only evidence reissuance reviewer",
                "implementation_owner": False,
                "expectation_author": False,
            },
            "artifacts": {
                "expectations": {
                    "path": str(EXPECTATIONS.relative_to(ROOT)),
                    "sha256": expectation_sha,
                    "rows": len(expectations),
                },
                "negative_fixtures": {
                    "path": str(FIXTURES.relative_to(ROOT)),
                    "sha256": fixture_sha,
                    "rows": len(fixtures),
                },
            },
        }
    )
    fixtures_by_claim: dict[str, dict[str, dict[str, Any]]] = {}
    for fixture in fixtures:
        fixtures_by_claim.setdefault(fixture["claim_id"], {})[fixture["kind"]] = fixture
    claim_reviews: list[dict[str, Any]] = []
    for claim in expectations:
        claim_id = claim["claim_id"]
        fixture_digests = {
            kind: _review_row_sha256(fixtures_by_claim[claim_id][kind])
            for kind in ("causal", "negative")
        }
        basis = claim_review_basis(claim, fixture_digests)
        claim_reviews.append(
            {
                "claim_id": claim_id,
                "expectation_sha256": basis["expectation_sha256"],
                "fixture_sha256": copy.deepcopy(basis["semantic_fixture_sha256"]),
                "reviewer_id": REVIEWER,
                "disposition": "pending",
                "review_basis": basis,
                "rationale": claim_review_rationale(claim, basis, accepted=False),
            }
        )
    issuance["claim_reviews"] = claim_reviews
    return issuance


def main() -> None:
    expectations = load_jsonl(EXPECTATIONS)
    fixtures = load_jsonl(FIXTURES)
    by_id = {row["claim_id"]: row for row in expectations}
    by_id["RFV3-CLAIM-001"] = claim_001(by_id["RFV3-CLAIM-001"])
    by_id["RFV3-CLAIM-003"] = claim_003(by_id["RFV3-CLAIM-003"])
    for claim_id in (
        "RFV3-CLAIM-004",
        "RFV3-CLAIM-005",
        "RFV3-CLAIM-006",
        "RFV3-CLAIM-007",
        "RFV3-CLAIM-008",
        "RFV3-CLAIM-009",
        "RFV3-CLAIM-010",
        "RFV3-CLAIM-011",
    ):
        by_id[claim_id] = update_query_claim(by_id[claim_id])
    by_id["RFV3-CLAIM-012"] = claim_012(by_id["RFV3-CLAIM-012"])
    by_id["RFV3-CLAIM-013"] = claim_013(by_id["RFV3-CLAIM-013"])
    by_id["RFV3-CLAIM-014"] = claim_014(by_id["RFV3-CLAIM-014"])
    by_id["RFV3-CLAIM-015"] = claim_015(by_id["RFV3-CLAIM-015"])
    by_id["RFV3-CLAIM-016"] = claim_016(by_id["RFV3-CLAIM-016"])
    by_id["RFV3-CLAIM-017"] = claim_017(by_id["RFV3-CLAIM-017"])
    by_id["RFV3-CLAIM-018"] = claim_018(by_id["RFV3-CLAIM-018"])
    expectations = [by_id[row["claim_id"]] for row in expectations]
    rewrite_fixtures(fixtures, by_id)

    # An issuance is indivisible: adopted claims and their fixtures must bind the
    # newly frozen transaction everywhere they carry an issuance/proof receipt.
    # This is semantic provenance binding, not an acceptance checksum.
    for row in [*expectations, *fixtures]:
        replace_identity_everywhere(row, "wp33:fixture-r2", ISSUANCE_ID)
        if "wp33:fixture-r2" in json.dumps(row, separators=(",", ":")):
            raise ValueError("a retained row still binds the superseded r2 issuance")

    # The issuance contract deliberately permits one expectation author only.  A
    # content change to any claim invalidates the transaction, so the reissue adopts
    # every retained claim and fixture under the new author and sends all of them
    # through independent review again.  REISSUED remains the material-change set.
    if {
        claim["claim_id"] for claim in expectations if claim["claim_id"] in REISSUED
    } != REISSUED:
        raise ValueError("the material WP33 reissue set is incomplete")
    for claim in expectations:
        claim["author_id"] = AUTHOR
    for fixture in fixtures:
        fixture["author_id"] = AUTHOR

    EXPECTATIONS.write_text(
        "".join(
            json.dumps(row, separators=(",", ":"), ensure_ascii=False) + "\n"
            for row in expectations
        ),
        encoding="utf-8",
    )
    FIXTURES.write_text(
        "".join(
            json.dumps(row, separators=(",", ":"), ensure_ascii=False) + "\n"
            for row in fixtures
        ),
        encoding="utf-8",
    )
    issuance = refresh_issuance(expectations, fixtures)
    ISSUANCE.write_text(
        json.dumps(issuance, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
