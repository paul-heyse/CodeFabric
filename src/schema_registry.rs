//! Generated Arrow schema registry for durable, overlay, and operational surfaces.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use arrow_schema::extension::ExtensionType;
use arrow_schema::{ArrowError, DataType, Field, Fields, Schema, SchemaRef, TimeUnit};
use datafusion::common::ScalarValue;
use datafusion::common::metadata::FieldMetadata;
use datafusion::common::types::DFExtensionType;
use datafusion::logical_expr::Expr;
use datafusion::logical_expr::registry::{ExtensionTypeRegistration, ExtensionTypeRegistrationRef};

/// Generated descriptor for one application-owned ID logical type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedIdDomainSpec {
    pub domain_slug: &'static str,
    pub extension_name: &'static str,
    pub rust_type: &'static str,
    pub preimage_recipe_id: &'static str,
    pub preimage_version: &'static str,
}

/// Common generated ID-extension behavior used by DataFusion registration factories.
pub trait CodeFabricIdExtension:
    ExtensionType<Metadata = ()> + Copy + std::fmt::Debug + Send + Sync + 'static
{
    const DOMAIN_SLUG: &'static str;
    const PREIMAGE_RECIPE_ID: &'static str;
    const PREIMAGE_VERSION: &'static str;
    const METADATA_V1: &'static str;
    fn v1() -> Self;
}

macro_rules! define_id_domain_extension {
    ($type:ident, $domain:literal, $name:literal, $recipe:literal, $version:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $type {
            metadata: (),
        }

        impl $type {
            pub const NAME: &'static str = $name;
            pub const METADATA_V1: &'static str = concat!(
                "{\"domain\":\"",
                $domain,
                "\",\"preimage_recipe_id\":\"",
                $recipe,
                "\",\"preimage_version\":\"",
                $version,
                "\"}"
            );

            #[must_use]
            pub const fn v1() -> Self {
                Self { metadata: () }
            }
        }

        impl ExtensionType for $type {
            const NAME: &'static str = Self::NAME;
            type Metadata = ();

            fn metadata(&self) -> &Self::Metadata {
                &self.metadata
            }

            fn serialize_metadata(&self) -> Option<String> {
                Some(Self::METADATA_V1.to_owned())
            }

            fn deserialize_metadata(metadata: Option<&str>) -> Result<Self::Metadata, ArrowError> {
                match metadata {
                    Some(Self::METADATA_V1) => Ok(()),
                    value => Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires metadata {}, received {value:?}",
                        Self::NAME,
                        Self::METADATA_V1
                    ))),
                }
            }

            fn supports_data_type(&self, data_type: &DataType) -> Result<(), ArrowError> {
                if data_type == &DataType::FixedSizeBinary(16) {
                    Ok(())
                } else {
                    Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires FixedSizeBinary(16), received {data_type}",
                        Self::NAME
                    )))
                }
            }

            fn try_new(data_type: &DataType, metadata: Self::Metadata) -> Result<Self, ArrowError> {
                let extension = Self { metadata };
                extension.supports_data_type(data_type)?;
                Ok(extension)
            }
        }

        impl CodeFabricIdExtension for $type {
            const DOMAIN_SLUG: &'static str = $domain;
            const PREIMAGE_RECIPE_ID: &'static str = $recipe;
            const PREIMAGE_VERSION: &'static str = $version;
            const METADATA_V1: &'static str = Self::METADATA_V1;

            fn v1() -> Self {
                Self::v1()
            }
        }
    };
}

macro_rules! define_hash32_extension {
    () => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct Hash32Extension {
            metadata: (),
        }

        impl Hash32Extension {
            pub const NAME: &'static str = "codefabric.hash32";
            pub const METADATA_V1: &'static str = "{\"width\":32,\"version\":1}";

            #[must_use]
            pub const fn v1() -> Self {
                Self { metadata: () }
            }
        }

        impl ExtensionType for Hash32Extension {
            const NAME: &'static str = Self::NAME;
            type Metadata = ();

            fn metadata(&self) -> &Self::Metadata {
                &self.metadata
            }

            fn serialize_metadata(&self) -> Option<String> {
                Some(Self::METADATA_V1.to_owned())
            }

            fn deserialize_metadata(metadata: Option<&str>) -> Result<Self::Metadata, ArrowError> {
                match metadata {
                    Some(Self::METADATA_V1) => Ok(()),
                    value => Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires metadata {}, received {value:?}",
                        Self::NAME,
                        Self::METADATA_V1
                    ))),
                }
            }

            fn supports_data_type(&self, data_type: &DataType) -> Result<(), ArrowError> {
                if data_type == &DataType::FixedSizeBinary(32) {
                    Ok(())
                } else {
                    Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires FixedSizeBinary(32), received {data_type}",
                        Self::NAME
                    )))
                }
            }

            fn try_new(data_type: &DataType, metadata: Self::Metadata) -> Result<Self, ArrowError> {
                let extension = Self { metadata };
                extension.supports_data_type(data_type)?;
                Ok(extension)
            }
        }
    };
}

#[derive(Debug)]
struct DataFusionCodeFabricExtension {
    storage_type: DataType,
    metadata: String,
}

impl DFExtensionType for DataFusionCodeFabricExtension {
    fn storage_type(&self) -> DataType {
        self.storage_type.clone()
    }

    fn serialize_metadata(&self) -> Option<String> {
        Some(self.metadata.clone())
    }
}

fn id_domain_registration<T: CodeFabricIdExtension>() -> ExtensionTypeRegistrationRef {
    ExtensionTypeRegistration::new_arc(T::NAME, |storage_type, metadata| {
        T::deserialize_metadata(metadata)?;
        T::try_new(storage_type, ())?;
        Ok(Arc::new(DataFusionCodeFabricExtension {
            storage_type: storage_type.clone(),
            metadata: T::METADATA_V1.to_owned(),
        }))
    })
}

fn hash32_registration() -> ExtensionTypeRegistrationRef {
    ExtensionTypeRegistration::new_arc(Hash32Extension::NAME, |storage_type, metadata| {
        Hash32Extension::deserialize_metadata(metadata)?;
        Hash32Extension::try_new(storage_type, ())?;
        Ok(Arc::new(DataFusionCodeFabricExtension {
            storage_type: storage_type.clone(),
            metadata: Hash32Extension::METADATA_V1.to_owned(),
        }))
    })
}

include!("generated/id_domains.rs");

/// Return the complete generated logical ID-domain registry.
#[must_use]
pub const fn id_domains() -> &'static [GeneratedIdDomainSpec] {
    GENERATED_ID_DOMAINS
}

/// Create one DataFusion registration factory for every generated logical domain.
#[must_use]
pub fn extension_type_registrations() -> Vec<ExtensionTypeRegistrationRef> {
    generated_id_domain_registrations()
}

/// Resolve a generated extension name to its application ID domain.
#[must_use]
pub fn id_domain_for_extension_name(extension_name: &str) -> Option<&'static str> {
    GENERATED_ID_DOMAINS
        .iter()
        .find(|domain| domain.extension_name == extension_name)
        .map(|domain| domain.domain_slug)
}

/// One domain-tagged fixed-size literal for logical-plan construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainTypedLiteral {
    domain: &'static GeneratedIdDomainSpec,
    value: [u8; 16],
}

impl DomainTypedLiteral {
    /// Bind a canonical value to one generated ID domain.
    ///
    /// # Errors
    ///
    /// Returns an argument error when `domain_slug` is not present in the generated registry.
    pub fn new(domain_slug: &str, value: [u8; 16]) -> Result<Self, ArrowError> {
        let domain = GENERATED_ID_DOMAINS
            .iter()
            .find(|domain| domain.domain_slug == domain_slug)
            .ok_or_else(|| {
                ArrowError::InvalidArgumentError(format!(
                    "unknown generated ID domain {domain_slug}"
                ))
            })?;
        Ok(Self { domain, value })
    }

    /// Lower the literal with the same Arrow extension metadata as its field authority.
    #[must_use]
    pub fn into_expr(self) -> Expr {
        let metadata = FieldMetadata::from(std::collections::BTreeMap::from([
            (
                arrow_schema::extension::EXTENSION_TYPE_NAME_KEY.to_owned(),
                self.domain.extension_name.to_owned(),
            ),
            (
                arrow_schema::extension::EXTENSION_TYPE_METADATA_KEY.to_owned(),
                format!(
                    "{{\"domain\":\"{}\",\"preimage_recipe_id\":\"{}\",\"preimage_version\":\"{}\"}}",
                    self.domain.domain_slug,
                    self.domain.preimage_recipe_id,
                    self.domain.preimage_version,
                ),
            ),
        ]));
        Expr::Literal(
            ScalarValue::FixedSizeBinary(16, Some(self.value.to_vec())),
            Some(metadata),
        )
    }
}

/// Validate any generated ID/hash field without selecting a second policy path.
///
/// # Errors
///
/// Returns an argument error for an unregistered extension name or a physical width that differs
/// from the generated logical-type contract.
pub fn validate_logical_extension_field(field: &Field) -> Result<(), ArrowError> {
    let Some(name) = field.extension_type_name() else {
        return Ok(());
    };
    let expected_width = if name == Hash32Extension::NAME {
        32
    } else if GENERATED_ID_DOMAINS
        .iter()
        .any(|domain| domain.extension_name == name)
    {
        16
    } else {
        return Err(ArrowError::InvalidArgumentError(format!(
            "unregistered CodeFabric extension type {name}"
        )));
    };
    if field.data_type() != &DataType::FixedSizeBinary(expected_width) {
        return Err(ArrowError::InvalidArgumentError(format!(
            "{name} requires FixedSizeBinary({expected_width}), received {}",
            field.data_type()
        )));
    }
    Ok(())
}

/// Durable Delta mutation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableMutationClass {
    StaticDimension,
    CurrentSingleton,
    OwnerReplacedFact,
    PublicationAppend,
    DerivedOwnerReplaced,
    GlobalDerivedReplacement,
}

/// Hot-overlay mutation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayMutationPolicy {
    OwnerReplace,
    PrimaryKeyUpsert,
    FullTableReplace,
    BaseImmutable,
    NotApplicable,
}

/// Query-visible materialization role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializationRole {
    DurableEffective,
    BundleDimension,
    QueryTimeDerived,
    OperationalProjection,
}

/// Closed role in the acyclic durable-publication manifest graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPinRole {
    PinnedData,
    ManifestControl,
    PointerControl,
    NotPublished,
}

/// Exhaustive P21 classification of one schema-IR annotation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataClass {
    Enforced,
    PlannerConsumed,
    Contractual,
    Governance,
    Lineage,
    Advisory,
}

/// Generated metadata classification and its concrete non-advisory consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataAnnotationSpec {
    pub annotation: &'static str,
    pub class: MetadataClass,
    pub consumer_path: Option<&'static str>,
    pub consumer_symbol: Option<&'static str>,
}

/// Authority kind that gives one semantic field type its governed meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAuthority {
    EnumRegistry,
    TypeAlgebra,
    OntologyEntityRegistry,
    OntologyRelationRegistry,
    OntologyPropertyRegistry,
    OntologyFactRegistry,
    CapabilityRegistry,
    SchemaIr,
    Intrinsic,
    ProviderCatalog,
    DiagnosticProtocol,
}

/// Generated semantic-type resolution and authority digest link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTypeBindingSpec {
    pub semantic_type: &'static str,
    pub authority: SemanticAuthority,
    pub domain: Option<&'static str>,
    pub authority_artifact_id: Option<&'static str>,
    pub authority_digest: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalStructureClass {
    StructurallyOwnedCohesive,
    IndependentRelation,
    IndependentlyFilterable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalStructureLowering {
    FlatColumns,
    RelationTable,
    TaggedColumns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructureGroupSpec {
    pub group_id: &'static str,
    pub table_codes: &'static [i16],
    pub columns: &'static [&'static str],
    pub logical_class: LogicalStructureClass,
    pub physical_lowering: PhysicalStructureLowering,
    pub validation_rule_id: Option<&'static str>,
}

/// Typed cross-table contract. Enforcement belongs to complete candidate publication state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignKeyContract {
    pub source_table_code: i16,
    pub source_column_index: usize,
    pub source_column: &'static str,
    pub target_table_code: i16,
    pub target_column_index: usize,
    pub target_column: &'static str,
}

/// Generated exact-pin schema-evolution policy consumed by Delta table authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaEvolutionPolicy {
    pub require_schema_digest_equality: bool,
    pub allow_type_widening: bool,
    pub column_mapping_mode: &'static str,
}

/// One immutable generated schema contract.
#[derive(Clone, Debug)]
pub struct TableSpec {
    pub table_code: i16,
    pub name: &'static str,
    pub family: &'static str,
    pub grain: &'static str,
    pub schema_version: &'static str,
    pub schema_digest: String,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub partition_columns: &'static [&'static str],
    pub zorder_columns: &'static [&'static str],
    pub durable_mutation: DurableMutationClass,
    pub overlay_mutation: OverlayMutationPolicy,
    pub materialization_role: MaterializationRole,
    pub publication_pin_role: PublicationPinRole,
    pub dependencies: &'static [i16],
    pub required_for_publication: bool,
}

/// Generated row-scope selectors applied at publication and provider construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableScopeSpec {
    pub table_code: i16,
    pub workspace_column: Option<&'static str>,
    pub analysis_context_column: Option<&'static str>,
    pub source_generation_column: Option<&'static str>,
    pub analysis_context_set_column: Option<&'static str>,
    pub owner_column: Option<&'static str>,
}

/// Closed role of one generated serving projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServingProjectionRole {
    EffectiveFact,
}

/// One generated `cpg_serving` projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingProjectionSpec {
    pub view_name: &'static str,
    pub source_table_code: i16,
    pub availability_wave: u16,
    pub projection_role: ServingProjectionRole,
}

/// Closed normalization applied to operational foreign keys before effective-state comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonForeignKeyNormalization {
    /// Replace provider-run-derived evidence and observation locators with one digest of the
    /// complete projected semantic evidence row.
    EvidenceContentIdentityV1,
}

/// Schema-registry-owned canonical comparison projection for one effective table.
///
/// The released comparison-ignore registry remains the sole column-exclusion authority. This
/// record supplies the governed primary sort key and any required operational foreign-key
/// normalization without duplicating ignored field names per table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonProjectionSpec {
    pub table_code: i16,
    pub primary_sort_key: &'static [&'static str],
    pub foreign_key_normalization: Option<ComparisonForeignKeyNormalization>,
}

/// Closed implementation role for one generated control projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlProjectionRole {
    OperationalSource,
    DerivedOperational,
    ActiveServingSnapshot,
}

/// One generated `cpg_control` projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlProjectionSpec {
    pub view_name: &'static str,
    pub availability_wave: u16,
    pub projection_role: ControlProjectionRole,
    pub source_table: Option<&'static str>,
    pub columns: &'static [&'static str],
}

/// Generated non-timing resource limits for serving and candidate construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingResourceProfile {
    pub batch_size: usize,
    pub max_execution_millis: u64,
    pub max_output_rows: usize,
    pub max_output_bytes: usize,
    pub max_output_batches: usize,
    pub max_control_rows: usize,
    pub max_control_bytes: usize,
    pub max_control_batches: usize,
    pub max_snapshot_validation_rows: usize,
    pub max_snapshot_validation_bytes: usize,
    pub max_snapshot_validation_batches: usize,
}

/// `SQLite` affinity mapped to one query-visible Arrow physical type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalSqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

/// One generated read-only operational projection schema.
#[derive(Clone, Debug)]
pub struct OperationalTableSpec {
    pub name: &'static str,
    pub sqlite_ddl: &'static str,
    pub sqlite_column_types: Vec<OperationalSqliteType>,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub workspace_scope: Option<OperationalWorkspaceScope>,
}

/// Generated route from an operational row to its owning workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalWorkspaceScope {
    Direct {
        workspace_column: &'static str,
    },
    ViaParent {
        parent_table: &'static str,
        child_column: &'static str,
        parent_column: &'static str,
        workspace_column: &'static str,
    },
}

impl TableSpec {
    /// Construct the DataFusion planning schema without changing field identity.
    ///
    /// # Errors
    ///
    /// Returns DataFusion's schema validation error for an invalid Arrow schema.
    pub fn datafusion_schema(
        &self,
    ) -> Result<datafusion::common::DFSchema, datafusion::common::DataFusionError> {
        datafusion::common::DFSchema::try_from(self.arrow_schema.clone())
    }

    /// Validate that this Arrow contract has an exact Delta Kernel mapping.
    ///
    /// # Errors
    ///
    /// Returns Arrow's conversion error when a physical type is not Delta-compatible.
    pub fn validate_delta_compatibility(&self) -> Result<(), arrow_schema::ArrowError> {
        crate::fabric::validate_delta_schema(&self.arrow_schema)
    }

    /// Report whether a candidate Arrow schema satisfies the generated exact-pin policy.
    #[must_use]
    pub fn accepts_schema(&self, candidate: &SchemaRef) -> bool {
        let policy = schema_evolution_policy();
        policy.require_schema_digest_equality
            && !policy.allow_type_widening
            && policy.column_mapping_mode == "none"
            && candidate == &self.arrow_schema
    }
}

fn schema_digest(schema: &SchemaRef) -> String {
    crate::fabric::delta_schema_digest(schema)
        .expect("generated TableSpec must have a canonical Delta schema identity")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // The generated IR owns the full reusable §7 logical-type vocabulary.
pub enum LogicalType {
    Id16,
    Hash32,
    Code16,
    Code32,
    Bucket16,
    Int16,
    Int32,
    Int64,
    UInt64,
    Float64,
    Boolean,
    Utf8,
    Binary,
    TimestampUtc,
    IdList,
    Int64List,
    StringMap,
}

#[derive(Clone, Copy)]
struct GeneratedColumn {
    name: &'static str,
    logical_type: LogicalType,
    nullable: bool,
    id_domain: Option<&'static str>,
    element_id_domain: Option<&'static str>,
    semantic_type: Option<&'static str>,
    foreign_key: Option<&'static str>,
    hidden_operational: bool,
}

#[derive(Clone, Copy)]
#[allow(dead_code)] // Legacy metadata remains only until the model driver projects every role.
struct GeneratedTableSpec {
    table_code: i16,
    name: &'static str,
    family: &'static str,
    grain: &'static str,
    schema_version: &'static str,
    columns: &'static [GeneratedColumn],
    primary_key: &'static [&'static str],
    partition_columns: &'static [&'static str],
    zorder_columns: &'static [&'static str],
    durable_mutation: DurableMutationClass,
    overlay_mutation: OverlayMutationPolicy,
    materialization_role: MaterializationRole,
    publication_pin_role: PublicationPinRole,
    dependencies: &'static [i16],
    required_for_publication: bool,
}

#[derive(Clone, Copy)]
struct GeneratedResultSchemaSpec {
    result_schema_id: &'static str,
    query_form_code: u16,
    result_role: &'static str,
    version: &'static str,
    fields: &'static [GeneratedColumn],
}

/// Read-only compiled view of one table or result column contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColumnContractSpec {
    pub name: &'static str,
    pub logical_type: LogicalType,
    pub nullable: bool,
    pub id_domain: Option<&'static str>,
    pub element_id_domain: Option<&'static str>,
    pub semantic_type: Option<&'static str>,
    pub foreign_key: Option<&'static str>,
    pub hidden_operational: bool,
}

/// Read-only compiled result contract used by the ontology plane and response shaper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultSchemaContractSpec {
    pub result_schema_id: &'static str,
    pub query_form_code: u16,
    pub result_role: &'static str,
    pub version: &'static str,
    pub fields: &'static [ColumnContractSpec],
}

#[derive(Clone, Copy)]
struct GeneratedOperationalColumn {
    name: &'static str,
    sqlite_type: OperationalSqliteType,
    logical_type: LogicalType,
    id_domain: Option<&'static str>,
    nullable: bool,
}

#[derive(Clone, Copy)]
struct GeneratedOperationalTableSpec {
    name: &'static str,
    sqlite_ddl: &'static str,
    columns: &'static [GeneratedOperationalColumn],
    primary_key: &'static [&'static str],
    workspace_scope: Option<OperationalWorkspaceScope>,
}

include!("generated/table_specs.rs");
include!("generated/result_schemas.rs");

const fn column_contract(column: GeneratedColumn) -> ColumnContractSpec {
    ColumnContractSpec {
        name: column.name,
        logical_type: column.logical_type,
        nullable: column.nullable,
        id_domain: column.id_domain,
        element_id_domain: column.element_id_domain,
        semantic_type: column.semantic_type,
        foreign_key: column.foreign_key,
        hidden_operational: column.hidden_operational,
    }
}

/// Return the single generated column authority for one durable table.
#[must_use]
pub fn table_column_contracts(table_code: i16) -> Option<&'static [ColumnContractSpec]> {
    static CONTRACTS: OnceLock<HashMap<i16, Box<[ColumnContractSpec]>>> = OnceLock::new();
    CONTRACTS
        .get_or_init(|| {
            GENERATED_TABLE_SPECS
                .iter()
                .map(|table| {
                    (
                        table.table_code,
                        table
                            .columns
                            .iter()
                            .copied()
                            .map(column_contract)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    )
                })
                .collect()
        })
        .get(&table_code)
        .map(AsRef::as_ref)
}

/// Return every compiled query-result contract without reparsing its source authority.
#[must_use]
pub fn result_schema_contracts() -> &'static [ResultSchemaContractSpec] {
    static CONTRACTS: OnceLock<Vec<ResultSchemaContractSpec>> = OnceLock::new();
    CONTRACTS.get_or_init(|| {
        GENERATED_RESULT_SCHEMAS
            .iter()
            .map(|schema| {
                let fields = schema
                    .fields
                    .iter()
                    .copied()
                    .map(column_contract)
                    .collect::<Vec<_>>()
                    .leak();
                ResultSchemaContractSpec {
                    result_schema_id: schema.result_schema_id,
                    query_form_code: schema.query_form_code,
                    result_role: schema.result_role,
                    version: schema.version,
                    fields,
                }
            })
            .collect()
    })
}

/// Resolve one governed query-result Arrow schema through the common field lowering.
#[must_use]
pub fn result_schema(result_schema_id: &str) -> Option<SchemaRef> {
    GENERATED_RESULT_SCHEMAS
        .iter()
        .find(|schema| schema.result_schema_id == result_schema_id)
        .map(|schema| {
            let metadata = HashMap::from([
                (
                    "com.codefabric.cpg.result_schema_id".to_owned(),
                    schema.result_schema_id.to_owned(),
                ),
                (
                    "com.codefabric.cpg.query_form_code".to_owned(),
                    schema.query_form_code.to_string(),
                ),
                (
                    "com.codefabric.cpg.result_role".to_owned(),
                    schema.result_role.to_owned(),
                ),
                (
                    "com.codefabric.cpg.result_schema_version".to_owned(),
                    schema.version.to_owned(),
                ),
            ]);
            Arc::new(Schema::new_with_metadata(
                schema
                    .fields
                    .iter()
                    .copied()
                    .map(|column| field(column, &[]))
                    .collect::<Vec<_>>(),
                metadata,
            ))
        })
}

/// Project an internal dependency schema from fields owned by one generated result contract.
/// This keeps helper plans on the same logical-type and metadata lowering without minting a
/// second hand-written field authority.
#[must_use]
pub fn project_result_schema(result_schema_id: &str, names: &[&str]) -> Option<SchemaRef> {
    let schema = GENERATED_RESULT_SCHEMAS
        .iter()
        .find(|schema| schema.result_schema_id == result_schema_id)?;
    let fields = names
        .iter()
        .map(|name| {
            schema
                .fields
                .iter()
                .copied()
                .find(|field| field.name == *name)
                .map(|field| super::schema_registry::field(field, &[]))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Arc::new(Schema::new_with_metadata(
        fields,
        HashMap::from([(
            "com.codefabric.cpg.projected_result_schema_id".to_owned(),
            result_schema_id.to_owned(),
        )]),
    )))
}

/// Return the IR-owned ontology version projected into every table schema.
#[must_use]
pub const fn ontology_version() -> &'static str {
    GENERATED_ONTOLOGY_VERSION
}

/// Canonical identity of the compiled Schema Contract IR authority.
#[must_use]
pub const fn schema_contract_digest() -> &'static str {
    GENERATED_SCHEMA_CONTRACT_DIGEST
}

/// Return the IR-owned compatibility mode projected into every table schema.
#[must_use]
pub const fn compatibility_mode() -> &'static str {
    GENERATED_COMPATIBILITY_MODE
}

/// Return the generated six-class metadata dictionary.
#[must_use]
pub const fn metadata_dictionary() -> &'static [MetadataAnnotationSpec] {
    GENERATED_METADATA_DICTIONARY
}

/// Resolve one semantic-type annotation to its governed authority.
#[must_use]
pub fn semantic_type_binding(semantic_type: &str) -> Option<&'static SemanticTypeBindingSpec> {
    GENERATED_SEMANTIC_TYPE_BINDINGS
        .iter()
        .find(|binding| binding.semantic_type == semantic_type)
}

/// Return every compiled semantic-type resolver binding.
#[must_use]
pub const fn semantic_type_bindings() -> &'static [SemanticTypeBindingSpec] {
    GENERATED_SEMANTIC_TYPE_BINDINGS
}

#[must_use]
pub const fn structure_groups() -> &'static [StructureGroupSpec] {
    GENERATED_STRUCTURE_GROUPS
}

#[must_use]
pub fn structure_class(table_code: i16, column: &str) -> Option<LogicalStructureClass> {
    GENERATED_STRUCTURE_GROUPS
        .iter()
        .find(|group| group.table_codes.contains(&table_code) && group.columns.contains(&column))
        .map(|group| group.logical_class)
}

/// Return every generated typed foreign-key contract.
#[must_use]
pub const fn foreign_key_contracts() -> &'static [ForeignKeyContract] {
    GENERATED_FOREIGN_KEY_CONTRACTS
}

/// Return the exact-pin schema evolution policy.
#[must_use]
pub const fn schema_evolution_policy() -> SchemaEvolutionPolicy {
    SchemaEvolutionPolicy {
        require_schema_digest_equality: GENERATED_REQUIRE_SCHEMA_DIGEST_EQUALITY,
        allow_type_widening: GENERATED_ALLOW_TYPE_WIDENING,
        column_mapping_mode: GENERATED_COLUMN_MAPPING_MODE,
    }
}

fn physical_type(logical: LogicalType) -> DataType {
    match logical {
        LogicalType::Id16 => DataType::FixedSizeBinary(16),
        LogicalType::Hash32 => DataType::FixedSizeBinary(32),
        LogicalType::Binary => DataType::Binary,
        LogicalType::Code16 | LogicalType::Bucket16 | LogicalType::Int16 => DataType::Int16,
        LogicalType::Code32 | LogicalType::Int32 => DataType::Int32,
        LogicalType::Int64 => DataType::Int64,
        LogicalType::UInt64 => DataType::UInt64,
        LogicalType::Float64 => DataType::Float64,
        LogicalType::Boolean => DataType::Boolean,
        LogicalType::Utf8 => DataType::Utf8,
        LogicalType::TimestampUtc => {
            DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC")))
        }
        LogicalType::IdList => {
            // Delta's Arrow conversion canonicalizes list children to `element`.
            // Emit that library-native name so the generated schema round-trips exactly.
            DataType::List(Arc::new(Field::new(
                "element",
                DataType::FixedSizeBinary(16),
                false,
            )))
        }
        LogicalType::Int64List => {
            DataType::List(Arc::new(Field::new("element", DataType::Int64, false)))
        }
        LogicalType::StringMap => DataType::Map(
            Arc::new(Field::new(
                "entries",
                DataType::Struct(Fields::from(vec![
                    Field::new("key", DataType::Utf8, false),
                    Field::new("value", DataType::Utf8, false),
                ])),
                false,
            )),
            false,
        ),
    }
}

fn field(contract: GeneratedColumn, primary_key: &[&str]) -> Field {
    let mut metadata = HashMap::new();
    if matches!(contract.logical_type, LogicalType::Id16) {
        metadata.insert("com.codefabric.cpg.id_width".to_owned(), "16".to_owned());
    }
    if let Some(value) = contract.semantic_type {
        metadata.insert(
            "com.codefabric.cpg.semantic_type".to_owned(),
            value.to_owned(),
        );
    }
    if let Some(value) = contract.foreign_key {
        metadata.insert(
            "com.codefabric.cpg.foreign_key".to_owned(),
            value.to_owned(),
        );
    }
    if primary_key.contains(&contract.name) {
        metadata.insert(
            "com.codefabric.cpg.primary_key_part".to_owned(),
            "true".to_owned(),
        );
    }
    if contract.hidden_operational {
        metadata.insert(
            "com.codefabric.cpg.hidden_operational".to_owned(),
            "true".to_owned(),
        );
    }
    let mut data_type = physical_type(contract.logical_type);
    if let (LogicalType::IdList, Some(domain)) = (contract.logical_type, contract.element_id_domain)
    {
        let DataType::List(element) = data_type else {
            unreachable!("IdList lowering always yields a List")
        };
        data_type = DataType::List(Arc::new(
            attach_generated_id_domain(element.as_ref().clone(), domain)
                .expect("validated generated ID-list domain"),
        ));
    }
    let field = Field::new(contract.name, data_type, contract.nullable).with_metadata(metadata);
    match contract.logical_type {
        LogicalType::Id16 => attach_generated_id_domain(
            field,
            contract
                .id_domain
                .expect("validated generated scalar ID domain"),
        )
        .expect("validated generated scalar ID extension"),
        LogicalType::Hash32 => field.with_extension_type(Hash32Extension::v1()),
        _ => field,
    }
}

const fn durable_name(value: DurableMutationClass) -> &'static str {
    match value {
        DurableMutationClass::StaticDimension => "STATIC_DIMENSION",
        DurableMutationClass::CurrentSingleton => "CURRENT_SINGLETON",
        DurableMutationClass::OwnerReplacedFact => "OWNER_REPLACED_FACT",
        DurableMutationClass::PublicationAppend => "PUBLICATION_APPEND",
        DurableMutationClass::DerivedOwnerReplaced => "DERIVED_OWNER_REPLACED",
        DurableMutationClass::GlobalDerivedReplacement => "GLOBAL_DERIVED_REPLACEMENT",
    }
}

const fn overlay_name(value: OverlayMutationPolicy) -> &'static str {
    match value {
        OverlayMutationPolicy::OwnerReplace => "OWNER_REPLACE",
        OverlayMutationPolicy::PrimaryKeyUpsert => "PRIMARY_KEY_UPSERT",
        OverlayMutationPolicy::FullTableReplace => "FULL_TABLE_REPLACE",
        OverlayMutationPolicy::BaseImmutable => "BASE_IMMUTABLE",
        OverlayMutationPolicy::NotApplicable => "NOT_APPLICABLE",
    }
}

const fn materialization_name(value: MaterializationRole) -> &'static str {
    match value {
        MaterializationRole::DurableEffective => "DURABLE_EFFECTIVE",
        MaterializationRole::BundleDimension => "BUNDLE_DIMENSION",
        MaterializationRole::QueryTimeDerived => "QUERY_TIME_DERIVED",
        MaterializationRole::OperationalProjection => "OPERATIONAL_PROJECTION",
    }
}

const fn publication_pin_name(value: PublicationPinRole) -> &'static str {
    match value {
        PublicationPinRole::PinnedData => "PINNED_DATA",
        PublicationPinRole::ManifestControl => "MANIFEST_CONTROL",
        PublicationPinRole::PointerControl => "POINTER_CONTROL",
        PublicationPinRole::NotPublished => "NOT_PUBLISHED",
    }
}

fn build(contract: GeneratedTableSpec) -> TableSpec {
    let metadata = HashMap::from([
        (
            "com.codefabric.cpg.table_name".to_owned(),
            contract.name.to_owned(),
        ),
        (
            "com.codefabric.cpg.table_family".to_owned(),
            contract.family.to_owned(),
        ),
        (
            "com.codefabric.cpg.table_grain".to_owned(),
            contract.grain.to_owned(),
        ),
        (
            "com.codefabric.cpg.schema_version".to_owned(),
            contract.schema_version.to_owned(),
        ),
        (
            "com.codefabric.cpg.ontology_version".to_owned(),
            ontology_version().to_owned(),
        ),
        (
            "com.codefabric.cpg.primary_key".to_owned(),
            contract.primary_key.join(","),
        ),
        (
            "com.codefabric.cpg.partition_columns".to_owned(),
            contract.partition_columns.join(","),
        ),
        (
            "com.codefabric.cpg.durable_mutation_class".to_owned(),
            durable_name(contract.durable_mutation).to_owned(),
        ),
        (
            "com.codefabric.cpg.overlay_mutation_policy".to_owned(),
            overlay_name(contract.overlay_mutation).to_owned(),
        ),
        (
            "com.codefabric.cpg.materialization_role".to_owned(),
            materialization_name(contract.materialization_role).to_owned(),
        ),
        (
            "com.codefabric.cpg.publication_pin_role".to_owned(),
            publication_pin_name(contract.publication_pin_role).to_owned(),
        ),
        (
            "com.codefabric.cpg.compatibility_mode".to_owned(),
            compatibility_mode().to_owned(),
        ),
    ]);
    let fields = contract
        .columns
        .iter()
        .copied()
        .map(|column| field(column, contract.primary_key))
        .collect::<Vec<_>>();
    let arrow_schema = Arc::new(Schema::new_with_metadata(fields, metadata));
    TableSpec {
        table_code: contract.table_code,
        name: contract.name,
        family: contract.family,
        grain: contract.grain,
        schema_version: contract.schema_version,
        schema_digest: schema_digest(&arrow_schema),
        arrow_schema,
        primary_key: contract.primary_key,
        partition_columns: contract.partition_columns,
        zorder_columns: contract.zorder_columns,
        durable_mutation: contract.durable_mutation,
        overlay_mutation: contract.overlay_mutation,
        materialization_role: contract.materialization_role,
        publication_pin_role: contract.publication_pin_role,
        dependencies: contract.dependencies,
        required_for_publication: contract.required_for_publication,
    }
}

/// Return the process-wide immutable `TableSpec` set.
#[must_use]
pub fn table_specs() -> &'static [TableSpec] {
    static TABLE_SPECS: OnceLock<Vec<TableSpec>> = OnceLock::new();
    TABLE_SPECS.get_or_init(|| GENERATED_TABLE_SPECS.iter().copied().map(build).collect())
}

/// Return the deterministic dependency-closed table order used for creation and publication.
///
/// # Panics
///
/// Panics if the generated table dependency graph contains a cycle. Model assurance rejects
/// such a graph before runtime artifacts are released.
#[must_use]
pub fn table_dependency_order() -> &'static [i16] {
    static ORDER: OnceLock<Vec<i16>> = OnceLock::new();
    ORDER.get_or_init(|| {
        let mut ordered = Vec::with_capacity(table_specs().len());
        while ordered.len() < table_specs().len() {
            let before = ordered.len();
            for spec in table_specs() {
                if !ordered.contains(&spec.table_code)
                    && spec
                        .dependencies
                        .iter()
                        .all(|dependency| ordered.contains(dependency))
                {
                    ordered.push(spec.table_code);
                }
            }
            assert_ne!(
                before,
                ordered.len(),
                "generated table dependency graph must be acyclic"
            );
        }
        ordered
    })
}

/// Resolve a table by stable numeric code.
#[must_use]
pub fn table_spec(table_code: i16) -> Option<&'static TableSpec> {
    table_specs()
        .iter()
        .find(|table| table.table_code == table_code)
}

/// Resolve generated row-scope selectors by table code.
#[must_use]
pub fn table_scope_spec(table_code: i16) -> Option<&'static TableScopeSpec> {
    GENERATED_TABLE_SCOPE_SPECS
        .iter()
        .find(|scope| scope.table_code == table_code)
}

/// Return every generated Wave-owned serving projection.
#[must_use]
pub const fn serving_projection_specs() -> &'static [ServingProjectionSpec] {
    GENERATED_SERVING_PROJECTION_SPECS
}

/// Resolve the canonical comparison projection for every current effective serving table and
/// diagnostic evidence table.
#[must_use]
pub fn comparison_projection_spec(table_code: i16) -> Option<ComparisonProjectionSpec> {
    let included = table_code == 10
        || serving_projection_specs()
            .iter()
            .any(|projection| projection.source_table_code == table_code);
    let table = included.then(|| table_spec(table_code)).flatten()?;
    Some(ComparisonProjectionSpec {
        table_code,
        primary_sort_key: table.primary_key,
        foreign_key_normalization: (table_code == 130)
            .then_some(ComparisonForeignKeyNormalization::EvidenceContentIdentityV1),
    })
}

/// Return every generated control projection.
#[must_use]
pub const fn control_projection_specs() -> &'static [ControlProjectionSpec] {
    GENERATED_CONTROL_PROJECTION_SPECS
}

/// Return the generated serving resource profile.
#[must_use]
pub const fn serving_resource_profile() -> ServingResourceProfile {
    GENERATED_SERVING_RESOURCE_PROFILE
}

fn build_operational(contract: GeneratedOperationalTableSpec) -> OperationalTableSpec {
    let fields = contract
        .columns
        .iter()
        .map(|column| {
            field(
                GeneratedColumn {
                    name: column.name,
                    logical_type: column.logical_type,
                    nullable: column.nullable,
                    id_domain: column.id_domain,
                    element_id_domain: None,
                    semantic_type: column.id_domain.map(|_| "id16"),
                    foreign_key: None,
                    hidden_operational: false,
                },
                contract.primary_key,
            )
        })
        .collect::<Vec<_>>();
    OperationalTableSpec {
        name: contract.name,
        sqlite_ddl: contract.sqlite_ddl,
        sqlite_column_types: contract
            .columns
            .iter()
            .map(|column| column.sqlite_type)
            .collect(),
        arrow_schema: Arc::new(Schema::new(fields)),
        primary_key: contract.primary_key,
        workspace_scope: contract.workspace_scope,
    }
}

/// Return every generated operational-store projection in source order.
#[must_use]
pub fn operational_table_specs() -> &'static [OperationalTableSpec] {
    static OPERATIONAL_SPECS: OnceLock<Vec<OperationalTableSpec>> = OnceLock::new();
    OPERATIONAL_SPECS.get_or_init(|| {
        GENERATED_OPERATIONAL_TABLE_SPECS
            .iter()
            .copied()
            .map(build_operational)
            .collect()
    })
}

/// Resolve one generated operational-store projection by table name.
#[must_use]
pub fn operational_table_spec(name: &str) -> Option<&'static OperationalTableSpec> {
    operational_table_specs()
        .iter()
        .find(|table| table.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_id16_batch(schema: SchemaRef) -> arrow::record_batch::RecordBatch {
        use arrow::array::{ArrayRef, FixedSizeBinaryBuilder};

        let mut values = FixedSizeBinaryBuilder::with_capacity(1, 16);
        values.append_value([0x58; 16]).unwrap();
        arrow::record_batch::RecordBatch::try_new(
            schema,
            vec![Arc::new(values.finish()) as ArrayRef],
        )
        .unwrap()
    }

    fn synthetic_wave4_batch(table: &TableSpec) -> arrow::record_batch::RecordBatch {
        use arrow::array::{
            ArrayRef, BinaryArray, BooleanArray, FixedSizeBinaryBuilder, Int16Array, Int32Array,
            Int64Array, Int64Builder, ListBuilder, StringArray,
        };

        const BYTES: [u8; 32] = [7; 32];
        let columns = table
            .arrow_schema
            .fields()
            .iter()
            .map(|field| -> ArrayRef {
                match field.data_type() {
                    DataType::Binary => Arc::new(BinaryArray::from(vec![Some(BYTES.as_slice())])),
                    DataType::FixedSizeBinary(16) => {
                        let mut values = FixedSizeBinaryBuilder::with_capacity(1, 16);
                        values.append_value(&BYTES[..16]).unwrap();
                        Arc::new(values.finish())
                    }
                    DataType::FixedSizeBinary(32) => {
                        let mut values = FixedSizeBinaryBuilder::with_capacity(1, 32);
                        values.append_value(BYTES).unwrap();
                        Arc::new(values.finish())
                    }
                    DataType::Int16 => Arc::new(Int16Array::from(vec![0])),
                    DataType::Int32 => Arc::new(Int32Array::from(vec![0])),
                    DataType::Int64 => Arc::new(Int64Array::from(vec![0])),
                    DataType::Boolean => Arc::new(BooleanArray::from(vec![false])),
                    DataType::Utf8 => Arc::new(StringArray::from(vec!["value"])),
                    DataType::List(element) if element.data_type() == &DataType::Int64 => {
                        let mut values =
                            ListBuilder::new(Int64Builder::new()).with_field(Arc::clone(element));
                        values.values().append_value(0);
                        values.append(true);
                        Arc::new(values.finish())
                    }
                    DataType::List(element)
                        if element.data_type() == &DataType::FixedSizeBinary(16) =>
                    {
                        let mut values = ListBuilder::new(FixedSizeBinaryBuilder::new(16))
                            .with_field(Arc::clone(element));
                        values.values().append_value(&BYTES[..16]).unwrap();
                        values.append(true);
                        Arc::new(values.finish())
                    }
                    other => panic!("unhandled Wave-4 synthetic type {other:?}"),
                }
            })
            .collect();
        arrow::record_batch::RecordBatch::try_new(table.arrow_schema.clone(), columns).unwrap()
    }

    fn synthetic_result_batch(schema: &SchemaRef) -> arrow::record_batch::RecordBatch {
        use arrow::array::{
            ArrayRef, BinaryArray, FixedSizeBinaryBuilder, Int16Array, Int32Array, Int64Array,
            ListBuilder, StringArray, UInt64Array,
        };

        const BYTES: [u8; 32] = [0x5a; 32];
        let columns = schema
            .fields()
            .iter()
            .map(|field| -> ArrayRef {
                match field.data_type() {
                    DataType::Binary => Arc::new(BinaryArray::from(vec![Some(BYTES.as_slice())])),
                    DataType::FixedSizeBinary(width) => {
                        let mut values = FixedSizeBinaryBuilder::with_capacity(1, *width);
                        values
                            .append_value(
                                &BYTES[..usize::try_from(*width).expect("positive width")],
                            )
                            .unwrap();
                        Arc::new(values.finish())
                    }
                    DataType::Int16 => Arc::new(Int16Array::from(vec![10])),
                    DataType::Int32 => Arc::new(Int32Array::from(vec![10])),
                    DataType::Int64 => Arc::new(Int64Array::from(vec![1])),
                    DataType::UInt64 => Arc::new(UInt64Array::from(vec![1])),
                    DataType::Utf8 => Arc::new(StringArray::from(vec!["value"])),
                    DataType::List(element)
                        if element.data_type() == &DataType::FixedSizeBinary(16) =>
                    {
                        let mut values = ListBuilder::new(FixedSizeBinaryBuilder::new(16))
                            .with_field(Arc::clone(element));
                        values.values().append_value(&BYTES[..16]).unwrap();
                        values.append(true);
                        Arc::new(values.finish())
                    }
                    other => panic!("unhandled generated result type {other:?}"),
                }
            })
            .collect();
        arrow::record_batch::RecordBatch::try_new(Arc::clone(schema), columns).unwrap()
    }

    #[test]
    fn generated_result_schema_arrow_roundtrip() {
        use std::io::Cursor;

        use arrow::ipc::reader::StreamReader;
        use arrow::ipc::writer::StreamWriter;

        let contracts = result_schema_contracts();
        assert_eq!(contracts.len(), 8);
        for contract in contracts {
            let schema = result_schema(contract.result_schema_id).expect("generated result schema");
            assert_eq!(schema.fields().len(), contract.fields.len());
            assert_eq!(
                schema.metadata()["com.codefabric.cpg.query_form_code"],
                contract.query_form_code.to_string()
            );
            let batch = synthetic_result_batch(&schema);
            assert_eq!(batch.schema(), schema);

            for field in schema.fields() {
                match field.data_type() {
                    DataType::FixedSizeBinary(16) => assert!(
                        field
                            .metadata()
                            .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                            .is_some_and(
                                |name| name.starts_with("codefabric.") && name.ends_with("_id")
                            )
                    ),
                    DataType::FixedSizeBinary(32) => assert_eq!(
                        field
                            .metadata()
                            .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                            .map(String::as_str),
                        Some("codefabric.hash32")
                    ),
                    DataType::List(element)
                        if element.data_type() == &DataType::FixedSizeBinary(16) =>
                    {
                        assert!(
                            element
                                .metadata()
                                .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                                .is_some_and(
                                    |name| name.starts_with("codefabric.") && name.ends_with("_id")
                                )
                        );
                    }
                    _ => {}
                }
            }

            let mut ipc = Vec::new();
            {
                let mut writer = StreamWriter::try_new(&mut ipc, &schema).unwrap();
                writer.write(&batch).unwrap();
                writer.finish().unwrap();
            }
            let replay = StreamReader::try_new(Cursor::new(ipc), None)
                .unwrap()
                .next()
                .unwrap()
                .unwrap();
            assert_eq!(replay.schema(), schema);
            assert_eq!(replay, batch);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Exhaustive generated-domain and consumer census.
    fn odf_id_domain_lowering_conformance() {
        use std::io::Cursor;

        use arrow::ipc::reader::StreamReader;
        use arrow::ipc::writer::StreamWriter;

        assert_eq!(extension_type_registrations().len(), id_domains().len() + 1);
        let mut observed_domains = std::collections::BTreeSet::new();
        for table in table_specs() {
            let contracts = table_column_contracts(table.table_code).unwrap();
            assert_eq!(contracts.len(), table.arrow_schema.fields().len());
            for (contract, field) in contracts.iter().zip(table.arrow_schema.fields()) {
                match contract.logical_type {
                    LogicalType::Id16 => {
                        let domain = contract.id_domain.expect("scalar ID domain");
                        let generated = id_domains()
                            .iter()
                            .find(|candidate| candidate.domain_slug == domain)
                            .expect("registered scalar ID domain");
                        assert_eq!(field.data_type(), &DataType::FixedSizeBinary(16));
                        assert_eq!(
                            field
                                .metadata()
                                .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                                .map(String::as_str),
                            Some(generated.extension_name)
                        );
                        assert!(DomainTypedLiteral::new(domain, [0x61; 16]).is_ok());
                        observed_domains.insert(domain);
                    }
                    LogicalType::IdList => {
                        let domain = contract.element_id_domain.expect("ID-list element domain");
                        let generated = id_domains()
                            .iter()
                            .find(|candidate| candidate.domain_slug == domain)
                            .expect("registered ID-list domain");
                        let DataType::List(element) = field.data_type() else {
                            panic!("{} is not a typed ID list", field.name())
                        };
                        assert_eq!(element.data_type(), &DataType::FixedSizeBinary(16));
                        assert_eq!(
                            element
                                .metadata()
                                .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                                .map(String::as_str),
                            Some(generated.extension_name)
                        );
                        observed_domains.insert(domain);
                    }
                    LogicalType::Hash32 => {
                        assert_eq!(field.data_type(), &DataType::FixedSizeBinary(32));
                        assert_eq!(
                            field
                                .metadata()
                                .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                                .map(String::as_str),
                            Some(Hash32Extension::NAME)
                        );
                    }
                    _ => {
                        assert!(contract.id_domain.is_none());
                        assert!(contract.element_id_domain.is_none());
                    }
                }
            }

            let batch =
                arrow::record_batch::RecordBatch::new_empty(Arc::clone(&table.arrow_schema));
            let mut ipc = Vec::new();
            {
                let mut writer = StreamWriter::try_new(&mut ipc, &table.arrow_schema).unwrap();
                writer.write(&batch).unwrap();
                writer.finish().unwrap();
            }
            let replay = StreamReader::try_new(Cursor::new(ipc), None)
                .unwrap()
                .next()
                .unwrap()
                .unwrap();
            assert_eq!(replay.schema(), table.arrow_schema);
            assert_eq!(
                table.datafusion_schema().unwrap().fields().len(),
                table.arrow_schema.fields().len()
            );
        }
        for contract in result_schema_contracts() {
            let schema = result_schema(contract.result_schema_id).unwrap();
            for (field_contract, field) in contract.fields.iter().zip(schema.fields()) {
                if let Some(domain) = field_contract.id_domain {
                    let generated = id_domains()
                        .iter()
                        .find(|candidate| candidate.domain_slug == domain)
                        .unwrap();
                    assert_eq!(
                        field
                            .metadata()
                            .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                            .map(String::as_str),
                        Some(generated.extension_name)
                    );
                    observed_domains.insert(domain);
                }
                if let Some(domain) = field_contract.element_id_domain {
                    let generated = id_domains()
                        .iter()
                        .find(|candidate| candidate.domain_slug == domain)
                        .unwrap();
                    let DataType::List(element) = field.data_type() else {
                        panic!("{} is not a typed ID list", field.name())
                    };
                    assert_eq!(
                        element
                            .metadata()
                            .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                            .map(String::as_str),
                        Some(generated.extension_name)
                    );
                    observed_domains.insert(domain);
                }
            }
        }
        for domain in id_domains() {
            assert!(DomainTypedLiteral::new(domain.domain_slug, [0x62; 16]).is_ok());
        }
        assert!(
            observed_domains.is_subset(
                &id_domains()
                    .iter()
                    .map(|domain| domain.domain_slug)
                    .collect()
            )
        );
    }

    #[test]
    fn wp09_structural_acceptance() {
        let tables = table_specs();
        assert!(tables.len() >= 17);
        for code in [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 100, 110, 120, 130, 900, 901,
        ] {
            assert!(table_spec(code).is_some(), "base table code {code} drifted");
        }
        let entity = table_spec(100).unwrap();
        assert_eq!(
            entity.partition_columns,
            ["entity_family_code", "owner_bucket"]
        );
        assert_eq!(
            entity
                .arrow_schema
                .field_with_name("entity_id")
                .unwrap()
                .data_type(),
            &DataType::FixedSizeBinary(16)
        );
        assert_eq!(entity.arrow_schema.metadata().len(), 12);
        assert_eq!(
            entity
                .arrow_schema
                .metadata()
                .get("com.codefabric.cpg.durable_mutation_class")
                .map(String::as_str),
            Some("OWNER_REPLACED_FACT")
        );
        assert_eq!(
            entity
                .arrow_schema
                .metadata()
                .get("com.codefabric.cpg.publication_pin_role")
                .map(String::as_str),
            Some("PINNED_DATA")
        );
        assert!(tables.iter().all(|table| {
            !table
                .arrow_schema
                .fields()
                .iter()
                .any(|field| matches!(field.data_type(), DataType::Utf8View))
        }));
        assert!(
            tables
                .iter()
                .filter(|table| table.materialization_role
                    == MaterializationRole::OperationalProjection)
                .all(|table| table.name.ends_with("tombstone"))
        );
        assert!(
            tables
                .iter()
                .filter(|table| table.materialization_role == MaterializationRole::DurableEffective)
                .all(|table| table.name != "owner_tombstone"
                    && table.name != "primary_key_tombstone")
        );
    }

    #[test]
    fn wp58_structural_acceptance() {
        use std::io::{Cursor, Seek as _};

        use arrow::ipc::reader::StreamReader;
        use arrow::ipc::writer::StreamWriter;
        use arrow_cast::cast;
        use parquet::arrow::ArrowWriter;
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let entity = table_spec(100).unwrap();
        let index = entity.arrow_schema.index_of("entity_id").unwrap();
        let field = entity.arrow_schema.field(index);
        assert_eq!(
            field.try_extension_type::<EntityIdExtension>().unwrap(),
            EntityIdExtension::v1()
        );
        assert_eq!(field.data_type(), &DataType::FixedSizeBinary(16));

        let projected = Arc::new(entity.arrow_schema.project(&[index]).unwrap());
        assert_eq!(
            projected
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1(),
            "Arrow schema projection must preserve the application extension contract"
        );
        let batch = one_id16_batch(Arc::clone(&projected));

        let mut ipc = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut ipc, &projected).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        let ipc_batch = StreamReader::try_new(Cursor::new(ipc), None)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            ipc_batch
                .schema()
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1(),
            "IPC must preserve the extension metadata"
        );

        let mut parquet_file = tempfile::tempfile().unwrap();
        {
            let mut writer =
                ArrowWriter::try_new(parquet_file.try_clone().unwrap(), projected, None).unwrap();
            writer.write(&batch).unwrap();
            writer.close().unwrap();
        }
        parquet_file.rewind().unwrap();
        let parquet_batch = ParquetRecordBatchReaderBuilder::try_new(parquet_file)
            .unwrap()
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            parquet_batch
                .schema()
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1(),
            "Parquet Arrow schema metadata must preserve the extension contract"
        );

        let binary = cast(batch.column(0), &DataType::Binary).unwrap();
        let fixed = cast(&binary, &DataType::FixedSizeBinary(16)).unwrap();
        let reattached =
            arrow::record_batch::RecordBatch::try_new(batch.schema(), vec![fixed]).unwrap();
        assert_eq!(
            reattached
                .schema()
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1(),
            "application schema reattachment restores metadata after storage casts"
        );

        let mut fallback_metadata = field.metadata().clone();
        fallback_metadata.remove(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY);
        fallback_metadata.remove(arrow_schema::extension::EXTENSION_TYPE_METADATA_KEY);
        let fallback = field.clone().with_metadata(fallback_metadata);
        assert_eq!(fallback.data_type(), &DataType::FixedSizeBinary(16));
        assert!(fallback.try_extension_type::<EntityIdExtension>().is_err());

        let mut unsupported_metadata = field.metadata().clone();
        unsupported_metadata.insert(
            arrow_schema::extension::EXTENSION_TYPE_METADATA_KEY.to_owned(),
            "version=2".to_owned(),
        );
        let unsupported = field.clone().with_metadata(unsupported_metadata);
        assert!(
            unsupported
                .try_extension_type::<EntityIdExtension>()
                .is_err()
        );
    }

    #[test]
    fn wp09_behavioral_acceptance() {
        let property = table_spec(120).unwrap();
        let value_columns = [
            "value_entity_id",
            "value_bool",
            "value_int64",
            "value_float64",
            "value_text",
            "value_bytes",
            "value_type_id",
        ];
        assert!(
            value_columns
                .iter()
                .all(|name| property.arrow_schema.field_with_name(name).is_ok())
        );
        let workspace = table_spec(1).unwrap();
        assert!(
            workspace
                .arrow_schema
                .field_with_name("registration_revision")
                .is_ok()
        );
        assert!(workspace.arrow_schema.field_with_name("updated_at").is_ok());
        for table in table_specs() {
            let batch = arrow::record_batch::RecordBatch::new_empty(table.arrow_schema.clone());
            assert_eq!(batch.num_columns(), table.arrow_schema.fields().len());
            assert_eq!(batch.num_rows(), 0);
            let datafusion = table.datafusion_schema().unwrap();
            assert_eq!(datafusion.fields().len(), table.arrow_schema.fields().len());
            table.validate_delta_compatibility().unwrap();
        }
    }

    #[test]
    fn wp28_behavioral_acceptance() {
        for code in [140, 150, 160, 170] {
            let table = table_spec(code).unwrap();
            let batch = synthetic_wave4_batch(table);
            assert_eq!(batch.num_rows(), 1);
            assert_eq!(batch.schema().fields(), table.arrow_schema.fields());
            table.validate_delta_compatibility().unwrap();
            assert_eq!(
                table.datafusion_schema().unwrap().fields().len(),
                table.arrow_schema.fields().len()
            );
            datafusion::datasource::MemTable::try_new(
                table.arrow_schema.clone(),
                vec![vec![batch]],
            )
            .unwrap();
        }
        assert_eq!(
            table_spec(140)
                .unwrap()
                .arrow_schema
                .field_with_name("line_start_offsets")
                .unwrap()
                .data_type(),
            &DataType::List(Arc::new(Field::new("element", DataType::Int64, false)))
        );
    }

    #[test]
    fn wp28_structural_acceptance() {
        for (code, name, primary_key) in [
            (140, "source_file", &["workspace_id", "file_id"][..]),
            (150, "source_token", &["token_id"][..]),
            (160, "source_annotation", &["annotation_id"][..]),
            (170, "syntax_detail", &["entity_id"][..]),
        ] {
            let table = table_spec(code).unwrap();
            assert_eq!(table.name, name);
            assert_eq!(table.primary_key, primary_key);
            assert_eq!(table.partition_columns, ["owner_bucket"]);
            assert_eq!(
                table.durable_mutation,
                DurableMutationClass::OwnerReplacedFact
            );
            assert_eq!(table.overlay_mutation, OverlayMutationPolicy::OwnerReplace);
            assert_eq!(
                table.materialization_role,
                MaterializationRole::DurableEffective
            );
            assert_eq!(table.publication_pin_role, PublicationPinRole::PinnedData);
        }
    }

    #[test]
    fn wp28_negative_zero_state() {
        let views = serving_projection_specs();
        assert!(
            views
                .iter()
                .any(|view| view.view_name == "files" && view.source_table_code == 140)
        );
        assert!(
            views
                .iter()
                .any(|view| view.view_name == "syntax" && view.source_table_code == 170)
        );
        assert!(!views.iter().any(|view| matches!(
            view.view_name,
            "tokens" | "annotations" | "python" | "rust" | "derived"
        )));
    }

    #[cfg(feature = "repository-state")]
    #[test]
    fn wp09_operational_store_ddl_executes() {
        let ddl = include_str!("../contracts/generated/model/schema/operational-store.sql");
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch(ddl).unwrap();
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let generated_table_count = ddl
            .lines()
            .filter(|line| line.starts_with("CREATE TABLE "))
            .count();
        assert_eq!(table_count, i64::try_from(generated_table_count).unwrap());
        let view_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'view' AND name = 'workspace_update_state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(view_count, 1);
    }
}
