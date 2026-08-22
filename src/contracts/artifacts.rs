//! Contract-tree generation and verification for the Wave 1 machine authority.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tempfile::NamedTempFile;
use thiserror::Error;

#[cfg(feature = "fact-generation")]
use ruff_python_ast::{NodeKind, token::TokenKind};

#[cfg(feature = "fact-generation")]
use super::catalog::PROVIDER_RAW_DERIVATION_ID;
use super::catalog::{
    ARTIFACT_INDEX_DERIVATION_ID, BundleKind, CatalogError, CompiledCatalog, ContractCatalog,
    DerivationInput, DerivationOutputKind, REGISTRY_DERIVATION_ID, ResolvedDerivationInvocation,
    SemanticProjectionSource, generator_identity,
};
use super::compiler::{ContractCompileError, compile_artifact, compile_artifact_for_generation};
use super::index::{
    ArtifactIndex, ArtifactIndexGeneration, ArtifactIndexOutput, ArtifactIndexRecord,
    DerivationIndexRecord,
};
use super::jcs::{
    CanonicalJsonError, PROFILE, canonicalize_slice, canonicalize_value, checksum, decode_strict,
    non_string_map_records, validate_bytes, validate_checksum, validate_int64,
    validate_lowercase_public, validate_uint64,
};
use super::models::{BrokenTraceEdgeFixture, RequirementRecord, TraceSelector, TraceabilityRecord};
use super::registry_models::{
    AcceptedRegistry, Capability, EntityKind, EnumDomain, FactKind, FlagDomain, PhraseRecord,
    PropertyKind, Provider, ProviderHardStopPolicy, ProviderResourceProfile, ProviderRetryPolicy,
    PublicError, RelationKind, StateMachine, UnknownKind, validate_duplicate_authorities,
};
#[cfg(feature = "fact-generation")]
use super::registry_models::{ProviderNormalization, RawKindDisposition};
use super::schema_artifacts::render_schema_outputs;
use super::schema_models::SchemaContractIr;

/// Verifier strictness profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationProfile {
    /// Validate drafts and report warnings without failing solely on draft status.
    Full,
    /// Require every artifact to be released and every warning to be resolved.
    Released,
}

impl VerificationProfile {
    /// Parse the stable CLI profile spelling.
    ///
    /// # Errors
    ///
    /// Returns an error for a profile other than full or released.
    pub fn parse(value: &str) -> Result<Self, ContractArtifactError> {
        match value {
            "full" => Ok(Self::Full),
            "released" => Ok(Self::Released),
            _ => Err(ContractArtifactError::UnknownProfile(value.to_owned())),
        }
    }
}

/// Successful verification evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// Number of required source artifacts checked.
    pub artifact_count: usize,
    /// Number of draft-status warnings.
    pub warning_count: usize,
}

/// Contract generation or verification failure.
#[derive(Debug, Error)]
pub enum ContractArtifactError {
    /// The typed catalog could not be loaded or compiled.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// Native bounded compilation failed.
    #[error(transparent)]
    Compile(#[from] ContractCompileError),
    /// A required path is absent.
    #[error("required contract path is absent: {0}")]
    Missing(PathBuf),
    /// A source artifact lacks its AC-G-02 metadata markers.
    #[error("artifact metadata is incomplete: {0}")]
    Metadata(PathBuf),
    /// A generated output differs from the deterministic rendering.
    #[error("generated contract output drifted: {0}")]
    Drift(PathBuf),
    /// Filesystem access failed.
    #[error("contract filesystem operation failed for {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON or canonicalization rejected an artifact.
    #[error("canonical artifact failure for {path}: {source}")]
    Canonical {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: CanonicalJsonError,
    },
    /// A fixture document has the wrong shape.
    #[error("invalid verification fixture {path}: {message}")]
    Fixture {
        /// Affected path.
        path: PathBuf,
        /// Shape or expectation error.
        message: String,
    },
    /// Requirement and traceability records are orphaned or malformed.
    #[error("traceability failure for {path}: {message}")]
    Traceability {
        /// Affected manifest path.
        path: PathBuf,
        /// Failed structural obligation.
        message: String,
    },
    /// The released profile encountered unresolved warnings.
    #[error("released profile has {0} unresolved draft artifact warnings")]
    ReleasedWarnings(usize),
    /// The requested verifier profile is unknown.
    #[error("unknown verification profile: {0}")]
    UnknownProfile(String),
}

fn read(path: &Path) -> Result<Vec<u8>, ContractArtifactError> {
    fs::read(path).map_err(|source| ContractArtifactError::Io {
        path: path.to_owned(),
        source,
    })
}

fn fixture_failure(path: &Path, message: impl Into<String>) -> ContractArtifactError {
    ContractArtifactError::Fixture {
        path: path.to_owned(),
        message: message.into(),
    }
}

fn collect_artifact_records(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<ArtifactIndexRecord>, ContractArtifactError> {
    let mut records = Vec::new();
    for artifact in catalog.artifacts() {
        let compiled = compile_artifact(repository_root, catalog, artifact)?;
        records.push(ArtifactIndexRecord {
            artifact_id: artifact.artifact_id.clone(),
            authority_path: artifact.authority_path.clone(),
            artifact_kind: artifact.artifact_kind,
            owner: artifact.owner,
            version: artifact.version.clone(),
            compatible_suite_major: artifact.compatible_suite_major,
            status: artifact.status,
            digest_projection: artifact.digest_projection,
            semantic_projection_source: artifact.semantic_projection_source.clone(),
            canonical_digest: compiled.canonical_digest,
            source_digest: compiled.source_digest,
            bundle_digest: compiled.bundle_digest,
            compatibility_family: artifact.compatibility_family,
            provenance_requirements: artifact.provenance_requirements.iter().copied().collect(),
            consumers: artifact.consumers.iter().copied().collect(),
        });
    }
    Ok(records)
}

fn collect_lineage(
    catalog: &CompiledCatalog,
    derivation_id: &str,
    lineage: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(derivation_id.to_owned()) {
        return;
    }
    let derivation = catalog
        .derivation(derivation_id)
        .expect("compiled derivation reference is valid");
    for input in &derivation.inputs {
        match input {
            DerivationInput::Artifact { artifact_id, .. } => {
                lineage.insert(artifact_id.clone());
                if let Some(artifact) = catalog.artifact(artifact_id)
                    && let SemanticProjectionSource::DerivationOutput { output } =
                        &artifact.semantic_projection_source
                {
                    collect_lineage(catalog, &output.derivation_id, lineage, visited);
                }
            }
            DerivationInput::Output { output } => {
                collect_lineage(catalog, &output.derivation_id, lineage, visited);
            }
            DerivationInput::AllCompiledArtifacts => {
                lineage.extend(
                    catalog
                        .artifacts()
                        .map(|artifact| artifact.artifact_id.clone()),
                );
            }
        }
    }
}

fn collect_derivation_records(catalog: &CompiledCatalog) -> Vec<DerivationIndexRecord> {
    catalog
        .derivations()
        .map(|derivation| {
            let mut lineage = BTreeSet::new();
            collect_lineage(
                catalog,
                &derivation.derivation_id,
                &mut lineage,
                &mut BTreeSet::new(),
            );
            DerivationIndexRecord {
                derivation_id: derivation.derivation_id.clone(),
                derivation_kind: derivation.derivation_kind,
                inputs: derivation.inputs.clone(),
                outputs: derivation
                    .outputs
                    .iter()
                    .map(|output| ArtifactIndexOutput {
                        path: output.path.clone(),
                        output_kind: output.output_kind,
                        primary_artifact_ids: output.primary_artifact_ids.iter().cloned().collect(),
                        consumers: output.consumers.iter().copied().collect(),
                        resource_budget_profile: output.resource_budget_profile.clone(),
                    })
                    .collect(),
                lineage_artifact_ids: lineage.into_iter().collect(),
                resource_budget_profile: derivation.resource_budget_profile.clone(),
                generator: generator_identity(derivation.derivation_kind),
            }
        })
        .collect()
}

fn render_registry_outputs(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    outputs: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), ContractArtifactError> {
    for (output_path, output) in catalog.outputs_of_kind(
        REGISTRY_DERIVATION_ID,
        DerivationOutputKind::CanonicalRegistry,
    ) {
        if output.primary_artifact_ids.len() != 1 {
            return Err(ContractArtifactError::Metadata(output_path.to_owned()));
        }
        let owner = output
            .primary_artifact_ids
            .iter()
            .next()
            .expect("length was checked");
        let artifact = catalog
            .artifact(owner)
            .expect("compiled primary artifact must exist");
        let compiled = compile_artifact(repository_root, catalog, artifact)?;
        outputs.insert(output_path.to_owned(), compiled.canonical_bytes);
    }
    Ok(())
}

#[cfg(feature = "fact-generation")]
#[derive(Serialize)]
struct ProviderRawInputIdentity {
    artifact_id: String,
    canonical_digest: String,
    source_digest: String,
}

#[cfg(feature = "fact-generation")]
#[derive(Serialize)]
struct ProviderRawKind {
    raw_key: String,
    raw_kind_id: u16,
    raw_name: String,
    named: bool,
    visible: bool,
    supertype: bool,
    subtypes: Vec<String>,
    disposition: &'static str,
    canonical_kind_name: Option<String>,
}

#[cfg(feature = "fact-generation")]
fn provider_raw_input_identities(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<ProviderRawInputIdentity>, ContractArtifactError> {
    let derivation = catalog
        .derivation(PROVIDER_RAW_DERIVATION_ID)
        .expect("validated provider raw derivation exists");
    derivation
        .inputs
        .iter()
        .map(|input| {
            let DerivationInput::Artifact { artifact_id, .. } = input else {
                return Err(ContractArtifactError::Metadata(PathBuf::from(
                    PROVIDER_RAW_DERIVATION_ID,
                )));
            };
            let artifact = catalog
                .artifact(artifact_id)
                .expect("validated provider raw input exists");
            let compiled = compile_artifact(repository_root, catalog, artifact)?;
            Ok(ProviderRawInputIdentity {
                artifact_id: artifact_id.clone(),
                canonical_digest: compiled.canonical_digest,
                source_digest: compiled.source_digest,
            })
        })
        .collect()
}

#[cfg(feature = "fact-generation")]
const TREE_SITTER_RECOVERY_QUERY: &str = "(ERROR) @error\n(MISSING) @missing\n";

#[cfg(feature = "fact-generation")]
macro_rules! define_ruff_inventory {
    ($kind:ident, $all:ident, $name:ident, [$($variant:ident),+ $(,)?]) => {
        const $all: &[$kind] = &[$($kind::$variant),+];

        const fn $name(value: $kind) -> &'static str {
            match value {
                $($kind::$variant => stringify!($variant)),+
            }
        }
    };
}

#[cfg(feature = "fact-generation")]
define_ruff_inventory!(
    NodeKind,
    RUFF_NODE_KINDS,
    ruff_node_kind_name,
    [
        ModModule,
        ModExpression,
        StmtFunctionDef,
        StmtClassDef,
        StmtReturn,
        StmtDelete,
        StmtTypeAlias,
        StmtAssign,
        StmtAugAssign,
        StmtAnnAssign,
        StmtFor,
        StmtWhile,
        StmtIf,
        StmtWith,
        StmtMatch,
        StmtRaise,
        StmtTry,
        StmtAssert,
        StmtImport,
        StmtImportFrom,
        StmtGlobal,
        StmtNonlocal,
        StmtExpr,
        StmtPass,
        StmtBreak,
        StmtContinue,
        StmtIpyEscapeCommand,
        ExprBoolOp,
        ExprNamed,
        ExprBinOp,
        ExprUnaryOp,
        ExprLambda,
        ExprIf,
        ExprDict,
        ExprSet,
        ExprListComp,
        ExprSetComp,
        ExprDictComp,
        ExprGenerator,
        ExprAwait,
        ExprYield,
        ExprYieldFrom,
        ExprCompare,
        ExprCall,
        ExprFString,
        ExprTString,
        ExprStringLiteral,
        ExprBytesLiteral,
        ExprNumberLiteral,
        ExprBooleanLiteral,
        ExprNoneLiteral,
        ExprEllipsisLiteral,
        ExprAttribute,
        ExprSubscript,
        ExprStarred,
        ExprName,
        ExprList,
        ExprTuple,
        ExprSlice,
        ExprIpyEscapeCommand,
        ExceptHandlerExceptHandler,
        InterpolatedElement,
        InterpolatedStringLiteralElement,
        PatternMatchValue,
        PatternMatchSingleton,
        PatternMatchSequence,
        PatternMatchMapping,
        PatternMatchClass,
        PatternMatchStar,
        PatternMatchAs,
        PatternMatchOr,
        TypeParamTypeVar,
        TypeParamTypeVarTuple,
        TypeParamParamSpec,
        InterpolatedStringFormatSpec,
        PatternArguments,
        PatternKeyword,
        Comprehension,
        Arguments,
        Parameters,
        Parameter,
        ParameterWithDefault,
        Keyword,
        Alias,
        WithItem,
        MatchCase,
        Decorator,
        ElifElseClause,
        TypeParams,
        FString,
        TString,
        StringLiteral,
        BytesLiteral,
        Identifier,
    ]
);

#[cfg(feature = "fact-generation")]
define_ruff_inventory!(
    TokenKind,
    RUFF_TOKEN_KINDS,
    ruff_token_kind_name,
    [
        Name,
        Int,
        Float,
        Complex,
        String,
        FStringStart,
        FStringMiddle,
        FStringEnd,
        TStringStart,
        TStringMiddle,
        TStringEnd,
        IpyEscapeCommand,
        Comment,
        Newline,
        NonLogicalNewline,
        Indent,
        Dedent,
        EndOfFile,
        Question,
        Exclamation,
        Lpar,
        Rpar,
        Lsqb,
        Rsqb,
        Colon,
        Comma,
        Semi,
        Plus,
        Minus,
        Star,
        Slash,
        Vbar,
        Amper,
        Less,
        Greater,
        Equal,
        Dot,
        Percent,
        Lbrace,
        Rbrace,
        EqEqual,
        NotEqual,
        LessEqual,
        GreaterEqual,
        Tilde,
        CircumFlex,
        LeftShift,
        RightShift,
        DoubleStar,
        DoubleStarEqual,
        PlusEqual,
        MinusEqual,
        StarEqual,
        SlashEqual,
        PercentEqual,
        AmperEqual,
        VbarEqual,
        CircumflexEqual,
        LeftShiftEqual,
        RightShiftEqual,
        DoubleSlash,
        DoubleSlashEqual,
        ColonEqual,
        At,
        AtEqual,
        Rarrow,
        Ellipsis,
        And,
        As,
        Assert,
        Async,
        Await,
        Break,
        Class,
        Continue,
        Def,
        Del,
        Elif,
        Else,
        Except,
        False,
        Finally,
        For,
        From,
        Global,
        If,
        Import,
        In,
        Is,
        Lambda,
        None,
        Nonlocal,
        Not,
        Or,
        Pass,
        Raise,
        Return,
        True,
        Try,
        While,
        With,
        Yield,
        Case,
        Lazy,
        Match,
        Type,
        Unknown,
    ]
);

#[cfg(feature = "fact-generation")]
#[allow(clippy::too_many_lines)] // One pass keeps grammar inventory and disposition expansion atomic.
fn render_provider_raw_catalog(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    catalog_id: &str,
    provider_version: &str,
    language_name: &str,
    language: &tree_sitter::Language,
    node_types_source: &str,
) -> Result<Vec<u8>, ContractArtifactError> {
    if language.node_kind_count() > usize::from(u16::MAX)
        || language.field_count() > usize::from(u16::MAX)
    {
        return Err(ContractArtifactError::Metadata(PathBuf::from(format!(
            "{catalog_id} grammar inventory exceeds the Tree-sitter u16 identifier contract"
        ))));
    }
    let normalization_value = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.provider-normalization-registry",
    )?;
    let normalizations: Vec<ProviderNormalization> =
        serde_json::from_value(normalization_value["records"].clone())
            .map_err(|_| ContractArtifactError::Metadata(PathBuf::from(catalog_id)))?;
    let normalization = normalizations
        .into_iter()
        .find(|record| record.raw_catalog_id == catalog_id)
        .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from(catalog_id)))?;

    let runtime_kind_ids = (0..language.node_kind_count())
        .map(|raw_kind_id| u16::try_from(raw_kind_id).expect("count was bounded"))
        .chain([u16::MAX - 1, u16::MAX])
        .collect::<Vec<_>>();
    let available_names = runtime_kind_ids
        .iter()
        .filter_map(|raw_kind_id| language.node_kind_for_id(*raw_kind_id).map(str::to_owned))
        .collect::<BTreeSet<_>>();
    if normalization
        .canonical_kind_names
        .keys()
        .chain(&normalization.ignored_raw_keys)
        .any(|raw_name| !available_names.contains(raw_name))
    {
        return Err(ContractArtifactError::Metadata(PathBuf::from(format!(
            "{catalog_id} normalization references an absent raw kind"
        ))));
    }

    let supertype_ids = language
        .supertypes()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut raw_kinds = Vec::with_capacity(runtime_kind_ids.len());
    for raw_kind_id in runtime_kind_ids {
        let Some(raw_name) = language.node_kind_for_id(raw_kind_id) else {
            continue;
        };
        let canonical_kind_name = normalization.canonical_kind_names.get(raw_name).cloned();
        let disposition = if canonical_kind_name.is_some() {
            "normalize"
        } else if normalization.ignored_raw_keys.contains(raw_name) {
            "ignore"
        } else {
            match normalization.default_disposition {
                RawKindDisposition::Ignore => "ignore",
                RawKindDisposition::Unsupported => "unsupported",
            }
        };
        let subtypes = if supertype_ids.contains(&raw_kind_id) {
            language
                .subtypes_for_supertype(raw_kind_id)
                .iter()
                .filter_map(|id| language.node_kind_for_id(*id))
                .map(str::to_owned)
                .collect()
        } else {
            Vec::new()
        };
        raw_kinds.push(ProviderRawKind {
            raw_key: format!(
                "{raw_kind_id}:{raw_name}:{}",
                language.node_kind_is_named(raw_kind_id)
            ),
            raw_kind_id,
            raw_name: raw_name.to_owned(),
            named: language.node_kind_is_named(raw_kind_id),
            visible: language.node_kind_is_visible(raw_kind_id),
            supertype: language.node_kind_is_supertype(raw_kind_id),
            subtypes,
            disposition,
            canonical_kind_name,
        });
    }
    let fields = (1..=language.field_count())
        .filter_map(|field_id| {
            language
                .field_name_for_id(u16::try_from(field_id).expect("count was bounded"))
                .map(|name| serde_json::json!({"field_id": field_id, "field_name": name}))
        })
        .collect::<Vec<_>>();
    let runtime_inventory = serde_json::json!({"raw_kinds": raw_kinds, "fields": fields});
    let runtime_bytes = canonicalize_value(&runtime_inventory).map_err(|source| {
        ContractArtifactError::Canonical {
            path: PathBuf::from(catalog_id),
            source,
        }
    })?;
    let node_types: Value = serde_json::from_str(node_types_source)
        .map_err(|_| ContractArtifactError::Metadata(PathBuf::from(catalog_id)))?;

    let input_identities = provider_raw_input_identities(repository_root, catalog)?;
    let output = serde_json::json!({
        "catalog_id": catalog_id,
        "provider_id": normalization.provider_id,
        "provider_version": provider_version,
        "language": language_name,
        "grammar_abi": language.abi_version(),
        "node_types_digest": checksum(node_types_source.as_bytes()),
        "runtime_inventory_fingerprint": checksum(&runtime_bytes),
        "query_bundle_digest": checksum(
            &canonicalize_value(&serde_json::json!({
                "queries": [{"query_id": "recovery", "source": TREE_SITTER_RECOVERY_QUERY}]
            }))
            .expect("static query bundle is canonicalizable"),
        ),
        "input_identities": input_identities,
        "node_types": node_types,
        "runtime_inventory": runtime_inventory,
    });
    canonicalize_value(&output).map_err(|source| ContractArtifactError::Canonical {
        path: PathBuf::from(catalog_id),
        source,
    })
}

#[cfg(feature = "fact-generation")]
#[derive(Serialize)]
struct RuffRawNodeKind {
    raw_kind_id: u16,
    raw_name: &'static str,
    disposition: &'static str,
    canonical_kind_name: Option<String>,
}

#[cfg(feature = "fact-generation")]
#[derive(Serialize)]
struct RuffRawTokenKind {
    raw_kind_id: u16,
    raw_name: &'static str,
}

#[cfg(feature = "fact-generation")]
fn ruff_normalization(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<ProviderNormalization, ContractArtifactError> {
    let value = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.provider-normalization-registry",
    )?;
    serde_json::from_value::<Vec<ProviderNormalization>>(value["records"].clone())
        .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("ruff-python-0-0-7")))?
        .into_iter()
        .find(|record| record.raw_catalog_id == "ruff-python-0-0-7")
        .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from("ruff-python-0-0-7")))
}

#[cfg(feature = "fact-generation")]
fn canonical_kind_for_raw_name(
    normalization: &ProviderNormalization,
    raw_name: &str,
) -> Result<Option<String>, ContractArtifactError> {
    if let Some(name) = normalization.canonical_kind_names.get(raw_name) {
        return Ok(Some(name.clone()));
    }
    if normalization.ignored_raw_keys.contains(raw_name) {
        return Ok(None);
    }
    let mut prefix_matches = normalization
        .canonical_kind_prefixes
        .iter()
        .filter(|(prefix, _)| raw_name.starts_with(prefix.as_str()));
    let prefix_match = prefix_matches.next().map(|(_, name)| name.clone());
    if prefix_matches.next().is_some() {
        return Err(ContractArtifactError::Metadata(PathBuf::from(format!(
            "{} has ambiguous prefix normalization for {raw_name}",
            normalization.mapping_id
        ))));
    }
    Ok(prefix_match.or_else(|| normalization.default_canonical_kind_name.clone()))
}

#[cfg(feature = "fact-generation")]
fn render_ruff_provider_raw_catalog(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<u8>, ContractArtifactError> {
    let catalog_id = "ruff-python-0-0-7";
    let normalization = ruff_normalization(repository_root, catalog)?;
    let available_names = RUFF_NODE_KINDS
        .iter()
        .copied()
        .map(ruff_node_kind_name)
        .collect::<BTreeSet<_>>();
    if normalization
        .canonical_kind_names
        .keys()
        .chain(&normalization.ignored_raw_keys)
        .any(|raw_name| !available_names.contains(raw_name.as_str()))
        || normalization.canonical_kind_prefixes.keys().any(|prefix| {
            !available_names
                .iter()
                .any(|raw_name| raw_name.starts_with(prefix))
        })
    {
        return Err(ContractArtifactError::Metadata(PathBuf::from(
            "ruff normalization references an absent NodeKind",
        )));
    }
    let node_kinds = RUFF_NODE_KINDS
        .iter()
        .copied()
        .enumerate()
        .map(|(raw_kind_id, kind)| {
            let raw_name = ruff_node_kind_name(kind);
            let canonical_kind_name = canonical_kind_for_raw_name(&normalization, raw_name)?;
            let disposition = if canonical_kind_name.is_some() {
                "normalize"
            } else if normalization.ignored_raw_keys.contains(raw_name) {
                "ignore"
            } else {
                match normalization.default_disposition {
                    RawKindDisposition::Ignore => "ignore",
                    RawKindDisposition::Unsupported => "unsupported",
                }
            };
            Ok(RuffRawNodeKind {
                raw_kind_id: u16::try_from(raw_kind_id).map_err(|_| {
                    ContractArtifactError::Metadata(PathBuf::from(
                        "Ruff NodeKind inventory exceeds u16",
                    ))
                })?,
                raw_name,
                disposition,
                canonical_kind_name,
            })
        })
        .collect::<Result<Vec<_>, ContractArtifactError>>()?;
    let token_kinds = RUFF_TOKEN_KINDS
        .iter()
        .copied()
        .enumerate()
        .map(|(raw_kind_id, kind)| {
            Ok(RuffRawTokenKind {
                raw_kind_id: u16::try_from(raw_kind_id).map_err(|_| {
                    ContractArtifactError::Metadata(PathBuf::from(
                        "Ruff TokenKind inventory exceeds u16",
                    ))
                })?,
                raw_name: ruff_token_kind_name(kind),
            })
        })
        .collect::<Result<Vec<_>, ContractArtifactError>>()?;
    let runtime_inventory = serde_json::json!({
        "node_kinds": node_kinds,
        "token_kinds": token_kinds,
    });
    let runtime_bytes = canonicalize_value(&runtime_inventory).map_err(|source| {
        ContractArtifactError::Canonical {
            path: PathBuf::from(catalog_id),
            source,
        }
    })?;
    canonicalize_value(&serde_json::json!({
        "catalog_id": catalog_id,
        "catalog_kind": "ruff-python-frontend",
        "provider_id": normalization.provider_id,
        "provider_version": normalization.provider_version,
        "language": normalization.language,
        "runtime_inventory_fingerprint": checksum(&runtime_bytes),
        "input_identities": provider_raw_input_identities(repository_root, catalog)?,
        "runtime_inventory": runtime_inventory,
    }))
    .map_err(|source| ContractArtifactError::Canonical {
        path: PathBuf::from(catalog_id),
        source,
    })
}

#[cfg(feature = "fact-generation")]
fn render_provider_raw_outputs(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    outputs: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), ContractArtifactError> {
    for (path, _) in catalog.outputs_of_kind(
        PROVIDER_RAW_DERIVATION_ID,
        DerivationOutputKind::ProviderRawKindCatalog,
    ) {
        let catalog_id = path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ContractArtifactError::Metadata(path.to_owned()))?;
        let bytes = match catalog_id {
            "tree-sitter-python-0-25-0" => render_provider_raw_catalog(
                repository_root,
                catalog,
                catalog_id,
                "tree-sitter=0.26.12;tree-sitter-python=0.25.0",
                "python",
                &tree_sitter::Language::from(tree_sitter_python::LANGUAGE),
                tree_sitter_python::NODE_TYPES,
            )?,
            "tree-sitter-rust-0-24-2" => render_provider_raw_catalog(
                repository_root,
                catalog,
                catalog_id,
                "tree-sitter=0.26.12;tree-sitter-rust=0.24.2",
                "rust",
                &tree_sitter::Language::from(tree_sitter_rust::LANGUAGE),
                tree_sitter_rust::NODE_TYPES,
            )?,
            "ruff-python-0-0-7" => render_ruff_provider_raw_catalog(repository_root, catalog)?,
            _ => return Err(ContractArtifactError::Metadata(path.to_owned())),
        };
        outputs.insert(path.to_owned(), bytes);
    }
    for (path, _) in catalog.outputs_of_kind(
        PROVIDER_RAW_DERIVATION_ID,
        DerivationOutputKind::RustProviderRawKindBindings,
    ) {
        outputs.insert(
            path.to_owned(),
            render_rust_provider_raw_kind_bindings(repository_root, catalog)?,
        );
    }
    Ok(())
}

#[cfg(feature = "fact-generation")]
#[allow(clippy::too_many_lines)] // One renderer keeps both grammar catalogs and the hot-path view byte-identical.
fn render_rust_provider_raw_kind_bindings(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<u8>, ContractArtifactError> {
    let enum_registry = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.enum-registry",
    )?;
    let syntax_kind_codes =
        serde_json::from_value::<Vec<EnumDomain>>(enum_registry["records"].clone())
            .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("enum-registry")))?
            .into_iter()
            .find(|domain| domain.domain == "SYNTAX_KIND")
            .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("SYNTAX_KIND")))?
            .values
            .into_iter()
            .map(|value| (value.name, value.code))
            .collect::<BTreeMap<_, _>>();
    let fallback_code = syntax_kind_codes
        .get("SYNTAX_NODE")
        .copied()
        .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("SYNTAX_NODE")))?;

    let catalogs = [
        (
            "TREE_SITTER_PYTHON_GRAMMAR",
            "tree-sitter-python-0-25-0",
            "tree-sitter=0.26.12;tree-sitter-python=0.25.0",
            "python",
            tree_sitter::Language::from(tree_sitter_python::LANGUAGE),
            tree_sitter_python::NODE_TYPES,
        ),
        (
            "TREE_SITTER_RUST_GRAMMAR",
            "tree-sitter-rust-0-24-2",
            "tree-sitter=0.26.12;tree-sitter-rust=0.24.2",
            "rust",
            tree_sitter::Language::from(tree_sitter_rust::LANGUAGE),
            tree_sitter_rust::NODE_TYPES,
        ),
    ];
    let mut output = String::from(
        "// @generated by codefabric-contracts; do not edit.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ProviderRawKindDisposition { Normalize, Ignore, Unsupported }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderRawKindEntry {\n\
             pub raw_kind_id: u16, pub raw_name: &'static str,\n\
             pub named: bool, pub visible: bool, pub supertype: bool,\n\
             pub disposition: ProviderRawKindDisposition,\n\
             pub normalized_kind_code: u16,\n\
         }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderRawFieldEntry {\n\
             pub field_id: u16, pub field_name: &'static str,\n\
         }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderGrammarInventory {\n\
             pub catalog_id: &'static str, pub language: &'static str,\n\
             pub provider_version: &'static str, pub grammar_abi: usize,\n\
             pub node_types_digest: &'static str,\n\
             pub runtime_inventory_fingerprint: &'static str,\n\
             pub query_bundle_digest: &'static str,\n\
             pub query_bundle_canonical_json: &'static [u8],\n\
             pub raw_kinds: &'static [ProviderRawKindEntry],\n\
             pub fields: &'static [ProviderRawFieldEntry],\n\
         }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffNodeKindEntry {\n\
             pub raw_kind_id: u16, pub raw_name: &'static str,\n\
             pub disposition: ProviderRawKindDisposition,\n\
             pub normalized_kind_code: u16,\n\
         }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffTokenKindEntry {\n\
             pub raw_kind_id: u16, pub raw_name: &'static str,\n\
         }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffPythonInventory {\n\
             pub catalog_id: &'static str, pub provider_version: &'static str,\n\
             pub runtime_inventory_fingerprint: &'static str,\n\
             pub node_kinds: &'static [RuffNodeKindEntry],\n\
             pub token_kinds: &'static [RuffTokenKindEntry],\n\
         }\n\n",
    );
    writeln!(
        output,
        "pub const TREE_SITTER_RECOVERY_QUERY: &str = {TREE_SITTER_RECOVERY_QUERY:?};\n"
    )
    .unwrap();
    for (constant, catalog_id, provider_version, language_name, language, node_types) in catalogs {
        let query_bundle_canonical_json = canonicalize_value(&serde_json::json!({
            "queries": [{"query_id": "recovery", "source": TREE_SITTER_RECOVERY_QUERY}]
        }))
        .expect("static query bundle is canonicalizable");
        let document = decode_strict(&render_provider_raw_catalog(
            repository_root,
            catalog,
            catalog_id,
            provider_version,
            language_name,
            &language,
            node_types,
        )?)
        .map_err(|source| ContractArtifactError::Canonical {
            path: PathBuf::from(catalog_id),
            source,
        })?;
        let kind_constant = format!("{constant}_RAW_KINDS");
        let field_constant = format!("{constant}_FIELDS");
        writeln!(
            output,
            "pub const {kind_constant}: &[ProviderRawKindEntry] = &["
        )
        .unwrap();
        for entry in document["runtime_inventory"]["raw_kinds"]
            .as_array()
            .expect("generated raw kinds are an array")
        {
            let canonical_name = entry["canonical_kind_name"].as_str();
            let normalized_kind_code = canonical_name
                .and_then(|name| syntax_kind_codes.get(name))
                .copied()
                .unwrap_or(fallback_code);
            let disposition = match entry["disposition"].as_str() {
                Some("normalize") => "ProviderRawKindDisposition::Normalize",
                Some("ignore") => "ProviderRawKindDisposition::Ignore",
                Some("unsupported") => "ProviderRawKindDisposition::Unsupported",
                _ => {
                    return Err(ContractArtifactError::Metadata(PathBuf::from(catalog_id)));
                }
            };
            writeln!(
                output,
                "    ProviderRawKindEntry {{ raw_kind_id: {}, raw_name: {:?}, named: {}, visible: {}, supertype: {}, disposition: {disposition}, normalized_kind_code: {normalized_kind_code} }},",
                entry["raw_kind_id"].as_u64().expect("generated u16"),
                entry["raw_name"].as_str().expect("generated raw name"),
                entry["named"].as_bool().expect("generated named flag"),
                entry["visible"].as_bool().expect("generated visible flag"),
                entry["supertype"].as_bool().expect("generated supertype flag"),
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
        writeln!(
            output,
            "pub const {field_constant}: &[ProviderRawFieldEntry] = &["
        )
        .unwrap();
        for entry in document["runtime_inventory"]["fields"]
            .as_array()
            .expect("generated fields are an array")
        {
            writeln!(
                output,
                "    ProviderRawFieldEntry {{ field_id: {}, field_name: {:?} }},",
                entry["field_id"].as_u64().expect("generated u16"),
                entry["field_name"].as_str().expect("generated field name"),
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
        writeln!(
            output,
            "pub const {constant}: ProviderGrammarInventory = ProviderGrammarInventory {{ catalog_id: {catalog_id:?}, language: {language_name:?}, provider_version: {provider_version:?}, grammar_abi: {}, node_types_digest: {:?}, runtime_inventory_fingerprint: {:?}, query_bundle_digest: {:?}, query_bundle_canonical_json: &{:?}, raw_kinds: {kind_constant}, fields: {field_constant} }};\n",
            document["grammar_abi"].as_u64().expect("generated ABI"),
            document["node_types_digest"].as_str().expect("generated digest"),
            document["runtime_inventory_fingerprint"].as_str().expect("generated digest"),
            document["query_bundle_digest"].as_str().expect("generated digest"),
            query_bundle_canonical_json,
        )
        .unwrap();
    }
    let ruff_document = decode_strict(&render_ruff_provider_raw_catalog(repository_root, catalog)?)
        .map_err(|source| ContractArtifactError::Canonical {
            path: PathBuf::from("ruff-python-0-0-7"),
            source,
        })?;
    writeln!(
        output,
        "pub const RUFF_PYTHON_NODE_KINDS: &[RuffNodeKindEntry] = &["
    )
    .unwrap();
    for entry in ruff_document["runtime_inventory"]["node_kinds"]
        .as_array()
        .expect("generated Ruff node kinds are an array")
    {
        let canonical_name = entry["canonical_kind_name"].as_str();
        let normalized_kind_code = canonical_name
            .and_then(|name| syntax_kind_codes.get(name))
            .copied()
            .unwrap_or(fallback_code);
        let disposition = match entry["disposition"].as_str() {
            Some("normalize") => "ProviderRawKindDisposition::Normalize",
            Some("ignore") => "ProviderRawKindDisposition::Ignore",
            Some("unsupported") => "ProviderRawKindDisposition::Unsupported",
            _ => {
                return Err(ContractArtifactError::Metadata(PathBuf::from(
                    "ruff-python-0-0-7",
                )));
            }
        };
        writeln!(
            output,
            "    RuffNodeKindEntry {{ raw_kind_id: {}, raw_name: {:?}, disposition: {disposition}, normalized_kind_code: {normalized_kind_code} }},",
            entry["raw_kind_id"].as_u64().expect("generated u16"),
            entry["raw_name"].as_str().expect("generated raw name"),
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();
    writeln!(
        output,
        "pub const RUFF_PYTHON_TOKEN_KINDS: &[RuffTokenKindEntry] = &["
    )
    .unwrap();
    for entry in ruff_document["runtime_inventory"]["token_kinds"]
        .as_array()
        .expect("generated Ruff token kinds are an array")
    {
        writeln!(
            output,
            "    RuffTokenKindEntry {{ raw_kind_id: {}, raw_name: {:?} }},",
            entry["raw_kind_id"].as_u64().expect("generated u16"),
            entry["raw_name"].as_str().expect("generated raw name"),
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();
    writeln!(
        output,
        "#[must_use]\npub const fn ruff_python_node_kind_entry(kind: ruff_python_ast::NodeKind) -> &'static RuffNodeKindEntry {{\n    match kind {{"
    )
    .unwrap();
    for (index, kind) in RUFF_NODE_KINDS.iter().copied().enumerate() {
        writeln!(
            output,
            "        ruff_python_ast::NodeKind::{} => &RUFF_PYTHON_NODE_KINDS[{index}],",
            ruff_node_kind_name(kind),
        )
        .unwrap();
    }
    writeln!(output, "    }}\n}}\n").unwrap();
    writeln!(
        output,
        "#[must_use]\n#[allow(clippy::too_many_lines)]\npub const fn ruff_python_token_kind_entry(kind: ruff_python_ast::token::TokenKind) -> &'static RuffTokenKindEntry {{\n    match kind {{"
    )
    .unwrap();
    for (index, kind) in RUFF_TOKEN_KINDS.iter().copied().enumerate() {
        writeln!(
            output,
            "        ruff_python_ast::token::TokenKind::{} => &RUFF_PYTHON_TOKEN_KINDS[{index}],",
            ruff_token_kind_name(kind),
        )
        .unwrap();
    }
    writeln!(output, "    }}\n}}\n").unwrap();
    writeln!(
        output,
        "pub const RUFF_PYTHON_FRONTEND: RuffPythonInventory = RuffPythonInventory {{ catalog_id: {:?}, provider_version: {:?}, runtime_inventory_fingerprint: {:?}, node_kinds: RUFF_PYTHON_NODE_KINDS, token_kinds: RUFF_PYTHON_TOKEN_KINDS }};\n",
        ruff_document["catalog_id"].as_str().expect("generated catalog ID"),
        ruff_document["provider_version"]
            .as_str()
            .expect("generated provider version"),
        ruff_document["runtime_inventory_fingerprint"]
            .as_str()
            .expect("generated fingerprint"),
    )
    .unwrap();
    output.push_str(
        "pub const PROVIDER_GRAMMAR_INVENTORIES: &[ProviderGrammarInventory] = &[\n\
             TREE_SITTER_PYTHON_GRAMMAR,\n\
             TREE_SITTER_RUST_GRAMMAR,\n\
         ];\n",
    );
    Ok(output.into_bytes())
}

fn registry_value(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    artifact_id: &str,
) -> Result<Value, ContractArtifactError> {
    let artifact = catalog
        .artifact(artifact_id)
        .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from(artifact_id)))?;
    let compiled = compile_artifact(repository_root, catalog, artifact)?;
    decode_strict(&compiled.canonical_bytes).map_err(|source| ContractArtifactError::Canonical {
        path: artifact.authority_path.clone(),
        source,
    })
}

fn registry_digest(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    artifact_id: &str,
) -> Result<String, ContractArtifactError> {
    let artifact = catalog
        .artifact(artifact_id)
        .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from(artifact_id)))?;
    Ok(compile_artifact(repository_root, catalog, artifact)?.canonical_digest)
}

fn pascal_case(value: &str) -> String {
    if !value.contains('_')
        && value.chars().any(char::is_lowercase)
        && value.chars().next().is_some_and(char::is_uppercase)
    {
        return value.to_owned();
    }
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let mut rendered = String::new();
            if let Some(first) = chars.next() {
                rendered.extend(first.to_uppercase());
            }
            rendered.extend(chars.flat_map(char::to_lowercase));
            rendered
        })
        .collect()
}

fn screaming_snake_from_pascal(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_uppercase() {
                vec!['_', character]
            } else {
                vec![character.to_ascii_uppercase()]
            }
        })
        .collect::<String>()
        .trim_start_matches('_')
        .to_owned()
}

fn rust_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            rendered.push('_');
        }
        rendered.push(character);
    }
    rendered
}

fn emit_rust_enum(output: &mut String, name: &str, values: &[super::registry_models::EnumValue]) {
    let type_name = pascal_case(name);
    writeln!(output, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]").unwrap();
    writeln!(output, "#[repr(u16)]").unwrap();
    writeln!(output, "pub enum {type_name} {{").unwrap();
    for value in values {
        writeln!(output, "    {} = {},", pascal_case(&value.name), value.code).unwrap();
    }
    writeln!(output, "}}").unwrap();
    writeln!(output, "impl TryFrom<u16> for {type_name} {{").unwrap();
    writeln!(output, "    type Error = UnknownRegistryCode;").unwrap();
    writeln!(
        output,
        "    fn try_from(code: u16) -> Result<Self, Self::Error> {{"
    )
    .unwrap();
    writeln!(output, "        match code {{").unwrap();
    for value in values {
        writeln!(
            output,
            "            {} => Ok(Self::{}),",
            value.code,
            pascal_case(&value.name)
        )
        .unwrap();
    }
    writeln!(
        output,
        "            _ => Err(UnknownRegistryCode {{ domain: \"{name}\", code }}),"
    )
    .unwrap();
    writeln!(output, "        }}").unwrap();
    writeln!(output, "    }}").unwrap();
    writeln!(output, "}}").unwrap();
}

// This is a deliberately linear emitter: keeping every generated Rust view in one
// pass makes the authority-to-output mapping auditable and order deterministic.
#[allow(clippy::too_many_lines)]
fn render_rust_registry_bindings(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<u8>, ContractArtifactError> {
    let authority_families = [
        (
            "entity",
            "codefabric.registry.ontology-entity-registry",
            "kind_slug",
        ),
        (
            "relation",
            "codefabric.registry.ontology-relation-registry",
            "relation_slug",
        ),
        (
            "property",
            "codefabric.registry.ontology-property-registry",
            "property_slug",
        ),
        (
            "fact",
            "codefabric.registry.ontology-fact-registry",
            "fact_slug",
        ),
    ];
    let mut authority_slugs = Vec::new();
    for (family, artifact_id, field) in authority_families {
        let value = registry_value(repository_root, catalog, artifact_id)?;
        let records = value["records"]
            .as_array()
            .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
        for record in records {
            let slug = record[field]
                .as_str()
                .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
            authority_slugs.push((family.to_owned(), slug.to_owned()));
        }
    }
    validate_duplicate_authorities(
        authority_slugs
            .iter()
            .map(|(family, slug)| (family.as_str(), slug.as_str())),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("ontology duplicate authority")))?;
    let enum_registry = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.enum-registry",
    )?;
    let enum_version = enum_registry["version"]
        .as_str()
        .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("enum-registry version")))?;
    let enum_digest = registry_digest(
        repository_root,
        catalog,
        "codefabric.registry.enum-registry",
    )?;
    let enums: Vec<EnumDomain> = serde_json::from_value(enum_registry["records"].clone())
        .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("enum-registry")))?;
    let flags: Vec<FlagDomain> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.flag-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("flag-registry")))?;
    let state_machine_registry = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.state-machine-registry",
    )?;
    let state_machine_version = state_machine_registry["version"].as_str().ok_or_else(|| {
        ContractArtifactError::Metadata(PathBuf::from("state-machine-registry version"))
    })?;
    let state_machine_digest = registry_digest(
        repository_root,
        catalog,
        "codefabric.registry.state-machine-registry",
    )?;
    let machines: Vec<StateMachine> =
        serde_json::from_value(state_machine_registry["records"].clone()).map_err(|_| {
            ContractArtifactError::Metadata(PathBuf::from("state-machine-registry"))
        })?;
    let phrases: Vec<PhraseRecord> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.phrase-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("phrase-registry")))?;
    let projection_registry = registry_value(
        repository_root,
        catalog,
        "codefabric.registry.projection-registry",
    )?;
    let projection_ids = projection_registry["records"]
        .as_array()
        .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("projection-registry")))?
        .iter()
        .filter_map(|record| record["projection_id"].as_str())
        .collect::<BTreeSet<_>>();
    if phrases
        .iter()
        .any(|phrase| !projection_ids.contains(phrase.contract_reference.code.as_str()))
    {
        return Err(ContractArtifactError::Metadata(PathBuf::from(
            "phrase registry projection reference",
        )));
    }

    let mut output = String::from(
        "// @generated by codefabric-contracts; edit registry authorities instead.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct UnknownRegistryCode { pub domain: &'static str, pub code: u16 }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RegistryEntry { pub code: u16, pub name: &'static str, pub slug: &'static str }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct FlagEntry { pub mask: u64, pub name: &'static str, pub slug: &'static str }\n\n",
    );
    for domain in &enums {
        emit_rust_enum(&mut output, &domain.domain, &domain.values);
        writeln!(
            output,
            "pub const {}_VALUES: &[RegistryEntry] = &[",
            domain.domain
        )
        .unwrap();
        for value in &domain.values {
            writeln!(
                output,
                "    RegistryEntry {{ code: {}, name: {:?}, slug: {:?} }},",
                value.code, value.name, value.slug
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    let enum_type_names = enums
        .iter()
        .map(|domain| pascal_case(&domain.domain))
        .collect::<BTreeSet<_>>();
    for machine in &machines {
        if enum_type_names.contains(&machine.machine_id) {
            continue;
        }
        emit_rust_enum(&mut output, &machine.machine_id, &machine.states);
        let constant = screaming_snake_from_pascal(&machine.machine_id);
        writeln!(output, "pub const {constant}_VALUES: &[RegistryEntry] = &[").unwrap();
        for value in &machine.states {
            writeln!(
                output,
                "    RegistryEntry {{ code: {}, name: {:?}, slug: {:?} }},",
                value.code, value.name, value.slug
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RegistryDomainEntry {\n\
             pub domain: &'static str, pub version: &'static str,\n\
             pub canonical_digest: &'static str, pub values: &'static [RegistryEntry],\n\
         }\n\n\
         pub const REGISTRY_DOMAINS: &[RegistryDomainEntry] = &[\n",
    );
    let mut emitted_domains = BTreeSet::new();
    for domain in &enums {
        emitted_domains.insert(domain.domain.clone());
        let domain_name = &domain.domain;
        writeln!(
            output,
            "    RegistryDomainEntry {{ domain: {domain_name:?}, version: {enum_version:?}, canonical_digest: {enum_digest:?}, values: {domain_name}_VALUES }},"
        )
        .unwrap();
    }
    for machine in &machines {
        let constant = screaming_snake_from_pascal(&machine.machine_id);
        if emitted_domains.insert(constant.clone()) {
            writeln!(
                output,
                "    RegistryDomainEntry {{ domain: {constant:?}, version: {state_machine_version:?}, canonical_digest: {state_machine_digest:?}, values: {constant}_VALUES }},"
            )
            .unwrap();
        }
    }
    output.push_str("];\n\n");
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct StateTransitionEntry {\n\
             pub from: &'static str, pub event: &'static str, pub guard: &'static str,\n\
             pub to: &'static str, pub actions: &'static [&'static str],\n\
             pub idempotency_key: &'static str, pub error_on_illegal: &'static str,\n\
         }\n\n",
    );
    for machine in &machines {
        let constant = screaming_snake_from_pascal(&machine.machine_id);
        writeln!(
            output,
            "pub const {constant}_TRANSITIONS: &[StateTransitionEntry] = &["
        )
        .unwrap();
        for transition in &machine.transitions {
            writeln!(
                output,
                "    StateTransitionEntry {{ from: {:?}, event: {:?}, guard: {:?}, to: {:?}, actions: &{:?}, idempotency_key: {:?}, error_on_illegal: {:?} }},",
                transition.from,
                transition.event,
                transition.guard,
                transition.to,
                transition.actions,
                transition.idempotency_key,
                transition.error_on_illegal,
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    for domain in &flags {
        writeln!(
            output,
            "pub const {}_FLAGS: &[FlagEntry] = &[",
            domain.domain
        )
        .unwrap();
        for value in &domain.values {
            writeln!(
                output,
                "    FlagEntry {{ mask: 1_u64 << {}, name: {:?}, slug: {:?} }},",
                value.bit, value.name, value.slug
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct PhraseEntry {\n\
             pub phrase_id: &'static str, pub owner_section: u16,\n\
             pub canonical_text: &'static str, pub accepted_aliases: &'static [&'static str],\n\
             pub plan_node_kind: &'static str, pub output_role: &'static str,\n\
         }\n\n\
         pub const PHRASE_ENTRIES: &[PhraseEntry] = &[\n",
    );
    for phrase in &phrases {
        writeln!(
            output,
            "    PhraseEntry {{ phrase_id: {:?}, owner_section: {}, canonical_text: {:?}, accepted_aliases: &{:?}, plan_node_kind: {:?}, output_role: {:?} }},",
            phrase.phrase_id,
            phrase.owner_section,
            phrase.canonical_text,
            phrase.accepted_aliases,
            phrase.planspec_mapping.node_kind.as_str(),
            phrase.planspec_mapping.output_role.as_str(),
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();
    let families = [
        (
            "ENTITY_KIND_IDS",
            "codefabric.registry.ontology-entity-registry",
            "canonical_name",
        ),
        (
            "RELATION_KIND_IDS",
            "codefabric.registry.ontology-relation-registry",
            "canonical_name",
        ),
        (
            "PROPERTY_KIND_IDS",
            "codefabric.registry.ontology-property-registry",
            "canonical_name",
        ),
        (
            "FACT_KIND_IDS",
            "codefabric.registry.ontology-fact-registry",
            "canonical_name",
        ),
        (
            "UNKNOWN_IDS",
            "codefabric.registry.unknown-registry",
            "name",
        ),
        (
            "PROJECTION_IDS",
            "codefabric.registry.projection-registry",
            "projection_id",
        ),
        (
            "SUMMARY_PROFILE_IDS",
            "codefabric.registry.summary-registry",
            "summary_profile_id",
        ),
        (
            "CAPABILITY_IDS",
            "codefabric.registry.capability-registry",
            "capability_code",
        ),
        (
            "PROVIDER_IDS",
            "codefabric.registry.provider-registry",
            "provider_id",
        ),
        (
            "PROVIDER_NORMALIZATION_IDS",
            "codefabric.registry.provider-normalization-registry",
            "mapping_id",
        ),
        (
            "PROVIDER_RESOURCE_PROFILE_IDS",
            "codefabric.registry.provider-resource-profile-registry",
            "profile_id",
        ),
        (
            "PUBLIC_ERROR_IDS",
            "codefabric.registry.error-registry",
            "name",
        ),
        (
            "DERIVATION_IDS",
            "codefabric.registry.derivation-registry",
            "derivation_id",
        ),
        (
            "PHRASE_IDS",
            "codefabric.registry.phrase-registry",
            "phrase_id",
        ),
    ];
    for (constant, artifact_id, field) in families {
        let value = registry_value(repository_root, catalog, artifact_id)?;
        let records = value["records"]
            .as_array()
            .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
        writeln!(output, "pub const {constant}: &[&str] = &[").unwrap();
        for record in records {
            let id = record[field]
                .as_str()
                .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
            writeln!(output, "    {id:?},").unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    let providers: Vec<Provider> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.provider-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("provider-registry")))?;
    let provider_codes = enums
        .iter()
        .find(|domain| domain.domain == "PROVIDER_CODE")
        .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("PROVIDER_CODE enum domain")))?
        .values
        .iter()
        .map(|value| (value.slug.as_str(), value.code))
        .collect::<BTreeMap<_, _>>();
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderEntry {\n\
             pub provider_code: i16, pub provider_id: &'static str,\n\
             pub placement: &'static str, pub capability_codes: &'static [&'static str],\n\
             pub resource_profile_id: &'static str, pub event_mapping_version: &'static str,\n\
         }\n\n\
         pub const PROVIDER_ENTRIES: &[ProviderEntry] = &[\n",
    );
    for provider in &providers {
        let code = provider_codes
            .get(provider.provider_id.as_str())
            .copied()
            .ok_or_else(|| {
                ContractArtifactError::Metadata(PathBuf::from(format!(
                    "provider code for {}",
                    provider.provider_id
                )))
            })?;
        let code = i16::try_from(code).map_err(|_| {
            ContractArtifactError::Metadata(PathBuf::from("provider code exceeds i16"))
        })?;
        writeln!(
            output,
            "    ProviderEntry {{ provider_code: {code}, provider_id: {:?}, placement: {:?}, capability_codes: &{:?}, resource_profile_id: {:?}, event_mapping_version: {:?} }},",
            provider.provider_id,
            provider.placement,
            provider.capability_codes,
            provider.resource_profile_id,
            provider.event_mapping_version,
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();

    let profiles: Vec<ProviderResourceProfile> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.provider-resource-profile-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| {
        ContractArtifactError::Metadata(PathBuf::from("provider-resource-profile-registry"))
    })?;
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderResourceProfileEntry {\n\
             pub profile_id: &'static str, pub provider_ids: &'static [&'static str],\n\
             pub max_parallel_jobs_global: u16, pub max_parallel_jobs_per_workspace: u16,\n\
             pub max_parallel_jobs_per_context: u16, pub max_input_bytes: u64,\n\
             pub max_work_units: u64, pub max_wall_millis: u64,\n\
             pub max_visited_nodes: u64, pub max_traversal_depth: u16,\n\
             pub max_output_records: u64, pub max_output_bytes: u64,\n\
             pub max_diagnostics: u16, pub max_parser_workers: u16,\n\
             pub max_retained_tree_revisions: u16, pub max_cpu_weight: u32,\n\
             pub max_memory_mib: u32, pub cancellation_check_interval: u32,\n\
             pub cancellation_ack_millis: u16, pub hard_stop_policy: &'static str,\n\
             pub retry_policy: &'static str, pub max_retries: u16,\n\
         }\n\n\
         pub const PROVIDER_RESOURCE_PROFILES: &[ProviderResourceProfileEntry] = &[\n",
    );
    for profile in &profiles {
        let hard_stop_policy = match profile.hard_stop_policy {
            ProviderHardStopPolicy::CooperativeDiscard => "COOPERATIVE_DISCARD",
            ProviderHardStopPolicy::ProcessGroupTerminate => "PROCESS_GROUP_TERMINATE",
            ProviderHardStopPolicy::CancellableTaskAbort => "CANCELLABLE_TASK_ABORT",
        };
        let retry_policy = match profile.retry_policy {
            ProviderRetryPolicy::NoRetry => "NO_RETRY",
            ProviderRetryPolicy::TransientOnly => "TRANSIENT_ONLY",
            ProviderRetryPolicy::IdempotentOnly => "IDEMPOTENT_ONLY",
        };
        let provider_ids = profile.provider_ids.iter().collect::<Vec<_>>();
        let max_input_bytes = rust_integer(profile.max_input_bytes);
        let max_work_units = rust_integer(profile.max_work_units);
        let max_wall_millis = rust_integer(profile.max_wall_millis);
        let max_visited_nodes = rust_integer(profile.max_visited_nodes);
        let max_output_records = rust_integer(profile.max_output_records);
        let max_output_bytes = rust_integer(profile.max_output_bytes);
        let cancellation_check_interval =
            rust_integer(u64::from(profile.cancellation_check_interval));
        writeln!(
            output,
            "    ProviderResourceProfileEntry {{ profile_id: {:?}, provider_ids: &{:?}, max_parallel_jobs_global: {}, max_parallel_jobs_per_workspace: {}, max_parallel_jobs_per_context: {}, max_input_bytes: {max_input_bytes}, max_work_units: {max_work_units}, max_wall_millis: {max_wall_millis}, max_visited_nodes: {max_visited_nodes}, max_traversal_depth: {}, max_output_records: {max_output_records}, max_output_bytes: {max_output_bytes}, max_diagnostics: {}, max_parser_workers: {}, max_retained_tree_revisions: {}, max_cpu_weight: {}, max_memory_mib: {}, cancellation_check_interval: {cancellation_check_interval}, cancellation_ack_millis: {}, hard_stop_policy: {hard_stop_policy:?}, retry_policy: {retry_policy:?}, max_retries: {} }},",
            profile.profile_id,
            provider_ids,
            profile.max_parallel_jobs_global,
            profile.max_parallel_jobs_per_workspace,
            profile.max_parallel_jobs_per_context,
            profile.max_traversal_depth,
            profile.max_diagnostics,
            profile.max_parser_workers,
            profile.max_retained_tree_revisions,
            profile.max_cpu_weight,
            profile.max_memory_mib,
            profile.cancellation_ack_millis,
            profile.max_retries,
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();
    let feature_registry =
        registry_value(repository_root, catalog, "codefabric.rpc.feature-registry")?;
    let feature_records = feature_registry["records"]
        .as_array()
        .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from("feature-registry")))?;
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderEventMappingEntry {\n\
             pub wire_event: &'static str, pub application_event: &'static str,\n\
             pub mapping_version: &'static str,\n\
         }\n\n\
         pub const PROVIDER_EVENT_MAPPINGS: &[ProviderEventMappingEntry] = &[\n",
    );
    for record in feature_records
        .iter()
        .filter(|record| record["domain"].as_str() == Some("PROVIDER_EVENT"))
    {
        let wire_event = record["wire_event"].as_str().ok_or_else(|| {
            ContractArtifactError::Metadata(PathBuf::from("feature-registry wire_event"))
        })?;
        let application_event = record["application_event"].as_str().ok_or_else(|| {
            ContractArtifactError::Metadata(PathBuf::from("feature-registry application_event"))
        })?;
        let mapping_version = record["mapping_version"].as_str().ok_or_else(|| {
            ContractArtifactError::Metadata(PathBuf::from("feature-registry mapping_version"))
        })?;
        writeln!(
            output,
            "    ProviderEventMappingEntry {{ wire_event: {wire_event:?}, application_event: {application_event:?}, mapping_version: {mapping_version:?} }},"
        )
        .unwrap();
    }
    writeln!(output, "];\n").unwrap();
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct OntologyCodeEntry {\n\
             pub code: i32, pub family_code: i16,\n\
         }\n\n",
    );
    for (constant, artifact_id, code_field, family_field) in [
        (
            "ENTITY_KIND_CODES",
            "codefabric.registry.ontology-entity-registry",
            "kind_code",
            Some("family_code"),
        ),
        (
            "RELATION_KIND_CODES",
            "codefabric.registry.ontology-relation-registry",
            "relation_code",
            Some("family_code"),
        ),
        (
            "PROPERTY_KIND_CODES",
            "codefabric.registry.ontology-property-registry",
            "property_code",
            None,
        ),
        (
            "FACT_KIND_CODES",
            "codefabric.registry.ontology-fact-registry",
            "fact_code",
            None,
        ),
    ] {
        let value = registry_value(repository_root, catalog, artifact_id)?;
        let records = value["records"]
            .as_array()
            .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
        writeln!(output, "pub const {constant}: &[OntologyCodeEntry] = &[").unwrap();
        for record in records {
            let code = record[code_field]
                .as_i64()
                .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
            let family_code = family_field
                .and_then(|field| record[field].as_i64())
                .unwrap_or_default();
            writeln!(
                output,
                "    OntologyCodeEntry {{ code: {code}, family_code: {family_code} }},"
            )
            .unwrap();
        }
        writeln!(output, "];\n").unwrap();
    }
    let content_len = output.trim_end().len();
    output.truncate(content_len);
    output.push('\n');
    Ok(output.into_bytes())
}

// This mirrors the Rust emitter as one deterministic pass over the same typed model.
#[allow(clippy::too_many_lines)]
fn render_python_registry_bindings(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<u8>, ContractArtifactError> {
    let enums: Vec<EnumDomain> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.enum-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("enum-registry")))?;
    let flags: Vec<FlagDomain> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.flag-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("flag-registry")))?;
    let machines: Vec<StateMachine> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.state-machine-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("state-machine-registry")))?;
    let phrases: Vec<PhraseRecord> = serde_json::from_value(
        registry_value(
            repository_root,
            catalog,
            "codefabric.registry.phrase-registry",
        )?["records"]
            .clone(),
    )
    .map_err(|_| ContractArtifactError::Metadata(PathBuf::from("phrase-registry")))?;
    let mut output = String::from(
        "\"\"\"Generated registry types and immutable lookup views.\"\"\"\n\n\
         from enum import IntEnum, IntFlag\n\
         from types import MappingProxyType\n\n\n",
    );
    for domain in &enums {
        writeln!(output, "class {}(IntEnum):", pascal_case(&domain.domain)).unwrap();
        for value in &domain.values {
            writeln!(output, "    {} = {}", value.name, value.code).unwrap();
        }
        writeln!(output).unwrap();
        writeln!(output).unwrap();
    }
    let enum_type_names = enums
        .iter()
        .map(|domain| pascal_case(&domain.domain))
        .collect::<BTreeSet<_>>();
    for machine in &machines {
        if enum_type_names.contains(&machine.machine_id) {
            continue;
        }
        writeln!(output, "class {}(IntEnum):", machine.machine_id).unwrap();
        for value in &machine.states {
            writeln!(output, "    {} = {}", value.name, value.code).unwrap();
        }
        writeln!(output).unwrap();
        writeln!(output).unwrap();
    }
    for domain in &flags {
        writeln!(output, "class {}(IntFlag):", pascal_case(&domain.domain)).unwrap();
        writeln!(output, "    NONE = 0").unwrap();
        for value in &domain.values {
            writeln!(output, "    {} = 1 << {}", value.name, value.bit).unwrap();
        }
        writeln!(output).unwrap();
        writeln!(output).unwrap();
    }
    writeln!(output, "ENUM_TRIPLES = MappingProxyType(").unwrap();
    writeln!(output, "    {{").unwrap();
    for domain in &enums {
        writeln!(output, "        {:?}: (", domain.domain).unwrap();
        for value in &domain.values {
            writeln!(output, "            (").unwrap();
            writeln!(output, "                {},", value.code).unwrap();
            writeln!(output, "                {:?},", value.name).unwrap();
            writeln!(output, "                {:?},", value.slug).unwrap();
            writeln!(output, "            ),").unwrap();
        }
        writeln!(output, "        ),").unwrap();
    }
    writeln!(output, "    }}").unwrap();
    writeln!(output, ")\n").unwrap();
    writeln!(output, "PHRASES = MappingProxyType(").unwrap();
    writeln!(output, "    {{").unwrap();
    for phrase in &phrases {
        writeln!(output, "        {:?}: MappingProxyType(", phrase.phrase_id).unwrap();
        writeln!(output, "            {{").unwrap();
        writeln!(
            output,
            "                \"owner_section\": {},",
            phrase.owner_section
        )
        .unwrap();
        writeln!(
            output,
            "                \"canonical_text\": {:?},",
            phrase.canonical_text
        )
        .unwrap();
        if phrase.accepted_aliases.len() == 1 {
            writeln!(
                output,
                "                \"accepted_aliases\": ({:?},),",
                phrase.accepted_aliases[0]
            )
            .unwrap();
        } else {
            writeln!(output, "                \"accepted_aliases\": (").unwrap();
            for alias in &phrase.accepted_aliases {
                writeln!(output, "                    {alias:?},").unwrap();
            }
            writeln!(output, "                ),").unwrap();
        }
        writeln!(
            output,
            "                \"plan_node_kind\": {:?},",
            phrase.planspec_mapping.node_kind.as_str()
        )
        .unwrap();
        writeln!(
            output,
            "                \"output_role\": {:?},",
            phrase.planspec_mapping.output_role.as_str()
        )
        .unwrap();
        writeln!(output, "            }}").unwrap();
        writeln!(output, "        ),").unwrap();
    }
    writeln!(output, "    }}").unwrap();
    writeln!(output, ")\n").unwrap();
    writeln!(output, "STATE_TRANSITIONS = MappingProxyType(").unwrap();
    writeln!(output, "    {{").unwrap();
    for machine in &machines {
        writeln!(output, "        {:?}: (", machine.machine_id).unwrap();
        for transition in &machine.transitions {
            writeln!(output, "            (").unwrap();
            for value in [
                &transition.from,
                &transition.event,
                &transition.guard,
                &transition.to,
            ] {
                writeln!(output, "                {value:?},").unwrap();
            }
            if transition.actions.len() == 1 {
                writeln!(output, "                ({:?},),", transition.actions[0]).unwrap();
            } else {
                writeln!(output, "                (").unwrap();
                for action in &transition.actions {
                    writeln!(output, "                    {action:?},").unwrap();
                }
                writeln!(output, "                ),").unwrap();
            }
            writeln!(output, "                {:?},", transition.idempotency_key).unwrap();
            writeln!(output, "                {:?},", transition.error_on_illegal).unwrap();
            writeln!(output, "            ),").unwrap();
        }
        writeln!(output, "        ),").unwrap();
    }
    writeln!(output, "    }}").unwrap();
    writeln!(output, ")\n").unwrap();
    let families = [
        (
            "entity_kinds",
            "codefabric.registry.ontology-entity-registry",
            "canonical_name",
        ),
        (
            "relation_kinds",
            "codefabric.registry.ontology-relation-registry",
            "canonical_name",
        ),
        (
            "property_kinds",
            "codefabric.registry.ontology-property-registry",
            "canonical_name",
        ),
        (
            "fact_kinds",
            "codefabric.registry.ontology-fact-registry",
            "canonical_name",
        ),
        ("unknowns", "codefabric.registry.unknown-registry", "name"),
        (
            "projections",
            "codefabric.registry.projection-registry",
            "projection_id",
        ),
        (
            "summary_profiles",
            "codefabric.registry.summary-registry",
            "summary_profile_id",
        ),
        (
            "capabilities",
            "codefabric.registry.capability-registry",
            "capability_code",
        ),
        (
            "providers",
            "codefabric.registry.provider-registry",
            "provider_id",
        ),
        (
            "provider_normalizations",
            "codefabric.registry.provider-normalization-registry",
            "mapping_id",
        ),
        (
            "provider_resource_profiles",
            "codefabric.registry.provider-resource-profile-registry",
            "profile_id",
        ),
        (
            "public_errors",
            "codefabric.registry.error-registry",
            "name",
        ),
        (
            "derivations",
            "codefabric.registry.derivation-registry",
            "derivation_id",
        ),
        (
            "phrases",
            "codefabric.registry.phrase-registry",
            "phrase_id",
        ),
    ];
    writeln!(output, "REGISTRY_IDS = MappingProxyType(").unwrap();
    writeln!(output, "    {{").unwrap();
    for (family, artifact_id, field) in families {
        let value = registry_value(repository_root, catalog, artifact_id)?;
        let records = value["records"]
            .as_array()
            .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
        if records.is_empty() {
            writeln!(output, "        {family:?}: (),").unwrap();
        } else if records.len() == 1 {
            let id = records[0][field]
                .as_str()
                .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
            writeln!(output, "        {family:?}: ({id:?},),").unwrap();
        } else {
            writeln!(output, "        {family:?}: (").unwrap();
            for record in records {
                let id = record[field]
                    .as_str()
                    .ok_or_else(|| ContractArtifactError::Metadata(PathBuf::from(artifact_id)))?;
                writeln!(output, "            {id:?},").unwrap();
            }
            writeln!(output, "        ),").unwrap();
        }
    }
    writeln!(output, "    }}").unwrap();
    writeln!(output, ")").unwrap();
    let content_len = output.trim_end().len();
    output.truncate(content_len);
    output.push('\n');
    Ok(output.into_bytes())
}

fn render_index(
    repository_root: &Path,
    index_path: &Path,
    artifact_records: &[ArtifactIndexRecord],
    derivation_records: &[DerivationIndexRecord],
) -> Result<Vec<u8>, ContractArtifactError> {
    let index = ArtifactIndex {
        generated: ArtifactIndexGeneration {
            catalog_artifact_id: "codefabric.manifests.suite-manifest".to_owned(),
            artifact_count: artifact_records.len(),
            derivation_count: derivation_records.len(),
            generator_revision: "codefabric-contracts-model-v2".to_owned(),
            profile: PROFILE.to_owned(),
        },
        artifacts: artifact_records.to_vec(),
        derivations: derivation_records.to_vec(),
    };
    canonicalize_value(
        &serde_json::to_value(index).expect("typed artifact index serialization is infallible"),
    )
    .map_err(|source| ContractArtifactError::Canonical {
        path: repository_root.join(index_path),
        source,
    })
}

fn render_outputs(
    repository_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    let mut outputs = BTreeMap::new();
    outputs.extend(render_schema_outputs(repository_root, &catalog)?);
    #[cfg(feature = "fact-generation")]
    render_provider_raw_outputs(repository_root, &catalog, &mut outputs)?;
    let artifact_records = collect_artifact_records(repository_root, &catalog)?;
    let derivation_records = collect_derivation_records(&catalog);
    render_registry_outputs(repository_root, &catalog, &mut outputs)?;
    let rust_bindings = required_output(
        &catalog,
        REGISTRY_DERIVATION_ID,
        DerivationOutputKind::RustRegistryBindings,
    )?;
    outputs.insert(
        rust_bindings,
        render_rust_registry_bindings(repository_root, &catalog)?,
    );
    let python_bindings = required_output(
        &catalog,
        REGISTRY_DERIVATION_ID,
        DerivationOutputKind::PythonRegistryBindings,
    )?;
    outputs.insert(
        python_bindings,
        render_python_registry_bindings(repository_root, &catalog)?,
    );
    let generated_index = required_output(
        &catalog,
        ARTIFACT_INDEX_DERIVATION_ID,
        DerivationOutputKind::ArtifactIndex,
    )?;
    let index_bytes = render_index(
        repository_root,
        &generated_index,
        &artifact_records,
        &derivation_records,
    )?;
    outputs.insert(generated_index, index_bytes);
    Ok(outputs)
}

fn required_output(
    catalog: &CompiledCatalog,
    derivation_id: &str,
    output_kind: DerivationOutputKind,
) -> Result<PathBuf, ContractArtifactError> {
    let mut matches = catalog.outputs_of_kind(derivation_id, output_kind);
    let Some((path, _)) = matches.next() else {
        return Err(ContractArtifactError::Missing(PathBuf::from(format!(
            "catalog derivation {derivation_id} output kind {output_kind:?}"
        ))));
    };
    if matches.next().is_some() {
        return Err(ContractArtifactError::Metadata(PathBuf::from(format!(
            "catalog derivation {derivation_id} has non-unique output kind {output_kind:?}"
        ))));
    }
    Ok(path.to_owned())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ContractArtifactError> {
    let parent = path
        .parent()
        .ok_or_else(|| ContractArtifactError::Missing(path.to_owned()))?;
    fs::create_dir_all(parent).map_err(|source| ContractArtifactError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| ContractArtifactError::Io {
            path: parent.to_owned(),
            source,
        })?;
    temporary
        .write_all(bytes)
        .map_err(|source| ContractArtifactError::Io {
            path: path.to_owned(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| ContractArtifactError::Io {
            path: path.to_owned(),
            source: error.error,
        })?;
    Ok(())
}

fn replace_unique_digest(
    path: &Path,
    bytes: &[u8],
    claimed: &str,
    computed: &str,
) -> Result<Option<Vec<u8>>, ContractArtifactError> {
    if claimed == computed {
        return Ok(None);
    }
    let matches = bytes
        .windows(claimed.len())
        .enumerate()
        .filter(|(_, window)| *window == claimed.as_bytes())
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let [offset] = matches.as_slice() else {
        return Err(ContractArtifactError::Metadata(path.to_owned()));
    };
    let mut updated = bytes.to_vec();
    updated.splice(
        *offset..(*offset + claimed.len()),
        computed.as_bytes().iter().copied(),
    );
    Ok(Some(updated))
}

fn embed_bundle_digests(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<(), ContractArtifactError> {
    for artifact in catalog.artifacts().filter(|artifact| {
        artifact.digest_projection == super::catalog::DigestProjection::BundleAcG07V1
    }) {
        let path = repository_root.join(&artifact.authority_path);
        let compiled = compile_artifact_for_generation(repository_root, catalog, artifact)?;
        let computed = compiled
            .bundle_digest
            .expect("the bundle projection always computes a bundle identity");
        let bytes = read(&path)?;
        let mut value =
            decode_strict(&bytes).map_err(|source| ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            })?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
        if object.get("bundle_digest").and_then(Value::as_str) == Some(&computed) {
            continue;
        }
        object.insert("bundle_digest".to_owned(), Value::String(computed));
        let mut updated = serde_json::to_vec_pretty(&value).map_err(|source| {
            ContractArtifactError::Canonical {
                path: path.clone(),
                source: CanonicalJsonError::Serialization(source),
            }
        })?;
        updated.push(b'\n');
        write_atomic(&path, &updated)?;
    }
    Ok(())
}

fn embed_semantic_digests(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    embed_bundle_digests(repository_root, &catalog)?;

    let catalog = ContractCatalog::load(repository_root)?;
    for artifact in catalog.artifacts() {
        let compiled = compile_artifact_for_generation(repository_root, &catalog, artifact)?;
        let Some(claimed) = compiled.embedded_canonical_digest else {
            continue;
        };
        let path = repository_root.join(&artifact.authority_path);
        let bytes = read(&path)?;
        if let Some(updated) =
            replace_unique_digest(&path, &bytes, &claimed, &compiled.canonical_digest)?
        {
            write_atomic(&path, &updated)?;
        }
    }

    let catalog = ContractCatalog::load(repository_root)?;
    for artifact in catalog.artifacts() {
        compile_artifact(repository_root, &catalog, artifact)?;
    }
    Ok(())
}

fn pretty_json(value: &Value) -> Result<Vec<u8>, ContractArtifactError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| ContractArtifactError::Canonical {
            path: PathBuf::from("generated JSON"),
            source: CanonicalJsonError::Serialization(source),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sync_toolchain_identity(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let relative = Path::new("contracts/toolchain/toolchain-identity.json");
    let path = repository_root.join(relative);
    let mut value =
        decode_strict(&read(&path)?).map_err(|source| ContractArtifactError::Canonical {
            path: path.clone(),
            source,
        })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
    let set_digest = |target: &mut Value, key: &str, source: &str| {
        let target = target
            .as_object_mut()
            .and_then(|value| value.get_mut(key))
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
        *target = Value::String(checksum(&read(&repository_root.join(source))?));
        Ok::<(), ContractArtifactError>(())
    };
    object.insert(
        "cargo_lock_digest".to_owned(),
        Value::String(checksum(&read(&repository_root.join("Cargo.lock"))?)),
    );
    set_digest(
        object
            .get_mut("adapter")
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?,
        "source_digest",
        "codefabric-cpg-mcp/uv.lock",
    )?;
    set_digest(
        object
            .get_mut("protobuf")
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?,
        "source_digest",
        "tooling/proto/toolchain-identity.json",
    )?;
    set_digest(
        object
            .get_mut("rustc_extractor")
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?,
        "source_digest",
        "rustc-extractor/toolchain-identity.json",
    )?;
    set_digest(
        object
            .get_mut("pyrefly")
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?,
        "source_digest",
        "pyrefly-sidecar/toolchain-identity.json",
    )?;
    write_atomic(&path, &pretty_json(&value)?)
}

fn typed_yaml_artifact<T: DeserializeOwned>(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    artifact_id: &str,
) -> Result<T, ContractArtifactError> {
    let descriptor = catalog
        .artifact(artifact_id)
        .ok_or_else(|| ContractArtifactError::Metadata(repository_root.to_owned()))?;
    let path = repository_root.join(&descriptor.authority_path);
    serde_yaml_ng::from_slice(&read(&path)?)
        .map_err(|error| fixture_failure(&path, format!("typed YAML decode failed: {error}")))
}

struct TraceUniverses {
    ontology_kinds: BTreeSet<String>,
    capability_codes: BTreeSet<String>,
    table_fields: BTreeSet<String>,
    query_phrase_ids: BTreeSet<String>,
    response_fields: BTreeSet<String>,
    error_codes: BTreeSet<String>,
}

#[allow(clippy::too_many_lines)] // One typed census keeps every trace family in one source join.
fn trace_universes(repository_root: &Path) -> Result<TraceUniverses, ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    let ontology_kinds = [
        typed_yaml_artifact::<AcceptedRegistry<EntityKind>>(
            repository_root,
            &catalog,
            "codefabric.registry.ontology-entity-registry",
        )?
        .records
        .into_iter()
        .map(|record| record.canonical_name)
        .collect::<BTreeSet<_>>(),
        typed_yaml_artifact::<AcceptedRegistry<RelationKind>>(
            repository_root,
            &catalog,
            "codefabric.registry.ontology-relation-registry",
        )?
        .records
        .into_iter()
        .map(|record| record.canonical_name)
        .collect(),
        typed_yaml_artifact::<AcceptedRegistry<PropertyKind>>(
            repository_root,
            &catalog,
            "codefabric.registry.ontology-property-registry",
        )?
        .records
        .into_iter()
        .map(|record| record.canonical_name)
        .collect(),
        typed_yaml_artifact::<AcceptedRegistry<FactKind>>(
            repository_root,
            &catalog,
            "codefabric.registry.ontology-fact-registry",
        )?
        .records
        .into_iter()
        .map(|record| record.canonical_name)
        .collect(),
        typed_yaml_artifact::<AcceptedRegistry<UnknownKind>>(
            repository_root,
            &catalog,
            "codefabric.registry.unknown-registry",
        )?
        .records
        .into_iter()
        .map(|record| record.name)
        .collect(),
    ]
    .into_iter()
    .flatten()
    .collect::<BTreeSet<_>>();
    let capability_codes = typed_yaml_artifact::<AcceptedRegistry<Capability>>(
        repository_root,
        &catalog,
        "codefabric.registry.capability-registry",
    )?
    .records
    .into_iter()
    .map(|record| record.capability_code)
    .collect::<BTreeSet<_>>();
    let query_phrase_ids = typed_yaml_artifact::<AcceptedRegistry<PhraseRecord>>(
        repository_root,
        &catalog,
        "codefabric.registry.phrase-registry",
    )?
    .records
    .into_iter()
    .map(|record| record.phrase_id)
    .collect::<BTreeSet<_>>();
    let error_codes = typed_yaml_artifact::<AcceptedRegistry<PublicError>>(
        repository_root,
        &catalog,
        "codefabric.registry.error-registry",
    )?
    .records
    .into_iter()
    .map(|record| record.code.to_string())
    .collect::<BTreeSet<_>>();

    let schema_descriptor = catalog
        .artifact("codefabric.schema.contract-ir")
        .ok_or_else(|| ContractArtifactError::Metadata(repository_root.to_owned()))?;
    let schema_path = repository_root.join(&schema_descriptor.authority_path);
    let schema: SchemaContractIr =
        serde_json::from_value(decode_strict(&read(&schema_path)?).map_err(|source| {
            ContractArtifactError::Canonical {
                path: schema_path.clone(),
                source,
            }
        })?)
        .map_err(|error| {
            fixture_failure(&schema_path, format!("typed JSON decode failed: {error}"))
        })?;
    let table_fields = schema
        .tables
        .into_iter()
        .flat_map(|table| {
            let name = table.name;
            table
                .columns
                .into_iter()
                .map(move |column| format!("{name}.{}", column.name))
        })
        .collect::<BTreeSet<_>>();

    let response_descriptor = catalog
        .artifact("codefabric.schema.cpg-semantic-query-response.schema")
        .ok_or_else(|| ContractArtifactError::Metadata(repository_root.to_owned()))?;
    let response_path = repository_root.join(&response_descriptor.authority_path);
    let response = decode_strict(&read(&response_path)?).map_err(|source| {
        ContractArtifactError::Canonical {
            path: response_path.clone(),
            source,
        }
    })?;
    let response_fields = response
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractArtifactError::Metadata(response_path.clone()))?
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    Ok(TraceUniverses {
        ontology_kinds,
        capability_codes,
        table_fields,
        query_phrase_ids,
        response_fields,
        error_codes,
    })
}

#[allow(clippy::too_many_lines)] // One record pass prevents partial requirement rewrites.
fn sync_requirements(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let TraceUniverses {
        ontology_kinds,
        capability_codes,
        table_fields,
        query_phrase_ids,
        response_fields,
        error_codes,
    } = trace_universes(repository_root)?;
    let catalog = ContractCatalog::load(repository_root)?;

    let path = repository_root.join("contracts/manifests/requirements.jsonl");
    let bytes = read(&path)?;
    let mut lines = bytes
        .strip_suffix(b"\n")
        .unwrap_or(&bytes)
        .split(|byte| *byte == b'\n');
    let first = lines
        .next()
        .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
    let mut metadata = decode_strict(first).map_err(|source| ContractArtifactError::Canonical {
        path: path.clone(),
        source,
    })?;
    metadata
        .as_object_mut()
        .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?
        .insert("status".to_owned(), Value::String("released".to_owned()));
    let mut output =
        serde_json::to_vec(&metadata).map_err(|source| ContractArtifactError::Canonical {
            path: path.clone(),
            source: CanonicalJsonError::Serialization(source),
        })?;
    output.push(b'\n');
    for line in lines {
        let value = decode_strict(line).map_err(|source| ContractArtifactError::Canonical {
            path: path.clone(),
            source,
        })?;
        let mut requirement: RequirementRecord =
            serde_json::from_value(value).map_err(|error| {
                fixture_failure(&path, format!("typed JSONL decode failed: {error}"))
            })?;
        requirement.normative_text_digest = checksum(requirement.normative_text.as_bytes());
        let source_matches = catalog
            .artifacts()
            .filter(|artifact| {
                artifact
                    .authority_path
                    .file_stem()
                    .and_then(OsStr::to_str)
                    .is_some_and(|stem| {
                        stem == requirement.source_artifact
                            || stem.starts_with(&format!("{}_v", requirement.source_artifact))
                    })
            })
            .collect::<Vec<_>>();
        let [source_descriptor] = source_matches.as_slice() else {
            return Err(ContractArtifactError::Traceability {
                path: path.clone(),
                message: format!(
                    "requirement {} source_artifact resolves to {} catalog records",
                    requirement.requirement_id,
                    source_matches.len()
                ),
            });
        };
        requirement.owner_acceptance.source_digest =
            compile_artifact_for_generation(repository_root, &catalog, source_descriptor)?
                .source_digest;
        for selector in &requirement.trace_selectors {
            match selector {
                TraceSelector::AllOntologyKinds => {
                    requirement.traces_to.ontology_kinds = ontology_kinds.iter().cloned().collect();
                }
                TraceSelector::AllCapabilityCodes => {
                    requirement.traces_to.capability_codes =
                        capability_codes.iter().cloned().collect();
                }
                TraceSelector::AllTableFields => {
                    requirement.traces_to.table_fields = table_fields.iter().cloned().collect();
                }
                TraceSelector::AllQueryPhraseIds => {
                    requirement.traces_to.query_phrase_ids =
                        query_phrase_ids.iter().cloned().collect();
                }
                TraceSelector::AllResponseFields => {
                    requirement.traces_to.response_fields =
                        response_fields.iter().cloned().collect();
                }
                TraceSelector::AllErrorCodes => {
                    requirement.traces_to.error_codes = error_codes.iter().cloned().collect();
                }
            }
        }
        output.extend(serde_json::to_vec(&requirement).map_err(|source| {
            ContractArtifactError::Canonical {
                path: path.clone(),
                source: CanonicalJsonError::Serialization(source),
            }
        })?);
        output.push(b'\n');
    }
    write_atomic(&path, &output)
}

fn sync_traceability(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let requirements_path = repository_root.join("contracts/manifests/requirements.jsonl");
    let traceability_path = repository_root.join("contracts/manifests/traceability.jsonl");
    let requirements = read(&requirements_path)?;
    let traceability = read(&traceability_path)?;
    let first_line = traceability
        .split(|byte| *byte == b'\n')
        .next()
        .ok_or_else(|| ContractArtifactError::Metadata(traceability_path.clone()))?;
    let mut metadata =
        decode_strict(first_line).map_err(|source| ContractArtifactError::Canonical {
            path: traceability_path.clone(),
            source,
        })?;
    metadata
        .as_object_mut()
        .ok_or_else(|| ContractArtifactError::Metadata(traceability_path.clone()))?
        .insert("status".to_owned(), Value::String("released".to_owned()));
    let mut output =
        serde_json::to_vec(&metadata).map_err(|source| ContractArtifactError::Canonical {
            path: traceability_path.clone(),
            source: CanonicalJsonError::Serialization(source),
        })?;
    output.push(b'\n');
    for line in requirements
        .strip_suffix(b"\n")
        .unwrap_or(&requirements)
        .split(|byte| *byte == b'\n')
        .skip(1)
    {
        let value = decode_strict(line).map_err(|source| ContractArtifactError::Canonical {
            path: requirements_path.clone(),
            source,
        })?;
        let requirement: RequirementRecord =
            serde_json::from_value(value).map_err(|source| ContractArtifactError::Canonical {
                path: requirements_path.clone(),
                source: CanonicalJsonError::Serialization(source),
            })?;
        let trace = TraceabilityRecord {
            requirement_id: requirement.requirement_id,
            implements: requirement.implements,
            traces_to: requirement.traces_to,
            verified_by: requirement.verified_by,
        };
        output.extend(serde_json::to_vec(&trace).map_err(|source| {
            ContractArtifactError::Canonical {
                path: traceability_path.clone(),
                source: CanonicalJsonError::Serialization(source),
            }
        })?);
        output.push(b'\n');
    }
    write_atomic(&traceability_path, &output)
}

fn sync_bundle_members(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    for kind in BundleKind::ALL {
        let bundle_id = format!("codefabric.bundles.{}-bundle", kind.artifact_slug());
        let descriptor = catalog
            .artifact(&bundle_id)
            .ok_or_else(|| ContractArtifactError::Metadata(repository_root.to_owned()))?;
        let path = repository_root.join(&descriptor.authority_path);
        let mut value =
            decode_strict(&read(&path)?).map_err(|source| ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            })?;
        let members = catalog
            .artifacts()
            .filter(|artifact| artifact.bundle_membership.contains(&kind))
            .map(|artifact| {
                let compiled =
                    compile_artifact_for_generation(repository_root, &catalog, artifact)?;
                Ok(serde_json::json!({
                    "artifact_id": artifact.artifact_id,
                    "version": artifact.version,
                    "canonical_digest": compiled.canonical_digest,
                    "required": true,
                    "feature_bits": [],
                }))
            })
            .collect::<Result<Vec<_>, ContractArtifactError>>()?;
        if members.is_empty() {
            return Err(ContractArtifactError::Metadata(path));
        }
        let object = value
            .as_object_mut()
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
        object.insert("artifacts".to_owned(), Value::Array(members));
        object.insert("status".to_owned(), Value::String("released".to_owned()));
        write_atomic(&path, &pretty_json(&value)?)?;
    }
    Ok(())
}

/// Generate every committed model-derived contract output using atomic replacement.
///
/// # Errors
///
/// Returns an error for missing/invalid sources, canonicalization, or filesystem failure.
pub fn generate(repository_root: &Path) -> Result<usize, ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    for (relative, bytes) in render_schema_outputs(repository_root, &catalog)? {
        write_atomic(&repository_root.join(relative), &bytes)?;
    }
    sync_toolchain_identity(repository_root)?;
    sync_requirements(repository_root)?;
    sync_traceability(repository_root)?;
    sync_bundle_members(repository_root)?;
    embed_semantic_digests(repository_root)?;
    let outputs = render_outputs(repository_root)?;
    for (relative, bytes) in &outputs {
        write_atomic(&repository_root.join(relative), bytes)?;
    }
    Ok(outputs.len())
}

fn verify_generated(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let outputs = render_outputs(repository_root)?;
    verify_generated_census(repository_root, &outputs, Path::new("contracts/generated"))?;
    for (relative, expected) in outputs {
        let path = repository_root.join(&relative);
        let actual = read(&path)?;
        if actual != expected {
            return Err(ContractArtifactError::Drift(relative));
        }
    }
    Ok(())
}

fn verify_generated_census(
    repository_root: &Path,
    outputs: &BTreeMap<PathBuf, Vec<u8>>,
    relative_directory: &Path,
) -> Result<(), ContractArtifactError> {
    let expected = outputs
        .keys()
        .filter(|path| path.starts_with(relative_directory))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    collect_generated_files(
        repository_root,
        &repository_root.join(relative_directory),
        &mut actual,
    )?;
    if actual != expected {
        let drift = actual
            .symmetric_difference(&expected)
            .next()
            .cloned()
            .unwrap_or_else(|| relative_directory.to_owned());
        return Err(ContractArtifactError::Drift(drift));
    }
    Ok(())
}

fn collect_generated_files(
    repository_root: &Path,
    directory: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), ContractArtifactError> {
    let entries = fs::read_dir(directory).map_err(|source| ContractArtifactError::Io {
        path: directory.to_owned(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ContractArtifactError::Io {
            path: directory.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| ContractArtifactError::Io {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            if entry.file_name() != "__pycache__" {
                collect_generated_files(repository_root, &path, files)?;
            }
        } else if file_type.is_file() {
            if path.extension() == Some(OsStr::new("pyc")) {
                continue;
            }
            let relative = path
                .strip_prefix(repository_root)
                .map_err(|_| ContractArtifactError::Drift(path.clone()))?;
            files.insert(relative.to_owned());
        } else {
            return Err(ContractArtifactError::Drift(path));
        }
    }
    Ok(())
}

fn typed_jsonl_records<T: DeserializeOwned>(
    path: &Path,
    canonical_bytes: &[u8],
) -> Result<Vec<T>, ContractArtifactError> {
    canonical_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .skip(1)
        .map(|line| {
            let value =
                decode_strict(line).map_err(|error| ContractArtifactError::Traceability {
                    path: path.to_owned(),
                    message: error.to_string(),
                })?;
            serde_json::from_value(value).map_err(|error| ContractArtifactError::Traceability {
                path: path.to_owned(),
                message: error.to_string(),
            })
        })
        .collect()
}

fn valid_requirement_id(identifier: &str) -> bool {
    let mut parts = identifier.split('-');
    let prefix = parts.next();
    let owner = parts.next();
    let number = parts.next();
    prefix == Some("CF")
        && owner.is_some_and(|value| {
            matches!(
                value,
                "ARCH" | "ONT" | "GEN" | "FAB" | "LIFE" | "QUERY" | "SERVE" | "SEC" | "TEST"
            )
        })
        && number.is_some_and(|value| {
            value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn non_empty_strings(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| !value.is_empty())
}

#[allow(clippy::too_many_lines)] // One verifier keeps ID, edge, and universe checks atomic.
fn verify_traceability(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<(), ContractArtifactError> {
    let requirements = catalog
        .artifact("codefabric.manifests.requirements")
        .expect("the compiled catalog owns the requirements manifest");
    let traceability = catalog
        .artifact("codefabric.manifests.traceability")
        .expect("the compiled catalog owns the traceability manifest");
    let requirements_path = repository_root.join(&requirements.authority_path);
    let traceability_path = repository_root.join(&traceability.authority_path);
    let compiled_requirements = compile_artifact(repository_root, catalog, requirements)?;
    let compiled_traceability = compile_artifact(repository_root, catalog, traceability)?;
    let mut requirement_ids = BTreeSet::new();
    let mut expected_trace_records = BTreeMap::new();
    let mut traced_ontology_kinds = BTreeSet::new();
    let mut traced_capability_codes = BTreeSet::new();
    let mut traced_table_fields = BTreeSet::new();
    let mut traced_query_phrase_ids = BTreeSet::new();
    let mut traced_response_fields = BTreeSet::new();
    let mut traced_error_codes = BTreeSet::new();
    for record in typed_jsonl_records::<RequirementRecord>(
        &requirements_path,
        &compiled_requirements.canonical_bytes,
    )? {
        let identifier = record.requirement_id;
        if !valid_requirement_id(&identifier) || !requirement_ids.insert(identifier.clone()) {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path.clone(),
                message: format!("invalid or duplicate requirement ID: {identifier}"),
            });
        }
        if record.normative_text_digest != checksum(record.normative_text.as_bytes())
            || !non_empty_strings(&record.implements)
            || !non_empty_strings(&record.verified_by)
            || record.owner_acceptance.approver.is_empty()
            || record.owner_acceptance.source_digest.is_empty()
        {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path.clone(),
                message: format!(
                    "requirement {identifier} is incomplete or has a stale text digest"
                ),
            });
        }
        traced_ontology_kinds.extend(record.traces_to.ontology_kinds.iter().cloned());
        traced_capability_codes.extend(record.traces_to.capability_codes.iter().cloned());
        traced_table_fields.extend(record.traces_to.table_fields.iter().cloned());
        traced_query_phrase_ids.extend(record.traces_to.query_phrase_ids.iter().cloned());
        traced_response_fields.extend(record.traces_to.response_fields.iter().cloned());
        traced_error_codes.extend(record.traces_to.error_codes.iter().cloned());
        expected_trace_records.insert(
            identifier,
            (
                record.implements.clone(),
                record.traces_to.clone(),
                record.verified_by.clone(),
            ),
        );
    }
    if requirement_ids.is_empty() {
        return Err(ContractArtifactError::Traceability {
            path: requirements_path.clone(),
            message: "no CF-* requirement records exist".to_owned(),
        });
    }
    let expected = trace_universes(repository_root)?;
    for (label, actual, expected) in [
        (
            "ontology kinds",
            &traced_ontology_kinds,
            &expected.ontology_kinds,
        ),
        (
            "capability codes",
            &traced_capability_codes,
            &expected.capability_codes,
        ),
        ("table fields", &traced_table_fields, &expected.table_fields),
        (
            "query phrase IDs",
            &traced_query_phrase_ids,
            &expected.query_phrase_ids,
        ),
        (
            "response fields",
            &traced_response_fields,
            &expected.response_fields,
        ),
        ("error codes", &traced_error_codes, &expected.error_codes),
    ] {
        if actual != expected {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path.clone(),
                message: format!("released trace coverage is not exact for {label}"),
            });
        }
    }

    let mut traced_ids = BTreeSet::new();
    for record in typed_jsonl_records::<TraceabilityRecord>(
        &traceability_path,
        &compiled_traceability.canonical_bytes,
    )? {
        let identifier = record.requirement_id;
        if !requirement_ids.contains(&identifier)
            || !traced_ids.insert(identifier.clone())
            || !non_empty_strings(&record.implements)
            || !non_empty_strings(&record.verified_by)
            || expected_trace_records.get(&identifier).is_none_or(
                |(implements, traces_to, verified_by)| {
                    implements != &record.implements
                        || traces_to != &record.traces_to
                        || verified_by != &record.verified_by
                },
            )
        {
            return Err(ContractArtifactError::Traceability {
                path: traceability_path.clone(),
                message: format!("trace for {identifier} is unknown, duplicate, or orphaned"),
            });
        }
    }
    if traced_ids != requirement_ids {
        return Err(ContractArtifactError::Traceability {
            path: traceability_path,
            message: "one or more requirements have no trace record".to_owned(),
        });
    }
    Ok(())
}

/// Prove the committed WP11 broken-edge fixture is rejected as an unknown requirement.
///
/// # Errors
///
/// Returns an error if the fixture is malformed, its target contract drifts, or the
/// candidate edge is no longer rejected by the released requirement inventory.
pub fn verify_broken_trace_edge_fixture(
    repository_root: &Path,
) -> Result<(), ContractArtifactError> {
    let path = repository_root.join("contracts/fixtures/negative/broken-trace-edge.json");
    let fixture: BrokenTraceEdgeFixture =
        serde_json::from_value(decode_strict(&read(&path)?).map_err(|source| {
            ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            }
        })?)
        .map_err(|error| fixture_failure(&path, format!("typed fixture decode failed: {error}")))?;
    if fixture.fixture_id != "wp11-broken-trace-edge"
        || fixture.target_artifact != "codefabric.manifests.traceability"
        || fixture.expected_failure_class != "unknown-requirement"
    {
        return Err(fixture_failure(
            &path,
            "fixture identity or expected class drifted",
        ));
    }
    let catalog = ContractCatalog::load(repository_root)?;
    let requirements = catalog
        .artifact("codefabric.manifests.requirements")
        .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
    let compiled = compile_artifact(repository_root, &catalog, requirements)?;
    let requirement_ids = typed_jsonl_records::<RequirementRecord>(
        &repository_root.join(&requirements.authority_path),
        &compiled.canonical_bytes,
    )?
    .into_iter()
    .map(|record| record.requirement_id)
    .collect::<BTreeSet<_>>();
    if requirement_ids.contains(&fixture.trace.requirement_id) {
        return Err(fixture_failure(
            &path,
            "broken trace edge unexpectedly resolved to a released requirement",
        ));
    }
    Ok(())
}

/// Verify the AC-G-05 source layout, metadata, shared JCS corpus, and generated bytes.
///
/// # Errors
///
/// Returns an error for an absent/invalid artifact, generated drift, corpus mismatch,
/// or any warning under the released profile.
pub fn verify(
    repository_root: &Path,
    profile: VerificationProfile,
) -> Result<VerificationReport, ContractArtifactError> {
    let contracts_root = repository_root.join("contracts");
    let arrow_delta = contracts_root.join("schema/arrow-delta");
    if !arrow_delta.is_dir() {
        return Err(ContractArtifactError::Missing(arrow_delta));
    }

    let catalog = ContractCatalog::load(repository_root)?;
    let warning_count = catalog.draft_count();
    for artifact in catalog.artifacts() {
        compile_artifact(repository_root, &catalog, artifact)?;
    }
    verify_traceability(repository_root, &catalog)?;
    verify_broken_trace_edge_fixture(repository_root)?;
    verify_generated(repository_root)?;
    verify_jcs_corpus(&contracts_root.join("fixtures/jcs/vectors.json"))?;
    verify_jcs_differential(&contracts_root.join("fixtures/jcs/differential-cases.json"))?;

    if profile == VerificationProfile::Released && warning_count != 0 {
        return Err(ContractArtifactError::ReleasedWarnings(warning_count));
    }
    Ok(VerificationReport {
        artifact_count: catalog.artifact_count(),
        warning_count,
    })
}

fn required_vector_string<'a>(
    vector: &'a Value,
    path: &Path,
    identifier: &str,
    field: &str,
) -> Result<&'a str, ContractArtifactError> {
    vector[field]
        .as_str()
        .ok_or_else(|| fixture_failure(path, format!("{identifier}: {field} must be a string")))
}

fn verify_positive_jcs_vectors(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let positives = corpus["positive"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "positive must be an array"))?;
    for vector in positives {
        let identifier = required_vector_string(vector, path, "positive", "id")?;
        let input = required_vector_string(vector, path, identifier, "input_json")?;
        let expected = required_vector_string(vector, path, identifier, "canonical_utf8")?;
        let expected_checksum = required_vector_string(vector, path, identifier, "checksum")?;
        validate_checksum(expected_checksum)
            .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
        let actual = canonicalize_slice(input.as_bytes())
            .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
        if actual != expected.as_bytes() || checksum(&actual) != expected_checksum {
            return Err(fixture_failure(
                path,
                format!("{identifier}: canonical bytes or checksum drifted"),
            ));
        }
    }
    Ok(())
}

fn verify_negative_jcs_vectors(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let negatives = corpus["negative"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "negative must be an array"))?;
    for vector in negatives {
        let identifier = required_vector_string(vector, path, "negative", "id")?;
        let input = required_vector_string(vector, path, identifier, "input_json")?;
        let expected_class = required_vector_string(vector, path, identifier, "error")?;
        match canonicalize_slice(input.as_bytes()) {
            Ok(_) => {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: negative vector was accepted"),
                ));
            }
            Err(error) if error.failure_class() != expected_class => {
                return Err(fixture_failure(
                    path,
                    format!(
                        "{identifier}: expected failure class {expected_class:?}, got {:?}",
                        error.failure_class()
                    ),
                ));
            }
            Err(_) => {}
        }
    }
    Ok(())
}

fn verify_non_string_map_vector(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let map_fixture = &corpus["non_string_map"];
    let entries = map_fixture["entries"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "non_string_map.entries must be an array"))?
        .iter()
        .map(|record| (record["key"].clone(), record["value"].clone()))
        .collect::<Vec<_>>();
    let expected = required_vector_string(map_fixture, path, "non_string_map", "canonical_utf8")?;
    let records = non_string_map_records(entries)
        .map_err(|error| fixture_failure(path, format!("non-string map: {error}")))?;
    let actual = canonicalize_value(&records)
        .map_err(|error| fixture_failure(path, format!("non-string map: {error}")))?;
    if actual == expected.as_bytes() {
        Ok(())
    } else {
        Err(fixture_failure(
            path,
            "non-string map record ordering drifted",
        ))
    }
}

/// Replay the cross-language canonical JSON vectors.
///
/// # Errors
///
/// Returns an error when a vector's bytes, checksum, or expected failure drifts.
pub fn verify_jcs_corpus(path: &Path) -> Result<(), ContractArtifactError> {
    let bytes = read(path)?;
    let corpus: Value =
        serde_json::from_slice(&bytes).map_err(|error| fixture_failure(path, error.to_string()))?;
    verify_positive_jcs_vectors(&corpus, path)?;
    verify_negative_jcs_vectors(&corpus, path)?;
    if corpus["profile"].as_str() != Some(PROFILE) {
        return Err(fixture_failure(path, "canonical profile identity drifted"));
    }
    verify_format_vectors(&corpus, path, "int64", validate_int64)?;
    verify_format_vectors(&corpus, path, "uint64", validate_uint64)?;
    verify_format_vectors(&corpus, path, "bytes", validate_bytes)?;
    verify_format_vectors(&corpus, path, "lowercase_public", validate_lowercase_public)?;
    verify_format_vectors(&corpus, path, "checksum", validate_checksum)?;
    verify_non_string_map_vector(&corpus, path)
}

fn verify_jcs_differential(path: &Path) -> Result<(), ContractArtifactError> {
    let corpus =
        decode_strict(&read(path)?).map_err(|source| ContractArtifactError::Canonical {
            path: path.to_owned(),
            source,
        })?;
    let cases = corpus["cases"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "cases must be an array"))?;
    for case in cases {
        let identifier = case["id"].as_str().unwrap_or("differential");
        let inputs = case["inputs"].as_array().ok_or_else(|| {
            fixture_failure(path, format!("{identifier}: inputs must be an array"))
        })?;
        let mut outputs = inputs.iter().map(|input| {
            let input = input.as_str().ok_or_else(|| {
                fixture_failure(path, format!("{identifier}: input must be a string"))
            })?;
            let output = canonicalize_slice(input.as_bytes())
                .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
            if canonicalize_slice(&output)
                .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?
                != output
            {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: canonicalization is not idempotent"),
                ));
            }
            Ok(output)
        });
        let first = outputs
            .next()
            .transpose()?
            .ok_or_else(|| fixture_failure(path, format!("{identifier}: inputs are empty")))?;
        for output in outputs {
            if output? != first {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: equivalent inputs diverged"),
                ));
            }
        }
    }
    Ok(())
}

fn verify_format_vectors(
    corpus: &Value,
    path: &Path,
    group: &str,
    validator: impl Fn(&str) -> Result<(), CanonicalJsonError>,
) -> Result<(), ContractArtifactError> {
    let format = &corpus["formats"][group];
    for value in format["positive"].as_array().into_iter().flatten() {
        let value = value.as_str().unwrap_or_default();
        validator(value).map_err(|error| ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: format!("{group} positive {value:?}: {error}"),
        })?;
    }
    for value in format["negative"].as_array().into_iter().flatten() {
        let value = value.as_str().unwrap_or_default();
        if validator(value).is_ok() {
            return Err(ContractArtifactError::Fixture {
                path: path.to_owned(),
                message: format!("{group} negative {value:?} was accepted"),
            });
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct NegativeFixture {
    source_utf8: String,
    claimed_checksum: String,
}

/// Verify one intentionally drifted checksum fixture.
///
/// This function succeeds only when the claim is valid, so the repository gate invokes
/// it against committed negative fixtures and requires a non-zero process result.
///
/// # Errors
///
/// Returns an error for malformed input or checksum mismatch.
pub fn verify_checksum_fixture(path: &Path) -> Result<(), ContractArtifactError> {
    let fixture: NegativeFixture =
        serde_json::from_slice(&read(path)?).map_err(|error| ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    validate_checksum(&fixture.claimed_checksum).map_err(|source| {
        ContractArtifactError::Canonical {
            path: path.to_owned(),
            source,
        }
    })?;
    let actual = checksum(fixture.source_utf8.as_bytes());
    if actual == fixture.claimed_checksum {
        Ok(())
    } else {
        Err(ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: format!(
                "checksum mismatch: claimed {}, actual {actual}",
                fixture.claimed_checksum
            ),
        })
    }
}

/// Resolve one catalog derivation to the paths and exact identities generators consume.
///
/// # Errors
///
/// Returns a typed catalog, I/O, or semantic-compilation error, or a missing marker for an
/// unknown derivation ID.
pub fn resolve_derivation_invocation(
    repository_root: &Path,
    derivation_id: &str,
) -> Result<ResolvedDerivationInvocation, ContractArtifactError> {
    let catalog = ContractCatalog::load_for_derivation(repository_root)?;
    let mut invocation = catalog
        .resolved_invocation(derivation_id)
        .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from(derivation_id)))?;
    for input in &mut invocation.artifact_inputs {
        let artifact = catalog
            .artifact(&input.artifact_id)
            .ok_or_else(|| ContractArtifactError::Missing(PathBuf::from(&input.artifact_id)))?;
        match input.view {
            super::catalog::ArtifactInputView::SourceBytes => {
                input.source_digest = Some(checksum(&read(
                    &repository_root.join(&artifact.authority_path),
                )?));
                if artifact.semantic_projection_source
                    != super::catalog::SemanticProjectionSource::Native
                {
                    input.canonical_digest = Some(
                        compile_artifact(repository_root, &catalog, artifact)?.canonical_digest,
                    );
                }
            }
            super::catalog::ArtifactInputView::CompiledSemantic => {
                let compiled = compile_artifact(repository_root, &catalog, artifact)?;
                input.canonical_digest = Some(compiled.canonical_digest);
                input.source_digest = Some(compiled.source_digest);
            }
        }
    }
    Ok(invocation)
}

/// Deterministic generator identity for the administrative CLI.
#[derive(Serialize)]
pub struct ContractToolIdentity<'a> {
    /// Executable identity.
    pub executable: &'a str,
    /// Package version.
    pub version: &'a str,
    /// Canonical JSON profile.
    pub canonical_json_profile: &'a str,
    /// Rust JCS library identity.
    pub rust_jcs: &'a str,
}

/// Construct the exact generator/verifier identity record.
#[must_use]
pub const fn identity() -> ContractToolIdentity<'static> {
    ContractToolIdentity {
        executable: "codefabric-contracts",
        version: env!("CARGO_PKG_VERSION"),
        canonical_json_profile: PROFILE,
        rust_jcs: "serde_json_canonicalizer 0.3.2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "fact-generation")]
    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    #[cfg(feature = "fact-generation")]
    fn assert_provider_profile_bindings() {
        let profiles = crate::registries::PROVIDER_RESOURCE_PROFILES;
        assert_eq!(profiles.len(), 3);
        assert!(profiles.iter().all(|profile| {
            profile.max_parallel_jobs_global > 0
                && profile.max_parallel_jobs_per_workspace > 0
                && profile.max_parallel_jobs_per_context > 0
                && profile.max_input_bytes > 0
                && profile.max_work_units > 0
                && profile.max_wall_millis > 0
                && profile.max_visited_nodes > 0
                && profile.max_traversal_depth > 0
                && profile.max_output_records > 0
                && profile.max_output_bytes > 0
                && profile.max_diagnostics > 0
                && profile.max_parser_workers > 0
                && profile.max_retained_tree_revisions > 0
                && profile.max_cpu_weight > 0
                && profile.max_memory_mib > 0
                && profile.cancellation_check_interval > 0
                && profile.cancellation_ack_millis > 0
                && (profile.retry_policy == "NO_RETRY") == (profile.max_retries == 0)
        }));
        let mut provider_codes = BTreeSet::new();
        for provider in crate::registries::PROVIDER_ENTRIES {
            assert!(provider_codes.insert(provider.provider_code));
            let profile = profiles
                .iter()
                .find(|profile| profile.profile_id == provider.resource_profile_id)
                .unwrap();
            assert!(profile.provider_ids.contains(&provider.provider_id));
        }
    }

    #[cfg(feature = "fact-generation")]
    fn assert_ruff_catalog() {
        let ruff_catalog = decode_strict(
            &read(
                &repository_root()
                    .join("contracts/generated/provider-raw-kinds/ruff-python-0-0-7.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let ruff_nodes = ruff_catalog["runtime_inventory"]["node_kinds"]
            .as_array()
            .unwrap();
        let ruff_tokens = ruff_catalog["runtime_inventory"]["token_kinds"]
            .as_array()
            .unwrap();
        assert_eq!(ruff_nodes.len(), RUFF_NODE_KINDS.len());
        assert_eq!(ruff_tokens.len(), RUFF_TOKEN_KINDS.len());
        assert!(ruff_nodes.iter().all(|entry| {
            entry["disposition"] == "normalize" && entry["canonical_kind_name"].is_string()
        }));
        let runtime_bytes = canonicalize_value(&ruff_catalog["runtime_inventory"]).unwrap();
        assert_eq!(
            ruff_catalog["runtime_inventory_fingerprint"]
                .as_str()
                .unwrap(),
            checksum(&runtime_bytes)
        );
    }

    #[test]
    fn verifier_profiles_parse_strictly() {
        assert_eq!(
            VerificationProfile::parse("full").unwrap(),
            VerificationProfile::Full
        );
        assert!(VerificationProfile::parse("Full").is_err());
    }

    #[cfg(feature = "fact-generation")]
    #[test]
    fn wp28_structural_acceptance() {
        let provider_bundle = decode_strict(
            &read(&repository_root().join("contracts/bundles/provider-bundle.json")).unwrap(),
        )
        .unwrap();
        let bundled_digests = provider_bundle["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| {
                (
                    member["artifact_id"].as_str().unwrap(),
                    member["canonical_digest"].as_str().unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for path in [
            "contracts/generated/provider-raw-kinds/tree-sitter-python-0-25-0.json",
            "contracts/generated/provider-raw-kinds/tree-sitter-rust-0-24-2.json",
        ] {
            let value = decode_strict(&read(&repository_root().join(path)).unwrap()).unwrap();
            let raw_kinds = value["runtime_inventory"]["raw_kinds"].as_array().unwrap();
            assert!(!raw_kinds.is_empty());
            let keys = raw_kinds
                .iter()
                .map(|entry| entry["raw_key"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(keys.len(), raw_kinds.len());
            assert!(raw_kinds.iter().all(|entry| {
                matches!(
                    entry["disposition"].as_str(),
                    Some("normalize" | "ignore" | "unsupported")
                ) && entry.get("canonical_kind_code").is_none()
                    && (entry["disposition"] == "normalize")
                        == entry["canonical_kind_name"].is_string()
            }));
            let runtime_bytes = canonicalize_value(&value["runtime_inventory"]).unwrap();
            assert_eq!(
                value["runtime_inventory_fingerprint"].as_str().unwrap(),
                checksum(&runtime_bytes)
            );
            let input_identities = value["input_identities"].as_array().unwrap();
            assert_eq!(input_identities.len(), 3);
            assert!(input_identities.iter().all(|identity| {
                bundled_digests
                    .get(identity["artifact_id"].as_str().unwrap())
                    .copied()
                    == identity["canonical_digest"].as_str()
            }));
        }
        assert_ruff_catalog();

        let entities = registry_value(
            repository_root(),
            &ContractCatalog::load(repository_root()).unwrap(),
            "codefabric.registry.ontology-entity-registry",
        )
        .unwrap();
        assert!(
            entities["records"]
                .as_array()
                .unwrap()
                .iter()
                .all(|record| {
                    record["kind_code"].as_u64().unwrap() <= 240
                        || record["kind_code"].as_u64().unwrap() >= 250
                })
        );
        let relations = registry_value(
            repository_root(),
            &ContractCatalog::load(repository_root()).unwrap(),
            "codefabric.registry.ontology-relation-registry",
        )
        .unwrap();
        assert!(
            relations["records"]
                .as_array()
                .unwrap()
                .iter()
                .all(|record| {
                    record["relation_code"].as_u64().unwrap() <= 130
                        || record["relation_code"].as_u64().unwrap() >= 140
                })
        );

        assert_provider_profile_bindings();
    }

    #[cfg(feature = "fact-generation")]
    #[test]
    fn wp28_negative_zero_state() {
        let outputs = render_outputs(repository_root()).unwrap();
        for path in [
            Path::new("contracts/generated/provider-raw-kinds/ruff-python-0-0-7.json"),
            Path::new("contracts/generated/provider-raw-kinds/tree-sitter-python-0-25-0.json"),
            Path::new("contracts/generated/provider-raw-kinds/tree-sitter-rust-0-24-2.json"),
            Path::new("src/generated/provider_raw_kinds.rs"),
        ] {
            assert_eq!(
                outputs.get(path).unwrap(),
                &read(&repository_root().join(path)).unwrap()
            );
        }
        let serving_ir = decode_strict(
            &read(&repository_root().join("contracts/schema/schema-contract-ir.json")).unwrap(),
        )
        .unwrap();
        let views = serving_ir["serving_projections"].as_array().unwrap();
        assert!(views.iter().any(|view| view["view_name"] == "files"));
        assert!(views.iter().any(|view| view["view_name"] == "syntax"));
        assert!(!views.iter().any(|view| {
            matches!(
                view["view_name"].as_str(),
                Some("tokens" | "annotations" | "python" | "rust" | "derived")
            )
        }));
    }

    #[cfg(feature = "fact-generation")]
    #[test]
    fn wp28_operational_acceptance() {
        let report = verify(repository_root(), VerificationProfile::Released).unwrap();
        assert_eq!(report.warning_count, 0);
        assert!(report.artifact_count >= 64);
    }
}
