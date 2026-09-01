//! Relational provider capability derived from coverage and independent oracle proof.
//!
//! Installed handlers do not advertise support. This module joins an accepted provider boundary
//! report to independently produced oracle receipts and emits one typed Arrow row per contract
//! family. Missing proof remains unknown; partial coverage can never be promoted to complete.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{RecordBatch, StringArray, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use crate::fabric::proof::{OracleId, ProofRelations, ProofTerminalStatus};
use crate::fabric::{hash32_array, id16_array};
use crate::provider_boundary::{
    BoundaryContractId, ProviderAuthorityRole, ProviderBoundaryReport, ProviderFamilyOutcome,
    ProviderFamilyRunOutcome, ProviderId, ProviderOracleId, ProviderSurfaceOutcome,
    ProviderUnknownCause,
};
use crate::relation_ipc::{ContextPin, RelationId, SchemaFingerprint, SourcePin};

/// Independent executable-oracle result for one provider contract family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderOracleProofStatus {
    Pass,
    Fail,
    Unknown,
}

impl ProviderOracleProofStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unknown => "unknown",
        }
    }
}

/// Receipt supplied by the independently owned proof evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOracleProof {
    pub oracle_id: ProviderOracleId,
    pub proof_oracle_id: OracleId,
    pub proof_epoch_id: [u8; 16],
    pub proof_run_id: Option<[u8; 16]>,
    pub oracle_implementation: [u8; 32],
    pub contract_id: BoundaryContractId,
    pub contract_revision: u32,
    pub provider_id: ProviderId,
    pub provider_release: String,
    pub provider_source_revision: [u8; 32],
    pub relation_id: RelationId,
    pub schema_fingerprint: SchemaFingerprint,
    pub source_pin: SourcePin,
    pub context_pin: ContextPin,
    pub status: ProviderOracleProofStatus,
    pub receipt: [u8; 32],
}

/// Application-owned association between one provider contract family and one proof oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderOracleProofBinding {
    pub provider_oracle_id: ProviderOracleId,
    pub relation_id: RelationId,
    pub proof_oracle_id: OracleId,
}

/// Capability state derived from both run coverage and proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCapabilityState {
    ProvedComplete,
    Partial,
    Unknown,
    Rejected,
    NotRequested,
}

impl ProviderCapabilityState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ProvedComplete => "proved-complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
            Self::Rejected => "rejected",
            Self::NotRequested => "not-requested",
        }
    }
}

/// Why a capability is unknown even though its row remains queryable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCapabilityUnknownCause {
    MissingOracleProof,
    OracleProofUnknown,
    ProviderDeclared,
    MissingCoverage,
    MissingSurface,
    MissingRelation,
}

impl ProviderCapabilityUnknownCause {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MissingOracleProof => "missing-oracle-proof",
            Self::OracleProofUnknown => "oracle-proof-unknown",
            Self::ProviderDeclared => "provider-declared",
            Self::MissingCoverage => "missing-coverage",
            Self::MissingSurface => "missing-surface",
            Self::MissingRelation => "missing-relation",
        }
    }
}

/// Typed capability relation. Private fields prevent callers from substituting fabricated rows.
#[derive(Clone, Debug)]
pub struct ProviderCapabilityRelation {
    schema: SchemaRef,
    batch: RecordBatch,
}

impl ProviderCapabilityRelation {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn batch(&self) -> &RecordBatch {
        &self.batch
    }
}

/// Fail-closed capability derivation errors.
#[derive(Debug, Error)]
pub enum ProviderCapabilityError {
    #[error("provider boundary report contains no contract families")]
    EmptyReport,
    #[error("provider boundary report duplicates an oracle/relation family identity")]
    DuplicateBoundaryFamily,
    #[error("provider oracle proof identity is duplicated")]
    DuplicateProof,
    #[error("provider-to-proof oracle binding is duplicated")]
    DuplicateProofBinding,
    #[error("provider-to-proof oracle binding does not belong to the boundary report")]
    UnboundProofBinding,
    #[error("provider oracle proof does not belong to the boundary report")]
    UnboundProof,
    #[error("provider oracle proof pins differ from the accepted boundary family")]
    ProofPinMismatch,
    #[error(
        "provider oracle proof oracle, epoch, run, or implementation identity uses the zero sentinel"
    )]
    ZeroProofIdentity,
    #[error("provider oracle proof receipt uses the zero sentinel")]
    ZeroReceipt,
    #[error("provider coverage unit count overflowed")]
    CoverageOverflow,
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
}

#[derive(Clone, Copy)]
struct DerivedCapability<'a> {
    family: &'a crate::provider_boundary::ProviderFamilyOutcome,
    proof: Option<&'a ProviderOracleProof>,
    state: ProviderCapabilityState,
    run_status: &'static str,
    requested_units: Option<u64>,
    completed_units: Option<u64>,
    remainder_units: u64,
    unknown_cause: Option<ProviderCapabilityUnknownCause>,
}

/// Derive exact provider-family proof receipts from the executable proof engine's sealed output.
///
/// Bindings are typed application inputs. A binding whose proof oracle has no observation produces no receipt,
/// so downstream capability remains explicitly unknown. No status is accepted from a separate
/// boolean or provider-authored claim.
///
/// # Errors
///
/// Rejects duplicate/unbound bindings or a structurally invalid provider boundary report.
pub fn provider_oracle_proofs_from_executable_relations(
    report: &ProviderBoundaryReport,
    proof_relations: &ProofRelations,
    bindings: &[ProviderOracleProofBinding],
) -> Result<Vec<ProviderOracleProof>, ProviderCapabilityError> {
    if report.families.is_empty() {
        return Err(ProviderCapabilityError::EmptyReport);
    }
    let families = report
        .families
        .iter()
        .map(|family| ((family.oracle_id, family.relation_id), family))
        .collect::<BTreeMap<_, _>>();
    if families.len() != report.families.len() {
        return Err(ProviderCapabilityError::DuplicateBoundaryFamily);
    }
    let observations = proof_relations
        .oracle_observations()
        .iter()
        .map(|observation| (observation.oracle_id(), *observation))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeMap::new();
    let mut proofs = Vec::new();
    for binding in bindings {
        let key = (binding.provider_oracle_id, binding.relation_id);
        if seen.insert(key, binding.proof_oracle_id).is_some() {
            return Err(ProviderCapabilityError::DuplicateProofBinding);
        }
        let family = families
            .get(&key)
            .copied()
            .ok_or(ProviderCapabilityError::UnboundProofBinding)?;
        let Some(observation) = observations.get(&binding.proof_oracle_id).copied() else {
            continue;
        };
        let status = match observation.status() {
            ProofTerminalStatus::Pass => ProviderOracleProofStatus::Pass,
            ProofTerminalStatus::Fail => ProviderOracleProofStatus::Fail,
            ProofTerminalStatus::Unknown => ProviderOracleProofStatus::Unknown,
        };
        let pins = proof_relations.candidate_pins();
        proofs.push(ProviderOracleProof {
            oracle_id: family.oracle_id,
            proof_oracle_id: binding.proof_oracle_id,
            proof_epoch_id: *observation.epoch().as_bytes(),
            proof_run_id: observation.run_id().map(|run| *run.as_bytes()),
            oracle_implementation: *observation.implementation().as_bytes(),
            contract_id: report.contract_id,
            contract_revision: report.contract_revision,
            provider_id: report.provider_revision.provider_id,
            provider_release: report.provider_revision.release.clone(),
            provider_source_revision: report.provider_revision.source_revision,
            relation_id: family.relation_id,
            schema_fingerprint: family.schema_fingerprint,
            source_pin: report.source_pin,
            context_pin: report.context_pin,
            status,
            receipt: executable_proof_receipt(report, family, binding, observation, pins),
        });
    }
    Ok(proofs)
}

fn executable_proof_receipt(
    report: &ProviderBoundaryReport,
    family: &ProviderFamilyOutcome,
    binding: &ProviderOracleProofBinding,
    observation: crate::fabric::proof::ProofOracleObservation,
    pins: crate::fabric::proof::ProofCandidatePins,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.provider-oracle-proof-receipt.v1\0");
    hasher.update(observation.epoch().as_bytes());
    hasher.update(binding.proof_oracle_id.as_bytes());
    hasher.update(observation.implementation().as_bytes());
    if let Some(run) = observation.run_id() {
        hasher.update(run.as_bytes());
    }
    hasher.update(&[u8::try_from(observation.status().code())
        .expect("proof status codes are positive and bounded")]);
    hasher.update(pins.input_release.as_bytes());
    hasher.update(pins.program_release.as_bytes());
    hasher.update(pins.application_release.as_bytes());
    hasher.update(pins.source_authority.as_bytes());
    hasher.update(&pins.source_generation.get().to_be_bytes());
    hasher.update(pins.source_images.as_bytes());
    hasher.update(pins.provider_release.as_bytes());
    hasher.update(pins.provider_set.as_bytes());
    hasher.update(pins.table_versions.as_bytes());
    hasher.update(pins.overlay_segments.as_bytes());
    hasher.update(pins.policy_set.as_bytes());
    hasher.update(pins.resource_envelope.as_bytes());
    hasher.update(&report.contract_id.0);
    hasher.update(&report.contract_revision.to_be_bytes());
    hasher.update(&report.provider_revision.provider_id.0);
    hasher.update(report.provider_revision.release.as_bytes());
    hasher.update(&report.provider_revision.source_revision);
    hasher.update(&report.source_pin.0);
    hasher.update(&report.context_pin.0);
    hasher.update(&family.oracle_id.0);
    hasher.update(&family.relation_id.0);
    hasher.update(&family.schema_fingerprint.0);
    *hasher.finalize().as_bytes()
}

/// Join accepted coverage to exact oracle receipts and emit the capability relation.
///
/// # Errors
///
/// Rejects empty reports, duplicate/unbound proofs, zero proof receipts, coverage arithmetic
/// overflow, or Arrow construction failures.
pub fn derive_provider_capability_relation(
    report: &ProviderBoundaryReport,
    proofs: &[ProviderOracleProof],
) -> Result<ProviderCapabilityRelation, ProviderCapabilityError> {
    if report.families.is_empty() {
        return Err(ProviderCapabilityError::EmptyReport);
    }
    let mut family_by_key = BTreeMap::new();
    for family in &report.families {
        if family_by_key
            .insert((family.oracle_id, family.relation_id), family)
            .is_some()
        {
            return Err(ProviderCapabilityError::DuplicateBoundaryFamily);
        }
    }
    let mut by_family = BTreeMap::new();
    for proof in proofs {
        if *proof.proof_oracle_id.as_bytes() == [0; 16]
            || proof.proof_epoch_id == [0; 16]
            || proof.proof_run_id == Some([0; 16])
            || proof.oracle_implementation == [0; 32]
        {
            return Err(ProviderCapabilityError::ZeroProofIdentity);
        }
        if proof.receipt == [0; 32] {
            return Err(ProviderCapabilityError::ZeroReceipt);
        }
        let key = (proof.oracle_id, proof.relation_id);
        let family = family_by_key
            .get(&key)
            .copied()
            .ok_or(ProviderCapabilityError::UnboundProof)?;
        validate_proof_pins(report, family, proof)?;
        if by_family.insert(key, proof).is_some() {
            return Err(ProviderCapabilityError::DuplicateProof);
        }
    }

    let rows = report
        .families
        .iter()
        .map(|family| {
            derive_family(
                family,
                by_family
                    .get(&(family.oracle_id, family.relation_id))
                    .copied(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let schema = capability_schema();
    let batch = capability_batch(report, &rows, Arc::clone(&schema))?;
    Ok(ProviderCapabilityRelation { schema, batch })
}

fn validate_proof_pins(
    report: &ProviderBoundaryReport,
    family: &ProviderFamilyOutcome,
    proof: &ProviderOracleProof,
) -> Result<(), ProviderCapabilityError> {
    if proof.contract_id != report.contract_id
        || proof.contract_revision != report.contract_revision
        || proof.provider_id != report.provider_revision.provider_id
        || proof.provider_release != report.provider_revision.release
        || proof.provider_source_revision != report.provider_revision.source_revision
        || proof.relation_id != family.relation_id
        || proof.schema_fingerprint != family.schema_fingerprint
        || proof.source_pin != report.source_pin
        || proof.context_pin != report.context_pin
    {
        return Err(ProviderCapabilityError::ProofPinMismatch);
    }
    Ok(())
}

fn derive_family<'a>(
    family: &'a crate::provider_boundary::ProviderFamilyOutcome,
    proof: Option<&'a ProviderOracleProof>,
) -> Result<DerivedCapability<'a>, ProviderCapabilityError> {
    let proof_failed = proof.is_some_and(|proof| proof.status == ProviderOracleProofStatus::Fail);
    let proof_passed = proof.is_some_and(|proof| proof.status == ProviderOracleProofStatus::Pass);
    let incomplete_proof_cause = match proof.map(|proof| proof.status) {
        None => Some(ProviderCapabilityUnknownCause::MissingOracleProof),
        Some(ProviderOracleProofStatus::Unknown) => {
            Some(ProviderCapabilityUnknownCause::OracleProofUnknown)
        }
        Some(ProviderOracleProofStatus::Pass | ProviderOracleProofStatus::Fail) => None,
    };
    let (run_status, requested_units, completed_units, remainder_units, unknown_cause, base) =
        match &family.run {
            ProviderFamilyRunOutcome::NotRequested => (
                "not-requested",
                None,
                None,
                0,
                None,
                ProviderCapabilityState::NotRequested,
            ),
            ProviderFamilyRunOutcome::Complete { requested_units } => (
                "complete",
                Some(*requested_units),
                Some(*requested_units),
                0,
                incomplete_proof_cause,
                if proof_passed {
                    ProviderCapabilityState::ProvedComplete
                } else {
                    ProviderCapabilityState::Unknown
                },
            ),
            ProviderFamilyRunOutcome::Partial { trailer } => (
                "partial",
                Some(trailer.requested_units),
                Some(trailer.completed_units),
                remainder_units(trailer)?,
                None,
                ProviderCapabilityState::Partial,
            ),
            ProviderFamilyRunOutcome::Unknown { trailer, cause } => (
                "unknown",
                trailer.as_ref().map(|trailer| trailer.requested_units),
                trailer.as_ref().map(|trailer| trailer.completed_units),
                trailer
                    .as_ref()
                    .map(remainder_units)
                    .transpose()?
                    .unwrap_or(0),
                Some(unknown_cause(*cause)),
                ProviderCapabilityState::Unknown,
            ),
        };
    let state = if proof_failed {
        ProviderCapabilityState::Rejected
    } else {
        base
    };
    Ok(DerivedCapability {
        family,
        proof,
        state,
        run_status,
        requested_units,
        completed_units,
        remainder_units,
        unknown_cause,
    })
}

fn remainder_units(
    trailer: &crate::relation_ipc::CoverageTrailer,
) -> Result<u64, ProviderCapabilityError> {
    trailer.remainders.iter().try_fold(0_u64, |total, row| {
        total
            .checked_add(row.unit_count)
            .ok_or(ProviderCapabilityError::CoverageOverflow)
    })
}

fn capability_schema() -> SchemaRef {
    let relation_id = "system.provider_capability.v1";
    let fields = vec![
        Field::new("contract_id", DataType::FixedSizeBinary(32), false),
        Field::new("contract_revision", DataType::UInt32, false),
        Field::new("installer_id", DataType::FixedSizeBinary(32), false),
        Field::new("provider_id", DataType::FixedSizeBinary(16), false),
        Field::new("provider_release", DataType::Utf8, false),
        Field::new(
            "provider_source_revision",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new("source_pin", DataType::FixedSizeBinary(32), false),
        Field::new("context_pin", DataType::FixedSizeBinary(32), false),
        Field::new("api_family", DataType::Utf8, false),
        Field::new("relation_id", DataType::FixedSizeBinary(16), false),
        Field::new("schema_fingerprint", DataType::FixedSizeBinary(32), false),
        Field::new("authority", DataType::Utf8, false),
        Field::new("surface_status", DataType::Utf8, false),
        Field::new("run_status", DataType::Utf8, false),
        Field::new("requested_units", DataType::UInt64, true),
        Field::new("completed_units", DataType::UInt64, true),
        Field::new("remainder_units", DataType::UInt64, false),
        Field::new("provider_oracle_id", DataType::FixedSizeBinary(32), false),
        Field::new("proof_oracle_id", DataType::FixedSizeBinary(16), true),
        Field::new("proof_epoch_id", DataType::FixedSizeBinary(16), true),
        Field::new("proof_run_id", DataType::FixedSizeBinary(16), true),
        Field::new("oracle_implementation", DataType::FixedSizeBinary(32), true),
        Field::new("proof_status", DataType::Utf8, false),
        Field::new("proof_receipt", DataType::FixedSizeBinary(32), true),
        Field::new("capability_state", DataType::Utf8, false),
        Field::new("unknown_cause", DataType::Utf8, true),
    ]
    .into_iter()
    .map(|field| {
        let field_id = format!("{relation_id}.{}", field.name());
        field.with_metadata(
            [("codefabric.field_id".to_owned(), field_id)]
                .into_iter()
                .collect(),
        )
    })
    .collect::<Vec<_>>();
    Arc::new(Schema::new_with_metadata(
        fields,
        [
            ("codefabric.relation_id".to_owned(), relation_id.to_owned()),
            ("codefabric.relation".to_owned(), relation_id.to_owned()),
            (
                "codefabric.derivation".to_owned(),
                "provider-boundary-coverage+independent-oracle-proof".to_owned(),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

fn capability_batch(
    report: &ProviderBoundaryReport,
    rows: &[DerivedCapability<'_>],
    schema: SchemaRef,
) -> Result<RecordBatch, arrow_schema::ArrowError> {
    let count = rows.len();
    let contract = report.contract_id.0;
    let installer = report.installer_id.0;
    let provider = report.provider_revision.provider_id.0;
    let source_revision = report.provider_revision.source_revision;
    let source_pin = report.source_pin.0;
    let context_pin = report.context_pin.0;
    RecordBatch::try_new(
        schema,
        vec![
            hash32_array((0..count).map(|_| Some(&contract))),
            Arc::new(UInt32Array::from(vec![report.contract_revision; count])),
            hash32_array((0..count).map(|_| Some(&installer))),
            id16_array((0..count).map(|_| Some(&provider))),
            Arc::new(StringArray::from(vec![
                report
                    .provider_revision
                    .release
                    .as_str();
                count
            ])),
            hash32_array((0..count).map(|_| Some(&source_revision))),
            hash32_array((0..count).map(|_| Some(&source_pin))),
            hash32_array((0..count).map(|_| Some(&context_pin))),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.family.api_family.as_str())
                    .collect::<Vec<_>>(),
            )),
            id16_array(rows.iter().map(|row| Some(&row.family.relation_id.0))),
            hash32_array(
                rows.iter()
                    .map(|row| Some(&row.family.schema_fingerprint.0)),
            ),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| authority(row.family.authority))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| surface(row.family.surface))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter().map(|row| row.run_status).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.requested_units)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.completed_units)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.remainder_units)
                    .collect::<Vec<_>>(),
            )),
            hash32_array(rows.iter().map(|row| Some(&row.family.oracle_id.0))),
            id16_array(
                rows.iter()
                    .map(|row| row.proof.map(|proof| proof.proof_oracle_id.as_bytes())),
            ),
            id16_array(
                rows.iter()
                    .map(|row| row.proof.map(|proof| &proof.proof_epoch_id)),
            ),
            id16_array(
                rows.iter()
                    .map(|row| row.proof.and_then(|proof| proof.proof_run_id.as_ref())),
            ),
            hash32_array(
                rows.iter()
                    .map(|row| row.proof.map(|proof| &proof.oracle_implementation)),
            ),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.proof.map_or("missing", |proof| proof.status.as_str()))
                    .collect::<Vec<_>>(),
            )),
            hash32_array(rows.iter().map(|row| row.proof.map(|proof| &proof.receipt))),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.state.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| {
                        row.unknown_cause
                            .map(ProviderCapabilityUnknownCause::as_str)
                    })
                    .collect::<Vec<_>>(),
            )),
        ],
    )
}

const fn authority(value: ProviderAuthorityRole) -> &'static str {
    match value {
        ProviderAuthorityRole::Primary => "primary",
        ProviderAuthorityRole::Fallback => "fallback",
        ProviderAuthorityRole::Corroborating => "corroborating",
        ProviderAuthorityRole::NarrowEnrichment => "narrow-enrichment",
        ProviderAuthorityRole::ForbiddenProviderNative => "forbidden-provider-native",
    }
}

const fn surface(value: ProviderSurfaceOutcome) -> &'static str {
    match value {
        ProviderSurfaceOutcome::Installed => "installed",
        ProviderSurfaceOutcome::Missing => "missing",
        ProviderSurfaceOutcome::IntentionalRemainder => "intentional-remainder",
    }
}

const fn unknown_cause(value: ProviderUnknownCause) -> ProviderCapabilityUnknownCause {
    match value {
        ProviderUnknownCause::ProviderDeclared => ProviderCapabilityUnknownCause::ProviderDeclared,
        ProviderUnknownCause::MissingCoverageDeclaration => {
            ProviderCapabilityUnknownCause::MissingCoverage
        }
        ProviderUnknownCause::MissingInstalledSurface => {
            ProviderCapabilityUnknownCause::MissingSurface
        }
        ProviderUnknownCause::MissingArrowRelation => {
            ProviderCapabilityUnknownCause::MissingRelation
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow_array::{Array as _, FixedSizeBinaryArray};

    use super::*;
    use crate::fabric::command::EpochId;
    use crate::fabric::proof::{
        OracleId, OracleImplementationRef, ProofRunId, test_relations_with_oracle,
    };
    use crate::provider_boundary::{
        BoundaryContractId, ProviderApiFamily, ProviderFamilyOutcome, ProviderId,
        ProviderInstallerId, ProviderRevision,
    };
    use crate::relation_ipc::{
        ContextPin, CoverageRemainder, CoverageScope, CoverageTrailer, RelationId, RemainderReason,
        SchemaFingerprint, SourcePin, TerminalStatus,
    };

    fn family(marker: u8, name: &str, run: ProviderFamilyRunOutcome) -> ProviderFamilyOutcome {
        ProviderFamilyOutcome {
            api_family: ProviderApiFamily::new(name).unwrap(),
            relation_id: RelationId([marker; 16]),
            schema_fingerprint: SchemaFingerprint([marker; 32]),
            authority: ProviderAuthorityRole::Primary,
            handler_id: None,
            surface: ProviderSurfaceOutcome::Installed,
            run,
            oracle_id: ProviderOracleId([marker.wrapping_add(10); 32]),
        }
    }

    fn report() -> ProviderBoundaryReport {
        ProviderBoundaryReport {
            contract_id: BoundaryContractId([1; 32]),
            contract_revision: 4,
            installer_id: ProviderInstallerId([2; 32]),
            provider_revision: ProviderRevision {
                provider_id: ProviderId([3; 16]),
                release: "provider-release".into(),
                source_revision: [4; 32],
            },
            source_pin: SourcePin([5; 32]),
            context_pin: ContextPin([6; 32]),
            status: TerminalStatus::Partial,
            families: vec![
                family(
                    10,
                    "provider.complete",
                    ProviderFamilyRunOutcome::Complete { requested_units: 2 },
                ),
                family(
                    20,
                    "provider.partial",
                    ProviderFamilyRunOutcome::Partial {
                        trailer: CoverageTrailer {
                            status: TerminalStatus::Partial,
                            requested_units: 3,
                            completed_units: 2,
                            remainders: vec![CoverageRemainder {
                                scope: CoverageScope([7; 16]),
                                unit_count: 1,
                                reason: RemainderReason::Unsupported,
                            }],
                        },
                    },
                ),
                family(
                    30,
                    "provider.unknown",
                    ProviderFamilyRunOutcome::Unknown {
                        trailer: None,
                        cause: ProviderUnknownCause::MissingArrowRelation,
                    },
                ),
            ],
        }
    }

    fn proof(
        report: &ProviderBoundaryReport,
        family_index: usize,
        status: ProviderOracleProofStatus,
        marker: u8,
    ) -> ProviderOracleProof {
        let family = &report.families[family_index];
        ProviderOracleProof {
            oracle_id: family.oracle_id,
            proof_oracle_id: OracleId::new([marker.wrapping_add(4); 16]).unwrap(),
            proof_epoch_id: [marker.wrapping_add(3); 16],
            proof_run_id: Some([marker; 16]),
            oracle_implementation: [marker.wrapping_add(1); 32],
            contract_id: report.contract_id,
            contract_revision: report.contract_revision,
            provider_id: report.provider_revision.provider_id,
            provider_release: report.provider_revision.release.clone(),
            provider_source_revision: report.provider_revision.source_revision,
            relation_id: family.relation_id,
            schema_fingerprint: family.schema_fingerprint,
            source_pin: report.source_pin,
            context_pin: report.context_pin,
            status,
            receipt: [marker.wrapping_add(2); 32],
        }
    }

    #[test]
    fn capability_requires_complete_coverage_and_passing_oracle() {
        let report = report();
        let proofs = [
            proof(&report, 0, ProviderOracleProofStatus::Pass, 41),
            proof(&report, 1, ProviderOracleProofStatus::Pass, 42),
            proof(&report, 2, ProviderOracleProofStatus::Fail, 43),
        ];
        let relation = derive_provider_capability_relation(&report, &proofs).unwrap();
        assert_eq!(relation.batch().num_rows(), 3);
        let states = relation
            .batch()
            .column(24)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(states.value(0), "proved-complete");
        assert_eq!(states.value(1), "partial");
        assert_eq!(states.value(2), "rejected");
        let receipts = relation
            .batch()
            .column(23)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert!(receipts.iter().all(|value| value.is_some()));
    }

    #[test]
    fn missing_proof_never_advertises_complete_support() {
        let relation = derive_provider_capability_relation(&report(), &[]).unwrap();
        let states = relation
            .batch()
            .column(24)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(states.value(0), "unknown");
        let proof_status = relation
            .batch()
            .column(22)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(proof_status.value(0), "missing");
        assert!(relation.batch().column(18).is_null(0));
        assert!(relation.batch().column(19).is_null(0));
        assert!(relation.batch().column(20).is_null(0));
        assert!(relation.batch().column(21).is_null(0));
        assert!(relation.batch().column(23).is_null(0));
        let unknown_cause = relation
            .batch()
            .column(25)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(unknown_cause.value(0), "missing-oracle-proof");
    }

    #[test]
    fn unknown_proof_is_explicit_and_unbound_or_ambiguous_evidence_is_rejected() {
        let report = report();
        let unknown = proof(&report, 0, ProviderOracleProofStatus::Unknown, 51);
        let relation =
            derive_provider_capability_relation(&report, std::slice::from_ref(&unknown)).unwrap();
        let causes = relation
            .batch()
            .column(25)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(causes.value(0), "oracle-proof-unknown");

        assert!(matches!(
            derive_provider_capability_relation(&report, &[unknown.clone(), unknown]),
            Err(ProviderCapabilityError::DuplicateProof)
        ));
        assert!(matches!(
            derive_provider_capability_relation(
                &report,
                &[ProviderOracleProof {
                    oracle_id: ProviderOracleId([99; 32]),
                    ..proof(&report, 0, ProviderOracleProofStatus::Pass, 52)
                }],
            ),
            Err(ProviderCapabilityError::UnboundProof)
        ));
        assert!(matches!(
            derive_provider_capability_relation(
                &report,
                &[ProviderOracleProof {
                    receipt: [0; 32],
                    ..proof(&report, 0, ProviderOracleProofStatus::Pass, 53)
                }],
            ),
            Err(ProviderCapabilityError::ZeroReceipt)
        ));
        assert!(matches!(
            derive_provider_capability_relation(
                &report,
                &[ProviderOracleProof {
                    context_pin: ContextPin([88; 32]),
                    ..proof(&report, 0, ProviderOracleProofStatus::Pass, 54)
                }],
            ),
            Err(ProviderCapabilityError::ProofPinMismatch)
        ));
        assert!(matches!(
            derive_provider_capability_relation(
                &report,
                &[ProviderOracleProof {
                    proof_run_id: Some([0; 16]),
                    ..proof(&report, 0, ProviderOracleProofStatus::Pass, 55)
                }],
            ),
            Err(ProviderCapabilityError::ZeroProofIdentity)
        ));
    }

    #[test]
    fn executable_proof_relations_are_the_receipt_authority() {
        let report = report();
        let proof_oracle = OracleId::new([81; 16]).unwrap();
        let implementation = OracleImplementationRef::new([82; 32]).unwrap();
        let run = ProofRunId::new([83; 16]).unwrap();
        let relations = test_relations_with_oracle(
            EpochId::from_bytes([84; 16]),
            proof_oracle,
            implementation,
            Some(run),
            ProofTerminalStatus::Pass,
        );
        let proofs = provider_oracle_proofs_from_executable_relations(
            &report,
            &relations,
            &[ProviderOracleProofBinding {
                provider_oracle_id: report.families[0].oracle_id,
                relation_id: report.families[0].relation_id,
                proof_oracle_id: proof_oracle,
            }],
        )
        .unwrap();
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].proof_oracle_id, proof_oracle);
        assert_eq!(proofs[0].proof_epoch_id, [84; 16]);
        assert_eq!(proofs[0].proof_run_id, Some([83; 16]));
        assert_eq!(proofs[0].oracle_implementation, [82; 32]);
        assert_ne!(proofs[0].receipt, [0; 32]);

        let capability = derive_provider_capability_relation(&report, &proofs).unwrap();
        let states = capability
            .batch()
            .column(24)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(states.value(0), "proved-complete");
        assert_eq!(states.value(1), "partial");
        assert_eq!(states.value(2), "unknown");
    }
}
