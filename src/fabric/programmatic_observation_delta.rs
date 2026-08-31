//! Delta-historicized programmatic catalog observations.
//!
//! A typed registry derives stable append-only Delta histories from every
//! programmatic observation relation. Exact predecessor providers are first
//! installed under `_storage` in the candidate's original DataFusion session.
//! Native `ViewTable` plans expose only the candidate epoch under `system`;
//! after every registered zero-retry write, the exact committed providers and
//! their views are rebound and the catalog is re-observed before sealing.
//!
//! Arrow batches are transient write/proof inputs. The durable authorities are
//! the Delta rows, exact table pins, per-commit materialization evidence
//! (including explicit empty relations), and the activation publication which
//! names that complete vector.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use arrow_select::concat::concat_batches;
use datafusion::catalog::TableProvider;
use datafusion::common::{ScalarValue, TableReference};
use datafusion::datasource::provider_as_source;
use datafusion::logical_expr::expr_fn::cast;
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
use datafusion::prelude::{col, lit};
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
use deltalake::kernel::transaction::{PROTOCOL, TransactionError};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use deltalake::table::config::TablePropertiesExt as _;
use deltalake::{DeltaTable, DeltaTableBuilder, DeltaTableError};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use super::activation::{TableVersionSet, TableVersionSetError, TableVersionSetRef};
use super::command::{EpochId, OperationId, TransactionRef, WriterGeneration};
use super::delta_exact::{
    ExactDeltaPin, ExactDeltaProviderError, ValidatedDeltaSnapshot,
    provider_from_validated_snapshot, read_exact_commit_info,
};
use super::delta_write::{
    ApplicationTransactionMarker, ControlledDeltaWriteMode, ControlledDeltaWriteOutcome,
    ControlledDeltaWriteSpec, SessionBoundLogicalPlan, write_exact_delta_plan,
};
use super::programmatic_schema::{
    PreparedObservationRelation, PreparedObservationRelationSpec, ProgrammaticRelationId,
    ProgrammaticSchemaAssembly, ProgrammaticSchemaError, ProgrammaticTransformationId,
    SealedProgrammaticSchemaAssembly, observation_view_identity_boundary,
};
use super::provider::{ProviderContractError, SchemaContractStorageProvider};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
    SchemaContractError,
};

const HISTORY_SOURCE_IDENTITY: &str = "programmatic-observation-history-v1";
const HISTORY_SCHEMA: &str = "_storage";
const EPOCH_FIELD: &str = "fabric_epoch_id";
const OBSERVATION_SET_FIELD: &str = "observation_set_id";
const ROW_ORDINAL_FIELD: &str = "row_ordinal";
const APPEND_ONLY_PROPERTY: &str = "delta.appendOnly";
const CDF_PROPERTY: &str = "delta.enableChangeDataFeed";
const STATS_COLUMNS_PROPERTY: &str = "delta.dataSkippingStatsColumns";
const DELETION_VECTORS_PROPERTY: &str = "delta.enableDeletionVectors";
const LOG_RETENTION_PROPERTY: &str = "delta.logRetentionDuration";
const DELETED_FILE_RETENTION_PROPERTY: &str = "delta.deletedFileRetentionDuration";
const CHECKPOINT_INTERVAL_PROPERTY: &str = "delta.checkpointInterval";
const EXPIRED_LOG_CLEANUP_PROPERTY: &str = "delta.enableExpiredLogCleanup";
const STATS_COLUMNS: &str = "fabric_epoch_id,observation_set_id";
const HISTORY_POLICY_ID: &str = "programmatic-observation-history-policy-v1";
const HISTORY_LOG_RETENTION: &str = "interval 30 days";
const HISTORY_DELETED_FILE_RETENTION: &str = "interval 1 weeks";
const HISTORY_CHECKPOINT_INTERVAL: &str = "100";
const HISTORY_EXPIRED_LOG_CLEANUP: &str = "false";
const HISTORY_MIN_READER_VERSION: i32 = 1;
const HISTORY_MIN_WRITER_VERSION: i32 = 4;
const ALLOWED_READER_FEATURES: [&str; 0] = [];
const ALLOWED_WRITER_FEATURES: [&str; 2] = ["appendOnly", "changeDataFeed"];
const META_MATERIALIZATION_RELATION_ID: &str = "codefabric.materialization.relation_id";
const META_MATERIALIZATION_EPOCH_ID: &str = "codefabric.materialization.fabric_epoch_id";
const META_MATERIALIZATION_SET_ID: &str = "codefabric.materialization.observation_set_id";
const META_MATERIALIZATION_ROW_COUNT: &str = "codefabric.materialization.row_count";
const META_MATERIALIZATION_EMPTY: &str = "codefabric.materialization.empty";
const META_MATERIALIZATION_SCHEMA: &str = "codefabric.materialization.schema_fingerprint";
const META_MATERIALIZATION_POLICY: &str = "codefabric.materialization.policy_fingerprint";

#[derive(Clone)]
struct ObservationHistorySpec {
    system: PreparedObservationRelationSpec,
    history_relation_id: ProgrammaticRelationId,
    history_reference: TableReference,
    history_contract: Arc<SchemaContract>,
    view_transformation_id: ProgrammaticTransformationId,
}

/// Extensible, typed registry of every programmatic observation history.
///
/// Registration is derived from the live assembly rather than a fixed list of
/// relation IDs, so adding a new observation relation makes its physical
/// history, publication pin, and restart proof mandatory automatically.
#[derive(Clone)]
pub struct ProgrammaticObservationHistoryRegistry {
    histories: BTreeMap<ProgrammaticRelationId, ObservationHistorySpec>,
}

impl fmt::Debug for ProgrammaticObservationHistoryRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticObservationHistoryRegistry")
            .field("relation_ids", &self.histories.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ProgrammaticObservationHistoryRegistry {
    /// Compile the dependency-closed history registry from the current
    /// programmatic observation contracts.
    pub fn try_from_assembly(
        assembly: &ProgrammaticSchemaAssembly,
    ) -> Result<Self, ProgrammaticObservationHistoryRegistryError> {
        let mut histories = BTreeMap::new();
        for system in assembly.observation_relation_specs()? {
            let history = history_spec(system)?;
            let relation_id = history.system.relation_id.clone();
            if histories.insert(relation_id.clone(), history).is_some() {
                return Err(
                    ProgrammaticObservationHistoryRegistryError::DuplicateRegistration {
                        relation_id,
                    },
                );
            }
        }
        if histories.is_empty() {
            return Err(ProgrammaticObservationHistoryRegistryError::EmptyRegistry);
        }
        Ok(Self { histories })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.histories.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.histories.is_empty()
    }

    pub fn relation_ids(&self) -> impl ExactSizeIterator<Item = &ProgrammaticRelationId> {
        self.histories.keys()
    }

    fn histories(&self) -> impl ExactSizeIterator<Item = &ObservationHistorySpec> {
        self.histories.values()
    }

    fn history(&self, relation_id: &ProgrammaticRelationId) -> Option<&ObservationHistorySpec> {
        self.histories.get(relation_id)
    }
}

/// Registry construction failures happen before any Delta mutation.
#[derive(Debug, Error)]
pub enum ProgrammaticObservationHistoryRegistryError {
    #[error(transparent)]
    Assembly(#[from] ProgrammaticSchemaError),
    #[error(transparent)]
    Schema(#[from] SchemaContractError),
    #[error("duplicate programmatic observation history registration {relation_id:?}")]
    DuplicateRegistration { relation_id: ProgrammaticRelationId },
    #[error("programmatic observation history registry must not be empty")]
    EmptyRegistry,
}

/// Exact schema/protocol/storage-policy evidence observed from one loaded
/// history snapshot before it can be queried or written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticObservationHistoryPolicyEvidence {
    schema_fingerprint: [u8; 32],
    protocol_fingerprint: [u8; 32],
    policy_fingerprint: [u8; 32],
    min_reader_version: i32,
    min_writer_version: i32,
    reader_features: Arc<[String]>,
    writer_features: Arc<[String]>,
    log_retention_seconds: u64,
    deleted_file_retention_seconds: u64,
    checkpoint_interval: u64,
    expired_log_cleanup_enabled: bool,
    require_files: bool,
    skip_stats: bool,
}

impl ProgrammaticObservationHistoryPolicyEvidence {
    #[must_use]
    pub const fn schema_fingerprint(&self) -> &[u8; 32] {
        &self.schema_fingerprint
    }

    #[must_use]
    pub const fn protocol_fingerprint(&self) -> &[u8; 32] {
        &self.protocol_fingerprint
    }

    #[must_use]
    pub const fn policy_fingerprint(&self) -> &[u8; 32] {
        &self.policy_fingerprint
    }

    #[must_use]
    pub const fn min_reader_version(&self) -> i32 {
        self.min_reader_version
    }

    #[must_use]
    pub const fn min_writer_version(&self) -> i32 {
        self.min_writer_version
    }

    #[must_use]
    pub fn reader_features(&self) -> &[String] {
        &self.reader_features
    }

    #[must_use]
    pub fn writer_features(&self) -> &[String] {
        &self.writer_features
    }

    #[must_use]
    pub const fn log_retention_seconds(&self) -> u64 {
        self.log_retention_seconds
    }

    #[must_use]
    pub const fn deleted_file_retention_seconds(&self) -> u64 {
        self.deleted_file_retention_seconds
    }

    #[must_use]
    pub const fn checkpoint_interval(&self) -> u64 {
        self.checkpoint_interval
    }

    #[must_use]
    pub const fn expired_log_cleanup_enabled(&self) -> bool {
        self.expired_log_cleanup_enabled
    }

    #[must_use]
    pub const fn require_files(&self) -> bool {
        self.require_files
    }

    #[must_use]
    pub const fn skip_stats(&self) -> bool {
        self.skip_stats
    }
}

/// Durable per-relation materialization evidence stored in the Delta commit.
/// A zero `row_count` plus `empty=true` is an explicit observed-empty relation,
/// not an absent provider or omitted write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticObservationMaterializationEvidence {
    relation_id: ProgrammaticRelationId,
    epoch_id: EpochId,
    observation_set_id: TransactionRef,
    row_count: u64,
    empty: bool,
    schema_fingerprint: [u8; 32],
    policy_fingerprint: [u8; 32],
}

impl ProgrammaticObservationMaterializationEvidence {
    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn observation_set_id(&self) -> TransactionRef {
        self.observation_set_id
    }

    #[must_use]
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.empty
    }

    #[must_use]
    pub const fn schema_fingerprint(&self) -> &[u8; 32] {
        &self.schema_fingerprint
    }

    #[must_use]
    pub const fn policy_fingerprint(&self) -> &[u8; 32] {
        &self.policy_fingerprint
    }

    fn commit_metadata(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            (
                META_MATERIALIZATION_RELATION_ID.to_owned(),
                Value::String(self.relation_id.as_str().to_owned()),
            ),
            (
                META_MATERIALIZATION_EPOCH_ID.to_owned(),
                Value::String(lower_hex(self.epoch_id.as_bytes())),
            ),
            (
                META_MATERIALIZATION_SET_ID.to_owned(),
                Value::String(lower_hex(self.observation_set_id.as_bytes())),
            ),
            (
                META_MATERIALIZATION_ROW_COUNT.to_owned(),
                Value::from(self.row_count),
            ),
            (
                META_MATERIALIZATION_EMPTY.to_owned(),
                Value::Bool(self.empty),
            ),
            (
                META_MATERIALIZATION_SCHEMA.to_owned(),
                Value::String(lower_hex(&self.schema_fingerprint)),
            ),
            (
                META_MATERIALIZATION_POLICY.to_owned(),
                Value::String(lower_hex(&self.policy_fingerprint)),
            ),
        ])
    }
}

/// One already-loaded exact predecessor for a stable observation history.
pub struct ProgrammaticObservationDeltaTarget {
    relation_id: ProgrammaticRelationId,
    predecessor: ExactDeltaPin,
    table: DeltaTable,
    policy_evidence: ProgrammaticObservationHistoryPolicyEvidence,
}

impl fmt::Debug for ProgrammaticObservationDeltaTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticObservationDeltaTarget")
            .field("relation_id", &self.relation_id)
            .field("predecessor", &self.predecessor)
            .field("policy_evidence", &self.policy_evidence)
            .finish_non_exhaustive()
    }
}

impl ProgrammaticObservationDeltaTarget {
    /// Bind a loaded table to its exact predecessor and required history-table
    /// properties. This constructor never loads or discovers latest state.
    fn try_new(
        registration: &ObservationHistorySpec,
        predecessor: ExactDeltaPin,
        table: DeltaTable,
    ) -> Result<Self, ProgrammaticObservationDeltaConfigurationError> {
        let relation_id = registration.system.relation_id.clone();
        let _ = ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &predecessor)
            .map_err(|source| {
                ProgrammaticObservationDeltaConfigurationError::ExactTargetIdentity {
                    relation_id: relation_id.clone(),
                    source,
                }
            })?;
        let policy_evidence = validate_history_policy(registration, &table)?;
        Ok(Self {
            relation_id,
            predecessor,
            table,
            policy_evidence,
        })
    }

    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }

    #[must_use]
    pub const fn policy_evidence(&self) -> &ProgrammaticObservationHistoryPolicyEvidence {
        &self.policy_evidence
    }
}

/// Dependency-closed target set for every registered observation history.
pub struct ProgrammaticObservationDeltaTargets {
    targets: BTreeMap<ProgrammaticRelationId, ProgrammaticObservationDeltaTarget>,
}

impl ProgrammaticObservationDeltaTargets {
    /// Require exactly one stable history target for every registered family.
    pub fn try_new(
        registry: &ProgrammaticObservationHistoryRegistry,
        targets: impl IntoIterator<Item = ProgrammaticObservationDeltaTarget>,
    ) -> Result<Self, ProgrammaticObservationDeltaConfigurationError> {
        let mut indexed = BTreeMap::new();
        for target in targets {
            let relation_id = target.relation_id.clone();
            if indexed.insert(relation_id.clone(), target).is_some() {
                return Err(
                    ProgrammaticObservationDeltaConfigurationError::DuplicateTarget { relation_id },
                );
            }
        }
        let expected = expected_relation_ids(registry);
        let actual = indexed.keys().cloned().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(
                ProgrammaticObservationDeltaConfigurationError::TargetSetMismatch {
                    expected,
                    actual,
                },
            );
        }
        Ok(Self { targets: indexed })
    }

    fn remove(
        &mut self,
        relation_id: &ProgrammaticRelationId,
    ) -> Option<ProgrammaticObservationDeltaTarget> {
        self.targets.remove(relation_id)
    }
}

/// One command-owned, fenced attempt to append an epoch's observation set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammaticObservationWriteIdentity {
    epoch_id: EpochId,
    operation_id: OperationId,
    writer_generation: WriterGeneration,
    observation_set_id: TransactionRef,
}

impl ProgrammaticObservationWriteIdentity {
    #[must_use]
    pub const fn new(
        epoch_id: EpochId,
        operation_id: OperationId,
        writer_generation: WriterGeneration,
        observation_set_id: TransactionRef,
    ) -> Self {
        Self {
            epoch_id,
            operation_id,
            writer_generation,
            observation_set_id,
        }
    }

    #[must_use]
    pub const fn epoch_id(self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn observation_set_id(self) -> TransactionRef {
        self.observation_set_id
    }
}

/// Application-level multi-table publication manifest.
///
/// Delta commits are atomic per table. This value is constructed only after
/// all registered writes, exact-provider replacements, current-view readbacks,
/// and semantic re-observation succeed.
#[derive(Clone)]
pub struct ProgrammaticObservationDeltaPublication {
    epoch_id: EpochId,
    table_versions: Arc<TableVersionSet>,
    materializations:
        Arc<BTreeMap<ProgrammaticRelationId, ProgrammaticObservationMaterializationEvidence>>,
    registry: Arc<ProgrammaticObservationHistoryRegistry>,
}

impl fmt::Debug for ProgrammaticObservationDeltaPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticObservationDeltaPublication")
            .field("epoch_id", &self.epoch_id)
            .field("table_versions", &self.table_versions)
            .field("materializations", &self.materializations)
            .field("registry", &self.registry)
            .finish()
    }
}

impl ProgrammaticObservationDeltaPublication {
    /// Reconstruct the complete publication authority from the selected epoch
    /// and its reversible exact-version vector.
    ///
    /// The operation, fence, and application transaction that produced each
    /// Delta commit remain durable in the Delta logs and history rows. They are
    /// not part of the selected epoch state and therefore are not duplicated in
    /// this reconstructible publication value.
    pub fn try_new(
        epoch_id: EpochId,
        table_versions: Arc<TableVersionSet>,
        materializations: BTreeMap<
            ProgrammaticRelationId,
            ProgrammaticObservationMaterializationEvidence,
        >,
        registry: Arc<ProgrammaticObservationHistoryRegistry>,
    ) -> Result<Self, ProgrammaticObservationDeltaConfigurationError> {
        validate_publication_relations(&table_versions, &registry)?;
        validate_materialization_relations(epoch_id, &materializations, &registry)?;
        Ok(Self {
            epoch_id,
            table_versions,
            materializations: Arc::new(materializations),
            registry,
        })
    }

    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub fn table_version(&self, relation_id: &ProgrammaticRelationId) -> Option<&ExactDeltaPin> {
        self.table_versions.pin(relation_id.as_str())
    }

    pub fn table_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &ExactDeltaPin)> + DoubleEndedIterator {
        self.table_versions.components()
    }

    /// Complete reversible exact-version vector selected by activation.
    #[must_use]
    pub const fn table_version_set(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
    }

    /// Exact per-relation row-count and empty-materialization evidence read
    /// back from the selected Delta commits.
    pub fn materializations(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &ProgrammaticRelationId,
            &ProgrammaticObservationMaterializationEvidence,
        ),
    > {
        self.materializations.iter()
    }

    /// Canonical activation pin for the complete registered history vector.
    /// Roots are included because a version number is meaningful only within
    /// its exact Delta table identity.
    #[must_use]
    pub fn table_version_set_ref(&self) -> TableVersionSetRef {
        self.table_versions.reference()
    }

    /// Reopen exactly the registered versions named by this publication. This
    /// never discovers or advances to a latest table version, and reads back
    /// the materialization evidence from each exact commit.
    pub async fn open_targets(
        &self,
    ) -> Result<ProgrammaticObservationDeltaTargets, ProgrammaticObservationDeltaOpenError> {
        let (targets, materializations) = load_published_targets(
            self.epoch_id,
            &self.table_versions,
            Arc::clone(&self.registry),
        )
        .await?;
        if materializations != self.materializations.as_ref().clone() {
            return Err(ProgrammaticObservationDeltaOpenError::Configuration(
                ProgrammaticObservationDeltaConfigurationError::MaterializationReadbackMismatch,
            ));
        }
        Ok(targets)
    }

    async fn reopen(
        epoch_id: EpochId,
        table_versions: Arc<TableVersionSet>,
        registry: Arc<ProgrammaticObservationHistoryRegistry>,
    ) -> Result<(Self, ProgrammaticObservationDeltaTargets), ProgrammaticObservationDeltaOpenError>
    {
        validate_publication_relations(&table_versions, &registry)?;
        let (targets, materializations) =
            load_published_targets(epoch_id, &table_versions, Arc::clone(&registry)).await?;
        let publication = Self::try_new(epoch_id, table_versions, materializations, registry)?;
        Ok((publication, targets))
    }
}

async fn load_published_targets(
    epoch_id: EpochId,
    table_versions: &TableVersionSet,
    registry: Arc<ProgrammaticObservationHistoryRegistry>,
) -> Result<
    (
        ProgrammaticObservationDeltaTargets,
        BTreeMap<ProgrammaticRelationId, ProgrammaticObservationMaterializationEvidence>,
    ),
    ProgrammaticObservationDeltaOpenError,
> {
    let mut targets = Vec::with_capacity(table_versions.len());
    let mut materializations = BTreeMap::new();
    for (relation_id, pin) in table_versions.components() {
        let relation_id = ProgrammaticRelationId::new(relation_id);
        let registration = registry.history(&relation_id).ok_or_else(|| {
            ProgrammaticObservationDeltaConfigurationError::UnknownRegistration {
                relation_id: relation_id.clone(),
            }
        })?;
        let table = DeltaTableBuilder::from_url(pin.canonical_root().clone())
            .map_err(|source| ProgrammaticObservationDeltaOpenError::Delta {
                relation_id: relation_id.clone(),
                source,
            })?
            // Query-serving snapshots require active files and parsed add-action
            // statistics. At the pinned delta-rs revision, `skip_stats=true`
            // disables both statistics and partition pruning.
            .with_skip_stats(false)
            .with_version(pin.version())
            .load()
            .await
            .map_err(|source| ProgrammaticObservationDeltaOpenError::Delta {
                relation_id: relation_id.clone(),
                source,
            })?;
        let target = ProgrammaticObservationDeltaTarget::try_new(registration, pin.clone(), table)?;
        let materialization = read_materialization_evidence(
            epoch_id,
            registration,
            &target.table,
            target.policy_evidence(),
        )
        .await?;
        materializations.insert(relation_id, materialization);
        targets.push(target);
    }
    Ok((
        ProgrammaticObservationDeltaTargets::try_new(&registry, targets)?,
        materializations,
    ))
}

/// A sealed same-session catalog and its complete exact Delta history vector.
pub struct ProgrammaticObservationHistoricization {
    sealed: SealedProgrammaticSchemaAssembly,
    publication: ProgrammaticObservationDeltaPublication,
}

impl ProgrammaticObservationHistoricization {
    #[must_use]
    pub const fn sealed(&self) -> &SealedProgrammaticSchemaAssembly {
        &self.sealed
    }

    #[must_use]
    pub const fn publication(&self) -> &ProgrammaticObservationDeltaPublication {
        &self.publication
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        SealedProgrammaticSchemaAssembly,
        ProgrammaticObservationDeltaPublication,
    ) {
        (self.sealed, self.publication)
    }
}

/// Configuration errors rejected before a candidate or Delta table is mutated.
#[derive(Debug, Error)]
pub enum ProgrammaticObservationDeltaConfigurationError {
    #[error("duplicate Delta observation target {relation_id:?}")]
    DuplicateTarget { relation_id: ProgrammaticRelationId },
    #[error("Delta observation target set differs: expected {expected:?}, actual {actual:?}")]
    TargetSetMismatch {
        expected: BTreeSet<ProgrammaticRelationId>,
        actual: BTreeSet<ProgrammaticRelationId>,
    },
    #[error("Delta observation relation has no typed history registration: {relation_id:?}")]
    UnknownRegistration { relation_id: ProgrammaticRelationId },
    #[error("exact Delta target identity failed for {relation_id:?}: {source}")]
    ExactTargetIdentity {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: ExactDeltaProviderError,
    },
    #[error("Delta history target {relation_id:?} has invalid property {key}: {actual:?}")]
    RequiredTableProperty {
        relation_id: ProgrammaticRelationId,
        key: &'static str,
        actual: Option<String>,
    },
    #[error("Delta history target {relation_id:?} has invalid native schema: {detail}")]
    NativeStorageSchema {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error(
        "Delta history target {relation_id:?} has protocol {min_reader_version}/{min_writer_version}, expected {HISTORY_MIN_READER_VERSION}/{HISTORY_MIN_WRITER_VERSION}"
    )]
    ProtocolVersion {
        relation_id: ProgrammaticRelationId,
        min_reader_version: i32,
        min_writer_version: i32,
    },
    #[error("Delta history target {relation_id:?} declares unsupported {side} feature {feature}")]
    UnsupportedProtocolFeature {
        relation_id: ProgrammaticRelationId,
        side: &'static str,
        feature: String,
    },
    #[error("Delta history target {relation_id:?} is not {operation}-compatible: {source}")]
    ProtocolSupport {
        relation_id: ProgrammaticRelationId,
        operation: &'static str,
        #[source]
        source: TransactionError,
    },
    #[error(
        "Delta history target {relation_id:?} has invalid provider load policy: require_files={require_files}, skip_stats={skip_stats}"
    )]
    ProviderLoadPolicy {
        relation_id: ProgrammaticRelationId,
        require_files: bool,
        skip_stats: bool,
    },
    #[error("Delta history target {relation_id:?} has no loaded snapshot: {source}")]
    MissingSnapshot {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: DeltaTableError,
    },
    #[error("Delta materialization relation set differs: expected {expected:?}, actual {actual:?}")]
    MaterializationSetMismatch {
        expected: BTreeSet<ProgrammaticRelationId>,
        actual: BTreeSet<ProgrammaticRelationId>,
    },
    #[error("Delta materialization evidence for {relation_id:?} names a different epoch")]
    MaterializationEpochMismatch { relation_id: ProgrammaticRelationId },
    #[error("exact Delta materialization evidence changed between publication and reopen")]
    MaterializationReadbackMismatch,
}

/// Initial table creation failures. Existing histories must instead be opened
/// from exact application-owned pins; this API never falls back to latest.
#[derive(Debug, Error)]
pub enum ProgrammaticObservationProvisionError {
    #[error(transparent)]
    Registry(#[from] ProgrammaticObservationHistoryRegistryError),
    #[error(transparent)]
    Assembly(#[from] ProgrammaticSchemaError),
    #[error(transparent)]
    Schema(#[from] SchemaContractError),
    #[error("missing initial Delta root for {relation_id:?}")]
    MissingRoot { relation_id: ProgrammaticRelationId },
    #[error("unexpected initial Delta roots for {relation_ids:?}")]
    UnexpectedRoots {
        relation_ids: BTreeSet<ProgrammaticRelationId>,
    },
    #[error("failed to create or load initial Delta history {relation_id:?}: {source}")]
    Delta {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: DeltaTableError,
    },
    #[error(transparent)]
    Configuration(#[from] ProgrammaticObservationDeltaConfigurationError),
}

/// Exact-version reopen failures for an existing complete publication.
#[derive(Debug, Error)]
pub enum ProgrammaticObservationDeltaOpenError {
    #[error("failed to load exact Delta history {relation_id:?}: {source}")]
    Delta {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: DeltaTableError,
    },
    #[error(
        "exact Delta history {relation_id:?} has invalid materialization metadata {key}: {detail}"
    )]
    MaterializationMetadata {
        relation_id: ProgrammaticRelationId,
        key: &'static str,
        detail: String,
    },
    #[error(transparent)]
    Configuration(#[from] ProgrammaticObservationDeltaConfigurationError),
}

/// Stage-local cause for a failed multi-table historicization attempt.
#[derive(Debug, Error)]
pub enum ProgrammaticObservationDeltaError {
    #[error(transparent)]
    Registry(#[from] ProgrammaticObservationHistoryRegistryError),
    #[error(transparent)]
    Assembly(#[from] ProgrammaticSchemaError),
    #[error(transparent)]
    Schema(#[from] SchemaContractError),
    #[error(transparent)]
    Configuration(#[from] ProgrammaticObservationDeltaConfigurationError),
    #[error(transparent)]
    Open(#[from] ProgrammaticObservationDeltaOpenError),
    #[error(transparent)]
    TableVersionSet(#[from] TableVersionSetError),
    #[error("missing exact Delta target for {relation_id:?}")]
    MissingTarget { relation_id: ProgrammaticRelationId },
    #[error("exact Delta provider reconstruction failed for {relation_id:?}: {source}")]
    ExactProvider {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: ExactDeltaProviderError,
    },
    #[error("storage schema restoration failed for {relation_id:?}: {source}")]
    StorageProvider {
        relation_id: ProgrammaticRelationId,
        #[source]
        source: ProviderContractError,
    },
    #[error("observation history batch failed for {relation_id:?}: {detail}")]
    HistoryBatch {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error("DataFusion write-plan construction failed for {relation_id:?}: {detail}")]
    WritePlan {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error("Delta materialization evidence failed for {relation_id:?}: {detail}")]
    MaterializationEvidence {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error("current-epoch view-plan construction failed for {relation_id:?}: {detail}")]
    ViewPlan {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error("Delta append did not produce one proved commit for {relation_id:?}: {outcome:?}")]
    WriteOutcome {
        relation_id: ProgrammaticRelationId,
        outcome: Box<ControlledDeltaWriteOutcome>,
    },
    #[error("current-epoch view readback failed for {relation_id:?}: {detail}")]
    ViewReadback {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error(
        "current-epoch view rows differ from the candidate observation set for {relation_id:?}"
    )]
    ViewReadbackMismatch { relation_id: ProgrammaticRelationId },
}

/// Fail-closed error retaining every exact version already staged by this
/// attempt so the command actor can reconcile without guessing or retrying.
#[derive(Debug)]
pub struct ProgrammaticObservationHistoricizationFailure {
    committed: BTreeMap<ProgrammaticRelationId, ExactDeltaPin>,
    source: ProgrammaticObservationDeltaError,
}

impl ProgrammaticObservationHistoricizationFailure {
    #[must_use]
    pub const fn committed(&self) -> &BTreeMap<ProgrammaticRelationId, ExactDeltaPin> {
        &self.committed
    }

    #[must_use]
    pub const fn cause(&self) -> &ProgrammaticObservationDeltaError {
        &self.source
    }
}

impl fmt::Display for ProgrammaticObservationHistoricizationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "programmatic observation historicization failed after {} staged commits: {}",
            self.committed.len(),
            self.source
        )
    }
}

impl Error for ProgrammaticObservationHistoricizationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Provision all registered version-zero history tables directly from the
/// programmatic observation contracts.
///
/// This is physical table initialization, not a semantic bootstrap model. It
/// is legal only for roots known to have no Delta table. Existing tables must
/// be supplied through exact pins and [`ProgrammaticObservationDeltaTarget`].
pub async fn provision_programmatic_observation_histories(
    assembly: &ProgrammaticSchemaAssembly,
    mut roots: BTreeMap<ProgrammaticRelationId, Url>,
) -> Result<ProgrammaticObservationDeltaTargets, ProgrammaticObservationProvisionError> {
    let registry = ProgrammaticObservationHistoryRegistry::try_from_assembly(assembly)?;
    let mut targets = Vec::with_capacity(registry.len());
    for history in registry.histories() {
        let relation_id = history.system.relation_id.clone();
        let root = roots.remove(&relation_id).ok_or_else(|| {
            ProgrammaticObservationProvisionError::MissingRoot {
                relation_id: relation_id.clone(),
            }
        })?;
        let kernel: deltalake::kernel::StructType = history
            .history_contract
            .storage_schema()
            .as_ref()
            .try_into_kernel()
            .map_err(|source| ProgrammaticObservationProvisionError::Delta {
                relation_id: relation_id.clone(),
                source: DeltaTableError::Arrow { source },
            })?;
        CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name(history.history_reference.table())
            .with_comment(format!(
                "CodeFabric append-only history for {}",
                relation_id.as_str()
            ))
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .with_configuration([
                (APPEND_ONLY_PROPERTY, Some("true")),
                (CDF_PROPERTY, Some("true")),
                (STATS_COLUMNS_PROPERTY, Some(STATS_COLUMNS)),
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
            .await
            .map_err(|source| ProgrammaticObservationProvisionError::Delta {
                relation_id: relation_id.clone(),
                source,
            })?;
        let table = DeltaTableBuilder::from_url(root.clone())
            .map_err(|source| ProgrammaticObservationProvisionError::Delta {
                relation_id: relation_id.clone(),
                source,
            })?
            .with_skip_stats(false)
            .with_version(0)
            .load()
            .await
            .map_err(|source| ProgrammaticObservationProvisionError::Delta {
                relation_id: relation_id.clone(),
                source,
            })?;
        let pin = ExactDeltaPin::new(&root, 0).map_err(|source| {
            ProgrammaticObservationProvisionError::Configuration(
                ProgrammaticObservationDeltaConfigurationError::ExactTargetIdentity {
                    relation_id: relation_id.clone(),
                    source,
                },
            )
        })?;
        targets.push(ProgrammaticObservationDeltaTarget::try_new(
            history, pin, table,
        )?);
    }
    if !roots.is_empty() {
        return Err(ProgrammaticObservationProvisionError::UnexpectedRoots {
            relation_ids: roots.into_keys().collect(),
        });
    }
    Ok(ProgrammaticObservationDeltaTargets::try_new(
        &registry, targets,
    )?)
}

async fn bind_exact_observation_histories(
    assembly: &mut ProgrammaticSchemaAssembly,
    epoch_id: EpochId,
    registry: &ProgrammaticObservationHistoryRegistry,
    mut targets: ProgrammaticObservationDeltaTargets,
    session: Arc<datafusion::execution::SessionState>,
) -> Result<
    BTreeMap<ProgrammaticRelationId, (ObservationHistorySpec, ProgrammaticObservationDeltaTarget)>,
    ProgrammaticObservationDeltaError,
> {
    let mut installed = BTreeMap::new();
    for history in registry.histories() {
        let history = history.clone();
        let relation_id = history.system.relation_id.clone();
        let target = targets.remove(&relation_id).ok_or_else(|| {
            ProgrammaticObservationDeltaError::MissingTarget {
                relation_id: relation_id.clone(),
            }
        })?;
        let provider = exact_history_provider(
            Arc::clone(&session),
            &history,
            target.table.clone(),
            &target.predecessor,
        )
        .await?;
        assembly
            .register_provider(super::programmatic_schema::ProviderInput::new(
                history.history_relation_id.clone(),
                history.history_reference.clone(),
                Arc::clone(&history.history_contract),
                Arc::clone(&provider),
            ))
            .map_err(ProgrammaticObservationDeltaError::from)?;
        let view = current_epoch_view_plan(assembly, &history, provider, epoch_id)?;
        assembly
            .register_observation_view(
                history.system.clone(),
                history.view_transformation_id.clone(),
                Arc::from([history.history_relation_id.clone()]),
                view,
            )
            .map_err(ProgrammaticObservationDeltaError::from)?;
        installed.insert(relation_id, (history, target));
    }
    Ok(installed)
}

/// Reconstruct and seal an epoch from the exact relation/version vector that
/// was selected by its durable activation event.
///
/// This path performs no Delta writes and never discovers latest state. It
/// loads each named version, installs native current-epoch views in the fresh
/// candidate session, and proves those views equal a new live catalog
/// observation before sealing.
pub async fn reopen_programmatic_observations(
    mut assembly: ProgrammaticSchemaAssembly,
    epoch_id: EpochId,
    table_versions: Arc<TableVersionSet>,
) -> Result<ProgrammaticObservationHistoricization, ProgrammaticObservationDeltaError> {
    let registry = Arc::new(ProgrammaticObservationHistoryRegistry::try_from_assembly(
        &assembly,
    )?);
    let (publication, targets) = ProgrammaticObservationDeltaPublication::reopen(
        epoch_id,
        Arc::clone(&table_versions),
        Arc::clone(&registry),
    )
    .await?;
    assembly.install_transformations().await?;
    let session = Arc::new(assembly.candidate_state());
    let context = assembly.candidate_context();
    let installed =
        bind_exact_observation_histories(&mut assembly, epoch_id, &registry, targets, session)
            .await?;
    let selected = installed
        .iter()
        .map(|(relation_id, (_, target))| (relation_id.clone(), target.predecessor.clone()))
        .collect::<BTreeMap<_, _>>();
    let prepared = assembly
        .materialize_live_observation_relations(epoch_id)
        .await?;
    let current_epoch_batches = prepared
        .into_iter()
        .map(|relation| (relation.relation_id, relation.batch))
        .collect::<BTreeMap<_, _>>();
    verify_current_epoch_views(&context, &registry, &current_epoch_batches, &selected).await?;
    let sealed = assembly
        .finish_seal(epoch_id, current_epoch_batches)
        .await?;
    Ok(ProgrammaticObservationHistoricization {
        sealed,
        publication,
    })
}

/// Append every registered history, rebind exact committed providers and native
/// current-epoch views in the original candidate session, prove fixed-point
/// equality, and only then seal the candidate.
pub async fn historicize_programmatic_observations(
    mut assembly: ProgrammaticSchemaAssembly,
    identity: ProgrammaticObservationWriteIdentity,
    targets: ProgrammaticObservationDeltaTargets,
) -> Result<ProgrammaticObservationHistoricization, ProgrammaticObservationHistoricizationFailure> {
    let mut committed = BTreeMap::new();
    let mut materializations = BTreeMap::new();
    let registry = Arc::new(
        ProgrammaticObservationHistoryRegistry::try_from_assembly(&assembly)
            .map_err(|source| failure(&committed, source.into()))?,
    );
    assembly
        .install_transformations()
        .await
        .map_err(|source| failure(&committed, source.into()))?;
    let session = Arc::new(assembly.candidate_state());
    let context = assembly.candidate_context();

    // Bind predecessor histories and current-epoch views before materializing
    // observations so both physical dependencies and logical views are part of
    // the self-inclusive catalog census.
    let mut installed = bind_exact_observation_histories(
        &mut assembly,
        identity.epoch_id,
        &registry,
        targets,
        Arc::clone(&session),
    )
    .await
    .map_err(|source| failure(&committed, source))?;

    let prepared = assembly
        .materialize_live_observation_relations(identity.epoch_id)
        .await
        .map_err(|source| failure(&committed, source.into()))?;
    let current_epoch_batches = prepared
        .iter()
        .map(|relation| (relation.relation_id.clone(), relation.batch.clone()))
        .collect::<BTreeMap<_, _>>();

    for relation in prepared {
        let relation_id = relation.relation_id.clone();
        let (history, target) = installed.remove(&relation_id).ok_or_else(|| {
            failure(
                &committed,
                ProgrammaticObservationDeltaError::MissingTarget {
                    relation_id: relation_id.clone(),
                },
            )
        })?;
        let write_schema = target
            .table
            .snapshot()
            .map_err(|source| {
                failure(
                    &committed,
                    ProgrammaticObservationDeltaError::HistoryBatch {
                        relation_id: relation_id.clone(),
                        detail: format!("loaded predecessor snapshot is unavailable: {source}"),
                    },
                )
            })?
            .snapshot()
            .arrow_schema();
        let history_batch =
            build_history_batch(&relation, identity.observation_set_id, write_schema)
                .map_err(|source| failure(&committed, source))?;
        let materialization =
            materialization_evidence(&relation, identity, target.policy_evidence())
                .map_err(|source| failure(&committed, source))?;
        let dataframe = context.read_batch(history_batch).map_err(|source| {
            failure(
                &committed,
                ProgrammaticObservationDeltaError::WritePlan {
                    relation_id: relation_id.clone(),
                    detail: source.to_string(),
                },
            )
        })?;
        let plan = SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&session), dataframe)
            .map_err(|source| {
                failure(
                    &committed,
                    ProgrammaticObservationDeltaError::WritePlan {
                        relation_id: relation_id.clone(),
                        detail: source.to_string(),
                    },
                )
            })?;
        let spec = ControlledDeltaWriteSpec::new(
            target.predecessor,
            identity.operation_id,
            identity.writer_generation,
            ApplicationTransactionMarker::from_transaction_ref(identity.observation_set_id),
            ControlledDeltaWriteMode::Append,
        )
        .with_commit_metadata(materialization.commit_metadata())
        .map_err(|source| {
            failure(
                &committed,
                ProgrammaticObservationDeltaError::MaterializationEvidence {
                    relation_id: relation_id.clone(),
                    detail: source.to_string(),
                },
            )
        })?;
        let write = write_exact_delta_plan(&target.table, &spec, plan).await;
        let committed_write = match write {
            ControlledDeltaWriteOutcome::Committed(committed_write) => committed_write,
            outcome => {
                return Err(failure(
                    &committed,
                    ProgrammaticObservationDeltaError::WriteOutcome {
                        relation_id,
                        outcome: Box::new(outcome),
                    },
                ));
            }
        };
        let committed_pin = committed_write.committed().clone();
        let committed_table = committed_write.into_table();
        committed.insert(relation_id.clone(), committed_pin.clone());
        materializations.insert(relation_id.clone(), materialization);

        let provider = exact_history_provider(
            Arc::clone(&session),
            &history,
            committed_table,
            &committed_pin,
        )
        .await
        .map_err(|source| failure(&committed, source))?;
        assembly
            .replace_registered_provider(&history.history_relation_id, Arc::clone(&provider))
            .map_err(|source| failure(&committed, source.into()))?;
        let view = current_epoch_view_plan(&assembly, &history, provider, identity.epoch_id)
            .map_err(|source| failure(&committed, source))?;
        assembly
            .replace_observation_view(&relation_id, view)
            .map_err(|source| failure(&committed, source.into()))?;
    }

    verify_current_epoch_views(&context, &registry, &current_epoch_batches, &committed)
        .await
        .map_err(|source| failure(&committed, source))?;
    let sealed = assembly
        .finish_seal(identity.epoch_id, current_epoch_batches)
        .await
        .map_err(|source| failure(&committed, source.into()))?;
    let table_versions = Arc::new(
        TableVersionSet::try_new(
            committed
                .iter()
                .map(|(relation_id, pin)| (Arc::<str>::from(relation_id.as_str()), pin.clone())),
        )
        .map_err(|source| failure(&committed, source.into()))?,
    );
    let publication = ProgrammaticObservationDeltaPublication::try_new(
        identity.epoch_id,
        table_versions,
        materializations,
        registry,
    )
    .map_err(|source| failure(&committed, source.into()))?;
    Ok(ProgrammaticObservationHistoricization {
        sealed,
        publication,
    })
}

fn history_spec(
    system: PreparedObservationRelationSpec,
) -> Result<ObservationHistorySpec, SchemaContractError> {
    let table_name = format!("{}_history", system.table_reference.table());
    let history_relation_id = ProgrammaticRelationId::new(format!("{HISTORY_SCHEMA}.{table_name}"));
    let catalog = system
        .table_reference
        .catalog()
        .expect("programmatic observation references are fully qualified");
    let history_reference = TableReference::full(catalog, HISTORY_SCHEMA, table_name.as_str());
    let relation_identity = history_relation_id.as_str();
    let system_storage = system.contract.storage_schema();
    let mut fields = Vec::with_capacity(system_storage.fields().len() + 2);
    fields.push(history_field(
        system_storage.field(0).as_ref(),
        relation_identity,
        EPOCH_FIELD,
    ));
    fields.push(history_field(
        &Field::new(OBSERVATION_SET_FIELD, DataType::Binary, false),
        relation_identity,
        OBSERVATION_SET_FIELD,
    ));
    fields.push(history_field(
        &Field::new(ROW_ORDINAL_FIELD, DataType::Int64, false),
        relation_identity,
        ROW_ORDINAL_FIELD,
    ));
    for field in system_storage.fields().iter().skip(1) {
        fields.push(history_field(
            field.as_ref(),
            relation_identity,
            field.name(),
        ));
    }
    let schema = Arc::new(Schema::new_with_metadata(
        fields,
        HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            relation_identity.to_owned(),
        )]),
    ));
    let mappings = (0..schema.fields().len())
        .map(|index| FieldIndexMapping::direct(index, index))
        .collect();
    let history_contract = Arc::new(SchemaContract::try_new(
        HISTORY_SOURCE_IDENTITY,
        history_reference.clone(),
        Arc::clone(&schema),
        schema,
        mappings,
    )?);
    let view_transformation_id = ProgrammaticTransformationId::new(format!(
        "system.current_epoch_view.{}",
        system.table_reference.table()
    ));
    Ok(ObservationHistorySpec {
        system,
        history_relation_id,
        history_reference,
        history_contract,
        view_transformation_id,
    })
}

fn history_field(source: &Field, relation_identity: &str, field_name: &str) -> Arc<Field> {
    let mut metadata = source.metadata().clone();
    metadata.insert(
        FIELD_ID_METADATA_KEY.to_owned(),
        format!("{relation_identity}.{field_name}"),
    );
    Arc::new(source.clone().with_metadata(metadata))
}

async fn exact_history_provider(
    session: Arc<datafusion::execution::SessionState>,
    history: &ObservationHistorySpec,
    table: DeltaTable,
    pin: &ExactDeltaPin,
) -> Result<Arc<dyn TableProvider>, ProgrammaticObservationDeltaError> {
    let snapshot = ValidatedDeltaSnapshot::try_from_loaded_table(table, pin).map_err(|source| {
        ProgrammaticObservationDeltaError::ExactProvider {
            relation_id: history.system.relation_id.clone(),
            source,
        }
    })?;
    let raw = provider_from_validated_snapshot(pin, snapshot, session)
        .await
        .map_err(|source| ProgrammaticObservationDeltaError::ExactProvider {
            relation_id: history.system.relation_id.clone(),
            source,
        })?;
    Ok(Arc::new(
        SchemaContractStorageProvider::try_new(Arc::clone(&history.history_contract), raw)
            .map_err(
                |source| ProgrammaticObservationDeltaError::StorageProvider {
                    relation_id: history.system.relation_id.clone(),
                    source,
                },
            )?,
    ) as Arc<dyn TableProvider>)
}

fn current_epoch_view_plan(
    assembly: &ProgrammaticSchemaAssembly,
    history: &ObservationHistorySpec,
    provider: Arc<dyn TableProvider>,
    epoch_id: EpochId,
) -> Result<LogicalPlan, ProgrammaticObservationDeltaError> {
    let view_plan_error =
        |source: datafusion::error::DataFusionError| ProgrammaticObservationDeltaError::ViewPlan {
            relation_id: history.system.relation_id.clone(),
            detail: source.to_string(),
        };
    let scan = LogicalPlanBuilder::scan(
        history.history_reference.clone(),
        provider_as_source(provider),
        None,
    )
    .and_then(|builder| {
        builder.filter(
            col(EPOCH_FIELD).eq(lit(ScalarValue::Binary(Some(epoch_id.as_bytes().to_vec())))),
        )
    })
    .and_then(|builder| builder.sort(vec![col(ROW_ORDINAL_FIELD).sort(true, true)]))
    .map_err(&view_plan_error)?;
    let mut expressions =
        Vec::with_capacity(history.system.contract.logical_schema().fields().len());
    expressions
        .push(cast(col(EPOCH_FIELD), DataType::FixedSizeBinary(16)).alias(EPOCH_FIELD.to_owned()));
    expressions.extend(
        history
            .system
            .contract
            .logical_schema()
            .fields()
            .iter()
            .skip(1)
            .map(|field| col(field.name()).alias(field.name().to_owned())),
    );
    let projected = scan
        .project(expressions)
        .and_then(LogicalPlanBuilder::build)
        .map_err(view_plan_error)?;
    let analyzed = assembly.analyze_plan(&projected)?;
    observation_view_identity_boundary(
        analyzed,
        &history.system.relation_id,
        history.system.contract.as_ref(),
    )
    .map_err(Into::into)
}

fn build_history_batch(
    relation: &PreparedObservationRelation,
    observation_set_id: TransactionRef,
    write_schema: SchemaRef,
) -> Result<RecordBatch, ProgrammaticObservationDeltaError> {
    let storage = relation
        .contract
        .adapt_logical_batch_to_storage(&relation.batch)
        .map_err(|source| ProgrammaticObservationDeltaError::HistoryBatch {
            relation_id: relation.relation_id.clone(),
            detail: source.to_string(),
        })?;
    let row_count = storage.num_rows();
    let mut columns = Vec::with_capacity(storage.num_columns() + 2);
    columns.push(Arc::clone(storage.column(0)));
    columns.push(Arc::new(BinaryArray::from_iter_values(std::iter::repeat_n(
        observation_set_id.as_bytes().as_slice(),
        row_count,
    ))) as ArrayRef);
    columns.push(Arc::new(Int64Array::from_iter_values((0..row_count).map(
        |ordinal| i64::try_from(ordinal).expect("one Arrow batch cannot exceed i64 row ordinals"),
    ))) as ArrayRef);
    columns.extend(storage.columns().iter().skip(1).cloned());
    RecordBatch::try_new(write_schema, columns).map_err(|source| {
        ProgrammaticObservationDeltaError::HistoryBatch {
            relation_id: relation.relation_id.clone(),
            detail: source.to_string(),
        }
    })
}

fn materialization_evidence(
    relation: &PreparedObservationRelation,
    identity: ProgrammaticObservationWriteIdentity,
    policy: &ProgrammaticObservationHistoryPolicyEvidence,
) -> Result<ProgrammaticObservationMaterializationEvidence, ProgrammaticObservationDeltaError> {
    let row_count = u64::try_from(relation.batch.num_rows()).map_err(|source| {
        ProgrammaticObservationDeltaError::MaterializationEvidence {
            relation_id: relation.relation_id.clone(),
            detail: format!("row count is not representable as u64: {source}"),
        }
    })?;
    Ok(ProgrammaticObservationMaterializationEvidence {
        relation_id: relation.relation_id.clone(),
        epoch_id: identity.epoch_id,
        observation_set_id: identity.observation_set_id,
        row_count,
        empty: row_count == 0,
        schema_fingerprint: *policy.schema_fingerprint(),
        policy_fingerprint: *policy.policy_fingerprint(),
    })
}

async fn read_materialization_evidence(
    epoch_id: EpochId,
    registration: &ObservationHistorySpec,
    table: &DeltaTable,
    policy: &ProgrammaticObservationHistoryPolicyEvidence,
) -> Result<ProgrammaticObservationMaterializationEvidence, ProgrammaticObservationDeltaOpenError> {
    let relation_id = &registration.system.relation_id;
    let commit = read_exact_commit_info(table).await.map_err(|source| {
        materialization_metadata_error(
            relation_id,
            "commitInfo",
            format!("the exact loaded version has no readable commitInfo: {source}"),
        )
    })?;
    let observed_relation =
        materialization_string(relation_id, &commit.info, META_MATERIALIZATION_RELATION_ID)?;
    if observed_relation != relation_id.as_str() {
        return Err(materialization_metadata_error(
            relation_id,
            META_MATERIALIZATION_RELATION_ID,
            format!("observed {observed_relation:?}"),
        ));
    }
    let observed_epoch =
        materialization_hex::<16>(relation_id, &commit.info, META_MATERIALIZATION_EPOCH_ID)?;
    if observed_epoch != *epoch_id.as_bytes() {
        return Err(materialization_metadata_error(
            relation_id,
            META_MATERIALIZATION_EPOCH_ID,
            "commit names a different fabric epoch",
        ));
    }
    let observation_set_id = TransactionRef::from_bytes(materialization_hex::<32>(
        relation_id,
        &commit.info,
        META_MATERIALIZATION_SET_ID,
    )?);
    let row_count = commit
        .info
        .get(META_MATERIALIZATION_ROW_COUNT)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            materialization_metadata_error(
                relation_id,
                META_MATERIALIZATION_ROW_COUNT,
                "missing or not an unsigned integer",
            )
        })?;
    let empty = commit
        .info
        .get(META_MATERIALIZATION_EMPTY)
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            materialization_metadata_error(
                relation_id,
                META_MATERIALIZATION_EMPTY,
                "missing or not a boolean",
            )
        })?;
    if empty != (row_count == 0) {
        return Err(materialization_metadata_error(
            relation_id,
            META_MATERIALIZATION_EMPTY,
            format!("empty={empty} disagrees with row_count={row_count}"),
        ));
    }
    let schema_fingerprint =
        materialization_hex::<32>(relation_id, &commit.info, META_MATERIALIZATION_SCHEMA)?;
    if &schema_fingerprint != policy.schema_fingerprint() {
        return Err(materialization_metadata_error(
            relation_id,
            META_MATERIALIZATION_SCHEMA,
            "fingerprint differs from the exact registered storage schema",
        ));
    }
    let policy_fingerprint =
        materialization_hex::<32>(relation_id, &commit.info, META_MATERIALIZATION_POLICY)?;
    if &policy_fingerprint != policy.policy_fingerprint() {
        return Err(materialization_metadata_error(
            relation_id,
            META_MATERIALIZATION_POLICY,
            "fingerprint differs from the exact protocol/retention/checkpoint policy",
        ));
    }
    Ok(ProgrammaticObservationMaterializationEvidence {
        relation_id: relation_id.clone(),
        epoch_id,
        observation_set_id,
        row_count,
        empty,
        schema_fingerprint,
        policy_fingerprint,
    })
}

fn materialization_string<'a>(
    relation_id: &ProgrammaticRelationId,
    info: &'a HashMap<String, Value>,
    key: &'static str,
) -> Result<&'a str, ProgrammaticObservationDeltaOpenError> {
    info.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| materialization_metadata_error(relation_id, key, "missing or not a string"))
}

fn materialization_hex<const N: usize>(
    relation_id: &ProgrammaticRelationId,
    info: &HashMap<String, Value>,
    key: &'static str,
) -> Result<[u8; N], ProgrammaticObservationDeltaOpenError> {
    let encoded = materialization_string(relation_id, info, key)?;
    parse_lower_hex(encoded)
        .map_err(|detail| materialization_metadata_error(relation_id, key, detail))
}

fn materialization_metadata_error(
    relation_id: &ProgrammaticRelationId,
    key: &'static str,
    detail: impl Into<String>,
) -> ProgrammaticObservationDeltaOpenError {
    ProgrammaticObservationDeltaOpenError::MaterializationMetadata {
        relation_id: relation_id.clone(),
        key,
        detail: detail.into(),
    }
}

async fn verify_current_epoch_views(
    context: &datafusion::prelude::SessionContext,
    registry: &ProgrammaticObservationHistoryRegistry,
    expected: &BTreeMap<ProgrammaticRelationId, RecordBatch>,
    committed: &BTreeMap<ProgrammaticRelationId, ExactDeltaPin>,
) -> Result<(), ProgrammaticObservationDeltaError> {
    for (relation_id, expected_batch) in expected {
        if !committed.contains_key(relation_id) {
            return Err(ProgrammaticObservationDeltaError::MissingTarget {
                relation_id: relation_id.clone(),
            });
        }
        let spec = registry
            .history(relation_id)
            .map(|history| &history.system)
            .ok_or_else(|| ProgrammaticObservationDeltaError::MissingTarget {
                relation_id: relation_id.clone(),
            })?;
        let batches = context
            .table(spec.table_reference.clone())
            .await
            .map_err(|source| ProgrammaticObservationDeltaError::ViewReadback {
                relation_id: relation_id.clone(),
                detail: source.to_string(),
            })?
            .collect()
            .await
            .map_err(|source| ProgrammaticObservationDeltaError::ViewReadback {
                relation_id: relation_id.clone(),
                detail: source.to_string(),
            })?;
        let actual =
            concat_batches(expected_batch.schema_ref(), batches.iter()).map_err(|source| {
                ProgrammaticObservationDeltaError::ViewReadback {
                    relation_id: relation_id.clone(),
                    detail: source.to_string(),
                }
            })?;
        if &actual != expected_batch {
            return Err(ProgrammaticObservationDeltaError::ViewReadbackMismatch {
                relation_id: relation_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_history_policy(
    registration: &ObservationHistorySpec,
    table: &DeltaTable,
) -> Result<
    ProgrammaticObservationHistoryPolicyEvidence,
    ProgrammaticObservationDeltaConfigurationError,
> {
    let relation_id = &registration.system.relation_id;
    let snapshot = table.snapshot().map_err(|source| {
        ProgrammaticObservationDeltaConfigurationError::MissingSnapshot {
            relation_id: relation_id.clone(),
            source,
        }
    })?;
    let load_config = snapshot.load_config();
    if !load_config.require_files || load_config.skip_stats {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::ProviderLoadPolicy {
                relation_id: relation_id.clone(),
                require_files: load_config.require_files,
                skip_stats: load_config.skip_stats,
            },
        );
    }
    PROTOCOL
        .can_read_from(snapshot.snapshot())
        .map_err(
            |source| ProgrammaticObservationDeltaConfigurationError::ProtocolSupport {
                relation_id: relation_id.clone(),
                operation: "read",
                source,
            },
        )?;
    PROTOCOL
        .can_write_to(snapshot.snapshot())
        .map_err(
            |source| ProgrammaticObservationDeltaConfigurationError::ProtocolSupport {
                relation_id: relation_id.clone(),
                operation: "write",
                source,
            },
        )?;
    let protocol = snapshot.protocol();
    let min_reader_version = protocol.min_reader_version();
    let min_writer_version = protocol.min_writer_version();
    if min_reader_version != HISTORY_MIN_READER_VERSION
        || min_writer_version != HISTORY_MIN_WRITER_VERSION
    {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::ProtocolVersion {
                relation_id: relation_id.clone(),
                min_reader_version,
                min_writer_version,
            },
        );
    }
    let mut reader_features = protocol
        .reader_features()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut writer_features = protocol
        .writer_features()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    reader_features.sort();
    writer_features.sort();
    validate_protocol_feature_allowlist(
        relation_id,
        "reader",
        &reader_features,
        &ALLOWED_READER_FEATURES,
    )?;
    validate_protocol_feature_allowlist(
        relation_id,
        "writer",
        &writer_features,
        &ALLOWED_WRITER_FEATURES,
    )?;

    let configuration = snapshot.metadata().configuration();
    for (key, expected) in [
        (APPEND_ONLY_PROPERTY, "true"),
        (CDF_PROPERTY, "true"),
        (STATS_COLUMNS_PROPERTY, STATS_COLUMNS),
        (DELETION_VECTORS_PROPERTY, "false"),
        (LOG_RETENTION_PROPERTY, HISTORY_LOG_RETENTION),
        (
            DELETED_FILE_RETENTION_PROPERTY,
            HISTORY_DELETED_FILE_RETENTION,
        ),
        (CHECKPOINT_INTERVAL_PROPERTY, HISTORY_CHECKPOINT_INTERVAL),
        (EXPIRED_LOG_CLEANUP_PROPERTY, HISTORY_EXPIRED_LOG_CLEANUP),
    ] {
        let actual = configuration.get(key).cloned();
        if actual.as_deref() != Some(expected) {
            return Err(
                ProgrammaticObservationDeltaConfigurationError::RequiredTableProperty {
                    relation_id: relation_id.clone(),
                    key,
                    actual,
                },
            );
        }
    }
    if !snapshot.metadata().partition_columns().is_empty() {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::RequiredTableProperty {
                relation_id: relation_id.clone(),
                key: "partitionColumns",
                actual: Some(snapshot.metadata().partition_columns().join(",")),
            },
        );
    }
    validate_native_history_schema(
        relation_id,
        registration.history_contract.storage_schema(),
        &snapshot.snapshot().arrow_schema(),
    )?;

    let table_config = snapshot.table_config();
    let schema_fingerprint = schema_fingerprint(registration.history_contract.storage_schema());
    let protocol_fingerprint = digest_frames([
        b"codefabric.delta.protocol.v1".as_slice(),
        &min_reader_version.to_be_bytes(),
        &min_writer_version.to_be_bytes(),
        reader_features.join(",").as_bytes(),
        writer_features.join(",").as_bytes(),
    ]);
    let log_retention_seconds = table_config.log_retention_duration().as_secs();
    let deleted_file_retention_seconds = table_config.deleted_file_retention_duration().as_secs();
    let checkpoint_interval = table_config.checkpoint_interval().get();
    let expired_log_cleanup_enabled = table_config.enable_expired_log_cleanup();
    let policy_fingerprint = digest_frames([
        HISTORY_POLICY_ID.as_bytes(),
        &schema_fingerprint,
        &protocol_fingerprint,
        &log_retention_seconds.to_be_bytes(),
        &deleted_file_retention_seconds.to_be_bytes(),
        &checkpoint_interval.to_be_bytes(),
        &[u8::from(expired_log_cleanup_enabled)],
        &[u8::from(load_config.require_files)],
        &[u8::from(load_config.skip_stats)],
    ]);
    Ok(ProgrammaticObservationHistoryPolicyEvidence {
        schema_fingerprint,
        protocol_fingerprint,
        policy_fingerprint,
        min_reader_version,
        min_writer_version,
        reader_features: reader_features.into(),
        writer_features: writer_features.into(),
        log_retention_seconds,
        deleted_file_retention_seconds,
        checkpoint_interval,
        expired_log_cleanup_enabled,
        require_files: load_config.require_files,
        skip_stats: load_config.skip_stats,
    })
}

fn validate_protocol_feature_allowlist(
    relation_id: &ProgrammaticRelationId,
    side: &'static str,
    actual: &[String],
    allowed: &[&str],
) -> Result<(), ProgrammaticObservationDeltaConfigurationError> {
    if let Some(feature) = actual
        .iter()
        .find(|feature| !allowed.contains(&feature.as_str()))
    {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::UnsupportedProtocolFeature {
                relation_id: relation_id.clone(),
                side,
                feature: feature.clone(),
            },
        );
    }
    Ok(())
}

fn validate_native_history_schema(
    relation_id: &ProgrammaticRelationId,
    expected: &SchemaRef,
    actual: &SchemaRef,
) -> Result<(), ProgrammaticObservationDeltaConfigurationError> {
    if expected.fields().len() != actual.fields().len() {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::NativeStorageSchema {
                relation_id: relation_id.clone(),
                detail: format!(
                    "field count {} differs from expected {}",
                    actual.fields().len(),
                    expected.fields().len()
                ),
            },
        );
    }
    for (ordinal, (expected, actual)) in expected.fields().iter().zip(actual.fields()).enumerate() {
        if expected.name() != actual.name()
            || expected.data_type() != actual.data_type()
            || expected.is_nullable() != actual.is_nullable()
            || (!actual.metadata().is_empty() && actual.metadata() != expected.metadata())
        {
            return Err(
                ProgrammaticObservationDeltaConfigurationError::NativeStorageSchema {
                    relation_id: relation_id.clone(),
                    detail: format!(
                        "field {ordinal} differs: expected={expected:?}, actual={actual:?}"
                    ),
                },
            );
        }
    }
    if !actual.metadata().is_empty() && actual.metadata() != expected.metadata() {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::NativeStorageSchema {
                relation_id: relation_id.clone(),
                detail: "schema metadata differs from the registered history contract".to_owned(),
            },
        );
    }
    Ok(())
}

fn expected_relation_ids(
    registry: &ProgrammaticObservationHistoryRegistry,
) -> BTreeSet<ProgrammaticRelationId> {
    registry.relation_ids().cloned().collect()
}

fn validate_publication_relations(
    table_versions: &TableVersionSet,
    registry: &ProgrammaticObservationHistoryRegistry,
) -> Result<(), ProgrammaticObservationDeltaConfigurationError> {
    let expected = expected_relation_ids(registry);
    let actual = table_versions
        .components()
        .map(|(relation_id, _)| ProgrammaticRelationId::new(relation_id))
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::TargetSetMismatch { expected, actual },
        );
    }
    Ok(())
}

fn validate_materialization_relations(
    epoch_id: EpochId,
    materializations: &BTreeMap<
        ProgrammaticRelationId,
        ProgrammaticObservationMaterializationEvidence,
    >,
    registry: &ProgrammaticObservationHistoryRegistry,
) -> Result<(), ProgrammaticObservationDeltaConfigurationError> {
    let expected = expected_relation_ids(registry);
    let actual = materializations.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(
            ProgrammaticObservationDeltaConfigurationError::MaterializationSetMismatch {
                expected,
                actual,
            },
        );
    }
    for (relation_id, evidence) in materializations {
        if evidence.relation_id() != relation_id || evidence.epoch_id() != epoch_id {
            return Err(
                ProgrammaticObservationDeltaConfigurationError::MaterializationEpochMismatch {
                    relation_id: relation_id.clone(),
                },
            );
        }
        if evidence.is_empty() != (evidence.row_count() == 0) {
            return Err(
                ProgrammaticObservationDeltaConfigurationError::MaterializationReadbackMismatch,
            );
        }
    }
    Ok(())
}

fn failure(
    committed: &BTreeMap<ProgrammaticRelationId, ExactDeltaPin>,
    source: ProgrammaticObservationDeltaError,
) -> ProgrammaticObservationHistoricizationFailure {
    ProgrammaticObservationHistoricizationFailure {
        committed: committed.clone(),
        source,
    }
}

fn schema_fingerprint(schema: &SchemaRef) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    digest_frame(&mut hasher, b"codefabric.arrow-schema.v1");
    let mut schema_metadata = schema.metadata().iter().collect::<Vec<_>>();
    schema_metadata.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in schema_metadata {
        digest_frame(&mut hasher, key.as_bytes());
        digest_frame(&mut hasher, value.as_bytes());
    }
    for field in schema.fields() {
        digest_frame(&mut hasher, field.name().as_bytes());
        digest_frame(&mut hasher, format!("{:?}", field.data_type()).as_bytes());
        digest_frame(&mut hasher, &[u8::from(field.is_nullable())]);
        let mut metadata = field.metadata().iter().collect::<Vec<_>>();
        metadata.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (key, value) in metadata {
            digest_frame(&mut hasher, key.as_bytes());
            digest_frame(&mut hasher, value.as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

fn digest_frames<const N: usize>(frames: [&[u8]; N]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for frame in frames {
        digest_frame(&mut hasher, frame);
    }
    *hasher.finalize().as_bytes()
}

fn digest_frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn parse_lower_hex<const N: usize>(encoded: &str) -> Result<[u8; N], String> {
    if encoded.len() != N * 2 {
        return Err(format!(
            "expected {} lowercase hexadecimal characters, got {}",
            N * 2,
            encoded.len()
        ));
    }
    let mut output = [0_u8; N];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or_else(|| {
            format!(
                "invalid lowercase hexadecimal character at byte {}",
                index * 2
            )
        })?;
        let low = hex_nibble(pair[1]).ok_or_else(|| {
            format!(
                "invalid lowercase hexadecimal character at byte {}",
                index * 2 + 1
            )
        })?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use arrow_array::{Array as _, BinaryArray, FixedSizeBinaryArray, StringArray, UInt64Array};
    use datafusion::execution::SessionStateBuilder;
    use datafusion::physical_plan::collect;
    use datafusion::prelude::SessionContext;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use tempfile::TempDir;

    use super::*;
    use crate::fabric::epoch::{FABRIC_CATALOG, FabricEpochRuntimeConfig};
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };
    use crate::fabric::programmatic_schema::{
        DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID, RELATION_OBSERVATION_RELATION_ID,
        SCHEMA_OBSERVATION_RELATION_ID,
    };

    const TEST_OBSERVATION_RELATION_IDS: [&str; 5] = [
        RELATION_OBSERVATION_RELATION_ID,
        FIELD_OBSERVATION_RELATION_ID,
        SCHEMA_OBSERVATION_RELATION_ID,
        DEPENDENCY_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID,
    ];

    fn roots(temporary: &TempDir) -> BTreeMap<ProgrammaticRelationId, Url> {
        TEST_OBSERVATION_RELATION_IDS
            .into_iter()
            .map(|relation| {
                let path = temporary.path().join(relation.replace('.', "_"));
                fs::create_dir_all(&path).expect("create stable observation-history root");
                (
                    ProgrammaticRelationId::new(relation),
                    Url::from_directory_path(path).expect("history root is a file URL"),
                )
            })
            .collect()
    }

    fn write_identity(seed: u8) -> ProgrammaticObservationWriteIdentity {
        ProgrammaticObservationWriteIdentity::new(
            EpochId::from_bytes([seed; 16]),
            OperationId::from_bytes([seed.wrapping_add(0x20); 16]),
            WriterGeneration::new(u64::from(seed)).expect("test seed is nonzero"),
            TransactionRef::from_bytes([seed.wrapping_add(0x40); 32]),
        )
    }

    async fn current_relation_batches(epoch: &ProgrammaticFabricEpoch) -> Vec<RecordBatch> {
        epoch
            .context()
            .table(TableReference::full(
                FABRIC_CATALOG,
                "system",
                "programmatic_relation_observation",
            ))
            .await
            .expect("resolve current observation view")
            .collect()
            .await
            .expect("collect current observation view")
    }

    async fn history_relation_batches(epoch: &ProgrammaticFabricEpoch) -> Vec<RecordBatch> {
        epoch
            .context()
            .table(TableReference::full(
                FABRIC_CATALOG,
                HISTORY_SCHEMA,
                "programmatic_relation_observation_history",
            ))
            .await
            .expect("resolve exact history provider")
            .collect()
            .await
            .expect("collect exact history provider")
    }

    fn assert_current_epoch(batches: &[RecordBatch], expected: EpochId) {
        assert!(!batches.is_empty());
        let mut rows = 0;
        for batch in batches {
            let epochs = batch
                .column(0)
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .expect("current view restores fixed-width epoch identity");
            for index in 0..epochs.len() {
                assert!(!epochs.is_null(index));
                assert_eq!(epochs.value(index), expected.as_bytes());
                rows += 1;
            }
        }
        assert!(rows > 0);
    }

    #[tokio::test]
    async fn empty_observation_relation_commits_explicit_materialization_evidence() {
        let temporary = TempDir::new().expect("empty-history fixture root");
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_query_planner(DeltaPlanner::new())
            .build();
        let assembly = ProgrammaticSchemaAssembly::new(state);
        let registry = Arc::new(
            ProgrammaticObservationHistoryRegistry::try_from_assembly(&assembly)
                .expect("typed history registry"),
        );
        assert_eq!(registry.len(), TEST_OBSERVATION_RELATION_IDS.len());
        let mut targets =
            provision_programmatic_observation_histories(&assembly, roots(&temporary))
                .await
                .expect("provision registered histories");
        let relation_id = registry
            .relation_ids()
            .next()
            .expect("one registered history")
            .clone();
        let history = registry
            .history(&relation_id)
            .expect("registered history")
            .clone();
        let target = targets.remove(&relation_id).expect("exact history target");
        let identity = write_identity(0x31);
        let relation = PreparedObservationRelation {
            relation_id: relation_id.clone(),
            table_reference: history.system.table_reference.clone(),
            contract: Arc::clone(&history.system.contract),
            batch: RecordBatch::new_empty(Arc::clone(history.system.contract.logical_schema())),
        };
        let write_schema = target
            .table
            .snapshot()
            .expect("loaded history snapshot")
            .snapshot()
            .arrow_schema();
        let history_batch =
            build_history_batch(&relation, identity.observation_set_id(), write_schema)
                .expect("empty history batch");
        assert_eq!(history_batch.num_rows(), 0);
        let materialization =
            materialization_evidence(&relation, identity, target.policy_evidence())
                .expect("empty materialization evidence");
        assert!(materialization.is_empty());
        assert_eq!(materialization.row_count(), 0);

        let context = assembly.candidate_context();
        let session = Arc::new(assembly.candidate_state());
        let dataframe = context
            .read_batch(history_batch)
            .expect("empty history DataFrame");
        let plan = SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&session), dataframe)
            .expect("session-bound empty plan");
        let spec = ControlledDeltaWriteSpec::new(
            target.predecessor.clone(),
            identity.operation_id,
            identity.writer_generation,
            ApplicationTransactionMarker::from_transaction_ref(identity.observation_set_id()),
            ControlledDeltaWriteMode::Append,
        )
        .with_commit_metadata(materialization.commit_metadata())
        .expect("materialization commit metadata");
        let outcome = write_exact_delta_plan(&target.table, &spec, plan).await;
        let ControlledDeltaWriteOutcome::Committed(committed) = outcome else {
            panic!("empty materialization must commit one Delta version: {outcome:?}");
        };
        assert_eq!(committed.committed().version(), 1);
        assert_eq!(
            committed.commit_metadata(),
            &materialization.commit_metadata()
        );
        let committed_pin = committed.committed().clone();
        let committed_table = committed.into_table();
        let reopened_target =
            ProgrammaticObservationDeltaTarget::try_new(&history, committed_pin, committed_table)
                .expect("validate exact committed empty history");
        let readback = read_materialization_evidence(
            identity.epoch_id(),
            &history,
            &reopened_target.table,
            reopened_target.policy_evidence(),
        )
        .await
        .expect("read explicit empty evidence from exact commit");
        assert_eq!(readback, materialization);
    }

    #[tokio::test]
    async fn exact_reopen_rejects_retention_or_checkpoint_policy_drift() {
        let temporary = TempDir::new().expect("policy-drift fixture root");
        let assembly = ProgrammaticSchemaAssembly::new(SessionContext::new().state());
        let registry = ProgrammaticObservationHistoryRegistry::try_from_assembly(&assembly)
            .expect("typed history registry");
        let mut targets =
            provision_programmatic_observation_histories(&assembly, roots(&temporary))
                .await
                .expect("provision registered histories");
        let relation_id = registry
            .relation_ids()
            .next()
            .expect("one registered history")
            .clone();
        let history = registry.history(&relation_id).expect("registered history");
        let target = targets.remove(&relation_id).expect("exact history target");
        let drifted = target
            .table
            .set_tbl_properties()
            .with_properties(HashMap::from([(
                CHECKPOINT_INTERVAL_PROPERTY.to_owned(),
                "10".to_owned(),
            )]))
            .await
            .expect("commit checkpoint-policy drift fixture");
        let drifted_pin = ExactDeltaPin::new(drifted.table_url(), 1).expect("drifted exact pin");
        let error = ProgrammaticObservationDeltaTarget::try_new(history, drifted_pin, drifted)
            .expect_err("exact reopen must reject checkpoint-policy drift");
        assert!(matches!(
            error,
            ProgrammaticObservationDeltaConfigurationError::RequiredTableProperty {
                key: CHECKPOINT_INTERVAL_PROPERTY,
                actual: Some(actual),
                ..
            } if actual == "10"
        ));
    }

    #[tokio::test]
    async fn stable_delta_histories_reopen_exactly_and_expose_only_the_current_epoch() {
        let temporary = TempDir::new().expect("observation-history fixture root");

        let first_identity = write_identity(1);
        let first_builder = ProgrammaticFabricEpochBuilder::try_new(
            first_identity.epoch_id(),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("first candidate");
        let initial_targets = first_builder
            .provision_observation_histories(roots(&temporary))
            .await
            .expect("provision all registered programmatic histories");
        let first = first_builder
            .seal(first_identity, initial_targets)
            .await
            .expect("historicize first epoch");
        assert_eq!(
            first.observation_publication().table_versions().len(),
            TEST_OBSERVATION_RELATION_IDS.len()
        );
        assert_eq!(
            first.observation_publication().materializations().len(),
            TEST_OBSERVATION_RELATION_IDS.len()
        );
        assert!(
            first
                .observation_publication()
                .materializations()
                .all(|(_, evidence)| !evidence.is_empty() && evidence.row_count() > 0)
        );
        assert!(
            first
                .observation_publication()
                .table_versions()
                .all(|(_, pin)| pin.version() == 1)
        );
        let first_current = current_relation_batches(&first).await;
        assert_current_epoch(&first_current, first_identity.epoch_id());

        let selected_versions = Arc::clone(first.observation_publication().table_version_set());
        let first_session_id = first.context().state().session_id().to_owned();
        let reopened_builder = ProgrammaticFabricEpochBuilder::try_new(
            first_identity.epoch_id(),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("fresh restart candidate");
        let reopened = reopened_builder
            .reopen(selected_versions)
            .await
            .expect("reconstruct the selected epoch without a write");
        assert_ne!(
            reopened.context().state().session_id(),
            first_session_id,
            "restart must bind a fresh DataFusion session"
        );
        assert_eq!(
            reopened.observation_publication().table_version_set_ref(),
            first.observation_publication().table_version_set_ref()
        );
        let reopened_current = current_relation_batches(&reopened).await;
        assert_current_epoch(&reopened_current, first_identity.epoch_id());
        for (_, pin) in reopened.observation_publication().table_versions() {
            let latest = DeltaTableBuilder::from_url(pin.canonical_root().clone())
                .expect("construct latest-version inspection loader")
                .load()
                .await
                .expect("inspect history after no-write restart");
            assert_eq!(
                latest.version(),
                Some(pin.version()),
                "reconstruction must not advance a Delta history"
            );
        }

        let second_targets = first
            .observation_publication()
            .open_targets()
            .await
            .expect("reopen only the registered exact predecessor versions");
        for target in second_targets.targets.values() {
            let load_config = target
                .table
                .snapshot()
                .expect("exact target is loaded")
                .load_config();
            assert!(load_config.require_files);
            assert!(!load_config.skip_stats);
            let evidence = target.policy_evidence();
            assert_eq!(evidence.min_reader_version(), HISTORY_MIN_READER_VERSION);
            assert_eq!(evidence.min_writer_version(), HISTORY_MIN_WRITER_VERSION);
            assert!(evidence.reader_features().is_empty());
            assert!(!evidence.expired_log_cleanup_enabled());
            assert_eq!(evidence.checkpoint_interval(), 100);
            assert!(evidence.require_files());
            assert!(!evidence.skip_stats());
        }
        let second_identity = write_identity(2);
        let second_builder = ProgrammaticFabricEpochBuilder::try_new(
            second_identity.epoch_id(),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("second candidate");
        let second = second_builder
            .seal(second_identity, second_targets)
            .await
            .expect("historicize second epoch");
        assert!(
            second
                .observation_publication()
                .table_versions()
                .all(|(_, pin)| pin.version() == 2)
        );
        let second_current = current_relation_batches(&second).await;
        assert_current_epoch(&second_current, second_identity.epoch_id());

        let retained_first_targets = first
            .observation_publication()
            .open_targets()
            .await
            .expect("reopen retained first materialization after newer heads exist");
        assert!(
            retained_first_targets
                .targets
                .values()
                .all(|target| target.predecessor().version() == 1)
        );

        let historicized = history_relation_batches(&second).await;
        let mut first_rows = 0;
        let mut second_rows = 0;
        for batch in &historicized {
            let epochs = batch
                .column(0)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .expect("Delta history stores epoch identity as binary");
            for index in 0..epochs.len() {
                if epochs.value(index) == first_identity.epoch_id().as_bytes() {
                    first_rows += 1;
                } else if epochs.value(index) == second_identity.epoch_id().as_bytes() {
                    second_rows += 1;
                } else {
                    panic!("history contains an unselected epoch identity");
                }
            }
        }
        assert_eq!(first_rows, second_rows);
        assert!(first_rows > 0);

        let relation_id = ProgrammaticRelationId::new(RELATION_OBSERVATION_RELATION_ID);
        let pin = second
            .observation_publication()
            .table_version(&relation_id)
            .expect("relation-observation history pin");
        let table = DeltaTableBuilder::from_url(pin.canonical_root().clone())
            .expect("construct exact history loader")
            .with_version(pin.version())
            .load()
            .await
            .expect("load exact second history version");
        assert_eq!(table.history(None).await.unwrap().count(), 3);

        // CDF is a retention-bound incremental transport, not the authority.
        // The test uses explicit inclusive endpoints after proving version 2.
        let context = second.context();
        let state = context.state();
        let cdf = table
            .scan_cdf()
            .with_starting_version(1)
            .with_ending_version(2)
            .build(&state, None)
            .await
            .expect("build explicit bounded CDF scan");
        let changes = collect(cdf, context.task_ctx())
            .await
            .expect("collect bounded CDF scan");
        let mut versions = BTreeSet::new();
        let mut change_rows = 0;
        for batch in changes {
            let version_index = batch.schema().index_of("_commit_version").unwrap();
            let change_index = batch.schema().index_of("_change_type").unwrap();
            let commit_versions = batch
                .column(version_index)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .expect("CDF commit version is unsigned long");
            let change_types = batch
                .column(change_index)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("CDF change type is UTF-8");
            for index in 0..batch.num_rows() {
                versions.insert(commit_versions.value(index));
                assert_eq!(change_types.value(index), "insert");
                change_rows += 1;
            }
        }
        assert_eq!(versions, BTreeSet::from([1_u64, 2_u64]));
        assert_eq!(change_rows, first_rows + second_rows);
    }
}
