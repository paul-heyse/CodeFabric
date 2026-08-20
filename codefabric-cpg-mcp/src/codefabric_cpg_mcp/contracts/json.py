"""The Python implementation of the ``codefabric-jcs-v1`` boundary."""

import base64
import binascii
import json as stdlib_json
import math
from collections.abc import Iterable
from typing import Any

import blake3
import rfc8785

PROFILE = "codefabric-jcs-v1"
MAX_SAFE_INTEGER = 9_007_199_254_740_991

type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]


class CanonicalJsonError(ValueError):
    """A value violated RFC 8785 or a CodeFabric profile restriction."""

    failure_class: str

    def __init__(self, failure_class: str, message: str) -> None:
        self.failure_class = failure_class
        super().__init__(message)


def _reject_constant(value: str) -> Any:
    raise CanonicalJsonError("invalid-json-number", f"non-finite JSON number: {value}")


def _parse_integer(value: str) -> int:
    parsed = int(value)
    if not -MAX_SAFE_INTEGER <= parsed <= MAX_SAFE_INTEGER:
        raise CanonicalJsonError(
            "integer-range", f"integer outside the interoperable JSON range: {value}"
        )
    return parsed


def _parse_float(value: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise CanonicalJsonError(
            "finite-number", f"non-finite or unrepresentable JSON number: {value}"
        )
    return parsed


def _object_without_duplicates(pairs: list[tuple[str, JsonValue]]) -> dict[str, JsonValue]:
    result: dict[str, JsonValue] = {}
    for key, value in pairs:
        if key in result:
            raise CanonicalJsonError("duplicate-key", f"duplicate object key: {key}")
        result[key] = value
    return result


def _decode(source: str | bytes) -> JsonValue:
    if isinstance(source, bytes):
        if source.startswith(b"\xef\xbb\xbf"):
            raise CanonicalJsonError("invalid-json", "UTF-8 BOM is prohibited")
    elif source.startswith("\ufeff"):
        raise CanonicalJsonError("invalid-json", "UTF-8 BOM is prohibited")
    try:
        return stdlib_json.loads(
            source,
            object_pairs_hook=_object_without_duplicates,
            parse_int=_parse_integer,
            parse_float=_parse_float,
            parse_constant=_reject_constant,
        )
    except CanonicalJsonError:
        raise
    except (UnicodeDecodeError, stdlib_json.JSONDecodeError) as error:
        raise CanonicalJsonError("invalid-json", f"invalid JSON: {error}") from error


def _validate_value(value: JsonValue) -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if not -MAX_SAFE_INTEGER <= value <= MAX_SAFE_INTEGER:
            raise CanonicalJsonError(
                "integer-range", f"integer outside the interoperable JSON range: {value}"
            )
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise CanonicalJsonError("finite-number", f"non-finite JSON number: {value}")
        return
    if isinstance(value, list):
        for item in value:
            _validate_value(item)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise CanonicalJsonError(
                    "non-string-object-key", "JSON object keys must be strings"
                )
            _validate_value(item)
        return
    raise CanonicalJsonError(
        "unsupported-json-value", f"unsupported JSON value: {type(value).__name__}"
    )


def canonicalize_json(source: str | bytes) -> bytes:
    """Decode with duplicate detection and return canonical UTF-8 bytes."""

    return canonicalize_value(_decode(source))


def canonicalize_value(value: JsonValue) -> bytes:
    """Return canonical UTF-8 bytes for a validated JSON-domain value."""

    _validate_value(value)
    try:
        canonical = rfc8785.dumps(value)
    except (rfc8785.CanonicalizationError, UnicodeEncodeError) as error:
        raise CanonicalJsonError(
            "serialization", f"RFC 8785 serialization failed: {error}"
        ) from error

    # RFC 8785 renders through binary64. A fractional input can round to an integer
    # token, so validate the emitted token domain and keep the profile closed under
    # canonicalization exactly as the Rust boundary does.
    _decode(canonical)
    return canonical


def checksum(canonical_bytes: bytes) -> str:
    """Return the AC-G-53 BLAKE3-256 checksum form."""

    return f"b3:{blake3.blake3(canonical_bytes).hexdigest()}"


def validate_checksum(value: str) -> None:
    """Validate the exact AC-G-53 BLAKE3-256 checksum frame."""

    prefix, separator, digest = value.partition(":")
    if (
        prefix != "b3"
        or separator != ":"
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
    ):
        raise CanonicalJsonError(
            "codefabric-checksum", f"invalid codefabric-checksum value: {value}"
        )


def validate_int64(value: str) -> None:
    """Validate a canonical signed 64-bit decimal string."""

    try:
        parsed = int(value)
    except ValueError as error:
        raise CanonicalJsonError(
            "codefabric-int64", f"invalid codefabric-int64 value: {value}"
        ) from error
    if str(parsed) != value or not -(2**63) <= parsed < 2**63:
        raise CanonicalJsonError("codefabric-int64", f"invalid codefabric-int64 value: {value}")


def validate_uint64(value: str) -> None:
    """Validate a canonical unsigned 64-bit decimal string."""

    try:
        parsed = int(value)
    except ValueError as error:
        raise CanonicalJsonError(
            "codefabric-uint64", f"invalid codefabric-uint64 value: {value}"
        ) from error
    if str(parsed) != value or not 0 <= parsed < 2**64:
        raise CanonicalJsonError("codefabric-uint64", f"invalid codefabric-uint64 value: {value}")


def validate_bytes(value: str) -> None:
    """Validate canonical unpadded base64url text."""

    if any(
        character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        for character in value
    ):
        raise CanonicalJsonError("codefabric-bytes", f"invalid codefabric-bytes value: {value}")
    padding = (-len(value)) % 4
    try:
        decoded = base64.b64decode(value + "=" * padding, altchars=b"-_", validate=True)
    except (ValueError, binascii.Error) as error:
        raise CanonicalJsonError(
            "codefabric-bytes", f"invalid codefabric-bytes value: {value}"
        ) from error
    canonical = base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii")
    if canonical != value:
        raise CanonicalJsonError("codefabric-bytes", f"invalid codefabric-bytes value: {value}")


def validate_lowercase_public(value: str) -> None:
    """Reject uppercase or non-ASCII public-ID text."""

    if not value.isascii() or any(character.isupper() for character in value):
        raise CanonicalJsonError("lowercase-public-id", f"invalid lowercase public ID: {value}")


def non_string_map_records(
    entries: Iterable[tuple[JsonValue, JsonValue]],
) -> list[JsonValue]:
    """Encode a non-string-keyed map as records sorted by canonical key bytes."""

    keyed = sorted((canonicalize_value(key), key, value) for key, value in entries)
    for left, right in zip(keyed, keyed[1:], strict=False):
        if left[0] == right[0]:
            raise CanonicalJsonError(
                "duplicate-canonical-key", "duplicate canonical non-string map key"
            )
    records: list[JsonValue] = []
    for _, key, value in keyed:
        records.append({"key": key, "value": value})
    return records
