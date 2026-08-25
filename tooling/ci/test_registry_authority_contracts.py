"""Cross-language acceptance for the single generated registry authority."""

from __future__ import annotations

import json
import re
from pathlib import Path

import yaml
from codefabric_cpg_mcp.contracts import model_registries as python_registries

ROOT = Path(__file__).resolve().parents[2]


def _rust_triples(domain: str) -> set[tuple[int, str, str]]:
    source = (ROOT / "src/generated/registries.rs").read_text(encoding="utf-8")
    section = re.search(
        rf"pub const {re.escape(domain)}_VALUES: &\[RegistryEntry\] = &\[(.*?)\n\];",
        source,
        re.DOTALL,
    )
    assert section is not None, domain
    return {
        (int(code), name, slug)
        for code, name, slug in re.findall(
            r'RegistryEntry \{\s*code: (\d+),\s*name: "([^"]+)",\s*slug: "([^"]+)",\s*\}',
            section.group(1),
            re.DOTALL,
        )
    }


def test_wp56_behavioral_acceptance() -> None:
    vectors = json.loads(
        (ROOT / "contracts/fixtures/registries/enum-flag-v1-vectors.json").read_text(
            encoding="utf-8"
        )
    )
    for vector in vectors["enum_triples"]:
        triple = tuple(vector[key] for key in ("code", "name", "slug"))
        domain = vector["domain"]
        assert triple in python_registries.ENUM_TRIPLES[domain]
        assert triple in _rust_triples(domain)

    cbef = yaml.safe_load((ROOT / "contracts/identity/cbef-v1.yaml").read_text())
    authority_domains = {record["name"]: record["code"] for record in cbef["domains"]}
    assert {
        domain.name: domain.value for domain in python_registries.IdentityDomain
    } == authority_domains
    rust_identity = (ROOT / "src/identity.rs").read_text(encoding="utf-8")
    assert "RootAuthorization = 17" in rust_identity
