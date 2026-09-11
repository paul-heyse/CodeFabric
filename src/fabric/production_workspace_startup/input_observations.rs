//! Immutable, schema-bearing source and provider-support inputs for one candidate epoch.
//!
//! These are application-owned input relations, not a second invalidation engine. Typed
//! dependencies are stored once per canonical bundle and joined through common/local bundle
//! references. The programmatic epoch builder owns their subsequent exact Delta sealing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, BooleanArray, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{Field, Schema};
use datafusion::common::TableReference;

use super::{ProductionWorkspaceStartupError, step};
use crate::fabric::epoch_runtime::{FABRIC_CATALOG, FabricSchemaRole};
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::programmatic_schema::{ProgrammaticRelationId, ProviderInput};
use crate::fabric::{hash32_array, id16_array};
use crate::provider_contracts::{
    AdmittedProviderResult, ProviderInputDisposition, ProviderLookupKey, ProviderLookupOutcome,
    ProviderPartitionAction, ProviderPartitionSupport, ProviderSourceInventory,
    ProviderSourceSelection, ProviderSupportDependency, ProviderSupportOwner,
    provider_support_bundle_identity,
};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
};

pub(super) fn install_input_observations(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventory: &ProviderSourceInventory,
    runs: &[AdmittedProviderResult],
) -> Result<(), ProductionWorkspaceStartupError> {
    let selected_files = inventory
        .members()
        .iter()
        .filter_map(|member| match member.disposition {
            ProviderInputDisposition::Captured {
                file_id, digest, ..
            } => Some((file_id, digest)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut inventories = BTreeMap::from([(inventory.identity(), inventory)]);
    let mut run_ids = BTreeSet::new();
    for run in runs {
        let source = run.job().source();
        let selected = match source.selection() {
            ProviderSourceSelection::Inventory(bound) => {
                inventories.insert(bound.identity(), &**bound);
                bound.has_same_capture(inventory)
            }
            ProviderSourceSelection::File {
                file_id,
                content_digest,
            } => selected_files.get(file_id) == Some(content_digest),
        };
        if !selected
            || source.workspace_id() != inventory.workspace_id()
            || source.generation() != inventory.source_generation()
        {
            return Err(step(
                "input-observation-source-binding",
                "admitted provider run is not bound to the candidate's exact source inventory",
            ));
        }
        if !run_ids.insert(run.job().run().provider_run_id()) {
            return Err(step(
                "input-observation-run-binding",
                "duplicate admitted provider run",
            ));
        }
    }
    install_inventory(builder, &inventories.into_values().collect::<Vec<_>>())?;
    install_provider_progress(builder, runs)?;
    install_support(builder, runs)
}

#[allow(clippy::too_many_lines)] // Two small typed Arrow tables share the admitted run census.
fn install_provider_progress(
    builder: &mut ProgrammaticFabricEpochBuilder,
    runs: &[AdmittedProviderResult],
) -> Result<(), ProductionWorkspaceStartupError> {
    use crate::provider_contracts::{ProviderCoverageState, ProviderLane};
    struct Family<'a> {
        run: [u8; 16],
        name: &'a str,
        scope: &'a str,
        requested: u64,
        completed: u64,
        state: &'static str,
        reason: String,
    }
    let ids = runs
        .iter()
        .map(|run| run.job().run().provider_run_id())
        .collect::<Vec<_>>();
    let contexts = runs
        .iter()
        .map(|run| run.job().context().analysis_context_id())
        .collect::<Vec<_>>();
    let fingerprints = runs
        .iter()
        .map(|run| run.job().context().context_fingerprint())
        .collect::<Vec<_>>();
    let inventories = runs
        .iter()
        .map(|run| match run.job().source().selection() {
            ProviderSourceSelection::Inventory(inventory) => Some(inventory.identity()),
            ProviderSourceSelection::File { .. } => None,
        })
        .collect::<Vec<_>>();
    let terminal = runs
        .iter()
        .map(|run| format!("{:?}", run.result().terminal()))
        .collect::<Vec<_>>();
    register(
        builder,
        FabricSchemaRole::System,
        "provider_run_scope",
        vec![
            ("provider_run_id", false, id16_array(ids.iter().map(Some))),
            (
                "provider_run_identity",
                false,
                strings(
                    runs.iter()
                        .map(|run| Some(run.job().run().identity().as_str())),
                ),
            ),
            ("context_id", false, id16_array(contexts.iter().map(Some))),
            (
                "context_fingerprint",
                false,
                hash32_array(fingerprints.iter().map(Some)),
            ),
            (
                "source_generation",
                false,
                numbers(runs.iter().map(|run| run.job().source().generation())),
            ),
            (
                "input_set_id",
                true,
                hash32_array(inventories.iter().map(Option::as_ref)),
            ),
            (
                "file_id",
                true,
                id16_array(runs.iter().map(|run| match run.job().source().selection() {
                    ProviderSourceSelection::File { file_id, .. } => Some(file_id),
                    ProviderSourceSelection::Inventory(_) => None,
                })),
            ),
            (
                "provider",
                false,
                strings(runs.iter().map(|run| {
                    Some(match run.job().lane() {
                        ProviderLane::TreeSitter => "tree-sitter",
                        ProviderLane::TreeSitterRust => "tree-sitter-rust",
                        ProviderLane::Ruff => "ruff",
                        ProviderLane::Pyrefly => "pyrefly",
                        ProviderLane::Rustc => "rustc",
                    })
                })),
            ),
            (
                "terminal_state",
                false,
                strings(terminal.iter().map(|value| Some(value.as_str()))),
            ),
        ],
    )?;
    let mut families = Vec::new();
    for run in runs {
        for coverage in run.result().coverage() {
            let request = run
                .job()
                .requests()
                .iter()
                .find(|request| request.family() == coverage.family())
                .ok_or_else(|| step("provider-progress", "unrequested coverage family"))?;
            let (state, reason) = match coverage.state() {
                ProviderCoverageState::Complete { .. } => ("complete", String::new()),
                ProviderCoverageState::IntentionalRemainder { reason, .. } => {
                    ("partial", format!("{reason:?}"))
                }
                ProviderCoverageState::Unknown { cause, .. } => ("unknown", format!("{cause:?}")),
            };
            families.push(Family {
                run: run.job().run().provider_run_id(),
                name: coverage.family().as_str(),
                scope: request.scope().as_str(),
                requested: request.requested_units(),
                completed: coverage.state().completed_units(),
                state,
                reason,
            });
        }
    }
    register(
        builder,
        FabricSchemaRole::System,
        "provider_family_progress",
        vec![
            (
                "provider_run_id",
                false,
                id16_array(families.iter().map(|row| Some(&row.run))),
            ),
            (
                "family",
                false,
                strings(families.iter().map(|row| Some(row.name))),
            ),
            (
                "scope",
                false,
                strings(families.iter().map(|row| Some(row.scope))),
            ),
            (
                "requested_units",
                false,
                numbers(families.iter().map(|row| row.requested)),
            ),
            (
                "completed_units",
                false,
                numbers(families.iter().map(|row| row.completed)),
            ),
            (
                "remaining_units",
                false,
                numbers(families.iter().map(|row| row.requested - row.completed)),
            ),
            (
                "processing_state",
                false,
                strings(families.iter().map(|row| Some(row.state))),
            ),
            (
                "reason",
                false,
                strings(families.iter().map(|row| Some(row.reason.as_str()))),
            ),
        ],
    )
}

pub(super) fn install_rust_target_progress(
    builder: &mut ProgrammaticFabricEpochBuilder,
    generation: u64,
    progress: &[super::rustc::RustTargetProgress],
) -> Result<(), ProductionWorkspaceStartupError> {
    register(
        builder,
        FabricSchemaRole::System,
        "rust_target_progress",
        vec![
            (
                "source_generation",
                false,
                Arc::new(UInt64Array::from(vec![generation; progress.len()])),
            ),
            (
                "manifest_path",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    progress.iter().map(|row| row.manifest.as_slice()),
                )),
            ),
            (
                "target_name",
                false,
                Arc::new(StringArray::from_iter_values(
                    progress.iter().map(|row| row.target.as_str()),
                )),
            ),
            (
                "target_kind",
                false,
                Arc::new(StringArray::from_iter_values(
                    progress.iter().map(|row| row.target_kind.as_str()),
                )),
            ),
            (
                "target_platform",
                true,
                Arc::new(StringArray::from_iter(
                    progress.iter().map(|row| row.target_platform.as_deref()),
                )),
            ),
            (
                "build_profile",
                true,
                Arc::new(StringArray::from_iter(progress.iter().map(|row| {
                    row.rust_build.as_ref().map(|build| build.profile.as_str())
                }))),
            ),
            (
                "build_features",
                true,
                string_lists(progress.iter().map(|row| {
                    row.rust_build
                        .as_ref()
                        .map(|build| build.features.as_slice())
                })),
            ),
            (
                "default_features",
                true,
                Arc::new(BooleanArray::from_iter(progress.iter().map(|row| {
                    row.rust_build.as_ref().map(|build| build.default_features)
                }))),
            ),
            (
                "context_id",
                true,
                id16_array(progress.iter().map(|row| row.context_id.as_ref())),
            ),
            (
                "processing_state",
                false,
                Arc::new(StringArray::from_iter_values(
                    progress.iter().map(|row| row.state),
                )),
            ),
            (
                "detail",
                false,
                Arc::new(StringArray::from_iter_values(
                    progress.iter().map(|row| row.detail.as_str()),
                )),
            ),
        ],
    )
}

pub(super) fn string_lists<'a>(values: impl Iterator<Item = Option<&'a [String]>>) -> ArrayRef {
    let mut builder =
        arrow_array::builder::ListBuilder::new(arrow_array::builder::StringBuilder::new());
    for values in values {
        if let Some(values) = values {
            for value in values {
                builder.values().append_value(value);
            }
            builder.append(true);
        } else {
            builder.append(false);
        }
    }
    Arc::new(builder.finish())
}

type Column = (&'static str, bool, ArrayRef);

/// Observe the native Arrow columns; field/relation identities remain explicit and stable.
pub(super) fn register(
    builder: &mut ProgrammaticFabricEpochBuilder,
    role: FabricSchemaRole,
    table: &'static str,
    columns: Vec<Column>,
) -> Result<(), ProductionWorkspaceStartupError> {
    register_batches(builder, role, table, vec![columns], None)
}

/// Only complete captured inputs with a fixed producer revision qualify for version reuse.
pub(super) fn register_immutable(
    builder: &mut ProgrammaticFabricEpochBuilder,
    role: FabricSchemaRole,
    table: &'static str,
    columns: Vec<Column>,
    identity: [u8; 32],
) -> Result<(), ProductionWorkspaceStartupError> {
    register_batches(builder, role, table, vec![columns], Some(identity))
}

fn register_batches(
    builder: &mut ProgrammaticFabricEpochBuilder,
    role: FabricSchemaRole,
    table: &'static str,
    batches: Vec<Vec<Column>>,
    immutable_input_identity: Option<[u8; 32]>,
) -> Result<(), ProductionWorkspaceStartupError> {
    let columns = batches
        .first()
        .ok_or_else(|| step("input-observation-batches", "missing schema batch"))?;
    let relation_id = format!("{}.{}", role.as_str(), table);
    let fields = columns
        .iter()
        .map(|(name, nullable, array)| {
            Field::new(*name, array.data_type().clone(), *nullable).with_metadata(HashMap::from([
                (
                    FIELD_ID_METADATA_KEY.to_owned(),
                    format!("{relation_id}.{name}"),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields).with_metadata(HashMap::from([(
        RELATION_ID_METADATA_KEY.to_owned(),
        relation_id.clone(),
    )])));
    let mappings = (0..columns.len())
        .map(|index| FieldIndexMapping::direct(index, index))
        .collect();
    let batches = batches
        .into_iter()
        .map(|columns| {
            RecordBatch::try_new(
                Arc::clone(&schema),
                columns.into_iter().map(|(_, _, array)| array).collect(),
            )
            .map_err(|error| step("input-observation-arrow-batch", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reference = TableReference::full(FABRIC_CATALOG, role.as_str(), table);
    let contract = SchemaContract::try_new(
        format!("codefabric.input-observation.v1:{relation_id}"),
        reference.clone(),
        Arc::clone(&schema),
        schema,
        mappings,
    )
    .map_err(|error| step("input-observation-schema-contract", error))?;
    let mut input = ProviderInput::try_from_arrow(
        ProgrammaticRelationId::new(relation_id),
        reference,
        Arc::new(contract),
        vec![batches],
    )
    .map_err(|error| step("input-observation-memtable", error))?;
    if let Some(identity) = immutable_input_identity {
        input = input.with_immutable_input_identity(identity);
    }
    builder
        .register_provider(input)
        .map_err(|error| step("input-observation-registration", error))
}

fn binary<'a>(values: impl IntoIterator<Item = Option<&'a [u8]>>) -> ArrayRef {
    Arc::new(BinaryArray::from_iter(values))
}

fn strings<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> ArrayRef {
    Arc::new(StringArray::from_iter(values))
}

fn numbers(values: impl IntoIterator<Item = u64>) -> ArrayRef {
    Arc::new(UInt64Array::from_iter_values(values))
}

fn booleans(values: impl IntoIterator<Item = bool>) -> ArrayRef {
    Arc::new(BooleanArray::from_iter(values.into_iter().map(Some)))
}

fn cardinality(value: usize) -> Result<u64, ProductionWorkspaceStartupError> {
    u64::try_from(value).map_err(|error| step("input-observation-cardinality", error))
}

fn install_inventory(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventories: &[&ProviderSourceInventory],
) -> Result<(), ProductionWorkspaceStartupError> {
    let mut tables = BTreeMap::<&'static str, Vec<Vec<Column>>>::new();
    for inventory in inventories {
        collect_inventory_columns(inventory, &mut |table, columns| {
            tables.entry(table).or_default().push(columns);
            Ok(())
        })?;
    }
    for (table, batches) in tables {
        register_batches(builder, FabricSchemaRole::Source, table, batches, None)?;
    }
    Ok(())
}

fn collect_inventory_columns(
    inventory: &ProviderSourceInventory,
    collect: &mut impl FnMut(&'static str, Vec<Column>) -> Result<(), ProductionWorkspaceStartupError>,
) -> Result<(), ProductionWorkspaceStartupError> {
    let workspace = inventory.workspace_id();
    let generation = inventory.source_generation();
    let input_set = inventory.identity();
    let members = inventory.members();
    collect(
        "input_inventory_set",
        vec![
            ("workspace_id", false, id16_array([Some(&workspace)])),
            ("source_generation", false, numbers([generation])),
            ("input_set_id", false, hash32_array([Some(&input_set)])),
            (
                "member_count",
                false,
                numbers([cardinality(members.len())?]),
            ),
            (
                "selected_count",
                false,
                numbers([cardinality(inventory.selected_files().count())?]),
            ),
            (
                "changed_count",
                false,
                numbers([cardinality(inventory.changed_paths().len())?]),
            ),
            (
                "withdrawn_count",
                false,
                numbers([cardinality(inventory.withdrawn().len())?]),
            ),
        ],
    )?;
    collect_inventory_members(inventory, collect)?;
    let withdrawn = inventory.withdrawn();
    collect(
        "input_inventory_withdrawal",
        vec![
            (
                "workspace_id",
                false,
                id16_array(withdrawn.iter().map(|_| Some(&workspace))),
            ),
            (
                "source_generation",
                false,
                numbers(withdrawn.iter().map(|_| generation)),
            ),
            (
                "input_set_id",
                false,
                hash32_array(withdrawn.iter().map(|_| Some(&input_set))),
            ),
            (
                "file_id",
                false,
                id16_array(withdrawn.iter().map(|(file, _)| Some(file))),
            ),
            (
                "relative_path",
                false,
                binary(withdrawn.iter().map(|(_, path)| Some(path.as_slice()))),
            ),
        ],
    )
}

fn collect_inventory_members(
    inventory: &ProviderSourceInventory,
    collect: &mut impl FnMut(&'static str, Vec<Column>) -> Result<(), ProductionWorkspaceStartupError>,
) -> Result<(), ProductionWorkspaceStartupError> {
    let workspace = inventory.workspace_id();
    let generation = inventory.source_generation();
    let input_set = inventory.identity();
    let members = inventory.members();
    let changed = inventory
        .changed_paths()
        .iter()
        .map(Vec::as_slice)
        .collect::<BTreeSet<_>>();
    collect(
        "input_inventory",
        vec![
            (
                "workspace_id",
                false,
                id16_array(members.iter().map(|_| Some(&workspace))),
            ),
            (
                "source_generation",
                false,
                numbers(members.iter().map(|_| generation)),
            ),
            (
                "input_set_id",
                false,
                hash32_array(members.iter().map(|_| Some(&input_set))),
            ),
            (
                "relative_path",
                false,
                binary(
                    members
                        .iter()
                        .map(|member| Some(member.relative_path.as_slice())),
                ),
            ),
            (
                "disposition",
                false,
                strings(
                    members
                        .iter()
                        .map(|member| Some(disposition(&member.disposition))),
                ),
            ),
            (
                "file_id",
                true,
                id16_array(members.iter().map(|member| match &member.disposition {
                    ProviderInputDisposition::Captured { file_id, .. } => Some(file_id),
                    _ => None,
                })),
            ),
            (
                "content_digest",
                true,
                hash32_array(members.iter().map(|member| match &member.disposition {
                    ProviderInputDisposition::Captured { digest, .. } => Some(digest),
                    _ => None,
                })),
            ),
            (
                "byte_length",
                true,
                Arc::new(UInt64Array::from_iter(members.iter().map(
                    |member| match member.disposition {
                        ProviderInputDisposition::Captured { byte_length, .. } => Some(byte_length),
                        _ => None,
                    },
                ))),
            ),
            (
                "selected_for_provider",
                false,
                booleans(members.iter().map(|member| member.selected_for_provider)),
            ),
            (
                "changed",
                false,
                booleans(
                    members
                        .iter()
                        .map(|member| changed.contains(member.relative_path.as_slice())),
                ),
            ),
        ],
    )
}

pub(super) const fn disposition(value: &ProviderInputDisposition) -> &'static str {
    match value {
        ProviderInputDisposition::Captured { .. } => "captured",
        ProviderInputDisposition::ExcludedPolicy => "excluded_policy",
        ProviderInputDisposition::ExcludedSpecialFile => "excluded_special_file",
        ProviderInputDisposition::ExcludedSizeLimit => "excluded_size_limit",
        ProviderInputDisposition::Binary => "binary",
        ProviderInputDisposition::UnsupportedEncoding => "unsupported_encoding",
        ProviderInputDisposition::Unreadable => "unreadable",
        ProviderInputDisposition::Generated => "generated",
        ProviderInputDisposition::Vendored => "vendored",
        ProviderInputDisposition::Unsupported => "unsupported",
    }
}

struct PartitionRow<'a> {
    run_id: [u8; 16],
    support: &'a ProviderPartitionSupport,
    common_set: [u8; 32],
    local_set: [u8; 32],
}

type Bundles<'a> = BTreeMap<[u8; 32], Vec<&'a ProviderSupportDependency>>;

fn intern_bundle<'a>(
    bundles: &mut Bundles<'a>,
    dependencies: &'a [ProviderSupportDependency],
) -> Result<[u8; 32], ProductionWorkspaceStartupError> {
    let identity = provider_support_bundle_identity(dependencies)
        .map_err(|error| step("input-observation-support-bundle", error))?;
    let mut ordered = dependencies.iter().collect::<Vec<_>>();
    ordered.sort();
    // The bundle contract orders dependencies canonically, preserving each lookup's root order.
    // Compare an existing payload too: silently aliasing unequal support is never admissible.
    if let Some(existing) = bundles.get(&identity) {
        if existing != &ordered {
            return Err(step(
                "input-observation-support-bundle",
                "bundle identity collision",
            ));
        }
    } else {
        bundles.insert(identity, ordered);
    }
    Ok(identity)
}

fn install_support(
    builder: &mut ProgrammaticFabricEpochBuilder,
    runs: &[AdmittedProviderResult],
) -> Result<(), ProductionWorkspaceStartupError> {
    let mut bundles = BTreeMap::new();
    let mut partitions = Vec::new();
    for run in runs {
        let support = run.result().support();
        let common_set = intern_bundle(&mut bundles, support.common_dependencies())?;
        for partition in &support.partitions {
            partitions.push(PartitionRow {
                run_id: run.job().run().provider_run_id(),
                support: partition,
                common_set,
                local_set: intern_bundle(&mut bundles, &partition.dependencies)?,
            });
        }
    }
    let counts = bundles
        .values()
        .map(|dependencies| cardinality(dependencies.len()))
        .collect::<Result<Vec<_>, _>>()?;
    register(
        builder,
        FabricSchemaRole::System,
        "provider_dependency_set",
        vec![
            (
                "dependency_set_id",
                false,
                hash32_array(bundles.keys().map(Some)),
            ),
            ("dependency_count", false, numbers(counts)),
        ],
    )?;
    install_dependencies(builder, &bundles)?;
    register_partitions(builder, &partitions)
}

fn register_partitions(
    builder: &mut ProgrammaticFabricEpochBuilder,
    partitions: &[PartitionRow<'_>],
) -> Result<(), ProductionWorkspaceStartupError> {
    register(
        builder,
        FabricSchemaRole::System,
        "provider_partition_support",
        vec![
            (
                "provider_run_id",
                false,
                id16_array(partitions.iter().map(|row| Some(&row.run_id))),
            ),
            (
                "owner_kind",
                false,
                strings(
                    partitions
                        .iter()
                        .map(|row| Some(owner(&row.support.partition.key.owner).0)),
                ),
            ),
            (
                "owner_id",
                false,
                id16_array(
                    partitions
                        .iter()
                        .map(|row| Some(owner(&row.support.partition.key.owner).1)),
                ),
            ),
            (
                "family",
                false,
                strings(
                    partitions
                        .iter()
                        .map(|row| Some(row.support.partition.key.family.as_str())),
                ),
            ),
            (
                "context_id",
                false,
                id16_array(
                    partitions
                        .iter()
                        .map(|row| Some(&row.support.partition.key.context_id)),
                ),
            ),
            (
                "producer_release",
                false,
                strings(
                    partitions
                        .iter()
                        .map(|row| Some(row.support.partition.key.producer_release.as_str())),
                ),
            ),
            (
                "action",
                false,
                strings(partitions.iter().map(|row| {
                    Some(match row.support.partition.action {
                        ProviderPartitionAction::Replace => "replace",
                        ProviderPartitionAction::Withdraw => "withdraw",
                    })
                })),
            ),
            (
                "support_complete",
                false,
                booleans(partitions.iter().map(|row| row.support.support_complete)),
            ),
            (
                "common_dependency_set_id",
                false,
                hash32_array(partitions.iter().map(|row| Some(&row.common_set))),
            ),
            (
                "local_dependency_set_id",
                false,
                hash32_array(partitions.iter().map(|row| Some(&row.local_set))),
            ),
        ],
    )
}

const fn owner(value: &ProviderSupportOwner) -> (&'static str, &[u8; 16]) {
    match value {
        ProviderSupportOwner::File(id) => ("file", id),
        ProviderSupportOwner::Entity(id) => ("entity", id),
        ProviderSupportOwner::Context(id) => ("context", id),
    }
}

#[derive(Default)]
struct LookupColumns<'a> {
    kind: &'static str,
    file_id: Option<&'a [u8; 16]>,
    relative_path: Option<&'a [u8]>,
    relation: Option<&'a str>,
    entity_id: Option<&'a [u8; 16]>,
    qualified_name: Option<&'a [u8]>,
    name: Option<&'a [u8]>,
    receiver_type: Option<&'a [u8; 16]>,
    trait_id: Option<&'a [u8; 16]>,
    callable_id: Option<&'a [u8; 16]>,
    projection_id: Option<&'a [u8; 32]>,
}

fn lookup_columns(value: &ProviderLookupKey) -> LookupColumns<'_> {
    match value {
        ProviderLookupKey::SourceBytes { file_id } => LookupColumns {
            kind: "source_bytes",
            file_id: Some(file_id),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Configuration { relative_path } => LookupColumns {
            kind: "configuration",
            relative_path: Some(relative_path),
            ..LookupColumns::default()
        },
        ProviderLookupKey::BuildInput { relative_path } => LookupColumns {
            kind: "build_input",
            relative_path: Some(relative_path),
            ..LookupColumns::default()
        },
        ProviderLookupKey::ProducerRelease => LookupColumns {
            kind: "producer_release",
            ..LookupColumns::default()
        },
        ProviderLookupKey::EffectiveContextInputs => LookupColumns {
            kind: "effective_context_inputs",
            ..LookupColumns::default()
        },
        ProviderLookupKey::PositiveFact {
            relation,
            entity_id,
        } => LookupColumns {
            kind: "positive_fact",
            relation: Some(relation),
            entity_id: Some(entity_id),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Import { qualified_name } => LookupColumns {
            kind: "import",
            qualified_name: Some(qualified_name),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Export { qualified_name } => LookupColumns {
            kind: "export",
            qualified_name: Some(qualified_name),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Name { name } => LookupColumns {
            kind: "name",
            name: Some(name),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Member {
            receiver_type,
            name,
        } => LookupColumns {
            kind: "member",
            receiver_type: Some(receiver_type),
            name: Some(name),
            ..LookupColumns::default()
        },
        ProviderLookupKey::Implementation {
            trait_id,
            receiver_type,
        } => LookupColumns {
            kind: "implementation",
            trait_id: Some(trait_id),
            receiver_type: Some(receiver_type),
            ..LookupColumns::default()
        },
        ProviderLookupKey::CallableSummary { callable_id } => LookupColumns {
            kind: "callable_summary",
            callable_id: Some(callable_id),
            ..LookupColumns::default()
        },
        ProviderLookupKey::GraphProjection { projection_id } => LookupColumns {
            kind: "graph_projection",
            projection_id: Some(projection_id),
            ..LookupColumns::default()
        },
    }
}

struct DependencyRow<'a> {
    set_id: &'a [u8; 32],
    ordinal: u64,
    dependency: &'a ProviderSupportDependency,
    key: LookupColumns<'a>,
}

fn install_dependencies(
    builder: &mut ProgrammaticFabricEpochBuilder,
    bundles: &Bundles<'_>,
) -> Result<(), ProductionWorkspaceStartupError> {
    let mut rows = Vec::new();
    for (set_id, dependencies) in bundles {
        for (index, dependency) in dependencies.iter().enumerate() {
            rows.push(DependencyRow {
                set_id,
                ordinal: cardinality(index)?,
                dependency,
                key: lookup_columns(&dependency.key),
            });
        }
    }
    register_dependencies(builder, &rows)?;
    let mut roots = Vec::new();
    let mut candidates = Vec::new();
    for row in &rows {
        for (index, root) in row.dependency.scope.ordered_roots.iter().enumerate() {
            roots.push((row, cardinality(index)?, root));
        }
        if let ProviderLookupOutcome::Consumed {
            candidates: values, ..
        } = &row.dependency.outcome
        {
            for (index, candidate) in values.iter().enumerate() {
                candidates.push((row, cardinality(index)?, candidate));
            }
        }
    }
    register(
        builder,
        FabricSchemaRole::System,
        "provider_dependency_search_root",
        vec![
            (
                "dependency_set_id",
                false,
                hash32_array(roots.iter().map(|(row, _, _)| Some(row.set_id))),
            ),
            (
                "dependency_ordinal",
                false,
                numbers(roots.iter().map(|(row, _, _)| row.ordinal)),
            ),
            (
                "root_ordinal",
                false,
                numbers(roots.iter().map(|(_, ordinal, _)| *ordinal)),
            ),
            (
                "relative_root",
                false,
                binary(roots.iter().map(|(_, _, root)| Some(root.as_slice()))),
            ),
        ],
    )?;
    register(
        builder,
        FabricSchemaRole::System,
        "provider_dependency_candidate",
        vec![
            (
                "dependency_set_id",
                false,
                hash32_array(candidates.iter().map(|(row, _, _)| Some(row.set_id))),
            ),
            (
                "dependency_ordinal",
                false,
                numbers(candidates.iter().map(|(row, _, _)| row.ordinal)),
            ),
            (
                "candidate_ordinal",
                false,
                numbers(candidates.iter().map(|(_, ordinal, _)| *ordinal)),
            ),
            (
                "candidate_id",
                false,
                id16_array(candidates.iter().map(|(_, _, candidate)| Some(*candidate))),
            ),
        ],
    )
}

fn register_dependencies(
    builder: &mut ProgrammaticFabricEpochBuilder,
    rows: &[DependencyRow<'_>],
) -> Result<(), ProductionWorkspaceStartupError> {
    let mut columns = dependency_key_columns(rows);
    columns.extend(dependency_scope_outcome_columns(rows));
    register(
        builder,
        FabricSchemaRole::System,
        "provider_dependency_support",
        columns,
    )
}

fn dependency_key_columns(rows: &[DependencyRow<'_>]) -> Vec<Column> {
    vec![
        (
            "dependency_set_id",
            false,
            hash32_array(rows.iter().map(|row| Some(row.set_id))),
        ),
        (
            "dependency_ordinal",
            false,
            numbers(rows.iter().map(|row| row.ordinal)),
        ),
        (
            "key_kind",
            false,
            strings(rows.iter().map(|row| Some(row.key.kind))),
        ),
        (
            "file_id",
            true,
            id16_array(rows.iter().map(|row| row.key.file_id)),
        ),
        (
            "relative_path",
            true,
            binary(rows.iter().map(|row| row.key.relative_path)),
        ),
        (
            "relation",
            true,
            strings(rows.iter().map(|row| row.key.relation)),
        ),
        (
            "entity_id",
            true,
            id16_array(rows.iter().map(|row| row.key.entity_id)),
        ),
        (
            "qualified_name",
            true,
            binary(rows.iter().map(|row| row.key.qualified_name)),
        ),
        ("name", true, binary(rows.iter().map(|row| row.key.name))),
        (
            "receiver_type",
            true,
            id16_array(rows.iter().map(|row| row.key.receiver_type)),
        ),
        (
            "trait_id",
            true,
            id16_array(rows.iter().map(|row| row.key.trait_id)),
        ),
        (
            "callable_id",
            true,
            id16_array(rows.iter().map(|row| row.key.callable_id)),
        ),
        (
            "projection_id",
            true,
            hash32_array(rows.iter().map(|row| row.key.projection_id)),
        ),
    ]
}

fn dependency_scope_outcome_columns(rows: &[DependencyRow<'_>]) -> Vec<Column> {
    vec![
        (
            "namespace",
            false,
            binary(
                rows.iter()
                    .map(|row| Some(row.dependency.scope.namespace.as_slice())),
            ),
        ),
        (
            "policy_identity",
            false,
            hash32_array(
                rows.iter()
                    .map(|row| Some(&row.dependency.scope.policy_identity)),
            ),
        ),
        (
            "effective_context",
            false,
            hash32_array(
                rows.iter()
                    .map(|row| Some(&row.dependency.scope.effective_context)),
            ),
        ),
        (
            "outcome_kind",
            false,
            strings(rows.iter().map(|row| {
                Some(match row.dependency.outcome {
                    ProviderLookupOutcome::Consumed { .. } => "consumed",
                    ProviderLookupOutcome::Absent { .. } => "absent",
                    ProviderLookupOutcome::Incomplete { .. } => "incomplete",
                })
            })),
        ),
        (
            "consumed_revision",
            true,
            hash32_array(rows.iter().map(|row| match &row.dependency.outcome {
                ProviderLookupOutcome::Consumed { revision, .. } => Some(revision),
                _ => None,
            })),
        ),
        (
            "closed_universe",
            true,
            hash32_array(rows.iter().map(|row| match &row.dependency.outcome {
                ProviderLookupOutcome::Absent { closed_universe } => Some(closed_universe),
                _ => None,
            })),
        ),
        (
            "observed_universe",
            true,
            hash32_array(rows.iter().map(|row| match &row.dependency.outcome {
                ProviderLookupOutcome::Incomplete { observed_universe } => Some(observed_universe),
                _ => None,
            })),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use arrow_array::{FixedSizeBinaryArray, Int64Array};
    use arrow_schema::DataType;
    use datafusion::prelude::SessionContext;

    use super::*;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig};
    use crate::provider_contracts::{
        CancellationProbe, ContextIdentity, ProviderBuildIdentity, ProviderContextBinding,
        ProviderCoverage, ProviderCoverageState, ProviderFamilyIdentity, ProviderFamilyRequest,
        ProviderIdentity, ProviderInventoryMember, ProviderJob, ProviderJobSpec, ProviderLane,
        ProviderPolicyIdentity, ProviderProgramIdentity, ProviderProtocolIdentity,
        ProviderRelationIdentity, ProviderRelationOutput, ProviderResourceCeilingSpec,
        ProviderResourceCeilings, ProviderRunBinding, ProviderRunEvidenceSpec, ProviderRunIdentity,
        ProviderRunProvenance, ProviderRunResult, ProviderRunSupport, ProviderSchemaIdentity,
        ProviderScopeIdentity, ProviderSearchScope, ProviderSourceBinding, ProviderTerminalStatus,
        ProviderTrustOutcome, ProviderTrustPosture, SourceIdentity, SuiteIdentity,
        admit_provider_result,
    };

    fn captured(path: &[u8], file: u8) -> ProviderInventoryMember {
        ProviderInventoryMember {
            relative_path: path.to_vec(),
            disposition: ProviderInputDisposition::Captured {
                file_id: [file; 16],
                digest: [17; 32],
                byte_length: 12,
            },
            selected_for_provider: true,
        }
    }

    fn inventory() -> ProviderSourceInventory {
        let old = ProviderSourceInventory::try_new(
            [6; 16],
            6,
            [25; 32],
            &[b"a.py".to_vec(), b"b.py".to_vec()],
            vec![captured(b"a.py", 7), captured(b"b.py", 8)],
            vec![],
            None,
        )
        .unwrap();
        let members = vec![
            captured(b"a.py", 7),
            ProviderInventoryMember {
                relative_path: b"b.py".to_vec(),
                disposition: ProviderInputDisposition::ExcludedPolicy,
                selected_for_provider: false,
            },
            ProviderInventoryMember {
                relative_path: b"odd\xff.dat".to_vec(),
                disposition: ProviderInputDisposition::Binary,
                selected_for_provider: false,
            },
        ];
        ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [26; 32],
            &members
                .iter()
                .map(|member| member.relative_path.clone())
                .collect::<Vec<_>>(),
            members.clone(),
            vec![b"a.py".to_vec()],
            Some(&old),
        )
        .unwrap()
    }

    fn scope() -> ProviderSearchScope {
        ProviderSearchScope {
            namespace: b"python-imports".to_vec(),
            ordered_roots: vec![b"src".to_vec(), b"stubs".to_vec()],
            policy_identity: [21; 32],
            effective_context: [3; 32],
        }
    }

    fn missing_import() -> ProviderSupportDependency {
        ProviderSupportDependency {
            key: ProviderLookupKey::Import {
                qualified_name: b"optional_module".to_vec(),
            },
            scope: scope(),
            outcome: ProviderLookupOutcome::Absent {
                closed_universe: [22; 32],
            },
        }
    }

    fn source(inventory: &ProviderSourceInventory) -> ProviderSourceBinding {
        ProviderSourceBinding::from_inventory_fixture(
            SourceIdentity::try_new("inventory-7").unwrap(),
            inventory.clone(),
        )
    }

    fn admitted(
        source: ProviderSourceBinding,
        run: u8,
        obligations: Vec<ProviderSupportDependency>,
        local: &[ProviderSupportDependency],
    ) -> AdmittedProviderResult {
        let workspace_id = source.workspace_id();
        let requested_units = match source.selection() {
            ProviderSourceSelection::File { .. } => 1,
            ProviderSourceSelection::Inventory(inventory) => {
                cardinality(inventory.selected_files().count()).unwrap()
            }
        };
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let request = ProviderFamilyRequest::try_new(
            ProviderFamilyIdentity::try_new("syntax.calls").unwrap(),
            ProviderRelationIdentity::try_new("raw.calls").unwrap(),
            ProviderSchemaIdentity::try_new("raw.calls.v1").unwrap(),
            Arc::clone(&schema),
            ProviderScopeIdentity::try_new("workspace").unwrap(),
            requested_units,
        )
        .unwrap();
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3.0").unwrap(),
            provider: ProviderIdentity::try_new("tree-sitter").unwrap(),
            protocol: ProviderProtocolIdentity::try_new("in-process-arrow@1").unwrap(),
            source,
            context: crate::provider_contracts::fixture_provider_context(
                workspace_id,
                ProviderContextBinding::try_new(
                    ContextIdentity::try_new("python-context").unwrap(),
                    [3; 16],
                    [3; 32],
                    [4; 32],
                )
                .unwrap()
                .with_support_obligations(obligations)
                .unwrap(),
            ),
            run: ProviderRunBinding::try_new(
                ProviderRunIdentity::try_new(format!("run-{run}")).unwrap(),
                [run; 16],
            )
            .unwrap(),
            lane: ProviderLane::TreeSitter,
            trust: ProviderTrustPosture::InProcessConstrained,
            requests: vec![request],
            ceilings: ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
                max_relations: 2,
                max_batches_per_relation: 2,
                max_input_bytes: 65_536,
                max_rows: 100,
                max_bytes: 65_536,
                max_diagnostics: 4,
                max_work_units: 10_000,
                max_wall_millis: 30_000,
                max_visited_nodes: 10_000,
                max_traversal_depth: 128,
                max_workers: 1,
                max_retained_revisions: 2,
                cancellation_poll_work_units: 128,
                cancellation_ack_millis: 2_000,
            })
            .unwrap(),
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation: CancellationProbe::pair(128).unwrap().1,
            resource_budget: crate::provider_contracts::fixture_provider_budget(
                workspace_id,
                [run; 16],
            ),
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new("tree-sitter-build").unwrap(),
                ProviderPolicyIdentity::try_new("policy.v1").unwrap(),
                ProviderProgramIdentity::try_new("provider-program.v2.3").unwrap(),
            ),
        })
        .unwrap();
        let request = &job.requests()[0];
        let mut support = ProviderRunSupport::from_job_inputs(&job, true);
        for partition in &mut support.partitions {
            partition.dependencies = local.to_vec();
        }
        let result = ProviderRunResult::try_from_job(
            &job,
            ProviderRunEvidenceSpec {
                relations: vec![
                    ProviderRelationOutput::try_new(
                        request.relation().clone(),
                        request.schema_identity().clone(),
                        Arc::clone(&schema),
                        vec![RecordBatch::new_empty(schema)],
                        job.resource_budget(),
                    )
                    .unwrap(),
                ],
                coverage: vec![ProviderCoverage::new(
                    request.family().clone(),
                    ProviderCoverageState::Complete {
                        completed_units: requested_units,
                    },
                )],
                gaps: vec![],
                diagnostics: vec![],
                trust: ProviderTrustOutcome::Trusted,
                terminal: ProviderTerminalStatus::Complete,
                support,
            },
        )
        .unwrap();
        admit_provider_result(job, result).unwrap()
    }

    fn builder() -> ProgrammaticFabricEpochBuilder {
        ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([42; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap()
    }

    fn context(
        inventory: &ProviderSourceInventory,
        runs: &[AdmittedProviderResult],
    ) -> SessionContext {
        let mut builder = builder();
        install_input_observations(&mut builder, inventory, runs).unwrap();
        builder.into_assembly_parts().3.candidate_context()
    }

    async fn query(context: &SessionContext, sql: &str) -> RecordBatch {
        let frame = context.sql(sql).await.unwrap();
        let schema = Arc::clone(frame.schema().inner());
        arrow::compute::concat_batches(&schema, &frame.collect().await.unwrap()).unwrap()
    }

    async fn count(context: &SessionContext, sql: &str) -> i64 {
        let batch = query(context, sql).await;
        batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .value(0)
    }

    #[tokio::test]
    async fn rt_cpg_wp78_input_observation_joins_zero_replacement_and_exclusion() {
        let inventory = inventory();
        let local = ProviderSupportDependency {
            key: ProviderLookupKey::Name {
                name: b"ambiguous_name".to_vec(),
            },
            scope: scope(),
            outcome: ProviderLookupOutcome::Consumed {
                revision: [40; 32],
                candidates: vec![[41; 16], [42; 16]],
            },
        };
        let runs = [19, 20].map(|run| {
            admitted(
                source(&inventory),
                run,
                vec![missing_import()],
                std::slice::from_ref(&local),
            )
        });
        assert!(runs.iter().all(|run| run.result().resources().rows == 0));
        let context = context(&inventory, &runs);
        let joined = query(&context, "SELECT p.producer_release, d.qualified_name, r.relative_root, i.relative_path, h.member_count
            FROM system.provider_partition_support p
            JOIN system.provider_dependency_support d ON p.common_dependency_set_id = d.dependency_set_id
            JOIN system.provider_dependency_search_root r ON d.dependency_set_id = r.dependency_set_id AND d.dependency_ordinal = r.dependency_ordinal
            JOIN source.input_inventory i ON p.owner_id = i.file_id
            JOIN source.input_inventory_set h ON h.input_set_id = i.input_set_id
            WHERE p.owner_kind = 'file' AND p.action = 'replace' AND p.support_complete
              AND d.key_kind = 'import' AND d.outcome_kind = 'absent' AND d.closed_universe IS NOT NULL
              AND d.consumed_revision IS NULL AND r.root_ordinal = 1").await;
        assert_eq!(
            joined.num_rows(),
            2,
            "both empty owner replacements retain the failed lookup"
        );
        assert_eq!(
            joined
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            runs[0].result().support().partitions[0]
                .partition
                .key
                .producer_release
        );
        for (column, expected) in [
            (1, b"optional_module".as_slice()),
            (2, b"stubs".as_slice()),
            (3, b"a.py".as_slice()),
        ] {
            assert_eq!(
                joined
                    .column(column)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .unwrap()
                    .value(0),
                expected
            );
        }
        assert_eq!(
            joined
                .column(4)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(0),
            3
        );
        assert_eq!(
            count(
                &context,
                "SELECT count(*) FROM system.provider_dependency_support WHERE key_kind = 'import'"
            )
            .await,
            1,
            "common dependencies are not expanded per owner or per identical run"
        );
        assert_eq!(
            count(
                &context,
                "SELECT count(*) FROM system.provider_dependency_set"
            )
            .await,
            2,
            "one common and one identical local bundle"
        );
        assert_exclusion_and_candidates(&context).await;
    }

    async fn assert_exclusion_and_candidates(context: &SessionContext) {
        assert_eq!(count(context, "SELECT count(*) FROM system.provider_partition_support p
            JOIN source.input_inventory_withdrawal w ON p.owner_id = w.file_id
            JOIN source.input_inventory i ON w.relative_path = i.relative_path AND w.input_set_id = i.input_set_id
            WHERE p.action = 'withdraw' AND i.disposition = 'excluded_policy' AND i.file_id IS NULL AND NOT i.selected_for_provider").await, 2);
        let candidate = query(context, "SELECT c.candidate_id FROM system.provider_partition_support p
            JOIN system.provider_dependency_support d ON p.local_dependency_set_id = d.dependency_set_id
            JOIN system.provider_dependency_candidate c ON d.dependency_set_id = c.dependency_set_id AND d.dependency_ordinal = c.dependency_ordinal
            WHERE p.owner_kind = 'file' AND p.action = 'replace' AND d.key_kind = 'name' AND c.candidate_ordinal = 1").await;
        assert_eq!(candidate.num_rows(), 2);
        assert_eq!(
            candidate
                .column(0)
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap()
                .value(0),
            &[42; 16]
        );
        let path = query(
            context,
            "SELECT relative_path FROM source.input_inventory WHERE disposition = 'binary'",
        )
        .await;
        assert_eq!(
            path.column(0)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
                .value(0),
            b"odd\xff.dat"
        );
    }

    #[tokio::test]
    async fn rt_cpg_wp78_input_observation_empty_inventory_is_schema_bearing() {
        let inventory =
            ProviderSourceInventory::try_new([6; 16], 0, [25; 32], &[], vec![], vec![], None)
                .unwrap();
        let context = context(&inventory, &[]);
        let header = query(&context, "SELECT member_count, selected_count, changed_count, withdrawn_count, input_set_id FROM source.input_inventory_set").await;
        assert_eq!(header.num_rows(), 1);
        for index in 0..4 {
            assert_eq!(
                header
                    .column(index)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(0),
                0
            );
        }
        assert_eq!(
            header
                .column(4)
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap()
                .value(0),
            inventory.identity()
        );
        for (role, table) in [
            ("source", "input_inventory"),
            ("source", "input_inventory_withdrawal"),
            ("system", "provider_dependency_set"),
            ("system", "provider_dependency_support"),
            ("system", "provider_dependency_search_root"),
            ("system", "provider_dependency_candidate"),
            ("system", "provider_partition_support"),
        ] {
            let frame = context
                .table(TableReference::full(FABRIC_CATALOG, role, table))
                .await
                .unwrap();
            for field in frame.schema().fields() {
                assert_eq!(
                    field.metadata()[FIELD_ID_METADATA_KEY],
                    format!("{role}.{table}.{}", field.name())
                );
            }
            assert_eq!(
                frame
                    .collect()
                    .await
                    .unwrap()
                    .iter()
                    .map(RecordBatch::num_rows)
                    .sum::<usize>(),
                0
            );
        }
    }

    #[tokio::test]
    async fn provider_selections_have_joinable_inventory_and_family_progress() {
        let mut rust = captured(b"b.rs", 8);
        rust.selected_for_provider = false;
        let base = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [26; 32],
            &[b"a.py".to_vec(), b"b.rs".to_vec()],
            vec![captured(b"a.py", 7), rust],
            vec![],
            None,
        )
        .unwrap();
        let selection = base.select_files(&BTreeSet::from([[8; 16]])).unwrap();
        let run = admitted(source(&selection), 19, vec![], &[]);
        let context = context(&base, &[run]);
        assert_eq!(
            count(&context, "SELECT count(*) FROM source.input_inventory_set").await,
            2
        );
        assert_eq!(
            count(&context, "SELECT count(*) FROM source.input_inventory").await,
            4
        );
        assert_eq!(count(&context, "SELECT count(*) FROM system.provider_run_scope r JOIN source.input_inventory i ON r.input_set_id = i.input_set_id WHERE i.selected_for_provider AND i.relative_path = arrow_cast('b.rs', 'Binary')").await, 1);
        assert_eq!(count(&context, "SELECT count(*) FROM system.provider_family_progress p JOIN system.provider_run_scope r USING (provider_run_id) WHERE p.completed_units = 1 AND p.remaining_units = 0 AND p.processing_state = 'complete'").await, 1);
    }

    #[tokio::test]
    async fn rt_cpg_wp78_input_observation_empty_provider_replacement_is_retained() {
        let inventory =
            ProviderSourceInventory::try_new([6; 16], 0, [25; 32], &[], vec![], vec![], None)
                .unwrap();
        let run = admitted(source(&inventory), 19, vec![missing_import()], &[]);
        assert_eq!(run.result().resources().rows, 0);
        let context = context(&inventory, &[run]);
        assert_eq!(count(&context, "SELECT count(*) FROM system.provider_partition_support WHERE owner_kind = 'context' AND action = 'replace'").await, 1);
    }

    #[tokio::test]
    async fn rt_cpg_wp78_input_observation_all_lookup_keys_and_outcomes_are_typed() {
        let inventory = inventory();
        let keys = vec![
            ProviderLookupKey::Configuration {
                relative_path: b"pyproject.toml".to_vec(),
            },
            ProviderLookupKey::BuildInput {
                relative_path: b"Cargo.lock".to_vec(),
            },
            ProviderLookupKey::PositiveFact {
                relation: "fact.symbol".to_owned(),
                entity_id: [43; 16],
            },
            ProviderLookupKey::Export {
                qualified_name: b"package.item".to_vec(),
            },
            ProviderLookupKey::Name {
                name: b"name".to_vec(),
            },
            ProviderLookupKey::Member {
                receiver_type: [44; 16],
                name: b"member".to_vec(),
            },
            ProviderLookupKey::Implementation {
                trait_id: [45; 16],
                receiver_type: [44; 16],
            },
            ProviderLookupKey::CallableSummary {
                callable_id: [46; 16],
            },
            ProviderLookupKey::GraphProjection {
                projection_id: [47; 32],
            },
        ];
        let mut obligations = vec![missing_import()];
        obligations.extend(keys.into_iter().map(|key| ProviderSupportDependency {
            key,
            scope: scope(),
            outcome: ProviderLookupOutcome::Consumed {
                revision: [48; 32],
                candidates: vec![[49; 16]],
            },
        }));
        obligations[2].outcome = ProviderLookupOutcome::Incomplete {
            observed_universe: [50; 32],
        };
        let run = admitted(source(&inventory), 19, obligations, &[]);
        assert!(run.result().support().requires_context_invalidation());
        let context = context(&inventory, &[run]);
        assert_eq!(
            count(
                &context,
                "SELECT count(DISTINCT key_kind) FROM system.provider_dependency_support"
            )
            .await,
            13
        );
        for predicate in [
            "key_kind = 'configuration' AND relative_path = X'707970726f6a6563742e746f6d6c'",
            "key_kind = 'build_input' AND relative_path = X'436172676f2e6c6f636b' AND outcome_kind = 'incomplete' AND observed_universe IS NOT NULL AND closed_universe IS NULL AND consumed_revision IS NULL",
            "key_kind = 'positive_fact' AND relation = 'fact.symbol' AND entity_id IS NOT NULL",
            "key_kind = 'export' AND qualified_name = X'7061636b6167652e6974656d'",
            "key_kind = 'name' AND name = X'6e616d65'",
            "key_kind = 'member' AND name = X'6d656d626572' AND receiver_type IS NOT NULL",
            "key_kind = 'implementation' AND trait_id IS NOT NULL AND receiver_type IS NOT NULL",
            "key_kind = 'callable_summary' AND callable_id IS NOT NULL",
            "key_kind = 'graph_projection' AND projection_id IS NOT NULL",
            "key_kind = 'source_bytes' AND file_id IS NOT NULL AND consumed_revision IS NOT NULL",
            "key_kind = 'producer_release' AND consumed_revision IS NOT NULL",
            "key_kind = 'effective_context_inputs' AND consumed_revision IS NOT NULL",
        ] {
            assert_eq!(
                count(
                    &context,
                    &format!(
                        "SELECT count(*) FROM system.provider_dependency_support WHERE {predicate}"
                    )
                )
                .await,
                1,
                "{predicate}"
            );
        }
        let pins = query(&context, "SELECT DISTINCT p.context_id, d.effective_context, d.policy_identity FROM system.provider_partition_support p
            JOIN system.provider_dependency_support d ON p.common_dependency_set_id = d.dependency_set_id WHERE d.key_kind = 'configuration'").await;
        assert_eq!(pins.column(0).data_type(), &DataType::FixedSizeBinary(16));
        assert_eq!(pins.column(1).data_type(), &DataType::FixedSizeBinary(32));
        assert_eq!(
            pins.column(2)
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap()
                .value(0),
            &[21; 32]
        );
        assert_lookup_pins(&context).await;
    }

    async fn assert_lookup_pins(context: &SessionContext) {
        for (kind, column, expected) in [
            ("source_bytes", "file_id", [7; 16].as_slice()),
            ("positive_fact", "entity_id", [43; 16].as_slice()),
            ("member", "receiver_type", [44; 16].as_slice()),
            ("implementation", "receiver_type", [44; 16].as_slice()),
            ("implementation", "trait_id", [45; 16].as_slice()),
            ("callable_summary", "callable_id", [46; 16].as_slice()),
            ("graph_projection", "projection_id", [47; 32].as_slice()),
            ("build_input", "observed_universe", [50; 32].as_slice()),
            ("import", "closed_universe", [22; 32].as_slice()),
        ] {
            let batch = query(context, &format!("SELECT {column} FROM system.provider_dependency_support WHERE key_kind = '{kind}'")).await;
            assert_eq!(batch.num_rows(), 1);
            assert_eq!(
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap()
                    .value(0),
                expected
            );
        }
    }

    #[test]
    fn rt_cpg_wp78_input_observation_rejects_stale_or_unselected_source() {
        let inventory = inventory();
        for (workspace, file, generation, digest, accepted) in [
            (6, 7, 7, 17, true),
            (5, 7, 7, 17, false),
            (6, 7, 6, 17, false),
            (6, 7, 7, 18, false),
            (6, 8, 7, 17, false),
            (6, 9, 7, 17, false),
        ] {
            let source = ProviderSourceBinding::try_file(
                SourceIdentity::try_new("file").unwrap(),
                [workspace; 16],
                [file; 16],
                generation,
                [digest; 32],
            )
            .unwrap();
            let run = admitted(source, 19, vec![missing_import()], &[]);
            assert_eq!(
                install_input_observations(&mut builder(), &inventory, &[run]).is_ok(),
                accepted
            );
        }
        let different = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [25; 32],
            &[b"a.py".to_vec()],
            vec![captured(b"a.py", 7)],
            vec![],
            None,
        )
        .unwrap();
        let run = admitted(source(&different), 19, vec![missing_import()], &[]);
        assert!(
            install_input_observations(&mut builder(), &inventory, &[run]).is_err(),
            "context jobs bind the entire inventory, not just its selected file digest"
        );
        let run = admitted(source(&inventory), 19, vec![missing_import()], &[]);
        assert!(
            install_input_observations(&mut builder(), &inventory, &[run.clone(), run]).is_err()
        );
    }
}
