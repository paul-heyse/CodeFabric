"""Emit review-only KAT candidates without mutating normative fixture authority."""

import argparse
import json
from pathlib import Path

from codefabric_cpg_mcp.contracts.json import canonicalize_json, checksum

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
NORMATIVE_FIXTURE_ROOT = (REPOSITORY_ROOT / "contracts/fixtures").resolve()


def _isolated_output_directory(value: str) -> Path:
    output = Path(value).resolve()
    if output == NORMATIVE_FIXTURE_ROOT or output.is_relative_to(
        NORMATIVE_FIXTURE_ROOT
    ):
        raise ValueError(
            "candidate output must be outside normative contracts/fixtures"
        )
    if output.exists() and any(output.iterdir()):
        raise ValueError("candidate output directory must be absent or empty")
    return output


def emit_candidates(output_directory: Path) -> tuple[Path, Path]:
    """Write derived candidate answers into one empty review directory."""

    output_directory.mkdir(parents=True, exist_ok=True)
    jcs = json.loads(
        (NORMATIVE_FIXTURE_ROOT / "jcs/vectors.json").read_text(encoding="utf-8")
    )
    jcs_candidates = [
        {
            "id": vector["id"],
            "canonical_utf8": canonicalize_json(vector["input_json"]).decode(),
            "checksum": checksum(canonicalize_json(vector["input_json"])),
        }
        for vector in jcs["positive"]
    ]
    jcs_path = output_directory / "jcs-candidates.json"
    jcs_path.write_text(
        json.dumps({"candidate_only": True, "vectors": jcs_candidates}, indent=2)
        + "\n",
        encoding="utf-8",
    )

    projections = json.loads(
        (NORMATIVE_FIXTURE_ROOT / "projections/vectors.json").read_text(
            encoding="utf-8"
        )
    )
    projection_candidates = [
        {
            "id": vector["id"],
            "source_digest": checksum(vector["source_utf8"].encode()),
            "canonical_digest": checksum(vector["canonical_utf8"].encode()),
            **(
                {"bundle_digest": checksum(vector["bundle_identity_utf8"].encode())}
                if "bundle_identity_utf8" in vector
                else {}
            ),
        }
        for vector in projections["vectors"]
    ]
    projection_path = output_directory / "projection-candidates.json"
    projection_path.write_text(
        json.dumps({"candidate_only": True, "vectors": projection_candidates}, indent=2)
        + "\n",
        encoding="utf-8",
    )
    return jcs_path, projection_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", required=True)
    arguments = parser.parse_args()
    output = _isolated_output_directory(arguments.output_dir)
    for path in emit_candidates(output):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
