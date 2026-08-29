from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

LIVE_ROOTS = (
    "Cargo.toml",
    "src",
    "tests",
    "scripts",
    "tooling",
    "contracts",
    "codefabric-cpg-mcp",
    "justfile",
    ".github",
    "rules",
    "rule-tests",
    "docs/authoritative_design",
)
HISTORICAL_EXCLUSIONS = (
    "docs/plans",
    "docs/reviews",
    "docs/designs",
    "docs/library_ref",
    "tests/golden",
)
SELF_EXCLUSIONS = {
    "tooling/ci/ontology_datafabric_legacy_zero_state.py",
}


def _forbidden_patterns() -> tuple[re.Pattern[str], ...]:
    tokens = (
        "CompiledRule" + "OperationKind",
        "RuntimeCompiled" + "Ontology",
        "validate_compiled_" + "ontology_rules",
        "activate_" + "stage2b",
        "OntologyCandidate" + "Dossier",
        "OntologyActivation" + "State",
        "RESULT_CHECKSUM_" + "VERSION",
        "id16-extension-" + "contract-check",
        "ontology_fabric_" + "probe_suite",
        "probe-" + "suite",
        "SEMANTIC_OPERATION_" + "SPECS",
        "SemanticOperation" + "Spec",
        "NativeValidation" + "Operation",
        "DecodedProgram" + "Operation",
        "LoweredValidation" + "Operation",
        "program.rule_" + "operation",
        "program.phrase_" + "operation",
        "program.calculation_" + "catalog",
    )
    return tuple(
        re.compile(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])")
        for token in tokens
    )


def _files(roots: tuple[str, ...], *, exclude_library: bool = True) -> list[Path]:
    existing = [root for root in roots if (ROOT / root).exists()]
    globs = ["-g", "!.git/**", "-g", "!target/**"]
    if exclude_library:
        for excluded_root in HISTORICAL_EXCLUSIONS:
            globs.extend(["-g", f"!{excluded_root}/**"])
    result = subprocess.run(
        [
            "rg",
            "--files",
            "--hidden",
            *globs,
            *existing,
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return sorted(ROOT / line for line in result.stdout.splitlines() if line)


def _scan_text(files: list[Path]) -> tuple[list[dict[str, object]], list[str]]:
    findings: list[dict[str, object]] = []
    skipped: list[str] = []
    patterns = _forbidden_patterns()
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        if relative in SELF_EXCLUSIONS:
            skipped.append(f"self-describing-checker:{relative}")
            continue
        if relative.startswith("rules/"):
            skipped.append(f"self-describing-governance-rule:{relative}")
            continue
        if relative.startswith("rule-tests/"):
            skipped.append(f"tested-governance-fixture:{relative}")
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            skipped.append(f"non-utf8-or-unreadable:{relative}")
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            for pattern in patterns:
                if pattern.search(line):
                    findings.append(
                        {
                            "path": relative,
                            "line": line_number,
                            "pattern": pattern.pattern,
                        }
                    )
    return findings, skipped


def _historical_census() -> dict[str, int]:
    return {
        root: len(_files((root,), exclude_library=False))
        if (ROOT / root).exists()
        else 0
        for root in HISTORICAL_EXCLUSIONS
    }


def _command_contract() -> tuple[list[str], list[str]]:
    result = subprocess.run(
        ["just", "--list", "--unsorted"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    output = result.stdout
    retired = [
        "id16-extension-" + "contract-check",
        "probe-" + "suite",
        "ontology-stage2b-" + "activation-check",
    ]
    required = [
        "id-domain-extension-check",
        "ontology-datafabric-legacy-zero-state-check",
    ]
    present_retired = [
        name for name in retired if re.search(rf"(?m)^\s*{re.escape(name)}\b", output)
    ]
    missing_required = [
        name
        for name in required
        if not re.search(rf"(?m)^\s*{re.escape(name)}\b", output)
    ]
    return present_retired, missing_required


def _structural_scan() -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "ast-grep",
            "scan",
            "--filter",
            r"^(?:ontology|semantic-(?:phrase|code))",
            "--inspect",
            "summary",
            "--globs",
            "!contracts/generated/**",
            "--globs",
            "!src/generated/**",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def _semantic_authority_findings() -> list[dict[str, object]]:
    findings: list[dict[str, object]] = []
    raw_commit = ".commit_" + "fact_snapshot("
    for path in sorted((ROOT / "src").rglob("*.rs")):
        relative = path.relative_to(ROOT).as_posix()
        if relative == "src/snapshot_runtime.rs":
            continue
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1
        ):
            if raw_commit in line:
                findings.append(
                    {
                        "path": relative,
                        "line": line_number,
                        "authority": "raw-serving-pointer-call-outside-private-kernel",
                    }
                )

    for relative in (
        "src/domain_conformance.rs",
        "src/fabric/serving.rs",
    ):
        text = (ROOT / relative).read_text(encoding="utf-8")
        if "apply_with_" + "subqueries" not in text:
            findings.append(
                {
                    "path": relative,
                    "line": 0,
                    "authority": "subquery-aware-governance-traversal-absent",
                }
            )

    compiler_root = ROOT / "src" / "bin" / "codefabric_model" / "schema_driver"
    hard_coded_rule = re.compile(r'"ontology\.[a-z0-9_]+"')
    for path in sorted(compiler_root.rglob("*.rs")):
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1
        ):
            if hard_coded_rule.search(line):
                findings.append(
                    {
                        "path": path.relative_to(ROOT).as_posix(),
                        "line": line_number,
                        "authority": "hard-coded-ontology-rule-dispatch",
                    }
                )
    governed_session = (ROOT / "src" / "governed_session.rs").read_text(
        encoding="utf-8"
    )
    public_sealer = re.compile(r"\bpub\s+(?:async\s+)?fn\s+(?:seal_plan|seal_sql)\b")
    for match in public_sealer.finditer(governed_session):
        findings.append(
            {
                "path": "src/governed_session.rs",
                "line": governed_session.count("\n", 0, match.start()) + 1,
                "authority": "public-governed-plan-sealer",
            }
        )
    return findings


def main() -> int:
    candidates = _files(LIVE_ROOTS)
    findings, skipped = _scan_text(candidates)
    retired, missing = _command_contract()
    structural = _structural_scan()
    authority_findings = _semantic_authority_findings()
    report = {
        "oracle": "ontology_datafabric_legacy_zero_state",
        "live_roots": list(LIVE_ROOTS),
        "candidate_file_count": len(candidates),
        "candidate_files": [path.relative_to(ROOT).as_posix() for path in candidates],
        "skipped_candidates": skipped,
        "historical_exclusions": _historical_census(),
        "historical_policy": "immutable plans, reviews, designs, library references, and accepted golden evidence are reported but not live runtime authority",
        "text_findings": findings,
        "retired_commands_present": retired,
        "required_commands_missing": missing,
        "structural_scan_returncode": structural.returncode,
        "structural_scan_stdout": structural.stdout,
        "structural_scan_stderr": structural.stderr,
        "semantic_authority_findings": authority_findings,
        "obsolete_master_root_present": (
            ROOT / "docs" / ("upfront_" + "design")
        ).exists(),
    }
    print(json.dumps(report, sort_keys=True))
    if (
        findings
        or retired
        or missing
        or structural.returncode != 0
        or authority_findings
        or report["obsolete_master_root_present"]
    ):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
