//! Production construction of the exact CodeFabric v2.1 provider-admission transaction.
//!
//! The relation census, Arrow schemas, authority roles, coverage routing, and upstream API
//! surfaces in this module are compiled Rust over the exact provider enums. No serialized model,
//! ontology, manifest, generated registry, row-count comparison, or plan-text digest decides what
//! a provider is allowed to contribute. Digests below are used only to construct the typed,
//! domain-separated identities required by the boundary contracts.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use arrow_schema::{FieldRef, SchemaRef};
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::fabric::epoch_runtime::FabricSchemaRole;
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
    ExactPythonSyntaxRunner, NativeSyntaxRelation, ProviderNativeSourceImage,
    ProviderNativeSyntaxError, ProviderNativeSyntaxRun, PythonModuleInput, PythonSyntaxRunPins,
    RUFF_COMPONENT_RELEASE, SyntaxProviderRunPin, TREE_SITTER_PYTHON_GRAMMAR_RELEASE,
    TREE_SITTER_RUNTIME_RELEASE,
};
use crate::provider_types::ProviderText;
use crate::pyrefly_service::{AcceptedPyreflyRun, PyreflyRelation};
use crate::relation_ipc::{
    ContextPin, RelationId, RemainderReason, SchemaFingerprint, SourcePin, TerminalStatus,
};
use crate::rustc_relation_schema::{
    RUSTC_PUBLIC_RELEASE, RUSTC_RELATION_PROTOCOL_VERSION, RUSTC_TOOLCHAIN, RustcRelation,
};
use crate::rustc_service::TrustQualifiedRustcCompilation;

const RECIPE_RELEASE: &str = "codefabric-provider-admission-v2.1.0";

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
    #[error("the compiled native-syntax schema carrier could not be constructed: {0}")]
    NativeSyntaxSchema(#[source] ProviderNativeSyntaxError),
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
/// Returns a typed schema-carrier or provider-admission failure. On error the consumed candidate
/// builder is not recoverable, preventing partial provider registration from escaping.
pub fn admit_production_provider_relations(
    builder: ProgrammaticFabricEpochBuilder,
    authority: ProductionProviderAuthority,
    runs: ProductionProviderRuns<'_>,
) -> Result<ProgrammaticProviderAdmissionOutcome, ProductionProviderRecipeError> {
    // Native syntax builds relation schemas inside its exact projection. Reuse a real accepted
    // run when one exists. A source-less workspace uses a real, non-admitted in-process schema
    // carrier so the compiled contract still exists and the empty lane becomes Unknown.
    let fallback_schema_run;
    let native_schema_run = if let Some(run) = runs.native_syntax.first() {
        run
    } else {
        fallback_schema_run = native_syntax_schema_carrier()?;
        &fallback_schema_run
    };

    let tree_sitter_plan = native_syntax_plan(
        ProviderNativeLane::TreeSitter,
        native_schema_run,
        authority.native_syntax,
    )?;
    let ruff_plan = native_syntax_plan(
        ProviderNativeLane::Ruff,
        native_schema_run,
        authority.native_syntax,
    )?;
    let pyrefly_plan = pyrefly_plan(authority.pyrefly)?;
    let rustc_plan = rustc_plan(authority.rustc, authority.rustc_owner_units)?;

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

#[derive(Clone)]
struct CompiledProviderRelation {
    relation_identity: &'static str,
    schema: SchemaRef,
    upstream_symbol: &'static str,
    authority: ProviderAuthorityRole,
    purpose: ProviderRelationPurpose,
    coverage: ProviderCoverageSource,
    requested_units: NonZeroU64,
}

fn native_syntax_plan(
    lane: ProviderNativeLane,
    schema_run: &ProviderNativeSyntaxRun,
    authority: ExactProviderLaneAuthority,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let relations = NativeSyntaxRelation::ALL
        .into_iter()
        .filter(|relation| native_lane(*relation) == lane)
        .map(|relation| {
            let (purpose, coverage) = native_coverage(relation)?;
            Ok(CompiledProviderRelation {
                relation_identity: relation.as_str(),
                schema: schema_run.relation(relation).schema(),
                upstream_symbol: native_upstream_symbol(relation),
                authority: native_authority(relation),
                purpose,
                coverage,
                requested_units: authority.requested_units,
            })
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
            let (purpose, coverage) = pyrefly_coverage(relation)?;
            Ok(CompiledProviderRelation {
                relation_identity: relation.relation_id(),
                schema: relation.schema(),
                upstream_symbol: pyrefly_upstream_symbol(relation),
                authority: pyrefly_authority(relation),
                purpose,
                coverage,
                requested_units: authority.requested_units,
            })
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
    authority: ExactProviderLaneAuthority,
    owner_units: NonZeroU64,
) -> Result<ProviderAdmissionPlan, ProductionProviderRecipeError> {
    let relations = RustcRelation::ALL
        .into_iter()
        .map(|relation| {
            let (purpose, coverage) = rustc_coverage(relation)?;
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
            Ok(CompiledProviderRelation {
                relation_identity: relation.relation_id(),
                schema: relation.schema(),
                upstream_symbol: rustc_upstream_symbol(relation),
                authority: ProviderAuthorityRole::Primary,
                purpose,
                coverage,
                requested_units,
            })
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
    relations: Vec<CompiledProviderRelation>,
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
    for relation in relations {
        let family = ProviderApiFamily::new(relation.relation_identity.to_owned())
            .map_err(ProviderAdmissionError::from)?;
        let relation_id = RelationId(identity16(
            "provider-relation",
            &[relation.relation_identity.as_bytes()],
        ));
        let schema_fingerprint = SchemaFingerprint(schema_fingerprint(&relation.schema));
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
                fields: relation_fields(&relation.schema, lane),
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
            requested_units: relation.requested_units.get(),
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
    let control = match relation {
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::RuffRun
        | NativeSyntaxRelation::RuffCoverage
        | NativeSyntaxRelation::RuffRemainder
        | NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => true,
        NativeSyntaxRelation::TreeSitterCstNode
        | NativeSyntaxRelation::TreeSitterChangedRange
        | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic
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
        | NativeSyntaxRelation::RuffExport => false,
    };
    if control {
        return Ok((
            ProviderRelationPurpose::ControlEvidence,
            ProviderCoverageSource::StructuralPresence,
        ));
    }
    let family = relation
        .as_str()
        .strip_prefix("provider.")
        .expect("the closed native syntax relation has the provider prefix");
    let coverage_relation = match native_lane(relation) {
        ProviderNativeLane::TreeSitter => NativeSyntaxRelation::TreeSitterCoverage.as_str(),
        ProviderNativeLane::Ruff => NativeSyntaxRelation::RuffCoverage.as_str(),
        ProviderNativeLane::Pyrefly | ProviderNativeLane::Rustc => unreachable!(),
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

fn native_lane(relation: NativeSyntaxRelation) -> ProviderNativeLane {
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

fn relation_fields(schema: &SchemaRef, lane: ProviderNativeLane) -> Vec<ProviderBoundaryField> {
    schema
        .fields()
        .iter()
        .enumerate()
        .map(|(ordinal, field)| relation_field(ordinal, Arc::clone(field), lane))
        .collect()
}

fn relation_field(
    ordinal: usize,
    field: FieldRef,
    lane: ProviderNativeLane,
) -> ProviderBoundaryField {
    let name = field.name();
    let coordinate = coordinate_role(name);
    let stable_key = lane == ProviderNativeLane::Rustc
        && (name.contains("stable_crate_id") || name.contains("def_path_hash"));
    let provider_local_identity = if stable_key {
        ProviderLocalIdentityRole::NativeStableKeyEvidence
    } else if name.contains("provider_local") {
        ProviderLocalIdentityRole::SnapshotLocalKey
    } else if lane == ProviderNativeLane::Pyrefly && name.contains("local_type_index") {
        ProviderLocalIdentityRole::ResponseLocalIndex
    } else if lane == ProviderNativeLane::Rustc && rustc_compiler_local_field(name) {
        ProviderLocalIdentityRole::CompilerLocalIndex
    } else {
        ProviderLocalIdentityRole::None
    };
    let application_observation_identity = field
        .metadata()
        .get("codefabric.meaning")
        .is_some_and(|meaning| meaning == "application-owned-observation-id");
    let canonical_identity = if stable_key || matches!(coordinate, CoordinateRole::FileIdentity) {
        CanonicalIdentityRole::CanonicalIdentityInput
    } else if application_observation_identity
        || matches!(
            coordinate,
            CoordinateRole::ContentDigest | CoordinateRole::ByteStart | CoordinateRole::ByteEnd
        )
    {
        CanonicalIdentityRole::OccurrenceIdentityInput
    } else {
        CanonicalIdentityRole::NotCanonical
    };
    let provider_kind = field
        .metadata()
        .get("codefabric.meaning")
        .is_some_and(|meaning| meaning.contains("provider-native-kind"))
        || name.starts_with("raw_")
        || name.ends_with("_kind");
    let diagnostic = matches!(
        name.as_str(),
        "message" | "detail" | "reason" | "reason_code" | "severity" | "rendered_text"
    );
    let meaning = if coordinate != CoordinateRole::None {
        FieldMeaning::Coordinate
    } else if stable_key || application_observation_identity {
        FieldMeaning::CanonicalIdentityInput
    } else if provider_local_identity != ProviderLocalIdentityRole::None {
        FieldMeaning::ProviderLocalIdentity
    } else if provider_kind {
        FieldMeaning::ProviderNativeKind
    } else if diagnostic {
        FieldMeaning::Diagnostic
    } else {
        FieldMeaning::TypedFact
    };
    let retention = if diagnostic {
        RetentionPolicy::RetainDiagnosticBounded
    } else if coordinate != CoordinateRole::None {
        RetentionPolicy::RetainForProvenance
    } else {
        RetentionPolicy::RetainProviderNative
    };
    ProviderBoundaryField {
        ordinal,
        field,
        meaning,
        provider_local_identity,
        canonical_identity,
        coordinate,
        retention,
    }
}

fn coordinate_role(name: &str) -> CoordinateRole {
    match name {
        "file_id" | "source_file_id" | "span_file" => CoordinateRole::FileIdentity,
        "content_digest" | "source_content_digest" => CoordinateRole::ContentDigest,
        "start_byte" | "span_start_byte" => CoordinateRole::ByteStart,
        "end_byte" | "span_end_byte" => CoordinateRole::ByteEnd,
        value
            if value.ends_with("_line")
                || value.ends_with("_column")
                || value.contains("provider_start_")
                || value.contains("provider_end_") =>
        {
            CoordinateRole::ProviderNativeCoordinate
        }
        _ => CoordinateRole::None,
    }
}

fn rustc_compiler_local_field(name: &str) -> bool {
    matches!(
        name,
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
            | "unwind_target"
    )
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

fn schema_fingerprint(schema: &SchemaRef) -> [u8; 32] {
    let mut hasher = identity_hasher("arrow-schema");
    let mut metadata = schema.metadata().iter().collect::<Vec<_>>();
    metadata.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in metadata {
        digest_frame(&mut hasher, key.as_bytes());
        digest_frame(&mut hasher, value.as_bytes());
    }
    for field in schema.fields() {
        digest_frame(&mut hasher, field.name().as_bytes());
        digest_frame(&mut hasher, format!("{:?}", field.data_type()).as_bytes());
        digest_frame(&mut hasher, &[u8::from(field.is_nullable())]);
        let mut metadata = field.metadata().iter().collect::<Vec<_>>();
        metadata.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (key, value) in metadata {
            digest_frame(&mut hasher, key.as_bytes());
            digest_frame(&mut hasher, value.as_bytes());
        }
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

fn native_syntax_schema_carrier() -> Result<ProviderNativeSyntaxRun, ProductionProviderRecipeError>
{
    let source_text = "pass\n";
    let bytes = Arc::<[u8]>::from(source_text.as_bytes());
    let source = ProviderNativeSourceImage::new(
        identity16("native-schema-file", &[RECIPE_RELEASE.as_bytes()]),
        1,
        Arc::clone(&bytes),
        crate::integrity::digest_bytes(&bytes),
        ProviderText {
            text: Arc::from(source_text),
            original_byte_offsets: Arc::from(
                source_text
                    .char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap_or(u64::MAX))
                    .chain(std::iter::once(
                        u64::try_from(source_text.len()).unwrap_or(u64::MAX),
                    ))
                    .collect::<Vec<_>>(),
            ),
        },
    )
    .map_err(ProductionProviderRecipeError::NativeSyntaxSchema)?;
    let context = identity32("native-schema-context", &[RECIPE_RELEASE.as_bytes()]);
    let environment = identity32("native-schema-environment", &[RECIPE_RELEASE.as_bytes()]);
    ExactPythonSyntaxRunner::new()
        .map_err(ProductionProviderRecipeError::NativeSyntaxSchema)?
        .run_full(
            1,
            &source,
            PythonSyntaxRunPins {
                tree_sitter: SyntaxProviderRunPin {
                    provider_run_id: identity16(
                        "native-schema-tree-sitter-run",
                        &[RECIPE_RELEASE.as_bytes()],
                    ),
                    analysis_context_id: context,
                    semantic_environment_id: environment,
                },
                ruff: SyntaxProviderRunPin {
                    provider_run_id: identity16(
                        "native-schema-ruff-run",
                        &[RECIPE_RELEASE.as_bytes()],
                    ),
                    analysis_context_id: context,
                    semantic_environment_id: environment,
                },
            },
            PythonModuleInput {
                module_name: "codefabric.__provider_schema__",
                module_path: Path::new("codefabric/__provider_schema__.py"),
            },
            &Cancellation::default(),
        )
        .map_err(ProductionProviderRecipeError::NativeSyntaxSchema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig};
    use crate::provider_admission::{
        ProviderAdmissionUnknownCause, ProviderRegistrationDisposition,
    };

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
        let native = vec![real_native_run()];
        let outcome = admit_production_provider_relations(
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
        let schema_run = native_syntax_schema_carrier().unwrap();
        let authority = authority();
        let tree = native_syntax_plan(
            ProviderNativeLane::TreeSitter,
            &schema_run,
            authority.native_syntax,
        )
        .unwrap();
        let ruff = native_syntax_plan(
            ProviderNativeLane::Ruff,
            &schema_run,
            authority.native_syntax,
        )
        .unwrap();
        let pyrefly = pyrefly_plan(authority.pyrefly).unwrap();
        let rustc = rustc_plan(authority.rustc, authority.rustc_owner_units).unwrap();

        assert_eq!(tree.bindings.len(), 6);
        assert_eq!(ruff.bindings.len(), 19);
        assert_eq!(pyrefly.bindings.len(), PyreflyRelation::ALL.len());
        assert_eq!(rustc.bindings.len(), RustcRelation::ALL.len());
        for plan in [&tree, &ruff, &pyrefly, &rustc] {
            assert_eq!(plan.bindings.len(), plan.contract.rows.len());
            assert!(plan.contract.rows.iter().all(|row| {
                row.relation.fields.len() == row.relation.schema.fields().len()
                    && row.relation.schema_fingerprint.0 != [0; 32]
            }));
        }
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
