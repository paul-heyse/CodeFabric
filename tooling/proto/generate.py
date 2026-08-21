"""Generate and verify Wave 0 bindings from one committed descriptor authority."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import importlib.resources
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from google.protobuf import descriptor_pb2, json_format
from grpc_tools import protoc

ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = ROOT / "tooling" / "proto" / "source"
CATALOG_PATH = ROOT / "contracts" / "manifests" / "suite-manifest.json"
BASELINE = ROOT / "tooling" / "proto" / "compatibility-baseline.json"
EXACT_PYTHON_PACKAGES = {
    "grpcio": "1.83.0",
    "grpcio-tools": "1.83.0",
    "protobuf": "7.36.0",
}
GRPC_TOOLS_PROTOC = "libprotoc 35.1"


def contract_catalog() -> dict[str, Any]:
    return json.loads(CATALOG_PATH.read_bytes())


def proto_sources() -> tuple[tuple[Path, Path], ...]:
    sources = []
    for artifact in contract_catalog()["artifacts"]:
        if artifact["native_format"] != "proto":
            continue
        authority = Path(artifact["authority_path"])
        try:
            compiler_name = authority.relative_to("tooling/proto/source")
        except ValueError:
            compiler_name = authority
        sources.append((compiler_name, ROOT / authority))
    return tuple(sorted(sources))


def output_destination(output_kind: str) -> Path:
    matches = [
        ROOT / output["path"]
        for artifact in contract_catalog()["artifacts"]
        for output in artifact["generated_outputs"]
        if output.get("producer", "contract-compiler") == "proto-compiler"
        and output["output_kind"] == output_kind
    ]
    if len(matches) != 1:
        raise RuntimeError(
            f"catalog must declare one {output_kind} proto output, got {matches}"
        )
    return matches[0]


COMPILER_SOURCES = proto_sources()
SOURCE_RELATIVE = Path("codefabric_cpg_mcp/daemon/generated/wave0_probe.proto")
SOURCE = dict(COMPILER_SOURCES)[SOURCE_RELATIVE]
DESCRIPTOR_DESTINATION = output_destination("proto-descriptor-set")
CENSUS_DESTINATION = output_destination("proto-descriptor-census")
IDENTITY_DESTINATION = output_destination("proto-toolchain-identity")
RUST_DESTINATION_FILE = output_destination("rust-proto-bindings")
RUST_DESTINATION = RUST_DESTINATION_FILE.parent
RUST_OUTPUT = RUST_DESTINATION_FILE.name
PYTHON_DESTINATIONS = {
    "wave0_probe_pb2.py": output_destination("python-proto-bindings"),
    "wave0_probe_pb2.pyi": output_destination("python-proto-stub"),
    "wave0_probe_pb2_grpc.py": output_destination("python-grpc-bindings"),
}
PYTHON_OUTPUTS = tuple(PYTHON_DESTINATIONS)


def clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.pop("VIRTUAL_ENV", None)
    environment.pop("UV_PROJECT_ENVIRONMENT", None)
    return environment


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, env=clean_environment(), check=True)


def source_digest(path: Path = SOURCE) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identities() -> dict[str, str]:
    return {
        relative.as_posix(): source_digest(path) for relative, path in COMPILER_SOURCES
    }


def source_set_digest() -> str:
    digest = hashlib.sha256()
    for relative, path in COMPILER_SOURCES:
        digest.update(relative.as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def generated_header(comment: str) -> bytes:
    return (
        f"{comment} @generated from catalog proto source set "
        f"sha256:{source_set_digest()}; do not edit.\n"
    ).encode()


def prepend_header(path: Path, comment: str) -> None:
    path.write_bytes(generated_header(comment) + path.read_bytes())


def assert_exact_python_versions(
    versions: dict[str, str] | None = None,
) -> dict[str, str]:
    actual = versions or {
        package: importlib.metadata.version(package)
        for package in EXACT_PYTHON_PACKAGES
    }
    if actual != EXACT_PYTHON_PACKAGES:
        raise RuntimeError(
            f"Protobuf toolchain version mismatch: expected {EXACT_PYTHON_PACKAGES}, got {actual}"
        )
    return actual


def cargo_package_versions() -> dict[str, str]:
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--features", "proto-tooling"],
        cwd=ROOT,
        env=clean_environment(),
        check=True,
        capture_output=True,
        text=True,
    )
    packages = json.loads(completed.stdout)["packages"]
    wanted = {
        "prost",
        "prost-build",
        "prost-types",
        "tonic",
        "tonic-build",
        "tonic-prost",
        "tonic-prost-build",
    }
    resolved = {
        package["name"]: package["version"]
        for package in packages
        if package["name"] in wanted
    }
    missing = wanted - resolved.keys()
    if missing:
        raise RuntimeError(f"missing generator packages: {sorted(missing)}")
    return dict(sorted(resolved.items()))


def invoke_compiler(python_output: Path, descriptor: Path) -> None:
    """Invoke the exact compiler once for Python code and descriptor IR."""
    bundled_include = importlib.resources.files("grpc_tools").joinpath("_proto")
    arguments = [
        "grpc_tools.protoc",
        f"-I{ROOT}",
        f"-I{SOURCE_ROOT}",
        f"-I{bundled_include}",
        f"--python_out={python_output}",
        f"--pyi_out={python_output}",
        f"--grpc_python_out={python_output}",
        f"--descriptor_set_out={descriptor}",
        "--include_imports",
        *(relative.as_posix() for relative, _ in COMPILER_SOURCES),
    ]
    result = protoc.main(arguments)
    if result != 0:
        raise RuntimeError(f"grpc_tools.protoc failed with exit status {result}")


def descriptor_set(path: Path) -> descriptor_pb2.FileDescriptorSet:
    descriptors = descriptor_pb2.FileDescriptorSet()
    descriptors.ParseFromString(path.read_bytes())
    return descriptors


def assert_descriptor_profile(descriptors: descriptor_pb2.FileDescriptorSet) -> None:
    names = [file.name for file in descriptors.file]
    if len(names) != len(set(names)):
        raise RuntimeError("descriptor set contains duplicate file names")
    known = set(names)
    for file in descriptors.file:
        if file.source_code_info.location:
            raise RuntimeError(f"semantic descriptor contains source info: {file.name}")
        missing = set(file.dependency) - known
        if missing:
            raise RuntimeError(
                f"descriptor dependency closure is incomplete for {file.name}: {sorted(missing)}"
            )


def normalized_options(options: Any) -> dict[str, Any]:
    normalized = json_format.MessageToDict(
        options,
        preserving_proto_field_name=True,
        always_print_fields_with_no_presence=False,
    )
    encoded = options.SerializeToString(deterministic=True)
    if encoded:
        # Unknown custom extensions are preserved in the options message even when
        # this runtime has not registered their generated Python extension module.
        normalized["$wire_hex"] = encoded.hex()
    return normalized


def full_name(package: str, parents: tuple[str, ...], name: str) -> str:
    return ".".join(part for part in (package, *parents, name) if part)


def normalized_enum(
    package: str,
    parents: tuple[str, ...],
    enum: descriptor_pb2.EnumDescriptorProto,
) -> dict[str, Any]:
    return {
        "full_name": full_name(package, parents, enum.name),
        "options": normalized_options(enum.options),
        "reserved_names": sorted(enum.reserved_name),
        "reserved_ranges": sorted(
            (
                {"start": item.start, "end_inclusive": item.end}
                for item in enum.reserved_range
            ),
            key=lambda item: (item["start"], item["end_inclusive"]),
        ),
        "values": sorted(
            (
                {
                    "name": value.name,
                    "number": value.number,
                    "options": normalized_options(value.options),
                }
                for value in enum.value
            ),
            key=lambda item: (item["number"], item["name"]),
        ),
    }


def field_has_presence(syntax: str, field: descriptor_pb2.FieldDescriptorProto) -> bool:
    message_kinds = {
        descriptor_pb2.FieldDescriptorProto.TYPE_MESSAGE,
        descriptor_pb2.FieldDescriptorProto.TYPE_GROUP,
    }
    return (
        syntax != "proto3"
        or field.proto3_optional
        or field.HasField("oneof_index")
        or field.type in message_kinds
    )


def normalized_message(
    package: str,
    parents: tuple[str, ...],
    syntax: str,
    message: descriptor_pb2.DescriptorProto,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    current_parents = (*parents, message.name)
    oneofs = [item.name for item in message.oneof_decl]
    normalized = {
        "full_name": full_name(package, parents, message.name),
        "fields": sorted(
            (
                {
                    "name": field.name,
                    "number": field.number,
                    "label": descriptor_pb2.FieldDescriptorProto.Label.Name(
                        field.label
                    ),
                    "type": descriptor_pb2.FieldDescriptorProto.Type.Name(field.type),
                    "type_name": field.type_name,
                    "json_name": field.json_name,
                    "oneof": oneofs[field.oneof_index]
                    if field.HasField("oneof_index")
                    else None,
                    "proto3_optional": field.proto3_optional,
                    "has_presence": field_has_presence(syntax, field),
                    "default_value": field.default_value
                    if field.HasField("default_value")
                    else None,
                    "options": normalized_options(field.options),
                }
                for field in message.field
            ),
            key=lambda item: (item["number"], item["name"]),
        ),
        "oneofs": [
            {"name": item.name, "options": normalized_options(item.options)}
            for item in message.oneof_decl
        ],
        "options": normalized_options(message.options),
        "reserved_names": sorted(message.reserved_name),
        "reserved_ranges": sorted(
            (
                {"start": item.start, "end_exclusive": item.end}
                for item in message.reserved_range
            ),
            key=lambda item: (item["start"], item["end_exclusive"]),
        ),
        "extension_ranges": sorted(
            (
                {"start": item.start, "end_exclusive": item.end}
                for item in message.extension_range
            ),
            key=lambda item: (item["start"], item["end_exclusive"]),
        ),
    }
    messages = [normalized]
    enums = [
        normalized_enum(package, current_parents, enum) for enum in message.enum_type
    ]
    for nested in message.nested_type:
        nested_messages, nested_enums = normalized_message(
            package, current_parents, syntax, nested
        )
        messages.extend(nested_messages)
        enums.extend(nested_enums)
    return messages, enums


def normalized_census(descriptors: descriptor_pb2.FileDescriptorSet) -> dict[str, Any]:
    files: list[dict[str, Any]] = []
    for file in descriptors.file:
        syntax = file.syntax or "proto2"
        messages: list[dict[str, Any]] = []
        enums = [normalized_enum(file.package, (), enum) for enum in file.enum_type]
        for message in file.message_type:
            nested_messages, nested_enums = normalized_message(
                file.package, (), syntax, message
            )
            messages.extend(nested_messages)
            enums.extend(nested_enums)
        dependencies = list(file.dependency)
        files.append(
            {
                "name": file.name,
                "package": file.package,
                "syntax": syntax,
                "edition": descriptor_pb2.Edition.Name(file.edition)
                if file.HasField("edition")
                else None,
                "dependencies": sorted(dependencies),
                "public_dependencies": sorted(
                    dependencies[index] for index in file.public_dependency
                ),
                "weak_dependencies": sorted(
                    dependencies[index] for index in file.weak_dependency
                ),
                "options": normalized_options(file.options),
                "messages": sorted(messages, key=lambda item: item["full_name"]),
                "enums": sorted(enums, key=lambda item: item["full_name"]),
                "services": sorted(
                    (
                        {
                            "full_name": full_name(file.package, (), service.name),
                            "options": normalized_options(service.options),
                            "methods": sorted(
                                (
                                    {
                                        "name": method.name,
                                        "input_type": method.input_type,
                                        "output_type": method.output_type,
                                        "client_streaming": method.client_streaming,
                                        "server_streaming": method.server_streaming,
                                        "options": normalized_options(method.options),
                                    }
                                    for method in service.method
                                ),
                                key=lambda item: item["name"],
                            ),
                        }
                        for service in file.service
                    ),
                    key=lambda item: item["full_name"],
                ),
            }
        )
    return {"schema": 1, "files": sorted(files, key=lambda item: item["name"])}


def encoded_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def indexed(items: list[dict[str, Any]], key: str) -> dict[Any, dict[str, Any]]:
    return {item[key]: item for item in items}


def range_contains(ranges: list[dict[str, int]], number: int, end_key: str) -> bool:
    if end_key == "end_exclusive":
        return any(item["start"] <= number < item[end_key] for item in ranges)
    return any(item["start"] <= number <= item[end_key] for item in ranges)


def require_subset(old: list[Any], new: list[Any], context: str) -> None:
    missing = [item for item in old if item not in new]
    if missing:
        raise RuntimeError(f"{context} removed retained values: {missing}")


def require_ranges_preserved(
    old: list[dict[str, int]],
    new: list[dict[str, int]],
    context: str,
    end_key: str,
) -> None:
    for old_range in old:
        start = old_range["start"]
        end = old_range[end_key]
        cursor = start
        for new_range in sorted(new, key=lambda item: item["start"]):
            if new_range[end_key] < cursor or new_range["start"] > cursor:
                continue
            cursor = max(cursor, new_range[end_key] + (end_key == "end_inclusive"))
            if cursor >= end + (end_key == "end_inclusive"):
                break
        if cursor < end + (end_key == "end_inclusive"):
            raise RuntimeError(f"{context} removed reserved range: {old_range}")


def assert_fields_compatible(old: dict[str, Any], new: dict[str, Any]) -> None:
    context = old["full_name"]
    old_by_number = indexed(old["fields"], "number")
    new_by_number = indexed(new["fields"], "number")
    new_by_name = indexed(new["fields"], "name")
    require_subset(old["reserved_names"], new["reserved_names"], context)
    require_ranges_preserved(
        old["reserved_ranges"], new["reserved_ranges"], context, "end_exclusive"
    )
    for number, field in old_by_number.items():
        replacement = new_by_number.get(number)
        if replacement is None:
            if field["name"] not in new["reserved_names"] or not range_contains(
                new["reserved_ranges"], number, "end_exclusive"
            ):
                raise RuntimeError(
                    f"{context}.{field['name']} ({number}) removed without reserving name and number"
                )
            continue
        if replacement != field:
            raise RuntimeError(
                f"{context} field {number} changed incompatibly: {field} -> {replacement}"
            )
        same_name = new_by_name.get(field["name"])
        if same_name != replacement:
            raise RuntimeError(f"{context}.{field['name']} changed field number")


def assert_enums_compatible(old: dict[str, Any], new: dict[str, Any]) -> None:
    context = old["full_name"]
    require_subset(old["reserved_names"], new["reserved_names"], context)
    require_ranges_preserved(
        old["reserved_ranges"], new["reserved_ranges"], context, "end_inclusive"
    )
    new_by_number = indexed(new["values"], "number")
    for value in old["values"]:
        replacement = new_by_number.get(value["number"])
        if replacement is None:
            if value["name"] not in new["reserved_names"] or not range_contains(
                new["reserved_ranges"], value["number"], "end_inclusive"
            ):
                raise RuntimeError(
                    f"{context}.{value['name']} ({value['number']}) removed without reservation"
                )
            continue
        if replacement != value:
            raise RuntimeError(
                f"{context} enum number {value['number']} changed incompatibly"
            )


def assert_supported_features(census: dict[str, Any]) -> None:
    for file in census["files"]:
        if not file["package"].startswith("codefabric."):
            continue
        if file["syntax"] != "proto3" or file["edition"] is not None:
            raise RuntimeError(
                f"unsupported required Protobuf feature set in {file['name']}: "
                f"syntax={file['syntax']}, edition={file['edition']}"
            )
        for message in file["messages"]:
            for field in message["fields"]:
                if field["label"] == "LABEL_REQUIRED":
                    raise RuntimeError(
                        f"unsupported required field: {message['full_name']}"
                    )


def assert_compatible(baseline: dict[str, Any], current: dict[str, Any]) -> None:
    assert_supported_features(current)
    current_files = indexed(current["files"], "name")
    for old_file in baseline["files"]:
        new_file = current_files.get(old_file["name"])
        if new_file is None:
            raise RuntimeError(f"descriptor file removed: {old_file['name']}")
        for key in ("package", "syntax", "edition", "options"):
            if old_file[key] != new_file[key]:
                raise RuntimeError(f"{old_file['name']} changed {key} incompatibly")
        for key in (
            "dependencies",
            "public_dependencies",
            "weak_dependencies",
        ):
            require_subset(old_file[key], new_file[key], f"{old_file['name']} {key}")

        new_messages = indexed(new_file["messages"], "full_name")
        for old_message in old_file["messages"]:
            new_message = new_messages.get(old_message["full_name"])
            if new_message is None:
                raise RuntimeError(f"message removed: {old_message['full_name']}")
            assert_fields_compatible(old_message, new_message)

        new_enums = indexed(new_file["enums"], "full_name")
        for old_enum in old_file["enums"]:
            new_enum = new_enums.get(old_enum["full_name"])
            if new_enum is None:
                raise RuntimeError(f"enum removed: {old_enum['full_name']}")
            assert_enums_compatible(old_enum, new_enum)

        new_services = indexed(new_file["services"], "full_name")
        for old_service in old_file["services"]:
            new_service = new_services.get(old_service["full_name"])
            if new_service is None:
                raise RuntimeError(f"service removed: {old_service['full_name']}")
            new_methods = indexed(new_service["methods"], "name")
            for old_method in old_service["methods"]:
                if new_methods.get(old_method["name"]) != old_method:
                    raise RuntimeError(
                        f"RPC cardinality or type drift: "
                        f"{old_service['full_name']}.{old_method['name']}"
                    )


def generate_into(root: Path) -> tuple[dict[str, Path], dict[str, Any]]:
    assert_exact_python_versions()
    rust_output = root / "rust"
    python_output = root / "python"
    descriptor = root / "wave0-probe-descriptor.pb"
    rust_roundtrip = root / "rust-decoded-descriptor.pb"
    census_path = root / "descriptor-census.json"
    rust_output.mkdir(parents=True)
    python_output.mkdir(parents=True)

    invoke_compiler(python_output, descriptor)
    descriptors = descriptor_set(descriptor)
    assert_descriptor_profile(descriptors)
    census = normalized_census(descriptors)
    assert_supported_features(census)
    census_path.write_bytes(encoded_json(census))

    run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--no-default-features",
            "--features",
            "proto-tooling",
            "--bin",
            "codefabric-proto-gen",
            "--",
            "--descriptor",
            str(descriptor),
            "--roundtrip-descriptor-out",
            str(rust_roundtrip),
            "--rust-out",
            str(rust_output),
        ]
    )
    if rust_roundtrip.read_bytes() != descriptor.read_bytes():
        raise RuntimeError(
            "Rust descriptor decode/re-encode drifted from the compiler IR"
        )

    rust_file = rust_output / RUST_OUTPUT
    python_package = python_output / SOURCE_RELATIVE.parent
    files = {"descriptor": descriptor, "census": census_path, "rust": rust_file}
    for output in PYTHON_OUTPUTS:
        files[f"python/{output}"] = python_package / output

    prepend_header(rust_file, "//")
    for key, path in files.items():
        if key.startswith("python/"):
            prepend_header(path, "#")
    run(["rustfmt", "--edition", "2024", str(rust_file)])
    run(["rustfmt", "--edition", "2024", "--check", str(rust_file)])
    return files, census


def output_digest(files: dict[str, Path]) -> str:
    digest = hashlib.sha256()
    for name, path in sorted(files.items()):
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def identity(files: dict[str, Path], versions: dict[str, str]) -> dict[str, Any]:
    return {
        "schema": 3,
        "authority": (
            "single grpc_tools.protoc invocation -> descriptor + Python; Rust compile_fds"
        ),
        "sources": source_identities(),
        "descriptor_sha256": hashlib.sha256(
            files["descriptor"].read_bytes()
        ).hexdigest(),
        "generated_sha256": output_digest(files),
        "rust": {
            "packages": cargo_package_versions(),
            "descriptor_api": "tonic_prost_build::Builder::compile_fds",
            "toolchain_policy": "stable root; declared MSRV 1.94.1",
        },
        "python": {
            **versions,
            "protoc": GRPC_TOOLS_PROTOC,
            "runtime": sys.version.split()[0],
        },
    }


def destination_files() -> dict[str, Path]:
    files = {
        "descriptor": DESCRIPTOR_DESTINATION,
        "census": CENSUS_DESTINATION,
        "rust": RUST_DESTINATION / RUST_OUTPUT,
    }
    files.update({f"python/{name}": path for name, path in PYTHON_DESTINATIONS.items()})
    return files


def assert_equal(expected: dict[str, Path], actual: dict[str, Path]) -> None:
    for name in sorted(expected):
        if not actual[name].is_file():
            raise RuntimeError(f"missing committed generated output: {actual[name]}")
        if expected[name].read_bytes() != actual[name].read_bytes():
            raise RuntimeError(f"generated output drift: {name}")


def write_outputs(files: dict[str, Path], identity_value: dict[str, Any]) -> None:
    RUST_DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, source in files.items():
        destination = destination_files()[name]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    IDENTITY_DESTINATION.write_bytes(encoded_json(identity_value))


def compatibility_baseline() -> dict[str, Any]:
    if not BASELINE.is_file():
        raise RuntimeError(
            "missing reviewed compatibility baseline; bootstrap it explicitly from "
            "descriptor-census.json"
        )
    return json.loads(BASELINE.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("write", "check", "repro-check"))
    arguments = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="codefabric-proto-a-") as first_raw:
        first_files, first_census = generate_into(Path(first_raw))
        baseline = compatibility_baseline()
        assert_compatible(baseline, first_census)
        versions = assert_exact_python_versions()
        first_identity = identity(first_files, versions)

        if arguments.mode == "write":
            write_outputs(first_files, first_identity)
            print(first_identity["generated_sha256"])
            return 0

        assert_equal(first_files, destination_files())
        if IDENTITY_DESTINATION.read_bytes() != encoded_json(first_identity):
            raise RuntimeError("generator identity drift")

        if arguments.mode == "repro-check":
            with tempfile.TemporaryDirectory(
                prefix="codefabric-proto-b-"
            ) as second_raw:
                second_files, second_census = generate_into(Path(second_raw))
                assert_compatible(baseline, second_census)
                if output_digest(first_files) != output_digest(second_files):
                    raise RuntimeError(
                        "isolated descriptor and binding generations were not byte-identical"
                    )

        print(first_identity["generated_sha256"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
