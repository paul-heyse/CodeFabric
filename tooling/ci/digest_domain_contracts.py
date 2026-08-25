"""Purpose-aware BLAKE3 and semantic fingerprint governance."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import yaml
from blake3 import blake3

from tooling.ci import artifact_contracts

ROOT = artifact_contracts.ROOT
REGISTRY_PATH = Path("contracts/identity/fingerprint-domain-registry.yaml")
GENERATED_PATH = Path("src/generated/model_identity_recipes.rs")

PURPOSE_TYPES = {
    "SEMANTIC_FINGERPRINT": "SemanticFingerprintDomain",
    "INTEGRITY": "IntegrityDomain",
    "CACHE_KEY": "CacheKeyDomain",
    "SECURITY_MAC": "SecurityMacDomain",
}

# Direct library calls are confined to narrow purpose authorities. The two
# rustc protocol paths are an explicit, plan-ordered WP56 cross-root projection
# seam; their domain literals must still be present in the registry now.
DIRECT_AUTHORITIES = {
    "src/identity.rs": "SEMANTIC_FINGERPRINT",
    "src/integrity.rs": "INTEGRITY_OR_CACHE_KEY",
    "src/security.rs": "SECURITY_MAC",
    "src/rustc_service.rs": "WP56_CROSS_ROOT_INTEGRITY",
    "rustc-extractor/src/main.rs": "CROSS_ROOT_INTEGRITY",
    "rustc-extractor/src/wrapper.rs": "WP56_CROSS_ROOT_MIXED",
    "pyrefly-sidecar/build.rs": "BUILD_INTEGRITY",
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/identity.py": (
        "PYTHON_CBEF_IDENTITY"
    ),
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/json.py": "JCS_INTEGRITY",
    "tooling/data_fabric_revision_benchmark.rs": "BENCHMARK_INTEGRITY",
    "tooling/model/adapter_driver.py": "MODEL_INTEGRITY",
    "tooling/model/proto_driver.py": "MODEL_INTEGRITY",
    "tooling/model/test_proto_driver.py": "TEST_ORACLE",
    "tooling/model/validate_aggregate.py": "MODEL_INTEGRITY",
}

SCAN_ROOTS = (
    Path("src"),
    Path("rustc-extractor"),
    Path("pyrefly-sidecar"),
    Path("codefabric-cpg-mcp/src"),
    Path("tooling"),
)

RAW_PATTERNS = (
    re.compile(r"blake3::(?:Hasher|hash|Hash)"),
    re.compile(r"\bnew_keyed\s*\("),
    re.compile(r"\bfrom\s+blake3\s+import\b"),
    re.compile(r"\bimport\s+blake3\b"),
    re.compile(r"\bblake3\s*\("),
    re.compile(r"\buse\s+blake3\b"),
    re.compile(r"\bextern\s+crate\s+blake3\b"),
    re.compile(r"\bblake3\s+as\s+\w+"),
)


class DigestDomainError(ValueError):
    """The hash-purpose census or API boundary has drifted."""


def _pascal(value: str) -> str:
    return "".join(part.title() for part in value.lower().split("_"))


def _registry(root: Path = ROOT) -> tuple[Mapping[str, Any], list[Mapping[str, Any]]]:
    path = root / REGISTRY_PATH
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(document, Mapping):
        raise DigestDomainError("fingerprint registry root must be a mapping")
    expected = {
        "artifact_id",
        "artifact_kind",
        "version",
        "compatible_suite_major",
        "status",
        "canonical_digest",
        "schema_version",
        "records",
        "owner_acceptance",
    }
    if (
        set(document) != expected
        or document.get("artifact_id")
        != "codefabric.identity.fingerprint-domain-registry"
        or document.get("schema_version") != 1
        or not isinstance(document.get("records"), list)
    ):
        raise DigestDomainError("fingerprint registry has an invalid envelope")
    detached = dict(document)
    declared = detached.pop("canonical_digest", None)
    canonical = json.dumps(
        detached,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    observed = f"b3:{blake3(canonical).hexdigest()}"
    if declared != observed:
        raise DigestDomainError(
            f"fingerprint registry canonical digest mismatch: {declared} != {observed}"
        )
    records = document["records"]
    ids: set[str] = set()
    domains: set[bytes] = set()
    for record in records:
        if not isinstance(record, Mapping):
            raise DigestDomainError("fingerprint record must be a mapping")
        domain_id = record.get("domain_id")
        purpose = record.get("purpose")
        domain_string = record.get("domain_string")
        separator = record.get("separator")
        if (
            not isinstance(domain_id, str)
            or domain_id in ids
            or purpose not in PURPOSE_TYPES
            or not isinstance(domain_string, str)
            or separator not in {"NUL", "NONE"}
        ):
            raise DigestDomainError(f"invalid or duplicate domain record {domain_id}")
        domain = domain_string.encode() + (b"\0" if separator == "NUL" else b"")
        if domain in domains:
            raise DigestDomainError(f"duplicate domain bytes for {domain_id}")
        ids.add(domain_id)
        domains.add(domain)
        consumers = record.get("consumers")
        if not isinstance(consumers, list) or not consumers:
            raise DigestDomainError(f"{domain_id} has no consumers")
        missing = [value for value in consumers if not (root / str(value)).is_file()]
        if missing:
            raise DigestDomainError(f"{domain_id} has missing consumers {missing}")
    return document, records


def _source_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for scan_root in SCAN_ROOTS:
        absolute = root / scan_root
        if absolute.is_file():
            files.append(absolute)
            continue
        if not absolute.exists():
            continue
        files.extend(
            path
            for path in absolute.rglob("*")
            if path.is_file()
            and path.suffix in {".rs", ".py"}
            and "target" not in path.parts
            and "generated" not in path.parts
            and "tests" not in path.parts
            and not path.name.startswith("test_")
            and path.name != "digest_domain_contracts.py"
        )
    build = root / "pyrefly-sidecar/build.rs"
    if build.is_file() and build not in files:
        files.append(build)
    return sorted(set(files))


def _raw_hash_paths(root: Path) -> set[str]:
    result: set[str] = set()
    for path in _source_files(root):
        source = path.read_text(encoding="utf-8")
        if any(pattern.search(source) for pattern in RAW_PATTERNS):
            result.add(path.relative_to(root).as_posix())
    return result


def _generated_variants(source: str, type_name: str) -> set[str]:
    match = re.search(rf"pub enum {type_name} \{{(.*?)\n\}}", source, re.DOTALL)
    if match is None:
        raise DigestDomainError(f"generated {type_name} is absent")
    return set(re.findall(r"^\s+(\w+),$", match.group(1), re.MULTILINE))


def _used_variants(root: Path, type_name: str) -> set[str]:
    pattern = re.compile(rf"\b{type_name}::(\w+)")
    result: set[str] = set()
    for path in _source_files(root):
        result.update(pattern.findall(path.read_text(encoding="utf-8")))
    return result


def _direct_domain_literals(root: Path) -> set[bytes]:
    result: set[bytes] = set()
    pattern = re.compile(r"(?:digest_frames\s*\(|\.update\s*\()b\"(codefabric[^\"]*)\"")
    for relative in DIRECT_AUTHORITIES:
        path = root / relative
        if not path.is_file() or path.suffix != ".rs":
            continue
        for literal in pattern.findall(path.read_text(encoding="utf-8")):
            result.add(
                bytes(literal, "utf-8").decode("unicode_escape").encode("latin1")
            )
    return result


def validate_source_text(path: str, source: str) -> None:
    """Reject direct hashing in an unapproved source, including renamed forms."""
    if path in DIRECT_AUTHORITIES:
        return
    matched = [pattern.pattern for pattern in RAW_PATTERNS if pattern.search(source)]
    if matched:
        raise DigestDomainError(f"{path} bypasses a purpose authority: {matched}")


def validate(root: Path = ROOT) -> tuple[int, int, int]:
    _, records = _registry(root)
    generated = (root / GENERATED_PATH).read_text(encoding="utf-8")
    registered_domains = {
        str(record["domain_string"]).encode()
        + (b"\0" if record["separator"] == "NUL" else b"")
        for record in records
    }
    registered_by_purpose = {
        purpose: {
            _pascal(str(record["domain_id"]))
            for record in records
            if record["purpose"] == purpose
        }
        for purpose in PURPOSE_TYPES
    }
    for purpose, type_name in PURPOSE_TYPES.items():
        generated_variants = _generated_variants(generated, type_name)
        if generated_variants != registered_by_purpose[purpose]:
            raise DigestDomainError(
                f"generated {type_name} differs from registry: "
                f"{sorted(generated_variants ^ registered_by_purpose[purpose])}"
            )
        unknown_uses = _used_variants(root, type_name) - generated_variants
        if unknown_uses:
            raise DigestDomainError(f"unknown {type_name} uses: {sorted(unknown_uses)}")

    raw_paths = _raw_hash_paths(root)
    unauthorized = raw_paths - DIRECT_AUTHORITIES.keys()
    stale_authorities = {
        path
        for path in DIRECT_AUTHORITIES.keys() - raw_paths
        if (root / path).is_file() and DIRECT_AUTHORITIES[path] != "TEST_ORACLE"
    }
    if unauthorized:
        raise DigestDomainError(
            f"unclassified direct hash paths: {sorted(unauthorized)}"
        )
    if stale_authorities:
        raise DigestDomainError(
            f"stale direct hash authorities: {sorted(stale_authorities)}"
        )
    for path in raw_paths:
        validate_source_text(path, (root / path).read_text(encoding="utf-8"))

    literal_domains = _direct_domain_literals(root)
    unregistered = literal_domains - registered_domains
    if unregistered:
        raise DigestDomainError(f"unregistered direct domains: {sorted(unregistered)}")

    forbidden_id_tokens = (
        "finalize_id16",
        "DerivedIdentity",
        "IdentityDomain",
        "encode_public_id",
        "copy_from_slice(&digest[..16]",
    )
    for relative in ("src/integrity.rs", "src/security.rs"):
        source = (root / relative).read_text(encoding="utf-8")
        if any(token in source for token in forbidden_id_tokens):
            raise DigestDomainError(f"{relative} can mint a semantic ID")
    return len(records), len(raw_paths), len(literal_domains)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check",))
    parser.add_argument("--root", type=Path, default=ROOT)
    arguments = parser.parse_args(argv)
    try:
        domains, authorities, literals = validate(arguments.root.resolve())
    except (DigestDomainError, OSError, ValueError) as error:
        print(f"digest-domain-contract-check: {error}", file=sys.stderr)
        return 1
    print(
        "digest-domain-contract-check: "
        f"{domains} purpose-classified domains; "
        f"{authorities} direct authority paths; {literals} direct domain literals"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
