//! Independent provider-boundary contracts and Arrow conformance evaluation.
//!
//! The contract side is authored and accepted outside the provider installer. The installer
//! contributes only its observed handler/schema surface. Evaluation joins those two inputs with
//! relation-scoped Arrow results and explicit requested/completed/remainder coverage; missing
//! output therefore becomes partial or unknown instead of an empty result that looks complete.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use arrow_schema::{DataType, Field, FieldRef, SchemaRef};
use thiserror::Error;

use crate::relation_ipc::{
    AssembledRelation, ContextPin, CoverageTrailer, RelationId, RemainderReason, SchemaFingerprint,
    SourcePin, TerminalStatus,
};

const MAX_CONTRACT_ROWS: usize = 4_096;
const MAX_FIELDS_PER_RELATION: usize = 1_024;
const MAX_SYMBOLS_PER_FAMILY: usize = 256;
const MAX_COVERAGE_REMAINDERS: usize = 4_096;
const MAX_NAME_BYTES: usize = 512;

/// Stable provider identity; it is not a generated provider registry ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderId(pub [u8; 16]);

/// Accountable owner identity used to prove contract/installer separation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundaryOwnerId(pub [u8; 32]);

/// Immutable identity of an independently accepted boundary contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundaryContractId(pub [u8; 32]);

/// Immutable identity of the provider installer build.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderInstallerId(pub [u8; 32]);

/// Identity of a concrete installed handler.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderHandlerId(pub [u8; 16]);

/// Identity of the executable oracle that proves one contract row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderOracleId(pub [u8; 32]);

/// Exact upstream API family named by a boundary contract row.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderApiFamily(String);

impl ProviderApiFamily {
    /// Create a bounded non-empty API family name.
    ///
    /// # Errors
    ///
    /// Rejects leading/trailing whitespace, control characters, empty names, and oversized names.
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderBoundaryError> {
        Ok(Self(validate_name(value.into(), "provider API family")?))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact upstream API symbol used by the installed handler.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UpstreamApiSymbol(String);

impl UpstreamApiSymbol {
    /// Create a bounded non-empty upstream API symbol.
    ///
    /// # Errors
    ///
    /// Rejects leading/trailing whitespace, control characters, empty names, and oversized names.
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderBoundaryError> {
        Ok(Self(validate_name(value.into(), "upstream API symbol")?))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact provider release and immutable source/toolchain revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRevision {
    pub provider_id: ProviderId,
    pub release: String,
    pub source_revision: [u8; 32],
}

/// Independent authorship and acceptance identities for a contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndependentContractAcceptance {
    pub author_owner: BoundaryOwnerId,
    pub reviewer_owner: BoundaryOwnerId,
    pub acceptance_authority: BoundaryOwnerId,
}

/// Identity and ownership of the actual provider installer being evaluated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderInstallerIdentity {
    pub installer_id: ProviderInstallerId,
    pub owner: BoundaryOwnerId,
    pub provider_revision: ProviderRevision,
}

/// How a provider-native field participates in provider-local identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderLocalIdentityRole {
    None,
    SnapshotLocalKey,
    ResponseLocalIndex,
    CompilerLocalIndex,
    NativeStableKeyEvidence,
}

/// How a field may contribute to application-owned canonical identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalIdentityRole {
    NotCanonical,
    CanonicalIdentityInput,
    OccurrenceIdentityInput,
}

/// Coordinate/provenance role of one typed Arrow field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateRole {
    None,
    FileIdentity,
    ContentDigest,
    ByteStart,
    ByteEnd,
    ProviderNativeCoordinate,
    MacroOrHygieneEvidence,
}

/// Meaning class prevents a text/binary carrier from silently standing in for a relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldMeaning {
    TypedFact,
    ProviderNativeKind,
    ProviderLocalIdentity,
    CanonicalIdentityInput,
    Coordinate,
    Diagnostic,
    RawProviderRendering,
}

/// Retention policy for provider-native evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicy {
    RetainProviderNative,
    RetainForProvenance,
    RetainDiagnosticBounded,
    ProviderRunOnly,
    NeverPersist,
}

/// Authority role independently assigned to a provider family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAuthorityRole {
    Primary,
    Fallback,
    Corroborating,
    NarrowEnrichment,
    ForbiddenProviderNative,
}

/// Whether the row is required from a provider or intentionally remains a remainder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractDisposition {
    Required,
    IntentionalRemainder { reason: RemainderReason },
}

/// Required behavior when a normally available provider family cannot complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailableBehavior {
    pub status: TerminalStatus,
    pub allowed_reasons: Vec<RemainderReason>,
}

/// Field-level contract bound exactly to one Arrow schema field and ordinal.
#[derive(Clone, Debug)]
pub struct ProviderBoundaryField {
    pub ordinal: usize,
    pub field: FieldRef,
    pub meaning: FieldMeaning,
    pub provider_local_identity: ProviderLocalIdentityRole,
    pub canonical_identity: CanonicalIdentityRole,
    pub coordinate: CoordinateRole,
    pub retention: RetentionPolicy,
}

/// Arrow relation and application-owned schema identity for one API family.
#[derive(Clone, Debug)]
pub struct ProviderArrowRelationContract {
    pub relation_id: RelationId,
    pub schema_fingerprint: SchemaFingerprint,
    pub schema: SchemaRef,
    pub fields: Vec<ProviderBoundaryField>,
}

/// One independently authored provider-boundary row.
#[derive(Clone, Debug)]
pub struct ProviderBoundaryContractRow {
    pub api_family: ProviderApiFamily,
    pub upstream_symbols: Vec<UpstreamApiSymbol>,
    pub relation: ProviderArrowRelationContract,
    pub authority: ProviderAuthorityRole,
    pub disposition: ContractDisposition,
    pub unavailable_behavior: UnavailableBehavior,
    pub oracle_id: ProviderOracleId,
}

/// Accepted provider-boundary contract. The installer cannot supply its acceptance identities.
#[derive(Clone, Debug)]
pub struct ProviderBoundaryContract {
    pub contract_id: BoundaryContractId,
    pub contract_revision: u32,
    pub provider_revision: ProviderRevision,
    pub acceptance: IndependentContractAcceptance,
    pub rows: Vec<ProviderBoundaryContractRow>,
}

/// Handler-derived installed surface. It contains no semantic payload bytes.
#[derive(Clone, Debug)]
pub struct InstalledProviderSurface {
    pub installer_id: ProviderInstallerId,
    pub handler_id: ProviderHandlerId,
    pub api_family: ProviderApiFamily,
    pub upstream_symbols: Vec<UpstreamApiSymbol>,
    pub relation_id: RelationId,
    pub schema_fingerprint: SchemaFingerprint,
    pub schema: SchemaRef,
}

/// One explicitly requested provider family and its requested scope size.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFamilyRequest {
    pub api_family: ProviderApiFamily,
    pub requested_units: u64,
}

/// Provider-declared requested/completed/remainder/unknown coverage for one family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFamilyCoverage {
    pub api_family: ProviderApiFamily,
    pub trailer: CoverageTrailer,
}

/// Inputs evaluated against an independent contract. Semantic data can enter only through an
/// already assembled Arrow relation.
#[derive(Clone, Copy, Debug)]
pub struct ProviderBoundaryEvidence<'a> {
    pub expected_source_pin: SourcePin,
    pub expected_context_pin: ContextPin,
    pub installed_surfaces: &'a [InstalledProviderSurface],
    pub requested_families: &'a [ProviderFamilyRequest],
    pub family_coverage: &'a [ProviderFamilyCoverage],
    pub relations: &'a [AssembledRelation],
}

/// Installed-surface disposition for one contract family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderSurfaceOutcome {
    Installed,
    Missing,
    IntentionalRemainder,
}

/// Why evaluation had to materialize an unknown family outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderUnknownCause {
    ProviderDeclared,
    MissingCoverageDeclaration,
    MissingInstalledSurface,
    MissingArrowRelation,
}

/// Run outcome for one contract family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderFamilyRunOutcome {
    NotRequested,
    Complete {
        requested_units: u64,
    },
    Partial {
        trailer: CoverageTrailer,
    },
    Unknown {
        trailer: Option<CoverageTrailer>,
        cause: ProviderUnknownCause,
    },
}

/// Joined contract, installation, and run result for one family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFamilyOutcome {
    pub api_family: ProviderApiFamily,
    pub relation_id: RelationId,
    pub schema_fingerprint: SchemaFingerprint,
    pub authority: ProviderAuthorityRole,
    pub handler_id: Option<ProviderHandlerId>,
    pub surface: ProviderSurfaceOutcome,
    pub run: ProviderFamilyRunOutcome,
    pub oracle_id: ProviderOracleId,
}

/// Truthful provider capability result. Status is derived from family rows, never claimed by an
/// installer boolean.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBoundaryReport {
    pub contract_id: BoundaryContractId,
    pub contract_revision: u32,
    pub installer_id: ProviderInstallerId,
    pub provider_revision: ProviderRevision,
    pub source_pin: SourcePin,
    pub context_pin: ContextPin,
    pub status: TerminalStatus,
    pub families: Vec<ProviderFamilyOutcome>,
}

/// Stable contract, ownership, schema, and evidence failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProviderBoundaryError {
    #[error("invalid {kind}: {detail}")]
    InvalidName {
        kind: &'static str,
        detail: &'static str,
    },
    #[error("provider revision is invalid")]
    InvalidProviderRevision,
    #[error("contract, provider, schema, or provenance identity uses the all-zero sentinel")]
    ZeroIdentity,
    #[error("contract authorship/review is not independent from the provider installer")]
    OwnershipNotIndependent,
    #[error("contract provider revision differs from the provider installer")]
    ProviderRevisionMismatch,
    #[error("contract revision must be non-zero")]
    InvalidContractRevision,
    #[error("provider boundary contract has no rows")]
    EmptyContract,
    #[error("provider boundary contract resource limit exceeded: {0}")]
    ContractLimit(&'static str),
    #[error("duplicate provider API family in contract")]
    DuplicateContractFamily,
    #[error("duplicate relation identity in contract")]
    DuplicateContractRelation,
    #[error("upstream API symbol set is empty or duplicated")]
    InvalidSymbolSet,
    #[error("Arrow field contract does not exactly match schema order, field, or metadata")]
    FieldContractMismatch,
    #[error("Arrow schema contains an opaque JSON/payload carrier")]
    OpaqueSemanticCarrier,
    #[error("byte coordinates lack file identity, content digest, or a complete half-open range")]
    CoordinateClosureMissing,
    #[error("snapshot/response/compiler-local identity is marked as canonical input")]
    ProviderLocalIdentityPromoted,
    #[error("field role combination is internally inconsistent")]
    InvalidFieldRoles,
    #[error("intentional omission must be an unsupported remainder and forbidden provider output")]
    InvalidIntentionalRemainder,
    #[error("unavailable behavior is internally inconsistent")]
    InvalidUnavailableBehavior,
    #[error("duplicate installed provider surface family")]
    DuplicateInstalledSurface,
    #[error("installed surface belongs to another installer")]
    SurfaceInstallerMismatch,
    #[error("installed surface has no independently authored contract row")]
    UncontractedInstalledSurface,
    #[error("provider emitted a surface for an intentional remainder")]
    SurfaceForIntentionalRemainder,
    #[error("installed symbols, relation identity, fingerprint, or schema differ from contract")]
    InstalledSurfaceMismatch,
    #[error("duplicate requested provider family")]
    DuplicateFamilyRequest,
    #[error("requested provider family has no contract row")]
    UncontractedFamilyRequest,
    #[error("duplicate provider family coverage declaration")]
    DuplicateFamilyCoverage,
    #[error("coverage was declared for an unrequested family")]
    CoverageWithoutRequest,
    #[error("family coverage is invalid: {0}")]
    InvalidCoverage(&'static str),
    #[error("coverage remainder violates the contract's unavailable behavior")]
    UnavailableBehaviorMismatch,
    #[error("duplicate Arrow relation identity")]
    DuplicateArrowRelation,
    #[error("Arrow relation has no contract row")]
    UncontractedArrowRelation,
    #[error("provider emitted an Arrow relation for an unrequested family")]
    UnrequestedArrowRelation,
    #[error("provider emitted an Arrow relation without its installed handler surface")]
    ArrowRelationWithoutSurface,
    #[error("Arrow relation identity, pins, fingerprint, or schema differ from contract")]
    ArrowRelationMismatch,
    #[error("Arrow RecordBatch schema differs from its relation contract")]
    ArrowBatchSchemaMismatch,
    #[error("relation trailer differs from provider family coverage")]
    RelationCoverageMismatch,
    #[error("provider claimed completed units without an installed surface and Arrow relation")]
    FalseCompletionClaim,
}

/// Validate independent ownership and every typed contract row before comparing provider output.
///
/// # Errors
///
/// Rejects same-owner authorship, provider revision drift, duplicate families/relations,
/// incomplete field contracts, unsafe identity promotion, coordinate gaps, and opaque carriers.
pub fn validate_provider_boundary_contract(
    contract: &ProviderBoundaryContract,
    installer: &ProviderInstallerIdentity,
) -> Result<(), ProviderBoundaryError> {
    validate_nonzero(contract.contract_id.0)?;
    validate_nonzero(installer.installer_id.0)?;
    validate_nonzero(installer.owner.0)?;
    validate_nonzero(contract.acceptance.author_owner.0)?;
    validate_nonzero(contract.acceptance.reviewer_owner.0)?;
    validate_nonzero(contract.acceptance.acceptance_authority.0)?;
    validate_provider_revision(&contract.provider_revision)?;
    validate_provider_revision(&installer.provider_revision)?;
    if contract.contract_revision == 0 {
        return Err(ProviderBoundaryError::InvalidContractRevision);
    }
    let acceptance = contract.acceptance;
    if acceptance.author_owner == acceptance.reviewer_owner
        || acceptance.author_owner == acceptance.acceptance_authority
        || acceptance.reviewer_owner == acceptance.acceptance_authority
        || installer.owner == acceptance.author_owner
        || installer.owner == acceptance.reviewer_owner
        || installer.owner == acceptance.acceptance_authority
    {
        return Err(ProviderBoundaryError::OwnershipNotIndependent);
    }
    if contract.provider_revision != installer.provider_revision {
        return Err(ProviderBoundaryError::ProviderRevisionMismatch);
    }
    if contract.rows.is_empty() {
        return Err(ProviderBoundaryError::EmptyContract);
    }
    if contract.rows.len() > MAX_CONTRACT_ROWS {
        return Err(ProviderBoundaryError::ContractLimit("contract rows"));
    }
    let mut families = BTreeSet::new();
    let mut relations = BTreeSet::new();
    for row in &contract.rows {
        if !families.insert(row.api_family.clone()) {
            return Err(ProviderBoundaryError::DuplicateContractFamily);
        }
        if !relations.insert(row.relation.relation_id) {
            return Err(ProviderBoundaryError::DuplicateContractRelation);
        }
        validate_symbol_set(&row.upstream_symbols)?;
        validate_nonzero(row.oracle_id.0)?;
        validate_contract_row(row)?;
    }
    Ok(())
}

/// Compare installed handlers, Arrow batches, and explicit coverage to an independent contract.
///
/// Missing surfaces, relations, or coverage declarations become family-level partial/unknown
/// rows. Contradictory claims, schema drift, extra output, wrong pins, or hidden authority fail.
///
/// # Errors
///
/// Returns a typed protocol error for contract, ownership, schema, coverage, or provenance
/// contradictions. Ordinary provider unavailability is returned in `ProviderBoundaryReport`.
pub fn evaluate_provider_boundary(
    contract: &ProviderBoundaryContract,
    installer: &ProviderInstallerIdentity,
    evidence: ProviderBoundaryEvidence<'_>,
) -> Result<ProviderBoundaryReport, ProviderBoundaryError> {
    validate_provider_boundary_contract(contract, installer)?;
    validate_nonzero(evidence.expected_source_pin.0)?;
    validate_nonzero(evidence.expected_context_pin.0)?;

    let contract_by_family = contract
        .rows
        .iter()
        .map(|row| (row.api_family.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let contract_by_relation = contract
        .rows
        .iter()
        .map(|row| (row.relation.relation_id, row))
        .collect::<BTreeMap<_, _>>();

    let surfaces =
        validate_installed_surfaces(installer, evidence.installed_surfaces, &contract_by_family)?;
    let requests = validate_requests(evidence.requested_families, &contract_by_family)?;
    let coverage = validate_coverage(evidence.family_coverage, &requests)?;
    let relations = validate_relations(
        evidence.relations,
        &contract_by_relation,
        &surfaces,
        &requests,
        evidence.expected_source_pin,
        evidence.expected_context_pin,
    )?;

    let mut families = Vec::with_capacity(contract.rows.len());
    for row in contract_by_family.values() {
        let installed_surface = surfaces.get(&row.api_family).copied();
        let has_installed_surface = installed_surface.is_some();
        let surface = match row.disposition {
            ContractDisposition::Required if has_installed_surface => {
                ProviderSurfaceOutcome::Installed
            }
            ContractDisposition::Required => ProviderSurfaceOutcome::Missing,
            ContractDisposition::IntentionalRemainder { .. } => {
                ProviderSurfaceOutcome::IntentionalRemainder
            }
        };
        let run = match requests.get(&row.api_family) {
            None => ProviderFamilyRunOutcome::NotRequested,
            Some(request) => evaluate_family_run(
                row,
                request,
                coverage.get(&row.api_family).copied(),
                relations.get(&row.relation.relation_id).copied(),
                has_installed_surface,
            )?,
        };
        families.push(ProviderFamilyOutcome {
            api_family: row.api_family.clone(),
            relation_id: row.relation.relation_id,
            schema_fingerprint: row.relation.schema_fingerprint,
            authority: row.authority,
            handler_id: installed_surface.map(|surface| surface.handler_id),
            surface,
            run,
            oracle_id: row.oracle_id,
        });
    }

    let status = aggregate_status(&families);
    Ok(ProviderBoundaryReport {
        contract_id: contract.contract_id,
        contract_revision: contract.contract_revision,
        installer_id: installer.installer_id,
        provider_revision: installer.provider_revision.clone(),
        source_pin: evidence.expected_source_pin,
        context_pin: evidence.expected_context_pin,
        status,
        families,
    })
}

fn validate_contract_row(row: &ProviderBoundaryContractRow) -> Result<(), ProviderBoundaryError> {
    validate_nonzero(row.relation.relation_id.0)?;
    validate_nonzero(row.relation.schema_fingerprint.0)?;
    if row.relation.fields.is_empty() {
        return Err(ProviderBoundaryError::FieldContractMismatch);
    }
    if row.relation.fields.len() > MAX_FIELDS_PER_RELATION {
        return Err(ProviderBoundaryError::ContractLimit("fields per relation"));
    }
    if row.relation.fields.len() != row.relation.schema.fields().len() {
        return Err(ProviderBoundaryError::FieldContractMismatch);
    }
    if has_forbidden_opaque_metadata(row.relation.schema.metadata()) {
        return Err(ProviderBoundaryError::OpaqueSemanticCarrier);
    }
    let mut names = BTreeSet::new();
    let mut has_file = false;
    let mut has_digest = false;
    let mut has_start = false;
    let mut has_end = false;
    for (ordinal, contract_field) in row.relation.fields.iter().enumerate() {
        if contract_field.ordinal != ordinal
            || contract_field.field.as_ref() != row.relation.schema.field(ordinal)
            || !names.insert(contract_field.field.name())
        {
            return Err(ProviderBoundaryError::FieldContractMismatch);
        }
        validate_field_roles(contract_field)?;
        if arrow_field_contains_opaque_carrier(contract_field.field.as_ref()) {
            return Err(ProviderBoundaryError::OpaqueSemanticCarrier);
        }
        has_file |= contract_field.coordinate == CoordinateRole::FileIdentity;
        has_digest |= contract_field.coordinate == CoordinateRole::ContentDigest;
        has_start |= contract_field.coordinate == CoordinateRole::ByteStart;
        has_end |= contract_field.coordinate == CoordinateRole::ByteEnd;
    }
    if has_start != has_end || ((has_start || has_end) && !(has_file && has_digest)) {
        return Err(ProviderBoundaryError::CoordinateClosureMissing);
    }
    validate_unavailable_behavior(&row.unavailable_behavior)?;
    match row.disposition {
        ContractDisposition::Required
            if row.authority != ProviderAuthorityRole::ForbiddenProviderNative =>
        {
            Ok(())
        }
        ContractDisposition::IntentionalRemainder {
            reason: RemainderReason::Unsupported,
        } if row.authority == ProviderAuthorityRole::ForbiddenProviderNative
            && row.unavailable_behavior.status == TerminalStatus::Partial
            && row
                .unavailable_behavior
                .allowed_reasons
                .contains(&RemainderReason::Unsupported) =>
        {
            Ok(())
        }
        ContractDisposition::IntentionalRemainder { .. } => {
            Err(ProviderBoundaryError::InvalidIntentionalRemainder)
        }
        ContractDisposition::Required => Err(ProviderBoundaryError::InvalidIntentionalRemainder),
    }
}

fn validate_field_roles(field: &ProviderBoundaryField) -> Result<(), ProviderBoundaryError> {
    if matches!(
        field.provider_local_identity,
        ProviderLocalIdentityRole::SnapshotLocalKey
            | ProviderLocalIdentityRole::ResponseLocalIndex
            | ProviderLocalIdentityRole::CompilerLocalIndex
    ) && field.canonical_identity != CanonicalIdentityRole::NotCanonical
    {
        return Err(ProviderBoundaryError::ProviderLocalIdentityPromoted);
    }
    if field.provider_local_identity != ProviderLocalIdentityRole::None
        && !matches!(
            field.meaning,
            FieldMeaning::ProviderLocalIdentity | FieldMeaning::CanonicalIdentityInput
        )
    {
        return Err(ProviderBoundaryError::InvalidFieldRoles);
    }
    if field.canonical_identity != CanonicalIdentityRole::NotCanonical
        && !matches!(
            field.meaning,
            FieldMeaning::CanonicalIdentityInput | FieldMeaning::Coordinate
        )
    {
        return Err(ProviderBoundaryError::InvalidFieldRoles);
    }
    if field.coordinate != CoordinateRole::None
        && !matches!(
            field.meaning,
            FieldMeaning::Coordinate | FieldMeaning::CanonicalIdentityInput
        )
    {
        return Err(ProviderBoundaryError::InvalidFieldRoles);
    }
    if (field.meaning == FieldMeaning::ProviderLocalIdentity
        && field.provider_local_identity == ProviderLocalIdentityRole::None)
        || (field.meaning == FieldMeaning::CanonicalIdentityInput
            && field.canonical_identity == CanonicalIdentityRole::NotCanonical)
        || (field.meaning == FieldMeaning::Coordinate && field.coordinate == CoordinateRole::None)
        || (field.canonical_identity != CanonicalIdentityRole::NotCanonical
            && field.retention == RetentionPolicy::NeverPersist)
        || (field.coordinate != CoordinateRole::None
            && field.retention == RetentionPolicy::NeverPersist)
        || (field.meaning == FieldMeaning::ProviderNativeKind
            && !matches!(
                field.retention,
                RetentionPolicy::RetainProviderNative | RetentionPolicy::RetainForProvenance
            ))
    {
        return Err(ProviderBoundaryError::InvalidFieldRoles);
    }
    Ok(())
}

fn validate_unavailable_behavior(
    behavior: &UnavailableBehavior,
) -> Result<(), ProviderBoundaryError> {
    if behavior.allowed_reasons.is_empty() {
        return Err(ProviderBoundaryError::InvalidUnavailableBehavior);
    }
    if behavior
        .allowed_reasons
        .iter()
        .enumerate()
        .any(|(index, reason)| behavior.allowed_reasons[..index].contains(reason))
    {
        return Err(ProviderBoundaryError::InvalidUnavailableBehavior);
    }
    match behavior.status {
        TerminalStatus::Complete => Err(ProviderBoundaryError::InvalidUnavailableBehavior),
        TerminalStatus::Partial
            if !behavior.allowed_reasons.contains(&RemainderReason::Unknown) =>
        {
            Ok(())
        }
        TerminalStatus::Unknown if behavior.allowed_reasons.contains(&RemainderReason::Unknown) => {
            Ok(())
        }
        TerminalStatus::Partial | TerminalStatus::Unknown => {
            Err(ProviderBoundaryError::InvalidUnavailableBehavior)
        }
    }
}

fn validate_installed_surfaces<'a>(
    installer: &ProviderInstallerIdentity,
    surfaces: &'a [InstalledProviderSurface],
    contract: &BTreeMap<ProviderApiFamily, &ProviderBoundaryContractRow>,
) -> Result<BTreeMap<ProviderApiFamily, &'a InstalledProviderSurface>, ProviderBoundaryError> {
    let mut by_family = BTreeMap::new();
    for surface in surfaces {
        if surface.installer_id != installer.installer_id {
            return Err(ProviderBoundaryError::SurfaceInstallerMismatch);
        }
        if by_family
            .insert(surface.api_family.clone(), surface)
            .is_some()
        {
            return Err(ProviderBoundaryError::DuplicateInstalledSurface);
        }
        validate_nonzero(surface.handler_id.0)?;
        validate_symbol_set(&surface.upstream_symbols)?;
        let row = contract
            .get(&surface.api_family)
            .ok_or(ProviderBoundaryError::UncontractedInstalledSurface)?;
        if matches!(
            row.disposition,
            ContractDisposition::IntentionalRemainder { .. }
        ) {
            return Err(ProviderBoundaryError::SurfaceForIntentionalRemainder);
        }
        let actual_symbols = surface.upstream_symbols.iter().collect::<BTreeSet<_>>();
        let contracted_symbols = row.upstream_symbols.iter().collect::<BTreeSet<_>>();
        if actual_symbols != contracted_symbols
            || surface.relation_id != row.relation.relation_id
            || surface.schema_fingerprint != row.relation.schema_fingerprint
            || surface.schema.as_ref() != row.relation.schema.as_ref()
        {
            return Err(ProviderBoundaryError::InstalledSurfaceMismatch);
        }
    }
    Ok(by_family)
}

fn validate_requests<'a>(
    requests: &'a [ProviderFamilyRequest],
    contract: &BTreeMap<ProviderApiFamily, &ProviderBoundaryContractRow>,
) -> Result<BTreeMap<ProviderApiFamily, &'a ProviderFamilyRequest>, ProviderBoundaryError> {
    let mut requested = BTreeMap::new();
    for request in requests {
        if !contract.contains_key(&request.api_family) {
            return Err(ProviderBoundaryError::UncontractedFamilyRequest);
        }
        if requested
            .insert(request.api_family.clone(), request)
            .is_some()
        {
            return Err(ProviderBoundaryError::DuplicateFamilyRequest);
        }
    }
    Ok(requested)
}

fn validate_coverage<'a>(
    coverage: &'a [ProviderFamilyCoverage],
    requests: &BTreeMap<ProviderApiFamily, &ProviderFamilyRequest>,
) -> Result<BTreeMap<ProviderApiFamily, &'a ProviderFamilyCoverage>, ProviderBoundaryError> {
    let mut by_family = BTreeMap::new();
    for observation in coverage {
        let request = requests
            .get(&observation.api_family)
            .ok_or(ProviderBoundaryError::CoverageWithoutRequest)?;
        if by_family
            .insert(observation.api_family.clone(), observation)
            .is_some()
        {
            return Err(ProviderBoundaryError::DuplicateFamilyCoverage);
        }
        validate_coverage_trailer(&observation.trailer, request.requested_units)?;
    }
    Ok(by_family)
}

fn validate_relations<'a>(
    relations: &'a [AssembledRelation],
    contract: &BTreeMap<RelationId, &ProviderBoundaryContractRow>,
    surfaces: &BTreeMap<ProviderApiFamily, &InstalledProviderSurface>,
    requests: &BTreeMap<ProviderApiFamily, &ProviderFamilyRequest>,
    source_pin: SourcePin,
    context_pin: ContextPin,
) -> Result<BTreeMap<RelationId, &'a AssembledRelation>, ProviderBoundaryError> {
    let mut by_relation = BTreeMap::new();
    for relation in relations {
        if by_relation
            .insert(relation.identity.relation_id, relation)
            .is_some()
        {
            return Err(ProviderBoundaryError::DuplicateArrowRelation);
        }
        let row = contract
            .get(&relation.identity.relation_id)
            .ok_or(ProviderBoundaryError::UncontractedArrowRelation)?;
        if !requests.contains_key(&row.api_family) {
            return Err(ProviderBoundaryError::UnrequestedArrowRelation);
        }
        if !surfaces.contains_key(&row.api_family) {
            return Err(ProviderBoundaryError::ArrowRelationWithoutSurface);
        }
        if relation.identity.relation_id != row.relation.relation_id
            || relation.identity.schema_fingerprint != row.relation.schema_fingerprint
            || relation.identity.source_pin != source_pin
            || relation.identity.context_pin != context_pin
            || relation.schema.as_ref() != row.relation.schema.as_ref()
        {
            return Err(ProviderBoundaryError::ArrowRelationMismatch);
        }
        if relation
            .batches
            .iter()
            .any(|batch| batch.schema().as_ref() != row.relation.schema.as_ref())
        {
            return Err(ProviderBoundaryError::ArrowBatchSchemaMismatch);
        }
    }
    Ok(by_relation)
}

fn evaluate_family_run(
    row: &ProviderBoundaryContractRow,
    request: &ProviderFamilyRequest,
    coverage: Option<&ProviderFamilyCoverage>,
    relation: Option<&AssembledRelation>,
    installed: bool,
) -> Result<ProviderFamilyRunOutcome, ProviderBoundaryError> {
    let Some(coverage) = coverage else {
        return Ok(ProviderFamilyRunOutcome::Unknown {
            trailer: None,
            cause: ProviderUnknownCause::MissingCoverageDeclaration,
        });
    };
    if let Some(relation) = relation
        && relation.trailer != coverage.trailer
    {
        return Err(ProviderBoundaryError::RelationCoverageMismatch);
    }
    match row.disposition {
        ContractDisposition::IntentionalRemainder { reason } => {
            if relation.is_some()
                || installed
                || coverage.trailer.completed_units != 0
                || coverage.trailer.status != TerminalStatus::Partial
                || coverage
                    .trailer
                    .remainders
                    .iter()
                    .any(|remainder| remainder.reason != reason)
            {
                return Err(ProviderBoundaryError::InvalidIntentionalRemainder);
            }
            Ok(ProviderFamilyRunOutcome::Partial {
                trailer: coverage.trailer.clone(),
            })
        }
        ContractDisposition::Required => match coverage.trailer.status {
            TerminalStatus::Complete => {
                if !installed || relation.is_none() {
                    return Err(ProviderBoundaryError::FalseCompletionClaim);
                }
                Ok(ProviderFamilyRunOutcome::Complete {
                    requested_units: request.requested_units,
                })
            }
            TerminalStatus::Partial => {
                validate_actual_unavailable(&coverage.trailer, &row.unavailable_behavior)?;
                if coverage.trailer.completed_units > 0 && (!installed || relation.is_none()) {
                    return Err(ProviderBoundaryError::FalseCompletionClaim);
                }
                Ok(ProviderFamilyRunOutcome::Partial {
                    trailer: coverage.trailer.clone(),
                })
            }
            TerminalStatus::Unknown => {
                validate_actual_unavailable(&coverage.trailer, &row.unavailable_behavior)?;
                if coverage.trailer.completed_units > 0 && (!installed || relation.is_none()) {
                    return Err(ProviderBoundaryError::FalseCompletionClaim);
                }
                let cause = if !installed {
                    ProviderUnknownCause::MissingInstalledSurface
                } else if relation.is_none() {
                    ProviderUnknownCause::MissingArrowRelation
                } else {
                    ProviderUnknownCause::ProviderDeclared
                };
                Ok(ProviderFamilyRunOutcome::Unknown {
                    trailer: Some(coverage.trailer.clone()),
                    cause,
                })
            }
        },
    }
}

fn validate_actual_unavailable(
    trailer: &CoverageTrailer,
    behavior: &UnavailableBehavior,
) -> Result<(), ProviderBoundaryError> {
    if trailer.status != behavior.status
        || trailer
            .remainders
            .iter()
            .any(|remainder| !behavior.allowed_reasons.contains(&remainder.reason))
    {
        return Err(ProviderBoundaryError::UnavailableBehaviorMismatch);
    }
    Ok(())
}

fn validate_coverage_trailer(
    trailer: &CoverageTrailer,
    requested_units: u64,
) -> Result<(), ProviderBoundaryError> {
    if trailer.requested_units != requested_units
        || trailer.completed_units > trailer.requested_units
    {
        return Err(ProviderBoundaryError::InvalidCoverage(
            "requested/completed count mismatch",
        ));
    }
    if trailer.remainders.len() > MAX_COVERAGE_REMAINDERS {
        return Err(ProviderBoundaryError::InvalidCoverage(
            "remainder resource limit exceeded",
        ));
    }
    let mut scopes = BTreeSet::new();
    let mut remainder_units = 0_u64;
    let mut has_unknown = false;
    for remainder in &trailer.remainders {
        if remainder.unit_count == 0 || !scopes.insert(remainder.scope) {
            return Err(ProviderBoundaryError::InvalidCoverage(
                "zero-sized or duplicate remainder",
            ));
        }
        remainder_units = remainder_units.checked_add(remainder.unit_count).ok_or(
            ProviderBoundaryError::InvalidCoverage("remainder count overflow"),
        )?;
        has_unknown |= remainder.reason == RemainderReason::Unknown;
    }
    if trailer.completed_units.checked_add(remainder_units).ok_or(
        ProviderBoundaryError::InvalidCoverage("coverage count overflow"),
    )? != trailer.requested_units
    {
        return Err(ProviderBoundaryError::InvalidCoverage(
            "completed and remainder scopes do not close request",
        ));
    }
    let valid = match trailer.status {
        TerminalStatus::Complete => {
            trailer.completed_units == trailer.requested_units && trailer.remainders.is_empty()
        }
        TerminalStatus::Partial => {
            trailer.completed_units < trailer.requested_units
                && !trailer.remainders.is_empty()
                && !has_unknown
        }
        TerminalStatus::Unknown => {
            trailer.completed_units < trailer.requested_units
                && !trailer.remainders.is_empty()
                && has_unknown
        }
    };
    if !valid {
        return Err(ProviderBoundaryError::InvalidCoverage(
            "terminal status does not match coverage",
        ));
    }
    Ok(())
}

fn aggregate_status(families: &[ProviderFamilyOutcome]) -> TerminalStatus {
    let has_unknown = families.iter().any(|family| {
        matches!(
            family.run,
            ProviderFamilyRunOutcome::NotRequested | ProviderFamilyRunOutcome::Unknown { .. }
        ) || family.surface == ProviderSurfaceOutcome::Missing
    });
    let has_partial = families.iter().any(|family| {
        matches!(family.run, ProviderFamilyRunOutcome::Partial { .. })
            || family.surface == ProviderSurfaceOutcome::IntentionalRemainder
    });
    let has_positive = families.iter().any(|family| {
        matches!(
            family.run,
            ProviderFamilyRunOutcome::Complete { .. } | ProviderFamilyRunOutcome::Partial { .. }
        )
    });
    if has_unknown && !has_positive && !has_partial {
        TerminalStatus::Unknown
    } else if has_unknown || has_partial {
        TerminalStatus::Partial
    } else {
        TerminalStatus::Complete
    }
}

fn validate_provider_revision(revision: &ProviderRevision) -> Result<(), ProviderBoundaryError> {
    if revision.provider_id.0 == [0; 16]
        || revision.source_revision == [0; 32]
        || validate_name(revision.release.clone(), "provider release").is_err()
    {
        return Err(ProviderBoundaryError::InvalidProviderRevision);
    }
    Ok(())
}

fn validate_symbol_set(symbols: &[UpstreamApiSymbol]) -> Result<(), ProviderBoundaryError> {
    if symbols.is_empty() || symbols.len() > MAX_SYMBOLS_PER_FAMILY {
        return Err(ProviderBoundaryError::InvalidSymbolSet);
    }
    let unique = symbols.iter().collect::<BTreeSet<_>>();
    if unique.len() != symbols.len() {
        return Err(ProviderBoundaryError::InvalidSymbolSet);
    }
    Ok(())
}

fn validate_nonzero<const N: usize>(value: [u8; N]) -> Result<(), ProviderBoundaryError> {
    if value.iter().all(|byte| *byte == 0) {
        Err(ProviderBoundaryError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn validate_name(value: String, kind: &'static str) -> Result<String, ProviderBoundaryError> {
    if value.is_empty() {
        return Err(ProviderBoundaryError::InvalidName {
            kind,
            detail: "empty",
        });
    }
    if value.len() > MAX_NAME_BYTES {
        return Err(ProviderBoundaryError::InvalidName {
            kind,
            detail: "too long",
        });
    }
    if value.trim() != value {
        return Err(ProviderBoundaryError::InvalidName {
            kind,
            detail: "leading or trailing whitespace",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ProviderBoundaryError::InvalidName {
            kind,
            detail: "control character",
        });
    }
    Ok(value)
}

fn arrow_field_contains_opaque_carrier(field: &Field) -> bool {
    if has_forbidden_opaque_metadata(field.metadata()) {
        return true;
    }
    let data_type = field.data_type();
    if matches!(
        data_type,
        DataType::Binary | DataType::LargeBinary | DataType::BinaryView
    ) || (matches!(
        data_type,
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
    ) && contains_opaque_token(field.name()))
    {
        return true;
    }
    match data_type {
        DataType::List(child)
        | DataType::ListView(child)
        | DataType::FixedSizeList(child, _)
        | DataType::LargeList(child)
        | DataType::LargeListView(child)
        | DataType::Map(child, _) => arrow_field_contains_opaque_carrier(child),
        DataType::Struct(children) => children
            .iter()
            .any(|child| arrow_field_contains_opaque_carrier(child)),
        DataType::Union(children, _) => children
            .iter()
            .any(|(_, child)| arrow_field_contains_opaque_carrier(child)),
        DataType::Dictionary(key, value) => {
            data_type_contains_opaque_binary(key) || data_type_contains_opaque_binary(value)
        }
        DataType::RunEndEncoded(run_ends, values) => {
            arrow_field_contains_opaque_carrier(run_ends)
                || arrow_field_contains_opaque_carrier(values)
        }
        _ => false,
    }
}

fn data_type_contains_opaque_binary(data_type: &DataType) -> bool {
    match data_type {
        DataType::Binary | DataType::LargeBinary | DataType::BinaryView => true,
        DataType::List(child)
        | DataType::ListView(child)
        | DataType::FixedSizeList(child, _)
        | DataType::LargeList(child)
        | DataType::LargeListView(child)
        | DataType::Map(child, _) => arrow_field_contains_opaque_carrier(child),
        DataType::Struct(children) => children
            .iter()
            .any(|child| arrow_field_contains_opaque_carrier(child)),
        DataType::Union(children, _) => children
            .iter()
            .any(|(_, child)| arrow_field_contains_opaque_carrier(child)),
        DataType::Dictionary(key, value) => {
            data_type_contains_opaque_binary(key) || data_type_contains_opaque_binary(value)
        }
        DataType::RunEndEncoded(run_ends, values) => {
            arrow_field_contains_opaque_carrier(run_ends)
                || arrow_field_contains_opaque_carrier(values)
        }
        _ => false,
    }
}

fn has_forbidden_opaque_metadata(metadata: &HashMap<String, String>) -> bool {
    metadata
        .iter()
        .any(|(key, value)| contains_opaque_token(key) || contains_opaque_token(value))
}

fn contains_opaque_token(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["json", "payload", "opaque", "blob"]
        .iter()
        .any(|token| value.contains(token))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{ArrayRef, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};

    use crate::relation_ipc::{
        CoverageRemainder, CoverageScope, RelationId, SchemaFingerprint, StreamId, StreamIdentity,
    };

    use super::*;

    fn family(value: &str) -> ProviderApiFamily {
        ProviderApiFamily::new(value).unwrap()
    }

    fn symbol(value: &str) -> UpstreamApiSymbol {
        UpstreamApiSymbol::new(value).unwrap()
    }

    fn provider_revision() -> ProviderRevision {
        ProviderRevision {
            provider_id: ProviderId([1; 16]),
            release: "provider-1.2.3".to_owned(),
            source_revision: [2; 32],
        }
    }

    fn installer() -> ProviderInstallerIdentity {
        ProviderInstallerIdentity {
            installer_id: ProviderInstallerId([3; 32]),
            owner: BoundaryOwnerId([4; 32]),
            provider_revision: provider_revision(),
        }
    }

    fn schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "native_kind",
            DataType::Utf8,
            false,
        )]))
    }

    fn boundary_field(schema: &SchemaRef) -> ProviderBoundaryField {
        ProviderBoundaryField {
            ordinal: 0,
            field: schema.fields()[0].clone(),
            meaning: FieldMeaning::ProviderNativeKind,
            provider_local_identity: ProviderLocalIdentityRole::None,
            canonical_identity: CanonicalIdentityRole::NotCanonical,
            coordinate: CoordinateRole::None,
            retention: RetentionPolicy::RetainProviderNative,
        }
    }

    fn unavailable() -> UnavailableBehavior {
        UnavailableBehavior {
            status: TerminalStatus::Partial,
            allowed_reasons: vec![
                RemainderReason::ProviderUnavailable,
                RemainderReason::ResourceLimit,
                RemainderReason::Cancelled,
                RemainderReason::InvalidSource,
                RemainderReason::Unsupported,
            ],
        }
    }

    fn required_row(marker: u8, family_name: &str) -> ProviderBoundaryContractRow {
        let schema = schema();
        ProviderBoundaryContractRow {
            api_family: family(family_name),
            upstream_symbols: vec![symbol("query::exact_symbol")],
            relation: ProviderArrowRelationContract {
                relation_id: RelationId([marker; 16]),
                schema_fingerprint: SchemaFingerprint([marker.wrapping_add(1); 32]),
                fields: vec![boundary_field(&schema)],
                schema,
            },
            authority: ProviderAuthorityRole::Primary,
            disposition: ContractDisposition::Required,
            unavailable_behavior: unavailable(),
            oracle_id: ProviderOracleId([marker.wrapping_add(2); 32]),
        }
    }

    fn contract(row: ProviderBoundaryContractRow) -> ProviderBoundaryContract {
        ProviderBoundaryContract {
            contract_id: BoundaryContractId([5; 32]),
            contract_revision: 1,
            provider_revision: provider_revision(),
            acceptance: IndependentContractAcceptance {
                author_owner: BoundaryOwnerId([6; 32]),
                reviewer_owner: BoundaryOwnerId([7; 32]),
                acceptance_authority: BoundaryOwnerId([8; 32]),
            },
            rows: vec![row],
        }
    }

    fn installed(row: &ProviderBoundaryContractRow) -> InstalledProviderSurface {
        InstalledProviderSurface {
            installer_id: installer().installer_id,
            handler_id: ProviderHandlerId([9; 16]),
            api_family: row.api_family.clone(),
            upstream_symbols: row.upstream_symbols.clone(),
            relation_id: row.relation.relation_id,
            schema_fingerprint: row.relation.schema_fingerprint,
            schema: row.relation.schema.clone(),
        }
    }

    fn batch(schema: &SchemaRef, values: &[&str]) -> RecordBatch {
        RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(StringArray::from(values.to_vec())) as ArrayRef],
        )
        .unwrap()
    }

    fn relation(row: &ProviderBoundaryContractRow, trailer: CoverageTrailer) -> AssembledRelation {
        AssembledRelation {
            identity: StreamIdentity {
                relation_id: row.relation.relation_id,
                stream_id: StreamId([10; 16]),
                schema_fingerprint: row.relation.schema_fingerprint,
                source_pin: SourcePin([11; 32]),
                context_pin: ContextPin([12; 32]),
            },
            schema: row.relation.schema.clone(),
            batches: vec![batch(&row.relation.schema, &["call_expression"])],
            ipc_bytes: vec![1],
            trailer,
        }
    }

    fn request(row: &ProviderBoundaryContractRow, units: u64) -> ProviderFamilyRequest {
        ProviderFamilyRequest {
            api_family: row.api_family.clone(),
            requested_units: units,
        }
    }

    fn coverage(
        row: &ProviderBoundaryContractRow,
        trailer: CoverageTrailer,
    ) -> ProviderFamilyCoverage {
        ProviderFamilyCoverage {
            api_family: row.api_family.clone(),
            trailer,
        }
    }

    fn evidence<'a>(
        surfaces: &'a [InstalledProviderSurface],
        requests: &'a [ProviderFamilyRequest],
        coverage: &'a [ProviderFamilyCoverage],
        relations: &'a [AssembledRelation],
    ) -> ProviderBoundaryEvidence<'a> {
        ProviderBoundaryEvidence {
            expected_source_pin: SourcePin([11; 32]),
            expected_context_pin: ContextPin([12; 32]),
            installed_surfaces: surfaces,
            requested_families: requests,
            family_coverage: coverage,
            relations,
        }
    }

    #[test]
    fn complete_surface_batches_and_coverage_match_independent_contract() {
        let row = required_row(20, "python.call_targets");
        let contract = contract(row.clone());
        let surfaces = [installed(&row)];
        let requests = [request(&row, 1)];
        let trailer = CoverageTrailer::complete(1);
        let coverage = [coverage(&row, trailer.clone())];
        let relations = [relation(&row, trailer)];
        let report = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &relations),
        )
        .unwrap();
        assert_eq!(report.status, TerminalStatus::Complete);
        assert_eq!(report.families.len(), 1);
        assert_eq!(
            report.families[0].surface,
            ProviderSurfaceOutcome::Installed
        );
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Complete { requested_units: 1 }
        );
    }

    #[test]
    fn installer_cannot_author_review_or_accept_its_own_contract() {
        let row = required_row(30, "python.members");
        let mut contract = contract(row);
        contract.acceptance.author_owner = installer().owner;
        let error = validate_provider_boundary_contract(&contract, &installer()).unwrap_err();
        assert_eq!(error, ProviderBoundaryError::OwnershipNotIndependent);
    }

    #[test]
    fn missing_surface_and_coverage_become_explicit_unknown() {
        let row = required_row(40, "python.import_resolution");
        let contract = contract(row.clone());
        let requests = [request(&row, 3)];
        let report =
            evaluate_provider_boundary(&contract, &installer(), evidence(&[], &requests, &[], &[]))
                .unwrap();
        assert_eq!(report.status, TerminalStatus::Unknown);
        assert_eq!(report.families[0].surface, ProviderSurfaceOutcome::Missing);
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Unknown {
                trailer: None,
                cause: ProviderUnknownCause::MissingCoverageDeclaration,
            }
        );
    }

    #[test]
    fn installed_surface_without_requested_execution_is_not_completeness_proof() {
        let row = required_row(45, "python.hover_types");
        let contract = contract(row.clone());
        let surfaces = [installed(&row)];
        let report =
            evaluate_provider_boundary(&contract, &installer(), evidence(&surfaces, &[], &[], &[]))
                .unwrap();
        assert_eq!(report.status, TerminalStatus::Unknown);
        assert_eq!(
            report.families[0].surface,
            ProviderSurfaceOutcome::Installed
        );
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::NotRequested
        );
    }

    #[test]
    fn provider_declared_unknown_and_missing_relation_are_distinct() {
        let mut row = required_row(47, "python.module_resolver");
        row.unavailable_behavior = UnavailableBehavior {
            status: TerminalStatus::Unknown,
            allowed_reasons: vec![RemainderReason::Unknown],
        };
        let contract = contract(row.clone());
        let surfaces = [installed(&row)];
        let requests = [request(&row, 2)];
        let trailer = CoverageTrailer {
            status: TerminalStatus::Unknown,
            requested_units: 2,
            completed_units: 0,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([3; 16]),
                unit_count: 2,
                reason: RemainderReason::Unknown,
            }],
        };
        let coverage = [coverage(&row, trailer.clone())];
        let mut unknown_relation = relation(&row, trailer);
        unknown_relation.batches.clear();
        let relations = [unknown_relation];
        let report = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &relations),
        )
        .unwrap();
        assert_eq!(report.status, TerminalStatus::Unknown);
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Unknown {
                trailer: Some(coverage[0].trailer.clone()),
                cause: ProviderUnknownCause::ProviderDeclared,
            }
        );

        let report = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &[]),
        )
        .unwrap();
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Unknown {
                trailer: Some(coverage[0].trailer.clone()),
                cause: ProviderUnknownCause::MissingArrowRelation,
            }
        );
    }

    #[test]
    fn partial_relation_preserves_completed_and_remainder_scope() {
        let row = required_row(50, "rust.mir_bodies");
        let contract = contract(row.clone());
        let surfaces = [installed(&row)];
        let requests = [request(&row, 2)];
        let trailer = CoverageTrailer {
            status: TerminalStatus::Partial,
            requested_units: 2,
            completed_units: 1,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([1; 16]),
                unit_count: 1,
                reason: RemainderReason::ProviderUnavailable,
            }],
        };
        let coverage = [coverage(&row, trailer.clone())];
        let relations = [relation(&row, trailer.clone())];
        let report = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &relations),
        )
        .unwrap();
        assert_eq!(report.status, TerminalStatus::Partial);
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Partial { trailer }
        );
    }

    #[test]
    fn completion_claim_without_relation_is_rejected() {
        let row = required_row(60, "python.types");
        let contract = contract(row.clone());
        let surfaces = [installed(&row)];
        let requests = [request(&row, 1)];
        let coverage = [coverage(&row, CoverageTrailer::complete(1))];
        let error = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &[]),
        )
        .unwrap_err();
        assert_eq!(error, ProviderBoundaryError::FalseCompletionClaim);
    }

    #[test]
    fn schema_drift_in_surface_or_batch_is_rejected() {
        let row = required_row(70, "rust.instances");
        let contract = contract(row.clone());
        let mut surface = installed(&row);
        surface.schema = Arc::new(Schema::new(vec![Field::new(
            "wrong",
            DataType::Utf8,
            false,
        )]));
        let error = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&[surface], &[], &[], &[]),
        )
        .unwrap_err();
        assert_eq!(error, ProviderBoundaryError::InstalledSurfaceMismatch);

        let surfaces = [installed(&row)];
        let requests = [request(&row, 1)];
        let trailer = CoverageTrailer::complete(1);
        let coverage = [coverage(&row, trailer.clone())];
        let mut wrong_relation = relation(&row, trailer);
        let wrong_schema = Arc::new(Schema::new(vec![Field::new(
            "wrong",
            DataType::Utf8,
            false,
        )]));
        wrong_relation.batches = vec![batch(&wrong_schema, &["x"])];
        let error = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&surfaces, &requests, &coverage, &[wrong_relation]),
        )
        .unwrap_err();
        assert_eq!(error, ProviderBoundaryError::ArrowBatchSchemaMismatch);
    }

    #[test]
    fn intentional_omission_is_a_counted_partial_remainder() {
        let mut row = required_row(80, "rust.exact_borrowck_loans");
        row.authority = ProviderAuthorityRole::ForbiddenProviderNative;
        row.disposition = ContractDisposition::IntentionalRemainder {
            reason: RemainderReason::Unsupported,
        };
        row.unavailable_behavior = UnavailableBehavior {
            status: TerminalStatus::Partial,
            allowed_reasons: vec![RemainderReason::Unsupported],
        };
        let contract = contract(row.clone());
        let requests = [request(&row, 1)];
        let trailer = CoverageTrailer {
            status: TerminalStatus::Partial,
            requested_units: 1,
            completed_units: 0,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([2; 16]),
                unit_count: 1,
                reason: RemainderReason::Unsupported,
            }],
        };
        let coverage = [coverage(&row, trailer.clone())];
        let report = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&[], &requests, &coverage, &[]),
        )
        .unwrap();
        assert_eq!(report.status, TerminalStatus::Partial);
        assert_eq!(
            report.families[0].surface,
            ProviderSurfaceOutcome::IntentionalRemainder
        );
        assert_eq!(
            report.families[0].run,
            ProviderFamilyRunOutcome::Partial { trailer }
        );
    }

    #[test]
    fn opaque_json_carrier_is_not_a_valid_typed_relation() {
        let mut row = required_row(90, "python.module_types");
        let schema = Arc::new(Schema::new(vec![Field::new(
            "semantic_json",
            DataType::Utf8,
            false,
        )]));
        row.relation.schema = schema.clone();
        row.relation.fields = vec![ProviderBoundaryField {
            ordinal: 0,
            field: schema.fields()[0].clone(),
            meaning: FieldMeaning::TypedFact,
            provider_local_identity: ProviderLocalIdentityRole::None,
            canonical_identity: CanonicalIdentityRole::NotCanonical,
            coordinate: CoordinateRole::None,
            retention: RetentionPolicy::RetainProviderNative,
        }];
        let error = validate_provider_boundary_contract(&contract(row), &installer()).unwrap_err();
        assert_eq!(error, ProviderBoundaryError::OpaqueSemanticCarrier);
    }

    #[test]
    fn variable_binary_cannot_be_used_as_an_opaque_semantic_carrier() {
        let mut row = required_row(95, "python.glean_facts");
        let schema = Arc::new(Schema::new(vec![Field::new(
            "provider_value",
            DataType::Binary,
            false,
        )]));
        row.relation.schema = schema.clone();
        row.relation.fields = vec![ProviderBoundaryField {
            ordinal: 0,
            field: schema.fields()[0].clone(),
            meaning: FieldMeaning::TypedFact,
            provider_local_identity: ProviderLocalIdentityRole::None,
            canonical_identity: CanonicalIdentityRole::NotCanonical,
            coordinate: CoordinateRole::None,
            retention: RetentionPolicy::RetainForProvenance,
        }];
        let error = validate_provider_boundary_contract(&contract(row), &installer()).unwrap_err();
        assert_eq!(error, ProviderBoundaryError::OpaqueSemanticCarrier);
    }

    #[test]
    fn provider_local_index_cannot_become_canonical_identity() {
        let mut row = required_row(100, "python.type_table");
        row.relation.fields[0].meaning = FieldMeaning::CanonicalIdentityInput;
        row.relation.fields[0].provider_local_identity =
            ProviderLocalIdentityRole::ResponseLocalIndex;
        row.relation.fields[0].canonical_identity = CanonicalIdentityRole::CanonicalIdentityInput;
        let error = validate_provider_boundary_contract(&contract(row), &installer()).unwrap_err();
        assert_eq!(error, ProviderBoundaryError::ProviderLocalIdentityPromoted);
    }

    #[test]
    fn byte_ranges_require_file_and_digest_coordinate_closure() {
        let mut row = required_row(110, "tree_sitter.nodes");
        row.relation.fields[0].meaning = FieldMeaning::Coordinate;
        row.relation.fields[0].coordinate = CoordinateRole::ByteStart;
        let error = validate_provider_boundary_contract(&contract(row), &installer()).unwrap_err();
        assert_eq!(error, ProviderBoundaryError::CoordinateClosureMissing);
    }

    #[test]
    fn uncontracted_surface_and_request_are_rejected() {
        let row = required_row(120, "python.definitions");
        let contract = contract(row.clone());
        let mut extra_surface = installed(&row);
        extra_surface.api_family = family("python.uncontracted");
        let error = evaluate_provider_boundary(
            &contract,
            &installer(),
            evidence(&[extra_surface], &[], &[], &[]),
        )
        .unwrap_err();
        assert_eq!(error, ProviderBoundaryError::UncontractedInstalledSurface);

        let requests = [ProviderFamilyRequest {
            api_family: family("python.uncontracted"),
            requested_units: 1,
        }];
        let error =
            evaluate_provider_boundary(&contract, &installer(), evidence(&[], &requests, &[], &[]))
                .unwrap_err();
        assert_eq!(error, ProviderBoundaryError::UncontractedFamilyRequest);
    }
}
