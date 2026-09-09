"""Check current design navigation; this does not certify product behavior."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = Path(
    "docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md"
)
REQUIRED_TAGS = frozenset({"SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"})


class AuthoritativeDesignError(ValueError):
    """A selected document is missing or ambiguous."""


def selected_documents(root: Path = ROOT) -> dict[str, Path]:
    governance = root / GOVERNANCE
    rows = re.findall(
        r"^\| ([A-Z]+) \| \[[^\]]+\]\(([^)]+)\) \|$",
        governance.read_text(),
        re.MULTILINE,
    )
    result = {}
    for tag, target in rows:
        if tag in result or tag not in REQUIRED_TAGS:
            raise AuthoritativeDesignError(f"duplicate or unknown role: {tag}")
        path = (governance.parent / target).resolve()
        if (
            not path.is_relative_to((root / "docs/authoritative_design").resolve())
            or not path.is_file()
        ):
            raise AuthoritativeDesignError(f"invalid selected path: {target}")
        text = path.read_text()
        metadata = text.split("---", 2)
        if (
            len(metadata) != 3
            or metadata[0].strip()
            or not re.search(rf"^artifact_tag: {tag}$", metadata[1], re.MULTILINE)
        ):
            raise AuthoritativeDesignError(f"malformed role metadata: {target}")
        result[tag] = path
    if result.keys() != REQUIRED_TAGS or len(set(result.values())) != len(
        REQUIRED_TAGS
    ):
        raise AuthoritativeDesignError(
            "current selection must contain one document per role"
        )
    if result["SUITE"] != governance.resolve():
        raise AuthoritativeDesignError("SUITE must select this governance document")
    return result


def validate_authoritative_design(root: Path = ROOT) -> dict:
    selected = selected_documents(root)
    return {
        "kind": "navigation-only",
        "documents": {
            tag: str(path.relative_to(root.resolve())) for tag, path in selected.items()
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--paths", action="store_true")
    args = parser.parse_args()
    try:
        result = selected_documents()
    except (ValueError, OSError) as error:
        parser.exit(1, f"design navigation: {error}\n")
    if args.paths:
        print("\n".join(str(path) for path in result.values()))
    else:
        print(json.dumps(validate_authoritative_design(), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
