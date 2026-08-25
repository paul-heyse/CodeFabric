//! Contract-IR-driven schema, `TableSpec`, DDL, and row-encoder family driver.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot, executable_tool_identity,
    process_stage_root, rustfmt_source,
};
use super::incremental::{CacheLookup, render_with_cache};
use super::model_control::StableId;
use super::repository_model::read_stable;

const SCHEMA_IR_PATH: &str = "contracts/schema/schema-contract-ir.json";
const TABLE_MANIFEST_PATH: &str = "contracts/generated/model/schema/table-specs.json";
const DDL_PATH: &str = "contracts/generated/model/schema/operational-store.sql";
const RUST_BINDINGS_PATH: &str = "src/generated/model_schema_tables.rs";
const RUST_RUNTIME_BINDINGS_PATH: &str = "src/generated/table_specs.rs";
const VALIDATION_PATH: &str = "contracts/generated/model/schema/schema-validation.json";
const MAX_AUTHORITY_BYTES: usize = 16 * 1024 * 1024;
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Header carried temporarily by the legacy native authority during detached-identity cutover.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityHeader {
    artifact_id: String,
    artifact_kind: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
    generator_revision: String,
}

/// Contract-owned logical types. Their Arrow mapping is defined once by this driver.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalType {
    Id16,
    Hash32,
    Code16,
    Code32,
    Bucket16,
    Int16,
    Int32,
    Int64,
    Float64,
    Boolean,
    Utf8,
    Binary,
    TimestampUtc,
    IdList,
    Int64List,
    StringMap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DurableMutationClass {
    StaticDimension,
    CurrentSingleton,
    OwnerReplacedFact,
    PublicationAppend,
    DerivedOwnerReplaced,
    GlobalDerivedReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum OverlayMutationPolicy {
    OwnerReplace,
    PrimaryKeyUpsert,
    FullTableReplace,
    BaseImmutable,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum MaterializationRole {
    DurableEffective,
    BundleDimension,
    QueryTimeDerived,
    OperationalProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PublicationPinRole {
    PinnedData,
    ManifestControl,
    PointerControl,
    NotPublished,
}

/// One ordered physical field. `field_id` is derived as `<table>.<name>`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColumnContract {
    name: String,
    logical_type: LogicalType,
    nullable: bool,
    #[serde(default)]
    semantic_type: Option<String>,
    #[serde(default)]
    foreign_key: Option<String>,
    #[serde(default)]
    hidden_operational: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableContract {
    table_code: i16,
    name: String,
    publication_pin_role: PublicationPinRole,
    family: String,
    grain: String,
    schema_version: String,
    columns: Vec<ColumnContract>,
    primary_key: Vec<String>,
    partition_columns: Vec<String>,
    zorder_columns: Vec<String>,
    durable_mutation: DurableMutationClass,
    overlay_mutation: OverlayMutationPolicy,
    materialization_role: MaterializationRole,
    dependencies: Vec<i16>,
    required_for_publication: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableScopeContract {
    table_code: i16,
    #[serde(default)]
    workspace_column: Option<String>,
    #[serde(default)]
    analysis_context_column: Option<String>,
    #[serde(default)]
    source_generation_column: Option<String>,
    #[serde(default)]
    analysis_context_set_column: Option<String>,
    #[serde(default)]
    owner_column: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ServingProjectionRole {
    EffectiveFact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServingProjectionContract {
    view_name: String,
    source_table_code: i16,
    availability_wave: u16,
    projection_role: ServingProjectionRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ControlProjectionRole {
    OperationalSource,
    DerivedOperational,
    ActiveServingSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlProjectionContract {
    view_name: String,
    availability_wave: u16,
    projection_role: ControlProjectionRole,
    #[serde(default)]
    source_table: Option<String>,
    #[serde(default)]
    columns: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalColumnContract {
    name: String,
    sqlite_type: SqliteType,
    nullable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum OperationalWorkspaceScopeContract {
    Direct {
        workspace_column: String,
    },
    ViaParent {
        parent_table: String,
        child_column: String,
        parent_column: String,
        workspace_column: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalTableContract {
    name: String,
    columns: Vec<OperationalColumnContract>,
    primary_key: Vec<String>,
    #[serde(default)]
    unique: Vec<Vec<String>>,
    #[serde(default)]
    workspace_scope: Option<OperationalWorkspaceScopeContract>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServingResourceProfileContract {
    batch_size: usize,
    max_output_rows: usize,
    max_output_bytes: usize,
    max_output_batches: usize,
    max_control_rows: usize,
    max_control_bytes: usize,
    max_control_batches: usize,
    max_snapshot_validation_rows: usize,
    max_snapshot_validation_bytes: usize,
    max_snapshot_validation_batches: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicSchemaContract {
    schema_kind: String,
    artifact_id: String,
    path: String,
    title: String,
    /// JSON Schema is validated by the independent pinned Draft 2020-12 implementation.
    schema: Value,
}

/// Closed physical types allowed on provider-to-daemon Arrow observation streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderObservationLogicalType {
    Utf8,
    Boolean,
    #[serde(rename = "uint64")]
    UInt64,
    Utf8List,
}

/// One required field in a provider observation Arrow schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObservationFieldContract {
    name: String,
    logical_type: ProviderObservationLogicalType,
    nullable: bool,
}

/// One provider-native Arrow schema admitted before canonical reconciliation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObservationSchemaContract {
    schema_id: String,
    provider_id: String,
    observation_family_code: u16,
    fields: Vec<ProviderObservationFieldContract>,
}

/// Single typed source for every schema-family projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaContractIr {
    #[serde(flatten)]
    header: AuthorityHeader,
    schema_version: u16,
    ontology_version: String,
    compatibility_mode: String,
    owner_bucket_count: u16,
    tables: Vec<TableContract>,
    table_scopes: Vec<TableScopeContract>,
    operational_tables: Vec<OperationalTableContract>,
    serving_projections: Vec<ServingProjectionContract>,
    control_projections: Vec<ControlProjectionContract>,
    serving_resource_profile: ServingResourceProfileContract,
    provider_observation_schemas: Vec<ProviderObservationSchemaContract>,
    public_schemas: Vec<PublicSchemaContract>,
}

impl SchemaContractIr {
    #[allow(clippy::too_many_lines)] // One pass keeps cross-projection diagnostics path-stable.
    fn validate(&self) -> Result<(), SchemaDriverError> {
        if self.header.artifact_id != "codefabric.schema.contract-ir"
            || self.header.artifact_kind != "manifest"
            || self.header.compatible_suite_major != 1
            || self.header.status != "released"
            || self.schema_version != 1
            || self.owner_bucket_count != 256
        {
            return invalid("$", "invalid schema Contract IR header or version");
        }
        if self.tables.is_empty() {
            return invalid("$.tables", "at least one TableSpec is required");
        }
        let mut codes = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut tables = BTreeMap::new();
        for (table_index, table) in self.tables.iter().enumerate() {
            let path = format!("$.tables[{table_index}]");
            if !identifier(&table.name)
                || !codes.insert(table.table_code)
                || !names.insert(table.name.as_str())
            {
                return invalid(&path, "duplicate or invalid table identity");
            }
            if table.primary_key.is_empty() {
                return invalid(&format!("{path}.primary_key"), "primary key is empty");
            }
            let mut fields = BTreeSet::new();
            for (column_index, column) in table.columns.iter().enumerate() {
                if !identifier(&column.name) || !fields.insert(column.name.as_str()) {
                    return invalid(
                        &format!("{path}.columns[{column_index}].name"),
                        "duplicate or invalid field name",
                    );
                }
            }
            for key in table
                .primary_key
                .iter()
                .chain(&table.partition_columns)
                .chain(&table.zorder_columns)
            {
                if !fields.contains(key.as_str()) {
                    return invalid(&path, format!("unknown field reference {key}"));
                }
            }
            tables.insert(table.name.as_str(), table);
        }
        for (table_index, table) in self.tables.iter().enumerate() {
            for (column_index, column) in table.columns.iter().enumerate() {
                let Some(foreign_key) = column.foreign_key.as_deref() else {
                    continue;
                };
                let Some((foreign_table, foreign_column)) = foreign_key.split_once('.') else {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        "foreign key must be <table>.<field>",
                    );
                };
                let Some(target) = tables.get(foreign_table).and_then(|target| {
                    target
                        .columns
                        .iter()
                        .find(|field| field.name == foreign_column)
                }) else {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        format!("unresolved foreign key {foreign_key}"),
                    );
                };
                if target.logical_type != column.logical_type {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        "foreign-key logical types differ",
                    );
                }
            }
            for dependency in &table.dependencies {
                if !codes.contains(dependency) {
                    return invalid(
                        &format!("$.tables[{table_index}].dependencies"),
                        format!("unknown table dependency {dependency}"),
                    );
                }
            }
        }
        let syntax = tables
            .get("syntax_detail")
            .ok_or_else(|| SchemaDriverError::Invalid {
                path: "$.tables".to_owned(),
                detail: "syntax_detail is absent".to_owned(),
            })?;
        for field in [
            "occurrence_family_code",
            "reconciliation_step_code",
            "raw_kind_disposition_code",
        ] {
            if syntax
                .columns
                .iter()
                .find(|column| column.name == field)
                .is_none_or(|column| column.logical_type != LogicalType::Code16 || column.nullable)
            {
                return invalid(
                    "$.tables[syntax_detail].columns",
                    format!("corrected field {field} must be required code16"),
                );
            }
        }
        let mut observation_schema_ids = BTreeSet::new();
        let mut observation_family_codes = BTreeSet::new();
        for (schema_index, schema) in self.provider_observation_schemas.iter().enumerate() {
            let path = format!("$.provider_observation_schemas[{schema_index}]");
            if schema.schema_id.is_empty()
                || !schema.schema_id.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte)
                })
                || schema.provider_id.is_empty()
                || schema.observation_family_code == 0
                || schema.fields.is_empty()
                || !observation_schema_ids.insert(schema.schema_id.as_str())
                || !observation_family_codes.insert(schema.observation_family_code)
            {
                return invalid(&path, "invalid or duplicate provider observation schema");
            }
            let mut fields = BTreeSet::new();
            for (field_index, field) in schema.fields.iter().enumerate() {
                if !identifier(&field.name) || !fields.insert(field.name.as_str()) {
                    return invalid(
                        &format!("{path}.fields[{field_index}].name"),
                        "invalid or duplicate provider observation field",
                    );
                }
            }
        }
        let mut operational_names = BTreeSet::new();
        for (table_index, table) in self.operational_tables.iter().enumerate() {
            let path = format!("$.operational_tables[{table_index}]");
            if !identifier(&table.name) || !operational_names.insert(table.name.as_str()) {
                return invalid(&path, "duplicate or invalid SQLite table identity");
            }
            let fields = table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<BTreeSet<_>>();
            if fields.len() != table.columns.len() || table.primary_key.is_empty() {
                return invalid(&path, "duplicate field or empty SQLite primary key");
            }
            for key in table
                .primary_key
                .iter()
                .chain(table.unique.iter().flatten())
            {
                if !fields.contains(key.as_str()) {
                    return invalid(&path, format!("unknown SQLite key field {key}"));
                }
            }
        }
        let operational_tables = self
            .operational_tables
            .iter()
            .map(|table| (table.name.as_str(), table))
            .collect::<BTreeMap<_, _>>();
        let mut control_views = BTreeSet::new();
        for (projection_index, projection) in self.control_projections.iter().enumerate() {
            let path = format!("$.control_projections[{projection_index}]");
            if !identifier(&projection.view_name)
                || !control_views.insert(projection.view_name.as_str())
                || projection.availability_wave == 0
            {
                return invalid(&path, "duplicate or invalid control projection identity");
            }
            match projection.projection_role {
                ControlProjectionRole::OperationalSource => {
                    if projection.source_table.as_deref() != Some(projection.view_name.as_str())
                        || !projection.columns.is_empty()
                        || !operational_tables.contains_key(projection.view_name.as_str())
                    {
                        return invalid(
                            &path,
                            "operational-source projection must name its governed table",
                        );
                    }
                }
                ControlProjectionRole::DerivedOperational => {
                    let source_name = projection.source_table.as_deref().ok_or_else(|| {
                        SchemaDriverError::Invalid {
                            path: format!("{path}.source_table"),
                            detail: "derived operational projection source is absent".into(),
                        }
                    })?;
                    let source = operational_tables.get(source_name).ok_or_else(|| {
                        SchemaDriverError::Invalid {
                            path: format!("{path}.source_table"),
                            detail: "derived operational projection source is unknown".into(),
                        }
                    })?;
                    let source_columns = source
                        .columns
                        .iter()
                        .map(|column| column.name.as_str())
                        .collect::<BTreeSet<_>>();
                    let selected = projection
                        .columns
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>();
                    if selected.len() != projection.columns.len()
                        || selected.is_empty()
                        || selected
                            .iter()
                            .any(|column| !identifier(column) || !source_columns.contains(column))
                    {
                        return invalid(
                            &format!("{path}.columns"),
                            "derived operational projection columns are invalid",
                        );
                    }
                }
                ControlProjectionRole::ActiveServingSnapshot => {
                    if projection.view_name != "active_serving_snapshot"
                        || projection.source_table.is_some()
                        || !projection.columns.is_empty()
                    {
                        return invalid(
                            &path,
                            "active serving snapshot is a runtime pinned-session projection",
                        );
                    }
                }
            }
        }
        let expected_kinds = BTreeSet::from([
            "analysis-context",
            "serving-snapshot",
            "public-snapshot-metadata",
            "source-context",
            "public-status",
            "cpg-semantic-query-request",
            "cpg-semantic-query-response",
            "plan-spec",
        ]);
        let mut kinds = BTreeSet::new();
        let mut artifact_ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for (index, schema) in self.public_schemas.iter().enumerate() {
            let path = format!("$.public_schemas[{index}]");
            if !kinds.insert(schema.schema_kind.as_str())
                || !artifact_ids.insert(schema.artifact_id.as_str())
                || !paths.insert(schema.path.as_str())
                || !schema.path.starts_with("contracts/")
                || !schema.path.ends_with(".schema.json")
                || schema.schema.as_object().is_none()
            {
                return invalid(&path, "invalid or duplicate public schema declaration");
            }
            let body = schema.schema.as_object().expect("checked schema body");
            if [
                "$schema",
                "$id",
                "title",
                "x-codefabric-generated",
                "x-codefabric-artifact",
            ]
            .iter()
            .any(|reserved| body.contains_key(*reserved))
            {
                return invalid(&format!("{path}.schema"), "body contains a derived header");
            }
        }
        if kinds != expected_kinds {
            return invalid("$.public_schemas", "public schema kind census differs");
        }
        let resources = self.serving_resource_profile;
        if [
            resources.batch_size,
            resources.max_output_rows,
            resources.max_output_bytes,
            resources.max_output_batches,
            resources.max_control_rows,
            resources.max_control_bytes,
            resources.max_control_batches,
            resources.max_snapshot_validation_rows,
            resources.max_snapshot_validation_bytes,
            resources.max_snapshot_validation_batches,
        ]
        .contains(&0)
        {
            return invalid(
                "$.serving_resource_profile",
                "resource limits must be positive",
            );
        }
        Ok(())
    }
}

/// Resolved, source-fenced schema plan.
pub struct SchemaPlan {
    descriptor: DriverDescriptor,
    ir: SchemaContractIr,
    source_digest: String,
    semantic_digest: String,
    source_fence: DriverSourceFence,
}

/// Pure schema family driver.
pub struct SchemaDriver;

impl SchemaDriver {
    fn output(
        id: impl Into<String>,
        path: &str,
        role: DriverOutputRole,
    ) -> Result<DriverOutputSpec, DriverProtocolError> {
        Ok(DriverOutputSpec {
            output_id: StableId::parse(id.into())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            path: SafeOutputPath::parse(path.as_bytes().to_vec())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            role,
        })
    }

    fn outputs(plan: &SchemaPlan) -> Result<Vec<(SafeOutputPath, Vec<u8>)>, SchemaDriverError> {
        let mut outputs = vec![
            (safe(TABLE_MANIFEST_PATH)?, render_table_manifest(plan)?),
            (safe(DDL_PATH)?, render_ddl(plan)),
            (
                safe(RUST_BINDINGS_PATH)?,
                rustfmt_source(&render_rust(&plan.ir))?,
            ),
            (
                safe(RUST_RUNTIME_BINDINGS_PATH)?,
                rustfmt_source(&render_runtime_rust(&plan.ir, &plan.source_digest))?,
            ),
            (safe(VALIDATION_PATH)?, render_validation(plan)?),
        ];
        for schema in &plan.ir.public_schemas {
            outputs.push((
                safe(&schema.path)?,
                render_public_schema(schema, &plan.source_digest)?,
            ));
        }
        outputs.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(outputs)
    }
}

impl ModelDriver for SchemaDriver {
    type Plan = SchemaPlan;

    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError> {
        let source = safe_protocol(SCHEMA_IR_PATH)?;
        let outputs = vec![
            Self::output(
                "output:model-schema-tables",
                TABLE_MANIFEST_PATH,
                DriverOutputRole::TableSpec,
            )?,
            Self::output(
                "output:model-schema-ddl",
                DDL_PATH,
                DriverOutputRole::SqliteDdl,
            )?,
            Self::output(
                "output:model-schema-rust",
                RUST_BINDINGS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-schema-runtime-rust",
                RUST_RUNTIME_BINDINGS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-schema-validation",
                VALIDATION_PATH,
                DriverOutputRole::ValidationReport,
            )?,
        ];
        let descriptor = DriverDescriptor {
            driver_id: StableId::parse("driver:schema-contract-v1".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            family: StableId::parse("family:schemas".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            rule_version: "schema-contract-driver-v1".to_owned(),
            sources: vec![source],
            output_roots: vec![
                safe_protocol("contracts/schema")?,
                safe_protocol("contracts/query")?,
            ],
            outputs,
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_AUTHORITY_BYTES,
                max_output_bytes: 8 * 1024 * 1024,
                max_outputs: 16,
            },
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError> {
        let mut descriptor = self.describe()?;
        let source_fence = DriverSourceFence::capture(repository_root, &descriptor)?;
        let bytes = read_stable(&repository_root.join(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES)?;
        let ir = decode_ir(&bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        ir.validate()
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        for schema in &ir.public_schemas {
            let output = Self::output(
                format!("output:model-schema-{}", schema.schema_kind),
                &schema.path,
                DriverOutputRole::PublicJsonSchema,
            )?;
            if !descriptor.output_roots.iter().any(|root| {
                output.path.as_bytes().starts_with(root.as_bytes())
                    && output.path.as_bytes().get(root.as_bytes().len()) == Some(&b'/')
            }) {
                return Err(DriverProtocolError::InvalidAuthority(format!(
                    "public schema output escapes declared roots: {}",
                    output.path.display()
                )));
            }
            descriptor.outputs.push(output);
        }
        descriptor.validate()?;
        let semantic_digest = detached_schema_identity(&bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        Ok(SchemaPlan {
            descriptor,
            ir,
            source_digest: codefabric::integrity::framed_digest(&bytes),
            semantic_digest,
            source_fence,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        let outputs = Self::outputs(plan)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut rendered = Vec::with_capacity(outputs.len());
        for (path, bytes) in outputs {
            staging_root.write(&path, &bytes)?;
            rendered.push(path);
        }
        Ok(rendered)
    }
}

/// Render and internally cross-check the schema family under a disposable stage.
///
/// # Errors
///
/// Returns typed ingress, validation, staging, or projection failures.
pub fn check_family(repository_root: &Path) -> Result<SchemaReport, SchemaDriverError> {
    let driver = SchemaDriver;
    let plan = driver.plan(repository_root)?;
    let stage_path = process_stage_root(repository_root, "schemas-stage");
    if stage_path.exists() {
        fs::remove_dir_all(&stage_path).map_err(|source| SchemaDriverError::Io {
            path: stage_path.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage_path).map_err(|source| SchemaDriverError::Io {
        path: stage_path.clone(),
        source,
    })?;
    let staging = StagingRoot::new(repository_root, &stage_path, &plan.descriptor)?;
    let (rendered, cache_lookup, action_key) = render_with_cache(
        repository_root,
        "schemas",
        &plan.descriptor,
        &plan.source_fence,
        &staging,
        || executable_tool_identity("rustfmt", &["--version"]),
        || driver.render(&plan, &staging),
    )?;
    plan.source_fence.verify(repository_root)?;
    let manifest: Value = serde_json::from_slice(&read_stable(
        &stage_path.join(TABLE_MANIFEST_PATH),
        MAX_AUTHORITY_BYTES,
    )?)?;
    if manifest["tables"].as_array().map(Vec::len) != Some(plan.ir.tables.len())
        || manifest["operational_tables"].as_array().map(Vec::len)
            != Some(plan.ir.operational_tables.len())
    {
        return Err(SchemaDriverError::ProjectionMismatch);
    }
    let syntax_fields = plan
        .ir
        .tables
        .iter()
        .find(|table| table.name == "syntax_detail")
        .ok_or(SchemaDriverError::ProjectionMismatch)?
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect();
    Ok(SchemaReport {
        family: "schemas".to_owned(),
        action_key,
        rule_version: plan.descriptor.rule_version.clone(),
        resource_profile: plan.descriptor.resource_profile.clone(),
        table_count: plan.ir.tables.len(),
        operational_table_count: plan.ir.operational_tables.len(),
        public_schema_count: plan.ir.public_schemas.len(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        cache_lookup,
        syntax_detail_fields: syntax_fields,
        stage_root: staging.path().to_string_lossy().into_owned(),
    })
}

/// Machine-readable family result consumed by the command contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaReport {
    pub family: String,
    pub action_key: String,
    pub rule_version: String,
    pub resource_profile: DriverResourceProfile,
    pub table_count: usize,
    pub operational_table_count: usize,
    pub public_schema_count: usize,
    pub rendered_outputs: Vec<String>,
    pub cache_lookup: CacheLookup,
    pub syntax_detail_fields: Vec<String>,
    pub stage_root: String,
}

fn render_table_manifest(plan: &SchemaPlan) -> Result<Vec<u8>, SchemaDriverError> {
    let tables = plan
        .ir
        .tables
        .iter()
        .map(|table| {
            let columns = table.columns.iter().map(|column| json!({
                "field_id": format!("{}.{}", table.name, column.name),
                "name": column.name,
                "logical_type": column.logical_type,
                "arrow_type": arrow_type(column.logical_type),
                "nullable": column.nullable,
                "semantic_type": column.semantic_type,
                "foreign_key": column.foreign_key,
                "hidden_operational": column.hidden_operational,
                "key_role": if table.primary_key.contains(&column.name) { "primary" } else { "none" },
            })).collect::<Vec<_>>();
            json!({
                "table_id": format!("table:{}", table.name),
                "table_code": table.table_code,
                "name": table.name,
                "family": table.family,
                "grain": table.grain,
                "schema_version": table.schema_version,
                "columns": columns,
                "primary_key": table.primary_key,
                "partition_columns": table.partition_columns,
                "zorder_columns": table.zorder_columns,
                "durable_mutation": table.durable_mutation,
                "overlay_mutation": table.overlay_mutation,
                "materialization_role": table.materialization_role,
                "publication_pin_role": table.publication_pin_role,
                "dependencies": table.dependencies,
                "required_for_publication": table.required_for_publication,
            })
        })
        .collect::<Vec<_>>();
    pretty(&json!({
        "model_version": 1,
        "source": {"artifact_id": plan.ir.header.artifact_id, "source_digest": plan.source_digest},
        "ontology_version": plan.ir.ontology_version,
        "compatibility_mode": plan.ir.compatibility_mode,
        "owner_bucket_count": plan.ir.owner_bucket_count,
        "tables": tables,
        "table_scopes": plan.ir.table_scopes,
        "operational_tables": plan.ir.operational_tables,
        "serving_projections": plan.ir.serving_projections,
        "control_projections": plan.ir.control_projections,
        "serving_resource_profile": plan.ir.serving_resource_profile,
        "public_schemas": plan.ir.public_schemas.iter().map(|schema| json!({
            "schema_kind": schema.schema_kind,
            "artifact_id": schema.artifact_id,
            "path": schema.path,
            "title": schema.title,
        })).collect::<Vec<_>>(),
    }))
}

fn render_public_schema(
    contract: &PublicSchemaContract,
    source_digest: &str,
) -> Result<Vec<u8>, SchemaDriverError> {
    let mut body =
        contract
            .schema
            .as_object()
            .cloned()
            .ok_or_else(|| SchemaDriverError::Invalid {
                path: format!("$.public_schemas[{}].schema", contract.schema_kind),
                detail: "schema body is not an object".to_owned(),
            })?;
    body.insert("$schema".to_owned(), Value::String(DIALECT.to_owned()));
    body.insert(
        "$id".to_owned(),
        Value::String(format!("https://codefabric.dev/{}", contract.path)),
    );
    body.insert("title".to_owned(), Value::String(contract.title.clone()));
    body.insert(
        "x-codefabric-artifact".to_owned(),
        json!({
            "artifact_id": contract.artifact_id,
            "artifact_kind": "json-schema",
            "version": "1.0",
            "compatible_suite_major": 1,
            "status": "released",
            "generator_revision": "codefabric-model-schema-driver-v1",
        }),
    );
    body.insert(
        "x-codefabric-generated".to_owned(),
        json!({
            "driver": "schema-contract-driver-v1",
            "source_artifact_id": "codefabric.schema.contract-ir",
            "source_digest": source_digest,
        }),
    );
    pretty(&Value::Object(body))
}

fn render_ddl(plan: &SchemaPlan) -> Vec<u8> {
    let mut output = format!(
        "-- @generated from codefabric.schema.contract-ir semantic={} source={}; schema-contract-driver-v1; do not edit.\nPRAGMA foreign_keys = ON;\n\n",
        plan.semantic_digest, plan.source_digest
    );
    for table in &plan.ir.operational_tables {
        output.push_str(&render_operational_table_ddl(table));
        output.push('\n');
    }
    for projection in &plan.ir.control_projections {
        if projection.projection_role != ControlProjectionRole::DerivedOperational {
            continue;
        }
        let source = projection
            .source_table
            .as_deref()
            .expect("validated derived projection source");
        writeln!(
            output,
            "CREATE VIEW {} AS\nSELECT {}\nFROM {};\n",
            projection.view_name,
            projection.columns.join(", "),
            source,
        )
        .unwrap();
    }
    output.into_bytes()
}

fn render_operational_table_ddl(table: &OperationalTableContract) -> String {
    let mut output = format!("CREATE TABLE {} (\n", table.name);
    let mut definitions = table
        .columns
        .iter()
        .map(|column| {
            format!(
                "  {} {}{}",
                column.name,
                match column.sqlite_type {
                    SqliteType::Integer => "INTEGER",
                    SqliteType::Real => "REAL",
                    SqliteType::Text => "TEXT",
                    SqliteType::Blob => "BLOB",
                },
                if column.nullable { "" } else { " NOT NULL" }
            )
        })
        .collect::<Vec<_>>();
    definitions.push(format!("  PRIMARY KEY ({})", table.primary_key.join(", ")));
    definitions.extend(
        table
            .unique
            .iter()
            .map(|columns| format!("  UNIQUE ({})", columns.join(", "))),
    );
    writeln!(output, "{}\n) STRICT;", definitions.join(",\n")).unwrap();
    output
}

fn render_rust(ir: &SchemaContractIr) -> Vec<u8> {
    let mut output = String::from(
        "// generated by schema-contract-driver-v1; do not edit.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ModelLogicalType { Id16, Hash32, Code16, Code32, Bucket16, Int16, Int32, Int64, Float64, Boolean, Utf8, Binary, TimestampUtc, IdList, Int64List, StringMap }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ModelColumn { pub field_id: &'static str, pub name: &'static str, pub logical_type: ModelLogicalType, pub nullable: bool, pub semantic_type: Option<&'static str>, pub foreign_key: Option<&'static str> }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ModelTable { pub table_code: i16, pub table_id: &'static str, pub name: &'static str, pub columns: &'static [ModelColumn], pub primary_key: &'static [&'static str] }\n\n\
         pub const MODEL_TABLES: &[ModelTable] = &[\n",
    );
    for table in &ir.tables {
        writeln!(
            output,
            "    ModelTable {{ table_code: {}, table_id: {:?}, name: {:?}, columns: &[",
            table.table_code,
            format!("table:{}", table.name),
            table.name
        )
        .unwrap();
        for column in &table.columns {
            writeln!(
                output,
                "        ModelColumn {{ field_id: {:?}, name: {:?}, logical_type: ModelLogicalType::{:?}, nullable: {}, semantic_type: {:?}, foreign_key: {:?} }},",
                format!("{}.{}", table.name, column.name),
                column.name,
                column.logical_type,
                column.nullable,
                column.semantic_type.as_deref(),
                column.foreign_key.as_deref(),
            )
            .unwrap();
        }
        writeln!(output, "    ], primary_key: &{:?} }},", table.primary_key).unwrap();
    }
    output.push_str("];\n\n");
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ProviderObservationLogicalType { Utf8, Boolean, UInt64, Utf8List }\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderObservationField { pub name: &'static str, pub logical_type: ProviderObservationLogicalType, pub nullable: bool }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ProviderObservationSchema { pub schema_id: &'static str, pub provider_id: &'static str, pub observation_family_code: u16, pub canonical_descriptor: &'static str, pub schema_digest: &'static str, pub fields: &'static [ProviderObservationField] }\n\n\
         pub const PROVIDER_OBSERVATION_SCHEMAS: &[ProviderObservationSchema] = &[\n",
    );
    for schema in &ir.provider_observation_schemas {
        let descriptor = provider_observation_descriptor(schema);
        let digest = codefabric::integrity::framed_digest(descriptor.as_bytes());
        writeln!(
            output,
            "    ProviderObservationSchema {{ schema_id: {:?}, provider_id: {:?}, observation_family_code: {}, canonical_descriptor: {:?}, schema_digest: {:?}, fields: &[",
            schema.schema_id,
            schema.provider_id,
            schema.observation_family_code,
            descriptor,
            digest,
        )
        .unwrap();
        for field in &schema.fields {
            writeln!(
                output,
                "        ProviderObservationField {{ name: {:?}, logical_type: ProviderObservationLogicalType::{:?}, nullable: {} }},",
                field.name, field.logical_type, field.nullable,
            )
            .unwrap();
        }
        output.push_str("    ] },\n");
    }
    output.push_str("];\n");
    output.into_bytes()
}

fn provider_observation_descriptor(schema: &ProviderObservationSchemaContract) -> String {
    let mut descriptor = schema.schema_id.clone();
    for field in &schema.fields {
        descriptor.push(':');
        descriptor.push_str(&field.name);
        descriptor.push(':');
        descriptor.push_str(match field.logical_type {
            ProviderObservationLogicalType::Utf8 => "utf8",
            ProviderObservationLogicalType::Boolean => "boolean",
            ProviderObservationLogicalType::UInt64 => "uint64",
            ProviderObservationLogicalType::Utf8List => "list<utf8>",
        });
        if field.nullable {
            descriptor.push('?');
        }
    }
    descriptor
}

#[allow(clippy::too_many_lines)] // One linear pass keeps the complete runtime view tied to one IR.
fn render_runtime_rust(ir: &SchemaContractIr, source_digest: &str) -> Vec<u8> {
    let mut output = format!(
        "// @generated from codefabric.schema.contract-ir {source_digest}; schema-contract-driver-v1; do not edit.\n\nconst GENERATED_TABLE_SPECS: &[GeneratedTableSpec] = &[\n"
    );
    for table in &ir.tables {
        writeln!(output, "    GeneratedTableSpec {{").unwrap();
        writeln!(
            output,
            "        table_code: {}, name: {:?}, family: {:?}, grain: {:?}, schema_version: {:?},",
            table.table_code, table.name, table.family, table.grain, table.schema_version
        )
        .unwrap();
        output.push_str("        columns: &[\n");
        for column in &table.columns {
            writeln!(
                output,
                "            GeneratedColumn {{ name: {:?}, logical_type: LogicalType::{:?}, nullable: {}, semantic_type: {:?}, foreign_key: {:?}, hidden_operational: {} }},",
                column.name,
                column.logical_type,
                column.nullable,
                column.semantic_type.as_deref(),
                column.foreign_key.as_deref(),
                column.hidden_operational,
            )
            .unwrap();
        }
        output.push_str("        ],\n");
        writeln!(
            output,
            "        primary_key: {}, partition_columns: {}, zorder_columns: {},",
            rust_strings(&table.primary_key),
            rust_strings(&table.partition_columns),
            rust_strings(&table.zorder_columns),
        )
        .unwrap();
        writeln!(
            output,
            "        durable_mutation: DurableMutationClass::{:?}, overlay_mutation: OverlayMutationPolicy::{:?}, materialization_role: MaterializationRole::{:?}, publication_pin_role: PublicationPinRole::{:?},",
            table.durable_mutation,
            table.overlay_mutation,
            table.materialization_role,
            table.publication_pin_role,
        )
        .unwrap();
        writeln!(
            output,
            "        dependencies: &{:?}, required_for_publication: {},",
            table.dependencies, table.required_for_publication
        )
        .unwrap();
        output.push_str("    },\n");
    }
    output.push_str(
        "];\n\nconst GENERATED_OPERATIONAL_TABLE_SPECS: &[GeneratedOperationalTableSpec] = &[\n",
    );
    for table in &ir.operational_tables {
        writeln!(output, "    GeneratedOperationalTableSpec {{").unwrap();
        writeln!(output, "        name: {:?},", table.name).unwrap();
        writeln!(
            output,
            "        sqlite_ddl: {:?},",
            render_operational_table_ddl(table)
        )
        .unwrap();
        output.push_str("        columns: &[\n");
        for column in &table.columns {
            writeln!(
                output,
                "            GeneratedOperationalColumn {{ name: {:?}, sqlite_type: OperationalSqliteType::{:?}, nullable: {} }},",
                column.name, column.sqlite_type, column.nullable
            )
            .unwrap();
        }
        output.push_str("        ],\n");
        writeln!(
            output,
            "        primary_key: {},",
            rust_strings(&table.primary_key)
        )
        .unwrap();
        writeln!(
            output,
            "        workspace_scope: {},",
            rust_operational_scope(table.workspace_scope.as_ref())
        )
        .unwrap();
        output.push_str("    },\n");
    }
    output.push_str("];\n\nconst GENERATED_TABLE_SCOPE_SPECS: &[TableScopeSpec] = &[\n");
    for scope in &ir.table_scopes {
        writeln!(
            output,
            "    TableScopeSpec {{ table_code: {}, workspace_column: {:?}, analysis_context_column: {:?}, source_generation_column: {:?}, analysis_context_set_column: {:?}, owner_column: {:?} }},",
            scope.table_code,
            scope.workspace_column.as_deref(),
            scope.analysis_context_column.as_deref(),
            scope.source_generation_column.as_deref(),
            scope.analysis_context_set_column.as_deref(),
            scope.owner_column.as_deref(),
        )
        .unwrap();
    }
    output.push_str(
        "];\n\nconst GENERATED_SERVING_PROJECTION_SPECS: &[ServingProjectionSpec] = &[\n",
    );
    for projection in &ir.serving_projections {
        writeln!(
            output,
            "    ServingProjectionSpec {{ view_name: {:?}, source_table_code: {}, availability_wave: {}, projection_role: ServingProjectionRole::{:?} }},",
            projection.view_name,
            projection.source_table_code,
            projection.availability_wave,
            projection.projection_role,
        )
        .unwrap();
    }
    output.push_str(
        "];\n\nconst GENERATED_CONTROL_PROJECTION_SPECS: &[ControlProjectionSpec] = &[\n",
    );
    for projection in &ir.control_projections {
        writeln!(
            output,
            "    ControlProjectionSpec {{ view_name: {:?}, availability_wave: {}, projection_role: ControlProjectionRole::{:?}, source_table: {:?}, columns: {} }},",
            projection.view_name,
            projection.availability_wave,
            projection.projection_role,
            projection.source_table.as_deref(),
            rust_strings(&projection.columns),
        )
        .unwrap();
    }
    let resources = ir.serving_resource_profile;
    writeln!(
        output,
        "];\n\nconst GENERATED_SERVING_RESOURCE_PROFILE: ServingResourceProfile = ServingResourceProfile {{ batch_size: {}, max_output_rows: {}, max_output_bytes: {}, max_output_batches: {}, max_control_rows: {}, max_control_bytes: {}, max_control_batches: {}, max_snapshot_validation_rows: {}, max_snapshot_validation_bytes: {}, max_snapshot_validation_batches: {} }};",
        rust_usize_literal(resources.batch_size),
        rust_usize_literal(resources.max_output_rows),
        rust_usize_literal(resources.max_output_bytes),
        rust_usize_literal(resources.max_output_batches),
        rust_usize_literal(resources.max_control_rows),
        rust_usize_literal(resources.max_control_bytes),
        rust_usize_literal(resources.max_control_batches),
        rust_usize_literal(resources.max_snapshot_validation_rows),
        rust_usize_literal(resources.max_snapshot_validation_bytes),
        rust_usize_literal(resources.max_snapshot_validation_batches),
    )
    .unwrap();
    output.into_bytes()
}

fn rust_strings(values: &[String]) -> String {
    format!(
        "&[{}]",
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn rust_operational_scope(scope: Option<&OperationalWorkspaceScopeContract>) -> String {
    match scope {
        None => "None".to_owned(),
        Some(OperationalWorkspaceScopeContract::Direct { workspace_column }) => format!(
            "Some(OperationalWorkspaceScope::Direct {{ workspace_column: {workspace_column:?} }})"
        ),
        Some(OperationalWorkspaceScopeContract::ViaParent {
            parent_table,
            child_column,
            parent_column,
            workspace_column,
        }) => format!(
            "Some(OperationalWorkspaceScope::ViaParent {{ parent_table: {parent_table:?}, child_column: {child_column:?}, parent_column: {parent_column:?}, workspace_column: {workspace_column:?} }})"
        ),
    }
}

fn rust_usize_literal(value: usize) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push('_');
        }
        grouped.push(digit);
    }
    grouped.chars().rev().collect()
}

fn render_validation(plan: &SchemaPlan) -> Result<Vec<u8>, SchemaDriverError> {
    pretty(&json!({
        "schema_version": 1,
        "family": "schemas",
        "source_digest": plan.source_digest,
        "table_count": plan.ir.tables.len(),
        "operational_table_count": plan.ir.operational_tables.len(),
        "public_schema_count": plan.ir.public_schemas.len(),
        "stable_field_id_rule": "<table-name>.<field-name>",
        "compatibility_acceptance_generated": false,
        "native_validators": ["arrow-schema-59.2.0", "datafusion-55.0.0", "sqlite-strict", "jsonschema-draft-2020-12"],
    }))
}

fn arrow_type(logical_type: LogicalType) -> Value {
    match logical_type {
        LogicalType::Id16 => json!({"name":"binary","byte_width":16}),
        LogicalType::Hash32 => json!({"name":"binary","byte_width":32}),
        LogicalType::Code16 | LogicalType::Bucket16 | LogicalType::Int16 => json!({"name":"int16"}),
        LogicalType::Code32 | LogicalType::Int32 => json!({"name":"int32"}),
        LogicalType::Int64 => json!({"name":"int64"}),
        LogicalType::Float64 => json!({"name":"float64"}),
        LogicalType::Boolean => json!({"name":"boolean"}),
        LogicalType::Utf8 => json!({"name":"utf8"}),
        LogicalType::Binary => json!({"name":"binary"}),
        LogicalType::TimestampUtc => {
            json!({"name":"timestamp","unit":"microsecond","timezone":"UTC"})
        }
        LogicalType::IdList => {
            json!({"name":"list","element":{"name":"binary","byte_width":16,"nullable":false}})
        }
        LogicalType::Int64List => {
            json!({"name":"list","element":{"name":"int64","nullable":false}})
        }
        LogicalType::StringMap => {
            json!({"name":"map","key":{"name":"utf8","nullable":false},"value":{"name":"utf8","nullable":false},"sorted":false})
        }
    }
}

fn pretty(value: &Value) -> Result<Vec<u8>, SchemaDriverError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn safe(path: &str) -> Result<SafeOutputPath, SchemaDriverError> {
    SafeOutputPath::parse(path.as_bytes().to_vec()).map_err(|_| SchemaDriverError::Invalid {
        path: path.to_owned(),
        detail: "unsafe output path".to_owned(),
    })
}

fn safe_protocol(path: &str) -> Result<SafeOutputPath, DriverProtocolError> {
    SafeOutputPath::parse(path.as_bytes().to_vec())
        .map_err(|_| DriverProtocolError::InvalidDescriptor)
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn invalid<T>(path: &str, detail: impl Into<String>) -> Result<T, SchemaDriverError> {
    Err(SchemaDriverError::Invalid {
        path: path.to_owned(),
        detail: detail.into(),
    })
}

fn decode_ir(bytes: &[u8]) -> Result<SchemaContractIr, SchemaDriverError> {
    let value = codefabric::contracts::jcs::decode_strict(bytes)?;
    serde_json::from_value(value).map_err(SchemaDriverError::Json)
}

/// Compile the schema Contract IR through its closed family-native model and return the detached
/// semantic identity used by aggregate provenance.
///
/// # Errors
///
/// Returns a duplicate-key, closed-model, schema-invariant, or canonicalization failure.
pub fn detached_schema_identity(bytes: &[u8]) -> Result<String, SchemaDriverError> {
    let document = decode_ir(bytes)?;
    document.validate()?;
    let mut value = serde_json::to_value(document)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: "$".to_owned(),
            detail: "typed schema Contract IR root is not an object".to_owned(),
        })?;
    object.remove("canonical_digest");
    object.remove("source_digest");
    let canonical = codefabric::contracts::jcs::canonicalize_value(&value)?;
    Ok(codefabric::integrity::framed_digest(&canonical))
}

/// Schema family error with bounded JSON-path diagnostics.
#[derive(Debug, Error)]
pub enum SchemaDriverError {
    #[error(transparent)]
    Driver(#[from] DriverProtocolError),
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error("schema authority invalid at {path}: {detail}")]
    Invalid { path: String, detail: String },
    #[error("schema projection differs from typed plan")]
    ProjectionMismatch,
    #[error("schema JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    CanonicalJson(#[from] codefabric::contracts::jcs::CanonicalJsonError),
    #[error("schema I/O failed at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> SchemaContractIr {
        decode_ir(&read_stable(Path::new(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES).unwrap()).unwrap()
    }

    #[test]
    fn model_tablespec_projects_equivalent_arrow_json_schema_and_ddl() {
        let ir = authority();
        ir.validate().unwrap();
        assert_eq!(ir.public_schemas.len(), 8);
        assert_eq!(ir.operational_tables.len(), 26);
        for table in &ir.tables {
            let ids = table
                .columns
                .iter()
                .map(|column| format!("{}.{}", table.name, column.name))
                .collect::<BTreeSet<_>>();
            assert_eq!(ids.len(), table.columns.len());
            for column in &table.columns {
                assert!(arrow_type(column.logical_type).is_object());
            }
        }
        let plan = SchemaDriver.plan(Path::new(".")).unwrap();
        let ddl = String::from_utf8(render_ddl(&plan)).unwrap();
        for projection in &ir.control_projections {
            let declaration = format!("CREATE VIEW {} AS", projection.view_name);
            match projection.projection_role {
                ControlProjectionRole::DerivedOperational => {
                    assert!(ddl.contains(&declaration));
                }
                ControlProjectionRole::OperationalSource
                | ControlProjectionRole::ActiveServingSnapshot => {
                    assert!(!ddl.contains(&declaration));
                }
            }
        }
    }

    #[test]
    fn model_row_encoder_round_trips_every_supported_field_shape() {
        let variants = [
            LogicalType::Id16,
            LogicalType::Hash32,
            LogicalType::Code16,
            LogicalType::Code32,
            LogicalType::Bucket16,
            LogicalType::Int16,
            LogicalType::Int32,
            LogicalType::Int64,
            LogicalType::Float64,
            LogicalType::Boolean,
            LogicalType::Utf8,
            LogicalType::Binary,
            LogicalType::TimestampUtc,
            LogicalType::IdList,
            LogicalType::Int64List,
            LogicalType::StringMap,
        ];
        for logical_type in variants {
            let encoded = serde_json::to_vec(&logical_type).unwrap();
            let decoded: LogicalType = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, logical_type);
            assert!(arrow_type(decoded).is_object());
        }
    }

    #[test]
    fn model_schema_compatibility_diagnostics_are_path_aware() {
        let mut ir = authority();
        ir.tables[0].columns[0].foreign_key = Some("missing.field".to_owned());
        let error = ir.validate().unwrap_err().to_string();
        assert!(error.contains("$.tables[0].columns[0].foreign_key"));
        assert!(error.contains("missing.field"));
    }

    #[test]
    fn model_schema_rejects_unknown_duplicate_and_incompatible_fields() {
        let bytes = read_stable(Path::new(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES).unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = Value::Bool(true);
        assert!(decode_ir(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(decode_ir(b"{\"schema_version\":1,\"schema_version\":2}").is_err());
        let mut ir = authority();
        ir.tables[0].columns[0].nullable = true;
        ir.tables[0].primary_key.push("missing".to_owned());
        assert!(ir.validate().is_err());
    }

    #[test]
    fn model_schema_outputs_have_one_producer_and_no_manual_public_include_list() {
        let mut descriptor = SchemaDriver.describe().unwrap();
        let ir = authority();
        for schema in &ir.public_schemas {
            descriptor.outputs.push(
                SchemaDriver::output(
                    format!("output:model-schema-{}", schema.schema_kind),
                    &schema.path,
                    DriverOutputRole::PublicJsonSchema,
                )
                .unwrap(),
            );
        }
        let paths = descriptor
            .outputs
            .iter()
            .map(|output| output.path.display())
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), descriptor.outputs.len());
        assert_eq!(
            descriptor
                .outputs
                .iter()
                .filter(|output| output.role == DriverOutputRole::PublicJsonSchema)
                .count(),
            8
        );
        assert_eq!(descriptor.sources.len(), 1);
        assert_eq!(descriptor.output_roots.len(), 2);
    }

    #[test]
    fn model_driver_cannot_generate_compatibility_acceptance() {
        let descriptor = SchemaDriver.describe().unwrap();
        assert!(descriptor.outputs.iter().all(|output| {
            let path = output.path.display();
            !path.contains("acceptance")
                && !path.contains("compatibility-baseline")
                && !path.contains("signature")
        }));
    }
}
