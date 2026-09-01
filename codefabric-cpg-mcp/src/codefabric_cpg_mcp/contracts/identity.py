"""CBEF-v1 identity, public-ID, type-term, and workspace-path primitives."""

from __future__ import annotations

import base64
import struct
import unicodedata
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from enum import IntEnum

import blake3

_MAGIC = b"CFID"
_FORMAT_VERSION = 1
SOURCE_CONTEXT_ID = b"\xff" * 16


class IdentityDomain(IntEnum):
    """Append-only CBEF public-identity domain allocation."""

    WORKSPACE = 1
    REPOSITORY = 2
    WORKTREE = 3
    ANALYSIS_CONTEXT = 4
    CONTEXT_SET = 5
    SOURCE_FILE = 6
    OWNER = 7
    ENTITY = 8
    RELATION_FACT = 9
    PROPERTY_FACT = 10
    TYPE = 11
    PUBLICATION = 12
    SERVING_SNAPSHOT = 13
    RESULT_ARTIFACT = 14
    SOURCE_CONTEXT = 15
    UNKNOWN_REMAINDER = 16
    ROOT_AUTHORIZATION = 17
    PATH_RESULT = 18
    OBJECTIVE_INPUT_SET = 19
    OBJECTIVE_GROUP = 20
    QUERY_SOURCE_CONTEXT = 21
    ACCESS_SCOPE = 22
    RESULT_ARTIFACT_V2 = 23


class TypeCode(IntEnum):
    """Append-only CBEF value-type allocation."""

    ABSENT = 0
    BYTES = 1
    UTF8 = 2
    RAW_PATH = 3
    UNSIGNED = 4
    SIGNED = 5
    BOOLEAN = 6
    ID = 7
    DIGEST = 8
    ORDERED_LIST = 9
    SET = 10
    MAP = 11
    TAGGED_UNION = 12


class IdentityError(ValueError):
    """A CBEF, public-ID, type-term, or canonical-path contract violation."""


@dataclass(frozen=True, slots=True)
class CbefValue:
    """One typed value at the CBEF boundary."""

    type_code: TypeCode
    value: object = None
    normalization: str = "NONE"
    platform_code: int | None = None
    variant: int | None = None


@dataclass(frozen=True, slots=True)
class CbefField:
    """One owner-schema-tagged CBEF field."""

    tag: int
    value: CbefValue


@dataclass(frozen=True, slots=True)
class DerivedIdentity:
    """Truncated ID plus collision-diagnostic evidence."""

    id_bytes: bytes
    full_digest: bytes
    preimage: bytes


@dataclass(frozen=True, slots=True)
class CbefRecord:
    """One completely decoded CBEF-v1 record."""

    domain: IdentityDomain
    fields: tuple[CbefField, ...]


def _u32(value: int) -> bytes:
    try:
        return struct.pack(">I", value)
    except struct.error as error:
        raise IdentityError("value does not fit unsigned 32-bit framing") from error


def _normalize(value: str, rule: str) -> str:
    match rule:
        case "NONE" | "RUST_CANONICAL":
            return value
        case "NFC":
            return unicodedata.normalize("NFC", value)
        case "NFKC" | "PYTHON_IDENTIFIER_NFKC":
            return unicodedata.normalize("NFKC", value)
        case "ASCII_LOWER":
            if not value.isascii():
                raise IdentityError("ASCII_LOWER received non-ASCII text")
            return value.lower()
        case _:
            raise IdentityError(f"unknown normalization rule: {rule}")


def _as_values(value: object) -> tuple[CbefValue, ...]:
    if not isinstance(value, (list, tuple)) or not all(
        isinstance(item, CbefValue) for item in value
    ):
        raise IdentityError("container members must be CbefValue instances")
    return tuple(value)


def _typed(value: CbefValue) -> bytes:
    payload = _payload(value)
    return bytes([value.type_code]) + _u32(len(payload)) + payload


def _payload(value: CbefValue) -> bytes:
    match value.type_code:
        case TypeCode.ABSENT:
            if value.value is not None:
                raise IdentityError("absent payload must be None")
            return b""
        case TypeCode.BYTES:
            return bytes(value.value)  # type: ignore[arg-type]
        case TypeCode.UTF8:
            if not isinstance(value.value, str):
                raise IdentityError("UTF8 payload must be text")
            return _normalize(value.value, value.normalization).encode()
        case TypeCode.RAW_PATH:
            if value.platform_code not in (1, 2, 3):
                raise IdentityError("unsupported raw-path platform code")
            return bytes([value.platform_code]) + bytes(value.value)  # type: ignore[arg-type]
        case TypeCode.UNSIGNED | TypeCode.SIGNED:
            payload = bytes(value.value)  # type: ignore[arg-type]
            if len(payload) not in (1, 2, 4, 8, 16):
                raise IdentityError("integer payload has invalid schema width")
            return payload
        case TypeCode.BOOLEAN:
            if not isinstance(value.value, bool):
                raise IdentityError("boolean payload must be bool")
            return bytes([value.value])
        case TypeCode.ID:
            payload = bytes(value.value)  # type: ignore[arg-type]
            if len(payload) != 16:
                raise IdentityError("ID payload must be 16 bytes")
            return payload
        case TypeCode.DIGEST:
            payload = bytes(value.value)  # type: ignore[arg-type]
            if len(payload) != 32:
                raise IdentityError("digest payload must be 32 bytes")
            return payload
        case TypeCode.ORDERED_LIST | TypeCode.SET:
            members = [_typed(item) for item in _as_values(value.value)]
            if value.type_code is TypeCode.SET:
                members = sorted(set(members))
            return _u32(len(members)) + b"".join(_u32(len(item)) + item for item in members)
        case TypeCode.MAP:
            if not isinstance(value.value, (list, tuple)):
                raise IdentityError("map payload must be a sequence of pairs")
            pairs: list[tuple[bytes, bytes]] = []
            for pair in value.value:
                if (
                    not isinstance(pair, (list, tuple))
                    or len(pair) != 2
                    or not isinstance(pair[0], CbefValue)
                    or not isinstance(pair[1], CbefValue)
                ):
                    raise IdentityError("map entry must contain two typed values")
                pairs.append((_typed(pair[0]), _typed(pair[1])))
            pairs.sort(key=lambda pair: pair[0])
            if any(left[0] == right[0] for left, right in zip(pairs, pairs[1:], strict=False)):
                raise IdentityError("duplicate encoded map key")
            return _u32(len(pairs)) + b"".join(
                _u32(len(key)) + key + _u32(len(item)) + item for key, item in pairs
            )
        case TypeCode.TAGGED_UNION:
            if value.variant is None or not 0 <= value.variant <= 0xFFFF:
                raise IdentityError("tagged union requires a u16 variant")
            if not isinstance(value.value, CbefValue):
                raise IdentityError("tagged union requires a typed value")
            encoded = _typed(value.value)
            return struct.pack(">H", value.variant) + _u32(len(encoded)) + encoded
    raise AssertionError("closed TypeCode match is exhaustive")


def encode_record(domain: IdentityDomain, fields: Sequence[CbefField]) -> bytes:
    """Encode one strictly tagged CBEF-v1 record."""

    if len(fields) > 0xFFFF:
        raise IdentityError("field count exceeds u16")
    if any(left.tag >= right.tag for left, right in zip(fields, fields[1:], strict=False)):
        raise IdentityError("field tags must be unique and ascending")
    encoded = bytearray(_MAGIC)
    encoded.extend(struct.pack(">BHH", _FORMAT_VERSION, domain, len(fields)))
    for field in fields:
        if not 0 < field.tag <= 0xFFFF:
            raise IdentityError("field tag must be a positive u16")
        payload = _payload(field.value)
        encoded.extend(struct.pack(">HB", field.tag, field.value.type_code))
        encoded.extend(_u32(len(payload)))
        encoded.extend(payload)
    return bytes(encoded)


def derive_identity(domain: IdentityDomain, fields: Sequence[CbefField]) -> DerivedIdentity:
    """Derive BLAKE3-256 and the canonical first-16-byte application ID."""

    preimage = encode_record(domain, fields)
    digest = blake3.blake3(preimage).digest()
    return DerivedIdentity(digest[:16], digest, preimage)


class _Reader:
    def __init__(self, value: bytes) -> None:
        self.value = value
        self.cursor = 0

    def take(self, length: int) -> bytes:
        end = self.cursor + length
        if end > len(self.value):
            raise IdentityError("truncated CBEF payload")
        result = self.value[self.cursor : end]
        self.cursor = end
        return result

    def u16(self) -> int:
        return struct.unpack(">H", self.take(2))[0]

    def u32(self) -> int:
        return struct.unpack(">I", self.take(4))[0]

    def finish(self) -> None:
        if self.cursor != len(self.value):
            raise IdentityError("trailing CBEF bytes")


def _decode_typed(encoded: bytes) -> CbefValue:
    reader = _Reader(encoded)
    try:
        type_code = TypeCode(reader.take(1)[0])
    except (IndexError, ValueError) as error:
        raise IdentityError("unknown or truncated CBEF type code") from error
    payload = reader.take(reader.u32())
    reader.finish()
    return _decode_payload(type_code, payload)


def _decode_payload(type_code: TypeCode, payload: bytes) -> CbefValue:
    match type_code:
        case TypeCode.ABSENT:
            if payload:
                raise IdentityError("absent payload must be empty")
            return CbefValue(type_code)
        case TypeCode.BYTES:
            return CbefValue(type_code, payload)
        case TypeCode.UTF8:
            try:
                return CbefValue(type_code, payload.decode())
            except UnicodeDecodeError as error:
                raise IdentityError("invalid UTF-8 payload") from error
        case TypeCode.RAW_PATH:
            if not payload or payload[0] not in (1, 2, 3):
                raise IdentityError("invalid raw-path payload")
            return CbefValue(type_code, payload[1:], platform_code=payload[0])
        case TypeCode.UNSIGNED | TypeCode.SIGNED:
            if len(payload) not in (1, 2, 4, 8, 16):
                raise IdentityError("invalid integer width")
            return CbefValue(type_code, payload)
        case TypeCode.BOOLEAN:
            if payload not in (b"\x00", b"\x01"):
                raise IdentityError("invalid boolean payload")
            return CbefValue(type_code, payload == b"\x01")
        case TypeCode.ID:
            if len(payload) != 16:
                raise IdentityError("invalid ID width")
            return CbefValue(type_code, payload)
        case TypeCode.DIGEST:
            if len(payload) != 32:
                raise IdentityError("invalid digest width")
            return CbefValue(type_code, payload)
        case TypeCode.ORDERED_LIST | TypeCode.SET:
            reader = _Reader(payload)
            encoded = [reader.take(reader.u32()) for _ in range(reader.u32())]
            reader.finish()
            if type_code is TypeCode.SET and any(
                left >= right for left, right in zip(encoded, encoded[1:], strict=False)
            ):
                raise IdentityError("set payload is not sorted and unique")
            return CbefValue(type_code, tuple(_decode_typed(item) for item in encoded))
        case TypeCode.MAP:
            reader = _Reader(payload)
            encoded_pairs = [
                (reader.take(reader.u32()), reader.take(reader.u32())) for _ in range(reader.u32())
            ]
            reader.finish()
            if any(
                left[0] >= right[0]
                for left, right in zip(encoded_pairs, encoded_pairs[1:], strict=False)
            ):
                raise IdentityError("map keys are not sorted and unique")
            return CbefValue(
                type_code,
                tuple((_decode_typed(key), _decode_typed(item)) for key, item in encoded_pairs),
            )
        case TypeCode.TAGGED_UNION:
            reader = _Reader(payload)
            variant = reader.u16()
            item = _decode_typed(reader.take(reader.u32()))
            reader.finish()
            return CbefValue(type_code, item, variant=variant)
    raise AssertionError("closed TypeCode match is exhaustive")


def decode_record(encoded: bytes) -> CbefRecord:
    """Decode one complete canonical CBEF-v1 record."""

    reader = _Reader(encoded)
    if reader.take(4) != _MAGIC or reader.take(1) != bytes([_FORMAT_VERSION]):
        raise IdentityError("invalid CBEF magic or version")
    try:
        domain = IdentityDomain(reader.u16())
    except ValueError as error:
        raise IdentityError("unknown CBEF domain") from error
    fields: list[CbefField] = []
    previous = 0
    for _ in range(reader.u16()):
        tag = reader.u16()
        if tag <= previous:
            raise IdentityError("field tags must be unique and ascending")
        try:
            type_code = TypeCode(reader.take(1)[0])
        except (IndexError, ValueError) as error:
            raise IdentityError("unknown or truncated CBEF type code") from error
        fields.append(CbefField(tag, _decode_payload(type_code, reader.take(reader.u32()))))
        previous = tag
    reader.finish()
    return CbefRecord(domain, tuple(fields))


_PREFIXES = {
    IdentityDomain.WORKSPACE: "workspace",
    IdentityDomain.REPOSITORY: "repository",
    IdentityDomain.WORKTREE: "worktree",
    IdentityDomain.ANALYSIS_CONTEXT: "context",
    IdentityDomain.CONTEXT_SET: "context-set",
    IdentityDomain.SOURCE_FILE: "file",
    IdentityDomain.OWNER: "owner",
    IdentityDomain.ENTITY: "entity",
    IdentityDomain.RELATION_FACT: "fact",
    IdentityDomain.PROPERTY_FACT: "fact",
    IdentityDomain.TYPE: "type",
    IdentityDomain.PUBLICATION: "publication",
    IdentityDomain.SERVING_SNAPSHOT: "snapshot",
    IdentityDomain.RESULT_ARTIFACT: "artifact",
    IdentityDomain.SOURCE_CONTEXT: "source-context",
    IdentityDomain.UNKNOWN_REMAINDER: "unknown",
}
_SLUG_DOMAINS = {
    IdentityDomain.ENTITY,
    IdentityDomain.RELATION_FACT,
    IdentityDomain.PROPERTY_FACT,
}


def _valid_slug(value: str) -> bool:
    return (
        bool(value)
        and not value.startswith("-")
        and not value.endswith("-")
        and "--" not in value
        and all(
            character.isascii() and (character.islower() or character.isdigit() or character == "-")
            for character in value
        )
    )


def encode_public_id(domain: IdentityDomain, id_bytes: bytes, kind_slug: str | None = None) -> str:
    """Encode one domain-checked lowercase public ID."""

    if len(id_bytes) != 16 or ((domain in _SLUG_DOMAINS) != (kind_slug is not None)):
        raise IdentityError("public ID domain, slug, or width mismatch")
    if kind_slug is not None and not _valid_slug(kind_slug):
        raise IdentityError("invalid public ID kind slug")
    components = [_PREFIXES[domain]]
    if kind_slug is not None:
        components.append(kind_slug)
    components.append(id_bytes.hex())
    return ":".join(components)


def decode_public_id(domain: IdentityDomain, value: str, kind_slug: str | None = None) -> bytes:
    """Decode only a value with the expected domain and kind slug."""

    if value == "context:source":
        if domain is IdentityDomain.ANALYSIS_CONTEXT and kind_slug is None:
            return SOURCE_CONTEXT_ID
        raise IdentityError("symbolic source context used for the wrong domain")
    expected = [_PREFIXES[domain]]
    if domain in _SLUG_DOMAINS:
        if kind_slug is None or not _valid_slug(kind_slug):
            raise IdentityError("kind slug required")
        expected.append(kind_slug)
    elif kind_slug is not None:
        raise IdentityError("kind slug forbidden")
    parts = value.split(":")
    if parts[:-1] != expected or len(parts[-1]) != 32:
        raise IdentityError("public ID prefix or width mismatch")
    if any(character not in "0123456789abcdef" for character in parts[-1]):
        raise IdentityError("public ID payload is not lowercase hexadecimal")
    return bytes.fromhex(parts[-1])


@dataclass(frozen=True, slots=True)
class WorkspacePath:
    """The authoritative, canonical, comparison, display, and URI path views."""

    workspace_id: bytes
    platform_code: int
    raw_relative_path_bytes: bytes
    canonical_component_bytes: bytes
    comparison_key_bytes: bytes
    case_sensitivity_mode: str
    display_string: str
    display_is_lossy: bool

    @classmethod
    def from_components(
        cls,
        workspace_id: bytes,
        platform_code: int,
        case_sensitivity_mode: str,
        components: Sequence[bytes],
    ) -> WorkspacePath:
        """Construct all views without consulting or resolving symlinks."""

        if len(workspace_id) != 16 or platform_code not in (1, 2):
            raise IdentityError("unsupported workspace or platform identity")
        if case_sensitivity_mode not in ("sensitive", "insensitive"):
            raise IdentityError("unknown case-sensitivity mode")
        raw = b"/".join(components)
        canonical = b"/".join(_encode_component(component) for component in components)
        lossy = any(_decode_utf8(component) is None for component in components)
        display = "/".join(
            text
            if (text := _decode_utf8(component)) is not None
            else _encode_component(component).decode()
            for component in components
        )
        if platform_code == 2 and case_sensitivity_mode == "insensitive" and not lossy:
            comparison = (
                unicodedata.normalize(
                    "NFD", "/".join(component.decode() for component in components)
                )
                .casefold()
                .encode()
            )
        else:
            comparison = raw
        return cls(
            workspace_id,
            platform_code,
            raw,
            canonical,
            comparison,
            case_sensitivity_mode,
            display,
            lossy,
        )

    def decoded_components(self) -> tuple[bytes, ...]:
        """Reverse the canonical component encoding."""

        decoded = tuple(
            _decode_component(component) for component in self.canonical_component_bytes.split(b"/")
        )
        if (
            b"/".join(_encode_component(component) for component in decoded)
            != self.canonical_component_bytes
        ):
            raise IdentityError("noncanonical component encoding")
        return decoded

    def canonical_uri(self) -> str:
        """Return the exact AC-G-18 canonical URI."""

        payload = base64.urlsafe_b64encode(self.raw_relative_path_bytes).rstrip(b"=").decode()
        return f"codefabric://workspace/{self.workspace_id.hex()}/path/{payload}"

    def ordering_key(self) -> tuple[bytes, bytes]:
        """Return the total deterministic path ordering key."""

        return self.comparison_key_bytes, self.raw_relative_path_bytes


def _decode_utf8(value: bytes) -> str | None:
    try:
        return value.decode()
    except UnicodeDecodeError:
        return None


def _encode_component(component: bytes) -> bytes:
    text = _decode_utf8(component)
    encoded = bytearray()
    for byte in component:
        if (
            byte in (ord("/"), ord("%"))
            or byte < 32
            or byte == 127
            or (text is None and byte >= 128)
        ):
            encoded.extend(f"%{byte:02X}".encode())
        else:
            encoded.append(byte)
    return bytes(encoded)


def _decode_component(component: bytes) -> bytes:
    decoded = bytearray()
    cursor = 0
    while cursor < len(component):
        if component[cursor] == ord("%"):
            pair = component[cursor + 1 : cursor + 3]
            if len(pair) != 2 or any(byte not in b"0123456789ABCDEF" for byte in pair):
                raise IdentityError("invalid canonical percent escape")
            decoded.append(int(pair, 16))
            cursor += 3
        else:
            decoded.append(component[cursor])
            cursor += 1
    return bytes(decoded)


def validate_workspace_paths(paths: Iterable[WorkspacePath]) -> None:
    """Reject two unequal raw paths collapsing to one comparison key."""

    seen: dict[bytes, bytes] = {}
    for path in paths:
        previous = seen.setdefault(path.comparison_key_bytes, path.raw_relative_path_bytes)
        if previous != path.raw_relative_path_bytes:
            raise IdentityError("BLOCKED_PATH_COLLISION")


def canonical_type_term(constructor_code: int, fields: Sequence[CbefField]) -> bytes:
    """Encode one versioned type constructor through the shared CBEF model."""

    if not 1 <= constructor_code <= 35:
        raise IdentityError("unknown type constructor")
    normalized = list(fields)
    if constructor_code in (10, 11):
        normalized = [
            CbefField(field.tag, CbefValue(TypeCode.SET, field.value.value))
            if field.value.type_code is TypeCode.ORDERED_LIST
            else field
            for field in fields
        ]
    return (
        b"\x01"
        + struct.pack(">H", constructor_code)
        + encode_record(IdentityDomain.TYPE, normalized)
    )


__all__ = [
    "SOURCE_CONTEXT_ID",
    "CbefField",
    "CbefRecord",
    "CbefValue",
    "DerivedIdentity",
    "IdentityDomain",
    "IdentityError",
    "TypeCode",
    "WorkspacePath",
    "canonical_type_term",
    "decode_public_id",
    "decode_record",
    "derive_identity",
    "encode_public_id",
    "encode_record",
    "validate_workspace_paths",
]
