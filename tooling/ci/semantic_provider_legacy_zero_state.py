"""Validate governed DB01-DB03 semantic-provider legacy candidates and allows."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any

import yaml
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = Path("contracts/governance/semantic-provider-legacy-candidates.yaml")
SCOPES = {"all", "python", "rust"}


class SemanticProviderLegacyError(ValueError):
    """The governed semantic-provider legacy surface is incomplete or stale."""


def _detached_digest(document: dict[str, Any]) -> str:
    detached = dict(document)
    detached.pop("canonical_digest", None)
    return checksum(canonicalize_value(detached))


def _regex_paths(root: Path, probe: dict[str, Any]) -> set[str]:
    command = [
        "rg",
        "--json",
        "--hidden",
        "-g",
        "!.git/**",
        "-g",
        "!docs/library_ref/**",
        "-g",
        "!**/generated/**",
        "-e",
        str(probe["pattern"]),
        "--",
        *map(str, probe["paths"]),
    ]
    completed = subprocess.run(
        command, cwd=root, check=False, capture_output=True, text=True
    )
    if completed.returncode not in {0, 1}:
        raise SemanticProviderLegacyError(
            f"rg candidate probe failed: {completed.stderr.strip()}"
        )
    observed: set[str] = set()
    for line in completed.stdout.splitlines():
        event = json.loads(line)
        if event.get("type") == "match":
            observed.add(event["data"]["path"]["text"])
    return observed


def _observed_paths(root: Path, probe: dict[str, Any]) -> set[str]:
    kind = probe.get("kind")
    paths = probe.get("paths")
    if (
        not isinstance(paths, list)
        or not paths
        or any(not isinstance(path, str) for path in paths)
    ):
        raise SemanticProviderLegacyError("candidate probe paths are absent or invalid")
    if kind == "path_exists":
        return {path for path in paths if (root / path).exists()}
    if kind == "regex" and isinstance(probe.get("pattern"), str):
        return _regex_paths(root, probe)
    raise SemanticProviderLegacyError(f"unsupported candidate probe kind {kind!r}")


def check(scope: str = "all", root: Path = ROOT) -> dict[str, object]:
    if scope not in SCOPES:
        raise SemanticProviderLegacyError(f"unsupported scope {scope!r}")
    document = yaml.safe_load((root / REGISTRY).read_text(encoding="utf-8"))
    if not isinstance(document, dict) or document.get(
        "canonical_digest"
    ) != _detached_digest(document):
        raise SemanticProviderLegacyError(
            "semantic-provider legacy registry digest is stale"
        )
    candidates = document.get("candidates")
    allows = document.get("allows")
    if not isinstance(candidates, list) or not isinstance(allows, list):
        raise SemanticProviderLegacyError(
            "candidate and allow registries must be lists"
        )
    candidate_by_id: dict[str, dict[str, Any]] = {}
    for candidate in candidates:
        candidate_id = candidate.get("candidate_id")
        if (
            not isinstance(candidate_id, str)
            or candidate_id in candidate_by_id
            or candidate.get("batch") not in {"DB01", "DB02", "DB03"}
            or candidate.get("scope") not in SCOPES
            or not isinstance(candidate.get("zero_state_after"), str)
            or not isinstance(candidate.get("probe"), dict)
        ):
            raise SemanticProviderLegacyError(
                f"invalid candidate record {candidate_id!r}"
            )
        candidate_by_id[candidate_id] = candidate

    required_allow_fields = {
        "allow_id",
        "candidate_id",
        "path",
        "scope",
        "rationale",
        "owner",
        "expiry_packet",
        "replacement",
    }
    allow_ids: set[str] = set()
    allow_paths: dict[str, set[str]] = {
        candidate_id: set() for candidate_id in candidate_by_id
    }
    active_state = json.loads(
        (root / str(document["active_state_path"])).read_text(encoding="utf-8")
    )
    packets = active_state.get("packets", {})
    for allow in allows:
        if not isinstance(allow, dict) or not required_allow_fields <= allow.keys():
            raise SemanticProviderLegacyError(
                "legacy allow record is structurally incomplete"
            )
        allow_id = allow["allow_id"]
        candidate_id = allow["candidate_id"]
        if (
            not isinstance(allow_id, str)
            or allow_id in allow_ids
            or candidate_id not in candidate_by_id
            or allow["scope"] != candidate_by_id[candidate_id]["scope"]
            or any(
                not isinstance(allow[field], str) or not allow[field].strip()
                for field in required_allow_fields
            )
        ):
            raise SemanticProviderLegacyError(f"invalid legacy allow {allow_id!r}")
        allow_ids.add(allow_id)
        path = allow["path"]
        if path in allow_paths[candidate_id]:
            raise SemanticProviderLegacyError(
                f"duplicate reviewed allow for {candidate_id}: {path}"
            )
        allow_paths[candidate_id].add(path)
        expiry = allow["expiry_packet"]
        if expiry != "NEVER" and (
            expiry not in packets or packets[expiry].get("status") == "complete"
        ):
            raise SemanticProviderLegacyError(
                f"stale or invalid allow {allow_id}: expiry packet {expiry}"
            )

    selected = [
        candidate
        for candidate in candidates
        if scope == "all" or candidate["scope"] in {scope, "all"}
    ]
    open_candidates: list[dict[str, object]] = []
    for candidate in selected:
        candidate_id = candidate["candidate_id"]
        observed = _observed_paths(root, candidate["probe"])
        allowed = allow_paths[candidate_id]
        unexpected = observed - allowed
        stale = allowed - observed
        if unexpected:
            raise SemanticProviderLegacyError(
                f"{candidate_id} has unexpected candidates: {', '.join(sorted(unexpected))}"
            )
        if stale:
            raise SemanticProviderLegacyError(
                f"{candidate_id} has stale reviewed allows: {', '.join(sorted(stale))}"
            )
        if observed:
            open_candidates.append(
                {
                    "candidate_id": candidate_id,
                    "paths": sorted(observed),
                    "zero_state_after": candidate["zero_state_after"],
                }
            )
    return {
        "scope": scope,
        "candidate_count": len(selected),
        "open_candidate_count": len(open_candidates),
        "open_candidates": open_candidates,
        "registry_digest": document["canonical_digest"],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("scope", nargs="?", default="all", choices=sorted(SCOPES))
    arguments = parser.parse_args()
    print(json.dumps(check(arguments.scope), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
