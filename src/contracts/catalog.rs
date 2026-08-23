//! Closed wire enums shared by handwritten contract models.
//!
//! Artifact discovery, dependency planning, output ownership, and generation live in the
//! repository model compiler. This module deliberately contains no catalog path, loader, graph,
//! derivation manifest, or writer.

use serde::{Deserialize, Serialize};

/// Public release state carried by machine-readable contract authorities.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactStatus {
    Draft,
    Released,
    Deprecated,
}

/// Closed source-kind codes retained by shared contract records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    NormativeDocument,
    Manifest,
    JsonSchema,
    JsonLines,
    Registry,
    YamlContract,
    EbnfGrammar,
    ProtobufSchema,
    BundleManifest,
}

/// Versioned semantic digest projection named by source records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestProjection {
    ProseUtf8V1,
    JsonJcsV1,
    #[serde(rename = "yaml-ac-g-53-v1")]
    YamlAcG53V1,
    JsonlJcsV1,
    ProtoDescriptorV1,
    EbnfSourceV1,
    #[serde(rename = "bundle-ac-g-07-v1")]
    BundleAcG07V1,
}

/// Permanent owner of a contract meaning.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractOwner {
    Suite,
    Ontology,
    FactGeneration,
    DataFabric,
    Lifecycle,
    SemanticQuery,
    Serving,
    Roadmap,
}

/// Compatibility bundle families.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleKind {
    Derivation,
    ModelPack,
    Ontology,
    Provider,
    QueryLanguage,
    Schema,
    ToolContract,
    Toolchain,
}

impl BundleKind {
    /// Stable artifact-ID and file-name component.
    #[must_use]
    pub const fn artifact_slug(self) -> &'static str {
        match self {
            Self::Derivation => "derivation",
            Self::ModelPack => "model-pack",
            Self::Ontology => "ontology",
            Self::Provider => "provider",
            Self::QueryLanguage => "query-language",
            Self::Schema => "schema",
            Self::ToolContract => "tool-contract",
            Self::Toolchain => "toolchain",
        }
    }

    /// All built-in bundle families in canonical order.
    pub const ALL: [Self; 8] = [
        Self::Derivation,
        Self::ModelPack,
        Self::Ontology,
        Self::Provider,
        Self::QueryLanguage,
        Self::Schema,
        Self::ToolContract,
        Self::Toolchain,
    ];
}
