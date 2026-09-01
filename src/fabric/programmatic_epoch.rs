//! Sealed fabric epochs assembled directly from provider contracts and native
//! DataFusion transformations.
//!
//! This is the target epoch path. It deliberately has no predecessor replay,
//! bootstrap catalog, SQL definition, or serialized-plan input.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use datafusion::catalog::TableProvider;
use datafusion::catalog::{
    CatalogProvider as _, CatalogProviderList as _, MemoryCatalogProvider,
    MemoryCatalogProviderList, MemorySchemaProvider,
};
use datafusion::common::TableReference;
use datafusion::datasource::source_as_provider;
use datafusion::execution::SessionStateBuilder;
use datafusion::execution::runtime_env::RuntimeEnv;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::collect;
use datafusion::prelude::SessionContext;
use deltalake::delta_datafusion::planner::DeltaPlanner;

use crate::relational_program::{
    CompilationObservations, ProgramBindings, ProgramRelationContract, RelationId, RelationInput,
    RelationalProgram, RelationalProgramCompiler, RelationalProgramError,
};

use super::activation::TableVersionSet;
use super::datafusion_cache::{
    CachedLogicalPlan, EpochLogicalPlanCache, LogicalPlanAuthorityBuilder,
    LogicalPlanAuthorityFingerprint, LogicalPlanCacheError, LogicalPlanCacheKey,
    LogicalPlanCacheObservation, LogicalPlanCacheOutcome, LogicalPlanCacheScope,
    LogicalPlanExecutionObservation, execution_observation, frame_schema_contract,
    frame_session_logical_authority, validate_logical_plan_references,
};
use super::epoch_runtime::{
    FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole, epoch_identity_text,
};
use super::programmatic_observation_delta::{
    ProgrammaticObservationDeltaError, ProgrammaticObservationDeltaPublication,
    ProgrammaticObservationDeltaTargets, ProgrammaticObservationHistoricization,
    ProgrammaticObservationHistoricizationFailure, ProgrammaticObservationProvisionError,
    ProgrammaticObservationWriteIdentity, historicize_programmatic_observations,
    provision_programmatic_observation_histories, reopen_programmatic_observations,
};
use super::programmatic_schema::{
    ProgrammaticRelationId, ProgrammaticSchemaAssembly, ProgrammaticSchemaError,
    ProgrammaticTransformation, ProviderInput, SealedRelationBinding,
};

/// Mutable owner of one programmatic candidate session.
pub struct ProgrammaticFabricEpochBuilder {
    identity: FabricEpochId,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_env: Arc<RuntimeEnv>,
    assembly: ProgrammaticSchemaAssembly,
}

impl ProgrammaticFabricEpochBuilder {
    /// Create a fresh candidate with the exact runtime and role-schema
    /// isolation boundary. The legacy `model` schema is intentionally absent.
    pub(crate) fn try_new(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
    ) -> Result<Self, ProgrammaticFabricEpochError> {
        let runtime_env = runtime_config.runtime_env()?;
        let catalog_list = Arc::new(MemoryCatalogProviderList::new());
        let catalog = Arc::new(MemoryCatalogProvider::new());
        if catalog_list
            .register_catalog(FABRIC_CATALOG.to_owned(), Arc::clone(&catalog) as _)
            .is_some()
        {
            return Err(ProgrammaticFabricEpochError::CatalogClosure(
                "fresh catalog list already contained codefabric".to_owned(),
            ));
        }
        for role in FabricSchemaRole::ALL {
            if catalog
                .register_schema(role.as_str(), Arc::new(MemorySchemaProvider::new()))?
                .is_some()
            {
                return Err(ProgrammaticFabricEpochError::CatalogClosure(format!(
                    "fresh catalog already contained schema {}",
                    role.as_str()
                )));
            }
        }
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_config(runtime_config.session_config())
            .with_runtime_env(Arc::clone(&runtime_env))
            .with_catalog_list(catalog_list)
            .with_query_planner(DeltaPlanner::new())
            .build();
        Ok(Self {
            identity,
            runtime_config,
            runtime_env,
            assembly: ProgrammaticSchemaAssembly::new(state),
        })
    }

    #[must_use]
    pub const fn identity(&self) -> &FabricEpochId {
        &self.identity
    }

    /// Register one exact code-fact or execution-input provider.
    pub(crate) fn register_provider(
        &mut self,
        input: ProviderInput,
    ) -> Result<(), ProgrammaticFabricEpochError> {
        self.assembly.register_provider(input)?;
        Ok(())
    }

    /// Register one typed native transformation for dependency-ordered build.
    pub(crate) fn add_transformation(
        &mut self,
        transformation: Arc<dyn ProgrammaticTransformation>,
    ) -> Result<(), ProgrammaticFabricEpochError> {
        self.assembly.add_transformation(transformation)?;
        Ok(())
    }

    /// Transfer the assembly for a consuming admission step. The matching
    /// constructor retains the same identity/runtime ownership.
    #[must_use]
    pub(crate) fn into_assembly_parts(
        self,
    ) -> (
        FabricEpochId,
        FabricEpochRuntimeConfig,
        Arc<RuntimeEnv>,
        ProgrammaticSchemaAssembly,
    ) {
        (
            self.identity,
            self.runtime_config,
            self.runtime_env,
            self.assembly,
        )
    }

    /// Reconstitute ownership after a consuming admission operation without
    /// rebuilding or replaying the candidate session.
    #[must_use]
    pub(crate) fn from_assembly_parts(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        runtime_env: Arc<RuntimeEnv>,
        assembly: ProgrammaticSchemaAssembly,
    ) -> Self {
        Self {
            identity,
            runtime_config,
            runtime_env,
            assembly,
        }
    }

    /// Create the five empty, stable Delta histories from the observation
    /// contracts derived by this candidate. Existing histories must be opened
    /// from an exact prior publication instead.
    pub(crate) async fn provision_observation_histories(
        &self,
        roots: BTreeMap<ProgrammaticRelationId, url::Url>,
    ) -> Result<ProgrammaticObservationDeltaTargets, ProgrammaticFabricEpochError> {
        Ok(provision_programmatic_observation_histories(&self.assembly, roots).await?)
    }

    /// Build all transformations, append the candidate's five observation
    /// relations to their stable Delta histories, rebind the exact committed
    /// versions in this same session, and seal only after fixed-point proof.
    pub(crate) async fn seal(
        self,
        write_identity: ProgrammaticObservationWriteIdentity,
        targets: ProgrammaticObservationDeltaTargets,
    ) -> Result<ProgrammaticFabricEpoch, ProgrammaticFabricEpochError> {
        if write_identity.epoch_id() != self.identity {
            return Err(
                ProgrammaticFabricEpochError::ObservationEpochIdentityMismatch {
                    builder: self.identity,
                    write: write_identity.epoch_id(),
                },
            );
        }
        let Self {
            identity,
            runtime_config,
            runtime_env,
            assembly,
        } = self;
        let historicized =
            historicize_programmatic_observations(assembly, write_identity, targets).await?;
        Self::finish_historicized(identity, runtime_config, runtime_env, historicized)
    }

    /// Rebuild a sealed candidate from an activation-selected exact Delta
    /// version vector in a fresh `SessionContext`.
    ///
    /// No table is provisioned or written and no latest version is resolved.
    /// The fresh candidate must still contain the same provider inputs and
    /// transformations; catalog observations prove that correspondence before
    /// the epoch becomes sealable.
    pub(crate) async fn reopen(
        self,
        table_versions: Arc<TableVersionSet>,
    ) -> Result<ProgrammaticFabricEpoch, ProgrammaticFabricEpochError> {
        let Self {
            identity,
            runtime_config,
            runtime_env,
            assembly,
        } = self;
        let historicized =
            reopen_programmatic_observations(assembly, identity, table_versions).await?;
        Self::finish_historicized(identity, runtime_config, runtime_env, historicized)
    }

    fn finish_historicized(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        runtime_env: Arc<RuntimeEnv>,
        historicized: ProgrammaticObservationHistoricization,
    ) -> Result<ProgrammaticFabricEpoch, ProgrammaticFabricEpochError> {
        let (sealed, observation_publication) = historicized.into_parts();
        let (session, relations) = sealed.into_parts().into_components();
        let authority_id = candidate_session_authority(identity, &runtime_config.identity());
        let contracts = relations
            .iter()
            .map(|(relation_id, binding)| {
                Ok(ProgramRelationContract {
                    relation_id: RelationId::new(relation_id.as_str())?,
                    table_reference: binding.table_reference.clone(),
                    contract: Arc::clone(&binding.contract),
                })
            })
            .collect::<Result<Vec<_>, RelationalProgramError>>()?;
        let program_bindings = Arc::new(ProgramBindings::try_new(authority_id, contracts)?);
        let logical_plan_cache = Arc::new(EpochLogicalPlanCache::new(
            runtime_config.cache_policy().logical_plan_entries(),
            runtime_config.cache_policy().logical_plan_bytes(),
        ));
        let state = session.state();
        if !Arc::ptr_eq(state.runtime_env(), &runtime_env) {
            return Err(ProgrammaticFabricEpochError::RuntimeAuthorityDrift);
        }
        let logical_plan_authority = derive_epoch_logical_plan_authority(
            identity,
            &runtime_config,
            &state,
            &relations,
            &observation_publication,
            &program_bindings,
        )?;
        Ok(ProgrammaticFabricEpoch {
            identity,
            runtime_config,
            runtime_env,
            session,
            relations,
            observation_publication,
            program_bindings,
            logical_plan_authority,
            logical_plan_cache,
            #[cfg(test)]
            observation_history_root: None,
        })
    }

    /// Test convenience that still exercises the real five-table Delta route.
    #[cfg(test)]
    pub async fn seal_for_test(
        self,
    ) -> Result<ProgrammaticFabricEpoch, ProgrammaticFabricEpochError> {
        use std::fs;

        use tempfile::TempDir;

        use super::programmatic_schema::{
            DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
            PROVENANCE_OBSERVATION_RELATION_ID, RELATION_OBSERVATION_RELATION_ID,
            SCHEMA_OBSERVATION_RELATION_ID,
        };

        let temporary = TempDir::new().map_err(|source| {
            ProgrammaticFabricEpochError::CatalogClosure(format!(
                "cannot create test observation-history root: {source}"
            ))
        })?;
        let mut roots = BTreeMap::new();
        for relation in [
            RELATION_OBSERVATION_RELATION_ID,
            FIELD_OBSERVATION_RELATION_ID,
            SCHEMA_OBSERVATION_RELATION_ID,
            DEPENDENCY_OBSERVATION_RELATION_ID,
            PROVENANCE_OBSERVATION_RELATION_ID,
        ] {
            let path = temporary.path().join(relation.replace('.', "_"));
            fs::create_dir_all(&path).map_err(|source| {
                ProgrammaticFabricEpochError::CatalogClosure(format!(
                    "cannot create test history root for {relation}: {source}"
                ))
            })?;
            roots.insert(
                ProgrammaticRelationId::new(relation),
                url::Url::from_directory_path(path).map_err(|()| {
                    ProgrammaticFabricEpochError::CatalogClosure(format!(
                        "test history root for {relation} is not a file URL"
                    ))
                })?,
            );
        }
        let targets = self.provision_observation_histories(roots).await?;
        let mut transaction = [0_u8; 32];
        transaction[..16].copy_from_slice(self.identity.as_bytes());
        transaction[16..].copy_from_slice(self.identity.as_bytes());
        let identity = ProgrammaticObservationWriteIdentity::new(
            self.identity,
            super::command::OperationId::from_bytes(*self.identity.as_bytes()),
            super::command::WriterGeneration::new(1).expect("one is a writer generation"),
            super::command::TransactionRef::from_bytes(transaction),
        );
        let mut epoch = self.seal(identity, targets).await?;
        epoch.observation_history_root = Some(temporary);
        Ok(epoch)
    }
}

fn derive_epoch_logical_plan_authority(
    identity: FabricEpochId,
    runtime_config: &FabricEpochRuntimeConfig,
    state: &datafusion::execution::SessionState,
    relations: &BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    publication: &ProgrammaticObservationDeltaPublication,
    program_bindings: &ProgramBindings,
) -> Result<LogicalPlanAuthorityFingerprint, ProgrammaticFabricEpochError> {
    let mut authority = LogicalPlanAuthorityBuilder::new(b"programmatic-fabric-epoch-authority.v1");
    authority.frame(identity.as_bytes());
    authority.frame_str(&runtime_config.identity());
    authority.frame(publication.table_version_set_ref().as_bytes());
    authority.frame_usize(publication.table_version_set().len());
    for (relation_id, pin) in publication.table_versions() {
        authority.frame_str(relation_id);
        authority.frame_str(pin.canonical_root().as_str());
        authority.frame_u64(pin.version());
    }
    authority.frame_str(program_bindings.authority_id());
    authority.frame_usize(relations.len());
    for (relation_id, binding) in relations {
        frame_schema_contract(&mut authority, relation_id.as_str(), &binding.contract)
            .map_err(ProgrammaticFabricEpochError::LogicalPlanAuthority)?;
        let actual_schema = Arc::new(binding.actual_datafusion_schema.as_arrow().clone());
        authority
            .frame_schema(&actual_schema)
            .map_err(ProgrammaticFabricEpochError::LogicalPlanAuthority)?;
    }
    // These are execution-local capability identities. The complete reversible table-version
    // set and schema contracts above remain the semantic/durable authorities.
    authority.frame_arc_identity(state.catalog_list());
    authority.frame_arc_identity(state.runtime_env());
    frame_session_logical_authority(
        &mut authority,
        state,
        "deltalake::delta_datafusion::planner::DeltaPlanner@43a0cf10/datafusion-55",
    );
    Ok(authority.finish())
}

fn candidate_session_authority(identity: FabricEpochId, runtime_identity: &str) -> String {
    const DOMAIN: &[u8] = b"codefabric.candidate-session.runtime.v1";
    let mut digest = blake3::Hasher::new();
    digest.update(&(DOMAIN.len() as u64).to_be_bytes());
    digest.update(DOMAIN);
    digest.update(&(runtime_identity.len() as u64).to_be_bytes());
    digest.update(runtime_identity.as_bytes());
    format!(
        "candidate-session:{}:runtime-b3:{}",
        epoch_identity_text(identity),
        digest.finalize().to_hex()
    )
}

/// Sealed session authority for one exact set of provider facts and
/// programmatic transformations.
pub struct ProgrammaticFabricEpoch {
    identity: FabricEpochId,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_env: Arc<RuntimeEnv>,
    session: SessionContext,
    relations: BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    observation_publication: ProgrammaticObservationDeltaPublication,
    program_bindings: Arc<ProgramBindings>,
    logical_plan_authority: LogicalPlanAuthorityFingerprint,
    logical_plan_cache: Arc<EpochLogicalPlanCache>,
    #[cfg(test)]
    observation_history_root: Option<tempfile::TempDir>,
}

impl fmt::Debug for ProgrammaticFabricEpoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticFabricEpoch")
            .field("identity", &self.identity)
            .field("schema_authority", &self.program_bindings.authority_id())
            .field("relation_count", &self.relations.len())
            .finish_non_exhaustive()
    }
}

impl ProgrammaticFabricEpoch {
    #[must_use]
    pub const fn identity(&self) -> &FabricEpochId {
        &self.identity
    }

    #[must_use]
    pub fn schema_authority_id(&self) -> &str {
        self.program_bindings.authority_id()
    }

    #[must_use]
    pub const fn observation_publication(&self) -> &ProgrammaticObservationDeltaPublication {
        &self.observation_publication
    }

    #[must_use]
    pub const fn program_bindings(&self) -> &Arc<ProgramBindings> {
        &self.program_bindings
    }

    #[must_use]
    pub fn logical_plan_cache_observation(&self) -> LogicalPlanCacheObservation {
        self.logical_plan_cache.observation()
    }

    pub(super) const fn logical_plan_cache(&self) -> &Arc<EpochLogicalPlanCache> {
        &self.logical_plan_cache
    }

    pub(super) const fn logical_plan_authority(&self) -> LogicalPlanAuthorityFingerprint {
        self.logical_plan_authority
    }

    #[must_use]
    pub fn relation(&self, relation_id: &ProgrammaticRelationId) -> Option<&SealedRelationBinding> {
        self.relations.get(relation_id)
    }

    /// Enumerate every stable relation identity sealed into this exact session.
    pub fn relation_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProgrammaticRelationId> + DoubleEndedIterator {
        self.relations.keys()
    }

    /// Resolve the exact provider and executable contract for a stable relation
    /// identity without exposing a parent catalog or mutable session handle.
    pub(super) async fn resolve_sealed_relation(
        &self,
        relation_id: &ProgrammaticRelationId,
    ) -> Result<
        (
            TableReference,
            Arc<dyn TableProvider>,
            Arc<crate::schema_contract::SchemaContract>,
            Option<Arc<LogicalPlan>>,
        ),
        ProgrammaticFabricEpochError,
    > {
        let binding = self.relations.get(relation_id).ok_or_else(|| {
            ProgrammaticFabricEpochError::CatalogClosure(format!(
                "sealed relation {} is absent",
                relation_id.as_str()
            ))
        })?;
        let provider = self
            .context()
            .table_provider(binding.table_reference.clone())
            .await?;
        let actual = provider.schema();
        if actual.as_ref() != binding.contract.logical_schema().as_ref() {
            return Err(ProgrammaticFabricEpochError::CatalogClosure(format!(
                "sealed provider schema drifted for relation {}",
                relation_id.as_str()
            )));
        }
        Ok((
            binding.table_reference.clone(),
            provider,
            Arc::clone(&binding.contract),
            binding.logical_plan.as_ref().map(Arc::clone),
        ))
    }

    /// Prove that a reduced child owns fresh runtime and catalog authorities.
    pub(super) fn child_authorities_are_distinct(
        &self,
        runtime: &Arc<RuntimeEnv>,
        catalog_list: &Arc<dyn datafusion::catalog::CatalogProviderList>,
    ) -> bool {
        !Arc::ptr_eq(&self.runtime_env, runtime)
            && !Arc::ptr_eq(self.session.state().catalog_list(), catalog_list)
            && !Arc::ptr_eq(&self.runtime_env.memory_pool, &runtime.memory_pool)
            && !Arc::ptr_eq(&self.runtime_env.disk_manager, &runtime.disk_manager)
            && !Arc::ptr_eq(&self.runtime_env.cache_manager, &runtime.cache_manager)
            && !Arc::ptr_eq(
                &self.runtime_env.object_store_registry,
                &runtime.object_store_registry,
            )
    }

    /// Execute a typed program using catalog scans and schema bindings from
    /// this exact sealed session.
    pub(crate) async fn execute_relational_program(
        &self,
        program: &RelationalProgram,
    ) -> Result<ProgrammaticFabricProgramResult, ProgrammaticFabricEpochError> {
        let context = self.session.clone();
        let cache_key = LogicalPlanCacheKey::new(
            self.identity,
            self.observation_publication.table_version_set_ref(),
            self.program_bindings.authority_id(),
            self.runtime_config.identity(),
            self.logical_plan_authority,
            LogicalPlanCacheScope::Epoch,
            program,
        );
        let (cached, cache_outcome) = if let Some(cached) = self.logical_plan_cache.get(&cache_key)
        {
            (cached, LogicalPlanCacheOutcome::Hit)
        } else {
            let catalog_inputs = RelationalProgramCompiler::bind_catalog_inputs_with_bindings(
                &self.program_bindings,
                program,
            )?;
            let mut inputs = Vec::with_capacity(catalog_inputs.len());
            for catalog_input in catalog_inputs {
                let relation_id = ProgrammaticRelationId::new(catalog_input.relation_id.as_str());
                let sealed = self.relations.get(&relation_id).ok_or_else(|| {
                    ProgrammaticFabricEpochError::CatalogClosure(format!(
                        "relation {} is absent from the sealed session",
                        catalog_input.relation_id.as_str()
                    ))
                })?;
                if sealed.table_reference != catalog_input.table_reference {
                    return Err(ProgrammaticFabricEpochError::CatalogClosure(format!(
                        "relation {} resolves to {}, expected {}",
                        catalog_input.relation_id.as_str(),
                        sealed.table_reference,
                        catalog_input.table_reference
                    )));
                }
                let plan = context
                    .table(catalog_input.table_reference)
                    .await?
                    .into_unoptimized_plan();
                inputs.push(RelationInput {
                    relation_id: catalog_input.relation_id,
                    plan,
                });
            }
            let compiled = RelationalProgramCompiler::compile_with_bindings(
                &self.program_bindings,
                inputs,
                program,
            )?;
            let schema = Arc::new(compiled.plan.schema().as_arrow().clone());
            let state = context.state();
            let optimized = state.optimize(&compiled.plan)?;
            let cached = self.logical_plan_cache.try_insert(
                cache_key,
                CachedLogicalPlan::new(compiled.plan, optimized, schema, compiled.observations),
            )?;
            (cached, LogicalPlanCacheOutcome::Miss)
        };
        let optimized_schema = cached.optimized_plan().schema().as_arrow();
        if cached.compiled_plan().schema().as_arrow() != cached.output_schema().as_ref()
            || optimized_schema != cached.output_schema().as_ref()
        {
            return Err(ProgrammaticFabricEpochError::CachedPlanSchemaDrift);
        }
        let state = context.state();
        let mut admitted_providers = BTreeMap::new();
        for binding in self.relations.values() {
            let provider = context
                .table_provider(binding.table_reference.clone())
                .await?;
            admitted_providers.insert(binding.table_reference.clone(), provider);
        }
        for plan in [cached.compiled_plan(), cached.optimized_plan()] {
            validate_logical_plan_references(plan, &state, false, |scan| {
                let Some(admitted) = admitted_providers.get(&scan.table_name) else {
                    return Err(format!(
                        "cached scan {} is absent from the sealed epoch catalog",
                        scan.table_name
                    ));
                };
                let provider = source_as_provider(&scan.source).map_err(|error| {
                    format!(
                        "cached scan {} has a non-provider table source: {error}",
                        scan.table_name
                    )
                })?;
                if Arc::ptr_eq(&provider, admitted) {
                    Ok(())
                } else {
                    Err(format!(
                        "cached scan {} retains a different provider capability",
                        scan.table_name
                    ))
                }
            })
            .map_err(ProgrammaticFabricEpochError::CachedPlanAuthorityDrift)?;
        }
        let physical_plan = state
            .query_planner()
            .create_physical_plan(cached.optimized_plan(), &state)
            .await?;
        let schema = Arc::clone(cached.output_schema());
        let physical_schema = physical_plan.schema();
        if physical_schema.as_ref() != schema.as_ref() {
            return Err(ProgrammaticFabricEpochError::OutputSchemaDrift {
                expected: schema,
                actual: physical_schema,
            });
        }
        let physical_batches = collect(physical_plan, state.task_ctx()).await?;
        let schema = Arc::clone(cached.output_schema());
        let observations = cached.observations().clone();
        let plan = execution_observation(&cached, cache_outcome);
        let mut batches = Vec::with_capacity(physical_batches.len());
        for batch in physical_batches {
            let actual = batch.schema();
            if actual.as_ref() != schema.as_ref() {
                return Err(ProgrammaticFabricEpochError::OutputSchemaDrift {
                    expected: Arc::clone(&schema),
                    actual,
                });
            }
            batches.push(batch);
        }
        Ok(ProgrammaticFabricProgramResult {
            schema,
            batches,
            observations,
            plan,
        })
    }

    #[must_use]
    pub fn runtime_configuration_identity(&self) -> String {
        self.runtime_config.identity()
    }

    #[must_use]
    pub fn memory_reserved_bytes(&self) -> usize {
        self.runtime_env.memory_pool.reserved()
    }

    pub(super) fn context(&self) -> SessionContext {
        self.session.clone()
    }
}

/// Arrow-native result and causal compiler observations.
pub struct ProgrammaticFabricProgramResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    observations: CompilationObservations,
    plan: LogicalPlanExecutionObservation,
}

impl ProgrammaticFabricProgramResult {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub const fn observations(&self) -> &CompilationObservations {
        &self.observations
    }

    #[must_use]
    pub const fn plan_observation(&self) -> LogicalPlanExecutionObservation {
        self.plan
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }
}

/// Fail-closed candidate construction, sealing, and execution failures.
#[derive(Debug, thiserror::Error)]
pub enum ProgrammaticFabricEpochError {
    #[error("programmatic candidate catalog is not closed: {0}")]
    CatalogClosure(String),
    #[error("candidate session runtime authority changed during assembly")]
    RuntimeAuthorityDrift,
    #[error("observation write epoch {write:?} differs from candidate epoch {builder:?}")]
    ObservationEpochIdentityMismatch {
        builder: FabricEpochId,
        write: FabricEpochId,
    },
    #[error("program output schema drifted from {expected:?} to {actual:?}")]
    OutputSchemaDrift {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("cached logical-plan schema differs from its admitted output contract")]
    CachedPlanSchemaDrift,
    #[error(transparent)]
    LogicalPlanCache(#[from] LogicalPlanCacheError),
    #[error("logical-plan semantic authority could not be derived: {0}")]
    LogicalPlanAuthority(String),
    #[error("cached logical plan escaped its sealed epoch authority: {0}")]
    CachedPlanAuthorityDrift(String),
    #[error(transparent)]
    ProgrammaticSchema(#[from] ProgrammaticSchemaError),
    #[error(transparent)]
    ObservationHistoricization(#[from] ProgrammaticObservationHistoricizationFailure),
    #[error(transparent)]
    ObservationReopen(#[from] ProgrammaticObservationDeltaError),
    #[error(transparent)]
    ObservationProvision(#[from] ProgrammaticObservationProvisionError),
    #[error(transparent)]
    RelationalProgram(#[from] RelationalProgramError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::Int64Array;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::datasource::MemTable;
    use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
    use datafusion::prelude::{col, lit};

    use super::*;
    use crate::fabric::programmatic_schema::{
        ProgrammaticFieldId, ProgrammaticTransformationContract, ProgrammaticTransformationId,
        RELATION_OBSERVATION_RELATION_ID, TransformationDeterminismPolicy,
        TransformationFieldIdentity, TransformationInputs, TransformationOrderingPolicy,
        TransformationOutput, TransformationPlanError, TransformationProvenance,
        TransformationProvenanceIdentity, TransformationRecursionPolicy,
        TransformationReleaseIdentity, TransformationResourceClass, TransformationSemanticVersion,
    };
    use crate::relational_program::{CompilationDependency, FieldId, RelationalExpression};
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
    };

    struct PositiveValues {
        contract: ProgrammaticTransformationContract,
        output: TransformationOutput,
        dependencies: Arc<[ProgrammaticRelationId]>,
    }

    impl ProgrammaticTransformation for PositiveValues {
        fn contract(&self) -> &ProgrammaticTransformationContract {
            &self.contract
        }

        fn output(&self) -> &TransformationOutput {
            &self.output
        }

        fn dependencies(&self) -> &[ProgrammaticRelationId] {
            &self.dependencies
        }

        fn build(
            &self,
            inputs: &TransformationInputs,
        ) -> Result<LogicalPlan, TransformationPlanError> {
            Ok(
                LogicalPlanBuilder::from(inputs.plan(&self.dependencies[0])?)
                    .filter(col("value").gt(lit(0_i64)))?
                    .project(vec![col("value")])?
                    .build()?,
            )
        }
    }

    fn provider_input() -> ProviderInput {
        let relation_id = "facts.input_values";
        let field = Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([(
            FIELD_ID_METADATA_KEY.to_owned(),
            "facts.input_values.value".to_owned(),
        )]));
        let schema = Arc::new(Schema::new(vec![field]).with_metadata(HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            relation_id.to_owned(),
        )])));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![-1_i64, 1, 2]))],
        )
        .unwrap();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).unwrap());
        let reference = datafusion::common::TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::Fact.as_str(),
            "input_values",
        );
        let contract = Arc::new(
            SchemaContract::try_new(
                "provider:test-values",
                reference.clone(),
                Arc::clone(&schema),
                schema,
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );
        ProviderInput::new(
            ProgrammaticRelationId::new(relation_id),
            reference,
            contract,
            provider,
        )
    }

    #[tokio::test]
    async fn provider_plan_schema_observations_and_query_share_one_sealed_session() {
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([9; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap();
        builder.register_provider(provider_input()).unwrap();
        let input = ProgrammaticRelationId::new("facts.input_values");
        let output = ProgrammaticRelationId::new("facts.positive_values");
        builder
            .add_transformation(Arc::new(PositiveValues {
                contract: ProgrammaticTransformationContract::new(
                    ProgrammaticTransformationId::new("transform.positive_values"),
                    TransformationSemanticVersion::new(1, 0, 0),
                    TransformationResourceClass::BoundedInMemory {
                        max_rows: 1_000,
                        max_memory_bytes: 1 << 20,
                    },
                    TransformationDeterminismPolicy::DeterministicSet,
                    TransformationOrderingPolicy::Unordered,
                    TransformationRecursionPolicy::Forbidden,
                    TransformationProvenance::new(
                        TransformationProvenanceIdentity::from_bytes([0x51; 32]),
                        TransformationReleaseIdentity::from_bytes([0x61; 32]),
                    ),
                ),
                output: TransformationOutput::new(
                    output.clone(),
                    datafusion::common::TableReference::full(
                        FABRIC_CATALOG,
                        FabricSchemaRole::Derived.as_str(),
                        "positive_values",
                    ),
                    vec![TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        "facts.positive_values.value",
                    ))],
                ),
                dependencies: Arc::from([input]),
            }))
            .unwrap();

        let epoch = builder.seal_for_test().await.unwrap();
        let first_context = epoch.context();
        let second_context = epoch.context();
        assert_eq!(first_context.session_id(), second_context.session_id());
        assert!(Arc::ptr_eq(
            first_context.state().runtime_env(),
            second_context.state().runtime_env()
        ));
        assert!(Arc::ptr_eq(
            first_context.state().catalog_list(),
            second_context.state().catalog_list()
        ));
        assert!(epoch.relation(&output).is_some());
        let observation = epoch
            .relation(&ProgrammaticRelationId::new(
                RELATION_OBSERVATION_RELATION_ID,
            ))
            .expect("sealed relation-observation binding");
        let observation_rows = epoch
            .context()
            .table(observation.table_reference.clone())
            .await
            .expect("resolve relation-observation provider")
            .collect()
            .await
            .expect("collect relation-observation rows")
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>();
        assert!(observation_rows > 0);

        let program = RelationalProgram {
            root: RelationalExpression::Input(RelationId::new("facts.positive_values").unwrap()),
            output_fields: vec![FieldId::new("facts.positive_values.value").unwrap()],
        };
        let result = epoch.execute_relational_program(&program).await.unwrap();
        assert_eq!(result.row_count(), 2);
        assert_eq!(
            result.plan_observation().outcome,
            LogicalPlanCacheOutcome::Miss
        );
        assert!(result.observations().dependencies.contains(
            &CompilationDependency::SessionAuthority(epoch.schema_authority_id().to_owned())
        ));
        let repeated = epoch.execute_relational_program(&program).await.unwrap();
        assert_eq!(repeated.row_count(), 2);
        assert_eq!(
            repeated.plan_observation().outcome,
            LogicalPlanCacheOutcome::Hit
        );
        assert_eq!(
            repeated.plan_observation().compiled_plan_digest,
            result.plan_observation().compiled_plan_digest
        );
        assert_eq!(
            repeated.plan_observation().optimized_plan_digest,
            result.plan_observation().optimized_plan_digest
        );
        let cache = epoch.logical_plan_cache_observation();
        assert_eq!(cache.capacity_entries, 256);
        assert!(cache.accounting_capacity_bytes > 0);
        assert_eq!(cache.resident_entries, 1);
        assert!(cache.accounted_bytes > 0);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.evictions, 0);
        assert_eq!(cache.oversized_bypasses, 0);
        assert_eq!(cache.collisions, 0);
        assert!(
            !epoch
                .context()
                .catalog(FABRIC_CATALOG)
                .unwrap()
                .schema_names()
                .iter()
                .any(|name| name == "model")
        );
    }
}
