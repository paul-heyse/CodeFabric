//! Generated-policy Delta mutations with coordinator-journal recovery.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array as _, BinaryArray, RecordBatch};
use arrow_row::{RowConverter, SortField};
use arrow_schema::Schema;
use arrow_select::concat::concat_batches;
use datafusion::common::ScalarValue;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::SessionContext;
use deltalake::kernel::{Transaction, transaction::CommitProperties};
use deltalake::protocol::SaveMode;
use serde_json::Value;

use super::{DeltaAccessProfile, DeltaHandleFactory, FabricError, WorkspaceFabric, exact_provider};
use crate::fact_ingest::{FactBatchScope, ValidatedFactBatch};
use crate::identity::{IdentityDomain, encode_public_id};
use crate::schema_registry::{DurableMutationClass, TableSpec, table_spec, table_specs};

/// Closed independently retryable Delta mutation phases.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MutationPhase {
    OwnerDelete,
    OwnerAppend,
    PublicationAppend,
    PublicationTransition,
    SingletonUpsert,
}

/// Closed writer operation selected from the generated durable mutation class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DurableWriteKind {
    BootstrapReplace,
    CurrentPointerSwap,
    OwnerReplace,
    PublicationAppend,
    DerivedOwnerReplace,
    GlobalDerivedReplace,
}

pub(super) const fn write_kind(class: DurableMutationClass) -> DurableWriteKind {
    match class {
        DurableMutationClass::StaticDimension => DurableWriteKind::BootstrapReplace,
        DurableMutationClass::CurrentSingleton => DurableWriteKind::CurrentPointerSwap,
        DurableMutationClass::OwnerReplacedFact => DurableWriteKind::OwnerReplace,
        DurableMutationClass::PublicationAppend => DurableWriteKind::PublicationAppend,
        DurableMutationClass::DerivedOwnerReplaced => DurableWriteKind::DerivedOwnerReplace,
        DurableMutationClass::GlobalDerivedReplacement => DurableWriteKind::GlobalDerivedReplace,
    }
}

pub(super) fn enforce_write_kind(
    spec: &TableSpec,
    attempted: DurableWriteKind,
) -> Result<(), FabricError> {
    let required = write_kind(spec.durable_mutation);
    if required == attempted {
        Ok(())
    } else {
        Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: format!(
                "generated mutation policy requires {required:?}, received {attempted:?}"
            ),
        })
    }
}

impl MutationPhase {
    /// Stable metadata/journal spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerDelete => "owner-delete",
            Self::OwnerAppend => "owner-append",
            Self::PublicationAppend => "publication-append",
            Self::PublicationTransition => "publication-transition",
            Self::SingletonUpsert => "singleton-upsert",
        }
    }
}

/// Complete immutable identity of one journaled Delta phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationPhaseSpec {
    pub operation_id: [u8; 16],
    pub publication_id: [u8; 16],
    pub table_code: i16,
    pub phase: MutationPhase,
    pub application_id: String,
    pub owner_set_fingerprint: [u8; 32],
    pub input_checksum: [u8; 32],
    pub expected_output_checksum: [u8; 32],
    pub expected_predecessor: Option<u64>,
}

/// Journal result after exact-field reconciliation and monotonic allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedMutation {
    pub spec: MutationPhaseSpec,
    pub application_version: i64,
    pub committed_delta_version: Option<u64>,
}

/// Coordinator-owned durable operation journal used by the sole writer.
pub trait MutationJournal {
    /// Return the exact existing phase or durably allocate its next application version.
    ///
    /// # Errors
    ///
    /// Returns a stable journal conflict or persistence diagnostic.
    fn prepare(&mut self, spec: &MutationPhaseSpec) -> Result<PreparedMutation, String>;

    /// Atomically mark the exact prepared phase committed at one Delta version.
    ///
    /// # Errors
    ///
    /// Returns a stable journal conflict or persistence diagnostic.
    fn mark_committed(
        &mut self,
        prepared: &PreparedMutation,
        delta_version: u64,
    ) -> Result<(), String>;
}

/// One owner replacement request; the validated batch is passed separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerMutationRequest {
    pub scope: FactBatchScope,
    pub publication_id: [u8; 16],
    pub operation_id: [u8; 16],
    pub table_code: i16,
    pub owner_ids: Vec<[u8; 16]>,
    pub expected_predecessor: Option<u64>,
}

/// Registered deterministic crash/conflict seams at the write boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationFaultPoint {
    AfterDeleteCommit,
    BeforeAppendCommit,
}

impl MutationFaultPoint {
    /// Closed fault registry used by recovery tests.
    pub const ALL: [Self; 2] = [Self::AfterDeleteCommit, Self::BeforeAppendCommit];
}

/// Exact committed result for publication-table assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationResult {
    pub table_code: i16,
    pub delete_version: Option<u64>,
    pub append_version: Option<u64>,
    pub deleted_rows: Option<usize>,
    pub final_row_count: usize,
    pub final_checksum: [u8; 32],
}

pub(super) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn owner_fingerprint(owners: &[[u8; 16]]) -> [u8; 32] {
    let mut hasher =
        crate::identity::semantic_fingerprint(crate::identity::SemanticFingerprintDomain::OwnerSet);
    for owner in owners {
        hasher.update(owner);
    }
    hasher.finalize()
}

/// Stable schema-and-row checksum independent of input row order.
///
/// # Errors
///
/// Returns an Arrow row-encoding error for a generated type unsupported by the
/// pinned Arrow version.
pub fn batch_checksum(batch: &RecordBatch) -> Result<[u8; 32], FabricError> {
    let fields = batch
        .schema()
        .fields()
        .iter()
        .map(|field| SortField::new(field.data_type().clone()))
        .collect();
    let converter = RowConverter::new(fields)?;
    let rows = converter.convert_columns(batch.columns())?;
    let mut ordered = rows.iter().map(|row| row.data()).collect::<Vec<_>>();
    ordered.sort_unstable();
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::ArrowBatch,
    );
    if let Some(digest) = batch
        .schema()
        .metadata()
        .get("com.codefabric.cpg.schema_digest")
    {
        hasher.update(digest.as_bytes());
    }
    hasher.update(&(batch.num_rows() as u64).to_be_bytes());
    for row in ordered {
        hasher.update(&(row.len() as u64).to_be_bytes());
        hasher.update(row);
    }
    Ok(hasher.finalize())
}

/// Stable checksum of the generated primary-key projection for one table batch.
///
/// # Errors
///
/// Returns an Arrow/schema error when the batch does not implement the generated
/// primary-key contract.
pub(super) fn primary_key_checksum(
    batch: &RecordBatch,
    spec: &TableSpec,
) -> Result<[u8; 32], FabricError> {
    let indices = spec
        .primary_key
        .iter()
        .map(|name| batch.schema().index_of(name))
        .collect::<Result<Vec<_>, _>>()?;
    let schema = Arc::new(batch.schema().project(&indices)?);
    let columns = indices
        .into_iter()
        .map(|index| Arc::clone(batch.column(index)))
        .collect();
    batch_checksum(&RecordBatch::try_new(schema, columns)?)
}

pub(super) fn application_id(
    workspace_id: [u8; 16],
    table_code: i16,
    phase: MutationPhase,
) -> Result<String, FabricError> {
    let workspace = encode_public_id(IdentityDomain::Workspace, None, workspace_id)?;
    Ok(format!(
        "codefabric/{workspace}/{table_code}/{}",
        phase.as_str()
    ))
}

fn metadata(prepared: &PreparedMutation) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "publication_id".into(),
            Value::String(hex(&prepared.spec.publication_id)),
        ),
        (
            "operation_id".into(),
            Value::String(hex(&prepared.spec.operation_id)),
        ),
        ("table_code".into(), Value::from(prepared.spec.table_code)),
        (
            "mutation_phase".into(),
            Value::String(prepared.spec.phase.as_str().into()),
        ),
        (
            "application_id".into(),
            Value::String(prepared.spec.application_id.clone()),
        ),
        (
            "application_version".into(),
            Value::from(prepared.application_version),
        ),
        (
            "owner_set_fingerprint".into(),
            Value::String(hex(&prepared.spec.owner_set_fingerprint)),
        ),
        (
            "input_checksum".into(),
            Value::String(hex(&prepared.spec.input_checksum)),
        ),
        (
            "expected_output_checksum".into(),
            Value::String(hex(&prepared.spec.expected_output_checksum)),
        ),
        (
            "expected_predecessor".into(),
            prepared
                .spec
                .expected_predecessor
                .map_or(Value::Null, Value::from),
        ),
    ])
}

pub(super) fn commit_properties(prepared: &PreparedMutation) -> CommitProperties {
    CommitProperties::default()
        .with_max_retries(0)
        .with_metadata(metadata(prepared))
        .with_application_transaction(Transaction::new(
            &prepared.spec.application_id,
            prepared.application_version,
        ))
}

fn owner_predicate(scope: FactBatchScope, owners: &[[u8; 16]]) -> Expr {
    let owner = owners
        .iter()
        .map(|owner| col("owner_id").eq(lit(ScalarValue::Binary(Some(owner.to_vec())))))
        .reduce(Expr::or)
        .expect("validated non-empty owner set");
    col("workspace_id")
        .eq(lit(ScalarValue::Binary(Some(scope.workspace_id.to_vec()))))
        .and(col("analysis_context_id").eq(lit(ScalarValue::Binary(Some(
            scope.analysis_context_id.to_vec(),
        )))))
        .and(owner)
}

fn validate_request(
    request: &OwnerMutationRequest,
    batch: Option<&ValidatedFactBatch>,
) -> Result<(&'static TableSpec, Vec<[u8; 16]>), FabricError> {
    let spec = table_spec(request.table_code).ok_or_else(|| FabricError::TableInvariant {
        table: request.table_code.to_string(),
        detail: "mutation table is not generated".into(),
    })?;
    let requested_kind = match spec.durable_mutation {
        DurableMutationClass::OwnerReplacedFact => DurableWriteKind::OwnerReplace,
        DurableMutationClass::DerivedOwnerReplaced => DurableWriteKind::DerivedOwnerReplace,
        _ => {
            return Err(FabricError::TableInvariant {
                table: spec.name.into(),
                detail: "generated durable mutation class is not owner-replaced".into(),
            });
        }
    };
    enforce_write_kind(spec, requested_kind)?;
    if spec.arrow_schema.index_of("owner_id").is_err() {
        return Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "generated durable mutation class is not owner-replaced".into(),
        });
    }
    let owners = request.owner_ids.iter().copied().collect::<BTreeSet<_>>();
    if owners.is_empty() || owners.len() != request.owner_ids.len() {
        return Err(FabricError::MutationConflict(
            "owner set must be non-empty and unique".into(),
        ));
    }
    let owners = owners.into_iter().collect::<Vec<_>>();
    if let Some(batch) = batch {
        if batch.table_code() != request.table_code
            || batch.batch().schema() != spec.arrow_schema
            || batch.scope().batch_scope() != request.scope
        {
            return Err(FabricError::MutationConflict(
                "validated batch/table/scope mismatch".into(),
            ));
        }
        let owner_index = spec
            .arrow_schema
            .index_of("owner_id")
            .expect("checked owner column");
        let workspace_index = spec
            .arrow_schema
            .index_of("workspace_id")
            .expect("generated workspace column");
        let batch_owners = batch
            .batch()
            .column(owner_index)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .expect("generated owner binary");
        let batch_workspaces = batch
            .batch()
            .column(workspace_index)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .expect("generated workspace binary");
        if batch_owners.iter().flatten().any(|owner| {
            <[u8; 16]>::try_from(owner).map_or(true, |owner| owners.binary_search(&owner).is_err())
        }) || batch_workspaces
            .iter()
            .flatten()
            .any(|workspace| workspace != request.scope.workspace_id)
        {
            return Err(FabricError::MutationConflict(
                "batch contains an undeclared owner or workspace".into(),
            ));
        }
    }
    Ok((spec, owners))
}

pub(super) fn phase_spec(
    request: &OwnerMutationRequest,
    owners: &[[u8; 16]],
    phase: MutationPhase,
    input_checksum: [u8; 32],
    expected_output_checksum: [u8; 32],
    expected_predecessor: Option<u64>,
) -> Result<MutationPhaseSpec, FabricError> {
    Ok(MutationPhaseSpec {
        operation_id: request.operation_id,
        publication_id: request.publication_id,
        table_code: request.table_code,
        phase,
        application_id: application_id(request.scope.workspace_id, request.table_code, phase)?,
        owner_set_fingerprint: owner_fingerprint(owners),
        input_checksum,
        expected_output_checksum,
        expected_predecessor,
    })
}

pub(super) async fn reload_table(
    table: &mut super::FabricTable,
    profile: DeltaAccessProfile,
) -> Result<(), FabricError> {
    if profile != DeltaAccessProfile::OptimizeDml || profile.skip_stats() {
        return Err(FabricError::SnapshotProviderIntegrity(
            "mutable Delta reload requires the OPTIMIZE_DML full-statistics profile".into(),
        ));
    }
    table.delta = DeltaHandleFactory::open(&table.path.to_string_lossy(), None, profile)
        .await?
        .into_table();
    table.provider = exact_provider(
        &table.delta,
        table_spec(table.table_code).expect("opened generated table"),
        DeltaAccessProfile::QueryServing,
    )
    .await?;
    Ok(())
}

async fn transaction_version(
    table: &super::FabricTable,
    application_id: &str,
) -> Result<Option<i64>, FabricError> {
    Ok(table
        .delta
        .snapshot()?
        .transaction_version(&table.delta.log_store(), application_id)
        .await?)
}

async fn commit_metadata_matches(
    table: &super::FabricTable,
    prepared: &PreparedMutation,
) -> Result<bool, FabricError> {
    let expected = metadata(prepared);
    let history = table.delta.history(None).await?.collect::<Vec<_>>();
    Ok(history.iter().any(|commit| {
        expected
            .iter()
            .all(|(key, value)| commit.info.get(key) == Some(value))
    }))
}

pub(super) async fn reconcile_prepared<J: MutationJournal>(
    table: &mut super::FabricTable,
    journal: &mut J,
    prepared: &PreparedMutation,
) -> Result<Option<u64>, FabricError> {
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    let observed = transaction_version(table, &prepared.spec.application_id).await?;
    if let Some(committed) = prepared.committed_delta_version {
        // delta-rs deliberately elides a predicate-delete commit when no file
        // changes. The coordinator's exact record is then the durable no-op
        // proof; every material Delta commit still requires the matching txn
        // action and commit metadata.
        let no_op_delete = prepared.spec.phase == MutationPhase::OwnerDelete
            && Some(committed) == prepared.spec.expected_predecessor;
        if !no_op_delete
            && (observed.is_none_or(|version| version < prepared.application_version)
                || !commit_metadata_matches(table, prepared).await?)
        {
            return Err(FabricError::MutationConflict(
                "journal commit does not match Delta transaction metadata".into(),
            ));
        }
        return Ok(Some(committed));
    }
    match observed {
        Some(version) if version == prepared.application_version => {
            if !commit_metadata_matches(table, prepared).await? {
                return Err(FabricError::MutationConflict(
                    "Delta application version belongs to another operation".into(),
                ));
            }
            let delta_version = table.delta.version().ok_or_else(|| {
                FabricError::MutationConflict("committed Delta table has no version".into())
            })?;
            journal
                .mark_committed(prepared, delta_version)
                .map_err(FabricError::MutationJournal)?;
            Ok(Some(delta_version))
        }
        Some(version) if version > prepared.application_version => {
            Err(FabricError::MutationConflict(format!(
                "application version advanced to {version} before recovery of {}",
                prepared.application_version
            )))
        }
        _ => Ok(None),
    }
}

async fn owner_batch(
    table: &super::FabricTable,
    spec: &TableSpec,
    scope: FactBatchScope,
    owners: &[[u8; 16]],
) -> Result<RecordBatch, FabricError> {
    let batches = SessionContext::new()
        .read_table(Arc::clone(&table.provider))?
        .filter(owner_predicate(scope, owners))?
        .collect()
        .await?;
    Ok(concat_batches(&spec.arrow_schema, &batches)?)
}

pub(super) fn storage_batch(batch: &RecordBatch) -> Result<RecordBatch, FabricError> {
    let fields = batch.schema().fields().clone();
    let columns = batch.columns().to_vec();
    Ok(RecordBatch::try_new(
        Arc::new(Schema::new(fields)),
        columns,
    )?)
}

async fn delete_phase<J: MutationJournal>(
    table: &mut super::FabricTable,
    journal: &mut J,
    spec: MutationPhaseSpec,
    scope: FactBatchScope,
    owners: &[[u8; 16]],
) -> Result<(Option<u64>, Option<usize>, bool), FabricError> {
    let prepared = journal
        .prepare(&spec)
        .map_err(FabricError::MutationJournal)?;
    if let Some(version) = reconcile_prepared(table, journal, &prepared).await? {
        return Ok((Some(version), None, true));
    }
    if table.delta.version() != prepared.spec.expected_predecessor {
        return Err(FabricError::MutationConflict(
            "delete predecessor changed before commit".into(),
        ));
    }
    let (delta, metrics) = table
        .delta
        .clone()
        .delete()
        .with_predicate(owner_predicate(scope, owners))
        .with_commit_properties(commit_properties(&prepared))
        .await?;
    table.delta = delta;
    table.provider = exact_provider(
        &table.delta,
        table_spec(table.table_code).expect("generated mutation table"),
        DeltaAccessProfile::QueryServing,
    )
    .await?;
    let version = table.delta.version().ok_or_else(|| {
        FabricError::MutationConflict("delete commit returned no Delta version".into())
    })?;
    journal
        .mark_committed(&prepared, version)
        .map_err(FabricError::MutationJournal)?;
    Ok((Some(version), metrics.num_deleted_rows, false))
}

pub(super) async fn append_phase<J: MutationJournal>(
    table: &mut super::FabricTable,
    journal: &mut J,
    spec: MutationPhaseSpec,
    batch: &RecordBatch,
) -> Result<Option<u64>, FabricError> {
    let prepared = journal
        .prepare(&spec)
        .map_err(FabricError::MutationJournal)?;
    if let Some(version) = reconcile_prepared(table, journal, &prepared).await? {
        return Ok(Some(version));
    }
    if table.delta.version() != prepared.spec.expected_predecessor {
        return Err(FabricError::MutationConflict(
            "append predecessor changed before commit".into(),
        ));
    }
    let delta = table
        .delta
        .clone()
        .write([storage_batch(batch)?])
        .with_save_mode(SaveMode::Append)
        .with_commit_properties(commit_properties(&prepared))
        .await?;
    table.delta = delta;
    table.provider = exact_provider(
        &table.delta,
        table_spec(table.table_code).expect("generated mutation table"),
        DeltaAccessProfile::QueryServing,
    )
    .await?;
    let version = table.delta.version().ok_or_else(|| {
        FabricError::MutationConflict("append commit returned no Delta version".into())
    })?;
    journal
        .mark_committed(&prepared, version)
        .map_err(FabricError::MutationJournal)?;
    Ok(Some(version))
}

impl WorkspaceFabric {
    /// Replace the complete row set for declared owners through the generated policy.
    ///
    /// # Errors
    ///
    /// Rejects mutation-class, owner, predecessor, journal, Delta transaction,
    /// checksum, row-count, schema, or optimistic-concurrency drift.
    pub async fn replace_owner_rows<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        request: &OwnerMutationRequest,
        batch: &ValidatedFactBatch,
    ) -> Result<MutationResult, FabricError> {
        self.replace_owner_rows_with_fault(journal, request, batch, None)
            .await
    }

    async fn replace_owner_rows_with_fault<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        request: &OwnerMutationRequest,
        batch: &ValidatedFactBatch,
        fault: Option<MutationFaultPoint>,
    ) -> Result<MutationResult, FabricError> {
        let (spec, owners) = validate_request(request, Some(batch))?;
        let input_checksum = batch_checksum(batch.batch())?;
        let empty_checksum =
            batch_checksum(&RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)))?;
        let table = self.tables.get_mut(&request.table_code).ok_or_else(|| {
            FabricError::TableInvariant {
                table: spec.name.into(),
                detail: "workspace table is absent".into(),
            }
        })?;
        reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
        let delete = phase_spec(
            request,
            &owners,
            MutationPhase::OwnerDelete,
            input_checksum,
            empty_checksum,
            request.expected_predecessor,
        )?;
        let (delete_version, deleted_rows, delete_replayed) =
            delete_phase(table, journal, delete, request.scope, &owners).await?;
        reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
        if !delete_replayed {
            let deleted_batch = owner_batch(table, spec, request.scope, &owners).await?;
            if deleted_batch.num_rows() != 0 || batch_checksum(&deleted_batch)? != empty_checksum {
                return Err(FabricError::MutationConflict(
                    "owner delete read-back is not empty".into(),
                ));
            }
            if fault == Some(MutationFaultPoint::AfterDeleteCommit) {
                return Err(FabricError::MutationFault(
                    MutationFaultPoint::AfterDeleteCommit,
                ));
            }
        }
        let append_predecessor = delete_version.or(table.delta.version());
        let append = phase_spec(
            request,
            &owners,
            MutationPhase::OwnerAppend,
            input_checksum,
            input_checksum,
            append_predecessor,
        )?;
        if fault == Some(MutationFaultPoint::BeforeAppendCommit) {
            let _prepared = journal
                .prepare(&append)
                .map_err(FabricError::MutationJournal)?;
            return Err(FabricError::MutationFault(
                MutationFaultPoint::BeforeAppendCommit,
            ));
        }
        let append_version = append_phase(table, journal, append, batch.batch()).await?;
        reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
        let final_batch = owner_batch(table, spec, request.scope, &owners).await?;
        let final_checksum = batch_checksum(&final_batch)?;
        if final_batch.num_rows() != batch.num_rows() || final_checksum != input_checksum {
            return Err(FabricError::MutationConflict(
                "owner replacement read-back differs from validated input".into(),
            ));
        }
        Ok(MutationResult {
            table_code: request.table_code,
            delete_version,
            append_version,
            deleted_rows,
            final_row_count: final_batch.num_rows(),
            final_checksum,
        })
    }

    /// Remove owners from every generated owner-scoped durable table.
    ///
    /// # Errors
    ///
    /// Rejects an empty/duplicate owner set or any journal, Delta, generated-policy,
    /// optimistic-concurrency, or read-back failure.
    pub async fn remove_owners<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        scope: FactBatchScope,
        publication_id: [u8; 16],
        operation_id: [u8; 16],
        owner_ids: Vec<[u8; 16]>,
    ) -> Result<BTreeMap<i16, MutationResult>, FabricError> {
        let owners = owner_ids.iter().copied().collect::<BTreeSet<_>>();
        if owners.is_empty() || owners.len() != owner_ids.len() {
            return Err(FabricError::MutationConflict(
                "owner set must be non-empty and unique".into(),
            ));
        }
        let owners = owners.into_iter().collect::<Vec<_>>();
        let table_codes = table_specs()
            .iter()
            .filter(|spec| {
                matches!(
                    spec.durable_mutation,
                    DurableMutationClass::OwnerReplacedFact
                        | DurableMutationClass::DerivedOwnerReplaced
                ) && spec.arrow_schema.index_of("owner_id").is_ok()
            })
            .map(|spec| spec.table_code)
            .collect::<Vec<_>>();
        let mut results = BTreeMap::new();
        for table_code in table_codes {
            let spec = table_spec(table_code).ok_or_else(|| FabricError::TableInvariant {
                table: table_code.to_string(),
                detail: "generated owner table disappeared".into(),
            })?;
            let table =
                self.tables
                    .get_mut(&table_code)
                    .ok_or_else(|| FabricError::TableInvariant {
                        table: spec.name.into(),
                        detail: "bootstrapped owner table is absent".into(),
                    })?;
            reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
            let existing = owner_batch(table, spec, scope, &owners).await?;
            let input_checksum = batch_checksum(&existing)?;
            let empty_checksum =
                batch_checksum(&RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)))?;
            let request = OwnerMutationRequest {
                scope,
                publication_id,
                operation_id,
                table_code,
                owner_ids: owners.clone(),
                expected_predecessor: table.delta.version(),
            };
            let delete = phase_spec(
                &request,
                &owners,
                MutationPhase::OwnerDelete,
                input_checksum,
                empty_checksum,
                request.expected_predecessor,
            )?;
            let (delete_version, deleted_rows, _) =
                delete_phase(table, journal, delete, scope, &owners).await?;
            reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
            let final_batch = owner_batch(table, spec, scope, &owners).await?;
            if final_batch.num_rows() != 0 || batch_checksum(&final_batch)? != empty_checksum {
                return Err(FabricError::MutationConflict(format!(
                    "owner removal read-back differs for {}",
                    spec.name
                )));
            }
            results.insert(
                table_code,
                MutationResult {
                    table_code,
                    delete_version,
                    append_version: None,
                    deleted_rows,
                    final_row_count: 0,
                    final_checksum: empty_checksum,
                },
            );
        }
        Ok(results)
    }
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::fact_ingest::{EntityRow, FactScope, ValidatedFactBatch, encode_entities};
    use crate::operational_store::OperationalStore;
    use crate::registries::WorkspaceRegistryLifecycle;
    use crate::workspace_registry::WorkspaceRecord;

    fn workspace_record() -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: [1; 16],
            workspace_registration_nonce: [2; 16],
            registration_revision: 1,
            administrative_key: vec![3],
            root_path_bytes: b"/workspace".to_vec(),
            root_path_display: "/workspace".into(),
            root_directory_file_identity: vec![4],
            platform_code: 2,
            case_sensitivity_mode: "sensitive".into(),
            authorization_revision: 1,
            allowed_source_disclosure_rules: Vec::new(),
            repository_id: None,
            worktree_id: None,
            authorization_fingerprint: [5; 32],
            context_fingerprint: [6; 32],
            status: WorkspaceRegistryLifecycle::Bootstrapping,
            created_at: "00000000000000001000".into(),
            updated_at: "00000000000000001000".into(),
        }
    }

    fn scope() -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: [2; 16],
            source_generation: 7,
            owner_id: [3; 16],
        }
    }

    fn validated_entity(entity_id: [u8; 16]) -> ValidatedFactBatch {
        let row = EntityRow {
            scope: scope(),
            entity_id,
            language: 10,
            entity_family_code: 1,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: None,
            start_byte: Some(0),
            end_byte: Some(0),
            name: Some("entity".into()),
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: i64::from(entity_id[0]),
        };
        ValidatedFactBatch::validate(100, encode_entities(&[row]).unwrap(), scope()).unwrap()
    }

    fn request(operation: u8, predecessor: Option<u64>, table_code: i16) -> OwnerMutationRequest {
        OwnerMutationRequest {
            scope: scope().batch_scope(),
            publication_id: [9; 16],
            operation_id: [operation; 16],
            table_code,
            owner_ids: vec![[3; 16]],
            expected_predecessor: predecessor,
        }
    }

    #[derive(Clone, Default)]
    struct MemoryJournal {
        state: Arc<Mutex<MemoryJournalState>>,
    }

    #[derive(Default)]
    struct MemoryJournalState {
        records: BTreeMap<([u8; 16], i16, MutationPhase), PreparedMutation>,
        versions: BTreeMap<String, i64>,
    }

    impl MutationJournal for MemoryJournal {
        fn prepare(&mut self, spec: &MutationPhaseSpec) -> Result<PreparedMutation, String> {
            let mut state = self.state.lock().map_err(|_| "poisoned journal")?;
            let key = (spec.operation_id, spec.table_code, spec.phase);
            if let Some(prepared) = state.records.get(&key) {
                if prepared.spec != *spec {
                    return Err("operation identity was reused".into());
                }
                return Ok(prepared.clone());
            }
            if state.records.values().any(|prepared| {
                prepared.spec.application_id == spec.application_id
                    && prepared.spec.expected_predecessor == spec.expected_predecessor
                    && prepared.spec.operation_id != spec.operation_id
            }) {
                return Err("application predecessor is already claimed".into());
            }
            let version = state
                .versions
                .entry(spec.application_id.clone())
                .or_default();
            *version += 1;
            let prepared = PreparedMutation {
                spec: spec.clone(),
                application_version: *version,
                committed_delta_version: None,
            };
            state.records.insert(key, prepared.clone());
            Ok(prepared)
        }

        fn mark_committed(
            &mut self,
            prepared: &PreparedMutation,
            delta_version: u64,
        ) -> Result<(), String> {
            let mut state = self.state.lock().map_err(|_| "poisoned journal")?;
            let key = (
                prepared.spec.operation_id,
                prepared.spec.table_code,
                prepared.spec.phase,
            );
            let record = state.records.get_mut(&key).ok_or("record absent")?;
            if record.application_version != prepared.application_version
                || record.spec != prepared.spec
                || record
                    .committed_delta_version
                    .is_some_and(|version| version != delta_version)
            {
                return Err("commit reconciliation failed".into());
            }
            record.committed_delta_version = Some(delta_version);
            Ok(())
        }
    }

    #[tokio::test]
    async fn wp21_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let record = workspace_record();
        let mut fabric = super::super::bootstrap_workspace(root.path(), &record)
            .await
            .unwrap();
        let mut journal = OperationalStore::open(&root.path().join("operations.sqlite3")).unwrap();

        let first_batch = validated_entity([4; 16]);
        let first_request = request(10, fabric.table(100).unwrap().version(), 100);
        let first = fabric
            .replace_owner_rows(&mut journal, &first_request, &first_batch)
            .await
            .unwrap();
        assert_eq!(first.final_row_count, 1);
        let duplicate = fabric
            .replace_owner_rows(&mut journal, &first_request, &first_batch)
            .await
            .unwrap();
        assert_eq!(duplicate.final_checksum, first.final_checksum);
        assert_eq!(duplicate.append_version, first.append_version);

        let replacement = validated_entity([5; 16]);
        let replacement_request = request(11, fabric.table(100).unwrap().version(), 100);
        fabric
            .replace_owner_rows(&mut journal, &replacement_request, &replacement)
            .await
            .unwrap();

        let recovered = validated_entity([6; 16]);
        let recovery_request = request(12, fabric.table(100).unwrap().version(), 100);
        assert!(matches!(
            fabric
                .replace_owner_rows_with_fault(
                    &mut journal,
                    &recovery_request,
                    &recovered,
                    Some(MutationFaultPoint::AfterDeleteCommit),
                )
                .await,
            Err(FabricError::MutationFault(
                MutationFaultPoint::AfterDeleteCommit
            ))
        ));
        let recovery = fabric
            .replace_owner_rows(&mut journal, &recovery_request, &recovered)
            .await
            .unwrap();
        assert_eq!(recovery.final_row_count, 1);

        let removed = fabric
            .remove_owners(
                &mut journal,
                scope().batch_scope(),
                [9; 16],
                [13; 16],
                vec![[3; 16]],
            )
            .await
            .unwrap();
        assert!(removed.contains_key(&100));
        assert!(removed.values().all(|result| result.final_row_count == 0));
        let readd = validated_entity([7; 16]);
        let readd_request = request(14, fabric.table(100).unwrap().version(), 100);
        assert_eq!(
            fabric
                .replace_owner_rows(&mut journal, &readd_request, &readd)
                .await
                .unwrap()
                .final_row_count,
            1
        );
    }

    #[test]
    fn wp21_structural_acceptance() {
        for (class, kind) in [
            (
                DurableMutationClass::StaticDimension,
                DurableWriteKind::BootstrapReplace,
            ),
            (
                DurableMutationClass::CurrentSingleton,
                DurableWriteKind::CurrentPointerSwap,
            ),
            (
                DurableMutationClass::OwnerReplacedFact,
                DurableWriteKind::OwnerReplace,
            ),
            (
                DurableMutationClass::PublicationAppend,
                DurableWriteKind::PublicationAppend,
            ),
            (
                DurableMutationClass::DerivedOwnerReplaced,
                DurableWriteKind::DerivedOwnerReplace,
            ),
            (
                DurableMutationClass::GlobalDerivedReplacement,
                DurableWriteKind::GlobalDerivedReplace,
            ),
        ] {
            assert_eq!(write_kind(class), kind);
        }
        assert!(table_specs().iter().all(|spec| matches!(
            spec.durable_mutation,
            DurableMutationClass::StaticDimension
                | DurableMutationClass::CurrentSingleton
                | DurableMutationClass::OwnerReplacedFact
                | DurableMutationClass::PublicationAppend
                | DurableMutationClass::DerivedOwnerReplaced
                | DurableMutationClass::GlobalDerivedReplacement
        )));
        let operation = include_str!("../../contracts/schema/schema-contract-ir.json");
        for field in [
            "operation_id",
            "table_code",
            "mutation_phase",
            "application_id",
            "application_version",
            "publication_id",
            "owner_set_fingerprint",
            "input_checksum",
            "expected_output_checksum",
            "expected_predecessor",
            "delta_version",
        ] {
            assert!(operation.contains(&format!("\"name\":\"{field}\"")));
        }
        assert_eq!(MutationFaultPoint::ALL.len(), 2);
        let prepared = PreparedMutation {
            spec: phase_spec(
                &request(1, Some(0), 100),
                &[[3; 16]],
                MutationPhase::OwnerAppend,
                [4; 32],
                [5; 32],
                Some(0),
            )
            .unwrap(),
            application_version: 1,
            committed_delta_version: None,
        };
        let fields = metadata(&prepared);
        for key in [
            "publication_id",
            "operation_id",
            "table_code",
            "mutation_phase",
            "application_id",
            "application_version",
            "owner_set_fingerprint",
            "input_checksum",
            "expected_output_checksum",
            "expected_predecessor",
        ] {
            assert!(fields.contains_key(key));
        }
    }

    #[tokio::test]
    async fn wp21_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let record = workspace_record();
        let mut first = super::super::bootstrap_workspace(root.path(), &record)
            .await
            .unwrap();
        let mut seed_journal = MemoryJournal::default();
        let seed_batch = validated_entity([4; 16]);
        let seed_request = request(20, first.table(100).unwrap().version(), 100);
        first
            .replace_owner_rows(&mut seed_journal, &seed_request, &seed_batch)
            .await
            .unwrap();

        assert!(
            enforce_write_kind(table_spec(1).unwrap(), DurableWriteKind::PublicationAppend)
                .is_err()
        );
        let input_checksum = batch_checksum(seed_batch.batch()).unwrap();
        let alien_request = request(99, first.table(100).unwrap().version(), 100);
        let alien = PreparedMutation {
            spec: phase_spec(
                &alien_request,
                &[[3; 16]],
                MutationPhase::OwnerAppend,
                input_checksum,
                input_checksum,
                alien_request.expected_predecessor,
            )
            .unwrap(),
            application_version: 1,
            committed_delta_version: None,
        };
        assert!(
            !commit_metadata_matches(first.table(100).unwrap(), &alien)
                .await
                .unwrap()
        );

        let mut second = super::super::bootstrap_workspace(root.path(), &record)
            .await
            .unwrap();
        let predecessor = first.table(100).unwrap().version();
        let shared = seed_journal.clone();
        let mut first_journal = shared.clone();
        let mut second_journal = shared;
        let first_request = request(21, predecessor, 100);
        let second_request = request(22, predecessor, 100);
        let first_batch = validated_entity([5; 16]);
        let second_batch = validated_entity([6; 16]);
        let (left, right) = tokio::join!(
            first.replace_owner_rows(&mut first_journal, &first_request, &first_batch),
            second.replace_owner_rows(&mut second_journal, &second_request, &second_batch),
        );
        assert!(
            left.is_ok() ^ right.is_ok(),
            "exactly one concurrent writer must win: left={left:?}, right={right:?}"
        );
        let failure = left.err().or_else(|| right.err()).unwrap();
        assert!(matches!(
            failure,
            FabricError::MutationConflict(_)
                | FabricError::MutationJournal(_)
                | FabricError::Delta(_)
        ));

        let mut journal = MemoryJournal::default();
        let wrong_class = request(23, first.table(1).unwrap().version(), 1);
        assert!(matches!(
            first
                .replace_owner_rows(&mut journal, &wrong_class, &validated_entity([7; 16]))
                .await,
            Err(FabricError::TableInvariant { .. } | FabricError::MutationConflict(_))
        ));
        let source = include_str!("mutation.rs");
        let forbidden = ["blind", "retry"].join("_");
        assert!(!source.contains(&forbidden));
    }

    #[tokio::test]
    async fn wp21_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let record = workspace_record();
        let mut fabric = super::super::bootstrap_workspace(root.path(), &record)
            .await
            .unwrap();
        let store_path = root.path().join("operations.sqlite3");
        let mut journal = OperationalStore::open(&store_path).unwrap();
        let batch = validated_entity([4; 16]);
        let request = request(30, fabric.table(100).unwrap().version(), 100);
        let result = fabric
            .replace_owner_rows(&mut journal, &request, &batch)
            .await
            .unwrap();
        assert!(matches!(result.deleted_rows, None | Some(0)));
        assert!(result.append_version.is_some());
        let reader = journal.reader_factory().open().unwrap();
        let (count, committed, versions): (i64, i64, i64) = reader
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*), SUM(state_code=20), COUNT(DISTINCT application_version)
                       FROM table_mutation_operation WHERE operation_id=?1",
                    [request.operation_id.as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
            })
            .unwrap();
        assert_eq!((count, committed, versions), (2, 2, 1));
        let table = fabric.table(100).unwrap();
        assert_eq!(result.append_version, table.version());
        let latest = table.delta.history(Some(1)).await.unwrap().next().unwrap();
        for key in [
            "publication_id",
            "operation_id",
            "owner_set_fingerprint",
            "input_checksum",
        ] {
            assert!(latest.info.contains_key(key), "missing persisted {key}");
        }
        let app_id = application_id([1; 16], 100, MutationPhase::OwnerAppend).unwrap();
        assert_eq!(transaction_version(table, &app_id).await.unwrap(), Some(1));
    }

    #[test]
    fn delta_43a0cf10_mutation_recovery_contract() {
        wp21_behavioral_acceptance();
        wp21_negative_zero_state();
        wp21_operational_acceptance();
    }

    #[test]
    fn wp05_structural_coordinator_retry_ownership() {
        let source = include_str!("mutation.rs");
        let retry_configuration = [".with_max_", "retries(0)"].concat();
        assert_eq!(source.matches(&retry_configuration).count(), 1);
        assert!(source.contains(".with_application_transaction(Transaction::new("));
        assert!(source.contains("transaction_version("));
        assert!(source.contains("reconcile_prepared("));
        assert!(!source.contains(&["blind", "retry"].join("_")));
    }
}
