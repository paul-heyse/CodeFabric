"""Prove that every public Rust error prefix belongs to the error registry."""

from __future__ import annotations

import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
ERROR_REGISTRY = Path("contracts/registry/error-registry.yaml")
ERROR_PREFIX = re.compile(r'#\[error\("([A-Z][A-Z0-9_]+):')


def check(root: Path = ROOT) -> dict[str, object]:
    registry = yaml.safe_load((root / ERROR_REGISTRY).read_text(encoding="utf-8"))
    registered = {record["name"] for record in registry["records"]}
    observations: dict[str, list[str]] = {}
    for source_root in (root / "src", root / "rustc-extractor" / "src"):
        for path in sorted(source_root.rglob("*.rs")):
            relative = path.relative_to(root).as_posix()
            for code in ERROR_PREFIX.findall(path.read_text(encoding="utf-8")):
                observations.setdefault(code, []).append(relative)
    missing = sorted(set(observations) - registered)
    if missing:
        details = ", ".join(
            f"{code} ({', '.join(sorted(set(observations[code])))})" for code in missing
        )
        raise ValueError(f"public error prefixes absent from registry: {details}")
    return {
        "observed_public_error_count": len(observations),
        "registered_public_error_count": len(registered),
        "unregistered_public_errors": missing,
    }


if __name__ == "__main__":
    print(json.dumps(check(), indent=2, sort_keys=True))
