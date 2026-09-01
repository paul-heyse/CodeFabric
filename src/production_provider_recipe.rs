//! Production construction of the exact CodeFabric v2.2 provider-admission transaction.
//!
//! The relation census, Arrow schemas, authority roles, coverage routing, and upstream API
//! surfaces in this module are compiled Rust over the exact provider enums. No serialized model,
//! ontology, manifest, generated registry, row-count comparison, or plan-text digest decides what
//! a provider is allowed to contribute. Digests below are used only to construct the typed,
//! domain-separated identities required by the boundary contracts.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arrow_schema::{FieldRef, SchemaRef};
use thiserror::Error;

use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::production_kernel::CompiledProviderAuthority;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::provider_admission::{
    DeclaredCoverageBinding, ExactProgrammaticProviderRuns, ProgrammaticProviderAdmissionOutcome,
    ProviderAdmissionError, ProviderAdmissionPlan, ProviderAuthorityClass, ProviderCoverageSource,
    ProviderNativeLane, ProviderRelationBinding, ProviderRelationIdentity, ProviderRelationPurpose,
    admit_provider_relations_programmatic,
};
use crate::provider_boundary::{
    BoundaryContractId, BoundaryOwnerId, CanonicalIdentityRole, ContractDisposition,
    CoordinateRole, FieldMeaning, IndependentContractAcceptance, ProviderApiFamily,
    ProviderArrowRelationContract, ProviderAuthorityRole, ProviderBoundaryContract,
    ProviderBoundaryContractRow, ProviderBoundaryField, ProviderHandlerId, ProviderId,
    ProviderInstallerId, ProviderInstallerIdentity, ProviderLocalIdentityRole, ProviderOracleId,
    ProviderRevision, RetentionPolicy, UnavailableBehavior, UpstreamApiSymbol,
};
use crate::provider_native_syntax::{
    NativeSyntaxRelation, ProviderNativeSyntaxRun, RUFF_COMPONENT_RELEASE,
    TREE_SITTER_PYTHON_GRAMMAR_RELEASE, TREE_SITTER_RUNTIME_RELEASE,
};
use crate::pyrefly_service::{AcceptedPyreflyRun, PyreflyRelation};
use crate::relation_ipc::{
    ContextPin, RelationId, RemainderReason, SchemaFingerprint, SourcePin, TerminalStatus,
};
use crate::rustc_relation_schema::{
    RUSTC_PUBLIC_RELEASE, RUSTC_RELATION_PROTOCOL_VERSION, RUSTC_TOOLCHAIN, RustcRelation,
};
use crate::rustc_service::TrustQualifiedRustcCompilation;
use crate::schema_contract::canonical_arrow_schema_fingerprint;

const RECIPE_RELEASE: &str = "codefabric-provider-admission-v2.2.0";

/// Independently supplied authority for one provider lane and its requested semantic scope.
///
/// `requested_units` is the application-owned request census. It is deliberately not inferred
/// from provider-emitted rows, so provider output cannot enlarge its own authority contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactProviderLaneAuthority {
    source_pin: SourcePin,
    context_pin: ContextPin,
    requested_units: NonZeroU64,
}

impl ExactProviderLaneAuthority {
    /// Construct one non-zero, independently supplied lane authority.
    ///
    /// # Errors
    ///
    /// Returns an error for zero source/context pins or a zero request census.
    pub fn try_new(
        source_pin: SourcePin,
        context_pin: ContextPin,
        requested_units: u64,
    ) -> Result<Self, ProductionProviderRecipeError> {
        if source_pin.0 == [0; 32] {
            return Err(ProductionProviderRecipeError::InvalidAuthority(
                "source pin uses the zero sentinel",
            ));
        }
        if context_pin.0 == [0; 32] {
            return Err(ProductionProviderRecipeError::InvalidAuthority(
                "context pin uses the zero sentinel",
            ));
        }
        let requested_units = NonZeroU64::new(requested_units).ok_or(
            ProductionProviderRecipeError::InvalidAuthority(
                "requested unit census uses the zero sentinel",
            ),
        )?;
        Ok(Self {
            source_pin,
            context_pin,
            requested_units,
        })
    }

    #[must_use]
    pub const fn source_pin(self) -> SourcePin {
        self.source_pin
    }

    #[must_use]
    pub const fn context_pin(self) -> ContextPin {
        self.context_pin
    }

    #[must_use]
    pub const fn requested_units(self) -> NonZeroU64 {
        self.requested_units
    }
}

/// Exact application-owned authority for the four provider lanes.
///
/// Tree-sitter and Ruff are one paired source-image request. Rust compiler compilation control
/// and owner-scoped MIR facts have different request censuses, while sharing one exact source and
/// compiler context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionProviderAuthority {
    native_syntax: ExactProviderLaneAuthority,
    pyrefly: ExactProviderLaneAuthority,
    rustc: ExactProviderLaneAuthority,
    rustc_owner_units: NonZeroU64,
}

impl ProductionProviderAuthority {
    /// Construct the complete provider request authority.
    ///
    /// # Errors
    ///
    /// Rejects a zero rustc owner request census.
    pub fn try_new(
        native_syntax: ExactProviderLaneAuthority,
        pyrefly: ExactProviderLaneAuthority,
        rustc: ExactProviderLaneAuthority,
        rustc_owner_units: u64,
    ) -> Result<Self, ProductionProviderRecipeError> {
        let rustc_owner_units = NonZeroU64::new(rustc_owner_units).ok_or(
            ProductionProviderRecipeError::InvalidAuthority(
                "rustc owner request census uses the zero sentinel",
            ),
        )?;
        Ok(Self {
            native_syntax,
            pyrefly,
            rustc,
            rustc_owner_units,
        })
    }
}

/// Exact application-owned DTOs accepted from the provider adapters.
///
/// An empty slice means the lane did not produce an accepted run. The production recipe still
/// installs its compiled contract and admission therefore reports every requested family as an
/// explicit unknown; it never turns absence into a successful empty relation.
#[derive(Clone, Copy)]
pub struct ProductionProviderRuns<'a> {
    native_syntax: &'a [ProviderNativeSyntaxRun],
    pyrefly: &'a [AcceptedPyreflyRun],
    rustc: &'a [TrustQualifiedRustcCompilation],
}

impl<'a> ProductionProviderRuns<'a> {
    #[must_use]
    pub const fn new(
        native_syntax: &'a [ProviderNativeSyntaxRun],
        pyrefly: &'a [AcceptedPyreflyRun],
        rustc: &'a [TrustQualifiedRustcCompilation],
    ) -> Self {
        Self {
            native_syntax,
            pyrefly,
            rustc,
        }
    }
}

/// Closed failures while compiling or applying the production provider recipe.
#[derive(Debug, Error)]
pub enum ProductionProviderRecipeError {
    #[error("production provider authority is invalid: {0}")]
    InvalidAuthority(&'static str),
    #[error("provider relation {relation} has no compiled field-role classification for {field}")]
    UnclassifiedField {
        relation: &'static str,
        field: String,
    },
    #[error("canonical Arrow schema identity failed for {relation}: {source}")]
    CanonicalSchema {
        relation: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Admission(#[from] ProviderAdmissionError),
}

/// Compile the exact four provider plans and atomically admit the accepted provider DTOs.
///
/// The candidate builder is consumed by the existing all-provider transaction. It is returned
/// only after all four lanes have been preflighted and registered. Missing Pyrefly, rustc, or
/// native-syntax runs remain explicit unknowns in the returned reports.
///
/// # Errors
///
/// Returns a compiled-descriptor or provider-admission failure. On error the consumed candidate
/// builder is not recoverable, preventing partial provider registration from escaping. The
/// compiled-authority capability is semantic; source/context pins and request counts remain
/// operational inputs.
pub(crate) fn admit_production_provider_relations(
    compiled_authority: &CompiledProviderAuthority,
    builder: ProgrammaticFabricEpochBuilder,
    authority: ProductionProviderAuthority,
    runs: ProductionProviderRuns<'_>,
) -> Result<ProgrammaticProviderAdmissionOutcome, ProductionProviderRecipeError> {
    let tree_sitter_plan = native_syntax_plan(
        compiled_authority,
        ProviderNativeLane::TreeSitter,
        authority.native_syntax,
    )?;
    let ruff_plan = native_syntax_plan(
        compiled_authority,
        ProviderNativeLane::Ruff,
        authority.native_syntax,
    )?;
    let pyrefly_plan = pyrefly_plan(compiled_authority, authority.pyrefly)?;
    let rustc_plan = rustc_plan(
        compiled_authority,
        authority.rustc,
        authority.rustc_owner_units,
    )?;

    Ok(admit_provider_relations_programmatic(
        builder,
        ExactProgrammaticProviderRuns::new(
            &tree_sitter_plan,
            &ruff_plan,
            runs.native_syntax,
            &pyrefly_plan,
            runs.pyrefly,
            &rustc_plan,
            runs.rustc,
        ),
    )?)
}

/// Closed provider-relation identity compiled into the v2.2 semantic release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderRelation {
    NativeSyntax(NativeSyntaxRelation),
    Pyrefly(PyreflyRelation),
    Rustc(RustcRelation),
}

/// Exact semantic roles assigned to one Arrow field by the compiled provider descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderFieldRole {
    meaning: FieldMeaning,
    provider_local_identity: ProviderLocalIdentityRole,
    canonical_identity: CanonicalIdentityRole,
    coordinate: CoordinateRole,
    retention: RetentionPolicy,
}

#[derive(Clone, Debug)]
struct ProviderFieldDescriptor {
    ordinal: usize,
    field: FieldRef,
    role: ProviderFieldRole,
}

/// Exhaustive application-owned descriptor for one exact provider relation.
///
/// The Arrow schema comes from the provider relation enum's compiled schema authority. Field
/// roles are closed exact-name mappings: an unclassified field rejects descriptor construction.
#[derive(Clone)]
pub(crate) struct ProviderRelationDescriptor {
    relation: ProviderRelation,
    relation_identity: &'static str,
    schema: SchemaRef,
    fields: Vec<ProviderFieldDescriptor>,
    upstream_symbol: &'static str,
    authority: ProviderAuthorityRole,
    purpose: ProviderRelationPurpose,
    coverage: ProviderCoverageSource,
}

impl ProviderRelation {
    const fn relation_identity(self) -> &'static str {
        match self {
            Self::NativeSyntax(relation) => relation.as_str(),
            Self::Pyrefly(relation) => relation.relation_id(),
            Self::Rustc(relation) => relation.relation_id(),
        }
    }

    fn schema(self) -> SchemaRef {
        match self {
            Self::NativeSyntax(relation) => relation.schema(),
            Self::Pyrefly(relation) => relation.schema(),
            Self::Rustc(relation) => relation.schema(),
        }
    }

    const fn lane(self) -> ProviderNativeLane {
        match self {
            Self::NativeSyntax(relation) => native_lane(relation),
            Self::Pyrefly(_) => ProviderNativeLane::Pyrefly,
            Self::Rustc(_) => ProviderNativeLane::Rustc,
        }
    }

    const fn upstream_symbol(self) -> &'static str {
        match self {
            Self::NativeSyntax(relation) => native_upstream_symbol(relation),
            Self::Pyrefly(relation) => pyrefly_upstream_symbol(relation),
            Self::Rustc(relation) => rustc_upstream_symbol(relation),
        }
    }

    const fn authority(self) -> ProviderAuthorityRole {
        match self {
            Self::NativeSyntax(relation) => native_authority(relation),
            Self::Pyrefly(relation) => pyrefly_authority(relation),
            Self::Rustc(_) => ProviderAuthorityRole::Primary,
        }
    }

    fn purpose_and_coverage(
        self,
    ) -> Result<(ProviderRelationPurpose, ProviderCoverageSource), ProductionProviderRecipeError>
    {
        match self {
            Self::NativeSyntax(relation) => native_coverage(relation),
            Self::Pyrefly(relation) => pyrefly_coverage(relation),
            Self::Rustc(relation) => rustc_coverage(relation),
        }
    }
}

impl ProviderRelationDescriptor {
    fn try_new(
        _compiled_authority: &CompiledProviderAuthority,
        relation: ProviderRelation,
    ) -> Result<Self, ProductionProviderRecipeError> {
        let relation_identity = relation.relation_identity();
        let schema = relation.schema();
        let fields = relation_fields(relation, &schema)?;
        let (purpose, coverage) = relation.purpose_and_coverage()?;
        Ok(Self {
            relation,
            relation_identity,
            schema,
            fields,
            upstream_symbol: relation.upstream_symbol(),
            authority: relation.authority(),
            purpose,
            coverage,
        })
    }
}

impl ProviderFieldDescriptor {
    fn boundary_field(&self) -> ProviderBoundaryField {
        ProviderBoundaryField {
            ordinal: self.ordinal,
            field: Arc::clone(&self.field),
            meaning: self.role.meaning,
            provider_local_identity: self.role.provider_local_identity,
            canonical_identity: self.role.canonical_identity,
            coordinate: self.role.coordinate,
            retention: self.role.retention,
        }
    }
}

fn native_syntax_plan(
    compiled_authority: &CompiledProviderAuthority,
    lane: ProviderNativeLane,
    authority: ExactProviderLaneAuthority,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let relations = NativeSyntaxRelation::ALL
        .into_iter()
        .filter(|relation| native_lane(*relation) == lane)
        .map(|relation| {
            Ok((
                ProviderRelationDescriptor::try_new(
                    compiled_authority,
                    ProviderRelation::NativeSyntax(relation),
                )?,
                authority.requested_units,
            ))
        })
        .collect::<Result<Vec<_>, ProductionProviderRecipeError>>()?;
    let (provider_kind, release) = match lane {
        ProviderNativeLane::TreeSitter => (
            "tree-sitter-python",
            format!(
                "tree-sitter={TREE_SITTER_RUNTIME_RELEASE};tree-sitter-python={TREE_SITTER_PYTHON_GRAMMAR_RELEASE}"
            ),
        ),
        ProviderNativeLane::Ruff => (
            "ruff-python",
            format!(
                "ruff-python-ast={RUFF_COMPONENT_RELEASE};ruff-python-parser={RUFF_COMPONENT_RELEASE};python-target=3.14"
            ),
        ),
        ProviderNativeLane::Pyrefly | ProviderNativeLane::Rustc => {
            unreachable!("native syntax plan only receives its two in-process lanes")
        }
    };
    build_plan(provider_kind, release, lane, authority, relations)
}

fn pyrefly_plan(
    compiled_authority: &CompiledProviderAuthority,
    authority: ExactProviderLaneAuthority,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let release_schema = PyreflyRelation::ModuleContext.schema();
    let metadata = release_schema.metadata();
    let provider_release = metadata.get("codefabric.provider_release").ok_or(
        ProductionProviderRecipeError::InvalidAuthority(
            "compiled Pyrefly schema omits its provider release",
        ),
    )?;
    let provider_revision = metadata.get("codefabric.provider_revision").ok_or(
        ProductionProviderRecipeError::InvalidAuthority(
            "compiled Pyrefly schema omits its source revision",
        ),
    )?;
    let protocol = metadata.get("codefabric.relation_protocol_version").ok_or(
        ProductionProviderRecipeError::InvalidAuthority(
            "compiled Pyrefly schema omits its protocol version",
        ),
    )?;
    let relations = PyreflyRelation::ALL
        .into_iter()
        .map(|relation| {
            Ok((
                ProviderRelationDescriptor::try_new(
                    compiled_authority,
                    ProviderRelation::Pyrefly(relation),
                )?,
                authority.requested_units,
            ))
        })
        .collect::<Result<Vec<_>, ProductionProviderRecipeError>>()?;
    build_plan(
        "pyrefly",
        format!("pyrefly={provider_release};source={provider_revision};protocol={protocol}"),
        ProviderNativeLane::Pyrefly,
        authority,
        relations,
    )
}

fn rustc_plan(
    compiled_authority: &CompiledProviderAuthority,
    authority: ExactProviderLaneAuthority,
    owner_units: NonZeroU64,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let relations = RustcRelation::ALL
        .into_iter()
        .map(|relation| {
            let requested_units = if matches!(
                relation,
                RustcRelation::Compilation
                    | RustcRelation::Diagnostic
                    | RustcRelation::Coverage
                    | RustcRelation::Remainder
            ) {
                authority.requested_units
            } else {
                owner_units
            };
            Ok((
                ProviderRelationDescriptor::try_new(
                    compiled_authority,
                    ProviderRelation::Rustc(relation),
                )?,
                requested_units,
            ))
        })
        .collect::<Result<Vec<_>, ProductionProviderRecipeError>>()?;
    build_plan(
        "rustc-public-mir",
        format!(
            "rustc-public={RUSTC_PUBLIC_RELEASE};toolchain={RUSTC_TOOLCHAIN};protocol={RUSTC_RELATION_PROTOCOL_VERSION}"
        ),
        ProviderNativeLane::Rustc,
        authority,
        relations,
    )
}

fn build_plan(
    provider_kind: &'static str,
    release: String,
    lane: ProviderNativeLane,
    authority: ExactProviderLaneAuthority,
    relations: Vec<(ProviderRelationDescriptor, NonZeroU64)>,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let provider_revision = ProviderRevision {
        provider_id: ProviderId(identity16("provider", &[provider_kind.as_bytes()])),
        release: release.clone(),
        source_revision: identity32(
            "provider-source-revision",
            &[provider_kind.as_bytes(), release.as_bytes()],
        ),
    };
    let installer = ProviderInstallerIdentity {
        installer_id: ProviderInstallerId(identity32(
            "provider-installer",
            &[
                RECIPE_RELEASE.as_bytes(),
                provider_kind.as_bytes(),
                release.as_bytes(),
            ],
        )),
        owner: BoundaryOwnerId(identity32(
            "provider-installer-owner",
            &[provider_kind.as_bytes()],
        )),
        provider_revision: provider_revision.clone(),
    };

    let mut rows = Vec::with_capacity(relations.len());
    let mut bindings = Vec::with_capacity(relations.len());
    for (relation, requested_units) in relations {
        debug_assert_eq!(relation.relation.lane(), lane);
        let family = ProviderApiFamily::new(relation.relation_identity.to_owned())
            .map_err(ProviderAdmissionError::from)?;
        let relation_id = RelationId(identity16(
            "provider-relation",
            &[relation.relation_identity.as_bytes()],
        ));
        let schema_fingerprint = SchemaFingerprint(
            canonical_arrow_schema_fingerprint(&relation.schema).map_err(|source| {
                ProductionProviderRecipeError::CanonicalSchema {
                    relation: relation.relation_identity,
                    source,
                }
            })?,
        );
        let handler_id = ProviderHandlerId(identity16(
            "provider-handler",
            &[
                provider_kind.as_bytes(),
                relation.relation_identity.as_bytes(),
                release.as_bytes(),
            ],
        ));
        let table_name = relation.relation_identity.replace('.', "_");
        rows.push(ProviderBoundaryContractRow {
            api_family: family.clone(),
            upstream_symbols: vec![
                UpstreamApiSymbol::new(relation.upstream_symbol)
                    .map_err(ProviderAdmissionError::from)?,
            ],
            relation: ProviderArrowRelationContract {
                relation_id,
                schema_fingerprint,
                fields: relation
                    .fields
                    .iter()
                    .map(ProviderFieldDescriptor::boundary_field)
                    .collect(),
                schema: Arc::clone(&relation.schema),
            },
            authority: relation.authority,
            disposition: ContractDisposition::Required,
            unavailable_behavior: UnavailableBehavior {
                status: TerminalStatus::Unknown,
                allowed_reasons: vec![
                    RemainderReason::Unknown,
                    RemainderReason::ProviderUnavailable,
                    RemainderReason::ResourceLimit,
                    RemainderReason::Cancelled,
                    RemainderReason::InvalidSource,
                    RemainderReason::Unsupported,
                ],
            },
            oracle_id: ProviderOracleId(identity32(
                "provider-oracle",
                &[
                    RECIPE_RELEASE.as_bytes(),
                    provider_kind.as_bytes(),
                    relation.relation_identity.as_bytes(),
                ],
            )),
        });
        bindings.push(ProviderRelationBinding {
            provider_relation: ProviderRelationIdentity::try_new(relation.relation_identity)?,
            api_family: family,
            lane,
            role: raw_role(lane),
            table_name,
            source_schema_identity: Arc::from(format!(
                "{RECIPE_RELEASE}:{}",
                relation.relation_identity
            )),
            handler_id,
            authority_class: ProviderAuthorityClass::ProviderNative,
            purpose: relation.purpose,
            requested_units: requested_units.get(),
            coverage: relation.coverage,
        });
    }
    let contract_id = contract_identity(provider_kind, &release, &rows);
    Ok(ProviderAdmissionPlan {
        provider_kind: Arc::from(provider_kind),
        expected_source_pin: authority.source_pin,
        expected_context_pin: authority.context_pin,
        installer,
        contract: ProviderBoundaryContract {
            contract_id: BoundaryContractId(contract_id),
            contract_revision: 1,
            provider_revision,
            acceptance: IndependentContractAcceptance {
                author_owner: BoundaryOwnerId(identity32(
                    "provider-contract-author",
                    &[b"codefabric-fact-contract"],
                )),
                reviewer_owner: BoundaryOwnerId(identity32(
                    "provider-contract-reviewer",
                    &[b"codefabric-fabric-boundary"],
                )),
                acceptance_authority: BoundaryOwnerId(identity32(
                    "provider-contract-acceptance",
                    &[b"codefabric-release-governance"],
                )),
            },
            rows,
        },
        bindings,
    })
}

fn native_coverage(
    relation: NativeSyntaxRelation,
) -> Result<(ProviderRelationPurpose, ProviderCoverageSource), ProductionProviderRecipeError> {
    let (coverage_relation, family) = match relation {
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::RuffRun
        | NativeSyntaxRelation::RuffCoverage
        | NativeSyntaxRelation::RuffRemainder
        | NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => {
            return Ok((
                ProviderRelationPurpose::ControlEvidence,
                ProviderCoverageSource::StructuralPresence,
            ));
        }
        NativeSyntaxRelation::TreeSitterCstNode => (
            NativeSyntaxRelation::TreeSitterCoverage.as_str(),
            "tree_sitter.cst_node",
        ),
        NativeSyntaxRelation::TreeSitterChangedRange => (
            NativeSyntaxRelation::TreeSitterCoverage.as_str(),
            "tree_sitter.changed_range",
        ),
        NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => (
            NativeSyntaxRelation::TreeSitterCoverage.as_str(),
            "tree_sitter.recovery_diagnostic",
        ),
        NativeSyntaxRelation::RuffToken => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.token")
        }
        NativeSyntaxRelation::RuffComment => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.comment")
        }
        NativeSyntaxRelation::RuffDirective => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.directive",
        ),
        NativeSyntaxRelation::RuffStringRegion => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.string_region",
        ),
        NativeSyntaxRelation::RuffDocstring => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.docstring",
        ),
        NativeSyntaxRelation::RuffContinuationLine => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.continuation_line",
        ),
        NativeSyntaxRelation::RuffAstNode => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.ast_node")
        }
        NativeSyntaxRelation::RuffParseDiagnostic => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.parse_diagnostic",
        ),
        NativeSyntaxRelation::RuffScope => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.scope")
        }
        NativeSyntaxRelation::RuffBinding => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.binding")
        }
        NativeSyntaxRelation::RuffReference => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.reference",
        ),
        NativeSyntaxRelation::RuffUnknownSymbol => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.unknown_symbol",
        ),
        NativeSyntaxRelation::RuffSemanticEdge => (
            NativeSyntaxRelation::RuffCoverage.as_str(),
            "ruff.semantic_edge",
        ),
        NativeSyntaxRelation::RuffImport => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.import")
        }
        NativeSyntaxRelation::RuffExport => {
            (NativeSyntaxRelation::RuffCoverage.as_str(), "ruff.export")
        }
    };
    Ok((
        ProviderRelationPurpose::SemanticFact,
        declared_coverage(
            coverage_relation,
            family,
            "family",
            "terminal_status",
            Some("remainder_reason"),
            None,
            native_reason_map(),
        )?,
    ))
}

fn pyrefly_coverage(
    relation: PyreflyRelation,
) -> Result<(ProviderRelationPurpose, ProviderCoverageSource), ProductionProviderRecipeError> {
    if matches!(
        relation,
        PyreflyRelation::ModuleContext | PyreflyRelation::Coverage
    ) {
        return Ok((
            ProviderRelationPurpose::ControlEvidence,
            ProviderCoverageSource::StructuralPresence,
        ));
    }
    let family = match relation {
        PyreflyRelation::TypeShape
        | PyreflyRelation::TypeComponent
        | PyreflyRelation::TypeTrait
        | PyreflyRelation::LocatedType => "computed_types",
        PyreflyRelation::CallTarget => "call_targets",
        PyreflyRelation::Member => "members",
        PyreflyRelation::Diagnostic => "diagnostics",
        PyreflyRelation::AffectedModule => "affected_modules",
        PyreflyRelation::ModuleContext | PyreflyRelation::Coverage => unreachable!(),
    };
    Ok((
        ProviderRelationPurpose::SemanticFact,
        declared_coverage(
            PyreflyRelation::Coverage.relation_id(),
            family,
            "fact_family",
            "completeness",
            Some("remainder_reason"),
            Some("unknown_semantics"),
            pyrefly_reason_map(),
        )?,
    ))
}

fn rustc_coverage(
    relation: RustcRelation,
) -> Result<(ProviderRelationPurpose, ProviderCoverageSource), ProductionProviderRecipeError> {
    if matches!(relation, RustcRelation::Coverage | RustcRelation::Remainder) {
        return Ok((
            ProviderRelationPurpose::ControlEvidence,
            ProviderCoverageSource::StructuralPresence,
        ));
    }
    Ok((
        ProviderRelationPurpose::SemanticFact,
        declared_coverage(
            RustcRelation::Coverage.relation_id(),
            relation.relation_id(),
            "fact_family",
            "completeness",
            None,
            Some("unknown_semantics"),
            BTreeMap::new(),
        )?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn declared_coverage(
    relation_identity: &'static str,
    family_value: &'static str,
    family_column: &'static str,
    status_column: &'static str,
    remainder_reason_column: Option<&'static str>,
    unknown_semantics_column: Option<&'static str>,
    remainder_reason_map: BTreeMap<String, RemainderReason>,
) -> Result<ProviderCoverageSource, ProductionProviderRecipeError> {
    Ok(ProviderCoverageSource::ProviderDeclared(
        DeclaredCoverageBinding {
            relation_identity: ProviderRelationIdentity::try_new(relation_identity)?,
            family_value: family_value.to_owned(),
            family_column: family_column.to_owned(),
            requested_units_column: "requested_units".to_owned(),
            completed_units_column: "completed_units".to_owned(),
            status_column: status_column.to_owned(),
            remainder_reason_column: remainder_reason_column.map(str::to_owned),
            unknown_semantics_column: unknown_semantics_column.map(str::to_owned),
            remainder_reason_map,
        },
    ))
}

fn native_reason_map() -> BTreeMap<String, RemainderReason> {
    BTreeMap::from([
        ("source-invalid".to_owned(), RemainderReason::InvalidSource),
        (
            "provider-unavailable".to_owned(),
            RemainderReason::ProviderUnavailable,
        ),
        ("resource-limit".to_owned(), RemainderReason::ResourceLimit),
        ("cancelled".to_owned(), RemainderReason::Cancelled),
        ("unsupported".to_owned(), RemainderReason::Unsupported),
    ])
}

fn pyrefly_reason_map() -> BTreeMap<String, RemainderReason> {
    BTreeMap::from([
        (
            "QUERY_RETURNED_NONE".to_owned(),
            RemainderReason::ProviderUnavailable,
        ),
        (
            "NO_STRUCTURAL_CALL_SITE_CENSUS".to_owned(),
            RemainderReason::Unsupported,
        ),
        (
            "QUERY_REQUIRES_CLASS_NAME_NO_CLASS_CENSUS".to_owned(),
            RemainderReason::Unsupported,
        ),
        (
            "TYPE_TABLE_UNAVAILABLE_FOR_MEMBER_CANDIDATES".to_owned(),
            RemainderReason::ProviderUnavailable,
        ),
        (
            "STRUCTURED_DIAGNOSTIC_API_UNAVAILABLE".to_owned(),
            RemainderReason::Unsupported,
        ),
        (
            "PINNED_QUERY_EXPOSES_NO_ACTUAL_AFFECTED_SET".to_owned(),
            RemainderReason::Unsupported,
        ),
    ])
}

const fn native_lane(relation: NativeSyntaxRelation) -> ProviderNativeLane {
    match relation {
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::TreeSitterCstNode
        | NativeSyntaxRelation::TreeSitterChangedRange
        | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => ProviderNativeLane::TreeSitter,
        NativeSyntaxRelation::RuffRun
        | NativeSyntaxRelation::RuffCoverage
        | NativeSyntaxRelation::RuffRemainder
        | NativeSyntaxRelation::RuffToken
        | NativeSyntaxRelation::RuffComment
        | NativeSyntaxRelation::RuffDirective
        | NativeSyntaxRelation::RuffStringRegion
        | NativeSyntaxRelation::RuffDocstring
        | NativeSyntaxRelation::RuffContinuationLine
        | NativeSyntaxRelation::RuffAstNode
        | NativeSyntaxRelation::RuffParseDiagnostic
        | NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence
        | NativeSyntaxRelation::RuffScope
        | NativeSyntaxRelation::RuffBinding
        | NativeSyntaxRelation::RuffReference
        | NativeSyntaxRelation::RuffUnknownSymbol
        | NativeSyntaxRelation::RuffSemanticEdge
        | NativeSyntaxRelation::RuffImport
        | NativeSyntaxRelation::RuffExport => ProviderNativeLane::Ruff,
    }
}

const fn raw_role(lane: ProviderNativeLane) -> FabricSchemaRole {
    match lane {
        ProviderNativeLane::TreeSitter => FabricSchemaRole::RawTreeSitter,
        ProviderNativeLane::Ruff => FabricSchemaRole::RawRuff,
        ProviderNativeLane::Pyrefly => FabricSchemaRole::RawPyrefly,
        ProviderNativeLane::Rustc => FabricSchemaRole::RawRustc,
    }
}

const fn native_authority(relation: NativeSyntaxRelation) -> ProviderAuthorityRole {
    match relation {
        NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => {
            ProviderAuthorityRole::Corroborating
        }
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::TreeSitterCstNode
        | NativeSyntaxRelation::TreeSitterChangedRange
        | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic
        | NativeSyntaxRelation::RuffRun
        | NativeSyntaxRelation::RuffCoverage
        | NativeSyntaxRelation::RuffRemainder
        | NativeSyntaxRelation::RuffToken
        | NativeSyntaxRelation::RuffComment
        | NativeSyntaxRelation::RuffDirective
        | NativeSyntaxRelation::RuffStringRegion
        | NativeSyntaxRelation::RuffDocstring
        | NativeSyntaxRelation::RuffContinuationLine
        | NativeSyntaxRelation::RuffAstNode
        | NativeSyntaxRelation::RuffParseDiagnostic
        | NativeSyntaxRelation::RuffScope
        | NativeSyntaxRelation::RuffBinding
        | NativeSyntaxRelation::RuffReference
        | NativeSyntaxRelation::RuffUnknownSymbol
        | NativeSyntaxRelation::RuffSemanticEdge
        | NativeSyntaxRelation::RuffImport
        | NativeSyntaxRelation::RuffExport => ProviderAuthorityRole::Primary,
    }
}

const fn pyrefly_authority(relation: PyreflyRelation) -> ProviderAuthorityRole {
    match relation {
        PyreflyRelation::Diagnostic | PyreflyRelation::AffectedModule => {
            ProviderAuthorityRole::NarrowEnrichment
        }
        PyreflyRelation::ModuleContext
        | PyreflyRelation::TypeShape
        | PyreflyRelation::TypeComponent
        | PyreflyRelation::TypeTrait
        | PyreflyRelation::LocatedType
        | PyreflyRelation::CallTarget
        | PyreflyRelation::Member
        | PyreflyRelation::Coverage => ProviderAuthorityRole::Primary,
    }
}

const fn native_upstream_symbol(relation: NativeSyntaxRelation) -> &'static str {
    match relation {
        NativeSyntaxRelation::TreeSitterRun => "tree_sitter::Parser::parse",
        NativeSyntaxRelation::TreeSitterCoverage => "codefabric::tree_sitter::coverage",
        NativeSyntaxRelation::TreeSitterRemainder => "codefabric::tree_sitter::remainder",
        NativeSyntaxRelation::TreeSitterCstNode => "tree_sitter::Tree::root_node",
        NativeSyntaxRelation::TreeSitterChangedRange => "tree_sitter::Tree::changed_ranges",
        NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => {
            "tree_sitter::Node::is_error/is_missing"
        }
        NativeSyntaxRelation::RuffRun => "ruff_python_parser::parse_module",
        NativeSyntaxRelation::RuffCoverage => "codefabric::ruff::coverage",
        NativeSyntaxRelation::RuffRemainder => "codefabric::ruff::remainder",
        NativeSyntaxRelation::RuffToken => "ruff_python_parser::lex",
        NativeSyntaxRelation::RuffComment => "ruff_python_trivia::CommentRanges",
        NativeSyntaxRelation::RuffDirective => "ruff_python_trivia::CommentRanges",
        NativeSyntaxRelation::RuffStringRegion => "ruff_python_ast::StringFlags",
        NativeSyntaxRelation::RuffDocstring => "ruff_python_ast::ModModule",
        NativeSyntaxRelation::RuffContinuationLine => "ruff_python_trivia::BackwardsTokenizer",
        NativeSyntaxRelation::RuffAstNode => "ruff_python_ast::AnyNodeRef",
        NativeSyntaxRelation::RuffParseDiagnostic => "ruff_python_parser::ParseError",
        NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => {
            "tree_sitter::Node::is_error/is_missing"
        }
        NativeSyntaxRelation::RuffScope => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffBinding => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffReference => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffUnknownSymbol => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffSemanticEdge => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffImport => "ruff_python_semantic::SemanticModel",
        NativeSyntaxRelation::RuffExport => "ruff_python_semantic::SemanticModel",
    }
}

const fn pyrefly_upstream_symbol(relation: PyreflyRelation) -> &'static str {
    match relation {
        PyreflyRelation::ModuleContext => "pyrefly::query::Query::add_files",
        PyreflyRelation::TypeShape
        | PyreflyRelation::TypeComponent
        | PyreflyRelation::TypeTrait
        | PyreflyRelation::LocatedType => "pyrefly::query::Query::get_type_table_in_file",
        PyreflyRelation::CallTarget => "pyrefly::query::Query::get_callees_with_location",
        PyreflyRelation::Member => "pyrefly::query::Query::get_attributes",
        PyreflyRelation::Diagnostic => "pyrefly::query::Query::add_files",
        PyreflyRelation::AffectedModule => "pyrefly::query::Query::change_files",
        PyreflyRelation::Coverage => "codefabric::pyrefly::coverage",
    }
}

const fn rustc_upstream_symbol(relation: RustcRelation) -> &'static str {
    match relation {
        RustcRelation::Compilation => "rustc_public::local_crate/all_local_items",
        RustcRelation::PublicItem => "rustc_public::CrateDef",
        RustcRelation::Type => "rustc_public::ty::TyKind",
        RustcRelation::Instance => "rustc_public::Instance::resolve",
        RustcRelation::MirBody => "rustc_public::mir::Body",
        RustcRelation::MirBlock => "rustc_public::mir::BasicBlock",
        RustcRelation::MirLocal => "rustc_public::mir::LocalDecl",
        RustcRelation::MirPlace => "rustc_public::mir::Place",
        RustcRelation::MirOperand => "rustc_public::mir::Operand",
        RustcRelation::MirRvalue => "rustc_public::mir::Rvalue",
        RustcRelation::MirStatement => "rustc_public::mir::StatementKind",
        RustcRelation::MirTerminator => "rustc_public::mir::TerminatorKind",
        RustcRelation::CfgEdge => "rustc_public::mir::Terminator::successors",
        RustcRelation::Call => "rustc_public::Instance::resolve",
        RustcRelation::Access => "rustc_public::mir::Place",
        RustcRelation::Diagnostic => "rustc_driver::diagnostic-boundary",
        RustcRelation::Coverage => "codefabric::rustc::coverage",
        RustcRelation::Remainder => "codefabric::rustc::remainder",
    }
}

fn relation_fields(
    relation: ProviderRelation,
    schema: &SchemaRef,
) -> Result<Vec<ProviderFieldDescriptor>, ProductionProviderRecipeError> {
    schema
        .fields()
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            Ok(ProviderFieldDescriptor {
                ordinal,
                field: Arc::clone(field),
                role: compiled_provider_field_role(relation, field.name())?,
            })
        })
        .collect()
}

const fn field_role(
    meaning: FieldMeaning,
    provider_local_identity: ProviderLocalIdentityRole,
    canonical_identity: CanonicalIdentityRole,
    coordinate: CoordinateRole,
    retention: RetentionPolicy,
) -> ProviderFieldRole {
    ProviderFieldRole {
        meaning,
        provider_local_identity,
        canonical_identity,
        coordinate,
        retention,
    }
}

fn relation_field_is_compiled(relation: ProviderRelation, name: &str) -> bool {
    match relation {
        ProviderRelation::NativeSyntax(relation) => {
            native_relation_field_is_compiled(relation, name)
        }
        ProviderRelation::Pyrefly(relation) => pyrefly_relation_field_is_compiled(relation, name),
        ProviderRelation::Rustc(relation) => rustc_relation_field_is_compiled(relation, name),
    }
}

fn native_common_field(name: &str) -> bool {
    matches!(
        name,
        "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "file_id"
            | "content_digest"
            | "source_generation"
    )
}

#[allow(clippy::too_many_lines)]
fn native_relation_field_is_compiled(relation: NativeSyntaxRelation, name: &str) -> bool {
    native_common_field(name)
        || match relation {
            NativeSyntaxRelation::TreeSitterRun | NativeSyntaxRelation::RuffRun => matches!(
                name,
                "provider_revision" | "catalog_id" | "inventory_fingerprint" | "grammar_release"
            ),
            NativeSyntaxRelation::TreeSitterCoverage | NativeSyntaxRelation::RuffCoverage => {
                matches!(
                    name,
                    "family"
                        | "requested_units"
                        | "completed_units"
                        | "terminal_status"
                        | "remainder_reason"
                )
            }
            NativeSyntaxRelation::TreeSitterRemainder | NativeSyntaxRelation::RuffRemainder => {
                matches!(name, "family" | "reason" | "detail")
            }
            NativeSyntaxRelation::TreeSitterCstNode => matches!(
                name,
                "provider_local_node_id"
                    | "parent_provider_local_node_id"
                    | "raw_kind_id"
                    | "raw_kind"
                    | "field_name"
                    | "start_byte"
                    | "end_byte"
                    | "named"
                    | "extra"
                    | "error"
                    | "missing"
                    | "ordinal"
                    | "depth"
                    | "raw_kind_disposition"
            ),
            NativeSyntaxRelation::TreeSitterChangedRange => {
                matches!(name, "range_ordinal" | "start_byte" | "end_byte")
            }
            NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => matches!(
                name,
                "provider_local_node_id" | "recovery_kind" | "raw_kind" | "start_byte" | "end_byte"
            ),
            NativeSyntaxRelation::RuffToken => matches!(
                name,
                "token_ordinal"
                    | "raw_kind_id"
                    | "raw_kind"
                    | "token_class"
                    | "start_byte"
                    | "end_byte"
                    | "line"
                    | "column"
                    | "spelling_kind"
                    | "spelling_value"
                    | "provider_local_ast_id"
            ),
            NativeSyntaxRelation::RuffComment => {
                matches!(
                    name,
                    "start_byte" | "end_byte" | "placement" | "block_member"
                )
            }
            NativeSyntaxRelation::RuffDirective => matches!(
                name,
                "directive_kind" | "start_byte" | "end_byte" | "provider_local_target_id"
            ),
            NativeSyntaxRelation::RuffStringRegion => matches!(
                name,
                "start_byte" | "end_byte" | "multiline" | "interpolated" | "provider_local_ast_id"
            ),
            NativeSyntaxRelation::RuffDocstring => {
                matches!(name, "start_byte" | "end_byte" | "provider_local_owner_id")
            }
            NativeSyntaxRelation::RuffContinuationLine => name == "start_byte",
            NativeSyntaxRelation::RuffAstNode => matches!(
                name,
                "provider_local_ast_id"
                    | "parent_provider_local_ast_id"
                    | "raw_kind_id"
                    | "raw_kind"
                    | "ast_category"
                    | "child_role"
                    | "start_byte"
                    | "end_byte"
                    | "line"
                    | "column"
                    | "child_ordinal"
                    | "source_ordinal"
                    | "evaluation_ordinal"
                    | "explicit_parenthesized"
                    | "raw_kind_disposition"
            ),
            NativeSyntaxRelation::RuffParseDiagnostic => matches!(
                name,
                "diagnostic_ordinal" | "diagnostic_kind" | "message" | "start_byte" | "end_byte"
            ),
            NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => {
                matches!(
                    name,
                    "diagnostic_ordinal" | "tree_sitter_provider_local_node_id"
                )
            }
            NativeSyntaxRelation::RuffScope => matches!(
                name,
                "scope_id" | "parent_scope_id" | "scope_kind" | "name" | "start_byte" | "end_byte"
            ),
            NativeSyntaxRelation::RuffBinding => matches!(
                name,
                "binding_id"
                    | "scope_id"
                    | "name"
                    | "binding_kind"
                    | "target_form"
                    | "start_byte"
                    | "end_byte"
            ),
            NativeSyntaxRelation::RuffReference => matches!(
                name,
                "reference_id"
                    | "scope_id"
                    | "name"
                    | "reference_class"
                    | "resolution"
                    | "target_id"
                    | "start_byte"
                    | "end_byte"
                    | "unknown_reason"
            ),
            NativeSyntaxRelation::RuffUnknownSymbol => {
                matches!(name, "unknown_symbol_id" | "scope_id" | "name" | "reason")
            }
            NativeSyntaxRelation::RuffSemanticEdge => {
                matches!(name, "subject_id" | "object_id" | "edge_kind")
            }
            NativeSyntaxRelation::RuffImport => matches!(
                name,
                "import_id"
                    | "scope_id"
                    | "import_kind"
                    | "relative_level"
                    | "source_name"
                    | "alias_name"
                    | "star_import"
                    | "target_module_id"
                    | "target_module_name"
                    | "ruff_qualified_name"
                    | "resolution"
                    | "imported_entity_id"
                    | "imported_name"
                    | "local_binding_id"
                    | "unknown_reason"
                    | "start_byte"
                    | "end_byte"
            ),
            NativeSyntaxRelation::RuffExport => matches!(
                name,
                "export_id"
                    | "name"
                    | "target_id"
                    | "reexport"
                    | "export_status"
                    | "start_byte"
                    | "end_byte"
            ),
        }
}

fn pyrefly_common_field(name: &str) -> bool {
    matches!(
        name,
        "provider_run_id"
            | "analysis_context_id"
            | "module_id"
            | "file_id"
            | "content_digest"
            | "semantic_environment_id"
            | "source_generation"
    )
}

fn pyrefly_relation_field_is_compiled(relation: PyreflyRelation, name: &str) -> bool {
    pyrefly_common_field(name)
        || match relation {
            PyreflyRelation::ModuleContext => matches!(
                name,
                "module_name"
                    | "provider_release"
                    | "provider_revision"
                    | "requested_module_require_tier"
                    | "dependency_require_tier"
                    | "source_byte_length"
                    | "long_lived_context"
            ),
            PyreflyRelation::TypeShape => matches!(
                name,
                "local_type_index"
                    | "structural_hash"
                    | "shape_kind"
                    | "name"
                    | "unspecified_type_arg_count"
                    | "is_staticmethod"
            ),
            PyreflyRelation::TypeComponent => matches!(
                name,
                "owner_local_type_index"
                    | "component_role"
                    | "component_ordinal"
                    | "referenced_local_type_index"
            ),
            PyreflyRelation::TypeTrait => {
                matches!(name, "owner_local_type_index" | "trait_kind")
            }
            PyreflyRelation::LocatedType => matches!(
                name,
                "occurrence_ordinal"
                    | "start_byte"
                    | "end_byte"
                    | "local_type_index"
                    | "type_role"
                    | "provider_start_line"
                    | "provider_start_column"
                    | "provider_end_line"
                    | "provider_end_column"
            ),
            PyreflyRelation::CallTarget => matches!(
                name,
                "call_occurrence_ordinal"
                    | "start_byte"
                    | "end_byte"
                    | "target_ordinal"
                    | "callee_kind"
                    | "qualified_target"
                    | "class_name"
                    | "resolution_state"
            ),
            PyreflyRelation::Member => matches!(
                name,
                "class_name"
                    | "member_ordinal"
                    | "member_name"
                    | "member_kind"
                    | "annotation_rendering"
                    | "annotation_representation"
                    | "is_final"
                    | "discovery_basis"
            ),
            PyreflyRelation::Diagnostic => matches!(
                name,
                "diagnostic_ordinal"
                    | "rendered_text"
                    | "structured_fields_available"
                    | "source_locator_redacted"
            ),
            PyreflyRelation::AffectedModule => matches!(
                name,
                "affected_module_id"
                    | "evidence_source"
                    | "exact_recheck_proven"
                    | "refresh_policy"
            ),
            PyreflyRelation::Coverage => matches!(
                name,
                "fact_family"
                    | "exact_authority_surface"
                    | "requested_units"
                    | "completed_units"
                    | "emitted_rows"
                    | "completeness"
                    | "remainder_reason"
                    | "unknown_semantics"
            ),
        }
}

fn rustc_common_field(name: &str) -> bool {
    matches!(
        name,
        "provider_run_id"
            | "compilation_unit_id"
            | "owner_id"
            | "source_generation"
            | "source_file_id"
            | "source_content_digest"
            | "stable_crate_id"
            | "def_path_hash"
    )
}

#[allow(clippy::too_many_lines)]
fn rustc_relation_field_is_compiled(relation: RustcRelation, name: &str) -> bool {
    rustc_common_field(name)
        || match relation {
            RustcRelation::Compilation => matches!(
                name,
                "crate_name"
                    | "is_local_crate"
                    | "local_item_count"
                    | "body_owner_count"
                    | "rustc_release"
                    | "rustc_toolchain"
                    | "stable_identity_authority"
                    | "source_hygiene_authority"
            ),
            RustcRelation::PublicItem => matches!(
                name,
                "qualified_name"
                    | "item_kind"
                    | "has_body"
                    | "is_foreign_item"
                    | "requires_monomorphization"
                    | "type_key"
                    | "span_file"
                    | "span_start_byte"
                    | "span_end_byte"
                    | "span_start_line"
                    | "span_end_line"
                    | "span_start_column"
                    | "span_end_column"
                    | "expansion_kind"
                    | "in_external_macro"
            ),
            RustcRelation::Type => matches!(
                name,
                "type_key"
                    | "type_kind"
                    | "definition_path"
                    | "definition_stable_crate_id"
                    | "definition_def_path_hash"
                    | "component_role"
                    | "component_ordinal"
                    | "component_type_key"
                    | "scalar_value"
                    | "mutability"
            ),
            RustcRelation::Instance => matches!(
                name,
                "instance_key"
                    | "definition_path"
                    | "definition_stable_crate_id"
                    | "definition_def_path_hash"
                    | "instance_kind"
                    | "generic_argument_count"
                    | "specialized_type_key"
                    | "has_body"
                    | "is_foreign_item"
                    | "mangled_name"
                    | "resolution_state"
            ),
            RustcRelation::MirBody => matches!(
                name,
                "block_count"
                    | "local_count"
                    | "argument_count"
                    | "debug_variable_count"
                    | "spread_argument_local"
                    | "span_file"
                    | "span_start_byte"
                    | "span_end_byte"
                    | "span_start_line"
                    | "span_end_line"
                    | "span_start_column"
                    | "span_end_column"
                    | "expansion_kind"
            ),
            RustcRelation::MirBlock => {
                matches!(
                    name,
                    "block_index" | "statement_count" | "terminator_kind" | "is_entry"
                )
            }
            RustcRelation::MirLocal => matches!(
                name,
                "local_index"
                    | "local_role"
                    | "type_key"
                    | "mutability"
                    | "span_file"
                    | "span_start_byte"
                    | "span_end_byte"
                    | "span_start_line"
                    | "span_end_line"
                    | "span_start_column"
                    | "span_end_column"
                    | "expansion_kind"
            ),
            RustcRelation::MirPlace => matches!(
                name,
                "place_id"
                    | "block_index"
                    | "slot_kind"
                    | "slot_index"
                    | "occurrence_role"
                    | "occurrence_ordinal"
                    | "base_local"
                    | "projection_ordinal"
                    | "projection_kind"
                    | "projection_local_or_field"
                    | "offset"
                    | "min_length"
                    | "slice_to"
                    | "from_end"
                    | "projection_type_key"
            ),
            RustcRelation::MirOperand => matches!(
                name,
                "operand_id"
                    | "block_index"
                    | "slot_kind"
                    | "slot_index"
                    | "parent_role"
                    | "operand_ordinal"
                    | "operand_kind"
                    | "place_id"
                    | "type_key"
                    | "constant_kind"
                    | "runtime_check_kind"
            ),
            RustcRelation::MirRvalue => matches!(
                name,
                "block_index"
                    | "statement_index"
                    | "rvalue_kind"
                    | "result_type_key"
                    | "operator_kind"
                    | "cast_kind"
                    | "aggregate_kind"
                    | "operand_count"
                    | "source_place_id"
                    | "region_kind"
                    | "mutability"
            ),
            RustcRelation::MirStatement => matches!(
                name,
                "block_index"
                    | "statement_index"
                    | "raw_statement_kind"
                    | "normalized_effect"
                    | "source_scope"
                    | "span_file"
                    | "span_start_byte"
                    | "span_end_byte"
                    | "span_start_line"
                    | "span_end_line"
                    | "span_start_column"
                    | "span_end_column"
                    | "expansion_kind"
            ),
            RustcRelation::MirTerminator => matches!(
                name,
                "block_index"
                    | "raw_terminator_kind"
                    | "source_scope"
                    | "normal_target_count"
                    | "unwind_action"
                    | "assert_message_kind"
                    | "destination_place_id"
                    | "span_file"
                    | "span_start_byte"
                    | "span_end_byte"
                    | "span_start_line"
                    | "span_end_line"
                    | "span_start_column"
                    | "span_end_column"
                    | "expansion_kind"
            ),
            RustcRelation::CfgEdge => matches!(
                name,
                "source_block"
                    | "target_block"
                    | "edge_kind"
                    | "branch_value_u128"
                    | "unwind_action"
            ),
            RustcRelation::Call => matches!(
                name,
                "block_index"
                    | "callable_operand_id"
                    | "argument_count"
                    | "destination_place_id"
                    | "normal_target"
                    | "unwind_target"
                    | "declared_target"
                    | "declared_stable_crate_id"
                    | "declared_def_path_hash"
                    | "resolved_instance_key"
                    | "dispatch_kind"
                    | "resolution_confidence"
            ),
            RustcRelation::Access => matches!(
                name,
                "block_index"
                    | "slot_kind"
                    | "slot_index"
                    | "access_ordinal"
                    | "place_id"
                    | "access_kind"
                    | "type_key"
                    | "structured_evidence"
                    | "runtime_effect"
            ),
            RustcRelation::Diagnostic => matches!(
                name,
                "diagnostic_ordinal"
                    | "severity"
                    | "reason_code"
                    | "message"
                    | "structured_compiler_diagnostic"
            ),
            RustcRelation::Coverage => matches!(
                name,
                "fact_family"
                    | "authority_surface"
                    | "requested_units"
                    | "completed_units"
                    | "emitted_rows"
                    | "completeness"
                    | "remainder_count"
                    | "unknown_semantics"
            ),
            RustcRelation::Remainder => {
                matches!(
                    name,
                    "fact_family" | "reason_code" | "authority_surface" | "bounded" | "detail"
                )
            }
        }
}

/// Resolve one field through the closed v2.2 role table.
///
/// Every arm is an exact released field name. There are deliberately no prefix, suffix,
/// substring, datatype, or provider-metadata fallbacks: a new name must be classified explicitly.
#[allow(clippy::too_many_lines)]
fn compiled_provider_field_role(
    relation: ProviderRelation,
    name: &str,
) -> Result<ProviderFieldRole, ProductionProviderRecipeError> {
    if !relation_field_is_compiled(relation, name) {
        return Err(ProductionProviderRecipeError::UnclassifiedField {
            relation: relation.relation_identity(),
            field: name.to_owned(),
        });
    }
    let role = match name {
        "file_id" | "source_file_id" | "span_file" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::CanonicalIdentityInput,
            CoordinateRole::FileIdentity,
            RetentionPolicy::RetainForProvenance,
        ),
        "content_digest" | "source_content_digest" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::OccurrenceIdentityInput,
            CoordinateRole::ContentDigest,
            RetentionPolicy::RetainForProvenance,
        ),
        "start_byte" | "span_start_byte" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::OccurrenceIdentityInput,
            CoordinateRole::ByteStart,
            RetentionPolicy::RetainForProvenance,
        ),
        "end_byte" | "span_end_byte" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::OccurrenceIdentityInput,
            CoordinateRole::ByteEnd,
            RetentionPolicy::RetainForProvenance,
        ),
        "line"
        | "column"
        | "provider_start_line"
        | "provider_start_column"
        | "provider_end_line"
        | "provider_end_column"
        | "span_start_line"
        | "span_start_column"
        | "span_end_line"
        | "span_end_column" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::ProviderNativeCoordinate,
            RetentionPolicy::RetainForProvenance,
        ),
        "expansion_kind" | "in_external_macro" | "source_hygiene_authority" => field_role(
            FieldMeaning::Coordinate,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::MacroOrHygieneEvidence,
            RetentionPolicy::RetainForProvenance,
        ),
        "stable_crate_id"
        | "def_path_hash"
        | "definition_stable_crate_id"
        | "definition_def_path_hash"
        | "declared_stable_crate_id"
        | "declared_def_path_hash" => field_role(
            FieldMeaning::CanonicalIdentityInput,
            ProviderLocalIdentityRole::NativeStableKeyEvidence,
            CanonicalIdentityRole::CanonicalIdentityInput,
            CoordinateRole::None,
            RetentionPolicy::RetainForProvenance,
        ),
        "provider_local_node_id"
        | "parent_provider_local_node_id"
        | "provider_local_ast_id"
        | "parent_provider_local_ast_id"
        | "provider_local_owner_id"
        | "provider_local_target_id"
        | "tree_sitter_provider_local_node_id" => field_role(
            FieldMeaning::ProviderLocalIdentity,
            ProviderLocalIdentityRole::SnapshotLocalKey,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainProviderNative,
        ),
        "local_type_index" | "owner_local_type_index" | "referenced_local_type_index" => {
            field_role(
                FieldMeaning::ProviderLocalIdentity,
                ProviderLocalIdentityRole::ResponseLocalIndex,
                CanonicalIdentityRole::NotCanonical,
                CoordinateRole::None,
                RetentionPolicy::RetainProviderNative,
            )
        }
        "block_index"
        | "local_index"
        | "source_scope"
        | "slot_index"
        | "statement_index"
        | "base_local"
        | "projection_local_or_field"
        | "source_block"
        | "target_block"
        | "normal_target"
        | "unwind_target" => field_role(
            FieldMeaning::ProviderLocalIdentity,
            ProviderLocalIdentityRole::CompilerLocalIndex,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainProviderNative,
        ),
        "scope_id" | "parent_scope_id" | "binding_id" | "reference_id" | "target_id"
        | "unknown_symbol_id" | "subject_id" | "object_id" | "import_id" | "target_module_id"
        | "imported_entity_id" | "local_binding_id" | "export_id" => field_role(
            FieldMeaning::CanonicalIdentityInput,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::OccurrenceIdentityInput,
            CoordinateRole::None,
            RetentionPolicy::RetainForProvenance,
        ),
        "module_id"
        | "affected_module_id"
        | "compilation_unit_id"
        | "owner_id"
        | "type_key"
        | "component_type_key"
        | "specialized_type_key"
        | "instance_key"
        | "resolved_instance_key"
        | "projection_type_key"
        | "result_type_key"
        | "place_id"
        | "source_place_id"
        | "destination_place_id"
        | "operand_id"
        | "callable_operand_id" => field_role(
            FieldMeaning::CanonicalIdentityInput,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::CanonicalIdentityInput,
            CoordinateRole::None,
            RetentionPolicy::RetainForProvenance,
        ),
        "raw_kind_id"
        | "raw_kind"
        | "raw_kind_disposition"
        | "recovery_kind"
        | "token_class"
        | "spelling_kind"
        | "ast_category"
        | "child_role"
        | "scope_kind"
        | "binding_kind"
        | "target_form"
        | "reference_class"
        | "edge_kind"
        | "import_kind"
        | "directive_kind"
        | "diagnostic_kind"
        | "shape_kind"
        | "trait_kind"
        | "callee_kind"
        | "member_kind"
        | "type_role"
        | "item_kind"
        | "type_kind"
        | "instance_kind"
        | "terminator_kind"
        | "local_role"
        | "projection_kind"
        | "operand_kind"
        | "constant_kind"
        | "aggregate_kind"
        | "rvalue_kind"
        | "operator_kind"
        | "cast_kind"
        | "raw_statement_kind"
        | "runtime_check_kind"
        | "raw_terminator_kind"
        | "unwind_action"
        | "dispatch_kind"
        | "access_kind"
        | "slot_kind"
        | "occurrence_role"
        | "component_role"
        | "parent_role"
        | "region_kind"
        | "assert_message_kind"
        | "annotation_representation" => field_role(
            FieldMeaning::ProviderNativeKind,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainProviderNative,
        ),
        "message"
        | "detail"
        | "reason"
        | "reason_code"
        | "severity"
        | "rendered_text"
        | "remainder_reason"
        | "unknown_reason"
        | "unknown_semantics"
        | "structured_compiler_diagnostic"
        | "structured_fields_available"
        | "source_locator_redacted" => field_role(
            FieldMeaning::Diagnostic,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainDiagnosticBounded,
        ),
        "spelling_value" | "annotation_rendering" | "scalar_value" | "mangled_name" => field_role(
            FieldMeaning::RawProviderRendering,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainProviderNative,
        ),
        "provider_run_id"
        | "provider_id"
        | "provider_release"
        | "provider_revision"
        | "analysis_context_id"
        | "semantic_environment_id"
        | "source_generation"
        | "rustc_release"
        | "rustc_toolchain"
        | "catalog_id"
        | "inventory_fingerprint"
        | "grammar_release" => field_role(
            FieldMeaning::TypedFact,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainForProvenance,
        ),
        "access_ordinal"
        | "alias_name"
        | "argument_count"
        | "authority_surface"
        | "block_count"
        | "block_member"
        | "body_owner_count"
        | "bounded"
        | "branch_value_u128"
        | "call_occurrence_ordinal"
        | "child_ordinal"
        | "class_name"
        | "completed_units"
        | "completeness"
        | "component_ordinal"
        | "crate_name"
        | "debug_variable_count"
        | "declared_target"
        | "definition_path"
        | "dependency_require_tier"
        | "depth"
        | "diagnostic_ordinal"
        | "discovery_basis"
        | "emitted_rows"
        | "error"
        | "evaluation_ordinal"
        | "evidence_source"
        | "exact_authority_surface"
        | "exact_recheck_proven"
        | "explicit_parenthesized"
        | "export_status"
        | "extra"
        | "fact_family"
        | "family"
        | "field_name"
        | "from_end"
        | "generic_argument_count"
        | "has_body"
        | "imported_name"
        | "interpolated"
        | "is_entry"
        | "is_final"
        | "is_foreign_item"
        | "is_local_crate"
        | "is_staticmethod"
        | "local_count"
        | "local_item_count"
        | "long_lived_context"
        | "member_name"
        | "member_ordinal"
        | "min_length"
        | "missing"
        | "module_name"
        | "multiline"
        | "mutability"
        | "name"
        | "named"
        | "normal_target_count"
        | "normalized_effect"
        | "occurrence_ordinal"
        | "offset"
        | "operand_count"
        | "operand_ordinal"
        | "ordinal"
        | "placement"
        | "projection_ordinal"
        | "qualified_name"
        | "qualified_target"
        | "range_ordinal"
        | "reexport"
        | "refresh_policy"
        | "relative_level"
        | "remainder_count"
        | "requested_module_require_tier"
        | "requested_units"
        | "requires_monomorphization"
        | "resolution"
        | "resolution_confidence"
        | "resolution_state"
        | "ruff_qualified_name"
        | "runtime_effect"
        | "slice_to"
        | "source_byte_length"
        | "source_name"
        | "source_ordinal"
        | "spread_argument_local"
        | "stable_identity_authority"
        | "star_import"
        | "statement_count"
        | "structural_hash"
        | "structured_evidence"
        | "target_module_name"
        | "target_ordinal"
        | "terminal_status"
        | "token_ordinal"
        | "unspecified_type_arg_count" => field_role(
            FieldMeaning::TypedFact,
            ProviderLocalIdentityRole::None,
            CanonicalIdentityRole::NotCanonical,
            CoordinateRole::None,
            RetentionPolicy::RetainProviderNative,
        ),
        _ => {
            return Err(ProductionProviderRecipeError::UnclassifiedField {
                relation: relation.relation_identity(),
                field: name.to_owned(),
            });
        }
    };
    Ok(role)
}

fn contract_identity(
    provider_kind: &str,
    release: &str,
    rows: &[ProviderBoundaryContractRow],
) -> [u8; 32] {
    let mut hasher = identity_hasher("provider-contract");
    digest_frame(&mut hasher, RECIPE_RELEASE.as_bytes());
    digest_frame(&mut hasher, provider_kind.as_bytes());
    digest_frame(&mut hasher, release.as_bytes());
    for row in rows {
        digest_frame(&mut hasher, row.api_family.as_str().as_bytes());
        digest_frame(&mut hasher, &row.relation.relation_id.0);
        digest_frame(&mut hasher, &row.relation.schema_fingerprint.0);
    }
    *hasher.finalize().as_bytes()
}

fn identity16(label: &str, frames: &[&[u8]]) -> [u8; 16] {
    let value = identity32(label, frames);
    let mut result = [0; 16];
    result.copy_from_slice(&value[..16]);
    result
}

fn identity32(label: &str, frames: &[&[u8]]) -> [u8; 32] {
    let mut hasher = identity_hasher(label);
    for frame in frames {
        digest_frame(&mut hasher, frame);
    }
    *hasher.finalize().as_bytes()
}

fn identity_hasher(label: &str) -> blake3::Hasher {
    let mut hasher = blake3::Hasher::new();
    digest_frame(
        &mut hasher,
        b"codefabric.production-provider-recipe.identity.v1",
    );
    digest_frame(&mut hasher, label.as_bytes());
    hasher
}

fn digest_frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::cancellation::Cancellation;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig};
    use crate::fabric::production_kernel::CompiledSemanticRelease;
    use crate::provider_admission::{
        ProviderAdmissionUnknownCause, ProviderRegistrationDisposition,
    };
    use crate::provider_native_syntax::{
        ExactPythonSyntaxRunner, ProviderNativeSourceImage, PythonModuleInput, PythonSyntaxRunPins,
        SyntaxProviderRunPin,
    };
    use crate::provider_types::ProviderText;

    fn real_native_run() -> ProviderNativeSyntaxRun {
        let source_text = "from pkg import value\nresult = value + 1\n";
        let bytes = Arc::<[u8]>::from(source_text.as_bytes());
        let source = ProviderNativeSourceImage::new(
            [0x31; 16],
            9,
            Arc::clone(&bytes),
            crate::integrity::digest_bytes(&bytes),
            ProviderText {
                text: Arc::from(source_text),
                original_byte_offsets: Arc::from(
                    source_text
                        .char_indices()
                        .map(|(offset, _)| u64::try_from(offset).unwrap())
                        .chain(std::iter::once(u64::try_from(source_text.len()).unwrap()))
                        .collect::<Vec<_>>(),
                ),
            },
        )
        .unwrap();
        ExactPythonSyntaxRunner::new()
            .unwrap()
            .run_full(
                1,
                &source,
                PythonSyntaxRunPins {
                    tree_sitter: SyntaxProviderRunPin {
                        provider_run_id: [0x41; 16],
                        analysis_context_id: [0x51; 32],
                        semantic_environment_id: [0x61; 32],
                    },
                    ruff: SyntaxProviderRunPin {
                        provider_run_id: [0x42; 16],
                        analysis_context_id: [0x51; 32],
                        semantic_environment_id: [0x61; 32],
                    },
                },
                PythonModuleInput {
                    module_name: "fixture.production_provider_recipe",
                    module_path: Path::new("fixture/production_provider_recipe.py"),
                },
                &Cancellation::default(),
            )
            .unwrap()
    }

    fn native_source_pin() -> SourcePin {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric.native-syntax-workspace-source.v1\0");
        hasher.update(&1_u64.to_be_bytes());
        hasher.update(&[0x31; 16]);
        hasher.update(&9_u64.to_be_bytes());
        let source = b"from pkg import value\nresult = value + 1\n";
        hasher.update(crate::integrity::digest_bytes(source).as_slice());
        SourcePin(*hasher.finalize().as_bytes())
    }

    fn authority() -> ProductionProviderAuthority {
        ProductionProviderAuthority::try_new(
            ExactProviderLaneAuthority::try_new(native_source_pin(), ContextPin([0x51; 32]), 1)
                .unwrap(),
            ExactProviderLaneAuthority::try_new(SourcePin([0x71; 32]), ContextPin([0x72; 32]), 1)
                .unwrap(),
            ExactProviderLaneAuthority::try_new(SourcePin([0x73; 32]), ContextPin([0x74; 32]), 1)
                .unwrap(),
            1,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn real_tree_sitter_and_ruff_admit_while_missing_external_lanes_stay_unknown() {
        let release = CompiledSemanticRelease::current();
        let native = vec![real_native_run()];
        let outcome = admit_production_provider_relations(
            release.provider_authority(),
            ProgrammaticFabricEpochBuilder::try_new(
                FabricEpochId::from_bytes([0x81; 16]),
                FabricEpochRuntimeConfig::default(),
            )
            .unwrap(),
            authority(),
            ProductionProviderRuns::new(&native, &[], &[]),
        )
        .unwrap();

        assert_eq!(
            outcome.reports().tree_sitter().boundary.status,
            TerminalStatus::Complete
        );
        assert_eq!(
            outcome.reports().ruff().boundary.status,
            TerminalStatus::Complete
        );
        for report in [outcome.reports().pyrefly(), outcome.reports().rustc()] {
            assert_eq!(report.boundary.status, TerminalStatus::Unknown);
            assert!(report.relations.iter().all(|relation| {
                relation.disposition
                    == ProviderRegistrationDisposition::Unknown {
                        cause: ProviderAdmissionUnknownCause::MissingRelation,
                    }
            }));
        }

        let (builder, _) = outcome.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly
            .seal(FabricEpochId::from_bytes([0x81; 16]))
            .await
            .unwrap();
        assert!(
            sealed
                .relation(
                    &crate::fabric::programmatic_schema::ProgrammaticRelationId::new(
                        NativeSyntaxRelation::TreeSitterCstNode.as_str(),
                    )
                )
                .is_some()
        );
        assert!(
            sealed
                .relation(
                    &crate::fabric::programmatic_schema::ProgrammaticRelationId::new(
                        PyreflyRelation::TypeShape.relation_id(),
                    )
                )
                .is_none()
        );
    }

    #[test]
    fn compiled_relation_census_and_schema_contracts_are_exhaustive() {
        let release = CompiledSemanticRelease::current();
        let compiled_authority = release.provider_authority();
        let authority = authority();
        let tree = native_syntax_plan(
            compiled_authority,
            ProviderNativeLane::TreeSitter,
            authority.native_syntax,
        )
        .unwrap();
        let ruff = native_syntax_plan(
            compiled_authority,
            ProviderNativeLane::Ruff,
            authority.native_syntax,
        )
        .unwrap();
        let pyrefly = pyrefly_plan(compiled_authority, authority.pyrefly).unwrap();
        let rustc = rustc_plan(
            compiled_authority,
            authority.rustc,
            authority.rustc_owner_units,
        )
        .unwrap();

        assert_eq!(
            tree.bindings.len(),
            NativeSyntaxRelation::ALL
                .into_iter()
                .filter(|relation| native_lane(*relation) == ProviderNativeLane::TreeSitter)
                .count()
        );
        assert_eq!(
            ruff.bindings.len(),
            NativeSyntaxRelation::ALL
                .into_iter()
                .filter(|relation| native_lane(*relation) == ProviderNativeLane::Ruff)
                .count()
        );
        assert_eq!(pyrefly.bindings.len(), PyreflyRelation::ALL.len());
        assert_eq!(rustc.bindings.len(), RustcRelation::ALL.len());
        for plan in [&tree, &ruff, &pyrefly, &rustc] {
            assert_eq!(plan.bindings.len(), plan.contract.rows.len());
            for row in &plan.contract.rows {
                assert_eq!(
                    row.relation.fields.len(),
                    row.relation.schema.fields().len()
                );
                assert_eq!(
                    row.relation.schema_fingerprint.0,
                    canonical_arrow_schema_fingerprint(&row.relation.schema).unwrap()
                );
                for (ordinal, field) in row.relation.fields.iter().enumerate() {
                    assert_eq!(field.ordinal, ordinal);
                    assert_eq!(field.field.as_ref(), row.relation.schema.field(ordinal));
                }
            }
        }
    }

    #[test]
    fn provider_descriptor_rejects_an_unclassified_field() {
        let relation = ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterRun);
        let compiled = relation.schema();
        let mut fields = compiled.fields().iter().cloned().collect::<Vec<_>>();
        fields.push(Arc::new(Field::new(
            "unclassified_future_provider_field",
            DataType::Utf8,
            false,
        )));
        let changed = Arc::new(Schema::new_with_metadata(
            fields,
            compiled.metadata().clone(),
        ));

        assert!(matches!(
            relation_fields(relation, &changed),
            Err(ProductionProviderRecipeError::UnclassifiedField {
                relation: "provider.tree_sitter.run",
                field,
            }) if field == "unclassified_future_provider_field"
        ));
    }

    #[test]
    fn provider_descriptor_rejects_a_cross_relation_known_field_name() {
        let relation = ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterChangedRange);
        let compiled = relation.schema();
        let mut fields = compiled.fields().iter().cloned().collect::<Vec<_>>();
        fields.push(Arc::new(Field::new(
            "provider_revision",
            DataType::Utf8,
            false,
        )));
        let changed = Arc::new(Schema::new_with_metadata(
            fields,
            compiled.metadata().clone(),
        ));

        assert!(matches!(
            relation_fields(relation, &changed),
            Err(ProductionProviderRecipeError::UnclassifiedField {
                relation: "provider.tree_sitter.changed_range",
                field,
            }) if field == "provider_revision"
        ));
    }

    #[test]
    fn compiled_provider_authority_denies_caller_authored_provider_admission() {
        type AuthorityGatedAdmission<'a> =
            fn(
                &CompiledProviderAuthority,
                ProgrammaticFabricEpochBuilder,
                ProductionProviderAuthority,
                ProductionProviderRuns<'a>,
            )
                -> Result<ProgrammaticProviderAdmissionOutcome, ProductionProviderRecipeError>;

        let _closed_constructor: AuthorityGatedAdmission<'_> = admit_production_provider_relations;
        let release = CompiledSemanticRelease::current();
        let variable_operational_scope =
            ExactProviderLaneAuthority::try_new(SourcePin([0x91; 32]), ContextPin([0x92; 32]), 7)
                .unwrap();
        let plan = native_syntax_plan(
            release.provider_authority(),
            ProviderNativeLane::TreeSitter,
            variable_operational_scope,
        )
        .unwrap();
        assert!(
            plan.bindings
                .iter()
                .all(|binding| binding.requested_units == 7)
        );
    }

    #[test]
    fn zero_authority_is_rejected_before_a_candidate_is_consumed() {
        assert!(matches!(
            ExactProviderLaneAuthority::try_new(SourcePin([0; 32]), ContextPin([1; 32]), 1),
            Err(ProductionProviderRecipeError::InvalidAuthority(_))
        ));
        assert!(matches!(
            ExactProviderLaneAuthority::try_new(SourcePin([1; 32]), ContextPin([2; 32]), 0),
            Err(ProductionProviderRecipeError::InvalidAuthority(_))
        ));
    }
}
