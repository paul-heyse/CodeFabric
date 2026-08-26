"""Execute the released fixture, security, fault, and comparison contracts."""

from __future__ import annotations

import argparse
import copy
import json
import re
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

import blake3
import jsonschema
import rfc8785
import yaml

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_MANIFEST = Path("contracts/manifests/fixture-oracles.json")
SECURITY_MANIFEST = Path("contracts/security/security-corpus-manifest.yaml")
FAULT_REGISTRY = Path("contracts/faults/fault-point-registry.yaml")
COMPARISON_REGISTRY = Path("contracts/comparison/comparison-ignore-registry.yaml")
MODEL_PACK_SCHEMA = Path("contracts/registry/model-pack.schema.json")

NEGATIVE_FIXTURES = (
    Path("contracts/fixtures/model-packs/invalid-executable-field.json"),
    Path("contracts/fixtures/negative/broken-trace-edge.json"),
    Path("contracts/fixtures/negative/drifted-digest.json"),
    Path("contracts/fixtures/negative/perturbed-artifact.json"),
    Path("contracts/fixtures/negative/schema-version-drift.json"),
)


class FixtureVerificationError(ValueError):
    """A released fixture or registry failed its executable contract."""


def _reject_duplicate(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise FixtureVerificationError(f"duplicate JSON member {key!r}")
        value[key] = item
    return value


def strict_json_bytes(source: bytes) -> Any:
    try:
        return json.loads(source, object_pairs_hook=_reject_duplicate)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FixtureVerificationError(f"invalid strict JSON: {error}") from error


def _read_json(root: Path, path: Path) -> Any:
    try:
        return strict_json_bytes((root / path).read_bytes())
    except OSError as error:
        raise FixtureVerificationError(f"cannot read {path}: {error}") from error


def _read_yaml(root: Path, path: Path) -> dict[str, Any]:
    try:
        value = yaml.safe_load((root / path).read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as error:
        raise FixtureVerificationError(f"cannot decode {path}: {error}") from error
    if not isinstance(value, dict):
        raise FixtureVerificationError(f"{path}: expected an object")
    return value


def detached_digest(value: Mapping[str, Any]) -> str:
    projection = dict(value)
    projection.pop("canonical_digest", None)
    projection.pop("source_digest", None)
    return f"b3:{blake3.blake3(rfc8785.dumps(projection)).hexdigest()}"


def _checksum_source(source_utf8: str) -> str:
    value = strict_json_bytes(source_utf8.encode())
    return f"b3:{blake3.blake3(rfc8785.dumps(value)).hexdigest()}"


def _require_object(value: Any, path: Path) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise FixtureVerificationError(f"{path}: expected an object")
    return value


def _requirements(root: Path) -> set[str]:
    values = []
    for line in (
        (root / "contracts/manifests/requirements.jsonl").read_bytes().splitlines()
    ):
        values.append(
            _require_object(strict_json_bytes(line), Path("requirements.jsonl"))
        )
    return {
        value["requirement_id"]
        for value in values
        if isinstance(value.get("requirement_id"), str)
    }


def _replace_pointer(value: Any, pointer: str, replacement: Any) -> None:
    if not pointer.startswith("/"):
        raise FixtureVerificationError("JSON pointer must be absolute")
    parts = [
        part.replace("~1", "/").replace("~0", "~") for part in pointer[1:].split("/")
    ]
    target = value
    for part in parts[:-1]:
        if not isinstance(target, dict) or part not in target:
            raise FixtureVerificationError(f"JSON pointer does not resolve: {pointer}")
        target = target[part]
    if not isinstance(target, dict) or parts[-1] not in target:
        raise FixtureVerificationError(f"JSON pointer does not resolve: {pointer}")
    target[parts[-1]] = replacement


def _verify_model_pack(root: Path, value: Any, *, expected_valid: bool) -> None:
    schema = _read_json(root, MODEL_PACK_SCHEMA)
    validator = jsonschema.Draft202012Validator(schema)
    valid = validator.is_valid(value)
    if valid != expected_valid:
        state = "valid" if valid else "invalid"
        raise FixtureVerificationError(f"model-pack fixture unexpectedly {state}")


def verify_negative_fixture(root: Path, path: Path, value: Any | None = None) -> str:
    """Execute one negative fixture and require its declared failure."""

    value = _require_object(_read_json(root, path) if value is None else value, path)
    if path == Path("contracts/fixtures/model-packs/invalid-executable-field.json"):
        _verify_model_pack(root, value, expected_valid=False)
        return "model-pack-schema-rejected"
    if path.name == "broken-trace-edge.json":
        requirement = value.get("trace", {}).get("requirement_id")
        if requirement in _requirements(root):
            raise FixtureVerificationError(
                "broken trace fixture did not contain an unknown edge"
            )
        if value.get("expected_failure_class") != "unknown-requirement":
            raise FixtureVerificationError("broken trace fixture failure class differs")
        return "unknown-requirement"
    if path.name in {"drifted-digest.json", "perturbed-artifact.json"}:
        source = value.get("source_utf8")
        claimed = value.get("claimed_checksum")
        if not isinstance(source, str) or not isinstance(claimed, str):
            raise FixtureVerificationError("digest fixture fields are invalid")
        if _checksum_source(source) == claimed:
            raise FixtureVerificationError(
                "negative digest fixture unexpectedly verified"
            )
        return "checksum-mismatch"
    if path.name == "schema-version-drift.json":
        if value.get("expected_error") != "SCHEMA_VERSION_NOT_ADVANCED":
            raise FixtureVerificationError(
                "schema drift fixture expected error differs"
            )
        schema_path = Path(str(value.get("schema_path", "")))
        source = _require_object(_read_json(root, schema_path), schema_path)
        mutated = copy.deepcopy(source)
        _replace_pointer(
            mutated, str(value.get("mutation_pointer", "")), value.get("replacement")
        )
        if rfc8785.dumps(source) == rfc8785.dumps(mutated):
            raise FixtureVerificationError(
                "schema drift fixture made no semantic change"
            )
        version_pointer = str(value.get("version_pointer", ""))
        original_version = copy.deepcopy(source)
        mutated_version = copy.deepcopy(mutated)
        marker = object()

        def resolve(document: Any) -> Any:
            current = document
            for part in version_pointer.removeprefix("/").split("/"):
                if not isinstance(current, dict):
                    return marker
                current = current.get(
                    part.replace("~1", "/").replace("~0", "~"), marker
                )
            return current

        if resolve(original_version) != resolve(mutated_version):
            raise FixtureVerificationError("schema drift fixture advanced its version")
        return "SCHEMA_VERSION_NOT_ADVANCED"
    raise FixtureVerificationError(f"unregistered negative fixture {path}")


def verify_fixture_manifest(root: Path = ROOT) -> dict[str, Any]:
    manifest = _require_object(_read_json(root, FIXTURE_MANIFEST), FIXTURE_MANIFEST)
    if manifest.get("status") != "released":
        raise FixtureVerificationError("fixture oracle manifest must be released")
    digest = manifest.get("canonical_digest")
    if not isinstance(digest, str) or digest == f"b3:{'0' * 64}":
        raise FixtureVerificationError(
            "released fixture oracle manifest has a reserved digest"
        )
    if digest != detached_digest(manifest):
        raise FixtureVerificationError("fixture oracle manifest digest differs")
    records = manifest.get("records")
    if not isinstance(records, list):
        raise FixtureVerificationError("fixture oracle records are absent")
    paths = [Path(record["path"]) for record in records if isinstance(record, dict)]
    if len(paths) != len(records) or len(paths) != len(set(paths)):
        raise FixtureVerificationError("fixture oracle paths are missing or duplicated")
    missing = [path.as_posix() for path in paths if not (root / path).is_file()]
    if missing:
        raise FixtureVerificationError(f"fixture oracle paths are absent: {missing}")
    declared_negative = {
        Path(record["path"])
        for record in records
        if isinstance(record, dict) and record.get("oracle_class") == "negative-class"
    }
    if declared_negative != set(NEGATIVE_FIXTURES):
        raise FixtureVerificationError(
            "negative fixture census differs: "
            f"missing={sorted(map(str, set(NEGATIVE_FIXTURES) - declared_negative))}, "
            f"extra={sorted(map(str, declared_negative - set(NEGATIVE_FIXTURES)))}"
        )
    outcomes = {
        path.as_posix(): verify_negative_fixture(root, path)
        for path in NEGATIVE_FIXTURES
    }
    valid_pack = _read_json(
        root, Path("contracts/fixtures/model-packs/valid-minimal.json")
    )
    _verify_model_pack(root, valid_pack, expected_valid=True)
    return {
        "fixture_count": len(records),
        "negative_fixture_count": len(outcomes),
        "negative_outcomes": outcomes,
        "manifest_digest": digest,
    }


def _strict_json_rejects(source: str) -> bool:
    try:
        strict_json_bytes(source.encode())
    except FixtureVerificationError:
        return True
    return False


def verify_security_corpus(root: Path = ROOT) -> dict[str, Any]:
    manifest = _read_yaml(root, SECURITY_MANIFEST)
    records = manifest.get("records")
    if not isinstance(records, list):
        raise FixtureVerificationError("security corpus records are absent")
    operations: list[str] = []
    for record in records:
        if not isinstance(record, dict):
            raise FixtureVerificationError("security corpus record is not an object")
        operation = record.get("operation")
        fixture_path = Path(str(record.get("fixture_path", "")))
        if not (root / fixture_path).is_file():
            raise FixtureVerificationError(
                f"security fixture is absent: {fixture_path}"
            )
        if operation == "replay-negative-duplicate-member-vectors":
            vectors = _require_object(_read_json(root, fixture_path), fixture_path)
            duplicate_cases = [
                case
                for case in vectors.get("negative", [])
                if isinstance(case, dict) and case.get("error") == "duplicate-key"
            ]
            if not duplicate_cases or not all(
                _strict_json_rejects(str(case["input_json"]))
                for case in duplicate_cases
            ):
                raise FixtureVerificationError(
                    "duplicate-member security cases were not rejected"
                )
        elif operation in {"validate-model-pack", "verify-contract-checksum"}:
            verify_negative_fixture(root, fixture_path)
        elif operation == "decode-four-production-protobuf-packages":
            fixture = _require_object(_read_json(root, fixture_path), fixture_path)
            cases = fixture.get("cases")
            if not isinstance(cases, list) or len(cases) != 4:
                raise FixtureVerificationError("production wire fixture census differs")
            for case in cases:
                if not isinstance(case, dict) or not re.fullmatch(
                    r"(?:[0-9a-f]{2})+", str(case.get("wire_hex", ""))
                ):
                    raise FixtureVerificationError(
                        "production wire fixture is malformed"
                    )
                bytes.fromhex(str(case["wire_hex"]))
        elif operation == "capture-during-active-rewriter":
            fixture = _require_object(_read_json(root, fixture_path), fixture_path)
            if fixture.get("required_false_stable_publications") != 0:
                raise FixtureVerificationError(
                    "source capture security outcome is not fail-closed"
                )
            recipes = json.loads(
                subprocess.run(
                    ("just", "--dump", "--dump-format", "json"),
                    cwd=root,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout
            )["recipes"]
            if "source-capture-race-check" not in recipes:
                raise FixtureVerificationError(
                    "source capture security runner is absent"
                )
        else:
            raise FixtureVerificationError(
                f"security operation has no consumer: {operation}"
            )
        operations.append(str(operation))
    return {"case_count": len(records), "operations": sorted(operations)}


def verify_comparison_registry(root: Path = ROOT) -> dict[str, Any]:
    registry = _read_yaml(root, COMPARISON_REGISTRY)
    records = registry.get("records")
    if not isinstance(records, list) or not records:
        raise FixtureVerificationError("comparison-ignore records are absent")
    ignored = {
        record["field_name"]
        for record in records
        if isinstance(record, dict)
        and record.get("semantic") is False
        and isinstance(record.get("field_name"), str)
    }
    if len(ignored) != len(records):
        raise FixtureVerificationError(
            "comparison-ignore registry is not a closed unique census"
        )

    def semantic_projection(value: Mapping[str, Any]) -> dict[str, Any]:
        return {key: item for key, item in value.items() if key not in ignored}

    baseline = {"entity_count": 7, **{field: f"left-{field}" for field in ignored}}
    replay = {"entity_count": 7, **{field: f"right-{field}" for field in ignored}}
    changed = {**replay, "entity_count": 8}
    if semantic_projection(baseline) != semantic_projection(replay):
        raise FixtureVerificationError(
            "registered operational fields changed semantic equality"
        )
    if semantic_projection(baseline) == semantic_projection(changed):
        raise FixtureVerificationError("unregistered semantic field was ignored")
    return {"ignored_field_count": len(ignored), "semantic_difference_detected": True}


def verify_fault_registry(root: Path = ROOT) -> dict[str, Any]:
    registry = _read_yaml(root, FAULT_REGISTRY)
    records = registry.get("records")
    if not isinstance(records, list) or not records:
        raise FixtureVerificationError("fault-point records are absent")
    codes = []
    for record in records:
        if not isinstance(record, dict):
            raise FixtureVerificationError("fault-point record is not an object")
        code = record.get("code")
        if (
            not isinstance(code, str)
            or not re.fullmatch(r"[A-Z][A-Z0-9_]+", code)
            or record.get("production_exposable") is not False
            or not record.get("allowed_actions")
            or not record.get("expected_invariants")
            or not record.get("scenarios")
        ):
            raise FixtureVerificationError(f"fault-point record is incomplete: {code}")
        codes.append(code)
    if len(codes) != len(set(codes)):
        raise FixtureVerificationError("fault-point codes are duplicated")
    return {"fault_point_count": len(codes), "codes": sorted(codes)}


def verify_released_assurance(root: Path = ROOT) -> dict[str, Any]:
    return {
        "fixtures": verify_fixture_manifest(root),
        "security": verify_security_corpus(root),
        "faults": verify_fault_registry(root),
        "comparison": verify_comparison_registry(root),
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        report = verify_released_assurance(args.root)
    except (FixtureVerificationError, subprocess.CalledProcessError) as error:
        print(f"released fixture verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
