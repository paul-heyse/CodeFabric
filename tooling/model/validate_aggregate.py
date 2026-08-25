"""Independent Python consumer for the complete staged model DesiredTree."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import rfc8785
import yaml
from blake3 import blake3

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = Path("contracts/generated/model/governance")
VALIDATION = GOVERNANCE / "validation.json"


def digest_bytes(value: bytes) -> str:
    return f"b3:{blake3(value).hexdigest()}"


def read_json(stage: Path, relative: Path) -> object:
    return strict_json((stage / relative).read_bytes())


def unique_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Reject duplicate JSON object names before constructing the value."""
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON object key {key}")
        result[key] = value
    return result


def strict_json(value: bytes) -> Any:
    return json.loads(value, object_pairs_hook=unique_pairs)


def remove_own_identity(value: Any, path: str) -> None:
    assert isinstance(value, dict)
    header = (
        value.get("x-codefabric-artifact") if path.endswith(".schema.json") else value
    )
    assert isinstance(header, dict)
    header.pop("canonical_digest", None)
    header.pop("source_digest", None)


def detached_projection(
    path: str, source: bytes, proto_census: dict[str, Any]
) -> bytes:
    """Independent implementation of the six detached projection profiles."""
    suffix = Path(path).suffix.lower()
    if suffix == ".md":
        return source
    if suffix == ".json":
        value = strict_json(source)
        remove_own_identity(value, path)
        return rfc8785.dumps(value)
    if suffix in {".yaml", ".yml"}:
        value = yaml.safe_load(source)
        remove_own_identity(value, path)
        return rfc8785.dumps(value)
    if suffix == ".jsonl":
        assert source.endswith(b"\n")
        lines = source[:-1].split(b"\n")
        assert lines and all(lines)
        values = [strict_json(line) for line in lines]
        remove_own_identity(values[0], path)
        return b"".join(rfc8785.dumps(value) + b"\n" for value in values)
    if suffix == ".ebnf":
        normalized = source.decode().replace("\r\n", "\n")
        assert "\r" not in normalized
        lines = normalized.splitlines(keepends=True)
        offset = 0
        for line in lines:
            logical = line.strip()
            if not (logical.startswith("(*") and logical.endswith("*)")):
                break
            offset += len(line)
        assert offset > 0
        return normalized[offset:].encode()
    if suffix == ".proto":
        selected = next(
            item
            for item in proto_census["files"]
            if Path(item["name"]).name == Path(path).name
        )
        return rfc8785.dumps({"files": [selected]})
    return source


def validate_projection_kats() -> None:
    own = f"b3:{'0' * 64}"
    nested = f"b3:{'1' * 64}"
    json_source = json.dumps(
        {
            "canonical_digest": own,
            "source_digest": own,
            "nested": {"canonical_digest": nested},
            "value": 1,
        },
        separators=(",", ":"),
    ).encode()
    yaml_source = (
        f"canonical_digest: {own}\nsource_digest: {own}\n"
        f"nested:\n  canonical_digest: {nested}\nvalue: 1\n"
    ).encode()
    expected = "b3:56e968977129a90a5d28259ef45c4cb79b721124e30cc442ade8bc22545e3045"
    assert (
        digest_bytes(detached_projection("example.json", json_source, {})) == expected
    )
    assert (
        digest_bytes(detached_projection("example.yaml", yaml_source, {})) == expected
    )


def main() -> int:
    stage = Path(sys.argv[1]).resolve()
    validate_projection_kats()
    validation = read_json(stage, VALIDATION)
    manifest = read_json(stage, GOVERNANCE / "suite-manifest.json")
    artifact_index_path = (
        stage
        / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
    )
    artifact_index = strict_json(artifact_index_path.read_bytes())
    requirements = [
        strict_json(line)
        for line in (stage / GOVERNANCE / "requirements.jsonl")
        .read_bytes()
        .splitlines()
    ]
    traceability = [
        strict_json(line)
        for line in (stage / GOVERNANCE / "traceability.jsonl")
        .read_bytes()
        .splitlines()
    ]
    bundles = read_json(stage, GOVERNANCE / "bundles.json")["bundles"]
    fixtures = read_json(stage, GOVERNANCE / "fixture-index.json")["fixtures"]
    assert artifact_index_path.read_bytes() == rfc8785.dumps(artifact_index)
    artifacts = manifest["artifacts"]
    assert artifacts == artifact_index["artifacts"]
    assert len({artifact["artifact_id"] for artifact in artifacts}) == len(artifacts)
    assert all(
        artifact["canonical_digest"].startswith("b3:")
        and artifact["source_digest"].startswith("b3:")
        for artifact in artifacts
    )
    released = [
        artifact for artifact in artifacts if artifact["release_status"] == "released"
    ]
    proto_census = read_json(stage, Path("tooling/proto/descriptor-census.json"))
    for artifact in artifacts:
        staged_source = stage / artifact["authority_path"]
        source = (
            staged_source
            if staged_source.is_file()
            else ROOT / artifact["authority_path"]
        ).read_bytes()
        assert artifact["source_digest"] == digest_bytes(source)
        if "typed" not in artifact["projection_profile"]:
            assert artifact["canonical_digest"] == digest_bytes(
                detached_projection(artifact["authority_path"], source, proto_census)
            )
    assert len(requirements) == len(traceability) == 84
    requirement_ids = [record["requirement_id"] for record in requirements]
    traceability_ids = [record["requirement_id"] for record in traceability]
    assert len(set(requirement_ids)) == len(requirement_ids)
    assert traceability_ids == requirement_ids
    assert requirements != traceability
    assert all(
        record["source_artifact"]
        and record["source_section"]
        and record["status"] == "active"
        and record["owner_acceptance"]["approver"]
        and record["implements"]
        and record["verified_by"]
        and record["normative_text_digest"]
        == digest_bytes(record["normative_text"].encode())
        for record in requirements
    )
    assert all(
        record["implements"]
        and record["verified_by"]
        and isinstance(record["traces_to"], dict)
        for record in traceability
    )

    assert len(bundles) == 8
    for bundle in bundles:
        ids = [member["artifact_id"] for member in bundle["artifacts"]]
        assert ids == sorted(ids) and len(ids) == len(set(ids)) and ids
        assert bundle["bundle_major"] == 1
        assert bundle["bundle_version"] == "1.0"
        assert bundle["created_by"]["generator_id"] == "codefabric-model"
        assert bundle["created_by"]["generator_version"] == "1.0"
        assert bundle["compatibility"] == {
            "minimum_consumer_minor": 0,
            "maximum_consumer_minor": 0,
        }
        assert all(
            member["version"]
            and member["required"] is True
            and member["feature_bits"] == sorted(set(member["feature_bits"]))
            for member in bundle["artifacts"]
        )
        bundle_projection = dict(bundle)
        bundle_projection.pop("canonical_digest", None)
        bundle_projection.pop("source_digest", None)
        expected_bundle_digest = bundle_projection.pop("bundle_digest")
        bundle_projection.pop("signature", None)
        assert expected_bundle_digest == digest_bytes(rfc8785.dumps(bundle_projection))
    assert fixtures
    assert all(fixture["source_digest"].startswith("b3:") for fixture in fixtures)
    compatibility_manifest = read_json(
        stage, Path("contracts/manifests/suite-manifest.json")
    )
    assert (
        compatibility_manifest["artifact_id"] == "codefabric.manifests.suite-manifest"
    )
    assert compatibility_manifest["artifacts"] == artifacts
    for name in ("requirements", "traceability"):
        records = [
            strict_json(line)
            for line in (stage / f"contracts/manifests/{name}.jsonl")
            .read_bytes()
            .splitlines()
        ]
        assert records[0]["artifact_id"] == f"codefabric.manifests.{name}"
        expected_records = requirements if name == "requirements" else traceability
        assert records[1:] == expected_records
    fixture_manifest = read_json(
        stage, Path("contracts/manifests/fixture-oracles.json")
    )
    fixture_records = fixture_manifest["records"]
    assert fixture_manifest["generator_revision"] == "codefabric-model/1.0"
    assert [record["path"] for record in fixture_records] == [
        record["path"] for record in fixtures
    ]
    assert all(
        set(record)
        == {"path", "oracle_class", "origin", "owner", "version", "change_record"}
        and record["oracle_class"]
        in {
            "normative-kat",
            "differential",
            "property",
            "negative-class",
            "generated-example",
        }
        and record["origin"]
        and record["owner"]
        and record["version"] == "1.0"
        and record["change_record"].startswith("contracts/fixtures/CHANGELOG.md#")
        for record in fixture_records
    )
    for bundle in bundles:
        compatibility_bundle = read_json(
            stage,
            Path(f"contracts/bundles/{bundle['bundle_kind']}-bundle.json"),
        )
        assert compatibility_bundle["artifact_id"] == (
            f"codefabric.bundles.{bundle['bundle_kind']}-bundle"
        )
        for key in (
            "artifacts",
            "bundle_digest",
            "bundle_kind",
            "bundle_major",
            "bundle_version",
            "compatibility",
            "created_by",
            "signature",
        ):
            assert compatibility_bundle.get(key) == bundle.get(key)

    identities: dict[str, str] = {}
    actual_files = []
    for path in sorted(item for item in stage.rglob("*") if item.is_file()):
        relative = path.relative_to(stage).as_posix()
        actual_files.append(relative)
        if relative != VALIDATION.as_posix():
            identities[relative] = digest_bytes(path.read_bytes())
    assert validation["tree_digest"] == digest_bytes(rfc8785.dumps(identities))
    assert validation["output_count"] == len(actual_files)
    assert not any(
        path.startswith(
            (
                "contracts/acceptance/",
                "contracts/fixtures/",
                "docs/upfront_design/",
                "tooling/model-transition/",
            )
        )
        for path in actual_files
    )
    print(
        f"validated aggregate DesiredTree: {len(actual_files)} outputs, "
        f"{len(released)} released artifacts, {len(requirements)} requirements, "
        f"{len(bundles)} bundles"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
