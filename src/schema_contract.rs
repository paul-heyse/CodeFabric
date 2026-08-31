//! Executable logical/physical Arrow and DataFusion schema contracts.
//!
//! A [`SchemaContract`] is constructed from the exact provider or transformation
//! schema installed in one candidate DataFusion session. It keeps that logical
//! schema, its qualified DataFusion form, the bound storage schema, and every
//! index translation together so that a provider or sink cannot reinterpret
//! columns independently.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrow_array::{Array, ArrayRef, BooleanArray, RecordBatch, StringArray, UInt32Array};
use arrow_cast::cast::{CastOptions, can_cast_types, cast_with_options};
use arrow_schema::extension::{EXTENSION_TYPE_METADATA_KEY, EXTENSION_TYPE_NAME_KEY};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use datafusion::common::{
    Constraint, Constraints, DFSchema, DataFusionError, FunctionalDependencies, TableReference,
};
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::{ExecutionPlan, RecordBatchStream, SendableRecordBatchStream};
use futures::Stream;

use crate::relational_model::{ModelEpoch, ModelRelation};

/// Arrow schema metadata key carrying the session-owned relation identity.
pub const RELATION_ID_METADATA_KEY: &str = "codefabric.relation_id";
/// Arrow field metadata key carrying the session-owned field identity.
pub const FIELD_ID_METADATA_KEY: &str = "codefabric.field_id";
/// Arrow field metadata key carrying the field's semantic role.
pub const SEMANTIC_ROLE_METADATA_KEY: &str = "codefabric.semantic_role";

/// Compatibility name retained while replay-model consumers are removed.
pub const MODEL_RELATION_ID_METADATA_KEY: &str = RELATION_ID_METADATA_KEY;
/// Compatibility name retained while replay-model consumers are removed.
pub const MODEL_FIELD_ID_METADATA_KEY: &str = FIELD_ID_METADATA_KEY;
/// Compatibility name retained while replay-model consumers are removed.
pub const MODEL_SEMANTIC_ROLE_METADATA_KEY: &str = SEMANTIC_ROLE_METADATA_KEY;

/// A named boundary at which a schema contract is enforced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaPhase {
    ProviderIngress,
    AnalyzedLogicalPlan,
    OptimizedLogicalPlan,
    InitialPhysicalPlan,
    OptimizedPhysicalPlan,
    StreamConstruction,
    RecordBatch,
    WriteSink,
}

impl SchemaPhase {
    /// Return the schema role emitted by this phase after required restoration.
    #[must_use]
    pub const fn output_role(self) -> SchemaRole {
        match self {
            Self::WriteSink => SchemaRole::Storage,
            Self::ProviderIngress
            | Self::AnalyzedLogicalPlan
            | Self::OptimizedLogicalPlan
            | Self::InitialPhysicalPlan
            | Self::OptimizedPhysicalPlan
            | Self::StreamConstruction
            | Self::RecordBatch => SchemaRole::Logical,
        }
    }
}

/// Which model-owned schema is expected at a boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaRole {
    Logical,
    Storage,
}

/// Compatibility requested at one boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaCompatibility {
    /// Field order, types, nullability, dictionary ordering, and all metadata
    /// must be equal.
    Exact,
    /// The actual schema must contain the expected schema according to Arrow's
    /// native recursive [`Schema::contains`] semantics. Extension identity and
    /// extension metadata remain exact because they carry application meaning.
    Contains,
}

/// The consumer for which a logical field index is translated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexPurpose {
    Storage,
    Projection,
    Filter,
    Statistics,
}

/// All physical index bindings for one logical field.
///
/// The mappings are deliberately explicit. A storage adapter must not assume
/// that projection, filter, and statistics indices happen to equal the durable
/// column index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldIndexMapping {
    logical: usize,
    storage: usize,
    projection: usize,
    filter: usize,
    statistics: usize,
}

impl FieldIndexMapping {
    /// Construct the complete mapping for one logical field.
    #[must_use]
    pub const fn new(
        logical_index: usize,
        storage_index: usize,
        projection_index: usize,
        filter_index: usize,
        statistics_index: usize,
    ) -> Self {
        Self {
            logical: logical_index,
            storage: storage_index,
            projection: projection_index,
            filter: filter_index,
            statistics: statistics_index,
        }
    }

    /// Construct a mapping whose consumers all use the storage index.
    #[must_use]
    pub const fn direct(logical_index: usize, storage_index: usize) -> Self {
        Self::new(
            logical_index,
            storage_index,
            storage_index,
            storage_index,
            storage_index,
        )
    }

    #[must_use]
    pub const fn logical_index(self) -> usize {
        self.logical
    }

    #[must_use]
    pub const fn storage_index(self) -> usize {
        self.storage
    }

    #[must_use]
    pub const fn projection_index(self) -> usize {
        self.projection
    }

    #[must_use]
    pub const fn filter_index(self) -> usize {
        self.filter
    }

    #[must_use]
    pub const fn statistics_index(self) -> usize {
        self.statistics
    }

    const fn index_for(self, purpose: IndexPurpose) -> usize {
        match purpose {
            IndexPurpose::Storage => self.storage,
            IndexPurpose::Projection => self.projection,
            IndexPurpose::Filter => self.filter,
            IndexPurpose::Statistics => self.statistics,
        }
    }
}

/// How a durable source identifies physical columns across schema evolution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColumnMappingMode {
    /// Physical position is the only available identity.
    #[default]
    Positional,
    /// Stable field names carry storage identity.
    Name,
    /// Stable model field IDs carry storage identity.
    FieldId,
}

/// Which component owns deletion-vector application for a physical binding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DeletionVectorBehavior {
    /// The bound source cannot contain deletion vectors.
    #[default]
    Forbidden,
    /// The storage provider applies deletion vectors before emitting batches.
    AppliedByProvider,
    /// The storage provider exposes the model-declared visibility column.
    ExposedVisibilityColumn,
}

/// A model relation row used by the schema compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSchemaRelationRow {
    pub relation_id: String,
    pub qualifier: TableReference,
    pub relation_metadata: HashMap<String, String>,
}

/// A model semantic-type row used by the schema compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSchemaTypeRow {
    pub semantic_type_id: String,
    pub logical_data_type: DataType,
    pub allows_null: bool,
}

/// A model field row used by the schema compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSchemaFieldRow {
    pub field_id: String,
    pub relation_id: String,
    pub field_name: String,
    pub semantic_type_id: String,
    pub ordinal: usize,
    pub nullable: bool,
    pub semantic_role: String,
    pub field_metadata: HashMap<String, String>,
}

/// The relational meaning of one model key row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelSchemaKeyKind {
    Primary,
    Unique,
}

/// One ordered member of a model key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSchemaKeyRow {
    pub key_id: String,
    pub relation_id: String,
    pub field_id: String,
    pub ordinal: usize,
    pub key_kind: ModelSchemaKeyKind,
}

/// Model-owned logical-to-storage representation for a semantic type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSchemaRepresentationRow {
    pub representation_id: String,
    pub semantic_type_id: String,
    pub storage_data_type: DataType,
    pub storage_encoding: String,
    pub metadata_class: String,
    pub extension_name: Option<String>,
    pub extension_metadata: Option<String>,
}

/// Exact physical field binding selected by the model mapping program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPhysicalFieldBindingRow {
    pub logical_field_id: String,
    pub storage_field_id: String,
    pub projection_index: usize,
    pub filter_index: usize,
    pub statistics_index: usize,
}

/// Exact physical relation binding selected by the model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPhysicalBindingRow {
    pub physical_binding_id: String,
    pub mapping_program_id: String,
    pub source_schema_identity: String,
    pub logical_relation_id: String,
    pub storage_relation_id: String,
    pub compatibility: SchemaCompatibility,
    pub column_mapping_mode: ColumnMappingMode,
    pub deletion_vector_behavior: DeletionVectorBehavior,
    pub field_bindings: Vec<ModelPhysicalFieldBindingRow>,
}

/// Typed relation rows consumed by the model-derived schema compiler.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemaContractModelRows {
    pub relations: Vec<ModelSchemaRelationRow>,
    pub semantic_types: Vec<ModelSchemaTypeRow>,
    pub fields: Vec<ModelSchemaFieldRow>,
    pub keys: Vec<ModelSchemaKeyRow>,
    pub representations: Vec<ModelSchemaRepresentationRow>,
    pub physical_bindings: Vec<ModelPhysicalBindingRow>,
}

/// One compiled logical/storage cast, retaining both model field identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldCastBinding {
    logical_field_id: Arc<str>,
    storage_field_id: Arc<str>,
    logical_index: usize,
    storage_index: usize,
    logical_data_type: DataType,
    storage_data_type: DataType,
}

impl FieldCastBinding {
    #[must_use]
    pub fn logical_field_id(&self) -> &str {
        &self.logical_field_id
    }

    #[must_use]
    pub fn storage_field_id(&self) -> &str {
        &self.storage_field_id
    }

    #[must_use]
    pub const fn logical_index(&self) -> usize {
        self.logical_index
    }

    #[must_use]
    pub const fn storage_index(&self) -> usize {
        self.storage_index
    }

    #[must_use]
    pub const fn logical_data_type(&self) -> &DataType {
        &self.logical_data_type
    }

    #[must_use]
    pub const fn storage_data_type(&self) -> &DataType {
        &self.storage_data_type
    }
}

/// A precise reason that two Arrow schemas are incompatible.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SchemaDifferenceKind {
    #[error("field count differs: expected {expected}, actual {actual}")]
    FieldCount { expected: usize, actual: usize },
    #[error("field name differs: expected {expected:?}, actual {actual:?}")]
    FieldName { expected: String, actual: String },
    #[error("data type differs: expected {expected}, actual {actual}")]
    DataType {
        expected: DataType,
        actual: DataType,
    },
    #[error("nullability differs: expected {expected}, actual {actual}")]
    Nullability { expected: bool, actual: bool },
    #[error("dictionary ordering differs: expected {expected:?}, actual {actual:?}")]
    DictionaryOrdering {
        expected: Option<bool>,
        actual: Option<bool>,
    },
    #[error("dictionary key type differs: expected {expected}, actual {actual}")]
    DictionaryKeyType {
        expected: DataType,
        actual: DataType,
    },
    #[error("dictionary value type differs: expected {expected}, actual {actual}")]
    DictionaryValueType {
        expected: DataType,
        actual: DataType,
    },
    #[error("schema metadata {key:?} differs: expected {expected:?}, actual {actual:?}")]
    SchemaMetadata {
        key: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    #[error("field metadata {key:?} differs: expected {expected:?}, actual {actual:?}")]
    FieldMetadata {
        key: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    #[error("extension metadata {key:?} differs: expected {expected:?}, actual {actual:?}")]
    ExtensionMetadata {
        key: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    #[error("Arrow's native compatibility predicate rejected the schemas")]
    NativeCompatibility,
}

/// One located schema incompatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaDifference {
    path: String,
    kind: SchemaDifferenceKind,
}

impl SchemaDifference {
    fn new(path: impl Into<String>, kind: SchemaDifferenceKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> &SchemaDifferenceKind {
        &self.kind
    }
}

impl std::fmt::Display for SchemaDifference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.kind)
    }
}

/// Typed failures produced while constructing or enforcing a contract.
#[derive(Debug, thiserror::Error)]
pub enum SchemaContractError {
    #[error("source schema identity must not be empty")]
    EmptySourceSchemaIdentity,
    #[error("failed to construct the qualified DataFusion schema: {source}")]
    QualifiedSchema {
        #[source]
        source: DataFusionError,
    },
    #[error("{role:?} schema field {field_index} is missing model metadata {missing_key:?}")]
    IncompleteModelFieldMetadata {
        role: SchemaRole,
        field_index: usize,
        missing_key: &'static str,
    },
    #[error("{role:?} schema has an empty model metadata value for {key:?} at {path}")]
    EmptyModelMetadata {
        role: SchemaRole,
        key: &'static str,
        path: String,
    },
    #[error(
        "{role:?} schema model field ID {field_id:?} is duplicated at indices {first_index} and {second_index}"
    )]
    DuplicateModelFieldId {
        role: SchemaRole,
        field_id: String,
        first_index: usize,
        second_index: usize,
    },
    #[error("{role:?} schema does not declare a model relation identity")]
    MissingModelRelationId { role: SchemaRole },
    #[error("{role:?} schema has no compiled model metadata")]
    ModelMetadataUnavailable { role: SchemaRole },
    #[error("{role:?} schema has no model field with ID {field_id:?}")]
    UnknownModelFieldId { role: SchemaRole, field_id: String },
    #[error("{role:?} schema has no model field with semantic role {semantic_role:?}")]
    UnknownModelSemanticRole {
        role: SchemaRole,
        semantic_role: String,
    },
    #[error(
        "{role:?} schema semantic role {semantic_role:?} resolves to {match_count} fields, not one"
    )]
    AmbiguousModelSemanticRole {
        role: SchemaRole,
        semantic_role: String,
        match_count: usize,
    },
    #[error(
        "{role:?} cast binding at index {field_index} names model field {actual_field_id:?}, expected {expected_field_id:?}"
    )]
    ModelFieldBindingMismatch {
        role: SchemaRole,
        field_index: usize,
        expected_field_id: Arc<str>,
        actual_field_id: Arc<str>,
    },
    #[error(
        "model schema compilation for {identity:?} found compiler-owned metadata key {key:?} in {location}"
    )]
    ReservedModelMetadataKey {
        identity: String,
        location: String,
        key: String,
    },
    #[error("{role:?} field {path} has invalid extension metadata: {reason}")]
    InvalidExtensionMetadata {
        role: SchemaRole,
        path: String,
        reason: &'static str,
    },
    #[error("expected {expected} logical field mappings, received {actual}")]
    MappingCount { expected: usize, actual: usize },
    #[error(
        "logical field index {logical_index} is out of bounds for {logical_field_count} fields"
    )]
    LogicalMappingOutOfBounds {
        logical_index: usize,
        logical_field_count: usize,
    },
    #[error("logical field index {logical_index} is mapped more than once")]
    DuplicateLogicalMapping { logical_index: usize },
    #[error(
        "{purpose:?} index {mapped_index} for logical field {logical_index} is out of bounds for {storage_field_count} storage fields"
    )]
    PhysicalMappingOutOfBounds {
        purpose: IndexPurpose,
        logical_index: usize,
        mapped_index: usize,
        storage_field_count: usize,
    },
    #[error(
        "{purpose:?} index {mapped_index} is shared by logical fields {first_logical_index} and {second_logical_index}"
    )]
    DuplicatePhysicalMapping {
        purpose: IndexPurpose,
        mapped_index: usize,
        first_logical_index: usize,
        second_logical_index: usize,
    },
    #[error(
        "logical index {logical_index} requested for {purpose:?} is out of bounds for {logical_field_count} fields"
    )]
    LogicalIndexOutOfBounds {
        purpose: IndexPurpose,
        logical_index: usize,
        logical_field_count: usize,
    },
    #[error("storage index {storage_index} is out of bounds for {storage_field_count} fields")]
    StorageIndexOutOfBounds {
        storage_index: usize,
        storage_field_count: usize,
    },
    #[error("storage index {storage_index} has no logical field mapping")]
    UnmappedStorageIndex { storage_index: usize },
    #[error("{phase:?} {role:?} schema failed {compatibility:?} compatibility: {difference}")]
    IncompatibleSchema {
        phase: SchemaPhase,
        role: SchemaRole,
        compatibility: SchemaCompatibility,
        difference: SchemaDifference,
    },
    #[error(
        "{phase:?} qualifier differs at field {field_index}: expected {expected}, actual {actual:?}"
    )]
    QualifierMismatch {
        phase: SchemaPhase,
        field_index: usize,
        expected: TableReference,
        actual: Option<TableReference>,
    },
    #[error("RecordBatch schema differs from its declared stream schema: {difference}")]
    BatchStreamSchemaMismatch { difference: SchemaDifference },
    #[error("failed to project the {role:?} Arrow schema: {source}")]
    ArrowProjection {
        role: SchemaRole,
        #[source]
        source: ArrowError,
    },
    #[error("model schema compilation failed for {identity:?}: {reason}")]
    ModelCompilation { identity: String, reason: String },
    #[error(
        "model field {logical_field_id:?} cannot cast {logical_type} to storage field {storage_field_id:?} type {storage_type}"
    )]
    UnsupportedStorageCast {
        logical_field_id: String,
        storage_field_id: String,
        logical_type: DataType,
        storage_type: DataType,
    },
    #[error(
        "model field {logical_field_id:?} cannot restore storage field {storage_field_id:?} type {storage_type} to {logical_type}"
    )]
    UnsupportedRestorationCast {
        logical_field_id: String,
        storage_field_id: String,
        storage_type: DataType,
        logical_type: DataType,
    },
    #[error(
        "{direction} cast failed for logical field {logical_field_id:?} and storage field {storage_field_id:?}: {source}"
    )]
    FieldCast {
        direction: &'static str,
        logical_field_id: Arc<str>,
        storage_field_id: Arc<str>,
        #[source]
        source: ArrowError,
    },
    #[error(
        "logical-to-storage adaptation cannot synthesize unmapped storage field {field_name:?} at index {storage_index}"
    )]
    UnmappedStorageOutput {
        storage_index: usize,
        field_name: String,
    },
    #[error("failed to construct the {role:?} adapted RecordBatch: {source}")]
    AdaptedRecordBatch {
        role: SchemaRole,
        #[source]
        source: ArrowError,
    },
    #[error(
        "{phase:?} accepts {expected_role:?} schema validation, not the requested {actual_role:?} boundary"
    )]
    InvalidPhaseRole {
        phase: SchemaPhase,
        expected_role: SchemaRole,
        actual_role: SchemaRole,
    },
    #[error("non-nullable field {path} contains {null_count} null values at {phase:?}")]
    NullabilityViolation {
        phase: SchemaPhase,
        path: String,
        null_count: usize,
    },
}

#[derive(Clone, Debug)]
struct ModelSchemaIndex {
    relation_id: Arc<str>,
    field_ids_by_index: Arc<[Arc<str>]>,
    field_indices_by_id: BTreeMap<Arc<str>, usize>,
    field_indices_by_semantic_role: BTreeMap<Arc<str>, Arc<[usize]>>,
}

fn compile_model_schema_index(
    role: SchemaRole,
    schema: &Schema,
) -> Result<Option<ModelSchemaIndex>, SchemaContractError> {
    let relation_id = schema
        .metadata()
        .get(MODEL_RELATION_ID_METADATA_KEY)
        .map(|value| {
            if value.trim().is_empty() {
                Err(SchemaContractError::EmptyModelMetadata {
                    role,
                    key: MODEL_RELATION_ID_METADATA_KEY,
                    path: "$".to_owned(),
                })
            } else {
                Ok(Arc::<str>::from(value.as_str()))
            }
        })
        .transpose()?;
    let fields_declare_identity_metadata = schema.fields().iter().any(|field| {
        field.metadata().contains_key(MODEL_FIELD_ID_METADATA_KEY)
            || field
                .metadata()
                .contains_key(MODEL_SEMANTIC_ROLE_METADATA_KEY)
    });
    if relation_id.is_none() {
        if fields_declare_identity_metadata {
            return Err(SchemaContractError::MissingModelRelationId { role });
        }
        return Ok(None);
    }

    let mut field_indices_by_id = BTreeMap::new();
    let mut field_ids_by_index = Vec::with_capacity(schema.fields().len());
    let mut semantic_role_indices = BTreeMap::<Arc<str>, Vec<usize>>::new();
    for (field_index, field) in schema.fields().iter().enumerate() {
        let field_id = field.metadata().get(MODEL_FIELD_ID_METADATA_KEY).ok_or(
            SchemaContractError::IncompleteModelFieldMetadata {
                role,
                field_index,
                missing_key: MODEL_FIELD_ID_METADATA_KEY,
            },
        )?;
        let path = format!("$.{}", field.name());
        if field_id.trim().is_empty() {
            return Err(SchemaContractError::EmptyModelMetadata {
                role,
                key: MODEL_FIELD_ID_METADATA_KEY,
                path,
            });
        }
        let field_id = Arc::<str>::from(field_id.as_str());
        if let Some(first_index) = field_indices_by_id.insert(Arc::clone(&field_id), field_index) {
            return Err(SchemaContractError::DuplicateModelFieldId {
                role,
                field_id: field_id.to_string(),
                first_index,
                second_index: field_index,
            });
        }
        field_ids_by_index.push(field_id);
        if let Some(semantic_role) = field.metadata().get(MODEL_SEMANTIC_ROLE_METADATA_KEY) {
            if semantic_role.trim().is_empty() {
                return Err(SchemaContractError::EmptyModelMetadata {
                    role,
                    key: MODEL_SEMANTIC_ROLE_METADATA_KEY,
                    path,
                });
            }
            semantic_role_indices
                .entry(Arc::<str>::from(semantic_role.as_str()))
                .or_default()
                .push(field_index);
        }
    }

    Ok(Some(ModelSchemaIndex {
        relation_id: relation_id.expect("model field metadata requires a relation identity"),
        field_ids_by_index: field_ids_by_index.into(),
        field_indices_by_id,
        field_indices_by_semantic_role: semantic_role_indices
            .into_iter()
            .map(|(semantic_role, indices)| (semantic_role, Arc::from(indices)))
            .collect(),
    }))
}

/// An executable logical/physical schema contract installed in one live session.
#[derive(Clone, Debug)]
pub struct SchemaContract {
    source_schema_identity: Arc<str>,
    qualifier: TableReference,
    logical_schema: SchemaRef,
    qualified_logical_schema: Arc<DFSchema>,
    storage_schema: SchemaRef,
    mappings: Arc<[FieldIndexMapping]>,
    storage_to_logical: Arc<[Option<usize>]>,
    logical_model_index: Option<Arc<ModelSchemaIndex>>,
    storage_model_index: Option<Arc<ModelSchemaIndex>>,
    casts: Arc<[FieldCastBinding]>,
    constraints: Arc<Constraints>,
    compatibility: SchemaCompatibility,
    column_mapping_mode: ColumnMappingMode,
    deletion_vector_behavior: DeletionVectorBehavior,
    empty_stream_schema: SchemaRef,
}

impl SchemaContract {
    /// Build and validate a contract emitted by a provider or transformation.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the source identity, extension metadata,
    /// DataFusion qualification, or any index mapping is invalid.
    pub fn try_new(
        source_schema_identity: impl Into<Arc<str>>,
        qualifier: TableReference,
        logical_schema: SchemaRef,
        storage_schema: SchemaRef,
        mappings: Vec<FieldIndexMapping>,
    ) -> Result<Self, SchemaContractError> {
        Self::try_new_compiled(
            source_schema_identity,
            qualifier,
            logical_schema,
            storage_schema,
            mappings,
            Vec::new(),
            Constraints::default(),
            SchemaCompatibility::Exact,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::Forbidden,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "all compiled schema-lifecycle products are kept in one validated constructor"
    )]
    fn try_new_compiled(
        source_schema_identity: impl Into<Arc<str>>,
        qualifier: TableReference,
        logical_schema: SchemaRef,
        storage_schema: SchemaRef,
        mappings: Vec<FieldIndexMapping>,
        casts: Vec<FieldCastBinding>,
        constraints: Constraints,
        compatibility: SchemaCompatibility,
        column_mapping_mode: ColumnMappingMode,
        deletion_vector_behavior: DeletionVectorBehavior,
    ) -> Result<Self, SchemaContractError> {
        let source_schema_identity = source_schema_identity.into();
        if source_schema_identity.trim().is_empty() {
            return Err(SchemaContractError::EmptySourceSchemaIdentity);
        }

        validate_extension_metadata(SchemaRole::Logical, logical_schema.as_ref())?;
        validate_extension_metadata(SchemaRole::Storage, storage_schema.as_ref())?;
        let logical_model_index =
            compile_model_schema_index(SchemaRole::Logical, logical_schema.as_ref())?.map(Arc::new);
        let storage_model_index =
            compile_model_schema_index(SchemaRole::Storage, storage_schema.as_ref())?.map(Arc::new);

        let functional_dependencies = FunctionalDependencies::new_from_constraints(
            Some(&constraints),
            logical_schema.fields().len(),
        );
        let qualified_logical_schema = Arc::new(
            DFSchema::try_from_qualified_schema(qualifier.clone(), logical_schema.as_ref())
                .and_then(|schema| schema.with_functional_dependencies(functional_dependencies))
                .map_err(|source| SchemaContractError::QualifiedSchema { source })?,
        );

        let (mappings, storage_to_logical) = validate_mappings(
            logical_schema.fields().len(),
            storage_schema.fields().len(),
            mappings,
        )?;
        let casts = if casts.is_empty() {
            mappings
                .iter()
                .map(|mapping| {
                    let logical = logical_schema.field(mapping.logical_index());
                    let storage = storage_schema.field(mapping.storage_index());
                    FieldCastBinding {
                        logical_field_id: logical_model_index.as_ref().map_or_else(
                            || Arc::from(logical.name().as_str()),
                            |index| Arc::clone(&index.field_ids_by_index[mapping.logical_index()]),
                        ),
                        storage_field_id: storage_model_index.as_ref().map_or_else(
                            || Arc::from(storage.name().as_str()),
                            |index| Arc::clone(&index.field_ids_by_index[mapping.storage_index()]),
                        ),
                        logical_index: mapping.logical_index(),
                        storage_index: mapping.storage_index(),
                        logical_data_type: logical.data_type().clone(),
                        storage_data_type: storage.data_type().clone(),
                    }
                })
                .collect::<Vec<_>>()
        } else {
            casts
        };
        validate_cast_bindings(
            &casts,
            &mappings,
            &logical_schema,
            &storage_schema,
            logical_model_index.as_deref(),
            storage_model_index.as_deref(),
        )?;

        Ok(Self {
            source_schema_identity,
            qualifier,
            empty_stream_schema: Arc::clone(&logical_schema),
            logical_schema,
            qualified_logical_schema,
            storage_schema,
            mappings,
            storage_to_logical,
            logical_model_index,
            storage_model_index,
            casts: casts.into(),
            constraints: Arc::new(constraints),
            compatibility,
            column_mapping_mode,
            deletion_vector_behavior,
        })
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        &self.source_schema_identity
    }

    #[must_use]
    pub const fn qualifier(&self) -> &TableReference {
        &self.qualifier
    }

    #[must_use]
    pub const fn logical_schema(&self) -> &SchemaRef {
        &self.logical_schema
    }

    #[must_use]
    pub const fn qualified_logical_schema(&self) -> &Arc<DFSchema> {
        &self.qualified_logical_schema
    }

    #[must_use]
    pub const fn storage_schema(&self) -> &SchemaRef {
        &self.storage_schema
    }

    /// Resolve the programmatically installed relation identity for one schema role.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::ModelMetadataUnavailable`] for a legacy or otherwise
    /// unannotated schema. Callers must not substitute an Arrow table or field name for model
    /// identity.
    pub fn model_relation_id(&self, role: SchemaRole) -> Result<&str, SchemaContractError> {
        self.relation_id(role)
    }

    /// Resolve the exact relation identity carried by the live session schema.
    pub fn relation_id(&self, role: SchemaRole) -> Result<&str, SchemaContractError> {
        Ok(self.model_index(role)?.relation_id.as_ref())
    }

    /// Resolve a stable field ID to its exact index within the selected schema role.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::UnknownModelFieldId`] rather than falling back to a
    /// physical field name or ordinal.
    pub fn field_index_for_model_id(
        &self,
        role: SchemaRole,
        field_id: &str,
    ) -> Result<usize, SchemaContractError> {
        self.field_index_for_id(role, field_id)
    }

    /// Resolve a session-owned field identity to its exact schema index.
    pub fn field_index_for_id(
        &self,
        role: SchemaRole,
        field_id: &str,
    ) -> Result<usize, SchemaContractError> {
        self.model_index(role)?
            .field_indices_by_id
            .get(field_id)
            .copied()
            .ok_or_else(|| SchemaContractError::UnknownModelFieldId {
                role,
                field_id: field_id.to_owned(),
            })
    }

    /// Resolve the stable identity carried by a field at an exact schema index.
    ///
    /// This is the inverse of [`Self::field_index_for_id`]. It deliberately
    /// reads the metadata compiled from the provider/transformation schema;
    /// field names and ordinals are never substituted for identity.
    pub fn field_id_at(&self, role: SchemaRole, index: usize) -> Result<&str, SchemaContractError> {
        self.model_index(role)?
            .field_ids_by_index
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| SchemaContractError::UnknownModelFieldId {
                role,
                field_id: format!("index:{index}"),
            })
    }

    /// Resolve one logical model field ID without consulting Arrow field names.
    pub fn logical_index_for_field_id(&self, field_id: &str) -> Result<usize, SchemaContractError> {
        self.field_index_for_model_id(SchemaRole::Logical, field_id)
    }

    /// Resolve one storage model field ID without consulting Arrow field names.
    pub fn storage_index_for_field_id(&self, field_id: &str) -> Result<usize, SchemaContractError> {
        self.field_index_for_model_id(SchemaRole::Storage, field_id)
    }

    /// Resolve a model field ID to its exact Arrow field and index.
    pub fn field_for_model_id(
        &self,
        role: SchemaRole,
        field_id: &str,
    ) -> Result<(usize, &Field), SchemaContractError> {
        self.field_for_id(role, field_id)
    }

    /// Resolve a session-owned field identity to its exact Arrow field and index.
    pub fn field_for_id(
        &self,
        role: SchemaRole,
        field_id: &str,
    ) -> Result<(usize, &Field), SchemaContractError> {
        let index = self.field_index_for_id(role, field_id)?;
        let schema = match role {
            SchemaRole::Logical => self.logical_schema.as_ref(),
            SchemaRole::Storage => self.storage_schema.as_ref(),
        };
        Ok((index, schema.field(index)))
    }

    /// Return every exact field index carrying a model-owned semantic role.
    ///
    /// Semantic roles are intentionally one-to-many. An empty slice means a complete model
    /// relation did not declare the role; callers requiring exactly one field must use
    /// [`Self::unique_field_index_for_semantic_role`].
    pub fn field_indices_for_semantic_role(
        &self,
        role: SchemaRole,
        semantic_role: &str,
    ) -> Result<&[usize], SchemaContractError> {
        Ok(self
            .model_index(role)?
            .field_indices_by_semantic_role
            .get(semantic_role)
            .map_or(&[], AsRef::as_ref))
    }

    /// Return every logical field index carrying a model-owned semantic role.
    pub fn logical_indices_for_semantic_role(
        &self,
        semantic_role: &str,
    ) -> Result<&[usize], SchemaContractError> {
        self.field_indices_for_semantic_role(SchemaRole::Logical, semantic_role)
    }

    /// Resolve a semantic role that the consuming operation requires to be unique.
    ///
    /// # Errors
    ///
    /// Returns typed unknown and ambiguity errors. Uniqueness is an operation-level
    /// requirement, not a global restriction on model roles.
    pub fn unique_field_index_for_semantic_role(
        &self,
        role: SchemaRole,
        semantic_role: &str,
    ) -> Result<usize, SchemaContractError> {
        match self.field_indices_for_semantic_role(role, semantic_role)? {
            [index] => Ok(*index),
            [] => Err(SchemaContractError::UnknownModelSemanticRole {
                role,
                semantic_role: semantic_role.to_owned(),
            }),
            indices => Err(SchemaContractError::AmbiguousModelSemanticRole {
                role,
                semantic_role: semantic_role.to_owned(),
                match_count: indices.len(),
            }),
        }
    }

    fn model_index(&self, role: SchemaRole) -> Result<&ModelSchemaIndex, SchemaContractError> {
        let index = match role {
            SchemaRole::Logical => &self.logical_model_index,
            SchemaRole::Storage => &self.storage_model_index,
        };
        index
            .as_deref()
            .ok_or(SchemaContractError::ModelMetadataUnavailable { role })
    }

    #[must_use]
    pub fn mappings(&self) -> &[FieldIndexMapping] {
        &self.mappings
    }

    #[must_use]
    pub fn casts(&self) -> &[FieldCastBinding] {
        &self.casts
    }

    #[must_use]
    pub const fn constraints(&self) -> &Arc<Constraints> {
        &self.constraints
    }

    #[must_use]
    pub const fn compatibility(&self) -> SchemaCompatibility {
        self.compatibility
    }

    #[must_use]
    pub const fn column_mapping_mode(&self) -> ColumnMappingMode {
        self.column_mapping_mode
    }

    #[must_use]
    pub const fn deletion_vector_behavior(&self) -> DeletionVectorBehavior {
        self.deletion_vector_behavior
    }

    /// The schema to publish before an empty execution stream yields no batches.
    #[must_use]
    pub const fn empty_stream_schema(&self) -> &SchemaRef {
        &self.empty_stream_schema
    }

    /// Construct an explicit zero-row batch without losing the logical schema.
    #[must_use]
    pub fn empty_stream_batch(&self) -> RecordBatch {
        RecordBatch::new_empty(Arc::clone(&self.empty_stream_schema))
    }

    /// Translate one logical index for the requested consumer.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::LogicalIndexOutOfBounds`] when the
    /// logical index is not part of this contract.
    pub fn mapped_index(
        &self,
        purpose: IndexPurpose,
        logical_index: usize,
    ) -> Result<usize, SchemaContractError> {
        self.mappings
            .get(logical_index)
            .copied()
            .map(|mapping| mapping.index_for(purpose))
            .ok_or(SchemaContractError::LogicalIndexOutOfBounds {
                purpose,
                logical_index,
                logical_field_count: self.mappings.len(),
            })
    }

    /// Translate an ordered logical projection to provider projection indices.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds error when any logical index is unknown.
    pub fn map_projection(
        &self,
        logical_indices: &[usize],
    ) -> Result<Vec<usize>, SchemaContractError> {
        self.map_indices(IndexPurpose::Projection, logical_indices)
    }

    /// Translate logical field indices used by a pushed filter.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds error when any logical index is unknown.
    pub fn map_filter_indices(
        &self,
        logical_indices: &[usize],
    ) -> Result<Vec<usize>, SchemaContractError> {
        self.map_indices(IndexPurpose::Filter, logical_indices)
    }

    /// Translate logical field indices used to request column statistics.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds error when any logical index is unknown.
    pub fn map_statistics_indices(
        &self,
        logical_indices: &[usize],
    ) -> Result<Vec<usize>, SchemaContractError> {
        self.map_indices(IndexPurpose::Statistics, logical_indices)
    }

    /// Resolve a durable storage field back to its logical field, if mapped.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::StorageIndexOutOfBounds`] when the
    /// storage index is outside the bound storage schema.
    pub fn logical_index_for_storage(
        &self,
        storage_index: usize,
    ) -> Result<Option<usize>, SchemaContractError> {
        self.storage_to_logical.get(storage_index).copied().ok_or(
            SchemaContractError::StorageIndexOutOfBounds {
                storage_index,
                storage_field_count: self.storage_to_logical.len(),
            },
        )
    }

    /// Restore an ordered storage projection to logical indices.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds or unmapped-column error when restoration is not
    /// total for the requested projection.
    pub fn restore_storage_projection(
        &self,
        storage_indices: &[usize],
    ) -> Result<Vec<usize>, SchemaContractError> {
        storage_indices
            .iter()
            .map(|storage_index| {
                self.logical_index_for_storage(*storage_index)?.ok_or(
                    SchemaContractError::UnmappedStorageIndex {
                        storage_index: *storage_index,
                    },
                )
            })
            .collect()
    }

    /// Project the logical schema with Arrow's native schema projection.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::ArrowProjection`] when Arrow rejects an
    /// out-of-range projection.
    pub fn project_logical_schema(
        &self,
        logical_indices: &[usize],
    ) -> Result<SchemaRef, SchemaContractError> {
        self.logical_schema
            .project(logical_indices)
            .map(Arc::new)
            .map_err(|source| SchemaContractError::ArrowProjection {
                role: SchemaRole::Logical,
                source,
            })
    }

    /// Project the storage schema after applying the provider projection map.
    ///
    /// # Errors
    ///
    /// Returns a typed mapping or Arrow projection error when an index is not
    /// valid for the bound schemas.
    pub fn project_storage_schema(
        &self,
        logical_indices: &[usize],
    ) -> Result<SchemaRef, SchemaContractError> {
        let physical_indices = self.map_projection(logical_indices)?;
        self.storage_schema
            .project(&physical_indices)
            .map(Arc::new)
            .map_err(|source| SchemaContractError::ArrowProjection {
                role: SchemaRole::Storage,
                source,
            })
    }

    /// Validate the default output role for a named phase.
    ///
    /// # Errors
    ///
    /// Returns a located schema or extension-metadata mismatch.
    pub fn validate_phase_schema(
        &self,
        phase: SchemaPhase,
        actual: &Schema,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        self.validate_arrow_schema(phase, phase.output_role(), actual, compatibility)
    }

    /// Validate a phase using the compatibility policy selected by the model.
    pub fn validate_bound_phase_schema(
        &self,
        phase: SchemaPhase,
        actual: &Schema,
    ) -> Result<(), SchemaContractError> {
        self.validate_phase_schema(phase, actual, self.compatibility)
    }

    /// Validate either the logical or storage schema at a named phase.
    ///
    /// # Errors
    ///
    /// Returns a located schema or extension-metadata mismatch.
    pub fn validate_arrow_schema(
        &self,
        phase: SchemaPhase,
        role: SchemaRole,
        actual: &Schema,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        validate_extension_metadata(role, actual)?;
        let expected = self.schema_for_role(role);
        if schemas_compatible(expected, actual, compatibility) {
            return Ok(());
        }

        Err(SchemaContractError::IncompatibleSchema {
            phase,
            role,
            compatibility,
            difference: diagnose_schema(expected, actual, compatibility),
        })
    }

    /// Validate an analyzed or optimized DataFusion schema, including qualifiers.
    ///
    /// # Errors
    ///
    /// Returns a located Arrow mismatch or a field-specific qualifier mismatch.
    pub fn validate_qualified_logical_schema(
        &self,
        phase: SchemaPhase,
        actual: &DFSchema,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        self.validate_arrow_schema(phase, SchemaRole::Logical, actual.as_arrow(), compatibility)?;

        for (field_index, (actual_qualifier, _)) in actual.iter().enumerate() {
            if actual_qualifier != Some(&self.qualifier) {
                return Err(SchemaContractError::QualifierMismatch {
                    phase,
                    field_index,
                    expected: self.qualifier.clone(),
                    actual: actual_qualifier.cloned(),
                });
            }
        }
        Ok(())
    }

    /// Validate a DataFusion logical plan at its owned lifecycle phase.
    pub fn validate_logical_plan(
        &self,
        phase: SchemaPhase,
        plan: &LogicalPlan,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        if !matches!(
            phase,
            SchemaPhase::AnalyzedLogicalPlan | SchemaPhase::OptimizedLogicalPlan
        ) {
            return Err(SchemaContractError::InvalidPhaseRole {
                phase,
                expected_role: SchemaRole::Logical,
                actual_role: phase.output_role(),
            });
        }
        self.validate_qualified_logical_schema(phase, plan.schema(), compatibility)
    }

    /// Validate an initial or optimized DataFusion physical plan.
    pub fn validate_physical_plan(
        &self,
        phase: SchemaPhase,
        plan: &dyn ExecutionPlan,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        if !matches!(
            phase,
            SchemaPhase::InitialPhysicalPlan | SchemaPhase::OptimizedPhysicalPlan
        ) {
            return Err(SchemaContractError::InvalidPhaseRole {
                phase,
                expected_role: SchemaRole::Logical,
                actual_role: phase.output_role(),
            });
        }
        self.validate_arrow_schema(
            phase,
            SchemaRole::Logical,
            plan.schema().as_ref(),
            compatibility,
        )
    }

    /// Validate a DataFusion stream's declared schema before polling it.
    pub fn validate_stream_schema(
        &self,
        stream: &dyn RecordBatchStream,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        self.validate_arrow_schema(
            SchemaPhase::StreamConstruction,
            SchemaRole::Logical,
            stream.schema().as_ref(),
            compatibility,
        )
    }

    /// Wrap a DataFusion stream so every emitted batch is checked causally.
    pub fn validate_stream(
        self: &Arc<Self>,
        stream: SendableRecordBatchStream,
        compatibility: SchemaCompatibility,
    ) -> Result<SendableRecordBatchStream, SchemaContractError> {
        self.validate_stream_schema(stream.as_ref().get_ref(), compatibility)?;
        let schema = stream.schema();
        Ok(Box::pin(SchemaValidatedRecordBatchStream {
            contract: Arc::clone(self),
            inner: stream,
            schema,
            compatibility,
        }))
    }

    /// Validate a batch against both its declared stream schema and this contract.
    ///
    /// # Errors
    ///
    /// Returns a schema-contract error when the stream is incompatible, the
    /// batch disagrees with its stream, or the batch violates the contract.
    pub fn validate_batch(
        &self,
        stream_schema: &SchemaRef,
        batch: &RecordBatch,
        compatibility: SchemaCompatibility,
    ) -> Result<(), SchemaContractError> {
        self.validate_arrow_schema(
            SchemaPhase::StreamConstruction,
            SchemaRole::Logical,
            stream_schema.as_ref(),
            compatibility,
        )?;

        if batch.schema_ref().as_ref() != stream_schema.as_ref() {
            return Err(SchemaContractError::BatchStreamSchemaMismatch {
                difference: diagnose_schema(
                    stream_schema.as_ref(),
                    batch.schema_ref().as_ref(),
                    SchemaCompatibility::Exact,
                ),
            });
        }

        self.validate_arrow_schema(
            SchemaPhase::RecordBatch,
            SchemaRole::Logical,
            batch.schema_ref().as_ref(),
            compatibility,
        )?;
        validate_batch_nullability(SchemaPhase::RecordBatch, batch)
    }

    /// Validate the storage representation presented to a write sink.
    ///
    /// # Errors
    ///
    /// Returns a located exact-schema mismatch for the storage contract.
    pub fn validate_write_batch(&self, batch: &RecordBatch) -> Result<(), SchemaContractError> {
        self.validate_arrow_schema(
            SchemaPhase::WriteSink,
            SchemaRole::Storage,
            batch.schema_ref().as_ref(),
            SchemaCompatibility::Exact,
        )?;
        validate_batch_nullability(SchemaPhase::WriteSink, batch)
    }

    /// Project, reorder, and strictly cast a logical batch to storage form.
    ///
    /// Unmapped physical columns are never fabricated. A sink that owns such
    /// columns must bind them before calling this total adaptation path.
    pub fn adapt_logical_batch_to_storage(
        &self,
        batch: &RecordBatch,
    ) -> Result<RecordBatch, SchemaContractError> {
        self.validate_batch(&batch.schema(), batch, SchemaCompatibility::Exact)?;
        let mut columns: Vec<Option<ArrayRef>> = vec![None; self.storage_schema.fields().len()];
        for cast in self.casts.iter() {
            let array = batch.column(cast.logical_index());
            columns[cast.storage_index()] = Some(cast_array(
                array,
                cast.storage_data_type(),
                cast,
                "logical-to-storage",
            )?);
        }
        let columns = columns
            .into_iter()
            .enumerate()
            .map(|(storage_index, column)| {
                column.ok_or_else(|| SchemaContractError::UnmappedStorageOutput {
                    storage_index,
                    field_name: self.storage_schema.field(storage_index).name().clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch =
            RecordBatch::try_new(Arc::clone(&self.storage_schema), columns).map_err(|source| {
                SchemaContractError::AdaptedRecordBatch {
                    role: SchemaRole::Storage,
                    source,
                }
            })?;
        self.validate_write_batch(&batch)?;
        Ok(batch)
    }

    /// Restore a provider/storage batch to exact model-owned logical meaning.
    pub fn restore_storage_batch(
        &self,
        batch: &RecordBatch,
    ) -> Result<RecordBatch, SchemaContractError> {
        self.validate_write_batch(batch)?;
        let columns = self
            .casts
            .iter()
            .map(|cast| {
                cast_array(
                    batch.column(cast.storage_index()),
                    cast.logical_data_type(),
                    cast,
                    "storage-to-logical",
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch =
            RecordBatch::try_new(Arc::clone(&self.logical_schema), columns).map_err(|source| {
                SchemaContractError::AdaptedRecordBatch {
                    role: SchemaRole::Logical,
                    source,
                }
            })?;
        self.validate_batch(
            &self.empty_stream_schema,
            &batch,
            SchemaCompatibility::Exact,
        )?;
        Ok(batch)
    }

    fn schema_for_role(&self, role: SchemaRole) -> &Schema {
        match role {
            SchemaRole::Logical => self.logical_schema.as_ref(),
            SchemaRole::Storage => self.storage_schema.as_ref(),
        }
    }

    fn map_indices(
        &self,
        purpose: IndexPurpose,
        logical_indices: &[usize],
    ) -> Result<Vec<usize>, SchemaContractError> {
        logical_indices
            .iter()
            .map(|logical_index| self.mapped_index(purpose, *logical_index))
            .collect()
    }
}

struct SchemaValidatedRecordBatchStream {
    contract: Arc<SchemaContract>,
    inner: SendableRecordBatchStream,
    schema: SchemaRef,
    compatibility: SchemaCompatibility,
}

impl Stream for SchemaValidatedRecordBatchStream {
    type Item = datafusion::common::Result<RecordBatch>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(context) {
            Poll::Ready(Some(Ok(batch))) => Poll::Ready(Some(
                self.contract
                    .validate_batch(&self.schema, &batch, self.compatibility)
                    .map(|()| batch)
                    .map_err(|error| DataFusionError::External(Box::new(error))),
            )),
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl RecordBatchStream for SchemaValidatedRecordBatchStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

fn cast_array(
    array: &ArrayRef,
    target_type: &DataType,
    binding: &FieldCastBinding,
    direction: &'static str,
) -> Result<ArrayRef, SchemaContractError> {
    if array.data_type() == target_type {
        return Ok(Arc::clone(array));
    }
    cast_with_options(
        array.as_ref(),
        target_type,
        &CastOptions {
            safe: false,
            ..CastOptions::default()
        },
    )
    .map_err(|source| SchemaContractError::FieldCast {
        direction,
        logical_field_id: Arc::clone(&binding.logical_field_id),
        storage_field_id: Arc::clone(&binding.storage_field_id),
        source,
    })
}

fn validate_batch_nullability(
    phase: SchemaPhase,
    batch: &RecordBatch,
) -> Result<(), SchemaContractError> {
    for (field, array) in batch.schema_ref().fields().iter().zip(batch.columns()) {
        if !field.is_nullable() && array.null_count() != 0 {
            return Err(SchemaContractError::NullabilityViolation {
                phase,
                path: child_path("$", field.name()),
                null_count: array.null_count(),
            });
        }
    }
    Ok(())
}

impl SchemaContractModelRows {
    /// Project the replayed Arrow model relations into schema-compiler rows.
    ///
    /// `physical_bindings` are the typed results of the model-owned mapping
    /// programs for the exact provider schemas observed while assembling an
    /// epoch. Every result must match one authoritative `physical_binding`
    /// model row; missing, extra, or mismatched selections fail closed.
    pub fn from_model_epoch(
        epoch: &ModelEpoch,
        catalog_name: &str,
        mut physical_bindings: Vec<ModelPhysicalBindingRow>,
    ) -> Result<Self, SchemaContractError> {
        if catalog_name.trim().is_empty() {
            return Err(model_compilation(
                epoch.model_epoch_id(),
                "schema catalog name is empty".to_owned(),
            ));
        }
        let relations = project_model_relations(epoch, catalog_name)?;
        let semantic_types = project_model_semantic_types(epoch)?;
        let fields = project_model_fields(epoch)?;
        let keys = project_model_keys(epoch)?;
        let representations = project_model_representations(epoch)?;
        validate_model_physical_bindings(epoch, &physical_bindings)?;
        physical_bindings
            .sort_by(|left, right| left.physical_binding_id.cmp(&right.physical_binding_id));

        Ok(Self {
            relations,
            semantic_types,
            fields,
            keys,
            representations,
            physical_bindings,
        })
    }

    /// Compile every physical binding, keyed by its model identity.
    pub fn compile_all(&self) -> Result<BTreeMap<String, SchemaContract>, SchemaContractError> {
        self.physical_bindings
            .iter()
            .map(|binding| {
                self.compile(&binding.physical_binding_id)
                    .map(|contract| (binding.physical_binding_id.clone(), contract))
            })
            .collect()
    }

    /// Compile one exact physical binding into an executable schema contract.
    ///
    /// The row set remains the sole semantic authority: Arrow fields,
    /// `DFSchema` qualification and functional dependencies, storage casts,
    /// and all consumer index maps are derived together.
    pub fn compile(
        &self,
        physical_binding_id: &str,
    ) -> Result<SchemaContract, SchemaContractError> {
        let binding = unique_row(
            self.physical_bindings
                .iter()
                .filter(|row| row.physical_binding_id == physical_binding_id),
            physical_binding_id,
            "physical binding",
        )?;
        let logical_relation = unique_row(
            self.relations
                .iter()
                .filter(|row| row.relation_id == binding.logical_relation_id),
            &binding.logical_relation_id,
            "logical relation",
        )?;
        let storage_relation = unique_row(
            self.relations
                .iter()
                .filter(|row| row.relation_id == binding.storage_relation_id),
            &binding.storage_relation_id,
            "storage relation",
        )?;

        let semantic_types = unique_by(
            &self.semantic_types,
            |row| row.semantic_type_id.as_str(),
            physical_binding_id,
            "semantic type",
        )?;
        let representations = unique_by(
            &self.representations,
            |row| row.semantic_type_id.as_str(),
            physical_binding_id,
            "semantic-type representation",
        )?;
        let logical_fields = relation_fields(
            &self.fields,
            &binding.logical_relation_id,
            physical_binding_id,
        )?;
        let storage_fields = relation_fields(
            &self.fields,
            &binding.storage_relation_id,
            physical_binding_id,
        )?;

        let logical_by_id = logical_fields
            .iter()
            .enumerate()
            .map(|(index, row)| (row.field_id.as_str(), (index, *row)))
            .collect::<BTreeMap<_, _>>();
        let storage_by_id = storage_fields
            .iter()
            .enumerate()
            .map(|(index, row)| (row.field_id.as_str(), (index, *row)))
            .collect::<BTreeMap<_, _>>();

        let logical_arrow_fields = logical_fields
            .iter()
            .map(|row| {
                compile_model_field(
                    row,
                    SchemaRole::Logical,
                    &semantic_types,
                    &representations,
                    physical_binding_id,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let storage_arrow_fields = storage_fields
            .iter()
            .map(|row| {
                compile_model_field(
                    row,
                    SchemaRole::Storage,
                    &semantic_types,
                    &representations,
                    physical_binding_id,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        reject_reserved_relation_metadata(
            &logical_relation.relation_metadata,
            physical_binding_id,
            &binding.logical_relation_id,
        )?;
        reject_reserved_relation_metadata(
            &storage_relation.relation_metadata,
            physical_binding_id,
            &binding.storage_relation_id,
        )?;
        let mut logical_metadata = logical_relation.relation_metadata.clone();
        logical_metadata.insert(
            MODEL_RELATION_ID_METADATA_KEY.to_owned(),
            binding.logical_relation_id.clone(),
        );
        logical_metadata.insert(
            "codefabric.model.physical_binding_id".to_owned(),
            binding.physical_binding_id.clone(),
        );
        logical_metadata.insert(
            "codefabric.model.mapping_program_id".to_owned(),
            binding.mapping_program_id.clone(),
        );
        let mut storage_metadata = storage_relation.relation_metadata.clone();
        storage_metadata.insert(
            MODEL_RELATION_ID_METADATA_KEY.to_owned(),
            binding.storage_relation_id.clone(),
        );
        storage_metadata.insert(
            "codefabric.model.physical_binding_id".to_owned(),
            binding.physical_binding_id.clone(),
        );
        storage_metadata.insert(
            "codefabric.model.mapping_program_id".to_owned(),
            binding.mapping_program_id.clone(),
        );
        storage_metadata.insert(
            "codefabric.storage.column_mapping".to_owned(),
            column_mapping_name(binding.column_mapping_mode).to_owned(),
        );
        storage_metadata.insert(
            "codefabric.storage.deletion_vectors".to_owned(),
            deletion_vector_name(binding.deletion_vector_behavior).to_owned(),
        );

        let logical_schema = Arc::new(Schema::new_with_metadata(
            logical_arrow_fields,
            logical_metadata,
        ));
        let storage_schema = Arc::new(Schema::new_with_metadata(
            storage_arrow_fields,
            storage_metadata,
        ));

        if binding.field_bindings.len() != logical_fields.len() {
            return Err(model_compilation(
                physical_binding_id,
                format!(
                    "expected {} physical field bindings, received {}",
                    logical_fields.len(),
                    binding.field_bindings.len()
                ),
            ));
        }

        let mut mappings = Vec::with_capacity(logical_fields.len());
        let mut casts = Vec::with_capacity(logical_fields.len());
        let mut seen_logical = BTreeSet::new();
        let mut seen_storage = BTreeSet::new();
        for field_binding in &binding.field_bindings {
            let Some((logical_index, logical_field)) = logical_by_id
                .get(field_binding.logical_field_id.as_str())
                .copied()
            else {
                return Err(model_compilation(
                    physical_binding_id,
                    format!(
                        "physical field binding references unknown logical field {}",
                        field_binding.logical_field_id
                    ),
                ));
            };
            let Some((storage_index, storage_field)) = storage_by_id
                .get(field_binding.storage_field_id.as_str())
                .copied()
            else {
                return Err(model_compilation(
                    physical_binding_id,
                    format!(
                        "physical field binding references unknown storage field {}",
                        field_binding.storage_field_id
                    ),
                ));
            };
            if !seen_logical.insert(logical_index) {
                return Err(model_compilation(
                    physical_binding_id,
                    format!(
                        "logical field {} has more than one physical binding",
                        logical_field.field_id
                    ),
                ));
            }
            if !seen_storage.insert(storage_index) {
                return Err(model_compilation(
                    physical_binding_id,
                    format!(
                        "storage field {} is bound more than once",
                        storage_field.field_id
                    ),
                ));
            }

            mappings.push(FieldIndexMapping::new(
                logical_index,
                storage_index,
                field_binding.projection_index,
                field_binding.filter_index,
                field_binding.statistics_index,
            ));
            casts.push(FieldCastBinding {
                logical_field_id: Arc::from(logical_field.field_id.as_str()),
                storage_field_id: Arc::from(storage_field.field_id.as_str()),
                logical_index,
                storage_index,
                logical_data_type: logical_schema.field(logical_index).data_type().clone(),
                storage_data_type: storage_schema.field(storage_index).data_type().clone(),
            });
        }
        mappings.sort_by_key(|mapping| mapping.logical_index());
        casts.sort_by_key(FieldCastBinding::logical_index);

        let constraints = compile_model_constraints(
            &self.keys,
            &binding.logical_relation_id,
            &logical_by_id,
            physical_binding_id,
        )?;

        SchemaContract::try_new_compiled(
            binding.source_schema_identity.clone(),
            logical_relation.qualifier.clone(),
            logical_schema,
            storage_schema,
            mappings,
            casts,
            constraints,
            binding.compatibility,
            binding.column_mapping_mode,
            binding.deletion_vector_behavior,
        )
    }
}

fn project_model_relations(
    epoch: &ModelEpoch,
    catalog_name: &str,
) -> Result<Vec<ModelSchemaRelationRow>, SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::Relation);
    let relation_ids = model_utf8_column(batch, "relation_id")?;
    let schema_names = model_utf8_column(batch, "schema_name")?;
    let relation_names = model_utf8_column(batch, "relation_name")?;
    let semantic_roles = model_utf8_column(batch, "semantic_role")?;

    (0..batch.num_rows())
        .map(|row| {
            let relation_id = model_required_utf8(relation_ids, row, "relation.relation_id")?;
            let schema_name = model_required_utf8(schema_names, row, "relation.schema_name")?;
            let relation_name = model_required_utf8(relation_names, row, "relation.relation_name")?;
            let semantic_role = model_required_utf8(semantic_roles, row, "relation.semantic_role")?;
            if schema_name.trim().is_empty() || relation_name.trim().is_empty() {
                return Err(model_compilation(
                    relation_id,
                    "relation qualifier components must be non-empty".to_owned(),
                ));
            }
            Ok(ModelSchemaRelationRow {
                relation_id: relation_id.to_owned(),
                qualifier: TableReference::full(catalog_name, schema_name, relation_name),
                relation_metadata: HashMap::from([(
                    "codefabric.semantic.role".to_owned(),
                    semantic_role.to_owned(),
                )]),
            })
        })
        .collect()
}

fn project_model_semantic_types(
    epoch: &ModelEpoch,
) -> Result<Vec<ModelSchemaTypeRow>, SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::SemanticType);
    let semantic_type_ids = model_utf8_column(batch, "semantic_type_id")?;
    let logical_types = model_utf8_column(batch, "logical_type")?;
    let allows_null = model_bool_column(batch, "allows_null")?;

    (0..batch.num_rows())
        .map(|row| {
            let semantic_type_id =
                model_required_utf8(semantic_type_ids, row, "semantic_type.semantic_type_id")?;
            let logical_type =
                model_required_utf8(logical_types, row, "semantic_type.logical_type")?;
            let allows_null = model_required_bool(allows_null, row, "semantic_type.allows_null")?;
            Ok(ModelSchemaTypeRow {
                semantic_type_id: semantic_type_id.to_owned(),
                logical_data_type: parse_model_arrow_data_type(
                    semantic_type_id,
                    "logical_type",
                    logical_type,
                )?,
                allows_null,
            })
        })
        .collect()
}

fn project_model_fields(
    epoch: &ModelEpoch,
) -> Result<Vec<ModelSchemaFieldRow>, SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::Field);
    let field_ids = model_utf8_column(batch, "field_id")?;
    let relation_ids = model_utf8_column(batch, "relation_id")?;
    let field_names = model_utf8_column(batch, "field_name")?;
    let semantic_type_ids = model_utf8_column(batch, "semantic_type_id")?;
    let ordinals = model_u32_column(batch, "ordinal")?;
    let nullability = model_bool_column(batch, "nullable")?;
    let semantic_roles = model_utf8_column(batch, "semantic_role")?;

    (0..batch.num_rows())
        .map(|row| {
            Ok(ModelSchemaFieldRow {
                field_id: model_required_utf8(field_ids, row, "field.field_id")?.to_owned(),
                relation_id: model_required_utf8(relation_ids, row, "field.relation_id")?
                    .to_owned(),
                field_name: model_required_utf8(field_names, row, "field.field_name")?.to_owned(),
                semantic_type_id: model_required_utf8(
                    semantic_type_ids,
                    row,
                    "field.semantic_type_id",
                )?
                .to_owned(),
                ordinal: usize::try_from(model_required_u32(ordinals, row, "field.ordinal")?)
                    .expect("u32 always fits usize on supported targets"),
                nullable: model_required_bool(nullability, row, "field.nullable")?,
                semantic_role: model_required_utf8(semantic_roles, row, "field.semantic_role")?
                    .to_owned(),
                field_metadata: HashMap::new(),
            })
        })
        .collect()
}

fn project_model_keys(epoch: &ModelEpoch) -> Result<Vec<ModelSchemaKeyRow>, SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::Key);
    let key_ids = model_utf8_column(batch, "key_id")?;
    let relation_ids = model_utf8_column(batch, "relation_id")?;
    let field_ids = model_utf8_column(batch, "field_id")?;
    let ordinals = model_u32_column(batch, "ordinal")?;
    let key_kinds = model_utf8_column(batch, "key_kind")?;

    (0..batch.num_rows())
        .map(|row| {
            let key_id = model_required_utf8(key_ids, row, "key.key_id")?;
            let key_kind = match model_required_utf8(key_kinds, row, "key.key_kind")? {
                "primary" => ModelSchemaKeyKind::Primary,
                "unique" => ModelSchemaKeyKind::Unique,
                observed => {
                    return Err(model_compilation(
                        key_id,
                        format!("unsupported key kind {observed:?}"),
                    ));
                }
            };
            Ok(ModelSchemaKeyRow {
                key_id: key_id.to_owned(),
                relation_id: model_required_utf8(relation_ids, row, "key.relation_id")?.to_owned(),
                field_id: model_required_utf8(field_ids, row, "key.field_id")?.to_owned(),
                ordinal: usize::try_from(model_required_u32(ordinals, row, "key.ordinal")?)
                    .expect("u32 always fits usize on supported targets"),
                key_kind,
            })
        })
        .collect()
}

fn project_model_representations(
    epoch: &ModelEpoch,
) -> Result<Vec<ModelSchemaRepresentationRow>, SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::Representation);
    let representation_ids = model_utf8_column(batch, "representation_id")?;
    let semantic_type_ids = model_utf8_column(batch, "semantic_type_id")?;
    let arrow_data_types = model_utf8_column(batch, "arrow_data_type")?;
    let storage_encodings = model_utf8_column(batch, "storage_encoding")?;
    let metadata_classes = model_utf8_column(batch, "metadata_class")?;
    let extension_names = model_utf8_column(batch, "extension_name")?;
    let extension_metadata = model_utf8_column(batch, "extension_metadata")?;

    (0..batch.num_rows())
        .map(|row| {
            let representation_id =
                model_required_utf8(representation_ids, row, "representation.representation_id")?;
            Ok(ModelSchemaRepresentationRow {
                representation_id: representation_id.to_owned(),
                semantic_type_id: model_required_utf8(
                    semantic_type_ids,
                    row,
                    "representation.semantic_type_id",
                )?
                .to_owned(),
                storage_data_type: parse_model_arrow_data_type(
                    representation_id,
                    "arrow_data_type",
                    model_required_utf8(arrow_data_types, row, "representation.arrow_data_type")?,
                )?,
                storage_encoding: model_required_utf8(
                    storage_encodings,
                    row,
                    "representation.storage_encoding",
                )?
                .to_owned(),
                metadata_class: model_required_utf8(
                    metadata_classes,
                    row,
                    "representation.metadata_class",
                )?
                .to_owned(),
                extension_name: model_optional_utf8(extension_names, row),
                extension_metadata: model_optional_utf8(extension_metadata, row),
            })
        })
        .collect()
}

fn validate_model_physical_bindings(
    epoch: &ModelEpoch,
    physical_bindings: &[ModelPhysicalBindingRow],
) -> Result<(), SchemaContractError> {
    let batch = epoch.relations().batch(ModelRelation::PhysicalBinding);
    let binding_ids = model_utf8_column(batch, "physical_binding_id")?;
    let logical_relation_ids = model_utf8_column(batch, "logical_relation_id")?;
    let storage_relation_ids = model_utf8_column(batch, "storage_relation_id")?;
    let mapping_program_ids = model_utf8_column(batch, "mapping_program_id")?;
    let compatibility_modes = model_utf8_column(batch, "compatibility_mode")?;
    let mut expected = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let binding_id =
            model_required_utf8(binding_ids, row, "physical_binding.physical_binding_id")?;
        let authority = (
            model_required_utf8(
                logical_relation_ids,
                row,
                "physical_binding.logical_relation_id",
            )?,
            model_required_utf8(
                storage_relation_ids,
                row,
                "physical_binding.storage_relation_id",
            )?,
            model_required_utf8(
                mapping_program_ids,
                row,
                "physical_binding.mapping_program_id",
            )?,
            parse_model_compatibility(
                binding_id,
                model_required_utf8(
                    compatibility_modes,
                    row,
                    "physical_binding.compatibility_mode",
                )?,
            )?,
        );
        if expected.insert(binding_id, authority).is_some() {
            return Err(model_compilation(
                binding_id,
                "duplicate physical binding model row".to_owned(),
            ));
        }
    }

    let mut observed = BTreeSet::new();
    for binding in physical_bindings {
        if !observed.insert(binding.physical_binding_id.as_str()) {
            return Err(model_compilation(
                &binding.physical_binding_id,
                "duplicate resolved physical binding".to_owned(),
            ));
        }
        let Some((logical_relation_id, storage_relation_id, mapping_program_id, compatibility)) =
            expected.get(binding.physical_binding_id.as_str())
        else {
            return Err(model_compilation(
                &binding.physical_binding_id,
                "resolved binding has no authoritative model row".to_owned(),
            ));
        };
        if binding.logical_relation_id != *logical_relation_id
            || binding.storage_relation_id != *storage_relation_id
            || binding.mapping_program_id != *mapping_program_id
            || binding.compatibility != *compatibility
        {
            return Err(model_compilation(
                &binding.physical_binding_id,
                "resolved binding differs from its authoritative model row".to_owned(),
            ));
        }
    }

    let missing = expected
        .keys()
        .copied()
        .filter(|binding_id| !observed.contains(binding_id))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(model_compilation(
            epoch.model_epoch_id(),
            format!("physical binding programs produced no result for {missing:?}"),
        ));
    }
    Ok(())
}

fn parse_model_compatibility(
    identity: &str,
    value: &str,
) -> Result<SchemaCompatibility, SchemaContractError> {
    match value {
        "exact" => Ok(SchemaCompatibility::Exact),
        "contains" => Ok(SchemaCompatibility::Contains),
        observed => Err(model_compilation(
            identity,
            format!("unsupported schema compatibility {observed:?}"),
        )),
    }
}

fn parse_model_arrow_data_type(
    identity: &str,
    field: &str,
    value: &str,
) -> Result<DataType, SchemaContractError> {
    value.parse::<DataType>().or_else(|native_error| {
        let compatibility_alias = match value {
            "bool" | "boolean" => Some(DataType::Boolean),
            "i8" => Some(DataType::Int8),
            "i16" => Some(DataType::Int16),
            "i32" => Some(DataType::Int32),
            "i64" => Some(DataType::Int64),
            "u8" => Some(DataType::UInt8),
            "u16" => Some(DataType::UInt16),
            "u32" => Some(DataType::UInt32),
            "u64" => Some(DataType::UInt64),
            "f32" => Some(DataType::Float32),
            "f64" => Some(DataType::Float64),
            "utf8" => Some(DataType::Utf8),
            "large_utf8" => Some(DataType::LargeUtf8),
            "binary" => Some(DataType::Binary),
            "large_binary" => Some(DataType::LargeBinary),
            _ => None,
        };
        compatibility_alias.ok_or_else(|| {
            model_compilation(
                identity,
                format!("invalid Arrow {field} {value:?}: {native_error}"),
            )
        })
    })
}

fn model_column_index(batch: &RecordBatch, name: &str) -> Result<usize, SchemaContractError> {
    batch.schema().index_of(name).map_err(|error| {
        model_compilation(
            "model-epoch-projection",
            format!("missing model column {name}: {error}"),
        )
    })
}

fn model_utf8_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, SchemaContractError> {
    batch
        .column(model_column_index(batch, name)?)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| {
            model_compilation(
                "model-epoch-projection",
                format!("model column {name} is not Utf8"),
            )
        })
}

fn model_bool_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a BooleanArray, SchemaContractError> {
    batch
        .column(model_column_index(batch, name)?)
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| {
            model_compilation(
                "model-epoch-projection",
                format!("model column {name} is not Boolean"),
            )
        })
}

fn model_u32_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a UInt32Array, SchemaContractError> {
    batch
        .column(model_column_index(batch, name)?)
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| {
            model_compilation(
                "model-epoch-projection",
                format!("model column {name} is not UInt32"),
            )
        })
}

fn model_required_utf8<'a>(
    column: &'a StringArray,
    row: usize,
    location: &str,
) -> Result<&'a str, SchemaContractError> {
    if column.is_null(row) {
        return Err(model_compilation(
            "model-epoch-projection",
            format!("{location} is null"),
        ));
    }
    Ok(column.value(row))
}

fn model_optional_utf8(column: &StringArray, row: usize) -> Option<String> {
    (!column.is_null(row)).then(|| column.value(row).to_owned())
}

fn model_required_bool(
    column: &BooleanArray,
    row: usize,
    location: &str,
) -> Result<bool, SchemaContractError> {
    if column.is_null(row) {
        return Err(model_compilation(
            "model-epoch-projection",
            format!("{location} is null"),
        ));
    }
    Ok(column.value(row))
}

fn model_required_u32(
    column: &UInt32Array,
    row: usize,
    location: &str,
) -> Result<u32, SchemaContractError> {
    if column.is_null(row) {
        return Err(model_compilation(
            "model-epoch-projection",
            format!("{location} is null"),
        ));
    }
    Ok(column.value(row))
}

fn unique_row<'a, T: 'a>(
    mut rows: impl Iterator<Item = &'a T>,
    identity: &str,
    row_kind: &str,
) -> Result<&'a T, SchemaContractError> {
    let Some(row) = rows.next() else {
        return Err(model_compilation(
            identity,
            format!("missing {row_kind} row"),
        ));
    };
    if rows.next().is_some() {
        return Err(model_compilation(
            identity,
            format!("duplicate {row_kind} rows"),
        ));
    }
    Ok(row)
}

fn unique_by<'a, T: 'a>(
    rows: &'a [T],
    key: impl Fn(&'a T) -> &'a str,
    identity: &str,
    row_kind: &str,
) -> Result<BTreeMap<&'a str, &'a T>, SchemaContractError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let key = key(row);
        if indexed.insert(key, row).is_some() {
            return Err(model_compilation(
                identity,
                format!("duplicate {row_kind} row for {key}"),
            ));
        }
    }
    Ok(indexed)
}

fn relation_fields<'a>(
    rows: &'a [ModelSchemaFieldRow],
    relation_id: &str,
    identity: &str,
) -> Result<Vec<&'a ModelSchemaFieldRow>, SchemaContractError> {
    let mut fields = rows
        .iter()
        .filter(|row| row.relation_id == relation_id)
        .collect::<Vec<_>>();
    fields.sort_by_key(|row| row.ordinal);
    if fields.is_empty() {
        return Err(model_compilation(
            identity,
            format!("relation {relation_id} has no field rows"),
        ));
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (expected_ordinal, field) in fields.iter().enumerate() {
        if field.ordinal != expected_ordinal {
            return Err(model_compilation(
                identity,
                format!(
                    "relation {relation_id} field {} has ordinal {}, expected {expected_ordinal}",
                    field.field_id, field.ordinal
                ),
            ));
        }
        if !names.insert(field.field_name.as_str()) || !ids.insert(field.field_id.as_str()) {
            return Err(model_compilation(
                identity,
                format!("relation {relation_id} has a duplicate field ID or name"),
            ));
        }
    }
    Ok(fields)
}

fn compile_model_field(
    row: &ModelSchemaFieldRow,
    role: SchemaRole,
    semantic_types: &BTreeMap<&str, &ModelSchemaTypeRow>,
    representations: &BTreeMap<&str, &ModelSchemaRepresentationRow>,
    identity: &str,
) -> Result<Arc<Field>, SchemaContractError> {
    reject_reserved_field_metadata(&row.field_metadata, identity, &row.field_id)?;
    let semantic_type = semantic_types
        .get(row.semantic_type_id.as_str())
        .ok_or_else(|| {
            model_compilation(
                identity,
                format!(
                    "field {} references missing semantic type {}",
                    row.field_id, row.semantic_type_id
                ),
            )
        })?;
    let representation = representations
        .get(row.semantic_type_id.as_str())
        .ok_or_else(|| {
            model_compilation(
                identity,
                format!(
                    "field {} has no representation for semantic type {}",
                    row.field_id, row.semantic_type_id
                ),
            )
        })?;
    if row.nullable && !semantic_type.allows_null {
        return Err(model_compilation(
            identity,
            format!(
                "field {} is nullable but semantic type {} forbids null",
                row.field_id, row.semantic_type_id
            ),
        ));
    }
    if representation.storage_encoding.trim().is_empty()
        || representation.metadata_class.trim().is_empty()
    {
        return Err(model_compilation(
            identity,
            format!(
                "representation {} requires storage encoding and metadata class",
                representation.representation_id
            ),
        ));
    }

    let mut metadata = row.field_metadata.clone();
    metadata.insert(MODEL_FIELD_ID_METADATA_KEY.to_owned(), row.field_id.clone());
    metadata.insert(
        "codefabric.model.semantic_type_id".to_owned(),
        row.semantic_type_id.clone(),
    );
    metadata.insert(
        MODEL_SEMANTIC_ROLE_METADATA_KEY.to_owned(),
        row.semantic_role.clone(),
    );
    metadata.insert(
        "codefabric.model.representation_id".to_owned(),
        representation.representation_id.clone(),
    );
    metadata.insert(
        "codefabric.metadata.class".to_owned(),
        representation.metadata_class.clone(),
    );
    metadata.insert(
        "codefabric.storage.encoding".to_owned(),
        representation.storage_encoding.clone(),
    );
    if role == SchemaRole::Logical {
        if let Some(extension_name) = &representation.extension_name {
            metadata.insert(EXTENSION_TYPE_NAME_KEY.to_owned(), extension_name.clone());
        }
        if let Some(extension_metadata) = &representation.extension_metadata {
            metadata.insert(
                EXTENSION_TYPE_METADATA_KEY.to_owned(),
                extension_metadata.clone(),
            );
        }
    }
    let data_type = match role {
        SchemaRole::Logical => semantic_type.logical_data_type.clone(),
        SchemaRole::Storage => representation.storage_data_type.clone(),
    };
    Ok(Arc::new(
        Field::new(row.field_name.clone(), data_type, row.nullable).with_metadata(metadata),
    ))
}

fn reject_reserved_relation_metadata(
    metadata: &HashMap<String, String>,
    identity: &str,
    relation_id: &str,
) -> Result<(), SchemaContractError> {
    reject_reserved_metadata_key(
        metadata,
        identity,
        format!("relation {relation_id}"),
        |key| {
            key.starts_with("codefabric.model.")
                || matches!(
                    key,
                    "codefabric.storage.column_mapping" | "codefabric.storage.deletion_vectors"
                )
        },
    )
}

fn reject_reserved_field_metadata(
    metadata: &HashMap<String, String>,
    identity: &str,
    field_id: &str,
) -> Result<(), SchemaContractError> {
    reject_reserved_metadata_key(metadata, identity, format!("field {field_id}"), |key| {
        key.starts_with("codefabric.model.")
            || matches!(
                key,
                "codefabric.metadata.class" | "codefabric.storage.encoding"
            )
            || key == EXTENSION_TYPE_NAME_KEY
            || key == EXTENSION_TYPE_METADATA_KEY
    })
}

fn reject_reserved_metadata_key(
    metadata: &HashMap<String, String>,
    identity: &str,
    location: String,
    is_reserved: impl Fn(&str) -> bool,
) -> Result<(), SchemaContractError> {
    if let Some(key) = metadata.keys().find(|key| is_reserved(key)) {
        return Err(SchemaContractError::ReservedModelMetadataKey {
            identity: identity.to_owned(),
            location,
            key: key.clone(),
        });
    }
    Ok(())
}

fn compile_model_constraints(
    rows: &[ModelSchemaKeyRow],
    relation_id: &str,
    fields: &BTreeMap<&str, (usize, &ModelSchemaFieldRow)>,
    identity: &str,
) -> Result<Constraints, SchemaContractError> {
    let mut grouped: BTreeMap<&str, Vec<&ModelSchemaKeyRow>> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.relation_id == relation_id) {
        grouped.entry(row.key_id.as_str()).or_default().push(row);
    }
    let mut constraints = Vec::with_capacity(grouped.len());
    for (key_id, mut key_rows) in grouped {
        key_rows.sort_by_key(|row| row.ordinal);
        let Some(first) = key_rows.first() else {
            continue;
        };
        let key_kind = first.key_kind;
        let mut indices = Vec::with_capacity(key_rows.len());
        for (expected_ordinal, row) in key_rows.into_iter().enumerate() {
            if row.ordinal != expected_ordinal || row.key_kind != key_kind {
                return Err(model_compilation(
                    identity,
                    format!("key {key_id} has inconsistent kind or ordinal"),
                ));
            }
            let Some((index, field)) = fields.get(row.field_id.as_str()).copied() else {
                return Err(model_compilation(
                    identity,
                    format!("key {key_id} references unknown field {}", row.field_id),
                ));
            };
            if key_kind == ModelSchemaKeyKind::Primary && field.nullable {
                return Err(model_compilation(
                    identity,
                    format!(
                        "primary key {key_id} contains nullable field {}",
                        row.field_id
                    ),
                ));
            }
            indices.push(index);
        }
        constraints.push(match key_kind {
            ModelSchemaKeyKind::Primary => Constraint::PrimaryKey(indices),
            ModelSchemaKeyKind::Unique => Constraint::Unique(indices),
        });
    }
    Ok(Constraints::new_unverified(constraints))
}

fn model_compilation(identity: &str, reason: String) -> SchemaContractError {
    SchemaContractError::ModelCompilation {
        identity: identity.to_owned(),
        reason,
    }
}

const fn column_mapping_name(mode: ColumnMappingMode) -> &'static str {
    match mode {
        ColumnMappingMode::Positional => "position",
        ColumnMappingMode::Name => "name",
        ColumnMappingMode::FieldId => "field-id",
    }
}

const fn deletion_vector_name(behavior: DeletionVectorBehavior) -> &'static str {
    match behavior {
        DeletionVectorBehavior::Forbidden => "forbidden",
        DeletionVectorBehavior::AppliedByProvider => "applied-by-provider",
        DeletionVectorBehavior::ExposedVisibilityColumn => "exposed-visibility-column",
    }
}

type ValidatedMappings = (Arc<[FieldIndexMapping]>, Arc<[Option<usize>]>);

fn validate_cast_bindings(
    casts: &[FieldCastBinding],
    mappings: &[FieldIndexMapping],
    logical_schema: &SchemaRef,
    storage_schema: &SchemaRef,
    logical_model_index: Option<&ModelSchemaIndex>,
    storage_model_index: Option<&ModelSchemaIndex>,
) -> Result<(), SchemaContractError> {
    if casts.len() != mappings.len() {
        return Err(model_compilation(
            "field-cast-bindings",
            format!(
                "expected {} cast bindings, received {}",
                mappings.len(),
                casts.len()
            ),
        ));
    }
    for (logical_index, (cast, mapping)) in casts.iter().zip(mappings).enumerate() {
        if cast.logical_index != logical_index
            || cast.logical_index != mapping.logical_index()
            || cast.storage_index != mapping.storage_index()
            || &cast.logical_data_type != logical_schema.field(logical_index).data_type()
            || &cast.storage_data_type != storage_schema.field(cast.storage_index).data_type()
        {
            return Err(model_compilation(
                cast.logical_field_id(),
                "cast binding does not match the compiled field/index mapping".to_owned(),
            ));
        }
        if let Some(model_index) = logical_model_index {
            let expected = &model_index.field_ids_by_index[logical_index];
            if expected.as_ref() != cast.logical_field_id() {
                return Err(SchemaContractError::ModelFieldBindingMismatch {
                    role: SchemaRole::Logical,
                    field_index: logical_index,
                    expected_field_id: Arc::clone(expected),
                    actual_field_id: Arc::clone(&cast.logical_field_id),
                });
            }
        }
        if let Some(model_index) = storage_model_index {
            let expected = &model_index.field_ids_by_index[cast.storage_index];
            if expected.as_ref() != cast.storage_field_id() {
                return Err(SchemaContractError::ModelFieldBindingMismatch {
                    role: SchemaRole::Storage,
                    field_index: cast.storage_index,
                    expected_field_id: Arc::clone(expected),
                    actual_field_id: Arc::clone(&cast.storage_field_id),
                });
            }
        }
        if !can_cast_types(&cast.logical_data_type, &cast.storage_data_type) {
            return Err(SchemaContractError::UnsupportedStorageCast {
                logical_field_id: cast.logical_field_id().to_owned(),
                storage_field_id: cast.storage_field_id().to_owned(),
                logical_type: cast.logical_data_type.clone(),
                storage_type: cast.storage_data_type.clone(),
            });
        }
        if !can_cast_types(&cast.storage_data_type, &cast.logical_data_type) {
            return Err(SchemaContractError::UnsupportedRestorationCast {
                logical_field_id: cast.logical_field_id().to_owned(),
                storage_field_id: cast.storage_field_id().to_owned(),
                storage_type: cast.storage_data_type.clone(),
                logical_type: cast.logical_data_type.clone(),
            });
        }
    }
    Ok(())
}

fn validate_mappings(
    logical_field_count: usize,
    storage_field_count: usize,
    mappings: Vec<FieldIndexMapping>,
) -> Result<ValidatedMappings, SchemaContractError> {
    if mappings.len() != logical_field_count {
        return Err(SchemaContractError::MappingCount {
            expected: logical_field_count,
            actual: mappings.len(),
        });
    }

    let mut ordered = vec![None; logical_field_count];
    let mut reverse_by_purpose = [
        vec![None; storage_field_count],
        vec![None; storage_field_count],
        vec![None; storage_field_count],
        vec![None; storage_field_count],
    ];
    let purposes = [
        IndexPurpose::Storage,
        IndexPurpose::Projection,
        IndexPurpose::Filter,
        IndexPurpose::Statistics,
    ];

    for mapping in mappings {
        if mapping.logical >= logical_field_count {
            return Err(SchemaContractError::LogicalMappingOutOfBounds {
                logical_index: mapping.logical,
                logical_field_count,
            });
        }
        if ordered[mapping.logical].replace(mapping).is_some() {
            return Err(SchemaContractError::DuplicateLogicalMapping {
                logical_index: mapping.logical,
            });
        }

        for (purpose_offset, purpose) in purposes.iter().copied().enumerate() {
            let mapped_index = mapping.index_for(purpose);
            if mapped_index >= storage_field_count {
                return Err(SchemaContractError::PhysicalMappingOutOfBounds {
                    purpose,
                    logical_index: mapping.logical,
                    mapped_index,
                    storage_field_count,
                });
            }
            if let Some(first_logical_index) =
                reverse_by_purpose[purpose_offset][mapped_index].replace(mapping.logical)
            {
                return Err(SchemaContractError::DuplicatePhysicalMapping {
                    purpose,
                    mapped_index,
                    first_logical_index,
                    second_logical_index: mapping.logical,
                });
            }
        }
    }

    let ordered = ordered
        .into_iter()
        .map(|mapping| mapping.expect("mapping count and uniqueness guarantee totality"))
        .collect::<Vec<_>>();
    let storage_to_logical = reverse_by_purpose[0].clone();
    Ok((ordered.into(), storage_to_logical.into()))
}

fn validate_extension_metadata(
    role: SchemaRole,
    schema: &Schema,
) -> Result<(), SchemaContractError> {
    for field in schema.fields() {
        validate_field_extension_metadata(role, field.name(), field)?;
    }
    Ok(())
}

fn validate_field_extension_metadata(
    role: SchemaRole,
    path: &str,
    field: &Field,
) -> Result<(), SchemaContractError> {
    let extension_name = field.metadata().get(EXTENSION_TYPE_NAME_KEY);
    let extension_metadata = field.metadata().get(EXTENSION_TYPE_METADATA_KEY);
    if extension_metadata.is_some() && extension_name.is_none() {
        return Err(SchemaContractError::InvalidExtensionMetadata {
            role,
            path: path.to_owned(),
            reason: "extension metadata is present without an extension name",
        });
    }
    if extension_name.is_some_and(String::is_empty) {
        return Err(SchemaContractError::InvalidExtensionMetadata {
            role,
            path: path.to_owned(),
            reason: "extension name must not be empty",
        });
    }
    validate_data_type_extension_metadata(role, path, field.data_type())
}

fn validate_data_type_extension_metadata(
    role: SchemaRole,
    path: &str,
    data_type: &DataType,
) -> Result<(), SchemaContractError> {
    match data_type {
        DataType::List(field)
        | DataType::LargeList(field)
        | DataType::ListView(field)
        | DataType::LargeListView(field)
        | DataType::FixedSizeList(field, _)
        | DataType::Map(field, _) => {
            validate_field_extension_metadata(role, &child_path(path, field.name()), field)
        }
        DataType::Struct(fields) => {
            for field in fields {
                validate_field_extension_metadata(role, &child_path(path, field.name()), field)?;
            }
            Ok(())
        }
        DataType::Union(fields, _) => {
            for (_, field) in fields.iter() {
                validate_field_extension_metadata(role, &child_path(path, field.name()), field)?;
            }
            Ok(())
        }
        DataType::Dictionary(key, value) => {
            validate_data_type_extension_metadata(role, &format!("{path}.dictionary.key"), key)?;
            validate_data_type_extension_metadata(role, &format!("{path}.dictionary.value"), value)
        }
        DataType::RunEndEncoded(run_ends, values) => {
            validate_field_extension_metadata(role, &child_path(path, run_ends.name()), run_ends)?;
            validate_field_extension_metadata(role, &child_path(path, values.name()), values)
        }
        DataType::Null
        | DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64
        | DataType::Timestamp(_, _)
        | DataType::Date32
        | DataType::Date64
        | DataType::Time32(_)
        | DataType::Time64(_)
        | DataType::Duration(_)
        | DataType::Interval(_)
        | DataType::Binary
        | DataType::FixedSizeBinary(_)
        | DataType::LargeBinary
        | DataType::BinaryView
        | DataType::Utf8
        | DataType::LargeUtf8
        | DataType::Utf8View
        | DataType::Decimal32(_, _)
        | DataType::Decimal64(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => Ok(()),
    }
}

fn schemas_compatible(
    expected: &Schema,
    actual: &Schema,
    compatibility: SchemaCompatibility,
) -> bool {
    let arrow_compatible = match compatibility {
        SchemaCompatibility::Exact => actual == expected,
        SchemaCompatibility::Contains => actual.contains(expected),
    };
    arrow_compatible && extension_metadata_equal(expected, actual)
}

fn extension_metadata_equal(expected: &Schema, actual: &Schema) -> bool {
    expected.fields().len() == actual.fields().len()
        && expected
            .fields()
            .iter()
            .zip(actual.fields())
            .all(|(expected, actual)| field_extension_metadata_equal(expected, actual))
}

fn field_extension_metadata_equal(expected: &Field, actual: &Field) -> bool {
    extension_value(expected, EXTENSION_TYPE_NAME_KEY)
        == extension_value(actual, EXTENSION_TYPE_NAME_KEY)
        && extension_value(expected, EXTENSION_TYPE_METADATA_KEY)
            == extension_value(actual, EXTENSION_TYPE_METADATA_KEY)
        && data_type_extension_metadata_equal(expected.data_type(), actual.data_type())
}

fn data_type_extension_metadata_equal(expected: &DataType, actual: &DataType) -> bool {
    match (expected, actual) {
        (DataType::List(expected), DataType::List(actual))
        | (DataType::LargeList(expected), DataType::LargeList(actual))
        | (DataType::ListView(expected), DataType::ListView(actual))
        | (DataType::LargeListView(expected), DataType::LargeListView(actual)) => {
            field_extension_metadata_equal(expected, actual)
        }
        (
            DataType::FixedSizeList(expected, expected_size),
            DataType::FixedSizeList(actual, actual_size),
        ) => expected_size == actual_size && field_extension_metadata_equal(expected, actual),
        (DataType::Map(expected, expected_sorted), DataType::Map(actual, actual_sorted)) => {
            expected_sorted == actual_sorted && field_extension_metadata_equal(expected, actual)
        }
        (DataType::Struct(expected), DataType::Struct(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| field_extension_metadata_equal(expected, actual))
        }
        (DataType::Union(expected, expected_mode), DataType::Union(actual, actual_mode)) => {
            expected_mode == actual_mode
                && expected.len() == actual.len()
                && expected.iter().all(|(expected_id, expected_field)| {
                    actual.iter().any(|(actual_id, actual_field)| {
                        expected_id == actual_id
                            && field_extension_metadata_equal(expected_field, actual_field)
                    })
                })
        }
        (
            DataType::Dictionary(expected_key, expected_value),
            DataType::Dictionary(actual_key, actual_value),
        ) => {
            data_type_extension_metadata_equal(expected_key, actual_key)
                && data_type_extension_metadata_equal(expected_value, actual_value)
        }
        (
            DataType::RunEndEncoded(expected_run_ends, expected_values),
            DataType::RunEndEncoded(actual_run_ends, actual_values),
        ) => {
            field_extension_metadata_equal(expected_run_ends, actual_run_ends)
                && field_extension_metadata_equal(expected_values, actual_values)
        }
        _ => expected == actual,
    }
}

fn diagnose_schema(
    expected: &Schema,
    actual: &Schema,
    compatibility: SchemaCompatibility,
) -> SchemaDifference {
    if expected.fields().len() != actual.fields().len() {
        return SchemaDifference::new(
            "$",
            SchemaDifferenceKind::FieldCount {
                expected: expected.fields().len(),
                actual: actual.fields().len(),
            },
        );
    }

    if let Some((key, expected_value, actual_value)) =
        metadata_difference(expected.metadata(), actual.metadata(), compatibility)
    {
        return SchemaDifference::new(
            "$",
            SchemaDifferenceKind::SchemaMetadata {
                key,
                expected: expected_value,
                actual: actual_value,
            },
        );
    }

    for (expected_field, actual_field) in expected.fields().iter().zip(actual.fields()) {
        let path = child_path("$", expected_field.name());
        if let Some(difference) = diagnose_field(expected_field, actual_field, compatibility, &path)
        {
            return difference;
        }
    }

    SchemaDifference::new("$", SchemaDifferenceKind::NativeCompatibility)
}

fn diagnose_field(
    expected: &Field,
    actual: &Field,
    compatibility: SchemaCompatibility,
    path: &str,
) -> Option<SchemaDifference> {
    if expected.name() != actual.name() {
        return Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::FieldName {
                expected: expected.name().clone(),
                actual: actual.name().clone(),
            },
        ));
    }

    let nullability_compatible = match compatibility {
        SchemaCompatibility::Exact => expected.is_nullable() == actual.is_nullable(),
        SchemaCompatibility::Contains => actual.is_nullable() || !expected.is_nullable(),
    };
    if !nullability_compatible {
        return Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::Nullability {
                expected: expected.is_nullable(),
                actual: actual.is_nullable(),
            },
        ));
    }

    if expected.dict_is_ordered() != actual.dict_is_ordered() {
        return Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::DictionaryOrdering {
                expected: expected.dict_is_ordered(),
                actual: actual.dict_is_ordered(),
            },
        ));
    }

    for key in [EXTENSION_TYPE_NAME_KEY, EXTENSION_TYPE_METADATA_KEY] {
        let expected_value = extension_value(expected, key);
        let actual_value = extension_value(actual, key);
        if expected_value != actual_value {
            return Some(SchemaDifference::new(
                path,
                SchemaDifferenceKind::ExtensionMetadata {
                    key: key.to_owned(),
                    expected: expected_value.map(ToOwned::to_owned),
                    actual: actual_value.map(ToOwned::to_owned),
                },
            ));
        }
    }

    if let Some((key, expected_value, actual_value)) =
        metadata_difference(expected.metadata(), actual.metadata(), compatibility)
    {
        return Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::FieldMetadata {
                key,
                expected: expected_value,
                actual: actual_value,
            },
        ));
    }

    diagnose_data_type(
        expected.data_type(),
        actual.data_type(),
        compatibility,
        path,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive Arrow nested-type diagnostic keeps mismatch paths precise"
)]
fn diagnose_data_type(
    expected: &DataType,
    actual: &DataType,
    compatibility: SchemaCompatibility,
    path: &str,
) -> Option<SchemaDifference> {
    let compatible = match compatibility {
        SchemaCompatibility::Exact => actual == expected,
        SchemaCompatibility::Contains => actual.contains(expected),
    } && data_type_extension_metadata_equal(expected, actual);
    if compatible {
        return None;
    }

    match (expected, actual) {
        (DataType::List(expected), DataType::List(actual))
        | (DataType::LargeList(expected), DataType::LargeList(actual))
        | (DataType::ListView(expected), DataType::ListView(actual))
        | (DataType::LargeListView(expected), DataType::LargeListView(actual)) => diagnose_field(
            expected,
            actual,
            compatibility,
            &child_path(path, expected.name()),
        ),
        (
            DataType::FixedSizeList(expected, expected_size),
            DataType::FixedSizeList(actual, actual_size),
        ) if expected_size == actual_size => diagnose_field(
            expected,
            actual,
            compatibility,
            &child_path(path, expected.name()),
        ),
        (DataType::Map(expected, expected_sorted), DataType::Map(actual, actual_sorted))
            if expected_sorted == actual_sorted =>
        {
            diagnose_field(
                expected,
                actual,
                compatibility,
                &child_path(path, expected.name()),
            )
        }
        (DataType::Struct(expected), DataType::Struct(actual)) => {
            if expected.len() != actual.len() {
                return Some(SchemaDifference::new(
                    path,
                    SchemaDifferenceKind::FieldCount {
                        expected: expected.len(),
                        actual: actual.len(),
                    },
                ));
            }
            for (expected_field, actual_field) in expected.iter().zip(actual) {
                if let Some(difference) = diagnose_field(
                    expected_field,
                    actual_field,
                    compatibility,
                    &child_path(path, expected_field.name()),
                ) {
                    return Some(difference);
                }
            }
            None
        }
        (
            DataType::Dictionary(expected_key, expected_value),
            DataType::Dictionary(actual_key, actual_value),
        ) => {
            let key_compatible = match compatibility {
                SchemaCompatibility::Exact => actual_key == expected_key,
                SchemaCompatibility::Contains => actual_key.contains(expected_key),
            };
            if !key_compatible {
                return Some(SchemaDifference::new(
                    format!("{path}.dictionary.key"),
                    SchemaDifferenceKind::DictionaryKeyType {
                        expected: expected_key.as_ref().clone(),
                        actual: actual_key.as_ref().clone(),
                    },
                ));
            }
            if diagnose_data_type(
                expected_value,
                actual_value,
                compatibility,
                &format!("{path}.dictionary.value"),
            )
            .is_some()
            {
                return Some(SchemaDifference::new(
                    format!("{path}.dictionary.value"),
                    SchemaDifferenceKind::DictionaryValueType {
                        expected: expected_value.as_ref().clone(),
                        actual: actual_value.as_ref().clone(),
                    },
                ));
            }
            None
        }
        (
            DataType::RunEndEncoded(expected_run_ends, expected_values),
            DataType::RunEndEncoded(actual_run_ends, actual_values),
        ) => diagnose_field(
            expected_run_ends,
            actual_run_ends,
            compatibility,
            &child_path(path, expected_run_ends.name()),
        )
        .or_else(|| {
            diagnose_field(
                expected_values,
                actual_values,
                compatibility,
                &child_path(path, expected_values.name()),
            )
        }),
        _ => Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::DataType {
                expected: expected.clone(),
                actual: actual.clone(),
            },
        )),
    }
    .or_else(|| {
        Some(SchemaDifference::new(
            path,
            SchemaDifferenceKind::DataType {
                expected: expected.clone(),
                actual: actual.clone(),
            },
        ))
    })
}

fn metadata_difference(
    expected: &std::collections::HashMap<String, String>,
    actual: &std::collections::HashMap<String, String>,
    compatibility: SchemaCompatibility,
) -> Option<(String, Option<String>, Option<String>)> {
    let keys = match compatibility {
        SchemaCompatibility::Exact => expected
            .keys()
            .chain(actual.keys())
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        SchemaCompatibility::Contains => expected.keys().map(String::as_str).collect(),
    };

    keys.into_iter().find_map(|key| {
        let expected_value = expected.get(key);
        let actual_value = actual.get(key);
        (expected_value != actual_value).then(|| {
            (
                key.to_owned(),
                expected_value.cloned(),
                actual_value.cloned(),
            )
        })
    })
}

fn extension_value<'a>(field: &'a Field, key: &str) -> Option<&'a str> {
    field.metadata().get(key).map(String::as_str)
}

fn child_path(parent: &str, child: &str) -> String {
    format!("{parent}.{child}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{BinaryArray, FixedSizeBinaryArray, StringArray};
    use arrow_schema::{DataType, Field, Fields, Schema};
    use datafusion::physical_plan::empty::EmptyExec;

    use super::*;
    use crate::relational_model::{
        BootstrapMetamodel, FabricCompilerRelease, IntrinsicInstaller, ModelDecision,
        ModelMigration, ModelOperation, ModelRowBuilder, ReplayEngine,
    };

    const EXTENSION_NAME: &str = "codefabric.test_id";
    const EXTENSION_METADATA: &str = "{\"width\":16}";

    fn id_field(width: i32, metadata: &str) -> Field {
        Field::new("id", DataType::FixedSizeBinary(width), false).with_metadata(HashMap::from([
            (
                EXTENSION_TYPE_NAME_KEY.to_owned(),
                EXTENSION_NAME.to_owned(),
            ),
            (EXTENSION_TYPE_METADATA_KEY.to_owned(), metadata.to_owned()),
        ]))
    }

    fn payload_field(code_nullable: bool) -> Field {
        Field::new(
            "payload",
            DataType::Struct(Fields::from(vec![Field::new(
                "code",
                DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
                code_nullable,
            )])),
            true,
        )
    }

    fn logical_schema() -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            vec![
                Arc::new(id_field(16, EXTENSION_METADATA)),
                Arc::new(payload_field(false)),
            ],
            HashMap::from([("model.schema".to_owned(), "relation.v1".to_owned())]),
        ))
    }

    fn storage_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Arc::new(payload_field(true)),
            Arc::new(Field::new("id_bytes", DataType::Binary, false)),
            Arc::new(Field::new("row_version", DataType::UInt64, false)),
        ]))
    }

    fn contract() -> SchemaContract {
        SchemaContract::try_new(
            "model.relation.v1",
            TableReference::full("codefabric", "cpg_serving", "relation"),
            logical_schema(),
            storage_schema(),
            vec![
                FieldIndexMapping::direct(0, 1),
                FieldIndexMapping::new(1, 0, 0, 0, 0),
            ],
        )
        .expect("valid contract")
    }

    fn model_rows() -> SchemaContractModelRows {
        let logical_relation = "relation.logical".to_owned();
        let storage_relation = "relation.storage".to_owned();
        SchemaContractModelRows {
            relations: vec![
                ModelSchemaRelationRow {
                    relation_id: logical_relation.clone(),
                    qualifier: TableReference::full("codefabric", "cpg_serving", "entity"),
                    relation_metadata: HashMap::from([(
                        "model.schema".to_owned(),
                        "entity.v2".to_owned(),
                    )]),
                },
                ModelSchemaRelationRow {
                    relation_id: storage_relation.clone(),
                    qualifier: TableReference::partial("storage", "entity"),
                    relation_metadata: HashMap::from([(
                        "storage.schema".to_owned(),
                        "entity.delta.v1".to_owned(),
                    )]),
                },
            ],
            semantic_types: vec![
                ModelSchemaTypeRow {
                    semantic_type_id: "type.id16".to_owned(),
                    logical_data_type: DataType::FixedSizeBinary(16),
                    allows_null: false,
                },
                ModelSchemaTypeRow {
                    semantic_type_id: "type.text".to_owned(),
                    logical_data_type: DataType::Utf8,
                    allows_null: true,
                },
            ],
            fields: vec![
                ModelSchemaFieldRow {
                    field_id: "field.logical.id".to_owned(),
                    relation_id: logical_relation.clone(),
                    field_name: "entity_id".to_owned(),
                    semantic_type_id: "type.id16".to_owned(),
                    ordinal: 0,
                    nullable: false,
                    semantic_role: "canonical-id".to_owned(),
                    field_metadata: HashMap::new(),
                },
                ModelSchemaFieldRow {
                    field_id: "field.logical.name".to_owned(),
                    relation_id: logical_relation.clone(),
                    field_name: "name".to_owned(),
                    semantic_type_id: "type.text".to_owned(),
                    ordinal: 1,
                    nullable: true,
                    semantic_role: "semantic-text".to_owned(),
                    field_metadata: HashMap::new(),
                },
                ModelSchemaFieldRow {
                    field_id: "field.storage.id".to_owned(),
                    relation_id: storage_relation.clone(),
                    field_name: "entity_id_bytes".to_owned(),
                    semantic_type_id: "type.id16".to_owned(),
                    ordinal: 0,
                    nullable: false,
                    semantic_role: "storage-id".to_owned(),
                    field_metadata: HashMap::new(),
                },
                ModelSchemaFieldRow {
                    field_id: "field.storage.name".to_owned(),
                    relation_id: storage_relation.clone(),
                    field_name: "name".to_owned(),
                    semantic_type_id: "type.text".to_owned(),
                    ordinal: 1,
                    nullable: true,
                    semantic_role: "storage-value".to_owned(),
                    field_metadata: HashMap::new(),
                },
            ],
            keys: vec![ModelSchemaKeyRow {
                key_id: "key.entity".to_owned(),
                relation_id: logical_relation.clone(),
                field_id: "field.logical.id".to_owned(),
                ordinal: 0,
                key_kind: ModelSchemaKeyKind::Primary,
            }],
            representations: vec![
                ModelSchemaRepresentationRow {
                    representation_id: "representation.id16.binary".to_owned(),
                    semantic_type_id: "type.id16".to_owned(),
                    storage_data_type: DataType::Binary,
                    storage_encoding: "delta.binary".to_owned(),
                    metadata_class: "contractual".to_owned(),
                    extension_name: Some("codefabric.id16".to_owned()),
                    extension_metadata: Some("{\"width\":16}".to_owned()),
                },
                ModelSchemaRepresentationRow {
                    representation_id: "representation.text.utf8".to_owned(),
                    semantic_type_id: "type.text".to_owned(),
                    storage_data_type: DataType::Utf8,
                    storage_encoding: "delta.utf8".to_owned(),
                    metadata_class: "contractual".to_owned(),
                    extension_name: None,
                    extension_metadata: None,
                },
            ],
            physical_bindings: vec![ModelPhysicalBindingRow {
                physical_binding_id: "binding.entity.delta".to_owned(),
                mapping_program_id: "program.binding.entity.delta".to_owned(),
                source_schema_identity: "model:entity:v2/delta:v1".to_owned(),
                logical_relation_id: logical_relation,
                storage_relation_id: storage_relation,
                compatibility: SchemaCompatibility::Exact,
                column_mapping_mode: ColumnMappingMode::FieldId,
                deletion_vector_behavior: DeletionVectorBehavior::AppliedByProvider,
                field_bindings: vec![
                    ModelPhysicalFieldBindingRow {
                        logical_field_id: "field.logical.id".to_owned(),
                        storage_field_id: "field.storage.id".to_owned(),
                        projection_index: 0,
                        filter_index: 0,
                        statistics_index: 0,
                    },
                    ModelPhysicalFieldBindingRow {
                        logical_field_id: "field.logical.name".to_owned(),
                        storage_field_id: "field.storage.name".to_owned(),
                        projection_index: 1,
                        filter_index: 1,
                        statistics_index: 1,
                    },
                ],
            }],
        }
    }

    fn projectable_model_epoch() -> ModelEpoch {
        let metamodel = BootstrapMetamodel::new();
        let mut operations = Vec::new();
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::SemanticType)
                .value("semantic_type_id", "type.entity-id")
                .unwrap()
                .value("name", "entity identifier")
                .unwrap()
                .value("logical_type", "FixedSizeBinary(16)")
                .unwrap()
                .value("allows_null", false)
                .unwrap()
                .build(&metamodel)
                .unwrap(),
        ));
        for (relation_id, schema_name, relation_name, semantic_role) in [
            (
                "relation.entity.logical",
                "cpg_serving",
                "entity",
                "logical",
            ),
            (
                "relation.entity.storage",
                "cpg_storage",
                "entity",
                "storage",
            ),
        ] {
            operations.push(ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Relation)
                    .value("relation_id", relation_id)
                    .unwrap()
                    .value("schema_name", schema_name)
                    .unwrap()
                    .value("relation_name", relation_name)
                    .unwrap()
                    .value("semantic_role", semantic_role)
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ));
        }
        for (field_id, relation_id, field_name, semantic_role) in [
            (
                "field.entity.logical-id",
                "relation.entity.logical",
                "entity_id",
                "canonical-id",
            ),
            (
                "field.entity.storage-id",
                "relation.entity.storage",
                "entity_id_bytes",
                "storage-id",
            ),
        ] {
            operations.push(ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value("field_id", field_id)
                    .unwrap()
                    .value("relation_id", relation_id)
                    .unwrap()
                    .value("field_name", field_name)
                    .unwrap()
                    .value("semantic_type_id", "type.entity-id")
                    .unwrap()
                    .value("ordinal", 0_u32)
                    .unwrap()
                    .value("nullable", false)
                    .unwrap()
                    .value("semantic_role", semantic_role)
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ));
        }
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::Key)
                .value("key_id", "key.entity.primary")
                .unwrap()
                .value("relation_id", "relation.entity.logical")
                .unwrap()
                .value("field_id", "field.entity.logical-id")
                .unwrap()
                .value("ordinal", 0_u32)
                .unwrap()
                .value("key_kind", "primary")
                .unwrap()
                .build(&metamodel)
                .unwrap(),
        ));
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::Representation)
                .value("representation_id", "representation.entity-id.delta")
                .unwrap()
                .value("semantic_type_id", "type.entity-id")
                .unwrap()
                .value("arrow_data_type", "Binary")
                .unwrap()
                .value("storage_encoding", "delta.binary")
                .unwrap()
                .value("metadata_class", "contractual")
                .unwrap()
                .value("extension_name", "codefabric.entity_id")
                .unwrap()
                .value("extension_metadata", "{\"width\":16}")
                .unwrap()
                .build(&metamodel)
                .unwrap(),
        ));
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::Program)
                .value("program_id", "program.entity.delta-binding")
                .unwrap()
                .value("name", "entity Delta binding")
                .unwrap()
                .value("program_kind", "physical-binding")
                .unwrap()
                .null("result_semantic_type_id")
                .unwrap()
                .build(&metamodel)
                .unwrap(),
        ));
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::PhysicalBinding)
                .value("physical_binding_id", "binding.entity.delta")
                .unwrap()
                .value("logical_relation_id", "relation.entity.logical")
                .unwrap()
                .value("storage_relation_id", "relation.entity.storage")
                .unwrap()
                .value("mapping_program_id", "program.entity.delta-binding")
                .unwrap()
                .value("compatibility_mode", "exact")
                .unwrap()
                .build(&metamodel)
                .unwrap(),
        ));

        let decision = ModelDecision::new(
            "decision.schema-contract",
            "schema-owner",
            "schema projection test",
            "install a complete logical and physical schema model",
            operations,
        )
        .unwrap();
        let migration = ModelMigration::new(
            "migration.schema-contract.1",
            None,
            "model.bootstrap.schema.release",
            "model.schema.epoch.1",
            1,
            "schema-owner",
            vec![decision],
        )
        .unwrap();
        let release = FabricCompilerRelease::builder(
            "schema.release",
            "source:schema.release",
            "build:schema.release",
        )
        .with_abis(1, 1, 1)
        .with_intrinsic_package("schema.intrinsics")
        .add_dependency("arrow", "59.2.0")
        .unwrap()
        .add_dependency("datafusion", "55.0.0")
        .unwrap()
        .add_dependency("deltalake", "43a0cf10")
        .unwrap()
        .add_provider_schema("delta", "schema.v1")
        .unwrap()
        .with_policy_and_configuration("policy.v1", "config.v1")
        .add_toolchain("rust", "1.95.0")
        .unwrap()
        .add_wire_contract("schema.test")
        .unwrap()
        .build()
        .unwrap();
        ReplayEngine::new(
            release,
            IntrinsicInstaller::new("schema.intrinsics", "schema.impl").unwrap(),
        )
        .unwrap()
        .replay(&[migration])
        .unwrap()
    }

    fn resolved_model_binding() -> ModelPhysicalBindingRow {
        ModelPhysicalBindingRow {
            physical_binding_id: "binding.entity.delta".to_owned(),
            mapping_program_id: "program.entity.delta-binding".to_owned(),
            source_schema_identity: "delta:entity@17".to_owned(),
            logical_relation_id: "relation.entity.logical".to_owned(),
            storage_relation_id: "relation.entity.storage".to_owned(),
            compatibility: SchemaCompatibility::Exact,
            column_mapping_mode: ColumnMappingMode::FieldId,
            deletion_vector_behavior: DeletionVectorBehavior::AppliedByProvider,
            field_bindings: vec![ModelPhysicalFieldBindingRow {
                logical_field_id: "field.entity.logical-id".to_owned(),
                storage_field_id: "field.entity.storage-id".to_owned(),
                projection_index: 0,
                filter_index: 0,
                statistics_index: 0,
            }],
        }
    }

    #[test]
    fn constructs_qualified_schema_and_round_trips_index_mappings() {
        let contract = contract();
        assert_eq!(contract.source_schema_identity(), "model.relation.v1");
        assert!(matches!(
            contract.model_relation_id(SchemaRole::Logical),
            Err(SchemaContractError::ModelMetadataUnavailable {
                role: SchemaRole::Logical
            })
        ));
        assert_eq!(
            contract.qualified_logical_schema().as_arrow(),
            contract.logical_schema().as_ref()
        );
        assert!(
            contract
                .qualified_logical_schema()
                .iter()
                .all(|(qualifier, _)| qualifier == Some(contract.qualifier()))
        );

        let logical_projection = [1, 0];
        let storage_projection = contract
            .map_projection(&logical_projection)
            .expect("mapped projection");
        assert_eq!(storage_projection, vec![0, 1]);
        assert_eq!(
            contract
                .restore_storage_projection(&storage_projection)
                .expect("restored projection"),
            logical_projection
        );
        assert_eq!(
            contract
                .map_filter_indices(&logical_projection)
                .expect("mapped filters"),
            vec![0, 1]
        );
        assert_eq!(
            contract
                .map_statistics_indices(&logical_projection)
                .expect("mapped statistics"),
            vec![0, 1]
        );

        let projected = contract
            .project_storage_schema(&logical_projection)
            .expect("native Arrow projection");
        assert_eq!(projected.field(0).name(), "payload");
        assert_eq!(projected.field(1).name(), "id_bytes");
    }

    #[test]
    fn exact_validation_reports_width_and_extension_mismatches() {
        let contract = contract();
        let wrong_width = Schema::new(vec![
            Arc::new(id_field(32, EXTENSION_METADATA)),
            Arc::new(payload_field(false)),
        ])
        .with_metadata(contract.logical_schema().metadata().clone());
        let error = contract
            .validate_phase_schema(
                SchemaPhase::AnalyzedLogicalPlan,
                &wrong_width,
                SchemaCompatibility::Exact,
            )
            .expect_err("wrong fixed width must fail");
        assert!(matches!(
            error,
            SchemaContractError::IncompatibleSchema {
                difference: SchemaDifference {
                    kind: SchemaDifferenceKind::DataType { .. },
                    ..
                },
                ..
            }
        ));

        let wrong_extension = Schema::new(vec![
            Arc::new(id_field(16, "{\"width\":32}")),
            Arc::new(payload_field(false)),
        ])
        .with_metadata(contract.logical_schema().metadata().clone());
        let error = contract
            .validate_phase_schema(
                SchemaPhase::OptimizedLogicalPlan,
                &wrong_extension,
                SchemaCompatibility::Exact,
            )
            .expect_err("changed extension metadata must fail");
        assert!(matches!(
            error,
            SchemaContractError::IncompatibleSchema {
                difference: SchemaDifference {
                    kind: SchemaDifferenceKind::ExtensionMetadata { .. },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn nested_nullability_and_dictionary_key_mismatches_are_located() {
        let contract = contract();
        let relaxed_child = Schema::new_with_metadata(
            vec![
                Arc::new(id_field(16, EXTENSION_METADATA)),
                Arc::new(payload_field(true)),
            ],
            contract.logical_schema().metadata().clone(),
        );
        let error = contract
            .validate_phase_schema(
                SchemaPhase::InitialPhysicalPlan,
                &relaxed_child,
                SchemaCompatibility::Exact,
            )
            .expect_err("nested nullability change must fail exact validation");
        match error {
            SchemaContractError::IncompatibleSchema { difference, .. } => {
                assert_eq!(difference.path(), "$.payload.code");
                assert!(matches!(
                    difference.kind(),
                    SchemaDifferenceKind::Nullability { .. }
                ));
            }
            other => panic!("unexpected error: {other}"),
        }

        let wrong_dictionary = Field::new(
            "payload",
            DataType::Struct(Fields::from(vec![Field::new(
                "code",
                DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
                false,
            )])),
            true,
        );
        let actual = Schema::new_with_metadata(
            vec![
                Arc::new(id_field(16, EXTENSION_METADATA)),
                Arc::new(wrong_dictionary),
            ],
            contract.logical_schema().metadata().clone(),
        );
        let error = contract
            .validate_phase_schema(
                SchemaPhase::OptimizedPhysicalPlan,
                &actual,
                SchemaCompatibility::Exact,
            )
            .expect_err("dictionary key change must fail");
        assert!(matches!(
            error,
            SchemaContractError::IncompatibleSchema {
                difference: SchemaDifference {
                    kind: SchemaDifferenceKind::DictionaryKeyType { .. },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn contains_uses_native_recursive_arrow_semantics() {
        let expected = Arc::new(Schema::new(vec![Arc::new(Field::new(
            "nested",
            DataType::Struct(Fields::from(vec![Field::new(
                "required",
                DataType::Int64,
                false,
            )])),
            false,
        ))]));
        let actual = Arc::new(Schema::new(vec![Arc::new(
            Field::new(
                "nested",
                DataType::Struct(Fields::from(vec![Field::new(
                    "required",
                    DataType::Int64,
                    true,
                )])),
                true,
            )
            .with_metadata(HashMap::from([(
                "physical.encoding".to_owned(),
                "optional".to_owned(),
            )])),
        )]));
        let contract = SchemaContract::try_new(
            "contains.v1",
            TableReference::bare("contains_relation"),
            Arc::clone(&expected),
            Arc::clone(&expected),
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .expect("valid contract");

        contract
            .validate_arrow_schema(
                SchemaPhase::InitialPhysicalPlan,
                SchemaRole::Logical,
                actual.as_ref(),
                SchemaCompatibility::Contains,
            )
            .expect("physical superset contains the logical schema");
        assert!(
            contract
                .validate_arrow_schema(
                    SchemaPhase::InitialPhysicalPlan,
                    SchemaRole::Logical,
                    actual.as_ref(),
                    SchemaCompatibility::Exact,
                )
                .is_err()
        );
    }

    #[test]
    fn empty_batches_keep_the_declared_stream_and_write_schemas() {
        let contract = contract();
        let empty = contract.empty_stream_batch();
        assert_eq!(empty.num_rows(), 0);
        assert_eq!(empty.schema_ref(), contract.empty_stream_schema());
        contract
            .validate_batch(
                contract.empty_stream_schema(),
                &empty,
                SchemaCompatibility::Exact,
            )
            .expect("empty logical batch keeps the stream schema");

        let empty_write = RecordBatch::new_empty(Arc::clone(contract.storage_schema()));
        contract
            .validate_write_batch(&empty_write)
            .expect("empty write keeps the storage schema");

        let mismatched_stream = Arc::new(Schema::new(vec![Arc::new(Field::new(
            "other",
            DataType::Utf8,
            true,
        ))]));
        let error = contract
            .validate_batch(
                contract.empty_stream_schema(),
                &RecordBatch::new_empty(mismatched_stream),
                SchemaCompatibility::Exact,
            )
            .expect_err("batch and stream schemas must be identical");
        assert!(matches!(
            error,
            SchemaContractError::BatchStreamSchemaMismatch { .. }
        ));
    }

    #[test]
    fn rejects_ambiguous_and_incomplete_model_bindings() {
        let duplicate = SchemaContract::try_new(
            "duplicate.v1",
            TableReference::bare("duplicate"),
            logical_schema(),
            storage_schema(),
            vec![
                FieldIndexMapping::direct(0, 1),
                FieldIndexMapping::direct(1, 1),
            ],
        )
        .expect_err("two logical fields cannot share a physical index");
        assert!(matches!(
            duplicate,
            SchemaContractError::DuplicatePhysicalMapping {
                purpose: IndexPurpose::Storage,
                ..
            }
        ));

        let malformed = Arc::new(Schema::new(vec![Arc::new(
            Field::new("id", DataType::FixedSizeBinary(16), false).with_metadata(HashMap::from([
                (
                    EXTENSION_TYPE_METADATA_KEY.to_owned(),
                    EXTENSION_METADATA.to_owned(),
                ),
            ])),
        )]));
        let error = SchemaContract::try_new(
            "malformed.v1",
            TableReference::bare("malformed"),
            Arc::clone(&malformed),
            malformed,
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .expect_err("extension metadata needs an extension name");
        assert!(matches!(
            error,
            SchemaContractError::InvalidExtensionMetadata { .. }
        ));
    }

    #[test]
    fn compiles_model_rows_and_round_trips_fixed_width_logical_ids() {
        let contract = model_rows()
            .compile("binding.entity.delta")
            .expect("model rows compile");
        assert_eq!(
            contract.logical_schema().field(0).data_type(),
            &DataType::FixedSizeBinary(16)
        );
        assert_eq!(
            contract.storage_schema().field(0).data_type(),
            &DataType::Binary
        );
        assert_eq!(contract.column_mapping_mode(), ColumnMappingMode::FieldId);
        assert_eq!(
            contract.deletion_vector_behavior(),
            DeletionVectorBehavior::AppliedByProvider
        );
        assert_eq!(contract.constraints().len(), 1);
        assert_eq!(
            contract
                .qualified_logical_schema()
                .functional_dependencies()
                .len(),
            1
        );
        assert_eq!(
            contract
                .qualified_logical_schema()
                .functional_dependencies()[0]
                .source_indices,
            vec![0]
        );

        let ids = FixedSizeBinaryArray::try_from_iter([b"0123456789abcdef".as_slice()].into_iter())
            .expect("fixed-width ID");
        let batch = RecordBatch::try_new(
            Arc::clone(contract.logical_schema()),
            vec![
                Arc::new(ids),
                Arc::new(StringArray::from(vec![Some("entity")])),
            ],
        )
        .expect("logical batch");
        let stored = contract
            .adapt_logical_batch_to_storage(&batch)
            .expect("strict storage cast");
        assert!(stored.column(0).as_any().is::<BinaryArray>());
        let restored = contract
            .restore_storage_batch(&stored)
            .expect("strict logical restoration");
        assert_eq!(restored, batch);

        let plan = EmptyExec::new(Arc::clone(contract.logical_schema()));
        contract
            .validate_physical_plan(
                SchemaPhase::OptimizedPhysicalPlan,
                &plan,
                SchemaCompatibility::Exact,
            )
            .expect("physical plan schema");
        assert!(matches!(
            contract.validate_physical_plan(
                SchemaPhase::AnalyzedLogicalPlan,
                &plan,
                SchemaCompatibility::Exact,
            ),
            Err(SchemaContractError::InvalidPhaseRole { .. })
        ));
    }

    #[test]
    fn projects_replayed_model_relations_and_exact_binding_selection() {
        let epoch = projectable_model_epoch();
        let rows = SchemaContractModelRows::from_model_epoch(
            &epoch,
            "codefabric",
            vec![resolved_model_binding()],
        )
        .expect("replayed model projects into schema rows");
        let contract = rows
            .compile("binding.entity.delta")
            .expect("projected rows compile");

        assert_eq!(
            contract.logical_schema().field(0).data_type(),
            &DataType::FixedSizeBinary(16)
        );
        assert_eq!(
            contract.storage_schema().field(0).data_type(),
            &DataType::Binary
        );
        assert_eq!(
            contract
                .logical_schema()
                .metadata()
                .get("codefabric.model.mapping_program_id")
                .map(String::as_str),
            Some("program.entity.delta-binding")
        );
        assert_eq!(contract.constraints().len(), 1);
    }

    #[test]
    fn model_projection_rejects_unexecuted_or_mismatched_binding_programs() {
        let epoch = projectable_model_epoch();
        assert!(matches!(
            SchemaContractModelRows::from_model_epoch(&epoch, "codefabric", Vec::new()),
            Err(SchemaContractError::ModelCompilation { .. })
        ));

        let mut mismatched = resolved_model_binding();
        mismatched.mapping_program_id = "program.not-authoritative".to_owned();
        assert!(matches!(
            SchemaContractModelRows::from_model_epoch(&epoch, "codefabric", vec![mismatched]),
            Err(SchemaContractError::ModelCompilation { .. })
        ));
    }

    #[test]
    fn model_identity_and_semantic_role_lookups_are_compiled_from_arrow_metadata() {
        let contract = model_rows()
            .compile("binding.entity.delta")
            .expect("model rows compile");
        assert_eq!(
            contract.model_relation_id(SchemaRole::Logical).unwrap(),
            "relation.logical"
        );
        assert_eq!(
            contract.model_relation_id(SchemaRole::Storage).unwrap(),
            "relation.storage"
        );
        assert_eq!(
            contract
                .logical_index_for_field_id("field.logical.name")
                .unwrap(),
            1
        );
        assert_eq!(
            contract
                .storage_index_for_field_id("field.storage.id")
                .unwrap(),
            0
        );
        assert_eq!(
            contract
                .logical_indices_for_semantic_role("canonical-id")
                .unwrap(),
            [0]
        );
        assert_eq!(
            contract
                .unique_field_index_for_semantic_role(SchemaRole::Logical, "semantic-text")
                .unwrap(),
            1
        );
        assert!(matches!(
            contract.logical_index_for_field_id("field.logical.missing"),
            Err(SchemaContractError::UnknownModelFieldId {
                role: SchemaRole::Logical,
                ..
            })
        ));
        assert!(matches!(
            contract.unique_field_index_for_semantic_role(SchemaRole::Logical, "missing-role"),
            Err(SchemaContractError::UnknownModelSemanticRole {
                role: SchemaRole::Logical,
                ..
            })
        ));

        let mut repeated_role_rows = model_rows();
        repeated_role_rows
            .fields
            .iter_mut()
            .filter(|field| field.relation_id == "relation.logical")
            .for_each(|field| field.semantic_role = "shared-role".to_owned());
        let repeated_role = repeated_role_rows
            .compile("binding.entity.delta")
            .expect("semantic roles may be one-to-many");
        assert_eq!(
            repeated_role
                .logical_indices_for_semantic_role("shared-role")
                .unwrap(),
            [0, 1]
        );
        assert!(matches!(
            repeated_role.unique_field_index_for_semantic_role(SchemaRole::Logical, "shared-role"),
            Err(SchemaContractError::AmbiguousModelSemanticRole {
                role: SchemaRole::Logical,
                match_count: 2,
                ..
            })
        ));

        let rebound = SchemaContract::try_new(
            "model-rebound",
            contract.qualifier().clone(),
            Arc::clone(contract.logical_schema()),
            Arc::clone(contract.storage_schema()),
            contract.mappings().to_vec(),
        )
        .expect("generic construction preserves authoritative model field IDs");
        assert_eq!(rebound.casts()[0].logical_field_id(), "field.logical.id");
        assert_eq!(rebound.casts()[0].storage_field_id(), "field.storage.id");
    }

    #[test]
    fn model_compiler_rejects_caller_values_in_compiler_owned_metadata_namespaces() {
        let mut field_collision = model_rows();
        field_collision.fields[0].field_metadata.insert(
            MODEL_FIELD_ID_METADATA_KEY.to_owned(),
            "forged.field".to_owned(),
        );
        assert!(matches!(
            field_collision.compile("binding.entity.delta"),
            Err(SchemaContractError::ReservedModelMetadataKey { key, .. })
                if key == MODEL_FIELD_ID_METADATA_KEY
        ));

        let mut relation_collision = model_rows();
        relation_collision.relations[0].relation_metadata.insert(
            MODEL_RELATION_ID_METADATA_KEY.to_owned(),
            "forged.relation".to_owned(),
        );
        assert!(matches!(
            relation_collision.compile("binding.entity.delta"),
            Err(SchemaContractError::ReservedModelMetadataKey { key, .. })
                if key == MODEL_RELATION_ID_METADATA_KEY
        ));

        let mut extension_collision = model_rows();
        extension_collision.fields[0].field_metadata.insert(
            EXTENSION_TYPE_NAME_KEY.to_owned(),
            "forged.extension".to_owned(),
        );
        assert!(matches!(
            extension_collision.compile("binding.entity.delta"),
            Err(SchemaContractError::ReservedModelMetadataKey { key, .. })
                if key == EXTENSION_TYPE_NAME_KEY
        ));
    }

    #[test]
    fn missing_or_duplicate_field_identity_fails_contract_construction() {
        let partial = Arc::new(Schema::new_with_metadata(
            vec![Arc::new(
                Field::new("id", DataType::Utf8, false).with_metadata(HashMap::from([(
                    MODEL_SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                    "identity".to_owned(),
                )])),
            )],
            HashMap::from([(
                MODEL_RELATION_ID_METADATA_KEY.to_owned(),
                "relation.partial".to_owned(),
            )]),
        ));
        let partial_error = SchemaContract::try_new(
            "partial-model-metadata",
            TableReference::bare("partial"),
            Arc::clone(&partial),
            partial,
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .expect_err("a relation identity requires every stable field identity");
        assert!(matches!(
            partial_error,
            SchemaContractError::IncompleteModelFieldMetadata {
                role: SchemaRole::Logical,
                field_index: 0,
                missing_key: MODEL_FIELD_ID_METADATA_KEY,
            }
        ));

        let duplicate = Arc::new(Schema::new_with_metadata(
            ["left", "right"]
                .into_iter()
                .map(|name| {
                    Arc::new(
                        Field::new(name, DataType::Utf8, false).with_metadata(HashMap::from([
                            (
                                MODEL_FIELD_ID_METADATA_KEY.to_owned(),
                                "field.duplicate".to_owned(),
                            ),
                            (
                                MODEL_SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                                "value".to_owned(),
                            ),
                        ])),
                    )
                })
                .collect::<Vec<_>>(),
            HashMap::from([(
                MODEL_RELATION_ID_METADATA_KEY.to_owned(),
                "relation.duplicate".to_owned(),
            )]),
        ));
        let duplicate_error = SchemaContract::try_new(
            "duplicate-model-field-id",
            TableReference::bare("duplicate"),
            Arc::clone(&duplicate),
            duplicate,
            vec![
                FieldIndexMapping::direct(0, 0),
                FieldIndexMapping::direct(1, 1),
            ],
        )
        .expect_err("model field IDs remain unique independent of Arrow names");
        assert!(matches!(
            duplicate_error,
            SchemaContractError::DuplicateModelFieldId {
                role: SchemaRole::Logical,
                first_index: 0,
                second_index: 1,
                ..
            }
        ));
    }

    #[test]
    fn restoration_rejects_wrong_width_and_adaptation_rejects_unmapped_outputs() {
        let compiled_contract = model_rows()
            .compile("binding.entity.delta")
            .expect("model rows compile");
        let wrong_width = RecordBatch::try_new(
            Arc::clone(compiled_contract.storage_schema()),
            vec![
                Arc::new(BinaryArray::from(vec![b"too-short".as_slice()])),
                Arc::new(StringArray::from(vec![Some("entity")])),
            ],
        )
        .expect("storage-shape batch");
        assert!(matches!(
            compiled_contract.restore_storage_batch(&wrong_width),
            Err(SchemaContractError::FieldCast {
                direction: "storage-to-logical",
                ..
            })
        ));

        let legacy = contract();
        let logical = RecordBatch::new_empty(Arc::clone(legacy.logical_schema()));
        assert!(matches!(
            legacy.adapt_logical_batch_to_storage(&logical),
            Err(SchemaContractError::UnmappedStorageOutput {
                storage_index: 2,
                ..
            })
        ));
    }
}
