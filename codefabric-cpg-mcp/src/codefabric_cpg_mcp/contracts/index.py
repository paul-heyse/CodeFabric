"""Typed, cached access to the canonical model-derived artifact index."""

from __future__ import annotations

from functools import lru_cache
from importlib.resources import files
from pathlib import PurePosixPath
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, StringConstraints, TypeAdapter

from .json import canonicalize_json, checksum

type Checksum = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]
type SourceRole = Literal["acceptance", "authority", "derived", "evidence-authority"]
type ReleaseStatus = Literal["released", "unreleased"]


class _ClosedModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True, strict=True)


class NamedResourceProfile(_ClosedModel):
    """A model-family resource policy selected by stable profile name."""

    profile: str = Field(min_length=1)


class ExternalDriverBudget(_ClosedModel):
    """Closed bounded external-driver budget embedded in the model index."""

    max_source_bytes: int = Field(gt=0)
    max_output_bytes: int = Field(gt=0)
    max_outputs: int = Field(gt=0)


type ResourceProfile = NamedResourceProfile | ExternalDriverBudget


class ModelArtifactRecord(_ClosedModel):
    """One model-discovered authority, acceptance, evidence, or derived resource."""

    artifact_id: str = Field(min_length=1)
    artifact_kind: str = Field(min_length=1)
    authority_path: str = Field(min_length=1)
    canonical_digest: Checksum
    compatible_suite_major: Literal[1]
    compilation_unit: str = Field(min_length=1)
    owner: str = Field(min_length=1)
    projection_profile: str = Field(min_length=1)
    provenance: tuple[str, ...] = Field(min_length=1)
    release_status: ReleaseStatus
    resource_profile: ResourceProfile
    source_digest: Checksum
    source_role: SourceRole
    status: str = Field(min_length=1)
    version: str = Field(min_length=1)


class ModelOutputProjection(_ClosedModel):
    """Closed projection metadata for one generated output."""

    artifact_kind: str | None = None
    projection_kind: Literal["canonical-artifact", "json-schema", "python-source", "rust-source"]
    public_identity: str | None = None


class ModelOutputRecord(_ClosedModel):
    """One complete DesiredTree output and its real producer/consumer edges."""

    consumers: tuple[str, ...] = Field(min_length=1)
    lineage: tuple[str, ...] = Field(min_length=1)
    output_id: str = Field(min_length=1)
    path: str = Field(min_length=1)
    producer: str = Field(min_length=1)
    projection: ModelOutputProjection
    public_artifact_id: str | None
    resource_profile: ResourceProfile
    validators: tuple[str, ...] = Field(min_length=1)


class ModelArtifactIndex(_ClosedModel):
    """Canonical packaged projection of the compiled RepositoryModel."""

    schema_version: Literal[1]
    source: Literal["RepositoryModel + accepted release census + complete DesiredTree census"]
    artifacts: tuple[ModelArtifactRecord, ...]
    outputs: tuple[ModelOutputRecord, ...]


_MODEL_INDEX_ADAPTER = TypeAdapter(ModelArtifactIndex)


def _safe_relative_path(value: str) -> bool:
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and "\\" not in value
        and all(part not in {"", "."} for part in path.parts)
    )


@lru_cache(maxsize=1)
def model_artifact_index_bytes() -> bytes:
    """Read the exact package resource bytes once."""

    return files(__package__).joinpath("model_artifact_index.json").read_bytes()


@lru_cache(maxsize=1)
def model_artifact_index() -> ModelArtifactIndex:
    """Validate canonical bytes and decode the closed model once."""

    resource = model_artifact_index_bytes()
    if canonicalize_json(resource) != resource:
        raise ValueError("model artifact index resource is not canonical JSON")
    index = _MODEL_INDEX_ADAPTER.validate_json(resource, strict=True)
    artifact_ids = tuple(record.artifact_id for record in index.artifacts)
    if artifact_ids != tuple(sorted(set(artifact_ids))):
        raise ValueError("model artifact index IDs are not unique and strictly sorted")
    paths = tuple(record.authority_path for record in index.artifacts)
    if any(not _safe_relative_path(path) for path in paths):
        raise ValueError("model artifact index contains an unsafe authority path")
    output_ids = tuple(record.output_id for record in index.outputs)
    if len(set(output_ids)) != len(output_ids):
        raise ValueError("model artifact index output IDs are not unique")
    output_paths = tuple(record.path for record in index.outputs)
    if output_paths != tuple(sorted(set(output_paths))) or any(
        not _safe_relative_path(path) for path in output_paths
    ):
        raise ValueError("model artifact index output paths are unsafe or unsorted")
    return index


@lru_cache(maxsize=1)
def model_artifact_index_digest() -> str:
    """Return the BLAKE3 identity of the exact package resource bytes."""

    return checksum(model_artifact_index_bytes())
