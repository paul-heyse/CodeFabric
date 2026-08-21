//! Immutable exact-version provider and private-catalog construction for serving snapshots.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::{
    FabricError, LocalProviderFactory, LocalProviderRequest, PublicationOutcome, exact_provider,
    validate_open_table,
};
use crate::fabric::batch_checksum;
use crate::schema_registry::{PublicationPinRole, TableSpec, table_spec, table_specs};
use arrow_array::{Array as _, BinaryArray, RecordBatch};
use arrow_select::concat::concat_batches;
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, Session, TableProvider};
use datafusion::common::{DataFusionError, Statistics};
use datafusion::logical_expr::{Expr, TableProviderFilterPushDown, TableType};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::prelude::SessionContext;
use deltalake::{DeltaTable, DeltaTableBuilder};

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
}

/// Exact immutable identity and provider for one pinned table.
#[derive(Clone)]
pub struct SnapshotProviderRecord {
    pub manifest: super::PublicationTableRecord,
    pub access_profile: DeltaAccessProfile,
    provider: Arc<dyn TableProvider>,
}

impl fmt::Debug for SnapshotProviderRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotProviderRecord")
            .field("manifest", &self.manifest)
            .field("access_profile", &self.access_profile)
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
    overlay_generation: u64,
    overlay_checksum: [u8; 32],
    providers: BTreeMap<i16, SnapshotProviderRecord>,
    catalog: Arc<FrozenCatalogProvider>,
    trace: Vec<SnapshotConstructionStage>,
    metrics: SnapshotConstructionMetrics,
}

impl SnapshotProviderCatalog {
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
            wrapped.insert(
                table_code,
                SnapshotProviderRecord {
                    manifest,
                    access_profile: DeltaAccessProfile::QueryServing,
                    provider,
                },
            );
        }

        trace.push(SnapshotConstructionStage::Validate);
        for record in wrapped.values() {
            validate_provider_record(record).await?;
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
            overlay_generation: overlay.generation(),
            overlay_checksum: overlay.checksum(),
            providers: wrapped,
            catalog,
            trace,
            metrics: SnapshotConstructionMetrics {
                provider_count,
                exact_version_count: provider_count,
                overlay_generation: overlay.generation(),
            },
        })
    }

    #[must_use]
    pub const fn publication_id(&self) -> [u8; 16] {
        self.publication_id
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

fn validate_publication_census(publication: &PublicationOutcome) -> Result<(), FabricError> {
    if publication.pointer.publication_id != publication.publication_id {
        return Err(FabricError::SnapshotProviderIntegrity(
            "publication outcome and current pointer identities differ".into(),
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
            || record.workspace_id != publication.pointer.workspace_id
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

async fn validate_provider_record(record: &SnapshotProviderRecord) -> Result<(), FabricError> {
    let spec = table_spec(record.manifest.table_code).expect("validated generated table code");
    if record.access_profile != DeltaAccessProfile::QueryServing
        || record.access_profile.skip_stats()
        || record.provider.schema() != spec.arrow_schema
        || record.manifest.schema_fingerprint != schema_fingerprint(spec)?
    {
        return Err(FabricError::SnapshotProviderIntegrity(format!(
            "{} access profile or schema differs",
            spec.name
        )));
    }
    let batch = provider_batch(record.provider(), spec).await?;
    let rows = i64::try_from(batch.num_rows())
        .map_err(|_| FabricError::SnapshotProviderIntegrity("row count exceeds i64".into()))?;
    if rows != record.manifest.row_count
        || owner_count(&batch)? != record.manifest.owner_count
        || batch_checksum(&batch)? != record.manifest.table_checksum
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
                    required: spec.required_for_publication,
                    validated: true,
                },
            );
        }
        PublicationOutcome {
            publication_id,
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
                record.provider().schema(),
                table_spec(table_code).unwrap().arrow_schema
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

        let mut wrong_checksum = publication.clone();
        wrong_checksum.tables.get_mut(&1).unwrap().table_checksum = [88; 32];
        assert!(matches!(
            SnapshotProviderCatalog::build(&wrong_checksum, &EmptySnapshotOverlay).await,
            Err(FabricError::SnapshotProviderIntegrity(_))
        ));

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
        assert_eq!(candidate.catalog().schema_names(), [CATALOG_SCHEMA]);
    }
}
