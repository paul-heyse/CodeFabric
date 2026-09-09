"""Small local-link, navigation and tracked-output checks, with no plan state."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

from tooling.ci.authoritative_design_conformance import ROOT, selected_documents


def local_link_errors(path: Path) -> list[str]:
    # Ignore code examples; validate Markdown destinations relative to their document.
    text = re.sub(r"```.*?```", "", path.read_text(), flags=re.DOTALL)
    errors = []
    for target in re.findall(r"\]\(([^)]+)\)", text):
        target = target.strip().split(' "', 1)[0].strip("<>")
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc or not parsed.path:
            continue
        destination = (path.parent / unquote(parsed.path)).resolve()
        if not destination.exists():
            errors.append(f"{path}: missing local link {target}")
    return errors


def tracked_output_errors(root: Path = ROOT) -> list[str]:
    paths = (
        subprocess.check_output(["git", "ls-files", "-z"], cwd=root)
        .decode()
        .split("\0")
    )
    return [
        f"tracked generated output: {path}"
        for path in paths
        if "target" in Path(path).parts or ".venv" in Path(path).parts
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    selected = selected_documents()
    paths = args.paths or [
        ROOT / "AGENTS.md",
        ROOT / "STATUS.md",
        ROOT / "README.md",
        *selected.values(),
        *(ROOT / "docs/spec_index").glob("*.md"),
    ]
    errors = tracked_output_errors()
    for path in paths:
        if not path.is_file():
            errors.append(f"missing document: {path}")
        else:
            errors.extend(local_link_errors(path))
    for error in errors:
        print(error)
    print(
        f"document navigation: {len(paths)} files, {len(errors)} errors (not product certification)"
    )
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())
