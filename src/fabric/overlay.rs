//! Immutable, generated-policy hot overlays and the durable three-snapshot rebase.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use arrow_array::{
    Array as _, ArrayRef, BinaryArray, Int16Array, Int64Array, RecordBatch, UInt32Array,
};
use arrow_row::{RowConverter, SortField};
use arrow_schema::{Field, Schema, SchemaRef};
use arrow_select::concat::concat_batches;
use arrow_select::take::take;
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::{Column, Constraints, Statistics};
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::execution::memory_pool::{GreedyMemoryPool, MemoryConsumer, MemoryReservation};
use datafusion::logical_expr::{
    Expr, JoinType, LogicalPlan, LogicalPlanBuilder, TableProviderFilterPushDown, TableType,
};
use datafusion::physical_plan::ExecutionPlan;

use super::snapshot_catalog::SnapshotOverlayProviderFactory;
use super::{FabricError, batch_checksum};
use crate::schema_registry::{OverlayMutationPolicy, TableSpec, table_spec};
use crate::snapshot::SnapshotOverlayTable;

#[cfg(feature = "daemon")]
use super::{
    MutationJournal, OwnerPublicationWrite, PublicationOutcome, PublicationRequest,
    SnapshotProviderCatalog, WorkspaceFabric,
};
#[cfg(feature = "daemon")]
use crate::operational_store::OperationalStore;
#[cfg(feature = "daemon")]
use crate::snapshot::ServingSnapshotManifestBody;
#[cfg(feature = "daemon")]
use crate::snapshot_runtime::{
    ServingSnapshotCandidate, ServingSnapshotRuntime, SnapshotRuntimeError,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MutationScope {
    Owner([u8; 16]),
    PrimaryKey(Vec<u8>),
    FullTable,
}

#[derive(Clone, Debug)]
enum MutationAction {
    OwnerReplacement {
        owner_id: [u8; 16],
        owner_bucket: i16,
        batch: Arc<RecordBatch>,
    },
    OwnerTombstone {
        owner_id: [u8; 16],
        owner_bucket: i16,
        reason_code: i16,
    },
    PrimaryKeyUpsert {
        encoded_key: Vec<u8>,
        key_row: Arc<RecordBatch>,
    },
    PrimaryKeyTombstone {
        encoded_key: Vec<u8>,
        key_row: Arc<RecordBatch>,
        reason_code: i16,
    },
    FullTableReplacement {
        batch: Arc<RecordBatch>,
    },
}

impl MutationAction {
    fn scope(&self) -> MutationScope {
        match self {
            Self::OwnerReplacement { owner_id, .. } | Self::OwnerTombstone { owner_id, .. } => {
                MutationScope::Owner(*owner_id)
            }
            Self::PrimaryKeyUpsert { encoded_key, .. }
            | Self::PrimaryKeyTombstone { encoded_key, .. } => {
                MutationScope::PrimaryKey(encoded_key.clone())
            }
            Self::FullTableReplacement { .. } => MutationScope::FullTable,
        }
    }

    const fn replacement(&self) -> Option<&Arc<RecordBatch>> {
        match self {
            Self::OwnerReplacement { batch, .. } | Self::FullTableReplacement { batch } => {
                Some(batch)
            }
            Self::PrimaryKeyUpsert { key_row, .. } => Some(key_row),
            Self::OwnerTombstone { .. } | Self::PrimaryKeyTombstone { .. } => None,
        }
    }

    const fn key_row(&self) -> Option<&Arc<RecordBatch>> {
        match self {
            Self::PrimaryKeyUpsert { key_row, .. } | Self::PrimaryKeyTombstone { key_row, .. } => {
                Some(key_row)
            }
            _ => None,
        }
    }
}

/// One closed, typed mutation admitted to the hot-overlay consolidator.
#[derive(Clone, Debug)]
pub struct OverlayMutation {
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    table_code: i16,
    source_generation: i64,
    action: MutationAction,
    payload_digest: [u8; 32],
}

impl OverlayMutation {
    /// Replace every effective row for the one owner encoded by a non-empty exact-schema batch.
    ///
    /// # Errors
    ///
    /// Rejects an unknown table, policy mismatch, mixed owner, schema drift, or generation fence.
    pub fn owner_replacement(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        batch: Arc<RecordBatch>,
    ) -> Result<Self, FabricError> {
        if batch.num_rows() == 0 {
            return Err(policy_error(
                table_code,
                "owner replacement cannot be empty",
            ));
        }
        let owner_id = fixed_id_column(&batch, "owner_id", 0)?;
        let owner_bucket = int16_value(&batch, "owner_bucket", 0)?;
        for row in 1..batch.num_rows() {
            if fixed_id_column(&batch, "owner_id", row)? != owner_id
                || int16_value(&batch, "owner_bucket", row)? != owner_bucket
            {
                return Err(policy_error(
                    table_code,
                    "one owner replacement batch contains multiple owners",
                ));
            }
        }
        Self::new(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            MutationAction::OwnerReplacement {
                owner_id,
                owner_bucket,
                batch,
            },
        )
    }

    /// Hide every base/older-overlay row for one owner.
    ///
    /// # Errors
    ///
    /// Rejects an unknown table or a policy mismatch.
    pub fn owner_tombstone(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        owner_id: [u8; 16],
        owner_bucket: i16,
        reason_code: i16,
    ) -> Result<Self, FabricError> {
        Self::new(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            MutationAction::OwnerTombstone {
                owner_id,
                owner_bucket,
                reason_code,
            },
        )
    }

    /// Upsert exactly one row selected by its generated primary-key ordering.
    ///
    /// # Errors
    ///
    /// Rejects a non-singleton row, schema drift, generation fence, or policy mismatch.
    pub fn primary_key_upsert(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        key_row: Arc<RecordBatch>,
    ) -> Result<Self, FabricError> {
        if key_row.num_rows() != 1 {
            return Err(policy_error(
                table_code,
                "primary-key upsert requires exactly one row",
            ));
        }
        let spec = resolve_spec(table_code)?;
        let encoded_key = encoded_primary_key(&key_row, spec, 0)?;
        Self::new(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            MutationAction::PrimaryKeyUpsert {
                encoded_key,
                key_row,
            },
        )
    }

    /// Hide one exact primary key while retaining its typed key row for effective anti-joins.
    ///
    /// # Errors
    ///
    /// Rejects a non-singleton key row, schema drift, generation fence, or policy mismatch.
    pub fn primary_key_tombstone(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        key_row: Arc<RecordBatch>,
        reason_code: i16,
    ) -> Result<Self, FabricError> {
        if key_row.num_rows() != 1 {
            return Err(policy_error(
                table_code,
                "primary-key tombstone requires exactly one key row",
            ));
        }
        let spec = resolve_spec(table_code)?;
        let encoded_key = encoded_primary_key(&key_row, spec, 0)?;
        Self::new(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            MutationAction::PrimaryKeyTombstone {
                encoded_key,
                key_row,
                reason_code,
            },
        )
    }

    /// Replace a complete workspace-global table. Partial replacement is not constructible.
    ///
    /// # Errors
    ///
    /// Rejects a partial batch, schema drift, generation fence, or policy mismatch.
    pub fn full_table_replacement(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        batch: Arc<RecordBatch>,
        complete: bool,
    ) -> Result<Self, FabricError> {
        if !complete {
            return Err(policy_error(
                table_code,
                "FULL_TABLE_REPLACE requires a formally complete table batch",
            ));
        }
        Self::new(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            MutationAction::FullTableReplacement { batch },
        )
    }

    fn new(
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        table_code: i16,
        source_generation: i64,
        action: MutationAction,
    ) -> Result<Self, FabricError> {
        if source_generation < 0 {
            return Err(policy_error(
                table_code,
                "source generation cannot be negative",
            ));
        }
        let spec = resolve_spec(table_code)?;
        validate_action_policy(spec, &action)?;
        if let Some(batch) = action.replacement().or_else(|| action.key_row()) {
            validate_replacement_batch(
                batch,
                spec,
                workspace_id,
                analysis_context_id,
                source_generation,
            )?;
        }
        let payload_digest = mutation_digest(
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            &action,
        )?;
        Ok(Self {
            workspace_id,
            analysis_context_id,
            table_code,
            source_generation,
            action,
            payload_digest,
        })
    }

    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }
}

/// Immutable exact-schema representation for one effective overlay table.
pub struct OverlayTable {
    table_code: i16,
    mutation_policy: OverlayMutationPolicy,
    replacement_batches: Arc<[Arc<RecordBatch>]>,
    owner_tombstones: Arc<RecordBatch>,
    key_tombstones: Arc<RecordBatch>,
    touched_keys: Arc<RecordBatch>,
    full_table_replacement: bool,
    min_source_generation: i64,
    max_source_generation: i64,
    primary_key_ordering: Arc<[String]>,
    content_digest: [u8; 32],
    row_digest: [u8; 32],
    tombstone_digest: [u8; 32],
    memory_bytes: usize,
}

impl fmt::Debug for OverlayTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OverlayTable")
            .field("table_code", &self.table_code)
            .field("mutation_policy", &self.mutation_policy)
            .field("replacement_batches", &self.replacement_batches.len())
            .field("owner_tombstones", &self.owner_tombstones.num_rows())
            .field("key_tombstones", &self.key_tombstones.num_rows())
            .field("touched_keys", &self.touched_keys.num_rows())
            .field("full_table_replacement", &self.full_table_replacement)
            .field(
                "generation_bounds",
                &(self.min_source_generation, self.max_source_generation),
            )
            .field("content_digest", &self.content_digest)
            .field("primary_key_ordering", &self.primary_key_ordering)
            .field("row_digest", &self.row_digest)
            .field("tombstone_digest", &self.tombstone_digest)
            .field("memory_bytes", &self.memory_bytes)
            .finish()
    }
}

impl OverlayTable {
    #[must_use]
    pub const fn table_code(&self) -> i16 {
        self.table_code
    }

    #[must_use]
    pub const fn mutation_policy(&self) -> OverlayMutationPolicy {
        self.mutation_policy
    }

    #[must_use]
    pub fn replacement_batches(&self) -> &[Arc<RecordBatch>] {
        &self.replacement_batches
    }

    #[must_use]
    pub fn owner_tombstones(&self) -> Arc<RecordBatch> {
        Arc::clone(&self.owner_tombstones)
    }

    #[must_use]
    pub fn key_tombstones(&self) -> Arc<RecordBatch> {
        Arc::clone(&self.key_tombstones)
    }

    #[must_use]
    pub const fn generation_bounds(&self) -> (i64, i64) {
        (self.min_source_generation, self.max_source_generation)
    }

    #[must_use]
    pub fn primary_key_ordering(&self) -> &[String] {
        &self.primary_key_ordering
    }

    #[must_use]
    pub const fn content_digest(&self) -> [u8; 32] {
        self.content_digest
    }

    #[must_use]
    pub const fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }

    fn replacement_batch(&self, schema: SchemaRef) -> RecordBatch {
        self.replacement_batches.first().map_or_else(
            || RecordBatch::new_empty(schema),
            |batch| batch.as_ref().clone(),
        )
    }
}

/// Inputs to one bounded deterministic consolidation operation.
#[derive(Clone, Copy)]
pub struct OverlayConsolidationRequest<'a> {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub overlay_generation: u64,
    pub prior: Option<&'a ConsolidatedOverlay>,
    pub incoming: &'a [OverlayMutation],
    pub memory_limit_bytes: usize,
}

/// One immutable overlay generation and the DataFusion reservation that owns its bytes.
pub struct ConsolidatedOverlay {
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    overlay_generation: u64,
    tables: BTreeMap<i16, Arc<OverlayTable>>,
    selected: BTreeMap<(i16, MutationScope), OverlayMutation>,
    checksum: [u8; 32],
    reservation: Arc<MemoryReservation>,
}

impl fmt::Debug for ConsolidatedOverlay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsolidatedOverlay")
            .field("workspace_id", &self.workspace_id)
            .field("analysis_context_id", &self.analysis_context_id)
            .field("overlay_generation", &self.overlay_generation)
            .field("tables", &self.tables.keys().collect::<Vec<_>>())
            .field("selected", &self.selected.len())
            .field("checksum", &self.checksum)
            .field("memory_bytes", &self.reservation.size())
            .finish()
    }
}

impl ConsolidatedOverlay {
    /// Apply the seven AC-G-22 rules under a DataFusion hard memory reservation.
    ///
    /// # Errors
    ///
    /// Rejects scope/policy/schema/generation conflicts, duplicate logical keys, and any
    /// reservation that exceeds the caller's hard memory limit.
    pub fn consolidate(request: OverlayConsolidationRequest<'_>) -> Result<Self, FabricError> {
        validate_consolidation_request(&request)?;
        let estimated = estimate_consolidation_bytes(request.prior, request.incoming)?;
        let pool: Arc<dyn datafusion::execution::memory_pool::MemoryPool> =
            Arc::new(GreedyMemoryPool::new(request.memory_limit_bytes));
        let reservation = Arc::new(
            MemoryConsumer::new(format!("overlay-{}", request.overlay_generation)).register(&pool),
        );
        reservation.try_grow(estimated).map_err(|error| {
            FabricError::OverlayMemoryReservation(format!(
                "requires {estimated} bytes within {}: {error}",
                request.memory_limit_bytes
            ))
        })?;

        let selected = select_mutations(&request)?;

        let mut grouped: BTreeMap<i16, Vec<OverlayMutation>> = BTreeMap::new();
        for mutation in selected.values() {
            grouped
                .entry(mutation.table_code)
                .or_default()
                .push(mutation.clone());
        }
        let tables = grouped
            .into_iter()
            .map(|(table_code, mutations)| {
                build_overlay_table(
                    request.workspace_id,
                    request.analysis_context_id,
                    request.overlay_generation,
                    table_code,
                    &mutations,
                )
                .map(|table| (table_code, Arc::new(table)))
            })
            .collect::<Result<BTreeMap<_, _>, FabricError>>()?;
        let exact = tables.values().try_fold(0_usize, |total, table| {
            total.checked_add(table.memory_bytes()).ok_or_else(|| {
                FabricError::OverlayMemoryReservation("overlay byte count overflow".into())
            })
        })?;
        reservation.try_resize(exact).map_err(|error| {
            FabricError::OverlayMemoryReservation(format!(
                "exact overlay requires {exact} bytes: {error}"
            ))
        })?;
        let checksum = overlay_checksum(request.overlay_generation, &tables);
        Ok(Self {
            workspace_id: request.workspace_id,
            analysis_context_id: request.analysis_context_id,
            overlay_generation: request.overlay_generation,
            tables,
            selected,
            checksum,
            reservation,
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }

    #[must_use]
    pub const fn analysis_context_id(&self) -> [u8; 16] {
        self.analysis_context_id
    }

    #[must_use]
    pub fn table(&self, table_code: i16) -> Option<Arc<OverlayTable>> {
        self.tables.get(&table_code).cloned()
    }

    #[must_use]
    pub fn tables(&self) -> impl ExactSizeIterator<Item = &Arc<OverlayTable>> {
        self.tables.values()
    }

    #[must_use]
    pub fn max_source_generation(&self) -> Option<i64> {
        self.selected
            .values()
            .map(|mutation| mutation.source_generation)
            .max()
    }

    #[must_use]
    pub fn payload_digests(&self) -> BTreeSet<[u8; 32]> {
        self.selected
            .values()
            .map(OverlayMutation::payload_digest)
            .collect()
    }

    /// Monotonic immutable overlay generation.
    #[must_use]
    pub const fn overlay_generation(&self) -> u64 {
        self.overlay_generation
    }

    /// Exact reserved Arrow bytes for this immutable generation.
    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        u64::try_from(self.reservation.size()).unwrap_or(u64::MAX)
    }

    /// Number of selected typed mutation scopes after deterministic consolidation.
    #[must_use]
    pub fn touched_scope_count(&self) -> u64 {
        u64::try_from(self.selected.len()).unwrap_or(u64::MAX)
    }

    /// Exact replacement and tombstone row count in the consolidated overlay.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        self.tables.values().fold(0_u64, |count, table| {
            count
                .saturating_add(
                    table
                        .replacement_batches
                        .iter()
                        .map(|batch| batch.num_rows() as u64)
                        .sum::<u64>(),
                )
                .saturating_add(table.owner_tombstones.num_rows() as u64)
                .saturating_add(table.key_tombstones.num_rows() as u64)
        })
    }

    #[cfg(any(feature = "daemon", test))]
    fn rebased_delta(
        &self,
        flush: &Self,
        durable_payload_digests: &BTreeSet<[u8; 32]>,
        memory_limit_bytes: usize,
    ) -> Result<Self, FabricError> {
        if !flush.payload_digests().is_subset(durable_payload_digests) {
            return Err(FabricError::OverlayRebaseRestartRequired(
                "durable base lacks a flushed logical-content digest".into(),
            ));
        }
        if let (Some(flush_max), Some(delta_min)) = (
            flush.max_source_generation(),
            self.selected
                .values()
                .map(|mutation| mutation.source_generation)
                .min(),
        ) && delta_min <= flush_max
        {
            return Err(FabricError::OverlayRebaseRestartRequired(
                "delta is not generation-fenced after the captured flush".into(),
            ));
        }
        let retained = self
            .selected
            .values()
            .filter(|mutation| !durable_payload_digests.contains(&mutation.payload_digest))
            .cloned()
            .collect::<Vec<_>>();
        Self::consolidate(OverlayConsolidationRequest {
            workspace_id: self.workspace_id,
            analysis_context_id: self.analysis_context_id,
            overlay_generation: self.overlay_generation,
            prior: None,
            incoming: &retained,
            memory_limit_bytes,
        })
    }
}

impl SnapshotOverlayProviderFactory for ConsolidatedOverlay {
    fn generation(&self) -> u64 {
        self.overlay_generation
    }

    fn checksum(&self) -> [u8; 32] {
        self.checksum
    }

    fn memory_bytes(&self) -> u64 {
        u64::try_from(self.reservation.size()).unwrap_or(u64::MAX)
    }

    fn table_manifests(&self) -> Vec<SnapshotOverlayTable> {
        self.tables
            .values()
            .map(|table| SnapshotOverlayTable {
                table_code: u16::try_from(table.table_code).expect("generated table code"),
                mutation_policy: policy_name(table.mutation_policy).into(),
                replacement_row_count: table
                    .replacement_batches
                    .iter()
                    .map(|batch| batch.num_rows() as u64)
                    .sum(),
                owner_tombstone_count: table.owner_tombstones.num_rows() as u64,
                key_tombstone_count: table.key_tombstones.num_rows() as u64,
                table_replacement: table.full_table_replacement,
                row_digest: framed_digest(table.row_digest),
                tombstone_digest: framed_digest(table.tombstone_digest),
            })
            .collect()
    }

    fn wrap(
        &self,
        spec: &TableSpec,
        base: Arc<dyn TableProvider>,
    ) -> Result<Arc<dyn TableProvider>, FabricError> {
        if base.schema() != spec.arrow_schema {
            return Err(policy_error(spec.table_code, "base provider schema drift"));
        }
        let plan = effective_plan(&base, spec, self.tables.get(&spec.table_code))?;
        Ok(Arc::new(OverlayEffectiveProvider {
            base,
            table_code: spec.table_code,
            plan,
        }))
    }
}

struct OverlayEffectiveProvider {
    base: Arc<dyn TableProvider>,
    table_code: i16,
    plan: LogicalPlan,
}

impl fmt::Debug for OverlayEffectiveProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OverlayEffectiveProvider")
            .field("table_code", &self.table_code)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl TableProvider for OverlayEffectiveProvider {
    fn schema(&self) -> SchemaRef {
        self.base.schema()
    }

    fn constraints(&self) -> Option<&Constraints> {
        self.base.constraints()
    }

    fn table_type(&self) -> TableType {
        TableType::View
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        let mut plan = LogicalPlanBuilder::from(self.plan.clone());
        if let Some(filter) = filters.iter().cloned().reduce(Expr::and) {
            plan = plan.filter(filter)?;
        }
        if let Some(projection) = projection {
            let identity = (0..plan.schema().fields().len()).collect::<Vec<_>>();
            if projection != &identity {
                let fields = projection
                    .iter()
                    .map(|index| {
                        Expr::Column(Column::from(self.plan.schema().qualified_field(*index)))
                    })
                    .collect::<Vec<_>>();
                plan = plan.project(fields)?;
            }
        }
        if let Some(limit) = limit {
            plan = plan.limit(0, Some(limit))?;
        }
        state.create_physical_plan(&plan.build()?).await
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
        Ok(vec![TableProviderFilterPushDown::Exact; filters.len()])
    }

    fn statistics(&self) -> Option<Statistics> {
        None
    }
}

fn validate_consolidation_request(
    request: &OverlayConsolidationRequest<'_>,
) -> Result<(), FabricError> {
    if let Some(prior) = request.prior
        && (prior.workspace_id != request.workspace_id
            || prior.analysis_context_id != request.analysis_context_id
            || request.overlay_generation <= prior.overlay_generation)
    {
        return Err(FabricError::OverlayGenerationConflict(
            "overlay scope changed or generation did not advance".into(),
        ));
    }
    Ok(())
}

fn select_mutations(
    request: &OverlayConsolidationRequest<'_>,
) -> Result<BTreeMap<(i16, MutationScope), OverlayMutation>, FabricError> {
    let mut selected = request
        .prior
        .map_or_else(BTreeMap::new, |prior| prior.selected.clone());
    let prior_max = request
        .prior
        .and_then(ConsolidatedOverlay::max_source_generation);
    let mut incoming = request.incoming.to_vec();
    incoming.sort_by(|left, right| {
        (
            left.source_generation,
            left.table_code,
            left.action.scope(),
            left.payload_digest,
        )
            .cmp(&(
                right.source_generation,
                right.table_code,
                right.action.scope(),
                right.payload_digest,
            ))
    });
    for mutation in incoming {
        validate_mutation_scope(&mutation, request)?;
        if let Some(generation) = prior_max
            && mutation.source_generation < generation
        {
            return Err(FabricError::OverlayGenerationConflict(format!(
                "stale generation {} precedes consolidated generation {generation}",
                mutation.source_generation
            )));
        }
        if matches!(mutation.action, MutationAction::FullTableReplacement { .. }) {
            selected.retain(|(table_code, _), prior| {
                *table_code != mutation.table_code
                    || prior.source_generation > mutation.source_generation
            });
        }
        let key = (mutation.table_code, mutation.action.scope());
        match selected.get(&key) {
            Some(prior) if prior.source_generation > mutation.source_generation => {
                return Err(FabricError::OverlayGenerationConflict(format!(
                    "stale mutation for table {}",
                    mutation.table_code
                )));
            }
            Some(prior) if prior.source_generation == mutation.source_generation => {
                if prior.payload_digest != mutation.payload_digest {
                    return Err(FabricError::OverlayGenerationConflict(format!(
                        "equal generation has conflicting payload for table {}",
                        mutation.table_code
                    )));
                }
            }
            _ => {
                selected.insert(key, mutation);
            }
        }
    }
    Ok(selected)
}

fn effective_plan(
    base: &Arc<dyn TableProvider>,
    spec: &TableSpec,
    overlay: Option<&Arc<OverlayTable>>,
) -> Result<LogicalPlan, FabricError> {
    let base_plan = || {
        LogicalPlanBuilder::scan(
            format!("base_{}", spec.table_code),
            provider_as_source(Arc::clone(base)),
            None,
        )
    };
    let Some(overlay) = overlay else {
        return Ok(base_plan()?.build()?);
    };
    let replacement = overlay.replacement_batch(Arc::clone(&spec.arrow_schema));
    let replacement_rows = replacement.num_rows();
    let replacement_provider: Arc<dyn TableProvider> = Arc::new(MemTable::try_new(
        Arc::clone(&spec.arrow_schema),
        vec![vec![replacement]],
    )?);
    if overlay.full_table_replacement {
        return Ok(LogicalPlanBuilder::scan(
            format!("overlay_{}", spec.table_code),
            provider_as_source(replacement_provider),
            None,
        )?
        .build()?);
    }
    let join_keys = match overlay.mutation_policy {
        OverlayMutationPolicy::OwnerReplace => vec!["owner_id"],
        OverlayMutationPolicy::PrimaryKeyUpsert => spec.primary_key.to_vec(),
        OverlayMutationPolicy::FullTableReplace => {
            return Err(policy_error(
                spec.table_code,
                "FULL_TABLE_REPLACE overlay lacks the table-replacement fence",
            ));
        }
        OverlayMutationPolicy::BaseImmutable | OverlayMutationPolicy::NotApplicable => {
            return Err(policy_error(
                spec.table_code,
                "immutable/non-applicable table acquired an overlay",
            ));
        }
    };
    let mut effective = base_plan()?;
    if overlay.touched_keys.num_rows() != 0 {
        let touched: Arc<dyn TableProvider> = Arc::new(MemTable::try_new(
            overlay.touched_keys.schema(),
            vec![vec![overlay.touched_keys.as_ref().clone()]],
        )?);
        let right = LogicalPlanBuilder::scan(
            format!("touched_{}", spec.table_code),
            provider_as_source(touched),
            None,
        )?
        .build()?;
        effective = effective.join(
            right,
            JoinType::LeftAnti,
            (join_keys.clone(), join_keys),
            None,
        )?;
    }
    if replacement_rows != 0 {
        let replacements = LogicalPlanBuilder::scan(
            format!("overlay_{}", spec.table_code),
            provider_as_source(replacement_provider),
            None,
        )?
        .build()?;
        effective = effective.union(replacements)?;
    }
    Ok(effective.build()?)
}

fn build_overlay_table(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    overlay_generation: u64,
    table_code: i16,
    mutations: &[OverlayMutation],
) -> Result<OverlayTable, FabricError> {
    let spec = resolve_spec(table_code)?;
    let replacement_inputs = mutations
        .iter()
        .filter_map(|mutation| mutation.action.replacement().map(AsRef::as_ref))
        .cloned()
        .collect::<Vec<_>>();
    let replacement = if replacement_inputs.is_empty() {
        RecordBatch::new_empty(Arc::clone(&spec.arrow_schema))
    } else {
        let combined = concat_batches(&spec.arrow_schema, &replacement_inputs)?;
        sort_and_dedup(&combined, spec.primary_key)?
    };
    let owner_tombstones = owner_tombstone_batch(
        workspace_id,
        analysis_context_id,
        overlay_generation,
        table_code,
        mutations,
    )?;
    let key_tombstones = key_tombstone_batch(
        workspace_id,
        analysis_context_id,
        overlay_generation,
        table_code,
        mutations,
    )?;
    let touched_keys = touched_key_batch(spec, mutations)?;
    let min_source_generation = mutations
        .iter()
        .map(|mutation| mutation.source_generation)
        .min()
        .ok_or_else(|| policy_error(table_code, "empty table mutation set"))?;
    let max_source_generation = mutations
        .iter()
        .map(|mutation| mutation.source_generation)
        .max()
        .expect("non-empty mutation set");
    let full_table_replacement = mutations
        .iter()
        .any(|mutation| matches!(mutation.action, MutationAction::FullTableReplacement { .. }));
    let row_digest = batch_checksum(&replacement)?;
    let tombstone_digest = tombstone_digest(&owner_tombstones, &key_tombstones)?;
    let content_digest = table_content_digest(
        table_code,
        spec.overlay_mutation,
        min_source_generation,
        max_source_generation,
        full_table_replacement,
        row_digest,
        tombstone_digest,
    );
    let memory_bytes = replacement
        .get_array_memory_size()
        .checked_add(owner_tombstones.get_array_memory_size())
        .and_then(|value| value.checked_add(key_tombstones.get_array_memory_size()))
        .and_then(|value| value.checked_add(touched_keys.get_array_memory_size()))
        .ok_or_else(|| FabricError::OverlayMemoryReservation("table byte count overflow".into()))?;
    let replacement_batches: Arc<[Arc<RecordBatch>]> = if replacement.num_rows() == 0 {
        Arc::from([])
    } else {
        Arc::from([Arc::new(replacement)])
    };
    Ok(OverlayTable {
        table_code,
        mutation_policy: spec.overlay_mutation,
        replacement_batches,
        owner_tombstones: Arc::new(owner_tombstones),
        key_tombstones: Arc::new(key_tombstones),
        touched_keys: Arc::new(touched_keys),
        full_table_replacement,
        min_source_generation,
        max_source_generation,
        primary_key_ordering: spec
            .primary_key
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
            .into(),
        content_digest,
        row_digest,
        tombstone_digest,
        memory_bytes,
    })
}

fn validate_mutation_scope(
    mutation: &OverlayMutation,
    request: &OverlayConsolidationRequest<'_>,
) -> Result<(), FabricError> {
    if mutation.workspace_id != request.workspace_id
        || mutation.analysis_context_id != request.analysis_context_id
    {
        return Err(policy_error(
            mutation.table_code,
            "mutation workspace/context differs from overlay scope",
        ));
    }
    let spec = resolve_spec(mutation.table_code)?;
    validate_action_policy(spec, &mutation.action)
}

fn validate_action_policy(spec: &TableSpec, action: &MutationAction) -> Result<(), FabricError> {
    let valid = matches!(
        (spec.overlay_mutation, action),
        (
            OverlayMutationPolicy::OwnerReplace,
            MutationAction::OwnerReplacement { .. } | MutationAction::OwnerTombstone { .. }
        ) | (
            OverlayMutationPolicy::PrimaryKeyUpsert,
            MutationAction::PrimaryKeyUpsert { .. } | MutationAction::PrimaryKeyTombstone { .. }
        ) | (
            OverlayMutationPolicy::FullTableReplace,
            MutationAction::FullTableReplacement { .. }
        )
    );
    valid.then_some(()).ok_or_else(|| {
        policy_error(
            spec.table_code,
            &format!(
                "generated policy {} rejects this mutation kind",
                policy_name(spec.overlay_mutation)
            ),
        )
    })
}

fn validate_replacement_batch(
    batch: &RecordBatch,
    spec: &TableSpec,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: i64,
) -> Result<(), FabricError> {
    if batch.schema() != spec.arrow_schema {
        return Err(policy_error(
            spec.table_code,
            "replacement schema is not exact",
        ));
    }
    for row in 0..batch.num_rows() {
        if batch.schema().index_of("workspace_id").is_ok()
            && fixed_id_column(batch, "workspace_id", row)? != workspace_id
        {
            return Err(policy_error(
                spec.table_code,
                "workspace generation fence failed",
            ));
        }
        if batch.schema().index_of("analysis_context_id").is_ok()
            && fixed_id_column(batch, "analysis_context_id", row)? != analysis_context_id
        {
            return Err(policy_error(
                spec.table_code,
                "context generation fence failed",
            ));
        }
        if batch.schema().index_of("source_generation").is_ok()
            && int64_value(batch, "source_generation", row)? != source_generation
        {
            return Err(policy_error(
                spec.table_code,
                "source generation fence failed",
            ));
        }
    }
    Ok(())
}

fn owner_tombstone_batch(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    overlay_generation: u64,
    table_code: i16,
    mutations: &[OverlayMutation],
) -> Result<RecordBatch, FabricError> {
    let spec = resolve_spec(900)?;
    let records = mutations
        .iter()
        .filter_map(|mutation| match mutation.action {
            MutationAction::OwnerTombstone {
                owner_id,
                owner_bucket,
                reason_code,
            } => Some((mutation, owner_id, owner_bucket, reason_code)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Ok(RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)));
    }
    let overlay_generation = i64::try_from(overlay_generation)
        .map_err(|_| policy_error(table_code, "overlay generation exceeds i64"))?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|_| Some(workspace_id.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|_| Some(analysis_context_id.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int64Array::from(
            records
                .iter()
                .map(|(mutation, ..)| mutation.source_generation)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int64Array::from(vec![overlay_generation; records.len()])),
        Arc::new(Int16Array::from(vec![table_code; records.len()])),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|(_, owner_id, ..)| Some(owner_id.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int16Array::from(
            records
                .iter()
                .map(|(_, _, bucket, _)| *bucket)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int16Array::from(
            records
                .iter()
                .map(|(_, _, _, reason)| *reason)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|(mutation, ..)| Some(mutation.payload_digest.as_slice()))
                .collect::<Vec<_>>(),
        )),
    ];
    sort_and_dedup(
        &RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns)?,
        spec.primary_key,
    )
}

fn key_tombstone_batch(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    overlay_generation: u64,
    table_code: i16,
    mutations: &[OverlayMutation],
) -> Result<RecordBatch, FabricError> {
    let spec = resolve_spec(901)?;
    let records = mutations
        .iter()
        .filter_map(|mutation| match &mutation.action {
            MutationAction::PrimaryKeyTombstone {
                encoded_key,
                reason_code,
                ..
            } => Some((mutation, primary_key_digest(encoded_key), *reason_code)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Ok(RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)));
    }
    let overlay_generation = i64::try_from(overlay_generation)
        .map_err(|_| policy_error(table_code, "overlay generation exceeds i64"))?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|_| Some(workspace_id.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|_| Some(analysis_context_id.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int64Array::from(
            records
                .iter()
                .map(|(mutation, ..)| mutation.source_generation)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int64Array::from(vec![overlay_generation; records.len()])),
        Arc::new(Int16Array::from(vec![table_code; records.len()])),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|(_, digest, _)| Some(digest.as_slice()))
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int16Array::from(
            records
                .iter()
                .map(|(_, _, reason)| *reason)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BinaryArray::from(
            records
                .iter()
                .map(|(mutation, ..)| Some(mutation.payload_digest.as_slice()))
                .collect::<Vec<_>>(),
        )),
    ];
    sort_and_dedup(
        &RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns)?,
        spec.primary_key,
    )
}

fn touched_key_batch(
    spec: &TableSpec,
    mutations: &[OverlayMutation],
) -> Result<RecordBatch, FabricError> {
    match spec.overlay_mutation {
        OverlayMutationPolicy::OwnerReplace => {
            let mut owners = mutations
                .iter()
                .filter_map(|mutation| match mutation.action {
                    MutationAction::OwnerReplacement { owner_id, .. }
                    | MutationAction::OwnerTombstone { owner_id, .. } => Some(owner_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            owners.sort_unstable();
            owners.dedup();
            let field = spec
                .arrow_schema
                .field_with_name("owner_id")
                .map_err(|_| policy_error(spec.table_code, "OWNER_REPLACE lacks owner_id"))?
                .clone();
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![field])),
                vec![Arc::new(BinaryArray::from(
                    owners
                        .iter()
                        .map(|owner| Some(owner.as_slice()))
                        .collect::<Vec<_>>(),
                ))],
            )
            .map_err(FabricError::from)
        }
        OverlayMutationPolicy::PrimaryKeyUpsert => {
            let rows = mutations
                .iter()
                .filter_map(|mutation| mutation.action.key_row().map(AsRef::as_ref))
                .cloned()
                .collect::<Vec<_>>();
            if rows.is_empty() {
                return empty_primary_key_batch(spec);
            }
            let combined = concat_batches(&spec.arrow_schema, &rows)?;
            let projected = primary_key_projection(&combined, spec)?;
            sort_and_dedup(&projected, spec.primary_key)
        }
        OverlayMutationPolicy::FullTableReplace
        | OverlayMutationPolicy::BaseImmutable
        | OverlayMutationPolicy::NotApplicable => {
            Ok(RecordBatch::new_empty(Arc::new(Schema::empty())))
        }
    }
}

fn empty_primary_key_batch(spec: &TableSpec) -> Result<RecordBatch, FabricError> {
    let fields = spec
        .primary_key
        .iter()
        .map(|name| {
            spec.arrow_schema
                .field_with_name(name)
                .cloned()
                .map_err(|_| policy_error(spec.table_code, "unknown primary-key field"))
        })
        .collect::<Result<Vec<Field>, FabricError>>()?;
    Ok(RecordBatch::new_empty(Arc::new(Schema::new(fields))))
}

fn primary_key_projection(
    batch: &RecordBatch,
    spec: &TableSpec,
) -> Result<RecordBatch, FabricError> {
    let indices = spec
        .primary_key
        .iter()
        .map(|name| {
            batch
                .schema()
                .index_of(name)
                .map_err(|_| policy_error(spec.table_code, "unknown primary-key field"))
        })
        .collect::<Result<Vec<_>, FabricError>>()?;
    let fields = indices
        .iter()
        .map(|&index| batch.schema().field(index).clone())
        .collect::<Vec<_>>();
    let columns = indices
        .iter()
        .map(|&index| Arc::clone(batch.column(index)))
        .collect::<Vec<_>>();
    Ok(RecordBatch::try_new(
        Arc::new(Schema::new(fields)),
        columns,
    )?)
}

fn sort_and_dedup(batch: &RecordBatch, key_names: &[&str]) -> Result<RecordBatch, FabricError> {
    if batch.num_rows() < 2 {
        return Ok(batch.clone());
    }
    let key_indices = key_names
        .iter()
        .map(|name| {
            batch.schema().index_of(name).map_err(|_| {
                FabricError::OverlayPolicyViolation(format!("unknown ordering field {name}"))
            })
        })
        .collect::<Result<Vec<_>, FabricError>>()?;
    let key_converter = RowConverter::new(
        key_indices
            .iter()
            .map(|&index| SortField::new(batch.schema().field(index).data_type().clone()))
            .collect(),
    )?;
    let key_rows = key_converter.convert_columns(
        &key_indices
            .iter()
            .map(|&index| Arc::clone(batch.column(index)))
            .collect::<Vec<_>>(),
    )?;
    let full_converter = RowConverter::new(
        batch
            .schema()
            .fields()
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect(),
    )?;
    let full_rows = full_converter.convert_columns(batch.columns())?;
    let mut rows = (0..batch.num_rows())
        .map(|index| {
            (
                key_rows.row(index).data().to_vec(),
                full_rows.row(index).data().to_vec(),
                index,
            )
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    let mut selected = Vec::with_capacity(rows.len());
    for index in 0..rows.len() {
        if index != 0 && rows[index - 1].0 == rows[index].0 {
            if rows[index - 1].1 != rows[index].1 {
                return Err(FabricError::OverlayGenerationConflict(
                    "duplicate primary key has distinct logical payload".into(),
                ));
            }
            continue;
        }
        selected.push(
            u32::try_from(rows[index].2).map_err(|_| {
                FabricError::OverlayMemoryReservation("row index exceeds u32".into())
            })?,
        );
    }
    let indices = UInt32Array::from(selected);
    let columns = batch
        .columns()
        .iter()
        .map(|column| take(column.as_ref(), &indices, None))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordBatch::try_new(batch.schema(), columns)?)
}

fn validate_batch_column<'a, T: 'static>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a T, FabricError> {
    let index = batch
        .schema()
        .index_of(name)
        .map_err(|_| FabricError::OverlayPolicyViolation(format!("missing column {name}")))?;
    batch
        .column(index)
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| FabricError::OverlayPolicyViolation(format!("invalid type for {name}")))
}

fn fixed_id_column(batch: &RecordBatch, name: &str, row: usize) -> Result<[u8; 16], FabricError> {
    let array = validate_batch_column::<BinaryArray>(batch, name)?;
    let value = array.value(row);
    value.try_into().map_err(|_| {
        FabricError::OverlayPolicyViolation(format!("{name} is not a 16-byte identity"))
    })
}

fn int16_value(batch: &RecordBatch, name: &str, row: usize) -> Result<i16, FabricError> {
    Ok(validate_batch_column::<Int16Array>(batch, name)?.value(row))
}

fn int64_value(batch: &RecordBatch, name: &str, row: usize) -> Result<i64, FabricError> {
    Ok(validate_batch_column::<Int64Array>(batch, name)?.value(row))
}

fn encoded_primary_key(
    batch: &RecordBatch,
    spec: &TableSpec,
    row: usize,
) -> Result<Vec<u8>, FabricError> {
    let indices = spec
        .primary_key
        .iter()
        .map(|name| {
            batch
                .schema()
                .index_of(name)
                .map_err(|_| policy_error(spec.table_code, "unknown primary-key field"))
        })
        .collect::<Result<Vec<_>, FabricError>>()?;
    let converter = RowConverter::new(
        indices
            .iter()
            .map(|&index| SortField::new(batch.schema().field(index).data_type().clone()))
            .collect(),
    )?;
    let rows = converter.convert_columns(
        &indices
            .iter()
            .map(|&index| Arc::clone(batch.column(index)))
            .collect::<Vec<_>>(),
    )?;
    Ok(rows.row(row).data().to_vec())
}

fn mutation_digest(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    table_code: i16,
    source_generation: i64,
    action: &MutationAction,
) -> Result<[u8; 32], FabricError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-overlay-mutation-v1\0");
    hasher.update(&workspace_id);
    hasher.update(&analysis_context_id);
    hasher.update(&table_code.to_be_bytes());
    hasher.update(&source_generation.to_be_bytes());
    match action {
        MutationAction::OwnerReplacement {
            owner_id,
            owner_bucket,
            batch,
        } => {
            hasher.update(b"owner-replacement\0");
            hasher.update(owner_id);
            hasher.update(&owner_bucket.to_be_bytes());
            hasher.update(&batch_checksum(batch)?);
        }
        MutationAction::OwnerTombstone {
            owner_id,
            owner_bucket,
            reason_code,
        } => {
            hasher.update(b"owner-tombstone\0");
            hasher.update(owner_id);
            hasher.update(&owner_bucket.to_be_bytes());
            hasher.update(&reason_code.to_be_bytes());
        }
        MutationAction::PrimaryKeyUpsert {
            encoded_key,
            key_row,
        } => {
            hasher.update(b"primary-key-upsert\0");
            hasher.update(encoded_key);
            hasher.update(&batch_checksum(key_row)?);
        }
        MutationAction::PrimaryKeyTombstone {
            encoded_key,
            reason_code,
            ..
        } => {
            hasher.update(b"primary-key-tombstone\0");
            hasher.update(encoded_key);
            hasher.update(&reason_code.to_be_bytes());
        }
        MutationAction::FullTableReplacement { batch } => {
            hasher.update(b"full-table-replacement\0");
            hasher.update(&batch_checksum(batch)?);
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

fn tombstone_digest(owner: &RecordBatch, key: &RecordBatch) -> Result<[u8; 32], FabricError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-overlay-tombstones-v1\0");
    hasher.update(&batch_checksum(owner)?);
    hasher.update(&batch_checksum(key)?);
    Ok(*hasher.finalize().as_bytes())
}

fn table_content_digest(
    table_code: i16,
    policy: OverlayMutationPolicy,
    minimum: i64,
    maximum: i64,
    full: bool,
    rows: [u8; 32],
    tombstones: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-overlay-table-v1\0");
    hasher.update(&table_code.to_be_bytes());
    hasher.update(policy_name(policy).as_bytes());
    hasher.update(&minimum.to_be_bytes());
    hasher.update(&maximum.to_be_bytes());
    hasher.update(&[u8::from(full)]);
    hasher.update(&rows);
    hasher.update(&tombstones);
    *hasher.finalize().as_bytes()
}

fn overlay_checksum(
    overlay_generation: u64,
    tables: &BTreeMap<i16, Arc<OverlayTable>>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-consolidated-overlay-v1\0");
    hasher.update(&overlay_generation.to_be_bytes());
    for (&table_code, table) in tables {
        hasher.update(&table_code.to_be_bytes());
        hasher.update(&table.content_digest);
    }
    *hasher.finalize().as_bytes()
}

fn primary_key_digest(encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-overlay-primary-key-v1\0");
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn estimate_consolidation_bytes(
    prior: Option<&ConsolidatedOverlay>,
    incoming: &[OverlayMutation],
) -> Result<usize, FabricError> {
    let prior_bytes = prior.map_or(0, |overlay| overlay.reservation.size());
    let input_bytes = incoming.iter().try_fold(0_usize, |total, mutation| {
        let bytes = mutation
            .action
            .replacement()
            .or_else(|| mutation.action.key_row())
            .map_or(0, |batch| batch.get_array_memory_size());
        total
            .checked_add(bytes)
            .and_then(|value| value.checked_add(512))
            .ok_or_else(|| FabricError::OverlayMemoryReservation("estimate overflow".into()))
    })?;
    prior_bytes
        .checked_add(input_bytes)
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| FabricError::OverlayMemoryReservation("estimate overflow".into()))
}

const fn policy_name(policy: OverlayMutationPolicy) -> &'static str {
    match policy {
        OverlayMutationPolicy::OwnerReplace => "OWNER_REPLACE",
        OverlayMutationPolicy::PrimaryKeyUpsert => "PRIMARY_KEY_UPSERT",
        OverlayMutationPolicy::FullTableReplace => "FULL_TABLE_REPLACE",
        OverlayMutationPolicy::BaseImmutable => "BASE_IMMUTABLE",
        OverlayMutationPolicy::NotApplicable => "NOT_APPLICABLE",
    }
}

fn framed_digest(digest: [u8; 32]) -> String {
    let mut framed = String::from("b3:");
    for byte in digest {
        write!(&mut framed, "{byte:02x}").expect("writing to a String cannot fail");
    }
    framed
}

fn resolve_spec(table_code: i16) -> Result<&'static TableSpec, FabricError> {
    table_spec(table_code).ok_or_else(|| policy_error(table_code, "unknown generated table code"))
}

fn policy_error(table_code: i16, detail: &str) -> FabricError {
    FabricError::OverlayPolicyViolation(format!("table {table_code}: {detail}"))
}

/// Registered crash boundaries for durable overlay rebase.
#[cfg(feature = "daemon")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayRebaseFaultPoint {
    AfterFlushCapture,
    AfterPublicationCas,
    BeforeSnapshotActivation,
}

#[cfg(feature = "daemon")]
impl OverlayRebaseFaultPoint {
    pub const ALL: [Self; 3] = [
        Self::AfterFlushCapture,
        Self::AfterPublicationCas,
        Self::BeforeSnapshotActivation,
    ];
}

/// All dependencies and predecessor observations for one three-snapshot rebase.
#[cfg(feature = "daemon")]
pub struct OverlayRebaseRequest<'a, J: MutationJournal> {
    pub fabric: &'a mut WorkspaceFabric,
    pub journal: &'a mut J,
    pub store: &'a mut OperationalStore,
    pub runtime: &'a ServingSnapshotRuntime,
    pub old_snapshot: Arc<ServingSnapshotCandidate>,
    pub publication_request: &'a PublicationRequest,
    pub publication_writes: &'a [OwnerPublicationWrite],
    pub flush: &'a ConsolidatedOverlay,
    pub delta: &'a ConsolidatedOverlay,
    pub durable_payload_digests: &'a BTreeSet<[u8; 32]>,
    pub candidate_body: ServingSnapshotManifestBody,
    pub memory_limit_bytes: usize,
    pub expected_active_pointer_generation: u64,
    pub now: u64,
    pub fault: Option<OverlayRebaseFaultPoint>,
}

/// Durable publication, rebased delta, and newly active immutable snapshot.
#[cfg(feature = "daemon")]
#[derive(Debug)]
pub struct OverlayRebaseOutcome {
    pub publication: PublicationOutcome,
    pub overlay: Arc<ConsolidatedOverlay>,
    pub snapshot: Arc<ServingSnapshotCandidate>,
}

#[cfg(feature = "daemon")]
impl ConsolidatedOverlay {
    /// Execute the AC-G-22 publication-CAS-rebase-validation-activation protocol.
    ///
    /// # Errors
    ///
    /// Rejects a changed predecessor, publication CAS failure, incomplete flush proof,
    /// effective-content mismatch, provider build failure, or activation conflict.
    pub async fn execute_rebase<J: MutationJournal>(
        request: OverlayRebaseRequest<'_, J>,
    ) -> Result<OverlayRebaseOutcome, FabricError> {
        let active = request.runtime.active().ok_or_else(|| {
            FabricError::OverlayRebaseRestartRequired("no active predecessor snapshot".into())
        })?;
        if !Arc::ptr_eq(&active, &request.old_snapshot) {
            return Err(FabricError::OverlayRebaseRestartRequired(
                "active snapshot changed before flush capture".into(),
            ));
        }
        inject_rebase_fault(request.fault, OverlayRebaseFaultPoint::AfterFlushCapture)?;
        let expected_effective = request.old_snapshot.providers().effective_state_digest();
        let publication = request
            .fabric
            .publish(
                request.journal,
                request.publication_request,
                request.publication_writes,
            )
            .await?;
        inject_rebase_fault(request.fault, OverlayRebaseFaultPoint::AfterPublicationCas)?;
        let overlay = Arc::new(request.delta.rebased_delta(
            request.flush,
            request.durable_payload_digests,
            request.memory_limit_bytes,
        )?);
        let providers =
            Arc::new(SnapshotProviderCatalog::build(&publication, overlay.as_ref()).await?);
        if providers.effective_state_digest() != expected_effective {
            return Err(FabricError::OverlayRebaseRestartRequired(
                "rebased effective content differs from the captured snapshot".into(),
            ));
        }
        let snapshot = Arc::new(
            ServingSnapshotCandidate::build(
                request.candidate_body,
                providers,
                request.old_snapshot.source_blob_digests(),
            )
            .map_err(|error| snapshot_runtime_error(&error))?,
        );
        inject_rebase_fault(
            request.fault,
            OverlayRebaseFaultPoint::BeforeSnapshotActivation,
        )?;
        let predecessor = request
            .old_snapshot
            .manifest()
            .raw_snapshot_id()
            .map_err(|error| FabricError::OverlayRebaseRestartRequired(error.to_string()))?;
        let durable_generation =
            u64::try_from(publication.pointer.pointer_generation).map_err(|_| {
                FabricError::OverlayRebaseRestartRequired(
                    "negative durable pointer generation".into(),
                )
            })?;
        request
            .runtime
            .activate(
                request.store,
                Arc::clone(&snapshot),
                Some(predecessor),
                request.expected_active_pointer_generation,
                durable_generation,
                request.now,
                None,
            )
            .map_err(|error| snapshot_runtime_error(&error))?;
        Ok(OverlayRebaseOutcome {
            publication,
            overlay,
            snapshot,
        })
    }
}

#[cfg(feature = "daemon")]
pub(super) fn inject_rebase_fault(
    selected: Option<OverlayRebaseFaultPoint>,
    current: OverlayRebaseFaultPoint,
) -> Result<(), FabricError> {
    if selected == Some(current) {
        return Err(FabricError::OverlayRebaseRestartRequired(format!(
            "injected rebase fault at {current:?}"
        )));
    }
    Ok(())
}

#[cfg(feature = "daemon")]
fn snapshot_runtime_error(error: &SnapshotRuntimeError) -> FabricError {
    FabricError::OverlayRebaseRestartRequired(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use arrow_array::{StringArray, TimestampMicrosecondArray};
    use datafusion::prelude::SessionContext;

    const WORKSPACE: [u8; 16] = [1; 16];
    const CONTEXT: [u8; 16] = [2; 16];
    const MEMORY_LIMIT: usize = 1 << 20;

    fn owner_batch(rows: &[(u8, i64, i64)]) -> Arc<RecordBatch> {
        let spec = table_spec(8).unwrap();
        let owner_ids = rows
            .iter()
            .map(|(owner, ..)| [*owner; 16])
            .collect::<Vec<_>>();
        let count = rows.len();
        let columns: Vec<ArrayRef> = vec![
            Arc::new(BinaryArray::from(vec![Some(WORKSPACE.as_slice()); count])),
            Arc::new(BinaryArray::from(vec![Some(CONTEXT.as_slice()); count])),
            Arc::new(Int64Array::from(
                rows.iter()
                    .map(|(_, generation, _)| *generation)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BinaryArray::from(
                owner_ids
                    .iter()
                    .map(|owner| Some(owner.as_slice()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; count])),
            Arc::new(Int16Array::from(
                rows.iter()
                    .map(|(owner, ..)| i16::from(*owner))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int16Array::from(vec![1; count])),
            Arc::new(Int16Array::from(vec![1; count])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; count])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; count])),
            Arc::new(Int64Array::from(vec![None::<i64>; count])),
            Arc::new(Int64Array::from(vec![None::<i64>; count])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; count])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; count])),
            Arc::new(Int64Array::from(
                rows.iter()
                    .map(|(_, _, payload)| *payload)
                    .collect::<Vec<_>>(),
            )),
        ];
        Arc::new(RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns).unwrap())
    }

    fn workspace_row(workspace: u8, revision: i64) -> Arc<RecordBatch> {
        let spec = table_spec(1).unwrap();
        let workspace_id = [workspace; 16];
        let authorization = [9_u8; 32];
        let columns: Vec<ArrayRef> = vec![
            Arc::new(BinaryArray::from(vec![Some(workspace_id.as_slice())])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(Int16Array::from(vec![1])),
            Arc::new(StringArray::from(vec![format!("workspace-{workspace}")])),
            Arc::new(BinaryArray::from(vec![Some(b"/workspace".as_slice())])),
            Arc::new(StringArray::from(vec!["/workspace"])),
            Arc::new(Int16Array::from(vec![1])),
            Arc::new(BinaryArray::from(vec![Some(authorization.as_slice())])),
            Arc::new(Int16Array::from(vec![1])),
            Arc::new(Int64Array::from(vec![revision])),
            Arc::new(TimestampMicrosecondArray::from(vec![revision]).with_timezone("UTC")),
            Arc::new(TimestampMicrosecondArray::from(vec![revision]).with_timezone("UTC")),
        ];
        Arc::new(RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns).unwrap())
    }

    fn consolidate(
        generation: u64,
        prior: Option<&ConsolidatedOverlay>,
        incoming: &[OverlayMutation],
    ) -> ConsolidatedOverlay {
        ConsolidatedOverlay::consolidate(OverlayConsolidationRequest {
            workspace_id: WORKSPACE,
            analysis_context_id: CONTEXT,
            overlay_generation: generation,
            prior,
            incoming,
            memory_limit_bytes: MEMORY_LIMIT,
        })
        .unwrap()
    }

    async fn effective_batch(base: Arc<RecordBatch>, overlay: &ConsolidatedOverlay) -> RecordBatch {
        let spec = table_spec(8).unwrap();
        let base: Arc<dyn TableProvider> = Arc::new(
            MemTable::try_new(
                Arc::clone(&spec.arrow_schema),
                vec![vec![base.as_ref().clone()]],
            )
            .unwrap(),
        );
        let provider = overlay.wrap(spec, base).unwrap();
        let batches = SessionContext::new()
            .read_table(provider)
            .unwrap()
            .collect()
            .await
            .unwrap();
        concat_batches(&spec.arrow_schema, &batches).unwrap()
    }

    #[tokio::test]
    async fn wp23_behavioral_acceptance() {
        let first = OverlayMutation::owner_replacement(
            WORKSPACE,
            CONTEXT,
            8,
            1,
            owner_batch(&[(1, 1, 11)]),
        )
        .unwrap();
        let second = OverlayMutation::owner_replacement(
            WORKSPACE,
            CONTEXT,
            8,
            2,
            owner_batch(&[(2, 2, 22)]),
        )
        .unwrap();
        let staged = consolidate(1, None, std::slice::from_ref(&first));
        let incremental = consolidate(2, Some(&staged), std::slice::from_ref(&second));
        let one_shot = consolidate(2, None, &[second.clone(), first.clone()]);
        assert_eq!(incremental.checksum(), one_shot.checksum());

        let base = owner_batch(&[(1, 0, 10), (2, 0, 20), (3, 0, 30)]);
        let old_effective = effective_batch(Arc::clone(&base), &incremental).await;
        let durable_after_flush = owner_batch(&[(1, 1, 11), (2, 0, 20), (3, 0, 30)]);
        let delta = consolidate(2, None, std::slice::from_ref(&second));
        let rebased = delta
            .rebased_delta(&staged, &staged.payload_digests(), MEMORY_LIMIT)
            .unwrap();
        let new_effective = effective_batch(durable_after_flush, &rebased).await;
        assert_eq!(
            batch_checksum(&old_effective).unwrap(),
            batch_checksum(&new_effective).unwrap()
        );
        assert_eq!(old_effective.num_rows(), 3);
    }

    #[test]
    fn wp23_structural_acceptance() {
        let replacement = OverlayMutation::owner_replacement(
            WORKSPACE,
            CONTEXT,
            8,
            3,
            owner_batch(&[(1, 3, 31), (1, 3, 31)]),
        )
        .unwrap();
        let tombstone =
            OverlayMutation::owner_tombstone(WORKSPACE, CONTEXT, 8, 2, [2; 16], 2, 7).unwrap();
        let overlay = consolidate(3, None, &[replacement, tombstone]);
        let table = overlay.table(8).unwrap();
        assert_eq!(
            table.replacement_batches()[0].schema(),
            table_spec(8).unwrap().arrow_schema
        );
        assert_eq!(table.replacement_batches()[0].num_rows(), 1);
        assert_eq!(
            table.owner_tombstones().schema(),
            table_spec(900).unwrap().arrow_schema
        );
        assert_eq!(
            table.key_tombstones().schema(),
            table_spec(901).unwrap().arrow_schema
        );
        assert_eq!(
            table.mutation_policy(),
            table_spec(8).unwrap().overlay_mutation
        );
        assert_eq!(
            table.primary_key_ordering(),
            table_spec(8).unwrap().primary_key
        );

        let key = workspace_row(1, 1);
        let key_overlay = consolidate(
            4,
            None,
            &[
                OverlayMutation::primary_key_upsert(WORKSPACE, CONTEXT, 1, 4, Arc::clone(&key))
                    .unwrap(),
                OverlayMutation::primary_key_tombstone(WORKSPACE, CONTEXT, 1, 5, key, 8).unwrap(),
            ],
        );
        assert_eq!(key_overlay.table(1).unwrap().key_tombstones().num_rows(), 1);
    }

    #[test]
    fn wp23_negative_zero_state() {
        let left = OverlayMutation::owner_replacement(
            WORKSPACE,
            CONTEXT,
            8,
            1,
            owner_batch(&[(1, 1, 10)]),
        )
        .unwrap();
        let right = OverlayMutation::owner_replacement(
            WORKSPACE,
            CONTEXT,
            8,
            1,
            owner_batch(&[(1, 1, 11)]),
        )
        .unwrap();
        assert!(matches!(
            ConsolidatedOverlay::consolidate(OverlayConsolidationRequest {
                workspace_id: WORKSPACE,
                analysis_context_id: CONTEXT,
                overlay_generation: 1,
                prior: None,
                incoming: &[left.clone(), right],
                memory_limit_bytes: MEMORY_LIMIT,
            }),
            Err(FabricError::OverlayGenerationConflict(_))
        ));
        assert!(matches!(
            ConsolidatedOverlay::consolidate(OverlayConsolidationRequest {
                workspace_id: WORKSPACE,
                analysis_context_id: CONTEXT,
                overlay_generation: 1,
                prior: None,
                incoming: &[left],
                memory_limit_bytes: 0,
            }),
            Err(FabricError::OverlayMemoryReservation(_))
        ));
        assert!(
            OverlayMutation::full_table_replacement(
                WORKSPACE,
                CONTEXT,
                1,
                1,
                workspace_row(1, 1),
                false,
            )
            .is_err()
        );
        let immutable = table_spec(11).unwrap();
        assert_eq!(
            immutable.overlay_mutation,
            OverlayMutationPolicy::BaseImmutable
        );
        assert!(
            OverlayMutation::primary_key_upsert(
                WORKSPACE,
                CONTEXT,
                11,
                1,
                Arc::new(RecordBatch::new_empty(Arc::clone(&immutable.arrow_schema))),
            )
            .is_err()
        );
        let flush = consolidate(
            1,
            None,
            &[OverlayMutation::owner_tombstone(WORKSPACE, CONTEXT, 8, 1, [1; 16], 1, 1).unwrap()],
        );
        let delta = consolidate(
            2,
            None,
            &[OverlayMutation::owner_tombstone(WORKSPACE, CONTEXT, 8, 2, [2; 16], 2, 1).unwrap()],
        );
        assert!(
            delta
                .rebased_delta(&flush, &BTreeSet::new(), MEMORY_LIMIT)
                .is_err()
        );
    }

    #[test]
    fn wp23_operational_acceptance() {
        let overlay = consolidate(
            1,
            None,
            &[OverlayMutation::owner_tombstone(WORKSPACE, CONTEXT, 8, 1, [1; 16], 1, 1).unwrap()],
        );
        assert!(overlay.memory_bytes() > 0);
        assert_eq!(overlay.memory_bytes(), overlay.reservation.size() as u64);
        assert_eq!(overlay.table_manifests().len(), 1);
        assert_eq!(OverlayRebaseFaultPoint::ALL.len(), 3);
    }
}
