//! Sealed fabric epochs assembled directly from provider contracts and native
//! DataFusion transformations.
//!
//! This is the target epoch path. It deliberately has no predecessor replay,
//! bootstrap catalog, SQL definition, or serialized-plan input.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
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
use datafusion::execution::object_store::ObjectStoreRegistry;
use datafusion::execution::runtime_env::RuntimeEnv;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::collect;
use datafusion::prelude::SessionContext;
use deltalake::delta_datafusion::planner::DeltaPlanner;
use object_store::ObjectStore;

use crate::relational_program::{
    CompilationObservations, ProgramBindings, ProgramRelationContract, RelationId, RelationInput,
    RelationalProgram, RelationalProgramCompiler, RelationalProgramError,
};
use crate::resource_budget::{
    ResourceAmounts, ResourceBudgetError, ResourceClass, ResourceReservation,
};

use super::activation::{TableVersionSet, TableVersionSetRef};
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
#[cfg(test)]
use super::programmatic_relation_delta::ProgrammaticRelationDeltaLayout;
use super::programmatic_relation_delta::{
    ProgrammaticRelationDeltaError, ProgrammaticRelationDeltaPreparation,
    ProgrammaticRelationDeltaPublication, persist_programmatic_relation_snapshots,
    reopen_programmatic_relation_snapshots,
};
use super::programmatic_schema::{
    ProgrammaticRelationId, ProgrammaticSchemaAssembly, ProgrammaticSchemaError,
    ProgrammaticTransformation, ProviderInput, SealedRelationBinding,
};
use super::resource_ownership::WorkspaceFabricResources;

/// The cache and its generation/capacity retain one lifetime across epoch and child handles.
/// No bare Arc to the cache is exported, so an escaped cache cannot silently lose its charge.
#[derive(Clone, Debug)]
pub(super) struct EpochLogicalPlanCacheHandle {
    cache: Arc<EpochLogicalPlanCache>,
    retention: Option<Arc<EpochResourceRetention>>,
}

impl Deref for EpochLogicalPlanCacheHandle {
    type Target = EpochLogicalPlanCache;
    fn deref(&self) -> &Self::Target {
        &self.cache
    }
}

#[derive(Debug)]
struct EpochResourceRetention {
    resources: WorkspaceFabricResources,
    // All cache consumers retain this owner; cache backing drops before this guard.
    _reservation: ResourceReservation,
}

/// Opaque linear assembly carrier: consuming admission cannot discard only the generation guard
/// while retaining its runtime. Its fields are not caller-reconstructible authority.
pub(crate) struct ProgrammaticEpochRuntimeOwner {
    runtime: Arc<RuntimeEnv>,
    logical_plan_cache: EpochLogicalPlanCacheHandle,
}

impl ProgrammaticEpochRuntimeOwner {
    fn is_governed(&self) -> bool {
        self.logical_plan_cache.retention.is_some()
    }
}

/// Fixed local-origin capability for the current explicitly local Delta deployment. It has no
/// lazy URL factories and no registration path that can add another origin or replace this store.
#[derive(Debug)]
struct CandidateLocalObjectStoreRegistry {
    store: Arc<dyn ObjectStore>,
}

impl CandidateLocalObjectStoreRegistry {
    fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }
}

/// Fresh registry with only the explicitly selected local filesystem origin; no store discovery.
pub(crate) fn local_fabric_object_store_registry(
    store: Arc<dyn ObjectStore>,
) -> Arc<dyn ObjectStoreRegistry> {
    Arc::new(CandidateLocalObjectStoreRegistry::new(store))
}

impl ObjectStoreRegistry for CandidateLocalObjectStoreRegistry {
    fn register_store(
        &self,
        _url: &url::Url,
        store: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore>> {
        Some(store)
    }
    fn get_store(&self, url: &url::Url) -> datafusion::common::Result<Arc<dyn ObjectStore>> {
        if url.scheme() == "file"
            && url.host_str().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
        {
            Ok(Arc::clone(&self.store))
        } else {
            Err(datafusion::common::DataFusionError::Plan(
                "candidate store origin is not the explicitly selected local filesystem".into(),
            ))
        }
    }
}

/// Mutable owner of one programmatic candidate session.
pub struct ProgrammaticFabricEpochBuilder {
    identity: FabricEpochId,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_owner: ProgrammaticEpochRuntimeOwner,
    assembly: ProgrammaticSchemaAssembly,
}

impl ProgrammaticFabricEpochBuilder {
    /// Test-only bounded isolated fixture. Production always supplies its one workspace owner.
    #[cfg(test)]
    pub(crate) fn try_new(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
    ) -> Result<Self, ProgrammaticFabricEpochError> {
        let runtime_env = runtime_config.runtime_env()?;
        let logical_plan_cache = EpochLogicalPlanCacheHandle {
            cache: Arc::new(EpochLogicalPlanCache::new(
                runtime_config.cache_policy().logical_plan_entries(),
                runtime_config.cache_policy().logical_plan_bytes(),
            )),
            retention: None,
        };
        Self::assemble(
            identity,
            runtime_config,
            ProgrammaticEpochRuntimeOwner {
                runtime: runtime_env,
                logical_plan_cache,
            },
        )
    }

    /// Admit one generation against the shared workspace before creating its session/catalog/cache.
    /// The logical cache reserves its finite backing capacity; entries remain epoch-authorized.
    pub(crate) fn try_new_governed(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        resources: WorkspaceFabricResources,
    ) -> Result<Self, ProgrammaticFabricEpochError> {
        if runtime_config.native_config() != *resources.config() {
            return Err(ProgrammaticFabricEpochError::ResourcePolicyDrift);
        }
        let cache_bytes = u64::try_from(runtime_config.cache_policy().logical_plan_bytes())
            .map_err(|_| ResourceBudgetError::Overflow)?;
        let reservation = resources.workspace_budget().try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                retained_generations: 1,
                memory_bytes: cache_bytes,
                retained_bytes: cache_bytes,
                ..ResourceAmounts::default()
            },
        )?;
        let runtime = resources.runtime_env();
        let logical_plan_cache = EpochLogicalPlanCacheHandle {
            cache: Arc::new(EpochLogicalPlanCache::new(
                runtime_config.cache_policy().logical_plan_entries(),
                runtime_config.cache_policy().logical_plan_bytes(),
            )),
            retention: Some(Arc::new(EpochResourceRetention {
                resources,
                _reservation: reservation,
            })),
        };
        Self::assemble(
            identity,
            runtime_config,
            ProgrammaticEpochRuntimeOwner {
                runtime,
                logical_plan_cache,
            },
        )
    }

    fn assemble(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        runtime_owner: ProgrammaticEpochRuntimeOwner,
    ) -> Result<Self, ProgrammaticFabricEpochError> {
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
        let mut session_config = runtime_config.session_config();
        if let Some(retention) = &runtime_owner.logical_plan_cache.retention {
            // SessionState/TaskContext clones and consuming assembly retain this guard even when
            // an assembly carrier is dropped. It is an owned native configuration extension.
            session_config = session_config.with_extension(Arc::clone(retention));
        }
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_config(session_config)
            .with_runtime_env(Arc::clone(&runtime_owner.runtime))
            .with_catalog_list(catalog_list)
            .with_query_planner(DeltaPlanner::new())
            .build();
        Ok(Self {
            identity,
            runtime_config,
            runtime_owner,
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
        ProgrammaticEpochRuntimeOwner,
        ProgrammaticSchemaAssembly,
    ) {
        (
            self.identity,
            self.runtime_config,
            self.runtime_owner,
            self.assembly,
        )
    }

    /// Reconstitute ownership after a consuming admission operation without
    /// rebuilding or replaying the candidate session.
    #[must_use]
    pub(crate) fn from_assembly_parts(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        runtime_owner: ProgrammaticEpochRuntimeOwner,
        assembly: ProgrammaticSchemaAssembly,
    ) -> Self {
        Self {
            identity,
            runtime_config,
            runtime_owner,
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
    /// versions in this same session, and validate schema and dependency consistency.
    #[allow(clippy::result_large_err)] // Preserve the existing typed publication errors in this forwarding method.
    pub(crate) async fn seal(
        self,
        write_identity: ProgrammaticObservationWriteIdentity,
        targets: ProgrammaticObservationDeltaTargets,
        relation_preparation: ProgrammaticRelationDeltaPreparation,
    ) -> Result<ProgrammaticFabricEpoch, ProgrammaticFabricEpochError> {
        self.seal_cancellable(
            write_identity,
            targets,
            relation_preparation,
            &crate::cancellation::Cancellation::default(),
        )
        .await
    }

    pub(crate) async fn seal_cancellable(
        self,
        write_identity: ProgrammaticObservationWriteIdentity,
        targets: ProgrammaticObservationDeltaTargets,
        relation_preparation: ProgrammaticRelationDeltaPreparation,
        cancellation: &crate::cancellation::Cancellation,
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
            runtime_owner,
            assembly,
        } = self;
        let historicized =
            historicize_programmatic_observations(assembly, write_identity, targets).await?;
        let relation_publication = persist_programmatic_relation_snapshots(
            historicized.sealed(),
            identity,
            write_identity.operation_id(),
            write_identity.writer_generation(),
            write_identity.observation_set_id(),
            relation_preparation,
            cancellation,
        )
        .await?;
        let table_versions =
            combine_table_versions(historicized.publication(), &relation_publication)?;
        Self::finish_historicized(
            identity,
            runtime_config,
            runtime_owner,
            historicized,
            relation_publication,
            table_versions,
        )
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
            runtime_owner,
            mut assembly,
        } = self;
        let (observation_versions, relation_versions) = split_table_versions(&table_versions)?;
        let (relation_publication, providers) = reopen_programmatic_relation_snapshots(
            Arc::new(assembly.candidate_state()),
            identity,
            relation_versions,
        )
        .await?;
        for provider in providers {
            assembly.register_provider(provider)?;
        }
        let historicized =
            reopen_programmatic_observations(assembly, identity, observation_versions).await?;
        Self::finish_historicized(
            identity,
            runtime_config,
            runtime_owner,
            historicized,
            relation_publication,
            table_versions,
        )
    }

    fn finish_historicized(
        identity: FabricEpochId,
        runtime_config: FabricEpochRuntimeConfig,
        runtime_owner: ProgrammaticEpochRuntimeOwner,
        historicized: ProgrammaticObservationHistoricization,
        relation_publication: ProgrammaticRelationDeltaPublication,
        table_versions: Arc<TableVersionSet>,
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
        let state = session.state();
        if !Arc::ptr_eq(state.runtime_env(), &runtime_owner.runtime) {
            return Err(ProgrammaticFabricEpochError::RuntimeAuthorityDrift);
        }
        let logical_plan_authority = derive_epoch_logical_plan_authority(
            identity,
            &runtime_config,
            &state,
            &relations,
            &table_versions,
            &program_bindings,
        )?;
        Ok(ProgrammaticFabricEpoch {
            identity,
            runtime_config,
            runtime_owner,
            session,
            relations,
            observation_publication,
            relation_publication,
            table_versions,
            program_bindings,
            logical_plan_authority,
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
        let relation_root = temporary.path().join("relation-snapshots");
        fs::create_dir_all(&relation_root).map_err(|source| {
            ProgrammaticFabricEpochError::CatalogClosure(format!(
                "cannot create test relation-snapshot root: {source}"
            ))
        })?;
        let relation_layout = ProgrammaticRelationDeltaLayout::try_new(
            url::Url::from_directory_path(relation_root).map_err(|()| {
                ProgrammaticFabricEpochError::CatalogClosure(
                    "test relation-snapshot root is not a file URL".to_owned(),
                )
            })?,
        )?;
        let mut epoch = self
            .seal(
                identity,
                targets,
                ProgrammaticRelationDeltaPreparation::Genesis(relation_layout),
            )
            .await?;
        epoch.observation_history_root = Some(temporary);
        Ok(epoch)
    }
}

fn combine_table_versions(
    observations: &ProgrammaticObservationDeltaPublication,
    relations: &ProgrammaticRelationDeltaPublication,
) -> Result<Arc<TableVersionSet>, ProgrammaticFabricEpochError> {
    let mut components = observations
        .table_versions()
        .map(|(relation_id, pin)| (Arc::<str>::from(relation_id), pin.clone()))
        .collect::<Vec<_>>();
    components.extend(
        relations
            .table_versions()
            .map(|(relation_id, pin)| (Arc::<str>::from(relation_id), pin.clone())),
    );
    Ok(Arc::new(TableVersionSet::try_new(components)?))
}

fn split_table_versions(
    selected: &TableVersionSet,
) -> Result<
    (
        Arc<TableVersionSet>,
        BTreeMap<ProgrammaticRelationId, super::delta_exact::ExactDeltaPin>,
    ),
    ProgrammaticFabricEpochError,
> {
    let mut observations = Vec::new();
    let mut relations = BTreeMap::new();
    for (relation_id, pin) in selected.components() {
        let relation_id = ProgrammaticRelationId::new(relation_id);
        if super::programmatic_observation_delta::is_programmatic_observation_relation(&relation_id)
        {
            observations.push((Arc::<str>::from(relation_id.as_str()), pin.clone()));
        } else {
            relations.insert(relation_id, pin.clone());
        }
    }
    Ok((Arc::new(TableVersionSet::try_new(observations)?), relations))
}

fn derive_epoch_logical_plan_authority(
    identity: FabricEpochId,
    runtime_config: &FabricEpochRuntimeConfig,
    state: &datafusion::execution::SessionState,
    relations: &BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    table_versions: &TableVersionSet,
    program_bindings: &ProgramBindings,
) -> Result<LogicalPlanAuthorityFingerprint, ProgrammaticFabricEpochError> {
    let mut authority = LogicalPlanAuthorityBuilder::new(b"programmatic-fabric-epoch-authority.v1");
    authority.frame(identity.as_bytes());
    authority.frame_str(&runtime_config.identity());
    authority.frame(table_versions.reference().as_bytes());
    authority.frame_usize(table_versions.len());
    for (relation_id, pin) in table_versions.components() {
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

fn native_cache_backing_shared(left: &RuntimeEnv, right: &RuntimeEnv) -> bool {
    fn same_optional<T: ?Sized>(left: Option<Arc<T>>, right: Option<Arc<T>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => Arc::ptr_eq(&left, &right),
            (None, None) => true,
            _ => false,
        }
    }
    Arc::ptr_eq(
        &left.cache_manager.get_file_metadata_cache(),
        &right.cache_manager.get_file_metadata_cache(),
    ) && same_optional(
        left.cache_manager.get_file_statistic_cache(),
        right.cache_manager.get_file_statistic_cache(),
    ) && same_optional(
        left.cache_manager.get_list_files_cache(),
        right.cache_manager.get_list_files_cache(),
    )
}

/// Sealed session authority for one exact set of provider facts and
/// programmatic transformations.
pub struct ProgrammaticFabricEpoch {
    identity: FabricEpochId,
    runtime_config: FabricEpochRuntimeConfig,
    runtime_owner: ProgrammaticEpochRuntimeOwner,
    session: SessionContext,
    relations: BTreeMap<ProgrammaticRelationId, SealedRelationBinding>,
    observation_publication: ProgrammaticObservationDeltaPublication,
    relation_publication: ProgrammaticRelationDeltaPublication,
    table_versions: Arc<TableVersionSet>,
    program_bindings: Arc<ProgramBindings>,
    logical_plan_authority: LogicalPlanAuthorityFingerprint,
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
    pub const fn relation_publication(&self) -> &ProgrammaticRelationDeltaPublication {
        &self.relation_publication
    }

    /// Complete reversible exact-version authority selected for this epoch.
    #[must_use]
    pub const fn table_version_set(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
    }

    /// Canonical reference derived from the complete reversible table-version set.
    #[must_use]
    pub fn table_version_set_ref(&self) -> TableVersionSetRef {
        self.table_versions.reference()
    }

    #[must_use]
    pub const fn program_bindings(&self) -> &Arc<ProgramBindings> {
        &self.program_bindings
    }

    #[must_use]
    pub fn logical_plan_cache_observation(&self) -> LogicalPlanCacheObservation {
        self.runtime_owner.logical_plan_cache.observation()
    }

    pub(super) const fn logical_plan_cache(&self) -> &EpochLogicalPlanCacheHandle {
        &self.runtime_owner.logical_plan_cache
    }

    /// The actual injected process/workspace owner, absent only for isolated test fixtures.
    #[must_use]
    pub fn workspace_resources(&self) -> Option<&WorkspaceFabricResources> {
        self.runtime_owner
            .logical_plan_cache
            .retention
            .as_ref()
            .map(|retention| &retention.resources)
    }

    pub(super) const fn logical_plan_authority(&self) -> LogicalPlanAuthorityFingerprint {
        self.logical_plan_authority
    }

    #[must_use]
    pub fn relation(&self, relation_id: &ProgrammaticRelationId) -> Option<&SealedRelationBinding> {
        self.relations.get(relation_id)
    }

    /// Enumerate every stable relation identity sealed into this exact session.
    #[must_use]
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
        let parent = &self.runtime_owner.runtime;
        let authorities_distinct = !Arc::ptr_eq(parent, runtime)
            && !Arc::ptr_eq(self.session.state().catalog_list(), catalog_list)
            && !Arc::ptr_eq(
                &parent.object_store_registry,
                &runtime.object_store_registry,
            );
        if !authorities_distinct {
            return false;
        }
        if self.runtime_owner.is_governed() {
            Arc::ptr_eq(&parent.memory_pool, &runtime.memory_pool)
                && Arc::ptr_eq(&parent.disk_manager, &runtime.disk_manager)
                && native_cache_backing_shared(parent, runtime)
        } else {
            // The isolated constructor is cfg(test), never a production budget alternative.
            !Arc::ptr_eq(&parent.memory_pool, &runtime.memory_pool)
                && !Arc::ptr_eq(&parent.disk_manager, &runtime.disk_manager)
                && !Arc::ptr_eq(&parent.cache_manager, &runtime.cache_manager)
        }
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
            self.table_version_set_ref(),
            self.program_bindings.authority_id(),
            self.runtime_config.identity(),
            self.logical_plan_authority,
            LogicalPlanCacheScope::Epoch,
            program,
        );
        let (cached, cache_outcome) = if let Some(cached) =
            self.logical_plan_cache().get(&cache_key)
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
            let cached = self.logical_plan_cache().try_insert(
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
        self.runtime_owner.runtime.memory_pool.reserved()
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
    #[error("epoch native resource policy differs from its injected workspace owner")]
    ResourcePolicyDrift,
    #[error(transparent)]
    ResourceBudget(#[from] ResourceBudgetError),
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
    RelationDelta(#[from] ProgrammaticRelationDeltaError),
    #[error(transparent)]
    TableVersions(#[from] super::activation::TableVersionSetError),
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
    use std::fs;

    use arrow_array::Int64Array;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::datasource::MemTable;
    use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
    use datafusion::prelude::{col, lit};
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::fabric::programmatic_schema::{
        DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID, ProgrammaticFieldId,
        ProgrammaticTransformationContract, ProgrammaticTransformationId,
        RELATION_OBSERVATION_RELATION_ID, SCHEMA_OBSERVATION_RELATION_ID,
        TransformationDeterminismPolicy, TransformationFieldIdentity, TransformationInputs,
        TransformationOrderingPolicy, TransformationOutput, TransformationPlanError,
        TransformationProvenance, TransformationProvenanceIdentity, TransformationRecursionPolicy,
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

    fn provider_input_with_values(values: Vec<i64>) -> ProviderInput {
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
            vec![Arc::new(Int64Array::from(values))],
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

    fn provider_input() -> ProviderInput {
        provider_input_with_values(vec![-1_i64, 1, 2])
    }

    fn governed_resources(
        generations: u64,
    ) -> (
        crate::resource_budget::ResourceBudget,
        WorkspaceFabricResources,
        FabricEpochRuntimeConfig,
    ) {
        use crate::resource_budget::{ResourceBudget, ResourceBudgetPolicy};
        let policy = ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: 2 << 30,
                disk_bytes: 4 << 30,
                running_jobs: 16,
                queued_jobs: 32,
                retained_generations: generations,
                retained_bytes: 2 << 30,
                rows: 1_000_000,
                pages: 10_000,
            },
            control_reserve: ResourceAmounts::default(),
        };
        let root = ResourceBudget::try_process([0x91; 16], policy).unwrap();
        let workspace = root.workspace([0x92; 16], policy).unwrap();
        let config = FabricEpochRuntimeConfig::default();
        let resources = WorkspaceFabricResources::try_new(
            workspace,
            config.native_config(),
            local_fabric_object_store_registry(Arc::new(
                object_store::local::LocalFileSystem::new(),
            )),
        )
        .unwrap();
        (root, resources, config)
    }

    /// Assembly conversion and escaped sessions retain one generation charge; neither a second
    /// full-budget epoch nor a mismatched native policy can bypass the workspace owner.
    #[test]
    fn rt_cpg_wp79_integrity() {
        let (root, resources, config) = governed_resources(1);
        let native_cache = root.observation().used.memory_bytes;
        let builder = ProgrammaticFabricEpochBuilder::try_new_governed(
            FabricEpochId::from_bytes([0x93; 16]),
            config.clone(),
            resources.clone(),
        )
        .unwrap();
        assert_eq!(root.observation().used.retained_generations, 1);
        assert_eq!(
            root.observation().used.memory_bytes,
            native_cache + config.cache_policy().logical_plan_bytes() as u128
        );
        assert!(matches!(
            ProgrammaticFabricEpochBuilder::try_new_governed(
                FabricEpochId::from_bytes([0x94; 16]),
                config.clone(),
                resources.clone()
            ),
            Err(ProgrammaticFabricEpochError::ResourceBudget(_))
        ));
        let (id, config, owner, assembly) = builder.into_assembly_parts();
        let rebuilt =
            ProgrammaticFabricEpochBuilder::from_assembly_parts(id, config, owner, assembly);
        assert_eq!(root.observation().used.retained_generations, 1);
        let (_, _, owner, assembly) = rebuilt.into_assembly_parts();
        drop(owner);
        let escaped_context = assembly.candidate_context();
        drop(assembly);
        assert_eq!(root.observation().used.retained_generations, 1);
        drop(escaped_context);
        assert_eq!(root.observation().used.retained_generations, 0);
        assert_eq!(root.observation().used.memory_bytes, native_cache);
        let changed_config =
            FabricEpochRuntimeConfig::try_new(1 << 20, 1 << 20, 2, 1, 128, 1, true).unwrap();
        assert!(matches!(
            ProgrammaticFabricEpochBuilder::try_new_governed(
                FabricEpochId::from_bytes([0x95; 16]),
                changed_config,
                resources.clone()
            ),
            Err(ProgrammaticFabricEpochError::ResourcePolicyDrift)
        ));
        drop(resources);
        assert_eq!(
            root.observation().used,
            crate::resource_budget::ResourceUsage::default()
        );
    }

    #[tokio::test]
    async fn rt_cpg_wp79_epoch_seal_reopen_and_escaped_cache_retain_one_charge() {
        let (root, resources, config) = governed_resources(2);
        let id = FabricEpochId::from_bytes([0x96; 16]);
        let mut builder =
            ProgrammaticFabricEpochBuilder::try_new_governed(id, config.clone(), resources.clone())
                .unwrap();
        builder.register_provider(provider_input()).unwrap();
        let epoch = builder.seal_for_test().await.unwrap();
        assert_eq!(root.observation().used.retained_generations, 1);
        assert!(
            epoch
                .workspace_resources()
                .unwrap()
                .workspace_budget()
                .same_scope(resources.workspace_budget())
        );
        let cache = epoch.logical_plan_cache().clone();
        let reopened =
            ProgrammaticFabricEpochBuilder::try_new_governed(id, config.clone(), resources.clone())
                .unwrap()
                .reopen(Arc::clone(epoch.table_version_set()))
                .await
                .unwrap();
        assert_eq!(root.observation().used.retained_generations, 2);
        assert!(Arc::ptr_eq(
            &epoch.runtime_owner.runtime.memory_pool,
            &reopened.runtime_owner.runtime.memory_pool
        ));
        assert!(Arc::ptr_eq(
            &epoch.runtime_owner.runtime.disk_manager,
            &reopened.runtime_owner.runtime.disk_manager
        ));
        assert!(native_cache_backing_shared(
            &epoch.runtime_owner.runtime,
            &reopened.runtime_owner.runtime
        ));
        let rows = reopened
            .context()
            .table(TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Fact.as_str(),
                "input_values",
            ))
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(rows.iter().map(RecordBatch::num_rows).sum::<usize>(), 3);
        drop(rows);
        assert!(
            ProgrammaticFabricEpochBuilder::try_new_governed(
                FabricEpochId::from_bytes([0x97; 16]),
                config,
                resources.clone()
            )
            .is_err()
        );
        let child_runtime = resources
            .runtime_with_registry(Arc::new(CandidateLocalObjectStoreRegistry::new(Arc::new(
                object_store::local::LocalFileSystem::new(),
            ))))
            .unwrap();
        let child_catalog: Arc<dyn datafusion::catalog::CatalogProviderList> =
            Arc::new(MemoryCatalogProviderList::new());
        assert!(epoch.child_authorities_are_distinct(&child_runtime, &child_catalog));
        assert!(
            !epoch.child_authorities_are_distinct(&epoch.runtime_owner.runtime, &child_catalog)
        );
        assert!(
            !epoch.child_authorities_are_distinct(
                &child_runtime,
                epoch.session.state().catalog_list()
            )
        );
        drop(child_runtime);
        drop(epoch);
        assert_eq!(root.observation().used.retained_generations, 2); // escaped cache owns its epoch backing
        drop(cache);
        assert_eq!(root.observation().used.retained_generations, 1);
        let escaped_session = reopened.context();
        drop(reopened);
        assert_eq!(root.observation().used.retained_generations, 1);
        drop(escaped_session);
        assert_eq!(root.observation().used.retained_generations, 0);
        drop(resources);
        assert_eq!(
            root.observation().used,
            crate::resource_budget::ResourceUsage::default()
        );
    }

    #[test]
    fn rt_cpg_wp79_epoch_store_registry_has_only_selected_local_origin() {
        let registry = CandidateLocalObjectStoreRegistry::new(Arc::new(
            object_store::local::LocalFileSystem::new(),
        ));
        let local = url::Url::parse("file:///exact/delta/table").unwrap();
        let selected = registry.get_store(&local).unwrap();
        let foreign = url::Url::parse("memory://unselected/table").unwrap();
        registry.register_store(&foreign, Arc::clone(&selected));
        assert!(registry.get_store(&foreign).is_err());
        assert!(
            registry
                .get_store(&url::Url::parse("file://unselected-host/table").unwrap())
                .is_err()
        );
        assert!(
            registry
                .get_store(&url::Url::parse("file:///exact/table?alternate=1").unwrap())
                .is_err()
        );
        assert!(Arc::ptr_eq(&selected, &registry.get_store(&local).unwrap()));
    }

    fn positive_builder(
        epoch_id: FabricEpochId,
        values: Vec<i64>,
    ) -> ProgrammaticFabricEpochBuilder {
        positive_builder_with_input(epoch_id, provider_input_with_values(values))
    }

    fn positive_builder_with_input(
        epoch_id: FabricEpochId,
        input: ProviderInput,
    ) -> ProgrammaticFabricEpochBuilder {
        let mut builder =
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
                .expect("programmatic epoch builder");
        builder
            .register_provider(input)
            .expect("register exact provider");
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
                    output,
                    TableReference::full(
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
            .expect("register positive-values transformation");
        builder
    }

    fn observation_roots(temporary: &TempDir) -> BTreeMap<ProgrammaticRelationId, Url> {
        [
            RELATION_OBSERVATION_RELATION_ID,
            FIELD_OBSERVATION_RELATION_ID,
            SCHEMA_OBSERVATION_RELATION_ID,
            DEPENDENCY_OBSERVATION_RELATION_ID,
            PROVENANCE_OBSERVATION_RELATION_ID,
        ]
        .into_iter()
        .map(|relation_id| {
            let path = temporary.path().join(relation_id.replace('.', "_"));
            fs::create_dir_all(&path).expect("create exact observation root");
            (
                ProgrammaticRelationId::new(relation_id),
                Url::from_directory_path(path).expect("observation root file URL"),
            )
        })
        .collect()
    }

    fn relation_layout(temporary: &TempDir) -> ProgrammaticRelationDeltaLayout {
        ProgrammaticRelationDeltaLayout::try_new(
            Url::from_directory_path(temporary.path().join("relation-snapshots"))
                .expect("relation snapshot file URL"),
        )
        .expect("exact relation layout")
    }

    fn write_identity(seed: u8) -> ProgrammaticObservationWriteIdentity {
        ProgrammaticObservationWriteIdentity::new(
            FabricEpochId::from_bytes([seed; 16]),
            super::super::command::OperationId::from_bytes([seed.wrapping_add(0x20); 16]),
            super::super::command::WriterGeneration::new(u64::from(seed))
                .expect("nonzero writer generation"),
            super::super::command::TransactionRef::from_bytes([seed.wrapping_add(0x40); 32]),
        )
    }

    async fn positive_rows(epoch: &ProgrammaticFabricEpoch) -> Vec<i64> {
        let batches = epoch
            .context()
            .table(TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Derived.as_str(),
                "positive_values",
            ))
            .await
            .expect("resolve positive-values relation")
            .collect()
            .await
            .expect("execute positive-values relation");
        let mut values = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .expect("positive values are int64")
                    .iter()
                    .flatten()
            })
            .collect::<Vec<_>>();
        values.sort_unstable();
        values
    }

    async fn prepare_reuse_candidate(
        seed: u8,
        values: Vec<i64>,
        identity: Option<[u8; 32]>,
        selected: Option<&ProgrammaticFabricEpoch>,
    ) -> (TempDir, ProgrammaticFabricEpoch) {
        let mut input = provider_input_with_values(values);
        if let Some(identity) = identity {
            input = input.with_immutable_input_identity(identity);
        }
        prepare_reuse_input(seed, input, selected).await
    }

    async fn prepare_reuse_input(
        seed: u8,
        input: ProviderInput,
        selected: Option<&ProgrammaticFabricEpoch>,
    ) -> (TempDir, ProgrammaticFabricEpoch) {
        let root = TempDir::new().unwrap();
        let write = write_identity(seed);
        let builder = positive_builder_with_input(write.epoch_id(), input);
        let observations = builder
            .provision_observation_histories(observation_roots(&root))
            .await
            .unwrap();
        let layout = relation_layout(&root);
        let preparation = selected.map_or_else(
            || ProgrammaticRelationDeltaPreparation::Genesis(layout.clone()),
            |selected| ProgrammaticRelationDeltaPreparation::ReuseUnchanged {
                selected: selected.relation_publication().clone(),
                layout: layout.clone(),
            },
        );
        let epoch = builder
            .seal(write, observations, preparation)
            .await
            .unwrap();
        (root, epoch)
    }

    fn materialized_values(values: Vec<i64>) -> ProviderInput {
        let input = provider_input_with_values(vec![]);
        let schema = Arc::clone(input.contract.storage_schema());
        let batch = RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(values))]).unwrap();
        ProviderInput::try_from_arrow(
            input.relation_id,
            input.table_reference,
            input.contract,
            vec![vec![batch]],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn materialized_empty_inputs_reuse_only_exact_empty_versions_and_preserve_reopen() {
        let id = ProgrammaticRelationId::new("facts.input_values");
        let (_first_root, first) = prepare_reuse_input(61, materialized_values(vec![]), None).await;
        let (second_root, second) =
            prepare_reuse_input(62, materialized_values(vec![]), Some(&first)).await;
        let first_pin = &first.relation_publication().table_version_map()[&id];
        assert_eq!(
            first_pin,
            &second.relation_publication().table_version_map()[&id]
        );
        assert!(positive_rows(&second).await.is_empty());
        assert!(
            !relation_layout(&second_root)
                .root()
                .to_file_path()
                .unwrap()
                .join(
                    first_pin
                        .canonical_root()
                        .to_file_path()
                        .unwrap()
                        .file_name()
                        .unwrap()
                )
                .exists()
        );

        let (_populated_root, populated) =
            prepare_reuse_input(63, materialized_values(vec![3, 3]), Some(&second)).await;
        assert_ne!(
            first_pin,
            &populated.relation_publication().table_version_map()[&id]
        );
        assert_eq!(positive_rows(&populated).await, [3, 3]);
        let (_deleted_root, deleted) =
            prepare_reuse_input(64, materialized_values(vec![]), Some(&populated)).await;
        assert_ne!(
            &populated.relation_publication().table_version_map()[&id],
            &deleted.relation_publication().table_version_map()[&id]
        );
        assert!(positive_rows(&deleted).await.is_empty());
        assert_eq!(
            positive_rows(&populated).await,
            [3, 3],
            "old facts remain readable"
        );

        let reopened = ProgrammaticFabricEpochBuilder::try_new(
            *deleted.identity(),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap()
        .reopen(Arc::clone(deleted.table_version_set()))
        .await
        .unwrap();
        let (_after_root, after) =
            prepare_reuse_input(65, materialized_values(vec![]), Some(&reopened)).await;
        assert_eq!(
            &deleted.relation_publication().table_version_map()[&id],
            &after.relation_publication().table_version_map()[&id]
        );
        assert!(positive_rows(&after).await.is_empty());
    }

    #[tokio::test]
    async fn immutable_inputs_reuse_exact_versions_across_abandonment_and_reopen() {
        fn input_pin(epoch: &ProgrammaticFabricEpoch) -> &super::super::delta_exact::ExactDeltaPin {
            &epoch.relation_publication().table_version_map()
                [&ProgrammaticRelationId::new("facts.input_values")]
        }
        let (_first_root, first) =
            prepare_reuse_candidate(51, vec![-1, 1, 1, 2], Some([1; 32]), None).await;
        let (second_root, second) =
            prepare_reuse_candidate(52, vec![2, 1, -1, 1], Some([1; 32]), Some(&first)).await;
        assert_eq!(input_pin(&first), input_pin(&second));
        assert_eq!(
            positive_rows(&second).await,
            [1, 1, 2],
            "multiplicity survives exact reuse"
        );
        assert!(
            !relation_layout(&second_root)
                .root()
                .to_file_path()
                .unwrap()
                .join(
                    input_pin(&first)
                        .canonical_root()
                        .to_file_path()
                        .unwrap()
                        .file_name()
                        .unwrap()
                )
                .exists(),
            "reused relation has no newly provisioned table"
        );
        let derived = ProgrammaticRelationId::new("facts.positive_values");
        assert_ne!(
            first.relation_publication().table_version_map()[&derived],
            second.relation_publication().table_version_map()[&derived],
            "transformation dependencies have not been declared eligible"
        );

        // A published but unselected candidate may already have committed changed relation files.
        // Preparing from the accepted predecessor again must not conflict with that abandoned work.
        let (_abandoned_root, abandoned) =
            prepare_reuse_candidate(53, vec![7], Some([2; 32]), Some(&second)).await;
        let (_retry_root, retry) =
            prepare_reuse_candidate(54, vec![9], Some([3; 32]), Some(&second)).await;
        assert_ne!(input_pin(&abandoned), input_pin(&retry));
        assert_eq!(positive_rows(&retry).await, [9]);
        assert_eq!(positive_rows(&abandoned).await, [7]);
        assert_eq!(positive_rows(&first).await, [1, 1, 2]);
        let (_empty_root, empty) =
            prepare_reuse_candidate(55, vec![], Some([4; 32]), Some(&retry)).await;
        assert_ne!(input_pin(&empty), input_pin(&retry));
        assert!(
            positive_rows(&empty).await.is_empty(),
            "empty replacement deletes old facts"
        );
        let (_no_key_root, no_key) = prepare_reuse_candidate(56, vec![], None, Some(&empty)).await;
        assert_ne!(
            input_pin(&no_key),
            input_pin(&empty),
            "unknown inputs do not qualify"
        );

        let reopened = ProgrammaticFabricEpochBuilder::try_new(
            *second.identity(),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap()
        .reopen(Arc::clone(second.table_version_set()))
        .await
        .unwrap();
        assert_eq!(input_pin(&reopened), input_pin(&first));
        assert_eq!(positive_rows(&reopened).await, [1, 1, 2]);
        let (_after_root, after) =
            prepare_reuse_candidate(57, vec![-1, 1, 1, 2], Some([1; 32]), Some(&reopened)).await;
        assert_eq!(
            input_pin(&after),
            input_pin(&first),
            "reopen retains immutable input identity"
        );
    }

    #[tokio::test]
    async fn wp32_beh_exact_selected_older_relation_versions_reconstruct_decoded_rows() {
        let temporary = TempDir::new().expect("exact relation fixture root");
        let first_identity = write_identity(1);
        let first_builder = positive_builder(first_identity.epoch_id(), vec![-1, 1, 2]);
        let first_observations = first_builder
            .provision_observation_histories(observation_roots(&temporary))
            .await
            .expect("provision observation histories");
        let first = first_builder
            .seal(
                first_identity,
                first_observations,
                ProgrammaticRelationDeltaPreparation::Genesis(relation_layout(&temporary)),
            )
            .await
            .expect("seal first exact epoch");
        assert_eq!(positive_rows(&first).await, vec![1, 2]);
        assert_eq!(first.relation_publication().table_versions().len(), 2);
        assert!(
            first
                .relation_publication()
                .table_versions()
                .all(|(_, pin)| pin.version() == 1)
        );

        let second_identity = write_identity(2);
        let second_builder = positive_builder(second_identity.epoch_id(), vec![-1, 7, 8]);
        let second_observations = first
            .observation_publication()
            .open_targets(&first.context().state())
            .await
            .expect("open first exact observation versions");
        let second = second_builder
            .seal(
                second_identity,
                second_observations,
                ProgrammaticRelationDeltaPreparation::Advance {
                    selected: first.relation_publication().clone(),
                    layout: relation_layout(&temporary),
                },
            )
            .await
            .expect("seal second exact epoch");
        assert_eq!(positive_rows(&second).await, vec![7, 8]);
        assert!(
            second
                .relation_publication()
                .table_versions()
                .all(|(_, pin)| pin.version() == 2)
        );

        let selected_older = Arc::clone(first.table_version_set());
        let reopened = ProgrammaticFabricEpochBuilder::try_new(
            first_identity.epoch_id(),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("fresh recovery builder")
        .reopen(selected_older)
        .await
        .expect("reopen exact older selected epoch without provider replay");
        assert_eq!(
            reopened.table_version_set_ref(),
            first.table_version_set_ref()
        );
        assert_eq!(positive_rows(&reopened).await, vec![1, 2]);
        assert_ne!(
            reopened.table_version_set_ref(),
            second.table_version_set_ref()
        );
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
