"""Type stub for the private native extension.

Hand-maintained (spec section 8). Native runtime signatures alone are not the public
typing contract: keeping a stub for the raw FFI surface separate from the annotated
public facade lets the Python API stay richer and more stable than the bindings.

This describes ``codefabric._native``, which is private. The supported typed surface is
``codefabric`` itself.
"""

def version() -> str: ...
def normalize_workspace_id(raw: str) -> str: ...
