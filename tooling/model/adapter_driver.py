"""Closed external Pydantic/FastMCP projection driver for the repository model."""

from __future__ import annotations

import importlib.metadata
import json
import operator
import subprocess
import sys
from functools import reduce
from pathlib import Path
from typing import Literal

ROOT = Path(__file__).resolve().parents[2]
ADAPTER_SOURCE = ROOT / "codefabric-cpg-mcp/src"
for search_path in (ROOT, ADAPTER_SOURCE):
    value = str(search_path)
    if value not in sys.path:
        sys.path.insert(0, value)

from codefabric_cpg_mcp.contracts.json import (
    canonicalize_value,
    checksum,
)
from mcp.types import Tool as MCPTool
from pydantic import BaseModel, ConfigDict, Field, TypeAdapter, model_validator

from tooling.contracts.generate_adapter_models import (
    DIALECT,
    AdapterModelIr,
    IrRecord,
    IrUnion,
    _load_candidate,
    _schema_id,
    render_source,
)

PROTOCOL_VERSION = "codefabric-external-adapter-driver-v1"
EXACT_PYDANTIC = "2.13.4"
EXACT_FASTMCP = "3.4.7"
MAX_IR_BYTES = 4 * 1024 * 1024
MAX_OUTPUT_BYTES = 8 * 1024 * 1024

type DriverRole = Literal[
    "python-binding",
    "canonical-projection",
    "public-json-schema",
    "validation-report",
]
type SchemaMode = Literal["validation", "serialization"]
type ProjectionKind = Literal[
    "pydantic-model-source",
    "pydantic-schema-manifest",
    "pydantic-fingerprint-manifest",
    "fastmcp-fingerprint-module",
    "python-package-manifest",
    "public-json-schema",
    "validation-report",
]


class ClosedModel(BaseModel):
    """Strict immutable external-driver protocol base."""

    model_config = ConfigDict(extra="forbid", frozen=True, strict=True)


class FastMCPToolProfile(ClosedModel):
    profile: str = Field(min_length=1)
    fields: tuple[str, ...] = Field(min_length=1)

    @model_validator(mode="after")
    def validate_fields(self) -> FastMCPToolProfile:
        if len(self.fields) != len(set(self.fields)):
            raise ValueError("FastMCP tool profile fields must be unique")
        installed = tuple(
            field.serialization_alias or field.alias or name
            for name, field in MCPTool.model_fields.items()
        )
        if set(installed) != set(self.fields):
            raise ValueError(
                "accepted FastMCP Tool profile differs from the installed protocol model"
            )
        return self


class AdapterProjection(ClosedModel):
    output_id: str = Field(pattern=r"^output:[a-z0-9][a-z0-9-]*$")
    path: str = Field(min_length=1)
    role: DriverRole
    projection_kind: ProjectionKind
    package_path: str | None = None
    artifact_id: str | None = None
    title: str | None = None
    model_roots: tuple[str, ...] = ()
    mode: SchemaMode | None = None

    @model_validator(mode="after")
    def validate_shape(self) -> AdapterProjection:
        public = self.projection_kind == "public-json-schema"
        metadata = (
            self.artifact_id is not None
            and self.title is not None
            and bool(self.model_roots)
            and self.mode is not None
        )
        if public != metadata:
            raise ValueError(
                "only public JSON Schema projections require complete metadata"
            )
        if self.path.startswith(("contracts/acceptance/", "contracts/fixtures/")):
            raise ValueError("adapter driver cannot own acceptance or KAT paths")
        path = Path(self.path)
        if (
            path.is_absolute()
            or ".." in path.parts
            or not all(part not in {"", "."} for part in path.parts)
        ):
            raise ValueError(
                "adapter projection path must be a safe repository-relative path"
            )
        if self.package_path is not None:
            package_path = Path(self.package_path)
            if (
                package_path.is_absolute()
                or ".." in package_path.parts
                or not all(part not in {"", "."} for part in package_path.parts)
            ):
                raise ValueError("adapter package path must be safe and relative")
        expected_roles: dict[ProjectionKind, DriverRole] = {
            "pydantic-model-source": "python-binding",
            "pydantic-schema-manifest": "canonical-projection",
            "pydantic-fingerprint-manifest": "canonical-projection",
            "fastmcp-fingerprint-module": "python-binding",
            "python-package-manifest": "canonical-projection",
            "public-json-schema": "public-json-schema",
            "validation-report": "validation-report",
        }
        if expected_roles[self.projection_kind] != self.role:
            raise ValueError("adapter projection kind and output role disagree")
        return self


class AdapterDriverIr(ClosedModel):
    artifact_id: Literal["codefabric.adapter.model-ir"]
    artifact_kind: Literal["manifest"]
    version: str
    compatible_suite_major: Literal[1]
    status: Literal["draft", "released", "deprecated"]
    canonical_digest: str = Field(pattern=r"^b3:[0-9a-f]{64}$")
    schema_version: Literal[1]
    fastmcp_tool_profile: FastMCPToolProfile
    projections: tuple[AdapterProjection, ...]
    models: tuple[IrRecord, ...]
    unions: tuple[IrUnion, ...]

    @model_validator(mode="after")
    def validate_projection_graph(self) -> AdapterDriverIr:
        output_ids = [projection.output_id for projection in self.projections]
        paths = [projection.path for projection in self.projections]
        if len(output_ids) != len(set(output_ids)) or len(paths) != len(set(paths)):
            raise ValueError("adapter projection IDs and paths must be unique")
        model_names = {model.name for model in self.models} | {
            union.name for union in self.unions
        }
        singleton_kinds = {
            "pydantic-model-source",
            "pydantic-schema-manifest",
            "pydantic-fingerprint-manifest",
            "fastmcp-fingerprint-module",
            "python-package-manifest",
            "validation-report",
        }
        actual = {
            kind: sum(
                projection.projection_kind == kind for projection in self.projections
            )
            for kind in singleton_kinds
        }
        if any(count != 1 for count in actual.values()) or not any(
            projection.projection_kind == "public-json-schema"
            for projection in self.projections
        ):
            raise ValueError(f"adapter projection kind census differs: {actual}")
        package_paths = [
            projection.package_path
            for projection in self.projections
            if projection.package_path is not None
        ]
        if len(package_paths) != len(set(package_paths)):
            raise ValueError("adapter package projection paths must be unique")
        for projection in self.projections:
            if any(root not in model_names for root in projection.model_roots):
                raise ValueError(
                    f"{projection.output_id} references an unknown root model"
                )
        return self

    def legacy_model(self) -> AdapterModelIr:
        """Project the semantic model graph into the existing pure source renderer."""

        return AdapterModelIr.model_validate(
            {
                "artifact_id": self.artifact_id,
                "artifact_kind": self.artifact_kind,
                "version": self.version,
                "compatible_suite_major": self.compatible_suite_major,
                "status": self.status,
                "canonical_digest": self.canonical_digest,
                "schema_version": self.schema_version,
                "models": self.models,
                "unions": self.unions,
            },
            strict=True,
        )


class PlannedOutput(ClosedModel):
    output_id: str
    path: str
    role: DriverRole


class DriverRequest(ClosedModel):
    protocol_version: Literal["codefabric-external-adapter-driver-v1"]
    operation: Literal["plan", "render"]
    source: str = Field(max_length=MAX_IR_BYTES)
    source_digest: str = Field(pattern=r"^b3:[0-9a-f]{64}$")
    planned_outputs: tuple[PlannedOutput, ...] = ()


class DriverToolIdentity(ClosedModel):
    python_path: str
    python_digest: str
    python_version: str
    script_digest: str
    lock_digest: str
    project_digest: str
    ruff_path: str
    ruff_digest: str
    ruff_version: str
    pydantic_version: Literal["2.13.4"]
    fastmcp_version: Literal["3.4.7"]
    mcp_version: str


class RenderedOutput(PlannedOutput):
    contents: str = Field(max_length=MAX_OUTPUT_BYTES)


class PlanResponse(ClosedModel):
    protocol_version: Literal["codefabric-external-adapter-driver-v1"]
    tool_identity: DriverToolIdentity
    outputs: tuple[PlannedOutput, ...]


class RenderResponse(ClosedModel):
    protocol_version: Literal["codefabric-external-adapter-driver-v1"]
    tool_identity: DriverToolIdentity
    outputs: tuple[RenderedOutput, ...]


def _blake3_file_digest(path: Path) -> str:
    from blake3 import blake3

    return "b3:" + blake3(path.read_bytes()).hexdigest()


def _format_source(source: bytes) -> bytes:
    ruff = Path(sys.prefix) / "bin/ruff"
    result = subprocess.run(
        [
            str(ruff),
            "format",
            "--line-length",
            "100",
            "--stdin-filename",
            "generated_adapter.py",
            "-",
        ],
        input=source,
        check=False,
        capture_output=True,
        env={"PATH": str(ruff.parent)},
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def _tool_identity() -> DriverToolIdentity:
    python = Path(sys.executable).resolve()
    ruff = Path(sys.prefix) / "bin/ruff"
    if not ruff.is_file():
        raise RuntimeError("the exact adapter environment does not contain Ruff")
    ruff_result = subprocess.run(
        [str(ruff), "--version"],
        check=True,
        capture_output=True,
        text=True,
        env={"PATH": str(ruff.parent)},
    )
    pydantic_version = importlib.metadata.version("pydantic")
    fastmcp_version = importlib.metadata.version("fastmcp")
    if pydantic_version != EXACT_PYDANTIC or fastmcp_version != EXACT_FASTMCP:
        raise RuntimeError(
            "adapter driver dependency versions differ from the accepted pins"
        )
    return DriverToolIdentity(
        python_path=str(python),
        python_digest=_blake3_file_digest(python),
        python_version=".".join(str(value) for value in sys.version_info[:3]),
        script_digest=_blake3_file_digest(Path(__file__)),
        lock_digest=_blake3_file_digest(ROOT / "codefabric-cpg-mcp/uv.lock"),
        project_digest=_blake3_file_digest(ROOT / "codefabric-cpg-mcp/pyproject.toml"),
        ruff_path=str(ruff.resolve()),
        ruff_digest=_blake3_file_digest(ruff.resolve()),
        ruff_version=ruff_result.stdout.strip(),
        pydantic_version=EXACT_PYDANTIC,
        fastmcp_version=EXACT_FASTMCP,
        mcp_version=importlib.metadata.version("mcp"),
    )


def _decode(request: DriverRequest) -> AdapterDriverIr:
    source = request.source.encode()
    if checksum(source) != request.source_digest:
        raise ValueError("adapter invocation source digest differs from source bytes")
    return AdapterDriverIr.model_validate_json(source, strict=True)


def _plan(ir: AdapterDriverIr) -> tuple[PlannedOutput, ...]:
    return tuple(
        PlannedOutput(
            output_id=projection.output_id,
            path=projection.path,
            role=projection.role,
        )
        for projection in sorted(ir.projections, key=lambda item: item.path)
    )


def _generated(source_digest: str) -> dict[str, object]:
    return {
        "generator_revision": "codefabric-model-adapter-driver-v1",
        "profile": "codefabric-jcs-v1",
        "source_artifact_id": "codefabric.adapter.model-ir",
        "source_digest": source_digest,
    }


def _model_source(ir: AdapterDriverIr, source_digest: str) -> bytes:
    source = render_source(ir.legacy_model(), {"canonical_digest": source_digest})
    lines = source.decode().splitlines()
    lines[0] = (
        "# @generated from codefabric.adapter.model-ir source "
        f"{source_digest}; codefabric-model-adapter-driver-v1; do not edit."
    )
    export_names = [
        *(model.name for model in ir.models),
        *(union.name for union in ir.unions),
        "JSON_OBJECT_ADAPTER",
        "MODEL_ADAPTERS",
        "TYPE_ADAPTERS",
    ]
    exports = ", ".join(repr(name) for name in export_names)
    lines.extend(
        [
            "MODEL_ADAPTERS = {",
            *[f'    "{model.name}": TypeAdapter({model.name}),' for model in ir.models],
            "}",
            f"__all__ = [{exports}]",
            "",
        ]
    )
    return _format_source("\n".join(lines).encode())


def _schema_views(
    module: object, source_digest: str
) -> tuple[dict[str, object], dict[str, object]]:
    generated = _generated(source_digest)
    schemas: dict[str, dict[str, object]] = {"validation": {}, "serialization": {}}
    fingerprints: dict[str, dict[str, str]] = {"validation": {}, "serialization": {}}
    types = {**module.MODEL_BY_NAME, **module.TYPE_ADAPTERS}  # type: ignore[attr-defined]
    for mode in ("validation", "serialization"):
        for name, contract in sorted(types.items()):
            schema = (
                contract.json_schema(mode=mode, by_alias=True)
                if isinstance(contract, TypeAdapter)
                else contract.model_json_schema(mode=mode, by_alias=True)
            )
            schema["$schema"] = DIALECT
            schema["$id"] = _schema_id(name, mode)
            schemas[mode][name] = schema
            fingerprints[mode][name] = checksum(canonicalize_value(schema))
    return (
        {"_generated": generated, **schemas},
        {
            "_generated": generated,
            "profile": "codefabric-adapter-schema-fingerprints-v1",
            **fingerprints,
        },
    )


def _public_schema(
    module: object, projection: AdapterProjection, source_digest: str
) -> bytes:
    model_types = tuple(getattr(module, name) for name in projection.model_roots)
    contract_type = (
        model_types[0] if len(model_types) == 1 else reduce(operator.or_, model_types)
    )
    schema = TypeAdapter(contract_type).json_schema(
        mode=projection.mode,
        by_alias=True,
    )
    schema["$schema"] = DIALECT
    schema["$id"] = f"https://codefabric.dev/{projection.path}"
    schema["title"] = projection.title
    schema["x-codefabric-generated"] = _generated(source_digest)
    schema["x-codefabric-artifact"] = {
        "artifact_id": projection.artifact_id,
        "artifact_kind": "json-schema",
        "version": "1.0",
        "compatible_suite_major": 1,
        "status": "released",
        "generator_revision": "codefabric-model-adapter-driver-v1",
    }
    return (json.dumps(schema, indent=2, sort_keys=True) + "\n").encode()


def _fingerprint_module(profile: FastMCPToolProfile) -> bytes:
    fields = ",\n".join(f"    {field!r}" for field in profile.fields)
    source = f'''\
"""Generated canonical FastMCP protocol fingerprint policy."""

from collections.abc import Iterable, Mapping
from typing import Any, Protocol

from mcp.types import Tool as MCPTool

from .json import canonicalize_value, checksum

FASTMCP_TOOL_PROFILE = {profile.profile!r}
FASTMCP_TOOL_KEYS = (\n{fields},\n)
_FASTMCP_TOOL_KEY_SET = frozenset(FASTMCP_TOOL_KEYS)


class FastMCPToolView(Protocol):
    def to_mcp_tool(self) -> MCPTool: ...


def normalize_mcp_tool(value: Mapping[str, Any]) -> dict[str, Any]:
    unexpected = set(value) - _FASTMCP_TOOL_KEY_SET
    if unexpected:
        raise ValueError(f"unexpected MCP Tool fields: {{sorted(unexpected)}}")
    return {{key: value[key] for key in FASTMCP_TOOL_KEYS if key in value}}


def fastmcp_tool_manifest(tools: Iterable[FastMCPToolView]) -> dict[str, Any]:
    records = [
        normalize_mcp_tool(
            tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)
        )
        for tool in tools
    ]
    records.sort(key=lambda record: str(record["name"]))
    names = [record["name"] for record in records]
    if len(names) != len(set(names)):
        raise ValueError("duplicate public tool name in fingerprint manifest")
    return {{"profile": FASTMCP_TOOL_PROFILE, "tools": records}}


def fastmcp_tool_fingerprint(tools: Iterable[FastMCPToolView]) -> str:
    return checksum(canonicalize_value(fastmcp_tool_manifest(tools)))
'''
    return _format_source(source.encode())


def _render(ir: AdapterDriverIr, source_digest: str) -> dict[str, bytes]:
    source = _model_source(ir, source_digest)
    module = _load_candidate(source)
    schemas, fingerprints = _schema_views(module, source_digest)
    rendered: dict[str, bytes] = {}
    for projection in ir.projections:
        if projection.projection_kind == "pydantic-model-source":
            value = source
        elif projection.projection_kind == "pydantic-schema-manifest":
            value = canonicalize_value(schemas)
        elif projection.projection_kind == "pydantic-fingerprint-manifest":
            value = canonicalize_value(fingerprints)
        elif projection.projection_kind == "fastmcp-fingerprint-module":
            value = _fingerprint_module(ir.fastmcp_tool_profile)
        elif projection.projection_kind == "public-json-schema":
            value = _public_schema(module, projection, source_digest)
        elif projection.projection_kind == "python-package-manifest":
            package_outputs = [
                {
                    "path": item.package_path,
                    "projection_kind": item.projection_kind,
                    "role": item.role,
                }
                for item in ir.projections
                if item.package_path is not None
            ]
            value = canonicalize_value(
                {
                    "_generated": _generated(source_digest),
                    "package": "codefabric_cpg_mcp",
                    "outputs": sorted(package_outputs, key=lambda item: item["path"]),
                }
            )
        elif projection.projection_kind == "validation-report":
            value = canonicalize_value(
                {
                    "_generated": _generated(source_digest),
                    "family": "adapter",
                    "model_count": len(ir.models),
                    "union_count": len(ir.unions),
                    "projection_count": len(ir.projections),
                    "validation_schema_count": len(schemas["validation"]),
                    "serialization_schema_count": len(schemas["serialization"]),
                    "fastmcp_tool_fields": list(ir.fastmcp_tool_profile.fields),
                }
            )
        else:
            raise ValueError(f"unhandled adapter projection {projection.output_id}")
        if len(value) > MAX_OUTPUT_BYTES:
            raise ValueError(
                f"adapter projection exceeds output budget: {projection.path}"
            )
        rendered[projection.path] = value
    if set(rendered) != {projection.path for projection in ir.projections}:
        raise ValueError("adapter projection output census is incomplete")
    return rendered


def execute(request: DriverRequest) -> PlanResponse | RenderResponse:
    ir = _decode(request)
    planned = _plan(ir)
    identity = _tool_identity()
    if request.operation == "plan":
        if request.planned_outputs:
            raise ValueError("plan request cannot predeclare output answers")
        return PlanResponse(
            protocol_version=PROTOCOL_VERSION,
            tool_identity=identity,
            outputs=planned,
        )
    if request.planned_outputs != planned:
        raise ValueError("render request output plan differs from typed Contract IR")
    rendered = _render(ir, request.source_digest)
    return RenderResponse(
        protocol_version=PROTOCOL_VERSION,
        tool_identity=identity,
        outputs=tuple(
            RenderedOutput(
                **output.model_dump(),
                contents=rendered[output.path].decode(),
            )
            for output in planned
        ),
    )


def main() -> int:
    request = DriverRequest.model_validate_json(sys.stdin.buffer.read(), strict=True)
    response = execute(request)
    sys.stdout.buffer.write(canonicalize_value(response.model_dump(mode="json")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
