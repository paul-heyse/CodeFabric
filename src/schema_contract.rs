//! Executable logical/physical Arrow and DataFusion schema contracts.
//!
//! A [`SchemaContract`] is constructed from the exact provider or transformation
//! schema installed in one candidate DataFusion session. It keeps that logical
//! schema, its qualified DataFusion form, the bound storage schema, and every
//! index translation together so that a provider or sink cannot reinterpret
//! columns independently.

use std::collections::{BTreeMap, BTreeSet};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_cast::cast::{CastOptions, can_cast_types, cast_with_options};
use arrow_schema::extension::{EXTENSION_TYPE_METADATA_KEY, EXTENSION_TYPE_NAME_KEY};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use datafusion::common::{
    Constraints, DFSchema, DataFusionError, FunctionalDependencies, TableReference,
};
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::{ExecutionPlan, RecordBatchStream, SendableRecordBatchStream};
use futures::Stream;

/// Arrow schema metadata key carrying the session-owned relation identity.
pub const RELATION_ID_METADATA_KEY: &str = "codefabric.relation_id";
/// Arrow field metadata key carrying the session-owned field identity.
pub const FIELD_ID_METADATA_KEY: &str = "codefabric.field_id";
/// Arrow field metadata key carrying the field's semantic role.
pub const SEMANTIC_ROLE_METADATA_KEY: &str = "codefabric.semantic_role";

/// Return a domain-separated fingerprint of the RFC 8785 canonical Arrow schema bytes.
///
/// This is a compact boundary identity for an already authoritative [`Schema`]; it is never used
/// to prove schema equality. Callers must still compare the exact Arrow schema at every phase.
pub(crate) fn canonical_arrow_schema_fingerprint(
    schema: &SchemaRef,
) -> Result<[u8; 32], serde_json::Error> {
    let canonical = serde_json_canonicalizer::to_vec(schema.as_ref())?;
    let mut hasher = blake3::Hasher::new();
    let domain = b"codefabric.arrow-schema.canonical.v1";
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update(&(canonical.len() as u64).to_be_bytes());
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}

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

/// Which contract-owned schema is expected at a boundary.
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
    /// Stable session-owned field IDs carry storage identity.
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
    /// The storage provider exposes the contract-declared visibility column.
    ExposedVisibilityColumn,
}

/// Non-schema policies bound to schemas observed from a provider or logical plan.
///
/// The schemas remain the authority for fields, identities, types, nullability,
/// and metadata. These options carry only behavior that Arrow schemas cannot
/// express by themselves.
#[derive(Clone, Debug)]
pub struct SchemaContractOptions {
    constraints: Constraints,
    compatibility: SchemaCompatibility,
    column_mapping_mode: ColumnMappingMode,
    deletion_vector_behavior: DeletionVectorBehavior,
}

impl SchemaContractOptions {
    /// Construct an explicit policy set for one observed logical/storage binding.
    #[must_use]
    pub const fn new(
        constraints: Constraints,
        compatibility: SchemaCompatibility,
        column_mapping_mode: ColumnMappingMode,
        deletion_vector_behavior: DeletionVectorBehavior,
    ) -> Self {
        Self {
            constraints,
            compatibility,
            column_mapping_mode,
            deletion_vector_behavior,
        }
    }
}

impl Default for SchemaContractOptions {
    fn default() -> Self {
        Self::new(
            Constraints::default(),
            SchemaCompatibility::Exact,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::Forbidden,
        )
    }
}

/// One compiled logical/storage cast, retaining both exact field identities.
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
    #[error("{role:?} schema field {field_index} is missing identity metadata {missing_key:?}")]
    IncompleteFieldIdentityMetadata {
        role: SchemaRole,
        field_index: usize,
        missing_key: &'static str,
    },
    #[error("{role:?} schema has an empty identity metadata value for {key:?} at {path}")]
    EmptyIdentityMetadata {
        role: SchemaRole,
        key: &'static str,
        path: String,
    },
    #[error(
        "{role:?} schema field ID {field_id:?} is duplicated at indices {first_index} and {second_index}"
    )]
    DuplicateFieldId {
        role: SchemaRole,
        field_id: String,
        first_index: usize,
        second_index: usize,
    },
    #[error("{role:?} schema does not declare a relation identity")]
    MissingRelationId { role: SchemaRole },
    #[error("{role:?} schema has no compiled identity metadata")]
    IdentityMetadataUnavailable { role: SchemaRole },
    #[error("{role:?} schema has no field with ID {field_id:?}")]
    UnknownFieldId { role: SchemaRole, field_id: String },
    #[error("{role:?} schema has no field with semantic role {semantic_role:?}")]
    UnknownSemanticRole {
        role: SchemaRole,
        semantic_role: String,
    },
    #[error(
        "{role:?} schema semantic role {semantic_role:?} resolves to {match_count} fields, not one"
    )]
    AmbiguousSemanticRole {
        role: SchemaRole,
        semantic_role: String,
        match_count: usize,
    },
    #[error(
        "{role:?} cast binding at index {field_index} names field {actual_field_id:?}, expected {expected_field_id:?}"
    )]
    FieldBindingMismatch {
        role: SchemaRole,
        field_index: usize,
        expected_field_id: Arc<str>,
        actual_field_id: Arc<str>,
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
    #[error("schema contract cast binding {identity:?} is invalid: {reason}")]
    InvalidCastBinding { identity: String, reason: String },
    #[error(
        "logical field {logical_field_id:?} cannot cast {logical_type} to storage field {storage_field_id:?} type {storage_type}"
    )]
    UnsupportedStorageCast {
        logical_field_id: String,
        storage_field_id: String,
        logical_type: DataType,
        storage_type: DataType,
    },
    #[error(
        "logical field {logical_field_id:?} cannot restore storage field {storage_field_id:?} type {storage_type} to {logical_type}"
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
struct SchemaIdentityIndex {
    relation_id: Arc<str>,
    field_ids_by_index: Arc<[Arc<str>]>,
    field_indices_by_id: BTreeMap<Arc<str>, usize>,
    field_indices_by_semantic_role: BTreeMap<Arc<str>, Arc<[usize]>>,
}

fn compile_schema_identity_index(
    role: SchemaRole,
    schema: &Schema,
) -> Result<Option<SchemaIdentityIndex>, SchemaContractError> {
    let relation_id = schema
        .metadata()
        .get(RELATION_ID_METADATA_KEY)
        .map(|value| {
            if value.trim().is_empty() {
                Err(SchemaContractError::EmptyIdentityMetadata {
                    role,
                    key: RELATION_ID_METADATA_KEY,
                    path: "$".to_owned(),
                })
            } else {
                Ok(Arc::<str>::from(value.as_str()))
            }
        })
        .transpose()?;
    let fields_declare_identity_metadata = schema.fields().iter().any(|field| {
        field.metadata().contains_key(FIELD_ID_METADATA_KEY)
            || field.metadata().contains_key(SEMANTIC_ROLE_METADATA_KEY)
    });
    if relation_id.is_none() {
        if fields_declare_identity_metadata {
            return Err(SchemaContractError::MissingRelationId { role });
        }
        return Ok(None);
    }

    let mut field_indices_by_id = BTreeMap::new();
    let mut field_ids_by_index = Vec::with_capacity(schema.fields().len());
    let mut semantic_role_indices = BTreeMap::<Arc<str>, Vec<usize>>::new();
    for (field_index, field) in schema.fields().iter().enumerate() {
        let field_id = field.metadata().get(FIELD_ID_METADATA_KEY).ok_or(
            SchemaContractError::IncompleteFieldIdentityMetadata {
                role,
                field_index,
                missing_key: FIELD_ID_METADATA_KEY,
            },
        )?;
        let path = format!("$.{}", field.name());
        if field_id.trim().is_empty() {
            return Err(SchemaContractError::EmptyIdentityMetadata {
                role,
                key: FIELD_ID_METADATA_KEY,
                path,
            });
        }
        let field_id = Arc::<str>::from(field_id.as_str());
        if let Some(first_index) = field_indices_by_id.insert(Arc::clone(&field_id), field_index) {
            return Err(SchemaContractError::DuplicateFieldId {
                role,
                field_id: field_id.to_string(),
                first_index,
                second_index: field_index,
            });
        }
        field_ids_by_index.push(field_id);
        if let Some(semantic_role) = field.metadata().get(SEMANTIC_ROLE_METADATA_KEY) {
            if semantic_role.trim().is_empty() {
                return Err(SchemaContractError::EmptyIdentityMetadata {
                    role,
                    key: SEMANTIC_ROLE_METADATA_KEY,
                    path,
                });
            }
            semantic_role_indices
                .entry(Arc::<str>::from(semantic_role.as_str()))
                .or_default()
                .push(field_index);
        }
    }

    Ok(Some(SchemaIdentityIndex {
        relation_id: relation_id.expect("field identity metadata requires a relation identity"),
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
    logical_identity_index: Option<Arc<SchemaIdentityIndex>>,
    storage_identity_index: Option<Arc<SchemaIdentityIndex>>,
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
        Self::try_new_with_options(
            source_schema_identity,
            qualifier,
            logical_schema,
            storage_schema,
            mappings,
            SchemaContractOptions::default(),
        )
    }

    /// Build a contract from observed schemas and explicit non-schema policies.
    ///
    /// Field identities, types, nullability, metadata, and cast bindings are
    /// always derived from `logical_schema`, `storage_schema`, and `mappings`.
    ///
    /// # Errors
    ///
    /// Returns a typed error when an observed schema, identity, constraint, or
    /// mapping cannot form one internally consistent contract.
    pub fn try_new_with_options(
        source_schema_identity: impl Into<Arc<str>>,
        qualifier: TableReference,
        logical_schema: SchemaRef,
        storage_schema: SchemaRef,
        mappings: Vec<FieldIndexMapping>,
        options: SchemaContractOptions,
    ) -> Result<Self, SchemaContractError> {
        let SchemaContractOptions {
            constraints,
            compatibility,
            column_mapping_mode,
            deletion_vector_behavior,
        } = options;
        let source_schema_identity = source_schema_identity.into();
        if source_schema_identity.trim().is_empty() {
            return Err(SchemaContractError::EmptySourceSchemaIdentity);
        }

        validate_extension_metadata(SchemaRole::Logical, logical_schema.as_ref())?;
        validate_extension_metadata(SchemaRole::Storage, storage_schema.as_ref())?;
        let logical_identity_index =
            compile_schema_identity_index(SchemaRole::Logical, logical_schema.as_ref())?
                .map(Arc::new);
        let storage_identity_index =
            compile_schema_identity_index(SchemaRole::Storage, storage_schema.as_ref())?
                .map(Arc::new);

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
        let casts = mappings
            .iter()
            .map(|mapping| {
                let logical = logical_schema.field(mapping.logical_index());
                let storage = storage_schema.field(mapping.storage_index());
                FieldCastBinding {
                    logical_field_id: logical_identity_index.as_ref().map_or_else(
                        || Arc::from(logical.name().as_str()),
                        |index| Arc::clone(&index.field_ids_by_index[mapping.logical_index()]),
                    ),
                    storage_field_id: storage_identity_index.as_ref().map_or_else(
                        || Arc::from(storage.name().as_str()),
                        |index| Arc::clone(&index.field_ids_by_index[mapping.storage_index()]),
                    ),
                    logical_index: mapping.logical_index(),
                    storage_index: mapping.storage_index(),
                    logical_data_type: logical.data_type().clone(),
                    storage_data_type: storage.data_type().clone(),
                }
            })
            .collect::<Vec<_>>();
        validate_cast_bindings(
            &casts,
            &mappings,
            &logical_schema,
            &storage_schema,
            logical_identity_index.as_deref(),
            storage_identity_index.as_deref(),
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
            logical_identity_index,
            storage_identity_index,
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

    /// Resolve the exact relation identity carried by the live session schema.
    ///
    /// Returns [`SchemaContractError::IdentityMetadataUnavailable`] when the
    /// observed schema lacks the required identity metadata. Callers must not
    /// substitute an Arrow table name.
    pub fn relation_id(&self, role: SchemaRole) -> Result<&str, SchemaContractError> {
        Ok(self.identity_index(role)?.relation_id.as_ref())
    }

    /// Resolve a stable field ID to its exact index within the selected schema role.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaContractError::UnknownFieldId`] rather than falling back to a
    /// physical field name or ordinal.
    pub fn field_index_for_id(
        &self,
        role: SchemaRole,
        field_id: &str,
    ) -> Result<usize, SchemaContractError> {
        self.identity_index(role)?
            .field_indices_by_id
            .get(field_id)
            .copied()
            .ok_or_else(|| SchemaContractError::UnknownFieldId {
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
        self.identity_index(role)?
            .field_ids_by_index
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| SchemaContractError::UnknownFieldId {
                role,
                field_id: format!("index:{index}"),
            })
    }

    /// Resolve one logical field ID without consulting Arrow field names.
    pub fn logical_index_for_field_id(&self, field_id: &str) -> Result<usize, SchemaContractError> {
        self.field_index_for_id(SchemaRole::Logical, field_id)
    }

    /// Resolve one storage field ID without consulting Arrow field names.
    pub fn storage_index_for_field_id(&self, field_id: &str) -> Result<usize, SchemaContractError> {
        self.field_index_for_id(SchemaRole::Storage, field_id)
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

    /// Return every exact field index carrying a contract-owned semantic role.
    ///
    /// Semantic roles are intentionally one-to-many. An empty slice means the
    /// observed relation did not declare the role; callers requiring exactly one field must use
    /// [`Self::unique_field_index_for_semantic_role`].
    pub fn field_indices_for_semantic_role(
        &self,
        role: SchemaRole,
        semantic_role: &str,
    ) -> Result<&[usize], SchemaContractError> {
        Ok(self
            .identity_index(role)?
            .field_indices_by_semantic_role
            .get(semantic_role)
            .map_or(&[], AsRef::as_ref))
    }

    /// Return every logical field index carrying a contract-owned semantic role.
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
    /// requirement, not a global restriction on semantic roles.
    pub fn unique_field_index_for_semantic_role(
        &self,
        role: SchemaRole,
        semantic_role: &str,
    ) -> Result<usize, SchemaContractError> {
        match self.field_indices_for_semantic_role(role, semantic_role)? {
            [index] => Ok(*index),
            [] => Err(SchemaContractError::UnknownSemanticRole {
                role,
                semantic_role: semantic_role.to_owned(),
            }),
            indices => Err(SchemaContractError::AmbiguousSemanticRole {
                role,
                semantic_role: semantic_role.to_owned(),
                match_count: indices.len(),
            }),
        }
    }

    fn identity_index(
        &self,
        role: SchemaRole,
    ) -> Result<&SchemaIdentityIndex, SchemaContractError> {
        let index = match role {
            SchemaRole::Logical => &self.logical_identity_index,
            SchemaRole::Storage => &self.storage_identity_index,
        };
        index
            .as_deref()
            .ok_or(SchemaContractError::IdentityMetadataUnavailable { role })
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

    /// Validate a phase using the compatibility policy bound to this contract.
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

    /// Restore a provider/storage batch to exact contract-owned logical meaning.
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

type ValidatedMappings = (Arc<[FieldIndexMapping]>, Arc<[Option<usize>]>);

fn validate_cast_bindings(
    casts: &[FieldCastBinding],
    mappings: &[FieldIndexMapping],
    logical_schema: &SchemaRef,
    storage_schema: &SchemaRef,
    logical_identity_index: Option<&SchemaIdentityIndex>,
    storage_identity_index: Option<&SchemaIdentityIndex>,
) -> Result<(), SchemaContractError> {
    if casts.len() != mappings.len() {
        return Err(SchemaContractError::InvalidCastBinding {
            identity: "field-cast-bindings".to_owned(),
            reason: format!(
                "expected {} cast bindings, received {}",
                mappings.len(),
                casts.len()
            ),
        });
    }
    for (logical_index, (cast, mapping)) in casts.iter().zip(mappings).enumerate() {
        if cast.logical_index != logical_index
            || cast.logical_index != mapping.logical_index()
            || cast.storage_index != mapping.storage_index()
            || &cast.logical_data_type != logical_schema.field(logical_index).data_type()
            || &cast.storage_data_type != storage_schema.field(cast.storage_index).data_type()
        {
            return Err(SchemaContractError::InvalidCastBinding {
                identity: cast.logical_field_id().to_owned(),
                reason: "cast binding does not match the observed field/index mapping".to_owned(),
            });
        }
        if let Some(identity_index) = logical_identity_index {
            let expected = &identity_index.field_ids_by_index[logical_index];
            if expected.as_ref() != cast.logical_field_id() {
                return Err(SchemaContractError::FieldBindingMismatch {
                    role: SchemaRole::Logical,
                    field_index: logical_index,
                    expected_field_id: Arc::clone(expected),
                    actual_field_id: Arc::clone(&cast.logical_field_id),
                });
            }
        }
        if let Some(identity_index) = storage_identity_index {
            let expected = &identity_index.field_ids_by_index[cast.storage_index];
            if expected.as_ref() != cast.storage_field_id() {
                return Err(SchemaContractError::FieldBindingMismatch {
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
    use datafusion::common::Constraint;
    use datafusion::physical_plan::empty::EmptyExec;

    use super::*;

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

    #[test]
    fn canonical_schema_fingerprint_tracks_exact_schema_not_map_insertion_order() {
        let left = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([
                    ("z".to_owned(), "last".to_owned()),
                    ("a".to_owned(), "first".to_owned()),
                ])),
            ],
            HashMap::from([
                ("release".to_owned(), "2.2.0".to_owned()),
                ("relation".to_owned(), "facts.values".to_owned()),
            ]),
        ));
        let right = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([
                    ("a".to_owned(), "first".to_owned()),
                    ("z".to_owned(), "last".to_owned()),
                ])),
            ],
            HashMap::from([
                ("relation".to_owned(), "facts.values".to_owned()),
                ("release".to_owned(), "2.2.0".to_owned()),
            ]),
        ));
        let nullable = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("value", DataType::Int64, true).with_metadata(HashMap::from([
                    ("a".to_owned(), "first".to_owned()),
                    ("z".to_owned(), "last".to_owned()),
                ])),
            ],
            right.metadata().clone(),
        ));

        assert_eq!(
            canonical_arrow_schema_fingerprint(&left).unwrap(),
            canonical_arrow_schema_fingerprint(&right).unwrap()
        );
        assert_ne!(
            canonical_arrow_schema_fingerprint(&left).unwrap(),
            canonical_arrow_schema_fingerprint(&nullable).unwrap()
        );
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
            HashMap::from([("provider.schema".to_owned(), "relation.v1".to_owned())]),
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
            "provider.relation.v1",
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

    fn field_identity_metadata(field_id: &str, semantic_role: &str) -> HashMap<String, String> {
        HashMap::from([
            (FIELD_ID_METADATA_KEY.to_owned(), field_id.to_owned()),
            (
                SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                semantic_role.to_owned(),
            ),
        ])
    }

    fn observed_binding_schemas(logical_roles: [&str; 2]) -> (SchemaRef, SchemaRef) {
        let mut logical_id_metadata = field_identity_metadata("field.logical.id", logical_roles[0]);
        logical_id_metadata.insert(
            EXTENSION_TYPE_NAME_KEY.to_owned(),
            "codefabric.id16".to_owned(),
        );
        logical_id_metadata.insert(
            EXTENSION_TYPE_METADATA_KEY.to_owned(),
            EXTENSION_METADATA.to_owned(),
        );
        let logical_schema = Arc::new(Schema::new_with_metadata(
            vec![
                Arc::new(
                    Field::new("entity_id", DataType::FixedSizeBinary(16), false)
                        .with_metadata(logical_id_metadata),
                ),
                Arc::new(Field::new("name", DataType::Utf8, true).with_metadata(
                    field_identity_metadata("field.logical.name", logical_roles[1]),
                )),
            ],
            HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                "relation.logical".to_owned(),
            )]),
        ));
        let storage_schema = Arc::new(Schema::new_with_metadata(
            vec![
                Arc::new(
                    Field::new("entity_id_bytes", DataType::Binary, false)
                        .with_metadata(field_identity_metadata("field.storage.id", "storage-id")),
                ),
                Arc::new(Field::new("name", DataType::Utf8, true).with_metadata(
                    field_identity_metadata("field.storage.name", "storage-value"),
                )),
            ],
            HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                "relation.storage".to_owned(),
            )]),
        ));
        (logical_schema, storage_schema)
    }

    fn observed_binding_contract() -> SchemaContract {
        let (logical_schema, storage_schema) =
            observed_binding_schemas(["canonical-id", "semantic-text"]);
        SchemaContract::try_new_with_options(
            "delta:entity@17",
            TableReference::full("codefabric", "cpg_serving", "entity"),
            logical_schema,
            storage_schema,
            vec![
                FieldIndexMapping::direct(0, 0),
                FieldIndexMapping::direct(1, 1),
            ],
            SchemaContractOptions::new(
                Constraints::new_unverified(vec![Constraint::PrimaryKey(vec![0])]),
                SchemaCompatibility::Exact,
                ColumnMappingMode::FieldId,
                DeletionVectorBehavior::AppliedByProvider,
            ),
        )
        .expect("observed schemas form one direct contract")
    }

    #[test]
    fn constructs_qualified_schema_and_round_trips_index_mappings() {
        let contract = contract();
        assert_eq!(contract.source_schema_identity(), "provider.relation.v1");
        assert!(matches!(
            contract.relation_id(SchemaRole::Logical),
            Err(SchemaContractError::IdentityMetadataUnavailable {
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
    fn rejects_ambiguous_mappings_and_incomplete_extension_metadata() {
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
    fn direct_observed_schemas_bind_policy_casts_and_public_phase_output() {
        let contract = observed_binding_contract();
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
        assert_eq!(contract.casts()[0].logical_field_id(), "field.logical.id");
        assert_eq!(contract.casts()[0].storage_field_id(), "field.storage.id");

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
        contract
            .validate_batch(
                contract.logical_schema(),
                &batch,
                SchemaCompatibility::Exact,
            )
            .expect("public batch retains the observed logical schema");
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
    fn field_identity_and_semantic_role_lookups_come_from_arrow_metadata() {
        let contract = observed_binding_contract();
        assert_eq!(
            contract.relation_id(SchemaRole::Logical).unwrap(),
            "relation.logical"
        );
        assert_eq!(
            contract.relation_id(SchemaRole::Storage).unwrap(),
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
            Err(SchemaContractError::UnknownFieldId {
                role: SchemaRole::Logical,
                ..
            })
        ));
        assert!(matches!(
            contract.unique_field_index_for_semantic_role(SchemaRole::Logical, "missing-role"),
            Err(SchemaContractError::UnknownSemanticRole {
                role: SchemaRole::Logical,
                ..
            })
        ));

        let (logical_schema, storage_schema) =
            observed_binding_schemas(["shared-role", "shared-role"]);
        let repeated_role = SchemaContract::try_new(
            "same-role-observation",
            TableReference::full("codefabric", "cpg_serving", "entity"),
            logical_schema,
            storage_schema,
            vec![
                FieldIndexMapping::direct(0, 0),
                FieldIndexMapping::direct(1, 1),
            ],
        )
        .expect("semantic roles may be one-to-many");
        assert_eq!(
            repeated_role
                .logical_indices_for_semantic_role("shared-role")
                .unwrap(),
            [0, 1]
        );
        assert!(matches!(
            repeated_role.unique_field_index_for_semantic_role(SchemaRole::Logical, "shared-role"),
            Err(SchemaContractError::AmbiguousSemanticRole {
                role: SchemaRole::Logical,
                match_count: 2,
                ..
            })
        ));

        let rebound = SchemaContract::try_new(
            "programmatic-rebound",
            contract.qualifier().clone(),
            Arc::clone(contract.logical_schema()),
            Arc::clone(contract.storage_schema()),
            contract.mappings().to_vec(),
        )
        .expect("direct construction preserves authoritative field IDs");
        assert_eq!(rebound.casts()[0].logical_field_id(), "field.logical.id");
        assert_eq!(rebound.casts()[0].storage_field_id(), "field.storage.id");
    }

    #[test]
    fn missing_or_duplicate_field_identity_fails_contract_construction() {
        let partial = Arc::new(Schema::new_with_metadata(
            vec![Arc::new(
                Field::new("id", DataType::Utf8, false).with_metadata(HashMap::from([(
                    SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                    "identity".to_owned(),
                )])),
            )],
            HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                "relation.partial".to_owned(),
            )]),
        ));
        let partial_error = SchemaContract::try_new(
            "partial-identity-metadata",
            TableReference::bare("partial"),
            Arc::clone(&partial),
            partial,
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .expect_err("a relation identity requires every stable field identity");
        assert!(matches!(
            partial_error,
            SchemaContractError::IncompleteFieldIdentityMetadata {
                role: SchemaRole::Logical,
                field_index: 0,
                missing_key: FIELD_ID_METADATA_KEY,
            }
        ));

        let duplicate = Arc::new(Schema::new_with_metadata(
            ["left", "right"]
                .into_iter()
                .map(|name| {
                    Arc::new(
                        Field::new(name, DataType::Utf8, false).with_metadata(HashMap::from([
                            (
                                FIELD_ID_METADATA_KEY.to_owned(),
                                "field.duplicate".to_owned(),
                            ),
                            (SEMANTIC_ROLE_METADATA_KEY.to_owned(), "value".to_owned()),
                        ])),
                    )
                })
                .collect::<Vec<_>>(),
            HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                "relation.duplicate".to_owned(),
            )]),
        ));
        let duplicate_error = SchemaContract::try_new(
            "duplicate-field-id",
            TableReference::bare("duplicate"),
            Arc::clone(&duplicate),
            duplicate,
            vec![
                FieldIndexMapping::direct(0, 0),
                FieldIndexMapping::direct(1, 1),
            ],
        )
        .expect_err("field IDs remain unique independent of Arrow names");
        assert!(matches!(
            duplicate_error,
            SchemaContractError::DuplicateFieldId {
                role: SchemaRole::Logical,
                first_index: 0,
                second_index: 1,
                ..
            }
        ));
    }

    #[test]
    fn restoration_rejects_wrong_width_and_adaptation_rejects_unmapped_outputs() {
        let compiled_contract = observed_binding_contract();
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

        let unmapped = contract();
        let logical = RecordBatch::new_empty(Arc::clone(unmapped.logical_schema()));
        assert!(matches!(
            unmapped.adapt_logical_batch_to_storage(&logical),
            Err(SchemaContractError::UnmappedStorageOutput {
                storage_index: 2,
                ..
            })
        ));
    }
}
