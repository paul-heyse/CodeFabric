//! Atomic composition of application-owned derived analyses into a programmatic epoch.
//!
//! Exact provider admission remains the only raw-input authority. This module consumes that
//! admission outcome, closes a runtime-supplied derived-family inventory with exactly one
//! application producer or one explicit remainder, and registers native DataFusion
//! transformations into the same candidate builder. Producer metadata is projected onto every
//! output row and is also folded into the transformation provenance identity, so changing an
//! algorithm, precision, input vector, completeness contract, witness binding, or application
//! owner changes the catalog-observed authority.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU32;
use std::sync::Arc;

use arrow_array::builder::{FixedSizeBinaryBuilder, StringBuilder};
use arrow_array::{Array, ArrayRef, BooleanArray, FixedSizeBinaryArray, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use datafusion::common::{Column, ScalarValue};
use datafusion::functions_aggregate::expr_fn::count;
use datafusion::functions_aggregate::string_agg::string_agg;
use datafusion::functions_window::expr_fn::row_number;
use datafusion::logical_expr::expr_fn::cast;
use datafusion::logical_expr::{
    ColumnarValue, Expr, ExprFunctionExt, JoinType, LogicalPlan, LogicalPlanBuilder, ScalarUDF,
    SortExpr, Volatility, create_udf,
};
use datafusion::prelude::{col, lit};
use thiserror::Error;

use crate::common_derived_analysis::{CommonAnalysisBindings, CommonAnalysisFamilies};
use crate::fabric::epoch_runtime::FabricEpochId;
use crate::fabric::production_kernel::CompiledTransformationAuthority;
use crate::fabric::programmatic_epoch::{
    ProgrammaticFabricEpochBuilder, ProgrammaticFabricEpochError,
};
use crate::fabric::programmatic_schema::{
    ProgrammaticFieldId, ProgrammaticRelationId, ProgrammaticTransformation,
    ProgrammaticTransformationContract, ProgrammaticTransformationId,
    TransformationDeterminismPolicy, TransformationFieldIdentity, TransformationInputs,
    TransformationOrderingPolicy, TransformationOutput, TransformationPlanError,
    TransformationProvenance, TransformationProvenanceIdentity, TransformationRecursionPolicy,
    TransformationReleaseIdentity, TransformationResourceClass, TransformationSemanticVersion,
};
use crate::provider_admission::{
    ExactProgrammaticProviderReports, ExactProgrammaticProviderRuns,
    ProgrammaticProviderAdmissionOutcome, ProviderAdmissionError, ProviderAuthorityClass,
    ProviderNativeLane, ProviderRegistrationDisposition, admit_provider_relations_programmatic,
};
use crate::provider_native_syntax::NativeSyntaxRelation;
use crate::pyrefly_service::PyreflyRelation;
use crate::python_derived_analysis::{PythonDerivedRelation, PythonFlowBindings};
use crate::relation_ipc::{CoverageTrailer, RemainderReason, TerminalStatus};
use crate::rust_mir_derived_analysis::{RustMirAnalysisBindings, RustMirDerivedRelation};
use crate::rustc_relation_schema::RustcRelation;
use crate::schema_contract::FIELD_ID_METADATA_KEY;

const MAX_DERIVED_FAMILIES: usize = 4_096;
const MAX_DERIVED_IDENTITY_BYTES: usize = 512;

/// Aggregate pre-registration envelope for one derived-analysis composition.
///
/// Individual [`ProgrammaticTransformation`] contracts remain the execution authority for each
/// output's rows, memory, and spill. This envelope closes the higher-level planning hazard where
/// a caller could otherwise submit thousands of individually bounded producers whose combined
/// declared work is effectively unbounded. The aggregate is checked before any derived
/// transformation is registered into the candidate epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedAnalysisResourceEnvelope {
    producer_limit: u64,
    remainder_limit: u64,
    dependency_edge_limit: u64,
    declared_row_limit: u64,
    declared_memory_byte_limit: u64,
    declared_spill_byte_limit: u64,
}

impl DerivedAnalysisResourceEnvelope {
    /// Construct an explicit non-zero aggregate envelope.
    ///
    /// # Errors
    ///
    /// Rejects every zero limit so no bound can be interpreted as unlimited.
    #[allow(clippy::result_large_err)]
    pub fn try_new(
        max_producers: u64,
        max_remainders: u64,
        max_dependency_edges: u64,
        max_declared_rows: u64,
        max_declared_memory_bytes: u64,
        max_declared_spill_bytes: u64,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        for (resource, value) in [
            ("max_producers", max_producers),
            ("max_remainders", max_remainders),
            ("max_dependency_edges", max_dependency_edges),
            ("max_declared_rows", max_declared_rows),
            ("max_declared_memory_bytes", max_declared_memory_bytes),
            ("max_declared_spill_bytes", max_declared_spill_bytes),
        ] {
            if value == 0 {
                return Err(ProgrammaticDerivedAnalysisError::ZeroResourceBound(
                    resource,
                ));
            }
        }
        Ok(Self {
            producer_limit: max_producers,
            remainder_limit: max_remainders,
            dependency_edge_limit: max_dependency_edges,
            declared_row_limit: max_declared_rows,
            declared_memory_byte_limit: max_declared_memory_bytes,
            declared_spill_byte_limit: max_declared_spill_bytes,
        })
    }

    /// Bounded workstation policy used by the compatibility constructor.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            producer_limit: 4_096,
            remainder_limit: 4_096,
            dependency_edge_limit: 262_144,
            declared_row_limit: 1_000_000_000,
            declared_memory_byte_limit: 64 * 1024 * 1024 * 1024,
            declared_spill_byte_limit: 256 * 1024 * 1024 * 1024,
        }
    }

    #[must_use]
    pub const fn max_producers(self) -> u64 {
        self.producer_limit
    }

    #[must_use]
    pub const fn max_remainders(self) -> u64 {
        self.remainder_limit
    }

    #[must_use]
    pub const fn max_dependency_edges(self) -> u64 {
        self.dependency_edge_limit
    }

    #[must_use]
    pub const fn max_declared_rows(self) -> u64 {
        self.declared_row_limit
    }

    #[must_use]
    pub const fn max_declared_memory_bytes(self) -> u64 {
        self.declared_memory_byte_limit
    }

    #[must_use]
    pub const fn max_declared_spill_bytes(self) -> u64 {
        self.declared_spill_byte_limit
    }
}

/// Exact aggregate declared resources retained in the causal closure observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedAnalysisResourceObservation {
    pub envelope: DerivedAnalysisResourceEnvelope,
    pub producer_count: u64,
    pub remainder_count: u64,
    pub dependency_edge_count: u64,
    pub declared_max_rows: u64,
    pub declared_max_memory_bytes: u64,
    pub declared_max_spill_bytes: u64,
}

/// Runtime identity of one accepted application-derived fact family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DerivedFamilyId(Arc<str>);

impl DerivedFamilyId {
    /// Construct a bounded, non-sentinel family identity.
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        value: impl Into<Arc<str>>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let value = value.into();
        validate_text("derived family", &value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Application analysis domain. Provider lanes never inhabit this type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedAnalysisDomain {
    Python,
    RustMir,
    Common,
}

/// Whether a family contains semantic facts or explicit uncertainty evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedFamilyKind {
    Fact,
    UnknownEvidence,
}

/// Exact declared algorithm identity, semantic version, and immutable release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAlgorithmContract {
    semantic_id: ProgrammaticTransformationId,
    semantic_version: TransformationSemanticVersion,
    release_identity: TransformationReleaseIdentity,
}

impl DerivedAlgorithmContract {
    #[must_use]
    pub(crate) const fn new(
        _authority: &CompiledTransformationAuthority,
        semantic_id: ProgrammaticTransformationId,
        semantic_version: TransformationSemanticVersion,
        release_identity: TransformationReleaseIdentity,
    ) -> Self {
        Self {
            semantic_id,
            semantic_version,
            release_identity,
        }
    }

    #[must_use]
    pub const fn semantic_id(&self) -> &ProgrammaticTransformationId {
        &self.semantic_id
    }

    #[must_use]
    pub const fn semantic_version(&self) -> TransformationSemanticVersion {
        self.semantic_version
    }

    #[must_use]
    pub const fn release_identity(&self) -> TransformationReleaseIdentity {
        self.release_identity
    }
}

/// Precision promised by one accepted application algorithm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedPrecisionPolicy {
    Exact,
    SoundMay,
    SoundMust,
    Bounded { max_steps: NonZeroU32 },
}

/// Completeness carried by one producer output.
///
/// Partial or unknown output is legal only when another accepted producer emits the named
/// unknown-evidence family. Unsupported work is represented by [`ExplicitDerivedRemainder`], not
/// by an empty producer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedCompletenessPolicy {
    Complete,
    Partial { unknown_family: DerivedFamilyId },
    Unknown { unknown_family: DerivedFamilyId },
}

/// Exact application authority or a forbidden provider-native ownership claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedProducerAuthority {
    ApplicationOwned([u8; 32]),
    ProviderNative(ProviderNativeLane),
}

/// One runtime-supplied accepted family and its exact dependency contract.
#[derive(Clone, Debug)]
pub struct AcceptedDerivedFamily {
    family_id: DerivedFamilyId,
    domain: DerivedAnalysisDomain,
    kind: DerivedFamilyKind,
    algorithm: DerivedAlgorithmContract,
    precision: DerivedPrecisionPolicy,
    output_relation_id: ProgrammaticRelationId,
    dependencies: Arc<[ProgrammaticRelationId]>,
}

impl AcceptedDerivedFamily {
    /// Define one accepted family without consulting a static semantic inventory.
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        family_id: DerivedFamilyId,
        domain: DerivedAnalysisDomain,
        kind: DerivedFamilyKind,
        algorithm: DerivedAlgorithmContract,
        precision: DerivedPrecisionPolicy,
        output_relation_id: ProgrammaticRelationId,
        dependencies: impl Into<Arc<[ProgrammaticRelationId]>>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        validate_text("transformation", algorithm.semantic_id().as_str())?;
        validate_text("output relation", output_relation_id.as_str())?;
        if algorithm.semantic_version() == TransformationSemanticVersion::new(0, 0, 0) {
            return Err(ProgrammaticDerivedAnalysisError::SentinelAlgorithmVersion { family_id });
        }
        if algorithm.release_identity().as_bytes() == &[0; 32] {
            return Err(ProgrammaticDerivedAnalysisError::SentinelAlgorithmRelease { family_id });
        }
        let dependencies = dependencies.into();
        let mut unique = BTreeSet::new();
        for dependency in dependencies.iter() {
            validate_text("input relation", dependency.as_str())?;
            if !unique.insert(dependency.clone()) {
                return Err(
                    ProgrammaticDerivedAnalysisError::DuplicateFamilyDependency {
                        family_id,
                        relation_id: dependency.clone(),
                    },
                );
            }
        }
        Ok(Self {
            family_id,
            domain,
            kind,
            algorithm,
            precision,
            output_relation_id,
            dependencies,
        })
    }

    #[must_use]
    pub const fn family_id(&self) -> &DerivedFamilyId {
        &self.family_id
    }

    #[must_use]
    pub const fn domain(&self) -> DerivedAnalysisDomain {
        self.domain
    }

    #[must_use]
    pub const fn kind(&self) -> DerivedFamilyKind {
        self.kind
    }

    #[must_use]
    pub const fn output_relation_id(&self) -> &ProgrammaticRelationId {
        &self.output_relation_id
    }

    #[must_use]
    pub fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependencies
    }
}

/// One accepted application producer backed by a typed programmatic transformation.
#[derive(Clone)]
pub struct AcceptedDerivedProducer {
    family_id: DerivedFamilyId,
    authority: DerivedProducerAuthority,
    algorithm: DerivedAlgorithmContract,
    precision: DerivedPrecisionPolicy,
    completeness: DerivedCompletenessPolicy,
    witness_field_id: ProgrammaticFieldId,
    transformation: Arc<dyn ProgrammaticTransformation>,
}

impl AcceptedDerivedProducer {
    #[must_use]
    pub(crate) fn new(
        _authority: &CompiledTransformationAuthority,
        family_id: DerivedFamilyId,
        authority: DerivedProducerAuthority,
        algorithm: DerivedAlgorithmContract,
        precision: DerivedPrecisionPolicy,
        completeness: DerivedCompletenessPolicy,
        witness_field_id: ProgrammaticFieldId,
        transformation: Arc<dyn ProgrammaticTransformation>,
    ) -> Self {
        Self {
            family_id,
            authority,
            algorithm,
            precision,
            completeness,
            witness_field_id,
            transformation,
        }
    }

    #[must_use]
    pub const fn family_id(&self) -> &DerivedFamilyId {
        &self.family_id
    }
}

impl std::fmt::Debug for AcceptedDerivedProducer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcceptedDerivedProducer")
            .field("family_id", &self.family_id)
            .field("authority", &self.authority)
            .field("algorithm", &self.algorithm)
            .field("precision", &self.precision)
            .field("completeness", &self.completeness)
            .field("witness_field_id", &self.witness_field_id)
            .field("transformation_id", self.transformation.id())
            .finish()
    }
}

/// Why an accepted family has no executable producer in this epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedRemainderReason {
    Unsupported,
    ProviderUnavailable,
    ResourceLimit,
    AlgorithmUnavailable,
    PrivateCompilerEvidenceUnavailable,
    /// The existing procedural producer has not yet been rewritten against catalog inputs.
    TypedTransformationAdapterUnavailable,
}

/// Whether a later epoch can retry an explicit remainder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedRemainderRetryability {
    Retryable,
    RequiresReleaseChange,
    PermanentlyUnsupported,
}

/// Explicit typed disposition for an accepted family with no producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitDerivedRemainder {
    family_id: DerivedFamilyId,
    algorithm: DerivedAlgorithmContract,
    reason: DerivedRemainderReason,
    evidence_identity: [u8; 32],
    retryability: DerivedRemainderRetryability,
}

impl ExplicitDerivedRemainder {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        family_id: DerivedFamilyId,
        algorithm: DerivedAlgorithmContract,
        reason: DerivedRemainderReason,
        evidence_identity: [u8; 32],
        retryability: DerivedRemainderRetryability,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        if evidence_identity == [0; 32] {
            return Err(ProgrammaticDerivedAnalysisError::SentinelRemainderEvidence { family_id });
        }
        Ok(Self {
            family_id,
            algorithm,
            reason,
            evidence_identity,
            retryability,
        })
    }

    #[must_use]
    pub const fn family_id(&self) -> &DerivedFamilyId {
        &self.family_id
    }
}

/// Exactly one proposed disposition for one accepted family.
#[derive(Clone, Debug)]
pub enum DerivedFamilyDisposition {
    Producer(AcceptedDerivedProducer),
    Remainder(ExplicitDerivedRemainder),
}

impl DerivedFamilyDisposition {
    fn family_id(&self) -> &DerivedFamilyId {
        match self {
            Self::Producer(producer) => producer.family_id(),
            Self::Remainder(remainder) => remainder.family_id(),
        }
    }
}

/// Closed roles projected onto every derived output row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedMetadataRole {
    FamilyIdentity,
    Domain,
    AuthorityIdentity,
    AlgorithmIdentity,
    AlgorithmVersionMajor,
    AlgorithmVersionMinor,
    AlgorithmVersionPatch,
    ReleaseIdentity,
    PrecisionIdentity,
    InputVectorIdentity,
    CompletenessState,
    ProvenanceClosureIdentity,
}

impl DerivedMetadataRole {
    pub const ALL: [Self; 12] = [
        Self::FamilyIdentity,
        Self::Domain,
        Self::AuthorityIdentity,
        Self::AlgorithmIdentity,
        Self::AlgorithmVersionMajor,
        Self::AlgorithmVersionMinor,
        Self::AlgorithmVersionPatch,
        Self::ReleaseIdentity,
        Self::PrecisionIdentity,
        Self::InputVectorIdentity,
        Self::CompletenessState,
        Self::ProvenanceClosureIdentity,
    ];
}

/// Application-supplied physical and semantic binding for one metadata role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedMetadataColumnBinding {
    role: DerivedMetadataRole,
    field_id: ProgrammaticFieldId,
    physical_name: Arc<str>,
}

impl DerivedMetadataColumnBinding {
    #[must_use]
    pub(crate) fn new(
        _authority: &CompiledTransformationAuthority,
        role: DerivedMetadataRole,
        field_id: ProgrammaticFieldId,
        physical_name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            role,
            field_id,
            physical_name: physical_name.into(),
        }
    }
}

/// Exact field/name bindings for producer metadata. No default synthesizes these identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedMetadataBindings {
    columns: BTreeMap<DerivedMetadataRole, DerivedMetadataColumnBinding>,
}

impl DerivedMetadataBindings {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        columns: impl IntoIterator<Item = DerivedMetadataColumnBinding>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let mut by_role = BTreeMap::new();
        let mut field_ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for column in columns {
            validate_text("derived metadata field", column.field_id.as_str())?;
            validate_text("derived metadata column", &column.physical_name)?;
            if !field_ids.insert(column.field_id.clone()) {
                return Err(ProgrammaticDerivedAnalysisError::DuplicateMetadataField(
                    column.field_id,
                ));
            }
            if !names.insert(Arc::clone(&column.physical_name)) {
                return Err(ProgrammaticDerivedAnalysisError::DuplicateMetadataColumn(
                    column.physical_name,
                ));
            }
            let role = column.role;
            if by_role.insert(role, column).is_some() {
                return Err(ProgrammaticDerivedAnalysisError::DuplicateMetadataRole(
                    role,
                ));
            }
        }
        for role in DerivedMetadataRole::ALL {
            if !by_role.contains_key(&role) {
                return Err(ProgrammaticDerivedAnalysisError::MissingMetadataRole(role));
            }
        }
        Ok(Self { columns: by_role })
    }

    fn column(&self, role: DerivedMetadataRole) -> &DerivedMetadataColumnBinding {
        self.columns
            .get(&role)
            .expect("all closed metadata roles were validated")
    }

    fn ordered(&self) -> impl Iterator<Item = &DerivedMetadataColumnBinding> {
        DerivedMetadataRole::ALL
            .into_iter()
            .map(|role| self.column(role))
    }
}

/// Extra closed roles on the explicit-remainder relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedRemainderMetadataRole {
    Reason,
    EvidenceIdentity,
    Retryability,
}

impl DerivedRemainderMetadataRole {
    pub const ALL: [Self; 3] = [Self::Reason, Self::EvidenceIdentity, Self::Retryability];
}

/// Application binding for one explicit-remainder-only field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedRemainderMetadataColumnBinding {
    role: DerivedRemainderMetadataRole,
    field_id: ProgrammaticFieldId,
    physical_name: Arc<str>,
}

impl DerivedRemainderMetadataColumnBinding {
    #[must_use]
    pub(crate) fn new(
        _authority: &CompiledTransformationAuthority,
        role: DerivedRemainderMetadataRole,
        field_id: ProgrammaticFieldId,
        physical_name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            role,
            field_id,
            physical_name: physical_name.into(),
        }
    }
}

/// Exact relation and transformation contract for persisted/queryable remainders.
#[derive(Clone, Debug)]
pub struct DerivedRemainderRelationBinding {
    authority_identity: [u8; 32],
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    columns: BTreeMap<DerivedRemainderMetadataRole, DerivedRemainderMetadataColumnBinding>,
}

impl DerivedRemainderRelationBinding {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        authority_identity: [u8; 32],
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        columns: impl IntoIterator<Item = DerivedRemainderMetadataColumnBinding>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        if authority_identity == [0; 32] {
            return Err(ProgrammaticDerivedAnalysisError::SentinelApplicationAuthority);
        }
        let mut by_role = BTreeMap::new();
        for column in columns {
            validate_text("remainder metadata field", column.field_id.as_str())?;
            validate_text("remainder metadata column", &column.physical_name)?;
            let role = column.role;
            if by_role.insert(role, column).is_some() {
                return Err(ProgrammaticDerivedAnalysisError::DuplicateRemainderMetadataRole(role));
            }
        }
        for role in DerivedRemainderMetadataRole::ALL {
            if !by_role.contains_key(&role) {
                return Err(ProgrammaticDerivedAnalysisError::MissingRemainderMetadataRole(role));
            }
        }
        Ok(Self {
            authority_identity,
            contract,
            output,
            columns: by_role,
        })
    }

    fn column(&self, role: DerivedRemainderMetadataRole) -> &DerivedRemainderMetadataColumnBinding {
        self.columns
            .get(&role)
            .expect("all closed remainder roles were validated")
    }
}

/// Complete runtime input for one atomic derived-analysis composition.
pub struct ProgrammaticDerivedAnalysisComposition {
    release_authority: CompiledTransformationAuthority,
    families: Arc<[AcceptedDerivedFamily]>,
    dispositions: Vec<DerivedFamilyDisposition>,
    metadata: DerivedMetadataBindings,
    remainder_relation: DerivedRemainderRelationBinding,
    resource_envelope: DerivedAnalysisResourceEnvelope,
}

impl ProgrammaticDerivedAnalysisComposition {
    /// Construct a composition under the bounded workstation envelope.
    ///
    /// Use [`Self::try_new_with_resource_envelope`] when a deployment owns a narrower explicit
    /// aggregate budget. Per-transformation resource contracts are still mandatory and enforced
    /// independently during DataFusion execution.
    pub(crate) fn try_new(
        authority: &CompiledTransformationAuthority,
        families: impl Into<Arc<[AcceptedDerivedFamily]>>,
        dispositions: Vec<DerivedFamilyDisposition>,
        metadata: DerivedMetadataBindings,
        remainder_relation: DerivedRemainderRelationBinding,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        Self::try_new_with_resource_envelope(
            authority,
            families,
            dispositions,
            metadata,
            remainder_relation,
            DerivedAnalysisResourceEnvelope::production(),
        )
    }

    /// Construct a composition under an explicit aggregate planning envelope.
    pub(crate) fn try_new_with_resource_envelope(
        authority: &CompiledTransformationAuthority,
        families: impl Into<Arc<[AcceptedDerivedFamily]>>,
        dispositions: Vec<DerivedFamilyDisposition>,
        metadata: DerivedMetadataBindings,
        remainder_relation: DerivedRemainderRelationBinding,
        resource_envelope: DerivedAnalysisResourceEnvelope,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let families = families.into();
        if families.is_empty() {
            return Err(ProgrammaticDerivedAnalysisError::EmptyAcceptedFamilySet);
        }
        if families.len() > MAX_DERIVED_FAMILIES {
            return Err(ProgrammaticDerivedAnalysisError::FamilyLimitExceeded {
                observed: families.len(),
                maximum: MAX_DERIVED_FAMILIES,
            });
        }
        Ok(Self {
            release_authority: *authority,
            families,
            dispositions,
            metadata,
            remainder_relation,
            resource_envelope,
        })
    }
}

/// Closed common-analysis roles currently exposed by [`CommonAnalysisBindings`].
///
/// The ten semantic fact families share the application-owned `facts` relation. Support relations are
/// separate roles because unknown, completeness, and invalidation evidence are not facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExistingCommonDerivedFamilyRole {
    Dominator,
    PostDominator,
    ControlDependence,
    DataDependence,
    CallGraph,
    SccMembership,
    Reachability,
    CallableEffect,
    CallableResource,
    CallableSummary,
    Unknown,
    Completeness,
    Invalidation,
}

impl ExistingCommonDerivedFamilyRole {
    pub const ALL: [Self; 13] = [
        Self::Dominator,
        Self::PostDominator,
        Self::ControlDependence,
        Self::DataDependence,
        Self::CallGraph,
        Self::SccMembership,
        Self::Reachability,
        Self::CallableEffect,
        Self::CallableResource,
        Self::CallableSummary,
        Self::Unknown,
        Self::Completeness,
        Self::Invalidation,
    ];

    const fn kind(self) -> DerivedFamilyKind {
        match self {
            Self::Unknown | Self::Completeness => DerivedFamilyKind::UnknownEvidence,
            Self::Dominator
            | Self::PostDominator
            | Self::ControlDependence
            | Self::DataDependence
            | Self::CallGraph
            | Self::SccMembership
            | Self::Reachability
            | Self::CallableEffect
            | Self::CallableResource
            | Self::CallableSummary
            | Self::Invalidation => DerivedFamilyKind::Fact,
        }
    }

    fn output_relation<'a>(
        self,
        bindings: &'a CommonAnalysisBindings,
    ) -> &'a crate::relational_program::RelationId {
        match self {
            Self::Dominator
            | Self::PostDominator
            | Self::ControlDependence
            | Self::DataDependence
            | Self::CallGraph
            | Self::SccMembership
            | Self::Reachability
            | Self::CallableEffect
            | Self::CallableResource
            | Self::CallableSummary => &bindings.relations.facts,
            Self::Unknown => &bindings.relations.unknowns,
            Self::Completeness => &bindings.relations.completeness,
            Self::Invalidation => &bindings.relations.invalidation,
        }
    }

    fn semantic_identity<'a>(self, families: &'a CommonAnalysisFamilies) -> Option<&'a str> {
        match self {
            Self::Dominator => Some(&families.dominator),
            Self::PostDominator => Some(&families.post_dominator),
            Self::ControlDependence => Some(&families.control_dependence),
            Self::DataDependence => Some(&families.data_dependence),
            Self::CallGraph => Some(&families.call_graph),
            Self::SccMembership => Some(&families.scc_membership),
            Self::Reachability => Some(&families.reachability),
            Self::CallableEffect => Some(&families.callable_effect),
            Self::CallableResource => Some(&families.callable_resource),
            Self::CallableSummary => Some(&families.callable_summary),
            Self::Unknown | Self::Completeness | Self::Invalidation => None,
        }
        .map(AsRef::as_ref)
    }
}

/// Closed census role over the three existing application-analysis modules.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExistingDerivedFamilyRole {
    Python(PythonDerivedRelation),
    RustMir(RustMirDerivedRelation),
    Common(ExistingCommonDerivedFamilyRole),
}

impl ExistingDerivedFamilyRole {
    /// Return the complete current role census without deriving it from strings.
    #[must_use]
    pub fn all() -> Vec<Self> {
        PythonDerivedRelation::ALL
            .into_iter()
            .map(Self::Python)
            .chain(RustMirDerivedRelation::ALL.into_iter().map(Self::RustMir))
            .chain(
                ExistingCommonDerivedFamilyRole::ALL
                    .into_iter()
                    .map(Self::Common),
            )
            .collect()
    }

    const fn domain(self) -> DerivedAnalysisDomain {
        match self {
            Self::Python(_) => DerivedAnalysisDomain::Python,
            Self::RustMir(_) => DerivedAnalysisDomain::RustMir,
            Self::Common(_) => DerivedAnalysisDomain::Common,
        }
    }

    const fn kind(self) -> DerivedFamilyKind {
        match self {
            Self::Python(PythonDerivedRelation::Unknown)
            | Self::RustMir(RustMirDerivedRelation::Unknown) => DerivedFamilyKind::UnknownEvidence,
            Self::Common(role) => role.kind(),
            Self::Python(_) | Self::RustMir(_) => DerivedFamilyKind::Fact,
        }
    }

    fn output_relation(
        self,
        python: &PythonFlowBindings,
        rust_mir: &RustMirAnalysisBindings,
        common: &CommonAnalysisBindings,
    ) -> ProgrammaticRelationId {
        let relation = match self {
            Self::Python(role) => python.relation_id(role),
            Self::RustMir(role) => rust_mir.relation_id(role),
            Self::Common(role) => role.output_relation(common),
        };
        ProgrammaticRelationId::new(relation.as_str())
    }

    /// Return the exact intended catalog-input vector for this existing family role.
    ///
    /// These vectors are application authority: provider identities come from the closed exact
    /// provider enums, while application-input identities come from the live binding objects.
    /// No role in the current census is source-free. A role whose typed adapter is unavailable
    /// still records the inputs that adapter must consume, so its remainder cannot erase causal
    /// dependency intent.
    #[must_use]
    pub fn dependency_contract(
        self,
        python: &PythonFlowBindings,
        rust_mir: &RustMirAnalysisBindings,
        common: &CommonAnalysisBindings,
    ) -> Arc<[ProgrammaticRelationId]> {
        let syntax =
            |relation: NativeSyntaxRelation| ProgrammaticRelationId::new(relation.as_str());
        let pyrefly =
            |relation: PyreflyRelation| ProgrammaticRelationId::new(relation.relation_id());
        let rustc = |relation: RustcRelation| ProgrammaticRelationId::new(relation.relation_id());
        let python_output = |role: PythonDerivedRelation| {
            ProgrammaticRelationId::new(python.relation_id(role).as_str())
        };
        let rust_output = |role: RustMirDerivedRelation| {
            ProgrammaticRelationId::new(rust_mir.relation_id(role).as_str())
        };
        let common_input = |relation: &crate::relational_program::RelationId| {
            ProgrammaticRelationId::new(relation.as_str())
        };

        let dependencies = match self {
            Self::Python(PythonDerivedRelation::CfgNode | PythonDerivedRelation::CfgEdge) => {
                vec![syntax(NativeSyntaxRelation::RuffAstNode)]
            }
            Self::Python(PythonDerivedRelation::EvaluationOrder) => {
                vec![python_output(PythonDerivedRelation::CfgEdge)]
            }
            Self::Python(PythonDerivedRelation::DefUse) => vec![
                python_output(PythonDerivedRelation::CfgNode),
                syntax(NativeSyntaxRelation::RuffBinding),
                syntax(NativeSyntaxRelation::RuffReference),
                syntax(NativeSyntaxRelation::RuffSemanticEdge),
            ],
            Self::Python(PythonDerivedRelation::ReachingDefinition) => vec![
                python_output(PythonDerivedRelation::CfgNode),
                python_output(PythonDerivedRelation::CfgEdge),
                python_output(PythonDerivedRelation::DefUse),
            ],
            Self::Python(PythonDerivedRelation::Liveness) => vec![
                python_output(PythonDerivedRelation::CfgNode),
                python_output(PythonDerivedRelation::ReachingDefinition),
            ],
            Self::Python(PythonDerivedRelation::ValueFlow) => {
                vec![python_output(PythonDerivedRelation::ReachingDefinition)]
            }
            Self::Python(PythonDerivedRelation::MemoryLocation) => vec![
                syntax(NativeSyntaxRelation::RuffAstNode),
                syntax(NativeSyntaxRelation::RuffBinding),
                syntax(NativeSyntaxRelation::RuffReference),
                pyrefly(PyreflyRelation::LocatedType),
                pyrefly(PyreflyRelation::Member),
            ],
            Self::Python(PythonDerivedRelation::AliasPointsTo) => vec![
                python_output(PythonDerivedRelation::MemoryLocation),
                syntax(NativeSyntaxRelation::RuffSemanticEdge),
                pyrefly(PyreflyRelation::LocatedType),
                pyrefly(PyreflyRelation::Member),
            ],
            Self::Python(PythonDerivedRelation::Effect) => vec![
                syntax(NativeSyntaxRelation::RuffAstNode),
                syntax(NativeSyntaxRelation::RuffReference),
                syntax(NativeSyntaxRelation::RuffSemanticEdge),
                pyrefly(PyreflyRelation::CallTarget),
                pyrefly(PyreflyRelation::Member),
            ],
            Self::Python(PythonDerivedRelation::ResourceLifecycle) => vec![
                python_output(PythonDerivedRelation::Effect),
                python_output(PythonDerivedRelation::CfgEdge),
                syntax(NativeSyntaxRelation::RuffAstNode),
                pyrefly(PyreflyRelation::CallTarget),
                pyrefly(PyreflyRelation::Member),
            ],
            Self::Python(PythonDerivedRelation::AsyncSuspension) => vec![
                syntax(NativeSyntaxRelation::RuffAstNode),
                python_output(PythonDerivedRelation::CfgEdge),
                pyrefly(PyreflyRelation::LocatedType),
            ],
            Self::Python(PythonDerivedRelation::Invalidation) => vec![
                syntax(NativeSyntaxRelation::TreeSitterChangedRange),
                syntax(NativeSyntaxRelation::RuffImport),
                syntax(NativeSyntaxRelation::RuffExport),
                pyrefly(PyreflyRelation::AffectedModule),
            ],
            Self::Python(PythonDerivedRelation::Unknown) => {
                let mut relations = vec![
                    syntax(NativeSyntaxRelation::TreeSitterCoverage),
                    syntax(NativeSyntaxRelation::TreeSitterRemainder),
                    syntax(NativeSyntaxRelation::TreeSitterRecoveryDiagnostic),
                    syntax(NativeSyntaxRelation::RuffCoverage),
                    syntax(NativeSyntaxRelation::RuffRemainder),
                    syntax(NativeSyntaxRelation::RuffParseDiagnostic),
                    syntax(NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence),
                    syntax(NativeSyntaxRelation::RuffUnknownSymbol),
                    pyrefly(PyreflyRelation::Diagnostic),
                    pyrefly(PyreflyRelation::Coverage),
                ];
                relations.extend(
                    PythonDerivedRelation::ALL
                        .into_iter()
                        .filter(|role| *role != PythonDerivedRelation::Unknown)
                        .map(python_output),
                );
                relations
            }
            Self::RustMir(RustMirDerivedRelation::CfgEdge) => {
                vec![rustc(RustcRelation::CfgEdge)]
            }
            Self::RustMir(RustMirDerivedRelation::DefUse) => vec![
                rustc(RustcRelation::MirBlock),
                rustc(RustcRelation::MirPlace),
                rustc(RustcRelation::CfgEdge),
                rustc(RustcRelation::Access),
            ],
            Self::RustMir(
                RustMirDerivedRelation::ReachingDefinition | RustMirDerivedRelation::Liveness,
            ) => vec![
                rustc(RustcRelation::MirBlock),
                rustc(RustcRelation::MirPlace),
                rust_output(RustMirDerivedRelation::CfgEdge),
                rustc(RustcRelation::Access),
            ],
            Self::RustMir(RustMirDerivedRelation::OwnershipState) => vec![
                rustc(RustcRelation::MirLocal),
                rustc(RustcRelation::MirPlace),
                rustc(RustcRelation::Access),
                rustc(RustcRelation::Coverage),
                rustc(RustcRelation::Remainder),
            ],
            Self::RustMir(RustMirDerivedRelation::AliasPointsTo) => vec![
                rustc(RustcRelation::MirPlace),
                rustc(RustcRelation::MirRvalue),
                rustc(RustcRelation::MirStatement),
                rustc(RustcRelation::Access),
            ],
            Self::RustMir(RustMirDerivedRelation::ResourceLifecycle) => vec![
                rustc(RustcRelation::MirPlace),
                rustc(RustcRelation::Access),
                rustc(RustcRelation::MirTerminator),
                rust_output(RustMirDerivedRelation::CfgEdge),
                rustc(RustcRelation::Call),
                rustc(RustcRelation::Instance),
            ],
            Self::RustMir(RustMirDerivedRelation::AsyncLowering) => vec![
                rustc(RustcRelation::MirBody),
                rustc(RustcRelation::MirRvalue),
                rustc(RustcRelation::MirStatement),
                rustc(RustcRelation::MirTerminator),
                rust_output(RustMirDerivedRelation::CfgEdge),
            ],
            Self::RustMir(RustMirDerivedRelation::UnsafeFfi) => vec![
                rustc(RustcRelation::PublicItem),
                rustc(RustcRelation::Type),
                rustc(RustcRelation::MirRvalue),
                rustc(RustcRelation::MirStatement),
                rustc(RustcRelation::MirTerminator),
                rustc(RustcRelation::Call),
                rustc(RustcRelation::Instance),
                rustc(RustcRelation::Access),
            ],
            Self::RustMir(RustMirDerivedRelation::ControlDependenceInput) => vec![
                rustc(RustcRelation::MirBlock),
                rustc(RustcRelation::MirOperand),
                rustc(RustcRelation::MirTerminator),
                rust_output(RustMirDerivedRelation::CfgEdge),
            ],
            Self::RustMir(RustMirDerivedRelation::Unknown) => {
                let mut relations = RustcRelation::ALL
                    .into_iter()
                    .map(rustc)
                    .collect::<Vec<_>>();
                relations.extend(
                    RustMirDerivedRelation::ALL
                        .into_iter()
                        .filter(|role| *role != RustMirDerivedRelation::Unknown)
                        .map(rust_output),
                );
                relations
            }
            Self::Common(
                ExistingCommonDerivedFamilyRole::Dominator
                | ExistingCommonDerivedFamilyRole::PostDominator
                | ExistingCommonDerivedFamilyRole::ControlDependence,
            ) => vec![
                common_input(&common.relations.cfg_nodes),
                common_input(&common.relations.cfg_edges),
            ],
            Self::Common(ExistingCommonDerivedFamilyRole::DataDependence) => {
                vec![common_input(&common.relations.def_use_reaching)]
            }
            Self::Common(ExistingCommonDerivedFamilyRole::CallGraph) => {
                vec![pyrefly(PyreflyRelation::CallTarget)]
            }
            Self::Common(
                ExistingCommonDerivedFamilyRole::SccMembership
                | ExistingCommonDerivedFamilyRole::Reachability,
            ) => vec![common_input(&common.relations.call_targets)],
            Self::Common(
                ExistingCommonDerivedFamilyRole::CallableEffect
                | ExistingCommonDerivedFamilyRole::CallableResource
                | ExistingCommonDerivedFamilyRole::CallableSummary,
            ) => vec![
                common_input(&common.relations.call_targets),
                common_input(&common.relations.local_semantics),
            ],
            Self::Common(ExistingCommonDerivedFamilyRole::Unknown) => vec![
                common_input(&common.relations.cfg_nodes),
                common_input(&common.relations.cfg_edges),
                common_input(&common.relations.def_use_reaching),
                common_input(&common.relations.call_targets),
                common_input(&common.relations.local_semantics),
                syntax(NativeSyntaxRelation::TreeSitterCoverage),
                syntax(NativeSyntaxRelation::TreeSitterRemainder),
                syntax(NativeSyntaxRelation::RuffCoverage),
                syntax(NativeSyntaxRelation::RuffRemainder),
                pyrefly(PyreflyRelation::Diagnostic),
                pyrefly(PyreflyRelation::Coverage),
                rustc(RustcRelation::Diagnostic),
                rustc(RustcRelation::Coverage),
                rustc(RustcRelation::Remainder),
            ],
            Self::Common(ExistingCommonDerivedFamilyRole::Completeness) => vec![
                common_input(&common.relations.cfg_nodes),
                common_input(&common.relations.cfg_edges),
                common_input(&common.relations.def_use_reaching),
                common_input(&common.relations.call_targets),
                common_input(&common.relations.local_semantics),
                syntax(NativeSyntaxRelation::TreeSitterCoverage),
                syntax(NativeSyntaxRelation::TreeSitterRemainder),
                syntax(NativeSyntaxRelation::RuffCoverage),
                syntax(NativeSyntaxRelation::RuffRemainder),
                pyrefly(PyreflyRelation::Coverage),
                rustc(RustcRelation::Coverage),
                rustc(RustcRelation::Remainder),
            ],
            Self::Common(ExistingCommonDerivedFamilyRole::Invalidation) => vec![
                common_input(&common.relations.call_targets),
                python_output(PythonDerivedRelation::Invalidation),
                syntax(NativeSyntaxRelation::TreeSitterChangedRange),
                pyrefly(PyreflyRelation::AffectedModule),
                rustc(RustcRelation::Compilation),
            ],
        };
        dependencies.into()
    }
}

/// One self-observed exact dependency vector in the closed existing-family census.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExistingDerivedDependencyObservation {
    pub role: ExistingDerivedFamilyRole,
    pub dependencies: Arc<[ProgrammaticRelationId]>,
}

/// One explicit declaration in the current production-family census.
///
/// Algorithms, dependencies, authorities, and evidence are compiled release declarations. This
/// type has no public constructor and never derives authority from runtime or legacy data.
#[derive(Clone, Debug)]
pub struct ExistingDerivedFamilyDeclaration {
    role: ExistingDerivedFamilyRole,
    family_id: DerivedFamilyId,
    algorithm: DerivedAlgorithmContract,
    precision: DerivedPrecisionPolicy,
    dependencies: Arc<[ProgrammaticRelationId]>,
    disposition: DerivedFamilyDisposition,
}

impl ExistingDerivedFamilyDeclaration {
    #[must_use]
    pub(crate) fn producer(
        _authority: &CompiledTransformationAuthority,
        role: ExistingDerivedFamilyRole,
        family_id: DerivedFamilyId,
        algorithm: DerivedAlgorithmContract,
        precision: DerivedPrecisionPolicy,
        dependencies: impl Into<Arc<[ProgrammaticRelationId]>>,
        producer: AcceptedDerivedProducer,
    ) -> Self {
        Self {
            role,
            family_id,
            algorithm,
            precision,
            dependencies: dependencies.into(),
            disposition: DerivedFamilyDisposition::Producer(producer),
        }
    }

    pub(crate) fn adapter_unavailable(
        authority: &CompiledTransformationAuthority,
        role: ExistingDerivedFamilyRole,
        family_id: DerivedFamilyId,
        algorithm: DerivedAlgorithmContract,
        precision: DerivedPrecisionPolicy,
        dependencies: impl Into<Arc<[ProgrammaticRelationId]>>,
        evidence_identity: [u8; 32],
        retryability: DerivedRemainderRetryability,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let disposition = DerivedFamilyDisposition::Remainder(ExplicitDerivedRemainder::try_new(
            authority,
            family_id.clone(),
            algorithm.clone(),
            DerivedRemainderReason::TypedTransformationAdapterUnavailable,
            evidence_identity,
            retryability,
        )?);
        Ok(Self {
            role,
            family_id,
            algorithm,
            precision,
            dependencies: dependencies.into(),
            disposition,
        })
    }
}

/// Exact current census and its producer/remainder closure.
pub struct ExistingDerivedAnalysisCensus {
    families: Arc<[AcceptedDerivedFamily]>,
    dispositions: Vec<DerivedFamilyDisposition>,
    observation: ExistingDerivedAnalysisCensusObservation,
}

/// Typed observation proving which current-module roles execute and which remain unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExistingDerivedAnalysisCensusObservation {
    pub accepted_roles: Arc<[ExistingDerivedFamilyRole]>,
    pub programmatic_producer_roles: Arc<[ExistingDerivedFamilyRole]>,
    pub explicit_remainder_roles: Arc<[ExistingDerivedFamilyRole]>,
    pub dependency_contracts: Arc<[ExistingDerivedDependencyObservation]>,
    pub common_semantic_identities: Arc<[Arc<str>]>,
}

impl ExistingDerivedAnalysisCensus {
    /// Validate the exact role census against the real module binding types.
    pub(crate) fn try_new(
        authority: &CompiledTransformationAuthority,
        python: &PythonFlowBindings,
        rust_mir: &RustMirAnalysisBindings,
        common: &CommonAnalysisBindings,
        declarations: Vec<ExistingDerivedFamilyDeclaration>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        python.fields.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        if python.cfg_authority != ProviderAuthorityClass::PythonCfg
            || python.dataflow_authority != ProviderAuthorityClass::PythonDataflow
            || python.alias_authority != ProviderAuthorityClass::PythonAlias
            || python.effect_authority != ProviderAuthorityClass::PythonEffect
            || python.summary_authority != ProviderAuthorityClass::PythonSummary
        {
            return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(
                "Python derived bindings do not retain application authority".to_owned(),
            ));
        }
        RustMirDerivedRelation::CfgEdge
            .schema(rust_mir)
            .map_err(|error| {
                ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
            })?;
        common.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;

        let expected = ExistingDerivedFamilyRole::all();
        let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
        let mut declarations_by_role = BTreeMap::new();
        for declaration in declarations {
            let role = declaration.role;
            if !expected_set.contains(&role) {
                return Err(ProgrammaticDerivedAnalysisError::UnexpectedExistingCensusRole(role));
            }
            if declarations_by_role.insert(role, declaration).is_some() {
                return Err(ProgrammaticDerivedAnalysisError::DuplicateExistingCensusRole(role));
            }
        }
        for role in &expected {
            if !declarations_by_role.contains_key(role) {
                return Err(ProgrammaticDerivedAnalysisError::MissingExistingCensusRole(
                    *role,
                ));
            }
        }

        let mut families = Vec::with_capacity(expected.len());
        let mut dispositions = Vec::with_capacity(expected.len());
        let mut producers = Vec::new();
        let mut remainders = Vec::new();
        let mut dependency_contracts = Vec::with_capacity(expected.len());
        for role in &expected {
            let declaration = declarations_by_role
                .remove(role)
                .expect("complete closed census was checked");
            if declaration.disposition.family_id() != &declaration.family_id {
                return Err(
                    ProgrammaticDerivedAnalysisError::ExistingCensusDispositionMismatch(*role),
                );
            }
            let expected_dependencies = role.dependency_contract(python, rust_mir, common);
            if expected_dependencies.is_empty() {
                return Err(ProgrammaticDerivedAnalysisError::ExistingCensusSourceFreeRole(*role));
            }
            if declaration.dependencies != expected_dependencies {
                return Err(
                    ProgrammaticDerivedAnalysisError::ExistingCensusDependencyMismatch(*role),
                );
            }
            dependency_contracts.push(ExistingDerivedDependencyObservation {
                role: *role,
                dependencies: Arc::clone(&expected_dependencies),
            });
            let output_relation = role.output_relation(python, rust_mir, common);
            match &declaration.disposition {
                DerivedFamilyDisposition::Producer(producer) => {
                    if producer.algorithm != declaration.algorithm
                        || producer.precision != declaration.precision
                        || producer.transformation.output().relation_id() != &output_relation
                    {
                        return Err(
                            ProgrammaticDerivedAnalysisError::ExistingCensusDispositionMismatch(
                                *role,
                            ),
                        );
                    }
                    producers.push(*role);
                }
                DerivedFamilyDisposition::Remainder(remainder) => {
                    if remainder.algorithm != declaration.algorithm
                        || remainder.reason
                            != DerivedRemainderReason::TypedTransformationAdapterUnavailable
                    {
                        return Err(
                            ProgrammaticDerivedAnalysisError::ExistingCensusDispositionMismatch(
                                *role,
                            ),
                        );
                    }
                    remainders.push(*role);
                }
            }
            families.push(AcceptedDerivedFamily::try_new(
                authority,
                declaration.family_id,
                role.domain(),
                role.kind(),
                declaration.algorithm,
                declaration.precision,
                output_relation,
                declaration.dependencies,
            )?);
            dispositions.push(declaration.disposition);
        }
        let common_semantic_identities = ExistingCommonDerivedFamilyRole::ALL
            .into_iter()
            .filter_map(|role| role.semantic_identity(&common.families))
            .map(Arc::from)
            .collect::<Vec<_>>();
        Ok(Self {
            families: families.into(),
            dispositions,
            observation: ExistingDerivedAnalysisCensusObservation {
                accepted_roles: expected.into(),
                programmatic_producer_roles: producers.into(),
                explicit_remainder_roles: remainders.into(),
                dependency_contracts: dependency_contracts.into(),
                common_semantic_identities: common_semantic_identities.into(),
            },
        })
    }

    #[must_use]
    pub const fn observation(&self) -> &ExistingDerivedAnalysisCensusObservation {
        &self.observation
    }

    pub(crate) fn into_composition(
        self,
        authority: &CompiledTransformationAuthority,
        metadata: DerivedMetadataBindings,
        remainder_relation: DerivedRemainderRelationBinding,
    ) -> Result<ProgrammaticDerivedAnalysisComposition, ProgrammaticDerivedAnalysisError> {
        ProgrammaticDerivedAnalysisComposition::try_new(
            authority,
            self.families,
            self.dispositions,
            metadata,
            remainder_relation,
        )
    }
}

/// Result of the exact provider-admission plus existing-family-census transaction.
pub struct ExistingProgrammaticDerivedAnalysisOutcome {
    derived: ProgrammaticDerivedAnalysisOutcome,
    census: ExistingDerivedAnalysisCensusObservation,
}

impl ExistingProgrammaticDerivedAnalysisOutcome {
    #[must_use]
    pub const fn derived(&self) -> &ProgrammaticDerivedAnalysisOutcome {
        &self.derived
    }

    #[must_use]
    pub const fn census(&self) -> &ExistingDerivedAnalysisCensusObservation {
        &self.census
    }

    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProgrammaticDerivedAnalysisOutcome,
        ExistingDerivedAnalysisCensusObservation,
    ) {
        (self.derived, self.census)
    }
}

/// Admit the exact four provider lanes and close every role exposed by the three current modules.
pub(crate) fn admit_and_compose_existing_programmatic_derived_analyses(
    authority: &CompiledTransformationAuthority,
    builder: ProgrammaticFabricEpochBuilder,
    runs: ExactProgrammaticProviderRuns<'_>,
    census: ExistingDerivedAnalysisCensus,
    metadata: DerivedMetadataBindings,
    remainder_relation: DerivedRemainderRelationBinding,
) -> Result<ExistingProgrammaticDerivedAnalysisOutcome, ProgrammaticDerivedAnalysisError> {
    let census_observation = census.observation.clone();
    let composition = census.into_composition(authority, metadata, remainder_relation)?;
    let derived =
        admit_and_compose_programmatic_derived_analyses(authority, builder, runs, composition)?;
    Ok(ExistingProgrammaticDerivedAnalysisOutcome {
        derived,
        census: census_observation,
    })
}

const PYTHON_CFG_SOURCE_NODE: &str = "__cf_programmatic_python_cfg_source_node";
const PYTHON_CFG_TARGET_NODE: &str = "__cf_programmatic_python_cfg_target_node";
const PYTHON_CFG_RANK: &str = "__cf_programmatic_python_cfg_rank";
const PYTHON_CFG_SOURCE_ALIAS: &str = "__cf_programmatic_python_cfg_source";
const PYTHON_CFG_TARGET_ALIAS: &str = "__cf_programmatic_python_cfg_target";
const PYTHON_FLOW_BINDING_ALIAS: &str = "__cf_programmatic_python_flow_binding";
const PYTHON_FLOW_REFERENCE_ALIAS: &str = "__cf_programmatic_python_flow_reference";
const PYTHON_FLOW_SEMANTIC_ALIAS: &str = "__cf_programmatic_python_flow_semantic";
const PYTHON_FLOW_RESOLVED_ALIAS: &str = "__cf_programmatic_python_flow_resolved";
const PYTHON_FLOW_CANDIDATE_ALIAS: &str = "__cf_programmatic_python_flow_candidate";
const PYTHON_FLOW_SOURCE_NODE_ALIAS: &str = "__cf_programmatic_python_flow_source_node";
const PYTHON_FLOW_TARGET_NODE_ALIAS: &str = "__cf_programmatic_python_flow_target_node";
const PYTHON_FLOW_WITH_SOURCE_ALIAS: &str = "__cf_programmatic_python_flow_with_source";
const PYTHON_FLOW_WITH_NODES_ALIAS: &str = "__cf_programmatic_python_flow_with_nodes";
const PYTHON_REACHING_SOURCE_NODE_ALIAS: &str = "__cf_programmatic_python_reaching_source_node";
const PYTHON_REACHING_TARGET_NODE_ALIAS: &str = "__cf_programmatic_python_reaching_target_node";
const PYTHON_REACHING_WITH_SOURCE_ALIAS: &str = "__cf_programmatic_python_reaching_with_source";
const PYTHON_REACHING_CANDIDATE_ALIAS: &str = "__cf_programmatic_python_reaching_candidate";
const PYTHON_REACHING_CFG_EDGE_ALIAS: &str = "__cf_programmatic_python_reaching_cfg_edge";
const PYTHON_REACHING_EDGE_SOURCE_ALIAS: &str = "__cf_programmatic_python_reaching_edge_source";
const PYTHON_REACHING_EDGE_TARGET_ALIAS: &str = "__cf_programmatic_python_reaching_edge_target";
const PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS: &str =
    "__cf_programmatic_python_reaching_edge_with_source";
const PYTHON_REACHING_EDGE_POSITION_ALIAS: &str = "__cf_programmatic_python_reaching_edge_position";
const PYTHON_REACHING_PATH_ALIAS: &str = "__cf_programmatic_python_reaching_path";
const PYTHON_REACHING_RANK: &str = "__cf_programmatic_python_reaching_rank";
const PYTHON_LIVENESS_SOURCE_NODE_ALIAS: &str = "__cf_programmatic_python_live_source";
const PYTHON_LIVENESS_TARGET_NODE_ALIAS: &str = "__cf_programmatic_python_live_target";
const PYTHON_LIVENESS_WITH_SOURCE_ALIAS: &str = "__cf_programmatic_python_live_with_source";
const PYTHON_LIVENESS_RANGE_ALIAS: &str = "__cf_programmatic_python_live_range";
const PYTHON_LIVENESS_NODE_ALIAS: &str = "__cf_programmatic_python_live_node";
const PYTHON_LIVENESS_MEMBER_ALIAS: &str = "__cf_programmatic_python_live_member";
const PYTHON_FLOW_BINDING_ID: &str = "__cf_python_binding_id";
const PYTHON_FLOW_BINDING_SCOPE: &str = "__cf_python_binding_scope";
const PYTHON_FLOW_BINDING_NAME: &str = "__cf_python_binding_name";
const PYTHON_FLOW_BINDING_START: &str = "__cf_python_binding_start";
const PYTHON_FLOW_BINDING_END: &str = "__cf_python_binding_end";
const PYTHON_FLOW_REFERENCE_ID: &str = "__cf_python_reference_id";
const PYTHON_FLOW_REFERENCE_START: &str = "__cf_python_reference_start";
const PYTHON_FLOW_REFERENCE_END: &str = "__cf_python_reference_end";
const PYTHON_FLOW_REFERENCE_CLASS: &str = "__cf_python_reference_class";
const PYTHON_FLOW_DEFINITION_NODE: &str = "__cf_python_definition_node";
const PYTHON_FLOW_USE_NODE: &str = "__cf_python_use_node";
const PYTHON_REACHING_DEFINITION_ORDINAL: &str = "__cf_python_definition_ordinal";
const PYTHON_REACHING_USE_ORDINAL: &str = "__cf_python_use_ordinal";
const PYTHON_REACHING_EDGE_SOURCE_ORDINAL: &str = "__cf_python_edge_source_ordinal";
const PYTHON_REACHING_EDGE_TARGET_ORDINAL: &str = "__cf_python_edge_target_ordinal";
const PYTHON_REACHING_PATH_EDGE_COUNT: &str = "__cf_python_path_edge_count";
const PYTHON_LIVENESS_NODE_ORDINAL: &str = "__cf_python_live_node_ordinal";
const RUST_CONTROL_BLOCK_ALIAS: &str = "__cf_programmatic_rust_control_block";
const RUST_CONTROL_OPERAND_ALIAS: &str = "__cf_programmatic_rust_control_operand";
const RUST_CONTROL_TERMINATOR_ALIAS: &str = "__cf_programmatic_rust_control_terminator";
const RUST_CONTROL_EDGE_ALIAS: &str = "__cf_programmatic_rust_control_edge";
const RUST_CONTROL_CONTROLLER_ALIAS: &str = "__cf_programmatic_rust_control_controller";
const RUST_CONTROL_EDGE_COUNT: &str = "__cf_programmatic_rust_control_edge_count";
const RUST_STRUCTURAL_PLACE_ALIAS: &str = "__cf_programmatic_rust_structural_place";
const RUST_STRUCTURAL_LOCATION_ALIAS: &str = "__cf_programmatic_rust_structural_location";
const RUST_STRUCTURAL_ACCESS_ALIAS: &str = "__cf_programmatic_rust_structural_access";
const RUST_STRUCTURAL_LOCAL_ALIAS: &str = "__cf_programmatic_rust_structural_local";
const RUST_STRUCTURAL_RVALUE_ALIAS: &str = "__cf_programmatic_rust_structural_rvalue";
const RUST_STRUCTURAL_STATEMENT_ALIAS: &str = "__cf_programmatic_rust_structural_statement";
const RUST_STRUCTURAL_TERMINATOR_ALIAS: &str = "__cf_programmatic_rust_structural_terminator";
const RUST_STRUCTURAL_CALL_ALIAS: &str = "__cf_programmatic_rust_structural_call";
const RUST_STRUCTURAL_INSTANCE_ALIAS: &str = "__cf_programmatic_rust_structural_instance";
const RUST_STRUCTURAL_BODY_ALIAS: &str = "__cf_programmatic_rust_structural_body";
const RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS: &str =
    "__cf_programmatic_rust_structural_source_location";
const RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS: &str =
    "__cf_programmatic_rust_structural_destination_location";
const RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS: &str =
    "__cf_programmatic_rust_structural_destination_access";
const RUST_PROJECTION_COMPONENT: &str = "__cf_programmatic_rust_projection_component";
const RUST_STRUCTURAL_COVERAGE_ALIAS: &str = "__cf_programmatic_rust_structural_coverage";
const RUST_STRUCTURAL_REMAINDER_ALIAS: &str = "__cf_programmatic_rust_structural_remainder";
const RUST_STRUCTURAL_CFG_ALIAS: &str = "__cf_programmatic_rust_structural_cfg";
const RUST_STRUCTURAL_PUBLIC_ALIAS: &str = "__cf_programmatic_rust_structural_public";
const RUST_STRUCTURAL_TYPE_ALIAS: &str = "__cf_programmatic_rust_structural_type";
const RUST_STRUCTURAL_UNSAFE_ACCESS_ALIAS: &str = "__cf_programmatic_rust_structural_unsafe_access";
const RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS: &str = "__cf_programmatic_rust_structural_unsafe_rows";

/// Explicit row-level semantic strings for programmatic Python CFG nodes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticPythonCfgNodeRowContract {
    algorithm_release: Arc<str>,
    precision_release: Arc<str>,
    authority: Arc<str>,
}

impl ProgrammaticPythonCfgNodeRowContract {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        algorithm_release: impl Into<Arc<str>>,
        precision_release: impl Into<Arc<str>>,
        authority: impl Into<Arc<str>>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let row = Self {
            algorithm_release: algorithm_release.into(),
            precision_release: precision_release.into(),
            authority: authority.into(),
        };
        for (kind, value) in [
            ("Python CFG-node algorithm release", &row.algorithm_release),
            ("Python CFG-node precision release", &row.precision_release),
            ("Python CFG-node authority", &row.authority),
        ] {
            validate_text(kind, value)?;
        }
        Ok(row)
    }
}

/// Native catalog-input normalization of accepted Ruff typed-AST nodes.
///
/// The plan keeps projection, filtering, distinctness, and ordering visible to DataFusion. The
/// sole UDF is the immutable application-owned canonical node identity that built-ins cannot
/// express without changing the identity algorithm.
pub struct ProgrammaticPythonCfgNodeTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependency: Arc<[ProgrammaticRelationId]>,
    bindings: PythonFlowBindings,
    fabric_epoch_id: FabricEpochId,
    row: ProgrammaticPythonCfgNodeRowContract,
    node_identity: Arc<ScalarUDF>,
}

impl ProgrammaticPythonCfgNodeTransformation {
    pub const OUTPUT_FIELD_COUNT: usize = 18;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &PythonFlowBindings,
        fabric_epoch_id: FabricEpochId,
        row: ProgrammaticPythonCfgNodeRowContract,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        bindings.fields.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        if bindings.cfg_authority != ProviderAuthorityClass::PythonCfg {
            return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(
                "Python CFG-node output is not application-owned".to_owned(),
            ));
        }
        let expected = ProgrammaticRelationId::new(
            bindings
                .relation_id(PythonDerivedRelation::CfgNode)
                .as_str(),
        );
        validate_existing_output(
            ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgNode),
            &output,
            &expected,
            Self::OUTPUT_FIELD_COUNT,
        )?;
        Ok(Self {
            contract,
            output,
            dependency: Arc::from([ProgrammaticRelationId::new(
                NativeSyntaxRelation::RuffAstNode.as_str(),
            )]),
            bindings: bindings.clone(),
            fabric_epoch_id,
            row,
            node_identity: python_cfg_node_identity_udf(),
        })
    }
}

impl ProgrammaticTransformation for ProgrammaticPythonCfgNodeTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependency
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let input = inputs.plan(&self.dependency[0])?;
        let node_id = self.node_identity.call(vec![
            Expr::Literal(
                ScalarValue::FixedSizeBinary(16, Some(self.fabric_epoch_id.as_bytes().to_vec())),
                None,
            ),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col("start_byte"),
            col("end_byte"),
            col("raw_kind"),
        ]);
        Ok(LogicalPlanBuilder::from(input)
            .filter(col("evaluation_ordinal").is_not_null())?
            .project([
                Expr::Literal(
                    ScalarValue::FixedSizeBinary(
                        16,
                        Some(self.fabric_epoch_id.as_bytes().to_vec()),
                    ),
                    None,
                )
                .alias(fields.fabric_epoch_id.as_ref()),
                col("content_digest").alias(fields.source_pin.as_ref()),
                col("analysis_context_id").alias(fields.analysis_context_id.as_ref()),
                col("source_generation").alias(fields.source_generation.as_ref()),
                col("file_id").alias(fields.owner_id.as_ref()),
                col("provider_run_id").alias(fields.ruff_provider_run_id.as_ref()),
                col("provider_release").alias(fields.ruff_provider_release.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_run_id.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                utf8_literal(&Arc::from("complete")).alias(fields.analysis_completeness.as_ref()),
                node_id.alias(fields.node_id.as_ref()),
                col("evaluation_ordinal").alias(fields.node_ordinal.as_ref()),
                col("raw_kind").alias(fields.node_kind.as_ref()),
                col("start_byte").alias(fields.start_byte.as_ref()),
                col("end_byte").alias(fields.end_byte.as_ref()),
            ])?
            .distinct()?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.node_ordinal.as_ref()).sort(true, false),
                col(fields.start_byte.as_ref()).sort(true, false),
            ])?
            .build()?)
    }
}

/// Explicit row-level semantic strings for the programmatic Python CFG release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticPythonCfgEdgeRowContract {
    algorithm_release: Arc<str>,
    precision_release: Arc<str>,
    authority: Arc<str>,
    sequential_edge_kind: Arc<str>,
}

impl ProgrammaticPythonCfgEdgeRowContract {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        algorithm_release: impl Into<Arc<str>>,
        precision_release: impl Into<Arc<str>>,
        authority: impl Into<Arc<str>>,
        sequential_edge_kind: impl Into<Arc<str>>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let row = Self {
            algorithm_release: algorithm_release.into(),
            precision_release: precision_release.into(),
            authority: authority.into(),
            sequential_edge_kind: sequential_edge_kind.into(),
        };
        for (kind, value) in [
            ("Python CFG algorithm release", &row.algorithm_release),
            ("Python CFG precision release", &row.precision_release),
            ("Python CFG authority", &row.authority),
            ("Python CFG sequential edge", &row.sequential_edge_kind),
        ] {
            validate_text(kind, value)?;
        }
        Ok(row)
    }
}

/// Native DataFusion rewrite of the existing Python sequential-CFG slice.
///
/// It consumes accepted Ruff typed-AST rows from the live candidate catalog, constructs canonical
/// application node/edge identities in immutable UDFs, and uses DataFusion's native window,
/// filter, projection, distinct, and sort operators. No batch or private `MemTable` is captured.
pub struct ProgrammaticPythonCfgEdgeTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependency: Arc<[ProgrammaticRelationId]>,
    bindings: PythonFlowBindings,
    fabric_epoch_id: FabricEpochId,
    row: ProgrammaticPythonCfgEdgeRowContract,
    node_identity: Arc<ScalarUDF>,
    edge_identity: Arc<ScalarUDF>,
}

impl ProgrammaticPythonCfgEdgeTransformation {
    pub const OUTPUT_FIELD_COUNT: usize = 17;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &PythonFlowBindings,
        fabric_epoch_id: FabricEpochId,
        row: ProgrammaticPythonCfgEdgeRowContract,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        bindings.fields.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        if bindings.cfg_authority != ProviderAuthorityClass::PythonCfg {
            return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(
                "Python CFG output is not application-owned".to_owned(),
            ));
        }
        let expected = ProgrammaticRelationId::new(
            bindings
                .relation_id(PythonDerivedRelation::CfgEdge)
                .as_str(),
        );
        validate_existing_output(
            ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgEdge),
            &output,
            &expected,
            Self::OUTPUT_FIELD_COUNT,
        )?;
        Ok(Self {
            contract,
            output,
            dependency: Arc::from([ProgrammaticRelationId::new(
                NativeSyntaxRelation::RuffAstNode.as_str(),
            )]),
            bindings: bindings.clone(),
            fabric_epoch_id,
            row,
            node_identity: python_cfg_node_identity_udf(),
            edge_identity: python_cfg_edge_identity_udf(),
        })
    }
}

impl ProgrammaticTransformation for ProgrammaticPythonCfgEdgeTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependency
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let input = inputs.plan(&self.dependency[0])?;
        let source_node = self.node_identity.call(vec![
            Expr::Literal(
                ScalarValue::FixedSizeBinary(16, Some(self.fabric_epoch_id.as_bytes().to_vec())),
                None,
            ),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col("start_byte"),
            col("end_byte"),
            col("raw_kind"),
        ]);
        let staged = LogicalPlanBuilder::from(input)
            .filter(col("evaluation_ordinal").is_not_null())?
            .project([
                col("provider_run_id"),
                col("provider_release"),
                col("analysis_context_id"),
                col("file_id"),
                col("content_digest"),
                col("source_generation"),
                col("source_ordinal"),
                col("evaluation_ordinal"),
                source_node.alias(PYTHON_CFG_SOURCE_NODE),
            ])?
            .build()?;
        let rank = row_number()
            .partition_by(vec![col("file_id")])
            .order_by(vec![
                col("evaluation_ordinal").sort(true, false),
                col("source_ordinal").sort(true, false),
            ])
            .build()?
            .alias(PYTHON_CFG_RANK);
        let ranked = LogicalPlanBuilder::from(staged).window([rank])?.build()?;
        let source = LogicalPlanBuilder::from(ranked.clone())
            .alias(PYTHON_CFG_SOURCE_ALIAS)?
            .build()?;
        let target = LogicalPlanBuilder::from(ranked)
            .alias(PYTHON_CFG_TARGET_ALIAS)?
            .build()?;
        let adjacent = LogicalPlanBuilder::from(source)
            .join_on(
                target,
                JoinType::Inner,
                [
                    qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "file_id")
                        .eq(qualified_programmatic(PYTHON_CFG_TARGET_ALIAS, "file_id")),
                    (qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, PYTHON_CFG_RANK) + lit(1_u64))
                        .eq(qualified_programmatic(
                            PYTHON_CFG_TARGET_ALIAS,
                            PYTHON_CFG_RANK,
                        )),
                ],
            )?
            .project([
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "provider_run_id"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "provider_release"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "analysis_context_id"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "file_id"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "content_digest"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, "source_generation"),
                qualified_programmatic(PYTHON_CFG_SOURCE_ALIAS, PYTHON_CFG_SOURCE_NODE)
                    .alias(PYTHON_CFG_SOURCE_NODE),
                qualified_programmatic(PYTHON_CFG_TARGET_ALIAS, PYTHON_CFG_SOURCE_NODE)
                    .alias(PYTHON_CFG_TARGET_NODE),
            ])?
            .build()?;
        let edge_id = self.edge_identity.call(vec![
            Expr::Literal(
                ScalarValue::FixedSizeBinary(16, Some(self.fabric_epoch_id.as_bytes().to_vec())),
                None,
            ),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col(PYTHON_CFG_SOURCE_NODE),
            col(PYTHON_CFG_TARGET_NODE),
            utf8_literal(&self.row.sequential_edge_kind),
        ]);
        Ok(LogicalPlanBuilder::from(adjacent)
            .project([
                Expr::Literal(
                    ScalarValue::FixedSizeBinary(
                        16,
                        Some(self.fabric_epoch_id.as_bytes().to_vec()),
                    ),
                    None,
                )
                .alias("fabric_epoch_id"),
                col("content_digest").alias(fields.source_pin.as_ref()),
                col("analysis_context_id").alias(fields.analysis_context_id.as_ref()),
                col("source_generation").alias(fields.source_generation.as_ref()),
                col("file_id").alias(fields.owner_id.as_ref()),
                col("provider_run_id").alias(fields.ruff_provider_run_id.as_ref()),
                col("provider_release").alias(fields.ruff_provider_release.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_run_id.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                utf8_literal(&Arc::from("complete")).alias(fields.analysis_completeness.as_ref()),
                edge_id.alias(fields.edge_id.as_ref()),
                col(PYTHON_CFG_SOURCE_NODE).alias(fields.source_node_id.as_ref()),
                col(PYTHON_CFG_TARGET_NODE).alias(fields.target_node_id.as_ref()),
                utf8_literal(&self.row.sequential_edge_kind).alias(fields.edge_kind.as_ref()),
            ])?
            .distinct()?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.source_node_id.as_ref()).sort(true, false),
                col(fields.target_node_id.as_ref()).sort(true, false),
            ])?
            .build()?)
    }
}

/// Row-level semantic contract shared by the owner-local Python dataflow chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticPythonDataflowRowContract {
    algorithm_release: Arc<str>,
    precision_release: Arc<str>,
    authority: Arc<str>,
}

impl ProgrammaticPythonDataflowRowContract {
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        algorithm_release: impl Into<Arc<str>>,
        precision_release: impl Into<Arc<str>>,
        authority: impl Into<Arc<str>>,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let row = Self {
            algorithm_release: algorithm_release.into(),
            precision_release: precision_release.into(),
            authority: authority.into(),
        };
        for (kind, value) in [
            ("Python dataflow algorithm release", &row.algorithm_release),
            ("Python dataflow precision release", &row.precision_release),
            ("Python dataflow authority", &row.authority),
        ] {
            validate_text(kind, value)?;
        }
        Ok(row)
    }
}

/// Native catalog-input producer for one role in the owner-local Python dataflow chain.
///
/// Every role consumes only live catalog relations. Def-use candidates preserve distinct binding
/// entities and reference occurrences; reaching definitions then validate a complete sequential
/// CFG path and select the latest candidate with a native window. Liveness and value flow consume
/// that selected application-owned relation rather than relabeling provider rows.
pub struct ProgrammaticPythonDataflowTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependency: Arc<[ProgrammaticRelationId]>,
    bindings: PythonFlowBindings,
    fabric_epoch_id: FabricEpochId,
    role: PythonDerivedRelation,
    row: ProgrammaticPythonDataflowRowContract,
    event_identity: Arc<ScalarUDF>,
    location_identity: Arc<ScalarUDF>,
    relation_identity: Arc<ScalarUDF>,
}

impl ProgrammaticPythonDataflowTransformation {
    pub const EVALUATION_OUTPUT_FIELD_COUNT: usize = 19;
    pub const FLOW_LINK_OUTPUT_FIELD_COUNT: usize = 20;
    pub const LIVENESS_OUTPUT_FIELD_COUNT: usize = 18;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &PythonFlowBindings,
        fabric_epoch_id: FabricEpochId,
        role: PythonDerivedRelation,
        row: ProgrammaticPythonDataflowRowContract,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        bindings.fields.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        if bindings.dataflow_authority != ProviderAuthorityClass::PythonDataflow {
            return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(
                "Python dataflow output is not application-owned".to_owned(),
            ));
        }
        let output_field_count = match role {
            PythonDerivedRelation::EvaluationOrder => Self::EVALUATION_OUTPUT_FIELD_COUNT,
            PythonDerivedRelation::DefUse
            | PythonDerivedRelation::ReachingDefinition
            | PythonDerivedRelation::ValueFlow => Self::FLOW_LINK_OUTPUT_FIELD_COUNT,
            PythonDerivedRelation::Liveness => Self::LIVENESS_OUTPUT_FIELD_COUNT,
            _ => {
                return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(format!(
                    "Python role {role:?} is not an owner-local dataflow adapter"
                )));
            }
        };
        let expected = ProgrammaticRelationId::new(bindings.relation_id(role).as_str());
        validate_existing_output(
            ExistingDerivedFamilyRole::Python(role),
            &output,
            &expected,
            output_field_count,
        )?;
        let python_output = |dependency_role: PythonDerivedRelation| {
            ProgrammaticRelationId::new(bindings.relation_id(dependency_role).as_str())
        };
        let dependency: Arc<[ProgrammaticRelationId]> = match role {
            PythonDerivedRelation::EvaluationOrder => {
                Arc::from([python_output(PythonDerivedRelation::CfgEdge)])
            }
            PythonDerivedRelation::DefUse => Arc::from([
                python_output(PythonDerivedRelation::CfgNode),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffBinding.as_str()),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffReference.as_str()),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffSemanticEdge.as_str()),
            ]),
            PythonDerivedRelation::ReachingDefinition => Arc::from([
                python_output(PythonDerivedRelation::CfgNode),
                python_output(PythonDerivedRelation::CfgEdge),
                python_output(PythonDerivedRelation::DefUse),
            ]),
            PythonDerivedRelation::Liveness => Arc::from([
                python_output(PythonDerivedRelation::CfgNode),
                python_output(PythonDerivedRelation::ReachingDefinition),
            ]),
            PythonDerivedRelation::ValueFlow => {
                Arc::from([python_output(PythonDerivedRelation::ReachingDefinition)])
            }
            _ => unreachable!("role was validated above"),
        };
        Ok(Self {
            contract,
            output,
            dependency,
            bindings: bindings.clone(),
            fabric_epoch_id,
            role,
            row,
            event_identity: python_flow_event_identity_udf(),
            location_identity: python_flow_location_identity_udf(),
            relation_identity: python_flow_relation_identity_udf(),
        })
    }

    fn evaluation_order_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let input = inputs.plan(&self.dependency[0])?;
        let relation_kind = Arc::<str>::from("NODE_EVALUATES_BEFORE");
        let edge_id = self.relation_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col(fields.source_pin.as_ref()),
            col(fields.analysis_context_id.as_ref()),
            col(fields.owner_id.as_ref()),
            col(fields.source_node_id.as_ref()),
            col(fields.target_node_id.as_ref()),
            col(fields.edge_id.as_ref()),
            utf8_literal(&relation_kind),
        ]);
        Ok(LogicalPlanBuilder::from(input)
            .project([
                col(fields.fabric_epoch_id.as_ref()),
                col(fields.source_pin.as_ref()),
                col(fields.analysis_context_id.as_ref()),
                col(fields.source_generation.as_ref()),
                col(fields.owner_id.as_ref()),
                col(fields.ruff_provider_run_id.as_ref()),
                col(fields.ruff_provider_release.as_ref()),
                col(fields.pyrefly_provider_run_id.as_ref()),
                col(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                col(fields.analysis_completeness.as_ref()),
                edge_id.alias(fields.edge_id.as_ref()),
                col(fields.source_node_id.as_ref()).alias(fields.predecessor_id.as_ref()),
                col(fields.target_node_id.as_ref()).alias(fields.successor_id.as_ref()),
                col(fields.source_node_id.as_ref()),
                col(fields.target_node_id.as_ref()),
                utf8_literal(&relation_kind).alias(fields.relation_kind.as_ref()),
            ])?
            .distinct()?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.predecessor_id.as_ref()).sort(true, false),
                col(fields.successor_id.as_ref()).sort(true, false),
            ])?
            .build()?)
    }

    fn def_use_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let cfg_nodes = inputs.plan(&self.dependency[0])?;
        let bindings = LogicalPlanBuilder::from(inputs.plan(&self.dependency[1])?)
            .alias(PYTHON_FLOW_BINDING_ALIAS)?
            .build()?;
        let reference_filter = col("reference_class")
            .eq(lit("read"))
            .or(col("reference_class").eq(lit("read-write")))
            .or(col("reference_class").eq(lit("call-reference")))
            .or(col("reference_class").eq(lit("type-reference")));
        let references = LogicalPlanBuilder::from(inputs.plan(&self.dependency[2])?)
            .filter(col("resolution").eq(lit("resolved")).and(reference_filter))?
            .alias(PYTHON_FLOW_REFERENCE_ALIAS)?
            .build()?;
        let semantic = LogicalPlanBuilder::from(inputs.plan(&self.dependency[3])?)
            .filter(col("edge_kind").eq(lit("refers-to")))?
            .alias(PYTHON_FLOW_SEMANTIC_ALIAS)?
            .build()?;

        let resolved = LogicalPlanBuilder::from(references)
            .join_on(
                semantic,
                JoinType::Inner,
                python_provider_join(PYTHON_FLOW_REFERENCE_ALIAS, PYTHON_FLOW_SEMANTIC_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "reference_id").eq(
                            qualified_programmatic(PYTHON_FLOW_SEMANTIC_ALIAS, "subject_id"),
                        ),
                        qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "target_id").eq(
                            qualified_programmatic(PYTHON_FLOW_SEMANTIC_ALIAS, "object_id"),
                        ),
                    ]),
            )?
            .project([
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "provider_run_id"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "provider_release"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "analysis_context_id"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "file_id"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "content_digest"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "source_generation"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "scope_id"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "name"),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "reference_id")
                    .alias(PYTHON_FLOW_REFERENCE_ID),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "start_byte")
                    .alias(PYTHON_FLOW_REFERENCE_START),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "end_byte")
                    .alias(PYTHON_FLOW_REFERENCE_END),
                qualified_programmatic(PYTHON_FLOW_REFERENCE_ALIAS, "reference_class")
                    .alias(PYTHON_FLOW_REFERENCE_CLASS),
            ])?
            .alias(PYTHON_FLOW_RESOLVED_ALIAS)?
            .build()?;

        let candidates = LogicalPlanBuilder::from(bindings)
            .join_on(
                resolved,
                JoinType::Inner,
                python_provider_join(PYTHON_FLOW_BINDING_ALIAS, PYTHON_FLOW_RESOLVED_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "scope_id").eq(
                            qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "scope_id"),
                        ),
                        qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "name")
                            .eq(qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "name")),
                        qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "start_byte").lt_eq(
                            qualified_programmatic(
                                PYTHON_FLOW_RESOLVED_ALIAS,
                                PYTHON_FLOW_REFERENCE_START,
                            ),
                        ),
                    ]),
            )?
            .project([
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "provider_run_id"),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "provider_release"),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "analysis_context_id"),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "file_id"),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "content_digest"),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, "source_generation"),
                qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "binding_id")
                    .alias(PYTHON_FLOW_BINDING_ID),
                qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "scope_id")
                    .alias(PYTHON_FLOW_BINDING_SCOPE),
                qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "name")
                    .alias(PYTHON_FLOW_BINDING_NAME),
                qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "start_byte")
                    .alias(PYTHON_FLOW_BINDING_START),
                qualified_programmatic(PYTHON_FLOW_BINDING_ALIAS, "end_byte")
                    .alias(PYTHON_FLOW_BINDING_END),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, PYTHON_FLOW_REFERENCE_ID),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, PYTHON_FLOW_REFERENCE_START),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, PYTHON_FLOW_REFERENCE_END),
                qualified_programmatic(PYTHON_FLOW_RESOLVED_ALIAS, PYTHON_FLOW_REFERENCE_CLASS),
            ])?
            .alias(PYTHON_FLOW_CANDIDATE_ALIAS)?
            .build()?;

        let source_nodes = LogicalPlanBuilder::from(cfg_nodes.clone())
            .alias(PYTHON_FLOW_SOURCE_NODE_ALIAS)?
            .build()?;
        let with_source = LogicalPlanBuilder::from(candidates)
            .join_on(
                source_nodes,
                JoinType::Inner,
                python_raw_to_derived_join(
                    PYTHON_FLOW_CANDIDATE_ALIAS,
                    PYTHON_FLOW_SOURCE_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(PYTHON_FLOW_CANDIDATE_ALIAS, PYTHON_FLOW_BINDING_START)
                        .eq(qualified_programmatic(
                            PYTHON_FLOW_SOURCE_NODE_ALIAS,
                            fields.start_byte.as_ref(),
                        )),
                    qualified_programmatic(PYTHON_FLOW_CANDIDATE_ALIAS, PYTHON_FLOW_BINDING_END)
                        .eq(qualified_programmatic(
                            PYTHON_FLOW_SOURCE_NODE_ALIAS,
                            fields.end_byte.as_ref(),
                        )),
                ]),
            )?
            .project(
                python_def_use_candidate_projection(PYTHON_FLOW_CANDIDATE_ALIAS)
                    .into_iter()
                    .chain([qualified_programmatic(
                        PYTHON_FLOW_SOURCE_NODE_ALIAS,
                        fields.node_id.as_ref(),
                    )
                    .alias(PYTHON_FLOW_DEFINITION_NODE)]),
            )?
            .alias(PYTHON_FLOW_WITH_SOURCE_ALIAS)?
            .build()?;
        let target_nodes = LogicalPlanBuilder::from(cfg_nodes)
            .alias(PYTHON_FLOW_TARGET_NODE_ALIAS)?
            .build()?;
        let with_nodes = LogicalPlanBuilder::from(with_source)
            .join_on(
                target_nodes,
                JoinType::Inner,
                python_raw_to_derived_join(
                    PYTHON_FLOW_WITH_SOURCE_ALIAS,
                    PYTHON_FLOW_TARGET_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(
                        PYTHON_FLOW_WITH_SOURCE_ALIAS,
                        PYTHON_FLOW_REFERENCE_START,
                    )
                    .eq(qualified_programmatic(
                        PYTHON_FLOW_TARGET_NODE_ALIAS,
                        fields.start_byte.as_ref(),
                    )),
                    qualified_programmatic(
                        PYTHON_FLOW_WITH_SOURCE_ALIAS,
                        PYTHON_FLOW_REFERENCE_END,
                    )
                    .eq(qualified_programmatic(
                        PYTHON_FLOW_TARGET_NODE_ALIAS,
                        fields.end_byte.as_ref(),
                    )),
                ]),
            )?
            .project(
                python_def_use_candidate_projection(PYTHON_FLOW_WITH_SOURCE_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(
                            PYTHON_FLOW_WITH_SOURCE_ALIAS,
                            PYTHON_FLOW_DEFINITION_NODE,
                        ),
                        qualified_programmatic(
                            PYTHON_FLOW_TARGET_NODE_ALIAS,
                            fields.node_id.as_ref(),
                        )
                        .alias(PYTHON_FLOW_USE_NODE),
                    ]),
            )?
            .alias(PYTHON_FLOW_WITH_NODES_ALIAS)?
            .build()?;

        let definition_event = self.event_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col(PYTHON_FLOW_BINDING_ID),
            col(PYTHON_FLOW_BINDING_START),
            col(PYTHON_FLOW_BINDING_END),
            utf8_literal(&self.bindings.values.definition_event),
        ]);
        let use_event = self.event_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col(PYTHON_FLOW_REFERENCE_ID),
            col(PYTHON_FLOW_REFERENCE_START),
            col(PYTHON_FLOW_REFERENCE_END),
            utf8_literal(&self.bindings.values.use_event),
        ]);
        let location = self.location_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            col(PYTHON_FLOW_BINDING_SCOPE),
            col(PYTHON_FLOW_BINDING_NAME),
        ]);
        let edge_id = self.relation_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col("content_digest"),
            col("analysis_context_id"),
            col("file_id"),
            definition_event.clone(),
            use_event.clone(),
            location.clone(),
            utf8_literal(&self.bindings.values.def_use),
        ]);
        Ok(LogicalPlanBuilder::from(with_nodes)
            .project([
                fabric_epoch_literal(self.fabric_epoch_id).alias(fields.fabric_epoch_id.as_ref()),
                col("content_digest").alias(fields.source_pin.as_ref()),
                col("analysis_context_id").alias(fields.analysis_context_id.as_ref()),
                col("source_generation").alias(fields.source_generation.as_ref()),
                col("file_id").alias(fields.owner_id.as_ref()),
                col("provider_run_id").alias(fields.ruff_provider_run_id.as_ref()),
                col("provider_release").alias(fields.ruff_provider_release.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_run_id.as_ref()),
                Expr::Literal(ScalarValue::Utf8(None), None)
                    .alias(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                lit("complete").alias(fields.analysis_completeness.as_ref()),
                edge_id.alias(fields.edge_id.as_ref()),
                definition_event.alias(fields.definition_event_id.as_ref()),
                use_event.alias(fields.use_event_id.as_ref()),
                location.alias(fields.location_id.as_ref()),
                col(PYTHON_FLOW_DEFINITION_NODE).alias(fields.source_node_id.as_ref()),
                col(PYTHON_FLOW_USE_NODE).alias(fields.target_node_id.as_ref()),
                utf8_literal(&self.bindings.values.def_use).alias(fields.relation_kind.as_ref()),
            ])?
            .distinct()?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.use_event_id.as_ref()).sort(true, false),
                col(fields.definition_event_id.as_ref()).sort(true, false),
            ])?
            .build()?)
    }
}

impl ProgrammaticPythonDataflowTransformation {
    fn reaching_definition_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let cfg_nodes = inputs.plan(&self.dependency[0])?;
        let cfg_edges = inputs.plan(&self.dependency[1])?;
        let def_use = LogicalPlanBuilder::from(inputs.plan(&self.dependency[2])?)
            .alias(PYTHON_FLOW_CANDIDATE_ALIAS)?
            .build()?;

        let source_nodes = LogicalPlanBuilder::from(cfg_nodes.clone())
            .alias(PYTHON_REACHING_SOURCE_NODE_ALIAS)?
            .build()?;
        let with_source = LogicalPlanBuilder::from(def_use)
            .join_on(
                source_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_FLOW_CANDIDATE_ALIAS,
                    PYTHON_REACHING_SOURCE_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_FLOW_CANDIDATE_ALIAS,
                    fields.source_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_REACHING_SOURCE_NODE_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project(
                python_flow_link_projection(PYTHON_FLOW_CANDIDATE_ALIAS, fields)
                    .into_iter()
                    .chain([qualified_programmatic(
                        PYTHON_REACHING_SOURCE_NODE_ALIAS,
                        fields.node_ordinal.as_ref(),
                    )
                    .alias(PYTHON_REACHING_DEFINITION_ORDINAL)]),
            )?
            .alias(PYTHON_REACHING_WITH_SOURCE_ALIAS)?
            .build()?;
        let target_nodes = LogicalPlanBuilder::from(cfg_nodes.clone())
            .alias(PYTHON_REACHING_TARGET_NODE_ALIAS)?
            .build()?;
        let candidates = LogicalPlanBuilder::from(with_source)
            .join_on(
                target_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_REACHING_WITH_SOURCE_ALIAS,
                    PYTHON_REACHING_TARGET_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_REACHING_WITH_SOURCE_ALIAS,
                    fields.target_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_REACHING_TARGET_NODE_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project(
                python_flow_link_projection(PYTHON_REACHING_WITH_SOURCE_ALIAS, fields)
                    .into_iter()
                    .chain([
                        qualified_programmatic(
                            PYTHON_REACHING_WITH_SOURCE_ALIAS,
                            PYTHON_REACHING_DEFINITION_ORDINAL,
                        ),
                        qualified_programmatic(
                            PYTHON_REACHING_TARGET_NODE_ALIAS,
                            fields.node_ordinal.as_ref(),
                        )
                        .alias(PYTHON_REACHING_USE_ORDINAL),
                    ]),
            )?
            .filter(
                col(PYTHON_REACHING_DEFINITION_ORDINAL).lt_eq(col(PYTHON_REACHING_USE_ORDINAL)),
            )?
            .alias(PYTHON_REACHING_CANDIDATE_ALIAS)?
            .build()?;

        let edge = LogicalPlanBuilder::from(cfg_edges)
            .alias(PYTHON_REACHING_CFG_EDGE_ALIAS)?
            .build()?;
        let edge_source_nodes = LogicalPlanBuilder::from(cfg_nodes.clone())
            .alias(PYTHON_REACHING_EDGE_SOURCE_ALIAS)?
            .build()?;
        let edge_with_source = LogicalPlanBuilder::from(edge)
            .join_on(
                edge_source_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    PYTHON_REACHING_EDGE_SOURCE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    fields.source_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_REACHING_EDGE_SOURCE_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project([
                qualified_programmatic(PYTHON_REACHING_CFG_EDGE_ALIAS, fields.source_pin.as_ref()),
                qualified_programmatic(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    fields.analysis_context_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    fields.source_generation.as_ref(),
                ),
                qualified_programmatic(PYTHON_REACHING_CFG_EDGE_ALIAS, fields.owner_id.as_ref()),
                qualified_programmatic(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    fields.ruff_provider_run_id.as_ref(),
                ),
                qualified_programmatic(PYTHON_REACHING_CFG_EDGE_ALIAS, fields.edge_id.as_ref()),
                qualified_programmatic(
                    PYTHON_REACHING_CFG_EDGE_ALIAS,
                    fields.target_node_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_SOURCE_ALIAS,
                    fields.node_ordinal.as_ref(),
                )
                .alias(PYTHON_REACHING_EDGE_SOURCE_ORDINAL),
            ])?
            .alias(PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS)?
            .build()?;
        let edge_target_nodes = LogicalPlanBuilder::from(cfg_nodes)
            .alias(PYTHON_REACHING_EDGE_TARGET_ALIAS)?
            .build()?;
        let edge_positions = LogicalPlanBuilder::from(edge_with_source)
            .join_on(
                edge_target_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    PYTHON_REACHING_EDGE_TARGET_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.target_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_REACHING_EDGE_TARGET_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project([
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.source_pin.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.analysis_context_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.source_generation.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.owner_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.ruff_provider_run_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    fields.edge_id.as_ref(),
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_WITH_SOURCE_ALIAS,
                    PYTHON_REACHING_EDGE_SOURCE_ORDINAL,
                ),
                qualified_programmatic(
                    PYTHON_REACHING_EDGE_TARGET_ALIAS,
                    fields.node_ordinal.as_ref(),
                )
                .alias(PYTHON_REACHING_EDGE_TARGET_ORDINAL),
            ])?
            .filter(
                col(PYTHON_REACHING_EDGE_TARGET_ORDINAL)
                    .eq(col(PYTHON_REACHING_EDGE_SOURCE_ORDINAL) + lit(1_u32)),
            )?
            .alias(PYTHON_REACHING_EDGE_POSITION_ALIAS)?
            .build()?;

        let joined_path = LogicalPlanBuilder::from(candidates)
            .join_on(
                edge_positions,
                JoinType::Left,
                python_reaching_path_join(fields).into_iter().chain([
                    qualified_programmatic(
                        PYTHON_REACHING_EDGE_POSITION_ALIAS,
                        PYTHON_REACHING_EDGE_SOURCE_ORDINAL,
                    )
                    .gt_eq(qualified_programmatic(
                        PYTHON_REACHING_CANDIDATE_ALIAS,
                        PYTHON_REACHING_DEFINITION_ORDINAL,
                    )),
                    qualified_programmatic(
                        PYTHON_REACHING_EDGE_POSITION_ALIAS,
                        PYTHON_REACHING_EDGE_TARGET_ORDINAL,
                    )
                    .lt_eq(qualified_programmatic(
                        PYTHON_REACHING_CANDIDATE_ALIAS,
                        PYTHON_REACHING_USE_ORDINAL,
                    )),
                ]),
            )?
            .build()?;
        let mut groups = python_flow_link_group(PYTHON_REACHING_CANDIDATE_ALIAS, fields);
        groups.extend([
            qualified_programmatic(
                PYTHON_REACHING_CANDIDATE_ALIAS,
                PYTHON_REACHING_DEFINITION_ORDINAL,
            ),
            qualified_programmatic(PYTHON_REACHING_CANDIDATE_ALIAS, PYTHON_REACHING_USE_ORDINAL),
        ]);
        let path = LogicalPlanBuilder::from(joined_path)
            .aggregate(
                groups,
                [count(qualified_programmatic(
                    PYTHON_REACHING_EDGE_POSITION_ALIAS,
                    fields.edge_id.as_ref(),
                ))
                .alias(PYTHON_REACHING_PATH_EDGE_COUNT)],
            )?
            .filter(col(PYTHON_REACHING_PATH_EDGE_COUNT).eq(cast(
                col(PYTHON_REACHING_USE_ORDINAL),
                DataType::UInt64,
            ) - cast(
                col(PYTHON_REACHING_DEFINITION_ORDINAL),
                DataType::UInt64,
            )))?
            .alias(PYTHON_REACHING_PATH_ALIAS)?
            .build()?;
        let rank = row_number()
            .partition_by(vec![
                qualified_programmatic(PYTHON_REACHING_PATH_ALIAS, fields.owner_id.as_ref()),
                qualified_programmatic(PYTHON_REACHING_PATH_ALIAS, fields.use_event_id.as_ref()),
                qualified_programmatic(PYTHON_REACHING_PATH_ALIAS, fields.location_id.as_ref()),
            ])
            .order_by(vec![
                qualified_programmatic(
                    PYTHON_REACHING_PATH_ALIAS,
                    PYTHON_REACHING_DEFINITION_ORDINAL,
                )
                .sort(false, false),
                qualified_programmatic(
                    PYTHON_REACHING_PATH_ALIAS,
                    fields.definition_event_id.as_ref(),
                )
                .sort(true, false),
            ])
            .build()?
            .alias(PYTHON_REACHING_RANK);
        let ranked = LogicalPlanBuilder::from(path).window([rank])?.build()?;
        let selected = LogicalPlanBuilder::from(ranked)
            .filter(col(PYTHON_REACHING_RANK).eq(lit(1_u64)))?
            .build()?;
        self.flow_link_projection_plan(selected, &self.bindings.values.reaching_definition)
    }

    fn liveness_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let nodes = inputs.plan(&self.dependency[0])?;
        let reaching = LogicalPlanBuilder::from(inputs.plan(&self.dependency[1])?)
            .alias(PYTHON_REACHING_CANDIDATE_ALIAS)?
            .build()?;
        let source_nodes = LogicalPlanBuilder::from(nodes.clone())
            .alias(PYTHON_LIVENESS_SOURCE_NODE_ALIAS)?
            .build()?;
        let with_source = LogicalPlanBuilder::from(reaching)
            .join_on(
                source_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_REACHING_CANDIDATE_ALIAS,
                    PYTHON_LIVENESS_SOURCE_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_REACHING_CANDIDATE_ALIAS,
                    fields.source_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_LIVENESS_SOURCE_NODE_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project(
                python_flow_link_projection(PYTHON_REACHING_CANDIDATE_ALIAS, fields)
                    .into_iter()
                    .chain([qualified_programmatic(
                        PYTHON_LIVENESS_SOURCE_NODE_ALIAS,
                        fields.node_ordinal.as_ref(),
                    )
                    .alias(PYTHON_REACHING_DEFINITION_ORDINAL)]),
            )?
            .alias(PYTHON_LIVENESS_WITH_SOURCE_ALIAS)?
            .build()?;
        let target_nodes = LogicalPlanBuilder::from(nodes.clone())
            .alias(PYTHON_LIVENESS_TARGET_NODE_ALIAS)?
            .build()?;
        let range = LogicalPlanBuilder::from(with_source)
            .join_on(
                target_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_LIVENESS_WITH_SOURCE_ALIAS,
                    PYTHON_LIVENESS_TARGET_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    PYTHON_LIVENESS_WITH_SOURCE_ALIAS,
                    fields.target_node_id.as_ref(),
                )
                .eq(qualified_programmatic(
                    PYTHON_LIVENESS_TARGET_NODE_ALIAS,
                    fields.node_id.as_ref(),
                ))]),
            )?
            .project(
                python_flow_link_projection(PYTHON_LIVENESS_WITH_SOURCE_ALIAS, fields)
                    .into_iter()
                    .chain([
                        qualified_programmatic(
                            PYTHON_LIVENESS_WITH_SOURCE_ALIAS,
                            PYTHON_REACHING_DEFINITION_ORDINAL,
                        ),
                        qualified_programmatic(
                            PYTHON_LIVENESS_TARGET_NODE_ALIAS,
                            fields.node_ordinal.as_ref(),
                        )
                        .alias(PYTHON_REACHING_USE_ORDINAL),
                    ]),
            )?
            .alias(PYTHON_LIVENESS_RANGE_ALIAS)?
            .build()?;
        let live_nodes = LogicalPlanBuilder::from(nodes)
            .alias(PYTHON_LIVENESS_NODE_ALIAS)?
            .build()?;
        let members = LogicalPlanBuilder::from(range)
            .join_on(
                live_nodes,
                JoinType::Inner,
                python_derived_join(
                    PYTHON_LIVENESS_RANGE_ALIAS,
                    PYTHON_LIVENESS_NODE_ALIAS,
                    fields,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(
                        PYTHON_LIVENESS_NODE_ALIAS,
                        fields.node_ordinal.as_ref(),
                    )
                    .gt_eq(qualified_programmatic(
                        PYTHON_LIVENESS_RANGE_ALIAS,
                        PYTHON_REACHING_DEFINITION_ORDINAL,
                    )),
                    qualified_programmatic(
                        PYTHON_LIVENESS_NODE_ALIAS,
                        fields.node_ordinal.as_ref(),
                    )
                    .lt_eq(qualified_programmatic(
                        PYTHON_LIVENESS_RANGE_ALIAS,
                        PYTHON_REACHING_USE_ORDINAL,
                    )),
                ]),
            )?
            .project(
                python_flow_link_projection(PYTHON_LIVENESS_RANGE_ALIAS, fields)
                    .into_iter()
                    .chain([
                        qualified_programmatic(
                            PYTHON_LIVENESS_RANGE_ALIAS,
                            PYTHON_REACHING_DEFINITION_ORDINAL,
                        ),
                        qualified_programmatic(
                            PYTHON_LIVENESS_RANGE_ALIAS,
                            PYTHON_REACHING_USE_ORDINAL,
                        ),
                        qualified_programmatic(PYTHON_LIVENESS_NODE_ALIAS, fields.node_id.as_ref()),
                        qualified_programmatic(
                            PYTHON_LIVENESS_NODE_ALIAS,
                            fields.node_ordinal.as_ref(),
                        )
                        .alias(PYTHON_LIVENESS_NODE_ORDINAL),
                    ]),
            )?
            .alias(PYTHON_LIVENESS_MEMBER_ALIAS)?
            .build()?;

        let entry = self.liveness_boundary_plan(
            members.clone(),
            "ENTRY",
            self.bindings.values.live_entry.as_ref(),
            col(PYTHON_LIVENESS_NODE_ORDINAL)
                .gt(col(PYTHON_REACHING_DEFINITION_ORDINAL))
                .and(col(PYTHON_LIVENESS_NODE_ORDINAL).lt_eq(col(PYTHON_REACHING_USE_ORDINAL))),
        )?;
        let exit = self.liveness_boundary_plan(
            members,
            "EXIT",
            self.bindings.values.live_exit.as_ref(),
            col(PYTHON_LIVENESS_NODE_ORDINAL)
                .gt_eq(col(PYTHON_REACHING_DEFINITION_ORDINAL))
                .and(col(PYTHON_LIVENESS_NODE_ORDINAL).lt(col(PYTHON_REACHING_USE_ORDINAL))),
        )?;
        Ok(LogicalPlanBuilder::from(entry)
            .union_distinct(exit)?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.node_ordinal.as_ref()).sort(true, false),
                col(fields.location_id.as_ref()).sort(true, false),
                col(fields.boundary.as_ref()).sort(true, false),
            ])?
            .build()?)
    }

    fn value_flow_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        self.flow_link_projection_plan(
            inputs.plan(&self.dependency[0])?,
            &self.bindings.values.value_flow,
        )
    }

    fn flow_link_projection_plan(
        &self,
        input: LogicalPlan,
        relation_kind: &Arc<str>,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let edge_id = self.relation_identity.call(vec![
            fabric_epoch_literal(self.fabric_epoch_id),
            col(fields.source_pin.as_ref()),
            col(fields.analysis_context_id.as_ref()),
            col(fields.owner_id.as_ref()),
            col(fields.definition_event_id.as_ref()),
            col(fields.use_event_id.as_ref()),
            col(fields.location_id.as_ref()),
            utf8_literal(relation_kind),
        ]);
        Ok(LogicalPlanBuilder::from(input)
            .project([
                col(fields.fabric_epoch_id.as_ref()),
                col(fields.source_pin.as_ref()),
                col(fields.analysis_context_id.as_ref()),
                col(fields.source_generation.as_ref()),
                col(fields.owner_id.as_ref()),
                col(fields.ruff_provider_run_id.as_ref()),
                col(fields.ruff_provider_release.as_ref()),
                col(fields.pyrefly_provider_run_id.as_ref()),
                col(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                col(fields.analysis_completeness.as_ref()),
                edge_id.alias(fields.edge_id.as_ref()),
                col(fields.definition_event_id.as_ref()),
                col(fields.use_event_id.as_ref()),
                col(fields.location_id.as_ref()),
                col(fields.source_node_id.as_ref()),
                col(fields.target_node_id.as_ref()),
                utf8_literal(relation_kind).alias(fields.relation_kind.as_ref()),
            ])?
            .distinct()?
            .sort([
                col(fields.owner_id.as_ref()).sort(true, false),
                col(fields.use_event_id.as_ref()).sort(true, false),
                col(fields.definition_event_id.as_ref()).sort(true, false),
            ])?
            .build()?)
    }

    fn liveness_boundary_plan(
        &self,
        input: LogicalPlan,
        boundary: &'static str,
        relation_kind: &str,
        filter: Expr,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        Ok(LogicalPlanBuilder::from(input)
            .filter(filter)?
            .project([
                col(fields.fabric_epoch_id.as_ref()),
                col(fields.source_pin.as_ref()),
                col(fields.analysis_context_id.as_ref()),
                col(fields.source_generation.as_ref()),
                col(fields.owner_id.as_ref()),
                col(fields.ruff_provider_run_id.as_ref()),
                col(fields.ruff_provider_release.as_ref()),
                col(fields.pyrefly_provider_run_id.as_ref()),
                col(fields.pyrefly_provider_release.as_ref()),
                utf8_literal(&self.row.algorithm_release).alias(fields.algorithm_release.as_ref()),
                utf8_literal(&self.row.precision_release).alias(fields.precision_release.as_ref()),
                utf8_literal(&self.row.authority).alias(fields.authority.as_ref()),
                col(fields.analysis_completeness.as_ref()),
                col(fields.node_id.as_ref()),
                col(PYTHON_LIVENESS_NODE_ORDINAL).alias(fields.node_ordinal.as_ref()),
                lit(boundary).alias(fields.boundary.as_ref()),
                col(fields.location_id.as_ref()),
                lit(relation_kind).alias(fields.relation_kind.as_ref()),
            ])?
            .distinct()?
            .build()?)
    }
}

impl ProgrammaticTransformation for ProgrammaticPythonDataflowTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependency
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        match self.role {
            PythonDerivedRelation::EvaluationOrder => self.evaluation_order_plan(inputs),
            PythonDerivedRelation::DefUse => self.def_use_plan(inputs),
            PythonDerivedRelation::ReachingDefinition => self.reaching_definition_plan(inputs),
            PythonDerivedRelation::Liveness => self.liveness_plan(inputs),
            PythonDerivedRelation::ValueFlow => self.value_flow_plan(inputs),
            _ => unreachable!("role was validated at construction"),
        }
    }
}

/// Native catalog-input normalization of raw rustc MIR CFG edges.
pub struct ProgrammaticRustMirCfgEdgeTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependency: Arc<[ProgrammaticRelationId]>,
    edge_identity: Arc<ScalarUDF>,
}

impl ProgrammaticRustMirCfgEdgeTransformation {
    pub const OUTPUT_FIELD_COUNT: usize = 15;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &RustMirAnalysisBindings,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        RustMirDerivedRelation::CfgEdge
            .schema(bindings)
            .map_err(|error| {
                ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
            })?;
        let expected = ProgrammaticRelationId::new(
            bindings
                .relation_id(RustMirDerivedRelation::CfgEdge)
                .as_str(),
        );
        validate_existing_output(
            ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::CfgEdge),
            &output,
            &expected,
            Self::OUTPUT_FIELD_COUNT,
        )?;
        Ok(Self {
            contract,
            output,
            dependency: Arc::from([ProgrammaticRelationId::new(
                RustcRelation::CfgEdge.relation_id(),
            )]),
            edge_identity: rust_mir_cfg_edge_identity_udf(),
        })
    }
}

impl ProgrammaticTransformation for ProgrammaticRustMirCfgEdgeTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependency
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let input = inputs.plan(&self.dependency[0])?;
        let edge_id = self.edge_identity.call(vec![
            col("stable_crate_id"),
            col("def_path_hash"),
            col("source_block"),
            col("target_block"),
            col("edge_kind"),
            col("branch_value_u128"),
            col("unwind_action"),
        ]);
        let canonical = col("stable_crate_id")
            .is_not_null()
            .and(col("def_path_hash").is_not_null());
        Ok(LogicalPlanBuilder::from(input)
            .project([
                col("provider_run_id"),
                col("compilation_unit_id"),
                col("owner_id"),
                col("source_generation"),
                col("source_file_id"),
                col("source_content_digest"),
                col("stable_crate_id"),
                col("def_path_hash"),
                edge_id.alias("edge_id"),
                canonical.alias("canonical_identity_available"),
                col("source_block"),
                col("target_block"),
                col("edge_kind"),
                col("branch_value_u128"),
                col("unwind_action"),
            ])?
            .distinct()?
            .sort([
                col("provider_run_id").sort(true, false),
                col("owner_id").sort(true, false),
                col("source_block").sort(true, false),
                col("target_block").sort(true, false),
                col("edge_kind").sort(true, false),
            ])?
            .build()?)
    }
}

/// Native control-dependence input normalization over exact public-MIR relations.
///
/// This is deliberately an input fact family, not a control-dependence verdict. Every accepted
/// controller edge remains visible, while an absent optional predicate stays null rather than
/// being filtered into apparent absence. Joins, controller selection, projection, distinctness,
/// and ordering remain native optimizer-visible DataFusion operators.
pub struct ProgrammaticRustMirControlInputTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependencies: Arc<[ProgrammaticRelationId]>,
    control_identity: Arc<ScalarUDF>,
}

impl ProgrammaticRustMirControlInputTransformation {
    pub const OUTPUT_FIELD_COUNT: usize = 22;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &RustMirAnalysisBindings,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        RustMirDerivedRelation::ControlDependenceInput
            .schema(bindings)
            .map_err(|error| {
                ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
            })?;
        let expected = ProgrammaticRelationId::new(
            bindings
                .relation_id(RustMirDerivedRelation::ControlDependenceInput)
                .as_str(),
        );
        validate_existing_output(
            ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::ControlDependenceInput),
            &output,
            &expected,
            Self::OUTPUT_FIELD_COUNT,
        )?;
        Ok(Self {
            contract,
            output,
            dependencies: Arc::from([
                ProgrammaticRelationId::new(RustcRelation::MirBlock.relation_id()),
                ProgrammaticRelationId::new(RustcRelation::MirOperand.relation_id()),
                ProgrammaticRelationId::new(RustcRelation::MirTerminator.relation_id()),
                ProgrammaticRelationId::new(
                    bindings
                        .relation_id(RustMirDerivedRelation::CfgEdge)
                        .as_str(),
                ),
            ]),
            control_identity: rust_mir_control_input_identity_udf(),
        })
    }
}

impl ProgrammaticTransformation for ProgrammaticRustMirControlInputTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependencies
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let block = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[0])?)
            .alias(RUST_CONTROL_BLOCK_ALIAS)?
            .build()?;
        let operand = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[1])?)
            .filter(
                col("slot_kind")
                    .eq(lit("terminator"))
                    .and(col("slot_index").eq(lit(0_u64))),
            )?
            .alias(RUST_CONTROL_OPERAND_ALIAS)?
            .build()?;
        let terminator = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[2])?)
            .alias(RUST_CONTROL_TERMINATOR_ALIAS)?
            .build()?;
        let edge = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[3])?)
            .alias(RUST_CONTROL_EDGE_ALIAS)?
            .build()?;

        let multiple_edge_controllers = LogicalPlanBuilder::from(edge.clone())
            .aggregate(
                rust_control_edge_group(RUST_CONTROL_EDGE_ALIAS),
                [
                    count(qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_id"))
                        .alias(RUST_CONTROL_EDGE_COUNT),
                ],
            )?
            .filter(col(RUST_CONTROL_EDGE_COUNT).gt(lit(1_i64)))?
            .project(rust_control_group_projection())?
            .build()?;
        let unwind_controllers = LogicalPlanBuilder::from(edge.clone())
            .filter(qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_kind").eq(lit("Unwind")))?
            .project(rust_control_edge_projection(RUST_CONTROL_EDGE_ALIAS))?
            .distinct()?
            .build()?;
        let controllers = LogicalPlanBuilder::from(multiple_edge_controllers)
            .union(unwind_controllers)?
            .distinct()?
            .alias(RUST_CONTROL_CONTROLLER_ALIAS)?
            .build()?;

        let block_terminator = LogicalPlanBuilder::from(block)
            .join_on(
                terminator,
                JoinType::Inner,
                rust_owner_join(RUST_CONTROL_BLOCK_ALIAS, RUST_CONTROL_TERMINATOR_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_CONTROL_BLOCK_ALIAS, "block_index").eq(
                            qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index"),
                        ),
                    ]),
            )?
            .build()?;
        // Keep the usually-small controller-key set on DataFusion's hash-build side. Besides
        // avoiding needless memory, this prevents a later statistics-driven swap after dynamic
        // filters have been attached to the native hash join.
        let controllers_with_terminator = LogicalPlanBuilder::from(controllers)
            .join_on(
                block_terminator,
                JoinType::Inner,
                rust_owner_join(RUST_CONTROL_TERMINATOR_ALIAS, RUST_CONTROL_CONTROLLER_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index").eq(
                            qualified_programmatic(
                                RUST_CONTROL_CONTROLLER_ALIAS,
                                "controller_block",
                            ),
                        ),
                    ]),
            )?
            .build()?;
        let controller_edges = LogicalPlanBuilder::from(controllers_with_terminator)
            .join_on(
                edge,
                JoinType::Inner,
                rust_owner_join(RUST_CONTROL_TERMINATOR_ALIAS, RUST_CONTROL_EDGE_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index").eq(
                            qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "source_block"),
                        ),
                    ]),
            )?
            .build()?;
        let predicate_kind =
            qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "raw_terminator_kind");
        let predicate_role = qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "parent_role");
        let predicate_match = predicate_kind
            .clone()
            .eq(lit("SwitchInt"))
            .and(predicate_role.clone().eq(lit("switch-discriminant")))
            .or(predicate_kind
                .eq(lit("Assert"))
                .and(predicate_role.eq(lit("assert-condition"))));
        let with_predicate = LogicalPlanBuilder::from(controller_edges)
            .join_on(
                operand,
                JoinType::Left,
                rust_owner_join(RUST_CONTROL_TERMINATOR_ALIAS, RUST_CONTROL_OPERAND_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index").eq(
                            qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "block_index"),
                        ),
                        predicate_match,
                    ]),
            )?
            .build()?;

        let control_id = self.control_identity.call(vec![
            qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "owner_id"),
            qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index"),
            qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_id"),
            qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "operand_id"),
            qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "raw_terminator_kind"),
        ]);
        let canonical = qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "stable_crate_id")
            .is_not_null()
            .and(
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "def_path_hash")
                    .is_not_null(),
            );
        Ok(LogicalPlanBuilder::from(with_predicate)
            .project([
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "provider_run_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "compilation_unit_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "owner_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "source_generation"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "source_file_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "source_content_digest"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "stable_crate_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "def_path_hash"),
                canonical.alias("canonical_identity_available"),
                control_id.alias("control_input_id"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "block_index")
                    .alias("controller_block"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "raw_terminator_kind")
                    .alias("controller_kind"),
                qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "operand_id")
                    .alias("predicate_operand_id"),
                qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "parent_role")
                    .alias("predicate_role"),
                qualified_programmatic(RUST_CONTROL_OPERAND_ALIAS, "operand_kind")
                    .alias("predicate_operand_kind"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "source_scope"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "normal_target_count"),
                qualified_programmatic(RUST_CONTROL_TERMINATOR_ALIAS, "unwind_action"),
                qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_id"),
                qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "target_block"),
                qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_kind"),
                qualified_programmatic(RUST_CONTROL_EDGE_ALIAS, "edge_kind")
                    .eq(lit("Unwind"))
                    .alias("is_unwind"),
            ])?
            .distinct()?
            .sort([
                col("owner_id").sort(true, false),
                col("controller_block").sort(true, false),
                col("target_block").sort(true, false),
                col("edge_kind").sort(true, false),
            ])?
            .build()?)
    }
}

/// Programmatic structural analyses over accepted public-MIR relations.
///
/// These five roles are intentionally limited to facts directly supported by the admitted
/// relations: observed ownership-relevant accesses, conservative reference-construction edges,
/// storage/drop events, coroutine aggregate lowering, and unsafe/FFI occurrences. They do not
/// claim borrow-checker loans, must-alias closure, semantic resource kinds, exact suspension-state
/// maps, or lexical unsafe-scope membership. Provider-native kinds and structured evidence remain
/// visible beside the application-owned normalized observation.
pub struct ProgrammaticRustMirStructuralTransformation {
    role: RustMirDerivedRelation,
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependencies: Arc<[ProgrammaticRelationId]>,
}

impl ProgrammaticRustMirStructuralTransformation {
    /// Construct one of the five exact public-MIR structural producers.
    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        role: RustMirDerivedRelation,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &RustMirAnalysisBindings,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        if !matches!(
            role,
            RustMirDerivedRelation::OwnershipState
                | RustMirDerivedRelation::AliasPointsTo
                | RustMirDerivedRelation::ResourceLifecycle
                | RustMirDerivedRelation::AsyncLowering
                | RustMirDerivedRelation::UnsafeFfi
        ) {
            return Err(ProgrammaticDerivedAnalysisError::ExistingBinding(format!(
                "{role:?} is not a public-MIR structural producer role"
            )));
        }
        role.schema(bindings).map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        let expected = ProgrammaticRelationId::new(bindings.relation_id(role).as_str());
        validate_existing_output(
            ExistingDerivedFamilyRole::RustMir(role),
            &output,
            &expected,
            Self::output_field_count(role),
        )?;
        Ok(Self {
            role,
            contract,
            output,
            dependencies: rust_mir_structural_dependencies(role, bindings),
        })
    }

    #[must_use]
    pub const fn output_field_count(role: RustMirDerivedRelation) -> usize {
        match role {
            RustMirDerivedRelation::OwnershipState => 24,
            RustMirDerivedRelation::AliasPointsTo => 22,
            RustMirDerivedRelation::ResourceLifecycle => 19,
            RustMirDerivedRelation::AsyncLowering => 17,
            RustMirDerivedRelation::UnsafeFfi => 20,
            _ => 0,
        }
    }

    fn ownership_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let local = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[0])?)
            .alias(RUST_STRUCTURAL_LOCAL_ALIAS)?
            .build()?;
        let locations = LogicalPlanBuilder::from(rust_mir_place_location_plan(
            inputs.plan(&self.dependencies[1])?,
        )?)
        .alias(RUST_STRUCTURAL_LOCATION_ALIAS)?
        .build()?;
        let access = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[2])?)
            .alias(RUST_STRUCTURAL_ACCESS_ALIAS)?
            .filter(rust_ownership_access_filter(RUST_STRUCTURAL_ACCESS_ALIAS))?
            .build()?;
        let coverage = rust_owner_presence(
            inputs.plan(&self.dependencies[3])?,
            RUST_STRUCTURAL_COVERAGE_ALIAS,
        )?;
        let remainder = rust_owner_presence(
            inputs.plan(&self.dependencies[4])?,
            RUST_STRUCTURAL_REMAINDER_ALIAS,
        )?;

        let access_location = LogicalPlanBuilder::from(access)
            .join_on(
                locations,
                JoinType::Inner,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_LOCATION_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id").eq(
                            qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "place_id"),
                        ),
                    ]),
            )?
            .build()?;
        let with_local = LogicalPlanBuilder::from(access_location)
            .join_on(
                local,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_LOCAL_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "base_local").eq(
                            qualified_programmatic(RUST_STRUCTURAL_LOCAL_ALIAS, "local_index"),
                        ),
                    ]),
            )?
            .build()?;
        let with_coverage = LogicalPlanBuilder::from(with_local)
            .join_on(
                coverage,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_COVERAGE_ALIAS),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_coverage)
            .join_on(
                remainder,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_ACCESS_ALIAS,
                    RUST_STRUCTURAL_REMAINDER_ALIAS,
                ),
            )?
            .build()?;
        let event_id = rust_mir_access_event_identity_udf().call(vec![
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "stable_crate_id"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "def_path_hash"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "block_index"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_kind"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_index"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_ordinal"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_kind"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "structured_evidence"),
        ]);
        Ok(LogicalPlanBuilder::from(complete)
            .project([
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "provider_run_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "compilation_unit_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "owner_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_generation"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_file_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_content_digest"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "stable_crate_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "def_path_hash"),
                qualified_programmatic(
                    RUST_STRUCTURAL_LOCATION_ALIAS,
                    "canonical_identity_available",
                ),
                event_id.alias("event_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "memory_location_id"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "base_local"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "projection_path"),
                qualified_programmatic(RUST_STRUCTURAL_LOCAL_ALIAS, "local_role"),
                qualified_programmatic(RUST_STRUCTURAL_LOCAL_ALIAS, "type_key")
                    .alias("local_type_key"),
                qualified_programmatic(RUST_STRUCTURAL_LOCAL_ALIAS, "mutability")
                    .alias("local_mutability"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "block_index"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_kind"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_index"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_ordinal"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_kind"),
                rust_ownership_observation(RUST_STRUCTURAL_ACCESS_ALIAS)
                    .alias("ownership_observation"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "structured_evidence"),
            ])?
            .distinct()?
            .sort(rust_structural_event_sort())?
            .build()?)
    }

    fn alias_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let base_locations = rust_mir_place_location_plan(inputs.plan(&self.dependencies[0])?)?;
        let source_location = LogicalPlanBuilder::from(base_locations.clone())
            .alias(RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS)?
            .build()?;
        let destination_location = LogicalPlanBuilder::from(base_locations)
            .alias(RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS)?
            .build()?;
        let rvalue = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[1])?)
            .alias(RUST_STRUCTURAL_RVALUE_ALIAS)?
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind")
                    .eq(lit("Ref"))
                    .or(
                        qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind")
                            .eq(lit("Reborrow")),
                    )
                    .or(
                        qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind")
                            .eq(lit("AddressOf")),
                    ),
            )?
            .build()?;
        let statement = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[2])?)
            .alias(RUST_STRUCTURAL_STATEMENT_ALIAS)?
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "raw_statement_kind")
                    .eq(lit("Assign")),
            )?
            .build()?;
        let destination_access = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[3])?)
            .alias(RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS)?
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS, "slot_kind")
                    .eq(lit("statement"))
                    .and(
                        qualified_programmatic(
                            RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                            "structured_evidence",
                        )
                        .eq(lit("StatementKind::Assign.destination")),
                    ),
            )?
            .build()?;

        let rvalue_statement = LogicalPlanBuilder::from(rvalue)
            .join_on(
                statement,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_STATEMENT_ALIAS,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "block_index"),
                    ),
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "statement_index"),
                    ),
                ]),
            )?
            .build()?;
        let with_source = LogicalPlanBuilder::from(rvalue_statement)
            .join_on(
                source_location,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    "source_place_id",
                )
                .eq(qualified_programmatic(
                    RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS,
                    "place_id",
                ))]),
            )?
            .build()?;
        let with_destination_access = LogicalPlanBuilder::from(with_source)
            .join_on(
                destination_access,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index").eq(
                        qualified_programmatic(
                            RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                            "block_index",
                        ),
                    ),
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index").eq(
                        qualified_programmatic(
                            RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                            "slot_index",
                        ),
                    ),
                ]),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_destination_access)
            .join_on(
                destination_location,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                    RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS,
                )
                .into_iter()
                .chain([qualified_programmatic(
                    RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS,
                    "place_id",
                )
                .eq(qualified_programmatic(
                    RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS,
                    "place_id",
                ))]),
            )?
            .build()?;
        let alias_id = rust_mir_alias_identity_udf().call(vec![
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "stable_crate_id"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "def_path_hash"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index"),
            qualified_programmatic(RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS, "place_id"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_place_id"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind"),
        ]);
        let canonical = qualified_programmatic(
            RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS,
            "canonical_identity_available",
        )
        .and(qualified_programmatic(
            RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS,
            "canonical_identity_available",
        ));
        Ok(LogicalPlanBuilder::from(complete)
            .project([
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "provider_run_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "compilation_unit_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "owner_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_generation"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_file_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_content_digest"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "stable_crate_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "def_path_hash"),
                canonical.alias("canonical_identity_available"),
                alias_id.alias("alias_observation_id"),
                qualified_programmatic(RUST_STRUCTURAL_DESTINATION_ACCESS_ALIAS, "place_id")
                    .alias("pointer_place_id"),
                qualified_programmatic(
                    RUST_STRUCTURAL_DESTINATION_LOCATION_ALIAS,
                    "memory_location_id",
                )
                .alias("pointer_location_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_place_id")
                    .alias("pointee_place_id"),
                qualified_programmatic(RUST_STRUCTURAL_SOURCE_LOCATION_ALIAS, "memory_location_id")
                    .alias("pointee_location_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind"),
                qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "normalized_effect"),
                qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "source_scope"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "region_kind"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "mutability"),
                lit("MAY_POINT_TO").alias("relation_kind"),
            ])?
            .distinct()?
            .sort([
                col("owner_id").sort(true, false),
                col("block_index").sort(true, false),
                col("statement_index").sort(true, false),
                col("alias_observation_id").sort(true, false),
            ])?
            .build()?)
    }

    fn resource_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let locations = LogicalPlanBuilder::from(rust_mir_place_location_plan(
            inputs.plan(&self.dependencies[0])?,
        )?)
        .alias(RUST_STRUCTURAL_LOCATION_ALIAS)?
        .build()?;
        let access = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[1])?)
            .alias(RUST_STRUCTURAL_ACCESS_ALIAS)?
            .filter(rust_resource_access_filter(RUST_STRUCTURAL_ACCESS_ALIAS))?
            .build()?;
        let terminator = rust_owner_presence(
            inputs.plan(&self.dependencies[2])?,
            RUST_STRUCTURAL_TERMINATOR_ALIAS,
        )?;
        let cfg = rust_owner_presence(
            inputs.plan(&self.dependencies[3])?,
            RUST_STRUCTURAL_CFG_ALIAS,
        )?;
        let call = rust_owner_presence(
            inputs.plan(&self.dependencies[4])?,
            RUST_STRUCTURAL_CALL_ALIAS,
        )?;
        let instance = rust_owner_presence(
            inputs.plan(&self.dependencies[5])?,
            RUST_STRUCTURAL_INSTANCE_ALIAS,
        )?;
        let joined = LogicalPlanBuilder::from(access)
            .join_on(
                locations,
                JoinType::Inner,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_LOCATION_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id").eq(
                            qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "place_id"),
                        ),
                    ]),
            )?
            .build()?;
        let with_terminator = LogicalPlanBuilder::from(joined)
            .join_on(
                terminator,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_ACCESS_ALIAS,
                    RUST_STRUCTURAL_TERMINATOR_ALIAS,
                ),
            )?
            .build()?;
        let with_cfg = LogicalPlanBuilder::from(with_terminator)
            .join_on(
                cfg,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_CFG_ALIAS),
            )?
            .build()?;
        let with_call = LogicalPlanBuilder::from(with_cfg)
            .join_on(
                call,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_CALL_ALIAS),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_call)
            .join_on(
                instance,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_ACCESS_ALIAS, RUST_STRUCTURAL_INSTANCE_ALIAS),
            )?
            .build()?;
        let event_id = rust_mir_access_event_identity_udf().call(vec![
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "stable_crate_id"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "def_path_hash"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "block_index"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_kind"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_index"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_ordinal"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "access_kind"),
            qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "structured_evidence"),
        ]);
        Ok(LogicalPlanBuilder::from(complete)
            .project([
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "provider_run_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "compilation_unit_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "owner_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_generation"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_file_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "source_content_digest"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "stable_crate_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "def_path_hash"),
                qualified_programmatic(
                    RUST_STRUCTURAL_LOCATION_ALIAS,
                    "canonical_identity_available",
                ),
                event_id.alias("lifecycle_event_id"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "place_id"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "memory_location_id"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "base_local"),
                qualified_programmatic(RUST_STRUCTURAL_LOCATION_ALIAS, "projection_path"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "block_index"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_kind"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "slot_index"),
                rust_resource_lifecycle(RUST_STRUCTURAL_ACCESS_ALIAS).alias("lifecycle_event"),
                qualified_programmatic(RUST_STRUCTURAL_ACCESS_ALIAS, "structured_evidence"),
            ])?
            .distinct()?
            .sort(rust_structural_event_sort())?
            .build()?)
    }

    fn async_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let body = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[0])?)
            .alias(RUST_STRUCTURAL_BODY_ALIAS)?
            .build()?;
        let rvalue = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[1])?)
            .alias(RUST_STRUCTURAL_RVALUE_ALIAS)?
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "aggregate_kind")
                    .eq(lit("Coroutine"))
                    .or(
                        qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "aggregate_kind")
                            .eq(lit("CoroutineClosure")),
                    ),
            )?
            .build()?;
        let statement = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[2])?)
            .alias(RUST_STRUCTURAL_STATEMENT_ALIAS)?
            .build()?;
        let terminator = rust_owner_presence(
            inputs.plan(&self.dependencies[3])?,
            RUST_STRUCTURAL_TERMINATOR_ALIAS,
        )?;
        let cfg = rust_owner_presence(
            inputs.plan(&self.dependencies[4])?,
            RUST_STRUCTURAL_CFG_ALIAS,
        )?;
        let with_statement = LogicalPlanBuilder::from(rvalue)
            .join_on(
                statement,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_STATEMENT_ALIAS,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "block_index"),
                    ),
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "statement_index"),
                    ),
                ]),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_statement)
            .join_on(
                body,
                JoinType::Inner,
                rust_owner_join(RUST_STRUCTURAL_RVALUE_ALIAS, RUST_STRUCTURAL_BODY_ALIAS),
            )?
            .build()?;
        let with_terminator = LogicalPlanBuilder::from(complete)
            .join_on(
                terminator,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_TERMINATOR_ALIAS,
                ),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_terminator)
            .join_on(
                cfg,
                JoinType::Left,
                rust_owner_join(RUST_STRUCTURAL_RVALUE_ALIAS, RUST_STRUCTURAL_CFG_ALIAS),
            )?
            .build()?;
        let observation_id = rust_mir_async_identity_udf().call(vec![
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "stable_crate_id"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "def_path_hash"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index"),
            qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "aggregate_kind"),
        ]);
        Ok(LogicalPlanBuilder::from(complete)
            .project([
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "provider_run_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "compilation_unit_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "owner_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_generation"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_file_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "source_content_digest"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "stable_crate_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "def_path_hash"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "stable_crate_id")
                    .is_not_null()
                    .and(
                        qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "def_path_hash")
                            .is_not_null(),
                    )
                    .alias("canonical_identity_available"),
                observation_id.alias("observation_id"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index"),
                qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "source_scope"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "rvalue_kind"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "aggregate_kind"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "result_type_key"),
                lit("COROUTINE_AGGREGATE_OBSERVED").alias("observation_kind"),
            ])?
            .distinct()?
            .sort([
                col("owner_id").sort(true, false),
                col("block_index").sort(true, false),
                col("statement_index").sort(true, false),
            ])?
            .build()?)
    }

    fn unsafe_ffi_plan(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let public_item = rust_owner_presence(
            inputs.plan(&self.dependencies[0])?,
            RUST_STRUCTURAL_PUBLIC_ALIAS,
        )?;
        let type_relation = rust_owner_presence(
            inputs.plan(&self.dependencies[1])?,
            RUST_STRUCTURAL_TYPE_ALIAS,
        )?;
        let rvalue = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[2])?)
            .alias(RUST_STRUCTURAL_RVALUE_ALIAS)?
            .build()?;
        let statement = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[3])?)
            .alias(RUST_STRUCTURAL_STATEMENT_ALIAS)?
            .build()?;
        let terminator = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[4])?)
            .alias(RUST_STRUCTURAL_TERMINATOR_ALIAS)?
            .build()?;
        let call = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[5])?)
            .alias(RUST_STRUCTURAL_CALL_ALIAS)?
            .build()?;
        let instance = LogicalPlanBuilder::from(inputs.plan(&self.dependencies[6])?)
            .alias(RUST_STRUCTURAL_INSTANCE_ALIAS)?
            .build()?;
        let access = rust_owner_presence(
            inputs.plan(&self.dependencies[7])?,
            RUST_STRUCTURAL_UNSAFE_ACCESS_ALIAS,
        )?;

        let inline = LogicalPlanBuilder::from(terminator.clone())
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "raw_terminator_kind")
                    .eq(lit("InlineAsm")),
            )?
            .project(rust_unsafe_projection(
                RUST_STRUCTURAL_TERMINATOR_ALIAS,
                qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "block_index"),
                lit("terminator"),
                lit(0_u64),
                qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "source_scope"),
                lit("INLINE_ASM"),
                qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "raw_terminator_kind"),
                lit(ScalarValue::Utf8(None)),
                lit(ScalarValue::FixedSizeBinary(32, None)),
                lit(ScalarValue::Boolean(None)),
                lit("MirTerminator::InlineAsm"),
            ))?
            .build()?;

        let unsafe_cast = qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "cast_kind");
        let cast_filtered = LogicalPlanBuilder::from(rvalue)
            .filter(rust_unsafe_cast_filter(unsafe_cast.clone()))?
            .build()?;
        let cast_joined = LogicalPlanBuilder::from(cast_filtered)
            .join_on(
                statement,
                JoinType::Inner,
                rust_owner_join(
                    RUST_STRUCTURAL_RVALUE_ALIAS,
                    RUST_STRUCTURAL_STATEMENT_ALIAS,
                )
                .into_iter()
                .chain([
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "block_index"),
                    ),
                    qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index").eq(
                        qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "statement_index"),
                    ),
                ]),
            )?
            .project(rust_unsafe_projection(
                RUST_STRUCTURAL_RVALUE_ALIAS,
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "block_index"),
                lit("statement"),
                qualified_programmatic(RUST_STRUCTURAL_RVALUE_ALIAS, "statement_index"),
                qualified_programmatic(RUST_STRUCTURAL_STATEMENT_ALIAS, "source_scope"),
                lit("UNSAFE_RELEVANT_CAST"),
                unsafe_cast,
                lit(ScalarValue::Utf8(None)),
                lit(ScalarValue::FixedSizeBinary(32, None)),
                lit(ScalarValue::Boolean(None)),
                lit("MirRvalue::Cast"),
            ))?
            .build()?;

        let call_instance = LogicalPlanBuilder::from(call)
            .join_on(
                instance,
                JoinType::Inner,
                rust_owner_join(RUST_STRUCTURAL_CALL_ALIAS, RUST_STRUCTURAL_INSTANCE_ALIAS)
                    .into_iter()
                    .chain([qualified_programmatic(
                        RUST_STRUCTURAL_CALL_ALIAS,
                        "resolved_instance_key",
                    )
                    .eq(qualified_programmatic(
                        RUST_STRUCTURAL_INSTANCE_ALIAS,
                        "instance_key",
                    ))]),
            )?
            .filter(
                qualified_programmatic(RUST_STRUCTURAL_INSTANCE_ALIAS, "is_foreign_item")
                    .eq(lit(true)),
            )?
            .build()?;
        let foreign = LogicalPlanBuilder::from(call_instance)
            .join_on(
                terminator,
                JoinType::Inner,
                rust_owner_join(RUST_STRUCTURAL_CALL_ALIAS, RUST_STRUCTURAL_TERMINATOR_ALIAS)
                    .into_iter()
                    .chain([
                        qualified_programmatic(RUST_STRUCTURAL_CALL_ALIAS, "block_index").eq(
                            qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "block_index"),
                        ),
                    ]),
            )?
            .project(rust_unsafe_projection(
                RUST_STRUCTURAL_CALL_ALIAS,
                qualified_programmatic(RUST_STRUCTURAL_CALL_ALIAS, "block_index"),
                lit("terminator"),
                lit(0_u64),
                qualified_programmatic(RUST_STRUCTURAL_TERMINATOR_ALIAS, "source_scope"),
                lit("FOREIGN_CALL"),
                lit("Call"),
                qualified_programmatic(RUST_STRUCTURAL_CALL_ALIAS, "declared_target"),
                qualified_programmatic(RUST_STRUCTURAL_CALL_ALIAS, "resolved_instance_key"),
                qualified_programmatic(RUST_STRUCTURAL_INSTANCE_ALIAS, "is_foreign_item"),
                qualified_programmatic(RUST_STRUCTURAL_CALL_ALIAS, "dispatch_kind"),
            ))?
            .build()?;

        let rows = LogicalPlanBuilder::from(inline)
            .union(cast_joined)?
            .union(foreign)?
            .distinct()?
            .alias(RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS)?
            .build()?;
        let with_public = LogicalPlanBuilder::from(rows)
            .join_on(
                public_item,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS,
                    RUST_STRUCTURAL_PUBLIC_ALIAS,
                ),
            )?
            .build()?;
        let with_type = LogicalPlanBuilder::from(with_public)
            .join_on(
                type_relation,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS,
                    RUST_STRUCTURAL_TYPE_ALIAS,
                ),
            )?
            .build()?;
        let complete = LogicalPlanBuilder::from(with_type)
            .join_on(
                access,
                JoinType::Left,
                rust_owner_join(
                    RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS,
                    RUST_STRUCTURAL_UNSAFE_ACCESS_ALIAS,
                ),
            )?
            .build()?;
        let rows = LogicalPlanBuilder::from(complete)
            .project(rust_unsafe_output_columns(
                RUST_STRUCTURAL_UNSAFE_ROWS_ALIAS,
            ))?
            .distinct()?
            .sort([
                col("owner_id").sort(true, false),
                col("block_index").sort(true, false),
                col("slot_kind").sort(true, false),
                col("slot_index").sort(true, false),
                col("observation_kind").sort(true, false),
            ])?
            .build()?;
        Ok(rows)
    }
}

impl ProgrammaticTransformation for ProgrammaticRustMirStructuralTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependencies
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        match self.role {
            RustMirDerivedRelation::OwnershipState => self.ownership_plan(inputs),
            RustMirDerivedRelation::AliasPointsTo => self.alias_plan(inputs),
            RustMirDerivedRelation::ResourceLifecycle => self.resource_plan(inputs),
            RustMirDerivedRelation::AsyncLowering => self.async_plan(inputs),
            RustMirDerivedRelation::UnsafeFfi => self.unsafe_ffi_plan(inputs),
            _ => unreachable!("role was validated at construction"),
        }
    }
}

/// Native programmatic common call-graph facts derived from accepted Pyrefly call-target rows.
///
/// Resolved and unresolved candidates coexist; `complete` is true only for provider-resolved
/// targets. This preserves explicit uncertainty instead of filtering it into apparent absence.
pub struct ProgrammaticCommonCallGraphTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependency: Arc<[ProgrammaticRelationId]>,
    bindings: CommonAnalysisBindings,
    call_site_identity: Arc<ScalarUDF>,
}

impl ProgrammaticCommonCallGraphTransformation {
    pub const OUTPUT_FIELD_COUNT: usize = 10;

    pub(crate) fn try_new(
        _authority: &CompiledTransformationAuthority,
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        bindings: &CommonAnalysisBindings,
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        bindings.validate().map_err(|error| {
            ProgrammaticDerivedAnalysisError::ExistingBinding(error.to_string())
        })?;
        let expected = ProgrammaticRelationId::new(bindings.relations.facts.as_str());
        validate_existing_output(
            ExistingDerivedFamilyRole::Common(ExistingCommonDerivedFamilyRole::CallGraph),
            &output,
            &expected,
            Self::OUTPUT_FIELD_COUNT,
        )?;
        Ok(Self {
            contract,
            output,
            dependency: Arc::from([ProgrammaticRelationId::new(
                PyreflyRelation::CallTarget.relation_id(),
            )]),
            bindings: bindings.clone(),
            call_site_identity: common_call_site_identity_udf(),
        })
    }
}

impl ProgrammaticTransformation for ProgrammaticCommonCallGraphTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependency
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let fields = &self.bindings.fields;
        let input = inputs.plan(&self.dependency[0])?;
        let call_site = self.call_site_identity.call(vec![
            col("module_id"),
            col("content_digest"),
            col("start_byte"),
            col("end_byte"),
            col("call_occurrence_ordinal"),
        ]);
        let resolved = col("resolution_state").eq(lit("resolved"));
        Ok(LogicalPlanBuilder::from(input)
            .project([
                utf8_literal(&self.bindings.families.call_graph).alias(fields.family_id.as_str()),
                col("module_id").alias(fields.subject_id.as_str()),
                col("qualified_target").alias(fields.object_id.as_str()),
                call_site.alias(fields.value_id.as_str()),
                resolved.alias(fields.complete.as_str()),
                col("resolution_state"),
                col("call_occurrence_ordinal"),
                col("target_ordinal"),
                col("source_generation"),
                col("content_digest"),
            ])?
            .distinct()?
            .sort([
                col(fields.subject_id.as_str()).sort(true, false),
                col(fields.object_id.as_str()).sort(true, false),
                col(fields.value_id.as_str()).sort(true, false),
            ])?
            .build()?)
    }
}

fn validate_existing_output(
    role: ExistingDerivedFamilyRole,
    output: &TransformationOutput,
    expected_relation: &ProgrammaticRelationId,
    expected_fields: usize,
) -> Result<(), ProgrammaticDerivedAnalysisError> {
    if output.relation_id() != expected_relation || output.fields().len() != expected_fields {
        return Err(
            ProgrammaticDerivedAnalysisError::ExistingProgrammaticOutputMismatch {
                role,
                expected_relation: expected_relation.clone(),
                actual_relation: output.relation_id().clone(),
                expected_fields,
                actual_fields: output.fields().len(),
            },
        );
    }
    Ok(())
}

fn utf8_literal(value: &Arc<str>) -> Expr {
    Expr::Literal(ScalarValue::Utf8(Some(value.to_string())), None)
}

fn fabric_epoch_literal(epoch: FabricEpochId) -> Expr {
    Expr::Literal(
        ScalarValue::FixedSizeBinary(16, Some(epoch.as_bytes().to_vec())),
        None,
    )
}

fn qualified_programmatic(alias: &'static str, name: &str) -> Expr {
    Expr::Column(Column::new(
        Some(datafusion::common::TableReference::bare(alias)),
        name.to_owned(),
    ))
}

fn python_provider_join(left: &'static str, right: &'static str) -> Vec<Expr> {
    [
        "provider_run_id",
        "analysis_context_id",
        "file_id",
        "content_digest",
        "source_generation",
    ]
    .into_iter()
    .map(|field| qualified_programmatic(left, field).eq(qualified_programmatic(right, field)))
    .collect()
}

fn python_raw_to_derived_join(
    raw: &'static str,
    derived: &'static str,
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    vec![
        qualified_programmatic(raw, "provider_run_id").eq(qualified_programmatic(
            derived,
            fields.ruff_provider_run_id.as_ref(),
        )),
        qualified_programmatic(raw, "analysis_context_id").eq(qualified_programmatic(
            derived,
            fields.analysis_context_id.as_ref(),
        )),
        qualified_programmatic(raw, "file_id")
            .eq(qualified_programmatic(derived, fields.owner_id.as_ref())),
        qualified_programmatic(raw, "content_digest")
            .eq(qualified_programmatic(derived, fields.source_pin.as_ref())),
        qualified_programmatic(raw, "source_generation").eq(qualified_programmatic(
            derived,
            fields.source_generation.as_ref(),
        )),
    ]
}

fn python_derived_join(
    left: &'static str,
    right: &'static str,
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    [
        fields.source_pin.as_ref(),
        fields.analysis_context_id.as_ref(),
        fields.source_generation.as_ref(),
        fields.owner_id.as_ref(),
        fields.ruff_provider_run_id.as_ref(),
    ]
    .into_iter()
    .map(|field| qualified_programmatic(left, field).eq(qualified_programmatic(right, field)))
    .collect()
}

fn python_def_use_candidate_projection(alias: &'static str) -> Vec<Expr> {
    [
        "provider_run_id",
        "provider_release",
        "analysis_context_id",
        "file_id",
        "content_digest",
        "source_generation",
        PYTHON_FLOW_BINDING_ID,
        PYTHON_FLOW_BINDING_SCOPE,
        PYTHON_FLOW_BINDING_NAME,
        PYTHON_FLOW_BINDING_START,
        PYTHON_FLOW_BINDING_END,
        PYTHON_FLOW_REFERENCE_ID,
        PYTHON_FLOW_REFERENCE_START,
        PYTHON_FLOW_REFERENCE_END,
        PYTHON_FLOW_REFERENCE_CLASS,
    ]
    .into_iter()
    .map(|field| qualified_programmatic(alias, field).alias(field))
    .collect()
}

fn python_common_projection(
    alias: &'static str,
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    [
        fields.fabric_epoch_id.as_ref(),
        fields.source_pin.as_ref(),
        fields.analysis_context_id.as_ref(),
        fields.source_generation.as_ref(),
        fields.owner_id.as_ref(),
        fields.ruff_provider_run_id.as_ref(),
        fields.ruff_provider_release.as_ref(),
        fields.pyrefly_provider_run_id.as_ref(),
        fields.pyrefly_provider_release.as_ref(),
        fields.algorithm_release.as_ref(),
        fields.precision_release.as_ref(),
        fields.authority.as_ref(),
        fields.analysis_completeness.as_ref(),
    ]
    .into_iter()
    .map(|field| qualified_programmatic(alias, field).alias(field))
    .collect()
}

fn python_flow_link_projection(
    alias: &'static str,
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    python_common_projection(alias, fields)
        .into_iter()
        .chain(
            [
                fields.edge_id.as_ref(),
                fields.definition_event_id.as_ref(),
                fields.use_event_id.as_ref(),
                fields.location_id.as_ref(),
                fields.source_node_id.as_ref(),
                fields.target_node_id.as_ref(),
                fields.relation_kind.as_ref(),
            ]
            .into_iter()
            .map(|field| qualified_programmatic(alias, field).alias(field)),
        )
        .collect()
}

fn python_flow_link_group(
    alias: &'static str,
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    [
        fields.fabric_epoch_id.as_ref(),
        fields.source_pin.as_ref(),
        fields.analysis_context_id.as_ref(),
        fields.source_generation.as_ref(),
        fields.owner_id.as_ref(),
        fields.ruff_provider_run_id.as_ref(),
        fields.ruff_provider_release.as_ref(),
        fields.pyrefly_provider_run_id.as_ref(),
        fields.pyrefly_provider_release.as_ref(),
        fields.algorithm_release.as_ref(),
        fields.precision_release.as_ref(),
        fields.authority.as_ref(),
        fields.analysis_completeness.as_ref(),
        fields.edge_id.as_ref(),
        fields.definition_event_id.as_ref(),
        fields.use_event_id.as_ref(),
        fields.location_id.as_ref(),
        fields.source_node_id.as_ref(),
        fields.target_node_id.as_ref(),
        fields.relation_kind.as_ref(),
    ]
    .into_iter()
    .map(|field| qualified_programmatic(alias, field))
    .collect()
}

fn python_reaching_path_join(
    fields: &crate::python_derived_analysis::PythonFlowFields,
) -> Vec<Expr> {
    [
        fields.source_pin.as_ref(),
        fields.analysis_context_id.as_ref(),
        fields.source_generation.as_ref(),
        fields.owner_id.as_ref(),
        fields.ruff_provider_run_id.as_ref(),
    ]
    .into_iter()
    .map(|field| {
        qualified_programmatic(PYTHON_REACHING_CANDIDATE_ALIAS, field).eq(qualified_programmatic(
            PYTHON_REACHING_EDGE_POSITION_ALIAS,
            field,
        ))
    })
    .collect()
}

fn rust_owner_join(left: &'static str, right: &'static str) -> [Expr; 6] {
    [
        qualified_programmatic(left, "provider_run_id")
            .eq(qualified_programmatic(right, "provider_run_id")),
        qualified_programmatic(left, "compilation_unit_id")
            .eq(qualified_programmatic(right, "compilation_unit_id")),
        qualified_programmatic(left, "owner_id").eq(qualified_programmatic(right, "owner_id")),
        qualified_programmatic(left, "source_generation")
            .eq(qualified_programmatic(right, "source_generation")),
        qualified_programmatic(left, "source_file_id")
            .eq(qualified_programmatic(right, "source_file_id")),
        qualified_programmatic(left, "source_content_digest")
            .eq(qualified_programmatic(right, "source_content_digest")),
    ]
}

fn rust_control_edge_group(alias: &'static str) -> [Expr; 7] {
    [
        qualified_programmatic(alias, "provider_run_id"),
        qualified_programmatic(alias, "compilation_unit_id"),
        qualified_programmatic(alias, "owner_id"),
        qualified_programmatic(alias, "source_generation"),
        qualified_programmatic(alias, "source_file_id"),
        qualified_programmatic(alias, "source_content_digest"),
        qualified_programmatic(alias, "source_block"),
    ]
}

fn rust_control_group_projection() -> [Expr; 7] {
    [
        col("provider_run_id"),
        col("compilation_unit_id"),
        col("owner_id"),
        col("source_generation"),
        col("source_file_id"),
        col("source_content_digest"),
        col("source_block").alias("controller_block"),
    ]
}

fn rust_control_edge_projection(alias: &'static str) -> [Expr; 7] {
    [
        qualified_programmatic(alias, "provider_run_id").alias("provider_run_id"),
        qualified_programmatic(alias, "compilation_unit_id").alias("compilation_unit_id"),
        qualified_programmatic(alias, "owner_id").alias("owner_id"),
        qualified_programmatic(alias, "source_generation").alias("source_generation"),
        qualified_programmatic(alias, "source_file_id").alias("source_file_id"),
        qualified_programmatic(alias, "source_content_digest").alias("source_content_digest"),
        qualified_programmatic(alias, "source_block").alias("controller_block"),
    ]
}

fn rust_mir_structural_dependencies(
    role: RustMirDerivedRelation,
    bindings: &RustMirAnalysisBindings,
) -> Arc<[ProgrammaticRelationId]> {
    let raw = |relation: RustcRelation| ProgrammaticRelationId::new(relation.relation_id());
    let derived = |relation: RustMirDerivedRelation| {
        ProgrammaticRelationId::new(bindings.relation_id(relation).as_str())
    };
    match role {
        RustMirDerivedRelation::OwnershipState => Arc::from([
            raw(RustcRelation::MirLocal),
            raw(RustcRelation::MirPlace),
            raw(RustcRelation::Access),
            raw(RustcRelation::Coverage),
            raw(RustcRelation::Remainder),
        ]),
        RustMirDerivedRelation::AliasPointsTo => Arc::from([
            raw(RustcRelation::MirPlace),
            raw(RustcRelation::MirRvalue),
            raw(RustcRelation::MirStatement),
            raw(RustcRelation::Access),
        ]),
        RustMirDerivedRelation::ResourceLifecycle => Arc::from([
            raw(RustcRelation::MirPlace),
            raw(RustcRelation::Access),
            raw(RustcRelation::MirTerminator),
            derived(RustMirDerivedRelation::CfgEdge),
            raw(RustcRelation::Call),
            raw(RustcRelation::Instance),
        ]),
        RustMirDerivedRelation::AsyncLowering => Arc::from([
            raw(RustcRelation::MirBody),
            raw(RustcRelation::MirRvalue),
            raw(RustcRelation::MirStatement),
            raw(RustcRelation::MirTerminator),
            derived(RustMirDerivedRelation::CfgEdge),
        ]),
        RustMirDerivedRelation::UnsafeFfi => Arc::from([
            raw(RustcRelation::PublicItem),
            raw(RustcRelation::Type),
            raw(RustcRelation::MirRvalue),
            raw(RustcRelation::MirStatement),
            raw(RustcRelation::MirTerminator),
            raw(RustcRelation::Call),
            raw(RustcRelation::Instance),
            raw(RustcRelation::Access),
        ]),
        _ => Arc::from([]),
    }
}

fn rust_mir_place_location_plan(
    input: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let place = LogicalPlanBuilder::from(input)
        .alias(RUST_STRUCTURAL_PLACE_ALIAS)?
        .build()?;
    let component = rust_mir_projection_component_udf().call(vec![
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "projection_ordinal"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "projection_kind"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "projection_local_or_field"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "offset"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "min_length"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "slice_to"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "from_end"),
        qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "projection_type_key"),
    ]);
    let components = LogicalPlanBuilder::from(place)
        .project([
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "provider_run_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "compilation_unit_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "owner_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "source_generation"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "source_file_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "source_content_digest"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "stable_crate_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "def_path_hash"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "place_id"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "base_local"),
            qualified_programmatic(RUST_STRUCTURAL_PLACE_ALIAS, "projection_ordinal"),
            component.alias(RUST_PROJECTION_COMPONENT),
        ])?
        .build()?;
    let projection_path = cast(
        string_agg(col(RUST_PROJECTION_COMPONENT), lit("/"))
            .order_by(vec![col("projection_ordinal").sort(true, false)])
            .build()?,
        DataType::Utf8,
    );
    let grouped = LogicalPlanBuilder::from(components)
        .aggregate(
            [
                col("provider_run_id"),
                col("compilation_unit_id"),
                col("owner_id"),
                col("source_generation"),
                col("source_file_id"),
                col("source_content_digest"),
                col("stable_crate_id"),
                col("def_path_hash"),
                col("place_id"),
                col("base_local"),
            ],
            [projection_path.alias("projection_path")],
        )?
        .build()?;
    let memory_location = rust_mir_memory_location_identity_udf().call(vec![
        col("stable_crate_id"),
        col("def_path_hash"),
        col("base_local"),
        col("projection_path"),
    ]);
    Ok(LogicalPlanBuilder::from(grouped)
        .project([
            col("provider_run_id"),
            col("compilation_unit_id"),
            col("owner_id"),
            col("source_generation"),
            col("source_file_id"),
            col("source_content_digest"),
            col("stable_crate_id"),
            col("def_path_hash"),
            col("place_id"),
            col("base_local"),
            col("projection_path"),
            memory_location.alias("memory_location_id"),
            col("stable_crate_id")
                .is_not_null()
                .and(col("def_path_hash").is_not_null())
                .alias("canonical_identity_available"),
        ])?
        .build()?)
}

fn rust_owner_presence(
    input: LogicalPlan,
    alias: &'static str,
) -> Result<LogicalPlan, TransformationPlanError> {
    let input = LogicalPlanBuilder::from(input).alias(alias)?.build()?;
    Ok(LogicalPlanBuilder::from(input)
        .project([
            qualified_programmatic(alias, "provider_run_id"),
            qualified_programmatic(alias, "compilation_unit_id"),
            qualified_programmatic(alias, "owner_id"),
            qualified_programmatic(alias, "source_generation"),
            qualified_programmatic(alias, "source_file_id"),
            qualified_programmatic(alias, "source_content_digest"),
        ])?
        .distinct()?
        .alias(alias)?
        .build()?)
}

fn rust_ownership_access_filter(alias: &'static str) -> Expr {
    let kind = qualified_programmatic(alias, "access_kind");
    [
        "BorrowShared",
        "BorrowFake",
        "BorrowMut",
        "ReborrowShared",
        "ReborrowMut",
        "Move",
        "Copy",
        "CopyForDeref",
        "StorageLive",
        "StorageDead",
        "Drop",
        "AddressOfMut",
        "AddressOfConst",
        "AddressOfMetadata",
    ]
    .into_iter()
    .map(|value| kind.clone().eq(lit(value)))
    .reduce(Expr::or)
    .expect("ownership access vocabulary is non-empty")
}

fn rust_ownership_observation(alias: &'static str) -> Expr {
    let kind = qualified_programmatic(alias, "access_kind");
    datafusion::logical_expr::expr_fn::when(
        kind.clone()
            .eq(lit("BorrowShared"))
            .or(kind.clone().eq(lit("BorrowFake"))),
        lit("SHARED_BORROW_OBSERVED"),
    )
    .when(
        kind.clone().eq(lit("BorrowMut")),
        lit("MUTABLE_BORROW_OBSERVED"),
    )
    .when(
        kind.clone().eq(lit("ReborrowShared")),
        lit("SHARED_REBORROW_OBSERVED"),
    )
    .when(
        kind.clone().eq(lit("ReborrowMut")),
        lit("MUTABLE_REBORROW_OBSERVED"),
    )
    .when(kind.clone().eq(lit("Move")), lit("MOVE_OBSERVED"))
    .when(
        kind.clone()
            .eq(lit("Copy"))
            .or(kind.clone().eq(lit("CopyForDeref"))),
        lit("COPY_OBSERVED"),
    )
    .when(
        kind.clone().eq(lit("StorageLive")),
        lit("STORAGE_LIVE_OBSERVED"),
    )
    .when(
        kind.clone().eq(lit("StorageDead")),
        lit("STORAGE_DEAD_OBSERVED"),
    )
    .when(kind.clone().eq(lit("Drop")), lit("DROP_OBSERVED"))
    .otherwise(lit("RAW_ADDRESS_OBSERVED"))
    .expect("literal ownership CASE expression is valid")
}

fn rust_resource_access_filter(alias: &'static str) -> Expr {
    let kind = qualified_programmatic(alias, "access_kind");
    kind.clone()
        .eq(lit("StorageLive"))
        .or(kind.clone().eq(lit("StorageDead")))
        .or(kind.eq(lit("Drop")))
}

fn rust_resource_lifecycle(alias: &'static str) -> Expr {
    let kind = qualified_programmatic(alias, "access_kind");
    datafusion::logical_expr::expr_fn::when(
        kind.clone().eq(lit("StorageLive")),
        lit("STORAGE_LIVE"),
    )
    .when(kind.clone().eq(lit("StorageDead")), lit("STORAGE_DEAD"))
    .otherwise(lit("DROP_EXECUTED"))
    .expect("literal resource CASE expression is valid")
}

fn rust_unsafe_cast_filter(cast_kind: Expr) -> Expr {
    [
        "PointerExposeAddress",
        "PointerWithExposedProvenance",
        "PtrToPtr",
        "FnPtrToPtr",
        "Transmute",
        "BoxDerefTransmute",
    ]
    .into_iter()
    .map(|value| cast_kind.clone().eq(lit(value)))
    .reduce(Expr::or)
    .expect("unsafe cast vocabulary is non-empty")
}

fn rust_structural_event_sort() -> [SortExpr; 5] {
    [
        col("owner_id").sort(true, false),
        col("block_index").sort(true, false),
        col("slot_kind").sort(true, false),
        col("slot_index").sort(true, false),
        col("place_id").sort(true, false),
    ]
}

#[allow(clippy::too_many_arguments)]
fn rust_unsafe_projection(
    source_alias: &'static str,
    block_index: Expr,
    slot_kind: Expr,
    slot_index: Expr,
    source_scope: Expr,
    observation_kind: Expr,
    raw_kind: Expr,
    declared_target: Expr,
    resolved_instance_key: Expr,
    is_foreign_item: Expr,
    structured_evidence: Expr,
) -> Vec<Expr> {
    // Provider relations attach different field-identity metadata to their native kind
    // columns.  A union must not let the first provider branch silently lend that identity to
    // the others, so route the unchanged native value through an application-owned scalar
    // expression whose output field has application metadata rather than provider metadata.
    let raw_kind = rust_mir_native_kind_udf().call(vec![raw_kind]);
    let observation_id = rust_mir_unsafe_identity_udf().call(vec![
        qualified_programmatic(source_alias, "stable_crate_id"),
        qualified_programmatic(source_alias, "def_path_hash"),
        block_index.clone(),
        slot_kind.clone(),
        slot_index.clone(),
        observation_kind.clone(),
        raw_kind.clone(),
        resolved_instance_key.clone(),
    ]);
    vec![
        cast(
            qualified_programmatic(source_alias, "provider_run_id"),
            DataType::Utf8,
        )
        .alias("provider_run_id"),
        cast(
            qualified_programmatic(source_alias, "compilation_unit_id"),
            DataType::Utf8,
        )
        .alias("compilation_unit_id"),
        cast(
            qualified_programmatic(source_alias, "owner_id"),
            DataType::Utf8,
        )
        .alias("owner_id"),
        cast(
            qualified_programmatic(source_alias, "source_generation"),
            DataType::UInt64,
        )
        .alias("source_generation"),
        cast(
            qualified_programmatic(source_alias, "source_file_id"),
            DataType::Utf8,
        )
        .alias("source_file_id"),
        cast(
            qualified_programmatic(source_alias, "source_content_digest"),
            DataType::FixedSizeBinary(32),
        )
        .alias("source_content_digest"),
        cast(
            qualified_programmatic(source_alias, "stable_crate_id"),
            DataType::UInt64,
        )
        .alias("stable_crate_id"),
        cast(
            qualified_programmatic(source_alias, "def_path_hash"),
            DataType::FixedSizeBinary(16),
        )
        .alias("def_path_hash"),
        qualified_programmatic(source_alias, "stable_crate_id")
            .is_not_null()
            .and(qualified_programmatic(source_alias, "def_path_hash").is_not_null())
            .alias("canonical_identity_available"),
        observation_id.alias("observation_id"),
        cast(block_index, DataType::UInt64).alias("block_index"),
        cast(slot_kind, DataType::Utf8).alias("slot_kind"),
        cast(slot_index, DataType::UInt64).alias("slot_index"),
        cast(source_scope, DataType::UInt64).alias("source_scope"),
        cast(observation_kind, DataType::Utf8).alias("observation_kind"),
        cast(raw_kind, DataType::Utf8).alias("raw_kind"),
        cast(declared_target, DataType::Utf8).alias("declared_target"),
        cast(resolved_instance_key, DataType::FixedSizeBinary(32)).alias("resolved_instance_key"),
        cast(is_foreign_item, DataType::Boolean).alias("is_foreign_item"),
        cast(structured_evidence, DataType::Utf8).alias("structured_evidence"),
    ]
}

fn rust_mir_native_kind_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_native_kind_v3",
        vec![DataType::Utf8],
        DataType::Utf8,
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            Ok(ColumnarValue::Array(Arc::clone(&arrays[0])))
        }),
    ))
}

fn rust_unsafe_output_columns(alias: &'static str) -> Vec<Expr> {
    [
        "provider_run_id",
        "compilation_unit_id",
        "owner_id",
        "source_generation",
        "source_file_id",
        "source_content_digest",
        "stable_crate_id",
        "def_path_hash",
        "canonical_identity_available",
        "observation_id",
        "block_index",
        "slot_kind",
        "slot_index",
        "source_scope",
        "observation_kind",
        "raw_kind",
        "declared_target",
        "resolved_instance_key",
        "is_foreign_item",
        "structured_evidence",
    ]
    .into_iter()
    .map(|name| qualified_programmatic(alias, name))
    .collect()
}

fn python_flow_event_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_flow_event_identity_v3",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::UInt64,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let epoch = fixed_array(&arrays[0], 16, "fabric_epoch_id")?;
            let source = fixed_array(&arrays[1], 32, "source_pin")?;
            let context = fixed_array(&arrays[2], 32, "analysis_context_id")?;
            let owner = fixed_array(&arrays[3], 16, "owner_id")?;
            let occurrence = fixed_array(&arrays[4], 16, "occurrence_id")?;
            let start = u64_array(&arrays[5], "start_byte")?;
            let end = u64_array(&arrays[6], "end_byte")?;
            let role = string_array(&arrays[7], "event_role")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(epoch.len(), 16);
            for row in 0..epoch.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-flow-event.v3");
                for part in [
                    epoch.value(row),
                    source.value(row),
                    context.value(row),
                    owner.value(row),
                    occurrence.value(row),
                    &start.value(row).to_be_bytes(),
                    &end.value(row).to_be_bytes(),
                    role.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(&hasher.finalize().as_bytes()[..16])?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn python_flow_location_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_flow_location_identity_v3",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let epoch = fixed_array(&arrays[0], 16, "fabric_epoch_id")?;
            let source = fixed_array(&arrays[1], 32, "source_pin")?;
            let context = fixed_array(&arrays[2], 32, "analysis_context_id")?;
            let owner = fixed_array(&arrays[3], 16, "owner_id")?;
            let scope = fixed_array(&arrays[4], 16, "scope_id")?;
            let name = string_array(&arrays[5], "binding_name")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(epoch.len(), 16);
            for row in 0..epoch.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher =
                    blake3::Hasher::new_derive_key("codefabric.python-flow-location.v3");
                for part in [
                    epoch.value(row),
                    source.value(row),
                    context.value(row),
                    owner.value(row),
                    scope.value(row),
                    name.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(&hasher.finalize().as_bytes()[..16])?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn python_flow_relation_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_flow_relation_identity_v3",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let epoch = fixed_array(&arrays[0], 16, "fabric_epoch_id")?;
            let source = fixed_array(&arrays[1], 32, "source_pin")?;
            let context = fixed_array(&arrays[2], 32, "analysis_context_id")?;
            let owner = fixed_array(&arrays[3], 16, "owner_id")?;
            let predecessor = fixed_array(&arrays[4], 16, "predecessor_id")?;
            let successor = fixed_array(&arrays[5], 16, "successor_id")?;
            let witness = fixed_array(&arrays[6], 16, "witness_id")?;
            let kind = string_array(&arrays[7], "relation_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(epoch.len(), 16);
            for row in 0..epoch.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher =
                    blake3::Hasher::new_derive_key("codefabric.python-flow-relation.v3");
                for part in [
                    epoch.value(row),
                    source.value(row),
                    context.value(row),
                    owner.value(row),
                    predecessor.value(row),
                    successor.value(row),
                    witness.value(row),
                    kind.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(&hasher.finalize().as_bytes()[..16])?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn python_cfg_node_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_cfg_node_identity_v3",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::UInt64,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let epoch = fixed_array(&arrays[0], 16, "fabric_epoch_id")?;
            let source = fixed_array(&arrays[1], 32, "content_digest")?;
            let context = fixed_array(&arrays[2], 32, "analysis_context_id")?;
            let owner = fixed_array(&arrays[3], 16, "file_id")?;
            let start = u64_array(&arrays[4], "start_byte")?;
            let end = u64_array(&arrays[5], "end_byte")?;
            let kind = string_array(&arrays[6], "raw_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(epoch.len(), 16);
            for row in 0..epoch.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher =
                    blake3::Hasher::new_derive_key("codefabric.python-programmatic-cfg-node.v3");
                for part in [
                    epoch.value(row),
                    source.value(row),
                    context.value(row),
                    owner.value(row),
                    &start.value(row).to_be_bytes(),
                    &end.value(row).to_be_bytes(),
                    kind.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(&hasher.finalize().as_bytes()[..16])?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn python_cfg_edge_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_cfg_edge_identity_v3",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let epoch = fixed_array(&arrays[0], 16, "fabric_epoch_id")?;
            let source = fixed_array(&arrays[1], 32, "content_digest")?;
            let context = fixed_array(&arrays[2], 32, "analysis_context_id")?;
            let owner = fixed_array(&arrays[3], 16, "file_id")?;
            let source_node = fixed_array(&arrays[4], 16, "source_node_id")?;
            let target_node = fixed_array(&arrays[5], 16, "target_node_id")?;
            let kind = string_array(&arrays[6], "edge_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(epoch.len(), 16);
            for row in 0..epoch.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher =
                    blake3::Hasher::new_derive_key("codefabric.python-programmatic-cfg-edge.v3");
                for part in [
                    epoch.value(row),
                    source.value(row),
                    context.value(row),
                    owner.value(row),
                    source_node.value(row),
                    target_node.value(row),
                    kind.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(&hasher.finalize().as_bytes()[..16])?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_cfg_edge_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_cfg_edge_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::UInt64,
            DataType::Utf8,
            DataType::Utf8,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let source = u64_array(&arrays[2], "source_block")?;
            let target = u64_array(&arrays[3], "target_block")?;
            let kind = string_array(&arrays[4], "edge_kind")?;
            let branch = string_array(&arrays[5], "branch_value_u128")?;
            let unwind = string_array(&arrays[6], "unwind_action")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if stable_crate.is_null(row) || path.is_null(row) {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-derived-cfg-edge.v1\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    source.value(row).to_be_bytes().as_slice(),
                    target.value(row).to_be_bytes().as_slice(),
                    kind.value(row).as_bytes(),
                    (!branch.is_null(row))
                        .then(|| branch.value(row).as_bytes())
                        .unwrap_or_default(),
                    (!unwind.is_null(row))
                        .then(|| unwind.value(row).as_bytes())
                        .unwrap_or_default(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_control_input_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_control_input_identity_v3",
        vec![
            DataType::Utf8,
            DataType::UInt64,
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let owner = string_array(&arrays[0], "owner_id")?;
            let block = u64_array(&arrays[1], "controller_block")?;
            let edge = fixed_array(&arrays[2], 32, "edge_id")?;
            let predicate = fixed_array(&arrays[3], 32, "predicate_operand_id")?;
            let kind = string_array(&arrays[4], "controller_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(owner.len(), 32);
            for row in 0..owner.len() {
                if owner.is_null(row)
                    || block.is_null(row)
                    || edge.is_null(row)
                    || kind.is_null(row)
                {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-control-input.v3\0");
                for part in [
                    owner.value(row).as_bytes(),
                    block.value(row).to_be_bytes().as_slice(),
                    edge.value(row),
                    (!predicate.is_null(row))
                        .then(|| predicate.value(row))
                        .unwrap_or_default(),
                    kind.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_projection_component_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_projection_component_v3",
        vec![
            DataType::UInt64,
            DataType::Utf8,
            DataType::UInt64,
            DataType::UInt64,
            DataType::UInt64,
            DataType::UInt64,
            DataType::Boolean,
            DataType::FixedSizeBinary(32),
        ],
        DataType::Utf8,
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let ordinal = u64_array(&arrays[0], "projection_ordinal")?;
            let kind = string_array(&arrays[1], "projection_kind")?;
            let local_or_field = u64_array(&arrays[2], "projection_local_or_field")?;
            let offset = u64_array(&arrays[3], "offset")?;
            let min_length = u64_array(&arrays[4], "min_length")?;
            let slice_to = u64_array(&arrays[5], "slice_to")?;
            let from_end = boolean_array(&arrays[6], "from_end")?;
            let type_key = fixed_array(&arrays[7], 32, "projection_type_key")?;
            let mut builder = StringBuilder::with_capacity(kind.len(), kind.len() * 64);
            for row in 0..kind.len() {
                if kind.is_null(row) {
                    builder.append_null();
                    continue;
                }
                let number = |array: &UInt64Array| {
                    (!array.is_null(row))
                        .then(|| array.value(row).to_string())
                        .unwrap_or_else(|| "-".to_owned())
                };
                let ordinal = (!ordinal.is_null(row))
                    .then(|| ordinal.value(row).to_string())
                    .unwrap_or_else(|| "base".to_owned());
                let from_end = (!from_end.is_null(row))
                    .then(|| if from_end.value(row) { "1" } else { "0" })
                    .unwrap_or("-");
                let type_key = (!type_key.is_null(row))
                    .then(|| hex_bytes(type_key.value(row)))
                    .unwrap_or_else(|| "-".to_owned());
                builder.append_value(format!(
                    "{ordinal}:{}:{}:{}:{}:{}:{from_end}:{type_key}",
                    kind.value(row),
                    number(local_or_field),
                    number(offset),
                    number(min_length),
                    number(slice_to),
                ));
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_memory_location_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_memory_location_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let base_local = u64_array(&arrays[2], "base_local")?;
            let projection = string_array(&arrays[3], "projection_path")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if stable_crate.is_null(row)
                    || path.is_null(row)
                    || base_local.is_null(row)
                    || projection.is_null(row)
                {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-memory-location.v1\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    base_local.value(row).to_be_bytes().as_slice(),
                    projection.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_access_event_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_access_event_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::Utf8,
            DataType::UInt64,
            DataType::UInt64,
            DataType::FixedSizeBinary(32),
            DataType::Utf8,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let block = u64_array(&arrays[2], "block_index")?;
            let slot_kind = string_array(&arrays[3], "slot_kind")?;
            let slot_index = u64_array(&arrays[4], "slot_index")?;
            let ordinal = u64_array(&arrays[5], "access_ordinal")?;
            let place = fixed_array(&arrays[6], 32, "place_id")?;
            let access_kind = string_array(&arrays[7], "access_kind")?;
            let evidence = string_array(&arrays[8], "structured_evidence")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-access-event.v3\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    block.value(row).to_be_bytes().as_slice(),
                    slot_kind.value(row).as_bytes(),
                    slot_index.value(row).to_be_bytes().as_slice(),
                    ordinal.value(row).to_be_bytes().as_slice(),
                    place.value(row),
                    access_kind.value(row).as_bytes(),
                    evidence.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_alias_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_alias_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::UInt64,
            DataType::FixedSizeBinary(32),
            DataType::FixedSizeBinary(32),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let block = u64_array(&arrays[2], "block_index")?;
            let statement = u64_array(&arrays[3], "statement_index")?;
            let destination = fixed_array(&arrays[4], 32, "pointer_place_id")?;
            let source = fixed_array(&arrays[5], 32, "pointee_place_id")?;
            let kind = string_array(&arrays[6], "rvalue_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-alias-observation.v3\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    block.value(row).to_be_bytes().as_slice(),
                    statement.value(row).to_be_bytes().as_slice(),
                    destination.value(row),
                    source.value(row),
                    kind.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_async_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_async_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::UInt64,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let block = u64_array(&arrays[2], "block_index")?;
            let statement = u64_array(&arrays[3], "statement_index")?;
            let aggregate = string_array(&arrays[4], "aggregate_kind")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-async-lowering.v3\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    block.value(row).to_be_bytes().as_slice(),
                    statement.value(row).to_be_bytes().as_slice(),
                    aggregate.value(row).as_bytes(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn rust_mir_unsafe_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_mir_unsafe_identity_v3",
        vec![
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::Utf8,
            DataType::UInt64,
            DataType::Utf8,
            DataType::Utf8,
            DataType::FixedSizeBinary(32),
        ],
        DataType::FixedSizeBinary(32),
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let stable_crate = u64_array(&arrays[0], "stable_crate_id")?;
            let path = fixed_array(&arrays[1], 16, "def_path_hash")?;
            let block = u64_array(&arrays[2], "block_index")?;
            let slot_kind = string_array(&arrays[3], "slot_kind")?;
            let slot_index = u64_array(&arrays[4], "slot_index")?;
            let observation_kind = string_array(&arrays[5], "observation_kind")?;
            let raw_kind = string_array(&arrays[6], "raw_kind")?;
            let instance = fixed_array(&arrays[7], 32, "resolved_instance_key")?;
            let mut builder = FixedSizeBinaryBuilder::with_capacity(stable_crate.len(), 32);
            for row in 0..stable_crate.len() {
                if stable_crate.is_null(row)
                    || path.is_null(row)
                    || block.is_null(row)
                    || slot_kind.is_null(row)
                    || slot_index.is_null(row)
                    || observation_kind.is_null(row)
                    || raw_kind.is_null(row)
                {
                    builder.append_null();
                    continue;
                }
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"codefabric.rust-mir-unsafe-observation.v3\0");
                for part in [
                    stable_crate.value(row).to_be_bytes().as_slice(),
                    path.value(row),
                    block.value(row).to_be_bytes().as_slice(),
                    slot_kind.value(row).as_bytes(),
                    slot_index.value(row).to_be_bytes().as_slice(),
                    observation_kind.value(row).as_bytes(),
                    raw_kind.value(row).as_bytes(),
                    (!instance.is_null(row))
                        .then(|| instance.value(row))
                        .unwrap_or_default(),
                ] {
                    frame(&mut hasher, part);
                }
                builder.append_value(hasher.finalize().as_bytes())?;
            }
            Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
        }),
    ))
}

fn common_call_site_identity_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_common_call_site_identity_v3",
        vec![
            DataType::Utf8,
            DataType::FixedSizeBinary(32),
            DataType::UInt64,
            DataType::UInt64,
            DataType::UInt64,
        ],
        DataType::Utf8,
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let module = string_array(&arrays[0], "module_id")?;
            let content = fixed_array(&arrays[1], 32, "content_digest")?;
            let start = u64_array(&arrays[2], "start_byte")?;
            let end = u64_array(&arrays[3], "end_byte")?;
            let occurrence = u64_array(&arrays[4], "call_occurrence_ordinal")?;
            let values = (0..module.len())
                .map(|row| {
                    if arrays.iter().any(|array| array.is_null(row)) {
                        return None;
                    }
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(b"codefabric.common-programmatic-call-site.v3\0");
                    for part in [
                        module.value(row).as_bytes(),
                        content.value(row),
                        start.value(row).to_be_bytes().as_slice(),
                        end.value(row).to_be_bytes().as_slice(),
                        occurrence.value(row).to_be_bytes().as_slice(),
                    ] {
                        frame(&mut hasher, part);
                    }
                    Some(format!("b3:{}", hasher.finalize().to_hex()))
                })
                .collect::<Vec<_>>();
            Ok(ColumnarValue::Array(
                Arc::new(StringArray::from(values)) as ArrayRef
            ))
        }),
    ))
}

fn fixed_array<'a>(
    array: &'a ArrayRef,
    width: i32,
    name: &'static str,
) -> datafusion::common::Result<&'a FixedSizeBinaryArray> {
    array
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .filter(|array| array.value_length() == width)
        .ok_or_else(|| {
            datafusion::error::DataFusionError::Execution(format!(
                "programmatic identity UDF expected fixed[{width}] {name}"
            ))
        })
}

fn u64_array<'a>(
    array: &'a ArrayRef,
    name: &'static str,
) -> datafusion::common::Result<&'a UInt64Array> {
    array.as_any().downcast_ref::<UInt64Array>().ok_or_else(|| {
        datafusion::error::DataFusionError::Execution(format!(
            "programmatic identity UDF expected uint64 {name}"
        ))
    })
}

fn string_array<'a>(
    array: &'a ArrayRef,
    name: &'static str,
) -> datafusion::common::Result<&'a StringArray> {
    array.as_any().downcast_ref::<StringArray>().ok_or_else(|| {
        datafusion::error::DataFusionError::Execution(format!(
            "programmatic identity UDF expected utf8 {name}"
        ))
    })
}

fn boolean_array<'a>(
    array: &'a ArrayRef,
    name: &'static str,
) -> datafusion::common::Result<&'a BooleanArray> {
    array
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| {
            datafusion::error::DataFusionError::Execution(format!(
                "programmatic identity UDF expected boolean {name}"
            ))
        })
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Source of one exact producer input authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedInputAuthoritySource {
    Provider(ProviderNativeLane),
    Derived(DerivedFamilyId),
    /// Unavailable application input retained by an explicit remainder declaration.
    ///
    /// The family identifies the upstream remainder when that relation has one unique declared
    /// owner. Otherwise it identifies the dependent remainder whose exact contract is the only
    /// current authority for the intended input. This is dependency evidence, not a claim that
    /// the semantic input or output table exists.
    DeclaredRemainder(DerivedFamilyId),
}

/// One exact ordered input entry retained in the composition observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedInputObservation {
    pub relation_id: ProgrammaticRelationId,
    pub source: DerivedInputAuthoritySource,
    pub authority_identity: [u8; 32],
    pub completeness: DerivedInputCompleteness,
}

/// Execution-derived completeness of one direct input relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DerivedInputCompleteness {
    Complete,
    Partial,
    Unknown,
}

/// Causal observation of one accepted producer registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundDerivedProducerObservation {
    pub family_id: DerivedFamilyId,
    pub domain: DerivedAnalysisDomain,
    pub output_relation_id: ProgrammaticRelationId,
    pub algorithm: DerivedAlgorithmContract,
    pub precision: DerivedPrecisionPolicy,
    pub completeness: DerivedCompletenessPolicy,
    pub witness_field_id: ProgrammaticFieldId,
    pub inputs: Arc<[DerivedInputObservation]>,
    pub input_vector_identity: [u8; 32],
    pub provenance_closure_identity: [u8; 32],
    pub resource_class: TransformationResourceClass,
}

/// Causal observation of one explicit unsupported family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundDerivedRemainderObservation {
    pub family_id: DerivedFamilyId,
    pub domain: DerivedAnalysisDomain,
    pub algorithm: DerivedAlgorithmContract,
    pub precision: DerivedPrecisionPolicy,
    pub inputs: Arc<[DerivedInputObservation]>,
    pub input_vector_identity: [u8; 32],
    pub reason: DerivedRemainderReason,
    pub evidence_identity: [u8; 32],
    pub retryability: DerivedRemainderRetryability,
    pub provenance_closure_identity: [u8; 32],
}

/// Complete typed composition observation retained with the returned builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAnalysisCompositionObservation {
    pub provider_authority_identity: [u8; 32],
    pub closure_identity: [u8; 32],
    pub resources: DerivedAnalysisResourceObservation,
    pub producers: Vec<BoundDerivedProducerObservation>,
    pub remainders: Vec<BoundDerivedRemainderObservation>,
    pub remainder_relation_id: ProgrammaticRelationId,
}

/// Atomic result: the builder is available only after all producers and remainders register.
pub struct ProgrammaticDerivedAnalysisOutcome {
    builder: ProgrammaticFabricEpochBuilder,
    provider_reports: ExactProgrammaticProviderReports,
    observation: DerivedAnalysisCompositionObservation,
}

impl ProgrammaticDerivedAnalysisOutcome {
    #[must_use]
    pub const fn provider_reports(&self) -> &ExactProgrammaticProviderReports {
        &self.provider_reports
    }

    #[must_use]
    pub const fn observation(&self) -> &DerivedAnalysisCompositionObservation {
        &self.observation
    }

    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProgrammaticFabricEpochBuilder,
        ExactProgrammaticProviderReports,
        DerivedAnalysisCompositionObservation,
    ) {
        (self.builder, self.provider_reports, self.observation)
    }
}

/// Admit all four exact provider lanes and compose all accepted derived families in one call.
///
/// Both stages consume their candidate. A provider or later producer failure returns no builder,
/// so a partially registered session cannot escape.
pub(crate) fn admit_and_compose_programmatic_derived_analyses(
    authority: &CompiledTransformationAuthority,
    builder: ProgrammaticFabricEpochBuilder,
    runs: ExactProgrammaticProviderRuns<'_>,
    composition: ProgrammaticDerivedAnalysisComposition,
) -> Result<ProgrammaticDerivedAnalysisOutcome, ProgrammaticDerivedAnalysisError> {
    let admitted = admit_provider_relations_programmatic(builder, runs)?;
    compose_programmatic_derived_analyses(authority, admitted, composition)
}

/// Bind accepted Python, Rust MIR, and common producers into an admitted candidate.
pub(crate) fn compose_programmatic_derived_analyses(
    authority: &CompiledTransformationAuthority,
    admitted: ProgrammaticProviderAdmissionOutcome,
    composition: ProgrammaticDerivedAnalysisComposition,
) -> Result<ProgrammaticDerivedAnalysisOutcome, ProgrammaticDerivedAnalysisError> {
    debug_assert_eq!(*authority, composition.release_authority);
    let (mut builder, provider_reports) = admitted.into_parts();
    let (mut relation_authorities, provider_authority_identity) =
        provider_relation_authorities(&provider_reports)?;
    let ProgrammaticDerivedAnalysisComposition {
        release_authority: _,
        families,
        dispositions,
        metadata,
        remainder_relation,
        resource_envelope,
    } = composition;

    let mut families_by_id = BTreeMap::new();
    let mut domains = BTreeSet::new();
    for family in families.iter().cloned() {
        domains.insert(family.domain);
        if families_by_id
            .insert(family.family_id.clone(), family.clone())
            .is_some()
        {
            return Err(ProgrammaticDerivedAnalysisError::DuplicateAcceptedFamily(
                family.family_id,
            ));
        }
    }
    for required in [
        DerivedAnalysisDomain::Python,
        DerivedAnalysisDomain::RustMir,
        DerivedAnalysisDomain::Common,
    ] {
        if !domains.contains(&required) {
            return Err(ProgrammaticDerivedAnalysisError::MissingAnalysisDomain(
                required,
            ));
        }
    }

    let mut dispositions_by_id = BTreeMap::new();
    for disposition in dispositions {
        let family_id = disposition.family_id().clone();
        if !families_by_id.contains_key(&family_id) {
            return Err(ProgrammaticDerivedAnalysisError::UndeclaredFamilyDisposition(family_id));
        }
        if dispositions_by_id
            .insert(family_id.clone(), disposition)
            .is_some()
        {
            return Err(ProgrammaticDerivedAnalysisError::DuplicateFamilyDisposition(family_id));
        }
    }
    for family_id in families_by_id.keys() {
        if !dispositions_by_id.contains_key(family_id) {
            return Err(ProgrammaticDerivedAnalysisError::MissingFamilyDisposition(
                family_id.clone(),
            ));
        }
    }

    validate_remainder_output_binding(&metadata, &remainder_relation)?;
    let resources = validate_composition_resource_envelope(
        &families_by_id,
        &dispositions_by_id,
        &remainder_relation,
        resource_envelope,
    )?;
    let produced_family_ids = dispositions_by_id
        .iter()
        .filter_map(|(family_id, disposition)| {
            matches!(disposition, DerivedFamilyDisposition::Producer(_)).then(|| family_id.clone())
        })
        .collect::<BTreeSet<_>>();
    // Remainder-only semantic families may share the physical relation they would inhabit after
    // migration (the current common analysis multiplexes ten fact families into one table). Only
    // executable producers own catalog output, so only producer/producer collisions are illegal.
    let mut output_owners = BTreeMap::new();
    for family_id in &produced_family_ids {
        let family = &families_by_id[family_id];
        if let Some(existing) =
            output_owners.insert(family.output_relation_id.clone(), family.family_id.clone())
        {
            return Err(ProgrammaticDerivedAnalysisError::DuplicateFamilyOutput {
                relation_id: family.output_relation_id.clone(),
                first: existing,
                second: family.family_id.clone(),
            });
        }
    }
    let producer_order = producer_topological_order(
        &families_by_id,
        &dispositions_by_id,
        &output_owners,
        &relation_authorities,
    )?;
    let mut producer_observations = Vec::new();

    for family_id in producer_order {
        let family = families_by_id
            .get(&family_id)
            .expect("topological family exists");
        let DerivedFamilyDisposition::Producer(producer) = dispositions_by_id
            .remove(&family_id)
            .expect("topological producer disposition exists")
        else {
            unreachable!("only producers enter the topological order")
        };
        validate_producer(
            family,
            &producer,
            &families_by_id,
            &produced_family_ids,
            &metadata,
        )?;
        let inputs = resolve_inputs(family, &relation_authorities, true)?;
        if producer.completeness == DerivedCompletenessPolicy::Complete
            && inputs
                .iter()
                .any(|input| input.completeness != DerivedInputCompleteness::Complete)
        {
            return Err(
                ProgrammaticDerivedAnalysisError::CompleteProducerHasIncompleteInput {
                    family_id: family_id.clone(),
                },
            );
        }
        let input_vector_identity = input_vector_identity(&inputs);
        let authority_identity = match producer.authority {
            DerivedProducerAuthority::ApplicationOwned(identity) if identity != [0; 32] => identity,
            DerivedProducerAuthority::ApplicationOwned(_) => {
                return Err(ProgrammaticDerivedAnalysisError::SentinelApplicationAuthority);
            }
            DerivedProducerAuthority::ProviderNative(lane) => {
                return Err(
                    ProgrammaticDerivedAnalysisError::ProviderOwnedDerivedFamily {
                        family_id: family_id.clone(),
                        lane,
                    },
                );
            }
        };
        let precision_identity = precision_identity(&producer.precision);
        let provenance_closure_identity = producer_provenance_identity(
            family,
            &producer,
            authority_identity,
            input_vector_identity,
            precision_identity,
        );
        let wrapped = Arc::new(MetadataBoundDerivedTransformation::try_new(
            producer.transformation,
            &metadata,
            MetadataValues {
                family_identity: family_identity(&family_id),
                domain: domain_code(family.domain),
                authority_identity,
                algorithm_identity: algorithm_identity(&family.algorithm),
                semantic_version: family.algorithm.semantic_version(),
                release_identity: *family.algorithm.release_identity().as_bytes(),
                precision_identity,
                input_vector_identity,
                completeness: completeness_code(&producer.completeness),
                provenance_closure_identity,
            },
            provenance_closure_identity,
        )?);
        let resource_class = wrapped.contract().resource_class();
        builder.add_transformation(wrapped)?;

        let completeness = producer_output_completeness(&producer.completeness);
        relation_authorities.insert(
            family.output_relation_id.clone(),
            RelationAuthority {
                observation: DerivedInputObservation {
                    relation_id: family.output_relation_id.clone(),
                    source: DerivedInputAuthoritySource::Derived(family_id.clone()),
                    authority_identity: provenance_closure_identity,
                    completeness,
                },
                available: true,
            },
        );
        producer_observations.push(BoundDerivedProducerObservation {
            family_id: family_id.clone(),
            domain: family.domain,
            output_relation_id: family.output_relation_id.clone(),
            algorithm: family.algorithm.clone(),
            precision: producer.precision,
            completeness: producer.completeness,
            witness_field_id: producer.witness_field_id,
            inputs: inputs.into(),
            input_vector_identity,
            provenance_closure_identity,
            resource_class,
        });
    }

    extend_declared_remainder_input_authorities(
        &families_by_id,
        &dispositions_by_id,
        &mut relation_authorities,
    );
    let mut remainder_observations = Vec::new();
    for (family_id, disposition) in dispositions_by_id {
        let DerivedFamilyDisposition::Remainder(remainder) = disposition else {
            unreachable!("all producers were removed in topological order")
        };
        let family = families_by_id
            .get(&family_id)
            .expect("remainder family exists");
        if remainder.algorithm != family.algorithm {
            return Err(
                ProgrammaticDerivedAnalysisError::UndeclaredRemainderAlgorithm { family_id },
            );
        }
        let inputs = resolve_remainder_inputs(family, &remainder, &relation_authorities);
        let input_vector_identity = input_vector_identity(&inputs);
        let provenance_closure_identity = remainder_provenance_identity(
            family,
            &remainder,
            input_vector_identity,
            remainder_relation.authority_identity,
        );
        remainder_observations.push(BoundDerivedRemainderObservation {
            family_id: family.family_id.clone(),
            domain: family.domain,
            algorithm: family.algorithm.clone(),
            precision: family.precision.clone(),
            inputs: inputs.into(),
            input_vector_identity,
            reason: remainder.reason,
            evidence_identity: remainder.evidence_identity,
            retryability: remainder.retryability,
            provenance_closure_identity,
        });
    }
    remainder_observations.sort_by(|left, right| left.family_id.cmp(&right.family_id));

    let remainder_transformation = Arc::new(DerivedRemainderTransformation::try_new(
        remainder_relation,
        metadata,
        &remainder_observations,
        provider_authority_identity,
    )?);
    let remainder_relation_id = remainder_transformation.output().relation_id().clone();
    let remainder_relation_authority_identity =
        remainder_transformation.contract().authority_identity();
    builder.add_transformation(remainder_transformation)?;

    producer_observations.sort_by(|left, right| left.family_id.cmp(&right.family_id));
    let closure_identity = composition_closure_identity(
        provider_authority_identity,
        &producer_observations,
        &remainder_observations,
        &remainder_relation_id,
        remainder_relation_authority_identity,
        resources,
    );
    Ok(ProgrammaticDerivedAnalysisOutcome {
        builder,
        provider_reports,
        observation: DerivedAnalysisCompositionObservation {
            provider_authority_identity,
            closure_identity,
            resources,
            producers: producer_observations,
            remainders: remainder_observations,
            remainder_relation_id,
        },
    })
}

fn validate_composition_resource_envelope(
    families: &BTreeMap<DerivedFamilyId, AcceptedDerivedFamily>,
    dispositions: &BTreeMap<DerivedFamilyId, DerivedFamilyDisposition>,
    remainder_relation: &DerivedRemainderRelationBinding,
    envelope: DerivedAnalysisResourceEnvelope,
) -> Result<DerivedAnalysisResourceObservation, ProgrammaticDerivedAnalysisError> {
    let mut observation = DerivedAnalysisResourceObservation {
        envelope,
        producer_count: 0,
        remainder_count: 0,
        dependency_edge_count: 0,
        declared_max_rows: 0,
        declared_max_memory_bytes: 0,
        declared_max_spill_bytes: 0,
    };

    for family in families.values() {
        observation.dependency_edge_count = checked_resource_add(
            "dependency_edge_count",
            observation.dependency_edge_count,
            u64::try_from(family.dependencies().len()).unwrap_or(u64::MAX),
        )?;
    }
    for (family_id, disposition) in dispositions {
        match disposition {
            DerivedFamilyDisposition::Producer(producer) => {
                observation.producer_count =
                    checked_resource_add("producer_count", observation.producer_count, 1)?;
                accumulate_resource_class(
                    &mut observation,
                    producer.transformation.contract().resource_class(),
                    family_id.as_str(),
                )?;
            }
            DerivedFamilyDisposition::Remainder(_) => {
                observation.remainder_count =
                    checked_resource_add("remainder_count", observation.remainder_count, 1)?;
            }
        }
    }
    // The explicit-remainder relation is an installed native transformation even when the current
    // epoch has no remainder rows, so its contract belongs to the aggregate declared budget.
    accumulate_resource_class(
        &mut observation,
        remainder_relation.contract.resource_class(),
        remainder_relation.output.relation_id().as_str(),
    )?;

    for (resource, observed, maximum) in [
        (
            "producer_count",
            observation.producer_count,
            envelope.producer_limit,
        ),
        (
            "remainder_count",
            observation.remainder_count,
            envelope.remainder_limit,
        ),
        (
            "dependency_edge_count",
            observation.dependency_edge_count,
            envelope.dependency_edge_limit,
        ),
        (
            "declared_max_rows",
            observation.declared_max_rows,
            envelope.declared_row_limit,
        ),
        (
            "declared_max_memory_bytes",
            observation.declared_max_memory_bytes,
            envelope.declared_memory_byte_limit,
        ),
        (
            "declared_max_spill_bytes",
            observation.declared_max_spill_bytes,
            envelope.declared_spill_byte_limit,
        ),
    ] {
        if observed > maximum {
            return Err(
                ProgrammaticDerivedAnalysisError::CompositionResourceLimitExceeded {
                    resource,
                    observed,
                    maximum,
                },
            );
        }
    }
    Ok(observation)
}

fn accumulate_resource_class(
    observation: &mut DerivedAnalysisResourceObservation,
    resource_class: TransformationResourceClass,
    subject: &str,
) -> Result<(), ProgrammaticDerivedAnalysisError> {
    for (resource, value) in [
        ("max_rows", resource_class.max_rows()),
        ("max_memory_bytes", resource_class.max_memory_bytes()),
    ] {
        if value == 0 {
            return Err(
                ProgrammaticDerivedAnalysisError::ZeroTransformationResourceBound {
                    subject: subject.to_owned(),
                    resource,
                },
            );
        }
    }
    if resource_class.max_spill_bytes() == Some(0) {
        return Err(
            ProgrammaticDerivedAnalysisError::ZeroTransformationResourceBound {
                subject: subject.to_owned(),
                resource: "max_spill_bytes",
            },
        );
    }
    observation.declared_max_rows = checked_resource_add(
        "declared_max_rows",
        observation.declared_max_rows,
        resource_class.max_rows(),
    )?;
    observation.declared_max_memory_bytes = checked_resource_add(
        "declared_max_memory_bytes",
        observation.declared_max_memory_bytes,
        resource_class.max_memory_bytes(),
    )?;
    observation.declared_max_spill_bytes = checked_resource_add(
        "declared_max_spill_bytes",
        observation.declared_max_spill_bytes,
        resource_class.max_spill_bytes().unwrap_or(0),
    )?;
    Ok(())
}

fn checked_resource_add(
    resource: &'static str,
    left: u64,
    right: u64,
) -> Result<u64, ProgrammaticDerivedAnalysisError> {
    left.checked_add(right)
        .ok_or(ProgrammaticDerivedAnalysisError::CompositionResourceCounterOverflow(resource))
}

#[derive(Clone)]
struct RelationAuthority {
    observation: DerivedInputObservation,
    available: bool,
}

fn provider_relation_authorities(
    reports: &ExactProgrammaticProviderReports,
) -> Result<
    (
        BTreeMap<ProgrammaticRelationId, RelationAuthority>,
        [u8; 32],
    ),
    ProgrammaticDerivedAnalysisError,
> {
    let mut authorities = BTreeMap::new();
    let mut total = blake3::Hasher::new();
    total.update(b"codefabric.derived.provider-authority-vector.v1");
    for (lane, report) in [
        (ProviderNativeLane::TreeSitter, reports.tree_sitter()),
        (ProviderNativeLane::Ruff, reports.ruff()),
        (ProviderNativeLane::Pyrefly, reports.pyrefly()),
        (ProviderNativeLane::Rustc, reports.rustc()),
    ] {
        for relation in &report.relations {
            let relation_id = ProgrammaticRelationId::new(relation.provider_relation.as_str());
            let completeness = disposition_completeness(&relation.disposition);
            let available = matches!(
                relation.disposition,
                ProviderRegistrationDisposition::Registered { .. }
                    | ProviderRegistrationDisposition::RegisteredUnknown { .. }
            );
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"codefabric.derived.provider-relation-authority.v1");
            frame(&mut hasher, relation_id.as_str().as_bytes());
            hasher.update(&[lane_code(lane)]);
            hasher.update(&report.boundary.contract_id.0);
            hasher.update(&report.boundary.contract_revision.to_be_bytes());
            hasher.update(&report.boundary.installer_id.0);
            hasher.update(&report.boundary.provider_revision.provider_id.0);
            frame(
                &mut hasher,
                report.boundary.provider_revision.release.as_bytes(),
            );
            hasher.update(&report.boundary.provider_revision.source_revision);
            hasher.update(&report.boundary.source_pin.0);
            hasher.update(&report.boundary.context_pin.0);
            frame(&mut hasher, relation.api_family.as_str().as_bytes());
            frame_disposition(&mut hasher, &relation.disposition);
            let identity = *hasher.finalize().as_bytes();
            frame(&mut total, relation_id.as_str().as_bytes());
            total.update(&identity);
            if authorities
                .insert(
                    relation_id.clone(),
                    RelationAuthority {
                        observation: DerivedInputObservation {
                            relation_id: relation_id.clone(),
                            source: DerivedInputAuthoritySource::Provider(lane),
                            authority_identity: identity,
                            completeness,
                        },
                        available,
                    },
                )
                .is_some()
            {
                return Err(
                    ProgrammaticDerivedAnalysisError::DuplicateProviderInputAuthority(relation_id),
                );
            }
        }
    }
    Ok((authorities, *total.finalize().as_bytes()))
}

fn disposition_completeness(
    disposition: &ProviderRegistrationDisposition,
) -> DerivedInputCompleteness {
    match disposition {
        ProviderRegistrationDisposition::Registered { coverage, .. } => {
            terminal_completeness(*coverage)
        }
        ProviderRegistrationDisposition::RegisteredUnknown { .. }
        | ProviderRegistrationDisposition::Unknown { .. } => DerivedInputCompleteness::Unknown,
        ProviderRegistrationDisposition::Remainder { trailer } => {
            terminal_completeness(trailer.status)
        }
    }
}

const fn terminal_completeness(status: TerminalStatus) -> DerivedInputCompleteness {
    match status {
        TerminalStatus::Complete => DerivedInputCompleteness::Complete,
        TerminalStatus::Partial => DerivedInputCompleteness::Partial,
        TerminalStatus::Unknown => DerivedInputCompleteness::Unknown,
    }
}

fn producer_topological_order(
    families: &BTreeMap<DerivedFamilyId, AcceptedDerivedFamily>,
    dispositions: &BTreeMap<DerivedFamilyId, DerivedFamilyDisposition>,
    output_owners: &BTreeMap<ProgrammaticRelationId, DerivedFamilyId>,
    provider_authorities: &BTreeMap<ProgrammaticRelationId, RelationAuthority>,
) -> Result<Vec<DerivedFamilyId>, ProgrammaticDerivedAnalysisError> {
    let producer_ids = dispositions
        .iter()
        .filter_map(|(family_id, disposition)| {
            matches!(disposition, DerivedFamilyDisposition::Producer(_)).then(|| family_id.clone())
        })
        .collect::<BTreeSet<_>>();
    let mut indegree = producer_ids
        .iter()
        .cloned()
        .map(|family_id| (family_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<DerivedFamilyId, BTreeSet<DerivedFamilyId>>::new();
    for family_id in &producer_ids {
        let family = &families[family_id];
        for dependency in family.dependencies() {
            if let Some(owner) = output_owners.get(dependency) {
                if !producer_ids.contains(owner) {
                    return Err(ProgrammaticDerivedAnalysisError::DependencyOnRemainder {
                        family_id: family_id.clone(),
                        dependency: dependency.clone(),
                    });
                }
                if dependents
                    .entry(owner.clone())
                    .or_default()
                    .insert(family_id.clone())
                {
                    *indegree.get_mut(family_id).expect("producer has indegree") += 1;
                }
            } else if !provider_authorities.contains_key(dependency) {
                return Err(ProgrammaticDerivedAnalysisError::OrphanDependency {
                    family_id: family_id.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(family_id, count)| (*count == 0).then(|| family_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(producer_ids.len());
    while let Some(family_id) = ready.pop_first() {
        order.push(family_id.clone());
        if let Some(children) = dependents.get(&family_id) {
            for child in children {
                let count = indegree.get_mut(child).expect("dependent producer exists");
                *count -= 1;
                if *count == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if order.len() != producer_ids.len() {
        return Err(
            ProgrammaticDerivedAnalysisError::CyclicProducerDependencies(
                indegree
                    .into_iter()
                    .filter_map(|(family_id, count)| (count != 0).then_some(family_id))
                    .collect(),
            ),
        );
    }
    Ok(order)
}

fn validate_producer(
    family: &AcceptedDerivedFamily,
    producer: &AcceptedDerivedProducer,
    families: &BTreeMap<DerivedFamilyId, AcceptedDerivedFamily>,
    produced_family_ids: &BTreeSet<DerivedFamilyId>,
    metadata: &DerivedMetadataBindings,
) -> Result<(), ProgrammaticDerivedAnalysisError> {
    if producer.algorithm != family.algorithm || producer.precision != family.precision {
        return Err(
            ProgrammaticDerivedAnalysisError::UndeclaredProducerAlgorithm {
                family_id: family.family_id.clone(),
            },
        );
    }
    let transformation = &producer.transformation;
    if transformation.contract().semantic_id() != family.algorithm.semantic_id()
        || transformation.contract().semantic_version() != family.algorithm.semantic_version()
        || transformation.contract().provenance().release_identity()
            != family.algorithm.release_identity()
    {
        return Err(
            ProgrammaticDerivedAnalysisError::TransformationAlgorithmDrift {
                family_id: family.family_id.clone(),
            },
        );
    }
    if transformation.output().relation_id() != &family.output_relation_id {
        return Err(ProgrammaticDerivedAnalysisError::ProducerOutputMismatch {
            family_id: family.family_id.clone(),
            expected: family.output_relation_id.clone(),
            actual: transformation.output().relation_id().clone(),
        });
    }
    if transformation.dependencies() != family.dependencies() {
        return Err(
            ProgrammaticDerivedAnalysisError::ProducerInputVectorMismatch {
                family_id: family.family_id.clone(),
            },
        );
    }
    if transformation.contract().determinism_policy() == TransformationDeterminismPolicy::Volatile {
        return Err(ProgrammaticDerivedAnalysisError::VolatileDerivedProducer {
            family_id: family.family_id.clone(),
        });
    }
    if let TransformationRecursionPolicy::Bounded { max_iterations } =
        transformation.contract().recursion_policy()
    {
        return Err(
            ProgrammaticDerivedAnalysisError::BoundedNativeRecursionUnavailable {
                family_id: family.family_id.clone(),
                max_iterations,
            },
        );
    }
    if !transformation
        .output()
        .fields()
        .iter()
        .any(|field| field.field_id() == &producer.witness_field_id)
    {
        return Err(ProgrammaticDerivedAnalysisError::MissingProducerWitness {
            family_id: family.family_id.clone(),
            field_id: producer.witness_field_id.clone(),
        });
    }
    let original_fields = transformation
        .output()
        .fields()
        .iter()
        .map(|field| field.field_id())
        .collect::<BTreeSet<_>>();
    for column in metadata.ordered() {
        if original_fields.contains(&column.field_id) {
            return Err(
                ProgrammaticDerivedAnalysisError::ProducerMetadataFieldCollision {
                    family_id: family.family_id.clone(),
                    field_id: column.field_id.clone(),
                },
            );
        }
    }
    match &producer.completeness {
        DerivedCompletenessPolicy::Complete => {}
        DerivedCompletenessPolicy::Partial { unknown_family }
        | DerivedCompletenessPolicy::Unknown { unknown_family } => {
            let Some(unknown) = families.get(unknown_family) else {
                return Err(ProgrammaticDerivedAnalysisError::MissingUnknownFamily {
                    family_id: family.family_id.clone(),
                    unknown_family: unknown_family.clone(),
                });
            };
            if unknown.domain != family.domain || unknown.kind != DerivedFamilyKind::UnknownEvidence
            {
                return Err(ProgrammaticDerivedAnalysisError::InvalidUnknownFamily {
                    family_id: family.family_id.clone(),
                    unknown_family: unknown_family.clone(),
                });
            }
            if !produced_family_ids.contains(unknown_family) {
                return Err(ProgrammaticDerivedAnalysisError::UnknownFamilyNotProduced {
                    family_id: family.family_id.clone(),
                    unknown_family: unknown_family.clone(),
                });
            }
        }
    }
    Ok(())
}

fn resolve_inputs(
    family: &AcceptedDerivedFamily,
    authorities: &BTreeMap<ProgrammaticRelationId, RelationAuthority>,
    require_available: bool,
) -> Result<Vec<DerivedInputObservation>, ProgrammaticDerivedAnalysisError> {
    family
        .dependencies()
        .iter()
        .map(|relation_id| {
            let authority = authorities.get(relation_id).ok_or_else(|| {
                ProgrammaticDerivedAnalysisError::OrphanDependency {
                    family_id: family.family_id.clone(),
                    dependency: relation_id.clone(),
                }
            })?;
            if require_available && !authority.available {
                return Err(ProgrammaticDerivedAnalysisError::UnavailableProducerInput {
                    family_id: family.family_id.clone(),
                    dependency: relation_id.clone(),
                });
            }
            Ok(authority.observation.clone())
        })
        .collect()
}

fn resolve_remainder_inputs(
    family: &AcceptedDerivedFamily,
    remainder: &ExplicitDerivedRemainder,
    authorities: &BTreeMap<ProgrammaticRelationId, RelationAuthority>,
) -> Vec<DerivedInputObservation> {
    family
        .dependencies()
        .iter()
        .map(|relation_id| {
            authorities
                .get(relation_id)
                .map(|authority| authority.observation.clone())
                .unwrap_or_else(|| DerivedInputObservation {
                    relation_id: relation_id.clone(),
                    source: DerivedInputAuthoritySource::DeclaredRemainder(
                        family.family_id.clone(),
                    ),
                    authority_identity: declared_remainder_dependency_identity(
                        family,
                        remainder,
                        relation_id,
                    ),
                    completeness: DerivedInputCompleteness::Unknown,
                })
        })
        .collect()
}

fn declared_remainder_dependency_identity(
    family: &AcceptedDerivedFamily,
    remainder: &ExplicitDerivedRemainder,
    relation_id: &ProgrammaticRelationId,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.declared-derived-remainder-dependency.v1");
    frame(&mut hasher, family.family_id.as_str().as_bytes());
    frame(&mut hasher, relation_id.as_str().as_bytes());
    frame(
        &mut hasher,
        family.algorithm.semantic_id().as_str().as_bytes(),
    );
    let version = family.algorithm.semantic_version();
    hasher.update(&version.major().to_be_bytes());
    hasher.update(&version.minor().to_be_bytes());
    hasher.update(&version.patch().to_be_bytes());
    hasher.update(family.algorithm.release_identity().as_bytes());
    hasher.update(&precision_identity(&family.precision));
    hasher.update(&[remainder_reason_code(remainder.reason)]);
    hasher.update(&remainder.evidence_identity);
    hasher.update(&[retryability_code(remainder.retryability)]);
    *hasher.finalize().as_bytes()
}

fn extend_declared_remainder_input_authorities(
    families: &BTreeMap<DerivedFamilyId, AcceptedDerivedFamily>,
    dispositions: &BTreeMap<DerivedFamilyId, DerivedFamilyDisposition>,
    authorities: &mut BTreeMap<ProgrammaticRelationId, RelationAuthority>,
) {
    let mut candidates =
        BTreeMap::<ProgrammaticRelationId, Vec<(DerivedFamilyId, [u8; 32])>>::new();
    for (family_id, disposition) in dispositions {
        let DerivedFamilyDisposition::Remainder(remainder) = disposition else {
            continue;
        };
        let family = &families[family_id];
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric.declared-derived-remainder-input.v1");
        frame(&mut hasher, family.family_id.as_str().as_bytes());
        frame(
            &mut hasher,
            family.algorithm.semantic_id().as_str().as_bytes(),
        );
        let version = family.algorithm.semantic_version();
        hasher.update(&version.major().to_be_bytes());
        hasher.update(&version.minor().to_be_bytes());
        hasher.update(&version.patch().to_be_bytes());
        hasher.update(family.algorithm.release_identity().as_bytes());
        hasher.update(&precision_identity(&family.precision));
        hasher.update(&[remainder_reason_code(remainder.reason)]);
        hasher.update(&remainder.evidence_identity);
        hasher.update(&[retryability_code(remainder.retryability)]);
        candidates
            .entry(family.output_relation_id.clone())
            .or_default()
            .push((family_id.clone(), *hasher.finalize().as_bytes()));
    }
    for (relation_id, candidates) in candidates {
        let [(family_id, authority_identity)] = candidates.as_slice() else {
            // Several remainder families may intentionally share a future multiplexed table.
            // Such a relation is not a unique dependency authority until a real producer exists.
            continue;
        };
        authorities
            .entry(relation_id.clone())
            .or_insert_with(|| RelationAuthority {
                observation: DerivedInputObservation {
                    relation_id,
                    source: DerivedInputAuthoritySource::DeclaredRemainder(family_id.clone()),
                    authority_identity: *authority_identity,
                    completeness: DerivedInputCompleteness::Unknown,
                },
                available: false,
            });
    }
}

fn validate_remainder_output_binding(
    metadata: &DerivedMetadataBindings,
    binding: &DerivedRemainderRelationBinding,
) -> Result<(), ProgrammaticDerivedAnalysisError> {
    if binding.contract.determinism_policy() != TransformationDeterminismPolicy::DeterministicSet
        || binding.contract.ordering_policy() != &TransformationOrderingPolicy::Unordered
        || binding.contract.recursion_policy() != TransformationRecursionPolicy::Forbidden
    {
        return Err(ProgrammaticDerivedAnalysisError::InvalidRemainderExecutionContract);
    }
    let expected = metadata
        .ordered()
        .map(|column| &column.field_id)
        .chain(
            DerivedRemainderMetadataRole::ALL
                .into_iter()
                .map(|role| &binding.column(role).field_id),
        )
        .collect::<Vec<_>>();
    let actual = binding
        .output
        .fields()
        .iter()
        .map(TransformationFieldIdentity::field_id)
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(ProgrammaticDerivedAnalysisError::RemainderOutputFieldMismatch);
    }
    let mut names = metadata
        .ordered()
        .map(|column| Arc::clone(&column.physical_name))
        .collect::<BTreeSet<_>>();
    for role in DerivedRemainderMetadataRole::ALL {
        let column = binding.column(role);
        if !names.insert(Arc::clone(&column.physical_name)) {
            return Err(ProgrammaticDerivedAnalysisError::DuplicateMetadataColumn(
                Arc::clone(&column.physical_name),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct MetadataValues {
    family_identity: [u8; 32],
    domain: u8,
    authority_identity: [u8; 32],
    algorithm_identity: [u8; 32],
    semantic_version: TransformationSemanticVersion,
    release_identity: [u8; 32],
    precision_identity: [u8; 32],
    input_vector_identity: [u8; 32],
    completeness: u8,
    provenance_closure_identity: [u8; 32],
}

struct MetadataBoundDerivedTransformation {
    inner: Arc<dyn ProgrammaticTransformation>,
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    metadata: DerivedMetadataBindings,
    values: MetadataValues,
}

impl MetadataBoundDerivedTransformation {
    fn try_new(
        inner: Arc<dyn ProgrammaticTransformation>,
        metadata: &DerivedMetadataBindings,
        values: MetadataValues,
        provenance_closure_identity: [u8; 32],
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let inner_contract = inner.contract();
        let contract = ProgrammaticTransformationContract::new(
            inner_contract.semantic_id().clone(),
            inner_contract.semantic_version(),
            inner_contract.resource_class(),
            inner_contract.determinism_policy(),
            inner_contract.ordering_policy().clone(),
            inner_contract.recursion_policy(),
            TransformationProvenance::new(
                TransformationProvenanceIdentity::from_bytes(provenance_closure_identity),
                inner_contract.provenance().release_identity(),
            ),
        );
        let mut fields = inner.output().fields().to_vec();
        fields.extend(
            metadata
                .ordered()
                .map(|column| TransformationFieldIdentity::new(column.field_id.clone())),
        );
        let mut output = TransformationOutput::new(
            inner.output().relation_id().clone(),
            inner.output().table_reference().clone(),
            fields,
        );
        if let Some(assertion) = inner.output().schema_assertion() {
            let mut assertion_fields = assertion.fields().to_vec();
            for column in metadata.ordered() {
                let metadata = HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    column.field_id.as_str().to_owned(),
                )]);
                assertion_fields.push(Arc::new(
                    Field::new(
                        column.physical_name.as_ref(),
                        metadata_type(column.role),
                        false,
                    )
                    .with_metadata(metadata),
                ));
            }
            output = output.with_schema_assertion(Arc::new(
                Schema::new(assertion_fields).with_metadata(assertion.metadata().clone()),
            ));
        }
        Ok(Self {
            inner,
            contract,
            output,
            metadata: metadata.clone(),
            values,
        })
    }
}

impl ProgrammaticTransformation for MetadataBoundDerivedTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        self.inner.dependencies()
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let plan = self.inner.build(inputs)?;
        let mut projection = plan
            .schema()
            .iter()
            .map(|(qualifier, field)| {
                Expr::Column(Column::new(qualifier.cloned(), field.name().clone()))
            })
            .collect::<Vec<_>>();
        projection.extend(metadata_expressions(&self.metadata, self.values));
        Ok(LogicalPlanBuilder::from(plan)
            .project(projection)?
            .build()?)
    }
}

struct DerivedRemainderTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    metadata: DerivedMetadataBindings,
    columns: BTreeMap<DerivedRemainderMetadataRole, DerivedRemainderMetadataColumnBinding>,
    rows: Arc<[RemainderRow]>,
}

#[derive(Clone, Copy)]
struct RemainderRow {
    metadata: MetadataValues,
    reason: u8,
    evidence_identity: [u8; 32],
    retryability: u8,
}

impl DerivedRemainderTransformation {
    fn try_new(
        binding: DerivedRemainderRelationBinding,
        metadata: DerivedMetadataBindings,
        observations: &[BoundDerivedRemainderObservation],
        provider_authority_identity: [u8; 32],
    ) -> Result<Self, ProgrammaticDerivedAnalysisError> {
        let observed_rows = u64::try_from(observations.len()).unwrap_or(u64::MAX);
        if observed_rows > binding.contract.resource_class().max_rows() {
            return Err(
                ProgrammaticDerivedAnalysisError::RemainderRowBoundExceeded {
                    observed: observed_rows,
                    maximum: binding.contract.resource_class().max_rows(),
                },
            );
        }
        let mut provenance = blake3::Hasher::new();
        provenance.update(b"codefabric.derived.remainder-relation.v1");
        provenance.update(&binding.contract.authority_identity());
        provenance.update(&provider_authority_identity);
        let rows = observations
            .iter()
            .map(|observation| {
                provenance.update(&observation.provenance_closure_identity);
                RemainderRow {
                    metadata: MetadataValues {
                        family_identity: family_identity(&observation.family_id),
                        domain: domain_code(observation.domain),
                        authority_identity: binding.authority_identity,
                        algorithm_identity: algorithm_identity(&observation.algorithm),
                        semantic_version: observation.algorithm.semantic_version(),
                        release_identity: *observation.algorithm.release_identity().as_bytes(),
                        precision_identity: precision_identity(&observation.precision),
                        input_vector_identity: observation.input_vector_identity,
                        completeness: 3,
                        provenance_closure_identity: observation.provenance_closure_identity,
                    },
                    reason: remainder_reason_code(observation.reason),
                    evidence_identity: observation.evidence_identity,
                    retryability: retryability_code(observation.retryability),
                }
            })
            .collect::<Vec<_>>();
        let provenance_identity = *provenance.finalize().as_bytes();
        let base = &binding.contract;
        let contract = ProgrammaticTransformationContract::new(
            base.semantic_id().clone(),
            base.semantic_version(),
            base.resource_class(),
            base.determinism_policy(),
            base.ordering_policy().clone(),
            base.recursion_policy(),
            TransformationProvenance::new(
                TransformationProvenanceIdentity::from_bytes(provenance_identity),
                base.provenance().release_identity(),
            ),
        );
        Ok(Self {
            contract,
            output: binding.output,
            metadata,
            columns: binding.columns,
            rows: rows.into(),
        })
    }

    fn row_expressions(&self, row: RemainderRow) -> Vec<Expr> {
        let mut expressions = metadata_expressions(&self.metadata, row.metadata);
        expressions.extend([
            Expr::Literal(ScalarValue::UInt8(Some(row.reason)), None).alias(
                self.columns[&DerivedRemainderMetadataRole::Reason]
                    .physical_name
                    .as_ref(),
            ),
            Expr::Literal(
                ScalarValue::FixedSizeBinary(32, Some(row.evidence_identity.to_vec())),
                None,
            )
            .alias(
                self.columns[&DerivedRemainderMetadataRole::EvidenceIdentity]
                    .physical_name
                    .as_ref(),
            ),
            Expr::Literal(ScalarValue::UInt8(Some(row.retryability)), None).alias(
                self.columns[&DerivedRemainderMetadataRole::Retryability]
                    .physical_name
                    .as_ref(),
            ),
        ]);
        expressions
    }
}

impl ProgrammaticTransformation for DerivedRemainderTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &[]
    }

    fn build(
        &self,
        _inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let placeholder = RemainderRow {
            metadata: MetadataValues {
                family_identity: [0; 32],
                domain: 0,
                authority_identity: [0; 32],
                algorithm_identity: [0; 32],
                semantic_version: TransformationSemanticVersion::new(0, 0, 0),
                release_identity: [0; 32],
                precision_identity: [0; 32],
                input_vector_identity: [0; 32],
                completeness: 3,
                provenance_closure_identity: [0; 32],
            },
            reason: 0,
            evidence_identity: [0; 32],
            retryability: 0,
        };
        let mut plan = LogicalPlanBuilder::empty(false)
            .project(self.row_expressions(placeholder))?
            .build()?;
        for row in self.rows.iter().copied() {
            let next = LogicalPlanBuilder::empty(true)
                .project(self.row_expressions(row))?
                .build()?;
            plan = LogicalPlanBuilder::from(plan).union(next)?.build()?;
        }
        Ok(plan)
    }
}

fn metadata_expressions(bindings: &DerivedMetadataBindings, values: MetadataValues) -> Vec<Expr> {
    DerivedMetadataRole::ALL
        .into_iter()
        .map(|role| {
            let value = match role {
                DerivedMetadataRole::FamilyIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.family_identity.to_vec()))
                }
                DerivedMetadataRole::Domain => ScalarValue::UInt8(Some(values.domain)),
                DerivedMetadataRole::AuthorityIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.authority_identity.to_vec()))
                }
                DerivedMetadataRole::AlgorithmIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.algorithm_identity.to_vec()))
                }
                DerivedMetadataRole::AlgorithmVersionMajor => {
                    ScalarValue::UInt16(Some(values.semantic_version.major()))
                }
                DerivedMetadataRole::AlgorithmVersionMinor => {
                    ScalarValue::UInt16(Some(values.semantic_version.minor()))
                }
                DerivedMetadataRole::AlgorithmVersionPatch => {
                    ScalarValue::UInt16(Some(values.semantic_version.patch()))
                }
                DerivedMetadataRole::ReleaseIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.release_identity.to_vec()))
                }
                DerivedMetadataRole::PrecisionIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.precision_identity.to_vec()))
                }
                DerivedMetadataRole::InputVectorIdentity => {
                    ScalarValue::FixedSizeBinary(32, Some(values.input_vector_identity.to_vec()))
                }
                DerivedMetadataRole::CompletenessState => {
                    ScalarValue::UInt8(Some(values.completeness))
                }
                DerivedMetadataRole::ProvenanceClosureIdentity => ScalarValue::FixedSizeBinary(
                    32,
                    Some(values.provenance_closure_identity.to_vec()),
                ),
            };
            Expr::Literal(value, None).alias(bindings.column(role).physical_name.as_ref())
        })
        .collect()
}

const fn metadata_type(role: DerivedMetadataRole) -> DataType {
    match role {
        DerivedMetadataRole::Domain | DerivedMetadataRole::CompletenessState => DataType::UInt8,
        DerivedMetadataRole::AlgorithmVersionMajor
        | DerivedMetadataRole::AlgorithmVersionMinor
        | DerivedMetadataRole::AlgorithmVersionPatch => DataType::UInt16,
        DerivedMetadataRole::FamilyIdentity
        | DerivedMetadataRole::AuthorityIdentity
        | DerivedMetadataRole::AlgorithmIdentity
        | DerivedMetadataRole::ReleaseIdentity
        | DerivedMetadataRole::PrecisionIdentity
        | DerivedMetadataRole::InputVectorIdentity
        | DerivedMetadataRole::ProvenanceClosureIdentity => DataType::FixedSizeBinary(32),
    }
}

fn input_vector_identity(inputs: &[DerivedInputObservation]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.exact-input-vector.v1");
    hasher.update(
        &u64::try_from(inputs.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for input in inputs {
        frame(&mut hasher, input.relation_id.as_str().as_bytes());
        hasher.update(&input.authority_identity);
        hasher.update(&[input_completeness_code(input.completeness)]);
        match &input.source {
            DerivedInputAuthoritySource::Provider(lane) => {
                hasher.update(&[0, lane_code(*lane)]);
            }
            DerivedInputAuthoritySource::Derived(family_id) => {
                hasher.update(&[1]);
                frame(&mut hasher, family_id.as_str().as_bytes());
            }
            DerivedInputAuthoritySource::DeclaredRemainder(family_id) => {
                hasher.update(&[2]);
                frame(&mut hasher, family_id.as_str().as_bytes());
            }
        }
    }
    *hasher.finalize().as_bytes()
}

fn producer_provenance_identity(
    family: &AcceptedDerivedFamily,
    producer: &AcceptedDerivedProducer,
    authority_identity: [u8; 32],
    input_vector_identity: [u8; 32],
    precision_identity: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.producer-provenance.v1");
    frame(&mut hasher, family.family_id.as_str().as_bytes());
    hasher.update(&[domain_code(family.domain), family_kind_code(family.kind)]);
    hasher.update(&authority_identity);
    hasher.update(&algorithm_identity(&family.algorithm));
    hasher.update(family.algorithm.release_identity().as_bytes());
    hasher.update(&precision_identity);
    hasher.update(&input_vector_identity);
    frame(&mut hasher, producer.witness_field_id.as_str().as_bytes());
    frame_completeness(&mut hasher, &producer.completeness);
    hasher.update(&producer.transformation.contract().authority_identity());
    *hasher.finalize().as_bytes()
}

fn remainder_provenance_identity(
    family: &AcceptedDerivedFamily,
    remainder: &ExplicitDerivedRemainder,
    input_vector_identity: [u8; 32],
    authority_identity: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.explicit-remainder.v1");
    frame(&mut hasher, family.family_id.as_str().as_bytes());
    hasher.update(&[domain_code(family.domain), family_kind_code(family.kind)]);
    hasher.update(&authority_identity);
    hasher.update(&algorithm_identity(&family.algorithm));
    hasher.update(family.algorithm.release_identity().as_bytes());
    hasher.update(&precision_identity(&family.precision));
    hasher.update(&input_vector_identity);
    hasher.update(&[remainder_reason_code(remainder.reason)]);
    hasher.update(&remainder.evidence_identity);
    hasher.update(&[retryability_code(remainder.retryability)]);
    *hasher.finalize().as_bytes()
}

fn composition_closure_identity(
    provider_authority_identity: [u8; 32],
    producers: &[BoundDerivedProducerObservation],
    remainders: &[BoundDerivedRemainderObservation],
    remainder_relation_id: &ProgrammaticRelationId,
    remainder_relation_authority_identity: [u8; 32],
    resources: DerivedAnalysisResourceObservation,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.composition-closure.v1");
    hasher.update(&provider_authority_identity);
    frame(&mut hasher, remainder_relation_id.as_str().as_bytes());
    hasher.update(&remainder_relation_authority_identity);
    for value in [
        resources.envelope.producer_limit,
        resources.envelope.remainder_limit,
        resources.envelope.dependency_edge_limit,
        resources.envelope.declared_row_limit,
        resources.envelope.declared_memory_byte_limit,
        resources.envelope.declared_spill_byte_limit,
        resources.producer_count,
        resources.remainder_count,
        resources.dependency_edge_count,
        resources.declared_max_rows,
        resources.declared_max_memory_bytes,
        resources.declared_max_spill_bytes,
    ] {
        hasher.update(&value.to_be_bytes());
    }
    hasher.update(
        &u64::try_from(producers.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for producer in producers {
        frame(&mut hasher, producer.family_id.as_str().as_bytes());
        frame(&mut hasher, producer.output_relation_id.as_str().as_bytes());
        frame(&mut hasher, producer.witness_field_id.as_str().as_bytes());
        hasher.update(&producer.input_vector_identity);
        hasher.update(&producer.provenance_closure_identity);
        frame_resource_class(&mut hasher, producer.resource_class);
    }
    hasher.update(
        &u64::try_from(remainders.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for remainder in remainders {
        frame(&mut hasher, remainder.family_id.as_str().as_bytes());
        hasher.update(&remainder.input_vector_identity);
        hasher.update(&remainder.provenance_closure_identity);
        hasher.update(&[remainder_reason_code(remainder.reason)]);
        hasher.update(&remainder.evidence_identity);
        hasher.update(&[retryability_code(remainder.retryability)]);
    }
    *hasher.finalize().as_bytes()
}

fn frame_resource_class(hasher: &mut blake3::Hasher, resource: TransformationResourceClass) {
    match resource {
        TransformationResourceClass::BoundedInMemory {
            max_rows,
            max_memory_bytes,
        } => {
            hasher.update(&[0]);
            hasher.update(&max_rows.to_be_bytes());
            hasher.update(&max_memory_bytes.to_be_bytes());
        }
        TransformationResourceClass::BoundedSpillable {
            max_rows,
            max_memory_bytes,
            max_spill_bytes,
        } => {
            hasher.update(&[1]);
            hasher.update(&max_rows.to_be_bytes());
            hasher.update(&max_memory_bytes.to_be_bytes());
            hasher.update(&max_spill_bytes.to_be_bytes());
        }
    }
}

fn algorithm_identity(contract: &DerivedAlgorithmContract) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.algorithm.v1");
    frame(&mut hasher, contract.semantic_id().as_str().as_bytes());
    hasher.update(&contract.semantic_version().major().to_be_bytes());
    hasher.update(&contract.semantic_version().minor().to_be_bytes());
    hasher.update(&contract.semantic_version().patch().to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn family_identity(family_id: &DerivedFamilyId) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.family.v1");
    frame(&mut hasher, family_id.as_str().as_bytes());
    *hasher.finalize().as_bytes()
}

fn precision_identity(precision: &DerivedPrecisionPolicy) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derived.precision.v1");
    match precision {
        DerivedPrecisionPolicy::Exact => {
            hasher.update(&[0]);
        }
        DerivedPrecisionPolicy::SoundMay => {
            hasher.update(&[1]);
        }
        DerivedPrecisionPolicy::SoundMust => {
            hasher.update(&[2]);
        }
        DerivedPrecisionPolicy::Bounded { max_steps } => {
            hasher.update(&[3]);
            hasher.update(&max_steps.get().to_be_bytes());
        }
    };
    *hasher.finalize().as_bytes()
}

fn frame_completeness(hasher: &mut blake3::Hasher, completeness: &DerivedCompletenessPolicy) {
    match completeness {
        DerivedCompletenessPolicy::Complete => {
            hasher.update(&[0]);
        }
        DerivedCompletenessPolicy::Partial { unknown_family } => {
            hasher.update(&[1]);
            frame(hasher, unknown_family.as_str().as_bytes());
        }
        DerivedCompletenessPolicy::Unknown { unknown_family } => {
            hasher.update(&[2]);
            frame(hasher, unknown_family.as_str().as_bytes());
        }
    };
}

fn frame_disposition(hasher: &mut blake3::Hasher, disposition: &ProviderRegistrationDisposition) {
    match disposition {
        ProviderRegistrationDisposition::Registered {
            row_count,
            coverage,
        } => {
            hasher.update(&[0, terminal_code(*coverage)]);
            hasher.update(&u64::try_from(*row_count).unwrap_or(u64::MAX).to_be_bytes());
        }
        ProviderRegistrationDisposition::RegisteredUnknown { row_count, cause } => {
            hasher.update(&[1, unknown_cause_code(*cause)]);
            hasher.update(&u64::try_from(*row_count).unwrap_or(u64::MAX).to_be_bytes());
        }
        ProviderRegistrationDisposition::Unknown { cause } => {
            hasher.update(&[2, unknown_cause_code(*cause)]);
        }
        ProviderRegistrationDisposition::Remainder { trailer } => {
            hasher.update(&[3]);
            frame_coverage_trailer(hasher, trailer);
        }
    }
}

fn frame_coverage_trailer(hasher: &mut blake3::Hasher, trailer: &CoverageTrailer) {
    hasher.update(&[terminal_code(trailer.status)]);
    hasher.update(&trailer.requested_units.to_be_bytes());
    hasher.update(&trailer.completed_units.to_be_bytes());
    hasher.update(
        &u64::try_from(trailer.remainders.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for remainder in &trailer.remainders {
        hasher.update(&remainder.scope.0);
        hasher.update(&remainder.unit_count.to_be_bytes());
        hasher.update(&[provider_remainder_reason_code(remainder.reason)]);
    }
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn validate_text(kind: &'static str, value: &str) -> Result<(), ProgrammaticDerivedAnalysisError> {
    if value.is_empty()
        || value.len() > MAX_DERIVED_IDENTITY_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ProgrammaticDerivedAnalysisError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

const fn producer_output_completeness(
    completeness: &DerivedCompletenessPolicy,
) -> DerivedInputCompleteness {
    match completeness {
        DerivedCompletenessPolicy::Complete => DerivedInputCompleteness::Complete,
        DerivedCompletenessPolicy::Partial { .. } => DerivedInputCompleteness::Partial,
        DerivedCompletenessPolicy::Unknown { .. } => DerivedInputCompleteness::Unknown,
    }
}

const fn completeness_code(completeness: &DerivedCompletenessPolicy) -> u8 {
    match completeness {
        DerivedCompletenessPolicy::Complete => 0,
        DerivedCompletenessPolicy::Partial { .. } => 1,
        DerivedCompletenessPolicy::Unknown { .. } => 2,
    }
}

const fn input_completeness_code(completeness: DerivedInputCompleteness) -> u8 {
    match completeness {
        DerivedInputCompleteness::Complete => 0,
        DerivedInputCompleteness::Partial => 1,
        DerivedInputCompleteness::Unknown => 2,
    }
}

const fn domain_code(domain: DerivedAnalysisDomain) -> u8 {
    match domain {
        DerivedAnalysisDomain::Python => 0,
        DerivedAnalysisDomain::RustMir => 1,
        DerivedAnalysisDomain::Common => 2,
    }
}

const fn family_kind_code(kind: DerivedFamilyKind) -> u8 {
    match kind {
        DerivedFamilyKind::Fact => 0,
        DerivedFamilyKind::UnknownEvidence => 1,
    }
}

const fn lane_code(lane: ProviderNativeLane) -> u8 {
    match lane {
        ProviderNativeLane::TreeSitter => 0,
        ProviderNativeLane::Ruff => 1,
        ProviderNativeLane::Pyrefly => 2,
        ProviderNativeLane::Rustc => 3,
    }
}

const fn terminal_code(status: TerminalStatus) -> u8 {
    match status {
        TerminalStatus::Complete => 0,
        TerminalStatus::Partial => 1,
        TerminalStatus::Unknown => 2,
    }
}

const fn unknown_cause_code(cause: crate::provider_admission::ProviderAdmissionUnknownCause) -> u8 {
    use crate::provider_admission::ProviderAdmissionUnknownCause;
    match cause {
        ProviderAdmissionUnknownCause::MissingRelation => 0,
        ProviderAdmissionUnknownCause::MissingCoverage => 1,
        ProviderAdmissionUnknownCause::ProviderDeclared => 2,
    }
}

const fn provider_remainder_reason_code(reason: RemainderReason) -> u8 {
    match reason {
        RemainderReason::Unsupported => 0,
        RemainderReason::ProviderUnavailable => 1,
        RemainderReason::ResourceLimit => 2,
        RemainderReason::InvalidSource => 3,
        RemainderReason::Cancelled => 4,
        RemainderReason::Unknown => 5,
    }
}

const fn remainder_reason_code(reason: DerivedRemainderReason) -> u8 {
    match reason {
        DerivedRemainderReason::Unsupported => 0,
        DerivedRemainderReason::ProviderUnavailable => 1,
        DerivedRemainderReason::ResourceLimit => 2,
        DerivedRemainderReason::AlgorithmUnavailable => 3,
        DerivedRemainderReason::PrivateCompilerEvidenceUnavailable => 4,
        DerivedRemainderReason::TypedTransformationAdapterUnavailable => 5,
    }
}

const fn retryability_code(retryability: DerivedRemainderRetryability) -> u8 {
    match retryability {
        DerivedRemainderRetryability::Retryable => 0,
        DerivedRemainderRetryability::RequiresReleaseChange => 1,
        DerivedRemainderRetryability::PermanentlyUnsupported => 2,
    }
}

/// Fail-closed errors for derived-family closure and registration.
#[derive(Debug, Error)]
pub enum ProgrammaticDerivedAnalysisError {
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("derived-analysis aggregate resource bound {0} must be non-zero")]
    ZeroResourceBound(&'static str),
    #[error("derived transformation {subject:?} declares zero {resource}")]
    ZeroTransformationResourceBound {
        subject: String,
        resource: &'static str,
    },
    #[error("derived-analysis aggregate resource counter overflowed for {0}")]
    CompositionResourceCounterOverflow(&'static str),
    #[error("derived-analysis aggregate {resource} {observed} exceeds composition bound {maximum}")]
    CompositionResourceLimitExceeded {
        resource: &'static str,
        observed: u64,
        maximum: u64,
    },
    #[error("existing derived-analysis binding is invalid: {0}")]
    ExistingBinding(String),
    #[error("existing derived-analysis census contains unexpected role {0:?}")]
    UnexpectedExistingCensusRole(ExistingDerivedFamilyRole),
    #[error("existing derived-analysis census repeats role {0:?}")]
    DuplicateExistingCensusRole(ExistingDerivedFamilyRole),
    #[error("existing derived-analysis census is missing role {0:?}")]
    MissingExistingCensusRole(ExistingDerivedFamilyRole),
    #[error("existing derived-analysis census disposition does not match role {0:?}")]
    ExistingCensusDispositionMismatch(ExistingDerivedFamilyRole),
    #[error("existing derived-analysis role {0:?} has no catalog-input dependency contract")]
    ExistingCensusSourceFreeRole(ExistingDerivedFamilyRole),
    #[error("existing derived-analysis dependency contract does not match role {0:?}")]
    ExistingCensusDependencyMismatch(ExistingDerivedFamilyRole),
    #[error(
        "programmatic output for {role:?} is {actual_relation:?}/{actual_fields} fields; expected {expected_relation:?}/{expected_fields} fields"
    )]
    ExistingProgrammaticOutputMismatch {
        role: ExistingDerivedFamilyRole,
        expected_relation: ProgrammaticRelationId,
        actual_relation: ProgrammaticRelationId,
        expected_fields: usize,
        actual_fields: usize,
    },
    #[error("accepted derived-family set is empty")]
    EmptyAcceptedFamilySet,
    #[error("accepted derived-family count {observed} exceeds {maximum}")]
    FamilyLimitExceeded { observed: usize, maximum: usize },
    #[error("accepted family {0:?} is duplicated")]
    DuplicateAcceptedFamily(DerivedFamilyId),
    #[error("accepted family {family_id:?} repeats dependency {relation_id:?}")]
    DuplicateFamilyDependency {
        family_id: DerivedFamilyId,
        relation_id: ProgrammaticRelationId,
    },
    #[error("accepted families {first:?} and {second:?} share output {relation_id:?}")]
    DuplicateFamilyOutput {
        relation_id: ProgrammaticRelationId,
        first: DerivedFamilyId,
        second: DerivedFamilyId,
    },
    #[error("accepted family {family_id:?} declares sentinel algorithm version")]
    SentinelAlgorithmVersion { family_id: DerivedFamilyId },
    #[error("accepted family {family_id:?} declares sentinel algorithm release")]
    SentinelAlgorithmRelease { family_id: DerivedFamilyId },
    #[error("derived composition has no accepted {0:?} family or explicit remainder")]
    MissingAnalysisDomain(DerivedAnalysisDomain),
    #[error("disposition names undeclared family {0:?}")]
    UndeclaredFamilyDisposition(DerivedFamilyId),
    #[error("family {0:?} has multiple producer/remainder dispositions")]
    DuplicateFamilyDisposition(DerivedFamilyId),
    #[error("family {0:?} has neither one producer nor one explicit remainder")]
    MissingFamilyDisposition(DerivedFamilyId),
    #[error("provider relation {0:?} has duplicate cross-lane authority")]
    DuplicateProviderInputAuthority(ProgrammaticRelationId),
    #[error("family {family_id:?} depends on orphan relation {dependency:?}")]
    OrphanDependency {
        family_id: DerivedFamilyId,
        dependency: ProgrammaticRelationId,
    },
    #[error("family {family_id:?} depends on remainder-only relation {dependency:?}")]
    DependencyOnRemainder {
        family_id: DerivedFamilyId,
        dependency: ProgrammaticRelationId,
    },
    #[error("derived producer dependency cycle remains among {0:?}")]
    CyclicProducerDependencies(Vec<DerivedFamilyId>),
    #[error("family {family_id:?} names an undeclared producer algorithm/precision")]
    UndeclaredProducerAlgorithm { family_id: DerivedFamilyId },
    #[error("family {family_id:?} transformation contract drifts from its declared algorithm")]
    TransformationAlgorithmDrift { family_id: DerivedFamilyId },
    #[error("family {family_id:?} output is {actual:?}, expected {expected:?}")]
    ProducerOutputMismatch {
        family_id: DerivedFamilyId,
        expected: ProgrammaticRelationId,
        actual: ProgrammaticRelationId,
    },
    #[error(
        "family {family_id:?} transformation dependency order differs from its exact input vector"
    )]
    ProducerInputVectorMismatch { family_id: DerivedFamilyId },
    #[error("family {family_id:?} uses volatile derived semantics")]
    VolatileDerivedProducer { family_id: DerivedFamilyId },
    #[error(
        "family {family_id:?} requests bounded native recursion ({max_iterations}), unavailable in the pinned executor"
    )]
    BoundedNativeRecursionUnavailable {
        family_id: DerivedFamilyId,
        max_iterations: u32,
    },
    #[error("family {family_id:?} lacks declared witness field {field_id:?}")]
    MissingProducerWitness {
        family_id: DerivedFamilyId,
        field_id: ProgrammaticFieldId,
    },
    #[error("family {family_id:?} output collides with metadata field {field_id:?}")]
    ProducerMetadataFieldCollision {
        family_id: DerivedFamilyId,
        field_id: ProgrammaticFieldId,
    },
    #[error("family {family_id:?} names missing unknown family {unknown_family:?}")]
    MissingUnknownFamily {
        family_id: DerivedFamilyId,
        unknown_family: DerivedFamilyId,
    },
    #[error("family {family_id:?} names invalid unknown family {unknown_family:?}")]
    InvalidUnknownFamily {
        family_id: DerivedFamilyId,
        unknown_family: DerivedFamilyId,
    },
    #[error("family {family_id:?} unknown family {unknown_family:?} is not produced")]
    UnknownFamilyNotProduced {
        family_id: DerivedFamilyId,
        unknown_family: DerivedFamilyId,
    },
    #[error("family {family_id:?} claims complete output from an incomplete input")]
    CompleteProducerHasIncompleteInput { family_id: DerivedFamilyId },
    #[error("family {family_id:?} depends on unavailable input {dependency:?}")]
    UnavailableProducerInput {
        family_id: DerivedFamilyId,
        dependency: ProgrammaticRelationId,
    },
    #[error("provider lane {lane:?} attempts to own derived family {family_id:?}")]
    ProviderOwnedDerivedFamily {
        family_id: DerivedFamilyId,
        lane: ProviderNativeLane,
    },
    #[error("application-derived authority uses the all-zero sentinel")]
    SentinelApplicationAuthority,
    #[error("remainder for family {family_id:?} uses the all-zero evidence sentinel")]
    SentinelRemainderEvidence { family_id: DerivedFamilyId },
    #[error("remainder for family {family_id:?} names an undeclared algorithm")]
    UndeclaredRemainderAlgorithm { family_id: DerivedFamilyId },
    #[error("derived metadata role {0:?} is missing")]
    MissingMetadataRole(DerivedMetadataRole),
    #[error("derived metadata role {0:?} is duplicated")]
    DuplicateMetadataRole(DerivedMetadataRole),
    #[error("derived metadata field {0:?} is duplicated")]
    DuplicateMetadataField(ProgrammaticFieldId),
    #[error("derived metadata physical column {0:?} is duplicated")]
    DuplicateMetadataColumn(Arc<str>),
    #[error("remainder metadata role {0:?} is missing")]
    MissingRemainderMetadataRole(DerivedRemainderMetadataRole),
    #[error("remainder metadata role {0:?} is duplicated")]
    DuplicateRemainderMetadataRole(DerivedRemainderMetadataRole),
    #[error("remainder output field identities do not match its closed metadata binding")]
    RemainderOutputFieldMismatch,
    #[error("remainder transformation must be deterministic-set, unordered, and non-recursive")]
    InvalidRemainderExecutionContract,
    #[error("remainder rows {observed} exceed transformation row bound {maximum}")]
    RemainderRowBoundExceeded { observed: u64, maximum: u64 },
    #[error(transparent)]
    ProviderAdmission(#[from] ProviderAdmissionError),
    #[error(transparent)]
    Epoch(#[from] ProgrammaticFabricEpochError),
}

#[cfg(test)]
mod tests {
    use arrow_array::{
        ArrayRef, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray, UInt32Array,
        UInt64Array,
    };
    use arrow_schema::SchemaRef;
    use datafusion::common::TableReference;
    use datafusion::datasource::MemTable;
    use datafusion::prelude::SessionContext;

    use super::*;
    use crate::fabric::epoch_runtime::{FABRIC_CATALOG, FabricEpochId, FabricSchemaRole};
    use crate::fabric::programmatic_schema::{
        ProgrammaticSchemaError, ProvenanceObservation, ProviderInput,
    };
    use crate::provider_admission::{
        admit_provider_relations_programmatic,
        tests::{
            ExactWorkspaceFixture, changed_exact_workspace_fixture, exact_workspace_fixture,
            programmatic_epoch_builder,
        },
    };
    use crate::provider_native_syntax::NativeSyntaxRelation;
    use crate::pyrefly_service::PyreflyRelation;
    use crate::python_derived_analysis::{
        PYTHON_DERIVED_AUTHORITY, tests::bindings as python_flow_bindings,
    };
    use crate::rust_mir_derived_analysis::tests::bindings as rust_mir_bindings;
    use crate::rustc_relation_schema::RustcRelation;
    use crate::schema_contract::{FieldIndexMapping, SchemaContract};
    use crate::{
        common_derived_analysis::tests::bindings as common_analysis_bindings,
        fabric::programmatic_schema::SealedProgrammaticSchemaAssembly,
    };

    fn transformation_authority() -> CompiledTransformationAuthority {
        *crate::fabric::production_kernel::CompiledSemanticRelease::current()
            .transformation_authority()
    }

    #[test]
    fn compiled_transformation_authority_is_required_and_composition_input_is_not_public() {
        let _raw_compose: fn(
            &CompiledTransformationAuthority,
            ProgrammaticProviderAdmissionOutcome,
            ProgrammaticDerivedAnalysisComposition,
        ) -> Result<
            ProgrammaticDerivedAnalysisOutcome,
            ProgrammaticDerivedAnalysisError,
        > = compose_programmatic_derived_analyses;
        let _raw_admit: for<'a> fn(
            &CompiledTransformationAuthority,
            ProgrammaticFabricEpochBuilder,
            ExactProgrammaticProviderRuns<'a>,
            ProgrammaticDerivedAnalysisComposition,
        ) -> Result<
            ProgrammaticDerivedAnalysisOutcome,
            ProgrammaticDerivedAnalysisError,
        > = admit_and_compose_programmatic_derived_analyses;

        let analysis_source = include_str!("programmatic_derived_analysis.rs");
        for route in [
            concat!(
                "pub",
                " fn admit_and_compose_programmatic_derived_analyses("
            ),
            concat!("pub", " fn compose_programmatic_derived_analyses("),
        ] {
            assert!(
                !analysis_source.contains(route),
                "raw semantic-composition route became public: {route}"
            );
        }
        assert!(analysis_source.contains(concat!(
            "release_authority: Compiled",
            "TransformationAuthority"
        )));
        assert!(!analysis_source.contains(concat!("    pub", " fn into_parts(")));

        let kernel_source = include_str!("fabric/production_kernel.rs");
        assert!(!kernel_source.contains(concat!(
            "    pub",
            " fn admit_and_compose_derived_analyses("
        )));
        assert!(kernel_source.contains(concat!(
            "    pub(crate)",
            " fn admit_and_compose_derived_analyses("
        )));
    }

    struct ProjectSupportTransformation {
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        dependencies: Arc<[ProgrammaticRelationId]>,
        output_name: Arc<str>,
    }

    impl ProgrammaticTransformation for ProjectSupportTransformation {
        fn contract(&self) -> &ProgrammaticTransformationContract {
            &self.contract
        }

        fn output(&self) -> &TransformationOutput {
            &self.output
        }

        fn dependencies(&self) -> &[ProgrammaticRelationId] {
            &self.dependencies
        }

        fn build(
            &self,
            inputs: &TransformationInputs,
        ) -> Result<LogicalPlan, TransformationPlanError> {
            let Some(first) = self.dependencies.first() else {
                return Err(TransformationPlanError::DataFusion(
                    datafusion::error::DataFusionError::Plan(
                        "fixture producer requires one exact input".to_owned(),
                    ),
                ));
            };
            let mut plan = inputs.plan(first)?;
            for dependency in self.dependencies.iter().skip(1) {
                plan = LogicalPlanBuilder::from(plan)
                    .cross_join(inputs.plan(dependency)?)?
                    .build()?;
            }
            let (qualifier, field) = plan.schema().iter().next().ok_or_else(|| {
                TransformationPlanError::DataFusion(datafusion::error::DataFusionError::Plan(
                    "fixture input has no support field".to_owned(),
                ))
            })?;
            let support = Expr::Column(Column::new(qualifier.cloned(), field.name().clone()))
                .alias(self.output_name.as_ref());
            Ok(LogicalPlanBuilder::from(plan).project([support])?.build()?)
        }
    }

    fn metadata_name(role: DerivedMetadataRole) -> &'static str {
        match role {
            DerivedMetadataRole::FamilyIdentity => "__cf_derived_family",
            DerivedMetadataRole::Domain => "__cf_derived_domain",
            DerivedMetadataRole::AuthorityIdentity => "__cf_derived_authority",
            DerivedMetadataRole::AlgorithmIdentity => "__cf_derived_algorithm",
            DerivedMetadataRole::AlgorithmVersionMajor => "__cf_derived_version_major",
            DerivedMetadataRole::AlgorithmVersionMinor => "__cf_derived_version_minor",
            DerivedMetadataRole::AlgorithmVersionPatch => "__cf_derived_version_patch",
            DerivedMetadataRole::ReleaseIdentity => "__cf_derived_release",
            DerivedMetadataRole::PrecisionIdentity => "__cf_derived_precision",
            DerivedMetadataRole::InputVectorIdentity => "__cf_derived_input_vector",
            DerivedMetadataRole::CompletenessState => "__cf_derived_completeness",
            DerivedMetadataRole::ProvenanceClosureIdentity => "__cf_derived_provenance",
        }
    }

    fn metadata_bindings() -> DerivedMetadataBindings {
        DerivedMetadataBindings::try_new(
            &transformation_authority(),
            DerivedMetadataRole::ALL.into_iter().map(|role| {
                let name = metadata_name(role);
                DerivedMetadataColumnBinding::new(
                    &transformation_authority(),
                    role,
                    ProgrammaticFieldId::new(format!("fixture.metadata.{name}")),
                    name,
                )
            }),
        )
        .unwrap()
    }

    fn transformation_contract(
        semantic_id: &str,
        marker: u8,
        max_rows: u64,
    ) -> ProgrammaticTransformationContract {
        ProgrammaticTransformationContract::new(
            ProgrammaticTransformationId::new(semantic_id),
            TransformationSemanticVersion::new(1, u16::from(marker), 0),
            TransformationResourceClass::BoundedInMemory {
                max_rows,
                max_memory_bytes: 32 * 1024 * 1024,
            },
            TransformationDeterminismPolicy::DeterministicSet,
            TransformationOrderingPolicy::Unordered,
            TransformationRecursionPolicy::Forbidden,
            TransformationProvenance::new(
                TransformationProvenanceIdentity::from_bytes([marker; 32]),
                TransformationReleaseIdentity::from_bytes([marker.wrapping_add(1); 32]),
            ),
        )
    }

    fn transformation(
        semantic_id: &str,
        marker: u8,
        max_rows: u64,
        output_relation: &str,
        output_table: &str,
        witness_name: &str,
        witness_field: &str,
        dependencies: Vec<ProgrammaticRelationId>,
    ) -> Arc<ProjectSupportTransformation> {
        Arc::new(ProjectSupportTransformation {
            contract: transformation_contract(semantic_id, marker, max_rows),
            output: TransformationOutput::new(
                ProgrammaticRelationId::new(output_relation),
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Derived.as_str(),
                    output_table,
                ),
                vec![TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                    witness_field,
                ))],
            ),
            dependencies: dependencies.into(),
            output_name: Arc::from(witness_name),
        })
    }

    fn algorithm(transformation: &Arc<ProjectSupportTransformation>) -> DerivedAlgorithmContract {
        DerivedAlgorithmContract::new(
            &transformation_authority(),
            transformation.contract.semantic_id().clone(),
            transformation.contract.semantic_version(),
            transformation.contract.provenance().release_identity(),
        )
    }

    fn remainder_binding(metadata: &DerivedMetadataBindings) -> DerivedRemainderRelationBinding {
        let contract = transformation_contract("analysis.remainder.fixture", 240, 64);
        let columns = [
            DerivedRemainderMetadataColumnBinding::new(
                &transformation_authority(),
                DerivedRemainderMetadataRole::Reason,
                ProgrammaticFieldId::new("fixture.remainder.reason"),
                "__cf_derived_remainder_reason",
            ),
            DerivedRemainderMetadataColumnBinding::new(
                &transformation_authority(),
                DerivedRemainderMetadataRole::EvidenceIdentity,
                ProgrammaticFieldId::new("fixture.remainder.evidence"),
                "__cf_derived_remainder_evidence",
            ),
            DerivedRemainderMetadataColumnBinding::new(
                &transformation_authority(),
                DerivedRemainderMetadataRole::Retryability,
                ProgrammaticFieldId::new("fixture.remainder.retryability"),
                "__cf_derived_remainder_retryability",
            ),
        ];
        let fields = metadata
            .ordered()
            .map(|column| TransformationFieldIdentity::new(column.field_id.clone()))
            .chain(
                columns
                    .iter()
                    .map(|column| TransformationFieldIdentity::new(column.field_id.clone())),
            )
            .collect::<Vec<_>>();
        DerivedRemainderRelationBinding::try_new(
            &transformation_authority(),
            [241; 32],
            contract,
            TransformationOutput::new(
                ProgrammaticRelationId::new("proof.derived_remainder.fixture"),
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Proof.as_str(),
                    "derived_remainder_fixture",
                ),
                fields,
            ),
            columns,
        )
        .unwrap()
    }

    #[derive(Clone)]
    struct ExistingProducerRelations {
        python_cfg_node: ProgrammaticRelationId,
        python_cfg: ProgrammaticRelationId,
        python_evaluation_order: ProgrammaticRelationId,
        python_def_use: ProgrammaticRelationId,
        python_reaching_definition: ProgrammaticRelationId,
        python_liveness: ProgrammaticRelationId,
        python_value_flow: ProgrammaticRelationId,
        rust_mir_cfg: ProgrammaticRelationId,
        rust_mir_ownership: ProgrammaticRelationId,
        rust_mir_alias: ProgrammaticRelationId,
        rust_mir_resource: ProgrammaticRelationId,
        rust_mir_async: ProgrammaticRelationId,
        rust_mir_unsafe_ffi: ProgrammaticRelationId,
        common_call_graph: ProgrammaticRelationId,
    }

    fn programmatic_output(
        relation_id: ProgrammaticRelationId,
        table_name: &str,
        field_prefix: &str,
        field_count: usize,
    ) -> TransformationOutput {
        TransformationOutput::new(
            relation_id,
            TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Derived.as_str(),
                table_name,
            ),
            (0..field_count)
                .map(|ordinal| {
                    TransformationFieldIdentity::new(ProgrammaticFieldId::new(format!(
                        "{field_prefix}.{ordinal}"
                    )))
                })
                .collect::<Vec<_>>(),
        )
    }

    fn algorithm_from_contract(
        contract: &ProgrammaticTransformationContract,
    ) -> DerivedAlgorithmContract {
        DerivedAlgorithmContract::new(
            &transformation_authority(),
            contract.semantic_id().clone(),
            contract.semantic_version(),
            contract.provenance().release_identity(),
        )
    }

    fn existing_declarations(
        python_bindings: &PythonFlowBindings,
        rust_bindings: &RustMirAnalysisBindings,
        common_bindings: &CommonAnalysisBindings,
    ) -> (
        Vec<ExistingDerivedFamilyDeclaration>,
        ExistingProducerRelations,
    ) {
        let roles = ExistingDerivedFamilyRole::all();
        let family_ids = roles
            .iter()
            .enumerate()
            .map(|(index, role)| {
                (
                    *role,
                    DerivedFamilyId::try_new(
                        &transformation_authority(),
                        format!("family.existing.role.{index}"),
                    )
                    .unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let python_node_contract =
            transformation_contract("analysis.python.cfg_node.programmatic.v3", 74, 65_536);
        let python_node_relation = ProgrammaticRelationId::new(
            python_bindings
                .relation_id(PythonDerivedRelation::CfgNode)
                .as_str(),
        );
        let python_node_output = programmatic_output(
            python_node_relation.clone(),
            "python_cfg_node_programmatic",
            "programmatic.python.cfg.node",
            ProgrammaticPythonCfgNodeTransformation::OUTPUT_FIELD_COUNT,
        );
        let python_node_witness = python_node_output.fields()[13].field_id().clone();
        let python_node = Arc::new(
            ProgrammaticPythonCfgNodeTransformation::try_new(
                &transformation_authority(),
                python_node_contract.clone(),
                python_node_output,
                python_bindings,
                FabricEpochId::from_bytes([90; 16]),
                ProgrammaticPythonCfgNodeRowContract::try_new(
                    &transformation_authority(),
                    "codefabric.python-cfg-node.programmatic-datafusion-55.v3",
                    "ruff-typed-ast-node-normalization.v3",
                    PYTHON_DERIVED_AUTHORITY,
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let python_node_algorithm = algorithm_from_contract(&python_node_contract);
        let python_node_precision = DerivedPrecisionPolicy::Exact;
        let python_node_role = ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgNode);
        let python_node_dependencies = python_node.dependencies().to_vec();

        let python_contract =
            transformation_contract("analysis.python.cfg.programmatic.v3", 71, 65_536);
        let python_relation = ProgrammaticRelationId::new(
            python_bindings
                .relation_id(PythonDerivedRelation::CfgEdge)
                .as_str(),
        );
        let python_output = programmatic_output(
            python_relation.clone(),
            "python_cfg_edge_programmatic",
            "programmatic.python.cfg.edge",
            ProgrammaticPythonCfgEdgeTransformation::OUTPUT_FIELD_COUNT,
        );
        let python_witness = python_output.fields()[13].field_id().clone();
        let python = Arc::new(
            ProgrammaticPythonCfgEdgeTransformation::try_new(
                &transformation_authority(),
                python_contract.clone(),
                python_output,
                python_bindings,
                FabricEpochId::from_bytes([90; 16]),
                ProgrammaticPythonCfgEdgeRowContract::try_new(
                    &transformation_authority(),
                    "codefabric.python-cfg.programmatic-datafusion-55.v3",
                    "ruff-evaluation-order-sequential-cfg.v3",
                    PYTHON_DERIVED_AUTHORITY,
                    "sequential",
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let python_algorithm = algorithm_from_contract(&python_contract);
        let python_precision = DerivedPrecisionPolicy::SoundMay;
        let python_role = ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgEdge);
        let python_dependencies = python.dependencies().to_vec();

        let make_python_dataflow =
            |role: PythonDerivedRelation,
             semantic_id: &str,
             marker: u8,
             table_name: &str,
             field_prefix: &str,
             algorithm_release: &str,
             precision_release: &str,
             precision: DerivedPrecisionPolicy| {
                let contract = transformation_contract(semantic_id, marker, 262_144);
                let relation =
                    ProgrammaticRelationId::new(python_bindings.relation_id(role).as_str());
                let output = programmatic_output(
                    relation.clone(),
                    table_name,
                    field_prefix,
                    match role {
                        PythonDerivedRelation::EvaluationOrder => {
                            ProgrammaticPythonDataflowTransformation::EVALUATION_OUTPUT_FIELD_COUNT
                        }
                        PythonDerivedRelation::Liveness => {
                            ProgrammaticPythonDataflowTransformation::LIVENESS_OUTPUT_FIELD_COUNT
                        }
                        _ => ProgrammaticPythonDataflowTransformation::FLOW_LINK_OUTPUT_FIELD_COUNT,
                    },
                );
                let witness = output.fields()[13].field_id().clone();
                let transformation = Arc::new(
                    ProgrammaticPythonDataflowTransformation::try_new(
                        &transformation_authority(),
                        contract.clone(),
                        output,
                        python_bindings,
                        FabricEpochId::from_bytes([90; 16]),
                        role,
                        ProgrammaticPythonDataflowRowContract::try_new(
                            &transformation_authority(),
                            algorithm_release,
                            precision_release,
                            PYTHON_DERIVED_AUTHORITY,
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                );
                let role = ExistingDerivedFamilyRole::Python(role);
                let dependencies = transformation.dependencies().to_vec();
                let algorithm = algorithm_from_contract(&contract);
                let declaration = ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    role,
                    family_ids[&role].clone(),
                    algorithm.clone(),
                    precision.clone(),
                    dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([marker.wrapping_add(91); 32]),
                        algorithm,
                        precision,
                        DerivedCompletenessPolicy::Complete,
                        witness,
                        transformation,
                    ),
                );
                (role, declaration, relation)
            };

        let (python_evaluation_role, python_evaluation, python_evaluation_relation) =
            make_python_dataflow(
                PythonDerivedRelation::EvaluationOrder,
                "analysis.python.evaluation_order.programmatic.v3",
                76,
                "python_evaluation_order_programmatic",
                "programmatic.python.evaluation_order",
                "codefabric.python-evaluation-order.programmatic-datafusion-55.v3",
                "sequential-cfg-node-order.v3",
                DerivedPrecisionPolicy::SoundMay,
            );
        let (python_def_use_role, python_def_use, python_def_use_relation) = make_python_dataflow(
            PythonDerivedRelation::DefUse,
            "analysis.python.def_use.programmatic.v3",
            77,
            "python_def_use_programmatic",
            "programmatic.python.def_use",
            "codefabric.python-def-use.programmatic-datafusion-55.v3",
            "resolved-reference-owner-candidate.v3",
            DerivedPrecisionPolicy::SoundMay,
        );
        let (python_reaching_role, python_reaching, python_reaching_relation) =
            make_python_dataflow(
                PythonDerivedRelation::ReachingDefinition,
                "analysis.python.reaching_definition.programmatic.v3",
                78,
                "python_reaching_definition_programmatic",
                "programmatic.python.reaching_definition",
                "codefabric.python-reaching-definition.programmatic-datafusion-55.v3",
                "complete-sequential-cfg-latest-definition.v3",
                DerivedPrecisionPolicy::Exact,
            );
        let (python_liveness_role, python_liveness, python_liveness_relation) =
            make_python_dataflow(
                PythonDerivedRelation::Liveness,
                "analysis.python.liveness.programmatic.v3",
                79,
                "python_liveness_programmatic",
                "programmatic.python.liveness",
                "codefabric.python-liveness.programmatic-datafusion-55.v3",
                "complete-sequential-cfg-live-range.v3",
                DerivedPrecisionPolicy::Exact,
            );
        let (python_value_flow_role, python_value_flow, python_value_flow_relation) =
            make_python_dataflow(
                PythonDerivedRelation::ValueFlow,
                "analysis.python.value_flow.programmatic.v3",
                80,
                "python_value_flow_programmatic",
                "programmatic.python.value_flow",
                "codefabric.python-value-flow.programmatic-datafusion-55.v3",
                "selected-reaching-definition-value-flow.v3",
                DerivedPrecisionPolicy::Exact,
            );

        let rust_contract =
            transformation_contract("analysis.rust_mir.cfg.programmatic.v3", 72, 65_536);
        let rust_relation = ProgrammaticRelationId::new(
            rust_bindings
                .relation_id(RustMirDerivedRelation::CfgEdge)
                .as_str(),
        );
        let rust_output = programmatic_output(
            rust_relation.clone(),
            "rust_mir_cfg_edge_programmatic",
            "programmatic.rust_mir.cfg.edge",
            ProgrammaticRustMirCfgEdgeTransformation::OUTPUT_FIELD_COUNT,
        );
        let rust_witness = rust_output.fields()[8].field_id().clone();
        let rust = Arc::new(
            ProgrammaticRustMirCfgEdgeTransformation::try_new(
                &transformation_authority(),
                rust_contract.clone(),
                rust_output,
                rust_bindings,
            )
            .unwrap(),
        );
        let rust_algorithm = algorithm_from_contract(&rust_contract);
        let rust_precision = DerivedPrecisionPolicy::Exact;
        let rust_role = ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::CfgEdge);
        let rust_dependencies = rust.dependencies().to_vec();

        let rust_control_contract = transformation_contract(
            "analysis.rust_mir.control_input.programmatic.v3",
            75,
            65_536,
        );
        let rust_control_relation = ProgrammaticRelationId::new(
            rust_bindings
                .relation_id(RustMirDerivedRelation::ControlDependenceInput)
                .as_str(),
        );
        let rust_control_output = programmatic_output(
            rust_control_relation.clone(),
            "rust_mir_control_input_programmatic",
            "programmatic.rust_mir.control_input",
            ProgrammaticRustMirControlInputTransformation::OUTPUT_FIELD_COUNT,
        );
        let rust_control_witness = rust_control_output.fields()[9].field_id().clone();
        let rust_control = Arc::new(
            ProgrammaticRustMirControlInputTransformation::try_new(
                &transformation_authority(),
                rust_control_contract.clone(),
                rust_control_output,
                rust_bindings,
            )
            .unwrap(),
        );
        let rust_control_algorithm = algorithm_from_contract(&rust_control_contract);
        let rust_control_precision = DerivedPrecisionPolicy::Exact;
        let rust_control_role =
            ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::ControlDependenceInput);
        let rust_control_dependencies = rust_control.dependencies().to_vec();

        let make_rust_structural =
            |role: RustMirDerivedRelation,
             semantic_id: &str,
             marker: u8,
             table_name: &str,
             field_prefix: &str,
             precision: DerivedPrecisionPolicy| {
                let contract = transformation_contract(semantic_id, marker, 262_144);
                let relation =
                    ProgrammaticRelationId::new(rust_bindings.relation_id(role).as_str());
                let output = programmatic_output(
                    relation.clone(),
                    table_name,
                    field_prefix,
                    ProgrammaticRustMirStructuralTransformation::output_field_count(role),
                );
                let witness = output.fields()[9].field_id().clone();
                let transformation = Arc::new(
                    ProgrammaticRustMirStructuralTransformation::try_new(
                        &transformation_authority(),
                        role,
                        contract.clone(),
                        output,
                        rust_bindings,
                    )
                    .unwrap(),
                );
                let role = ExistingDerivedFamilyRole::RustMir(role);
                let dependencies = transformation.dependencies().to_vec();
                let algorithm = algorithm_from_contract(&contract);
                let declaration = ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    role,
                    family_ids[&role].clone(),
                    algorithm.clone(),
                    precision.clone(),
                    dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([marker.wrapping_add(91); 32]),
                        algorithm,
                        precision,
                        DerivedCompletenessPolicy::Complete,
                        witness,
                        transformation,
                    ),
                );
                (role, declaration, relation)
            };
        let (rust_ownership_role, rust_ownership, rust_ownership_relation) = make_rust_structural(
            RustMirDerivedRelation::OwnershipState,
            "analysis.rust_mir.ownership.programmatic.v3",
            81,
            "rust_mir_ownership_programmatic",
            "programmatic.rust_mir.ownership",
            DerivedPrecisionPolicy::SoundMay,
        );
        let (rust_alias_role, rust_alias, rust_alias_relation) = make_rust_structural(
            RustMirDerivedRelation::AliasPointsTo,
            "analysis.rust_mir.alias.programmatic.v3",
            82,
            "rust_mir_alias_programmatic",
            "programmatic.rust_mir.alias",
            DerivedPrecisionPolicy::SoundMay,
        );
        let (rust_resource_role, rust_resource, rust_resource_relation) = make_rust_structural(
            RustMirDerivedRelation::ResourceLifecycle,
            "analysis.rust_mir.resource.programmatic.v3",
            83,
            "rust_mir_resource_programmatic",
            "programmatic.rust_mir.resource",
            DerivedPrecisionPolicy::Exact,
        );
        let (rust_async_role, rust_async, rust_async_relation) = make_rust_structural(
            RustMirDerivedRelation::AsyncLowering,
            "analysis.rust_mir.async.programmatic.v3",
            84,
            "rust_mir_async_programmatic",
            "programmatic.rust_mir.async",
            DerivedPrecisionPolicy::SoundMay,
        );
        let (rust_unsafe_role, rust_unsafe, rust_unsafe_relation) = make_rust_structural(
            RustMirDerivedRelation::UnsafeFfi,
            "analysis.rust_mir.unsafe_ffi.programmatic.v3",
            85,
            "rust_mir_unsafe_ffi_programmatic",
            "programmatic.rust_mir.unsafe_ffi",
            DerivedPrecisionPolicy::SoundMay,
        );

        let common_contract =
            transformation_contract("analysis.common.call_graph.programmatic.v3", 73, 65_536);
        let common_relation = ProgrammaticRelationId::new(common_bindings.relations.facts.as_str());
        let common_output = programmatic_output(
            common_relation.clone(),
            "common_call_graph_programmatic",
            "programmatic.common.call_graph",
            ProgrammaticCommonCallGraphTransformation::OUTPUT_FIELD_COUNT,
        );
        let common_witness = common_output.fields()[3].field_id().clone();
        let common = Arc::new(
            ProgrammaticCommonCallGraphTransformation::try_new(
                &transformation_authority(),
                common_contract.clone(),
                common_output,
                common_bindings,
            )
            .unwrap(),
        );
        let common_algorithm = algorithm_from_contract(&common_contract);
        let common_precision = DerivedPrecisionPolicy::SoundMay;
        let common_role =
            ExistingDerivedFamilyRole::Common(ExistingCommonDerivedFamilyRole::CallGraph);
        let common_dependencies = common.dependencies().to_vec();

        let mut producers = BTreeMap::from([
            (
                python_node_role,
                ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    python_node_role,
                    family_ids[&python_node_role].clone(),
                    python_node_algorithm.clone(),
                    python_node_precision.clone(),
                    python_node_dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&python_node_role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([174; 32]),
                        python_node_algorithm,
                        python_node_precision,
                        DerivedCompletenessPolicy::Complete,
                        python_node_witness,
                        python_node,
                    ),
                ),
            ),
            (
                python_role,
                ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    python_role,
                    family_ids[&python_role].clone(),
                    python_algorithm.clone(),
                    python_precision.clone(),
                    python_dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&python_role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([171; 32]),
                        python_algorithm,
                        python_precision,
                        DerivedCompletenessPolicy::Complete,
                        python_witness,
                        python,
                    ),
                ),
            ),
            (
                rust_role,
                ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    rust_role,
                    family_ids[&rust_role].clone(),
                    rust_algorithm.clone(),
                    rust_precision.clone(),
                    rust_dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&rust_role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([172; 32]),
                        rust_algorithm,
                        rust_precision,
                        DerivedCompletenessPolicy::Complete,
                        rust_witness,
                        rust,
                    ),
                ),
            ),
            (
                rust_control_role,
                ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    rust_control_role,
                    family_ids[&rust_control_role].clone(),
                    rust_control_algorithm.clone(),
                    rust_control_precision.clone(),
                    rust_control_dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&rust_control_role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([175; 32]),
                        rust_control_algorithm,
                        rust_control_precision,
                        DerivedCompletenessPolicy::Complete,
                        rust_control_witness,
                        rust_control,
                    ),
                ),
            ),
            (
                common_role,
                ExistingDerivedFamilyDeclaration::producer(
                    &transformation_authority(),
                    common_role,
                    family_ids[&common_role].clone(),
                    common_algorithm.clone(),
                    common_precision.clone(),
                    common_dependencies,
                    AcceptedDerivedProducer::new(
                        &transformation_authority(),
                        family_ids[&common_role].clone(),
                        DerivedProducerAuthority::ApplicationOwned([173; 32]),
                        common_algorithm,
                        common_precision,
                        DerivedCompletenessPolicy::Complete,
                        common_witness,
                        common,
                    ),
                ),
            ),
        ]);
        for (role, declaration) in [
            (python_evaluation_role, python_evaluation),
            (python_def_use_role, python_def_use),
            (python_reaching_role, python_reaching),
            (python_liveness_role, python_liveness),
            (python_value_flow_role, python_value_flow),
            (rust_ownership_role, rust_ownership),
            (rust_alias_role, rust_alias),
            (rust_resource_role, rust_resource),
            (rust_async_role, rust_async),
            (rust_unsafe_role, rust_unsafe),
        ] {
            assert!(producers.insert(role, declaration).is_none());
        }

        let declarations = roles
            .into_iter()
            .enumerate()
            .map(|(index, role)| {
                if let Some(producer) = producers.remove(&role) {
                    return producer;
                }
                let marker = 180_u8.saturating_add(u8::try_from(index).unwrap());
                let algorithm = DerivedAlgorithmContract::new(
                    &transformation_authority(),
                    ProgrammaticTransformationId::new(format!(
                        "analysis.existing.adapter_pending.{index}"
                    )),
                    TransformationSemanticVersion::new(1, 0, 0),
                    TransformationReleaseIdentity::from_bytes([marker; 32]),
                );
                ExistingDerivedFamilyDeclaration::adapter_unavailable(
                    &transformation_authority(),
                    role,
                    family_ids[&role].clone(),
                    algorithm,
                    DerivedPrecisionPolicy::Exact,
                    role.dependency_contract(python_bindings, rust_bindings, common_bindings),
                    [marker.wrapping_add(1); 32],
                    DerivedRemainderRetryability::RequiresReleaseChange,
                )
                .unwrap()
            })
            .collect();
        (
            declarations,
            ExistingProducerRelations {
                python_cfg_node: python_node_relation,
                python_cfg: python_relation,
                python_evaluation_order: python_evaluation_relation,
                python_def_use: python_def_use_relation,
                python_reaching_definition: python_reaching_relation,
                python_liveness: python_liveness_relation,
                python_value_flow: python_value_flow_relation,
                rust_mir_cfg: rust_relation,
                rust_mir_ownership: rust_ownership_relation,
                rust_mir_alias: rust_alias_relation,
                rust_mir_resource: rust_resource_relation,
                rust_mir_async: rust_async_relation,
                rust_mir_unsafe_ffi: rust_unsafe_relation,
                common_call_graph: common_relation,
            },
        )
    }

    fn existing_census(
        python_bindings: &PythonFlowBindings,
        rust_bindings: &RustMirAnalysisBindings,
        common_bindings: &CommonAnalysisBindings,
    ) -> (ExistingDerivedAnalysisCensus, ExistingProducerRelations) {
        let (declarations, relations) =
            existing_declarations(python_bindings, rust_bindings, common_bindings);
        (
            ExistingDerivedAnalysisCensus::try_new(
                &transformation_authority(),
                python_bindings,
                rust_bindings,
                common_bindings,
                declarations,
            )
            .unwrap(),
            relations,
        )
    }

    fn composition(
        python_precision: DerivedPrecisionPolicy,
        python_max_rows: u64,
    ) -> ProgrammaticDerivedAnalysisComposition {
        let metadata = metadata_bindings();
        let python_relation = "derived.python.flow.fixture";
        let rust_relation = "derived.rust_mir.flow.fixture";
        let common_relation = "derived.common.graph.fixture";
        let remainder_relation = "derived.common.recursive.fixture";
        let python_family =
            DerivedFamilyId::try_new(&transformation_authority(), "family.python.flow.fixture")
                .unwrap();
        let rust_family =
            DerivedFamilyId::try_new(&transformation_authority(), "family.rust_mir.flow.fixture")
                .unwrap();
        let common_family =
            DerivedFamilyId::try_new(&transformation_authority(), "family.common.graph.fixture")
                .unwrap();
        let recursive_family = DerivedFamilyId::try_new(
            &transformation_authority(),
            "family.common.recursive.fixture",
        )
        .unwrap();

        let python = transformation(
            "analysis.python.flow.fixture",
            31,
            python_max_rows,
            python_relation,
            "python_flow_fixture",
            "python_support",
            "derived.python.flow.fixture.support",
            vec![
                ProgrammaticRelationId::new(NativeSyntaxRelation::TreeSitterRun.as_str()),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffRun.as_str()),
                ProgrammaticRelationId::new(PyreflyRelation::ModuleContext.relation_id()),
            ],
        );
        let rust = transformation(
            "analysis.rust_mir.flow.fixture",
            41,
            4_096,
            rust_relation,
            "rust_mir_flow_fixture",
            "rust_support",
            "derived.rust_mir.flow.fixture.support",
            vec![ProgrammaticRelationId::new(
                RustcRelation::Compilation.relation_id(),
            )],
        );
        let common = transformation(
            "analysis.common.graph.fixture",
            51,
            16_384,
            common_relation,
            "common_graph_fixture",
            "common_support",
            "derived.common.graph.fixture.support",
            vec![
                ProgrammaticRelationId::new(python_relation),
                ProgrammaticRelationId::new(rust_relation),
            ],
        );
        let recursive_algorithm = DerivedAlgorithmContract::new(
            &transformation_authority(),
            ProgrammaticTransformationId::new("analysis.common.recursive.fixture"),
            TransformationSemanticVersion::new(1, 0, 0),
            TransformationReleaseIdentity::from_bytes([61; 32]),
        );

        let families = vec![
            AcceptedDerivedFamily::try_new(
                &transformation_authority(),
                python_family.clone(),
                DerivedAnalysisDomain::Python,
                DerivedFamilyKind::Fact,
                algorithm(&python),
                python_precision.clone(),
                ProgrammaticRelationId::new(python_relation),
                python.dependencies.clone(),
            )
            .unwrap(),
            AcceptedDerivedFamily::try_new(
                &transformation_authority(),
                rust_family.clone(),
                DerivedAnalysisDomain::RustMir,
                DerivedFamilyKind::Fact,
                algorithm(&rust),
                DerivedPrecisionPolicy::Exact,
                ProgrammaticRelationId::new(rust_relation),
                rust.dependencies.clone(),
            )
            .unwrap(),
            AcceptedDerivedFamily::try_new(
                &transformation_authority(),
                common_family.clone(),
                DerivedAnalysisDomain::Common,
                DerivedFamilyKind::Fact,
                algorithm(&common),
                DerivedPrecisionPolicy::SoundMay,
                ProgrammaticRelationId::new(common_relation),
                common.dependencies.clone(),
            )
            .unwrap(),
            AcceptedDerivedFamily::try_new(
                &transformation_authority(),
                recursive_family.clone(),
                DerivedAnalysisDomain::Common,
                DerivedFamilyKind::Fact,
                recursive_algorithm.clone(),
                DerivedPrecisionPolicy::Bounded {
                    max_steps: NonZeroU32::new(32).unwrap(),
                },
                ProgrammaticRelationId::new(remainder_relation),
                vec![ProgrammaticRelationId::new(common_relation)],
            )
            .unwrap(),
        ];
        let dispositions = vec![
            DerivedFamilyDisposition::Producer(AcceptedDerivedProducer::new(
                &transformation_authority(),
                python_family,
                DerivedProducerAuthority::ApplicationOwned([101; 32]),
                algorithm(&python),
                python_precision,
                DerivedCompletenessPolicy::Complete,
                ProgrammaticFieldId::new("derived.python.flow.fixture.support"),
                python,
            )),
            DerivedFamilyDisposition::Producer(AcceptedDerivedProducer::new(
                &transformation_authority(),
                rust_family,
                DerivedProducerAuthority::ApplicationOwned([102; 32]),
                algorithm(&rust),
                DerivedPrecisionPolicy::Exact,
                DerivedCompletenessPolicy::Complete,
                ProgrammaticFieldId::new("derived.rust_mir.flow.fixture.support"),
                rust,
            )),
            DerivedFamilyDisposition::Producer(AcceptedDerivedProducer::new(
                &transformation_authority(),
                common_family,
                DerivedProducerAuthority::ApplicationOwned([103; 32]),
                algorithm(&common),
                DerivedPrecisionPolicy::SoundMay,
                DerivedCompletenessPolicy::Complete,
                ProgrammaticFieldId::new("derived.common.graph.fixture.support"),
                common,
            )),
            DerivedFamilyDisposition::Remainder(
                ExplicitDerivedRemainder::try_new(
                    &transformation_authority(),
                    recursive_family,
                    recursive_algorithm,
                    DerivedRemainderReason::AlgorithmUnavailable,
                    [104; 32],
                    DerivedRemainderRetryability::RequiresReleaseChange,
                )
                .unwrap(),
            ),
        ];
        ProgrammaticDerivedAnalysisComposition::try_new(
            &transformation_authority(),
            families,
            dispositions,
            metadata.clone(),
            remainder_binding(&metadata),
        )
        .unwrap()
    }

    fn admitted() -> ProgrammaticProviderAdmissionOutcome {
        let fixture = exact_workspace_fixture();
        admit_provider_relations_programmatic(programmatic_epoch_builder(), fixture.runs()).unwrap()
    }

    async fn collect_relation(
        sealed: &crate::fabric::programmatic_schema::SealedProgrammaticSchemaAssembly,
        relation_id: &ProgrammaticRelationId,
    ) -> Vec<arrow_array::RecordBatch> {
        let binding = sealed.relation(relation_id).expect("relation is sealed");
        sealed
            .session()
            .table(binding.table_reference.clone())
            .await
            .unwrap()
            .collect()
            .await
            .unwrap()
    }

    fn rust_control_batch(
        relation: RustcRelation,
        predicate_marker: u8,
        include_predicate: bool,
    ) -> RecordBatch {
        let schema = relation.schema();
        let row_count = match relation {
            RustcRelation::CfgEdge => 3,
            RustcRelation::MirOperand if !include_predicate => 0,
            RustcRelation::MirBlock | RustcRelation::MirOperand | RustcRelation::MirTerminator => 1,
            _ => panic!("unsupported control fixture relation {relation:?}"),
        };
        let columns = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Utf8 => {
                    let values = (0..row_count)
                        .map(|row| match field.name().as_str() {
                            "provider_run_id" => Some("run:rust-control".to_owned()),
                            "compilation_unit_id" => Some("unit:rust-control".to_owned()),
                            "owner_id" => Some("owner:rust-control".to_owned()),
                            "source_file_id" => Some("file:rust-control".to_owned()),
                            "terminator_kind" | "raw_terminator_kind" => {
                                Some("SwitchInt".to_owned())
                            }
                            "slot_kind" => Some("terminator".to_owned()),
                            "parent_role" => Some("switch-discriminant".to_owned()),
                            "operand_kind" => Some("Copy".to_owned()),
                            "edge_kind" => Some(
                                match row {
                                    0 => "SwitchIntValue",
                                    1 => "SwitchIntOtherwise",
                                    _ => "Unwind",
                                }
                                .to_owned(),
                            ),
                            "branch_value_u128" => (row < 2).then(|| row.to_string()),
                            "unwind_action" => (relation == RustcRelation::MirTerminator
                                || row == 2)
                                .then(|| "Cleanup".to_owned()),
                            _ => Some("fixture".to_owned()),
                        })
                        .collect::<Vec<_>>();
                    Arc::new(StringArray::from(values)) as ArrayRef
                }
                DataType::UInt64 => {
                    let values = (0..row_count)
                        .map(|row| match field.name().as_str() {
                            "stable_crate_id" => 77,
                            "source_generation" => 3,
                            "block_index" | "source_block" => 7,
                            "target_block" => match row {
                                0 => 8,
                                1 => 9,
                                _ => 99,
                            },
                            "normal_target_count" => 2,
                            "source_scope" => 5,
                            "slot_index" | "operand_ordinal" | "statement_count" => 0,
                            _ => 1,
                        })
                        .collect::<Vec<_>>();
                    Arc::new(UInt64Array::from(values)) as ArrayRef
                }
                DataType::Boolean => {
                    Arc::new(BooleanArray::from(vec![true; row_count])) as ArrayRef
                }
                DataType::FixedSizeBinary(width @ (16 | 32)) => {
                    let mut builder = FixedSizeBinaryBuilder::with_capacity(row_count, *width);
                    for row in 0..row_count {
                        let marker = match field.name().as_str() {
                            "source_content_digest" => 41,
                            "def_path_hash" => 42,
                            "operand_id" => predicate_marker,
                            _ => 50_u8.saturating_add(u8::try_from(row).unwrap_or(u8::MAX)),
                        };
                        builder
                            .append_value(vec![marker; usize::try_from(*width).unwrap()])
                            .unwrap();
                    }
                    Arc::new(builder.finish()) as ArrayRef
                }
                data_type => panic!("unexpected control fixture type {data_type:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).unwrap()
    }

    fn rust_structural_batch(relation: RustcRelation, marker: u8) -> RecordBatch {
        let schema = relation.schema();
        let columns = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Utf8 => {
                    let value = match field.name().as_str() {
                        "provider_run_id" => format!("run:rust-structural:{marker}"),
                        "compilation_unit_id" => format!("unit:rust-structural:{marker}"),
                        "owner_id" => format!("owner:rust-structural:{marker}"),
                        "source_file_id" => format!("file:rust-structural:{marker}"),
                        "slot_kind" => "statement".to_owned(),
                        "projection_kind" => "BaseLocal".to_owned(),
                        "occurrence_role" => "fixture-place".to_owned(),
                        "local_role" => "temporary".to_owned(),
                        "mutability" if relation == RustcRelation::MirLocal => "mut".to_owned(),
                        "rvalue_kind" => "Ref".to_owned(),
                        "cast_kind" => "PtrToPtr".to_owned(),
                        "aggregate_kind" => "Coroutine".to_owned(),
                        "region_kind" => "fixture-region".to_owned(),
                        "raw_statement_kind" => "Assign".to_owned(),
                        "normalized_effect" => "WRITE".to_owned(),
                        "raw_terminator_kind" => "InlineAsm".to_owned(),
                        "edge_kind" => "Goto".to_owned(),
                        "access_kind" => "Drop".to_owned(),
                        "structured_evidence" => "StatementKind::Assign.destination".to_owned(),
                        "declared_target" => "fixture::foreign_call".to_owned(),
                        "dispatch_kind" => "direct".to_owned(),
                        "resolution_confidence" => "exact".to_owned(),
                        _ => "fixture".to_owned(),
                    };
                    Arc::new(StringArray::from(vec![value])) as ArrayRef
                }
                DataType::UInt64 => {
                    let value = match field.name().as_str() {
                        "stable_crate_id" => u64::from(marker).saturating_add(100),
                        "source_generation" => u64::from(marker),
                        "block_index" | "source_block" | "statement_index" | "slot_index"
                        | "base_local" | "local_index" => 3,
                        "target_block" => 4,
                        "projection_ordinal" | "access_ordinal" | "occurrence_ordinal" => 0,
                        _ => 1,
                    };
                    Arc::new(UInt64Array::from(vec![value])) as ArrayRef
                }
                DataType::Boolean => Arc::new(BooleanArray::from(vec![true])) as ArrayRef,
                DataType::FixedSizeBinary(width @ (16 | 32)) => {
                    let field_marker = match field.name().as_str() {
                        "source_content_digest" => marker.wrapping_add(1),
                        "def_path_hash" => marker.wrapping_add(2),
                        "place_id" | "source_place_id" | "destination_place_id" => {
                            marker.wrapping_add(3)
                        }
                        "instance_key" | "resolved_instance_key" => marker.wrapping_add(4),
                        _ => marker.wrapping_add(5),
                    };
                    let mut builder = FixedSizeBinaryBuilder::with_capacity(1, *width);
                    builder
                        .append_value(vec![field_marker; usize::try_from(*width).unwrap()])
                        .unwrap();
                    Arc::new(builder.finish()) as ArrayRef
                }
                data_type => panic!("unexpected structural fixture type {data_type:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).unwrap()
    }

    fn rust_provider_input(relation: RustcRelation, batch: RecordBatch) -> ProviderInput {
        let relation_id = ProgrammaticRelationId::new(relation.relation_id());
        let table_reference = TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::RawRustc.as_str(),
            format!("control_fixture_{}", relation.family_code()),
        );
        let schema: SchemaRef = batch.schema();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let contract = Arc::new(
            SchemaContract::try_new(
                format!("control-fixture-{}", relation.family_code()),
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..schema.fields().len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .unwrap(),
        );
        ProviderInput::new(relation_id, table_reference, contract, provider)
    }

    async fn execute_rust_control_fixture(
        predicate_marker: u8,
        include_predicate: bool,
        epoch_marker: u8,
    ) -> (SealedProgrammaticSchemaAssembly, ProgrammaticRelationId) {
        let bindings = rust_mir_bindings("programmatic.rust_mir.control-fixture");
        let mut builder = programmatic_epoch_builder();
        for relation in [
            RustcRelation::MirBlock,
            RustcRelation::MirOperand,
            RustcRelation::MirTerminator,
            RustcRelation::CfgEdge,
        ] {
            builder
                .register_provider(rust_provider_input(
                    relation,
                    rust_control_batch(relation, predicate_marker, include_predicate),
                ))
                .unwrap();
        }

        let cfg_contract = transformation_contract("analysis.rust.control-fixture.cfg", 81, 64);
        let cfg_relation = ProgrammaticRelationId::new(
            bindings
                .relation_id(RustMirDerivedRelation::CfgEdge)
                .as_str(),
        );
        let cfg_output = programmatic_output(
            cfg_relation,
            "rust_control_fixture_cfg",
            "fixture.rust.control.cfg",
            ProgrammaticRustMirCfgEdgeTransformation::OUTPUT_FIELD_COUNT,
        );
        builder
            .add_transformation(Arc::new(
                ProgrammaticRustMirCfgEdgeTransformation::try_new(
                    &transformation_authority(),
                    cfg_contract,
                    cfg_output,
                    &bindings,
                )
                .unwrap(),
            ))
            .unwrap();

        let control_contract =
            transformation_contract("analysis.rust.control-fixture.input", 82, 64);
        let control_relation = ProgrammaticRelationId::new(
            bindings
                .relation_id(RustMirDerivedRelation::ControlDependenceInput)
                .as_str(),
        );
        let control_output = programmatic_output(
            control_relation.clone(),
            "rust_control_fixture_input",
            "fixture.rust.control.input",
            ProgrammaticRustMirControlInputTransformation::OUTPUT_FIELD_COUNT,
        );
        builder
            .add_transformation(Arc::new(
                ProgrammaticRustMirControlInputTransformation::try_new(
                    &transformation_authority(),
                    control_contract,
                    control_output,
                    &bindings,
                )
                .unwrap(),
            ))
            .unwrap();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        (
            assembly
                .seal(FabricEpochId::from_bytes([epoch_marker; 16]))
                .await
                .unwrap(),
            control_relation,
        )
    }

    async fn execute_rust_structural_fixture(
        marker: u8,
        epoch_marker: u8,
    ) -> (
        SealedProgrammaticSchemaAssembly,
        BTreeMap<RustMirDerivedRelation, ProgrammaticRelationId>,
    ) {
        let bindings = rust_mir_bindings("programmatic.rust_mir.structural-fixture");
        let mut builder = programmatic_epoch_builder();
        for relation in [
            RustcRelation::PublicItem,
            RustcRelation::Type,
            RustcRelation::Instance,
            RustcRelation::MirBody,
            RustcRelation::MirLocal,
            RustcRelation::MirPlace,
            RustcRelation::MirRvalue,
            RustcRelation::MirStatement,
            RustcRelation::MirTerminator,
            RustcRelation::CfgEdge,
            RustcRelation::Call,
            RustcRelation::Access,
            RustcRelation::Coverage,
            RustcRelation::Remainder,
        ] {
            builder
                .register_provider(rust_provider_input(
                    relation,
                    rust_structural_batch(relation, marker),
                ))
                .unwrap();
        }

        let cfg_relation = ProgrammaticRelationId::new(
            bindings
                .relation_id(RustMirDerivedRelation::CfgEdge)
                .as_str(),
        );
        let cfg_output = programmatic_output(
            cfg_relation.clone(),
            "rust_structural_fixture_cfg",
            "fixture.rust.structural.cfg",
            ProgrammaticRustMirCfgEdgeTransformation::OUTPUT_FIELD_COUNT,
        );
        builder
            .add_transformation(Arc::new(
                ProgrammaticRustMirCfgEdgeTransformation::try_new(
                    &transformation_authority(),
                    transformation_contract("analysis.rust.structural-fixture.cfg", 91, 64),
                    cfg_output,
                    &bindings,
                )
                .unwrap(),
            ))
            .unwrap();

        let roles = [
            RustMirDerivedRelation::OwnershipState,
            RustMirDerivedRelation::AliasPointsTo,
            RustMirDerivedRelation::ResourceLifecycle,
            RustMirDerivedRelation::AsyncLowering,
            RustMirDerivedRelation::UnsafeFfi,
        ];
        let mut relation_ids = BTreeMap::new();
        for (index, role) in roles.into_iter().enumerate() {
            let relation_id = ProgrammaticRelationId::new(bindings.relation_id(role).as_str());
            let output = programmatic_output(
                relation_id.clone(),
                &format!("rust_structural_fixture_{index}"),
                &format!("fixture.rust.structural.{index}"),
                ProgrammaticRustMirStructuralTransformation::output_field_count(role),
            );
            builder
                .add_transformation(Arc::new(
                    ProgrammaticRustMirStructuralTransformation::try_new(
                        &transformation_authority(),
                        role,
                        transformation_contract(
                            &format!("analysis.rust.structural-fixture.{index}"),
                            92_u8.saturating_add(u8::try_from(index).unwrap()),
                            64,
                        ),
                        output,
                        &bindings,
                    )
                    .unwrap(),
                ))
                .unwrap();
            assert!(relation_ids.insert(role, relation_id).is_none());
        }
        let (_, _, _, assembly) = builder.into_assembly_parts();
        (
            assembly
                .seal(FabricEpochId::from_bytes([epoch_marker; 16]))
                .await
                .unwrap(),
            relation_ids,
        )
    }

    async fn execute_existing_fixture(
        fixture: &ExactWorkspaceFixture,
        epoch_marker: u8,
    ) -> (
        SealedProgrammaticSchemaAssembly,
        DerivedAnalysisCompositionObservation,
        ExistingDerivedAnalysisCensusObservation,
        ExistingProducerRelations,
    ) {
        let python = python_flow_bindings();
        let rust = rust_mir_bindings("programmatic.rust_mir");
        let common = common_analysis_bindings();
        let (census, relations) = existing_census(&python, &rust, &common);
        let metadata = metadata_bindings();
        let outcome = admit_and_compose_existing_programmatic_derived_analyses(
            &transformation_authority(),
            programmatic_epoch_builder(),
            fixture.runs(),
            census,
            metadata.clone(),
            remainder_binding(&metadata),
        )
        .unwrap();
        let (derived, census_observation) = outcome.into_parts();
        let observation = derived.observation().clone();
        let (builder, _, _) = derived.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly
            .seal(FabricEpochId::from_bytes([epoch_marker; 16]))
            .await
            .unwrap();
        (sealed, observation, census_observation, relations)
    }

    #[tokio::test]
    async fn existing_family_census_executes_fifteen_real_catalog_input_producers() {
        let fixture = exact_workspace_fixture();
        let (sealed, observation, census, relations) = execute_existing_fixture(&fixture, 91).await;

        assert_eq!(census.accepted_roles.len(), 38);
        assert_eq!(census.programmatic_producer_roles.len(), 15);
        assert_eq!(census.explicit_remainder_roles.len(), 23);
        assert_eq!(census.dependency_contracts.len(), 38);
        assert_eq!(census.common_semantic_identities.len(), 10);
        assert_eq!(observation.producers.len(), 15);
        assert_eq!(observation.remainders.len(), 23);
        assert_eq!(
            census
                .programmatic_producer_roles
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgNode),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgEdge),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::EvaluationOrder),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::DefUse),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::ReachingDefinition),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::Liveness),
                ExistingDerivedFamilyRole::Python(PythonDerivedRelation::ValueFlow),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::CfgEdge),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::OwnershipState),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::AliasPointsTo),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::ResourceLifecycle),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::AsyncLowering),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::UnsafeFfi),
                ExistingDerivedFamilyRole::RustMir(RustMirDerivedRelation::ControlDependenceInput,),
                ExistingDerivedFamilyRole::Common(ExistingCommonDerivedFamilyRole::CallGraph,),
            ])
        );
        assert!(observation.remainders.iter().all(|remainder| {
            remainder.reason == DerivedRemainderReason::TypedTransformationAdapterUnavailable
        }));

        let raw_python = collect_relation(
            &sealed,
            &ProgrammaticRelationId::new(NativeSyntaxRelation::RuffAstNode.as_str()),
        )
        .await;
        let mut evaluable_nodes_by_file = BTreeMap::<Vec<u8>, usize>::new();
        for batch in &raw_python {
            let file_ids = batch
                .column_by_name("file_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            let evaluation = batch
                .column_by_name("evaluation_ordinal")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt32Array>()
                .unwrap();
            for row in 0..batch.num_rows() {
                if !evaluation.is_null(row) {
                    *evaluable_nodes_by_file
                        .entry(file_ids.value(row).to_vec())
                        .or_default() += 1;
                }
            }
        }
        let evaluable_python_count = evaluable_nodes_by_file.values().sum::<usize>();
        let python_node_rows = collect_relation(&sealed, &relations.python_cfg_node).await;
        assert_eq!(
            python_node_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            evaluable_python_count
        );
        assert!(evaluable_python_count > 0);
        assert!(python_node_rows.iter().all(|batch| {
            let complete = batch
                .column_by_name("analysis_completeness")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let identities = batch
                .column_by_name("node_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| {
                complete.value(row) == "complete"
                    && !identities.is_null(row)
                    && identities.value(row) != [0; 16]
            })
        }));
        let independent_python_edge_count = evaluable_nodes_by_file
            .values()
            .map(|nodes| nodes.saturating_sub(1))
            .sum::<usize>();
        let python_rows = collect_relation(&sealed, &relations.python_cfg).await;
        assert_eq!(
            python_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            independent_python_edge_count
        );
        assert!(independent_python_edge_count > 0);
        let evaluation_rows = collect_relation(&sealed, &relations.python_evaluation_order).await;
        assert_eq!(
            evaluation_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            independent_python_edge_count
        );
        assert!(evaluation_rows.iter().all(|batch| {
            let kinds = batch
                .column_by_name("relation_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| kinds.value(row) == "NODE_EVALUATES_BEFORE")
        }));

        let rust_rows = collect_relation(&sealed, &relations.rust_mir_cfg).await;
        assert_eq!(
            rust_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            2
        );
        let rust_sources = rust_rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("source_block")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let rust_targets = rust_rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("target_block")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(rust_sources, BTreeSet::from([41, 42]));
        assert_eq!(rust_targets, BTreeSet::from([42, 43]));
        assert!(rust_rows.iter().all(|batch| {
            let canonical = batch
                .column_by_name("canonical_identity_available")
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();
            (0..canonical.len()).all(|row| canonical.value(row))
        }));

        let rust_ownership = collect_relation(&sealed, &relations.rust_mir_ownership).await;
        let rust_alias = collect_relation(&sealed, &relations.rust_mir_alias).await;
        let rust_resource = collect_relation(&sealed, &relations.rust_mir_resource).await;
        let rust_async = collect_relation(&sealed, &relations.rust_mir_async).await;
        let rust_unsafe = collect_relation(&sealed, &relations.rust_mir_unsafe_ffi).await;
        let rust_row_count =
            |batches: &[RecordBatch]| batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        assert_eq!(rust_row_count(&rust_ownership), 2);
        assert_eq!(rust_row_count(&rust_alias), 2);
        assert_eq!(rust_row_count(&rust_resource), 2);
        assert_eq!(rust_row_count(&rust_async), 2);
        assert_eq!(rust_row_count(&rust_unsafe), 6);
        assert!(rust_ownership.iter().all(|batch| {
            let native = batch
                .column_by_name("access_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let normalized = batch
                .column_by_name("ownership_observation")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let locations = batch
                .column_by_name("memory_location_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| {
                native.value(row) == "Drop"
                    && normalized.value(row) == "DROP_OBSERVED"
                    && !locations.is_null(row)
            })
        }));
        assert!(rust_alias.iter().all(|batch| {
            let native = batch
                .column_by_name("rvalue_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let normalized = batch
                .column_by_name("relation_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows())
                .all(|row| native.value(row) == "Ref" && normalized.value(row) == "MAY_POINT_TO")
        }));
        assert!(rust_resource.iter().all(|batch| {
            let lifecycle = batch
                .column_by_name("lifecycle_event")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| lifecycle.value(row) == "DROP_EXECUTED")
        }));
        assert!(rust_async.iter().all(|batch| {
            let aggregate = batch
                .column_by_name("aggregate_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let observation = batch
                .column_by_name("observation_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| {
                aggregate.value(row) == "Coroutine"
                    && observation.value(row) == "COROUTINE_AGGREGATE_OBSERVED"
            })
        }));
        let unsafe_kinds = rust_unsafe
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("observation_kind")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unsafe_kinds,
            BTreeSet::from([
                "FOREIGN_CALL".to_owned(),
                "INLINE_ASM".to_owned(),
                "UNSAFE_RELEVANT_CAST".to_owned(),
            ])
        );
        let alias_plan = sealed
            .observations()
            .provenance(&relations.rust_mir_alias)
            .and_then(ProvenanceObservation::logical_plan)
            .unwrap()
            .display_indent()
            .to_string();
        for operator in ["Aggregate", "Inner Join", "Filter", "Sort"] {
            assert!(alias_plan.contains(operator), "{alias_plan}");
        }

        let common_rows = collect_relation(&sealed, &relations.common_call_graph).await;
        assert_eq!(
            common_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            2
        );
        let common_bindings = common_analysis_bindings();
        let subjects = common_rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name(common_bindings.fields.subject_id.as_str())
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let objects = common_rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name(common_bindings.fields.object_id.as_str())
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            subjects,
            BTreeSet::from([
                "module:pyrefly:31".to_owned(),
                "module:pyrefly:32".to_owned(),
            ])
        );
        assert_eq!(objects, BTreeSet::from(["fixture.target".to_owned()]));
        assert!(common_rows.iter().all(|batch| {
            let complete = batch
                .column_by_name(common_bindings.fields.complete.as_str())
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();
            (0..complete.len()).all(|row| complete.value(row))
        }));

        let remainder_rows = collect_relation(&sealed, &observation.remainder_relation_id).await;
        assert_eq!(
            remainder_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            23
        );
        let baseline_def_use_count = collect_relation(&sealed, &relations.python_def_use)
            .await
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>();

        let changed = changed_exact_workspace_fixture();
        let (changed_sealed, changed_observation, changed_census, changed_relations) =
            execute_existing_fixture(&changed, 92).await;
        assert_eq!(changed_census.programmatic_producer_roles.len(), 15);
        assert_eq!(changed_census.explicit_remainder_roles.len(), 23);
        assert_eq!(changed_observation.producers.len(), 15);
        assert_eq!(changed_observation.remainders.len(), 23);

        let def_use = collect_relation(&changed_sealed, &changed_relations.python_def_use).await;
        let reaching = collect_relation(
            &changed_sealed,
            &changed_relations.python_reaching_definition,
        )
        .await;
        let liveness = collect_relation(&changed_sealed, &changed_relations.python_liveness).await;
        let value_flow =
            collect_relation(&changed_sealed, &changed_relations.python_value_flow).await;
        let row_count =
            |batches: &[RecordBatch]| batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        let def_use_count = row_count(&def_use);
        let reaching_count = row_count(&reaching);
        let liveness_count = row_count(&liveness);
        let value_flow_count = row_count(&value_flow);
        assert!(def_use_count > 0);
        assert_ne!(baseline_def_use_count, def_use_count);
        assert!(reaching_count > 0 && reaching_count <= def_use_count);
        assert!(liveness_count > 0);
        assert_eq!(value_flow_count, reaching_count);
        for (batches, expected_kind) in [
            (&def_use, "def_use"),
            (&reaching, "reaching_definition"),
            (&value_flow, "value_flow"),
        ] {
            assert!(batches.iter().all(|batch| {
                let kinds = batch
                    .column_by_name("relation_kind")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                let edges = batch
                    .column_by_name("edge_id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap();
                let definitions = batch
                    .column_by_name("definition_event_id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap();
                let uses = batch
                    .column_by_name("use_event_id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap();
                let algorithms = batch
                    .column_by_name("algorithm_release")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                let authorities = batch
                    .column_by_name("authority")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                let completeness = batch
                    .column_by_name("analysis_completeness")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..batch.num_rows()).all(|row| {
                    kinds.value(row) == expected_kind
                        && edges.value(row) != [0; 16]
                        && definitions.value(row) != [0; 16]
                        && uses.value(row) != [0; 16]
                        && definitions.value(row) != uses.value(row)
                        && algorithms.value(row).starts_with("codefabric.python-")
                        && authorities.value(row) == PYTHON_DERIVED_AUTHORITY
                        && completeness.value(row) == "complete"
                })
            }));
        }
        assert!(liveness.iter().all(|batch| {
            let boundaries = batch
                .column_by_name("boundary")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let kinds = batch
                .column_by_name("relation_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| {
                matches!(boundaries.value(row), "ENTRY" | "EXIT")
                    && matches!(kinds.value(row), "live_entry" | "live_exit")
            })
        }));

        let reaching_plan = changed_sealed
            .observations()
            .provenance(&changed_relations.python_reaching_definition)
            .and_then(ProvenanceObservation::logical_plan)
            .unwrap()
            .display_indent()
            .to_string();
        for operator in ["Left Join", "Aggregate", "WindowAggr", "Filter"] {
            assert!(reaching_plan.contains(operator), "{reaching_plan}");
        }
    }

    #[tokio::test]
    async fn rust_control_input_executes_native_joins_and_preserves_controller_semantics() {
        let (sealed, relation_id) = execute_rust_control_fixture(61, true, 93).await;
        let rows = collect_relation(&sealed, &relation_id).await;
        assert_eq!(
            rows.iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            3
        );
        let targets = rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("target_block")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(targets, BTreeSet::from([8, 9, 99]));
        assert!(rows.iter().all(|batch| {
            let controller = batch
                .column_by_name("controller_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let predicate = batch
                .column_by_name("predicate_operand_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            let identity = batch
                .column_by_name("control_input_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            (0..batch.num_rows()).all(|row| {
                controller.value(row) == "SwitchInt"
                    && predicate.value(row) == [61; 32]
                    && !identity.is_null(row)
            })
        }));
        let unwind = rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("is_unwind")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(unwind.iter().filter(|value| **value).count(), 1);

        let plan = sealed
            .observations()
            .provenance(&relation_id)
            .and_then(ProvenanceObservation::logical_plan)
            .unwrap()
            .display_indent()
            .to_string();
        assert!(plan.contains("Inner Join"), "{plan}");
        assert!(plan.contains("Left Join"), "{plan}");
        assert!(plan.contains("Filter"), "{plan}");
        assert!(plan.contains("Sort"), "{plan}");
    }

    #[tokio::test]
    async fn rust_control_input_faults_are_explicit_and_causal() {
        let (baseline, baseline_relation) = execute_rust_control_fixture(62, true, 94).await;
        let (changed, changed_relation) = execute_rust_control_fixture(63, true, 94).await;
        let (missing, missing_relation) = execute_rust_control_fixture(62, false, 94).await;
        let identities = |batches: &[RecordBatch]| {
            batches
                .iter()
                .flat_map(|batch| {
                    let values = batch
                        .column_by_name("control_input_id")
                        .unwrap()
                        .as_any()
                        .downcast_ref::<FixedSizeBinaryArray>()
                        .unwrap();
                    (0..values.len())
                        .map(|row| values.value(row).to_vec())
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>()
        };
        let baseline_rows = collect_relation(&baseline, &baseline_relation).await;
        let changed_rows = collect_relation(&changed, &changed_relation).await;
        assert_ne!(identities(&baseline_rows), identities(&changed_rows));

        let missing_rows = collect_relation(&missing, &missing_relation).await;
        assert_eq!(
            missing_rows
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            3,
            "an unavailable optional predicate must not erase accepted controller edges"
        );
        assert!(missing_rows.iter().all(|batch| {
            let predicate = batch
                .column_by_name("predicate_operand_id")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            (0..predicate.len()).all(|row| predicate.is_null(row))
        }));
    }

    #[tokio::test]
    async fn rust_structural_producers_execute_native_plans_and_change_with_exact_inputs() {
        let (baseline, relations) = execute_rust_structural_fixture(61, 96).await;
        let (changed, changed_relations) = execute_rust_structural_fixture(62, 97).await;
        let expected_rows = [
            (RustMirDerivedRelation::OwnershipState, 1),
            (RustMirDerivedRelation::AliasPointsTo, 1),
            (RustMirDerivedRelation::ResourceLifecycle, 1),
            (RustMirDerivedRelation::AsyncLowering, 1),
            (RustMirDerivedRelation::UnsafeFfi, 3),
        ];
        for (role, expected) in expected_rows {
            let relation = &relations[&role];
            let baseline_rows = collect_relation(&baseline, relation).await;
            let changed_rows = collect_relation(&changed, &changed_relations[&role]).await;
            assert_eq!(
                baseline_rows
                    .iter()
                    .map(RecordBatch::num_rows)
                    .sum::<usize>(),
                expected,
                "{role:?} did not emit independently expected structural rows"
            );
            let witness = match role {
                RustMirDerivedRelation::OwnershipState => "event_id",
                RustMirDerivedRelation::AliasPointsTo => "alias_observation_id",
                RustMirDerivedRelation::ResourceLifecycle => "lifecycle_event_id",
                RustMirDerivedRelation::AsyncLowering | RustMirDerivedRelation::UnsafeFfi => {
                    "observation_id"
                }
                _ => unreachable!(),
            };
            let identities = |batches: &[RecordBatch]| {
                batches
                    .iter()
                    .flat_map(|batch| {
                        let values = batch
                            .column_by_name(witness)
                            .unwrap()
                            .as_any()
                            .downcast_ref::<FixedSizeBinaryArray>()
                            .unwrap();
                        (0..values.len())
                            .map(|row| values.value(row).to_vec())
                            .collect::<BTreeSet<_>>()
                    })
                    .collect::<BTreeSet<_>>()
            };
            assert_ne!(
                identities(&baseline_rows),
                identities(&changed_rows),
                "{role:?} identity ignored changed exact MIR inputs"
            );
            assert!(baseline_rows.iter().all(|batch| {
                let canonical = batch
                    .column_by_name("canonical_identity_available")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap();
                (0..canonical.len()).all(|row| canonical.value(row))
            }));
        }

        let ownership = collect_relation(
            &baseline,
            &relations[&RustMirDerivedRelation::OwnershipState],
        )
        .await;
        assert!(ownership.iter().all(|batch| {
            let native = batch
                .column_by_name("access_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let normalized = batch
                .column_by_name("ownership_observation")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows())
                .all(|row| native.value(row) == "Drop" && normalized.value(row) == "DROP_OBSERVED")
        }));
        let unsafe_rows =
            collect_relation(&baseline, &relations[&RustMirDerivedRelation::UnsafeFfi]).await;
        let unsafe_kinds = unsafe_rows
            .iter()
            .flat_map(|batch| {
                let values = batch
                    .column_by_name("observation_kind")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                (0..values.len())
                    .map(|row| values.value(row).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unsafe_kinds,
            BTreeSet::from([
                "FOREIGN_CALL".to_owned(),
                "INLINE_ASM".to_owned(),
                "UNSAFE_RELEVANT_CAST".to_owned(),
            ])
        );
        let alias_plan = baseline
            .observations()
            .provenance(&relations[&RustMirDerivedRelation::AliasPointsTo])
            .and_then(ProvenanceObservation::logical_plan)
            .unwrap()
            .display_indent()
            .to_string();
        for operator in ["Aggregate", "Inner Join", "Filter", "Sort"] {
            assert!(alias_plan.contains(operator), "{alias_plan}");
        }
    }

    #[test]
    fn existing_family_dependency_contracts_are_closed_and_non_placeholder() {
        let python = python_flow_bindings();
        let rust = rust_mir_bindings("programmatic.rust_mir.dependencies");
        let common = common_analysis_bindings();
        let (census, _) = existing_census(&python, &rust, &common);
        let observation = census.observation();

        assert_eq!(observation.accepted_roles.len(), 38);
        assert_eq!(observation.programmatic_producer_roles.len(), 15);
        assert_eq!(observation.explicit_remainder_roles.len(), 23);
        assert_eq!(observation.dependency_contracts.len(), 38);
        let by_role = observation
            .dependency_contracts
            .iter()
            .map(|contract| (contract.role, contract.dependencies.as_ref()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(by_role.len(), 38);
        for role in observation.accepted_roles.iter() {
            let dependencies = by_role
                .get(role)
                .unwrap_or_else(|| panic!("{role:?} has no dependency contract"));
            assert!(!dependencies.is_empty(), "{role:?} is source-free");
            assert_eq!(
                dependencies.iter().collect::<BTreeSet<_>>().len(),
                dependencies.len(),
                "{role:?} repeats a dependency"
            );
        }
        for role in observation.explicit_remainder_roles.iter() {
            assert!(
                !by_role[role].is_empty(),
                "remainder role {role:?} erased its intended inputs"
            );
        }

        assert_eq!(
            by_role[&ExistingDerivedFamilyRole::Python(PythonDerivedRelation::Invalidation,)],
            [
                ProgrammaticRelationId::new(NativeSyntaxRelation::TreeSitterChangedRange.as_str(),),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffImport.as_str()),
                ProgrammaticRelationId::new(NativeSyntaxRelation::RuffExport.as_str()),
                ProgrammaticRelationId::new(PyreflyRelation::AffectedModule.relation_id()),
            ]
        );
        assert_eq!(
            by_role[&ExistingDerivedFamilyRole::RustMir(
                RustMirDerivedRelation::ControlDependenceInput,
            )],
            [
                ProgrammaticRelationId::new(RustcRelation::MirBlock.relation_id()),
                ProgrammaticRelationId::new(RustcRelation::MirOperand.relation_id()),
                ProgrammaticRelationId::new(RustcRelation::MirTerminator.relation_id()),
                ProgrammaticRelationId::new(
                    rust.relation_id(RustMirDerivedRelation::CfgEdge).as_str(),
                ),
            ]
        );
        assert_eq!(
            by_role[&ExistingDerivedFamilyRole::Common(
                ExistingCommonDerivedFamilyRole::CallableSummary,
            )],
            [
                ProgrammaticRelationId::new(common.relations.call_targets.as_str()),
                ProgrammaticRelationId::new(common.relations.local_semantics.as_str()),
            ]
        );
    }

    #[test]
    fn existing_family_census_rejects_missing_and_duplicate_roles() {
        let python = python_flow_bindings();
        let rust = rust_mir_bindings("programmatic.rust_mir.negative");
        let common = common_analysis_bindings();
        let (mut missing, _) = existing_declarations(&python, &rust, &common);
        let missing_role = missing.pop().unwrap().role;
        assert!(matches!(
            ExistingDerivedAnalysisCensus::try_new(
                &transformation_authority(),
                &python,
                &rust,
                &common,
                missing,
            ),
            Err(ProgrammaticDerivedAnalysisError::MissingExistingCensusRole(role))
                if role == missing_role
        ));

        let (mut duplicate, _) = existing_declarations(&python, &rust, &common);
        let duplicate_role = duplicate[0].role;
        duplicate.push(duplicate[0].clone());
        assert!(matches!(
            ExistingDerivedAnalysisCensus::try_new(
                &transformation_authority(),
                &python,
                &rust,
                &common,
                duplicate,
            ),
            Err(ProgrammaticDerivedAnalysisError::DuplicateExistingCensusRole(role))
                if role == duplicate_role
        ));
    }

    #[test]
    fn existing_family_census_rejects_dependency_contract_drift() {
        let python = python_flow_bindings();
        let rust = rust_mir_bindings("programmatic.rust_mir.dependency-drift");
        let common = common_analysis_bindings();
        let (mut declarations, _) = existing_declarations(&python, &rust, &common);
        let drifted = declarations
            .iter_mut()
            .find(|declaration| {
                declaration.role
                    == ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgNode)
            })
            .expect("closed census includes Python CFG nodes");
        drifted.dependencies = Arc::from([]);

        assert!(matches!(
            ExistingDerivedAnalysisCensus::try_new(
                &transformation_authority(),
                &python,
                &rust,
                &common,
                declarations,
            ),
            Err(
                ProgrammaticDerivedAnalysisError::ExistingCensusDependencyMismatch(
                    ExistingDerivedFamilyRole::Python(PythonDerivedRelation::CfgNode),
                )
            )
        ));
    }

    #[tokio::test]
    async fn changed_catalog_inputs_causally_change_real_producer_outputs() {
        let baseline = exact_workspace_fixture();
        let changed = changed_exact_workspace_fixture();
        let (baseline_sealed, baseline_observation, _, baseline_relations) =
            execute_existing_fixture(&baseline, 92).await;
        let (changed_sealed, changed_observation, _, changed_relations) =
            execute_existing_fixture(&changed, 92).await;

        let producer_authority = |observation: &DerivedAnalysisCompositionObservation,
                                  domain: DerivedAnalysisDomain| {
            observation
                .producers
                .iter()
                .find(|producer| producer.domain == domain)
                .unwrap()
                .provenance_closure_identity
        };
        for domain in [
            DerivedAnalysisDomain::Python,
            DerivedAnalysisDomain::RustMir,
            DerivedAnalysisDomain::Common,
        ] {
            assert_ne!(
                producer_authority(&baseline_observation, domain),
                producer_authority(&changed_observation, domain)
            );
        }

        let fixed_identity_values = |batches: &[arrow_array::RecordBatch], name: &str| {
            batches
                .iter()
                .flat_map(|batch| {
                    let values = batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<FixedSizeBinaryArray>()
                        .unwrap();
                    (0..values.len())
                        .filter(|row| !values.is_null(*row))
                        .map(|row| values.value(row).to_vec())
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>()
        };
        let baseline_python =
            collect_relation(&baseline_sealed, &baseline_relations.python_cfg).await;
        let changed_python = collect_relation(&changed_sealed, &changed_relations.python_cfg).await;
        let python_bindings = python_flow_bindings();
        let baseline_python_nodes =
            collect_relation(&baseline_sealed, &baseline_relations.python_cfg_node).await;
        let changed_python_nodes =
            collect_relation(&changed_sealed, &changed_relations.python_cfg_node).await;
        assert_ne!(
            fixed_identity_values(
                &baseline_python_nodes,
                python_bindings.fields.node_id.as_ref()
            ),
            fixed_identity_values(
                &changed_python_nodes,
                python_bindings.fields.node_id.as_ref()
            )
        );
        assert_ne!(
            fixed_identity_values(&baseline_python, python_bindings.fields.edge_id.as_ref()),
            fixed_identity_values(&changed_python, python_bindings.fields.edge_id.as_ref())
        );

        let u64_values = |batches: &[arrow_array::RecordBatch], name: &str| {
            batches
                .iter()
                .flat_map(|batch| {
                    let values = batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap();
                    (0..values.len())
                        .map(|row| values.value(row))
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>()
        };
        let baseline_rust =
            collect_relation(&baseline_sealed, &baseline_relations.rust_mir_cfg).await;
        let changed_rust = collect_relation(&changed_sealed, &changed_relations.rust_mir_cfg).await;
        assert_ne!(
            u64_values(&baseline_rust, "source_block"),
            u64_values(&changed_rust, "source_block")
        );

        let string_values = |batches: &[arrow_array::RecordBatch], name: &str| {
            batches
                .iter()
                .flat_map(|batch| {
                    let values = batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .unwrap();
                    (0..values.len())
                        .map(|row| values.value(row).to_owned())
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>()
        };
        let baseline_common =
            collect_relation(&baseline_sealed, &baseline_relations.common_call_graph).await;
        let changed_common =
            collect_relation(&changed_sealed, &changed_relations.common_call_graph).await;
        let common_bindings = common_analysis_bindings();
        assert_ne!(
            string_values(&baseline_common, common_bindings.fields.value_id.as_str()),
            string_values(&changed_common, common_bindings.fields.value_id.as_str())
        );
    }

    #[tokio::test]
    async fn compiled_release_admits_all_domains_and_queryable_remainder() {
        let fixture = exact_workspace_fixture();
        let release = crate::fabric::production_kernel::CompiledSemanticRelease::current();
        let outcome = release
            .admit_and_compose_derived_analyses(
                programmatic_epoch_builder(),
                fixture.runs(),
                composition(DerivedPrecisionPolicy::Exact, 4_096),
            )
            .unwrap();
        assert_eq!(outcome.observation().producers.len(), 3);
        assert_eq!(outcome.observation().remainders.len(), 1);
        assert_eq!(
            outcome
                .observation()
                .producers
                .iter()
                .map(|producer| producer.domain)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                DerivedAnalysisDomain::Python,
                DerivedAnalysisDomain::RustMir,
                DerivedAnalysisDomain::Common,
            ])
        );
        assert!(outcome.observation().producers.iter().all(|producer| {
            !producer.inputs.is_empty()
                && producer.input_vector_identity != [0; 32]
                && producer.provenance_closure_identity != [0; 32]
        }));
        assert_eq!(
            outcome
                .observation()
                .producers
                .iter()
                .flat_map(|producer| producer.inputs.iter())
                .filter_map(|input| match input.source {
                    DerivedInputAuthoritySource::Provider(lane) => Some(lane_code(lane)),
                    DerivedInputAuthoritySource::Derived(_)
                    | DerivedInputAuthoritySource::DeclaredRemainder(_) => None,
                })
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 2, 3])
        );

        let observation = outcome.observation().clone();
        let (builder, _, _) = outcome.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly
            .seal(FabricEpochId::from_bytes([90; 16]))
            .await
            .unwrap();
        for producer in &observation.producers {
            let batches = collect_relation(&sealed, &producer.output_relation_id).await;
            assert!(
                batches
                    .iter()
                    .map(arrow_array::RecordBatch::num_rows)
                    .sum::<usize>()
                    > 0
            );
            let provenance = batches
                .iter()
                .find(|batch| batch.num_rows() != 0)
                .unwrap()
                .column_by_name("__cf_derived_provenance")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            assert_eq!(provenance.value(0), producer.provenance_closure_identity);
            let observed_contract = sealed
                .observations()
                .provenance(&producer.output_relation_id)
                .and_then(ProvenanceObservation::transformation_contract)
                .unwrap();
            assert_eq!(
                observed_contract
                    .provenance()
                    .provenance_identity()
                    .as_bytes(),
                &producer.provenance_closure_identity
            );
        }
        let remainder_batches = collect_relation(&sealed, &observation.remainder_relation_id).await;
        assert_eq!(
            remainder_batches
                .iter()
                .map(arrow_array::RecordBatch::num_rows)
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn closure_rejects_missing_duplicate_or_provider_owned_dispositions() {
        let mut missing = composition(DerivedPrecisionPolicy::Exact, 4_096);
        let removed = missing.dispositions.pop().unwrap();
        let missing_id = removed.family_id().clone();
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                missing,
            ),
            Err(ProgrammaticDerivedAnalysisError::MissingFamilyDisposition(id)) if id == missing_id
        ));

        let mut duplicate = composition(DerivedPrecisionPolicy::Exact, 4_096);
        duplicate
            .dispositions
            .push(duplicate.dispositions[0].clone());
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                duplicate,
            ),
            Err(ProgrammaticDerivedAnalysisError::DuplicateFamilyDisposition(_))
        ));

        let mut provider_owned = composition(DerivedPrecisionPolicy::Exact, 4_096);
        let DerivedFamilyDisposition::Producer(producer) = &mut provider_owned.dispositions[0]
        else {
            unreachable!()
        };
        producer.authority = DerivedProducerAuthority::ProviderNative(ProviderNativeLane::Ruff);
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                provider_owned,
            ),
            Err(ProgrammaticDerivedAnalysisError::ProviderOwnedDerivedFamily { .. })
        ));
    }

    #[test]
    fn closure_rejects_orphans_and_partial_output_without_unknown_producer() {
        let mut orphan = composition(DerivedPrecisionPolicy::Exact, 4_096);
        let mut families = orphan.families.to_vec();
        let python = families
            .iter_mut()
            .find(|family| family.domain == DerivedAnalysisDomain::Python)
            .unwrap();
        python.dependencies = Arc::from([ProgrammaticRelationId::new("raw.orphan.fixture")]);
        orphan.families = families.into();
        assert!(matches!(
            compose_programmatic_derived_analyses(&transformation_authority(), admitted(), orphan,),
            Err(ProgrammaticDerivedAnalysisError::OrphanDependency { .. })
        ));

        let mut partial = composition(DerivedPrecisionPolicy::Exact, 4_096);
        let DerivedFamilyDisposition::Producer(producer) = &mut partial.dispositions[0] else {
            unreachable!()
        };
        producer.completeness = DerivedCompletenessPolicy::Partial {
            unknown_family: DerivedFamilyId::try_new(
                &transformation_authority(),
                "family.python.unknown.missing",
            )
            .unwrap(),
        };
        assert!(matches!(
            compose_programmatic_derived_analyses(&transformation_authority(), admitted(), partial,),
            Err(ProgrammaticDerivedAnalysisError::MissingUnknownFamily { .. })
        ));
    }

    #[test]
    fn precision_change_causally_changes_observed_producer_authority() {
        let exact = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            composition(DerivedPrecisionPolicy::Exact, 4_096),
        )
        .unwrap();
        let may = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            composition(DerivedPrecisionPolicy::SoundMay, 4_096),
        )
        .unwrap();
        let python = |observation: &DerivedAnalysisCompositionObservation| {
            observation
                .producers
                .iter()
                .find(|producer| producer.domain == DerivedAnalysisDomain::Python)
                .unwrap()
                .provenance_closure_identity
        };
        assert_ne!(python(exact.observation()), python(may.observation()));
    }

    #[test]
    fn aggregate_resource_envelope_rejects_zero_and_overcommit_before_registration() {
        for (resource, bounds) in [
            ("max_producers", [0, 1, 1, 1, 1, 1]),
            ("max_remainders", [1, 0, 1, 1, 1, 1]),
            ("max_dependency_edges", [1, 1, 0, 1, 1, 1]),
            ("max_declared_rows", [1, 1, 1, 0, 1, 1]),
            ("max_declared_memory_bytes", [1, 1, 1, 1, 0, 1]),
            ("max_declared_spill_bytes", [1, 1, 1, 1, 1, 0]),
        ] {
            assert!(matches!(
                DerivedAnalysisResourceEnvelope::try_new(
                    bounds[0], bounds[1], bounds[2], bounds[3], bounds[4], bounds[5],
                ),
                Err(ProgrammaticDerivedAnalysisError::ZeroResourceBound(observed))
                    if observed == resource
            ));
        }

        let mut producer_overcommit = composition(DerivedPrecisionPolicy::Exact, 4_096);
        producer_overcommit.resource_envelope = DerivedAnalysisResourceEnvelope::try_new(
            2,
            8,
            128,
            1_000_000,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        )
        .unwrap();
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                producer_overcommit,
            ),
            Err(
                ProgrammaticDerivedAnalysisError::CompositionResourceLimitExceeded {
                    resource: "producer_count",
                    observed: 3,
                    maximum: 2,
                }
            )
        ));

        let mut edge_overcommit = composition(DerivedPrecisionPolicy::Exact, 4_096);
        edge_overcommit.resource_envelope = DerivedAnalysisResourceEnvelope::try_new(
            8,
            8,
            6,
            1_000_000,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        )
        .unwrap();
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                edge_overcommit,
            ),
            Err(
                ProgrammaticDerivedAnalysisError::CompositionResourceLimitExceeded {
                    resource: "dependency_edge_count",
                    observed: 7,
                    maximum: 6,
                }
            )
        ));

        let mut row_overcommit = composition(DerivedPrecisionPolicy::Exact, 4_096);
        row_overcommit.resource_envelope = DerivedAnalysisResourceEnvelope::try_new(
            8,
            8,
            128,
            24_639,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        )
        .unwrap();
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                row_overcommit,
            ),
            Err(
                ProgrammaticDerivedAnalysisError::CompositionResourceLimitExceeded {
                    resource: "declared_max_rows",
                    observed: 24_640,
                    maximum: 24_639,
                }
            )
        ));

        let mut memory_overcommit = composition(DerivedPrecisionPolicy::Exact, 4_096);
        memory_overcommit.resource_envelope = DerivedAnalysisResourceEnvelope::try_new(
            8,
            8,
            128,
            1_000_000,
            128 * 1024 * 1024 - 1,
            1024 * 1024 * 1024,
        )
        .unwrap();
        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                memory_overcommit,
            ),
            Err(
                ProgrammaticDerivedAnalysisError::CompositionResourceLimitExceeded {
                    resource: "declared_max_memory_bytes",
                    observed: 134_217_728,
                    maximum: 134_217_727,
                }
            )
        ));

        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                composition(DerivedPrecisionPolicy::Exact, 0),
            ),
            Err(
                ProgrammaticDerivedAnalysisError::ZeroTransformationResourceBound {
                    resource: "max_rows",
                    ..
                }
            )
        ));
    }

    #[test]
    fn resource_contract_is_causal_in_composition_closure_observation() {
        let baseline = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            composition(DerivedPrecisionPolicy::Exact, 4_096),
        )
        .unwrap();
        let expanded = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            composition(DerivedPrecisionPolicy::Exact, 8_192),
        )
        .unwrap();

        assert_eq!(
            baseline.observation().resources,
            DerivedAnalysisResourceObservation {
                envelope: DerivedAnalysisResourceEnvelope::production(),
                producer_count: 3,
                remainder_count: 1,
                dependency_edge_count: 7,
                declared_max_rows: 24_640,
                declared_max_memory_bytes: 128 * 1024 * 1024,
                declared_max_spill_bytes: 0,
            }
        );
        assert_eq!(expanded.observation().resources.declared_max_rows, 28_736);
        assert_ne!(
            baseline.observation().closure_identity,
            expanded.observation().closure_identity
        );
        assert_ne!(
            baseline.observation().producers[0].provenance_closure_identity,
            expanded.observation().producers[0].provenance_closure_identity
        );

        let mut policy_only = composition(DerivedPrecisionPolicy::Exact, 4_096);
        policy_only.resource_envelope = DerivedAnalysisResourceEnvelope::try_new(
            8,
            8,
            128,
            1_000_000,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        )
        .unwrap();
        let policy_only = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            policy_only,
        )
        .unwrap();
        assert_eq!(
            baseline.observation().resources.declared_max_rows,
            policy_only.observation().resources.declared_max_rows
        );
        assert_eq!(
            baseline.observation().producers,
            policy_only.observation().producers
        );
        assert_ne!(
            baseline.observation().closure_identity,
            policy_only.observation().closure_identity
        );
    }

    #[test]
    fn later_duplicate_transformation_registration_returns_no_partial_builder() {
        let mut composition = composition(DerivedPrecisionPolicy::Exact, 4_096);
        let rust_index = composition
            .families
            .iter()
            .position(|family| family.domain == DerivedAnalysisDomain::RustMir)
            .unwrap();
        let rust_family_id = composition.families[rust_index].family_id.clone();
        let rust_dependencies = composition.families[rust_index].dependencies.clone();
        let replacement = transformation(
            "analysis.python.flow.fixture",
            31,
            4_096,
            "derived.rust_mir.flow.fixture",
            "rust_mir_flow_fixture",
            "rust_support",
            "derived.rust_mir.flow.fixture.support",
            rust_dependencies.to_vec(),
        );
        let replacement_algorithm = algorithm(&replacement);
        let mut families = composition.families.to_vec();
        families[rust_index].algorithm = replacement_algorithm.clone();
        composition.families = families.into();
        let DerivedFamilyDisposition::Producer(rust_producer) = composition
            .dispositions
            .iter_mut()
            .find(|disposition| disposition.family_id() == &rust_family_id)
            .unwrap()
        else {
            unreachable!()
        };
        rust_producer.algorithm = replacement_algorithm;
        rust_producer.transformation = replacement;

        assert!(matches!(
            compose_programmatic_derived_analyses(
                &transformation_authority(),
                admitted(),
                composition,
            ),
            Err(ProgrammaticDerivedAnalysisError::Epoch(
                ProgrammaticFabricEpochError::ProgrammaticSchema(
                    ProgrammaticSchemaError::DuplicateTransformation { .. }
                )
            ))
        ));
    }

    #[tokio::test]
    async fn execution_bound_aborts_seal_without_returning_partial_epoch() {
        let outcome = compose_programmatic_derived_analyses(
            &transformation_authority(),
            admitted(),
            composition(DerivedPrecisionPolicy::Exact, 1),
        )
        .unwrap();
        let (builder, _, _) = outcome.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        assert!(matches!(
            assembly.seal(FabricEpochId::from_bytes([90; 16])).await,
            Err(ProgrammaticSchemaError::TransformationOutputRowsExceeded { .. })
        ));
    }

    #[allow(dead_code)]
    fn _session_type_is_production(context: &SessionContext) -> &SessionContext {
        context
    }
}
