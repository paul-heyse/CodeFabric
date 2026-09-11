//! Exact Delta snapshots for every non-observation relation in a sealed fabric epoch.
//!
//! The five self-observation families retain their append-only histories in
//! [`super::programmatic_observation_delta`]. Every provider, canonical, derived, input, and
//! proof relation retained by the sealed session is materialized here with a native DataFusion
//! plan and `SaveMode::Overwrite`, or reuses an exact selected version when its complete immutable
//! inputs and descriptor match. Recovery opens
//! only the root/version pairs selected by activation and restores their executable
//! [`SchemaContract`] directly from a canonical descriptor stored in Delta metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::sync::Arc;

use arrow_schema::{Schema, SchemaRef};
use datafusion::common::{Constraint, Constraints, TableReference};
use datafusion::logical_expr::{cast, col};
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel;
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableError};
use futures::StreamExt as _;
use futures::stream::FuturesUnordered;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use super::command::{EpochId, OperationId, TransactionRef, WriterGeneration};
use super::delta_exact::{
    ExactDeltaPin, ExactDeltaProviderError, ValidatedDeltaSnapshot,
    provider_read_from_validated_snapshot, session_delta_table_builder,
};
use super::delta_write::{
    ApplicationTransactionMarker, ControlledDeltaWriteMode, ControlledDeltaWriteOutcome,
    ControlledDeltaWriteSpec, SessionBoundLogicalPlan, write_exact_delta_plan,
};
use super::programmatic_schema::{
    IdentityPreservingViewTable, ProgrammaticRelationId, SealedProgrammaticSchemaAssembly,
    SealedRelationBinding,
};
use super::provider::SchemaContractStorageProvider;
use crate::schema_contract::{
    ColumnMappingMode, DeletionVectorBehavior, FieldIndexMapping, SchemaCompatibility,
    SchemaContract, SchemaContractError, SchemaContractOptions, delta_storage_field,
};

const DESCRIPTOR_PREFIX: &str = "codefabric-exact-relation-v1:";
const META_DESCRIPTOR: &str = "codefabric.relation_snapshot.descriptor";
const META_EPOCH: &str = "codefabric.relation_snapshot.epoch";
const META_RELATION: &str = "codefabric.relation_snapshot.relation";
const CDF_PROPERTY: &str = "delta.enableChangeDataFeed";
const STATS_COLUMNS_PROPERTY: &str = "delta.dataSkippingStatsColumns";
const DELETION_VECTORS_PROPERTY: &str = "delta.enableDeletionVectors";
const LOG_RETENTION_PROPERTY: &str = "delta.logRetentionDuration";
const DELETED_FILE_RETENTION_PROPERTY: &str = "delta.deletedFileRetentionDuration";
const CHECKPOINT_INTERVAL_PROPERTY: &str = "delta.checkpointInterval";
const EXPIRED_LOG_CLEANUP_PROPERTY: &str = "delta.enableExpiredLogCleanup";
const HISTORY_LOG_RETENTION: &str = "interval 36500 days";
const HISTORY_DELETED_FILE_RETENTION: &str = "interval 36500 days";
const HISTORY_CHECKPOINT_INTERVAL: &str = "100";
const HISTORY_EXPIRED_LOG_CLEANUP: &str = "false";

/// Stable root under which one workspace owns exact relation histories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticRelationDeltaLayout {
    root: Url,
}

impl ProgrammaticRelationDeltaLayout {
    /// Construct an explicit relation-history root. The URL must be hierarchical.
    pub fn try_new(mut root: Url) -> Result<Self, ProgrammaticRelationDeltaError> {
        if root.cannot_be_a_base() {
            return Err(ProgrammaticRelationDeltaError::InvalidLayoutRoot(root));
        }
        if !root.path().ends_with('/') {
            let path = format!("{}/", root.path());
            root.set_path(&path);
        }
        Ok(Self { root })
    }

    #[must_use]
    pub const fn root(&self) -> &Url {
        &self.root
    }

    fn relation_root(
        &self,
        relation_id: &ProgrammaticRelationId,
    ) -> Result<Url, ProgrammaticRelationDeltaError> {
        // Hex is a reversible path encoding, not an authority digest. It prevents separators,
        // Unicode normalization, or object-store key rules from aliasing two relation identities.
        let encoded = relation_id
            .as_str()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.root
            .join(&format!("relation-{encoded}/"))
            .map_err(|source| ProgrammaticRelationDeltaError::LayoutJoin {
                root: self.root.clone(),
                relation_id: relation_id.clone(),
                source,
            })
    }
}

/// How a candidate acquires exact predecessor tables before its zero-retry writes.
#[derive(Clone, Debug)]
pub enum ProgrammaticRelationDeltaPreparation {
    /// First lawful epoch: provision version-zero tables below the explicit layout.
    Genesis(ProgrammaticRelationDeltaLayout),
    /// Reuse selected versions with equal complete immutable input identities and descriptors.
    /// Changed or ineligible relations get candidate-owned histories; selected tables are not written.
    ReuseUnchanged {
        selected: ProgrammaticRelationDeltaPublication,
        layout: ProgrammaticRelationDeltaLayout,
    },
    /// Later epoch: open selected predecessors exactly and provision only newly released relations.
    Advance {
        selected: ProgrammaticRelationDeltaPublication,
        layout: ProgrammaticRelationDeltaLayout,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct StoredFieldMapping {
    logical: usize,
    storage: usize,
    projection: usize,
    filter: usize,
    statistics: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "indices", rename_all = "snake_case")]
enum StoredConstraint {
    PrimaryKey(Vec<usize>),
    Unique(Vec<usize>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredSchemaCompatibility {
    Exact,
    Contains,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredColumnMappingMode {
    Positional,
    Name,
    FieldId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredDeletionVectorBehavior {
    Forbidden,
    AppliedByProvider,
    ExposedVisibilityColumn,
}

/// Complete executable relation/schema address stored in each Delta table's metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct StoredRelationDescriptor {
    version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    immutable_input_identity: Option<[u8; 32]>,
    relation_id: String,
    catalog: String,
    schema: String,
    table: String,
    source_schema_identity: String,
    logical_schema: Schema,
    storage_schema: Schema,
    mappings: Vec<StoredFieldMapping>,
    constraints: Vec<StoredConstraint>,
    compatibility: StoredSchemaCompatibility,
    column_mapping_mode: StoredColumnMappingMode,
    deletion_vector_behavior: StoredDeletionVectorBehavior,
}

#[derive(Clone)]
struct RelationSnapshotSpec {
    relation_id: ProgrammaticRelationId,
    table_reference: TableReference,
    contract: Arc<SchemaContract>,
    descriptor: StoredRelationDescriptor,
    descriptor_json: Arc<str>,
}

impl fmt::Debug for RelationSnapshotSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelationSnapshotSpec")
            .field("relation_id", &self.relation_id)
            .field("table_reference", &self.table_reference)
            .finish_non_exhaustive()
    }
}

impl RelationSnapshotSpec {
    fn from_binding(
        relation_id: ProgrammaticRelationId,
        binding: &SealedRelationBinding,
    ) -> Result<Self, ProgrammaticRelationDeltaError> {
        let contract = Arc::new(delta_snapshot_contract(
            &binding.table_reference,
            &binding.contract,
        )?);
        let mut descriptor = StoredRelationDescriptor::from_contract(
            &relation_id,
            &binding.table_reference,
            &contract,
        )?;
        descriptor.immutable_input_identity = binding.immutable_input_identity;
        let descriptor_json = canonical_descriptor(&descriptor)?;
        Ok(Self {
            relation_id,
            table_reference: binding.table_reference.clone(),
            contract,
            descriptor,
            descriptor_json: Arc::from(descriptor_json),
        })
    }

    fn from_stored(
        descriptor: StoredRelationDescriptor,
    ) -> Result<Self, ProgrammaticRelationDeltaError> {
        let relation_id = ProgrammaticRelationId::new(descriptor.relation_id.as_str());
        let table_reference = TableReference::full(
            descriptor.catalog.as_str(),
            descriptor.schema.as_str(),
            descriptor.table.as_str(),
        );
        let contract = Arc::new(descriptor.to_contract(table_reference.clone())?);
        let descriptor_json = canonical_descriptor(&descriptor)?;
        Ok(Self {
            relation_id,
            table_reference,
            contract,
            descriptor,
            descriptor_json: Arc::from(descriptor_json),
        })
    }
}

fn delta_snapshot_contract(
    table_reference: &TableReference,
    source: &SchemaContract,
) -> Result<SchemaContract, SchemaContractError> {
    let logical = Arc::clone(source.logical_schema());
    let storage = Arc::new(Schema::new_with_metadata(
        logical
            .fields()
            .iter()
            .map(|field| Arc::new(delta_storage_field(field)))
            .collect::<Vec<_>>(),
        logical.metadata().clone(),
    ));
    let mappings = (0..logical.fields().len())
        .map(|index| FieldIndexMapping::direct(index, index))
        .collect();
    SchemaContract::try_new_with_options(
        source.source_schema_identity(),
        table_reference.clone(),
        logical,
        storage,
        mappings,
        SchemaContractOptions::new(
            source.constraints().as_ref().clone(),
            source.compatibility(),
            source.column_mapping_mode(),
            source.deletion_vector_behavior(),
        ),
    )
}

impl StoredRelationDescriptor {
    fn from_contract(
        relation_id: &ProgrammaticRelationId,
        table_reference: &TableReference,
        contract: &SchemaContract,
    ) -> Result<Self, ProgrammaticRelationDeltaError> {
        let catalog = table_reference.catalog().ok_or_else(|| {
            ProgrammaticRelationDeltaError::UnqualifiedTableReference(table_reference.clone())
        })?;
        let schema = table_reference.schema().ok_or_else(|| {
            ProgrammaticRelationDeltaError::UnqualifiedTableReference(table_reference.clone())
        })?;
        let constraints = contract
            .constraints()
            .iter()
            .map(|constraint| match constraint {
                Constraint::PrimaryKey(indices) => StoredConstraint::PrimaryKey(indices.clone()),
                Constraint::Unique(indices) => StoredConstraint::Unique(indices.clone()),
            })
            .collect();
        Ok(Self {
            version: 1,
            immutable_input_identity: None,
            relation_id: relation_id.as_str().to_owned(),
            catalog: catalog.to_owned(),
            schema: schema.to_owned(),
            table: table_reference.table().to_owned(),
            source_schema_identity: contract.source_schema_identity().to_owned(),
            logical_schema: contract.logical_schema().as_ref().clone(),
            storage_schema: contract.storage_schema().as_ref().clone(),
            mappings: contract
                .mappings()
                .iter()
                .map(|mapping| StoredFieldMapping {
                    logical: mapping.logical_index(),
                    storage: mapping.storage_index(),
                    projection: mapping.projection_index(),
                    filter: mapping.filter_index(),
                    statistics: mapping.statistics_index(),
                })
                .collect(),
            constraints,
            compatibility: match contract.compatibility() {
                SchemaCompatibility::Exact => StoredSchemaCompatibility::Exact,
                SchemaCompatibility::Contains => StoredSchemaCompatibility::Contains,
            },
            column_mapping_mode: match contract.column_mapping_mode() {
                ColumnMappingMode::Positional => StoredColumnMappingMode::Positional,
                ColumnMappingMode::Name => StoredColumnMappingMode::Name,
                ColumnMappingMode::FieldId => StoredColumnMappingMode::FieldId,
            },
            deletion_vector_behavior: match contract.deletion_vector_behavior() {
                DeletionVectorBehavior::Forbidden => StoredDeletionVectorBehavior::Forbidden,
                DeletionVectorBehavior::AppliedByProvider => {
                    StoredDeletionVectorBehavior::AppliedByProvider
                }
                DeletionVectorBehavior::ExposedVisibilityColumn => {
                    StoredDeletionVectorBehavior::ExposedVisibilityColumn
                }
            },
        })
    }

    fn to_contract(
        &self,
        table_reference: TableReference,
    ) -> Result<SchemaContract, ProgrammaticRelationDeltaError> {
        if self.version != 1 || self.relation_id.trim().is_empty() {
            return Err(ProgrammaticRelationDeltaError::InvalidDescriptor(
                "unsupported descriptor version or empty relation identity".to_owned(),
            ));
        }
        let constraints = Constraints::new_unverified(
            self.constraints
                .iter()
                .map(|constraint| match constraint {
                    StoredConstraint::PrimaryKey(indices) => {
                        Constraint::PrimaryKey(indices.clone())
                    }
                    StoredConstraint::Unique(indices) => Constraint::Unique(indices.clone()),
                })
                .collect(),
        );
        let options = SchemaContractOptions::new(
            constraints,
            match self.compatibility {
                StoredSchemaCompatibility::Exact => SchemaCompatibility::Exact,
                StoredSchemaCompatibility::Contains => SchemaCompatibility::Contains,
            },
            match self.column_mapping_mode {
                StoredColumnMappingMode::Positional => ColumnMappingMode::Positional,
                StoredColumnMappingMode::Name => ColumnMappingMode::Name,
                StoredColumnMappingMode::FieldId => ColumnMappingMode::FieldId,
            },
            match self.deletion_vector_behavior {
                StoredDeletionVectorBehavior::Forbidden => DeletionVectorBehavior::Forbidden,
                StoredDeletionVectorBehavior::AppliedByProvider => {
                    DeletionVectorBehavior::AppliedByProvider
                }
                StoredDeletionVectorBehavior::ExposedVisibilityColumn => {
                    DeletionVectorBehavior::ExposedVisibilityColumn
                }
            },
        );
        Ok(SchemaContract::try_new_with_options(
            self.source_schema_identity.as_str(),
            table_reference,
            Arc::new(self.logical_schema.clone()),
            Arc::new(self.storage_schema.clone()),
            self.mappings
                .iter()
                .map(|mapping| {
                    FieldIndexMapping::new(
                        mapping.logical,
                        mapping.storage,
                        mapping.projection,
                        mapping.filter,
                        mapping.statistics,
                    )
                })
                .collect(),
            options,
        )?)
    }
}

fn canonical_descriptor(
    descriptor: &StoredRelationDescriptor,
) -> Result<String, ProgrammaticRelationDeltaError> {
    let bytes = serde_json_canonicalizer::to_vec(descriptor)?;
    String::from_utf8(bytes).map_err(|source| {
        ProgrammaticRelationDeltaError::InvalidDescriptor(format!(
            "canonical descriptor is not UTF-8: {source}"
        ))
    })
}

struct ProgrammaticRelationDeltaTarget {
    spec: RelationSnapshotSpec,
    predecessor: ExactDeltaPin,
    table: DeltaTable,
}

/// Exact predecessor set for one dependency-closed relation snapshot write.
pub struct ProgrammaticRelationDeltaTargets {
    targets: BTreeMap<ProgrammaticRelationId, ProgrammaticRelationDeltaTarget>,
}

/// Reversible root/version vector and executable descriptors for all snapshotted relations.
#[derive(Clone)]
pub struct ProgrammaticRelationDeltaPublication {
    epoch_id: EpochId,
    table_versions: Arc<BTreeMap<ProgrammaticRelationId, ExactDeltaPin>>,
    descriptors: Arc<BTreeMap<ProgrammaticRelationId, StoredRelationDescriptor>>,
}

impl fmt::Debug for ProgrammaticRelationDeltaPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticRelationDeltaPublication")
            .field("epoch_id", &self.epoch_id)
            .field("table_versions", &self.table_versions)
            .field("relation_count", &self.descriptors.len())
            .finish()
    }
}

impl ProgrammaticRelationDeltaPublication {
    fn try_new(
        epoch_id: EpochId,
        pins: BTreeMap<ProgrammaticRelationId, ExactDeltaPin>,
        descriptors: BTreeMap<ProgrammaticRelationId, StoredRelationDescriptor>,
    ) -> Result<Self, ProgrammaticRelationDeltaError> {
        if pins.keys().collect::<BTreeSet<_>>() != descriptors.keys().collect::<BTreeSet<_>>() {
            return Err(ProgrammaticRelationDeltaError::RelationSetMismatch);
        }
        Ok(Self {
            epoch_id,
            table_versions: Arc::new(pins),
            descriptors: Arc::new(descriptors),
        })
    }

    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn table_version_map(&self) -> &Arc<BTreeMap<ProgrammaticRelationId, ExactDeltaPin>> {
        &self.table_versions
    }

    #[must_use]
    pub fn table_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &ExactDeltaPin)> + DoubleEndedIterator {
        self.table_versions
            .iter()
            .map(|(relation_id, pin)| (relation_id.as_str(), pin))
    }

    pub async fn open_targets(
        &self,
        session: &datafusion::execution::SessionState,
    ) -> Result<ProgrammaticRelationDeltaTargets, ProgrammaticRelationDeltaError> {
        let mut targets = BTreeMap::new();
        for (relation_id, descriptor) in self.descriptors.iter() {
            let pin = self
                .table_versions
                .get(relation_id)
                .ok_or(ProgrammaticRelationDeltaError::RelationSetMismatch)?;
            let table = load_exact(pin, session).await?;
            let spec = RelationSnapshotSpec::from_stored(descriptor.clone())?;
            validate_loaded_descriptor(&table, &spec)?;
            targets.insert(
                relation_id.clone(),
                ProgrammaticRelationDeltaTarget {
                    spec,
                    predecessor: pin.clone(),
                    table,
                },
            );
        }
        Ok(ProgrammaticRelationDeltaTargets { targets })
    }
}

/// Materialize every non-observation relation from one sealed session into exact Delta versions.
pub async fn persist_programmatic_relation_snapshots(
    sealed: &SealedProgrammaticSchemaAssembly,
    epoch_id: EpochId,
    operation_id: OperationId,
    writer_generation: WriterGeneration,
    transaction: TransactionRef,
    preparation: ProgrammaticRelationDeltaPreparation,
) -> Result<ProgrammaticRelationDeltaPublication, ProgrammaticRelationDeltaError> {
    let specs = sealed
        .relations()
        .filter(|(relation_id, _)| is_semantic_snapshot_relation(relation_id))
        .map(|(relation_id, binding)| {
            RelationSnapshotSpec::from_binding(relation_id.clone(), binding)
                .map(|spec| (relation_id.clone(), spec))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let context = sealed.session().clone();
    let session = Arc::new(context.state());
    let (reuse, mut targets) = match preparation {
        ProgrammaticRelationDeltaPreparation::ReuseUnchanged { selected, layout } => (
            Some((selected, layout)),
            ProgrammaticRelationDeltaTargets {
                targets: BTreeMap::new(),
            },
        ),
        other => (None, prepare_targets(&specs, other, &session).await?),
    };
    let writer = RelationSnapshotWriter {
        context: &context,
        session: &session,
        reuse: reuse.as_ref(),
        epoch_id,
        operation_id,
        writer_generation,
        transaction,
    };
    // Four independent table pipelines fit the production native lane's 16 blocking roots.
    // They share the selected session and DataFusion memory pool; no runtime or writer owner
    // is multiplied. The parent native operation still joins the complete library runtime.
    let work = specs.into_values().map(|spec| {
        let prepared = targets.targets.remove(&spec.relation_id);
        writer.persist(spec, prepared)
    });
    let completed = collect_bounded_writes(work, 4).await?;
    if !targets.targets.is_empty() {
        return Err(ProgrammaticRelationDeltaError::RelationSetMismatch);
    }
    let mut pins = BTreeMap::new();
    let mut descriptors = BTreeMap::new();
    for (relation, pin, descriptor) in completed {
        pins.insert(relation.clone(), pin);
        descriptors.insert(relation, descriptor);
    }
    ProgrammaticRelationDeltaPublication::try_new(epoch_id, pins, descriptors)
}

// Futures remain children of this operation, without detached Tokio tasks. After the first
// failure stop admitting work and observe every already-started write before returning it.
async fn collect_bounded_writes<F, T, E>(
    mut work: impl Iterator<Item = F>,
    concurrency: usize,
) -> Result<Vec<T>, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    assert!(concurrency > 0, "write concurrency must admit work");
    let mut pending = FuturesUnordered::new();
    let mut results = Vec::new();
    let mut failure = None;
    loop {
        while failure.is_none() && pending.len() < concurrency {
            let Some(next) = work.next() else {
                break;
            };
            pending.push(Box::pin(next));
        }
        let Some(result) = pending.next().await else {
            break;
        };
        match result {
            Ok(result) => results.push(result),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    failure.map_or(Ok(results), Err)
}

struct RelationSnapshotWriter<'a> {
    context: &'a datafusion::prelude::SessionContext,
    session: &'a Arc<datafusion::execution::SessionState>,
    reuse: Option<&'a (
        ProgrammaticRelationDeltaPublication,
        ProgrammaticRelationDeltaLayout,
    )>,
    epoch_id: EpochId,
    operation_id: OperationId,
    writer_generation: WriterGeneration,
    transaction: TransactionRef,
}

impl RelationSnapshotWriter<'_> {
    #[allow(clippy::result_large_err)] // Preserve the existing public exact-snapshot error type.
    async fn persist(
        &self,
        spec: RelationSnapshotSpec,
        prepared: Option<ProgrammaticRelationDeltaTarget>,
    ) -> Result<
        (
            ProgrammaticRelationId,
            ExactDeltaPin,
            StoredRelationDescriptor,
        ),
        ProgrammaticRelationDeltaError,
    > {
        let relation_id = spec.relation_id.clone();
        let target = if let Some((selected, layout)) = self.reuse {
            if spec.descriptor.immutable_input_identity.is_some()
                && selected.descriptors.get(&relation_id) == Some(&spec.descriptor)
            {
                let pin = selected
                    .table_versions
                    .get(&relation_id)
                    .ok_or(ProgrammaticRelationDeltaError::RelationSetMismatch)?;
                // Validate the exact selected snapshot before retaining its pin. No latest lookup,
                // result comparison, or write occurs. Missing/corrupt history fails closed.
                let table = load_exact(pin, self.session).await?;
                validate_loaded_descriptor(&table, &spec)?;
                return Ok((relation_id, pin.clone(), spec.descriptor));
            }
            provision_target(spec.clone(), layout, self.session).await?
        } else {
            prepared.ok_or(ProgrammaticRelationDeltaError::RelationSetMismatch)?
        };
        if target.spec.descriptor != spec.descriptor {
            return Err(ProgrammaticRelationDeltaError::DescriptorDrift(relation_id));
        }
        let dataframe = storage_projection(
            self.context.table(spec.table_reference.clone()).await?,
            &spec,
        )?;
        let delta_schema = target.table.snapshot()?.snapshot().arrow_schema();
        validate_storage_plan_schema(&relation_id, delta_schema.as_ref(), &spec.contract)?;
        let provider = Arc::new(IdentityPreservingViewTable::with_schema(
            dataframe.into_unoptimized_plan(),
            Arc::clone(&delta_schema),
        )?);
        let dataframe = self.context.read_table(provider)?;
        let plan =
            SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(self.session), dataframe)?;
        let commit_metadata = BTreeMap::from([
            (
                META_DESCRIPTOR.to_owned(),
                Value::String(spec.descriptor_json.to_string()),
            ),
            (
                META_EPOCH.to_owned(),
                Value::String(hex(self.epoch_id.as_bytes())),
            ),
            (
                META_RELATION.to_owned(),
                Value::String(relation_id.as_str().to_owned()),
            ),
        ]);
        let write = ControlledDeltaWriteSpec::new(
            target.predecessor,
            self.operation_id,
            self.writer_generation,
            ApplicationTransactionMarker::from_transaction_ref(self.transaction),
            ControlledDeltaWriteMode::ReplaceAll,
        )
        .with_commit_metadata(commit_metadata)?;
        let committed = match write_exact_delta_plan(&target.table, &write, plan).await {
            ControlledDeltaWriteOutcome::Committed(committed) => committed,
            outcome => {
                return Err(ProgrammaticRelationDeltaError::WriteOutcome {
                    relation_id,
                    detail: format!("{outcome:?}"),
                });
            }
        };
        Ok((relation_id, committed.committed().clone(), spec.descriptor))
    }
}

#[allow(clippy::result_large_err)] // Preserve the existing public exact-snapshot error type.
fn storage_projection(
    dataframe: datafusion::dataframe::DataFrame,
    spec: &RelationSnapshotSpec,
) -> Result<datafusion::dataframe::DataFrame, ProgrammaticRelationDeltaError> {
    let mut expressions = Vec::with_capacity(spec.contract.storage_schema().fields().len());
    for (storage_index, storage_field) in spec.contract.storage_schema().fields().iter().enumerate()
    {
        let logical_index = spec
            .contract
            .logical_index_for_storage(storage_index)?
            .ok_or_else(|| ProgrammaticRelationDeltaError::UnmappedStorageField {
                relation_id: spec.relation_id.clone(),
                storage_index,
            })?;
        let logical_field = spec.contract.logical_schema().field(logical_index);
        let expression = if logical_field.data_type() == storage_field.data_type() {
            col(logical_field.name())
        } else {
            cast(col(logical_field.name()), storage_field.data_type().clone())
        };
        expressions.push(expression.alias(storage_field.name()));
    }
    let dataframe = dataframe.select(expressions)?;
    validate_storage_plan_schema(
        &spec.relation_id,
        dataframe.schema().as_arrow(),
        &spec.contract,
    )?;
    Ok(dataframe)
}

/// Reopen exact selected relation snapshots as application-contract providers in a fresh session.
pub async fn reopen_programmatic_relation_snapshots(
    session: Arc<datafusion::execution::SessionState>,
    epoch_id: EpochId,
    table_versions: BTreeMap<ProgrammaticRelationId, ExactDeltaPin>,
) -> Result<
    (
        ProgrammaticRelationDeltaPublication,
        Vec<super::programmatic_schema::ProviderInput>,
    ),
    ProgrammaticRelationDeltaError,
> {
    let mut pins = BTreeMap::new();
    let mut descriptors = BTreeMap::new();
    let mut providers = Vec::with_capacity(table_versions.len());
    for (relation_id, pin) in table_versions {
        let table = load_exact(&pin, &session).await?;
        let descriptor = read_descriptor(&table)?;
        if descriptor.relation_id != relation_id.as_str() {
            return Err(ProgrammaticRelationDeltaError::DescriptorRelationMismatch {
                selected: relation_id.clone(),
                descriptor: descriptor.relation_id,
            });
        }
        let spec = RelationSnapshotSpec::from_stored(descriptor.clone())?;
        validate_loaded_descriptor(&table, &spec)?;
        let snapshot = ValidatedDeltaSnapshot::try_from_loaded_table(table, &pin)?;
        let read =
            provider_read_from_validated_snapshot(&pin, snapshot, Arc::clone(&session)).await?;
        let provider = Arc::new(SchemaContractStorageProvider::try_new(
            Arc::clone(&spec.contract),
            read.into_provider(),
        )?) as Arc<dyn datafusion::catalog::TableProvider>;
        let mut input = super::programmatic_schema::ProviderInput::new(
            spec.relation_id.clone(),
            spec.table_reference,
            Arc::clone(&spec.contract),
            provider,
        );
        if let Some(identity) = descriptor.immutable_input_identity {
            input = input.with_immutable_input_identity(identity);
        }
        providers.push(input);
        pins.insert(spec.relation_id.clone(), pin);
        descriptors.insert(spec.relation_id, descriptor);
    }
    let publication = ProgrammaticRelationDeltaPublication::try_new(epoch_id, pins, descriptors)?;
    Ok((publication, providers))
}

async fn prepare_targets(
    specs: &BTreeMap<ProgrammaticRelationId, RelationSnapshotSpec>,
    preparation: ProgrammaticRelationDeltaPreparation,
    session: &datafusion::execution::SessionState,
) -> Result<ProgrammaticRelationDeltaTargets, ProgrammaticRelationDeltaError> {
    match preparation {
        ProgrammaticRelationDeltaPreparation::ReuseUnchanged { .. } => {
            unreachable!("reuse provisions only changed relations after identity selection")
        }
        ProgrammaticRelationDeltaPreparation::Genesis(layout) => {
            provision_targets(specs, &layout, session).await
        }
        ProgrammaticRelationDeltaPreparation::Advance { selected, layout } => {
            let mut opened = selected.open_targets(session).await?.targets;
            let mut targets = BTreeMap::new();
            for (relation_id, spec) in specs {
                if let Some(target) = opened.remove(relation_id) {
                    if target.spec.descriptor != spec.descriptor {
                        return Err(ProgrammaticRelationDeltaError::DescriptorDrift(
                            relation_id.clone(),
                        ));
                    }
                    targets.insert(relation_id.clone(), target);
                } else {
                    let target = provision_target(spec.clone(), &layout, session).await?;
                    targets.insert(relation_id.clone(), target);
                }
            }
            // Removed release relations remain retained in their old exact versions but are not
            // selected by the successor vector.
            Ok(ProgrammaticRelationDeltaTargets { targets })
        }
    }
}

async fn provision_targets(
    specs: &BTreeMap<ProgrammaticRelationId, RelationSnapshotSpec>,
    layout: &ProgrammaticRelationDeltaLayout,
    session: &datafusion::execution::SessionState,
) -> Result<ProgrammaticRelationDeltaTargets, ProgrammaticRelationDeltaError> {
    let mut targets = BTreeMap::new();
    for (relation_id, spec) in specs {
        targets.insert(
            relation_id.clone(),
            provision_target(spec.clone(), layout, session).await?,
        );
    }
    Ok(ProgrammaticRelationDeltaTargets { targets })
}

async fn provision_target(
    spec: RelationSnapshotSpec,
    layout: &ProgrammaticRelationDeltaLayout,
    session: &datafusion::execution::SessionState,
) -> Result<ProgrammaticRelationDeltaTarget, ProgrammaticRelationDeltaError> {
    let root = layout.relation_root(&spec.relation_id)?;
    if root.scheme() == "file" {
        let path = root
            .to_file_path()
            .map_err(|()| ProgrammaticRelationDeltaError::InvalidLayoutRoot(root.clone()))?;
        fs::create_dir_all(path)?;
    }
    let kernel: deltalake::kernel::StructType = spec
        .contract
        .storage_schema()
        .as_ref()
        .try_into_kernel()
        .map_err(|source| DeltaTableError::Arrow { source })?;
    let stats_columns = spec
        .contract
        .storage_schema()
        .fields()
        .iter()
        .map(|field| field.name().as_str())
        .collect::<Vec<_>>()
        .join(",");
    if stats_columns.is_empty() {
        return Err(ProgrammaticRelationDeltaError::EmptyStorageSchema(
            spec.relation_id.clone(),
        ));
    }
    CreateBuilder::new()
        .with_location(root.to_string())
        .with_log_store(session_delta_table_builder(root.clone(), session)?.build_storage()?)
        .with_table_name(spec.table_reference.table())
        .with_comment(format!("{DESCRIPTOR_PREFIX}{}", spec.descriptor_json))
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(kernel.fields().cloned())
        .with_configuration([
            (CDF_PROPERTY, Some("true")),
            (STATS_COLUMNS_PROPERTY, Some(stats_columns.as_str())),
            (DELETION_VECTORS_PROPERTY, Some("false")),
            (LOG_RETENTION_PROPERTY, Some(HISTORY_LOG_RETENTION)),
            (
                DELETED_FILE_RETENTION_PROPERTY,
                Some(HISTORY_DELETED_FILE_RETENTION),
            ),
            (
                CHECKPOINT_INTERVAL_PROPERTY,
                Some(HISTORY_CHECKPOINT_INTERVAL),
            ),
            (
                EXPIRED_LOG_CLEANUP_PROPERTY,
                Some(HISTORY_EXPIRED_LOG_CLEANUP),
            ),
        ])
        .await?;
    let table = session_delta_table_builder(root.clone(), session)?
        .with_skip_stats(false)
        .with_version(0)
        .load()
        .await?;
    validate_loaded_descriptor(&table, &spec)?;
    let predecessor = ExactDeltaPin::new(&root, 0)?;
    Ok(ProgrammaticRelationDeltaTarget {
        spec,
        predecessor,
        table,
    })
}

fn validate_storage_plan_schema(
    relation_id: &ProgrammaticRelationId,
    actual: &Schema,
    contract: &SchemaContract,
) -> Result<(), ProgrammaticRelationDeltaError> {
    let expected = contract.storage_schema();
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
        return Err(ProgrammaticRelationDeltaError::StoragePlanSchema {
            relation_id: relation_id.clone(),
            expected: Arc::clone(expected),
            actual: Arc::new(actual.clone()),
        });
    }
    Ok(())
}

fn validate_loaded_descriptor(
    table: &DeltaTable,
    expected: &RelationSnapshotSpec,
) -> Result<(), ProgrammaticRelationDeltaError> {
    let observed = read_descriptor(table)?;
    if observed != expected.descriptor {
        return Err(ProgrammaticRelationDeltaError::DescriptorDrift(
            expected.relation_id.clone(),
        ));
    }
    Ok(())
}

fn read_descriptor(
    table: &DeltaTable,
) -> Result<StoredRelationDescriptor, ProgrammaticRelationDeltaError> {
    let snapshot = table.snapshot()?;
    let description = snapshot
        .metadata()
        .description()
        .ok_or(ProgrammaticRelationDeltaError::MissingDescriptor)?;
    let encoded = description
        .strip_prefix(DESCRIPTOR_PREFIX)
        .ok_or(ProgrammaticRelationDeltaError::MissingDescriptor)?;
    let descriptor: StoredRelationDescriptor = serde_json::from_str(encoded)?;
    if canonical_descriptor(&descriptor)? != encoded {
        return Err(ProgrammaticRelationDeltaError::NonCanonicalDescriptor);
    }
    Ok(descriptor)
}

async fn load_exact(
    pin: &ExactDeltaPin,
    session: &datafusion::execution::SessionState,
) -> Result<DeltaTable, ProgrammaticRelationDeltaError> {
    Ok(
        session_delta_table_builder(pin.canonical_root().clone(), session)?
            .with_skip_stats(false)
            .with_version(pin.version())
            .load()
            .await?,
    )
}

/// Programmatic observation storage/view relations have their own exact append-only subsystem.
fn is_semantic_snapshot_relation(relation_id: &ProgrammaticRelationId) -> bool {
    !super::programmatic_observation_delta::is_programmatic_observation_relation(relation_id)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Fail-closed exact relation-snapshot errors.
#[derive(Debug, Error)]
pub enum ProgrammaticRelationDeltaError {
    #[error("programmatic relation snapshot layout root is invalid: {0}")]
    InvalidLayoutRoot(Url),
    #[error("cannot derive a relation root for {relation_id:?} below {root}: {source}")]
    LayoutJoin {
        root: Url,
        relation_id: ProgrammaticRelationId,
        source: url::ParseError,
    },
    #[error("programmatic relation table reference is not fully qualified: {0}")]
    UnqualifiedTableReference(TableReference),
    #[error("programmatic relation {0:?} has an empty storage schema")]
    EmptyStorageSchema(ProgrammaticRelationId),
    #[error("selected relation snapshot set differs from its descriptor set")]
    RelationSetMismatch,
    #[error("selected relation {selected:?} carries descriptor identity {descriptor:?}")]
    DescriptorRelationMismatch {
        selected: ProgrammaticRelationId,
        descriptor: String,
    },
    #[error("programmatic relation descriptor drifted for {0:?}")]
    DescriptorDrift(ProgrammaticRelationId),
    #[error("programmatic relation Delta table has no exact descriptor")]
    MissingDescriptor,
    #[error("programmatic relation Delta descriptor is not RFC 8785 canonical JSON")]
    NonCanonicalDescriptor,
    #[error("invalid programmatic relation descriptor: {0}")]
    InvalidDescriptor(String),
    #[error("relation {relation_id:?} storage field {storage_index} has no logical source")]
    UnmappedStorageField {
        relation_id: ProgrammaticRelationId,
        storage_index: usize,
    },
    #[error(
        "relation {relation_id:?} storage write plan differs from its exact contract: expected={expected:?}, actual={actual:?}"
    )]
    StoragePlanSchema {
        relation_id: ProgrammaticRelationId,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("relation {relation_id:?} exact Delta write did not commit: {detail}")]
    WriteOutcome {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Delta(#[from] DeltaTableError),
    #[error(transparent)]
    ExactProvider(#[from] ExactDeltaProviderError),
    #[error(transparent)]
    StorageProvider(#[from] super::provider::ProviderContractError),
    #[error(transparent)]
    Schema(#[from] SchemaContractError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
    #[error(transparent)]
    WriteInput(#[from] super::delta_write::ControlledDeltaWriteInputError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow_array::{ArrayRef, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;

    use crate::schema_contract::{FieldIndexMapping, SchemaContract, delta_storage_field};

    #[tokio::test]
    async fn bounded_snapshot_failure_drains_started_writes_without_admitting_more() {
        use std::time::Duration;

        let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let (finish_first, first) = tokio::sync::oneshot::channel();
        let (finish_second, second) = tokio::sync::oneshot::channel();
        let (finish_third, third) = tokio::sync::oneshot::channel();
        let work = [first, second, third]
            .into_iter()
            .enumerate()
            .map(move |(index, finish)| {
                let started = started.clone();
                async move {
                    started.send(index).unwrap();
                    finish.await.unwrap()
                }
            });
        let mut operation = tokio::spawn(super::collect_bounded_writes(work, 2));
        let started = [
            observed.recv().await.unwrap(),
            observed.recv().await.unwrap(),
        ];
        assert_eq!(
            std::collections::BTreeSet::from(started),
            std::collections::BTreeSet::from([0, 1])
        );
        assert!(observed.try_recv().is_err());

        finish_first.send(Err("first write failed")).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut operation)
                .await
                .is_err(),
            "a failed sibling cannot abandon an already-started write"
        );
        assert!(observed.try_recv().is_err(), "failure closes new admission");
        finish_second.send(Ok(1)).unwrap();
        assert_eq!(operation.await.unwrap(), Err("first write failed"));
        assert!(
            finish_third.send(Ok(2)).is_err(),
            "unstarted work is discarded"
        );
    }

    #[test]
    fn delta_list_storage_matches_kernel_and_restores_logical_values() {
        use deltalake::kernel::engine::arrow_conversion::{TryIntoArrow, TryIntoKernel};

        let child = Arc::new(
            Field::new("selected_feature", DataType::Utf8, true)
                .with_metadata(HashMap::from([("owner".into(), "logical-only".into())])),
        );
        let mut builder =
            arrow_array::builder::ListBuilder::new(arrow_array::builder::StringBuilder::new());
        builder.values().append_value("feature-a");
        builder.append(true);
        builder.append(false);
        builder.values().append_null();
        builder.append(true);
        let source = Arc::new(builder.finish()) as ArrayRef;
        for logical_type in [
            DataType::List(Arc::clone(&child)),
            DataType::LargeList(Arc::clone(&child)),
            DataType::ListView(Arc::clone(&child)),
            DataType::LargeListView(Arc::clone(&child)),
            DataType::FixedSizeList(Arc::clone(&child), 1),
        ] {
            let logical = Arc::new(Schema::new(vec![Field::new(
                "features",
                logical_type.clone(),
                true,
            )]));
            let storage = Arc::new(Schema::new(vec![delta_storage_field(logical.field(0))]));
            let kernel: deltalake::kernel::StructType = storage.as_ref().try_into_kernel().unwrap();
            let restored_schema: Schema = (&kernel).try_into_arrow().unwrap();
            assert_eq!(restored_schema, *storage);
            let contract = SchemaContract::try_new(
                "list-round-trip",
                TableReference::bare("features"),
                Arc::clone(&logical),
                storage,
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap();
            let values = arrow_cast::cast(&source, &logical_type).unwrap();
            let batch = RecordBatch::try_new(logical, vec![values]).unwrap();
            let stored = contract.adapt_logical_batch_to_storage(&batch).unwrap();
            assert_eq!(contract.restore_storage_batch(&stored).unwrap(), batch);
        }
    }
}
