"""Compile adapter Contract IR into typed Pydantic source and canonical schema views."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from types import ModuleType
from typing import Literal

from codefabric_cpg_mcp.contracts.json import (
    canonicalize_json,
    canonicalize_value,
    checksum,
)
from pydantic import BaseModel, ConfigDict, Field, model_validator

GENERATOR_REVISION = "codefabric-adapter-model-compiler-v1"
DIALECT = "https://json-schema.org/draft/2020-12/schema"
IR_ARTIFACT_ID = "codefabric.adapter.model-ir"

type TypeKind = Literal[
    "boolean",
    "checksum",
    "json-object",
    "json-object-list",
    "literal",
    "literal-default",
    "model",
    "model-list",
    "nonnegative-integer",
    "optional-json-object",
    "optional-model",
    "optional-string",
    "string",
    "string-list",
    "union",
]


class _IrModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True, strict=True)


class IrType(_IrModel):
    kind: TypeKind
    name: str | None = None
    values: tuple[str, ...] = ()
    default: str | None = None

    @model_validator(mode="after")
    def validate_shape(self) -> IrType:
        named = {"model", "model-list", "optional-model", "union"}
        literal = {"literal", "literal-default"}
        if (self.kind in named) != (self.name is not None):
            raise ValueError("named types require exactly one name")
        if (self.kind in literal) != bool(self.values):
            raise ValueError("literal types require non-empty values")
        if self.kind == "literal-default":
            if self.default not in self.values:
                raise ValueError("literal default must be one of its values")
        elif self.default is not None:
            raise ValueError("only literal-default may carry a default")
        return self


class IrField(_IrModel):
    name: str = Field(pattern=r"^[a-z][a-z0-9_]*$")
    type: IrType
    description: str = Field(min_length=1)
    alias: str | None = None


class IrRecord(_IrModel):
    name: str = Field(pattern=r"^[A-Z][A-Za-z0-9]*$")
    fields: tuple[IrField, ...]


class IrUnion(_IrModel):
    name: str = Field(pattern=r"^[A-Z][A-Za-z0-9]*$")
    discriminator: str = Field(pattern=r"^[a-z][a-z0-9_]*$")
    members: tuple[str, ...] = Field(min_length=2)


class AdapterModelIr(_IrModel):
    artifact_id: Literal["codefabric.adapter.model-ir"]
    artifact_kind: Literal["manifest"]
    version: str
    compatible_suite_major: Literal[1]
    status: Literal["draft", "released", "deprecated"]
    canonical_digest: str = Field(pattern=r"^b3:[0-9a-f]{64}$")
    schema_version: Literal[1]
    models: tuple[IrRecord, ...]
    unions: tuple[IrUnion, ...]

    @model_validator(mode="after")
    def validate_graph(self) -> AdapterModelIr:
        model_names = [model.name for model in self.models]
        union_names = [union.name for union in self.unions]
        all_names = set(model_names + union_names)
        if len(all_names) != len(model_names) + len(union_names):
            raise ValueError("model and union names must be unique")
        for model in self.models:
            field_names = [field.name for field in model.fields]
            aliases = [field.alias or field.name for field in model.fields]
            if len(set(field_names)) != len(field_names) or len(set(aliases)) != len(
                aliases
            ):
                raise ValueError(f"{model.name} field names and aliases must be unique")
            for field in model.fields:
                if field.type.name is not None and field.type.name not in all_names:
                    raise ValueError(
                        f"{model.name}.{field.name} references an unknown type"
                    )
        for union in self.unions:
            if any(member not in model_names for member in union.members):
                raise ValueError(f"{union.name} references an unknown model")
        return self


def _strict_json(path: Path) -> object:
    data = path.read_bytes()
    canonicalize_json(data)
    return json.loads(data)


def _catalog_records(root: Path) -> tuple[dict[str, object], dict[str, object]]:
    catalog = _strict_json(root / "contracts/manifests/suite-manifest.json")
    if not isinstance(catalog, dict) or not isinstance(catalog.get("artifacts"), list):
        raise TypeError("suite catalog shape is invalid")
    descriptor = next(
        record
        for record in catalog["artifacts"]
        if isinstance(record, dict) and record.get("artifact_id") == IR_ARTIFACT_ID
    )
    index_path = next(
        output["path"]
        for record in catalog["artifacts"]
        if isinstance(record, dict)
        for output in record.get("generated_outputs", [])
        if output.get("output_kind") == "artifact-index"
    )
    index = _strict_json(root / str(index_path))
    if not isinstance(index, dict) or not isinstance(index.get("artifacts"), list):
        raise TypeError("artifact index shape is invalid")
    identity = next(
        record
        for record in index["artifacts"]
        if isinstance(record, dict) and record.get("artifact_id") == IR_ARTIFACT_ID
    )
    return descriptor, identity


def _quoted_literal(values: tuple[str, ...]) -> str:
    return f"Literal[{', '.join(repr(value) for value in values)}]"


def _annotation(type_: IrType) -> str:
    if type_.kind == "boolean":
        return "bool"
    if type_.kind == "checksum":
        return "Checksum"
    if type_.kind == "json-object":
        return "JsonObject"
    if type_.kind == "json-object-list":
        return "tuple[JsonObject, ...]"
    if type_.kind in {"literal", "literal-default"}:
        return _quoted_literal(type_.values)
    if type_.kind == "model":
        return str(type_.name)
    if type_.kind == "model-list":
        return f"tuple[{type_.name}, ...]"
    if type_.kind == "nonnegative-integer":
        return "NonNegativeInt"
    if type_.kind == "optional-json-object":
        return "JsonObject | None"
    if type_.kind == "optional-model":
        return f"{type_.name} | None"
    if type_.kind == "optional-string":
        return "str | None"
    if type_.kind == "string":
        return "str"
    if type_.kind == "string-list":
        return "tuple[str, ...]"
    if type_.kind == "union":
        return str(type_.name)
    raise AssertionError(f"unhandled type kind: {type_.kind}")


def _dependencies(model: IrRecord) -> set[str]:
    return {field.type.name for field in model.fields if field.type.name is not None}


def _ordered_declarations(ir: AdapterModelIr) -> list[IrRecord | IrUnion]:
    pending_models = list(ir.models)
    pending_unions = list(ir.unions)
    available: set[str] = set()
    ordered: list[IrRecord | IrUnion] = []
    while pending_models or pending_unions:
        progress = False
        for model in list(pending_models):
            if _dependencies(model) <= available:
                ordered.append(model)
                available.add(model.name)
                pending_models.remove(model)
                progress = True
        for union in list(pending_unions):
            if set(union.members) <= available:
                ordered.append(union)
                available.add(union.name)
                pending_unions.remove(union)
                progress = True
        if not progress:
            unresolved = [item.name for item in [*pending_models, *pending_unions]]
            raise ValueError(f"adapter model graph is cyclic: {unresolved}")
    return ordered


def _render_model(model: IrRecord) -> list[str]:
    lines = [
        f"class {model.name}(StrictWireModel):",
        f'    """Generated {model.name} wire contract."""',
        "",
    ]
    if not model.fields:
        lines.append("    pass")
        return lines
    for field in model.fields:
        arguments = [f"description={field.description!r}"]
        if field.alias is not None:
            arguments.append(f"alias={field.alias!r}")
        default = ""
        if field.type.kind in {
            "optional-json-object",
            "optional-model",
            "optional-string",
        }:
            default = " = Field(default=None, " + ", ".join(arguments) + ")"
        elif field.type.kind == "literal-default":
            default = (
                f" = Field(default={field.type.default!r}, {', '.join(arguments)})"
            )
        else:
            default = " = Field(" + ", ".join(arguments) + ")"
        lines.append(f"    {field.name}: {_annotation(field.type)}{default}")
    return lines


def render_source(ir: AdapterModelIr, identity: dict[str, object]) -> bytes:
    lines = [
        f"# @generated from {IR_ARTIFACT_ID} {identity['canonical_digest']}; {GENERATOR_REVISION}; do not edit.",
        '"""Statically typed public adapter contracts compiled from Contract IR."""',
        "",
        "from typing import Annotated, Literal",
        "",
        "from pydantic import BaseModel, ConfigDict, Field, JsonValue, StringConstraints, TypeAdapter",
        "",
        'Checksum = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]',
        "NonNegativeInt = Annotated[int, Field(ge=0)]",
        "type JsonObject = dict[str, JsonValue]",
        "",
        "class StrictWireModel(BaseModel):",
        '    """Closed immutable model-visible MCP contract."""',
        "",
        "    model_config = ConfigDict(",
        '        extra="forbid",',
        "        strict=True,",
        "        frozen=True,",
        "        validate_default=True,",
        "        hide_input_in_errors=True,",
        "        allow_inf_nan=False,",
        "        validate_by_alias=True,",
        "        validate_by_name=True,",
        "        serialize_by_alias=True,",
        "    )",
        "",
        "JSON_OBJECT_ADAPTER = TypeAdapter(",
        "    JsonObject,",
        "    config=ConfigDict(strict=True, allow_inf_nan=False, hide_input_in_errors=True),",
        ")",
        "",
    ]
    model_names: list[str] = []
    union_names: list[str] = []
    for declaration in _ordered_declarations(ir):
        if isinstance(declaration, IrRecord):
            lines.extend(_render_model(declaration))
            model_names.append(declaration.name)
        else:
            members = " | ".join(declaration.members)
            lines.extend(
                [
                    f"type {declaration.name} = Annotated[",
                    f"    {members},",
                    f"    Field(discriminator={declaration.discriminator!r}),",
                    "]",
                ]
            )
            union_names.append(declaration.name)
        lines.append("")
    lines.extend(
        [
            f"MODEL_TYPES = ({', '.join(model_names)},)",
            "MODEL_BY_NAME = {model.__name__: model for model in MODEL_TYPES}",
            "TYPE_ADAPTERS = {",
            '    "JsonObject": JSON_OBJECT_ADAPTER,',
            *[f'    "{name}": TypeAdapter({name}),' for name in union_names],
            "}",
            "",
        ]
    )
    return "\n".join(lines).encode()


def _format_source(source: bytes) -> bytes:
    executable = shutil.which("ruff")
    if executable is None:
        raise RuntimeError("ruff is required to format generated adapter models")
    result = subprocess.run(
        [
            executable,
            "format",
            "--line-length",
            "100",
            "--stdin-filename",
            "wire_models.py",
            "-",
        ],
        input=source,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def _load_candidate(source: bytes) -> ModuleType:
    with tempfile.TemporaryDirectory(prefix="codefabric-adapter-models.") as directory:
        path = Path(directory) / "wire_models.py"
        path.write_bytes(source)
        name = "codefabric_cpg_mcp.contracts._candidate_wire_models"
        specification = importlib.util.spec_from_file_location(name, path)
        if specification is None or specification.loader is None:
            raise RuntimeError(
                "could not construct generated-model import specification"
            )
        module = importlib.util.module_from_spec(specification)
        sys.modules[name] = module
        try:
            specification.loader.exec_module(module)
        finally:
            sys.modules.pop(name, None)
        return module


def _schema_id(name: str, mode: str) -> str:
    slug = "".join(
        (f"-{character.lower()}" if character.isupper() else character)
        for character in name
    ).lstrip("-")
    return f"https://codefabric.dev/schema/adapter/1.0/{slug}.{mode}.schema.json"


def render_outputs(root: Path) -> dict[Path, bytes]:
    descriptor, identity = _catalog_records(root)
    authority = root / str(descriptor["authority_path"])
    ir_source = authority.read_bytes()
    ir = AdapterModelIr.model_validate_json(ir_source, strict=True)
    if (
        ir.canonical_digest != identity["canonical_digest"]
        or checksum(ir_source) != identity["source_digest"]
    ):
        raise ValueError(
            "adapter Contract IR identities disagree with the verified artifact index"
        )
    source = _format_source(render_source(ir, identity))
    module = _load_candidate(source)
    schemas: dict[str, dict[str, object]] = {"validation": {}, "serialization": {}}
    fingerprints: dict[str, dict[str, str]] = {"validation": {}, "serialization": {}}
    for mode in ("validation", "serialization"):
        for name, model in module.MODEL_BY_NAME.items():
            schema = model.model_json_schema(mode=mode, by_alias=True)
            schema["$schema"] = DIALECT
            schema["$id"] = _schema_id(name, mode)
            schemas[mode][name] = schema
            fingerprints[mode][name] = checksum(canonicalize_value(schema))
        for name, adapter in module.TYPE_ADAPTERS.items():
            schema = adapter.json_schema(mode=mode, by_alias=True)
            schema["$schema"] = DIALECT
            schema["$id"] = _schema_id(name, mode)
            schemas[mode][name] = schema
            fingerprints[mode][name] = checksum(canonicalize_value(schema))

    generated = {
        "generator_revision": GENERATOR_REVISION,
        "profile": "codefabric-jcs-v1",
        "source_artifact_id": IR_ARTIFACT_ID,
        "source_canonical_digest": identity["canonical_digest"],
        "source_digest": identity["source_digest"],
    }
    schema_manifest = {"_generated": generated, **schemas}
    fingerprint_manifest = {
        "_generated": generated,
        "profile": "codefabric-adapter-schema-fingerprints-v1",
        **fingerprints,
    }
    output_by_kind = {
        "python-adapter-models": source,
        "adapter-schema-manifest": canonicalize_value(schema_manifest),
        "adapter-fingerprint-manifest": canonicalize_value(fingerprint_manifest),
    }
    return {
        Path(str(output["path"])): output_by_kind[str(output["output_kind"])]
        for output in descriptor["generated_outputs"]
        if output.get("producer") == "adapter-model-compiler"
    }


def _write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
        Path(temporary).replace(path)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise


def write(root: Path) -> None:
    outputs = render_outputs(root)
    for relative, data in outputs.items():
        _write_atomic(root / relative, data)
    print(f"generated {len(outputs)} adapter Contract-IR outputs")


def check(root: Path) -> None:
    for relative, expected in render_outputs(root).items():
        if (root / relative).read_bytes() != expected:
            raise SystemExit(f"generated adapter contract drifted: {relative}")
    print("adapter Contract-IR outputs are current")


def repro_check(root: Path) -> None:
    descriptor, _ = _catalog_records(root)
    catalog_path = Path("contracts/manifests/suite-manifest.json")
    index_path = next(
        Path(str(output["path"]))
        for record in _strict_json(root / catalog_path)["artifacts"]  # type: ignore[index]
        for output in record.get("generated_outputs", [])  # type: ignore[union-attr]
        if output.get("output_kind") == "artifact-index"
    )
    authority_path = Path(str(descriptor["authority_path"]))
    with tempfile.TemporaryDirectory(prefix="codefabric-adapter-repro.") as directory:
        generated_roots = []
        for name in ("first", "second"):
            generated_root = Path(directory) / name
            for relative in (catalog_path, index_path, authority_path):
                destination = generated_root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(root / relative, destination)
            write(generated_root)
            generated_roots.append(generated_root)
        for relative in render_outputs(generated_roots[0]):
            if (generated_roots[0] / relative).read_bytes() != (
                generated_roots[1] / relative
            ).read_bytes():
                raise SystemExit(
                    f"adapter Contract-IR reproduction drifted: {relative}"
                )
    print("two isolated adapter Contract-IR generations are byte-identical")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("write", "check", "repro-check"))
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[2]
    )
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if arguments.mode == "write":
        write(root)
    elif arguments.mode == "check":
        check(root)
    else:
        repro_check(root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
