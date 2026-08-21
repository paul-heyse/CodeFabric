"""Typed, cached access to the canonical packaged contract artifact index."""

from functools import lru_cache
from importlib.resources import files
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, StringConstraints, TypeAdapter

from .json import PROFILE, canonicalize_json, checksum

type Checksum = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]
type ArtifactKind = Literal[
    "bundle-manifest",
    "ebnf-grammar",
    "json-lines",
    "json-schema",
    "manifest",
    "normative-document",
    "protobuf-schema",
    "registry",
    "yaml-contract",
]
type ArtifactStatus = Literal["draft", "released", "deprecated"]
type CompatibilityFamily = Literal[
    "adapter",
    "bundle",
    "conformance",
    "deployment",
    "identity",
    "lifecycle",
    "model-pack",
    "ontology",
    "provider",
    "query",
    "rpc",
    "schema",
    "suite",
    "toolchain",
    "traceability",
]
type ConsumerDomain = Literal[
    "rust-core",
    "python-adapter",
    "rustc-extractor",
    "pyrefly-sidecar",
    "contract-tooling",
    "packaging",
    "governance",
]
type ContractOwner = Literal[
    "suite",
    "ontology",
    "fact-generation",
    "data-fabric",
    "semantic-query",
    "lifecycle",
    "serving",
    "roadmap",
]
type DigestProjection = Literal[
    "bundle-ac-g-07-v1",
    "ebnf-source-v1",
    "json-jcs-v1",
    "jsonl-jcs-v1",
    "prose-utf8-v1",
    "proto-descriptor-v1",
    "yaml-ac-g-53-v1",
]
type DerivationOutputKind = Literal[
    "artifact-index",
    "canonical-registry",
    "python-registry-bindings",
    "proto-descriptor-set",
    "proto-descriptor-census",
    "proto-toolchain-identity",
    "rust-proto-bindings",
    "rust-registry-bindings",
    "python-proto-bindings",
    "python-proto-stub",
    "python-grpc-bindings",
    "python-adapter-models",
    "adapter-schema-manifest",
    "adapter-fingerprint-manifest",
]
type DerivationKind = Literal[
    "artifact-index",
    "canonical-registry-set",
    "protobuf-descriptor-and-python",
    "protobuf-rust-from-descriptor",
    "adapter-model-compilation",
]
type ArtifactInputView = Literal["source-bytes", "compiled-semantic"]
type ProvenanceRequirement = Literal[
    "source-digest",
    "canonical-digest",
    "generator-revision",
    "owner-acceptance",
    "native-validation",
]


class _ClosedModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True, strict=True)


class ArtifactIndexGeneration(_ClosedModel):
    """Generator provenance for the canonical index resource."""

    catalog_artifact_id: str
    artifact_count: int = Field(ge=0)
    derivation_count: int = Field(ge=0)
    generator_revision: str
    profile: Literal["codefabric-jcs-v1"]


class OutputRef(_ClosedModel):
    """Stable reference to one derivation output."""

    derivation_id: str
    path: str


class NativeSemanticProjection(_ClosedModel):
    """A semantic projection compiled from native authority bytes."""

    source_kind: Literal["native"]


class DerivedSemanticProjection(_ClosedModel):
    """A semantic projection compiled from a derivation output."""

    source_kind: Literal["derivation-output"]
    output: OutputRef


type SemanticProjectionSource = Annotated[
    NativeSemanticProjection | DerivedSemanticProjection,
    Field(discriminator="source_kind"),
]


class ArtifactDerivationInput(_ClosedModel):
    """One governed artifact input."""

    input_kind: Literal["artifact"]
    artifact_id: str
    view: ArtifactInputView


class OutputDerivationInput(_ClosedModel):
    """One upstream derivation-output input."""

    input_kind: Literal["output"]
    output: OutputRef


class AllCompiledArtifactsInput(_ClosedModel):
    """The closed set of all compiled governed artifacts."""

    input_kind: Literal["all-compiled-artifacts"]


type DerivationInput = Annotated[
    ArtifactDerivationInput | OutputDerivationInput | AllCompiledArtifactsInput,
    Field(discriminator="input_kind"),
]


class ArtifactIndexOutput(_ClosedModel):
    """One derivation-owned generated output edge."""

    path: str
    output_kind: DerivationOutputKind
    primary_artifact_ids: tuple[str, ...]
    consumers: tuple[ConsumerDomain, ...]
    resource_budget_profile: str | None


class GeneratorIdentity(_ClosedModel):
    """Maintained generator identity derived from a closed unit kind."""

    generator_id: str
    generator_revision: str
    toolchain: tuple[str, ...]


class ArtifactIndexRecord(_ClosedModel):
    """One fully compiled governed-source identity and consumer view."""

    artifact_id: str
    authority_path: str
    artifact_kind: ArtifactKind
    owner: ContractOwner
    version: str
    compatible_suite_major: int = Field(ge=0)
    status: ArtifactStatus
    digest_projection: DigestProjection
    semantic_projection_source: SemanticProjectionSource
    canonical_digest: Checksum
    source_digest: Checksum
    bundle_digest: Checksum | None
    compatibility_family: CompatibilityFamily
    provenance_requirements: tuple[ProvenanceRequirement, ...]
    consumers: tuple[ConsumerDomain, ...]


class DerivationIndexRecord(_ClosedModel):
    """One peer derivation record and its transitive governed lineage."""

    derivation_id: str
    derivation_kind: DerivationKind
    inputs: tuple[DerivationInput, ...]
    outputs: tuple[ArtifactIndexOutput, ...]
    lineage_artifact_ids: tuple[str, ...]
    resource_budget_profile: str
    generator: GeneratorIdentity


class ArtifactIndex(_ClosedModel):
    """The one canonical shared artifact-index document."""

    generated: ArtifactIndexGeneration = Field(alias="_generated")
    artifacts: tuple[ArtifactIndexRecord, ...]
    derivations: tuple[DerivationIndexRecord, ...]


_ARTIFACT_INDEX_ADAPTER = TypeAdapter(ArtifactIndex)


@lru_cache(maxsize=1)
def artifact_index_bytes() -> bytes:
    """Read the exact package resource bytes once."""

    return files(__package__).joinpath("artifact-index.json").read_bytes()


@lru_cache(maxsize=1)
def artifact_index() -> ArtifactIndex:
    """Validate canonical bytes and decode the closed model once."""

    resource = artifact_index_bytes()
    if canonicalize_json(resource) != resource:
        raise ValueError("artifact index resource is not canonical JSON")
    index = _ARTIFACT_INDEX_ADAPTER.validate_json(resource, strict=True)
    if index.generated.profile != PROFILE:
        raise ValueError("artifact index canonicalization profile drifted")
    if index.generated.artifact_count != len(index.artifacts):
        raise ValueError("artifact index count disagrees with its records")
    artifact_ids = tuple(record.artifact_id for record in index.artifacts)
    if artifact_ids != tuple(sorted(set(artifact_ids))):
        raise ValueError("artifact index IDs are not unique and strictly sorted")
    if index.generated.derivation_count != len(index.derivations):
        raise ValueError("artifact index derivation count disagrees with its records")
    derivation_ids = tuple(record.derivation_id for record in index.derivations)
    if derivation_ids != tuple(sorted(set(derivation_ids))):
        raise ValueError("artifact index derivation IDs are not unique and sorted")
    return index


@lru_cache(maxsize=1)
def artifact_index_digest() -> str:
    """Return the BLAKE3 identity of the exact package resource bytes."""

    return checksum(artifact_index_bytes())
