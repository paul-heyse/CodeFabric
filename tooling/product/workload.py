"""Create repeatable mixed-language source workloads without changing the product."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def create(destination: Path, modules: int):
    if not 1 <= modules <= 10000:
        raise ValueError("modules must be between 1 and 10000")
    if destination.exists():
        raise ValueError("destination must be new; existing work is never overwritten")
    shutil.copytree(ROOT / "tests/fixtures/pragmatic_cpg/workspace", destination)
    rust_root = destination / "src/lib.rs"
    with rust_root.open("a") as index:
        for number in range(modules):
            name = f"generated_{number}"
            (destination / f"{name}.py").write_text(
                f"from helpers import twice\n\ndef {name}(value: int) -> int:\n    return twice(value) + {number}\n"
            )
            (destination / "src" / f"{name}.rs").write_text(
                f"pub fn {name}(value: i32) -> i32 {{\n    super::increment(value) + {number}\n}}\n"
            )
            index.write(f"\npub mod {name};\n")
    sources = sorted([*destination.glob("*.py"), *destination.glob("src/*.rs")])
    result = {
        "kind": "mixed-language-source-workload",
        "modules_per_language": modules,
        "source_files": len(sources),
        "source_bytes": sum(path.stat().st_size for path in sources),
        "expected_relationships": "Each generated Python function calls helpers.twice; each generated Rust function calls crate::increment.",
        "runtime_observation": None,
    }
    (destination / "workload.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--modules", type=int, default=10)
    args = parser.parse_args()
    try:
        result = create(args.output, args.modules)
    except (OSError, ValueError) as error:
        parser.exit(1, f"workload: {error}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
