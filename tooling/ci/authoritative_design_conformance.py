"""Validate the sole authoritative design suite and its generated identities."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import blake3

ROOT = Path(__file__).resolve().parents[2]
AUTHORITY_ROOT = Path("docs/authoritative_design")
LEGACY_ROOT = b"docs/" + b"upfront_design"
LEGACY_OWNER = b"upfront" + b"-design"
MAX_SOURCE_BYTES = 16 * 1024 * 1024


class AuthoritativeDesignError(ValueError):
    """The authoritative suite or one of its projections is inconsistent."""


@dataclass(frozen=True)
class MasterContract:
    """Expected identity and amendment anchor for one master document."""

    artifact_id: str
    amendment_anchor: str


MASTERS: dict[str, MasterContract] = {
    "codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md": (
        MasterContract(
            "codefabric-present-state-cpg-suite-manifest",
            "## Ontology-program, receipt, and decision authority amendment",
        )
    ),
    "code_property_graph_present_state_fact_ontology_specification_v1.3.md": (
        MasterContract(
            "codefabric-present-state-cpg-ontology",
            "## Compiled ontology-program projection",
        )
    ),
    "present_state_cpg_fact_generation_specification_python_rust_v1.3.md": (
        MasterContract(
            "codefabric-present-state-cpg-fact-generation",
            "## Provider boundary for compiled ontology programs",
        )
    ),
    "present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md": (
        MasterContract(
            "codefabric-present-state-cpg-data-fabric",
            "## Arrow/DataFusion ontology-program and activation authority",
        )
    ),
    "code_property_graph_semantic_query_specification_v1.3.md": MasterContract(
        "codefabric-composable-semantic-cpg-query",
        "## Sealed semantic planning and lease-scoped result authority",
    ),
    "codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md": (
        MasterContract(
            "codefabric-continuous-cpg-lifecycle",
            "## Durable candidate activation and serving-epoch lifecycle",
        )
    ),
    "present_state_cpg_fastmcp_serving_specification_v1.3.md": MasterContract(
        "codefabric-present-state-cpg-fastmcp-serving",
        "## Administrative activation and presentation-only serving boundary",
    ),
    "codefabric_1.3_implementation_roadmap_v1.0.md": MasterContract(
        "codefabric-implementation-roadmap",
        "## Ontology-compiled data-fabric transition sequence",
    ),
}

GENERATED_MANIFESTS = (
    Path("contracts/manifests/suite-manifest.json"),
    Path("contracts/generated/model/governance/suite-manifest.json"),
    Path(
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
    ),
)

HISTORICAL_PREFIXES = (
    "docs/designs/",
    "docs/plans/",
    "docs/reviews/",
    "docs/library_ref/",
    "tests/golden/",
)


def _digest(payload: bytes) -> str:
    return f"b3:{blake3.blake3(payload).hexdigest()}"


def _git_paths(root: Path) -> list[str]:
    completed = subprocess.run(
        ("git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return [item.decode("utf-8") for item in completed.stdout.split(b"\0") if item]


def _tracked_masters(root: Path) -> set[str]:
    completed = subprocess.run(
        ("git", "ls-files", "-z", f"{AUTHORITY_ROOT.as_posix()}/*.md"),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return {
        Path(item.decode("utf-8")).name
        for item in completed.stdout.split(b"\0")
        if item
    }


def validate_master_directory(directory: Path) -> dict[str, str]:
    """Validate exact membership, identity headers, and amendment anchors."""
    if not directory.is_dir():
        raise AuthoritativeDesignError(f"missing authoritative root: {directory}")
    actual = {item.name for item in directory.iterdir() if item.is_file()}
    expected = set(MASTERS)
    if actual != expected:
        raise AuthoritativeDesignError(
            "authoritative master census differs: "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )

    digests: dict[str, str] = {}
    for name, contract in MASTERS.items():
        master = directory / name
        payload = master.read_bytes()
        if len(payload) > MAX_SOURCE_BYTES:
            raise AuthoritativeDesignError(
                f"authoritative master exceeds limit: {master}"
            )
        try:
            text = payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise AuthoritativeDesignError(
                f"authoritative master is not UTF-8: {master}"
            ) from error
        if f"`{contract.artifact_id}`" not in text:
            raise AuthoritativeDesignError(f"artifact identity missing from {master}")
        if contract.amendment_anchor not in text:
            raise AuthoritativeDesignError(f"amendment anchor missing from {master}")
        if "version" not in text[:2_048].lower():
            raise AuthoritativeDesignError(f"version header missing from {master}")
        digests[name] = _digest(payload)
    return digests


def _validate_generated_manifest(
    root: Path, manifest_path: Path, digests: dict[str, str]
) -> None:
    try:
        document: dict[str, Any] = json.loads(
            (root / manifest_path).read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError) as error:
        raise AuthoritativeDesignError(
            f"invalid generated manifest: {manifest_path}"
        ) from error
    artifacts = document.get("artifacts")
    if not isinstance(artifacts, list):
        raise AuthoritativeDesignError(
            f"generated manifest has no artifact list: {manifest_path}"
        )

    expected_paths = {
        f"{AUTHORITY_ROOT.as_posix()}/{name}": (contract, digests[name])
        for name, contract in MASTERS.items()
    }
    selected = {
        item.get("authority_path"): item
        for item in artifacts
        if isinstance(item, dict)
        and isinstance(item.get("authority_path"), str)
        and item["authority_path"].startswith(f"{AUTHORITY_ROOT.as_posix()}/")
    }
    if set(selected) != set(expected_paths):
        raise AuthoritativeDesignError(
            f"generated authority census differs in {manifest_path}: "
            f"expected={sorted(expected_paths)}, actual={sorted(selected)}"
        )
    for authority_path, (contract, digest) in expected_paths.items():
        item = selected[authority_path]
        if item.get("artifact_id") != contract.artifact_id:
            raise AuthoritativeDesignError(
                f"artifact ID drift in {manifest_path}: {authority_path}"
            )
        if item.get("owner") != "authoritative-design":
            raise AuthoritativeDesignError(
                f"owner drift in {manifest_path}: {authority_path}"
            )
        if item.get("source_role") != "authority":
            raise AuthoritativeDesignError(
                f"source role drift in {manifest_path}: {authority_path}"
            )
        if (
            item.get("source_digest") != digest
            or item.get("canonical_digest") != digest
        ):
            raise AuthoritativeDesignError(
                f"master digest drift in {manifest_path}: {authority_path}"
            )


def _legacy_hits(root: Path) -> tuple[list[str], list[str]]:
    live: list[str] = []
    historical: list[str] = []
    for relative in _git_paths(root):
        if relative.startswith((".git/", "target/")):
            continue
        candidate = root / relative
        if not candidate.is_file() or candidate.stat().st_size > MAX_SOURCE_BYTES:
            continue
        payload = candidate.read_bytes()
        if LEGACY_ROOT not in payload and LEGACY_OWNER not in payload:
            continue
        if relative.startswith(HISTORICAL_PREFIXES):
            historical.append(relative)
        else:
            live.append(relative)
    return live, historical


def validate_authoritative_design(root: Path = ROOT) -> dict[str, Any]:
    """Validate master census, generated identities, navigation, and path authority."""
    digests = validate_master_directory(root / AUTHORITY_ROOT)
    tracked = _tracked_masters(root)
    if tracked != set(MASTERS):
        raise AuthoritativeDesignError(
            f"tracked master census differs: expected={sorted(MASTERS)}, actual={sorted(tracked)}"
        )
    for manifest in GENERATED_MANIFESTS:
        _validate_generated_manifest(root, manifest, digests)

    outlined = subprocess.run(
        ("just", "spec-outline"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if outlined.returncode != 0:
        raise AuthoritativeDesignError("default spec-outline failed")
    missing_outlines = [name for name in MASTERS if name not in outlined.stdout]
    if missing_outlines:
        raise AuthoritativeDesignError(
            f"spec-outline omitted masters: {missing_outlines}"
        )

    live_hits, historical_hits = _legacy_hits(root)
    if live_hits:
        raise AuthoritativeDesignError(f"legacy live authority remains: {live_hits}")
    return {
        "master_count": len(MASTERS),
        "generated_manifest_count": len(GENERATED_MANIFESTS),
        "historical_exclusion_count": len(historical_hits),
        "authority_root": AUTHORITY_ROOT.as_posix(),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        report = validate_authoritative_design(args.root.resolve())
    except (AuthoritativeDesignError, OSError, subprocess.CalledProcessError) as error:
        print(f"authoritative design conformance error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
