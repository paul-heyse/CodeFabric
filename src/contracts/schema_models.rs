//! Closed Contract IR for generated storage and public schema surfaces.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::models::ArtifactHeader;

/// Logical type names fixed by Data Fabric §7.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    StringMap,
}

/// Durable write behavior for one table.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DurableMutationClass {
    StaticDimension,
    CurrentSingleton,
    OwnerReplacedFact,
    PublicationAppend,
    DerivedOwnerReplaced,
    GlobalDerivedReplacement,
}

/// Hot-overlay mutation behavior for one table.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverlayMutationPolicy {
    OwnerReplace,
    PrimaryKeyUpsert,
    FullTableReplace,
    BaseImmutable,
    NotApplicable,
}

/// Query-visible materialization role for one table.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MaterializationRole {
    DurableEffective,
    BundleDimension,
    QueryTimeDerived,
    OperationalProjection,
}

/// Participation of one table in the acyclic durable publication manifest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicationPinRole {
    PinnedData,
    ManifestControl,
    PointerControl,
    NotPublished,
}

/// One closed Arrow field declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnContract {
    pub name: String,
    pub logical_type: LogicalType,
    pub nullable: bool,
    #[serde(default)]
    pub semantic_type: Option<String>,
    #[serde(default)]
    pub foreign_key: Option<String>,
    #[serde(default)]
    pub hidden_operational: bool,
}

/// One `TableSpec` source record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableContract {
    pub table_code: i16,
    pub name: String,
    pub family: String,
    pub grain: String,
    pub schema_version: String,
    pub columns: Vec<ColumnContract>,
    pub primary_key: Vec<String>,
    pub partition_columns: Vec<String>,
    pub zorder_columns: Vec<String>,
    pub durable_mutation: DurableMutationClass,
    pub overlay_mutation: OverlayMutationPolicy,
    pub materialization_role: MaterializationRole,
    pub publication_pin_role: PublicationPinRole,
    pub dependencies: Vec<i16>,
    pub required_for_publication: bool,
}

/// Contract-owned row-scope selectors applied below every query-visible view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableScopeContract {
    pub table_code: i16,
    #[serde(default)]
    pub workspace_column: Option<String>,
    #[serde(default)]
    pub analysis_context_column: Option<String>,
    #[serde(default)]
    pub source_generation_column: Option<String>,
    #[serde(default)]
    pub analysis_context_set_column: Option<String>,
    #[serde(default)]
    pub owner_column: Option<String>,
}

/// Closed role of one generated serving projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServingProjectionRole {
    EffectiveFact,
}

/// One stable `cpg_serving` view derived from a generated fact table.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServingProjectionContract {
    pub view_name: String,
    pub source_table_code: i16,
    pub availability_wave: u16,
    pub projection_role: ServingProjectionRole,
}

/// Closed implementation class for one generated `cpg_control` projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlProjectionRole {
    OperationalSource,
    DerivedOperational,
    ActiveServingSnapshot,
}

/// One stable control projection and its generated source/column contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlProjectionContract {
    pub view_name: String,
    pub availability_wave: u16,
    pub projection_role: ControlProjectionRole,
    #[serde(default)]
    pub source_table: Option<String>,
    #[serde(default)]
    pub columns: Vec<String>,
}

/// Generated non-timing service and candidate-construction resource limits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServingResourceProfileContract {
    pub batch_size: usize,
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

/// `SQLite` affinity names accepted by the operational DDL generator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

/// One operational-store column declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalColumnContract {
    pub name: String,
    pub sqlite_type: SqliteType,
    pub nullable: bool,
}

/// Closed way an operational table is scoped to one workspace for serving projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationalWorkspaceScopeContract {
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

/// One generated `SQLite` table declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalTableContract {
    pub name: String,
    pub columns: Vec<OperationalColumnContract>,
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub unique: Vec<Vec<String>>,
    #[serde(default)]
    pub workspace_scope: Option<OperationalWorkspaceScopeContract>,
}

/// Closed public-schema families generated by the schema compiler.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PublicSchemaKind {
    AnalysisContext,
    ServingSnapshot,
    PublicSnapshotMetadata,
    SourceContext,
    PublicStatus,
    CpgSemanticQueryRequest,
    CpgSemanticQueryResponse,
    PlanSpec,
}

/// One public schema output and its exact governed identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSchemaContract {
    pub schema_kind: PublicSchemaKind,
    pub artifact_id: String,
    pub path: PathBuf,
    pub title: String,
}

/// Single typed authority for WP09 schema generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaContractIr {
    #[serde(flatten)]
    pub header: ArtifactHeader,
    pub schema_version: u16,
    pub ontology_version: String,
    pub compatibility_mode: String,
    pub owner_bucket_count: u16,
    pub tables: Vec<TableContract>,
    pub table_scopes: Vec<TableScopeContract>,
    pub operational_tables: Vec<OperationalTableContract>,
    pub serving_projections: Vec<ServingProjectionContract>,
    pub control_projections: Vec<ControlProjectionContract>,
    pub serving_resource_profile: ServingResourceProfileContract,
    pub public_schemas: Vec<PublicSchemaContract>,
}

impl SchemaContractIr {
    /// Validate all cross-record identities before any output allocation.
    #[allow(clippy::too_many_lines)] // One pass keeps cross-table IR validation atomic.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.owner_bucket_count != 256 {
            return Err("schema_version must be 1 and owner_bucket_count must be 256".into());
        }
        let mut table_codes = BTreeSet::new();
        let mut table_names = BTreeSet::new();
        for table in &self.tables {
            if !table_codes.insert(table.table_code) || !table_names.insert(&table.name) {
                return Err(format!("duplicate table identity: {}", table.name));
            }
            let columns = table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<BTreeSet<_>>();
            if columns.len() != table.columns.len() {
                return Err(format!("duplicate column in {}", table.name));
            }
            for key in table
                .primary_key
                .iter()
                .chain(&table.partition_columns)
                .chain(&table.zorder_columns)
            {
                if !columns.contains(key.as_str()) {
                    return Err(format!("{} references unknown column {key}", table.name));
                }
            }
            if table.columns.iter().any(|column| {
                matches!(column.logical_type, LogicalType::Utf8) && column.name.contains("json")
            }) {
                return Err(format!(
                    "{} contains a prohibited JSON/EAV text column",
                    table.name
                ));
            }
            match table.materialization_role {
                MaterializationRole::BundleDimension
                    if table.durable_mutation != DurableMutationClass::StaticDimension
                        || table.overlay_mutation != OverlayMutationPolicy::BaseImmutable =>
                {
                    return Err(format!(
                        "{} has an illegal bundle-dimension policy cross-product",
                        table.name
                    ));
                }
                MaterializationRole::QueryTimeDerived
                    if table.overlay_mutation != OverlayMutationPolicy::NotApplicable =>
                {
                    return Err(format!(
                        "{} gives a query-time-derived surface an overlay mutation",
                        table.name
                    ));
                }
                MaterializationRole::OperationalProjection
                    if table.required_for_publication || !table.name.ends_with("tombstone") =>
                {
                    return Err(format!(
                        "{} exposes an operational projection as an effective fact",
                        table.name
                    ));
                }
                MaterializationRole::DurableEffective if table.name.ends_with("tombstone") => {
                    return Err(format!(
                        "{} exposes an overlay tombstone as a durable fact",
                        table.name
                    ));
                }
                _ => {}
            }
            if matches!(table.publication_pin_role, PublicationPinRole::NotPublished)
                == table.required_for_publication
            {
                return Err(format!(
                    "{} has inconsistent publication requirement and pin role",
                    table.name
                ));
            }
            match table.publication_pin_role {
                PublicationPinRole::ManifestControl
                    if table.durable_mutation != DurableMutationClass::PublicationAppend =>
                {
                    return Err(format!(
                        "{} is manifest control without publication mutation policy",
                        table.name
                    ));
                }
                PublicationPinRole::PointerControl
                    if table.durable_mutation != DurableMutationClass::CurrentSingleton =>
                {
                    return Err(format!(
                        "{} is pointer control without singleton mutation policy",
                        table.name
                    ));
                }
                PublicationPinRole::NotPublished if table.required_for_publication => {
                    return Err(format!("{} is both unpublished and required", table.name));
                }
                _ => {}
            }
        }
        if self.tables.is_empty() {
            return Err("schema Contract IR has no TableSpecs".into());
        }
        let names_by_code = self
            .tables
            .iter()
            .map(|table| (table.table_code, table.name.as_str()))
            .collect::<BTreeMap<_, _>>();
        for table in &self.tables {
            for dependency in &table.dependencies {
                if !names_by_code.contains_key(dependency) {
                    return Err(format!(
                        "{} has unknown dependency {dependency}",
                        table.name
                    ));
                }
            }
        }
        let tables_by_code = self
            .tables
            .iter()
            .map(|table| (table.table_code, table))
            .collect::<BTreeMap<_, _>>();
        let mut scoped_table_codes = BTreeSet::new();
        for scope in &self.table_scopes {
            if !scoped_table_codes.insert(scope.table_code) {
                return Err(format!("duplicate table scope: {}", scope.table_code));
            }
            let table = tables_by_code
                .get(&scope.table_code)
                .ok_or_else(|| format!("scope references unknown table {}", scope.table_code))?;
            if scope.workspace_column.is_none()
                && scope.analysis_context_column.is_none()
                && scope.source_generation_column.is_none()
                && scope.analysis_context_set_column.is_none()
                && scope.owner_column.is_none()
            {
                return Err(format!("{} has an empty row-scope contract", table.name));
            }
            for (column_name, expected_type) in [
                (scope.workspace_column.as_deref(), LogicalType::Id16),
                (scope.analysis_context_column.as_deref(), LogicalType::Id16),
                (
                    scope.analysis_context_set_column.as_deref(),
                    LogicalType::Id16,
                ),
                (scope.owner_column.as_deref(), LogicalType::Id16),
                (
                    scope.source_generation_column.as_deref(),
                    LogicalType::Int64,
                ),
            ] {
                let Some(column_name) = column_name else {
                    continue;
                };
                let column = table
                    .columns
                    .iter()
                    .find(|column| column.name == column_name)
                    .ok_or_else(|| {
                        format!(
                            "{} scope references unknown column {column_name}",
                            table.name
                        )
                    })?;
                if column.nullable || column.logical_type != expected_type {
                    return Err(format!(
                        "{} scope column {column_name} has an incompatible type or nullability",
                        table.name
                    ));
                }
            }
            if matches!(
                table.durable_mutation,
                DurableMutationClass::OwnerReplacedFact
                    | DurableMutationClass::DerivedOwnerReplaced
            ) && table.materialization_role != MaterializationRole::OperationalProjection
                && (scope.workspace_column.is_none()
                    || scope.analysis_context_column.is_none()
                    || scope.source_generation_column.is_none()
                    || scope.owner_column.is_none())
            {
                return Err(format!(
                    "{} owner-replaced rows require the complete generated fact scope",
                    table.name
                ));
            }
        }
        for table in &self.tables {
            if matches!(
                table.durable_mutation,
                DurableMutationClass::OwnerReplacedFact
                    | DurableMutationClass::DerivedOwnerReplaced
            ) && table.materialization_role != MaterializationRole::OperationalProjection
                && !scoped_table_codes.contains(&table.table_code)
            {
                return Err(format!(
                    "{} owner-replaced rows lack a generated fact scope",
                    table.name
                ));
            }
        }
        let mut operational_names = BTreeSet::new();
        for table in &self.operational_tables {
            if !operational_names.insert(table.name.as_str()) {
                return Err(format!("duplicate operational table: {}", table.name));
            }
            let columns = table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<BTreeSet<_>>();
            for key in table
                .primary_key
                .iter()
                .chain(table.unique.iter().flatten())
            {
                if !columns.contains(key.as_str()) {
                    return Err(format!(
                        "{} references unknown SQL column {key}",
                        table.name
                    ));
                }
            }
            if let Some(scope) = &table.workspace_scope {
                match scope {
                    OperationalWorkspaceScopeContract::Direct { workspace_column } => {
                        let column = table
                            .columns
                            .iter()
                            .find(|column| column.name == *workspace_column);
                        if column.is_none_or(|column| {
                            column.nullable || column.sqlite_type != SqliteType::Blob
                        }) {
                            return Err(format!(
                                "{} workspace scope requires a non-null BLOB column {workspace_column}",
                                table.name
                            ));
                        }
                    }
                    OperationalWorkspaceScopeContract::ViaParent {
                        parent_table,
                        child_column,
                        parent_column,
                        workspace_column,
                    } => {
                        if !columns.contains(child_column.as_str()) {
                            return Err(format!(
                                "{} workspace scope references unknown child column {child_column}",
                                table.name
                            ));
                        }
                        let parent = self
                            .operational_tables
                            .iter()
                            .find(|candidate| candidate.name == *parent_table)
                            .ok_or_else(|| {
                                format!(
                                    "{} workspace scope references unknown parent {parent_table}",
                                    table.name
                                )
                            })?;
                        let child = table
                            .columns
                            .iter()
                            .find(|column| column.name == *child_column)
                            .expect("checked child scope column");
                        let parent_key = parent
                            .columns
                            .iter()
                            .find(|column| column.name == *parent_column);
                        let parent_workspace = parent
                            .columns
                            .iter()
                            .find(|column| column.name == *workspace_column);
                        if parent_key.is_none_or(|column| column.sqlite_type != child.sqlite_type)
                            || parent_workspace.is_none_or(|column| {
                                column.nullable || column.sqlite_type != SqliteType::Blob
                            })
                        {
                            return Err(format!(
                                "{} workspace scope has incompatible parent join or workspace columns",
                                table.name
                            ));
                        }
                    }
                }
            }
        }
        let scoped_operational_names = self
            .operational_tables
            .iter()
            .filter(|table| table.workspace_scope.is_some())
            .map(|table| table.name.as_str())
            .collect::<BTreeSet<_>>();
        let mut serving_view_names = BTreeSet::new();
        let mut serving_source_codes = BTreeSet::new();
        for projection in &self.serving_projections {
            if projection.availability_wave == 0
                || !serving_view_names.insert(projection.view_name.as_str())
                || !serving_source_codes.insert(projection.source_table_code)
            {
                return Err("serving projections require unique names/sources and a wave".into());
            }
            let table = tables_by_code
                .get(&projection.source_table_code)
                .ok_or_else(|| {
                    format!(
                        "serving projection {} references unknown table {}",
                        projection.view_name, projection.source_table_code
                    )
                })?;
            if table.materialization_role != MaterializationRole::DurableEffective
                || table.overlay_mutation == OverlayMutationPolicy::NotApplicable
            {
                return Err(format!(
                    "serving projection {} has an ineligible source",
                    projection.view_name
                ));
            }
        }
        let mut control_view_names = BTreeSet::new();
        let mut control_source_names = BTreeSet::new();
        for projection in &self.control_projections {
            if projection.availability_wave == 0
                || !control_view_names.insert(projection.view_name.as_str())
            {
                return Err("control projections require unique names and a wave".into());
            }
            match projection.projection_role {
                ControlProjectionRole::OperationalSource => {
                    let source = projection.source_table.as_deref().ok_or_else(|| {
                        format!(
                            "{} operational projection lacks a source",
                            projection.view_name
                        )
                    })?;
                    let table = self
                        .operational_tables
                        .iter()
                        .find(|table| table.name == source)
                        .ok_or_else(|| {
                            format!(
                                "{} references unknown source {source}",
                                projection.view_name
                            )
                        })?;
                    if table.workspace_scope.is_none()
                        || !projection.columns.is_empty()
                        || !control_source_names.insert(source)
                    {
                        return Err(format!(
                            "{} has an invalid operational-source projection",
                            projection.view_name
                        ));
                    }
                }
                ControlProjectionRole::DerivedOperational => {
                    let source = projection.source_table.as_deref().ok_or_else(|| {
                        format!("{} derived projection lacks a source", projection.view_name)
                    })?;
                    let table = self
                        .operational_tables
                        .iter()
                        .find(|table| table.name == source)
                        .ok_or_else(|| {
                            format!(
                                "{} references unknown source {source}",
                                projection.view_name
                            )
                        })?;
                    if projection.columns.is_empty()
                        || projection
                            .columns
                            .iter()
                            .any(|name| !table.columns.iter().any(|column| column.name == *name))
                    {
                        return Err(format!(
                            "{} derived projection references an unknown column",
                            projection.view_name
                        ));
                    }
                }
                ControlProjectionRole::ActiveServingSnapshot => {
                    if projection.source_table.is_some() || !projection.columns.is_empty() {
                        return Err(format!(
                            "{} synthetic projection must not declare a source or columns",
                            projection.view_name
                        ));
                    }
                }
            }
        }
        if control_source_names != scoped_operational_names {
            return Err(
                "every workspace-scoped operational table must have one generated control projection"
                    .into(),
            );
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
            return Err("serving resource profile limits must all be positive".into());
        }
        let expected_operational_names = BTreeSet::from([
            "active_snapshot",
            "audit_event",
            "common_repository_state",
            "credential_metadata",
            "git_operation_run",
            "git_state_vector",
            "hot_overlay_manifest",
            "nested_root_exclusion",
            "provider_run",
            "repository_registration",
            "result_artifact_lease",
            "serving_snapshot_manifest",
            "snapshot_lease",
            "source_blob",
            "source_blob_lease",
            "source_blob_lease_member",
            "source_inventory",
            "table_mutation_operation",
            "update_wave",
            "update_wave_item",
            "workspace_generation",
            "workspace_registration",
            "worktree_registration",
            "worktree_state",
        ]);
        if operational_names != expected_operational_names {
            return Err("operational table census differs from the closed AC-G-27 set".into());
        }
        let schemas = self
            .public_schemas
            .iter()
            .map(|schema| schema.schema_kind)
            .collect::<BTreeSet<_>>();
        if schemas.len() != 8 || schemas.len() != self.public_schemas.len() {
            return Err("the public-schema catalog must contain each of eight kinds once".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> SchemaContractIr {
        serde_json::from_str(include_str!(
            "../../contracts/schema/schema-contract-ir.json"
        ))
        .unwrap()
    }

    #[test]
    fn schema_contract_ir_is_closed_and_complete() {
        authority().validate().unwrap();
    }

    #[test]
    fn operational_workspace_scopes_are_closed_and_resolvable() {
        let mut missing = authority();
        missing
            .operational_tables
            .iter_mut()
            .find(|table| table.name == "update_wave_item")
            .unwrap()
            .workspace_scope = None;
        assert!(missing.validate().is_err());

        let mut unresolved = authority();
        unresolved
            .operational_tables
            .iter_mut()
            .find(|table| table.name == "result_artifact_lease")
            .unwrap()
            .workspace_scope = Some(OperationalWorkspaceScopeContract::ViaParent {
            parent_table: "missing_parent".into(),
            child_column: "lease_id".into(),
            parent_column: "lease_id".into(),
            workspace_column: "workspace_id".into(),
        });
        assert!(unresolved.validate().is_err());

        let mut nullable_workspace = authority();
        nullable_workspace
            .operational_tables
            .iter_mut()
            .find(|table| table.name == "worktree_state")
            .unwrap()
            .columns
            .iter_mut()
            .find(|column| column.name == "workspace_id")
            .unwrap()
            .nullable = true;
        assert!(nullable_workspace.validate().is_err());

        let mut incompatible_join = authority();
        incompatible_join
            .operational_tables
            .iter_mut()
            .find(|table| table.name == "result_artifact_lease")
            .unwrap()
            .columns
            .iter_mut()
            .find(|column| column.name == "lease_id")
            .unwrap()
            .sqlite_type = SqliteType::Text;
        assert!(incompatible_join.validate().is_err());
    }

    #[test]
    fn serving_and_control_projections_are_total_and_unique() {
        let mut duplicate = authority();
        duplicate.serving_projections[1].view_name =
            duplicate.serving_projections[0].view_name.clone();
        assert!(duplicate.validate().is_err());

        let mut unowned = authority();
        unowned.control_projections.remove(0);
        assert!(unowned.validate().is_err());

        let mut unknown_column = authority();
        unknown_column
            .control_projections
            .iter_mut()
            .find(|projection| {
                projection.projection_role == ControlProjectionRole::DerivedOperational
            })
            .unwrap()
            .columns
            .push("missing_column".into());
        assert!(unknown_column.validate().is_err());
    }

    #[test]
    fn wp09_negative_zero_state() {
        let mut duplicate = authority();
        duplicate.tables[1].table_code = duplicate.tables[0].table_code;
        assert!(
            duplicate
                .validate()
                .unwrap_err()
                .contains("duplicate table")
        );

        let mut json_blob = authority();
        json_blob.tables[0].columns[4].name = "opaque_json".to_owned();
        json_blob.tables[0].columns[4].logical_type = LogicalType::Utf8;
        assert!(json_blob.validate().unwrap_err().contains("JSON/EAV"));

        let mut illegal_axis = authority();
        illegal_axis.tables[10].overlay_mutation = OverlayMutationPolicy::OwnerReplace;
        assert!(
            illegal_axis
                .validate()
                .unwrap_err()
                .contains("bundle-dimension")
        );

        let source = include_str!("../../contracts/schema/schema-contract-ir.json").replacen(
            "\"logical_type\":\"utf8\"",
            "\"logical_type\":\"utf8_view\"",
            1,
        );
        assert!(serde_json::from_str::<SchemaContractIr>(&source).is_err());
    }
}
