//! Programmatic assembly of one candidate DataFusion catalog.
//!
//! Provider contracts and native logical transformations are installed into one
//! candidate [`SessionContext`]. Output schemas are observed from the analyzed plan;
//! an optional caller schema is only an equality assertion and never supplies a field
//! type, nullability, or metadata to the registered view.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{
    ArrayRef, BooleanArray, Int64Array, RecordBatch, RecordBatchOptions, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, FieldRef, Schema, SchemaRef};
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::metadata::FieldMetadata;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::common::{Column, DFSchema, DFSchemaRef, DataFusionError, TableReference};
#[cfg(test)]
use datafusion::datasource::MemTable;
use datafusion::datasource::{ViewTable, provider_as_source};
use datafusion::execution::context::{SessionContext, SessionState, TaskContext};
use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::logical_expr::logical_plan::Projection;
use datafusion::logical_expr::{
    Expr, LogicalPlan, LogicalPlanBuilder, TableProviderFilterPushDown, TableType, Volatility,
};
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_expr::expressions::Column as PhysicalColumn;
use datafusion::physical_plan::metrics::MetricValue;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, ExecutionPlanProperties, PlanProperties,
    SendableRecordBatchStream, execute_stream,
};
use futures::StreamExt as _;
use thiserror::Error;

use super::command::EpochId;
use super::id16_array;
use super::{ResultChecksumError, result_checksum_v2};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SEMANTIC_ROLE_METADATA_KEY,
    SchemaContract, SchemaContractError, SchemaRole,
};

/// Stable identity of a relation installed in the candidate catalog.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgrammaticRelationId(Arc<str>);

impl ProgrammaticRelationId {
    /// Construct an identity. Empty identities are rejected when registered.
    #[must_use]
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    /// Return the identity as text without assigning meaning to its spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identity of a programmatic transformation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgrammaticTransformationId(Arc<str>);

impl ProgrammaticTransformationId {
    /// Construct an identity. Empty identities are rejected when registered.
    #[must_use]
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    /// Return the identity as text without assigning meaning to its spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Application-owned semantic version of one transformation contract.
///
/// `0.0.0` is reserved as an uninitialized sentinel and is rejected when the
/// transformation is registered.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformationSemanticVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl TransformationSemanticVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    #[must_use]
    pub const fn patch(self) -> u16 {
        self.patch
    }

    const fn is_sentinel(self) -> bool {
        self.major == 0 && self.minor == 0 && self.patch == 0
    }
}

/// Bounded execution resource class declared by a transformation.
///
/// The values are contract limits, not scheduler hints. A zero bound is
/// rejected at registration rather than being interpreted as unlimited.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationResourceClass {
    BoundedInMemory {
        max_rows: u64,
        max_memory_bytes: u64,
    },
    BoundedSpillable {
        max_rows: u64,
        max_memory_bytes: u64,
        max_spill_bytes: u64,
    },
}

impl TransformationResourceClass {
    #[must_use]
    pub const fn max_rows(self) -> u64 {
        match self {
            Self::BoundedInMemory { max_rows, .. } | Self::BoundedSpillable { max_rows, .. } => {
                max_rows
            }
        }
    }

    #[must_use]
    pub const fn max_memory_bytes(self) -> u64 {
        match self {
            Self::BoundedInMemory {
                max_memory_bytes, ..
            }
            | Self::BoundedSpillable {
                max_memory_bytes, ..
            } => max_memory_bytes,
        }
    }

    #[must_use]
    pub const fn max_spill_bytes(self) -> Option<u64> {
        match self {
            Self::BoundedInMemory { .. } => None,
            Self::BoundedSpillable {
                max_spill_bytes, ..
            } => Some(max_spill_bytes),
        }
    }
}

/// Reproducibility promise made by one transformation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationDeterminismPolicy {
    /// The row multiset is deterministic for pinned inputs; row order is not semantic.
    DeterministicSet,
    /// Both values and the declared output ordering are deterministic for pinned inputs.
    DeterministicSequence,
    /// The transformation explicitly contains volatile semantics.
    Volatile,
}

/// Direction of one declared output ordering key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationSortDirection {
    Ascending,
    Descending,
}

/// Null placement of one declared output ordering key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationNullPlacement {
    First,
    Last,
}

/// One stable field-identity key in an ordered transformation result.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformationOrderingKey {
    field_id: ProgrammaticFieldId,
    direction: TransformationSortDirection,
    null_placement: TransformationNullPlacement,
}

impl TransformationOrderingKey {
    #[must_use]
    pub const fn new(
        field_id: ProgrammaticFieldId,
        direction: TransformationSortDirection,
        null_placement: TransformationNullPlacement,
    ) -> Self {
        Self {
            field_id,
            direction,
            null_placement,
        }
    }

    #[must_use]
    pub const fn field_id(&self) -> &ProgrammaticFieldId {
        &self.field_id
    }

    #[must_use]
    pub const fn direction(&self) -> TransformationSortDirection {
        self.direction
    }

    #[must_use]
    pub const fn null_placement(&self) -> TransformationNullPlacement {
        self.null_placement
    }
}

/// Whether result order is semantic and, if so, the exact output-field keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationOrderingPolicy {
    Unordered,
    ByOutputFields(Arc<[TransformationOrderingKey]>),
}

/// Whether a transformation may invoke bounded native recursive semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformationRecursionPolicy {
    Forbidden,
    Bounded { max_iterations: u32 },
}

/// Digest identity of the producer/build provenance for a transformation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformationProvenanceIdentity([u8; 32]);

impl TransformationProvenanceIdentity {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Digest identity of the immutable release supplying a transformation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformationReleaseIdentity([u8; 32]);

impl TransformationReleaseIdentity {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Provenance closure roots for one transformation implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformationProvenance {
    provenance_identity: TransformationProvenanceIdentity,
    release_identity: TransformationReleaseIdentity,
}

impl TransformationProvenance {
    #[must_use]
    pub const fn new(
        provenance_identity: TransformationProvenanceIdentity,
        release_identity: TransformationReleaseIdentity,
    ) -> Self {
        Self {
            provenance_identity,
            release_identity,
        }
    }

    #[must_use]
    pub const fn provenance_identity(&self) -> TransformationProvenanceIdentity {
        self.provenance_identity
    }

    #[must_use]
    pub const fn release_identity(&self) -> TransformationReleaseIdentity {
        self.release_identity
    }
}

/// Immutable application authority for one typed transformation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgrammaticTransformationContract {
    semantic_id: ProgrammaticTransformationId,
    semantic_version: TransformationSemanticVersion,
    resource_class: TransformationResourceClass,
    determinism_policy: TransformationDeterminismPolicy,
    ordering_policy: TransformationOrderingPolicy,
    recursion_policy: TransformationRecursionPolicy,
    provenance: TransformationProvenance,
}

impl ProgrammaticTransformationContract {
    #[must_use]
    pub const fn new(
        semantic_id: ProgrammaticTransformationId,
        semantic_version: TransformationSemanticVersion,
        resource_class: TransformationResourceClass,
        determinism_policy: TransformationDeterminismPolicy,
        ordering_policy: TransformationOrderingPolicy,
        recursion_policy: TransformationRecursionPolicy,
        provenance: TransformationProvenance,
    ) -> Self {
        Self {
            semantic_id,
            semantic_version,
            resource_class,
            determinism_policy,
            ordering_policy,
            recursion_policy,
            provenance,
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
    pub const fn resource_class(&self) -> TransformationResourceClass {
        self.resource_class
    }

    #[must_use]
    pub const fn determinism_policy(&self) -> TransformationDeterminismPolicy {
        self.determinism_policy
    }

    #[must_use]
    pub const fn ordering_policy(&self) -> &TransformationOrderingPolicy {
        &self.ordering_policy
    }

    #[must_use]
    pub const fn recursion_policy(&self) -> TransformationRecursionPolicy {
        self.recursion_policy
    }

    #[must_use]
    pub const fn provenance(&self) -> TransformationProvenance {
        self.provenance
    }

    /// Application-owned fingerprint over every semantic contract field.
    ///
    /// DataFusion plan display/hash output is deliberately absent because it
    /// is engine-version-local diagnostic material, not semantic authority.
    #[must_use]
    pub fn authority_identity(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric.programmatic-transformation-contract.v1");
        update_contract_frame(&mut hasher, self.semantic_id.as_str().as_bytes());
        hasher.update(&self.semantic_version.major.to_be_bytes());
        hasher.update(&self.semantic_version.minor.to_be_bytes());
        hasher.update(&self.semantic_version.patch.to_be_bytes());
        hasher.update(&[resource_class_tag(self.resource_class)]);
        hasher.update(&self.resource_class.max_rows().to_be_bytes());
        hasher.update(&self.resource_class.max_memory_bytes().to_be_bytes());
        match self.resource_class.max_spill_bytes() {
            Some(max_spill_bytes) => {
                hasher.update(&[1]);
                hasher.update(&max_spill_bytes.to_be_bytes());
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hasher.update(&[determinism_policy_tag(self.determinism_policy)]);
        match &self.ordering_policy {
            TransformationOrderingPolicy::Unordered => {
                hasher.update(&[0]);
            }
            TransformationOrderingPolicy::ByOutputFields(keys) => {
                hasher.update(&[1]);
                hasher.update(&u64::try_from(keys.len()).unwrap_or(u64::MAX).to_be_bytes());
                for key in keys.iter() {
                    update_contract_frame(&mut hasher, key.field_id.as_str().as_bytes());
                    hasher.update(&[sort_direction_tag(key.direction)]);
                    hasher.update(&[null_placement_tag(key.null_placement)]);
                }
            }
        }
        match self.recursion_policy {
            TransformationRecursionPolicy::Forbidden => {
                hasher.update(&[0]);
            }
            TransformationRecursionPolicy::Bounded { max_iterations } => {
                hasher.update(&[1]);
                hasher.update(&max_iterations.to_be_bytes());
            }
        }
        hasher.update(self.provenance.provenance_identity.as_bytes());
        hasher.update(self.provenance.release_identity.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Stable field identity carried independently of an Arrow field name or ordinal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgrammaticFieldId(Arc<str>);

impl ProgrammaticFieldId {
    /// Construct an identity. Empty identities are rejected with their binding.
    #[must_use]
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identity-only declaration for one transformation output field.
///
/// Position binds the identity to a plan-derived field. Neither this row nor its
/// optional semantic role can supply a name, data type, nullability, or Arrow shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformationFieldIdentity {
    field_id: ProgrammaticFieldId,
    semantic_role: Option<Arc<str>>,
}

impl TransformationFieldIdentity {
    #[must_use]
    pub fn new(field_id: ProgrammaticFieldId) -> Self {
        Self {
            field_id,
            semantic_role: None,
        }
    }

    #[must_use]
    pub fn with_semantic_role(mut self, semantic_role: impl Into<Arc<str>>) -> Self {
        self.semantic_role = Some(semantic_role.into());
        self
    }

    #[must_use]
    pub const fn field_id(&self) -> &ProgrammaticFieldId {
        &self.field_id
    }

    #[must_use]
    pub const fn semantic_role(&self) -> Option<&Arc<str>> {
        self.semantic_role.as_ref()
    }
}

/// One exact provider input and its executable schema contract.
pub struct ProviderInput {
    pub relation_id: ProgrammaticRelationId,
    pub table_reference: TableReference,
    pub contract: Arc<SchemaContract>,
    pub provider: Arc<dyn TableProvider>,
}

impl std::fmt::Debug for ProviderInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderInput")
            .field("relation_id", &self.relation_id)
            .field("table_reference", &self.table_reference)
            .field("contract", &self.contract)
            .field("provider", &self.provider)
            .finish()
    }
}

impl ProviderInput {
    /// Bind an exact provider contract to a fully qualified candidate table.
    #[must_use]
    pub fn new(
        relation_id: ProgrammaticRelationId,
        table_reference: TableReference,
        contract: Arc<SchemaContract>,
        provider: Arc<dyn TableProvider>,
    ) -> Self {
        Self {
            relation_id,
            table_reference,
            contract,
            provider,
        }
    }
}

/// The stable output binding of one transformation.
#[derive(Clone, Debug)]
pub struct TransformationOutput {
    relation_id: ProgrammaticRelationId,
    table_reference: TableReference,
    fields: Arc<[TransformationFieldIdentity]>,
    schema_assertion: Option<SchemaRef>,
}

impl TransformationOutput {
    /// Define an output identity and catalog address without declaring a schema.
    #[must_use]
    pub fn new(
        relation_id: ProgrammaticRelationId,
        table_reference: TableReference,
        fields: impl Into<Arc<[TransformationFieldIdentity]>>,
    ) -> Self {
        Self {
            relation_id,
            table_reference,
            fields: fields.into(),
            schema_assertion: None,
        }
    }

    /// Add an exact assertion checked against the schema derived from the plan.
    ///
    /// The assertion is never used to construct, coerce, or register the output.
    #[must_use]
    pub fn with_schema_assertion(mut self, schema: SchemaRef) -> Self {
        self.schema_assertion = Some(schema);
        self
    }

    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn table_reference(&self) -> &TableReference {
        &self.table_reference
    }

    #[must_use]
    pub fn fields(&self) -> &[TransformationFieldIdentity] {
        &self.fields
    }

    #[must_use]
    pub const fn schema_assertion(&self) -> Option<&SchemaRef> {
        self.schema_assertion.as_ref()
    }
}

/// Error returned by a transformation while constructing its native logical plan.
#[derive(Debug, Error)]
pub enum TransformationPlanError {
    #[error("transformation requested undeclared input {relation_id:?}")]
    UndeclaredInput { relation_id: ProgrammaticRelationId },
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
}

/// Dependency-closed logical inputs resolved from the live candidate catalog.
pub struct TransformationInputs {
    inputs: BTreeMap<ProgrammaticRelationId, TransformationInput>,
}

struct TransformationInput {
    table_reference: TableReference,
    plan: LogicalPlan,
}

impl TransformationInputs {
    /// Clone the live-catalog scan plan for a declared dependency.
    pub fn plan(
        &self,
        relation_id: &ProgrammaticRelationId,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        self.inputs
            .get(relation_id)
            .map(|input| input.plan.clone())
            .ok_or_else(|| TransformationPlanError::UndeclaredInput {
                relation_id: relation_id.clone(),
            })
    }

    /// Return the fully qualified catalog address of a declared dependency.
    pub fn table_reference(
        &self,
        relation_id: &ProgrammaticRelationId,
    ) -> Result<&TableReference, TransformationPlanError> {
        self.inputs
            .get(relation_id)
            .map(|input| &input.table_reference)
            .ok_or_else(|| TransformationPlanError::UndeclaredInput {
                relation_id: relation_id.clone(),
            })
    }
}

/// Closed interface for a programmatically built relational transformation.
///
/// Implementations receive only dependency plans resolved from the candidate catalog.
/// They return DataFusion's native [`LogicalPlan`], never SQL or a serialized plan.
pub trait ProgrammaticTransformation: Send + Sync {
    /// Return the complete application-owned semantic and execution contract.
    ///
    /// There is intentionally no default contract: every implementation must
    /// provide exact authority rather than inheriting placeholder metadata.
    fn contract(&self) -> &ProgrammaticTransformationContract;

    fn id(&self) -> &ProgrammaticTransformationId {
        self.contract().semantic_id()
    }

    fn output(&self) -> &TransformationOutput;
    fn dependencies(&self) -> &[ProgrammaticRelationId];
    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError>;
}

/// Whether a catalog relation is a direct provider or a derived view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationOrigin {
    Provider,
    Transformation,
    SystemObservation,
}

/// Observation of one relation reread from the live candidate catalog.
#[derive(Clone, Debug)]
pub struct RelationObservation {
    pub relation_id: ProgrammaticRelationId,
    pub table_reference: TableReference,
    pub origin: RelationOrigin,
    pub table_type: TableType,
}

/// Observation of one exact field in a live catalog relation.
#[derive(Clone, Debug)]
pub struct FieldObservation {
    pub relation_id: ProgrammaticRelationId,
    pub field_id: ProgrammaticFieldId,
    pub ordinal: usize,
    pub qualifier: Option<TableReference>,
    pub field: FieldRef,
}

/// Arrow and DataFusion schemas observed through the live catalog scan.
#[derive(Clone, Debug)]
pub struct SchemaObservation {
    pub relation_id: ProgrammaticRelationId,
    pub arrow_schema: SchemaRef,
    pub datafusion_schema: DFSchemaRef,
}

/// One direct dependency proved against the transformation's native plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyObservation {
    pub transformation_id: ProgrammaticTransformationId,
    pub output_relation_id: ProgrammaticRelationId,
    pub input_relation_id: ProgrammaticRelationId,
    pub ordinal: usize,
}

/// Provenance observed from the registered provider or view.
#[derive(Clone, Debug)]
pub enum ProvenanceObservation {
    Provider {
        relation_id: ProgrammaticRelationId,
        source_schema_identity: Arc<str>,
    },
    SystemObservation {
        relation_id: ProgrammaticRelationId,
        source_schema_identity: Arc<str>,
    },
    Transformation {
        relation_id: ProgrammaticRelationId,
        contract: Arc<ProgrammaticTransformationContract>,
        logical_plan: Arc<LogicalPlan>,
    },
    ObservationView {
        relation_id: ProgrammaticRelationId,
        transformation_id: ProgrammaticTransformationId,
        logical_plan: Arc<LogicalPlan>,
    },
}

impl ProvenanceObservation {
    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        match self {
            Self::Provider { relation_id, .. }
            | Self::SystemObservation { relation_id, .. }
            | Self::Transformation { relation_id, .. }
            | Self::ObservationView { relation_id, .. } => relation_id,
        }
    }

    /// Return a native plan only for programmatic transformation provenance.
    #[must_use]
    pub fn logical_plan(&self) -> Option<&LogicalPlan> {
        match self {
            Self::Provider { .. } | Self::SystemObservation { .. } => None,
            Self::Transformation { logical_plan, .. }
            | Self::ObservationView { logical_plan, .. } => Some(logical_plan),
        }
    }

    /// Return the complete application contract only for an application transformation.
    #[must_use]
    pub fn transformation_contract(&self) -> Option<&ProgrammaticTransformationContract> {
        match self {
            Self::Transformation { contract, .. } => Some(contract),
            Self::Provider { .. }
            | Self::SystemObservation { .. }
            | Self::ObservationView { .. } => None,
        }
    }
}

/// All assembly observations, kept as typed rows rather than serialized summaries.
#[derive(Clone, Debug, Default)]
pub struct CandidateAssemblyObservations {
    pub relations: Vec<RelationObservation>,
    pub fields: Vec<FieldObservation>,
    pub schemas: Vec<SchemaObservation>,
    pub dependencies: Vec<DependencyObservation>,
    pub provenance: Vec<ProvenanceObservation>,
}

impl CandidateAssemblyObservations {
    #[must_use]
    pub fn schema(&self, relation_id: &ProgrammaticRelationId) -> Option<&SchemaObservation> {
        self.schemas
            .iter()
            .find(|observation| &observation.relation_id == relation_id)
    }

    #[must_use]
    pub fn provenance(
        &self,
        relation_id: &ProgrammaticRelationId,
    ) -> Option<&ProvenanceObservation> {
        self.provenance
            .iter()
            .find(|observation| observation.relation_id() == relation_id)
    }
}

/// Epoch-facing lookup for one sealed relation.
#[derive(Clone, Debug)]
pub struct SealedRelationBinding {
    pub table_reference: TableReference,
    pub contract: Arc<SchemaContract>,
    pub actual_datafusion_schema: DFSchemaRef,
    pub(super) logical_plan: Option<Arc<LogicalPlan>>,
}

/// Sealed candidate retaining the exact session/catalog authority used for planning.
pub struct SealedProgrammaticSchemaAssembly {
    session: SessionContext,
    relations: BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    observation_fixed_point: ObservationFixedPointEvidence,
    #[cfg(test)]
    observations: CandidateAssemblyObservations,
}

impl std::fmt::Debug for SealedProgrammaticSchemaAssembly {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SealedProgrammaticSchemaAssembly")
            .field("relation_count", &self.relations.len())
            .finish_non_exhaustive()
    }
}

impl SealedProgrammaticSchemaAssembly {
    /// Borrow the same live candidate session used to analyze and register every plan.
    #[must_use]
    pub const fn session(&self) -> &SessionContext {
        &self.session
    }

    #[cfg(test)]
    #[must_use]
    pub const fn observations(&self) -> &CandidateAssemblyObservations {
        &self.observations
    }

    /// Iteration and conservatively measured resources for the self-observed catalog fixed point.
    #[must_use]
    pub const fn observation_fixed_point(&self) -> ObservationFixedPointEvidence {
        self.observation_fixed_point
    }

    /// Resolve a stable relation identity directly to its table and executable contract.
    #[must_use]
    pub fn relation(&self, relation_id: &ProgrammaticRelationId) -> Option<&SealedRelationBinding> {
        self.relations.get(relation_id)
    }

    /// Transfer the exact live session authority and all sealed products to an epoch builder.
    #[must_use]
    pub fn into_parts(self) -> ProgrammaticSchemaParts {
        ProgrammaticSchemaParts {
            session: self.session,
            relations: self.relations,
        }
    }
}

/// Ownership-transfer product for programmatic epoch integration.
pub struct ProgrammaticSchemaParts {
    session: SessionContext,
    relations: BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
}

impl ProgrammaticSchemaParts {
    #[must_use]
    pub const fn session(&self) -> &SessionContext {
        &self.session
    }

    #[must_use]
    pub fn relation(&self, relation_id: &ProgrammaticRelationId) -> Option<&SealedRelationBinding> {
        self.relations.get(relation_id)
    }

    /// Move the exact session and relation lookup without reconstructing a catalog.
    #[must_use]
    pub fn into_components(
        self,
    ) -> (
        SessionContext,
        BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    ) {
        (self.session, self.relations)
    }
}

#[derive(Clone)]
enum RegisteredOrigin {
    Provider {
        logical_plan: Option<Arc<LogicalPlan>>,
    },
    #[cfg(test)]
    SystemObservation,
    Transformation {
        contract: Arc<ProgrammaticTransformationContract>,
        dependencies: Arc<[ProgrammaticRelationId]>,
        plan: Arc<LogicalPlan>,
    },
    ObservationView {
        transformation_id: ProgrammaticTransformationId,
        dependencies: Arc<[ProgrammaticRelationId]>,
        plan: Arc<LogicalPlan>,
    },
}

struct RegisteredRelation {
    table_reference: TableReference,
    contract: Arc<SchemaContract>,
    origin: RegisteredOrigin,
}

/// A native DataFusion view whose physical scan reattaches the exact Arrow
/// metadata advertised by its application-owned logical plan.
///
/// DataFusion 55 derives a `ViewTable` scan's physical schema from the view's
/// input providers. That is correct for values, but schema-level identity
/// metadata attached by [`output_identity_boundary`] otherwise disappears
/// before record-batch execution. This transparent adapter delegates view
/// planning and uses one narrow physical boundary to make the advertised
/// identity an executable batch boundary as well.
#[derive(Debug)]
pub(super) struct IdentityPreservingViewTable {
    inner: ViewTable,
    schema: SchemaRef,
}

/// Opaque, value-preserving physical boundary for exact Arrow schema identity.
///
/// DataFusion 55's native projection-pushdown rule can legally merge identity
/// projections but currently rejects nested views when the inner and outer
/// relation metadata differ. This node is deliberately narrower than a custom
/// query operator: it owns no expressions or values, preserves partitioning and
/// ordering, and only rebinds each batch to the already validated target schema.
/// Keeping the node opaque prevents optimizer rewrites from conflating distinct
/// relation identities while leaving the complete child plan optimizer-visible.
#[derive(Clone, Debug)]
struct SchemaIdentityExec {
    input: Arc<dyn ExecutionPlan>,
    schema: SchemaRef,
    properties: Arc<PlanProperties>,
}

impl SchemaIdentityExec {
    fn try_new(input: Arc<dyn ExecutionPlan>, schema: SchemaRef) -> Result<Self, DataFusionError> {
        validate_schema_identity_shape(input.schema().as_ref(), schema.as_ref(), "planning")?;
        let equivalence = input.output_ordering().cloned().map_or_else(
            || EquivalenceProperties::new(Arc::clone(&schema)),
            |ordering| EquivalenceProperties::new_with_orderings(Arc::clone(&schema), [ordering]),
        );
        let properties =
            Arc::new(PlanProperties::clone(input.properties()).with_eq_properties(equivalence));
        Ok(Self {
            input,
            schema,
            properties,
        })
    }
}

impl DisplayAs for SchemaIdentityExec {
    fn fmt_as(
        &self,
        display_type: DisplayFormatType,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match display_type {
            DisplayFormatType::Default | DisplayFormatType::Verbose => {
                write!(formatter, "SchemaIdentityExec")
            }
            DisplayFormatType::TreeRender => writeln!(formatter, "schema_identity"),
        }
    }
}

impl ExecutionPlan for SchemaIdentityExec {
    fn name(&self) -> &str {
        "SchemaIdentityExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.properties
    }

    fn maintains_input_order(&self) -> Vec<bool> {
        vec![true]
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.input]
    }

    fn apply_expressions(
        &self,
        _visitor: &mut dyn FnMut(
            &Arc<dyn datafusion::physical_plan::PhysicalExpr>,
        ) -> datafusion::common::Result<TreeNodeRecursion>,
    ) -> datafusion::common::Result<TreeNodeRecursion> {
        Ok(TreeNodeRecursion::Continue)
    }

    fn with_new_children(
        self: Arc<Self>,
        mut children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        if children.len() != 1 {
            return Err(DataFusionError::Internal(format!(
                "SchemaIdentityExec requires one child, received {}",
                children.len()
            )));
        }
        Ok(Arc::new(Self::try_new(
            children.swap_remove(0),
            Arc::clone(&self.schema),
        )?))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        let schema = Arc::clone(&self.schema);
        let stream = self.input.execute(partition, context)?.map({
            let schema = Arc::clone(&schema);
            move |batch| {
                let batch = batch?;
                validate_schema_identity_shape(
                    batch.schema_ref().as_ref(),
                    schema.as_ref(),
                    "execution",
                )?;
                RecordBatch::try_new_with_options(
                    Arc::clone(&schema),
                    batch.columns().to_vec(),
                    &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
                )
                .map_err(DataFusionError::from)
            }
        });
        Ok(Box::pin(RecordBatchStreamAdapter::new(schema, stream)))
    }
}

fn validate_schema_identity_shape(
    actual: &Schema,
    expected: &Schema,
    phase: &str,
) -> Result<(), DataFusionError> {
    if actual.fields().len() != expected.fields().len() {
        return Err(DataFusionError::Execution(format!(
            "programmatic view {phase} field count {} differs from its identity schema count {}",
            actual.fields().len(),
            expected.fields().len()
        )));
    }
    for (index, (actual, expected)) in actual
        .fields()
        .iter()
        .zip(expected.fields().iter())
        .enumerate()
    {
        if actual.name() != expected.name()
            || actual.data_type() != expected.data_type()
            || actual.is_nullable() != expected.is_nullable()
        {
            return Err(DataFusionError::Execution(format!(
                "programmatic view {phase} field {index} shape differs: actual={actual:?}, expected={expected:?}"
            )));
        }
    }
    Ok(())
}

impl IdentityPreservingViewTable {
    fn new(plan: LogicalPlan) -> Self {
        Self::with_definition(plan, None)
    }

    pub(super) fn with_definition(plan: LogicalPlan, definition: Option<String>) -> Self {
        let schema = Arc::clone(plan.schema().inner());
        Self {
            inner: ViewTable::new(plan, definition),
            schema,
        }
    }

    fn projected_schema(
        &self,
        projection: Option<&Vec<usize>>,
    ) -> Result<SchemaRef, DataFusionError> {
        projection.map_or_else(
            || Ok(Arc::clone(&self.schema)),
            |indices| Ok(Arc::new(self.schema.project(indices)?)),
        )
    }

    fn logical_plan(&self) -> &LogicalPlan {
        self.inner.logical_plan()
    }
}

#[async_trait]
impl TableProvider for IdentityPreservingViewTable {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::View
    }

    fn get_table_definition(&self) -> Option<&str> {
        self.inner.get_table_definition()
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>, DataFusionError> {
        self.inner.supports_filters_pushdown(filters)
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
        // `ViewTable::scan` recursively creates and fully optimizes a physical plan. The outer
        // TableScan planner then runs the same physical optimizer over the returned subtree. In
        // DataFusion 55 that can make `JoinSelection` see hash joins whose post-optimization
        // dynamic filters are already attached, which is both an invalid optimizer order and a
        // hard planning error. Plan the nested view with the exact same session authorities but
        // no inner physical rules; the outer candidate-session pass remains the single complete
        // physical optimization and applies every correctness/resource rule to the whole tree.
        let candidate_state = state
            .as_any()
            .downcast_ref::<SessionState>()
            .ok_or_else(|| {
                DataFusionError::Plan(
                    "programmatic views require the candidate SessionState authority".to_owned(),
                )
            })?;
        let nested_state = SessionStateBuilder::new_from_existing(candidate_state.clone())
            .with_physical_optimizer_rules(Vec::new())
            .build();
        let physical = self
            .inner
            .scan(&nested_state, projection, filters, limit)
            .await?;
        let target = self.projected_schema(projection)?;
        Ok(Arc::new(SchemaIdentityExec::try_new(physical, target)?))
    }
}

pub(super) fn registered_view_logical_plan(provider: &dyn TableProvider) -> Option<LogicalPlan> {
    provider
        .downcast_ref::<IdentityPreservingViewTable>()
        .map(|view| view.logical_plan().clone())
        .or_else(|| provider.get_logical_plan().map(|plan| plan.into_owned()))
}

const OBSERVATION_SOURCE_IDENTITY: &str = "programmatic-schema-assembly-v1";
const OBSERVATION_SCHEMA: &str = "system";
/// Stable relation ID of the queryable relation census.
pub const RELATION_OBSERVATION_RELATION_ID: &str = "system.programmatic_relation_observation";
/// Stable relation ID of the queryable field census.
pub const FIELD_OBSERVATION_RELATION_ID: &str = "system.programmatic_field_observation";
/// Stable relation ID of the queryable schema census.
pub const SCHEMA_OBSERVATION_RELATION_ID: &str = "system.programmatic_schema_observation";
/// Stable relation ID of the queryable direct-dependency relation.
pub const DEPENDENCY_OBSERVATION_RELATION_ID: &str = "system.programmatic_dependency_observation";
/// Stable relation ID of the queryable provenance relation.
pub const PROVENANCE_OBSERVATION_RELATION_ID: &str = "system.programmatic_provenance_observation";

#[derive(Clone)]
pub(crate) struct PreparedObservationRelation {
    pub(crate) relation_id: ProgrammaticRelationId,
    #[cfg(test)]
    pub(crate) table_reference: TableReference,
    pub(crate) contract: Arc<SchemaContract>,
    pub(crate) batch: RecordBatch,
}

#[derive(Clone)]
pub(crate) struct PreparedObservationRelationSpec {
    pub(crate) relation_id: ProgrammaticRelationId,
    pub(crate) table_reference: TableReference,
    pub(crate) contract: Arc<SchemaContract>,
}

/// Deterministic resource envelope for self-observed catalog fixed-point materialization.
///
/// Every bound is nonzero. Rows and Arrow array-size bytes are limited both per relation and
/// across the complete observation family so a single large relation and many individually-small
/// relations both fail closed before historicization or sealing. Array-size bytes use
/// [`RecordBatch::get_array_memory_size`], a conservative estimate that can count shared buffers
/// more than once; overestimation is intentional for this fail-closed envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationFixedPointPolicy {
    max_iterations: u32,
    max_rows_per_relation: u64,
    max_total_rows: u64,
    max_bytes_per_relation: u64,
    max_total_bytes: u64,
}

impl ObservationFixedPointPolicy {
    /// Construct a complete nonzero fixed-point and materialization envelope.
    pub fn try_new(
        max_iterations: u32,
        max_rows_per_relation: u64,
        max_total_rows: u64,
        max_bytes_per_relation: u64,
        max_total_bytes: u64,
    ) -> Result<Self, ObservationFixedPointPolicyError> {
        for (field, value) in [
            ("max_iterations", u64::from(max_iterations)),
            ("max_rows_per_relation", max_rows_per_relation),
            ("max_total_rows", max_total_rows),
            ("max_bytes_per_relation", max_bytes_per_relation),
            ("max_total_bytes", max_total_bytes),
        ] {
            if value == 0 {
                return Err(ObservationFixedPointPolicyError::ZeroBound { field });
            }
        }
        Ok(Self {
            max_iterations,
            max_rows_per_relation,
            max_total_rows,
            max_bytes_per_relation,
            max_total_bytes,
        })
    }

    /// Stable workstation policy used by the target epoch builder.
    #[must_use]
    pub fn production() -> Self {
        Self::try_new(8, 1_000_000, 5_000_000, 256 << 20, 512 << 20)
            .expect("the static production observation policy is nonzero")
    }

    #[must_use]
    pub const fn max_iterations(self) -> u32 {
        self.max_iterations
    }

    #[must_use]
    pub const fn max_rows_per_relation(self) -> u64 {
        self.max_rows_per_relation
    }

    #[must_use]
    pub const fn max_total_rows(self) -> u64 {
        self.max_total_rows
    }

    #[must_use]
    pub const fn max_bytes_per_relation(self) -> u64 {
        self.max_bytes_per_relation
    }

    #[must_use]
    pub const fn max_total_bytes(self) -> u64 {
        self.max_total_bytes
    }
}

/// Invalid observation fixed-point policy rejected before a candidate session exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ObservationFixedPointPolicyError {
    #[error("observation fixed-point policy bound {field} must be nonzero")]
    ZeroBound { field: &'static str },
}

/// Iteration and conservatively measured resource evidence for the observation fixed point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationFixedPointEvidence {
    iterations: u32,
    relation_count: usize,
    total_rows: u64,
    total_bytes: u64,
}

impl ObservationFixedPointEvidence {
    #[must_use]
    pub const fn iterations(self) -> u32 {
        self.iterations
    }

    #[must_use]
    pub const fn relation_count(self) -> usize {
        self.relation_count
    }

    #[must_use]
    pub const fn total_rows(self) -> u64 {
        self.total_rows
    }

    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }
}

/// Mutable builder for one dependency-closed candidate catalog.
pub struct ProgrammaticSchemaAssembly {
    session: SessionContext,
    observation_policy: ObservationFixedPointPolicy,
    registered: BTreeMap<ProgrammaticRelationId, RegisteredRelation>,
    pending: BTreeMap<ProgrammaticRelationId, Arc<dyn ProgrammaticTransformation>>,
    transformations: BTreeMap<ProgrammaticTransformationId, ProgrammaticRelationId>,
    table_references: BTreeMap<TableReference, ProgrammaticRelationId>,
}

impl ProgrammaticSchemaAssembly {
    /// Start from the exact candidate `SessionState` later transferred to the epoch.
    #[must_use]
    pub(crate) fn new(candidate_state: SessionState) -> Self {
        Self::with_observation_policy(candidate_state, ObservationFixedPointPolicy::production())
    }

    /// Start a candidate with an explicit validated observation fixed-point envelope.
    #[must_use]
    pub(crate) fn with_observation_policy(
        candidate_state: SessionState,
        observation_policy: ObservationFixedPointPolicy,
    ) -> Self {
        // DataFusion 55's logical `optimize_projections` and physical
        // `ProjectionPushdown` rules treat a metadata-only identity projection
        // as removable. The former drops schema metadata; the latter either
        // drops field metadata or fails its own schema check. Retain every other
        // native optimizer while keeping the application-owned identity boundary
        // visible through logical, physical, batch, and view phases.
        let logical_rules = candidate_state
            .optimizers()
            .iter()
            .filter(|rule| rule.name() != "optimize_projections")
            .cloned()
            .collect();
        let physical_rules = candidate_state
            .physical_optimizers()
            .iter()
            .filter(|rule| rule.name() != "ProjectionPushdown")
            .cloned()
            .collect();
        let candidate_state = SessionStateBuilder::new_from_existing(candidate_state)
            .with_optimizer_rules(logical_rules)
            .with_physical_optimizer_rules(physical_rules)
            .build();
        Self {
            session: SessionContext::new_with_state(candidate_state),
            observation_policy,
            registered: BTreeMap::new(),
            pending: BTreeMap::new(),
            transformations: BTreeMap::new(),
            table_references: BTreeMap::new(),
        }
    }

    /// Clone the candidate state for constructing a provider that must be
    /// bound to this exact session authority. `SessionState::clone` retains
    /// the same runtime and catalog authorities; callers cannot replace the
    /// state held by this assembly.
    #[must_use]
    pub(crate) fn candidate_state(&self) -> SessionState {
        self.session.state()
    }

    /// Clone the exact candidate context for plan construction that must stay
    /// bound to the assembly's live catalog and runtime authorities.
    #[must_use]
    pub(crate) fn candidate_context(&self) -> SessionContext {
        self.session.clone()
    }

    /// Analyze and optimize one native plan in the exact candidate state.
    pub(crate) fn analyze_plan(
        &self,
        plan: &LogicalPlan,
    ) -> Result<LogicalPlan, ProgrammaticSchemaError> {
        Ok(self.session.state().optimize(plan)?)
    }

    /// Register one exact provider contract into the candidate catalog.
    pub(crate) fn register_provider(
        &mut self,
        input: ProviderInput,
    ) -> Result<(), ProgrammaticSchemaError> {
        validate_relation_id(&input.relation_id)?;
        validate_full_reference(&input.table_reference)?;
        self.ensure_binding_available(&input.relation_id, &input.table_reference)?;

        if input.contract.qualifier() != &input.table_reference {
            return Err(ProgrammaticSchemaError::ProviderContractQualifier {
                relation_id: input.relation_id,
                expected: input.table_reference,
                actual: input.contract.qualifier().clone(),
            });
        }
        let observed_schema = input.provider.schema();
        if observed_schema.as_ref() != input.contract.logical_schema().as_ref() {
            return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
                relation_id: input.relation_id,
                expected: Arc::clone(input.contract.logical_schema()),
                actual: observed_schema,
            });
        }
        let encoded_relation_id = input.contract.relation_id(SchemaRole::Logical)?;
        if encoded_relation_id != input.relation_id.as_str() {
            return Err(ProgrammaticSchemaError::ProviderRelationIdentityMismatch {
                relation_id: input.relation_id,
                encoded_relation_id: encoded_relation_id.to_owned(),
            });
        }
        for field_index in 0..observed_schema.fields().len() {
            let _ = input
                .contract
                .field_id_at(SchemaRole::Logical, field_index)?;
        }

        let logical_plan = input
            .provider
            .get_logical_plan()
            .map(|plan| Arc::new(plan.into_owned()));
        self.session
            .register_table(input.table_reference.clone(), input.provider)?;
        self.table_references
            .insert(input.table_reference.clone(), input.relation_id.clone());
        self.registered.insert(
            input.relation_id,
            RegisteredRelation {
                table_reference: input.table_reference,
                contract: input.contract,
                origin: RegisteredOrigin::Provider { logical_plan },
            },
        );
        Ok(())
    }

    /// Add one programmatic transformation. It is built during [`Self::seal`].
    pub(crate) fn add_transformation(
        &mut self,
        transformation: Arc<dyn ProgrammaticTransformation>,
    ) -> Result<(), ProgrammaticSchemaError> {
        validate_transformation_id(transformation.id())?;
        validate_relation_id(transformation.output().relation_id())?;
        validate_full_reference(transformation.output().table_reference())?;
        self.ensure_binding_available(
            transformation.output().relation_id(),
            transformation.output().table_reference(),
        )?;
        if self.transformations.contains_key(transformation.id()) {
            return Err(ProgrammaticSchemaError::DuplicateTransformation {
                transformation_id: transformation.id().clone(),
            });
        }
        validate_output_field_identities(transformation.id(), transformation.output().fields())?;
        validate_transformation_contract(transformation.contract(), transformation.output())?;
        let mut dependencies = BTreeSet::new();
        for dependency in transformation.dependencies() {
            validate_relation_id(dependency)?;
            if !dependencies.insert(dependency.clone()) {
                return Err(ProgrammaticSchemaError::DuplicateDependency {
                    transformation_id: transformation.id().clone(),
                    relation_id: dependency.clone(),
                });
            }
        }

        let output_relation_id = transformation.output().relation_id().clone();
        let table_reference = transformation.output().table_reference().clone();
        self.transformations
            .insert(transformation.id().clone(), output_relation_id.clone());
        self.table_references
            .insert(table_reference, output_relation_id.clone());
        self.pending.insert(output_relation_id, transformation);
        Ok(())
    }

    /// Build every transformation, register plan-backed views, reread the catalog,
    /// and freeze the candidate assembly.
    #[cfg(test)]
    pub(crate) async fn seal(
        mut self,
        epoch_id: EpochId,
    ) -> Result<SealedProgrammaticSchemaAssembly, ProgrammaticSchemaError> {
        self.install_transformations().await?;
        let prepared = self.prepare_observation_relations(epoch_id).await?;
        let installed_observation_batches = prepared
            .iter()
            .map(|relation| (relation.relation_id.clone(), relation.batch.clone()))
            .collect::<BTreeMap<_, _>>();
        for relation in prepared {
            let provider = Arc::new(MemTable::try_new(
                Arc::clone(relation.contract.logical_schema()),
                vec![vec![relation.batch.clone()]],
            )?);
            self.register_system_observation_provider(relation, provider)?;
        }
        self.finish_seal(epoch_id, installed_observation_batches)
            .await
    }

    pub(crate) async fn install_transformations(&mut self) -> Result<(), ProgrammaticSchemaError> {
        let order = self.transformation_order()?;
        for output_relation_id in order {
            let transformation = Arc::clone(
                self.pending
                    .get(&output_relation_id)
                    .expect("topological order contains only pending transformations"),
            );
            self.install_transformation(transformation).await?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn register_system_observation_provider(
        &mut self,
        relation: PreparedObservationRelation,
        provider: Arc<dyn TableProvider>,
    ) -> Result<(), ProgrammaticSchemaError> {
        if provider.schema().as_ref() != relation.contract.logical_schema().as_ref() {
            return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
                relation_id: relation.relation_id,
                expected: Arc::clone(relation.contract.logical_schema()),
                actual: provider.schema(),
            });
        }
        self.session
            .register_table(relation.table_reference.clone(), provider)?;
        self.table_references.insert(
            relation.table_reference.clone(),
            relation.relation_id.clone(),
        );
        self.registered.insert(
            relation.relation_id,
            RegisteredRelation {
                table_reference: relation.table_reference,
                contract: relation.contract,
                origin: RegisteredOrigin::SystemObservation,
            },
        );
        Ok(())
    }

    /// Register one native current-epoch observation view. Its dependency and
    /// logical plan become ordinary catalog dependency/provenance rows.
    pub(crate) fn register_observation_view(
        &mut self,
        spec: PreparedObservationRelationSpec,
        transformation_id: ProgrammaticTransformationId,
        dependencies: Arc<[ProgrammaticRelationId]>,
        plan: LogicalPlan,
    ) -> Result<(), ProgrammaticSchemaError> {
        validate_relation_id(&spec.relation_id)?;
        validate_transformation_id(&transformation_id)?;
        validate_full_reference(&spec.table_reference)?;
        self.ensure_binding_available(&spec.relation_id, &spec.table_reference)?;
        self.validate_plan_dependencies(&transformation_id, &dependencies, &plan)?;
        if plan.schema().inner().as_ref() != spec.contract.logical_schema().as_ref() {
            return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
                relation_id: spec.relation_id,
                expected: Arc::clone(spec.contract.logical_schema()),
                actual: Arc::clone(plan.schema().inner()),
            });
        }
        let provider = Arc::new(IdentityPreservingViewTable::new(plan.clone()));
        self.session.register_table(
            spec.table_reference.clone(),
            provider as Arc<dyn TableProvider>,
        )?;
        self.table_references
            .insert(spec.table_reference.clone(), spec.relation_id.clone());
        self.registered.insert(
            spec.relation_id,
            RegisteredRelation {
                table_reference: spec.table_reference,
                contract: spec.contract,
                origin: RegisteredOrigin::ObservationView {
                    transformation_id,
                    dependencies,
                    plan: Arc::new(plan),
                },
            },
        );
        Ok(())
    }

    /// Replace a registered provider inside the still-owned candidate. This
    /// operation is intentionally unavailable after sealing.
    pub(crate) fn replace_registered_provider(
        &mut self,
        relation_id: &ProgrammaticRelationId,
        provider: Arc<dyn TableProvider>,
    ) -> Result<(), ProgrammaticSchemaError> {
        let registered = self.registered.get(relation_id).ok_or_else(|| {
            ProgrammaticSchemaError::CatalogReplacementMissing {
                relation_id: relation_id.clone(),
            }
        })?;
        if provider.schema().as_ref() != registered.contract.logical_schema().as_ref() {
            return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
                relation_id: relation_id.clone(),
                expected: Arc::clone(registered.contract.logical_schema()),
                actual: provider.schema(),
            });
        }
        let table_reference = registered.table_reference.clone();
        if self
            .session
            .deregister_table(table_reference.clone())?
            .is_none()
        {
            return Err(ProgrammaticSchemaError::CatalogReplacementMissing {
                relation_id: relation_id.clone(),
            });
        }
        self.session.register_table(table_reference, provider)?;
        Ok(())
    }

    /// Rebuild a registered observation view after its exact Delta dependency
    /// is rebound. A `ViewTable` retains provider Arcs in its plan, so merely
    /// replacing the storage catalog entry would leave a stale view.
    pub(crate) fn replace_observation_view(
        &mut self,
        relation_id: &ProgrammaticRelationId,
        plan: LogicalPlan,
    ) -> Result<(), ProgrammaticSchemaError> {
        let registered = self.registered.get(relation_id).ok_or_else(|| {
            ProgrammaticSchemaError::CatalogReplacementMissing {
                relation_id: relation_id.clone(),
            }
        })?;
        let RegisteredOrigin::ObservationView {
            transformation_id,
            dependencies,
            ..
        } = &registered.origin
        else {
            return Err(ProgrammaticSchemaError::CatalogReplacementNotView {
                relation_id: relation_id.clone(),
            });
        };
        self.validate_plan_dependencies(transformation_id, dependencies, &plan)?;
        if plan.schema().inner().as_ref() != registered.contract.logical_schema().as_ref() {
            return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
                relation_id: relation_id.clone(),
                expected: Arc::clone(registered.contract.logical_schema()),
                actual: Arc::clone(plan.schema().inner()),
            });
        }
        let table_reference = registered.table_reference.clone();
        let origin = RegisteredOrigin::ObservationView {
            transformation_id: transformation_id.clone(),
            dependencies: Arc::clone(dependencies),
            plan: Arc::new(plan.clone()),
        };
        if self
            .session
            .deregister_table(table_reference.clone())?
            .is_none()
        {
            return Err(ProgrammaticSchemaError::CatalogReplacementMissing {
                relation_id: relation_id.clone(),
            });
        }
        self.session.register_table(
            table_reference,
            Arc::new(IdentityPreservingViewTable::new(plan)) as Arc<dyn TableProvider>,
        )?;
        self.registered
            .get_mut(relation_id)
            .expect("registered view remains owned during replacement")
            .origin = origin;
        Ok(())
    }

    fn validate_plan_dependencies(
        &self,
        transformation_id: &ProgrammaticTransformationId,
        dependencies: &[ProgrammaticRelationId],
        plan: &LogicalPlan,
    ) -> Result<(), ProgrammaticSchemaError> {
        let expected = dependencies
            .iter()
            .map(|dependency| {
                self.registered
                    .get(dependency)
                    .map(|registered| registered.table_reference.clone())
                    .ok_or_else(|| ProgrammaticSchemaError::UnresolvedDependency {
                        transformation_id: transformation_id.clone(),
                        relation_id: dependency.clone(),
                    })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let actual = scan_references(plan)?;
        if actual != expected {
            return Err(ProgrammaticSchemaError::PlanDependencyMismatch {
                transformation_id: transformation_id.clone(),
                expected,
                actual,
            });
        }
        Ok(())
    }

    /// Observe the already-installed catalog and materialize one transient
    /// Arrow batch for each programmatic observation family.
    pub(crate) async fn materialize_live_observation_relations(
        &self,
        epoch_id: EpochId,
    ) -> Result<Vec<PreparedObservationRelation>, ProgrammaticSchemaError> {
        let specs = self.observation_relation_specs()?;
        let (_, observations) = self.observe_live_catalog().await?;
        let batches = build_observation_batches(epoch_id, &observations, &specs)?;
        enforce_observation_materialization(&self.observation_policy, 1, &batches)?;
        specs
            .into_iter()
            .map(|spec| {
                let batch = batches.get(&spec.relation_id).cloned().ok_or_else(|| {
                    ProgrammaticSchemaError::ObservationBatchMissing {
                        relation_id: spec.relation_id.clone(),
                    }
                })?;
                Ok(PreparedObservationRelation {
                    relation_id: spec.relation_id,
                    #[cfg(test)]
                    table_reference: spec.table_reference,
                    contract: spec.contract,
                    batch,
                })
            })
            .collect()
    }

    pub(crate) async fn finish_seal(
        self,
        epoch_id: EpochId,
        installed_observation_batches: BTreeMap<ProgrammaticRelationId, RecordBatch>,
    ) -> Result<SealedProgrammaticSchemaAssembly, ProgrammaticSchemaError> {
        enforce_observation_materialization(
            &self.observation_policy,
            1,
            &installed_observation_batches,
        )?;
        let specs = self.observation_relation_specs()?;
        let mut previous = installed_observation_batches;
        for iteration in 2..=self.observation_policy.max_iterations() {
            let (relations, observations) = self.observe_live_catalog().await?;
            let current = build_observation_batches(epoch_id, &observations, &specs)?;
            let fixed_point =
                enforce_observation_materialization(&self.observation_policy, iteration, &current)?;
            if previous == current {
                return Ok(SealedProgrammaticSchemaAssembly {
                    session: self.session,
                    relations,
                    observation_fixed_point: fixed_point,
                    #[cfg(test)]
                    observations,
                });
            }
            previous = current;
        }
        Err(
            ProgrammaticSchemaError::ObservationFixedPointIterationsExceeded {
                limit: self.observation_policy.max_iterations(),
            },
        )
    }

    fn ensure_binding_available(
        &self,
        relation_id: &ProgrammaticRelationId,
        table_reference: &TableReference,
    ) -> Result<(), ProgrammaticSchemaError> {
        if self.registered.contains_key(relation_id) || self.pending.contains_key(relation_id) {
            return Err(ProgrammaticSchemaError::DuplicateRelation {
                relation_id: relation_id.clone(),
            });
        }
        if let Some(existing_relation_id) = self.table_references.get(table_reference) {
            return Err(ProgrammaticSchemaError::DuplicateTableReference {
                table_reference: table_reference.clone(),
                existing_relation_id: existing_relation_id.clone(),
            });
        }
        if self.session.table_exist(table_reference.clone())? {
            return Err(ProgrammaticSchemaError::PreexistingTable {
                table_reference: table_reference.clone(),
            });
        }
        Ok(())
    }

    fn transformation_order(&self) -> Result<Vec<ProgrammaticRelationId>, ProgrammaticSchemaError> {
        let mut indegree = self
            .pending
            .keys()
            .cloned()
            .map(|relation_id| (relation_id, 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut dependents = BTreeMap::<ProgrammaticRelationId, Vec<ProgrammaticRelationId>>::new();

        for (output_relation_id, transformation) in &self.pending {
            for dependency in transformation.dependencies() {
                if self.pending.contains_key(dependency) {
                    *indegree
                        .get_mut(output_relation_id)
                        .expect("all pending outputs have an indegree") += 1;
                    dependents
                        .entry(dependency.clone())
                        .or_default()
                        .push(output_relation_id.clone());
                } else if !self.registered.contains_key(dependency) {
                    return Err(ProgrammaticSchemaError::UnresolvedDependency {
                        transformation_id: transformation.id().clone(),
                        relation_id: dependency.clone(),
                    });
                }
            }
        }

        let mut ready = indegree
            .iter()
            .filter_map(|(relation_id, count)| (*count == 0).then_some(relation_id.clone()))
            .collect::<BTreeSet<_>>();
        let mut order = Vec::with_capacity(self.pending.len());
        while let Some(relation_id) = ready.pop_first() {
            order.push(relation_id.clone());
            if let Some(outputs) = dependents.get(&relation_id) {
                for output in outputs {
                    let count = indegree
                        .get_mut(output)
                        .expect("dependent output has an indegree");
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(output.clone());
                    }
                }
            }
        }
        if order.len() != self.pending.len() {
            let transformations = indegree
                .into_iter()
                .filter_map(|(output, count)| {
                    (count != 0).then(|| {
                        self.pending
                            .get(&output)
                            .expect("cyclic output remains pending")
                            .id()
                            .clone()
                    })
                })
                .collect();
            return Err(ProgrammaticSchemaError::CyclicTransformations { transformations });
        }
        Ok(order)
    }

    async fn install_transformation(
        &mut self,
        transformation: Arc<dyn ProgrammaticTransformation>,
    ) -> Result<(), ProgrammaticSchemaError> {
        let mut inputs = BTreeMap::new();
        let mut expected_references = BTreeSet::new();
        for dependency in transformation.dependencies() {
            let registered = self.registered.get(dependency).ok_or_else(|| {
                ProgrammaticSchemaError::UnresolvedDependency {
                    transformation_id: transformation.id().clone(),
                    relation_id: dependency.clone(),
                }
            })?;
            let provider = self
                .session
                .table_provider(registered.table_reference.clone())
                .await?;
            let plan = LogicalPlanBuilder::scan(
                registered.table_reference.clone(),
                provider_as_source(provider),
                None,
            )?
            .build()?;
            expected_references.insert(registered.table_reference.clone());
            inputs.insert(
                dependency.clone(),
                TransformationInput {
                    table_reference: registered.table_reference.clone(),
                    plan,
                },
            );
        }

        let raw_plan = transformation
            .build(&TransformationInputs { inputs })
            .map_err(|source| ProgrammaticSchemaError::TransformationBuild {
                transformation_id: transformation.id().clone(),
                source,
            })?;
        let actual_references = scan_references(&raw_plan)?;
        if actual_references != expected_references {
            return Err(ProgrammaticSchemaError::PlanDependencyMismatch {
                transformation_id: transformation.id().clone(),
                expected: expected_references,
                actual: actual_references,
            });
        }
        validate_transformation_plan_policy(transformation.contract(), &raw_plan)?;

        let analyzed_plan = self.session.state().optimize(&raw_plan).map_err(|source| {
            ProgrammaticSchemaError::TransformationAnalysis {
                transformation_id: transformation.id().clone(),
                source,
            }
        })?;
        let installed_plan = output_identity_boundary(analyzed_plan, transformation.output())?;
        let actual_schema = Arc::clone(installed_plan.schema().inner());
        ensure_unique_fields(transformation.output().relation_id(), &actual_schema)?;
        if let Some(assertion) = transformation.output().schema_assertion()
            && assertion.as_ref() != actual_schema.as_ref()
        {
            return Err(ProgrammaticSchemaError::OutputSchemaAssertionMismatch {
                transformation_id: transformation.id().clone(),
                asserted: Arc::clone(assertion),
                actual: actual_schema,
            });
        }
        prove_transformation_execution_contract(
            &self.session,
            transformation.contract(),
            transformation.output(),
            &installed_plan,
        )
        .await?;

        let mappings = (0..actual_schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect();
        let contract = Arc::new(SchemaContract::try_new(
            Arc::<str>::from(transformation.id().as_str()),
            transformation.output().table_reference().clone(),
            Arc::clone(&actual_schema),
            Arc::clone(&actual_schema),
            mappings,
        )?);
        let view = Arc::new(IdentityPreservingViewTable::new(installed_plan.clone()));
        self.session.register_table(
            transformation.output().table_reference().clone(),
            view as Arc<dyn TableProvider>,
        )?;

        self.registered.insert(
            transformation.output().relation_id().clone(),
            RegisteredRelation {
                table_reference: transformation.output().table_reference().clone(),
                contract,
                origin: RegisteredOrigin::Transformation {
                    contract: Arc::new(transformation.contract().clone()),
                    dependencies: Arc::from(transformation.dependencies()),
                    plan: Arc::new(installed_plan),
                },
            },
        );
        Ok(())
    }

    #[cfg(test)]
    async fn prepare_observation_relations(
        &self,
        epoch_id: EpochId,
    ) -> Result<Vec<PreparedObservationRelation>, ProgrammaticSchemaError> {
        let specs = self.observation_relation_specs()?;
        for spec in &specs {
            validate_relation_id(&spec.relation_id)?;
            validate_full_reference(&spec.table_reference)?;
            self.ensure_binding_available(&spec.relation_id, &spec.table_reference)?;
        }

        let (_, mut observations) = self.observe_live_catalog().await?;
        append_system_observations(&mut observations, &specs)?;
        let batches = build_observation_batches(epoch_id, &observations, &specs)?;
        enforce_observation_materialization(&self.observation_policy, 1, &batches)?;
        Ok(specs
            .into_iter()
            .map(|spec| PreparedObservationRelation {
                batch: batches
                    .get(&spec.relation_id)
                    .expect("every observation contract has one built batch")
                    .clone(),
                relation_id: spec.relation_id,
                table_reference: spec.table_reference,
                contract: spec.contract,
            })
            .collect())
    }

    pub(crate) fn observation_relation_specs(
        &self,
    ) -> Result<Vec<PreparedObservationRelationSpec>, ProgrammaticSchemaError> {
        let catalog = self
            .session
            .state()
            .config_options()
            .catalog
            .default_catalog
            .clone();
        observation_relation_specs(&catalog)
    }

    async fn observe_live_catalog(
        &self,
    ) -> Result<
        (
            BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
            CandidateAssemblyObservations,
        ),
        ProgrammaticSchemaError,
    > {
        let mut sealed_relations = BTreeMap::new();
        let mut observations = CandidateAssemblyObservations::default();
        for (relation_id, registered) in &self.registered {
            let provider = self
                .session
                .table_provider(registered.table_reference.clone())
                .await?;
            let arrow_schema = provider.schema();
            if arrow_schema.as_ref() != registered.contract.logical_schema().as_ref() {
                return Err(ProgrammaticSchemaError::CatalogSchemaDrift {
                    relation_id: relation_id.clone(),
                    expected: Arc::clone(registered.contract.logical_schema()),
                    actual: arrow_schema,
                });
            }
            ensure_unique_fields(relation_id, &arrow_schema)?;
            let scan_plan = LogicalPlanBuilder::scan(
                registered.table_reference.clone(),
                provider_as_source(Arc::clone(&provider)),
                None,
            )?
            .build()?;
            let actual_datafusion_schema = Arc::clone(scan_plan.schema());

            let validate_registered_view =
                |plan: &LogicalPlan| {
                    // `LogicalPlanBuilder::scan` inlines any provider whose public
                    // `get_logical_plan` returns a plan. Inlining bypasses the
                    // physical metadata boundary required by transformations, so
                    // application-owned views retain the plan privately and expose
                    // it to this observation path through a concrete downcast.
                    let observed_plan = registered_view_logical_plan(provider.as_ref())
                        .ok_or_else(|| ProgrammaticSchemaError::ViewPlanUnavailable {
                            relation_id: relation_id.clone(),
                        })?;
                    if &observed_plan != plan {
                        return Err(ProgrammaticSchemaError::ViewPlanDrift {
                            relation_id: relation_id.clone(),
                        });
                    }
                    if provider.get_table_definition().is_some() {
                        return Err(ProgrammaticSchemaError::SqlViewDefinition {
                            relation_id: relation_id.clone(),
                        });
                    }
                    Ok(())
                };
            let origin = match &registered.origin {
                RegisteredOrigin::Provider { .. } => RelationOrigin::Provider,
                #[cfg(test)]
                RegisteredOrigin::SystemObservation => RelationOrigin::SystemObservation,
                RegisteredOrigin::Transformation { plan, .. } => {
                    validate_registered_view(plan)?;
                    RelationOrigin::Transformation
                }
                RegisteredOrigin::ObservationView { plan, .. } => {
                    validate_registered_view(plan)?;
                    RelationOrigin::Transformation
                }
            };
            observations.relations.push(RelationObservation {
                relation_id: relation_id.clone(),
                table_reference: registered.table_reference.clone(),
                origin,
                table_type: provider.table_type(),
            });
            observations.schemas.push(SchemaObservation {
                relation_id: relation_id.clone(),
                arrow_schema: Arc::clone(&arrow_schema),
                datafusion_schema: Arc::clone(&actual_datafusion_schema),
            });
            for (ordinal, (qualifier, field)) in actual_datafusion_schema.iter().enumerate() {
                let field_id = registered
                    .contract
                    .field_id_at(SchemaRole::Logical, ordinal)?;
                observations.fields.push(FieldObservation {
                    relation_id: relation_id.clone(),
                    field_id: ProgrammaticFieldId::new(field_id),
                    ordinal,
                    qualifier: qualifier.cloned(),
                    field: Arc::clone(field),
                });
            }

            match &registered.origin {
                RegisteredOrigin::Provider { .. } => {
                    observations
                        .provenance
                        .push(ProvenanceObservation::Provider {
                            relation_id: relation_id.clone(),
                            source_schema_identity: Arc::from(
                                registered.contract.source_schema_identity(),
                            ),
                        });
                }
                #[cfg(test)]
                RegisteredOrigin::SystemObservation => {
                    observations
                        .provenance
                        .push(ProvenanceObservation::SystemObservation {
                            relation_id: relation_id.clone(),
                            source_schema_identity: Arc::from(OBSERVATION_SOURCE_IDENTITY),
                        });
                }
                RegisteredOrigin::Transformation {
                    contract,
                    dependencies,
                    ..
                } => {
                    let observed_plan = registered_view_logical_plan(provider.as_ref())
                        .expect("the transformation view plan was checked above");
                    observations
                        .provenance
                        .push(ProvenanceObservation::Transformation {
                            relation_id: relation_id.clone(),
                            contract: Arc::clone(contract),
                            logical_plan: Arc::new(observed_plan),
                        });
                    observations
                        .dependencies
                        .extend(
                            dependencies
                                .iter()
                                .enumerate()
                                .map(|(ordinal, dependency)| DependencyObservation {
                                    transformation_id: contract.semantic_id().clone(),
                                    output_relation_id: relation_id.clone(),
                                    input_relation_id: dependency.clone(),
                                    ordinal,
                                }),
                        );
                }
                RegisteredOrigin::ObservationView {
                    transformation_id,
                    dependencies,
                    ..
                } => {
                    let observed_plan = registered_view_logical_plan(provider.as_ref())
                        .expect("the observation view plan was checked above");
                    observations
                        .provenance
                        .push(ProvenanceObservation::ObservationView {
                            relation_id: relation_id.clone(),
                            transformation_id: transformation_id.clone(),
                            logical_plan: Arc::new(observed_plan),
                        });
                    observations
                        .dependencies
                        .extend(
                            dependencies
                                .iter()
                                .enumerate()
                                .map(|(ordinal, dependency)| DependencyObservation {
                                    transformation_id: transformation_id.clone(),
                                    output_relation_id: relation_id.clone(),
                                    input_relation_id: dependency.clone(),
                                    ordinal,
                                }),
                        );
                }
            }
            sealed_relations.insert(
                relation_id.clone(),
                SealedRelationBinding {
                    table_reference: registered.table_reference.clone(),
                    contract: Arc::clone(&registered.contract),
                    actual_datafusion_schema,
                    logical_plan: match &registered.origin {
                        RegisteredOrigin::Provider { logical_plan } => {
                            logical_plan.as_ref().map(Arc::clone)
                        }
                        RegisteredOrigin::Transformation { plan, .. }
                        | RegisteredOrigin::ObservationView { plan, .. } => Some(Arc::clone(plan)),
                        #[cfg(test)]
                        RegisteredOrigin::SystemObservation => None,
                    },
                },
            );
        }
        Ok((sealed_relations, observations))
    }
}

fn validate_relation_id(
    relation_id: &ProgrammaticRelationId,
) -> Result<(), ProgrammaticSchemaError> {
    if relation_id.as_str().trim().is_empty() {
        return Err(ProgrammaticSchemaError::EmptyRelationIdentity);
    }
    Ok(())
}

fn validate_transformation_id(
    transformation_id: &ProgrammaticTransformationId,
) -> Result<(), ProgrammaticSchemaError> {
    if transformation_id.as_str().trim().is_empty() {
        return Err(ProgrammaticSchemaError::EmptyTransformationIdentity);
    }
    Ok(())
}

fn validate_transformation_contract(
    contract: &ProgrammaticTransformationContract,
    output: &TransformationOutput,
) -> Result<(), ProgrammaticSchemaError> {
    let transformation_id = contract.semantic_id();
    if contract.semantic_version().is_sentinel() {
        return Err(
            ProgrammaticSchemaError::SentinelTransformationSemanticVersion {
                transformation_id: transformation_id.clone(),
            },
        );
    }
    let resource_class = contract.resource_class();
    if resource_class.max_rows() == 0
        || resource_class.max_memory_bytes() == 0
        || resource_class.max_spill_bytes() == Some(0)
        || i64::try_from(resource_class.max_rows()).is_err()
        || i64::try_from(resource_class.max_memory_bytes()).is_err()
        || resource_class
            .max_spill_bytes()
            .is_some_and(|bound| i64::try_from(bound).is_err())
    {
        return Err(
            ProgrammaticSchemaError::InvalidTransformationResourceBounds {
                transformation_id: transformation_id.clone(),
                resource_class,
            },
        );
    }
    if contract.provenance().provenance_identity().as_bytes() == &[0; 32] {
        return Err(
            ProgrammaticSchemaError::SentinelTransformationProvenanceIdentity {
                transformation_id: transformation_id.clone(),
            },
        );
    }
    if contract.provenance().release_identity().as_bytes() == &[0; 32] {
        return Err(
            ProgrammaticSchemaError::SentinelTransformationReleaseIdentity {
                transformation_id: transformation_id.clone(),
            },
        );
    }
    if matches!(
        contract.recursion_policy(),
        TransformationRecursionPolicy::Bounded { max_iterations: 0 }
    ) {
        return Err(
            ProgrammaticSchemaError::InvalidTransformationRecursionBound {
                transformation_id: transformation_id.clone(),
            },
        );
    }

    match (contract.determinism_policy(), contract.ordering_policy()) {
        (
            TransformationDeterminismPolicy::DeterministicSet,
            TransformationOrderingPolicy::ByOutputFields(_),
        )
        | (
            TransformationDeterminismPolicy::DeterministicSequence,
            TransformationOrderingPolicy::Unordered,
        ) => {
            return Err(
                ProgrammaticSchemaError::IncompatibleTransformationPolicies {
                    transformation_id: transformation_id.clone(),
                    determinism_policy: contract.determinism_policy(),
                    ordering_policy: contract.ordering_policy().clone(),
                },
            );
        }
        (
            TransformationDeterminismPolicy::DeterministicSet
            | TransformationDeterminismPolicy::DeterministicSequence
            | TransformationDeterminismPolicy::Volatile,
            TransformationOrderingPolicy::Unordered
            | TransformationOrderingPolicy::ByOutputFields(_),
        ) => {}
    }

    if let TransformationOrderingPolicy::ByOutputFields(keys) = contract.ordering_policy() {
        if keys.is_empty() {
            return Err(ProgrammaticSchemaError::EmptyTransformationOrdering {
                transformation_id: transformation_id.clone(),
            });
        }
        let output_fields = output
            .fields()
            .iter()
            .map(TransformationFieldIdentity::field_id)
            .collect::<BTreeSet<_>>();
        let mut ordered_fields = BTreeSet::new();
        for key in keys.iter() {
            if !output_fields.contains(key.field_id()) {
                return Err(
                    ProgrammaticSchemaError::UnknownTransformationOrderingField {
                        transformation_id: transformation_id.clone(),
                        field_id: key.field_id().clone(),
                    },
                );
            }
            if !ordered_fields.insert(key.field_id().clone()) {
                return Err(
                    ProgrammaticSchemaError::DuplicateTransformationOrderingField {
                        transformation_id: transformation_id.clone(),
                        field_id: key.field_id().clone(),
                    },
                );
            }
        }
    }
    Ok(())
}

fn validate_full_reference(reference: &TableReference) -> Result<(), ProgrammaticSchemaError> {
    if !matches!(reference, TableReference::Full { .. }) {
        return Err(ProgrammaticSchemaError::UnqualifiedTableReference {
            table_reference: reference.clone(),
        });
    }
    Ok(())
}

fn scan_references(plan: &LogicalPlan) -> Result<BTreeSet<TableReference>, DataFusionError> {
    let mut references = BTreeSet::new();
    plan.apply_with_subqueries(|node| {
        if let LogicalPlan::TableScan(scan) = node {
            references.insert(scan.table_name.clone());
        }
        Ok(TreeNodeRecursion::Continue)
    })?;
    Ok(references)
}

fn validate_transformation_plan_policy(
    contract: &ProgrammaticTransformationContract,
    plan: &LogicalPlan,
) -> Result<(), ProgrammaticSchemaError> {
    let mut recursive_nodes = 0_usize;
    let mut highest_volatility = Volatility::Immutable;
    plan.apply_with_subqueries(|node| {
        if matches!(node, LogicalPlan::RecursiveQuery(_)) {
            recursive_nodes = recursive_nodes.saturating_add(1);
        }
        for expression in node.expressions() {
            expression.apply(|candidate| {
                let volatility = match candidate {
                    Expr::ScalarVariable(..) => Volatility::Volatile,
                    Expr::ScalarFunction(function) => function.func.signature().volatility,
                    Expr::AggregateFunction(function) => function.func.signature().volatility,
                    Expr::WindowFunction(function) => function.fun.signature().volatility,
                    Expr::HigherOrderFunction(function) => function.func.signature().volatility,
                    _ => Volatility::Immutable,
                };
                highest_volatility = highest_volatility.max(volatility);
                Ok(TreeNodeRecursion::Continue)
            })?;
        }
        Ok(TreeNodeRecursion::Continue)
    })
    .map_err(|source| ProgrammaticSchemaError::TransformationAnalysis {
        transformation_id: contract.semantic_id().clone(),
        source,
    })?;

    match contract.recursion_policy() {
        TransformationRecursionPolicy::Forbidden if recursive_nodes != 0 => {
            return Err(ProgrammaticSchemaError::TransformationRecursionForbidden {
                transformation_id: contract.semantic_id().clone(),
            });
        }
        TransformationRecursionPolicy::Bounded { .. } if recursive_nodes == 0 => {
            return Err(
                ProgrammaticSchemaError::TransformationRecursionDeclarationInert {
                    transformation_id: contract.semantic_id().clone(),
                },
            );
        }
        TransformationRecursionPolicy::Bounded { max_iterations } => {
            // DataFusion 55's native RecursiveQueryExec repeats until the working
            // table is empty and exposes no iteration limit. A row LIMIT cannot
            // prove or enforce an iteration bound, so fail closed until the native
            // planner/executor supplies that contract.
            return Err(ProgrammaticSchemaError::BoundedNativeRecursionUnavailable {
                transformation_id: contract.semantic_id().clone(),
                max_iterations,
            });
        }
        TransformationRecursionPolicy::Forbidden => {}
    }

    match contract.determinism_policy() {
        TransformationDeterminismPolicy::DeterministicSet
        | TransformationDeterminismPolicy::DeterministicSequence
            if highest_volatility != Volatility::Immutable =>
        {
            Err(
                ProgrammaticSchemaError::NonImmutableTransformationExpression {
                    transformation_id: contract.semantic_id().clone(),
                },
            )
        }
        TransformationDeterminismPolicy::Volatile
            if highest_volatility == Volatility::Immutable =>
        {
            Err(
                ProgrammaticSchemaError::VolatileTransformationDeclarationInert {
                    transformation_id: contract.semantic_id().clone(),
                },
            )
        }
        TransformationDeterminismPolicy::DeterministicSet
        | TransformationDeterminismPolicy::DeterministicSequence
        | TransformationDeterminismPolicy::Volatile => Ok(()),
    }
}

async fn prove_transformation_execution_contract(
    session: &SessionContext,
    contract: &ProgrammaticTransformationContract,
    output: &TransformationOutput,
    plan: &LogicalPlan,
) -> Result<(), ProgrammaticSchemaError> {
    let first = execute_transformation_proof_once(session, contract, output, plan).await?;
    if contract.determinism_policy() == TransformationDeterminismPolicy::Volatile {
        return Ok(());
    }
    let first_identity = transformation_execution_identity(contract, plan, &first)?;
    drop(first);

    // A new physical plan is deliberate: deterministic authority is proved
    // across two executions, while physical operators and their metrics remain
    // per-execution state rather than a cached result or physical plan.
    let second = execute_transformation_proof_once(session, contract, output, plan).await?;
    let second_identity = transformation_execution_identity(contract, plan, &second)?;
    if first_identity != second_identity {
        return Err(ProgrammaticSchemaError::TransformationNotDeterministic {
            transformation_id: contract.semantic_id().clone(),
        });
    }
    Ok(())
}

async fn execute_transformation_proof_once(
    session: &SessionContext,
    contract: &ProgrammaticTransformationContract,
    output: &TransformationOutput,
    plan: &LogicalPlan,
) -> Result<Vec<RecordBatch>, ProgrammaticSchemaError> {
    // Prove the same native ViewTable scan path installed in the candidate
    // catalog, rather than executing the view definition as a detached plan.
    // This forces DataFusion to reconcile the provider's advertised schema and
    // its physical output exactly as a later consumer will observe them.
    let proof_view = Arc::new(IdentityPreservingViewTable::new(plan.clone()));
    let proof_plan = LogicalPlanBuilder::scan(
        output.table_reference().clone(),
        provider_as_source(proof_view as Arc<dyn TableProvider>),
        None,
    )?
    .build()?;
    let physical = session
        .state()
        .create_physical_plan(&proof_plan)
        .await
        .map_err(
            |source| ProgrammaticSchemaError::TransformationPhysicalPlanning {
                transformation_id: contract.semantic_id().clone(),
                source,
            },
        )?;
    validate_transformation_output_ordering(contract, output, &physical)?;

    let mut stream =
        execute_stream(Arc::clone(&physical), session.task_ctx()).map_err(|source| {
            ProgrammaticSchemaError::TransformationExecution {
                transformation_id: contract.semantic_id().clone(),
                source,
            }
        })?;
    let expected_schema = proof_plan.schema().inner();
    let resource_class = contract.resource_class();
    let mut rows = 0_u64;
    let mut output_bytes = 0_u64;
    let mut batches = Vec::new();
    while let Some(batch) = stream.next().await {
        let batch = batch.map_err(|source| ProgrammaticSchemaError::TransformationExecution {
            transformation_id: contract.semantic_id().clone(),
            source,
        })?;
        if batch.schema_ref().as_ref() != expected_schema.as_ref() {
            return Err(
                ProgrammaticSchemaError::TransformationExecutionSchemaMismatch {
                    transformation_id: contract.semantic_id().clone(),
                    expected: Arc::clone(expected_schema),
                    actual: batch.schema(),
                },
            );
        }
        rows = rows
            .checked_add(u64::try_from(batch.num_rows()).unwrap_or(u64::MAX))
            .ok_or_else(
                || ProgrammaticSchemaError::TransformationResourceCounterOverflow {
                    transformation_id: contract.semantic_id().clone(),
                },
            )?;
        if rows > resource_class.max_rows() {
            return Err(ProgrammaticSchemaError::TransformationOutputRowsExceeded {
                transformation_id: contract.semantic_id().clone(),
                limit: resource_class.max_rows(),
                observed: rows,
            });
        }
        output_bytes = output_bytes
            .checked_add(u64::try_from(batch.get_array_memory_size()).unwrap_or(u64::MAX))
            .ok_or_else(
                || ProgrammaticSchemaError::TransformationResourceCounterOverflow {
                    transformation_id: contract.semantic_id().clone(),
                },
            )?;
        enforce_transformation_memory_bound(contract, output_bytes, &physical)?;
        enforce_transformation_spill_bound(contract, &physical)?;
        batches.push(batch);
    }
    enforce_transformation_memory_bound(contract, output_bytes, &physical)?;
    enforce_transformation_spill_bound(contract, &physical)?;
    Ok(batches)
}

fn validate_transformation_output_ordering(
    contract: &ProgrammaticTransformationContract,
    output: &TransformationOutput,
    physical: &Arc<dyn ExecutionPlan>,
) -> Result<(), ProgrammaticSchemaError> {
    let TransformationOrderingPolicy::ByOutputFields(expected_keys) = contract.ordering_policy()
    else {
        return Ok(());
    };
    let partition_count = physical.output_partitioning().partition_count();
    if partition_count != 1 {
        return Err(ProgrammaticSchemaError::TransformationOrderingNotGlobal {
            transformation_id: contract.semantic_id().clone(),
            partition_count,
        });
    }
    let Some(actual_ordering) = physical.output_ordering() else {
        return Err(
            ProgrammaticSchemaError::TransformationOrderingNotSatisfied {
                transformation_id: contract.semantic_id().clone(),
            },
        );
    };
    if actual_ordering.len() < expected_keys.len() {
        return Err(
            ProgrammaticSchemaError::TransformationOrderingNotSatisfied {
                transformation_id: contract.semantic_id().clone(),
            },
        );
    }
    for (expected, actual) in expected_keys.iter().zip(actual_ordering.iter()) {
        let expected_index = output
            .fields()
            .iter()
            .position(|field| field.field_id() == expected.field_id())
            .expect("ordering fields were validated against the output contract");
        let Some(actual_column) = actual.expr.downcast_ref::<PhysicalColumn>() else {
            return Err(
                ProgrammaticSchemaError::TransformationOrderingNotSatisfied {
                    transformation_id: contract.semantic_id().clone(),
                },
            );
        };
        let expected_descending = expected.direction() == TransformationSortDirection::Descending;
        let expected_nulls_first = expected.null_placement() == TransformationNullPlacement::First;
        if actual_column.index() != expected_index
            || actual.options.descending != expected_descending
            || actual.options.nulls_first != expected_nulls_first
        {
            return Err(
                ProgrammaticSchemaError::TransformationOrderingNotSatisfied {
                    transformation_id: contract.semantic_id().clone(),
                },
            );
        }
    }
    Ok(())
}

fn enforce_transformation_memory_bound(
    contract: &ProgrammaticTransformationContract,
    output_bytes: u64,
    physical: &Arc<dyn ExecutionPlan>,
) -> Result<(), ProgrammaticSchemaError> {
    let observed = output_bytes.max(observed_physical_memory_bytes(physical.as_ref()));
    let limit = contract.resource_class().max_memory_bytes();
    if observed > limit {
        return Err(ProgrammaticSchemaError::TransformationMemoryBytesExceeded {
            transformation_id: contract.semantic_id().clone(),
            limit,
            observed,
        });
    }
    Ok(())
}

fn observed_physical_memory_bytes(plan: &dyn ExecutionPlan) -> u64 {
    let local = plan.metrics().map_or(0_u64, |metrics| {
        let mut current = 0_u64;
        let mut peak = 0_u64;
        for metric in metrics.iter() {
            match metric.value() {
                MetricValue::CurrentMemoryUsage(gauge) => {
                    current = current.saturating_add(gauge.value() as u64);
                }
                MetricValue::PeakMemoryUsage { gauge, .. } => {
                    peak = peak.saturating_add(gauge.value() as u64);
                }
                _ => {}
            }
        }
        current.max(peak)
    });
    plan.children().iter().fold(local, |observed, child| {
        observed.saturating_add(observed_physical_memory_bytes(child.as_ref()))
    })
}

fn enforce_transformation_spill_bound(
    contract: &ProgrammaticTransformationContract,
    physical: &Arc<dyn ExecutionPlan>,
) -> Result<(), ProgrammaticSchemaError> {
    let (spill_count, spilled_bytes) = observed_physical_spill(physical.as_ref());
    match contract.resource_class() {
        TransformationResourceClass::BoundedInMemory { .. } if spill_count != 0 => {
            Err(ProgrammaticSchemaError::TransformationUnexpectedSpill {
                transformation_id: contract.semantic_id().clone(),
                spill_count,
                spilled_bytes,
            })
        }
        TransformationResourceClass::BoundedSpillable {
            max_spill_bytes, ..
        } if spilled_bytes > max_spill_bytes => {
            Err(ProgrammaticSchemaError::TransformationSpillBytesExceeded {
                transformation_id: contract.semantic_id().clone(),
                limit: max_spill_bytes,
                observed: spilled_bytes,
            })
        }
        TransformationResourceClass::BoundedInMemory { .. }
        | TransformationResourceClass::BoundedSpillable { .. } => Ok(()),
    }
}

fn observed_physical_spill(plan: &dyn ExecutionPlan) -> (u64, u64) {
    let (mut spill_count, mut spilled_bytes) = plan.metrics().map_or((0, 0), |metrics| {
        (
            metrics.spill_count().unwrap_or_default() as u64,
            metrics.spilled_bytes().unwrap_or_default() as u64,
        )
    });
    for child in plan.children() {
        let (child_count, child_bytes) = observed_physical_spill(child.as_ref());
        spill_count = spill_count.saturating_add(child_count);
        spilled_bytes = spilled_bytes.saturating_add(child_bytes);
    }
    (spill_count, spilled_bytes)
}

#[derive(Eq, PartialEq)]
enum TransformationExecutionIdentity {
    Set(String),
    Sequence([u8; 32]),
}

fn transformation_execution_identity(
    contract: &ProgrammaticTransformationContract,
    plan: &LogicalPlan,
    batches: &[RecordBatch],
) -> Result<TransformationExecutionIdentity, ProgrammaticSchemaError> {
    let maximum_encoding_bytes = usize::try_from(contract.resource_class().max_memory_bytes())
        .map_err(
            |_| ProgrammaticSchemaError::TransformationResourceCounterOverflow {
                transformation_id: contract.semantic_id().clone(),
            },
        )?;
    match contract.determinism_policy() {
        TransformationDeterminismPolicy::DeterministicSet => result_checksum_v2(
            plan.schema().inner().as_ref(),
            batches,
            maximum_encoding_bytes,
        )
        .map(|result| TransformationExecutionIdentity::Set(result.checksum))
        .map_err(
            |source| ProgrammaticSchemaError::TransformationDeterminismProof {
                transformation_id: contract.semantic_id().clone(),
                source,
            },
        ),
        TransformationDeterminismPolicy::DeterministicSequence => {
            ordered_execution_identity(plan.schema().inner().as_ref(), batches)
                .map(TransformationExecutionIdentity::Sequence)
                .map_err(
                    |source| ProgrammaticSchemaError::TransformationDeterminismProof {
                        transformation_id: contract.semantic_id().clone(),
                        source,
                    },
                )
        }
        TransformationDeterminismPolicy::Volatile => {
            unreachable!("volatile transformations are executed once without comparison")
        }
    }
}

fn ordered_execution_identity(
    schema: &Schema,
    batches: &[RecordBatch],
) -> Result<[u8; 32], ResultChecksumError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.programmatic-transformation-sequence-proof.v1");
    let converter = arrow_row::RowConverter::new(
        schema
            .fields()
            .iter()
            .map(|field| arrow_row::SortField::new(field.data_type().clone()))
            .collect(),
    )?;
    let mut row_count = 0_u64;
    for batch in batches {
        let batch_rows =
            u64::try_from(batch.num_rows()).map_err(|_| ResultChecksumError::ResourceLimit)?;
        row_count = row_count
            .checked_add(batch_rows)
            .ok_or(ResultChecksumError::ResourceLimit)?;
        if schema.fields().is_empty() {
            for _ in 0..batch.num_rows() {
                hasher.update(&0_u64.to_be_bytes());
            }
            continue;
        }
        let rows = converter.convert_columns(batch.columns())?;
        for row in &rows {
            hasher.update(&(row.data().len() as u64).to_be_bytes());
            hasher.update(row.data());
        }
    }
    hasher.update(&row_count.to_be_bytes());
    Ok(*hasher.finalize().as_bytes())
}

/// Add identity metadata to a schema otherwise derived wholly from the analyzed plan.
fn output_identity_boundary(
    plan: LogicalPlan,
    output: &TransformationOutput,
) -> Result<LogicalPlan, ProgrammaticSchemaError> {
    if plan.schema().fields().len() != output.fields().len() {
        return Err(ProgrammaticSchemaError::OutputFieldIdentityCount {
            relation_id: output.relation_id().clone(),
            expected: plan.schema().fields().len(),
            actual: output.fields().len(),
        });
    }
    let mut schema_metadata = plan.schema().metadata().clone();
    remove_inherited_identity_metadata(&mut schema_metadata);
    schema_metadata.insert(
        RELATION_ID_METADATA_KEY.to_owned(),
        output.relation_id().as_str().to_owned(),
    );
    let mut expressions = Vec::with_capacity(plan.schema().fields().len());
    let mut fields = Vec::with_capacity(plan.schema().fields().len());
    for ((qualifier, field), identity) in plan.schema().iter().zip(output.fields()) {
        let mut metadata = field.metadata().clone();
        remove_inherited_identity_metadata(&mut metadata);
        metadata.insert(
            FIELD_ID_METADATA_KEY.to_owned(),
            identity.field_id().as_str().to_owned(),
        );
        if let Some(semantic_role) = identity.semantic_role() {
            metadata.insert(
                SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                semantic_role.to_string(),
            );
        }
        expressions.push(
            Expr::Column(Column::from((qualifier, field))).alias_with_metadata(
                field.name().to_owned(),
                Some(FieldMetadata::from(metadata.clone())),
            ),
        );
        fields.push((
            None,
            Arc::new(field.as_ref().clone().with_metadata(metadata)),
        ));
    }
    let schema = DFSchema::new_with_metadata(fields, schema_metadata)?
        .with_functional_dependencies(plan.schema().functional_dependencies().clone())?;
    Ok(LogicalPlan::Projection(Projection::try_new_with_schema(
        expressions,
        Arc::new(plan),
        Arc::new(schema),
    )?))
}

/// Bind a plan-derived observation view to its exact programmatic contract
/// without changing the analyzed names, types, nullability, or field order.
pub(crate) fn observation_view_identity_boundary(
    plan: LogicalPlan,
    relation_id: &ProgrammaticRelationId,
    contract: &SchemaContract,
) -> Result<LogicalPlan, ProgrammaticSchemaError> {
    let actual = plan.schema().inner();
    let expected = contract.logical_schema();
    if actual.fields().len() != expected.fields().len()
        || actual
            .fields()
            .iter()
            .zip(expected.fields())
            .any(|(actual, expected)| {
                actual.name() != expected.name()
                    || actual.data_type() != expected.data_type()
                    || actual.is_nullable() != expected.is_nullable()
            })
    {
        return Err(ProgrammaticSchemaError::ProviderSchemaMismatch {
            relation_id: relation_id.clone(),
            expected: Arc::clone(expected),
            actual: Arc::clone(actual),
        });
    }
    let expressions = plan
        .schema()
        .iter()
        .zip(expected.fields())
        .map(|((qualifier, field), expected)| {
            Expr::Column(Column::from((qualifier, field))).alias_with_metadata(
                expected.name().to_owned(),
                Some(FieldMetadata::from(expected.metadata().clone())),
            )
        })
        .collect::<Vec<_>>();
    Ok(LogicalPlan::Projection(Projection::try_new_with_schema(
        expressions,
        Arc::new(plan),
        Arc::clone(contract.qualified_logical_schema()),
    )?))
}

fn remove_inherited_identity_metadata(metadata: &mut HashMap<String, String>) {
    metadata.remove(RELATION_ID_METADATA_KEY);
    metadata.remove(FIELD_ID_METADATA_KEY);
    metadata.remove(SEMANTIC_ROLE_METADATA_KEY);
}

fn validate_output_field_identities(
    transformation_id: &ProgrammaticTransformationId,
    fields: &[TransformationFieldIdentity],
) -> Result<(), ProgrammaticSchemaError> {
    let mut identities = BTreeSet::new();
    for field in fields {
        if field.field_id().as_str().trim().is_empty() {
            return Err(ProgrammaticSchemaError::EmptyOutputFieldIdentity {
                transformation_id: transformation_id.clone(),
            });
        }
        if !identities.insert(field.field_id().clone()) {
            return Err(ProgrammaticSchemaError::DuplicateOutputFieldIdentity {
                transformation_id: transformation_id.clone(),
                field_id: field.field_id().clone(),
            });
        }
        if field
            .semantic_role()
            .is_some_and(|semantic_role| semantic_role.trim().is_empty())
        {
            return Err(ProgrammaticSchemaError::EmptyOutputSemanticRole {
                transformation_id: transformation_id.clone(),
                field_id: field.field_id().clone(),
            });
        }
    }
    Ok(())
}

fn ensure_unique_fields(
    relation_id: &ProgrammaticRelationId,
    schema: &SchemaRef,
) -> Result<(), ProgrammaticSchemaError> {
    let mut names = BTreeSet::new();
    for field in schema.fields() {
        if field.name().trim().is_empty() {
            return Err(ProgrammaticSchemaError::EmptyFieldName {
                relation_id: relation_id.clone(),
            });
        }
        if !names.insert(field.name()) {
            return Err(ProgrammaticSchemaError::DuplicateFieldName {
                relation_id: relation_id.clone(),
                field_name: field.name().clone(),
            });
        }
    }
    Ok(())
}

fn observation_relation_specs(
    catalog: &str,
) -> Result<Vec<PreparedObservationRelationSpec>, ProgrammaticSchemaError> {
    let definitions = vec![
        (
            "system.programmatic_relation_observation",
            "programmatic_relation_observation",
            vec![
                (
                    "relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.relation_id",
                ),
                (
                    "catalog_name",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.catalog_name",
                ),
                (
                    "schema_name",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.schema_name",
                ),
                (
                    "table_name",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.table_name",
                ),
                (
                    "origin",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.origin",
                ),
                (
                    "table_type",
                    DataType::Utf8,
                    false,
                    "system.programmatic_relation_observation.table_type",
                ),
            ],
        ),
        (
            "system.programmatic_field_observation",
            "programmatic_field_observation",
            vec![
                (
                    "relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_field_observation.relation_id",
                ),
                (
                    "field_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_field_observation.field_id",
                ),
                (
                    "ordinal",
                    DataType::Int64,
                    false,
                    "system.programmatic_field_observation.ordinal",
                ),
                (
                    "qualifier_catalog",
                    DataType::Utf8,
                    true,
                    "system.programmatic_field_observation.qualifier_catalog",
                ),
                (
                    "qualifier_schema",
                    DataType::Utf8,
                    true,
                    "system.programmatic_field_observation.qualifier_schema",
                ),
                (
                    "qualifier_table",
                    DataType::Utf8,
                    true,
                    "system.programmatic_field_observation.qualifier_table",
                ),
                (
                    "field_name",
                    DataType::Utf8,
                    false,
                    "system.programmatic_field_observation.field_name",
                ),
                (
                    "data_type",
                    DataType::Utf8,
                    false,
                    "system.programmatic_field_observation.data_type",
                ),
                (
                    "nullable",
                    DataType::Boolean,
                    false,
                    "system.programmatic_field_observation.nullable",
                ),
            ],
        ),
        (
            "system.programmatic_schema_observation",
            "programmatic_schema_observation",
            vec![
                (
                    "relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_schema_observation.relation_id",
                ),
                (
                    "field_count",
                    DataType::Int64,
                    false,
                    "system.programmatic_schema_observation.field_count",
                ),
                (
                    "metadata_count",
                    DataType::Int64,
                    false,
                    "system.programmatic_schema_observation.metadata_count",
                ),
            ],
        ),
        (
            "system.programmatic_dependency_observation",
            "programmatic_dependency_observation",
            vec![
                (
                    "transformation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_dependency_observation.transformation_id",
                ),
                (
                    "output_relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_dependency_observation.output_relation_id",
                ),
                (
                    "input_relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_dependency_observation.input_relation_id",
                ),
                (
                    "ordinal",
                    DataType::Int64,
                    false,
                    "system.programmatic_dependency_observation.ordinal",
                ),
            ],
        ),
        (
            "system.programmatic_provenance_observation",
            "programmatic_provenance_observation",
            vec![
                (
                    "relation_id",
                    DataType::Utf8,
                    false,
                    "system.programmatic_provenance_observation.relation_id",
                ),
                (
                    "provenance_kind",
                    DataType::Utf8,
                    false,
                    "system.programmatic_provenance_observation.provenance_kind",
                ),
                (
                    "source_schema_identity",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.source_schema_identity",
                ),
                (
                    "transformation_id",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.transformation_id",
                ),
                (
                    "semantic_version_major",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.semantic_version_major",
                ),
                (
                    "semantic_version_minor",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.semantic_version_minor",
                ),
                (
                    "semantic_version_patch",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.semantic_version_patch",
                ),
                (
                    "resource_class",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.resource_class",
                ),
                (
                    "resource_max_rows",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.resource_max_rows",
                ),
                (
                    "resource_max_memory_bytes",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.resource_max_memory_bytes",
                ),
                (
                    "resource_max_spill_bytes",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.resource_max_spill_bytes",
                ),
                (
                    "determinism_policy",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.determinism_policy",
                ),
                (
                    "ordering_policy",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.ordering_policy",
                ),
                (
                    "ordering_key_count",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.ordering_key_count",
                ),
                (
                    "recursion_policy",
                    DataType::Utf8,
                    true,
                    "system.programmatic_provenance_observation.recursion_policy",
                ),
                (
                    "recursion_max_iterations",
                    DataType::Int64,
                    true,
                    "system.programmatic_provenance_observation.recursion_max_iterations",
                ),
                (
                    "provenance_identity",
                    DataType::FixedSizeBinary(32),
                    true,
                    "system.programmatic_provenance_observation.provenance_identity",
                ),
                (
                    "release_identity",
                    DataType::FixedSizeBinary(32),
                    true,
                    "system.programmatic_provenance_observation.release_identity",
                ),
                (
                    "contract_authority_identity",
                    DataType::FixedSizeBinary(32),
                    true,
                    "system.programmatic_provenance_observation.contract_authority_identity",
                ),
            ],
        ),
    ];
    definitions
        .into_iter()
        .map(|(relation_id, table_name, fields)| {
            let table_reference = TableReference::full(catalog, OBSERVATION_SCHEMA, table_name);
            let mut arrow_fields = Vec::with_capacity(fields.len() + 1);
            arrow_fields.push(
                Field::new("fabric_epoch_id", DataType::FixedSizeBinary(16), false).with_metadata(
                    HashMap::from([(
                        FIELD_ID_METADATA_KEY.to_owned(),
                        format!("{relation_id}.fabric_epoch_id"),
                    )]),
                ),
            );
            arrow_fields.extend(
                fields
                    .into_iter()
                    .map(|(name, data_type, nullable, field_id)| {
                        Field::new(name, data_type, nullable).with_metadata(HashMap::from([(
                            FIELD_ID_METADATA_KEY.to_owned(),
                            field_id.to_owned(),
                        )]))
                    }),
            );
            let logical_schema = Arc::new(Schema::new_with_metadata(
                arrow_fields,
                HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
            ));
            let storage_schema = Arc::new(Schema::new_with_metadata(
                logical_schema
                    .fields()
                    .iter()
                    .map(|field| {
                        if matches!(field.data_type(), DataType::FixedSizeBinary(_)) {
                            Arc::new(
                                Field::new(field.name(), DataType::Binary, field.is_nullable())
                                    .with_metadata(field.metadata().clone()),
                            )
                        } else {
                            Arc::clone(field)
                        }
                    })
                    .collect::<Vec<_>>(),
                logical_schema.metadata().clone(),
            ));
            let contract = Arc::new(SchemaContract::try_new(
                OBSERVATION_SOURCE_IDENTITY,
                table_reference.clone(),
                Arc::clone(&logical_schema),
                storage_schema,
                (0..logical_schema.fields().len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )?);
            Ok(PreparedObservationRelationSpec {
                relation_id: ProgrammaticRelationId::new(relation_id),
                table_reference,
                contract,
            })
        })
        .collect()
}

#[cfg(test)]
fn append_system_observations(
    observations: &mut CandidateAssemblyObservations,
    specs: &[PreparedObservationRelationSpec],
) -> Result<(), ProgrammaticSchemaError> {
    for spec in specs {
        let datafusion_schema = Arc::clone(spec.contract.qualified_logical_schema());
        observations.relations.push(RelationObservation {
            relation_id: spec.relation_id.clone(),
            table_reference: spec.table_reference.clone(),
            origin: RelationOrigin::SystemObservation,
            table_type: TableType::Base,
        });
        observations.schemas.push(SchemaObservation {
            relation_id: spec.relation_id.clone(),
            arrow_schema: Arc::clone(spec.contract.logical_schema()),
            datafusion_schema: Arc::clone(&datafusion_schema),
        });
        for (ordinal, (qualifier, field)) in datafusion_schema.iter().enumerate() {
            observations.fields.push(FieldObservation {
                relation_id: spec.relation_id.clone(),
                field_id: ProgrammaticFieldId::new(
                    spec.contract.field_id_at(SchemaRole::Logical, ordinal)?,
                ),
                ordinal,
                qualifier: qualifier.cloned(),
                field: Arc::clone(field),
            });
        }
        observations
            .provenance
            .push(ProvenanceObservation::SystemObservation {
                relation_id: spec.relation_id.clone(),
                source_schema_identity: Arc::from(OBSERVATION_SOURCE_IDENTITY),
            });
    }
    Ok(())
}

fn enforce_observation_materialization(
    policy: &ObservationFixedPointPolicy,
    iteration: u32,
    batches: &BTreeMap<ProgrammaticRelationId, RecordBatch>,
) -> Result<ObservationFixedPointEvidence, ProgrammaticSchemaError> {
    if iteration == 0 || iteration > policy.max_iterations() {
        return Err(
            ProgrammaticSchemaError::ObservationFixedPointIterationsExceeded {
                limit: policy.max_iterations(),
            },
        );
    }
    let mut total_rows = 0_u64;
    let mut total_bytes = 0_u64;
    for (relation_id, batch) in batches {
        let rows = u64::try_from(batch.num_rows())
            .map_err(|_| ProgrammaticSchemaError::ObservationResourceCounterOverflow)?;
        let bytes = u64::try_from(batch.get_array_memory_size())
            .map_err(|_| ProgrammaticSchemaError::ObservationResourceCounterOverflow)?;
        if rows > policy.max_rows_per_relation() {
            return Err(ProgrammaticSchemaError::ObservationRelationRowsExceeded {
                relation_id: relation_id.clone(),
                limit: policy.max_rows_per_relation(),
                observed: rows,
            });
        }
        if bytes > policy.max_bytes_per_relation() {
            return Err(ProgrammaticSchemaError::ObservationRelationBytesExceeded {
                relation_id: relation_id.clone(),
                limit: policy.max_bytes_per_relation(),
                observed: bytes,
            });
        }
        total_rows = total_rows
            .checked_add(rows)
            .ok_or(ProgrammaticSchemaError::ObservationResourceCounterOverflow)?;
        total_bytes = total_bytes
            .checked_add(bytes)
            .ok_or(ProgrammaticSchemaError::ObservationResourceCounterOverflow)?;
    }
    if total_rows > policy.max_total_rows() {
        return Err(ProgrammaticSchemaError::ObservationTotalRowsExceeded {
            limit: policy.max_total_rows(),
            observed: total_rows,
        });
    }
    if total_bytes > policy.max_total_bytes() {
        return Err(ProgrammaticSchemaError::ObservationTotalBytesExceeded {
            limit: policy.max_total_bytes(),
            observed: total_bytes,
        });
    }
    Ok(ObservationFixedPointEvidence {
        iterations: iteration,
        relation_count: batches.len(),
        total_rows,
        total_bytes,
    })
}

fn build_observation_batches(
    epoch_id: EpochId,
    observations: &CandidateAssemblyObservations,
    specs: &[PreparedObservationRelationSpec],
) -> Result<BTreeMap<ProgrammaticRelationId, RecordBatch>, ProgrammaticSchemaError> {
    let schema = |relation_id: &str| {
        specs
            .iter()
            .find(|spec| spec.relation_id.as_str() == relation_id)
            .map(|spec| Arc::clone(spec.contract.logical_schema()))
            .expect("closed observation relation has a schema")
    };

    let mut relations = observations.relations.clone();
    relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    let relation_batch = RecordBatch::try_new(
        schema("system.programmatic_relation_observation"),
        vec![
            id16_array(std::iter::repeat_n(
                Some(epoch_id.as_bytes()),
                relations.len(),
            )),
            Arc::new(StringArray::from_iter_values(
                relations.iter().map(|row| row.relation_id.as_str()),
            )) as ArrayRef,
            Arc::new(StringArray::from_iter_values(
                relations
                    .iter()
                    .map(|row| full_reference_parts(&row.table_reference).0),
            )),
            Arc::new(StringArray::from_iter_values(
                relations
                    .iter()
                    .map(|row| full_reference_parts(&row.table_reference).1),
            )),
            Arc::new(StringArray::from_iter_values(
                relations
                    .iter()
                    .map(|row| full_reference_parts(&row.table_reference).2),
            )),
            Arc::new(StringArray::from_iter_values(
                relations.iter().map(|row| relation_origin_code(row.origin)),
            )),
            Arc::new(StringArray::from_iter_values(
                relations.iter().map(|row| table_type_code(row.table_type)),
            )),
        ],
    )?;

    let mut fields = observations.fields.clone();
    fields.sort_by(|left, right| {
        (&left.relation_id, left.ordinal).cmp(&(&right.relation_id, right.ordinal))
    });
    let field_batch = RecordBatch::try_new(
        schema("system.programmatic_field_observation"),
        vec![
            id16_array(std::iter::repeat_n(Some(epoch_id.as_bytes()), fields.len())),
            Arc::new(StringArray::from_iter_values(
                fields.iter().map(|row| row.relation_id.as_str()),
            )) as ArrayRef,
            Arc::new(StringArray::from_iter_values(
                fields.iter().map(|row| row.field_id.as_str()),
            )),
            Arc::new(Int64Array::from_iter_values(fields.iter().map(|row| {
                i64::try_from(row.ordinal).expect("field ordinal fits i64")
            }))),
            Arc::new(StringArray::from_iter(fields.iter().map(|row| {
                row.qualifier.as_ref().and_then(TableReference::catalog)
            }))),
            Arc::new(StringArray::from_iter(fields.iter().map(|row| {
                row.qualifier.as_ref().and_then(TableReference::schema)
            }))),
            Arc::new(StringArray::from_iter(
                fields
                    .iter()
                    .map(|row| row.qualifier.as_ref().map(TableReference::table)),
            )),
            Arc::new(StringArray::from_iter_values(
                fields.iter().map(|row| row.field.name()),
            )),
            Arc::new(StringArray::from_iter_values(
                fields.iter().map(|row| row.field.data_type().to_string()),
            )),
            Arc::new(BooleanArray::from(
                fields
                    .iter()
                    .map(|row| row.field.is_nullable())
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;

    let mut schemas = observations.schemas.clone();
    schemas.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    let schema_batch = RecordBatch::try_new(
        schema("system.programmatic_schema_observation"),
        vec![
            id16_array(std::iter::repeat_n(
                Some(epoch_id.as_bytes()),
                schemas.len(),
            )),
            Arc::new(StringArray::from_iter_values(
                schemas.iter().map(|row| row.relation_id.as_str()),
            )) as ArrayRef,
            Arc::new(Int64Array::from_iter_values(schemas.iter().map(|row| {
                i64::try_from(row.arrow_schema.fields().len()).expect("field count fits i64")
            }))),
            Arc::new(Int64Array::from_iter_values(schemas.iter().map(|row| {
                i64::try_from(row.arrow_schema.metadata().len()).expect("metadata count fits i64")
            }))),
        ],
    )?;

    let mut dependencies = observations.dependencies.clone();
    dependencies.sort_by(|left, right| {
        (&left.output_relation_id, left.ordinal).cmp(&(&right.output_relation_id, right.ordinal))
    });
    let dependency_batch = RecordBatch::try_new(
        schema("system.programmatic_dependency_observation"),
        vec![
            id16_array(std::iter::repeat_n(
                Some(epoch_id.as_bytes()),
                dependencies.len(),
            )),
            Arc::new(StringArray::from_iter_values(
                dependencies
                    .iter()
                    .map(|row| row.transformation_id.as_str()),
            )) as ArrayRef,
            Arc::new(StringArray::from_iter_values(
                dependencies
                    .iter()
                    .map(|row| row.output_relation_id.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(
                dependencies
                    .iter()
                    .map(|row| row.input_relation_id.as_str()),
            )),
            Arc::new(Int64Array::from_iter_values(dependencies.iter().map(
                |row| i64::try_from(row.ordinal).expect("dependency ordinal fits i64"),
            ))),
        ],
    )?;

    let mut provenance = observations.provenance.clone();
    provenance.sort_by(|left, right| left.relation_id().cmp(right.relation_id()));
    let provenance_identities = provenance
        .iter()
        .map(|row| {
            transformation_contract(row)
                .map(|contract| *contract.provenance().provenance_identity().as_bytes())
        })
        .collect::<Vec<_>>();
    let release_identities = provenance
        .iter()
        .map(|row| {
            transformation_contract(row)
                .map(|contract| *contract.provenance().release_identity().as_bytes())
        })
        .collect::<Vec<_>>();
    let contract_authority_identities = provenance
        .iter()
        .map(|row| {
            transformation_contract(row).map(ProgrammaticTransformationContract::authority_identity)
        })
        .collect::<Vec<_>>();
    let provenance_batch = RecordBatch::try_new(
        schema("system.programmatic_provenance_observation"),
        vec![
            id16_array(std::iter::repeat_n(
                Some(epoch_id.as_bytes()),
                provenance.len(),
            )),
            Arc::new(StringArray::from_iter_values(
                provenance.iter().map(|row| row.relation_id().as_str()),
            )) as ArrayRef,
            Arc::new(StringArray::from_iter_values(
                provenance.iter().map(provenance_kind_code),
            )),
            Arc::new(StringArray::from_iter(provenance.iter().map(
                |row| match row {
                    ProvenanceObservation::Provider {
                        source_schema_identity,
                        ..
                    }
                    | ProvenanceObservation::SystemObservation {
                        source_schema_identity,
                        ..
                    } => Some(source_schema_identity.as_ref()),
                    ProvenanceObservation::Transformation { .. }
                    | ProvenanceObservation::ObservationView { .. } => None,
                },
            ))),
            Arc::new(StringArray::from_iter(provenance.iter().map(|row| {
                transformation_id(row).map(ProgrammaticTransformationId::as_str)
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| i64::from(contract.semantic_version().major()))
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| i64::from(contract.semantic_version().minor()))
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| i64::from(contract.semantic_version().patch()))
            }))),
            Arc::new(StringArray::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| resource_class_code(contract.resource_class()))
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row).map(|contract| {
                    i64::try_from(contract.resource_class().max_rows())
                        .expect("validated transformation row bound fits i64")
                })
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row).map(|contract| {
                    i64::try_from(contract.resource_class().max_memory_bytes())
                        .expect("validated transformation memory bound fits i64")
                })
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row).and_then(|contract| {
                    contract.resource_class().max_spill_bytes().map(|bound| {
                        i64::try_from(bound).expect("validated transformation spill bound fits i64")
                    })
                })
            }))),
            Arc::new(StringArray::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| determinism_policy_code(contract.determinism_policy()))
            }))),
            Arc::new(StringArray::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| ordering_policy_code(contract.ordering_policy()))
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row).map(|contract| match contract.ordering_policy() {
                    TransformationOrderingPolicy::Unordered => 0,
                    TransformationOrderingPolicy::ByOutputFields(keys) => i64::try_from(keys.len())
                        .expect("transformation ordering key count fits i64"),
                })
            }))),
            Arc::new(StringArray::from_iter(provenance.iter().map(|row| {
                transformation_contract(row)
                    .map(|contract| recursion_policy_code(contract.recursion_policy()))
            }))),
            Arc::new(Int64Array::from_iter(provenance.iter().map(|row| {
                transformation_contract(row).and_then(|contract| {
                    match contract.recursion_policy() {
                        TransformationRecursionPolicy::Forbidden => None,
                        TransformationRecursionPolicy::Bounded { max_iterations } => {
                            Some(i64::from(max_iterations))
                        }
                    }
                })
            }))),
            fixed32_array(provenance_identities.iter().map(Option::as_ref)),
            fixed32_array(release_identities.iter().map(Option::as_ref)),
            fixed32_array(contract_authority_identities.iter().map(Option::as_ref)),
        ],
    )?;

    Ok(BTreeMap::from([
        (
            ProgrammaticRelationId::new("system.programmatic_relation_observation"),
            relation_batch,
        ),
        (
            ProgrammaticRelationId::new("system.programmatic_field_observation"),
            field_batch,
        ),
        (
            ProgrammaticRelationId::new("system.programmatic_schema_observation"),
            schema_batch,
        ),
        (
            ProgrammaticRelationId::new("system.programmatic_dependency_observation"),
            dependency_batch,
        ),
        (
            ProgrammaticRelationId::new("system.programmatic_provenance_observation"),
            provenance_batch,
        ),
    ]))
}

fn full_reference_parts(reference: &TableReference) -> (&str, &str, &str) {
    match reference {
        TableReference::Full {
            catalog,
            schema,
            table,
        } => (catalog, schema, table),
        TableReference::Bare { .. } | TableReference::Partial { .. } => {
            unreachable!("assembly accepts only fully qualified references")
        }
    }
}

fn update_contract_frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

const fn resource_class_tag(resource_class: TransformationResourceClass) -> u8 {
    match resource_class {
        TransformationResourceClass::BoundedInMemory { .. } => 0,
        TransformationResourceClass::BoundedSpillable { .. } => 1,
    }
}

const fn resource_class_code(resource_class: TransformationResourceClass) -> &'static str {
    match resource_class {
        TransformationResourceClass::BoundedInMemory { .. } => "bounded_in_memory",
        TransformationResourceClass::BoundedSpillable { .. } => "bounded_spillable",
    }
}

const fn determinism_policy_tag(policy: TransformationDeterminismPolicy) -> u8 {
    match policy {
        TransformationDeterminismPolicy::DeterministicSet => 0,
        TransformationDeterminismPolicy::DeterministicSequence => 1,
        TransformationDeterminismPolicy::Volatile => 2,
    }
}

const fn determinism_policy_code(policy: TransformationDeterminismPolicy) -> &'static str {
    match policy {
        TransformationDeterminismPolicy::DeterministicSet => "deterministic_set",
        TransformationDeterminismPolicy::DeterministicSequence => "deterministic_sequence",
        TransformationDeterminismPolicy::Volatile => "volatile",
    }
}

const fn ordering_policy_code(policy: &TransformationOrderingPolicy) -> &'static str {
    match policy {
        TransformationOrderingPolicy::Unordered => "unordered",
        TransformationOrderingPolicy::ByOutputFields(_) => "by_output_fields",
    }
}

const fn sort_direction_tag(direction: TransformationSortDirection) -> u8 {
    match direction {
        TransformationSortDirection::Ascending => 0,
        TransformationSortDirection::Descending => 1,
    }
}

const fn null_placement_tag(null_placement: TransformationNullPlacement) -> u8 {
    match null_placement {
        TransformationNullPlacement::First => 0,
        TransformationNullPlacement::Last => 1,
    }
}

const fn recursion_policy_code(policy: TransformationRecursionPolicy) -> &'static str {
    match policy {
        TransformationRecursionPolicy::Forbidden => "forbidden",
        TransformationRecursionPolicy::Bounded { .. } => "bounded",
    }
}

fn transformation_contract(
    provenance: &ProvenanceObservation,
) -> Option<&ProgrammaticTransformationContract> {
    provenance.transformation_contract()
}

fn transformation_id(provenance: &ProvenanceObservation) -> Option<&ProgrammaticTransformationId> {
    match provenance {
        ProvenanceObservation::Transformation { contract, .. } => Some(contract.semantic_id()),
        ProvenanceObservation::ObservationView {
            transformation_id, ..
        } => Some(transformation_id),
        ProvenanceObservation::Provider { .. }
        | ProvenanceObservation::SystemObservation { .. } => None,
    }
}

fn fixed32_array<'a>(values: impl IntoIterator<Item = Option<&'a [u8; 32]>>) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::new(32);
    for value in values {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("typed transformation identity has the governed storage width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

const fn relation_origin_code(origin: RelationOrigin) -> &'static str {
    match origin {
        RelationOrigin::Provider => "provider",
        RelationOrigin::Transformation => "transformation",
        RelationOrigin::SystemObservation => "system_observation",
    }
}

const fn table_type_code(table_type: TableType) -> &'static str {
    match table_type {
        TableType::Base => "base",
        TableType::View => "view",
        TableType::Temporary => "temporary",
    }
}

const fn provenance_kind_code(provenance: &ProvenanceObservation) -> &'static str {
    match provenance {
        ProvenanceObservation::Provider { .. } => "provider_contract",
        ProvenanceObservation::Transformation { .. } => "native_logical_plan",
        ProvenanceObservation::ObservationView { .. } => "native_observation_view",
        ProvenanceObservation::SystemObservation { .. } => "assembly_observation",
    }
}

/// Fail-closed errors at the candidate-session assembly boundary.
#[derive(Debug, Error)]
pub enum ProgrammaticSchemaError {
    #[error("programmatic relation identity is empty")]
    EmptyRelationIdentity,
    #[error("programmatic transformation identity is empty")]
    EmptyTransformationIdentity,
    #[error("table reference must be fully qualified: {table_reference}")]
    UnqualifiedTableReference { table_reference: TableReference },
    #[error("duplicate relation identity {relation_id:?}")]
    DuplicateRelation { relation_id: ProgrammaticRelationId },
    #[error("table {table_reference} is already bound to relation {existing_relation_id:?}")]
    DuplicateTableReference {
        table_reference: TableReference,
        existing_relation_id: ProgrammaticRelationId,
    },
    #[error("candidate catalog already contains table {table_reference}")]
    PreexistingTable { table_reference: TableReference },
    #[error("duplicate transformation identity {transformation_id:?}")]
    DuplicateTransformation {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} uses the reserved semantic version 0.0.0")]
    SentinelTransformationSemanticVersion {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} has a zero resource bound")]
    InvalidTransformationResourceBounds {
        transformation_id: ProgrammaticTransformationId,
        resource_class: TransformationResourceClass,
    },
    #[error("transformation {transformation_id:?} uses the all-zero provenance identity")]
    SentinelTransformationProvenanceIdentity {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} uses the all-zero release identity")]
    SentinelTransformationReleaseIdentity {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} declares zero recursive iterations")]
    InvalidTransformationRecursionBound {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "transformation {transformation_id:?} has incompatible determinism {determinism_policy:?} and ordering {ordering_policy:?} policies"
    )]
    IncompatibleTransformationPolicies {
        transformation_id: ProgrammaticTransformationId,
        determinism_policy: TransformationDeterminismPolicy,
        ordering_policy: TransformationOrderingPolicy,
    },
    #[error("transformation {transformation_id:?} declares an empty ordering key set")]
    EmptyTransformationOrdering {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "transformation {transformation_id:?} ordering names unknown output field {field_id:?}"
    )]
    UnknownTransformationOrderingField {
        transformation_id: ProgrammaticTransformationId,
        field_id: ProgrammaticFieldId,
    },
    #[error("transformation {transformation_id:?} repeats ordering output field {field_id:?}")]
    DuplicateTransformationOrderingField {
        transformation_id: ProgrammaticTransformationId,
        field_id: ProgrammaticFieldId,
    },
    #[error("transformation {transformation_id:?} repeats dependency {relation_id:?}")]
    DuplicateDependency {
        transformation_id: ProgrammaticTransformationId,
        relation_id: ProgrammaticRelationId,
    },
    #[error("transformation {transformation_id:?} has an empty output field identity")]
    EmptyOutputFieldIdentity {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} repeats output field identity {field_id:?}")]
    DuplicateOutputFieldIdentity {
        transformation_id: ProgrammaticTransformationId,
        field_id: ProgrammaticFieldId,
    },
    #[error(
        "transformation {transformation_id:?} output field {field_id:?} has an empty semantic role"
    )]
    EmptyOutputSemanticRole {
        transformation_id: ProgrammaticTransformationId,
        field_id: ProgrammaticFieldId,
    },
    #[error(
        "provider relation {relation_id:?} contract qualifier differs: expected {expected}, actual {actual}"
    )]
    ProviderContractQualifier {
        relation_id: ProgrammaticRelationId,
        expected: TableReference,
        actual: TableReference,
    },
    #[error("provider relation {relation_id:?} schema differs from its exact contract")]
    ProviderSchemaMismatch {
        relation_id: ProgrammaticRelationId,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error(
        "provider relation {relation_id:?} carries conflicting encoded identity {encoded_relation_id:?}"
    )]
    ProviderRelationIdentityMismatch {
        relation_id: ProgrammaticRelationId,
        encoded_relation_id: String,
    },
    #[error("transformation {transformation_id:?} has unresolved dependency {relation_id:?}")]
    UnresolvedDependency {
        transformation_id: ProgrammaticTransformationId,
        relation_id: ProgrammaticRelationId,
    },
    #[error("cyclic transformations: {transformations:?}")]
    CyclicTransformations {
        transformations: Vec<ProgrammaticTransformationId>,
    },
    #[error("transformation {transformation_id:?} failed to build")]
    TransformationBuild {
        transformation_id: ProgrammaticTransformationId,
        #[source]
        source: TransformationPlanError,
    },
    #[error(
        "transformation {transformation_id:?} plan dependencies differ: expected {expected:?}, actual {actual:?}"
    )]
    PlanDependencyMismatch {
        transformation_id: ProgrammaticTransformationId,
        expected: BTreeSet<TableReference>,
        actual: BTreeSet<TableReference>,
    },
    #[error("transformation {transformation_id:?} failed candidate-session analysis")]
    TransformationAnalysis {
        transformation_id: ProgrammaticTransformationId,
        #[source]
        source: DataFusionError,
    },
    #[error("transformation {transformation_id:?} contains recursion but declares it forbidden")]
    TransformationRecursionForbidden {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "transformation {transformation_id:?} declares bounded recursion but its plan has no native recursive query"
    )]
    TransformationRecursionDeclarationInert {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "transformation {transformation_id:?} requests {max_iterations} bounded recursive iterations, but DataFusion 55 native recursive execution exposes no iteration cap"
    )]
    BoundedNativeRecursionUnavailable {
        transformation_id: ProgrammaticTransformationId,
        max_iterations: u32,
    },
    #[error(
        "deterministic transformation {transformation_id:?} contains a stable, volatile, or ambient expression"
    )]
    NonImmutableTransformationExpression {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "volatile transformation {transformation_id:?} contains no stable, volatile, or ambient expression"
    )]
    VolatileTransformationDeclarationInert {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} failed physical planning")]
    TransformationPhysicalPlanning {
        transformation_id: ProgrammaticTransformationId,
        #[source]
        source: DataFusionError,
    },
    #[error("transformation {transformation_id:?} declared ordering is not globally partitioned")]
    TransformationOrderingNotGlobal {
        transformation_id: ProgrammaticTransformationId,
        partition_count: usize,
    },
    #[error("transformation {transformation_id:?} physical output does not satisfy its ordering")]
    TransformationOrderingNotSatisfied {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} failed its bounded proof execution")]
    TransformationExecution {
        transformation_id: ProgrammaticTransformationId,
        #[source]
        source: DataFusionError,
    },
    #[error("transformation {transformation_id:?} execution schema differs from its plan")]
    TransformationExecutionSchemaMismatch {
        transformation_id: ProgrammaticTransformationId,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("transformation {transformation_id:?} resource counter overflowed")]
    TransformationResourceCounterOverflow {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error(
        "transformation {transformation_id:?} produced {observed} rows beyond its {limit} row bound"
    )]
    TransformationOutputRowsExceeded {
        transformation_id: ProgrammaticTransformationId,
        limit: u64,
        observed: u64,
    },
    #[error(
        "transformation {transformation_id:?} used {observed} observed bytes beyond its {limit} memory bound"
    )]
    TransformationMemoryBytesExceeded {
        transformation_id: ProgrammaticTransformationId,
        limit: u64,
        observed: u64,
    },
    #[error(
        "in-memory transformation {transformation_id:?} spilled {spilled_bytes} bytes in {spill_count} spills"
    )]
    TransformationUnexpectedSpill {
        transformation_id: ProgrammaticTransformationId,
        spill_count: u64,
        spilled_bytes: u64,
    },
    #[error(
        "transformation {transformation_id:?} spilled {observed} bytes beyond its {limit} spill bound"
    )]
    TransformationSpillBytesExceeded {
        transformation_id: ProgrammaticTransformationId,
        limit: u64,
        observed: u64,
    },
    #[error("transformation {transformation_id:?} determinism proof failed")]
    TransformationDeterminismProof {
        transformation_id: ProgrammaticTransformationId,
        #[source]
        source: ResultChecksumError,
    },
    #[error("transformation {transformation_id:?} produced different results on re-execution")]
    TransformationNotDeterministic {
        transformation_id: ProgrammaticTransformationId,
    },
    #[error("transformation {transformation_id:?} output schema assertion differs from its plan")]
    OutputSchemaAssertionMismatch {
        transformation_id: ProgrammaticTransformationId,
        asserted: SchemaRef,
        actual: SchemaRef,
    },
    #[error(
        "relation {relation_id:?} plan has {expected} fields but its output binding has {actual} identities"
    )]
    OutputFieldIdentityCount {
        relation_id: ProgrammaticRelationId,
        expected: usize,
        actual: usize,
    },
    #[error("relation {relation_id:?} has an empty observed field name")]
    EmptyFieldName { relation_id: ProgrammaticRelationId },
    #[error("relation {relation_id:?} repeats observed field name {field_name:?}")]
    DuplicateFieldName {
        relation_id: ProgrammaticRelationId,
        field_name: String,
    },
    #[error("relation {relation_id:?} schema changed after catalog registration")]
    CatalogSchemaDrift {
        relation_id: ProgrammaticRelationId,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("transformation relation {relation_id:?} has no live view plan")]
    ViewPlanUnavailable { relation_id: ProgrammaticRelationId },
    #[error("transformation relation {relation_id:?} live view plan changed")]
    ViewPlanDrift { relation_id: ProgrammaticRelationId },
    #[error("transformation relation {relation_id:?} unexpectedly carries SQL authority")]
    SqlViewDefinition { relation_id: ProgrammaticRelationId },
    #[error("system observation fixed point did not converge within {limit} iterations")]
    ObservationFixedPointIterationsExceeded { limit: u32 },
    #[error("system observation resource counter overflowed")]
    ObservationResourceCounterOverflow,
    #[error(
        "observation relation {relation_id:?} produced {observed} rows beyond its {limit} row bound"
    )]
    ObservationRelationRowsExceeded {
        relation_id: ProgrammaticRelationId,
        limit: u64,
        observed: u64,
    },
    #[error("observation families produced {observed} rows beyond their {limit} total row bound")]
    ObservationTotalRowsExceeded { limit: u64, observed: u64 },
    #[error(
        "observation relation {relation_id:?} retained {observed} bytes beyond its {limit} memory bound"
    )]
    ObservationRelationBytesExceeded {
        relation_id: ProgrammaticRelationId,
        limit: u64,
        observed: u64,
    },
    #[error(
        "observation families retained {observed} bytes beyond their {limit} total memory bound"
    )]
    ObservationTotalBytesExceeded { limit: u64, observed: u64 },
    #[error("registered relation {relation_id:?} is missing during candidate-only replacement")]
    CatalogReplacementMissing { relation_id: ProgrammaticRelationId },
    #[error("registered relation {relation_id:?} is not an observation view")]
    CatalogReplacementNotView { relation_id: ProgrammaticRelationId },
    #[error("observation batch was not built for {relation_id:?}")]
    ObservationBatchMissing { relation_id: ProgrammaticRelationId },
    #[error(transparent)]
    SchemaContract(#[from] SchemaContractError),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{
        ArrayRef, BooleanArray, FixedSizeBinaryArray, Int64Array, RecordBatch, StringArray,
    };
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::catalog::MemorySchemaProvider;
    use datafusion::datasource::MemTable;
    use datafusion::logical_expr::{LogicalPlanBuilder, lit};
    use datafusion::prelude::col;

    use super::*;

    #[derive(Debug)]
    struct FilterProjection {
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        dependencies: Vec<ProgrammaticRelationId>,
        minimum_id: i64,
        include_active: bool,
    }

    impl ProgrammaticTransformation for FilterProjection {
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
            let input = inputs.plan(&self.dependencies[0])?;
            let mut projection = vec![col("id")];
            if self.include_active {
                projection.push(col("active"));
            }
            Ok(LogicalPlanBuilder::from(input)
                .filter(
                    col("active")
                        .eq(lit(true))
                        .and(col("id").gt_eq(lit(self.minimum_id))),
                )?
                .project(projection)?
                .build()?)
        }
    }

    #[derive(Debug)]
    struct Passthrough {
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        dependencies: Vec<ProgrammaticRelationId>,
    }

    impl ProgrammaticTransformation for Passthrough {
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
            inputs.plan(&self.dependencies[0])
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum PolicyPlanKind {
        Project,
        OrderedAscending,
        Volatile,
        Recursive,
    }

    #[derive(Debug)]
    struct PolicyProjection {
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        dependencies: Vec<ProgrammaticRelationId>,
        kind: PolicyPlanKind,
    }

    impl ProgrammaticTransformation for PolicyProjection {
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
            let input = inputs.plan(&self.dependencies[0])?;
            let projection = match self.kind {
                PolicyPlanKind::Volatile => LogicalPlanBuilder::from(input)
                    .project([
                        col("id"),
                        datafusion::functions::math::random()
                            .call(vec![])
                            .alias("nonce"),
                    ])?
                    .build()?,
                PolicyPlanKind::Project
                | PolicyPlanKind::OrderedAscending
                | PolicyPlanKind::Recursive => LogicalPlanBuilder::from(input)
                    .project([col("id")])?
                    .build()?,
            };
            match self.kind {
                PolicyPlanKind::Project | PolicyPlanKind::Volatile => Ok(projection),
                PolicyPlanKind::OrderedAscending => Ok(LogicalPlanBuilder::from(projection)
                    .sort([col("id").sort(true, false)])?
                    .build()?),
                PolicyPlanKind::Recursive => Ok(LogicalPlan::RecursiveQuery(
                    datafusion::logical_expr::RecursiveQuery::try_new(
                        "policy-recursive".to_owned(),
                        Arc::new(projection.clone()),
                        Arc::new(projection),
                        true,
                    )?,
                )),
            }
        }
    }

    fn table(name: &str) -> TableReference {
        TableReference::full("datafusion", "public", name)
    }

    fn candidate_state() -> SessionState {
        let context = SessionContext::new();
        context
            .catalog("datafusion")
            .expect("default catalog")
            .register_schema("system", Arc::new(MemorySchemaProvider::new()))
            .unwrap();
        context.state()
    }

    fn observation_epoch() -> EpochId {
        EpochId::from_bytes([0x5a; 16])
    }

    fn observation_policy(
        max_iterations: u32,
        max_rows_per_relation: u64,
        max_total_rows: u64,
        max_bytes_per_relation: u64,
        max_total_bytes: u64,
    ) -> ObservationFixedPointPolicy {
        ObservationFixedPointPolicy::try_new(
            max_iterations,
            max_rows_per_relation,
            max_total_rows,
            max_bytes_per_relation,
            max_total_bytes,
        )
        .expect("test observation policy is nonzero")
    }

    fn test_transformation_contract(
        id: &str,
        semantic_version: TransformationSemanticVersion,
    ) -> ProgrammaticTransformationContract {
        ProgrammaticTransformationContract::new(
            ProgrammaticTransformationId::new(id),
            semantic_version,
            TransformationResourceClass::BoundedInMemory {
                max_rows: 10_000,
                max_memory_bytes: 1 << 20,
            },
            TransformationDeterminismPolicy::DeterministicSet,
            TransformationOrderingPolicy::Unordered,
            TransformationRecursionPolicy::Forbidden,
            TransformationProvenance::new(
                TransformationProvenanceIdentity::from_bytes([0x31; 32]),
                TransformationReleaseIdentity::from_bytes([0x41; 32]),
            ),
        )
    }

    fn provider_input(
        relation_id: &str,
        table_reference: TableReference,
        with_note: bool,
    ) -> ProviderInput {
        let mut fields = vec![
            Field::new("id", DataType::Int64, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "provider.events.id".to_owned(),
            )])),
            Field::new("active", DataType::Boolean, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "provider.events.active".to_owned(),
            )])),
        ];
        let mut arrays: Vec<ArrayRef> = vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(BooleanArray::from(vec![true, false, true])),
        ];
        if with_note {
            fields.push(
                Field::new("note", DataType::Utf8, true).with_metadata(HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    "provider.events.note".to_owned(),
                )])),
            );
            arrays.push(Arc::new(StringArray::from(vec![
                Some("a"),
                None,
                Some("c"),
            ])));
        }
        let schema = Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
        ));
        let batch = RecordBatch::try_new(Arc::clone(&schema), arrays).unwrap();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let contract = Arc::new(
            SchemaContract::try_new(
                "provider-fixture",
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..schema.fields().len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .unwrap(),
        );
        ProviderInput::new(
            ProgrammaticRelationId::new(relation_id),
            table_reference,
            contract,
            provider,
        )
    }

    async fn fixture_with_contract(
        with_note: bool,
        minimum_id: i64,
        include_active: bool,
        assertion: Option<SchemaRef>,
        contract: ProgrammaticTransformationContract,
        observation_policy: ObservationFixedPointPolicy,
    ) -> Result<SealedProgrammaticSchemaAssembly, ProgrammaticSchemaError> {
        let input_id = ProgrammaticRelationId::new("provider.events");
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        let mut output_fields = vec![TransformationFieldIdentity::new(ProgrammaticFieldId::new(
            "derived.active_events.id",
        ))];
        if include_active {
            output_fields.push(TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                "derived.active_events.active",
            )));
        }
        let mut output =
            TransformationOutput::new(output_id, table("active_events"), output_fields);
        if let Some(assertion) = assertion {
            output = output.with_schema_assertion(assertion);
        }
        let mut assembly = ProgrammaticSchemaAssembly::with_observation_policy(
            candidate_state(),
            observation_policy,
        );
        assembly.register_provider(provider_input(
            input_id.as_str(),
            table("provider_events"),
            with_note,
        ))?;
        assembly.add_transformation(Arc::new(FilterProjection {
            contract,
            output,
            dependencies: vec![input_id],
            minimum_id,
            include_active,
        }))?;
        assembly.seal(observation_epoch()).await
    }

    async fn fixture(
        with_note: bool,
        minimum_id: i64,
        include_active: bool,
        assertion: Option<SchemaRef>,
    ) -> Result<SealedProgrammaticSchemaAssembly, ProgrammaticSchemaError> {
        fixture_with_contract(
            with_note,
            minimum_id,
            include_active,
            assertion,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            ObservationFixedPointPolicy::production(),
        )
        .await
    }

    async fn policy_fixture(
        contract: ProgrammaticTransformationContract,
        kind: PolicyPlanKind,
    ) -> Result<SealedProgrammaticSchemaAssembly, ProgrammaticSchemaError> {
        let input_id = ProgrammaticRelationId::new("provider.events");
        let output_id = ProgrammaticRelationId::new("derived.policy_output");
        let mut fields = vec![TransformationFieldIdentity::new(ProgrammaticFieldId::new(
            "derived.policy_output.id",
        ))];
        if matches!(kind, PolicyPlanKind::Volatile) {
            fields.push(TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                "derived.policy_output.nonce",
            )));
        }
        let mut assembly = ProgrammaticSchemaAssembly::new(candidate_state());
        assembly.register_provider(provider_input(
            input_id.as_str(),
            table("provider_events"),
            false,
        ))?;
        assembly.add_transformation(Arc::new(PolicyProjection {
            contract,
            output: TransformationOutput::new(output_id, table("policy_output"), fields),
            dependencies: vec![input_id],
            kind,
        }))?;
        assembly.seal(observation_epoch()).await
    }

    fn passthrough_registration_error(
        contract: ProgrammaticTransformationContract,
    ) -> ProgrammaticSchemaError {
        let mut assembly = ProgrammaticSchemaAssembly::new(candidate_state());
        assembly
            .add_transformation(Arc::new(Passthrough {
                contract,
                output: TransformationOutput::new(
                    ProgrammaticRelationId::new("derived.contract_validation"),
                    table("contract_validation"),
                    [TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        "derived.contract_validation.id",
                    ))],
                ),
                dependencies: vec![ProgrammaticRelationId::new("provider.events")],
            }))
            .unwrap_err()
    }

    async fn observed_contract_authority(
        sealed: &SealedProgrammaticSchemaAssembly,
        relation_id: &str,
    ) -> [u8; 32] {
        let binding = sealed
            .relation(&ProgrammaticRelationId::new(
                PROVENANCE_OBSERVATION_RELATION_ID,
            ))
            .expect("provenance relation is installed");
        let batches = sealed
            .session()
            .table(binding.table_reference.clone())
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        for batch in batches {
            let relation_ids = batch
                .column_by_name("relation_id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let identities = batch
                .column_by_name("contract_authority_identity")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            for row in 0..batch.num_rows() {
                if relation_ids.value(row) == relation_id {
                    return identities
                        .value(row)
                        .try_into()
                        .expect("contract authority has fixed width 32");
                }
            }
        }
        panic!("missing provenance row for {relation_id}")
    }

    #[tokio::test]
    async fn provider_filter_projection_derives_and_registers_its_schema() {
        let sealed = fixture(false, 2, false, None).await.unwrap();
        let fixed_point = sealed.observation_fixed_point();
        assert_eq!(fixed_point.iterations(), 2);
        assert_eq!(fixed_point.relation_count(), 5);
        assert!(fixed_point.total_rows() > 0);
        assert!(fixed_point.total_bytes() > 0);
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        let binding = sealed.relation(&output_id).unwrap();
        assert_eq!(binding.contract.logical_schema().fields().len(), 1);
        assert_eq!(binding.contract.logical_schema().field(0).name(), "id");
        assert_eq!(
            binding.contract.logical_schema().field(0).data_type(),
            &DataType::Int64
        );
        assert_eq!(
            binding.contract.relation_id(SchemaRole::Logical).unwrap(),
            output_id.as_str()
        );
        assert_eq!(
            binding
                .contract
                .field_id_at(SchemaRole::Logical, 0)
                .unwrap(),
            "derived.active_events.id"
        );
        let relation = sealed
            .observations()
            .relations
            .iter()
            .find(|observation| observation.relation_id == output_id)
            .unwrap();
        assert_eq!(relation.origin, RelationOrigin::Transformation);
        assert_eq!(relation.table_type, TableType::View);
        let provider = sealed
            .session()
            .table_provider(binding.table_reference.clone())
            .await
            .unwrap();
        assert!(provider.get_table_definition().is_none());
        let batches = sealed
            .session()
            .table(binding.table_reference.clone())
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);

        let relation_observation_id =
            ProgrammaticRelationId::new("system.programmatic_relation_observation");
        let observation_binding = sealed.relation(&relation_observation_id).unwrap();
        assert_eq!(
            observation_binding
                .contract
                .relation_id(SchemaRole::Logical)
                .unwrap(),
            relation_observation_id.as_str()
        );
        let queried_observations = sealed
            .session()
            .table(observation_binding.table_reference.clone())
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(
            queried_observations
                .iter()
                .map(RecordBatch::num_rows)
                .sum::<usize>(),
            7
        );
        assert!(queried_observations.iter().all(|batch| {
            batch.schema_ref().as_ref() == observation_binding.contract.logical_schema().as_ref()
        }));
    }

    #[tokio::test]
    async fn native_view_control_loses_relation_metadata_and_identity_boundary_restores_it() {
        let sealed = fixture(false, 2, false, None).await.unwrap();
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        let binding = sealed.relation(&output_id).unwrap();
        let plan = binding
            .logical_plan
            .as_ref()
            .expect("transformation view retains its application-owned logical plan")
            .as_ref()
            .clone();
        let target = Arc::clone(binding.contract.logical_schema());
        let state = sealed.session().state();

        let native = ViewTable::new(plan.clone(), None)
            .scan(&state, None, &[], None)
            .await
            .unwrap();
        assert_ne!(
            native.schema().as_ref(),
            target.as_ref(),
            "the pinned DataFusion 55 native control must keep proving the metadata-loss fault"
        );
        assert_eq!(native.schema().fields().len(), target.fields().len());
        assert!(
            native
                .schema()
                .fields()
                .iter()
                .zip(target.fields())
                .all(|(actual, expected)| actual.name() == expected.name()
                    && actual.data_type() == expected.data_type()
                    && actual.is_nullable() == expected.is_nullable())
        );

        let preserving = IdentityPreservingViewTable::new(plan)
            .scan(&state, None, &[], None)
            .await
            .unwrap();
        assert_eq!(preserving.schema().as_ref(), target.as_ref());
        let batches = datafusion::physical_plan::collect(preserving, state.task_ctx())
            .await
            .unwrap();
        assert!(
            batches
                .iter()
                .all(|batch| batch.schema_ref().as_ref() == target.as_ref())
        );
    }

    #[test]
    fn observation_fixed_point_policy_rejects_every_zero_bound() {
        for (expected, values) in [
            ("max_iterations", (0, 1, 1, 1, 1)),
            ("max_rows_per_relation", (1, 0, 1, 1, 1)),
            ("max_total_rows", (1, 1, 0, 1, 1)),
            ("max_bytes_per_relation", (1, 1, 1, 0, 1)),
            ("max_total_bytes", (1, 1, 1, 1, 0)),
        ] {
            let (iterations, per_rows, total_rows, per_bytes, total_bytes) = values;
            assert_eq!(
                ObservationFixedPointPolicy::try_new(
                    iterations,
                    per_rows,
                    total_rows,
                    per_bytes,
                    total_bytes,
                ),
                Err(ObservationFixedPointPolicyError::ZeroBound { field: expected })
            );
        }
    }

    #[tokio::test]
    async fn observation_fixed_point_iteration_bound_fails_closed() {
        let error = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            observation_policy(1, 10_000, 50_000, 1 << 20, 5 << 20),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::ObservationFixedPointIterationsExceeded { limit: 1 }
        ));
    }

    #[tokio::test]
    async fn observation_relation_row_bound_fails_closed_before_sealing() {
        let error = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            observation_policy(8, 1, 50_000, 1 << 20, 5 << 20),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::ObservationRelationRowsExceeded {
                limit: 1,
                observed,
                ..
            } if observed > 1
        ));
    }

    #[tokio::test]
    async fn observation_relation_memory_bound_fails_closed_before_sealing() {
        let error = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            observation_policy(8, 10_000, 50_000, 1, 5 << 20),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::ObservationRelationBytesExceeded {
                limit: 1,
                observed,
                ..
            } if observed > 1
        ));
    }

    #[tokio::test]
    async fn observation_total_row_bound_fails_closed_before_sealing() {
        let error = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            observation_policy(8, 10_000, 1, 1 << 20, 5 << 20),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::ObservationTotalRowsExceeded {
                limit: 1,
                observed,
            } if observed > 1
        ));
    }

    #[tokio::test]
    async fn observation_total_memory_bound_fails_closed_before_sealing() {
        let error = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            observation_policy(8, 10_000, 50_000, 1 << 20, 1),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::ObservationTotalBytesExceeded {
                limit: 1,
                observed,
            } if observed > 1
        ));
    }

    #[tokio::test]
    async fn transformation_contract_metadata_is_typed_and_queryable() {
        let sealed = fixture(false, 2, false, None).await.unwrap();
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        let observed = sealed.observations().provenance(&output_id).unwrap();
        let contract = observed.transformation_contract().unwrap();
        assert_eq!(
            contract.semantic_id(),
            &ProgrammaticTransformationId::new("filter-active-events")
        );
        assert_eq!(
            contract.semantic_version(),
            TransformationSemanticVersion::new(1, 0, 0)
        );
        assert_eq!(
            contract.resource_class(),
            TransformationResourceClass::BoundedInMemory {
                max_rows: 10_000,
                max_memory_bytes: 1 << 20,
            }
        );
        assert_eq!(
            contract.determinism_policy(),
            TransformationDeterminismPolicy::DeterministicSet
        );
        assert_eq!(
            contract.ordering_policy(),
            &TransformationOrderingPolicy::Unordered
        );
        assert_eq!(
            contract.recursion_policy(),
            TransformationRecursionPolicy::Forbidden
        );
        assert_eq!(
            observed_contract_authority(&sealed, output_id.as_str()).await,
            contract.authority_identity()
        );
    }

    #[test]
    fn transformation_registration_rejects_sentinels_and_incompatible_policies() {
        let mut contract = test_transformation_contract(
            "invalid-transform",
            TransformationSemanticVersion::new(0, 0, 0),
        );
        assert!(matches!(
            passthrough_registration_error(contract.clone()),
            ProgrammaticSchemaError::SentinelTransformationSemanticVersion { .. }
        ));

        contract.semantic_version = TransformationSemanticVersion::new(1, 0, 0);
        contract.provenance = TransformationProvenance::new(
            TransformationProvenanceIdentity::from_bytes([0; 32]),
            TransformationReleaseIdentity::from_bytes([0x41; 32]),
        );
        assert!(matches!(
            passthrough_registration_error(contract.clone()),
            ProgrammaticSchemaError::SentinelTransformationProvenanceIdentity { .. }
        ));

        contract.provenance = TransformationProvenance::new(
            TransformationProvenanceIdentity::from_bytes([0x31; 32]),
            TransformationReleaseIdentity::from_bytes([0; 32]),
        );
        assert!(matches!(
            passthrough_registration_error(contract.clone()),
            ProgrammaticSchemaError::SentinelTransformationReleaseIdentity { .. }
        ));

        contract.provenance = TransformationProvenance::new(
            TransformationProvenanceIdentity::from_bytes([0x31; 32]),
            TransformationReleaseIdentity::from_bytes([0x41; 32]),
        );
        contract.determinism_policy = TransformationDeterminismPolicy::DeterministicSequence;
        contract.ordering_policy = TransformationOrderingPolicy::Unordered;
        assert!(matches!(
            passthrough_registration_error(contract),
            ProgrammaticSchemaError::IncompatibleTransformationPolicies { .. }
        ));
    }

    #[tokio::test]
    async fn metadata_change_causally_changes_observed_transformation_authority() {
        let first = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 0),
            ),
            ObservationFixedPointPolicy::production(),
        )
        .await
        .unwrap();
        let second = fixture_with_contract(
            false,
            2,
            false,
            None,
            test_transformation_contract(
                "filter-active-events",
                TransformationSemanticVersion::new(1, 0, 1),
            ),
            ObservationFixedPointPolicy::production(),
        )
        .await
        .unwrap();
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        assert_eq!(
            first
                .observations()
                .provenance(&output_id)
                .unwrap()
                .logical_plan(),
            second
                .observations()
                .provenance(&output_id)
                .unwrap()
                .logical_plan()
        );
        assert_ne!(
            observed_contract_authority(&first, output_id.as_str()).await,
            observed_contract_authority(&second, output_id.as_str()).await
        );
    }

    #[tokio::test]
    async fn physical_output_ordering_must_satisfy_the_declared_field_identity_keys() {
        let mut contract = test_transformation_contract(
            "ordered-policy",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        contract.determinism_policy = TransformationDeterminismPolicy::DeterministicSequence;
        contract.ordering_policy = TransformationOrderingPolicy::ByOutputFields(Arc::from([
            TransformationOrderingKey::new(
                ProgrammaticFieldId::new("derived.policy_output.id"),
                TransformationSortDirection::Ascending,
                TransformationNullPlacement::Last,
            ),
        ]));
        policy_fixture(contract.clone(), PolicyPlanKind::OrderedAscending)
            .await
            .expect("a global physical sort satisfies the sequence contract");

        contract.ordering_policy = TransformationOrderingPolicy::ByOutputFields(Arc::from([
            TransformationOrderingKey::new(
                ProgrammaticFieldId::new("derived.policy_output.id"),
                TransformationSortDirection::Descending,
                TransformationNullPlacement::Last,
            ),
        ]));
        assert!(matches!(
            policy_fixture(contract, PolicyPlanKind::OrderedAscending)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::TransformationOrderingNotSatisfied { .. }
        ));
    }

    #[tokio::test]
    async fn determinism_policy_rejects_nonimmutable_and_inert_volatility() {
        let deterministic = test_transformation_contract(
            "deterministic-policy",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        assert!(matches!(
            policy_fixture(deterministic, PolicyPlanKind::Volatile)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::NonImmutableTransformationExpression { .. }
        ));

        let mut volatile = test_transformation_contract(
            "volatile-policy",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        volatile.determinism_policy = TransformationDeterminismPolicy::Volatile;
        policy_fixture(volatile.clone(), PolicyPlanKind::Volatile)
            .await
            .expect("an explicitly volatile plan is proof-executed once");
        assert!(matches!(
            policy_fixture(volatile, PolicyPlanKind::Project)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::VolatileTransformationDeclarationInert { .. }
        ));
    }

    #[tokio::test]
    async fn proof_execution_enforces_row_and_memory_resource_bounds() {
        let mut row_bounded = test_transformation_contract(
            "row-bounded-policy",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        row_bounded.resource_class = TransformationResourceClass::BoundedInMemory {
            max_rows: 2,
            max_memory_bytes: 1 << 20,
        };
        assert!(matches!(
            policy_fixture(row_bounded, PolicyPlanKind::Project)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::TransformationOutputRowsExceeded {
                limit: 2,
                observed: 3,
                ..
            }
        ));

        let mut memory_bounded = test_transformation_contract(
            "memory-bounded-policy",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        memory_bounded.resource_class = TransformationResourceClass::BoundedInMemory {
            max_rows: 10,
            max_memory_bytes: 1,
        };
        assert!(matches!(
            policy_fixture(memory_bounded, PolicyPlanKind::Project)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::TransformationMemoryBytesExceeded { limit: 1, .. }
        ));
    }

    #[tokio::test]
    async fn recursion_policy_fails_closed_without_a_native_iteration_cap() {
        let forbidden = test_transformation_contract(
            "forbidden-recursion",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        assert!(matches!(
            policy_fixture(forbidden, PolicyPlanKind::Recursive)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::TransformationRecursionForbidden { .. }
        ));

        let mut bounded = test_transformation_contract(
            "bounded-recursion",
            TransformationSemanticVersion::new(1, 0, 0),
        );
        bounded.recursion_policy = TransformationRecursionPolicy::Bounded { max_iterations: 8 };
        assert!(matches!(
            policy_fixture(bounded.clone(), PolicyPlanKind::Project)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::TransformationRecursionDeclarationInert { .. }
        ));
        assert!(matches!(
            policy_fixture(bounded, PolicyPlanKind::Recursive)
                .await
                .unwrap_err(),
            ProgrammaticSchemaError::BoundedNativeRecursionUnavailable {
                max_iterations: 8,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn input_and_plan_changes_causally_change_typed_observations() {
        let first = fixture(false, 1, false, None).await.unwrap();
        let second = fixture(true, 2, true, None).await.unwrap();
        let input_id = ProgrammaticRelationId::new("provider.events");
        let output_id = ProgrammaticRelationId::new("derived.active_events");
        assert_ne!(
            first.observations().schema(&input_id).unwrap().arrow_schema,
            second
                .observations()
                .schema(&input_id)
                .unwrap()
                .arrow_schema
        );
        assert_ne!(
            first
                .observations()
                .schema(&output_id)
                .unwrap()
                .arrow_schema,
            second
                .observations()
                .schema(&output_id)
                .unwrap()
                .arrow_schema
        );
        assert_ne!(
            first
                .observations()
                .provenance(&output_id)
                .unwrap()
                .logical_plan(),
            second
                .observations()
                .provenance(&output_id)
                .unwrap()
                .logical_plan()
        );
    }

    #[tokio::test]
    async fn output_schema_declaration_is_assertion_only_and_mismatch_fails() {
        let asserted = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
        let error = fixture(false, 1, false, Some(asserted)).await.unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::OutputSchemaAssertionMismatch { .. }
        ));
    }

    #[tokio::test]
    async fn unresolved_and_cyclic_transformations_fail_before_plan_building() {
        let missing = ProgrammaticRelationId::new("missing.input");
        let mut unresolved = ProgrammaticSchemaAssembly::new(candidate_state());
        unresolved
            .add_transformation(Arc::new(Passthrough {
                contract: test_transformation_contract(
                    "unresolved",
                    TransformationSemanticVersion::new(1, 0, 0),
                ),
                output: TransformationOutput::new(
                    ProgrammaticRelationId::new("unresolved.output"),
                    table("unresolved_output"),
                    [TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        "unresolved.output.value",
                    ))],
                ),
                dependencies: vec![missing],
            }))
            .unwrap();
        assert!(matches!(
            unresolved.seal(observation_epoch()).await.unwrap_err(),
            ProgrammaticSchemaError::UnresolvedDependency { .. }
        ));

        let left = ProgrammaticRelationId::new("cycle.left");
        let right = ProgrammaticRelationId::new("cycle.right");
        let mut cyclic = ProgrammaticSchemaAssembly::new(candidate_state());
        cyclic
            .add_transformation(Arc::new(Passthrough {
                contract: test_transformation_contract(
                    "left",
                    TransformationSemanticVersion::new(1, 0, 0),
                ),
                output: TransformationOutput::new(
                    left.clone(),
                    table("cycle_left"),
                    [TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        "cycle.left.value",
                    ))],
                ),
                dependencies: vec![right.clone()],
            }))
            .unwrap();
        cyclic
            .add_transformation(Arc::new(Passthrough {
                contract: test_transformation_contract(
                    "right",
                    TransformationSemanticVersion::new(1, 0, 0),
                ),
                output: TransformationOutput::new(
                    right,
                    table("cycle_right"),
                    [TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        "cycle.right.value",
                    ))],
                ),
                dependencies: vec![left],
            }))
            .unwrap();
        assert!(matches!(
            cyclic.seal(observation_epoch()).await.unwrap_err(),
            ProgrammaticSchemaError::CyclicTransformations { .. }
        ));
    }

    #[test]
    fn duplicate_relation_and_table_bindings_are_rejected_before_registration() {
        let mut assembly = ProgrammaticSchemaAssembly::new(candidate_state());
        assembly
            .register_provider(provider_input(
                "provider.events",
                table("provider_events"),
                false,
            ))
            .unwrap();
        let duplicate_relation = assembly
            .register_provider(provider_input(
                "provider.events",
                table("provider_events_two"),
                false,
            ))
            .unwrap_err();
        assert!(matches!(
            duplicate_relation,
            ProgrammaticSchemaError::DuplicateRelation { .. }
        ));

        let duplicate_table = assembly
            .register_provider(provider_input(
                "provider.events.two",
                table("provider_events"),
                false,
            ))
            .unwrap_err();
        assert!(matches!(
            duplicate_table,
            ProgrammaticSchemaError::DuplicateTableReference { .. }
        ));
    }

    #[test]
    fn provider_identity_metadata_is_mandatory() {
        let table_reference = table("unidentified_provider");
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![1]))],
        )
        .unwrap();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let contract = Arc::new(
            SchemaContract::try_new(
                "unidentified-provider",
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );
        let mut assembly = ProgrammaticSchemaAssembly::new(candidate_state());
        let error = assembly
            .register_provider(ProviderInput::new(
                ProgrammaticRelationId::new("provider.unidentified"),
                table_reference,
                contract,
                provider,
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            ProgrammaticSchemaError::SchemaContract(
                SchemaContractError::IdentityMetadataUnavailable { .. }
            )
        ));
    }
}
