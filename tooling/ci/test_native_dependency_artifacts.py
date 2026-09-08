"""Fault tests against real archive extraction, patch replay, and Cargo routing."""

from __future__ import annotations

import copy
import difflib
import hashlib
import io
import json
import os
import shutil
import subprocess
import tarfile
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from tooling.ci.native_dependency_artifacts import (
    MANIFEST,
    NativeArtifactError,
    census,
    main,
    package,
    verify,
    verify_resolution,
)


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path]:
    root = tmp_path / "repository"
    root.mkdir()
    inputs = tmp_path / "inputs"
    inputs.mkdir()
    source = inputs / "amended"
    source.mkdir()
    (source / "src").mkdir()
    (source / "Cargo.toml").write_text(
        '[package]\nname = "example"\nversion = "1.2.3"\nlicense = "Apache-2.0"\n'
    )
    (source / "LICENSE").write_text("test license\n")
    (source / "LICENSE-link").symlink_to("LICENSE")
    (source / "src/lib.rs").write_text("pub const VALUE: u64 = 1;\n")
    archive = inputs / "example.crate"
    with tarfile.open(archive, "w:gz") as target:
        target.add(source, arcname="example-1.2.3", recursive=True)
    (source / "src/lib.rs").write_text("pub const VALUE: u64 = 2;\n")
    (inputs / "change.patch").write_text(
        "diff --git a/src/lib.rs b/src/lib.rs\n"
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n"
        "-pub const VALUE: u64 = 1;\n+pub const VALUE: u64 = 2;\n"
    )
    spec = {
        "schema_version": 1,
        "sources": [
            {
                "name": "example",
                "upstream": {
                    "kind": "registry",
                    "archive": "example.crate",
                    "url": "https://example.invalid/index",
                    "name": "example",
                    "version": "1.2.3",
                    "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                },
                "staged_path": "amended",
                "patches": ["change.patch"],
                "packages": [
                    {
                        "name": "example",
                        "version": "1.2.3",
                        "manifest": "Cargo.toml",
                        "license": "Apache-2.0",
                        "license_files": [
                            {
                                "path": path,
                                "role": role,
                                "sha256": hashlib.sha256(
                                    (source / path).read_bytes()
                                ).hexdigest(),
                                "origin": "upstream",
                            }
                            for path, role in [
                                ("LICENSE", "license"),
                                ("LICENSE-link", "additional"),
                            ]
                        ],
                    }
                ],
            }
        ],
    }
    specification = inputs / "spec.json"
    _write_json(specification, spec)
    return root, specification, source


def _installed(root: Path) -> tuple[dict, Path]:
    manifest = json.loads((root / MANIFEST).read_bytes())
    return manifest, root / manifest["artifact_path"]


def _cargo_metadata(root: Path, *, native: bool = True) -> dict:
    _, artifact = _installed(root)
    dependency = (artifact / "sources/example").relative_to(root).as_posix()
    (root / "Cargo.toml").write_text(
        '[package]\nname = "consumer"\nversion = "0.1.0"\nedition = "2024"\n'
        + (f'[dependencies]\nexample = {{ path = "{dependency}" }}\n' if native else "")
    )
    (root / "src").mkdir(exist_ok=True)
    (root / "src/lib.rs").write_text("pub fn consumer() {}\n")
    result = subprocess.run(
        ["cargo", "metadata", "--offline", "--format-version", "1"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def test_resolved_native_sources_use_real_cargo_and_relocated_artifact(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture
) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    metadata = _cargo_metadata(root)
    report = verify_resolution(root, metadata)
    assert report["resolved_native_packages"] == ["example"]
    assert report["resolved_packages"] == 2
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps(metadata)))
    assert main(["verify-resolution", "--root", str(root), "--metadata", "-"]) == 0
    assert json.loads(capsys.readouterr().out) == report
    relocated = tmp_path / "relocated"
    shutil.copytree(root, relocated, symlinks=True)
    shutil.rmtree(root)
    assert verify_resolution(relocated, _cargo_metadata(relocated)) == report
    with pytest.raises(NativeArtifactError, match="this root"):
        verify_resolution(relocated, metadata)
    subset = _cargo_metadata(relocated, native=False)
    with pytest.raises(NativeArtifactError, match="absent"):
        verify_resolution(relocated, subset)
    assert (
        verify_resolution(relocated, subset, require_all=False)[
            "resolved_native_packages"
        ]
        == []
    )


@pytest.mark.parametrize(
    "fault",
    [
        "registry",
        "mutable-copy",
        "wrong-version",
        "duplicate-native",
        "duplicate-id",
        "no-deps",
        "missing-node",
        "unknown-node",
        "duplicate-node",
        "unknown-dependency",
        "undeclared-artifact",
        "missing-source",
        "relative-path",
        "wrong-root",
        "source-tampering",
    ],
)
def test_resolved_native_graph_faults_fail_closed(tmp_path: Path, fault: str) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    metadata = _cargo_metadata(root)
    native = next(item for item in metadata["packages"] if item["name"] == "example")
    if fault == "registry":
        native["source"] = "registry+https://github.com/rust-lang/crates.io-index"
    elif fault == "mutable-copy":
        native["manifest_path"] = str(tmp_path / "amended/Cargo.toml")
    elif fault == "wrong-version":
        native["version"] = "1.2.4"
    elif fault == "duplicate-native":
        duplicate = copy.deepcopy(native)
        duplicate["id"] += "-different-source"
        metadata["packages"].append(duplicate)
    elif fault == "duplicate-id":
        metadata["packages"].append(copy.deepcopy(native))
    elif fault == "no-deps":
        metadata["resolve"] = None
    elif fault == "missing-node":
        metadata["resolve"]["nodes"] = [
            node for node in metadata["resolve"]["nodes"] if node["id"] != native["id"]
        ]
    elif fault == "unknown-node":
        metadata["resolve"]["nodes"][0]["id"] = "unknown"
    elif fault == "duplicate-node":
        metadata["resolve"]["nodes"].append(metadata["resolve"]["nodes"][0])
    elif fault == "unknown-dependency":
        metadata["resolve"]["nodes"][0]["dependencies"].append("unknown")
    elif fault == "undeclared-artifact":
        native["name"] = "undeclared"
    elif fault == "missing-source":
        del native["source"]
    elif fault == "relative-path":
        native["manifest_path"] = "Cargo.toml"
    elif fault == "wrong-root":
        metadata["workspace_root"] = str(tmp_path)
    elif fault == "source-tampering":
        Path(native["manifest_path"]).with_name("LICENSE").write_text("altered")
    else:
        raise AssertionError(fault)
    # A narrow graph does not permit fallback or malformed native identities either.
    for require_all in (True, False):
        with pytest.raises(NativeArtifactError):
            verify_resolution(root, metadata, require_all=require_all)


def test_package_replays_patch_preserves_internal_links_and_relocates(
    tmp_path: Path,
) -> None:
    root, spec, _ = _fixture(tmp_path)
    report = package(root, spec)
    assert report["packages"] == ["example"]
    manifest, artifact = _installed(root)
    schema_root = Path(__file__).resolve().parents[1] / "native-dependencies"
    for schema_name, data in [
        ("package-spec.schema.json", json.loads(spec.read_bytes())),
        ("manifest.schema.json", manifest),
    ]:
        schema = json.loads((schema_root / schema_name).read_bytes())
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(data)
    assert (artifact / "sources/example/src/lib.rs").read_text().endswith("= 2;\n")
    assert (artifact / "sources/example/LICENSE-link").is_symlink()
    assert "amended" not in json.dumps(manifest)
    assert str(tmp_path) not in json.dumps(manifest)
    assert (
        package(root, spec) == report
    )  # Identical reinstallation changes no identity.
    relocated = tmp_path / "other-checkout"
    shutil.copytree(root, relocated, symlinks=True)
    shutil.rmtree(root)
    assert verify(relocated, replay=True) == report


@pytest.mark.parametrize(
    "fault", ["altered", "extra", "missing", "mode", "extra-directory", "external-link"]
)
def test_verifier_rejects_complete_census_faults(tmp_path: Path, fault: str) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    _, artifact = _installed(root)
    path = artifact / "sources/example/src/lib.rs"
    if fault == "altered":
        path.write_text("unreviewed mutation\n")
    elif fault == "extra":
        path.with_name("extra.rs").write_text("extra\n")
    elif fault == "missing":
        path.unlink()
    elif fault == "mode":
        path.chmod(0o755)
    elif fault == "extra-directory":
        path.with_name("empty").mkdir()
    else:
        path.unlink()
        path.symlink_to(spec)
    with pytest.raises(NativeArtifactError, match="census|external symlink"):
        verify(root)


@pytest.mark.parametrize("field", ["upstream", "patch"])
def test_verifier_rejects_provenance_bytes_tampering(
    tmp_path: Path, field: str
) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    manifest, artifact = _installed(root)
    source = manifest["sources"][0]
    relative = (
        source["upstream"]["archive"]
        if field == "upstream"
        else source["patches"][0]["path"]
    )
    with (artifact / relative).open("ab") as output:
        output.write(b"altered")
    with pytest.raises(NativeArtifactError, match="census"):
        verify(root)


@pytest.mark.parametrize(
    "fault", ["checksum", "undeclared-edit", "wrong-patch", "wrong-version"]
)
def test_packaging_rejects_unproved_sources_before_installation(
    tmp_path: Path, fault: str
) -> None:
    root, spec, amended = _fixture(tmp_path)
    data = json.loads(spec.read_bytes())
    if fault == "checksum":
        data["sources"][0]["upstream"]["sha256"] = "0" * 64
    elif fault == "undeclared-edit":
        (amended / "LICENSE").write_text("unexplained edit")
    elif fault == "wrong-patch":
        (spec.parent / "change.patch").write_text("not a patch")
    else:
        data["sources"][0]["packages"][0]["version"] = "9.9.9"
    _write_json(spec, data)
    with pytest.raises(NativeArtifactError):
        package(root, spec)
    assert not (root / MANIFEST).exists()


def test_manifest_tampering_and_missing_manifest_fail_closed(tmp_path: Path) -> None:
    root, spec, _ = _fixture(tmp_path)
    assert verify(root, if_installed=True) == {"installed": False}
    assert main(["verify", "--root", str(root)]) == 1
    package(root, spec)
    manifest, _ = _installed(root)
    manifest["sources"][0]["upstream"]["origin"]["url"] = "https://substituted.invalid"
    _write_json(root / MANIFEST, manifest)
    with pytest.raises(NativeArtifactError, match="descriptor checksum"):
        verify(root)


def test_different_installation_requires_explicit_manifest_replacement(
    tmp_path: Path,
) -> None:
    root, spec, _ = _fixture(tmp_path)
    first = package(root, spec)
    data = json.loads(spec.read_bytes())
    data["sources"][0]["upstream"]["url"] = "https://another-origin.invalid/index"
    _write_json(spec, data)
    with pytest.raises(NativeArtifactError, match="replace-manifest"):
        package(root, spec)
    second = package(root, spec, replace_manifest=True)
    assert first["artifact_sha256"] != second["artifact_sha256"]
    assert (root / "third_party/native" / first["artifact_sha256"]).is_dir()


@pytest.mark.parametrize("kind", ["traversal", "absolute-link", "hardlink"])
def test_archive_escape_is_rejected_without_writing_outside_stage(
    tmp_path: Path, kind: str
) -> None:
    root, spec, _ = _fixture(tmp_path)
    archive = spec.parent / "evil.tar"
    with tarfile.open(archive, "w") as output:
        member = tarfile.TarInfo(
            "example-1.2.3/../escaped" if kind == "traversal" else "example-1.2.3/link"
        )
        if kind == "absolute-link":
            member.type = tarfile.SYMTYPE
            member.linkname = str(tmp_path / "outside")
        elif kind == "hardlink":
            member.type = tarfile.LNKTYPE
            member.linkname = "../../outside"
        else:
            member.size = 1
        output.addfile(member, io.BytesIO(b"x"))
    data = json.loads(spec.read_bytes())
    data["sources"][0]["upstream"].update(
        archive=str(archive), sha256=hashlib.sha256(archive.read_bytes()).hexdigest()
    )
    _write_json(spec, data)
    with pytest.raises(NativeArtifactError):
        package(root, spec)
    assert not (tmp_path / "outside").exists()
    assert not (root / MANIFEST).exists()


def test_git_input_uses_exact_commit_not_dirty_worktree(tmp_path: Path) -> None:
    root, spec, amended = _fixture(tmp_path)
    repository = tmp_path / "upstream-repository"
    shutil.copytree(amended, repository, symlinks=True)
    (repository / "src/lib.rs").write_text("pub const VALUE: u64 = 1;\n")

    def git(*arguments: str) -> str:
        return subprocess.check_output(
            ["git", "-C", str(repository), *arguments], text=True
        ).strip()

    git("init", "-q")
    git("add", ".")
    git(
        "-c",
        "user.name=Artifact test",
        "-c",
        "user.email=artifact@example.invalid",
        "commit",
        "-qm",
        "upstream",
    )
    revision = git("rev-parse", "HEAD")
    (repository / "src/lib.rs").write_text("dirty upstream must not enter artifact\n")
    data = json.loads(spec.read_bytes())
    data["sources"][0]["upstream"] = {
        "kind": "git",
        "repository": str(repository),
        "url": "https://example.invalid/upstream.git",
        "revision": revision,
    }
    _write_json(spec, data)
    package(root, spec)
    manifest, _ = _installed(root)
    assert manifest["sources"][0]["upstream"]["origin"]["revision"] == revision
    verify(root, replay=True)


@pytest.mark.parametrize("fault", ["source", "manifest"])
def test_cargo_router_rejects_tampering_before_invoking_rustup(
    tmp_path: Path, fault: str
) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    actual_root = Path(__file__).resolve().parents[2]
    for relative in ["scripts/cargo", "tooling/ci/native_dependency_artifacts.py"]:
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(actual_root / relative, target)
    marker = tmp_path / "rustup-called"
    fake_home = tmp_path / "fake-home"
    rustup = fake_home / ".cargo/bin/rustup"
    rustup.parent.mkdir(parents=True)
    rustup.write_text(f"#!/bin/sh\ntouch '{marker}'\nexit 0\n")
    rustup.chmod(0o755)
    _, artifact = _installed(root)
    if fault == "source":
        (artifact / "sources/example/src/lib.rs").write_text("tampered")
    else:
        (root / MANIFEST).unlink()
    result = subprocess.run(
        [str(root / "scripts/cargo"), "metadata"],
        cwd=root,
        env={**os.environ, "HOME": str(fake_home)},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "native dependency artifact rejected" in result.stderr
    assert not marker.exists()


def test_external_staged_symlink_is_rejected(tmp_path: Path) -> None:
    _, _, amended = _fixture(tmp_path)
    (amended / "external").symlink_to(tmp_path)
    with pytest.raises(NativeArtifactError, match="external symlink"):
        census(amended)


@pytest.mark.parametrize("installed", [False, True])
def test_cargo_router_preserves_baseline_and_runs_verified_artifact(
    tmp_path: Path, installed: bool
) -> None:
    root, spec, _ = _fixture(tmp_path)
    if installed:
        package(root, spec)
    actual_root = Path(__file__).resolve().parents[2]
    for relative in ["scripts/cargo", "tooling/ci/native_dependency_artifacts.py"]:
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(actual_root / relative, target)
    (root / "tooling/rust-tool-versions.env").write_text(
        "CODEFABRIC_STABLE_TOOLCHAIN=test-stable\nCODEFABRIC_ASSURANCE_TOOLCHAIN=test-nightly\n"
    )
    marker = tmp_path / "cargo-called"
    fake_home = tmp_path / "fake-home"
    rustup = fake_home / ".cargo/bin/rustup"
    rustup.parent.mkdir(parents=True)
    rustup.write_text(
        f"#!/bin/sh\ncase \"$1\" in\nwhich) echo /bin/true;;\nrun) touch '{marker}';;\nesac\n"
    )
    rustup.chmod(0o755)
    result = subprocess.run(
        [str(root / "scripts/cargo"), "metadata"],
        cwd=root,
        env={**os.environ, "HOME": str(fake_home)},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert marker.exists()


def test_symlinked_installed_manifest_is_rejected_even_if_internal(
    tmp_path: Path,
) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    saved = (root / MANIFEST).with_name("saved.json")
    (root / MANIFEST).rename(saved)
    (root / MANIFEST).symlink_to(saved.name)
    with pytest.raises(NativeArtifactError, match="manifest must not be a symlink"):
        verify(root)


def test_missing_manifest_is_not_an_uninstalled_baseline(tmp_path: Path) -> None:
    root, spec, _ = _fixture(tmp_path)
    package(root, spec)
    (root / MANIFEST).unlink()
    with pytest.raises(NativeArtifactError, match="manifest is missing"):
        verify(root, if_installed=True)


@pytest.mark.parametrize("manifest", ["Cargo.toml", "pyrefly-sidecar/Cargo.toml"])
def test_selected_native_path_requires_manifest_even_without_artifact(
    tmp_path: Path, manifest: str
) -> None:
    root, _, _ = _fixture(tmp_path)
    path = root / manifest
    path.parent.mkdir(parents=True, exist_ok=True)
    prefix = "../" if path.parent != root else ""
    path.write_text(
        '[patch.crates-io]\nexample = { path = "'
        + prefix
        + 'third_party/native/missing/sources/example" }\n'
    )
    with pytest.raises(NativeArtifactError, match="manifest is missing"):
        verify(root, if_installed=True)


@pytest.mark.parametrize("path", ["../../mutable", "/tmp/mutable"])
def test_reproducible_patch_cannot_introduce_external_cargo_source(
    tmp_path: Path, path: str
) -> None:
    root, spec, amended = _fixture(tmp_path)
    manifest = amended / "Cargo.toml"
    original = manifest.read_text()
    changed = (
        original + f"\n[target.'cfg(unix)'.dependencies]\nx = {{ path = \"{path}\" }}\n"
    )
    manifest.write_text(changed)
    patch = spec.parent / "change.patch"
    patch.write_text(
        patch.read_text()
        + "".join(
            difflib.unified_diff(
                original.splitlines(keepends=True),
                changed.splitlines(keepends=True),
                fromfile="a/Cargo.toml",
                tofile="b/Cargo.toml",
            )
        )
    )
    with pytest.raises(
        NativeArtifactError, match="external symlink or path|absolute native Cargo"
    ):
        package(root, spec)
    assert not (root / MANIFEST).exists()


@pytest.mark.parametrize(
    "fault",
    [
        "expression",
        "checksum",
        "missing-records",
        "no-primary",
        "origin",
        "duplicate",
        "omitted-notice",
    ],
)
def test_license_identity_and_complete_notice_census_fail_closed(
    tmp_path: Path, fault: str
) -> None:
    root, spec, _ = _fixture(tmp_path)
    data = json.loads(spec.read_bytes())
    identity = data["sources"][0]["packages"][0]
    if fault == "expression":
        identity["license"] = "MIT"
    elif fault == "checksum":
        identity["license_files"][0]["sha256"] = "0" * 64
    elif fault == "missing-records":
        identity["license_files"] = []
    elif fault == "no-primary":
        identity["license_files"][0]["role"] = "notice"
    elif fault == "origin":
        identity["license_files"][0]["origin"] = "/private/staging/LICENSE"
    elif fault == "duplicate":
        identity["license_files"].append(identity["license_files"][0].copy())
    else:
        identity["license_files"].pop()
    _write_json(spec, data)
    with pytest.raises(NativeArtifactError, match="license|notice"):
        package(root, spec)
    assert not (root / MANIFEST).exists()


def test_workspace_inherited_license_is_checked_against_actual_manifest(
    tmp_path: Path,
) -> None:
    from tooling.ci.native_dependency_artifacts import _package_identity

    _, spec, source = _fixture(tmp_path)
    identity = json.loads(spec.read_bytes())["sources"][0]["packages"][0]
    (source / "Cargo.toml").write_text(
        '[workspace.package]\nversion="1.2.3"\nlicense="Apache-2.0"\n'
    )
    (source / "member").mkdir()
    (source / "member/Cargo.toml").write_text(
        '[package]\nname="example"\nversion.workspace=true\nlicense.workspace=true\n'
    )
    identity["manifest"] = "member/Cargo.toml"
    _package_identity(source, identity)
    identity["license"] = "MIT"
    with pytest.raises(NativeArtifactError, match="license identity mismatch"):
        _package_identity(source, identity)


def test_license_supplement_requires_explicit_origin_and_ordered_patch(
    tmp_path: Path,
) -> None:
    root, spec, source = _fixture(tmp_path)
    data = json.loads(spec.read_bytes())
    extra = "Copyright example upstream authors\n"
    (source / "NOTICE").write_text(extra)
    record = {
        "path": "NOTICE",
        "role": "notice",
        "sha256": hashlib.sha256(extra.encode()).hexdigest(),
        "origin": "upstream",
    }
    data["sources"][0]["packages"][0]["license_files"].append(record)
    (spec.parent / "notice.patch").write_text(
        "diff --git a/NOTICE b/NOTICE\nnew file mode 100644\n--- /dev/null\n+++ b/NOTICE\n@@ -0,0 +1 @@\n+"
        + extra
    )
    data["sources"][0]["patches"].append("notice.patch")
    _write_json(spec, data)
    with pytest.raises(NativeArtifactError, match="claimed as upstream"):
        package(root, spec)
    record["origin"] = "https://example.invalid/exact-commit/NOTICE"
    _write_json(spec, data)
    package(root, spec)
    verify(root, replay=True)
    manifest, _ = _installed(root)
    assert manifest["sources"][0]["packages"][0]["license_files"][-1] == record
