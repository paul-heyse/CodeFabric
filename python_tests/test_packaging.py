"""Package and import-surface smoke checks (spec section 19).

These stay meaningful when run against an installed wheel, which is what
``scripts/wheel_test.sh`` does -- a development install proves nothing about packaging
(spec sections 44.2 and 62.3).
"""

import importlib.util
from pathlib import Path

import codefabric


def test_public_exports_are_curated() -> None:
    """The package root exposes only names intentionally supported here (spec 5.2)."""
    assert set(codefabric.__all__) == {
        "__version__",
        "main",
        "normalize_workspace_id",
    }
    for name in codefabric.__all__:
        assert hasattr(codefabric, name), name


def test_py_typed_marker_ships_with_the_package() -> None:
    """PEP 561 marker must be present, or downstream type checkers ignore us."""
    package_root = Path(codefabric.__file__).parent
    assert (package_root / "py.typed").is_file()


def test_native_extension_is_importable_and_compiled() -> None:
    """The private extension exists as a real compiled module, not a Python shim."""
    spec = importlib.util.find_spec("codefabric._native")
    assert spec is not None
    assert spec.origin is not None
    assert Path(spec.origin).suffix in {".so", ".pyd", ".dylib"}


def test_native_version_matches_the_facade() -> None:
    from codefabric import _native

    assert _native.version() == codefabric.__version__
