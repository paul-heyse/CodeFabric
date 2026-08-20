"""CodeFabric: present-state code property graph.

This package is the supported Python contract. It is an *interface layer* over the Rust
core: argument handling, typing, and presentation of Rust results and errors belong here,
but domain behavior does not (spec sections 5.3 and 61.4). Implementation semantics have a
single source, in Rust.

``codefabric._native`` is a private implementation detail whose symbol layout may change
as bindings evolve; import from this package instead (spec sections 5.1 and 61.5).

How this module is divided into files is a free choice -- the repository specification
declines to prescribe it (spec sections 2 and 5.1).
"""

from codefabric import _native

__all__ = [
    "__version__",
    "main",
    "normalize_workspace_id",
]

#: Version of the compiled Rust core backing this package.
__version__: str = _native.version()


def normalize_workspace_id(raw: str) -> str:
    """Normalize a workspace identifier by trimming surrounding whitespace.

    Args:
        raw: The identifier to normalize.

    Returns:
        The identifier with leading and trailing whitespace removed.

    Raises:
        ValueError: If ``raw`` is empty or contains only whitespace.
    """
    return _native.normalize_workspace_id(raw)


def main() -> None:
    """Entry point for the ``codefabric`` console script."""
    print(f"CodeFabric {__version__}")
