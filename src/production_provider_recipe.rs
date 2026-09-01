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
    DeclaredCoverageBinding, ExactProgrammaticProviderRuns, ExactProviderLaneRuns,
    ProgrammaticProviderAdmissionOutcome, ProviderAdmissionError, ProviderAdmissionPlan,
    ProviderAuthorityClass, ProviderCoverageSource, ProviderNativeLane,
    ProviderRelationBinding, ProviderRelationIdentity, ProviderRelationPurpose,
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
/// Each lane is either an exact non-empty accepted run set or an explicit typed gap. The production
/// recipe still installs every compiled contract; absence and provider failure therefore become
/// governed remainder evidence rather than successful empty relations.
#[derive(Clone, Copy)]
pub struct ProductionProviderRuns<'a> {
    native_syntax: ExactProviderLaneRuns<'a, ProviderNativeSyntaxRun>,
    pyrefly: ExactProviderLaneRuns<'a, AcceptedPyreflyRun>,
    rustc: ExactProviderLaneRuns<'a, TrustQualifiedRustcCompilation>,
}

impl<'a> ProductionProviderRuns<'a> {
    #[must_use]
    pub const fn new(
        native_syntax: ExactProviderLaneRuns<'a, ProviderNativeSyntaxRun>,
        pyrefly: ExactProviderLaneRuns<'a, AcceptedPyreflyRun>,
        rustc: ExactProviderLaneRuns<'a, TrustQualifiedRustcCompilation>,
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
        ExactProgrammaticProviderRuns::try_new(
            &tree_sitter_plan,
            &ruff_plan,
            runs.native_syntax,
            &pyrefly_plan,
            runs.pyrefly,
            &rustc_plan,
            runs.rustc,
        )?,
    )?)
}

/// Closed provider-relation identity compiled into the v2.2 semantic release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderRelation {
    NativeSyntax(NativeSyntaxRelation),
    Pyrefly(PyreflyRelation),
    Rustc(RustcRelation),
}

/// Exact provider execution lane selected by the compiled semantic release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompiledProviderLane {
    TreeSitter,
    Ruff,
    Pyrefly,
    Rustc,
}

/// Application-owned execution bounds for one exact provider lane.
///
/// These values are part of the compiled provider recipe. They are not a second registry and
/// cannot be selected or replaced without the non-forgeable release authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledProviderExecutionProfile {
    pub(crate) provider_id: &'static str,
    pub(crate) placement: &'static str,
    pub(crate) resource_profile_id: &'static str,
    pub(crate) max_input_bytes: u64,
    pub(crate) max_work_units: u64,
    pub(crate) max_wall_millis: u64,
    pub(crate) max_visited_nodes: u64,
    pub(crate) max_traversal_depth: u16,
    pub(crate) max_output_records: u64,
    pub(crate) max_output_bytes: u64,
    pub(crate) max_diagnostics: u16,
    pub(crate) max_parser_workers: u16,
    pub(crate) max_retained_tree_revisions: u16,
    pub(crate) cancellation_check_interval: u32,
    pub(crate) cancellation_ack_millis: u16,
}

impl CompiledProviderAuthority {
    /// Select the exact provider-specific execution recipe compiled into this release.
    #[must_use]
    pub(crate) const fn execution_profile(
        &self,
        lane: CompiledProviderLane,
    ) -> CompiledProviderExecutionProfile {
        match lane {
            CompiledProviderLane::TreeSitter => CompiledProviderExecutionProfile {
                provider_id: "tree-sitter",
                placement: "IN_PROCESS",
                resource_profile_id: "in-process-syntax-standard",
                max_input_bytes: 16_777_216,
                max_work_units: 10_000_000,
                max_wall_millis: 30_000,
                max_visited_nodes: 2_000_000,
                max_traversal_depth: 256,
                max_output_records: 2_000_000,
                max_output_bytes: 268_435_456,
                max_diagnostics: 10_000,
                max_parser_workers: 4,
                max_retained_tree_revisions: 2,
                cancellation_check_interval: 1_024,
                cancellation_ack_millis: 2_000,
            },
            CompiledProviderLane::Ruff => CompiledProviderExecutionProfile {
                provider_id: "ruff-python",
                placement: "IN_PROCESS",
                resource_profile_id: "in-process-syntax-standard",
                max_input_bytes: 16_777_216,
                max_work_units: 10_000_000,
                max_wall_millis: 30_000,
                max_visited_nodes: 2_000_000,
                max_traversal_depth: 256,
                max_output_records: 2_000_000,
                max_output_bytes: 268_435_456,
                max_diagnostics: 10_000,
                max_parser_workers: 4,
                max_retained_tree_revisions: 2,
                cancellation_check_interval: 1_024,
                cancellation_ack_millis: 2_000,
            },
            CompiledProviderLane::Pyrefly => CompiledProviderExecutionProfile {
                provider_id: "pyrefly-python",
                placement: "SIDECAR",
                resource_profile_id: "sidecar-semantic-standard",
                max_input_bytes: 67_108_864,
                max_work_units: 20_000_000,
                max_wall_millis: 120_000,
                max_visited_nodes: 4_000_000,
                max_traversal_depth: 512,
                max_output_records: 4_000_000,
                max_output_bytes: 536_870_912,
                max_diagnostics: 20_000,
                max_parser_workers: 2,
                max_retained_tree_revisions: 1,
                cancellation_check_interval: 1_024,
                cancellation_ack_millis: 2_000,
            },
            CompiledProviderLane::Rustc => CompiledProviderExecutionProfile {
                provider_id: "rustc-mir",
                placement: "COMPILER_GROUP",
                resource_profile_id: "compiler-semantic-standard",
                max_input_bytes: 67_108_864,
                max_work_units: 20_000_000,
                max_wall_millis: 120_000,
                max_visited_nodes: 4_000_000,
                max_traversal_depth: 512,
                max_output_records: 4_000_000,
                max_output_bytes: 536_870_912,
                max_diagnostics: 20_000,
                max_parser_workers: 2,
                max_retained_tree_revisions: 1,
                cancellation_check_interval: 1_024,
                cancellation_ack_millis: 10_000,
            },
        }
    }
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
/// roles come from one closed `(relation, field)` table: an unclassified pair rejects descriptor
/// construction even when another relation contains the same field name.
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
                allowed_statuses: vec![TerminalStatus::Partial, TerminalStatus::Unknown],
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

// Explicit per-relation field-role authority. Every admitted pair appears in one arm.
const PROVENANCE_FACT_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::TypedFact,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainForProvenance,
);
const FILE_IDENTITY_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::CanonicalIdentityInput,
    CoordinateRole::FileIdentity,
    RetentionPolicy::RetainForProvenance,
);
const CONTENT_DIGEST_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::OccurrenceIdentityInput,
    CoordinateRole::ContentDigest,
    RetentionPolicy::RetainForProvenance,
);
const PROVIDER_FACT_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::TypedFact,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);
const DIAGNOSTIC_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Diagnostic,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainDiagnosticBounded,
);
const SNAPSHOT_LOCAL_KEY_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::ProviderLocalIdentity,
    ProviderLocalIdentityRole::SnapshotLocalKey,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);
const PROVIDER_KIND_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::ProviderNativeKind,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);
const BYTE_START_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::OccurrenceIdentityInput,
    CoordinateRole::ByteStart,
    RetentionPolicy::RetainForProvenance,
);
const BYTE_END_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::OccurrenceIdentityInput,
    CoordinateRole::ByteEnd,
    RetentionPolicy::RetainForProvenance,
);
const PROVIDER_COORDINATE_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::ProviderNativeCoordinate,
    RetentionPolicy::RetainForProvenance,
);
const RAW_RENDERING_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::RawProviderRendering,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);
const OCCURRENCE_REFERENCE_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::CanonicalIdentityInput,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::OccurrenceIdentityInput,
    CoordinateRole::None,
    RetentionPolicy::RetainForProvenance,
);
const CANONICAL_KEY_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::CanonicalIdentityInput,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::CanonicalIdentityInput,
    CoordinateRole::None,
    RetentionPolicy::RetainForProvenance,
);
const RESPONSE_LOCAL_INDEX_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::ProviderLocalIdentity,
    ProviderLocalIdentityRole::ResponseLocalIndex,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);
const NATIVE_STABLE_KEY_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::CanonicalIdentityInput,
    ProviderLocalIdentityRole::NativeStableKeyEvidence,
    CanonicalIdentityRole::CanonicalIdentityInput,
    CoordinateRole::None,
    RetentionPolicy::RetainForProvenance,
);
const HYGIENE_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::Coordinate,
    ProviderLocalIdentityRole::None,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::MacroOrHygieneEvidence,
    RetentionPolicy::RetainForProvenance,
);
const COMPILER_LOCAL_INDEX_ROLE: ProviderFieldRole = field_role(
    FieldMeaning::ProviderLocalIdentity,
    ProviderLocalIdentityRole::CompilerLocalIndex,
    CanonicalIdentityRole::NotCanonical,
    CoordinateRole::None,
    RetentionPolicy::RetainProviderNative,
);

#[allow(clippy::too_many_lines)]
fn compiled_provider_field_role(
    relation: ProviderRelation,
    name: &str,
) -> Result<ProviderFieldRole, ProductionProviderRecipeError> {
    let role = match relation {
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterRun) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation"
            | "provider_revision"
            | "catalog_id"
            | "inventory_fingerprint"
            | "grammar_release" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffRun) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation"
            | "provider_revision"
            | "catalog_id"
            | "inventory_fingerprint"
            | "grammar_release" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterCoverage) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "family" | "requested_units" | "completed_units" | "terminal_status" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "remainder_reason" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffCoverage) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "family" | "requested_units" | "completed_units" | "terminal_status" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "remainder_reason" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterRemainder) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "family" => Some(PROVIDER_FACT_ROLE),
            "reason" | "detail" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffRemainder) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "family" => Some(PROVIDER_FACT_ROLE),
            "reason" | "detail" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterCstNode) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "provider_local_node_id" | "parent_provider_local_node_id" => {
                Some(SNAPSHOT_LOCAL_KEY_ROLE)
            }
            "raw_kind_id" | "raw_kind" | "raw_kind_disposition" => Some(PROVIDER_KIND_ROLE),
            "field_name" | "named" | "extra" | "error" | "missing" | "ordinal" | "depth" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterChangedRange) => {
            match name {
                "provider_run_id"
                | "provider_id"
                | "provider_release"
                | "analysis_context_id"
                | "semantic_environment_id"
                | "source_generation" => Some(PROVENANCE_FACT_ROLE),
                "file_id" => Some(FILE_IDENTITY_ROLE),
                "content_digest" => Some(CONTENT_DIGEST_ROLE),
                "range_ordinal" => Some(PROVIDER_FACT_ROLE),
                "start_byte" => Some(BYTE_START_ROLE),
                "end_byte" => Some(BYTE_END_ROLE),
                _ => None,
            }
        }
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::TreeSitterRecoveryDiagnostic) => {
            match name {
                "provider_run_id"
                | "provider_id"
                | "provider_release"
                | "analysis_context_id"
                | "semantic_environment_id"
                | "source_generation" => Some(PROVENANCE_FACT_ROLE),
                "file_id" => Some(FILE_IDENTITY_ROLE),
                "content_digest" => Some(CONTENT_DIGEST_ROLE),
                "provider_local_node_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
                "recovery_kind" | "raw_kind" => Some(PROVIDER_KIND_ROLE),
                "start_byte" => Some(BYTE_START_ROLE),
                "end_byte" => Some(BYTE_END_ROLE),
                _ => None,
            }
        }
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffToken) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "token_ordinal" => Some(PROVIDER_FACT_ROLE),
            "raw_kind_id" | "raw_kind" | "token_class" | "spelling_kind" => {
                Some(PROVIDER_KIND_ROLE)
            }
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "line" | "column" => Some(PROVIDER_COORDINATE_ROLE),
            "spelling_value" => Some(RAW_RENDERING_ROLE),
            "provider_local_ast_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffComment) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "placement" | "block_member" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffDirective) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "directive_kind" => Some(PROVIDER_KIND_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "provider_local_target_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffStringRegion) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "multiline" | "interpolated" => Some(PROVIDER_FACT_ROLE),
            "provider_local_ast_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffDocstring) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "provider_local_owner_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffContinuationLine) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "start_byte" => Some(PROVIDER_COORDINATE_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffAstNode) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "provider_local_ast_id" | "parent_provider_local_ast_id" => {
                Some(SNAPSHOT_LOCAL_KEY_ROLE)
            }
            "raw_kind_id" | "raw_kind" | "ast_category" | "child_role" | "raw_kind_disposition" => {
                Some(PROVIDER_KIND_ROLE)
            }
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "line" | "column" => Some(PROVIDER_COORDINATE_ROLE),
            "child_ordinal"
            | "source_ordinal"
            | "evaluation_ordinal"
            | "explicit_parenthesized" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffParseDiagnostic) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "diagnostic_ordinal" => Some(PROVIDER_FACT_ROLE),
            "diagnostic_kind" => Some(PROVIDER_KIND_ROLE),
            "message" => Some(DIAGNOSTIC_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence) => {
            match name {
                "provider_run_id"
                | "provider_id"
                | "provider_release"
                | "analysis_context_id"
                | "semantic_environment_id"
                | "source_generation" => Some(PROVENANCE_FACT_ROLE),
                "file_id" => Some(FILE_IDENTITY_ROLE),
                "content_digest" => Some(CONTENT_DIGEST_ROLE),
                "diagnostic_ordinal" => Some(PROVIDER_FACT_ROLE),
                "tree_sitter_provider_local_node_id" => Some(SNAPSHOT_LOCAL_KEY_ROLE),
                _ => None,
            }
        }
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffScope) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "scope_id" | "parent_scope_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "scope_kind" => Some(PROVIDER_KIND_ROLE),
            "name" => Some(PROVIDER_FACT_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffBinding) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "binding_id" | "scope_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "name" => Some(PROVIDER_FACT_ROLE),
            "binding_kind" | "target_form" => Some(PROVIDER_KIND_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffReference) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "reference_id" | "scope_id" | "target_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "name" | "resolution" => Some(PROVIDER_FACT_ROLE),
            "reference_class" => Some(PROVIDER_KIND_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "unknown_reason" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffUnknownSymbol) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "unknown_symbol_id" | "scope_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "name" => Some(PROVIDER_FACT_ROLE),
            "reason" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffSemanticEdge) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "subject_id" | "object_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "edge_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffImport) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "import_id" | "scope_id" | "target_module_id" | "imported_entity_id"
            | "local_binding_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "import_kind" => Some(PROVIDER_KIND_ROLE),
            "relative_level"
            | "source_name"
            | "alias_name"
            | "star_import"
            | "target_module_name"
            | "ruff_qualified_name"
            | "resolution"
            | "imported_name" => Some(PROVIDER_FACT_ROLE),
            "unknown_reason" => Some(DIAGNOSTIC_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::NativeSyntax(NativeSyntaxRelation::RuffExport) => match name {
            "provider_run_id"
            | "provider_id"
            | "provider_release"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "export_id" | "target_id" => Some(OCCURRENCE_REFERENCE_ROLE),
            "name" | "reexport" | "export_status" => Some(PROVIDER_FACT_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::ModuleContext) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation"
            | "provider_release"
            | "provider_revision" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "module_name"
            | "requested_module_require_tier"
            | "dependency_require_tier"
            | "source_byte_length"
            | "long_lived_context" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::TypeShape) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "local_type_index" => Some(RESPONSE_LOCAL_INDEX_ROLE),
            "structural_hash" | "name" | "unspecified_type_arg_count" | "is_staticmethod" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "shape_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::TypeComponent) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "owner_local_type_index" | "referenced_local_type_index" => {
                Some(RESPONSE_LOCAL_INDEX_ROLE)
            }
            "component_role" => Some(PROVIDER_KIND_ROLE),
            "component_ordinal" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::TypeTrait) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "owner_local_type_index" => Some(RESPONSE_LOCAL_INDEX_ROLE),
            "trait_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::LocatedType) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "occurrence_ordinal" => Some(PROVIDER_FACT_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "local_type_index" => Some(RESPONSE_LOCAL_INDEX_ROLE),
            "type_role" => Some(PROVIDER_KIND_ROLE),
            "provider_start_line"
            | "provider_start_column"
            | "provider_end_line"
            | "provider_end_column" => Some(PROVIDER_COORDINATE_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::CallTarget) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "call_occurrence_ordinal"
            | "target_ordinal"
            | "qualified_target"
            | "class_name"
            | "resolution_state" => Some(PROVIDER_FACT_ROLE),
            "start_byte" => Some(BYTE_START_ROLE),
            "end_byte" => Some(BYTE_END_ROLE),
            "callee_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::Member) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "class_name" | "member_ordinal" | "member_name" | "is_final" | "discovery_basis" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "member_kind" | "annotation_representation" => Some(PROVIDER_KIND_ROLE),
            "annotation_rendering" => Some(RAW_RENDERING_ROLE),
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::Diagnostic) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "diagnostic_ordinal" => Some(PROVIDER_FACT_ROLE),
            "rendered_text" | "structured_fields_available" | "source_locator_redacted" => {
                Some(DIAGNOSTIC_ROLE)
            }
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::AffectedModule) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" | "affected_module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "evidence_source" | "exact_recheck_proven" | "refresh_policy" => {
                Some(PROVIDER_FACT_ROLE)
            }
            _ => None,
        },
        ProviderRelation::Pyrefly(PyreflyRelation::Coverage) => match name {
            "provider_run_id"
            | "analysis_context_id"
            | "semantic_environment_id"
            | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "module_id" => Some(CANONICAL_KEY_ROLE),
            "file_id" => Some(FILE_IDENTITY_ROLE),
            "content_digest" => Some(CONTENT_DIGEST_ROLE),
            "fact_family"
            | "exact_authority_surface"
            | "requested_units"
            | "completed_units"
            | "emitted_rows"
            | "completeness" => Some(PROVIDER_FACT_ROLE),
            "remainder_reason" | "unknown_semantics" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Compilation) => match name {
            "provider_run_id" | "source_generation" | "rustc_release" | "rustc_toolchain" => {
                Some(PROVENANCE_FACT_ROLE)
            }
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "crate_name"
            | "is_local_crate"
            | "local_item_count"
            | "body_owner_count"
            | "stable_identity_authority" => Some(PROVIDER_FACT_ROLE),
            "source_hygiene_authority" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::PublicItem) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "type_key" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "qualified_name" | "has_body" | "is_foreign_item" | "requires_monomorphization" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "item_kind" => Some(PROVIDER_KIND_ROLE),
            "span_file" | "span_start_byte" | "span_end_byte" => Some(PROVIDER_COORDINATE_ROLE),
            "span_start_line" | "span_end_line" | "span_start_column" | "span_end_column" => {
                Some(PROVIDER_COORDINATE_ROLE)
            }
            "expansion_kind" | "in_external_macro" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Type) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "type_key" | "component_type_key" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id"
            | "def_path_hash"
            | "definition_stable_crate_id"
            | "definition_def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "type_kind" | "component_role" => Some(PROVIDER_KIND_ROLE),
            "definition_path" | "component_ordinal" | "mutability" => Some(PROVIDER_FACT_ROLE),
            "scalar_value" => Some(RAW_RENDERING_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Instance) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "instance_key" | "specialized_type_key" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id"
            | "def_path_hash"
            | "definition_stable_crate_id"
            | "definition_def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "definition_path"
            | "generic_argument_count"
            | "has_body"
            | "is_foreign_item"
            | "resolution_state" => Some(PROVIDER_FACT_ROLE),
            "instance_kind" => Some(PROVIDER_KIND_ROLE),
            "mangled_name" => Some(RAW_RENDERING_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirBody) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_count"
            | "local_count"
            | "argument_count"
            | "debug_variable_count"
            | "spread_argument_local" => Some(PROVIDER_FACT_ROLE),
            "span_file" | "span_start_byte" | "span_end_byte" => Some(PROVIDER_COORDINATE_ROLE),
            "span_start_line" | "span_end_line" | "span_start_column" | "span_end_column" => {
                Some(PROVIDER_COORDINATE_ROLE)
            }
            "expansion_kind" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirBlock) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "statement_count" | "is_entry" => Some(PROVIDER_FACT_ROLE),
            "terminator_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirLocal) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "type_key" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "local_index" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "local_role" => Some(PROVIDER_KIND_ROLE),
            "mutability" => Some(PROVIDER_FACT_ROLE),
            "span_file" | "span_start_byte" | "span_end_byte" => Some(PROVIDER_COORDINATE_ROLE),
            "span_start_line" | "span_end_line" | "span_start_column" | "span_end_column" => {
                Some(PROVIDER_COORDINATE_ROLE)
            }
            "expansion_kind" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirPlace) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "place_id" | "projection_type_key" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "slot_index" | "base_local" | "projection_local_or_field" => {
                Some(COMPILER_LOCAL_INDEX_ROLE)
            }
            "slot_kind" | "occurrence_role" | "projection_kind" => Some(PROVIDER_KIND_ROLE),
            "occurrence_ordinal" | "projection_ordinal" | "offset" | "min_length" | "slice_to"
            | "from_end" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirOperand) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "operand_id" | "place_id" | "type_key" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "slot_index" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "slot_kind" | "parent_role" | "operand_kind" | "constant_kind"
            | "runtime_check_kind" => Some(PROVIDER_KIND_ROLE),
            "operand_ordinal" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirRvalue) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "result_type_key" | "source_place_id" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "statement_index" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "rvalue_kind" | "operator_kind" | "cast_kind" | "aggregate_kind" | "region_kind" => {
                Some(PROVIDER_KIND_ROLE)
            }
            "operand_count" | "mutability" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirStatement) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "statement_index" | "source_scope" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "raw_statement_kind" => Some(PROVIDER_KIND_ROLE),
            "normalized_effect" => Some(PROVIDER_FACT_ROLE),
            "span_file" | "span_start_byte" | "span_end_byte" => Some(PROVIDER_COORDINATE_ROLE),
            "span_start_line" | "span_end_line" | "span_start_column" | "span_end_column" => {
                Some(PROVIDER_COORDINATE_ROLE)
            }
            "expansion_kind" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::MirTerminator) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "destination_place_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "source_scope" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "raw_terminator_kind" | "unwind_action" | "assert_message_kind" => {
                Some(PROVIDER_KIND_ROLE)
            }
            "normal_target_count" => Some(PROVIDER_FACT_ROLE),
            "span_file" | "span_start_byte" | "span_end_byte" => Some(PROVIDER_COORDINATE_ROLE),
            "span_start_line" | "span_end_line" | "span_start_column" | "span_end_column" => {
                Some(PROVIDER_COORDINATE_ROLE)
            }
            "expansion_kind" => Some(HYGIENE_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::CfgEdge) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "source_block" | "target_block" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "edge_kind" | "unwind_action" => Some(PROVIDER_KIND_ROLE),
            "branch_value_u128" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Call) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id"
            | "owner_id"
            | "callable_operand_id"
            | "destination_place_id"
            | "resolved_instance_key" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id"
            | "def_path_hash"
            | "declared_stable_crate_id"
            | "declared_def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "normal_target" | "unwind_target" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "argument_count" | "declared_target" | "resolution_confidence" => {
                Some(PROVIDER_FACT_ROLE)
            }
            "dispatch_kind" => Some(PROVIDER_KIND_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Access) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" | "place_id" | "type_key" => {
                Some(CANONICAL_KEY_ROLE)
            }
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "block_index" | "slot_index" => Some(COMPILER_LOCAL_INDEX_ROLE),
            "slot_kind" | "access_kind" => Some(PROVIDER_KIND_ROLE),
            "access_ordinal" | "structured_evidence" | "runtime_effect" => Some(PROVIDER_FACT_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Diagnostic) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "diagnostic_ordinal" => Some(PROVIDER_FACT_ROLE),
            "severity" | "reason_code" | "message" | "structured_compiler_diagnostic" => {
                Some(DIAGNOSTIC_ROLE)
            }
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Coverage) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "fact_family" | "authority_surface" | "requested_units" | "completed_units"
            | "emitted_rows" | "completeness" | "remainder_count" => Some(PROVIDER_FACT_ROLE),
            "unknown_semantics" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
        ProviderRelation::Rustc(RustcRelation::Remainder) => match name {
            "provider_run_id" | "source_generation" => Some(PROVENANCE_FACT_ROLE),
            "compilation_unit_id" | "owner_id" => Some(CANONICAL_KEY_ROLE),
            "source_file_id" => Some(FILE_IDENTITY_ROLE),
            "source_content_digest" => Some(CONTENT_DIGEST_ROLE),
            "stable_crate_id" | "def_path_hash" => Some(NATIVE_STABLE_KEY_ROLE),
            "fact_family" | "authority_surface" | "bounded" => Some(PROVIDER_FACT_ROLE),
            "reason_code" | "detail" => Some(DIAGNOSTIC_ROLE),
            _ => None,
        },
    };
    role.ok_or_else(|| ProductionProviderRecipeError::UnclassifiedField {
        relation: relation.relation_identity(),
        field: name.to_owned(),
    })
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
    use std::collections::BTreeSet;
    use std::path::Path;

    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::cancellation::Cancellation;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig};
    use crate::fabric::production_kernel::CompiledSemanticRelease;
    use crate::provider_admission::{
        ProviderAdmissionUnknownCause, ProviderLaneGap, ProviderRegistrationDisposition,
    };
    use crate::provider_boundary::{ProviderBoundaryError, validate_provider_boundary_contract};
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
        let release = CompiledSemanticRelease::current();
        ExactPythonSyntaxRunner::new(release.provider_authority())
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
    async fn wp34_beh_real_tree_sitter_and_ruff_admit_while_missing_external_lanes_stay_unknown() {
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
            ProductionProviderRuns::new(
                ExactProviderLaneRuns::Accepted(&native),
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
            ),
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
    fn wp34_int_compiled_relation_census_and_schema_contracts_are_exhaustive() {
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
        let plans = [&tree, &ruff, &pyrefly, &rustc];
        let descriptor_relation_count = plans
            .into_iter()
            .map(|plan| plan.contract.rows.len())
            .sum::<usize>();
        let descriptor_field_count = plans
            .into_iter()
            .flat_map(|plan| &plan.contract.rows)
            .map(|row| row.relation.fields.len())
            .sum::<usize>();
        eprintln!(
            "WP34 provider descriptor census: {descriptor_relation_count} relations, \
             {descriptor_field_count} fields"
        );

        let expected_relations = NativeSyntaxRelation::ALL
            .into_iter()
            .map(|relation| relation.as_str())
            .chain(
                PyreflyRelation::ALL
                    .into_iter()
                    .map(PyreflyRelation::relation_id),
            )
            .chain(
                RustcRelation::ALL
                    .into_iter()
                    .map(RustcRelation::relation_id),
            )
            .collect::<BTreeSet<_>>();
        let actual_relations = plans
            .into_iter()
            .flat_map(|plan| &plan.contract.rows)
            .map(|row| row.api_family.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_relations, expected_relations);
        assert_eq!(actual_relations.len(), descriptor_relation_count);

        let expected_fields = NativeSyntaxRelation::ALL
            .into_iter()
            .map(|relation| (relation.as_str(), relation.schema()))
            .chain(
                PyreflyRelation::ALL
                    .into_iter()
                    .map(|relation| (relation.relation_id(), relation.schema())),
            )
            .chain(
                RustcRelation::ALL
                    .into_iter()
                    .map(|relation| (relation.relation_id(), relation.schema())),
            )
            .flat_map(|(relation, schema)| {
                schema
                    .fields()
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, field)| (relation, ordinal, field.name().to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let actual_fields = plans
            .into_iter()
            .flat_map(|plan| &plan.contract.rows)
            .flat_map(|row| {
                row.relation.fields.iter().map(|field| {
                    (
                        row.api_family.as_str(),
                        field.ordinal,
                        field.field.name().to_owned(),
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_fields, expected_fields);
        assert_eq!(actual_fields.len(), descriptor_field_count);

        for plan in plans {
            validate_provider_boundary_contract(&plan.contract, &plan.installer).unwrap();
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

        let rustc_span_fields = rustc
            .contract
            .rows
            .iter()
            .flat_map(|row| row.relation.fields.iter())
            .filter(|field| {
                matches!(
                    field.field.name().as_str(),
                    "span_file" | "span_start_byte" | "span_end_byte"
                )
            })
            .collect::<Vec<_>>();
        assert!(!rustc_span_fields.is_empty());
        assert!(rustc_span_fields.iter().all(|field| {
            field.meaning == FieldMeaning::Coordinate
                && field.coordinate == CoordinateRole::ProviderNativeCoordinate
                && field.canonical_identity == CanonicalIdentityRole::NotCanonical
        }));

        for plan in [&tree, &ruff, &pyrefly, &rustc] {
            for row in &plan.contract.rows {
                let has_file = row
                    .relation
                    .fields
                    .iter()
                    .any(|field| field.coordinate == CoordinateRole::FileIdentity);
                let has_digest = row
                    .relation
                    .fields
                    .iter()
                    .any(|field| field.coordinate == CoordinateRole::ContentDigest);
                let has_start = row
                    .relation
                    .fields
                    .iter()
                    .any(|field| field.coordinate == CoordinateRole::ByteStart);
                let has_end = row
                    .relation
                    .fields
                    .iter()
                    .any(|field| field.coordinate == CoordinateRole::ByteEnd);
                assert_eq!(has_start, has_end);
                assert!(!(has_start || has_end) || (has_file && has_digest));
                assert!(row.relation.fields.iter().all(|field| {
                    !matches!(
                        field.provider_local_identity,
                        ProviderLocalIdentityRole::SnapshotLocalKey
                            | ProviderLocalIdentityRole::ResponseLocalIndex
                            | ProviderLocalIdentityRole::CompilerLocalIndex
                    ) || field.canonical_identity == CanonicalIdentityRole::NotCanonical
                }));
            }
        }
    }

    #[test]
    fn wp34_neg_provider_gap_schema_shortcuts_and_provider_local_identity_are_rejected() {
        let release = CompiledSemanticRelease::current();
        let compiled_authority = release.provider_authority();
        let exact_authority = authority();

        let mut ruff = native_syntax_plan(
            compiled_authority,
            ProviderNativeLane::Ruff,
            exact_authority.native_syntax,
        )
        .unwrap();
        let continuation = ruff
            .contract
            .rows
            .iter_mut()
            .find(|row| {
                row.api_family.as_str() == NativeSyntaxRelation::RuffContinuationLine.as_str()
            })
            .unwrap();
        let start = continuation
            .relation
            .fields
            .iter_mut()
            .find(|field| field.field.name() == "start_byte")
            .unwrap();
        start.coordinate = CoordinateRole::ByteStart;
        start.canonical_identity = CanonicalIdentityRole::OccurrenceIdentityInput;
        assert_eq!(
            validate_provider_boundary_contract(&ruff.contract, &ruff.installer).unwrap_err(),
            ProviderBoundaryError::CoordinateClosureMissing
        );

        let mut tree = native_syntax_plan(
            compiled_authority,
            ProviderNativeLane::TreeSitter,
            exact_authority.native_syntax,
        )
        .unwrap();
        let node = tree
            .contract
            .rows
            .iter_mut()
            .find(|row| row.api_family.as_str() == NativeSyntaxRelation::TreeSitterCstNode.as_str())
            .unwrap();
        let local_key = node
            .relation
            .fields
            .iter_mut()
            .find(|field| field.field.name() == "provider_local_node_id")
            .unwrap();
        local_key.meaning = FieldMeaning::CanonicalIdentityInput;
        local_key.canonical_identity = CanonicalIdentityRole::CanonicalIdentityInput;
        assert_eq!(
            validate_provider_boundary_contract(&tree.contract, &tree.installer).unwrap_err(),
            ProviderBoundaryError::ProviderLocalIdentityPromoted
        );
    }

    #[test]
    fn wp34_neg_provider_descriptor_rejects_an_unclassified_field() {
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
    fn wp34_neg_provider_descriptor_rejects_a_cross_relation_known_field_name() {
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
