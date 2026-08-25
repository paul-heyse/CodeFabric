"""Mutation-oriented tests for the hash-purpose governance boundary."""

from __future__ import annotations

import subprocess

import pytest

from tooling.ci import digest_domain_contracts as contracts


def test_wp55_structural_acceptance() -> None:
    domains, authorities, literals = contracts.validate()
    assert domains >= 50
    assert authorities >= 10
    assert literals >= 5


@pytest.mark.parametrize(
    "source",
    [
        "fn x() { let _ = blake3::Hasher::new(); }",
        'fn x() { let _ = blake3::hash(b"x"); }',
        "fn x(k: &[u8; 32]) { let _ = blake3::Hasher::new_keyed(k); }",
        "use blake3::Hasher as SemanticHasher; fn x() { let _ = SemanticHasher::new(); }",
        "from blake3 import blake3 as semantic_hash\nsemantic_hash(b'x')",
    ],
)
def test_wp55_negative_zero_state(source: str) -> None:
    with pytest.raises(contracts.DigestDomainError, match="bypasses"):
        contracts.validate_source_text("src/not_an_authority.rs", source)


def test_wp55_integrity_and_security_cannot_mint_ids() -> None:
    for path in ("src/integrity.rs", "src/security.rs"):
        source = (contracts.ROOT / path).read_text(encoding="utf-8")
        assert "finalize_id16" not in source
        assert "DerivedIdentity" not in source
        assert "encode_public_id" not in source


def test_wp55_operational_acceptance() -> None:
    result = subprocess.run(
        ("just", "model-plan"),
        cwd=contracts.ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "replacement" not in result.stdout.lower(), result.stdout
    assert (contracts.ROOT / contracts.REGISTRY_PATH).is_file()
    assert (contracts.ROOT / contracts.GENERATED_PATH).is_file()
