//! Programmatic assembly of one candidate DataFusion catalog.
//!
//! Provider contracts and native logical transformations are installed into one
//! candidate [`SessionContext`]. Output schemas are observed from the analyzed plan;
//! an optional caller schema is only an equality assertion and never supplies a field
//! type, nullability, or metadata to the registered view.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow_schema::{ArrowError, DataType, Field, FieldRef, Schema, SchemaRef};
use datafusion::catalog::TableProvider;
use datafusion::common::tree_node::TreeNodeRecursion;
use datafusion::common::{Column, DFSchema, DFSchemaRef, DataFusionError, TableReference};
#[cfg(test)]
use datafusion::datasource::MemTable;
use datafusion::datasource::{ViewTable, provider_as_source};
use datafusion::execution::context::{SessionContext, SessionState};
use datafusion::logical_expr::logical_plan::Projection;
use datafusion::logical_expr::{Expr, LogicalPlan, LogicalPlanBuilder, TableType};
use thiserror::Error;

use super::command::EpochId;
use super::id16_array;
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
    fn id(&self) -> &ProgrammaticTransformationId;
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
            | Self::Transformation { relation_id, .. } => relation_id,
        }
    }

    /// Return a native plan only for programmatic transformation provenance.
    #[must_use]
    pub fn logical_plan(&self) -> Option<&LogicalPlan> {
        match self {
            Self::Provider { .. } | Self::SystemObservation { .. } => None,
            Self::Transformation { logical_plan, .. } => Some(logical_plan),
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
}

/// Sealed candidate retaining the exact session/catalog authority used for planning.
pub struct SealedProgrammaticSchemaAssembly {
    session: SessionContext,
    relations: BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
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

/// Ownership-transfer product for later `FabricEpochBuilder` integration.
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
    Provider,
    #[cfg(test)]
    SystemObservation,
    Transformation {
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

/// Mutable builder for one dependency-closed candidate catalog.
pub struct ProgrammaticSchemaAssembly {
    session: SessionContext,
    registered: BTreeMap<ProgrammaticRelationId, RegisteredRelation>,
    pending: BTreeMap<ProgrammaticRelationId, Arc<dyn ProgrammaticTransformation>>,
    transformations: BTreeMap<ProgrammaticTransformationId, ProgrammaticRelationId>,
    table_references: BTreeMap<TableReference, ProgrammaticRelationId>,
}

impl ProgrammaticSchemaAssembly {
    /// Start from the exact candidate `SessionState` later transferred to the epoch.
    #[must_use]
    pub fn new(candidate_state: SessionState) -> Self {
        Self {
            session: SessionContext::new_with_state(candidate_state),
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
    pub fn register_provider(
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

        self.session
            .register_table(input.table_reference.clone(), input.provider)?;
        self.table_references
            .insert(input.table_reference.clone(), input.relation_id.clone());
        self.registered.insert(
            input.relation_id,
            RegisteredRelation {
                table_reference: input.table_reference,
                contract: input.contract,
                origin: RegisteredOrigin::Provider,
            },
        );
        Ok(())
    }

    /// Add one programmatic transformation. It is built during [`Self::seal`].
    pub fn add_transformation(
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
        let provider = Arc::new(ViewTable::new(plan.clone(), None));
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
                origin: RegisteredOrigin::Transformation {
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
        let RegisteredOrigin::Transformation {
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
        let origin = RegisteredOrigin::Transformation {
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
            Arc::new(ViewTable::new(plan, None)) as Arc<dyn TableProvider>,
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
        let (relations, observations) = self.observe_live_catalog().await?;
        let final_observation_batches = build_observation_batches(
            epoch_id,
            &observations,
            &self.observation_relation_specs()?,
        )?;
        if installed_observation_batches != final_observation_batches {
            return Err(ProgrammaticSchemaError::ObservationSelfInclusionDrift);
        }
        Ok(SealedProgrammaticSchemaAssembly {
            session: self.session,
            relations,
            #[cfg(test)]
            observations,
        })
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
        let view = Arc::new(ViewTable::new(installed_plan.clone(), None));
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
                    transformation_id: transformation.id().clone(),
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

            let origin = match &registered.origin {
                RegisteredOrigin::Provider => RelationOrigin::Provider,
                #[cfg(test)]
                RegisteredOrigin::SystemObservation => RelationOrigin::SystemObservation,
                RegisteredOrigin::Transformation { plan, .. } => {
                    let observed_plan = provider.get_logical_plan().ok_or_else(|| {
                        ProgrammaticSchemaError::ViewPlanUnavailable {
                            relation_id: relation_id.clone(),
                        }
                    })?;
                    if observed_plan.as_ref() != plan.as_ref() {
                        return Err(ProgrammaticSchemaError::ViewPlanDrift {
                            relation_id: relation_id.clone(),
                        });
                    }
                    if provider.get_table_definition().is_some() {
                        return Err(ProgrammaticSchemaError::SqlViewDefinition {
                            relation_id: relation_id.clone(),
                        });
                    }
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
                RegisteredOrigin::Provider => {
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
                    transformation_id,
                    dependencies,
                    ..
                } => {
                    let observed_plan = provider
                        .get_logical_plan()
                        .expect("the transformation view plan was checked above")
                        .into_owned();
                    observations
                        .provenance
                        .push(ProvenanceObservation::Transformation {
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
        expressions
            .push(Expr::Column(Column::from((qualifier, field))).alias(field.name().to_owned()));
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
            Expr::Column(Column::from((qualifier, field))).alias(expected.name().to_owned())
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
                    .enumerate()
                    .map(|(ordinal, field)| {
                        if ordinal == 0 {
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
                    ProvenanceObservation::Transformation { .. } => None,
                },
            ))),
            Arc::new(StringArray::from_iter(provenance.iter().map(
                |row| match row {
                    ProvenanceObservation::Transformation {
                        transformation_id, ..
                    } => Some(transformation_id.as_str()),
                    ProvenanceObservation::Provider { .. }
                    | ProvenanceObservation::SystemObservation { .. } => None,
                },
            ))),
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
    #[error("system observation batches changed after self-inclusive catalog registration")]
    ObservationSelfInclusionDrift,
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

    use arrow_array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::catalog::MemorySchemaProvider;
    use datafusion::datasource::MemTable;
    use datafusion::logical_expr::{LogicalPlanBuilder, lit};
    use datafusion::prelude::col;

    use super::*;

    #[derive(Debug)]
    struct FilterProjection {
        id: ProgrammaticTransformationId,
        output: TransformationOutput,
        dependencies: Vec<ProgrammaticRelationId>,
        minimum_id: i64,
        include_active: bool,
    }

    impl ProgrammaticTransformation for FilterProjection {
        fn id(&self) -> &ProgrammaticTransformationId {
            &self.id
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
        id: ProgrammaticTransformationId,
        output: TransformationOutput,
        dependencies: Vec<ProgrammaticRelationId>,
    }

    impl ProgrammaticTransformation for Passthrough {
        fn id(&self) -> &ProgrammaticTransformationId {
            &self.id
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

    async fn fixture(
        with_note: bool,
        minimum_id: i64,
        include_active: bool,
        assertion: Option<SchemaRef>,
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
        let mut assembly = ProgrammaticSchemaAssembly::new(candidate_state());
        assembly.register_provider(provider_input(
            input_id.as_str(),
            table("provider_events"),
            with_note,
        ))?;
        assembly.add_transformation(Arc::new(FilterProjection {
            id: ProgrammaticTransformationId::new("filter-active-events"),
            output,
            dependencies: vec![input_id],
            minimum_id,
            include_active,
        }))?;
        assembly.seal(observation_epoch()).await
    }

    #[tokio::test]
    async fn provider_filter_projection_derives_and_registers_its_schema() {
        let sealed = fixture(false, 2, false, None).await.unwrap();
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
                id: ProgrammaticTransformationId::new("unresolved"),
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
                id: ProgrammaticTransformationId::new("left"),
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
                id: ProgrammaticTransformationId::new("right"),
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
                SchemaContractError::ModelMetadataUnavailable { .. }
            )
        ));
    }
}
