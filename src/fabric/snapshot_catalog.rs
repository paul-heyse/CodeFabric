//! Immutable exact-version provider and private-catalog construction for serving snapshots.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::{
    FabricError, LocalProviderFactory, LocalProviderRequest, PublicationOutcome, PublicationScope,
    exact_provider, validate_open_table,
};
#[cfg(test)]
use crate::fabric::batch_checksum;
use crate::fabric::publication::scope_filter;
use crate::schema_registry::{
    PublicationPinRole, TableSpec, table_scope_spec, table_spec, table_specs,
};
use crate::snapshot::SnapshotOverlayTable;
#[cfg(test)]
use arrow_array::RecordBatch;
use arrow_array::{Array as _, BinaryArray};
use arrow_row::{RowConverter, SortField};
#[cfg(test)]
use arrow_select::concat::concat_batches;
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, Session, TableProvider};
use datafusion::common::{DataFusionError, Statistics};
use datafusion::datasource::{ViewTable, provider_as_source};
use datafusion::execution::memory_pool::{FairSpillPool, MemoryConsumer, TrackConsumersPool};
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::logical_expr::LogicalPlanBuilder;
use datafusion::logical_expr::{Expr, TableProviderFilterPushDown, TableType};
use datafusion::physical_plan::{ExecutionPlan, execute_stream};
use datafusion::prelude::{SessionConfig, SessionContext};
use deltalake::{DeltaTable, DeltaTableBuilder};
use futures::StreamExt as _;
use std::num::NonZeroUsize;

const CATALOG_SCHEMA: &str = "cpg_base";

/// Closed purpose for every Delta handle constructed by CodeFabric.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaAccessProfile {
    QueryServing,
    PublicationMetadata,
    AppendOnlyWriter,
    VacuumFilesystemCheck,
    OptimizeDml,
}

impl DeltaAccessProfile {
    /// Complete profile registry used by structural governance.
    pub const ALL: [Self; 5] = [
        Self::QueryServing,
        Self::PublicationMetadata,
        Self::AppendOnlyWriter,
        Self::VacuumFilesystemCheck,
        Self::OptimizeDml,
    ];

    /// Whether Delta may omit file statistics while materializing this handle.
    #[must_use]
    pub const fn skip_stats(self) -> bool {
        matches!(
            self,
            Self::PublicationMetadata | Self::AppendOnlyWriter | Self::VacuumFilesystemCheck
        )
    }

    /// Library-native materialization posture bound to this profile.
    #[must_use]
    pub const fn materialization(self) -> DeltaMaterializationPosture {
        match self {
            Self::QueryServing => DeltaMaterializationPosture::ExactVersionProvider,
            Self::PublicationMetadata | Self::AppendOnlyWriter => {
                DeltaMaterializationPosture::MetadataOnly
            }
            Self::VacuumFilesystemCheck => DeltaMaterializationPosture::ActiveFiles,
            Self::OptimizeDml => DeltaMaterializationPosture::ActiveFilesAndStatistics,
        }
    }

    const fn requires_files(self) -> bool {
        !matches!(
            self.materialization(),
            DeltaMaterializationPosture::MetadataOnly
        )
    }
}

/// Closed Delta log/file materialization policy derived from an access profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaMaterializationPosture {
    ExactVersionProvider,
    MetadataOnly,
    ActiveFiles,
    ActiveFilesAndStatistics,
}

/// A loaded Delta table that permanently records why and how it was opened.
pub struct ProfiledDeltaHandle {
    table: DeltaTable,
    profile: DeltaAccessProfile,
}

impl ProfiledDeltaHandle {
    /// Access profile that governed this handle's construction.
    #[must_use]
    pub const fn profile(&self) -> DeltaAccessProfile {
        self.profile
    }

    /// Exact loaded Delta version.
    #[must_use]
    pub fn version(&self) -> Option<u64> {
        self.table.version()
    }

    pub(super) fn into_table(self) -> DeltaTable {
        self.table
    }
}

/// Sole constructor for classified Delta handles.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeltaHandleFactory;

impl DeltaHandleFactory {
    /// Open a local Delta handle under one explicit access profile.
    ///
    /// # Errors
    ///
    /// Rejects non-local storage, a query-serving request without an exact version,
    /// an unresolved version, or any Delta load failure.
    pub async fn open(
        table_uri: &str,
        version: Option<u64>,
        profile: DeltaAccessProfile,
    ) -> Result<ProfiledDeltaHandle, FabricError> {
        let request = LocalProviderRequest {
            location: table_uri.to_owned(),
            ..LocalProviderRequest::default()
        };
        let path = LocalProviderFactory::validate(&request)?;
        if profile == DeltaAccessProfile::QueryServing && version.is_none() {
            return Err(FabricError::SnapshotProviderIntegrity(
                "QUERY_SERVING requires an exact Delta version".into(),
            ));
        }
        let url = LocalProviderFactory::file_url(&path)?;
        let mut builder = DeltaTableBuilder::from_url(url)?.with_skip_stats(profile.skip_stats());
        if !profile.requires_files() {
            builder = builder.without_files();
        }
        if let Some(version) = version {
            builder = builder.with_version(version);
        }
        let table = builder.load().await?;
        if let Some(version) = version
            && table.version() != Some(version)
        {
            return Err(FabricError::SnapshotProviderIntegrity(format!(
                "requested Delta version {version}, opened {:?}",
                table.version()
            )));
        }
        Ok(ProfiledDeltaHandle { table, profile })
    }
}

/// Snapshot-owned overlay factory invoked exactly once per base provider before freeze.
pub trait SnapshotOverlayProviderFactory: fmt::Debug + Send + Sync {
    fn generation(&self) -> u64;
    fn checksum(&self) -> [u8; 32];
    fn memory_bytes(&self) -> u64;
    fn table_manifests(&self) -> Vec<SnapshotOverlayTable>;

    /// Wrap one exact-version base provider with this immutable overlay generation.
    ///
    /// # Errors
    ///
    /// Rejects schema, policy, tombstone, or memory-reservation inconsistencies.
    fn wrap(
        &self,
        spec: &TableSpec,
        base: Arc<dyn TableProvider>,
    ) -> Result<Arc<dyn TableProvider>, FabricError>;
}

/// Valid consolidated generation-zero overlay.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptySnapshotOverlay;

impl SnapshotOverlayProviderFactory for EmptySnapshotOverlay {
    fn generation(&self) -> u64 {
        0
    }

    fn checksum(&self) -> [u8; 32] {
        *blake3::hash(b"codefabric-empty-overlay-v1\0").as_bytes()
    }

    fn memory_bytes(&self) -> u64 {
        0
    }

    fn table_manifests(&self) -> Vec<SnapshotOverlayTable> {
        Vec::new()
    }

    fn wrap(
        &self,
        spec: &TableSpec,
        base: Arc<dyn TableProvider>,
    ) -> Result<Arc<dyn TableProvider>, FabricError> {
        Ok(Arc::new(OverlayIdentityProvider {
            inner: base,
            table_code: spec.table_code,
            generation: self.generation(),
            checksum: self.checksum(),
        }))
    }
}

struct OverlayIdentityProvider {
    inner: Arc<dyn TableProvider>,
    table_code: i16,
    generation: u64,
    checksum: [u8; 32],
}

impl fmt::Debug for OverlayIdentityProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OverlayIdentityProvider")
            .field("table_code", &self.table_code)
            .field("generation", &self.generation)
            .field("checksum", &self.checksum)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl TableProvider for OverlayIdentityProvider {
    fn schema(&self) -> arrow_schema::SchemaRef {
        self.inner.schema()
    }

    fn constraints(&self) -> Option<&datafusion::common::Constraints> {
        self.inner.constraints()
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        self.inner.scan(state, projection, filters, limit).await
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
        self.inner.supports_filters_pushdown(filters)
    }

    fn statistics(&self) -> Option<Statistics> {
        self.inner.statistics()
    }
}

/// Deterministic candidate-construction phase ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotConstructionStage {
    ResolveVersions,
    ConstructProviders,
    WrapOverlay,
    Validate,
    Freeze,
}

/// Non-timing construction evidence emitted for each frozen candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotConstructionMetrics {
    pub provider_count: usize,
    pub exact_version_count: usize,
    pub overlay_generation: u64,
    pub validation_scan_count: usize,
}

/// Exact immutable identity and provider for one pinned table.
#[derive(Clone)]
pub struct SnapshotProviderRecord {
    pub manifest: super::PublicationTableRecord,
    pub access_profile: DeltaAccessProfile,
    /// Digest of the ordered primary-key projection at the pinned Delta version.
    pub primary_key_digest: [u8; 32],
    /// Digest of the effective immutable table contents served by this provider.
    pub effective_content_digest: [u8; 32],
    pub effective_row_count: i64,
    pub effective_owner_count: i64,
    provider: Arc<dyn TableProvider>,
}

impl fmt::Debug for SnapshotProviderRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotProviderRecord")
            .field("manifest", &self.manifest)
            .field("access_profile", &self.access_profile)
            .field("primary_key_digest", &self.primary_key_digest)
            .field("effective_content_digest", &self.effective_content_digest)
            .field("effective_row_count", &self.effective_row_count)
            .field("effective_owner_count", &self.effective_owner_count)
            .finish_non_exhaustive()
    }
}

impl SnapshotProviderRecord {
    /// Lease the snapshot-owned provider object without reopening Delta.
    #[must_use]
    pub fn provider(&self) -> Arc<dyn TableProvider> {
        Arc::clone(&self.provider)
    }
}

#[derive(Debug)]
struct FrozenSchemaProvider {
    tables: BTreeMap<String, Arc<dyn TableProvider>>,
}

#[async_trait]
impl SchemaProvider for FrozenSchemaProvider {
    fn table_names(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    async fn table(&self, name: &str) -> Result<Option<Arc<dyn TableProvider>>, DataFusionError> {
        Ok(self.tables.get(name).cloned())
    }

    fn register_table(
        &self,
        name: String,
        _table: Arc<dyn TableProvider>,
    ) -> datafusion::common::Result<Option<Arc<dyn TableProvider>>> {
        Err(DataFusionError::Execution(format!(
            "SNAPSHOT_CATALOG_FROZEN:cannot register table {name}"
        )))
    }

    fn deregister_table(
        &self,
        name: &str,
    ) -> datafusion::common::Result<Option<Arc<dyn TableProvider>>> {
        Err(DataFusionError::Execution(format!(
            "SNAPSHOT_CATALOG_FROZEN:cannot deregister table {name}"
        )))
    }

    fn table_exist(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }
}

#[derive(Debug)]
struct FrozenCatalogProvider {
    schema: Arc<FrozenSchemaProvider>,
}

impl CatalogProvider for FrozenCatalogProvider {
    fn schema_names(&self) -> Vec<String> {
        vec![CATALOG_SCHEMA.to_owned()]
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        (name == CATALOG_SCHEMA).then(|| Arc::clone(&self.schema) as Arc<dyn SchemaProvider>)
    }

    fn register_schema(
        &self,
        name: &str,
        _schema: Arc<dyn SchemaProvider>,
    ) -> datafusion::common::Result<Option<Arc<dyn SchemaProvider>>> {
        Err(DataFusionError::Execution(format!(
            "SNAPSHOT_CATALOG_FROZEN:cannot register schema {name}"
        )))
    }

    fn deregister_schema(
        &self,
        name: &str,
        _cascade: bool,
    ) -> datafusion::common::Result<Option<Arc<dyn SchemaProvider>>> {
        Err(DataFusionError::Execution(format!(
            "SNAPSHOT_CATALOG_FROZEN:cannot deregister schema {name}"
        )))
    }
}

/// Frozen exact-version provider set and private DataFusion catalog for one candidate snapshot.
#[derive(Debug)]
pub struct SnapshotProviderCatalog {
    publication_id: [u8; 16],
    scope: PublicationScope,
    overlay_generation: u64,
    overlay_checksum: [u8; 32],
    overlay_memory_bytes: u64,
    overlay_tables: Arc<[SnapshotOverlayTable]>,
    providers: BTreeMap<i16, SnapshotProviderRecord>,
    catalog: Arc<FrozenCatalogProvider>,
    trace: Vec<SnapshotConstructionStage>,
    metrics: SnapshotConstructionMetrics,
}

impl SnapshotProviderCatalog {
    #[cfg(test)]
    pub(crate) fn empty_for_snapshot_tests(
        publication_id: [u8; 16],
        overlay_generation: u64,
        overlay_checksum: [u8; 32],
        scope: PublicationScope,
    ) -> Self {
        let schema = Arc::new(FrozenSchemaProvider {
            tables: BTreeMap::new(),
        });
        Self {
            publication_id,
            scope,
            overlay_generation,
            overlay_checksum,
            overlay_memory_bytes: 0,
            overlay_tables: Arc::from([]),
            providers: BTreeMap::new(),
            catalog: Arc::new(FrozenCatalogProvider { schema }),
            trace: vec![
                SnapshotConstructionStage::ResolveVersions,
                SnapshotConstructionStage::ConstructProviders,
                SnapshotConstructionStage::WrapOverlay,
                SnapshotConstructionStage::Validate,
                SnapshotConstructionStage::Freeze,
            ],
            metrics: SnapshotConstructionMetrics {
                provider_count: 0,
                exact_version_count: 0,
                overlay_generation,
                validation_scan_count: 0,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn single_for_snapshot_tests(
        publication_id: [u8; 16],
        workspace_id: [u8; 16],
        table_code: i16,
        overlay_checksum: [u8; 32],
        source_generation: i64,
        analysis_context_ids: Vec<[u8; 16]>,
    ) -> Self {
        let spec = snapshot_table_spec(table_code).expect("generated test table");
        let batch = RecordBatch::new_empty(Arc::clone(&spec.arrow_schema));
        Self::from_batches_for_snapshot_tests(
            publication_id,
            workspace_id,
            vec![(table_code, batch)],
            overlay_checksum,
            source_generation,
            analysis_context_ids,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_batches_for_snapshot_tests(
        publication_id: [u8; 16],
        workspace_id: [u8; 16],
        batches: Vec<(i16, RecordBatch)>,
        overlay_checksum: [u8; 32],
        source_generation: i64,
        analysis_context_ids: Vec<[u8; 16]>,
    ) -> Self {
        let mut providers = BTreeMap::new();
        let mut tables = BTreeMap::new();
        for (table_code, batch) in batches {
            let spec = snapshot_table_spec(table_code).expect("generated test table");
            assert_eq!(batch.schema(), spec.arrow_schema);
            let provider: Arc<dyn TableProvider> = Arc::new(
                datafusion::datasource::MemTable::try_new(
                    Arc::clone(&spec.arrow_schema),
                    vec![vec![batch.clone()]],
                )
                .expect("valid generated test schema"),
            );
            let checksum = batch_checksum(&batch).expect("test batch checksum");
            let record = SnapshotProviderRecord {
                manifest: super::PublicationTableRecord {
                    publication_id,
                    workspace_id,
                    table_code,
                    table_uri: format!("file:///snapshot-test/{}", spec.name),
                    delta_version: 1,
                    schema_fingerprint: schema_fingerprint(spec).expect("generated schema digest"),
                    row_count: i64::try_from(batch.num_rows()).expect("test row count"),
                    owner_count: 0,
                    table_checksum: checksum,
                    primary_key_digest: primary_key_digest(&batch, spec)
                        .expect("primary-key digest"),
                    required: spec.required_for_publication,
                    validated: true,
                },
                access_profile: DeltaAccessProfile::QueryServing,
                primary_key_digest: primary_key_digest(&batch, spec).expect("primary-key digest"),
                effective_content_digest: checksum,
                effective_row_count: i64::try_from(batch.num_rows()).expect("test row count"),
                effective_owner_count: owner_count(&batch).expect("test owner count"),
                provider: Arc::clone(&provider),
            };
            providers.insert(table_code, record);
            tables.insert(spec.name.to_owned(), provider);
        }
        let provider_count = providers.len();
        let schema = Arc::new(FrozenSchemaProvider { tables });
        Self {
            publication_id,
            scope: PublicationScope {
                workspace_id,
                source_generation,
                analysis_context_set_id: crate::identity::context_set_identity(
                    workspace_id,
                    &analysis_context_ids,
                )
                .expect("test context-set identity")
                .id,
                analysis_context_ids,
            },
            overlay_generation: 0,
            overlay_checksum,
            overlay_memory_bytes: 0,
            overlay_tables: Arc::from([]),
            providers,
            catalog: Arc::new(FrozenCatalogProvider { schema }),
            trace: vec![
                SnapshotConstructionStage::ResolveVersions,
                SnapshotConstructionStage::ConstructProviders,
                SnapshotConstructionStage::WrapOverlay,
                SnapshotConstructionStage::Validate,
                SnapshotConstructionStage::Freeze,
            ],
            metrics: SnapshotConstructionMetrics {
                provider_count,
                exact_version_count: provider_count,
                overlay_generation: 0,
                validation_scan_count: 0,
            },
        }
    }

    /// Construct and freeze every provider from one validated durable publication.
    ///
    /// # Errors
    ///
    /// Rejects an incomplete/unvalidated manifest, unresolved exact version, schema,
    /// row-count, owner-count, checksum, overlay, or private-catalog inconsistency.
    pub async fn build(
        publication: &PublicationOutcome,
        overlay: &dyn SnapshotOverlayProviderFactory,
    ) -> Result<Self, FabricError> {
        validate_publication_census(publication)?;
        let mut trace = vec![SnapshotConstructionStage::ResolveVersions];
        let mut opened = BTreeMap::new();
        trace.push(SnapshotConstructionStage::ConstructProviders);
        for (&table_code, record) in &publication.tables {
            let spec = snapshot_table_spec(table_code)?;
            let handle = DeltaHandleFactory::open(
                &record.table_uri,
                Some(record.delta_version),
                DeltaAccessProfile::QueryServing,
            )
            .await?;
            validate_open_table(&handle.table, spec)?;
            let provider = exact_provider(&handle.table, spec, handle.profile()).await?;
            opened.insert(table_code, (record.clone(), provider));
        }

        trace.push(SnapshotConstructionStage::WrapOverlay);
        let mut wrapped = BTreeMap::new();
        for (table_code, (manifest, provider)) in opened {
            let spec = snapshot_table_spec(table_code)?;
            let provider = overlay.wrap(spec, provider)?;
            let provider = scoped_provider(spec, &publication.scope, provider)?;
            let evidence = if overlay.generation() == 0 {
                None
            } else {
                Some(stream_provider_evidence(Arc::clone(&provider), spec).await?)
            };
            wrapped.insert(
                table_code,
                SnapshotProviderRecord {
                    primary_key_digest: evidence
                        .as_ref()
                        .map_or(manifest.primary_key_digest, |value| {
                            value.primary_key_digest
                        }),
                    effective_content_digest: evidence
                        .as_ref()
                        .map_or(manifest.table_checksum, |value| value.content_digest),
                    effective_row_count: evidence
                        .as_ref()
                        .map_or(manifest.row_count, |value| value.row_count),
                    effective_owner_count: evidence
                        .as_ref()
                        .map_or(manifest.owner_count, |value| value.owner_count),
                    manifest,
                    access_profile: DeltaAccessProfile::QueryServing,
                    provider,
                },
            );
        }

        trace.push(SnapshotConstructionStage::Validate);
        for record in wrapped.values() {
            validate_provider_record(record, overlay.generation())?;
        }
        let table_map = wrapped
            .iter()
            .map(|(&table_code, record)| {
                snapshot_table_spec(table_code)
                    .map(|spec| (spec.name.to_owned(), record.provider()))
            })
            .collect::<Result<_, _>>()?;
        let schema = Arc::new(FrozenSchemaProvider { tables: table_map });
        let catalog = Arc::new(FrozenCatalogProvider { schema });
        trace.push(SnapshotConstructionStage::Freeze);
        let provider_count = wrapped.len();
        Ok(Self {
            publication_id: publication.publication_id,
            scope: publication.scope.clone(),
            overlay_generation: overlay.generation(),
            overlay_checksum: overlay.checksum(),
            overlay_memory_bytes: overlay.memory_bytes(),
            overlay_tables: overlay.table_manifests().into(),
            providers: wrapped,
            catalog,
            trace,
            metrics: SnapshotConstructionMetrics {
                provider_count,
                exact_version_count: provider_count,
                overlay_generation: overlay.generation(),
                validation_scan_count: if overlay.generation() == 0 {
                    0
                } else {
                    provider_count
                },
            },
        })
    }

    #[must_use]
    pub const fn publication_id(&self) -> [u8; 16] {
        self.publication_id
    }

    #[must_use]
    pub const fn scope(&self) -> &PublicationScope {
        &self.scope
    }

    #[must_use]
    pub const fn overlay_generation(&self) -> u64 {
        self.overlay_generation
    }

    #[must_use]
    pub const fn overlay_checksum(&self) -> [u8; 32] {
        self.overlay_checksum
    }

    #[must_use]
    pub const fn overlay_memory_bytes(&self) -> u64 {
        self.overlay_memory_bytes
    }

    #[must_use]
    pub fn overlay_tables(&self) -> &[SnapshotOverlayTable] {
        &self.overlay_tables
    }

    #[must_use]
    pub fn trace(&self) -> &[SnapshotConstructionStage] {
        &self.trace
    }

    #[must_use]
    pub const fn metrics(&self) -> SnapshotConstructionMetrics {
        self.metrics
    }

    #[must_use]
    pub fn provider_record(&self, table_code: i16) -> Option<&SnapshotProviderRecord> {
        self.providers.get(&table_code)
    }

    /// Iterate the immutable records in stable table-code order.
    #[must_use]
    pub fn provider_records(&self) -> impl ExactSizeIterator<Item = &SnapshotProviderRecord> {
        self.providers.values()
    }

    /// Digest the exact effective table contents while excluding base/publication locators.
    #[must_use]
    pub fn effective_state_digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric-effective-snapshot-v1\0");
        for (&table_code, record) in &self.providers {
            hasher.update(&table_code.to_be_bytes());
            hasher.update(&record.effective_content_digest);
        }
        *hasher.finalize().as_bytes()
    }

    #[must_use]
    pub fn provider(&self, table_code: i16) -> Option<Arc<dyn TableProvider>> {
        self.provider_record(table_code)
            .map(SnapshotProviderRecord::provider)
    }

    #[must_use]
    pub fn catalog(&self) -> Arc<dyn CatalogProvider> {
        Arc::clone(&self.catalog) as Arc<dyn CatalogProvider>
    }
}

fn snapshot_table_spec(table_code: i16) -> Result<&'static TableSpec, FabricError> {
    table_spec(table_code).ok_or_else(|| {
        FabricError::SnapshotProviderIntegrity(format!(
            "publication names unknown table code {table_code}"
        ))
    })
}

fn scoped_provider(
    spec: &TableSpec,
    scope: &PublicationScope,
    provider: Arc<dyn TableProvider>,
) -> Result<Arc<dyn TableProvider>, FabricError> {
    let Some(filter) =
        table_scope_spec(spec.table_code).and_then(|selectors| scope_filter(selectors, scope))
    else {
        return Ok(provider);
    };
    let plan = LogicalPlanBuilder::scan(spec.name, provider_as_source(provider), None)?
        .filter(filter)?
        .build()?;
    Ok(Arc::new(ViewTable::new(plan, None)))
}

fn validate_publication_census(publication: &PublicationOutcome) -> Result<(), FabricError> {
    if publication.pointer.publication_id != publication.publication_id {
        return Err(FabricError::SnapshotProviderIntegrity(
            "publication outcome and current pointer identities differ".into(),
        ));
    }
    if publication.scope.workspace_id != publication.pointer.workspace_id
        || publication.scope.analysis_context_ids.is_empty()
        || !publication
            .scope
            .analysis_context_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || crate::identity::context_set_identity(
            publication.scope.workspace_id,
            &publication.scope.analysis_context_ids,
        )
        .map_err(|error| FabricError::SnapshotProviderIntegrity(error.to_string()))?
        .id != publication.scope.analysis_context_set_id
    {
        return Err(FabricError::SnapshotProviderIntegrity(
            "publication scope identity is invalid".into(),
        ));
    }
    let expected = table_specs()
        .iter()
        .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
        .map(|spec| spec.table_code)
        .collect::<BTreeSet<_>>();
    if publication.tables.keys().copied().collect::<BTreeSet<_>>() != expected {
        return Err(FabricError::SnapshotProviderIntegrity(
            "publication manifest does not match the generated PINNED_DATA census".into(),
        ));
    }
    for (&table_code, record) in &publication.tables {
        let spec = table_spec(table_code).expect("generated table code");
        if record.publication_id != publication.publication_id
            || record.workspace_id != publication.scope.workspace_id
            || record.table_code != table_code
            || record.required != spec.required_for_publication
            || !record.validated
        {
            return Err(FabricError::SnapshotProviderIntegrity(format!(
                "{} manifest identity or validation state differs",
                spec.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
async fn provider_batch(
    provider: Arc<dyn TableProvider>,
    spec: &TableSpec,
) -> Result<RecordBatch, FabricError> {
    let batches = SessionContext::new()
        .read_table(provider)?
        .collect()
        .await?;
    Ok(concat_batches(&spec.arrow_schema, &batches)?)
}

struct ProviderEvidence {
    primary_key_digest: [u8; 32],
    content_digest: [u8; 32],
    row_count: i64,
    owner_count: i64,
}

async fn stream_provider_evidence(
    provider: Arc<dyn TableProvider>,
    spec: &TableSpec,
) -> Result<ProviderEvidence, FabricError> {
    let limits = crate::schema_registry::serving_resource_profile();
    let pool = Arc::new(TrackConsumersPool::new(
        FairSpillPool::new(limits.max_snapshot_validation_bytes),
        NonZeroUsize::new(5).expect("positive tracked-consumer count"),
    ));
    let runtime = Arc::new(RuntimeEnvBuilder::new().with_memory_pool(pool).build()?);
    let context = SessionContext::new_with_config_rt(
        SessionConfig::new().with_batch_size(limits.batch_size),
        runtime,
    );
    let plan = context
        .state()
        .create_physical_plan(&context.read_table(provider)?.into_optimized_plan()?)
        .await?;
    let mut stream = execute_stream(plan, context.task_ctx())?;
    let full_converter = RowConverter::new(
        spec.arrow_schema
            .fields()
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect(),
    )?;
    let primary_indices = spec
        .primary_key
        .iter()
        .map(|name| spec.arrow_schema.index_of(name))
        .collect::<Result<Vec<_>, _>>()?;
    let primary_schema = Arc::new(spec.arrow_schema.project(&primary_indices)?);
    let primary_converter = RowConverter::new(
        primary_schema
            .fields()
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect(),
    )?;
    let owner_index = spec.arrow_schema.index_of("owner_id").ok();
    let reservation = MemoryConsumer::new("snapshot-provider-validation")
        .register(&context.runtime_env().memory_pool);
    let mut rows = Vec::<Vec<u8>>::new();
    let mut primary_rows = Vec::<Vec<u8>>::new();
    let mut owners = BTreeSet::new();
    let mut bytes = 0_usize;
    let mut batches = 0_usize;
    while let Some(batch) = stream.next().await.transpose()? {
        batches += 1;
        if rows.len() + batch.num_rows() > limits.max_snapshot_validation_rows
            || batches > limits.max_snapshot_validation_batches
        {
            return Err(FabricError::SnapshotProviderIntegrity(format!(
                "{} validation exceeds generated row/batch budget",
                spec.name
            )));
        }
        let encoded = full_converter.convert_columns(batch.columns())?;
        let primary_columns = primary_indices
            .iter()
            .map(|&index| Arc::clone(batch.column(index)))
            .collect::<Vec<_>>();
        let encoded_primary = primary_converter.convert_columns(&primary_columns)?;
        for row in &encoded {
            bytes = bytes.saturating_add(row.data().len());
            rows.push(row.data().to_vec());
        }
        for row in &encoded_primary {
            bytes = bytes.saturating_add(row.data().len());
            primary_rows.push(row.data().to_vec());
        }
        if bytes > limits.max_snapshot_validation_bytes {
            return Err(FabricError::SnapshotProviderIntegrity(format!(
                "{} validation exceeds generated byte budget",
                spec.name
            )));
        }
        reservation.try_resize(bytes)?;
        if let Some(index) = owner_index {
            let values = batch
                .column(index)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .expect("generated owner_id is Binary");
            owners.extend(values.iter().flatten().map(<[u8]>::to_vec));
        }
    }
    let row_count = i64::try_from(rows.len())
        .map_err(|_| FabricError::SnapshotProviderIntegrity("row count exceeds i64".into()))?;
    let owner_count = i64::try_from(owners.len())
        .map_err(|_| FabricError::SnapshotProviderIntegrity("owner count exceeds i64".into()))?;
    Ok(ProviderEvidence {
        content_digest: encoded_rows_checksum(&spec.arrow_schema, &mut rows),
        primary_key_digest: encoded_rows_checksum(&primary_schema, &mut primary_rows),
        row_count,
        owner_count,
    })
}

fn encoded_rows_checksum(schema: &arrow_schema::Schema, rows: &mut [Vec<u8>]) -> [u8; 32] {
    rows.sort_unstable();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-arrow-batch-v1\0");
    if let Some(digest) = schema.metadata().get("com.codefabric.cpg.schema_digest") {
        hasher.update(digest.as_bytes());
    }
    hasher.update(&(rows.len() as u64).to_be_bytes());
    for row in rows {
        hasher.update(&(row.len() as u64).to_be_bytes());
        hasher.update(row);
    }
    *hasher.finalize().as_bytes()
}

fn schema_fingerprint(spec: &TableSpec) -> Result<[u8; 32], FabricError> {
    let hex = spec
        .schema_digest
        .strip_prefix("b3:")
        .filter(|value| value.len() == 64)
        .ok_or_else(|| {
            FabricError::SnapshotProviderIntegrity(format!(
                "{} schema digest framing is invalid",
                spec.name
            ))
        })?;
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).map_err(|_| {
            FabricError::SnapshotProviderIntegrity(format!(
                "{} schema digest is not hexadecimal",
                spec.name
            ))
        })?;
    }
    Ok(digest)
}

#[cfg(test)]
fn owner_count(batch: &RecordBatch) -> Result<i64, FabricError> {
    let Ok(index) = batch.schema().index_of("owner_id") else {
        return Ok(0);
    };
    let owners = batch
        .column(index)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(|| FabricError::SnapshotProviderIntegrity("owner_id is not Binary".into()))?;
    i64::try_from(
        owners
            .iter()
            .flatten()
            .map(<[u8]>::to_vec)
            .collect::<BTreeSet<_>>()
            .len(),
    )
    .map_err(|_| FabricError::SnapshotProviderIntegrity("owner count exceeds i64".into()))
}

#[cfg(test)]
fn primary_key_digest(batch: &RecordBatch, spec: &TableSpec) -> Result<[u8; 32], FabricError> {
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

fn validate_provider_record(
    record: &SnapshotProviderRecord,
    overlay_generation: u64,
) -> Result<(), FabricError> {
    let spec = table_spec(record.manifest.table_code).expect("validated generated table code");
    if record.access_profile != DeltaAccessProfile::QueryServing
        || record.access_profile.skip_stats()
        || record.provider.schema().fields() != spec.arrow_schema.fields()
        || record.manifest.schema_fingerprint != schema_fingerprint(spec)?
    {
        return Err(FabricError::SnapshotProviderIntegrity(format!(
            "{} access profile or schema differs",
            spec.name
        )));
    }
    if record.effective_row_count < 0
        || record.effective_owner_count < 0
        || (overlay_generation == 0
            && (record.effective_content_digest != record.manifest.table_checksum
                || record.primary_key_digest != record.manifest.primary_key_digest
                || record.effective_row_count != record.manifest.row_count
                || record.effective_owner_count != record.manifest.owner_count))
    {
        return Err(FabricError::SnapshotProviderIntegrity(format!(
            "{} row, owner, or checksum evidence differs",
            spec.name
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fabric::{
        CurrentPublicationRecord, PublicationTableRecord, WorkspaceFabric, bootstrap_workspace,
    };
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

    async fn publication(fabric: &WorkspaceFabric) -> PublicationOutcome {
        let publication_id = [20; 16];
        let mut tables = BTreeMap::new();
        for spec in table_specs()
            .iter()
            .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
        {
            let table = fabric.table(spec.table_code).unwrap();
            let batch = provider_batch(table.provider(), spec).await.unwrap();
            tables.insert(
                spec.table_code,
                PublicationTableRecord {
                    publication_id,
                    workspace_id: [1; 16],
                    table_code: spec.table_code,
                    table_uri: LocalProviderFactory::file_url(&table.path)
                        .unwrap()
                        .to_string(),
                    delta_version: table.version().unwrap(),
                    schema_fingerprint: schema_fingerprint(spec).unwrap(),
                    row_count: i64::try_from(batch.num_rows()).unwrap(),
                    owner_count: owner_count(&batch).unwrap(),
                    table_checksum: batch_checksum(&batch).unwrap(),
                    primary_key_digest: primary_key_digest(&batch, spec).unwrap(),
                    required: spec.required_for_publication,
                    validated: true,
                },
            );
        }
        PublicationOutcome {
            publication_id,
            scope: PublicationScope {
                workspace_id: [1; 16],
                source_generation: 1,
                analysis_context_set_id: crate::identity::context_set_identity([1; 16], &[[2; 16]])
                    .unwrap()
                    .id,
                analysis_context_ids: vec![[2; 16]],
            },
            pointer: CurrentPublicationRecord {
                workspace_id: [1; 16],
                publication_id,
                pointer_generation: 1,
                updated_at_micros: 1_000,
            },
            tables,
        }
    }

    async fn fixture(root: &Path) -> (WorkspaceFabric, PublicationOutcome) {
        let fabric = bootstrap_workspace(root, &workspace_record())
            .await
            .unwrap();
        let publication = publication(&fabric).await;
        (fabric, publication)
    }

    #[derive(Debug)]
    struct ChangedIdentityOverlay;

    impl SnapshotOverlayProviderFactory for ChangedIdentityOverlay {
        fn generation(&self) -> u64 {
            1
        }

        fn checksum(&self) -> [u8; 32] {
            [0x51; 32]
        }

        fn memory_bytes(&self) -> u64 {
            0
        }

        fn table_manifests(&self) -> Vec<SnapshotOverlayTable> {
            Vec::new()
        }

        fn wrap(
            &self,
            _spec: &TableSpec,
            base: Arc<dyn TableProvider>,
        ) -> Result<Arc<dyn TableProvider>, FabricError> {
            Ok(base)
        }
    }

    #[tokio::test]
    async fn wp26_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (_fabric, publication) = fixture(root.path()).await;
        let candidate = SnapshotProviderCatalog::build(&publication, &EmptySnapshotOverlay)
            .await
            .unwrap();
        assert_eq!(
            candidate.trace(),
            [
                SnapshotConstructionStage::ResolveVersions,
                SnapshotConstructionStage::ConstructProviders,
                SnapshotConstructionStage::WrapOverlay,
                SnapshotConstructionStage::Validate,
                SnapshotConstructionStage::Freeze,
            ]
        );
        assert_eq!(candidate.publication_id(), publication.publication_id);
        assert_eq!(candidate.overlay_generation(), 0);
        for (&table_code, expected) in &publication.tables {
            let record = candidate.provider_record(table_code).unwrap();
            assert_eq!(&record.manifest, expected);
            assert_eq!(record.access_profile, DeltaAccessProfile::QueryServing);
            assert!(!record.access_profile.skip_stats());
            assert_eq!(
                record.provider().schema().fields(),
                table_spec(table_code).unwrap().arrow_schema.fields()
            );
        }
    }

    #[tokio::test]
    async fn wp26_structural_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (_fabric, publication) = fixture(root.path()).await;
        let candidate = SnapshotProviderCatalog::build(&publication, &EmptySnapshotOverlay)
            .await
            .unwrap();
        assert_eq!(DeltaAccessProfile::ALL.len(), 5);
        assert_eq!(
            DeltaAccessProfile::ALL.map(DeltaAccessProfile::skip_stats),
            [false, true, true, true, false]
        );
        assert_eq!(
            DeltaAccessProfile::QueryServing.materialization(),
            DeltaMaterializationPosture::ExactVersionProvider
        );
        assert!(!DeltaAccessProfile::QueryServing.skip_stats());
        let first = candidate.provider(1).unwrap();
        let second = candidate.provider(1).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let schema = candidate.catalog().schema(CATALOG_SCHEMA).unwrap();
        let catalog_provider = schema.table("workspace").await.unwrap().unwrap();
        assert!(Arc::ptr_eq(&first, &catalog_provider));
        assert_eq!(
            first.statistics().is_some(),
            catalog_provider.statistics().is_some()
        );
    }

    #[tokio::test]
    async fn wp26_negative_zero_state() {
        assert!(matches!(
            DeltaHandleFactory::open(
                "file:///tmp/does-not-matter",
                None,
                DeltaAccessProfile::QueryServing,
            )
            .await,
            Err(FabricError::SnapshotProviderIntegrity(_))
        ));
        assert!(matches!(
            DeltaHandleFactory::open(
                "s3://bucket/table",
                Some(0),
                DeltaAccessProfile::QueryServing,
            )
            .await,
            Err(FabricError::LocalProfile(_))
        ));

        let root = tempfile::tempdir().unwrap();
        let (_fabric, publication) = fixture(root.path()).await;
        let mut unresolved = publication.clone();
        unresolved.tables.get_mut(&1).unwrap().delta_version = u64::MAX;
        assert!(
            SnapshotProviderCatalog::build(&unresolved, &EmptySnapshotOverlay)
                .await
                .is_err()
        );

        let mut wrong_schema = publication.clone();
        wrong_schema.tables.get_mut(&1).unwrap().schema_fingerprint = [99; 32];
        assert!(matches!(
            SnapshotProviderCatalog::build(&wrong_schema, &EmptySnapshotOverlay).await,
            Err(FabricError::SnapshotProviderIntegrity(_))
        ));

        let mut corrected_evidence = publication.clone();
        corrected_evidence
            .tables
            .get_mut(&1)
            .unwrap()
            .table_checksum = [88; 32];
        let evidence_bound =
            SnapshotProviderCatalog::build(&corrected_evidence, &EmptySnapshotOverlay)
                .await
                .unwrap();
        assert_eq!(
            evidence_bound
                .provider_record(1)
                .unwrap()
                .effective_content_digest,
            [88; 32]
        );
        assert_eq!(evidence_bound.metrics().validation_scan_count, 0);

        let candidate = SnapshotProviderCatalog::build(&publication, &EmptySnapshotOverlay)
            .await
            .unwrap();
        let catalog = candidate.catalog();
        let schema = catalog.schema(CATALOG_SCHEMA).unwrap();
        assert!(
            catalog
                .register_schema("late", Arc::clone(&schema))
                .is_err()
        );
        assert!(
            schema
                .register_table("late".into(), candidate.provider(1).unwrap())
                .is_err()
        );
    }

    #[tokio::test]
    async fn wp26_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (_fabric, publication) = fixture(root.path()).await;
        let candidate = SnapshotProviderCatalog::build(&publication, &EmptySnapshotOverlay)
            .await
            .unwrap();
        let metrics = candidate.metrics();
        assert_eq!(metrics.provider_count, publication.tables.len());
        assert_eq!(metrics.exact_version_count, publication.tables.len());
        assert_eq!(metrics.overlay_generation, 0);
        assert_eq!(metrics.validation_scan_count, 0);
        assert_eq!(candidate.catalog().schema_names(), [CATALOG_SCHEMA]);

        let changed = SnapshotProviderCatalog::build(&publication, &ChangedIdentityOverlay)
            .await
            .unwrap();
        assert_eq!(
            changed.metrics().validation_scan_count,
            publication.tables.len()
        );
    }
}
