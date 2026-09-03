"""Read-only WP64 deployment census and dormant-authority zero-state proof.

The transition has no assumed predecessor.  This module proves that claim from the supported
local-workstation deployment surfaces before FreshActivation deletes the generic handoff shell.
It never mutates a service, process, runtime root, or durable workspace.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAX_CENSUS_FILE_BYTES = 1_048_576

# Construct displaced names so the assurance implementation is not its own positive finding.
DORMANT_AUTHORITY_TOKENS: Mapping[str, str] = {
    "switchable_module": "switchable_activation_" + "authority",
    "switchable_type": "SwitchableActivation" + "Authority",
    "generic_handoff_type": "Authority" + "Handoff",
    "old_cutover_fixture": "wp" + "41_prod_",
    "old_cutover_unit_fixture": "wp" + "41_",
}
DEPLOYED_PREDECESSOR_TOKENS: Mapping[str, str] = {
    "v1_wire": "codefabric.cpgd." + "v1",
    "v5_release": "relational-fabric-" + "v5",
    "v5_error_namespace": "RF" + "V5_",
    "old_suite": 'suite_version = "2.' + '2.0"',
    "global_release_lookup": "CompiledSemanticRelease::" + "current",
}
LIVE_ZERO_STATE_ROOTS = (
    Path("src"),
    Path("tests"),
    Path("scripts"),
    Path("tooling/ci"),
    Path(".github"),
    Path("justfile"),
)
SELF_PATHS = {
    Path("tooling/ci/fresh_activation_assurance.py"),
    Path("tooling/ci/test_fresh_activation_assurance.py"),
}
PRODUCT_PROCESS_NAMES = frozenset({"codefabric", "codefabricd", "codefabric-cpg-mcp"})
PRODUCT_PROCESS_PREFIXES = ("codefabric-cpg-",)
PRODUCT_MODULE_NAMES = frozenset({"codefabric_cpg_mcp", "codefabric_cpg_mcp.server"})
SERVICE_FILE_SUFFIXES = frozenset({".plist", ".service", ".socket"})
SERVICE_CONTENT_TOKENS = ("codefabricd", "codefabric-cpg-mcp")
TEXT_SUFFIXES = frozenset({".json", ".py", ".rs", ".sh", ".toml", ".yaml", ".yml"})


class FreshActivationAssuranceError(ValueError):
    """Fail-closed deployment-census or dormant-authority finding."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _iter_files(root: Path, relative_roots: Iterable[Path]) -> list[Path]:
    files: list[Path] = []
    for relative in relative_roots:
        candidate = root / relative
        if (candidate.is_file() or candidate.is_symlink()) and (
            candidate.name == "justfile" or candidate.suffix in TEXT_SUFFIXES
        ):
            files.append(relative)
        elif candidate.is_dir():
            files.extend(
                path.relative_to(root)
                for path in candidate.rglob("*")
                if (path.is_file() or path.is_symlink())
                and "__pycache__" not in path.parts
                and path.suffix in TEXT_SUFFIXES
            )
    return sorted(set(files))


def _bounded_text(path: Path) -> str:
    try:
        metadata = path.stat()
        if not path.is_file() or metadata.st_size > MAX_CENSUS_FILE_BYTES:
            raise FreshActivationAssuranceError(
                "CFV7_FRESH_CENSUS_FILE_INVALID", str(path)
            )
        return path.read_text(encoding="utf-8")
    except FreshActivationAssuranceError:
        raise
    except (OSError, UnicodeError) as error:
        raise FreshActivationAssuranceError(
            "CFV7_FRESH_CENSUS_FILE_INVALID", str(path)
        ) from error


def validate_dormant_authority_zero_state(root: Path = ROOT) -> Mapping[str, int]:
    """Reject the generic authority swapper and its deleted predecessor-era fixtures."""

    files = _iter_files(root, LIVE_ZERO_STATE_ROOTS)
    if not files:
        raise FreshActivationAssuranceError(
            "CFV7_FRESH_ZERO_SELECTION", "no live files selected"
        )
    findings: list[str] = []
    scanned = 0
    for relative in files:
        if relative in SELF_PATHS:
            continue
        text = _bounded_text(root / relative)
        scanned += 1
        for category, token in DORMANT_AUTHORITY_TOKENS.items():
            if token in text:
                findings.append(f"{category}:{relative.as_posix()}")
    if findings:
        raise FreshActivationAssuranceError(
            "CFV7_DORMANT_AUTHORITY_REACHABLE", ", ".join(findings)
        )
    return {"scanned_live_files": scanned, "dormant_findings": 0}


def _default_service_roots(home: Path) -> tuple[Path, ...]:
    xdg_config = Path(os.environ.get("XDG_CONFIG_HOME", home / ".config"))
    return (
        xdg_config / "systemd/user",
        Path("/etc/systemd/system"),
        Path("/etc/systemd/user"),
        Path("/usr/lib/systemd/system"),
        home / "Library/LaunchAgents",
        Path("/Library/LaunchAgents"),
        Path("/Library/LaunchDaemons"),
    )


def _default_product_roots(home: Path) -> tuple[Path, ...]:
    xdg_state = Path(os.environ.get("XDG_STATE_HOME", home / ".local/state"))
    xdg_config = Path(os.environ.get("XDG_CONFIG_HOME", home / ".config"))
    runtime = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
    return (xdg_state / "codefabric", xdg_config / "codefabric", runtime / "codefabric")


def _product_state_entries(roots: Iterable[Path]) -> list[str]:
    entries: list[str] = []
    for root in roots:
        if not root.exists():
            continue
        try:
            for child in root.iterdir():
                # The repository sccache service shares the vendor namespace but is build
                # infrastructure, not a deployed product predecessor.
                if child.name == "sccache":
                    continue
                entries.append(str(child))
        except OSError as error:
            raise FreshActivationAssuranceError(
                "CFV7_DEPLOYMENT_CENSUS_UNREADABLE", str(root)
            ) from error
    return sorted(entries)


def _service_candidates(roots: Iterable[Path]) -> list[str]:
    candidates: list[str] = []
    for root in roots:
        if not root.is_dir():
            continue
        try:
            paths = sorted(path for path in root.rglob("*") if path.is_file())
        except OSError as error:
            raise FreshActivationAssuranceError(
                "CFV7_DEPLOYMENT_CENSUS_UNREADABLE", str(root)
            ) from error
        for path in paths:
            name = path.name.lower()
            if path.suffix.lower() not in SERVICE_FILE_SUFFIXES:
                continue
            if "sccache" in name:
                continue
            named_for_product = "codefabric" in name
            try:
                text = _bounded_text(path).lower()
            except FreshActivationAssuranceError as error:
                raise FreshActivationAssuranceError(
                    "CFV7_DEPLOYMENT_CENSUS_UNREADABLE", str(path)
                ) from error
            if named_for_product or any(
                token in text for token in SERVICE_CONTENT_TOKENS
            ):
                candidates.append(str(path))
    return candidates


def _process_looks_like_product(process: Path, name: str) -> bool:
    if name in PRODUCT_PROCESS_NAMES or any(
        name.startswith(prefix) for prefix in PRODUCT_PROCESS_PREFIXES
    ):
        return True
    try:
        argv = [
            os.fsdecode(value)
            for value in (process / "cmdline").read_bytes().split(b"\0")
            if value
        ]
    except OSError:
        return False
    executable = Path(argv[0]).name if argv else ""
    if executable in PRODUCT_PROCESS_NAMES or any(
        executable.startswith(prefix) for prefix in PRODUCT_PROCESS_PREFIXES
    ):
        return True
    for index, argument in enumerate(argv):
        if (
            argument == "-m"
            and index + 1 < len(argv)
            and argv[index + 1] in PRODUCT_MODULE_NAMES
        ):
            return True
    return False


def _running_product_processes(proc_root: Path) -> list[str]:
    if not proc_root.is_dir():
        return []
    findings: list[str] = []
    for process in proc_root.iterdir():
        if not process.name.isdecimal():
            continue
        try:
            name = (process / "comm").read_text(encoding="utf-8").strip()
        except (OSError, UnicodeError):
            continue
        if _process_looks_like_product(process, name):
            findings.append(f"{process.name}:{name}")
    return sorted(findings)


def deployment_predecessor_census(
    root: Path = ROOT,
    *,
    home: Path | None = None,
    service_roots: Sequence[Path] | None = None,
    product_roots: Sequence[Path] | None = None,
    proc_root: Path = Path("/proc"),
) -> Mapping[str, object]:
    """Read every supported deployment surface and reject an unclassified installation."""

    deployment_root = root / "contracts/deployment"
    repository_files = _iter_files(root, (Path("contracts/deployment"),))
    if not deployment_root.is_dir() or not repository_files:
        raise FreshActivationAssuranceError(
            "CFV7_DEPLOYMENT_CONTRACT_MISSING", str(deployment_root)
        )
    predecessor_contracts: list[str] = []
    for relative in repository_files:
        text = _bounded_text(root / relative)
        for category, token in DEPLOYED_PREDECESSOR_TOKENS.items():
            if token in text:
                predecessor_contracts.append(f"{category}:{relative.as_posix()}")

    census_home = home if home is not None else Path.home()
    services = _service_candidates(
        service_roots
        if service_roots is not None
        else _default_service_roots(census_home)
    )
    state = _product_state_entries(
        product_roots
        if product_roots is not None
        else _default_product_roots(census_home)
    )
    processes = _running_product_processes(proc_root)
    findings = predecessor_contracts + services + state + processes
    if findings:
        raise FreshActivationAssuranceError(
            "CFV7_DEPLOYED_PREDECESSOR_FOUND", ", ".join(findings)
        )
    return {
        "repository_deployment_contracts": len(repository_files),
        "service_candidates": 0,
        "product_state_entries": 0,
        "running_product_processes": 0,
        "predecessor_found": False,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--skip-host", action="store_true")
    args = parser.parse_args(argv)
    try:
        zero_state = validate_dormant_authority_zero_state(args.root)
        if args.skip_host:
            census = deployment_predecessor_census(
                args.root,
                home=Path("/nonexistent"),
                service_roots=(),
                product_roots=(),
                proc_root=Path("/nonexistent"),
            )
        else:
            census = deployment_predecessor_census(args.root)
    except FreshActivationAssuranceError as error:
        print(
            json.dumps(
                {"status": "failed", "code": error.code, "message": str(error)},
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1
    print(
        json.dumps(
            {"status": "passed", "deployment_census": census, "zero_state": zero_state},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
