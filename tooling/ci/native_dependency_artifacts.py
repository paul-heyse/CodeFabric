"""Package and verify repository-local, immutable native dependency amendments.

This module uses only the standard library and Git; the Cargo router can invoke it
before dependency resolution without bootstrapping a Python package environment.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from collections.abc import Iterator
from pathlib import Path, PurePosixPath
from typing import Any

import tomllib

MANIFEST = Path("tooling/native-dependencies/manifest.json")
ARTIFACTS = Path("third_party/native")
HEX = re.compile(r"[0-9a-f]{64}\Z")
NAME = re.compile(r"[a-zA-Z0-9][a-zA-Z0-9_.-]*\Z")


class NativeArtifactError(ValueError):
    """The installed or staged artifact does not match its complete declaration."""


def _json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _decode_json(payload: bytes | str) -> dict[str, Any]:
    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise NativeArtifactError(f"duplicate JSON field: {key}")
            result[key] = value
        return result

    value = json.loads(payload, object_pairs_hook=unique)
    if not isinstance(value, dict):
        raise NativeArtifactError("expected JSON object")
    return value


def _read_json(path: Path) -> dict[str, Any]:
    return _decode_json(path.read_bytes())


def _relative(value: str) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise NativeArtifactError(f"invalid relative path: {value!r}")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in value.split("/")):
        raise NativeArtifactError(f"invalid relative path: {value!r}")
    return Path(*path.parts)


def _inside(path: Path, root: Path) -> None:
    if not path.resolve().is_relative_to(root.resolve()):
        raise NativeArtifactError(f"external symlink or path: {path}")


def census(root: Path) -> dict[str, dict[str, Any]]:
    """Include directories, executable bits, bytes, and confined symlink targets."""
    if root.is_symlink() or not root.is_dir():
        raise NativeArtifactError(f"expected ordinary source directory: {root}")
    result: dict[str, dict[str, Any]] = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in sorted([*dirs, *files]):
            path = Path(directory) / name
            relative = path.relative_to(root).as_posix()
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                _inside(path, root)
                if not path.exists():
                    raise NativeArtifactError(f"broken source symlink: {path}")
                entry = {"kind": "symlink", "target": os.readlink(path)}
            elif stat.S_ISDIR(mode):
                entry = {"kind": "directory"}
            elif stat.S_ISREG(mode):
                payload = path.read_bytes()
                entry = {
                    "kind": "file",
                    "sha256": _digest(payload),
                    "bytes": len(payload),
                    "executable": bool(mode & 0o111),
                }
            else:
                raise NativeArtifactError(f"special source file: {path}")
            result[relative] = entry
    return dict(sorted(result.items()))


def _git(repository: Path, *args: str, payload: bytes | None = None) -> bytes:
    result = subprocess.run(
        [
            "git",
            "-c",
            f"core.attributesFile={os.devnull}",
            "-C",
            str(repository),
            *args,
        ],
        input=payload,
        capture_output=True,
        check=False,
        env={
            **{
                key: value
                for key, value in os.environ.items()
                if not key.startswith("GIT_")
            },
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
        },
    )
    if result.returncode:
        raise NativeArtifactError(result.stderr.decode(errors="replace").strip())
    return result.stdout


def _extract(archive: bytes, destination: Path, prefix: str | None) -> None:
    """Extract only ordinary files/directories and confined symlinks, never hardlinks."""
    destination.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:*") as source:
        entries: set[str] = set()
        for member in source:
            name = member.name.rstrip("/")
            if prefix:
                if name == prefix:
                    continue
                if not name.startswith(prefix + "/"):
                    raise NativeArtifactError(
                        "upstream archive has an unexpected package prefix"
                    )
                name = name[len(prefix) + 1 :]
            relative = _relative(name)
            if name in entries:
                raise NativeArtifactError(f"duplicate archive entry: {name}")
            entries.add(name)
            path = destination / relative
            _inside(path.parent, destination)
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.exists() and not (path.is_dir() and member.isdir()):
                raise NativeArtifactError(f"overlapping archive entry: {name}")
            if member.isdir():
                path.mkdir(exist_ok=True)
            elif member.isfile():
                payload = source.extractfile(member)
                if payload is None:
                    raise NativeArtifactError(f"unreadable archive member: {name}")
                path.write_bytes(payload.read())
                path.chmod(0o755 if member.mode & 0o111 else 0o644)
            elif member.issym():
                if Path(member.linkname).is_absolute():
                    raise NativeArtifactError(f"absolute archive symlink: {name}")
                path.symlink_to(member.linkname)
                _inside(path, destination)
            else:
                raise NativeArtifactError(f"unsupported archive entry: {name}")
    census(destination)


def _source_input(base: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else base / path


def _upstream(
    spec: dict[str, Any], base: Path
) -> tuple[bytes, dict[str, Any], str | None]:
    kind = spec.get("kind")
    if kind == "git":
        if set(spec) != {"kind", "repository", "url", "revision"}:
            raise NativeArtifactError("invalid Git upstream fields")
        revision = spec["revision"]
        if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise NativeArtifactError("Git upstream requires an exact 40-digit commit")
        repository = _source_input(base, spec["repository"])
        attributes = Path(
            _git(repository, "rev-parse", "--git-path", "info/attributes")
            .decode()
            .strip()
        )
        attributes = attributes if attributes.is_absolute() else repository / attributes
        if attributes.exists() and attributes.read_bytes().strip():
            raise NativeArtifactError(
                "Git archive must not use uncommitted info/attributes"
            )
        if (
            _git(repository, "rev-parse", f"{revision}^{{commit}}").decode().strip()
            != revision
        ):
            raise NativeArtifactError("Git upstream identity is not the exact commit")
        archive = _git(repository, "archive", "--format=tar", revision)
        return archive, {"kind": kind, "url": spec["url"], "revision": revision}, None
    if kind == "registry":
        if set(spec) != {"kind", "archive", "url", "name", "version", "sha256"}:
            raise NativeArtifactError("invalid registry upstream fields")
        archive = _source_input(base, spec["archive"]).read_bytes()
        if _digest(archive) != spec["sha256"]:
            raise NativeArtifactError("upstream registry archive checksum mismatch")
        origin = {key: value for key, value in spec.items() if key != "archive"}
        return archive, origin, f"{spec['name']}-{spec['version']}"
    raise NativeArtifactError(f"unsupported upstream kind: {kind}")


def _validate_origin(upstream: dict[str, Any], archive: bytes) -> None:
    origin = upstream["origin"]
    if (
        not isinstance(origin, dict)
        or not isinstance(origin.get("url"), str)
        or not origin["url"]
    ):
        raise NativeArtifactError("upstream origin requires a nonempty source URL")
    if origin.get("kind") == "git":
        if (
            set(origin) != {"kind", "url", "revision"}
            or not re.fullmatch(r"[0-9a-f]{40}", origin["revision"])
            or upstream["strip_prefix"] is not None
        ):
            raise NativeArtifactError("invalid installed Git origin")
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:*") as source:
            if source.pax_headers.get("comment") != origin["revision"]:
                raise NativeArtifactError(
                    "Git archive commit marker differs from origin"
                )
    elif origin.get("kind") == "registry":
        if set(origin) != {"kind", "url", "name", "version", "sha256"}:
            raise NativeArtifactError("invalid installed registry origin")
        if (
            origin["sha256"] != upstream["sha256"]
            or upstream["strip_prefix"] != f"{origin['name']}-{origin['version']}"
        ):
            raise NativeArtifactError(
                "registry origin checksum or archive prefix differs"
            )
    else:
        raise NativeArtifactError("unsupported installed upstream origin")
    if not isinstance(upstream["tree_sha256"], str) or not HEX.fullmatch(
        upstream["tree_sha256"]
    ):
        raise NativeArtifactError("invalid upstream tree digest")


def _package_field(source: Path, manifest: Path, declared: dict, key: str) -> Any:
    value = declared.get(key)
    if value == {"workspace": True}:
        parent = manifest.parent
        while parent.is_relative_to(source):
            ancestor = parent / "Cargo.toml"
            if ancestor.is_file():
                value = (
                    tomllib.loads(ancestor.read_text())
                    .get("workspace", {})
                    .get("package", {})
                    .get(key, value)
                )
                if isinstance(value, str):
                    break
            parent = parent.parent
    return value


def _package_identity(source: Path, package: dict[str, Any]) -> None:
    if set(package) != {"name", "version", "manifest", "license", "license_files"}:
        raise NativeArtifactError(
            "package identity requires name, version, manifest, license, license_files"
        )
    if not all(
        isinstance(package[key], str) and package[key]
        for key in ("name", "version", "manifest", "license")
    ):
        raise NativeArtifactError(
            "package and license identities must be nonempty strings"
        )
    manifest = source / _relative(package["manifest"])
    _inside(manifest, source)
    declared = tomllib.loads(manifest.read_text()).get("package", {})
    if (
        declared.get("name") != package["name"]
        or _package_field(source, manifest, declared, "version") != package["version"]
    ):
        raise NativeArtifactError(f"package identity mismatch: {manifest}")
    if _package_field(source, manifest, declared, "license") != package["license"]:
        raise NativeArtifactError(f"package license identity mismatch: {manifest}")
    records = package["license_files"]
    if not isinstance(records, list) or not records:
        raise NativeArtifactError("package requires full license file records")
    paths: set[str] = set()
    primary = False
    for record in records:
        if not isinstance(record, dict) or set(record) != {
            "path",
            "role",
            "sha256",
            "origin",
        }:
            raise NativeArtifactError("invalid license file record fields")
        path = source / _relative(record["path"])
        _inside(path, source)
        if record["path"] in paths:
            raise NativeArtifactError("duplicate package license file")
        paths.add(record["path"])
        if record["role"] not in {"license", "notice", "additional"}:
            raise NativeArtifactError("invalid license file role")
        primary |= record["role"] == "license"
        origin = record["origin"]
        if not isinstance(origin, str) or not (
            origin == "upstream" or origin.startswith("https://")
        ):
            raise NativeArtifactError(
                "license origin must be upstream or a reviewed HTTPS source"
            )
        checksum = record["sha256"]
        if not isinstance(checksum, str) or not HEX.fullmatch(checksum):
            raise NativeArtifactError("invalid license file checksum")
        if (
            not path.is_file()
            or not path.stat().st_size
            or _digest(path.read_bytes()) != checksum
        ):
            raise NativeArtifactError("missing, empty, or altered full license file")
    if not primary:
        raise NativeArtifactError("package requires at least one full primary license")


def _verify_license_census(source: Path, packages: list[dict]) -> None:
    """Full source notices stay explicit even when only a subset of packages is selected."""
    declared = {
        record["path"] for package in packages for record in package["license_files"]
    }
    actual = {
        path.relative_to(source).as_posix()
        for path in source.rglob("*")
        if path.is_file()
        and re.fullmatch(
            r"(?:LICEN[CS]E|NOTICE|COPYING)(?:[._-].*)?", path.name, re.IGNORECASE
        )
    }
    if actual - declared:
        raise NativeArtifactError(
            "undeclared source license or notice files: "
            + ", ".join(sorted(actual - declared))
        )


def _manifest_paths(value: Any) -> Iterator[str]:
    """Inspect target/workspace/patch dependency tables and explicit source paths."""
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "path" and isinstance(child, str):
                yield child
            else:
                yield from _manifest_paths(child)
    elif isinstance(value, list):
        for child in value:
            yield from _manifest_paths(child)


def _verify_source_paths(source: Path) -> None:
    # Check all manifests, including unselected optional and workspace packages.
    # Otherwise selecting a feature could introduce mutable external source later.
    for manifest in source.rglob("Cargo.toml"):
        _inside(manifest, source)
        for value in _manifest_paths(tomllib.loads(manifest.read_text())):
            path = Path(value)
            if path.is_absolute():
                raise NativeArtifactError(
                    f"absolute native Cargo path: {manifest}: {value}"
                )
            _inside(manifest.parent / path, source)


def _artifact_required(root: Path) -> bool:
    artifacts = root / ARTIFACTS
    if artifacts.is_symlink() or (artifacts.exists() and any(artifacts.iterdir())):
        return True
    for relative in (
        "Cargo.toml",
        "rustc-extractor/Cargo.toml",
        "pyrefly-sidecar/Cargo.toml",
    ):
        manifest = root / relative
        if not manifest.exists():
            continue
        for value in _manifest_paths(tomllib.loads(manifest.read_text())):
            resolved = (manifest.parent / value).resolve()
            if resolved.is_relative_to(artifacts.resolve()):
                return True
    return False


def _replay(root: Path, source: dict[str, Any], destination: Path) -> None:
    upstream = source["upstream"]
    archive = root / _relative(upstream["archive"])
    if _digest(archive.read_bytes()) != upstream["sha256"]:
        raise NativeArtifactError("upstream archive checksum mismatch")
    _extract(archive.read_bytes(), destination, upstream["strip_prefix"])
    if _digest(_json_bytes(census(destination))) != upstream["tree_sha256"]:
        raise NativeArtifactError("upstream tree identity mismatch")
    for package in source["packages"]:
        manifest = destination / _relative(package["manifest"])
        _inside(manifest, destination)
        declared = tomllib.loads(manifest.read_text()).get("package", {})
        if (
            _package_field(destination, manifest, declared, "license")
            != package["license"]
        ):
            raise NativeArtifactError(
                "package license differs from the exact upstream declaration"
            )
        for record in package["license_files"]:
            if record["origin"] == "upstream":
                path = destination / _relative(record["path"])
                _inside(path, destination)
                if not path.is_file() or _digest(path.read_bytes()) != record["sha256"]:
                    raise NativeArtifactError(
                        "license claimed as upstream differs from exact archive"
                    )
    for patch in source["patches"]:
        payload = (root / _relative(patch["path"])).read_bytes()
        if _digest(payload) != patch["sha256"]:
            raise NativeArtifactError("amendment patch checksum mismatch")
        _git(
            destination, "apply", "--check", "--whitespace=nowarn", "-", payload=payload
        )
        _git(destination, "apply", "--whitespace=nowarn", "-", payload=payload)
    if census(destination) != census(root / _relative(source["path"])):
        raise NativeArtifactError("amendments do not reproduce the staged source tree")


def _verify_manifest(
    root: Path, manifest: dict[str, Any], replay: bool
) -> dict[str, Any]:
    if (
        set(manifest)
        != {"schema_version", "artifact_sha256", "artifact_path", "sources", "files"}
        or type(manifest["schema_version"]) is not int
        or manifest["schema_version"] != 1
    ):
        raise NativeArtifactError("unsupported native artifact manifest")
    descriptor = {
        key: value
        for key, value in manifest.items()
        if key not in {"artifact_sha256", "artifact_path"}
    }
    identity = _digest(_json_bytes(descriptor))
    if identity != manifest["artifact_sha256"]:
        raise NativeArtifactError("native artifact descriptor checksum mismatch")
    expected_path = (ARTIFACTS / identity).as_posix()
    if manifest["artifact_path"] != expected_path:
        raise NativeArtifactError("native artifact path is not its content identity")
    artifact = root / _relative(expected_path)
    _inside(artifact, root)
    if not isinstance(manifest["files"], dict) or not manifest["files"]:
        raise NativeArtifactError("native artifact requires a file census")
    for name, entry in manifest["files"].items():
        _relative(name)
        if not isinstance(entry, dict):
            raise NativeArtifactError("invalid census entry")
        kind = entry.get("kind")
        if kind == "directory" and set(entry) == {"kind"}:
            continue
        if (
            kind == "file"
            and set(entry) == {"kind", "sha256", "bytes", "executable"}
            and isinstance(entry["sha256"], str)
            and HEX.fullmatch(entry["sha256"])
            and type(entry["bytes"]) is int
            and entry["bytes"] >= 0
            and type(entry["executable"]) is bool
        ):
            continue
        if (
            kind == "symlink"
            and set(entry) == {"kind", "target"}
            and isinstance(entry["target"], str)
            and entry["target"]
            and not Path(entry["target"]).is_absolute()
        ):
            continue
        raise NativeArtifactError("invalid census entry fields")
    if census(artifact) != manifest["files"]:
        raise NativeArtifactError(
            "native artifact file census mismatch (altered, extra, or missing entry)"
        )
    if not isinstance(manifest["sources"], list) or not manifest["sources"]:
        raise NativeArtifactError("native artifact requires at least one source")
    names: set[str] = set()
    packages: set[str] = set()
    for source in manifest["sources"]:
        if set(source) != {"name", "path", "upstream", "patches", "packages"}:
            raise NativeArtifactError("invalid installed source fields")
        name = source["name"]
        if not isinstance(name, str) or not NAME.fullmatch(name) or name in names:
            raise NativeArtifactError("duplicate or invalid native source name")
        names.add(name)
        if source["path"] != f"sources/{name}":
            raise NativeArtifactError("source path does not match its declared name")
        upstream = source["upstream"]
        if set(upstream) != {
            "origin",
            "archive",
            "sha256",
            "strip_prefix",
            "tree_sha256",
        }:
            raise NativeArtifactError("invalid upstream provenance fields")
        if upstream["archive"] != f"upstream/{name}.tar":
            raise NativeArtifactError("upstream archive path does not match source")
        _validate_origin(
            upstream, (artifact / _relative(upstream["archive"])).read_bytes()
        )
        for index, patch in enumerate(source["patches"]):
            if (
                set(patch) != {"path", "sha256"}
                or patch["path"] != f"patches/{name}/{index:04d}.patch"
            ):
                raise NativeArtifactError("amendment path or order differs")
        for path, checksum in [
            (upstream["archive"], upstream["sha256"]),
            *[(patch["path"], patch["sha256"]) for patch in source["patches"]],
        ]:
            if (
                not isinstance(checksum, str)
                or not HEX.fullmatch(checksum)
                or _digest((artifact / _relative(path)).read_bytes()) != checksum
            ):
                raise NativeArtifactError("upstream or patch checksum mismatch")
        if not source["packages"]:
            raise NativeArtifactError("source has no declared package")
        _verify_source_paths(artifact / source["path"])
        for package in source["packages"]:
            if package["name"] in packages:
                raise NativeArtifactError("duplicate native package")
            packages.add(package["name"])
            _package_identity(artifact / source["path"], package)
        _verify_license_census(artifact / source["path"], source["packages"])
        if replay:
            with tempfile.TemporaryDirectory(
                prefix="codefabric-native-replay-"
            ) as temporary:
                _replay(artifact, source, Path(temporary) / "source")
    return {
        "artifact_sha256": identity,
        "sources": len(names),
        "packages": sorted(packages),
        "files": len(manifest["files"]),
    }


def verify(
    root: Path, *, if_installed: bool = False, replay: bool = False
) -> dict[str, Any]:
    root = root.resolve()
    path = root / MANIFEST
    if path.is_symlink():
        raise NativeArtifactError("installed native manifest must not be a symlink")
    if if_installed and not path.exists():
        if _artifact_required(root):
            raise NativeArtifactError(
                "native source exists or is selected but its manifest is missing"
            )
        return {"installed": False}
    _inside(path, root)
    return _verify_manifest(root, _read_json(path), replay)


def verify_resolution(
    root: Path, metadata: dict[str, Any], *, require_all: bool = True
) -> dict[str, Any]:
    """Check a complete Cargo graph against the verified installed source identity.

    A narrow feature graph may explicitly allow a subset. Every native package
    that it does resolve must still use the exact artifact manifest, including
    transitive dependencies; equal package versions alone prove nothing here.
    Metadata must be obtained from Cargo without ``--no-deps`` by the caller.
    """
    root = root.resolve()
    report = verify(root)
    manifest = _read_json(root / MANIFEST)
    artifact = root / manifest["artifact_path"]
    expected = {
        package["name"]: (
            package["version"],
            (artifact / source["path"] / package["manifest"]).resolve(),
        )
        for source in manifest["sources"]
        for package in source["packages"]
    }
    if (
        type(metadata.get("version")) is not int
        or metadata["version"] != 1
        or metadata.get("workspace_root") != str(root)
        or not isinstance(metadata.get("packages"), list)
        or not metadata["packages"]
        or not isinstance(metadata.get("resolve"), dict)
        or not isinstance(metadata["resolve"].get("nodes"), list)
        or not metadata["resolve"]["nodes"]
    ):
        raise NativeArtifactError("expected complete Cargo metadata for this root")
    packages: dict[str, dict[str, Any]] = {}
    selected: set[str] = set()
    for package in metadata["packages"]:
        if not isinstance(package, dict) or any(
            not isinstance(package.get(key), str) or not package[key]
            for key in ("id", "name", "version", "manifest_path")
        ):
            raise NativeArtifactError("malformed resolved Cargo package")
        identity = package["id"]
        if identity in packages:
            raise NativeArtifactError("duplicate resolved Cargo package ID")
        packages[identity] = package
        path = Path(package["manifest_path"])
        if not path.is_absolute() or "source" not in package:
            raise NativeArtifactError("incomplete resolved Cargo source identity")
        name = package["name"]
        if name in expected:
            if name in selected:
                raise NativeArtifactError(f"duplicate resolved native package: {name}")
            version, approved = expected[name]
            if (
                package["version"] != version
                or package["source"] is not None
                or path != approved
                or path.resolve() != approved
            ):
                raise NativeArtifactError(
                    f"resolved native package does not use exact artifact: {name}"
                )
            selected.add(name)
        elif path.resolve().is_relative_to(artifact.resolve()):
            raise NativeArtifactError(f"undeclared resolved artifact package: {name}")
    nodes: set[str] = set()
    for node in metadata["resolve"]["nodes"]:
        if (
            not isinstance(node, dict)
            or not isinstance(node.get("id"), str)
            or node["id"] not in packages
            or node["id"] in nodes
            or not isinstance(node.get("dependencies"), list)
            or any(
                not isinstance(dependency, str) or dependency not in packages
                for dependency in node["dependencies"]
            )
        ):
            raise NativeArtifactError("malformed or incomplete Cargo resolve graph")
        nodes.add(node["id"])
    if nodes != packages.keys():
        raise NativeArtifactError("Cargo packages and resolve nodes disagree")
    if require_all and selected != expected.keys():
        missing = ", ".join(sorted(expected.keys() - selected))
        raise NativeArtifactError(
            f"required native packages absent from graph: {missing}"
        )
    return {
        "artifact_sha256": report["artifact_sha256"],
        "resolved_native_packages": sorted(selected),
        "require_all": require_all,
        "resolved_packages": len(packages),
    }


def package(
    root: Path, specification: Path, *, replace_manifest: bool = False
) -> dict[str, Any]:
    root = root.resolve()
    specification = specification.resolve()
    spec = _read_json(specification)
    if (
        set(spec) != {"schema_version", "sources"}
        or type(spec["schema_version"]) is not int
        or spec["schema_version"] != 1
        or not spec["sources"]
    ):
        raise NativeArtifactError("invalid native package specification")
    with tempfile.TemporaryDirectory(prefix="codefabric-native-package-") as temporary:
        staged = Path(temporary) / "artifact"
        staged.mkdir()
        (staged / "sources").mkdir()
        (staged / "upstream").mkdir()
        (staged / "patches").mkdir()
        sources = []
        names: set[str] = set()
        packages: set[str] = set()
        for source in spec["sources"]:
            if set(source) != {
                "name",
                "upstream",
                "staged_path",
                "patches",
                "packages",
            }:
                raise NativeArtifactError("invalid package source fields")
            name = source["name"]
            if not isinstance(name, str) or not NAME.fullmatch(name) or name in names:
                raise NativeArtifactError("duplicate or invalid native source name")
            names.add(name)
            archive, origin, prefix = _upstream(
                source["upstream"], specification.parent
            )
            archive_path = f"upstream/{name}.tar"
            (staged / archive_path).write_bytes(archive)
            original = Path(temporary) / f"original-{name}"
            _extract(archive, original, prefix)
            source_path = f"sources/{name}"
            amended = _source_input(specification.parent, source["staged_path"])
            census(amended)
            shutil.copytree(amended, staged / source_path, symlinks=True)
            if not source["packages"]:
                raise NativeArtifactError("source has no declared package")
            for entry in source["packages"]:
                _package_identity(staged / source_path, entry)
                if entry["name"] in packages:
                    raise NativeArtifactError("duplicate native package")
                packages.add(entry["name"])
            _verify_license_census(staged / source_path, source["packages"])
            patches = []
            (staged / "patches" / name).mkdir()
            for index, patch in enumerate(source["patches"]):
                payload = _source_input(specification.parent, patch).read_bytes()
                patch_path = f"patches/{name}/{index:04d}.patch"
                (staged / patch_path).write_bytes(payload)
                patches.append({"path": patch_path, "sha256": _digest(payload)})
            record = {
                "name": name,
                "path": source_path,
                "upstream": {
                    "origin": origin,
                    "archive": archive_path,
                    "sha256": _digest(archive),
                    "strip_prefix": prefix,
                    "tree_sha256": _digest(_json_bytes(census(original))),
                },
                "patches": patches,
                "packages": source["packages"],
            }
            _validate_origin(record["upstream"], archive)
            _replay(staged, record, Path(temporary) / f"replay-{name}")
            sources.append(record)
        descriptor = {"schema_version": 1, "sources": sources, "files": census(staged)}
        identity = _digest(_json_bytes(descriptor))
        relative = ARTIFACTS / identity
        manifest = {
            **descriptor,
            "artifact_sha256": identity,
            "artifact_path": relative.as_posix(),
        }
        installed = root / MANIFEST
        if installed.is_symlink():
            raise NativeArtifactError("installed native manifest must not be a symlink")
        encoded = _json_bytes(manifest)
        if (
            installed.exists()
            and installed.read_bytes() != encoded
            and not replace_manifest
        ):
            raise NativeArtifactError(
                "different manifest exists; use --replace-manifest deliberately"
            )
        destination = root / relative
        _inside(destination, root)
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists():
            if census(destination) != descriptor["files"]:
                raise NativeArtifactError("existing immutable artifact differs")
        else:
            unpublished = Path(
                tempfile.mkdtemp(prefix=".native-package-", dir=destination.parent)
            )
            try:
                shutil.copytree(staged, unpublished, dirs_exist_ok=True, symlinks=True)
                unpublished.rename(destination)
            finally:
                if unpublished.exists():
                    shutil.rmtree(unpublished)
        _verify_manifest(root, manifest, replay=False)
        _inside(installed, root)
        installed.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=installed.parent, delete=False) as output:
            temporary_manifest = Path(output.name)
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        try:
            os.replace(temporary_manifest, installed)
        finally:
            temporary_manifest.unlink(missing_ok=True)
    return verify(root)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("verify")
    check.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    check.add_argument("--if-installed", action="store_true")
    check.add_argument("--replay", action="store_true")
    check.add_argument("--quiet", action="store_true")
    resolution = commands.add_parser("verify-resolution")
    resolution.add_argument("--root", type=Path, required=True)
    resolution.add_argument("--metadata", required=True, help="Cargo JSON path or -")
    resolution.add_argument("--allow-subset", action="store_true")
    build = commands.add_parser("package")
    build.add_argument("--root", type=Path, required=True)
    build.add_argument("--spec", type=Path, required=True)
    build.add_argument("--replace-manifest", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.command == "package":
            result = package(
                args.root, args.spec, replace_manifest=args.replace_manifest
            )
        elif args.command == "verify-resolution":
            metadata = (
                _decode_json(sys.stdin.read())
                if args.metadata == "-"
                else _read_json(Path(args.metadata))
            )
            result = verify_resolution(
                args.root, metadata, require_all=not args.allow_subset
            )
        else:
            result = verify(
                args.root, if_installed=args.if_installed, replay=args.replay
            )
        if not getattr(args, "quiet", False):
            print(json.dumps(result, sort_keys=True))
    except (
        NativeArtifactError,
        OSError,
        ValueError,
        KeyError,
        TypeError,
        tarfile.TarError,
    ) as error:
        print(f"native dependency artifact rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
