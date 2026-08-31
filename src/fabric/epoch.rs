//! Immutable DataFusion catalog/session ownership for one fabric epoch.
//!
//! The builder owns every concrete registration handle. Sealing consumes the
//! builder, verifies the live catalog through DataFusion's own
//! `information_schema`, and retains only a private `SessionState` query
//! facade. Callers can execute bounded, typed scans but cannot register or
//! deregister catalogs, schemas, tables, functions, or object stores.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use arrow_array::{Array as _, ArrayRef, FixedSizeBinaryArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::catalog::{
    CatalogProvider as _, CatalogProviderList as _, MemoryCatalogProvider,
    MemoryCatalogProviderList, MemorySchemaProvider, SchemaProvider as _, TableProvider,
};
use datafusion::common::{DataFusionError, TableReference};
use datafusion::datasource::MemTable;
use datafusion::execution::memory_pool::{FairSpillPool, TrackConsumersPool};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use datafusion::execution::{SessionState, SessionStateBuilder};
use datafusion::logical_expr::TableType;
use datafusion::prelude::{SessionConfig, SessionContext};
use deltalake::logstore::LogStoreRef;

use crate::relational_model::{ModelEpoch, ModelRelation};
use crate::relational_program::{
    CompilationObservations, RelationInput, RelationalProgram, RelationalProgramCompiler,
    RelationalProgramError,
};
use crate::schema_contract::{
    FieldIndexMapping, ModelPhysicalBindingRow, SchemaCompatibility, SchemaContract,
    SchemaContractError, SchemaContractModelRows, SchemaPhase, SchemaRole,
};

use super::datafusion_cache::DataFusionCachePolicy;
use super::delta_exact::{
    ExactDeltaPin, ExactDeltaProviderError, ValidatedDeltaSnapshot, provider_from_exact_log_store,
    provider_from_validated_snapshot,
};
use super::proof::{ProofRelationKind, ProofRelations};
use super::provider::{ProviderContractError, SchemaContractTableProvider};

/// The sealed runtime and durable command/activation relations share one
/// canonical epoch identity type; there is no adapter-local second identity.
pub use super::command::EpochId as FabricEpochId;

/// The single catalog owned by every sealed epoch.
pub const FABRIC_CATALOG: &str = "codefabric";
const INFORMATION_SCHEMA: &str = "information_schema";
const CATALOG_OBJECT_TABLE: &str = "catalog_object";
const RUNTIME_CONFIGURATION_TABLE: &str = "runtime_configuration";
const ARROW_RELEASE: &str = "59.2.0";
const DATAFUSION_RELEASE: &str = "55.0.0";

/// Architectural role schemas. Their contents are model/runtime data; the
/// roles themselves are the fixed isolation boundary described by D-22.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FabricSchemaRole {
    Model,
    Source,
    RawTreeSitter,
    RawRuff,
    RawPyrefly,
    RawRustc,
    Fact,
    Derived,
    Proof,
    System,
    Public,
    Storage,
}

impl FabricSchemaRole {
    /// Complete epoch-local role namespace.
    pub const ALL: [Self; 12] = [
        Self::Model,
        Self::Source,
        Self::RawTreeSitter,
        Self::RawRuff,
        Self::RawPyrefly,
        Self::RawRustc,
        Self::Fact,
        Self::Derived,
        Self::Proof,
        Self::System,
        Self::Public,
        Self::Storage,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Source => "source",
            Self::RawTreeSitter => "raw_tree_sitter",
            Self::RawRuff => "raw_ruff",
            Self::RawPyrefly => "raw_pyrefly",
            Self::RawRustc => "raw_rustc",
            Self::Fact => "fact",
            Self::Derived => "derived",
            Self::Proof => "proof",
            Self::System => "system",
            Self::Public => "public",
            Self::Storage => "_storage",
        }
    }

    fn from_model_schema(value: &str) -> Option<Self> {
        match value {
            "model" => Some(Self::Model),
            "source" => Some(Self::Source),
            "raw_tree_sitter" => Some(Self::RawTreeSitter),
            "raw_ruff" => Some(Self::RawRuff),
            "raw_pyrefly" => Some(Self::RawPyrefly),
            "raw_rustc" => Some(Self::RawRustc),
            "fact" => Some(Self::Fact),
            "derived" => Some(Self::Derived),
            "proof" => Some(Self::Proof),
            "system" => Some(Self::System),
            "public" => Some(Self::Public),
            "_storage" => Some(Self::Storage),
            _ => None,
        }
    }
}

/// Exact, release-bound execution settings used to create one fresh runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricEpochRuntimeConfig {
    memory_limit_bytes: usize,
    max_spill_bytes: u64,
    max_spill_merge_fan_in: usize,
    tracked_consumer_count: NonZeroUsize,
    batch_size: NonZeroUsize,
    target_partitions: NonZeroUsize,
    collect_statistics: bool,
    cache_policy: DataFusionCachePolicy,
}

impl FabricEpochRuntimeConfig {
    /// Construct an explicitly bounded DataFusion runtime configuration.
    ///
    /// # Errors
    ///
    /// Rejects any zero bound. A runtime with an implicit unbounded resource is
    /// not a valid epoch input.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        memory_limit_bytes: usize,
        max_spill_bytes: u64,
        max_spill_merge_fan_in: usize,
        tracked_consumer_count: usize,
        batch_size: usize,
        target_partitions: usize,
        collect_statistics: bool,
    ) -> Result<Self, FabricEpochError> {
        let tracked_consumer_count = NonZeroUsize::new(tracked_consumer_count);
        let batch_size = NonZeroUsize::new(batch_size);
        let target_partitions = NonZeroUsize::new(target_partitions);
        if memory_limit_bytes == 0
            || max_spill_bytes == 0
            || max_spill_merge_fan_in == 0
            || tracked_consumer_count.is_none()
            || batch_size.is_none()
            || target_partitions.is_none()
        {
            return Err(FabricEpochError::InvalidRuntimeConfiguration(
                "memory, spill, merge fan-in, tracked-consumer, batch, and partition bounds must all be non-zero"
                    .into(),
            ));
        }
        Ok(Self {
            memory_limit_bytes,
            max_spill_bytes,
            max_spill_merge_fan_in,
            tracked_consumer_count: tracked_consumer_count.expect("validated non-zero"),
            batch_size: batch_size.expect("validated non-zero"),
            target_partitions: target_partitions.expect("validated non-zero"),
            collect_statistics,
            cache_policy: DataFusionCachePolicy::proportional_to(memory_limit_bytes),
        })
    }

    /// Replace the bounded cache profile that participates in epoch identity.
    #[must_use]
    pub fn with_cache_policy(mut self, cache_policy: DataFusionCachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    #[must_use]
    pub const fn cache_policy(&self) -> &DataFusionCachePolicy {
        &self.cache_policy
    }

    /// Canonical identity captured by the exact [`crate::relational_model::FabricCompilerRelease`].
    #[must_use]
    pub fn identity(&self) -> String {
        format!(
            "fabric-runtime.v2:arrow={ARROW_RELEASE}:datafusion={DATAFUSION_RELEASE}:memory={}:spill={}:fan-in={}:consumers={}:batch={}:partitions={}:statistics={}:parquet-view-types=false:{}",
            self.memory_limit_bytes,
            self.max_spill_bytes,
            self.max_spill_merge_fan_in,
            self.tracked_consumer_count,
            self.batch_size,
            self.target_partitions,
            self.collect_statistics,
            self.cache_policy.identity_fragment(),
        )
    }

    pub(super) fn session_config(&self) -> SessionConfig {
        SessionConfig::new()
            .with_default_catalog_and_schema(FABRIC_CATALOG, FabricSchemaRole::Public.as_str())
            .with_create_default_catalog_and_schema(false)
            .with_information_schema(true)
            .with_batch_size(self.batch_size.get())
            .with_target_partitions(self.target_partitions.get())
            .set_bool(
                "datafusion.execution.collect_statistics",
                self.collect_statistics,
            )
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
    }

    pub(super) fn runtime_env(&self) -> Result<Arc<RuntimeEnv>, DataFusionError> {
        self.cache_policy
            .configure_runtime(RuntimeEnvBuilder::new())
            .with_memory_pool(Arc::new(TrackConsumersPool::new(
                FairSpillPool::new(self.memory_limit_bytes),
                self.tracked_consumer_count,
            )))
            .with_max_temp_directory_size(self.max_spill_bytes)
            .with_max_spill_merge_fan_in(self.max_spill_merge_fan_in)
            .build_arc()
    }
}

impl Default for FabricEpochRuntimeConfig {
    fn default() -> Self {
        Self::try_new(
            256 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            32,
            16,
            8_192,
            1,
            true,
        )
        .expect("the built-in epoch runtime profile is bounded")
    }
}

/// Read-only runtime values safe to expose from the sealed epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricRuntimeObservation {
    pub configuration_identity: String,
    pub memory_limit_bytes: usize,
    pub memory_reserved_bytes: usize,
    pub max_spill_bytes: u64,
    pub spilled_bytes: u64,
    pub active_spill_files: usize,
    pub batch_size: usize,
    pub target_partitions: usize,
    pub metadata_cache_limit_bytes: usize,
    pub file_statistics_cache_limit_bytes: usize,
    pub object_list_cache_limit_bytes: usize,
    pub object_list_cache_ttl_seconds: Option<u64>,
    pub logical_plan_cache_capacity_entries: usize,
}

/// One bounded table scan accepted by the sealed model-query facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelTableScan {
    relation: ModelRelation,
    projection: Option<Vec<usize>>,
    limit: Option<usize>,
}

impl ModelTableScan {
    #[must_use]
    pub const fn all(relation: ModelRelation) -> Self {
        Self {
            relation,
            projection: None,
            limit: None,
        }
    }

    #[must_use]
    pub fn with_projection(mut self, projection: Vec<usize>) -> Self {
        self.projection = Some(projection);
        self
    }

    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub const fn relation(&self) -> ModelRelation {
        self.relation
    }
}

/// Arrow result preserving its schema even when DataFusion emits no batches.
#[derive(Clone, Debug)]
pub struct FabricQueryResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
}

impl FabricQueryResult {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }
}

/// Arrow output and causal compiler observations from one native relational
/// program executed inside a sealed epoch.
#[derive(Clone, Debug)]
pub struct FabricProgramResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    observations: CompilationObservations,
}

/// Model-supplied catalog binding for one computed proof relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCatalogBinding {
    relation: ProofRelationKind,
    table_name: String,
    source_schema_identity: String,
}

impl ProofCatalogBinding {
    /// Bind one computed proof role to a model-selected table and schema identity.
    ///
    /// # Errors
    ///
    /// Rejects an invalid table name or an empty schema identity.
    pub fn try_new(
        relation: ProofRelationKind,
        table_name: impl Into<String>,
        source_schema_identity: impl Into<String>,
    ) -> Result<Self, FabricEpochError> {
        let table_name = table_name.into();
        let source_schema_identity = source_schema_identity.into();
        validate_table_name(&table_name)?;
        if source_schema_identity.trim().is_empty() {
            return Err(FabricEpochError::ProofBinding(
                "proof source-schema identity is empty".into(),
            ));
        }
        Ok(Self {
            relation,
            table_name,
            source_schema_identity,
        })
    }

    #[must_use]
    pub const fn relation(&self) -> ProofRelationKind {
        self.relation
    }

    #[must_use]
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        &self.source_schema_identity
    }
}

impl FabricProgramResult {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub fn observations(&self) -> &CompilationObservations {
        &self.observations
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RegisteredTableKey {
    role: FabricSchemaRole,
    table_name: String,
}

#[derive(Clone, Debug)]
struct RegisteredTable {
    provider_kind: Arc<str>,
    table_type: TableType,
    contract: Arc<SchemaContract>,
}

/// Mutable construction boundary. This is the only production type that ever
/// owns concrete DataFusion registration handles.
pub struct FabricEpochBuilder {
    identity: FabricEpochId,
    model_epoch: Arc<ModelEpoch>,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_env: Arc<RuntimeEnv>,
    session_config: SessionConfig,
    catalog_list: Arc<MemoryCatalogProviderList>,
    session_state: SessionState,
    catalog: Arc<MemoryCatalogProvider>,
    schemas: BTreeMap<FabricSchemaRole, Arc<MemorySchemaProvider>>,
    model_contracts: BTreeMap<String, Arc<SchemaContract>>,
    registered_model_bindings: BTreeMap<String, RegisteredTableKey>,
    registered_tables: BTreeMap<RegisteredTableKey, RegisteredTable>,
    catalog_observation: Option<RecordBatch>,
}

impl FabricEpochBuilder {
    /// Start a fresh model-only epoch and install all replayed model relations.
    ///
    /// # Errors
    ///
    /// Rejects compiler/configuration pin disagreement, catalog construction
    /// failures, or any model batch that violates its executable schema
    /// contract.
    pub fn try_new(
        identity: FabricEpochId,
        model_epoch: Arc<ModelEpoch>,
        runtime_config: FabricEpochRuntimeConfig,
    ) -> Result<Self, FabricEpochError> {
        Self::try_new_with_physical_bindings(identity, model_epoch, runtime_config, Vec::new())
    }

    /// Start a fresh epoch from the replayed model and the exact outputs of
    /// its physical-binding programs.
    ///
    /// # Errors
    ///
    /// Rejects any missing/extra binding result, model-to-schema compilation
    /// failure, compiler/configuration disagreement, or catalog construction
    /// failure.
    pub fn try_new_with_physical_bindings(
        identity: FabricEpochId,
        model_epoch: Arc<ModelEpoch>,
        runtime_config: FabricEpochRuntimeConfig,
        physical_bindings: Vec<ModelPhysicalBindingRow>,
    ) -> Result<Self, FabricEpochError> {
        validate_compiler_release(&model_epoch, &runtime_config)?;
        let model_contracts = SchemaContractModelRows::from_model_epoch(
            &model_epoch,
            FABRIC_CATALOG,
            physical_bindings,
        )?
        .compile_all()?
        .into_iter()
        .map(|(binding_id, contract)| (binding_id, Arc::new(contract)))
        .collect();
        let runtime_env = runtime_config.runtime_env()?;
        let session_config = runtime_config.session_config();
        let catalog_list = Arc::new(MemoryCatalogProviderList::new());
        let catalog = Arc::new(MemoryCatalogProvider::new());
        if catalog_list
            .register_catalog(FABRIC_CATALOG.to_owned(), Arc::clone(&catalog) as _)
            .is_some()
        {
            return Err(FabricEpochError::CatalogClosure(
                "fresh catalog list already contained codefabric".into(),
            ));
        }

        let mut schemas = BTreeMap::new();
        for role in FabricSchemaRole::ALL {
            let schema = Arc::new(MemorySchemaProvider::new());
            if catalog
                .register_schema(role.as_str(), Arc::clone(&schema) as _)?
                .is_some()
            {
                return Err(FabricEpochError::CatalogClosure(format!(
                    "fresh catalog already contained schema {}",
                    role.as_str()
                )));
            }
            schemas.insert(role, schema);
        }

        let session_state = SessionStateBuilder::new()
            .with_default_features()
            .with_config(session_config.clone())
            .with_runtime_env(Arc::clone(&runtime_env))
            .with_catalog_list(Arc::clone(&catalog_list) as _)
            .build();

        let mut builder = Self {
            identity,
            model_epoch,
            runtime_config,
            runtime_env,
            session_config,
            catalog_list,
            session_state,
            catalog,
            schemas,
            model_contracts,
            registered_model_bindings: BTreeMap::new(),
            registered_tables: BTreeMap::new(),
            catalog_observation: None,
        };
        builder.install_model_relations()?;
        builder.install_runtime_configuration()?;
        Ok(builder)
    }

    /// Exact candidate identity all subsequently registered relations must carry.
    #[must_use]
    pub const fn identity(&self) -> &FabricEpochId {
        &self.identity
    }

    /// Register one prevalidated provider before the epoch is sealed.
    ///
    /// `provider_kind` is an observation, not a dispatch key. Runtime behavior
    /// remains the `TableProvider` implementation and its executable contract.
    ///
    /// # Errors
    ///
    /// Rejects invalid names, qualifier/schema disagreement, duplicate tables,
    /// or DataFusion catalog errors.
    pub fn register_provider(
        &mut self,
        role: FabricSchemaRole,
        table_name: impl Into<String>,
        provider_kind: impl Into<Arc<str>>,
        provider: Arc<dyn TableProvider>,
        contract: Arc<SchemaContract>,
    ) -> Result<(), FabricEpochError> {
        let table_name = table_name.into();
        validate_table_name(&table_name)?;
        if role == FabricSchemaRole::System
            && matches!(
                table_name.as_str(),
                CATALOG_OBJECT_TABLE | RUNTIME_CONFIGURATION_TABLE
            )
        {
            return Err(FabricEpochError::ReservedTable(table_name));
        }
        let provider = Arc::new(SchemaContractTableProvider::try_new(
            Arc::clone(&contract),
            provider,
        )?);
        self.register_provider_internal(role, table_name, provider_kind, provider, contract)
    }

    /// Register a provider through one compiled model physical binding.
    ///
    /// The binding identity determines the full catalog/schema/table
    /// qualifier and executable schema contract. Callers cannot supply a
    /// parallel table name or schema interpretation.
    ///
    /// # Errors
    ///
    /// Rejects unknown/already-consumed bindings, non-CodeFabric qualifiers,
    /// unknown role schemas, provider drift, or duplicate catalog objects.
    pub fn register_model_bound_provider(
        &mut self,
        physical_binding_id: &str,
        provider_kind: impl Into<Arc<str>>,
        provider: Arc<dyn TableProvider>,
    ) -> Result<(), FabricEpochError> {
        if self
            .registered_model_bindings
            .contains_key(physical_binding_id)
        {
            return Err(FabricEpochError::ModelBinding(format!(
                "physical binding {physical_binding_id} was already consumed"
            )));
        }
        let contract = Arc::clone(self.model_contracts.get(physical_binding_id).ok_or_else(
            || {
                FabricEpochError::ModelBinding(format!(
                    "physical binding {physical_binding_id} is absent from the replayed model"
                ))
            },
        )?);
        let TableReference::Full {
            catalog,
            schema,
            table,
        } = contract.qualifier()
        else {
            return Err(FabricEpochError::ModelBinding(format!(
                "physical binding {physical_binding_id} is not fully qualified"
            )));
        };
        if catalog.as_ref() != FABRIC_CATALOG {
            return Err(FabricEpochError::ModelBinding(format!(
                "physical binding {physical_binding_id} targets catalog {catalog}, not {FABRIC_CATALOG}"
            )));
        }
        let role = FabricSchemaRole::from_model_schema(schema).ok_or_else(|| {
            FabricEpochError::ModelBinding(format!(
                "physical binding {physical_binding_id} targets unknown schema role {schema}"
            ))
        })?;
        let table_name = table.to_string();
        self.register_provider(
            role,
            table_name.clone(),
            provider_kind,
            provider,
            Arc::clone(&contract),
        )?;
        self.registered_model_bindings.insert(
            physical_binding_id.to_owned(),
            RegisteredTableKey { role, table_name },
        );
        Ok(())
    }

    /// Register the complete computed proof-relation census under model-supplied names.
    ///
    /// The method validates the candidate epoch pin and every binding before mutating the
    /// catalog. Proof-family table names are not compiled into the runtime.
    ///
    /// # Errors
    ///
    /// Rejects an incomplete/duplicate binding census, an epoch-pin mismatch, schema drift, or
    /// a collision with an already registered proof table.
    pub fn register_proof_relations(
        &mut self,
        relations: &ProofRelations,
        bindings: &[ProofCatalogBinding],
    ) -> Result<(), FabricEpochError> {
        validate_proof_epoch(relations, &self.identity)?;
        let by_kind = bindings
            .iter()
            .map(|binding| (binding.relation, binding))
            .collect::<BTreeMap<_, _>>();
        let expected = ProofRelationKind::ALL.into_iter().collect::<BTreeSet<_>>();
        if bindings.len() != by_kind.len()
            || by_kind.keys().copied().collect::<BTreeSet<_>>() != expected
        {
            return Err(FabricEpochError::ProofBinding(
                "proof catalog bindings do not cover each computed relation exactly once".into(),
            ));
        }
        let mut names = BTreeSet::new();
        let mut prepared = Vec::with_capacity(ProofRelationKind::ALL.len());
        for kind in ProofRelationKind::ALL {
            let binding = by_kind[&kind];
            if !names.insert(binding.table_name.as_str())
                || self.registered_tables.contains_key(&RegisteredTableKey {
                    role: FabricSchemaRole::Proof,
                    table_name: binding.table_name.clone(),
                })
            {
                return Err(FabricEpochError::ProofBinding(format!(
                    "proof table {} is duplicated or already registered",
                    binding.table_name
                )));
            }
            let output = relations.relation(kind);
            let schema = Arc::clone(output.schema());
            if output.batch().schema_ref().as_ref() != schema.as_ref() {
                return Err(FabricEpochError::ProofBinding(format!(
                    "proof relation {kind:?} differs from its Arrow schema"
                )));
            }
            let mappings = (0..schema.fields().len())
                .map(|index| FieldIndexMapping::direct(index, index))
                .collect();
            let contract = Arc::new(SchemaContract::try_new(
                binding.source_schema_identity.clone(),
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Proof.as_str(),
                    binding.table_name.as_str(),
                ),
                Arc::clone(&schema),
                Arc::clone(&schema),
                mappings,
            )?);
            contract.validate_batch(&schema, output.batch(), SchemaCompatibility::Exact)?;
            let provider = Arc::new(MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![output.batch().clone()]],
            )?);
            prepared.push((binding.table_name.clone(), provider, contract));
        }
        for (table_name, provider, contract) in prepared {
            self.register_provider(
                FabricSchemaRole::Proof,
                table_name,
                "datafusion.mem_table.computed_proof",
                provider,
                contract,
            )?;
        }
        Ok(())
    }

    /// Register an exact Delta provider from an already loaded and validated snapshot.
    ///
    /// The provider is constructed with this builder's actual epoch `SessionState`; callers
    /// cannot substitute a separately configured session. The snapshot recipe never supplies a
    /// table-version selector to delta-rs.
    ///
    /// # Errors
    ///
    /// Rejects exact-pin, session/object-store, schema-contract, or catalog-registration drift.
    pub async fn register_exact_delta_snapshot(
        &mut self,
        role: FabricSchemaRole,
        table_name: impl Into<String>,
        pin: &ExactDeltaPin,
        snapshot: ValidatedDeltaSnapshot,
        contract: Arc<SchemaContract>,
    ) -> Result<(), FabricEpochError> {
        let provider =
            provider_from_validated_snapshot(pin, snapshot, Arc::new(self.session_state.clone()))
                .await?;
        self.register_provider(
            role,
            table_name,
            "deltalake.delta_scan.exact_snapshot",
            provider,
            contract,
        )
    }

    /// Register an exact Delta provider from a log store and exact version selector.
    ///
    /// The provider is constructed with this builder's actual epoch `SessionState`; callers
    /// cannot substitute a separately configured session. The log-store recipe never supplies a
    /// snapshot to delta-rs and never discovers the latest version.
    ///
    /// # Errors
    ///
    /// Rejects exact-pin, log replay, schema-contract, or catalog-registration drift.
    pub async fn register_exact_delta_log_store(
        &mut self,
        role: FabricSchemaRole,
        table_name: impl Into<String>,
        pin: &ExactDeltaPin,
        log_store: LogStoreRef,
        contract: Arc<SchemaContract>,
    ) -> Result<(), FabricEpochError> {
        let provider =
            provider_from_exact_log_store(pin, log_store, Arc::new(self.session_state.clone()))
                .await?;
        self.register_provider(
            role,
            table_name,
            "deltalake.delta_scan.exact_log_version",
            provider,
            contract,
        )
    }

    /// Seal the candidate after comparing the concrete catalog and column set
    /// with DataFusion 55's own `information_schema` output.
    ///
    /// # Errors
    ///
    /// Fails closed on any catalog, schema, table, column, compiler, or runtime
    /// closure difference.
    pub async fn seal(mut self) -> Result<FabricEpoch, FabricEpochError> {
        let expected_bindings = self.model_contracts.keys().collect::<BTreeSet<_>>();
        let registered_bindings = self
            .registered_model_bindings
            .keys()
            .collect::<BTreeSet<_>>();
        if expected_bindings != registered_bindings {
            return Err(FabricEpochError::ModelBinding(format!(
                "model physical-binding closure differs: missing={:?}, extra={:?}",
                expected_bindings
                    .difference(&registered_bindings)
                    .collect::<Vec<_>>(),
                registered_bindings
                    .difference(&expected_bindings)
                    .collect::<Vec<_>>()
            )));
        }
        self.install_catalog_observation()?;
        self.validate_concrete_catalog()?;

        let state = self.session_state;
        validate_information_schema(&state, &self.registered_tables).await?;

        let catalog_observation = self.catalog_observation.take().ok_or_else(|| {
            FabricEpochError::CatalogClosure(
                "system.catalog_object observation was not retained at seal".into(),
            )
        })?;

        let table_contracts = self
            .registered_tables
            .into_iter()
            .map(|(key, table)| (key, table.contract))
            .collect();
        Ok(FabricEpoch {
            identity: self.identity,
            model_epoch: self.model_epoch,
            runtime_config: self.runtime_config,
            runtime_env: self.runtime_env,
            session: SealedEpochSession { state },
            table_contracts,
            catalog_observation,
        })
    }

    fn install_model_relations(&mut self) -> Result<(), FabricEpochError> {
        let model_batches = self
            .model_epoch
            .relations()
            .iter()
            .map(|(relation, batch)| (relation, batch.clone()))
            .collect::<Vec<_>>();
        for (relation, batch) in model_batches {
            let schema = batch.schema();
            let qualifier = TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Model.as_str(),
                relation.as_str(),
            );
            let mappings = (0..schema.fields().len())
                .map(|index| FieldIndexMapping::direct(index, index))
                .collect();
            let contract = Arc::new(SchemaContract::try_new(
                format!(
                    "model:{}:{}:{}",
                    self.model_epoch.compiler_release().release_id(),
                    self.model_epoch.model_epoch_id(),
                    relation.as_str()
                ),
                qualifier,
                Arc::clone(&schema),
                Arc::clone(&schema),
                mappings,
            )?);
            contract.validate_batch(&schema, &batch, SchemaCompatibility::Exact)?;
            let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?);
            self.register_provider_internal(
                FabricSchemaRole::Model,
                relation.as_str().to_owned(),
                "datafusion.mem_table",
                provider,
                contract,
            )?;
        }
        let model_data_batches = self
            .model_epoch
            .relations()
            .data_iter()
            .map(|(relation_id, batch)| (relation_id.to_owned(), batch.clone()))
            .collect::<Vec<_>>();
        for (relation_id, batch) in model_data_batches {
            let schema = batch.schema();
            let table_name = schema
                .metadata()
                .get("codefabric.relation_name")
                .filter(|name| !name.is_empty())
                .cloned()
                .ok_or_else(|| {
                    FabricEpochError::ModelBinding(format!(
                        "model data relation {relation_id} has no relation-name metadata"
                    ))
                })?;
            let qualifier = TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Model.as_str(),
                table_name.as_str(),
            );
            let mappings = (0..schema.fields().len())
                .map(|index| FieldIndexMapping::direct(index, index))
                .collect();
            let contract = Arc::new(SchemaContract::try_new(
                format!(
                    "model-data:{}:{}:{relation_id}",
                    self.model_epoch.compiler_release().release_id(),
                    self.model_epoch.model_epoch_id(),
                ),
                qualifier,
                Arc::clone(&schema),
                Arc::clone(&schema),
                mappings,
            )?);
            contract.validate_batch(&schema, &batch, SchemaCompatibility::Exact)?;
            let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?);
            self.register_provider_internal(
                FabricSchemaRole::Model,
                table_name,
                "datafusion.mem_table",
                provider,
                contract,
            )?;
        }
        Ok(())
    }

    fn install_runtime_configuration(&mut self) -> Result<(), FabricEpochError> {
        let (schema, batch) = runtime_configuration_batch(
            &self.runtime_config,
            &self.session_config,
            &self.runtime_env,
        )?;
        let contract = direct_contract(
            format!(
                "runtime:{}:{}",
                epoch_identity_text(self.identity),
                self.runtime_config.identity()
            ),
            FabricSchemaRole::System,
            RUNTIME_CONFIGURATION_TABLE,
            &schema,
        )?;
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?);
        self.register_provider_internal(
            FabricSchemaRole::System,
            RUNTIME_CONFIGURATION_TABLE.to_owned(),
            "datafusion.mem_table",
            provider,
            Arc::new(contract),
        )
    }

    fn install_catalog_observation(&mut self) -> Result<(), FabricEpochError> {
        let source_schema_identity = format!("catalog:{}", epoch_identity_text(self.identity));
        let (schema, batch) =
            catalog_observation_batch(&self.registered_tables, &source_schema_identity)?;
        let contract = direct_contract(
            source_schema_identity,
            FabricSchemaRole::System,
            CATALOG_OBJECT_TABLE,
            &schema,
        )?;
        let provider = Arc::new(MemTable::try_new(
            Arc::clone(&schema),
            vec![vec![batch.clone()]],
        )?);
        self.register_provider_internal(
            FabricSchemaRole::System,
            CATALOG_OBJECT_TABLE.to_owned(),
            "datafusion.mem_table",
            provider,
            Arc::new(contract),
        )?;
        self.catalog_observation = Some(batch);
        Ok(())
    }

    fn register_provider_internal(
        &mut self,
        role: FabricSchemaRole,
        table_name: String,
        provider_kind: impl Into<Arc<str>>,
        provider: Arc<dyn TableProvider>,
        contract: Arc<SchemaContract>,
    ) -> Result<(), FabricEpochError> {
        let expected_qualifier = TableReference::full(
            FABRIC_CATALOG,
            role.as_str(),
            Arc::<str>::from(table_name.as_str()),
        );
        if contract.qualifier() != &expected_qualifier {
            return Err(FabricEpochError::ContractQualifier {
                expected: expected_qualifier,
                actual: contract.qualifier().clone(),
            });
        }
        contract.validate_arrow_schema(
            SchemaPhase::ProviderIngress,
            SchemaRole::Logical,
            provider.schema().as_ref(),
            SchemaCompatibility::Exact,
        )?;
        let key = RegisteredTableKey {
            role,
            table_name: table_name.clone(),
        };
        if self.registered_tables.contains_key(&key) {
            return Err(FabricEpochError::DuplicateTable {
                schema: role,
                table: table_name,
            });
        }
        let table_type = provider.table_type();
        let schema = self.schemas.get(&role).ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!("missing role schema {}", role.as_str()))
        })?;
        if schema.register_table(table_name, provider)?.is_some() {
            return Err(FabricEpochError::CatalogClosure(
                "MemorySchemaProvider replaced an existing table".into(),
            ));
        }
        self.registered_tables.insert(
            key,
            RegisteredTable {
                provider_kind: provider_kind.into(),
                table_type,
                contract,
            },
        );
        Ok(())
    }

    fn validate_concrete_catalog(&self) -> Result<(), FabricEpochError> {
        let actual_catalogs = self
            .catalog_list
            .catalog_names()
            .into_iter()
            .collect::<BTreeSet<String>>();
        let expected_catalogs = BTreeSet::from([FABRIC_CATALOG.to_owned()]);
        if actual_catalogs != expected_catalogs {
            return Err(FabricEpochError::CatalogClosure(format!(
                "catalogs differ: expected {expected_catalogs:?}, actual {actual_catalogs:?}"
            )));
        }
        let actual_schemas = self
            .catalog
            .schema_names()
            .into_iter()
            .collect::<BTreeSet<String>>();
        let expected_schemas = FabricSchemaRole::ALL
            .into_iter()
            .map(|role| role.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        if actual_schemas != expected_schemas {
            return Err(FabricEpochError::CatalogClosure(format!(
                "schemas differ: expected {expected_schemas:?}, actual {actual_schemas:?}"
            )));
        }
        for role in FabricSchemaRole::ALL {
            let actual = self.schemas[&role]
                .table_names()
                .into_iter()
                .collect::<BTreeSet<_>>();
            let expected = self
                .registered_tables
                .keys()
                .filter(|key| key.role == role)
                .map(|key| key.table_name.clone())
                .collect::<BTreeSet<_>>();
            if actual != expected {
                return Err(FabricEpochError::CatalogClosure(format!(
                    "tables in {} differ: expected {expected:?}, actual {actual:?}",
                    role.as_str()
                )));
            }
        }
        Ok(())
    }
}

/// Sealed epoch ownership. No raw catalog, context, session state, runtime, or
/// registration handle is exposed by this type.
pub struct FabricEpoch {
    identity: FabricEpochId,
    model_epoch: Arc<ModelEpoch>,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_env: Arc<RuntimeEnv>,
    session: SealedEpochSession,
    table_contracts: BTreeMap<RegisteredTableKey, Arc<SchemaContract>>,
    catalog_observation: RecordBatch,
}

impl fmt::Debug for FabricEpoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FabricEpoch")
            .field("identity", &self.identity)
            .field("model_epoch_id", &self.model_epoch.model_epoch_id())
            .field(
                "compiler_release_id",
                &self.model_epoch.compiler_release().release_id(),
            )
            .field("table_count", &self.table_contracts.len())
            .finish_non_exhaustive()
    }
}

impl FabricEpoch {
    #[must_use]
    pub const fn identity(&self) -> &FabricEpochId {
        &self.identity
    }

    #[must_use]
    pub fn model_epoch_id(&self) -> &str {
        self.model_epoch.model_epoch_id()
    }

    #[must_use]
    pub fn compiler_release_id(&self) -> &str {
        self.model_epoch.compiler_release().release_id()
    }

    #[must_use]
    pub fn model_epoch(&self) -> &Arc<ModelEpoch> {
        &self.model_epoch
    }

    /// Captured `system.catalog_object` relation for this exact sealed epoch.
    #[must_use]
    pub const fn catalog_observation(&self) -> &RecordBatch {
        &self.catalog_observation
    }

    /// Observe bounded runtime state without revealing the mutable runtime.
    #[must_use]
    pub fn runtime_observation(&self) -> FabricRuntimeObservation {
        let spilling = self.runtime_env.spilling_progress();
        FabricRuntimeObservation {
            configuration_identity: self.runtime_config.identity(),
            memory_limit_bytes: self.runtime_config.memory_limit_bytes,
            memory_reserved_bytes: self.runtime_env.memory_pool.reserved(),
            max_spill_bytes: self.runtime_config.max_spill_bytes,
            spilled_bytes: spilling.current_bytes,
            active_spill_files: spilling.active_files_count,
            batch_size: self.runtime_config.batch_size.get(),
            target_partitions: self.runtime_config.target_partitions.get(),
            metadata_cache_limit_bytes: self.runtime_env.cache_manager.get_metadata_cache_limit(),
            file_statistics_cache_limit_bytes: self
                .runtime_env
                .cache_manager
                .get_file_statistic_cache_limit(),
            object_list_cache_limit_bytes: self
                .runtime_env
                .cache_manager
                .get_list_files_cache_limit(),
            object_list_cache_ttl_seconds: self
                .runtime_env
                .cache_manager
                .get_list_files_cache_ttl()
                .map(|ttl| ttl.as_secs()),
            logical_plan_cache_capacity_entries: self
                .runtime_config
                .cache_policy
                .logical_plan_entries(),
        }
    }

    /// Resolve one provider/contract pair from the already sealed catalog for
    /// reduced child-session construction. The returned values are the table
    /// capability itself and its executable contract, never a parent catalog,
    /// schema, session, or runtime handle.
    pub(super) async fn resolve_sealed_table(
        &self,
        role: FabricSchemaRole,
        table_name: &str,
    ) -> Result<(Arc<dyn TableProvider>, Arc<SchemaContract>), FabricEpochError> {
        let key = RegisteredTableKey {
            role,
            table_name: table_name.to_owned(),
        };
        let contract = self.table_contracts.get(&key).ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!(
                "sealed table contract is absent for {}.{table_name}",
                role.as_str()
            ))
        })?;
        let catalog = self
            .session
            .state
            .catalog_list()
            .catalog(FABRIC_CATALOG)
            .ok_or_else(|| {
                FabricEpochError::CatalogClosure(
                    "sealed codefabric catalog is absent during child resolution".into(),
                )
            })?;
        let schema = catalog.schema(role.as_str()).ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!(
                "sealed schema {} is absent during child resolution",
                role.as_str()
            ))
        })?;
        let provider = schema.table(table_name).await?.ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!(
                "sealed provider is absent for {}.{table_name}",
                role.as_str()
            ))
        })?;
        contract.validate_arrow_schema(
            SchemaPhase::ProviderIngress,
            SchemaRole::Logical,
            provider.schema().as_ref(),
            SchemaCompatibility::Exact,
        )?;
        Ok((provider, Arc::clone(contract)))
    }

    /// Prove that a proposed child owns a fresh catalog, runtime, memory pool,
    /// spill manager, cache, and object-store registry. This comparison keeps
    /// all parent authority handles inside the epoch module.
    pub(super) fn child_authorities_are_distinct(
        &self,
        runtime: &Arc<RuntimeEnv>,
        catalog_list: &Arc<dyn datafusion::catalog::CatalogProviderList>,
    ) -> bool {
        !Arc::ptr_eq(&self.runtime_env, runtime)
            && !Arc::ptr_eq(self.session.state.catalog_list(), catalog_list)
            && !Arc::ptr_eq(&self.runtime_env.memory_pool, &runtime.memory_pool)
            && !Arc::ptr_eq(&self.runtime_env.disk_manager, &runtime.disk_manager)
            && !Arc::ptr_eq(&self.runtime_env.cache_manager, &runtime.cache_manager)
            && !Arc::ptr_eq(
                &self.runtime_env.object_store_registry,
                &runtime.object_store_registry,
            )
    }

    /// Execute a typed projection/limit against one replayed model relation.
    /// The internal DataFusion `SessionContext` is request-local and never
    /// escapes this method.
    ///
    /// # Errors
    ///
    /// Returns a typed contract or DataFusion error for an invalid projection,
    /// planning failure, execution failure, or output-schema drift.
    pub async fn scan_model(
        &self,
        request: &ModelTableScan,
    ) -> Result<FabricQueryResult, FabricEpochError> {
        let key = RegisteredTableKey {
            role: FabricSchemaRole::Model,
            table_name: request.relation.as_str().to_owned(),
        };
        let contract = self.table_contracts.get(&key).ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!(
                "model relation {} has no sealed contract",
                request.relation.as_str()
            ))
        })?;
        let projection = request.projection.as_deref();
        let expected_schema = if let Some(projection) = projection {
            contract.project_logical_schema(projection)?
        } else {
            Arc::clone(contract.logical_schema())
        };
        let table_ref = TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::Model.as_str(),
            request.relation.as_str(),
        );
        let mut frame = self.session.context().table(table_ref).await?;
        if let Some(projection) = projection {
            let names = projection
                .iter()
                .map(|index| {
                    contract
                        .logical_schema()
                        .fields()
                        .get(*index)
                        .map(|field| field.name().as_str())
                        .ok_or_else(|| FabricEpochError::InvalidProjection {
                            relation: request.relation,
                            index: *index,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            frame = frame.select_columns(&names)?;
        }
        if let Some(limit) = request.limit {
            frame = frame.limit(0, Some(limit))?;
        }
        let batches = frame.collect().await?;
        for batch in &batches {
            if batch.schema_ref().as_ref() != expected_schema.as_ref() {
                return Err(FabricEpochError::QuerySchemaDrift {
                    relation: request.relation,
                    expected: Arc::clone(&expected_schema),
                    actual: batch.schema(),
                });
            }
        }
        Ok(FabricQueryResult {
            schema: expected_schema,
            batches,
        })
    }

    /// Resolve a closed relational program's model inputs from this epoch's
    /// concrete catalog, compile it to a native DataFusion logical plan, and
    /// execute it with the epoch's private session/runtime.
    ///
    /// # Errors
    ///
    /// Fails closed when a model relation names a non-role schema, references
    /// an unsealed provider, cannot compile under the epoch model, or produces
    /// batches whose Arrow schema differs from the compiled logical output.
    pub async fn execute_relational_program(
        &self,
        program: &RelationalProgram,
    ) -> Result<FabricProgramResult, FabricEpochError> {
        let context = self.session.context();
        let bindings = RelationalProgramCompiler::bind_catalog_inputs(&self.model_epoch, program)?;
        let mut inputs = Vec::with_capacity(bindings.len());
        for binding in bindings {
            if binding
                .table_reference
                .catalog()
                .is_some_and(|catalog| catalog != FABRIC_CATALOG)
            {
                return Err(FabricEpochError::CatalogClosure(format!(
                    "model relation {} names foreign catalog {:?}",
                    binding.relation_id.as_str(),
                    binding.table_reference.catalog()
                )));
            }
            let schema_name = binding.table_reference.schema().ok_or_else(|| {
                FabricEpochError::CatalogClosure(format!(
                    "model relation {} has no role schema",
                    binding.relation_id.as_str()
                ))
            })?;
            let role = FabricSchemaRole::from_model_schema(schema_name).ok_or_else(|| {
                FabricEpochError::CatalogClosure(format!(
                    "model relation {} names unknown role schema {schema_name:?}",
                    binding.relation_id.as_str()
                ))
            })?;
            let table_name = binding.table_reference.table();
            let key = RegisteredTableKey {
                role,
                table_name: table_name.to_owned(),
            };
            if !self.table_contracts.contains_key(&key) {
                return Err(FabricEpochError::CatalogClosure(format!(
                    "model relation {} resolves to unsealed table {}.{table_name}",
                    binding.relation_id.as_str(),
                    role.as_str()
                )));
            }
            let plan = context
                .table(TableReference::full(
                    FABRIC_CATALOG,
                    role.as_str(),
                    table_name,
                ))
                .await?
                .into_unoptimized_plan();
            inputs.push(RelationInput {
                relation_id: binding.relation_id,
                plan,
            });
        }

        let compiled = RelationalProgramCompiler::compile(&self.model_epoch, inputs, program)?;
        let schema = Arc::new(compiled.plan.schema().as_arrow().clone());
        let observations = compiled.observations;
        let batches = context
            .execute_logical_plan(compiled.plan)
            .await?
            .collect()
            .await?;
        for batch in &batches {
            if batch.schema_ref().as_ref() != schema.as_ref() {
                return Err(FabricEpochError::RelationalOutputSchemaDrift {
                    expected: Arc::clone(&schema),
                    actual: batch.schema(),
                });
            }
        }
        Ok(FabricProgramResult {
            schema,
            batches,
            observations,
        })
    }
}

struct SealedEpochSession {
    state: SessionState,
}

impl SealedEpochSession {
    fn context(&self) -> SessionContext {
        SessionContext::new_with_state(self.state.clone())
    }
}

/// Fail-closed epoch construction and query errors.
#[derive(Debug, thiserror::Error)]
pub enum FabricEpochError {
    #[error("invalid fabric epoch runtime configuration: {0}")]
    InvalidRuntimeConfiguration(String),
    #[error(transparent)]
    ExactDelta(#[from] ExactDeltaProviderError),
    #[error("invalid computed proof catalog binding: {0}")]
    ProofBinding(String),
    #[error("invalid model-derived catalog binding: {0}")]
    ModelBinding(String),
    #[error("compiler release dependency {dependency} must be {expected}, actual {actual:?}")]
    CompilerDependencyMismatch {
        dependency: &'static str,
        expected: &'static str,
        actual: Option<String>,
    },
    #[error("compiler release configuration {release:?} differs from runtime {runtime:?}")]
    RuntimeIdentityMismatch { release: String, runtime: String },
    #[error("invalid table name {0:?}")]
    InvalidTableName(String),
    #[error("table name {0:?} is reserved for a derived system relation")]
    ReservedTable(String),
    #[error("duplicate table {schema:?}.{table}")]
    DuplicateTable {
        schema: FabricSchemaRole,
        table: String,
    },
    #[error("schema contract qualifier differs: expected {expected}, actual {actual}")]
    ContractQualifier {
        expected: TableReference,
        actual: TableReference,
    },
    #[error("catalog closure failed: {0}")]
    CatalogClosure(String),
    #[error("invalid projection index {index} for model relation {relation:?}")]
    InvalidProjection {
        relation: ModelRelation,
        index: usize,
    },
    #[error("query schema drift for {relation:?}: expected {expected:?}, actual {actual:?}")]
    QuerySchemaDrift {
        relation: ModelRelation,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("relational program output schema drift: expected {expected:?}, actual {actual:?}")]
    RelationalOutputSchemaDrift {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error(transparent)]
    RelationalProgram(#[from] RelationalProgramError),
    #[error(transparent)]
    SchemaContract(#[from] SchemaContractError),
    #[error(transparent)]
    ProviderContract(#[from] ProviderContractError),
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
}

fn validate_compiler_release(
    model_epoch: &ModelEpoch,
    runtime_config: &FabricEpochRuntimeConfig,
) -> Result<(), FabricEpochError> {
    let release = model_epoch.compiler_release();
    for (dependency, expected) in [("arrow", ARROW_RELEASE), ("datafusion", DATAFUSION_RELEASE)] {
        let actual = release
            .dependencies()
            .get(dependency)
            .map(|value| value.identity().to_owned());
        if actual.as_deref() != Some(expected) {
            return Err(FabricEpochError::CompilerDependencyMismatch {
                dependency,
                expected,
                actual,
            });
        }
    }
    let runtime_identity = runtime_config.identity();
    if release.effective_configuration_identity() != runtime_identity {
        return Err(FabricEpochError::RuntimeIdentityMismatch {
            release: release.effective_configuration_identity().to_owned(),
            runtime: runtime_identity,
        });
    }
    Ok(())
}

fn validate_table_name(table_name: &str) -> Result<(), FabricEpochError> {
    if table_name.is_empty()
        || table_name.len() > 128
        || !table_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(FabricEpochError::InvalidTableName(table_name.to_owned()));
    }
    Ok(())
}

pub(super) fn epoch_identity_text(identity: FabricEpochId) -> String {
    identity
        .as_bytes()
        .iter()
        .fold(String::with_capacity(32), |mut encoded, byte| {
            use std::fmt::Write as _;
            write!(encoded, "{byte:02x}").expect("writing into a String cannot fail");
            encoded
        })
}

fn direct_contract(
    source_identity: String,
    role: FabricSchemaRole,
    table_name: &'static str,
    schema: &SchemaRef,
) -> Result<SchemaContract, SchemaContractError> {
    SchemaContract::try_new(
        source_identity,
        TableReference::full(FABRIC_CATALOG, role.as_str(), table_name),
        Arc::clone(schema),
        Arc::clone(schema),
        (0..schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect(),
    )
}

fn runtime_configuration_batch(
    config: &FabricEpochRuntimeConfig,
    session: &SessionConfig,
    runtime: &RuntimeEnv,
) -> Result<(SchemaRef, RecordBatch), arrow_schema::ArrowError> {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("scope", DataType::Utf8, false),
            Field::new("setting", DataType::Utf8, false),
            Field::new("value", DataType::Utf8, true),
            Field::new("description", DataType::Utf8, false),
        ],
        BTreeMap::from([
            ("codefabric.schema_role".to_owned(), "system".to_owned()),
            (
                "codefabric.system_relation".to_owned(),
                RUNTIME_CONFIGURATION_TABLE.to_owned(),
            ),
        ])
        .into_iter()
        .collect(),
    ));
    let mut entries = session
        .options()
        .entries()
        .into_iter()
        .map(|entry| {
            (
                "session".to_owned(),
                entry.key,
                entry.value,
                entry.description.to_owned(),
            )
        })
        .chain(runtime.config_entries().into_iter().map(|entry| {
            (
                "runtime".to_owned(),
                entry.key,
                entry.value,
                entry.description.to_owned(),
            )
        }))
        .collect::<Vec<_>>();
    entries.push((
        "codefabric".to_owned(),
        "effective_configuration_identity".to_owned(),
        Some(config.identity()),
        "Exact release-bound CodeFabric execution configuration".to_owned(),
    ));
    entries.sort();

    let scopes = StringArray::from_iter_values(entries.iter().map(|entry| entry.0.as_str()));
    let settings = StringArray::from_iter_values(entries.iter().map(|entry| entry.1.as_str()));
    let values = StringArray::from(
        entries
            .iter()
            .map(|entry| entry.2.as_deref())
            .collect::<Vec<_>>(),
    );
    let descriptions = StringArray::from_iter_values(entries.iter().map(|entry| entry.3.as_str()));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(scopes),
            Arc::new(settings),
            Arc::new(values),
            Arc::new(descriptions),
        ],
    )?;
    Ok((schema, batch))
}

fn catalog_observation_batch(
    registered_tables: &BTreeMap<RegisteredTableKey, RegisteredTable>,
    catalog_source_schema_identity: &str,
) -> Result<(SchemaRef, RecordBatch), arrow_schema::ArrowError> {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("object_kind", DataType::Utf8, false),
            Field::new("catalog_name", DataType::Utf8, false),
            Field::new("schema_name", DataType::Utf8, true),
            Field::new("table_name", DataType::Utf8, true),
            Field::new("table_type", DataType::Utf8, true),
            Field::new("provider_kind", DataType::Utf8, true),
            Field::new("source_schema_identity", DataType::Utf8, true),
        ],
        BTreeMap::from([
            ("codefabric.schema_role".to_owned(), "system".to_owned()),
            (
                "codefabric.system_relation".to_owned(),
                CATALOG_OBJECT_TABLE.to_owned(),
            ),
        ])
        .into_iter()
        .collect(),
    ));

    type ObservationRow = (
        &'static str,
        &'static str,
        Option<&'static str>,
        Option<String>,
        Option<&'static str>,
        Option<String>,
        Option<String>,
    );
    let mut rows: Vec<ObservationRow> =
        vec![("catalog", FABRIC_CATALOG, None, None, None, None, None)];
    rows.extend(FabricSchemaRole::ALL.into_iter().map(|role| {
        (
            "schema",
            FABRIC_CATALOG,
            Some(role.as_str()),
            None,
            None,
            None,
            None,
        )
    }));
    rows.extend(registered_tables.iter().map(|(key, table)| {
        (
            "table",
            FABRIC_CATALOG,
            Some(key.role.as_str()),
            Some(key.table_name.clone()),
            Some(information_schema_table_type(table.table_type)),
            Some(table.provider_kind.to_string()),
            Some(table.contract.source_schema_identity().to_owned()),
        )
    }));
    rows.push((
        "table",
        FABRIC_CATALOG,
        Some(FabricSchemaRole::System.as_str()),
        Some(CATALOG_OBJECT_TABLE.to_owned()),
        Some(information_schema_table_type(TableType::Base)),
        Some("datafusion.mem_table".to_owned()),
        Some(catalog_source_schema_identity.to_owned()),
    ));
    rows.sort();

    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.0))),
        Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.1))),
        Arc::new(StringArray::from(
            rows.iter().map(|row| row.2).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter().map(|row| row.3.as_deref()).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter().map(|row| row.4).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter().map(|row| row.5.as_deref()).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter().map(|row| row.6.as_deref()).collect::<Vec<_>>(),
        )),
    ];
    Ok((Arc::clone(&schema), RecordBatch::try_new(schema, columns)?))
}

fn validate_proof_epoch(
    relations: &ProofRelations,
    expected_epoch: &FabricEpochId,
) -> Result<(), FabricEpochError> {
    if relations
        .relation(ProofRelationKind::ProofRun)
        .batch()
        .num_rows()
        != 1
    {
        return Err(FabricEpochError::ProofBinding(
            "proof_run must contain exactly one candidate row".into(),
        ));
    }
    for kind in ProofRelationKind::ALL {
        let output = relations.relation(kind);
        let epoch = output
            .batch()
            .column_by_name("epoch_id")
            .and_then(|column| column.as_any().downcast_ref::<FixedSizeBinaryArray>())
            .ok_or_else(|| {
                FabricEpochError::ProofBinding(format!(
                    "proof relation {kind:?} has no FixedSizeBinary epoch_id"
                ))
            })?;
        if epoch.value_length() != 16
            || epoch
                .iter()
                .any(|value| value != Some(expected_epoch.as_bytes().as_slice()))
        {
            return Err(FabricEpochError::ProofBinding(format!(
                "proof relation {kind:?} is not pinned to the candidate epoch"
            )));
        }
    }
    Ok(())
}

async fn validate_information_schema(
    state: &SessionState,
    registered_tables: &BTreeMap<RegisteredTableKey, RegisteredTable>,
) -> Result<(), FabricEpochError> {
    let context = SessionContext::new_with_state(state.clone());
    let table_batches = context
        .table(TableReference::full(
            FABRIC_CATALOG,
            INFORMATION_SCHEMA,
            "tables",
        ))
        .await?
        .select_columns(&["table_catalog", "table_schema", "table_name", "table_type"])?
        .collect()
        .await?;
    let actual_tables = collect_information_schema_tables(&table_batches)?;
    let expected_tables = registered_tables
        .iter()
        .map(|(key, table)| {
            (
                key.role.as_str().to_owned(),
                key.table_name.clone(),
                information_schema_table_type(table.table_type).to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    if actual_tables != expected_tables {
        return Err(FabricEpochError::CatalogClosure(format!(
            "information_schema.tables differs: expected {expected_tables:?}, actual {actual_tables:?}"
        )));
    }

    let column_batches = context
        .table(TableReference::full(
            FABRIC_CATALOG,
            INFORMATION_SCHEMA,
            "columns",
        ))
        .await?
        .select_columns(&["table_catalog", "table_schema", "table_name", "column_name"])?
        .collect()
        .await?;
    let actual_columns = collect_information_schema_columns(&column_batches)?;
    let expected_columns = registered_tables
        .iter()
        .flat_map(|(key, table)| {
            table
                .contract
                .logical_schema()
                .fields()
                .iter()
                .map(|field| {
                    (
                        key.role.as_str().to_owned(),
                        key.table_name.clone(),
                        field.name().to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    if actual_columns != expected_columns {
        return Err(FabricEpochError::CatalogClosure(format!(
            "information_schema.columns differs: expected {expected_columns:?}, actual {actual_columns:?}"
        )));
    }
    Ok(())
}

fn collect_information_schema_tables(
    batches: &[RecordBatch],
) -> Result<BTreeSet<(String, String, String)>, FabricEpochError> {
    let mut rows = BTreeSet::new();
    for batch in batches {
        let catalog = utf8_column(batch, 0)?;
        let schema = utf8_column(batch, 1)?;
        let table = utf8_column(batch, 2)?;
        let table_type = utf8_column(batch, 3)?;
        for row in 0..batch.num_rows() {
            if catalog.value(row) == FABRIC_CATALOG && schema.value(row) != INFORMATION_SCHEMA {
                rows.insert((
                    schema.value(row).to_owned(),
                    table.value(row).to_owned(),
                    table_type.value(row).to_owned(),
                ));
            }
        }
    }
    Ok(rows)
}

fn collect_information_schema_columns(
    batches: &[RecordBatch],
) -> Result<BTreeSet<(String, String, String)>, FabricEpochError> {
    let mut rows = BTreeSet::new();
    for batch in batches {
        let catalog = utf8_column(batch, 0)?;
        let schema = utf8_column(batch, 1)?;
        let table = utf8_column(batch, 2)?;
        let column = utf8_column(batch, 3)?;
        for row in 0..batch.num_rows() {
            if catalog.value(row) == FABRIC_CATALOG && schema.value(row) != INFORMATION_SCHEMA {
                rows.insert((
                    schema.value(row).to_owned(),
                    table.value(row).to_owned(),
                    column.value(row).to_owned(),
                ));
            }
        }
    }
    Ok(rows)
}

fn utf8_column(batch: &RecordBatch, index: usize) -> Result<&StringArray, FabricEpochError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| {
            FabricEpochError::CatalogClosure(format!(
                "information_schema column {index} is not non-null Utf8"
            ))
        })
}

const fn information_schema_table_type(table_type: TableType) -> &'static str {
    match table_type {
        TableType::Base => "BASE TABLE",
        TableType::View => "VIEW",
        TableType::Temporary => "LOCAL TEMPORARY",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::RwLock;

    use async_trait::async_trait;
    use datafusion::catalog::Session;
    use datafusion::physical_plan::ExecutionPlan;
    use datafusion::physical_plan::empty::EmptyExec;
    use deltalake::DeltaTableBuilder;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use tempfile::TempDir;

    use super::*;
    use crate::fabric::proof::test_relations_for_epoch;
    use crate::relational_model::{
        BootstrapMetamodel, FabricCompilerRelease, IntrinsicInstaller, ModelDecision,
        ModelMigration, ModelOperation, ModelRowBuilder, ReplayEngine,
    };
    use crate::relational_program::{
        CompilationDependency, FieldId, RelationId, RelationalExpression,
    };

    #[derive(Debug)]
    struct MutableSchemaProvider {
        schema: RwLock<SchemaRef>,
    }

    impl MutableSchemaProvider {
        fn replace_schema(&self, schema: SchemaRef) {
            *self.schema.write().unwrap() = schema;
        }
    }

    #[async_trait]
    impl TableProvider for MutableSchemaProvider {
        fn schema(&self) -> SchemaRef {
            Arc::clone(&self.schema.read().unwrap())
        }

        fn table_type(&self) -> TableType {
            TableType::Base
        }

        async fn scan(
            &self,
            _state: &dyn Session,
            projection: Option<&Vec<usize>>,
            _filters: &[datafusion::logical_expr::Expr],
            _limit: Option<usize>,
        ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
            let schema = projection.map_or_else(
                || Ok(self.schema()),
                |projection| {
                    self.schema()
                        .project(projection)
                        .map(Arc::new)
                        .map_err(DataFusionError::from)
                },
            )?;
            Ok(Arc::new(EmptyExec::new(schema)))
        }
    }

    fn model_epoch(runtime: &FabricEpochRuntimeConfig) -> Arc<ModelEpoch> {
        replay_model_epoch(runtime, &[])
    }

    fn replay_model_epoch(
        runtime: &FabricEpochRuntimeConfig,
        migrations: &[ModelMigration],
    ) -> Arc<ModelEpoch> {
        let release = FabricCompilerRelease::builder(
            "fabric-release-epoch-test",
            "source:fabric-epoch-test",
            "build:fabric-epoch-test",
        )
        .with_abis(1, 1, 1)
        .with_intrinsic_package("intrinsics-v1")
        .add_dependency("arrow", ARROW_RELEASE)
        .unwrap()
        .add_dependency("datafusion", DATAFUSION_RELEASE)
        .unwrap()
        .add_dependency("deltalake", "43a0cf10")
        .unwrap()
        .add_provider_schema("tree-sitter", "python-0.25.0-rust-0.24.2")
        .unwrap()
        .with_policy_and_configuration("policy-v2", runtime.identity())
        .add_toolchain("rust", "1.95.0")
        .unwrap()
        .add_wire_contract("codefabric.rpc.cpg-query-service")
        .unwrap()
        .build()
        .unwrap();
        let engine = ReplayEngine::new(
            release,
            IntrinsicInstaller::new("intrinsics-v1", "implementation-v1").unwrap(),
        )
        .unwrap();
        Arc::new(engine.replay(migrations).unwrap())
    }

    fn relational_program_model_epoch(runtime: &FabricEpochRuntimeConfig) -> Arc<ModelEpoch> {
        let metamodel = BootstrapMetamodel::new();
        let relation_id = "test.relation.fact_values";
        let field_id = "test.field.fact_values.value";
        let operations = vec![
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Relation)
                    .value("relation_id", relation_id)
                    .unwrap()
                    .value("schema_name", FabricSchemaRole::Fact.as_str())
                    .unwrap()
                    .value("relation_name", "fact_values")
                    .unwrap()
                    .value("semantic_role", "epoch-execution-fixture")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value("field_id", field_id)
                    .unwrap()
                    .value("relation_id", relation_id)
                    .unwrap()
                    .value("field_name", "value")
                    .unwrap()
                    .value("semantic_type_id", "bootstrap.scalar.u64")
                    .unwrap()
                    .value("ordinal", 0_u32)
                    .unwrap()
                    .value("nullable", false)
                    .unwrap()
                    .value("semantic_role", "epoch-execution-fixture")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
        ];
        let decision = ModelDecision::new(
            "test.decision.fact_values",
            "epoch-tests",
            "exercise sealed catalog program execution",
            "bind a model relation to an epoch provider",
            operations,
        )
        .unwrap();
        let migration = ModelMigration::new(
            "test.migration.fact_values",
            None,
            "model.bootstrap.fabric-release-epoch-test",
            "test.model-epoch.fact_values",
            1,
            "epoch-tests",
            vec![decision],
        )
        .unwrap();
        replay_model_epoch(runtime, &[migration])
    }

    fn schema_bound_model_epoch(
        runtime: &FabricEpochRuntimeConfig,
    ) -> (Arc<ModelEpoch>, Vec<ModelPhysicalBindingRow>) {
        let metamodel = BootstrapMetamodel::new();
        let logical_relation_id = "test.relation.bound-logical";
        let storage_relation_id = "test.relation.bound-storage";
        let logical_field_id = "test.field.bound-logical.value";
        let storage_field_id = "test.field.bound-storage.value";
        let mapping_program_id = "test.program.bound-storage";
        let binding_id = "test.binding.bound-storage";
        let operations = vec![
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Relation)
                    .value("relation_id", logical_relation_id)
                    .unwrap()
                    .value("schema_name", FabricSchemaRole::Fact.as_str())
                    .unwrap()
                    .value("relation_name", "bound_values")
                    .unwrap()
                    .value("semantic_role", "logical-test-relation")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Relation)
                    .value("relation_id", storage_relation_id)
                    .unwrap()
                    .value("schema_name", FabricSchemaRole::Storage.as_str())
                    .unwrap()
                    .value("relation_name", "bound_values")
                    .unwrap()
                    .value("semantic_role", "storage-test-relation")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value("field_id", logical_field_id)
                    .unwrap()
                    .value("relation_id", logical_relation_id)
                    .unwrap()
                    .value("field_name", "value")
                    .unwrap()
                    .value("semantic_type_id", "bootstrap.scalar.u64")
                    .unwrap()
                    .value("ordinal", 0_u32)
                    .unwrap()
                    .value("nullable", false)
                    .unwrap()
                    .value("semantic_role", "measure")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value("field_id", storage_field_id)
                    .unwrap()
                    .value("relation_id", storage_relation_id)
                    .unwrap()
                    .value("field_name", "value")
                    .unwrap()
                    .value("semantic_type_id", "bootstrap.scalar.u64")
                    .unwrap()
                    .value("ordinal", 0_u32)
                    .unwrap()
                    .value("nullable", false)
                    .unwrap()
                    .value("semantic_role", "storage-measure")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Representation)
                    .value("representation_id", "test.representation.u64")
                    .unwrap()
                    .value("semantic_type_id", "bootstrap.scalar.u64")
                    .unwrap()
                    .value("arrow_data_type", "UInt64")
                    .unwrap()
                    .value("storage_encoding", "arrow.u64")
                    .unwrap()
                    .value("metadata_class", "contractual")
                    .unwrap()
                    .null("extension_name")
                    .unwrap()
                    .null("extension_metadata")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Program)
                    .value("program_id", mapping_program_id)
                    .unwrap()
                    .value("name", "bound storage mapping")
                    .unwrap()
                    .value("program_kind", "physical-binding")
                    .unwrap()
                    .null("result_semantic_type_id")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
            ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::PhysicalBinding)
                    .value("physical_binding_id", binding_id)
                    .unwrap()
                    .value("logical_relation_id", logical_relation_id)
                    .unwrap()
                    .value("storage_relation_id", storage_relation_id)
                    .unwrap()
                    .value("mapping_program_id", mapping_program_id)
                    .unwrap()
                    .value("compatibility_mode", "exact")
                    .unwrap()
                    .build(&metamodel)
                    .unwrap(),
            ),
        ];
        let decision = ModelDecision::new(
            "test.decision.bound-schema",
            "epoch-tests",
            "exercise model-derived catalog construction",
            "install one complete physical schema binding",
            operations,
        )
        .unwrap();
        let migration = ModelMigration::new(
            "test.migration.bound-schema",
            None,
            "model.bootstrap.fabric-release-epoch-test",
            "test.model-epoch.bound-schema",
            1,
            "epoch-tests",
            vec![decision],
        )
        .unwrap();
        let epoch = replay_model_epoch(runtime, &[migration]);
        let binding = ModelPhysicalBindingRow {
            physical_binding_id: binding_id.to_owned(),
            mapping_program_id: mapping_program_id.to_owned(),
            source_schema_identity: "provider:bound-values:v1".to_owned(),
            logical_relation_id: logical_relation_id.to_owned(),
            storage_relation_id: storage_relation_id.to_owned(),
            compatibility: SchemaCompatibility::Exact,
            column_mapping_mode: crate::schema_contract::ColumnMappingMode::FieldId,
            deletion_vector_behavior:
                crate::schema_contract::DeletionVectorBehavior::AppliedByProvider,
            field_bindings: vec![crate::schema_contract::ModelPhysicalFieldBindingRow {
                logical_field_id: logical_field_id.to_owned(),
                storage_field_id: storage_field_id.to_owned(),
                projection_index: 0,
                filter_index: 0,
                statistics_index: 0,
            }],
        };
        (epoch, vec![binding])
    }

    #[tokio::test]
    async fn seals_model_relations_and_derived_system_relations() {
        let runtime = FabricEpochRuntimeConfig::default();
        let model = model_epoch(&runtime);
        let epoch = FabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([1; 16]),
            Arc::clone(&model),
            runtime,
        )
        .unwrap()
        .seal()
        .await
        .unwrap();

        assert_eq!(epoch.model_epoch_id(), model.model_epoch_id());
        assert_eq!(
            epoch.catalog_observation().num_rows(),
            1 + FabricSchemaRole::ALL.len() + ModelRelation::ALL.len() + 2
        );
        let table_names = epoch
            .catalog_observation()
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let source_schema_identities = epoch
            .catalog_observation()
            .column(6)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let catalog_row = (0..epoch.catalog_observation().num_rows())
            .find(|row| {
                !table_names.is_null(*row) && table_names.value(*row) == CATALOG_OBJECT_TABLE
            })
            .unwrap();
        assert_eq!(
            source_schema_identities.value(catalog_row),
            format!("catalog:{}", epoch_identity_text(*epoch.identity()))
        );
        let result = epoch
            .scan_model(&ModelTableScan::all(ModelRelation::Relation))
            .await
            .unwrap();
        assert_eq!(
            result.batches(),
            &[model.relations().batch(ModelRelation::Relation).clone()]
        );
    }

    #[tokio::test]
    async fn model_physical_bindings_derive_catalog_registration_and_close_at_seal() {
        let runtime = FabricEpochRuntimeConfig::default();
        let (model, bindings) = schema_bound_model_epoch(&runtime);
        assert!(matches!(
            FabricEpochBuilder::try_new(
                FabricEpochId::from_bytes([31; 16]),
                Arc::clone(&model),
                runtime.clone(),
            ),
            Err(FabricEpochError::SchemaContract(_))
        ));

        let unregistered = FabricEpochBuilder::try_new_with_physical_bindings(
            FabricEpochId::from_bytes([32; 16]),
            Arc::clone(&model),
            runtime.clone(),
            bindings.clone(),
        )
        .unwrap();
        assert!(matches!(
            unregistered.seal().await,
            Err(FabricEpochError::ModelBinding(_))
        ));

        let mut builder = FabricEpochBuilder::try_new_with_physical_bindings(
            FabricEpochId::from_bytes([33; 16]),
            model,
            runtime,
            bindings,
        )
        .unwrap();
        let contract = Arc::clone(&builder.model_contracts["test.binding.bound-storage"]);
        let provider = Arc::new(
            MemTable::try_new(
                Arc::clone(contract.logical_schema()),
                vec![vec![RecordBatch::new_empty(Arc::clone(
                    contract.logical_schema(),
                ))]],
            )
            .unwrap(),
        );
        builder
            .register_model_bound_provider(
                "test.binding.bound-storage",
                "datafusion.mem_table.model-bound",
                provider,
            )
            .unwrap();
        let epoch = builder.seal().await.unwrap();
        assert!(epoch.table_contracts.contains_key(&RegisteredTableKey {
            role: FabricSchemaRole::Fact,
            table_name: "bound_values".to_owned(),
        }));
    }

    #[tokio::test]
    async fn computed_proof_relations_require_model_supplied_complete_bindings() {
        let runtime = FabricEpochRuntimeConfig::default();
        let identity = FabricEpochId::from_bytes([13; 16]);
        let mut builder =
            FabricEpochBuilder::try_new(identity, model_epoch(&runtime), runtime).unwrap();
        let relations = test_relations_for_epoch(identity);
        let bindings = ProofRelationKind::ALL
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                ProofCatalogBinding::try_new(
                    kind,
                    format!("proof_relation_{index}"),
                    format!("model-proof-schema-{index}"),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();

        builder
            .register_proof_relations(&relations, &bindings)
            .unwrap();
        let epoch = builder.seal().await.unwrap();

        for binding in bindings {
            let batches = epoch
                .session
                .context()
                .table(TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Proof.as_str(),
                    binding.table_name(),
                ))
                .await
                .unwrap()
                .collect()
                .await
                .unwrap();
            assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
        }
    }

    #[tokio::test]
    async fn bounded_projection_preserves_an_explicit_empty_schema() {
        let runtime = FabricEpochRuntimeConfig::default();
        let epoch = FabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([2; 16]),
            model_epoch(&runtime),
            runtime,
        )
        .unwrap()
        .seal()
        .await
        .unwrap();

        let result = epoch
            .scan_model(
                &ModelTableScan::all(ModelRelation::UnknownRule)
                    .with_projection(vec![0])
                    .with_limit(0),
            )
            .await
            .unwrap();
        assert_eq!(result.schema().fields().len(), 1);
        assert_eq!(result.row_count(), 0);
    }

    #[tokio::test]
    async fn executes_model_bound_program_through_the_private_epoch_catalog() {
        use arrow_array::UInt64Array;

        let runtime = FabricEpochRuntimeConfig::default();
        let model = relational_program_model_epoch(&runtime);
        let mut builder =
            FabricEpochBuilder::try_new(FabricEpochId::from_bytes([8; 16]), model, runtime)
                .unwrap();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::UInt64,
            false,
        )]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(UInt64Array::from(vec![3, 5, 8]))],
        )
        .unwrap();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let contract = Arc::new(
            SchemaContract::try_new(
                "facts:fact-values:v1",
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Fact.as_str(),
                    "fact_values",
                ),
                Arc::clone(&schema),
                Arc::clone(&schema),
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );
        builder
            .register_provider(
                FabricSchemaRole::Fact,
                "fact_values",
                "datafusion.mem_table",
                provider,
                contract,
            )
            .unwrap();
        let epoch = builder.seal().await.unwrap();
        let relation_id = RelationId::new("test.relation.fact_values").unwrap();
        let field_id = FieldId::new("test.field.fact_values.value").unwrap();
        let program = RelationalProgram {
            root: RelationalExpression::Input(relation_id.clone()),
            output_fields: vec![field_id],
        };

        let result = epoch.execute_relational_program(&program).await.unwrap();

        assert_eq!(result.row_count(), 3);
        assert_eq!(result.schema().field(0).name(), "value");
        assert!(
            result
                .observations()
                .dependencies
                .contains(&CompilationDependency::Relation(relation_id))
        );
        let values = result.batches()[0]
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        assert_eq!(values.values(), &[3, 5, 8]);
    }

    #[tokio::test]
    async fn public_registration_installs_the_schema_contract_provider_adapter() {
        use arrow_array::Int32Array;

        let runtime = FabricEpochRuntimeConfig::default();
        let mut builder = FabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([9; 16]),
            model_epoch(&runtime),
            runtime,
        )
        .unwrap();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int32,
            false,
        )]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int32Array::from(vec![7]))],
        )
        .unwrap();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let contract = Arc::new(
            SchemaContract::try_new(
                "facts:test-provider:v1",
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Fact.as_str(),
                    "test_provider",
                ),
                Arc::clone(&schema),
                Arc::clone(&schema),
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );

        builder
            .register_provider(
                FabricSchemaRole::Fact,
                "test_provider",
                "datafusion.mem_table",
                provider,
                contract,
            )
            .unwrap();
        let registered = builder.schemas[&FabricSchemaRole::Fact]
            .table("test_provider")
            .await
            .unwrap()
            .unwrap();
        let state = SessionContext::new().state();
        registered
            .scan(&state, None, &[], Some(1))
            .await
            .expect("registered provider remains executable");

        let epoch = builder.seal().await.unwrap();
        let batches = epoch
            .session
            .context()
            .table(TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Fact.as_str(),
                "test_provider",
            ))
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
    }

    #[tokio::test]
    async fn exact_delta_recipes_use_the_epoch_session_before_seal() {
        let temporary = TempDir::new().unwrap();
        let table_path = temporary.path().join("exact-epoch-table");
        fs::create_dir_all(&table_path).unwrap();
        let root = url::Url::from_directory_path(&table_path).unwrap();
        let schema = Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)]));
        let kernel: deltalake::kernel::StructType = schema.as_ref().try_into_kernel().unwrap();
        CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("exact_epoch_table")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .await
            .unwrap();
        let table = DeltaTableBuilder::from_url(root.clone())
            .unwrap()
            .with_version(0)
            .load()
            .await
            .unwrap();
        let pin = ExactDeltaPin::new(&root, 0).unwrap();
        let validated = ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &pin).unwrap();

        let runtime = FabricEpochRuntimeConfig::default();
        let mut builder = FabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([12; 16]),
            model_epoch(&runtime),
            runtime,
        )
        .unwrap();
        let contract = |table_name: &'static str| {
            Arc::new(
                SchemaContract::try_new(
                    format!("delta-exact:{table_name}:v1"),
                    TableReference::full(
                        FABRIC_CATALOG,
                        FabricSchemaRole::Storage.as_str(),
                        table_name,
                    ),
                    Arc::clone(&schema),
                    Arc::clone(&schema),
                    vec![FieldIndexMapping::direct(0, 0)],
                )
                .unwrap(),
            )
        };
        builder
            .register_exact_delta_snapshot(
                FabricSchemaRole::Storage,
                "snapshot_recipe",
                &pin,
                validated,
                contract("snapshot_recipe"),
            )
            .await
            .unwrap();
        builder
            .register_exact_delta_log_store(
                FabricSchemaRole::Storage,
                "log_store_recipe",
                &pin,
                table.log_store(),
                contract("log_store_recipe"),
            )
            .await
            .unwrap();

        let epoch = builder.seal().await.unwrap();
        for table_name in ["snapshot_recipe", "log_store_recipe"] {
            let batches = epoch
                .session
                .context()
                .table(TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Storage.as_str(),
                    table_name,
                ))
                .await
                .unwrap()
                .collect()
                .await
                .unwrap();
            assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 0);
        }
    }

    #[tokio::test]
    async fn public_registration_rejects_provider_schema_drift_at_scan_time() {
        let runtime = FabricEpochRuntimeConfig::default();
        let mut builder = FabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([10; 16]),
            model_epoch(&runtime),
            runtime,
        )
        .unwrap();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int32,
            false,
        )]));
        let provider = Arc::new(MutableSchemaProvider {
            schema: RwLock::new(Arc::clone(&schema)),
        });
        let contract = Arc::new(
            SchemaContract::try_new(
                "facts:mutable-provider:v1",
                TableReference::full(
                    FABRIC_CATALOG,
                    FabricSchemaRole::Fact.as_str(),
                    "mutable_provider",
                ),
                Arc::clone(&schema),
                Arc::clone(&schema),
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );
        builder
            .register_provider(
                FabricSchemaRole::Fact,
                "mutable_provider",
                "test.mutable",
                Arc::clone(&provider) as Arc<dyn TableProvider>,
                contract,
            )
            .unwrap();
        provider.replace_schema(Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Utf8,
            false,
        )])));

        let registered = builder.schemas[&FabricSchemaRole::Fact]
            .table("mutable_provider")
            .await
            .unwrap()
            .unwrap();
        let error = registered
            .scan(&SessionContext::new().state(), None, &[], None)
            .await
            .expect_err("schema-bound provider must reject post-registration drift");
        assert!(error.to_string().contains("schema"));
    }

    #[test]
    fn compiler_release_must_pin_the_effective_runtime() {
        let runtime = FabricEpochRuntimeConfig::default();
        let model = model_epoch(&runtime);
        let changed = FabricEpochRuntimeConfig::try_new(
            128 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            32,
            16,
            8_192,
            1,
            true,
        )
        .unwrap();
        let error = FabricEpochBuilder::try_new(FabricEpochId::from_bytes([3; 16]), model, changed)
            .err()
            .expect("mismatched runtime must fail");
        assert!(matches!(
            error,
            FabricEpochError::RuntimeIdentityMismatch { .. }
        ));
    }

    #[test]
    fn runtime_installs_the_exact_bounded_datafusion_caches() {
        let config = FabricEpochRuntimeConfig::default();
        let runtime = config.runtime_env().unwrap();
        let policy = config.cache_policy();
        assert_eq!(
            runtime.cache_manager.get_metadata_cache_limit(),
            policy.metadata_cache_bytes()
        );
        assert_eq!(
            runtime.cache_manager.get_file_statistic_cache_limit(),
            policy.file_statistics_cache_bytes()
        );
        assert_eq!(
            runtime.cache_manager.get_list_files_cache_limit(),
            policy.object_list_cache_bytes()
        );
        assert_eq!(
            runtime.cache_manager.get_list_files_cache_ttl(),
            Some(std::time::Duration::from_secs(
                policy.object_list_cache_ttl_seconds()
            ))
        );
        assert!(config.identity().contains(&policy.identity_fragment()));
    }

    #[test]
    fn role_schema_set_is_closed_and_private_storage_is_explicit() {
        let roles = FabricSchemaRole::ALL
            .into_iter()
            .map(FabricSchemaRole::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(roles.len(), FabricSchemaRole::ALL.len());
        assert!(roles.contains("_storage"));
        assert!(!roles.contains(INFORMATION_SCHEMA));
    }
}
