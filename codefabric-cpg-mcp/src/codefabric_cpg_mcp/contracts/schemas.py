"""Cached access to generated Pydantic schema and fingerprint resources."""

from functools import lru_cache
from importlib.resources import files
from typing import Any, cast

from .json import canonicalize_json, canonicalize_value, checksum, validate_checksum
from .wire_models import JSON_OBJECT_ADAPTER


def _resource(name: str) -> bytes:
    return files(__package__).joinpath(name).read_bytes()


@lru_cache(maxsize=1)
def schema_manifest() -> dict[str, Any]:
    """Decode and validate the one canonical schema manifest once."""

    resource = _resource("adapter-schemas.json")
    if canonicalize_json(resource) != resource:
        raise ValueError("adapter schema manifest is not canonical JSON")
    return cast(dict[str, Any], JSON_OBJECT_ADAPTER.validate_json(resource))


@lru_cache(maxsize=1)
def schema_fingerprints() -> dict[str, Any]:
    """Decode and verify every named validation/serialization fingerprint once."""

    resource = _resource("adapter-fingerprints.json")
    if canonicalize_json(resource) != resource:
        raise ValueError("adapter fingerprint manifest is not canonical JSON")
    manifest = JSON_OBJECT_ADAPTER.validate_json(resource)
    schemas = schema_manifest()
    for mode in ("validation", "serialization"):
        mode_schemas = schemas[mode]
        mode_fingerprints = manifest[mode]
        if not isinstance(mode_schemas, dict) or not isinstance(mode_fingerprints, dict):
            raise ValueError(f"adapter {mode} schema view is not an object")
        if set(mode_schemas) != set(mode_fingerprints):
            raise ValueError(f"adapter {mode} fingerprint census drifted")
        for name, schema in mode_schemas.items():
            expected = mode_fingerprints[name]
            if not isinstance(expected, str):
                raise ValueError(f"adapter {mode} fingerprint is not a string: {name}")
            validate_checksum(expected)
            if checksum(canonicalize_value(schema)) != expected:
                raise ValueError(f"adapter {mode} schema fingerprint drifted: {name}")
    return cast(dict[str, Any], manifest)
