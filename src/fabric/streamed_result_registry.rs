//! Session-authorized registry for sealed manifest-last query packages.
//!
//! The registry owns only control metadata plus sealed object capabilities. Page bytes are read
//! from object storage on demand and bounded before crossing the public resource stream. Internal
//! object paths never appear in the public manifest.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use crate::resource_budget::{
    ChargedSlice, ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation,
    ResourceScopeKind,
};
use object_store::path::Path as ObjectPath;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;

use super::arrow_result_resource::{QueryExecutionPin, ResultResourceLease};
use super::command::{EpochId, LeaseId, PrincipalId};
use super::query_coordinator::RetainedPackageLocator;
use super::relational_query_runtime::StreamedRelationalQueryPublication;
use super::streamed_result_package::{
    PendingResultObjectSet, ResultProvenance, SealedStreamedResultPackage,
    StreamedResultPackageBuilder, StreamedResultPackageError,
};

const MAX_REFERENCE_RESOURCES: usize = 128;

fn validate_budget_workspace(
    budget: &ResourceBudget,
    workspace: super::command::WorkspaceId,
) -> Result<(), StreamedResultRegistryError> {
    if budget
        .ancestor_owner(ResourceScopeKind::Workspace)
        .is_some_and(|owner| owner.id == *workspace.as_bytes())
    {
        Ok(())
    } else {
        Err(StreamedResultRegistryError::BudgetOwnerMismatch)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedResultRegistration {
    pub processing: Vec<super::processing_status::QueryProcessing>,
    pub package_id: String,
    pub manifest_resource_id: String,
    pub manifest: StreamedResultResourceRegistration,
    pub pages: Vec<StreamedResultResourceRegistration>,
    pub total_rows: u64,
    pub total_pages: u64,
    pub total_bytes: u64,
    pub retained_locator: RetainedPackageLocator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedResultResourceRegistration {
    pub public_handle: String,
    pub page_ordinal: Option<u32>,
    pub media_type: String,
    pub byte_length: u64,
    pub content_checksum: String,
    pub expires_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceResourcePublication {
    pub principal_id: PrincipalId,
    pub workspace_id: super::command::WorkspaceId,
    pub daemon_generation: u64,
    pub policy_generation: u64,
    pub revocation_generation: u64,
    pub selector: StreamedReferenceSelector,
    pub reference_id: String,
    pub media_type: String,
    pub content: Vec<u8>,
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceResourceRegistration {
    pub public_handle: String,
    pub content_checksum: String,
    pub byte_length: u64,
    pub expires_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedReferenceSelector {
    pub kind: String,
    pub version: Option<String>,
}

/// Closed selector set authorized by one daemon-minted public resource handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamedResourceSelector {
    Manifest,
    Page(u32),
    Reference(StreamedReferenceSelector),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedResourceRead {
    pub principal_id: PrincipalId,
    pub workspace_ids: Arc<BTreeSet<super::command::WorkspaceId>>,
    pub daemon_generation: u64,
    pub policy_generation: u64,
    pub revocation_generation: u64,
    pub public_handle: String,
    pub selector: StreamedResourceSelector,
    pub offset: u64,
    pub maximum_bytes: usize,
    pub observed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedResourceChunk {
    pub public_handle: String,
    pub offset: u64,
    pub next_offset: u64,
    pub total_length: u64,
    pub content_checksum: String,
    pub bytes: ChargedSlice<u8>,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamedReleaseOutcome {
    Released,
    AlreadyReleased,
}

#[derive(Debug)]
struct ResultPackageEntry {
    query_id: String,
    manifest_resource_id: String,
    manifest_bytes: ChargedSlice<u8>,
    _metadata_charge: ResourceReservation,
    package_id: String,
    resource_handles: BTreeSet<String>,
    total_rows: u64,
    total_pages: u64,
    total_bytes: u64,
    retained_locator: RetainedPackageLocator,
    package: Option<SealedStreamedResultPackage>,
    cleanup: Option<ResultCleanup>,
}

#[derive(Debug)]
enum ResultCleanup {
    Publication(StreamedRelationalQueryPublication),
    Package(SealedStreamedResultPackage),
}

impl ResultCleanup {
    fn into_package(self) -> SealedStreamedResultPackage {
        match self {
            Self::Publication(publication) => publication.into_released_package(),
            Self::Package(package) => package,
        }
    }
}

#[derive(Debug)]
struct ResultResourceEntry {
    principal_id: PrincipalId,
    workspace_id: super::command::WorkspaceId,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    package_id: String,
    selector: StreamedResourceSelector,
    media_type: String,
    byte_length: u64,
    content_checksum: String,
    expires_at_unix_ms: i64,
    release_id: Option<String>,
    released: bool,
}

#[derive(Debug, Default)]
struct ResultRegistryState {
    packages: BTreeMap<String, ResultPackageEntry>,
    resources: BTreeMap<String, ResultResourceEntry>,
    cleanup: BTreeMap<String, SealedStreamedResultPackage>,
}

enum ResultReadSource {
    Manifest(ChargedSlice<u8>),
    Page(SealedStreamedResultPackage, u32),
}

#[derive(Debug)]
struct ReferenceEntry {
    _metadata_charge: ResourceReservation,
    principal_id: PrincipalId,
    workspace_id: super::command::WorkspaceId,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    selector: StreamedReferenceSelector,
    content_checksum: String,
    content: ChargedSlice<u8>,
    expires_at_unix_ms: i64,
    release_id: Option<String>,
    released: bool,
}

/// Bounded process-wide owner of all externally reachable streamed query packages.
#[derive(Debug)]
pub struct StreamedResultRegistry {
    budget: ResourceBudget,
    maximum_chunk_bytes: usize,
    package_builder: OnceLock<StreamedResultPackageBuilder>,
    source_disclosure_reader: OnceLock<crate::operational_store::OperationalReaderFactory>,
    results: Mutex<ResultRegistryState>,
    references: Mutex<BTreeMap<String, ReferenceEntry>>,
}

impl StreamedResultRegistry {
    /// Construct the registry with the exact transport chunk bound.
    ///
    /// # Errors
    /// Rejects a zero chunk limit.
    pub fn try_new(
        maximum_chunk_bytes: usize,
        budget: ResourceBudget,
    ) -> Result<Self, StreamedResultRegistryError> {
        if maximum_chunk_bytes == 0 {
            return Err(StreamedResultRegistryError::InvalidChunkBound);
        }
        Ok(Self {
            budget,
            maximum_chunk_bytes,
            package_builder: OnceLock::new(),
            source_disclosure_reader: OnceLock::new(),
            results: Mutex::new(ResultRegistryState::default()),
            references: Mutex::new(BTreeMap::new()),
        })
    }

    /// Install the exact package builder already owned by the semantic backend. This one-time
    /// composition binding gives restart recovery the same sink and limits as initial sealing.
    pub(crate) fn install_package_builder(&self, builder: StreamedResultPackageBuilder) {
        let _ = self.package_builder.set(builder);
    }

    pub(crate) fn install_source_disclosure_reader(
        &self,
        reader: crate::operational_store::OperationalReaderFactory,
    ) {
        let _ = self.source_disclosure_reader.set(reader);
    }

    /// Register one bounded reference projection behind an unpredictable daemon-owned handle.
    ///
    /// # Errors
    /// Rejects invalid identities, expired lifetimes, exhausted capacity, or handle collisions.
    pub async fn publish_reference(
        &self,
        publication: ReferenceResourcePublication,
    ) -> Result<ReferenceResourceRegistration, StreamedResultRegistryError> {
        validate_budget_workspace(&self.budget, publication.workspace_id)?;
        if publication.daemon_generation == 0
            || publication.policy_generation == 0
            || publication.reference_id.is_empty()
            || publication.media_type.is_empty()
            || publication.content.is_empty()
            || publication.content.len() > 1024 * 1024
            || publication
                .reference_id
                .len()
                .saturating_add(publication.media_type.len())
                .saturating_add(publication.selector.kind.len())
                .saturating_add(publication.selector.version.as_ref().map_or(0, String::len))
                > 1024 * 1024
            || publication.expires_at_unix_ms <= publication.issued_at_unix_ms
        {
            return Err(StreamedResultRegistryError::InvalidIdentity);
        }
        let nonce = crate::identity::random_registration_nonce()
            .map_err(|_| StreamedResultRegistryError::Entropy)?;
        let public_handle = format!(
            "public:reference:{}",
            identity(
                b"codefabric.public-reference-handle.v2",
                &[
                    &publication.daemon_generation.to_be_bytes(),
                    publication.principal_id.as_bytes(),
                    publication.workspace_id.as_bytes(),
                    publication.reference_id.as_bytes(),
                    &nonce,
                ],
            )
            .trim_start_matches("b3:")
        );
        let content_checksum = checksum(&publication.content);
        let byte_length = u64::try_from(publication.content.len())
            .map_err(|_| StreamedResultRegistryError::RangeOverflow)?;
        let registration = ReferenceResourceRegistration {
            public_handle: public_handle.clone(),
            content_checksum: content_checksum.clone(),
            byte_length,
            expires_at_unix_ms: publication.expires_at_unix_ms,
        };
        let entry = ReferenceEntry {
            _metadata_charge: self.budget.try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: publication
                        .selector
                        .kind
                        .capacity()
                        .saturating_add(
                            publication
                                .selector
                                .version
                                .as_ref()
                                .map_or(0, String::capacity),
                        )
                        .saturating_add(1024) as u64,
                    ..ResourceAmounts::default()
                },
            )?,
            principal_id: publication.principal_id,
            workspace_id: publication.workspace_id,
            daemon_generation: publication.daemon_generation,
            policy_generation: publication.policy_generation,
            revocation_generation: publication.revocation_generation,
            selector: publication.selector,
            content_checksum,
            content: self
                .budget
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: publication.content.capacity() as u64,
                        ..ResourceAmounts::default()
                    },
                )?
                .into_charged_vec(publication.content)?,
            expires_at_unix_ms: publication.expires_at_unix_ms,
            release_id: None,
            released: false,
        };
        let mut references = self.references.lock().await;
        references.retain(|_, entry| entry.expires_at_unix_ms > publication.issued_at_unix_ms);
        if references.len() >= MAX_REFERENCE_RESOURCES {
            return Err(StreamedResultRegistryError::ReferenceCapacity);
        }
        if references.insert(public_handle, entry).is_some() {
            return Err(StreamedResultRegistryError::PackageIdentityCollision);
        }
        Ok(registration)
    }

    /// Register one sealed package after execution has closed successfully.
    ///
    /// # Errors
    /// Rejects invalid identities, exhausted capacity, and package or resource collisions.
    pub async fn publish(
        &self,
        query_id: &str,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        publication: StreamedRelationalQueryPublication,
    ) -> Result<StreamedResultRegistration, StreamedResultRegistryError> {
        let principal_id = publication.owner().agent_id();
        let workspace_id = publication.owner().workspace_id();
        let package = publication.package().clone();
        self.publish_package(
            query_id,
            principal_id,
            workspace_id,
            daemon_generation,
            policy_generation,
            revocation_generation,
            package,
            ResultCleanup::Publication(publication),
            None,
        )
        .await
    }

    /// Reconstruct a retained package after restart and mint fresh session/generation-bound
    /// public handles. The durable locator is private coordinator state; every object, pin,
    /// checksum, length, lease, and derived package identity is re-proved before admission.
    ///
    /// # Errors
    /// Rejects expired leases, unavailable recovery, resource exhaustion, and retained-locator drift.
    #[allow(clippy::too_many_arguments)]
    pub async fn reissue_retained(
        &self,
        query_id: &str,
        principal_id: PrincipalId,
        workspace_id: super::command::WorkspaceId,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        locator: &RetainedPackageLocator,
        observed_at_unix_ms: i64,
    ) -> Result<StreamedResultRegistration, StreamedResultRegistryError> {
        if observed_at_unix_ms < locator.lease_issued_at_unix_ms
            || observed_at_unix_ms >= locator.lease_expires_at_unix_ms
        {
            return Err(StreamedResultRegistryError::Expired);
        }
        let builder = self
            .package_builder
            .get()
            .cloned()
            .ok_or(StreamedResultRegistryError::RecoveryUnavailable)?;
        let epoch_id = EpochId::from_bytes(decode_hex_exact(&locator.epoch_id)?);
        let query_execution =
            QueryExecutionPin::from_bytes(decode_hex_exact(&locator.query_execution)?);
        let lease_id = LeaseId::from_bytes(decode_hex_exact(&locator.lease_id)?);
        let lease = ResultResourceLease::try_new(
            lease_id,
            locator.lease_issued_at_unix_ms,
            locator.lease_expires_at_unix_ms,
        )
        .map_err(|_| StreamedResultRegistryError::RetainedLocatorMismatch)?;
        let package = builder
            .reopen(
                ObjectPath::from(locator.manifest_object_path.clone()),
                epoch_id,
                query_execution,
                lease,
            )
            .await?;
        self.publish_package(
            query_id,
            principal_id,
            workspace_id,
            daemon_generation,
            policy_generation,
            revocation_generation,
            package.clone(),
            ResultCleanup::Package(package),
            Some(locator),
        )
        .await
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn publish_package(
        &self,
        query_id: &str,
        principal_id: PrincipalId,
        workspace_id: super::command::WorkspaceId,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        package: SealedStreamedResultPackage,
        cleanup: ResultCleanup,
        expected_locator: Option<&RetainedPackageLocator>,
    ) -> Result<StreamedResultRegistration, StreamedResultRegistryError> {
        validate_budget_workspace(&self.budget, workspace_id)?;
        validate_budget_workspace(package.resource_budget(), workspace_id)?;
        if !self.budget.same_root(package.resource_budget()) {
            return Err(StreamedResultRegistryError::BudgetOwnerMismatch);
        }
        if query_id.is_empty() || daemon_generation == 0 || policy_generation == 0 {
            return Err(StreamedResultRegistryError::InvalidIdentity);
        }
        let manifest = package.manifest();
        let mut metadata_charge = self.budget.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: package
                    .manifest_byte_length()
                    .checked_add(query_id.len() as u64)
                    .and_then(|n| n.checked_mul(32))
                    .ok_or(StreamedResultRegistryError::RangeOverflow)?,
                ..ResourceAmounts::default()
            },
        )?;
        let package_id = identity(
            b"codefabric.streamed-result-package.v2",
            &[
                workspace_id.as_bytes(),
                principal_id.as_bytes(),
                package.query_execution().as_bytes(),
                package.manifest_checksum(),
            ],
        );
        let manifest_resource_id = identity(
            b"codefabric.streamed-result-manifest-resource.v2",
            &[package_id.as_bytes(), package.manifest_checksum()],
        );
        let lease = package.lease();
        let retained_locator = RetainedPackageLocator {
            manifest_object_path: package.manifest_path().to_string(),
            page_object_paths: manifest
                .pages
                .iter()
                .map(|page| page.object_path.clone())
                .collect(),
            epoch_id: hex(package.epoch_id().as_bytes()),
            query_execution: hex(package.query_execution().as_bytes()),
            lease_id: hex(lease.lease_id().as_bytes()),
            lease_issued_at_unix_ms: lease.issued_at_unix_ms(),
            lease_expires_at_unix_ms: lease.expires_at_unix_ms(),
            expected_manifest_checksum: format!("b3:{}", hex(package.manifest_checksum())),
            expected_manifest_byte_length: package.manifest_byte_length(),
            package_id: package_id.clone(),
            manifest_resource_id: manifest_resource_id.clone(),
        };
        if expected_locator.is_some_and(|expected| expected != &retained_locator) {
            return Err(StreamedResultRegistryError::RetainedLocatorMismatch);
        }
        let lease_expires_at_unix_ms = package.lease().expires_at_unix_ms();
        let mut page_registrations = Vec::with_capacity(manifest.pages.len());
        let mut resource_handles = BTreeSet::new();
        for page in &manifest.pages {
            let page_ordinal = u32::try_from(page.page_ordinal)
                .map_err(|_| StreamedResultRegistryError::RangeOverflow)?;
            let resource_id = identity(
                b"codefabric.streamed-result-page-resource.v2",
                &[
                    package_id.as_bytes(),
                    &page.page_ordinal.to_be_bytes(),
                    page.content_checksum.as_bytes(),
                ],
            );
            let public_handle = mint_result_handle(
                b"codefabric.public-result-page-handle.v2",
                daemon_generation,
                &package_id,
                &resource_id,
            )?;
            if !resource_handles.insert(public_handle.clone()) {
                return Err(StreamedResultRegistryError::ResourceIdentityCollision);
            }
            page_registrations.push(StreamedResultResourceRegistration {
                public_handle,
                page_ordinal: Some(page_ordinal),
                media_type: super::arrow_result_resource::ARROW_STREAM_MEDIA_TYPE.to_owned(),
                byte_length: page.byte_length,
                content_checksum: format!("b3:{}", page.content_checksum),
                expires_at_unix_ms: lease_expires_at_unix_ms,
            });
        }
        if page_registrations.len() != manifest.pages.len() {
            return Err(StreamedResultRegistryError::ResourceIdentityCollision);
        }
        let public_manifest = PublicManifest {
            processing: &manifest.processing,
            format: "codefabric.public-streamed-result.v2",
            package_id: &package_id,
            epoch_id: &manifest.epoch_id,
            query_execution: &manifest.query_execution,
            canonical_semantic_response: &manifest.canonical_semantic_response,
            total_rows: manifest.total_rows,
            total_pages: manifest.total_pages,
            total_bytes: manifest.total_bytes,
            relations: manifest
                .relations
                .iter()
                .map(|relation| PublicRelation {
                    relation_id: &relation.relation_id,
                    page_start: relation.page_start,
                    page_count: relation.page_count,
                    row_count: relation.row_count,
                    coverage_state: &relation.coverage_state,
                    requested_units: relation.requested_units,
                    completed_units: relation.completed_units,
                    remainder_units: relation.remainder_units,
                    unknown_cause: relation.unknown_cause.as_deref(),
                    provenance: &relation.provenance,
                })
                .collect(),
            pages: manifest
                .pages
                .iter()
                .zip(&page_registrations)
                .map(|(page, resource)| PublicPage {
                    relation_id: &page.relation_id,
                    page_ordinal: page.page_ordinal,
                    row_count: page.row_count,
                    batch_count: page.batch_count,
                    public_handle: &resource.public_handle,
                    media_type: &resource.media_type,
                    byte_length: page.byte_length,
                    schema_checksum: &page.schema_checksum,
                    content_checksum: &resource.content_checksum,
                    expires_at_unix_ms: resource.expires_at_unix_ms,
                })
                .collect(),
        };
        let manifest_bytes = serde_json_canonicalizer::to_vec(&public_manifest)
            .map_err(StreamedResultRegistryError::Manifest)?;
        let manifest_handle = mint_result_handle(
            b"codefabric.public-result-manifest-handle.v2",
            daemon_generation,
            &package_id,
            &manifest_resource_id,
        )?;
        if !resource_handles.insert(manifest_handle.clone()) {
            return Err(StreamedResultRegistryError::ResourceIdentityCollision);
        }
        let manifest_registration = StreamedResultResourceRegistration {
            public_handle: manifest_handle,
            page_ordinal: None,
            media_type: "application/vnd.codefabric.result-manifest+json".to_owned(),
            byte_length: u64::try_from(manifest_bytes.len())
                .map_err(|_| StreamedResultRegistryError::RangeOverflow)?,
            content_checksum: checksum(&manifest_bytes),
            expires_at_unix_ms: lease_expires_at_unix_ms,
        };
        let registration = StreamedResultRegistration {
            processing: manifest.processing.clone(),
            package_id: package_id.clone(),
            manifest_resource_id: manifest_resource_id.clone(),
            manifest: manifest_registration.clone(),
            pages: page_registrations.clone(),
            total_rows: manifest.total_rows,
            total_pages: manifest.total_pages,
            total_bytes: manifest.total_bytes,
            retained_locator: retained_locator.clone(),
        };
        let package_entry = ResultPackageEntry {
            query_id: query_id.to_owned(),
            manifest_resource_id,
            manifest_bytes: metadata_charge
                .split(ResourceAmounts {
                    memory_bytes: manifest_bytes.capacity() as u64,
                    ..ResourceAmounts::default()
                })?
                .into_charged_vec(manifest_bytes)?,
            _metadata_charge: metadata_charge,
            package_id: package_id.clone(),
            resource_handles: resource_handles.clone(),
            total_rows: manifest.total_rows,
            total_pages: manifest.total_pages,
            total_bytes: manifest.total_bytes,
            retained_locator: retained_locator.clone(),
            package: Some(package),
            cleanup: Some(cleanup),
        };
        let mut resources = Vec::with_capacity(page_registrations.len() + 1);
        resources.push((
            manifest_registration.public_handle.clone(),
            ResultResourceEntry {
                principal_id,
                workspace_id,
                daemon_generation,
                policy_generation,
                revocation_generation,
                package_id: package_id.clone(),
                selector: StreamedResourceSelector::Manifest,
                media_type: manifest_registration.media_type.clone(),
                byte_length: manifest_registration.byte_length,
                content_checksum: manifest_registration.content_checksum.clone(),
                expires_at_unix_ms: lease_expires_at_unix_ms,
                release_id: None,
                released: false,
            },
        ));
        resources.extend(page_registrations.iter().map(|resource| {
            (
                resource.public_handle.clone(),
                ResultResourceEntry {
                    principal_id,
                    workspace_id,
                    daemon_generation,
                    policy_generation,
                    revocation_generation,
                    package_id: package_id.clone(),
                    selector: StreamedResourceSelector::Page(
                        resource
                            .page_ordinal
                            .expect("page registration has ordinal"),
                    ),
                    media_type: resource.media_type.clone(),
                    byte_length: resource.byte_length,
                    content_checksum: resource.content_checksum.clone(),
                    expires_at_unix_ms: lease_expires_at_unix_ms,
                    release_id: None,
                    released: false,
                },
            )
        }));
        let mut state = self.results.lock().await;
        if state.packages.contains_key(&package_id)
            || resources
                .iter()
                .any(|(handle, _)| state.resources.contains_key(handle))
        {
            return Err(StreamedResultRegistryError::PackageIdentityCollision);
        }
        state.packages.insert(package_id, package_entry);
        state.resources.extend(resources);
        Ok(registration)
    }

    /// Read one bounded manifest or page range after exact owner, token, and lease checks.
    ///
    /// # Errors
    /// Rejects unauthorized/expired handles, invalid ranges, resource exhaustion, and corruption.
    #[allow(clippy::too_many_lines)] // Keep authorization, exact read, and response closure adjacent.
    pub async fn read(
        &self,
        request: StreamedResourceRead,
    ) -> Result<StreamedResourceChunk, StreamedResultRegistryError> {
        if request.maximum_bytes == 0 || request.maximum_bytes > self.maximum_chunk_bytes {
            return Err(StreamedResultRegistryError::InvalidChunkBound);
        }
        let _operation = self.budget.try_reserve(
            ResourceClass::Control,
            ResourceAmounts {
                running_jobs: 1,
                ..ResourceAmounts::default()
            },
        )?;
        let mut source_authority = None;
        let (source, total_length, expected_checksum) =
            if let StreamedResourceSelector::Reference(selector) = &request.selector {
                let references = self.references.lock().await;
                let entry = references
                    .get(&request.public_handle)
                    .ok_or(StreamedResultRegistryError::UnknownPackage)?;
                authorize_reference(entry, &request)?;
                if entry.selector != *selector {
                    return Err(StreamedResultRegistryError::UnknownResource);
                }
                (
                    ResultReadSource::Manifest(entry.content.clone()),
                    entry.content.len() as u64,
                    entry.content_checksum.clone(),
                )
            } else {
                let (source, expected_length, expected_checksum) = {
                    let state = self.results.lock().await;
                    let resource = state
                        .resources
                        .get(&request.public_handle)
                        .ok_or(StreamedResultRegistryError::UnknownPackage)?;
                    authorize_result_resource(resource, &request)?;
                    if resource.selector != request.selector {
                        return Err(StreamedResultRegistryError::UnknownResource);
                    }
                    let package = state
                        .packages
                        .get(&resource.package_id)
                        .ok_or(StreamedResultRegistryError::UnknownPackage)?;
                    if package
                        .package
                        .as_ref()
                        .ok_or(StreamedResultRegistryError::Released)?
                        .manifest()
                        .relations
                        .iter()
                        .any(|relation| relation.relation_id == "query.result.source-context")
                    {
                        let authority = super::source_disclosure::SourceDisclosureAuthority::new(
                            self.source_disclosure_reader
                                .get()
                                .ok_or(StreamedResultRegistryError::SourceAccessDenied)?
                                .clone(),
                            *resource.workspace_id.as_bytes(),
                        );
                        authority
                            .authorize()
                            .map_err(|_| StreamedResultRegistryError::SourceAccessDenied)?;
                        source_authority = Some(authority);
                    }
                    let source = match resource.selector {
                        StreamedResourceSelector::Manifest => {
                            ResultReadSource::Manifest(package.manifest_bytes.clone())
                        }
                        StreamedResourceSelector::Page(page_ordinal) => ResultReadSource::Page(
                            package
                                .package
                                .as_ref()
                                .ok_or(StreamedResultRegistryError::Released)?
                                .clone(),
                            page_ordinal,
                        ),
                        StreamedResourceSelector::Reference(_) => unreachable!("handled above"),
                    };
                    (
                        source,
                        resource.byte_length,
                        resource.content_checksum.clone(),
                    )
                };
                (source, expected_length, expected_checksum)
            };
        if request.offset > total_length {
            return Err(StreamedResultRegistryError::RangeOutsideResource);
        }
        let start = usize::try_from(request.offset)
            .map_err(|_| StreamedResultRegistryError::RangeOverflow)?;
        let length = usize::try_from(total_length)
            .map_err(|_| StreamedResultRegistryError::RangeOverflow)?;
        let end = start.saturating_add(request.maximum_bytes).min(length);
        let bytes = match source {
            ResultReadSource::Manifest(bytes) => {
                if bytes.len() != length || checksum(&bytes) != expected_checksum {
                    return Err(StreamedResultRegistryError::ResourceIntegrity);
                }
                ChargedSlice::try_from_fn(
                    &self.budget,
                    ResourceClass::Control,
                    end - start,
                    || bytes[start..end].to_vec(),
                )?
            }
            ResultReadSource::Page(package, page_ordinal) => {
                let bytes = package
                    .read_page_range(
                        u64::from(page_ordinal),
                        request.offset,
                        request.maximum_bytes,
                        request.observed_at_unix_ms,
                    )
                    .await?;
                if bytes.len() != end - start {
                    return Err(StreamedResultRegistryError::ResourceIntegrity);
                }
                bytes
            }
        };
        let next_offset =
            u64::try_from(end).map_err(|_| StreamedResultRegistryError::RangeOverflow)?;
        // Recheck after asynchronous object I/O, before disclosing any bytes.
        if let Some(authority) = source_authority {
            authority
                .authorize()
                .map_err(|_| StreamedResultRegistryError::SourceAccessDenied)?;
        }
        Ok(StreamedResourceChunk {
            public_handle: request.public_handle,
            offset: request.offset,
            next_offset,
            total_length,
            content_checksum: expected_checksum,
            bytes,
            complete: end == length,
        })
    }

    /// Resolve the live public registration caused by one accepted query and principal.
    ///
    /// # Errors
    /// Rejects absent or unauthorized registrations and invalid resource identities.
    #[allow(clippy::too_many_arguments)]
    pub async fn registration_for_query(
        &self,
        query_id: &str,
        principal_id: PrincipalId,
        workspace_id: super::command::WorkspaceId,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        observed_at_unix_ms: i64,
    ) -> Result<StreamedResultRegistration, StreamedResultRegistryError> {
        let state = self.results.lock().await;
        let package = state
            .packages
            .values()
            .find(|package| {
                package.query_id == query_id
                    && package.resource_handles.iter().any(|handle| {
                        state
                            .resources
                            .get(handle)
                            .is_some_and(|resource| resource.principal_id == principal_id)
                    })
            })
            .ok_or(StreamedResultRegistryError::UnknownPackage)?;
        if package.package.is_none() {
            return Err(StreamedResultRegistryError::Released);
        }
        let mut manifest = None;
        let mut pages = Vec::new();
        for handle in &package.resource_handles {
            let resource = state
                .resources
                .get(handle)
                .ok_or(StreamedResultRegistryError::ResourceIdentityCollision)?;
            authorize_result_values(
                resource,
                principal_id,
                &BTreeSet::from([workspace_id]),
                daemon_generation,
                policy_generation,
                revocation_generation,
                observed_at_unix_ms,
            )?;
            if resource.released {
                return Err(StreamedResultRegistryError::Released);
            }
            let registration = result_resource_registration(handle, resource);
            match resource.selector {
                StreamedResourceSelector::Manifest => manifest = Some(registration),
                StreamedResourceSelector::Page(_) => pages.push(registration),
                StreamedResourceSelector::Reference(_) => {
                    return Err(StreamedResultRegistryError::ResourceIdentityCollision);
                }
            }
        }
        pages.sort_by_key(|resource| resource.page_ordinal);
        Ok(StreamedResultRegistration {
            processing: package
                .package
                .as_ref()
                .ok_or(StreamedResultRegistryError::Released)?
                .manifest()
                .processing
                .clone(),
            package_id: package.package_id.clone(),
            manifest_resource_id: package.manifest_resource_id.clone(),
            manifest: manifest.ok_or(StreamedResultRegistryError::ResourceIdentityCollision)?,
            pages,
            total_rows: package.total_rows,
            total_pages: package.total_pages,
            total_bytes: package.total_bytes,
            retained_locator: package.retained_locator.clone(),
        })
    }

    /// Compatibility-probe seam for independently generated clients over the real public
    /// service. The wrapper deliberately accepts only an already sealed package and always keeps
    /// exact package cleanup ownership; it is absent from non-probe feature graphs.
    #[cfg(feature = "compatibility-probes")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn publish_sealed_package_for_interop(
        &self,
        query_id: &str,
        principal_id: PrincipalId,
        workspace_id: super::command::WorkspaceId,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        package: SealedStreamedResultPackage,
    ) -> Result<StreamedResultRegistration, StreamedResultRegistryError> {
        self.publish_package(
            query_id,
            principal_id,
            workspace_id,
            daemon_generation,
            policy_generation,
            revocation_generation,
            package.clone(),
            ResultCleanup::Package(package),
            None,
        )
        .await
    }

    /// Remove a just-published package whose durable `ResultReady` event could not be committed.
    /// Failed object deletion remains owned by the normal retry tombstone rather than orphaning
    /// sealed pages outside both the registry and coordinator journal.
    pub(crate) async fn discard_unjournaled_query(
        &self,
        query_id: &str,
    ) -> Result<bool, StreamedResultRegistryError> {
        let removed = {
            let mut state = self.results.lock().await;
            let package_id = state.packages.iter().find_map(|(package_id, package)| {
                (package.query_id == query_id).then(|| package_id.clone())
            });
            let Some(package_id) = package_id else {
                return Ok(false);
            };
            let mut package = state
                .packages
                .remove(&package_id)
                .ok_or(StreamedResultRegistryError::UnknownPackage)?;
            for handle in &package.resource_handles {
                state.resources.remove(handle);
            }
            let cleanup = package
                .cleanup
                .take()
                .map(ResultCleanup::into_package)
                .or_else(|| package.package.take())
                .ok_or(StreamedResultRegistryError::Released)?;
            (package_id, cleanup)
        };
        let (package_id, package) = removed;
        if let Err(error) = package.clone().delete_objects().await {
            self.results
                .lock()
                .await
                .cleanup
                .insert(package_id, package);
            return Err(error.into());
        }
        Ok(true)
    }

    /// Release one handle and report the owning query only after every sibling is released.
    /// Object deletion is a separate phase and may begin only after the coordinator durably
    /// transitions the query to `cleanup_pending`.
    ///
    /// # Errors
    /// Rejects authorization mismatches, invalid release identities, and unavailable resources.
    #[allow(clippy::too_many_arguments)]
    pub async fn release(
        &self,
        principal_id: PrincipalId,
        workspace_ids: &BTreeSet<super::command::WorkspaceId>,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        public_handle: &str,
        release_id: &str,
        observed_at_unix_ms: i64,
    ) -> Result<(StreamedReleaseOutcome, Option<String>, bool), StreamedResultRegistryError> {
        if release_id.is_empty() || release_id.len() > 256 || !release_id.is_ascii() {
            return Err(StreamedResultRegistryError::InvalidReleaseId);
        }
        let (outcome, query_id, idempotent_replay) = {
            let mut state = self.results.lock().await;
            let Some(resource) = state.resources.get(public_handle) else {
                drop(state);
                return self
                    .release_reference(
                        principal_id,
                        workspace_ids,
                        daemon_generation,
                        policy_generation,
                        revocation_generation,
                        public_handle,
                        release_id,
                        observed_at_unix_ms,
                    )
                    .await;
            };
            authorize_result_values(
                resource,
                principal_id,
                workspace_ids,
                daemon_generation,
                policy_generation,
                revocation_generation,
                observed_at_unix_ms,
            )?;
            let package_id = resource.package_id.clone();
            let resource_is_live = state
                .packages
                .get(&package_id)
                .is_some_and(|package| package.resource_handles.contains(public_handle));
            if !resource_is_live && !resource.released {
                return Err(StreamedResultRegistryError::UnknownPackage);
            }
            let (outcome, idempotent_replay) = {
                let resource = state
                    .resources
                    .get_mut(public_handle)
                    .ok_or(StreamedResultRegistryError::UnknownPackage)?;
                record_release(&mut resource.release_id, &mut resource.released, release_id)
            };
            if !resource_is_live {
                return Ok((outcome, None, idempotent_replay));
            }
            let all_released = state
                .packages
                .get(&package_id)
                .ok_or(StreamedResultRegistryError::UnknownPackage)?
                .resource_handles
                .iter()
                .all(|handle| {
                    state
                        .resources
                        .get(handle)
                        .is_some_and(|resource| resource.released)
                });
            let query_id = all_released.then(|| {
                state
                    .packages
                    .get(&package_id)
                    .map(|package| package.query_id.clone())
                    .ok_or(StreamedResultRegistryError::UnknownPackage)
            });
            (outcome, query_id.transpose()?, idempotent_replay)
        };
        Ok((outcome, query_id, idempotent_replay))
    }

    /// Delete the process-local package object set only after durable reissue revocation wins.
    /// A failed delete remains retry-owned under the exact package identity. Successful cleanup
    /// retains only authorization-bearing release tombstones until their lease expiry.
    ///
    /// # Errors
    /// Rejects cleanup before revocation and propagates retryable object deletion failures.
    pub async fn finalize_released_query(
        &self,
        query_id: &str,
    ) -> Result<bool, StreamedResultRegistryError> {
        let cleanup = {
            let mut state = self.results.lock().await;
            let package_id = state
                .packages
                .iter()
                .find_map(|(package_id, package)| {
                    (package.query_id == query_id).then(|| package_id.clone())
                })
                .ok_or(StreamedResultRegistryError::UnknownPackage)?;
            let all_released = state
                .packages
                .get(&package_id)
                .ok_or(StreamedResultRegistryError::UnknownPackage)?
                .resource_handles
                .iter()
                .all(|handle| {
                    state
                        .resources
                        .get(handle)
                        .is_some_and(|resource| resource.released)
                });
            if !all_released {
                return Err(StreamedResultRegistryError::CleanupNotReady);
            }
            let retry = state.cleanup.remove(&package_id);
            let package = state
                .packages
                .get_mut(&package_id)
                .ok_or(StreamedResultRegistryError::UnknownPackage)?;
            package.package.take();
            retry
                .or_else(|| package.cleanup.take().map(ResultCleanup::into_package))
                .map(|package| (package_id, package))
        };
        let Some((package_id, package)) = cleanup else {
            return Ok(false);
        };
        if let Err(error) = package.clone().delete_objects().await {
            self.results
                .lock()
                .await
                .cleanup
                .insert(package_id, package);
            return Err(error.into());
        }
        let mut state = self.results.lock().await;
        state.packages.remove(&package_id);
        state.cleanup.remove(&package_id);
        Ok(true)
    }

    /// Finish a durable cleanup after restart using only the private exact object set retained in
    /// the coordinator journal. Deletes are idempotent and preserve manifest-last ordering.
    ///
    /// # Errors
    /// Rejects malformed locators, unavailable recovery, or failed object deletion.
    pub async fn cleanup_pending_object_set(
        &self,
        object_set: &PendingResultObjectSet,
    ) -> Result<(), StreamedResultRegistryError> {
        let builder = self
            .package_builder
            .get()
            .ok_or(StreamedResultRegistryError::RecoveryUnavailable)?;
        let manifest = validated_private_object_path(&object_set.manifest_object_path, false)?;
        let pages = object_set
            .page_object_paths
            .iter()
            .map(|path| validated_private_object_path(path, true))
            .collect::<Result<Vec<_>, _>>()?;
        let package_root = object_set
            .manifest_object_path
            .strip_suffix("/manifest.json")
            .ok_or(StreamedResultRegistryError::RetainedLocatorMismatch)?;
        let expected_package_root = format!(
            "packages/{}/{}",
            object_set.epoch_id, object_set.query_execution
        );
        if package_root != expected_package_root
            || pages.is_empty()
            || pages.len() > 1_024
            || pages.iter().collect::<BTreeSet<_>>().len() != pages.len()
            || object_set
                .page_object_paths
                .iter()
                .any(|path| !path.starts_with(&format!("{package_root}/pages/")))
        {
            return Err(StreamedResultRegistryError::RetainedLocatorMismatch);
        }
        builder
            .delete_retained_object_set(&pages, &manifest)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn release_reference(
        &self,
        principal_id: PrincipalId,
        workspace_ids: &BTreeSet<super::command::WorkspaceId>,
        daemon_generation: u64,
        policy_generation: u64,
        revocation_generation: u64,
        public_handle: &str,
        release_id: &str,
        observed_at_unix_ms: i64,
    ) -> Result<(StreamedReleaseOutcome, Option<String>, bool), StreamedResultRegistryError> {
        let mut references = self.references.lock().await;
        let entry = references
            .get_mut(public_handle)
            .ok_or(StreamedResultRegistryError::UnknownPackage)?;
        authorize_reference_values(
            entry,
            principal_id,
            workspace_ids,
            daemon_generation,
            policy_generation,
            revocation_generation,
            observed_at_unix_ms,
        )?;
        let (outcome, idempotent_replay) =
            record_release(&mut entry.release_id, &mut entry.released, release_id);
        entry.content =
            ChargedSlice::try_from_fn(&self.budget, ResourceClass::Control, 0, Vec::new)?;
        Ok((outcome, None, idempotent_replay))
    }

    /// Expire public handles without deleting package objects. Durable coordinator revocation is
    /// the sole authority that may move a retained package into exact object cleanup.
    ///
    /// # Errors
    /// Propagates invalid expiry bookkeeping or retryable cleanup failures.
    pub async fn collect_expired(
        &self,
        observed_at_unix_ms: i64,
    ) -> Result<usize, StreamedResultRegistryError> {
        let expired_count = {
            let mut state = self.results.lock().await;
            let live_handles = state
                .packages
                .values()
                .flat_map(|package| package.resource_handles.iter().cloned())
                .collect::<BTreeSet<_>>();
            let mut expired = 0_usize;
            state.resources.retain(|handle, resource| {
                if resource.expires_at_unix_ms > observed_at_unix_ms {
                    return true;
                }
                if !live_handles.contains(handle) {
                    expired = expired.saturating_add(1);
                    return false;
                }
                if !resource.released {
                    resource.released = true;
                    expired = expired.saturating_add(1);
                }
                true
            });
            expired
        };
        let expired_references = {
            let mut references = self.references.lock().await;
            let before = references.len();
            references.retain(|_, entry| entry.expires_at_unix_ms > observed_at_unix_ms);
            before.saturating_sub(references.len())
        };
        Ok(expired_count.saturating_add(expired_references))
    }
}

fn record_release(
    stable_release_id: &mut Option<String>,
    released: &mut bool,
    release_id: &str,
) -> (StreamedReleaseOutcome, bool) {
    if let Some(stable_release_id) = stable_release_id.as_deref() {
        *released = true;
        return (
            StreamedReleaseOutcome::AlreadyReleased,
            stable_release_id == release_id,
        );
    }
    *stable_release_id = Some(release_id.to_owned());
    if std::mem::replace(released, true) {
        (StreamedReleaseOutcome::AlreadyReleased, false)
    } else {
        (StreamedReleaseOutcome::Released, false)
    }
}

fn authorize_reference(
    entry: &ReferenceEntry,
    request: &StreamedResourceRead,
) -> Result<(), StreamedResultRegistryError> {
    authorize_reference_values(
        entry,
        request.principal_id,
        &request.workspace_ids,
        request.daemon_generation,
        request.policy_generation,
        request.revocation_generation,
        request.observed_at_unix_ms,
    )?;
    if entry.released {
        return Err(StreamedResultRegistryError::Released);
    }
    Ok(())
}

fn authorize_reference_values(
    entry: &ReferenceEntry,
    principal_id: PrincipalId,
    workspace_ids: &BTreeSet<super::command::WorkspaceId>,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    observed_at_unix_ms: i64,
) -> Result<(), StreamedResultRegistryError> {
    if entry.principal_id != principal_id {
        return Err(StreamedResultRegistryError::WrongOwner);
    }
    if !workspace_ids.contains(&entry.workspace_id) {
        return Err(StreamedResultRegistryError::WrongWorkspace);
    }
    if entry.daemon_generation != daemon_generation {
        return Err(StreamedResultRegistryError::GenerationMismatch);
    }
    if entry.policy_generation != policy_generation
        || entry.revocation_generation != revocation_generation
    {
        return Err(StreamedResultRegistryError::PolicyGenerationMismatch);
    }
    if observed_at_unix_ms >= entry.expires_at_unix_ms {
        return Err(StreamedResultRegistryError::Expired);
    }
    Ok(())
}

fn authorize_result_resource(
    entry: &ResultResourceEntry,
    request: &StreamedResourceRead,
) -> Result<(), StreamedResultRegistryError> {
    authorize_result_values(
        entry,
        request.principal_id,
        &request.workspace_ids,
        request.daemon_generation,
        request.policy_generation,
        request.revocation_generation,
        request.observed_at_unix_ms,
    )?;
    if entry.released {
        return Err(StreamedResultRegistryError::Released);
    }
    Ok(())
}

fn authorize_result_values(
    entry: &ResultResourceEntry,
    principal_id: PrincipalId,
    workspace_ids: &BTreeSet<super::command::WorkspaceId>,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    observed_at_unix_ms: i64,
) -> Result<(), StreamedResultRegistryError> {
    if entry.principal_id != principal_id {
        return Err(StreamedResultRegistryError::WrongOwner);
    }
    if !workspace_ids.contains(&entry.workspace_id) {
        return Err(StreamedResultRegistryError::WrongWorkspace);
    }
    if entry.daemon_generation != daemon_generation {
        return Err(StreamedResultRegistryError::GenerationMismatch);
    }
    if entry.policy_generation != policy_generation
        || entry.revocation_generation != revocation_generation
    {
        return Err(StreamedResultRegistryError::PolicyGenerationMismatch);
    }
    if observed_at_unix_ms >= entry.expires_at_unix_ms {
        return Err(StreamedResultRegistryError::Expired);
    }
    Ok(())
}

#[derive(Serialize)]
struct PublicManifest<'a> {
    processing: &'a [super::processing_status::QueryProcessing],
    format: &'static str,
    package_id: &'a str,
    epoch_id: &'a str,
    query_execution: &'a str,
    canonical_semantic_response: &'a serde_json::Value,
    total_rows: u64,
    total_pages: u64,
    total_bytes: u64,
    relations: Vec<PublicRelation<'a>>,
    pages: Vec<PublicPage<'a>>,
}

#[derive(Serialize)]
struct PublicRelation<'a> {
    relation_id: &'a str,
    page_start: u64,
    page_count: u64,
    row_count: u64,
    coverage_state: &'a str,
    requested_units: u64,
    completed_units: u64,
    remainder_units: u64,
    unknown_cause: Option<&'a str>,
    provenance: &'a [ResultProvenance],
}

#[derive(Serialize)]
struct PublicPage<'a> {
    relation_id: &'a str,
    page_ordinal: u64,
    row_count: u64,
    batch_count: u64,
    public_handle: &'a str,
    media_type: &'a str,
    byte_length: u64,
    schema_checksum: &'a str,
    content_checksum: &'a str,
    expires_at_unix_ms: i64,
}

fn result_resource_registration(
    public_handle: &str,
    resource: &ResultResourceEntry,
) -> StreamedResultResourceRegistration {
    StreamedResultResourceRegistration {
        public_handle: public_handle.to_owned(),
        page_ordinal: match resource.selector {
            StreamedResourceSelector::Page(page_ordinal) => Some(page_ordinal),
            StreamedResourceSelector::Manifest | StreamedResourceSelector::Reference(_) => None,
        },
        media_type: resource.media_type.clone(),
        byte_length: resource.byte_length,
        content_checksum: resource.content_checksum.clone(),
        expires_at_unix_ms: resource.expires_at_unix_ms,
    }
}

fn mint_result_handle(
    domain: &[u8],
    daemon_generation: u64,
    package_id: &str,
    resource_id: &str,
) -> Result<String, StreamedResultRegistryError> {
    let nonce = crate::identity::random_registration_nonce()
        .map_err(|_| StreamedResultRegistryError::Entropy)?;
    Ok(format!(
        "public:result:{}",
        identity(
            domain,
            &[
                &daemon_generation.to_be_bytes(),
                package_id.as_bytes(),
                resource_id.as_bytes(),
                &nonce,
            ],
        )
        .trim_start_matches("b3:")
    ))
}

fn identity(domain: &[u8], fields: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, domain);
    for field in fields {
        frame(&mut hasher, field);
    }
    format!("b3:{}", hex(hasher.finalize().as_bytes()))
}

fn checksum(bytes: &[u8]) -> String {
    format!("b3:{}", hex(blake3::hash(bytes).as_bytes()))
}

fn frame(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex_exact<const N: usize>(value: &str) -> Result<[u8; N], StreamedResultRegistryError> {
    if value.len() != N.saturating_mul(2) {
        return Err(StreamedResultRegistryError::RetainedLocatorMismatch);
    }
    let mut output = [0_u8; N];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        output[index] = (decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?;
    }
    Ok(output)
}

fn validated_private_object_path(
    value: &str,
    page: bool,
) -> Result<ObjectPath, StreamedResultRegistryError> {
    let suffix = if page { ".arrow" } else { "/manifest.json" };
    if value.is_empty()
        || value.len() > 1_024
        || !value.is_ascii()
        || !value.starts_with("packages/")
        || !value.ends_with(suffix)
        || page && !value.contains("/pages/")
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "..")
    {
        return Err(StreamedResultRegistryError::RetainedLocatorMismatch);
    }
    Ok(ObjectPath::from(value))
}

const fn decode_nibble(byte: u8) -> Result<u8, StreamedResultRegistryError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(StreamedResultRegistryError::RetainedLocatorMismatch),
    }
}

#[derive(Debug, Error)]
pub enum StreamedResultRegistryError {
    #[error("source disclosure is not authorized")]
    SourceAccessDenied,
    #[error("result resource budget is not owned by the selected workspace lineage")]
    BudgetOwnerMismatch,
    #[error(transparent)]
    Resource(#[from] crate::resource_budget::ResourceBudgetError),
    #[error("invalid resource chunk bound")]
    InvalidChunkBound,
    #[error("invalid streamed result identity")]
    InvalidIdentity,
    #[error("streamed resource identity entropy failed")]
    Entropy,
    #[error("reference resource registry reached capacity")]
    ReferenceCapacity,
    #[error("streamed package identity collision")]
    PackageIdentityCollision,
    #[error("streamed resource identity collision")]
    ResourceIdentityCollision,
    #[error("unknown streamed package")]
    UnknownPackage,
    #[error("unknown streamed resource")]
    UnknownResource,
    #[error("streamed resource belongs to another principal")]
    WrongOwner,
    #[error("streamed resource belongs to another workspace")]
    WrongWorkspace,
    #[error("streamed resource belongs to another daemon generation")]
    GenerationMismatch,
    #[error("streamed resource belongs to another policy generation")]
    PolicyGenerationMismatch,
    #[error("invalid resource release identity")]
    InvalidReleaseId,
    #[error("streamed resource was released")]
    Released,
    #[error("streamed resource lease expired")]
    Expired,
    #[error("streamed resource range is outside the resource")]
    RangeOutsideResource,
    #[error("streamed resource range overflow")]
    RangeOverflow,
    #[error("streamed resource bytes differ from their sealed descriptor")]
    ResourceIntegrity,
    #[error("retained package recovery builder is unavailable")]
    RecoveryUnavailable,
    #[error("retained package locator differs from the sealed package")]
    RetainedLocatorMismatch,
    #[error("streamed package cleanup is not ready")]
    CleanupNotReady,
    #[error("public manifest encoding failed: {0}")]
    Manifest(serde_json::Error),
    #[error(transparent)]
    Package(#[from] StreamedResultPackageError),
}

#[cfg(test)]
#[allow(clippy::too_many_lines)] // End-to-end authorization/lifecycle matrices stay in one fixture.
mod tests {
    fn test_resource_budget() -> ResourceBudget {
        thread_local! { static FIXTURE_OWNER: ResourceBudget = {
            let root = super::super::streamed_result_package::test_resource_budget();
            root.workspace(*WORKSPACE.as_bytes(), root.policy()).unwrap()
        }; }
        FIXTURE_OWNER.with(Clone::clone)
    }
    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use arrow_array::{ArrayRef, Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
    use futures::{TryStreamExt as _, stream};
    use object_store::{ObjectStore as _, memory::InMemory};

    use super::*;
    use crate::cancellation::Cancellation;
    use crate::fabric::arrow_result_resource::{
        QueryExecutionPin, ResultCoverage, ResultResourceLease,
    };
    use crate::fabric::command::{EpochId, LeaseId, WorkspaceId};
    use crate::fabric::streamed_result_package::{
        ObjectStoreResultSink, ResultObjectSink, StreamedRelationInput,
        StreamedResultPackageBuilder, StreamedResultPackageLimits,
    };
    use crate::relational_program::RelationId;

    const PRINCIPAL: PrincipalId = PrincipalId::from_bytes([0x11; 16]);
    const WORKSPACE: WorkspaceId = WorkspaceId::from_bytes([0x22; 16]);

    #[derive(Debug)]
    struct AcceptPublicationIntent;

    #[async_trait::async_trait]
    impl super::super::streamed_result_package::ResultPublicationIntentRecorder
        for AcceptPublicationIntent
    {
        async fn record_publication_intent(
            &self,
            _intent: PendingResultObjectSet,
        ) -> Result<(), super::super::streamed_result_package::ResultPublicationIntentError>
        {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FaultSink {
        store: Arc<InMemory>,
        fail_delete_once: AtomicBool,
    }

    impl FaultSink {
        fn new() -> Self {
            Self {
                store: Arc::new(InMemory::new()),
                fail_delete_once: AtomicBool::new(false),
            }
        }

        async fn object_count(&self) -> usize {
            self.store
                .list(None)
                .try_collect::<Vec<_>>()
                .await
                .expect("in-memory object list")
                .len()
        }
    }

    #[async_trait::async_trait]
    impl ResultObjectSink for FaultSink {
        async fn create(
            &self,
            path: &object_store::path::Path,
            bytes: Vec<u8>,
        ) -> Result<(), object_store::Error> {
            ObjectStoreResultSink::new(Arc::clone(&self.store) as Arc<dyn object_store::ObjectStore>)
                .create(path, bytes)
                .await
        }

        async fn size(&self, path: &object_store::path::Path) -> Result<u64, object_store::Error> {
            ObjectStoreResultSink::new(Arc::clone(&self.store) as Arc<dyn object_store::ObjectStore>).size(path).await
        }

        async fn read_range(
            &self,
            path: &object_store::path::Path,
            range: std::ops::Range<u64>,
        ) -> Result<Vec<u8>, object_store::Error> {
            ObjectStoreResultSink::new(Arc::clone(&self.store) as Arc<dyn object_store::ObjectStore>)
                .read_range(path, range)
                .await
        }

        async fn delete(&self, path: &object_store::path::Path) -> Result<(), object_store::Error> {
            if self.fail_delete_once.swap(false, Ordering::SeqCst) {
                return Err(object_store::Error::Generic {
                    store: "wp45-fault-sink",
                    source: Box::new(io::Error::other("injected cleanup failure")),
                });
            }
            ObjectStoreResultSink::new(Arc::clone(&self.store) as Arc<dyn object_store::ObjectStore>)
                .delete(path)
                .await
        }
    }

    fn test_package_builder(sink: Arc<dyn ResultObjectSink>) -> StreamedResultPackageBuilder {
        let limits = StreamedResultPackageLimits::try_new(
            1,
            4,
            1,
            64 * 1024,
            4,
            256 * 1024,
            64 * 1024,
            4,
            256,
        )
        .unwrap();
        StreamedResultPackageBuilder::new(sink, limits, test_resource_budget())
    }

    async fn result_package(sink: Arc<dyn ResultObjectSink>) -> SealedStreamedResultPackage {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![1_i64, 2, 3])) as ArrayRef],
        )
        .unwrap();
        let stream = Box::pin(RecordBatchStreamAdapter::new(
            Arc::clone(&schema),
            stream::iter([Ok(batch)]),
        ));
        test_package_builder(sink)
            .seal(
                EpochId::from_bytes([0x31; 16]),
                QueryExecutionPin::from_bytes([0x32; 32]),
                br#"{"request":"resource-scoped"}"#,
                vec![StreamedRelationInput {
                    relation_id: RelationId::new("query.result.v1").unwrap(),
                    schema,
                    stream,
                    max_rows: 4,
                    row_selection: None,
                    coverage: ResultCoverage::complete(3),
                    provenance: vec![ResultProvenance {
                        kind: "transformation_release".to_owned(),
                        identity: "release:wp45".to_owned(),
                    }],
                }],
                ResultResourceLease::try_new(LeaseId::from_bytes([0x33; 16]), 10, 10_000).unwrap(),
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await
            .unwrap()
    }

    fn result_read(
        public_handle: &str,
        selector: StreamedResourceSelector,
        offset: u64,
        maximum_bytes: usize,
    ) -> StreamedResourceRead {
        StreamedResourceRead {
            principal_id: PRINCIPAL,
            workspace_ids: Arc::new(BTreeSet::from([WORKSPACE])),
            daemon_generation: 7,
            policy_generation: 8,
            revocation_generation: 9,
            public_handle: public_handle.to_owned(),
            selector,
            offset,
            maximum_bytes,
            observed_at_unix_ms: 20,
        }
    }

    async fn publish_result_fixture(
        registry: &StreamedResultRegistry,
        sink: &Arc<FaultSink>,
        query_id: &str,
    ) -> StreamedResultRegistration {
        let package = result_package(Arc::clone(sink) as Arc<dyn ResultObjectSink>).await;
        registry
            .publish_package(
                query_id,
                PRINCIPAL,
                WORKSPACE,
                7,
                8,
                9,
                package.clone(),
                ResultCleanup::Package(package),
                None,
            )
            .await
            .unwrap()
    }

    fn reference_publication(index: usize, issued_at: i64) -> ReferenceResourcePublication {
        ReferenceResourcePublication {
            principal_id: PRINCIPAL,
            workspace_id: WORKSPACE,
            daemon_generation: 7,
            policy_generation: 8,
            revocation_generation: 9,
            selector: StreamedReferenceSelector {
                kind: "GUIDE".to_owned(),
                version: Some("2.3".to_owned()),
            },
            reference_id: format!("reference:{index}"),
            media_type: "application/json".to_owned(),
            content: format!(r#"{{"reference":{index}}}"#).into_bytes(),
            issued_at_unix_ms: issued_at,
            expires_at_unix_ms: issued_at + 100,
        }
    }

    fn reference_read(public_handle: &str) -> StreamedResourceRead {
        StreamedResourceRead {
            principal_id: PRINCIPAL,
            workspace_ids: Arc::new(BTreeSet::from([WORKSPACE])),
            daemon_generation: 7,
            policy_generation: 8,
            revocation_generation: 9,
            public_handle: public_handle.to_owned(),
            selector: StreamedResourceSelector::Reference(StreamedReferenceSelector {
                kind: "GUIDE".to_owned(),
                version: Some("2.3".to_owned()),
            }),
            offset: 0,
            maximum_bytes: 4,
            observed_at_unix_ms: 10,
        }
    }

    #[tokio::test]
    async fn reference_handle_reauthorizes_every_binding_and_releases_without_query() {
        let registry = StreamedResultRegistry::try_new(16, test_resource_budget()).unwrap();
        let publication = reference_publication(1, 1);
        let expected_content = publication.content.clone();
        let registration = registry
            .publish_reference(publication)
            .await
            .expect("reference publication");
        assert!(registration.public_handle.starts_with("public:reference:"));
        // A random opaque digest may legitimately begin with '1'; a substring assertion would
        // fail one time in sixteen without indicating disclosure of the private reference id.
        assert_ne!(registration.public_handle, "public:reference:1");
        let chunk = registry
            .read(reference_read(&registration.public_handle))
            .await
            .expect("authorized bounded read");
        assert_eq!(&*chunk.bytes, &expected_content[..4]);
        assert_eq!(chunk.next_offset, 4);
        assert!(!chunk.complete);

        let mut wrong_owner = reference_read(&registration.public_handle);
        wrong_owner.principal_id = PrincipalId::from_bytes([0x12; 16]);
        assert!(matches!(
            registry.read(wrong_owner).await,
            Err(StreamedResultRegistryError::WrongOwner)
        ));
        let mut wrong_workspace = reference_read(&registration.public_handle);
        wrong_workspace.workspace_ids =
            Arc::new(BTreeSet::from([WorkspaceId::from_bytes([0x23; 16])]));
        assert!(matches!(
            registry.read(wrong_workspace).await,
            Err(StreamedResultRegistryError::WrongWorkspace)
        ));
        let mut wrong_daemon = reference_read(&registration.public_handle);
        wrong_daemon.daemon_generation += 1;
        assert!(matches!(
            registry.read(wrong_daemon).await,
            Err(StreamedResultRegistryError::GenerationMismatch)
        ));
        for mut wrong_policy in [
            reference_read(&registration.public_handle),
            reference_read(&registration.public_handle),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, mut request)| {
            if index == 0 {
                request.policy_generation += 1;
            } else {
                request.revocation_generation += 1;
            }
            request
        }) {
            assert!(matches!(
                registry.read(wrong_policy.clone()).await,
                Err(StreamedResultRegistryError::PolicyGenerationMismatch)
            ));
            wrong_policy.observed_at_unix_ms = 101;
        }
        let mut wrong_selector = reference_read(&registration.public_handle);
        wrong_selector.selector = StreamedResourceSelector::Reference(StreamedReferenceSelector {
            kind: "RECIPE".to_owned(),
            version: Some("2.3".to_owned()),
        });
        assert!(matches!(
            registry.read(wrong_selector).await,
            Err(StreamedResultRegistryError::UnknownResource)
        ));
        let mut expired = reference_read(&registration.public_handle);
        expired.observed_at_unix_ms = 101;
        assert!(matches!(
            registry.read(expired).await,
            Err(StreamedResultRegistryError::Expired)
        ));

        let workspaces = BTreeSet::from([WORKSPACE]);
        assert!(matches!(
            registry
                .release(
                    PrincipalId::from_bytes([0x12; 16]),
                    &workspaces,
                    7,
                    8,
                    9,
                    &registration.public_handle,
                    "release:wrong-owner",
                    20,
                )
                .await,
            Err(StreamedResultRegistryError::WrongOwner)
        ));
        assert!(matches!(
            registry
                .release(
                    PRINCIPAL,
                    &BTreeSet::from([WorkspaceId::from_bytes([0x23; 16])]),
                    7,
                    8,
                    9,
                    &registration.public_handle,
                    "release:wrong-workspace",
                    20,
                )
                .await,
            Err(StreamedResultRegistryError::WrongWorkspace)
        ));
        assert!(matches!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    8,
                    8,
                    9,
                    &registration.public_handle,
                    "release:restart",
                    20,
                )
                .await,
            Err(StreamedResultRegistryError::GenerationMismatch)
        ));
        for (policy, revocation, release_id) in [
            (10, 9, "release:wrong-policy"),
            (8, 10, "release:wrong-revocation"),
        ] {
            assert!(matches!(
                registry
                    .release(
                        PRINCIPAL,
                        &workspaces,
                        7,
                        policy,
                        revocation,
                        &registration.public_handle,
                        release_id,
                        20,
                    )
                    .await,
                Err(StreamedResultRegistryError::PolicyGenerationMismatch)
            ));
        }

        let released = registry
            .release(
                PRINCIPAL,
                &workspaces,
                7,
                8,
                9,
                &registration.public_handle,
                "release:1",
                20,
            )
            .await
            .expect("reference release");
        assert_eq!(released, (StreamedReleaseOutcome::Released, None, false));
        let replay = registry
            .release(
                PRINCIPAL,
                &BTreeSet::from([WORKSPACE]),
                7,
                8,
                9,
                &registration.public_handle,
                "release:1",
                21,
            )
            .await
            .expect("reference release replay");
        assert_eq!(
            replay,
            (StreamedReleaseOutcome::AlreadyReleased, None, true)
        );
        assert!(matches!(
            registry
                .read(reference_read(&registration.public_handle))
                .await,
            Err(StreamedResultRegistryError::Released)
        ));
    }

    #[tokio::test]
    async fn reference_registry_capacity_is_strict_and_publish_preserves_live_tombstones() {
        let registry = StreamedResultRegistry::try_new(16, test_resource_budget()).unwrap();
        let mut first_handle = None;
        for index in 0..MAX_REFERENCE_RESOURCES {
            let registration = registry
                .publish_reference(reference_publication(index, 1))
                .await
                .expect("bounded reference admission");
            first_handle.get_or_insert(registration.public_handle);
        }
        assert!(matches!(
            registry
                .publish_reference(reference_publication(MAX_REFERENCE_RESOURCES, 1))
                .await,
            Err(StreamedResultRegistryError::ReferenceCapacity)
        ));

        registry
            .release(
                PRINCIPAL,
                &BTreeSet::from([WORKSPACE]),
                7,
                8,
                9,
                first_handle.as_deref().unwrap(),
                "release:capacity",
                20,
            )
            .await
            .expect("release capacity slot");
        assert!(matches!(
            registry
                .publish_reference(reference_publication(MAX_REFERENCE_RESOURCES + 1, 20))
                .await,
            Err(StreamedResultRegistryError::ReferenceCapacity)
        ));
        let first_handle = first_handle.as_deref().unwrap();
        assert_eq!(
            registry
                .release(
                    PRINCIPAL,
                    &BTreeSet::from([WORKSPACE]),
                    7,
                    8,
                    9,
                    first_handle,
                    "release:capacity",
                    21,
                )
                .await
                .expect("same release identity replays through an unrelated admission"),
            (StreamedReleaseOutcome::AlreadyReleased, None, true)
        );
        for index in 0..512 {
            assert_eq!(
                registry
                    .release(
                        PRINCIPAL,
                        &BTreeSet::from([WORKSPACE]),
                        7,
                        8,
                        9,
                        first_handle,
                        &format!("release:reference-flood:{index}"),
                        21,
                    )
                    .await
                    .expect("distinct release identities remain bounded"),
                (StreamedReleaseOutcome::AlreadyReleased, None, false)
            );
        }
        {
            let references = registry.references.lock().await;
            let tombstone = references.get(first_handle).unwrap();
            assert_eq!(tombstone.release_id.as_deref(), Some("release:capacity"));
            assert!(tombstone.released);
            assert!(tombstone.content.is_empty());
        }
        assert!(matches!(
            registry
                .release(
                    PrincipalId::from_bytes([0x12; 16]),
                    &BTreeSet::from([WORKSPACE]),
                    7,
                    8,
                    9,
                    first_handle,
                    "release:capacity",
                    21,
                )
                .await,
            Err(StreamedResultRegistryError::WrongOwner)
        ));

        registry
            .publish_reference(reference_publication(MAX_REFERENCE_RESOURCES + 2, 102))
            .await
            .expect("expired entries are pruned before admission");
        assert_eq!(registry.references.lock().await.len(), 1);
        assert!(matches!(
            registry
                .release(
                    PRINCIPAL,
                    &BTreeSet::from([WORKSPACE]),
                    7,
                    8,
                    9,
                    first_handle,
                    "release:capacity",
                    102,
                )
                .await,
            Err(StreamedResultRegistryError::UnknownPackage)
        ));
    }

    #[tokio::test]
    async fn wp79_result_live_lease_reads_use_reserved_control_capacity() {
        let budget = test_resource_budget();
        let registry = StreamedResultRegistry::try_new(32, budget.clone()).unwrap();
        let sink = Arc::new(FaultSink::new());
        let registration = publish_result_fixture(&registry, &sink, "query:pressure").await;
        let data_limit =
            budget.policy().limits.memory_bytes - budget.policy().control_reserve.memory_bytes;
        let fill = data_limit - u64::try_from(budget.observation().used.memory_bytes).unwrap();
        let pressure = budget
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: fill,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert!(
            budget
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: 1,
                        ..ResourceAmounts::default()
                    }
                )
                .is_err()
        );
        let page = &registration.pages[0];
        let chunk = registry
            .read(result_read(
                &page.public_handle,
                StreamedResourceSelector::Page(0),
                0,
                32,
            ))
            .await
            .unwrap();
        assert_eq!(chunk.bytes.len(), 32);
        let used = budget.observation().used.memory_bytes;
        let alias = chunk.clone();
        assert_eq!(budget.observation().used.memory_bytes, used);
        drop(chunk);
        assert_eq!(budget.observation().used.memory_bytes, used);
        drop(alias);
        assert_eq!(budget.observation().used.memory_bytes, used - 32);
        drop(pressure);
    }

    #[tokio::test]
    async fn wp79_result_registry_rejects_forged_workspace_budget_lineage() {
        let root = super::super::streamed_result_package::test_resource_budget();
        let foreign = root
            .workspace(*WORKSPACE.as_bytes(), root.policy())
            .unwrap();
        let registry = StreamedResultRegistry::try_new(32, foreign).unwrap();
        let sink = Arc::new(FaultSink::new());
        let package = result_package(sink.clone()).await;
        assert!(matches!(
            registry
                .publish_package(
                    "query:foreign-root",
                    PRINCIPAL,
                    WORKSPACE,
                    7,
                    8,
                    9,
                    package.clone(),
                    ResultCleanup::Package(package),
                    None
                )
                .await,
            Err(StreamedResultRegistryError::BudgetOwnerMismatch)
        ));
        let budget = test_resource_budget();
        let operation = budget.operation([0x79; 16], budget.policy()).unwrap();
        let registry = StreamedResultRegistry::try_new(32, operation).unwrap();
        registry
            .publish_reference(reference_publication(0, 10))
            .await
            .unwrap();
        let mut wrong = reference_publication(1, 10);
        wrong.workspace_id = WorkspaceId::from_bytes([0x23; 16]);
        assert!(matches!(
            registry.publish_reference(wrong).await,
            Err(StreamedResultRegistryError::BudgetOwnerMismatch)
        ));
    }

    #[tokio::test]
    async fn wp45_resource_scoped_manifest_pages_release_only_after_last_handle() {
        let registry = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        let sink = Arc::new(FaultSink::new());
        let registration = publish_result_fixture(&registry, &sink, "query:scoped").await;
        assert_eq!(registration.pages.len(), 3);
        let handles = std::iter::once(&registration.manifest.public_handle)
            .chain(registration.pages.iter().map(|page| &page.public_handle))
            .collect::<BTreeSet<_>>();
        assert_eq!(handles.len(), 4);
        assert!(
            handles
                .iter()
                .all(|handle| !handle.contains("query:scoped"))
        );

        assert!(matches!(
            registry
                .read(result_read(
                    &registration.manifest.public_handle,
                    StreamedResourceSelector::Page(0),
                    0,
                    32,
                ))
                .await,
            Err(StreamedResultRegistryError::UnknownResource)
        ));
        assert!(matches!(
            registry
                .read(result_read(
                    &registration.pages[0].public_handle,
                    StreamedResourceSelector::Manifest,
                    0,
                    32,
                ))
                .await,
            Err(StreamedResultRegistryError::UnknownResource)
        ));

        let workspaces = BTreeSet::from([WORKSPACE]);
        for (principal, workspace_scope, daemon, policy, revocation, expected) in [
            (
                PrincipalId::from_bytes([0x12; 16]),
                BTreeSet::from([WORKSPACE]),
                7,
                8,
                9,
                StreamedResultRegistryError::WrongOwner,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WorkspaceId::from_bytes([0x23; 16])]),
                7,
                8,
                9,
                StreamedResultRegistryError::WrongWorkspace,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                17,
                8,
                9,
                StreamedResultRegistryError::GenerationMismatch,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                7,
                18,
                9,
                StreamedResultRegistryError::PolicyGenerationMismatch,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                7,
                8,
                19,
                StreamedResultRegistryError::PolicyGenerationMismatch,
            ),
        ] {
            let denied = registry
                .release(
                    principal,
                    &workspace_scope,
                    daemon,
                    policy,
                    revocation,
                    &registration.manifest.public_handle,
                    "release:denied",
                    30,
                )
                .await
                .expect_err("wrong result-handle authority must fail before release");
            assert_eq!(
                std::mem::discriminant(&denied),
                std::mem::discriminant(&expected)
            );
        }
        let mut overrange = result_read(
            &registration.pages[0].public_handle,
            StreamedResourceSelector::Page(0),
            registration.pages[0].byte_length.saturating_add(1),
            32,
        );
        assert!(matches!(
            registry.read(overrange.clone()).await,
            Err(StreamedResultRegistryError::RangeOutsideResource)
        ));
        overrange.offset = 0;
        overrange.maximum_bytes = 33;
        assert!(matches!(
            registry.read(overrange).await,
            Err(StreamedResultRegistryError::InvalidChunkBound)
        ));
        let mut invented = result_read(
            "public:result:python-invented",
            StreamedResourceSelector::Manifest,
            0,
            32,
        );
        invented.daemon_generation = 7;
        assert!(matches!(
            registry.read(invented).await,
            Err(StreamedResultRegistryError::UnknownPackage)
        ));

        let mut manifest = Vec::new();
        let mut offset = 0;
        loop {
            let chunk = registry
                .read(result_read(
                    &registration.manifest.public_handle,
                    StreamedResourceSelector::Manifest,
                    offset,
                    32,
                ))
                .await
                .unwrap();
            manifest.extend_from_slice(&chunk.bytes);
            offset = chunk.next_offset;
            if chunk.complete {
                break;
            }
        }
        let public_manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        let manifest_pages = public_manifest["pages"].as_array().unwrap();
        assert_eq!(manifest_pages.len(), registration.pages.len());
        for (page, registered) in manifest_pages.iter().zip(&registration.pages) {
            assert_eq!(page["public_handle"], registered.public_handle);
            assert_eq!(page["page_ordinal"], registered.page_ordinal.unwrap());
            assert_eq!(page["byte_length"], registered.byte_length);
            assert_eq!(page["content_checksum"], registered.content_checksum);
            assert_eq!(page["media_type"], registered.media_type);
            assert_eq!(page["expires_at_unix_ms"], registered.expires_at_unix_ms);
        }
        let public_text = String::from_utf8(manifest).unwrap();
        for forbidden in ["object_path", "lease_token", "file://", "s3://"] {
            assert!(!public_text.contains(forbidden));
        }

        let partial = registry
            .read(result_read(
                &registration.pages[0].public_handle,
                StreamedResourceSelector::Page(0),
                0,
                7,
            ))
            .await
            .unwrap();
        assert!(!partial.complete);
        let retained_objects = sink.object_count().await;
        assert_eq!(retained_objects, 4);
        for (handle, release_id) in [
            (&registration.pages[2].public_handle, "release:page-2"),
            (&registration.manifest.public_handle, "release:manifest"),
            (&registration.pages[1].public_handle, "release:page-1"),
        ] {
            let outcome = registry
                .release(PRINCIPAL, &workspaces, 7, 8, 9, handle, release_id, 30)
                .await
                .unwrap();
            assert_eq!(outcome, (StreamedReleaseOutcome::Released, None, false));
            assert_eq!(sink.object_count().await, retained_objects);
        }
        let still_readable = registry
            .read(result_read(
                &registration.pages[0].public_handle,
                StreamedResourceSelector::Page(0),
                partial.next_offset,
                32,
            ))
            .await
            .unwrap();
        assert!(!still_readable.bytes.is_empty());

        let final_release = registry
            .release(
                PRINCIPAL,
                &workspaces,
                7,
                8,
                9,
                &registration.pages[0].public_handle,
                "release:page-0",
                30,
            )
            .await
            .unwrap();
        assert_eq!(
            final_release,
            (
                StreamedReleaseOutcome::Released,
                Some("query:scoped".to_owned()),
                false,
            )
        );
        assert_eq!(sink.object_count().await, retained_objects);
        assert!(
            registry
                .finalize_released_query("query:scoped")
                .await
                .unwrap()
        );
        assert_eq!(sink.object_count().await, 0);
        {
            let state = registry.results.lock().await;
            assert!(state.packages.is_empty());
            assert_eq!(state.resources.len(), 4);
        }
        for (principal, workspace_scope, daemon, policy, revocation, expected) in [
            (
                PrincipalId::from_bytes([0x12; 16]),
                BTreeSet::from([WORKSPACE]),
                7,
                8,
                9,
                StreamedResultRegistryError::WrongOwner,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WorkspaceId::from_bytes([0x23; 16])]),
                7,
                8,
                9,
                StreamedResultRegistryError::WrongWorkspace,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                17,
                8,
                9,
                StreamedResultRegistryError::GenerationMismatch,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                7,
                18,
                9,
                StreamedResultRegistryError::PolicyGenerationMismatch,
            ),
            (
                PRINCIPAL,
                BTreeSet::from([WORKSPACE]),
                7,
                8,
                19,
                StreamedResultRegistryError::PolicyGenerationMismatch,
            ),
        ] {
            let denied = registry
                .release(
                    principal,
                    &workspace_scope,
                    daemon,
                    policy,
                    revocation,
                    &registration.pages[0].public_handle,
                    "release:page-0",
                    31,
                )
                .await
                .expect_err("release tombstone must reauthorize every binding");
            assert_eq!(
                std::mem::discriminant(&denied),
                std::mem::discriminant(&expected)
            );
        }
        assert_eq!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    7,
                    8,
                    9,
                    &registration.pages[0].public_handle,
                    "release:page-0",
                    31,
                )
                .await
                .expect("final release identity replays after package cleanup"),
            (StreamedReleaseOutcome::AlreadyReleased, None, true)
        );
        for index in 0..512 {
            assert_eq!(
                registry
                    .release(
                        PRINCIPAL,
                        &workspaces,
                        7,
                        8,
                        9,
                        &registration.pages[0].public_handle,
                        &format!("release:result-flood:{index}"),
                        31,
                    )
                    .await
                    .expect("result release identity storage remains bounded"),
                (StreamedReleaseOutcome::AlreadyReleased, None, false)
            );
        }
        {
            let state = registry.results.lock().await;
            let tombstone = state
                .resources
                .get(&registration.pages[0].public_handle)
                .unwrap();
            assert_eq!(tombstone.release_id.as_deref(), Some("release:page-0"));
            assert!(tombstone.released);
        }
        assert_eq!(registry.collect_expired(9_999).await.unwrap(), 0);
        assert_eq!(registry.collect_expired(10_000).await.unwrap(), 4);
        assert!(registry.results.lock().await.resources.is_empty());
        assert!(matches!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    7,
                    8,
                    9,
                    &registration.pages[0].public_handle,
                    "release:page-0",
                    10_000,
                )
                .await,
            Err(StreamedResultRegistryError::UnknownPackage)
        ));
    }

    #[tokio::test]
    async fn wp45_resource_scoped_manifest_pages_release_only_after_last_handle_republication_is_isolated()
     {
        let registry = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        let sink = Arc::new(FaultSink::new());
        let workspaces = BTreeSet::from([WORKSPACE]);
        let original = publish_result_fixture(&registry, &sink, "query:original").await;
        for (index, page) in original.pages.iter().enumerate() {
            assert_eq!(
                registry
                    .release(
                        PRINCIPAL,
                        &workspaces,
                        7,
                        8,
                        9,
                        &page.public_handle,
                        &format!("release:original-page:{index}"),
                        30,
                    )
                    .await
                    .unwrap(),
                (StreamedReleaseOutcome::Released, None, false)
            );
        }
        assert_eq!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    7,
                    8,
                    9,
                    &original.manifest.public_handle,
                    "release:original-manifest",
                    30,
                )
                .await
                .unwrap(),
            (
                StreamedReleaseOutcome::Released,
                Some("query:original".to_owned()),
                false,
            )
        );
        assert!(
            registry
                .finalize_released_query("query:original")
                .await
                .unwrap()
        );

        let replacement_package =
            result_package(Arc::clone(&sink) as Arc<dyn ResultObjectSink>).await;
        let replacement = registry
            .publish_package(
                "query:replacement",
                PRINCIPAL,
                WORKSPACE,
                17,
                18,
                19,
                replacement_package.clone(),
                ResultCleanup::Package(replacement_package),
                None,
            )
            .await
            .expect("republish the same package identity under fresh resource handles");
        assert_eq!(replacement.package_id, original.package_id);
        let original_handles = std::iter::once(&original.manifest.public_handle)
            .chain(original.pages.iter().map(|page| &page.public_handle))
            .collect::<BTreeSet<_>>();
        let replacement_handles = std::iter::once(&replacement.manifest.public_handle)
            .chain(replacement.pages.iter().map(|page| &page.public_handle))
            .collect::<BTreeSet<_>>();
        assert!(original_handles.is_disjoint(&replacement_handles));

        for (index, page) in replacement.pages.iter().enumerate() {
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    17,
                    18,
                    19,
                    &page.public_handle,
                    &format!("release:replacement-page:{index}"),
                    31,
                )
                .await
                .unwrap();
        }
        assert_eq!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    17,
                    18,
                    19,
                    &replacement.manifest.public_handle,
                    "release:replacement-manifest",
                    31,
                )
                .await
                .unwrap(),
            (
                StreamedReleaseOutcome::Released,
                Some("query:replacement".to_owned()),
                false,
            )
        );
        assert_eq!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    7,
                    8,
                    9,
                    &original.manifest.public_handle,
                    "release:original-manifest",
                    32,
                )
                .await
                .expect("old tombstone remains independent of the replacement package"),
            (StreamedReleaseOutcome::AlreadyReleased, None, true)
        );
        assert!(matches!(
            registry
                .release(
                    PRINCIPAL,
                    &workspaces,
                    17,
                    18,
                    19,
                    &original.manifest.public_handle,
                    "release:original-manifest",
                    32,
                )
                .await,
            Err(StreamedResultRegistryError::GenerationMismatch)
        ));
    }

    #[tokio::test]
    async fn wp45_restart_reissues_fresh_handles_from_the_durable_package_locator() {
        let sink = Arc::new(FaultSink::new());
        let initial = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        let registration = publish_result_fixture(&initial, &sink, "query:restart").await;
        let initial_handles = std::iter::once(registration.manifest.public_handle.clone())
            .chain(
                registration
                    .pages
                    .iter()
                    .map(|page| page.public_handle.clone()),
            )
            .collect::<BTreeSet<_>>();

        let recovered = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        recovered.install_package_builder(test_package_builder(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>
        ));
        let reissued = recovered
            .reissue_retained(
                "query:restart",
                PRINCIPAL,
                WORKSPACE,
                17,
                18,
                19,
                &registration.retained_locator,
                20,
            )
            .await
            .expect("reopen retained manifest and mint current-generation handles");
        assert_eq!(reissued.package_id, registration.package_id);
        assert_eq!(
            reissued.manifest_resource_id,
            registration.manifest_resource_id
        );
        let reissued_handles = std::iter::once(reissued.manifest.public_handle.clone())
            .chain(reissued.pages.iter().map(|page| page.public_handle.clone()))
            .collect::<BTreeSet<_>>();
        assert!(initial_handles.is_disjoint(&reissued_handles));
        assert!(matches!(
            recovered
                .read(StreamedResourceRead {
                    principal_id: PRINCIPAL,
                    workspace_ids: Arc::new(BTreeSet::from([WORKSPACE])),
                    daemon_generation: 7,
                    policy_generation: 8,
                    revocation_generation: 9,
                    public_handle: reissued.manifest.public_handle.clone(),
                    selector: StreamedResourceSelector::Manifest,
                    offset: 0,
                    maximum_bytes: 32,
                    observed_at_unix_ms: 20,
                })
                .await,
            Err(StreamedResultRegistryError::GenerationMismatch)
        ));
        recovered
            .read(StreamedResourceRead {
                principal_id: PRINCIPAL,
                workspace_ids: Arc::new(BTreeSet::from([WORKSPACE])),
                daemon_generation: 17,
                policy_generation: 18,
                revocation_generation: 19,
                public_handle: reissued.manifest.public_handle,
                selector: StreamedResourceSelector::Manifest,
                offset: 0,
                maximum_bytes: 32,
                observed_at_unix_ms: 20,
            })
            .await
            .expect("fresh handle is authorized under the replacement session");

        let mut tampered = registration.retained_locator.clone();
        tampered.expected_manifest_checksum = format!("b3:{}", "00".repeat(32));
        let rejected = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        rejected.install_package_builder(test_package_builder(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>
        ));
        assert!(matches!(
            rejected
                .reissue_retained(
                    "query:restart",
                    PRINCIPAL,
                    WORKSPACE,
                    27,
                    28,
                    29,
                    &tampered,
                    20,
                )
                .await,
            Err(StreamedResultRegistryError::RetainedLocatorMismatch)
        ));
    }

    #[tokio::test]
    async fn wp45_restart_cleanup_uses_only_pre_result_ready_object_intent_and_retries() {
        let initial = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        let sink = Arc::new(FaultSink::new());
        let registration = publish_result_fixture(&initial, &sink, "query:intent-only").await;
        let object_set = PendingResultObjectSet {
            manifest_object_path: registration.retained_locator.manifest_object_path.clone(),
            page_object_paths: registration.retained_locator.page_object_paths.clone(),
            epoch_id: registration.retained_locator.epoch_id.clone(),
            query_execution: registration.retained_locator.query_execution.clone(),
        };
        assert_eq!(sink.object_count().await, 4);
        drop(initial);

        let recovered = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        recovered.install_package_builder(test_package_builder(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>
        ));
        sink.fail_delete_once.store(true, Ordering::SeqCst);
        assert!(matches!(
            recovered.cleanup_pending_object_set(&object_set).await,
            Err(StreamedResultRegistryError::Package(_))
        ));
        assert_eq!(sink.object_count().await, 4);
        recovered
            .cleanup_pending_object_set(&object_set)
            .await
            .expect("idempotent exact-set retry");
        assert_eq!(sink.object_count().await, 0);
    }

    #[tokio::test]
    async fn wp45_expiry_reclaims_handles_and_retries_failed_object_cleanup() {
        let registry = StreamedResultRegistry::try_new(32, test_resource_budget()).unwrap();
        let sink = Arc::new(FaultSink::new());
        let registration = publish_result_fixture(&registry, &sink, "query:expiry").await;
        assert_eq!(sink.object_count().await, 4);
        sink.fail_delete_once.store(true, Ordering::SeqCst);
        assert_eq!(registry.collect_expired(10_000).await.unwrap(), 4);
        assert!(matches!(
            registry
                .read(result_read(
                    &registration.manifest.public_handle,
                    StreamedResourceSelector::Manifest,
                    0,
                    32,
                ))
                .await,
            Err(StreamedResultRegistryError::Released)
        ));
        assert!(matches!(
            registry.finalize_released_query("query:expiry").await,
            Err(StreamedResultRegistryError::Package(_))
        ));
        assert_eq!(registry.results.lock().await.cleanup.len(), 1);
        assert_eq!(sink.object_count().await, 4);

        assert!(
            registry
                .finalize_released_query("query:expiry")
                .await
                .unwrap()
        );
        assert!(registry.results.lock().await.cleanup.is_empty());
        assert_eq!(sink.object_count().await, 0);
    }
}
