//! Durable serving-snapshot activation, leases, retention, and restart recovery.

#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwapOption;
use rusqlite::{OptionalExtension as _, params};
use thiserror::Error;

use crate::error::ErrorEnvelope;
use crate::fabric::SnapshotProviderCatalog;
use crate::identity::{
    IdentityDomain, decode_public_id, encode_public_id, random_registration_nonce,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
#[cfg(test)]
use crate::registries::WorkspaceRegistryLifecycle;
use crate::registries::{Phase, ServingActivationState, SnapshotLeaseKind, SnapshotLeaseState};
use crate::snapshot::{
    ResultAuthorityPin, ServingSnapshotManifest, ServingSnapshotManifestBody,
    SnapshotBasePublication, SnapshotBaseTable, SnapshotManifestError,
};
use crate::source_image::{SourceImageError, SourceImageStore};

/// Lease heartbeat cadence for work expected to exceed thirty seconds.
pub const SNAPSHOT_LEASE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
/// Default snapshot-lease expiry.
pub const SNAPSHOT_LEASE_TTL: Duration = Duration::from_mins(5);
/// Grace after process loss during which an orphaned lease still protects data.
pub const SNAPSHOT_ORPHAN_GRACE: Duration = Duration::from_hours(24);

/// Failures at the durable activation and lease boundary.
#[derive(Debug, Error)]
pub enum SnapshotRuntimeError {
    #[error(transparent)]
    Manifest(#[from] SnapshotManifestError),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
    #[error(transparent)]
    SourceImage(#[from] SourceImageError),
    #[error("INTERNAL_INVARIANT_VIOLATION:SNAPSHOT_CANDIDATE_INVALID:{0}")]
    Candidate(String),
    #[error("CURRENT_POINTER_CONFLICT:{0}")]
    PointerConflict(String),
    #[error("RESOURCE_EXPIRED:SNAPSHOT_LEASE_INVALID:{0}")]
    Lease(String),
    #[error("INTERNAL_INVARIANT_VIOLATION:SNAPSHOT_ACTIVATION_FAULT:{0:?}")]
    InjectedFault(SnapshotActivationFaultPoint),
}

/// Deterministic activation crash seams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotActivationFaultPoint {
    BeforeSqlCommit,
    AfterSqlCommitBeforeMemorySwap,
}

/// Observable activation stage order; timing is deliberately excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotActivationStage {
    CandidateValidated,
    ReadyInserted,
    PredecessorVerified,
    PriorRetired,
    CandidateActivated,
    DurablePointerCommitted,
    MemoryPointerSwapped,
}

/// Phase-carrying activation failure with every stage attempted before the failure.
pub type SnapshotActivationError = ErrorEnvelope<SnapshotRuntimeError, SnapshotActivationStage>;

/// Nonconstructible proof that the durable predecessor classified a candidate as an ordinary
/// fact publication under the currently active retained semantic package.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OrdinarySnapshotActivationPermit {
    workspace: [u8; 16],
    snapshot: [u8; 16],
    predecessor_snapshot: Option<[u8; 16]>,
}

impl OrdinarySnapshotActivationPermit {
    pub(crate) const fn workspace(&self) -> [u8; 16] {
        self.workspace
    }

    pub(crate) const fn snapshot(&self) -> [u8; 16] {
        self.snapshot
    }

    pub(crate) const fn predecessor_snapshot(&self) -> Option<[u8; 16]> {
        self.predecessor_snapshot
    }
}

fn snapshot_runtime_public_code(error: &SnapshotRuntimeError) -> &'static str {
    match error {
        SnapshotRuntimeError::PointerConflict(_) => "CURRENT_POINTER_CONFLICT",
        SnapshotRuntimeError::Lease(_) => "RESOURCE_EXPIRED",
        _ => "INTERNAL_INVARIANT_VIOLATION",
    }
}

/// A fully validated WP26 provider set bound to one immutable AC-G-19 manifest.
#[derive(Debug)]
pub struct ServingSnapshotCandidate {
    manifest: ServingSnapshotManifest,
    providers: Arc<SnapshotProviderCatalog>,
    source_blob_digests: Arc<[[u8; 32]]>,
}

impl ServingSnapshotCandidate {
    /// Build manifest-owned table evidence from the already frozen provider catalog.
    ///
    /// Caller-supplied table and empty-overlay identity fields are overwritten from the
    /// catalog so no sibling authority can drift from the providers actually served.
    ///
    /// # Errors
    ///
    /// Rejects invalid identities, provider evidence, overlay identity, or manifest fields.
    pub fn build(
        mut body: ServingSnapshotManifestBody,
        providers: Arc<SnapshotProviderCatalog>,
        source_blob_digests: &[[u8; 32]],
    ) -> Result<Self, SnapshotRuntimeError> {
        let publication_id = encode_public_id(
            IdentityDomain::Publication,
            None,
            providers.publication_id(),
        )?;
        let tables = providers
            .provider_records()
            .map(|record| {
                Ok(SnapshotBaseTable {
                    table_code: u16::try_from(record.manifest.table_code).map_err(|_| {
                        SnapshotRuntimeError::Candidate("negative table code".into())
                    })?,
                    table_uri: record.manifest.table_uri.clone(),
                    delta_version: record.manifest.delta_version,
                    schema_digest: framed_digest(record.manifest.schema_fingerprint),
                    row_count: u64::try_from(record.effective_row_count).map_err(|_| {
                        SnapshotRuntimeError::Candidate("negative row count".into())
                    })?,
                    primary_key_digest: framed_digest(record.primary_key_digest),
                    effective_content_digest: framed_digest(record.effective_content_digest),
                })
            })
            .collect::<Result<Vec<_>, SnapshotRuntimeError>>()?;
        body.base_publication = SnapshotBasePublication {
            publication_id,
            tables,
        };
        body.overlay.overlay_generation = providers.overlay_generation();
        body.overlay.overlay_digest = framed_digest(providers.overlay_checksum());
        body.overlay.total_memory_bytes = providers.overlay_memory_bytes();
        body.overlay.tables = providers.overlay_tables().to_vec();
        if let Some(authority) = body.result_authority.as_mut() {
            authority.rebind_exact_table_set(providers.exact_table_set_identity());
        }
        let mut unique_blobs = source_blob_digests.to_vec();
        unique_blobs.sort_unstable();
        unique_blobs.dedup();
        body.source_blob_digests = unique_blobs.iter().copied().map(framed_digest).collect();
        let manifest = body.derive()?;
        Self::validate_and_bind(manifest, providers, source_blob_digests)
    }

    /// Validate an externally decoded manifest against one frozen provider catalog.
    ///
    /// # Errors
    ///
    /// Rejects any identity, table, overlay, or content evidence mismatch.
    pub fn validate_and_bind(
        manifest: ServingSnapshotManifest,
        providers: Arc<SnapshotProviderCatalog>,
        source_blob_digests: &[[u8; 32]],
    ) -> Result<Self, SnapshotRuntimeError> {
        manifest.validate()?;
        if manifest.raw_publication_id()? != providers.publication_id()
            || manifest.body.overlay.overlay_generation != providers.overlay_generation()
            || manifest.body.overlay.overlay_digest != framed_digest(providers.overlay_checksum())
            || manifest.body.overlay.total_memory_bytes != providers.overlay_memory_bytes()
            || manifest.body.overlay.tables != providers.overlay_tables()
        {
            return Err(SnapshotRuntimeError::Candidate(
                "manifest publication or overlay differs from frozen catalog".into(),
            ));
        }
        let workspace_id = manifest.raw_workspace_id()?;
        let context_ids = manifest.raw_analysis_context_ids()?;
        let provider_scope = providers.scope();
        let manifest_generation = i64::try_from(manifest.body.source.source_generation)
            .map_err(|_| SnapshotRuntimeError::Candidate("source generation exceeds i64".into()))?;
        let manifest_context_set = decode_public_id(
            IdentityDomain::ContextSet,
            None,
            &manifest.body.contexts.context_set_id,
        )?;
        if provider_scope.workspace_id != workspace_id
            || provider_scope.source_generation != manifest_generation
            || provider_scope.analysis_context_set_id != manifest_context_set
            || provider_scope.analysis_context_ids != context_ids
        {
            return Err(SnapshotRuntimeError::Candidate(
                "manifest row scope differs from frozen provider scope".into(),
            ));
        }
        if manifest.body.base_publication.tables.len() != providers.provider_records().len() {
            return Err(SnapshotRuntimeError::Candidate(
                "manifest table census differs from frozen catalog".into(),
            ));
        }
        if let Some(authority) = manifest.body.result_authority.as_ref()
            && (authority.exact_table_set_identity != providers.exact_table_set_identity()
                || authority.result_authority_identity
                    != crate::snapshot::governed_result_authority_identity(authority))
        {
            return Err(SnapshotRuntimeError::Candidate(
                "result authority is not bound to the frozen exact provider set".into(),
            ));
        }
        for table in &manifest.body.base_publication.tables {
            let table_code = i16::try_from(table.table_code)
                .map_err(|_| SnapshotRuntimeError::Candidate("table code exceeds i16".into()))?;
            let record = providers.provider_record(table_code).ok_or_else(|| {
                SnapshotRuntimeError::Candidate(format!("unknown table code {table_code}"))
            })?;
            let row_count = u64::try_from(record.effective_row_count)
                .map_err(|_| SnapshotRuntimeError::Candidate("negative row count".into()))?;
            if record.manifest.workspace_id != workspace_id
                || table.table_uri != record.manifest.table_uri
                || table.delta_version != record.manifest.delta_version
                || table.schema_digest != framed_digest(record.manifest.schema_fingerprint)
                || table.row_count != row_count
                || table.primary_key_digest != framed_digest(record.primary_key_digest)
                || table.effective_content_digest != framed_digest(record.effective_content_digest)
            {
                return Err(SnapshotRuntimeError::Candidate(format!(
                    "table {table_code} differs from frozen provider evidence"
                )));
            }
        }
        let mut unique_blobs = source_blob_digests.to_vec();
        unique_blobs.sort_unstable();
        unique_blobs.dedup();
        if manifest.raw_source_blob_digests()? != unique_blobs {
            return Err(SnapshotRuntimeError::Candidate(
                "manifest source-blob closure differs from supplied snapshot bytes".into(),
            ));
        }
        Ok(Self {
            manifest,
            providers,
            source_blob_digests: unique_blobs.into(),
        })
    }

    #[must_use]
    pub const fn manifest(&self) -> &ServingSnapshotManifest {
        &self.manifest
    }

    #[must_use]
    pub fn providers(&self) -> Arc<SnapshotProviderCatalog> {
        Arc::clone(&self.providers)
    }

    #[must_use]
    pub fn source_blob_digests(&self) -> &[[u8; 32]] {
        &self.source_blob_digests
    }

    pub(crate) fn staged_record(
        &self,
    ) -> Result<crate::snapshot::StagedServingSnapshot, SnapshotRuntimeError> {
        self.manifest.validate()?;
        Ok(crate::snapshot::StagedServingSnapshot {
            snapshot_id: self.manifest.raw_snapshot_id()?,
            workspace_id: self.manifest.raw_workspace_id()?,
            publication_id: self.manifest.raw_publication_id()?,
            manifest_body: self.manifest.body.canonical_body()?,
            manifest_json: serde_json::to_vec(&self.manifest)
                .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?,
            manifest_digest: self.manifest.raw_manifest_digest()?,
            result_authority: self.manifest.body.result_authority.clone(),
        })
    }

    /// Reconstruct one immutable provider graph solely from its durable manifest and exact
    /// Delta versions. No caller-retained provider or candidate object participates.
    ///
    /// # Errors
    ///
    /// Rejects absent or altered manifest bytes, in-memory overlays, malformed exact tuples,
    /// unavailable Delta versions, or provider/schema/content drift.
    #[allow(clippy::too_many_lines)] // Reconstruction authenticates the complete durable provider closure.
    pub async fn reconstruct_durable(
        store: &OperationalStore,
        snapshot_id: [u8; 16],
    ) -> Result<Arc<Self>, SnapshotRuntimeError> {
        let (manifest_json, manifest_body, manifest_digest) = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT manifest_json_bytes, manifest_body_bytes, manifest_digest
                         FROM serving_snapshot_manifest WHERE snapshot_id=?1",
                        [snapshot_id.as_slice()],
                        |row| {
                            Ok((
                                row.get::<_, Vec<u8>>(0)?,
                                row.get::<_, Vec<u8>>(1)?,
                                row.get::<_, Vec<u8>>(2)?,
                            ))
                        },
                    )
                    .optional()
            })?
            .ok_or_else(|| SnapshotRuntimeError::Candidate("durable snapshot is absent".into()))?;
        let manifest: ServingSnapshotManifest = serde_json::from_slice(&manifest_json)
            .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?;
        manifest.validate()?;
        if manifest.raw_snapshot_id()? != snapshot_id
            || manifest.body.canonical_body()? != manifest_body
            || manifest.raw_manifest_digest()?.as_slice() != manifest_digest
        {
            return Err(SnapshotRuntimeError::Candidate(
                "durable snapshot manifest bytes or identity differ".into(),
            ));
        }
        if manifest.body.overlay.overlay_generation != 0 || !manifest.body.overlay.tables.is_empty()
        {
            return Err(SnapshotRuntimeError::Candidate(
                "durable reconstruction requires a consolidated generation-zero overlay".into(),
            ));
        }
        let workspace_id = manifest.raw_workspace_id()?;
        let publication_id = manifest.raw_publication_id()?;
        let analysis_context_ids = manifest.raw_analysis_context_ids()?;
        let analysis_context_set_id = decode_public_id(
            IdentityDomain::ContextSet,
            None,
            &manifest.body.contexts.context_set_id,
        )?;
        let source_generation = i64::try_from(manifest.body.source.source_generation)
            .map_err(|_| SnapshotRuntimeError::Candidate("source generation exceeds i64".into()))?;
        let retained_runtime = manifest
            .body
            .result_authority
            .as_ref()
            .map(|pin| {
                crate::ontology_program::load_installed_ontology_program_package(
                    &store.artifact_root(),
                    &pin.package_identity,
                )
                .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))
                .and_then(|package| {
                    if package.manifest.package_identity != pin.package_identity {
                        return Err(SnapshotRuntimeError::Candidate(
                            "retained package identity differs from the snapshot pin".into(),
                        ));
                    }
                    let runtime = crate::ontology_program::EpochRuntimeAuthority::decode(&package)
                        .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?;
                    if runtime.identity() != pin.epoch_runtime_authority_identity {
                        return Err(SnapshotRuntimeError::Candidate(
                            "retained runtime identity differs from the snapshot pin".into(),
                        ));
                    }
                    Ok(runtime)
                })
            })
            .transpose()?;
        let mut tables = std::collections::BTreeMap::new();
        for table in &manifest.body.base_publication.tables {
            let table_code = i16::try_from(table.table_code)
                .map_err(|_| SnapshotRuntimeError::Candidate("table code exceeds i16".into()))?;
            let required = if let Some(runtime) = &retained_runtime {
                runtime
                    .table_specs()
                    .spec(table_code)
                    .ok_or_else(|| {
                        SnapshotRuntimeError::Candidate(format!(
                            "retained epoch has no exact table {table_code}"
                        ))
                    })?
                    .required_for_publication
            } else {
                crate::schema_registry::table_spec(table_code)
                    .ok_or_else(|| {
                        SnapshotRuntimeError::Candidate(format!(
                            "unknown legacy exact table {table_code}"
                        ))
                    })?
                    .required_for_publication
            };
            let record = crate::fabric::PublicationTableRecord {
                publication_id,
                workspace_id,
                table_code,
                table_uri: table.table_uri.clone(),
                delta_version: table.delta_version,
                schema_fingerprint: decode_b3_digest(
                    &table.schema_digest,
                    "base_publication.tables.schema_digest",
                )?,
                row_count: i64::try_from(table.row_count).map_err(|_| {
                    SnapshotRuntimeError::Candidate("table row count exceeds i64".into())
                })?,
                // Owner count is not a semantic/provider identity and is not used to construct
                // the exact Delta provider. The reopened provider is authenticated by schema,
                // row count, primary key, and content digest below.
                owner_count: 0,
                table_checksum: decode_b3_digest(
                    &table.effective_content_digest,
                    "base_publication.tables.effective_content_digest",
                )?,
                primary_key_digest: decode_b3_digest(
                    &table.primary_key_digest,
                    "base_publication.tables.primary_key_digest",
                )?,
                required,
                validated: true,
            };
            if tables.insert(table_code, record).is_some() {
                return Err(SnapshotRuntimeError::Candidate(format!(
                    "duplicate exact table {table_code}"
                )));
            }
        }
        let publication = crate::fabric::PublicationOutcome {
            publication_id,
            scope: crate::fabric::PublicationScope {
                workspace_id,
                source_generation,
                analysis_context_set_id,
                analysis_context_ids,
            },
            pointer: crate::fabric::CurrentPublicationRecord {
                workspace_id,
                publication_id,
                pointer_generation: 0,
                updated_at_micros: 0,
            },
            tables,
        };
        let providers = Arc::new(if let Some(runtime) = &retained_runtime {
            SnapshotProviderCatalog::build_retained(
                &publication,
                &crate::fabric::EmptySnapshotOverlay,
                runtime.table_specs(),
            )
            .await
            .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?
        } else {
            SnapshotProviderCatalog::build(&publication, &crate::fabric::EmptySnapshotOverlay)
                .await
                .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?
        });
        let source_blob_digests = manifest.raw_source_blob_digests()?;
        Ok(Arc::new(Self::validate_and_bind(
            manifest,
            providers,
            &source_blob_digests,
        )?))
    }
}

fn decode_b3_digest(value: &str, field: &'static str) -> Result<[u8; 32], SnapshotRuntimeError> {
    let payload = value
        .strip_prefix("b3:")
        .ok_or_else(|| SnapshotRuntimeError::Candidate(format!("{field} is not b3")))?;
    if payload.len() != 64 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SnapshotRuntimeError::Candidate(format!(
            "{field} has invalid digest bytes"
        )));
    }
    let mut decoded = [0_u8; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        let chunk = &payload.as_bytes()[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(
            std::str::from_utf8(chunk)
                .map_err(|_| SnapshotRuntimeError::Candidate(format!("{field} is not UTF-8")))?,
            16,
        )
        .map_err(|_| SnapshotRuntimeError::Candidate(format!("{field} is not hexadecimal")))?;
    }
    Ok(decoded)
}

/// Process-local active snapshot pointer backed by durable `SQLite` activation state.
#[derive(Default)]
pub struct ServingSnapshotRuntime {
    active: ArcSwapOption<ServingSnapshotCandidate>,
}

impl ServingSnapshotRuntime {
    #[must_use]
    pub fn active(&self) -> Option<Arc<ServingSnapshotCandidate>> {
        self.active.load_full()
    }

    /// Activate an ordinary fact snapshot only when it preserves the currently active ontology
    /// and result authority. This route cannot introduce or replace semantic authority.
    ///
    /// # Errors
    ///
    /// Rejects any authority change, missing authority after ontology activation, or ordinary
    /// snapshot pointer failure.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_ordinary_fact_snapshot(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        expected_predecessor: Option<[u8; 16]>,
        expected_active_pointer_generation: u64,
        observed_durable_pointer_generation: u64,
        now: u64,
        fault: Option<SnapshotActivationFaultPoint>,
    ) -> Result<Vec<SnapshotActivationStage>, SnapshotActivationError> {
        let permit = Self::classify_ordinary_candidate(store, &candidate, expected_predecessor)
            .map_err(|source| {
                ErrorEnvelope::new(
                    Phase::SnapshotActivation,
                    snapshot_runtime_public_code(&source),
                    vec![SnapshotActivationStage::CandidateValidated],
                    source,
                )
            })?;
        self.commit_permitted_fact_snapshot(
            store,
            candidate,
            permit,
            expected_predecessor,
            expected_active_pointer_generation,
            observed_durable_pointer_generation,
            now,
            fault,
        )
    }

    /// Derive candidate class from the complete durable predecessor; callers never name it.
    #[allow(clippy::too_many_lines)] // The classifier compares the full protected manifest and retained package.
    fn classify_ordinary_candidate(
        store: &OperationalStore,
        candidate: &ServingSnapshotCandidate,
        expected_predecessor: Option<[u8; 16]>,
    ) -> Result<OrdinarySnapshotActivationPermit, SnapshotRuntimeError> {
        let workspace_id = candidate.manifest.raw_workspace_id()?;
        let snapshot_id = candidate.manifest.raw_snapshot_id()?;
        let predecessor = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT manifest.manifest_json_bytes
                     FROM active_snapshot AS active
                     JOIN serving_snapshot_manifest AS manifest
                       ON manifest.snapshot_id=active.snapshot_id
                     WHERE active.workspace_id=?1",
                        [workspace_id.as_slice()],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()
            })?;
        let predecessor_manifest = predecessor
            .map(|bytes| {
                serde_json::from_slice::<ServingSnapshotManifest>(&bytes)
                    .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))
            })
            .transpose()?;
        let predecessor_snapshot_id = predecessor_manifest
            .as_ref()
            .map(ServingSnapshotManifest::raw_snapshot_id)
            .transpose()?;
        if predecessor_snapshot_id != expected_predecessor {
            return Err(SnapshotRuntimeError::PointerConflict(
                "ordinary candidate predecessor differs from the durable active manifest".into(),
            ));
        }
        if let Some(predecessor) = predecessor_manifest.as_ref() {
            let prior = &predecessor.body;
            let next = &candidate.manifest.body;
            if prior.manifest_version != next.manifest_version
                || prior.workspace_id != next.workspace_id
                || prior.repository_id != next.repository_id
                || prior.worktree_id != next.worktree_id
                || prior.registration_revision != next.registration_revision
                || prior.bundles != next.bundles
                || prior.limits_profile_digest != next.limits_profile_digest
            {
                return Err(SnapshotRuntimeError::PointerConflict(
                    "manifest difference class requires governed semantic activation".into(),
                ));
            }
        }
        let active = store.active_ontology_authority(workspace_id)?;
        match (active, candidate.manifest.body.result_authority.as_ref()) {
            (None, None) => {}
            (Some(active), Some(pin)) => {
                if pin.package_identity != active.package_identity
                    || pin.epoch_runtime_authority_identity
                        != active.epoch_runtime_authority_identity
                    || pin.program_identity != active.program_identity
                    || pin.function_catalog_identity != active.function_catalog_identity
                    || pin.policy_identity != active.policy_identity
                    || pin.query_form_identity != active.query_form_identity
                    || pin.checksum_version != active.checksum_version
                    || pin.result_authority_identity
                        != crate::snapshot::governed_result_authority_identity(pin)
                {
                    return Err(SnapshotRuntimeError::PointerConflict(
                        "ordinary candidate changes retained semantic authority".into(),
                    ));
                }
                let package_identity = store
                    .reader_factory()
                    .ontology_epoch_package_identity(&active.epoch_identity)?
                    .ok_or_else(|| {
                        SnapshotRuntimeError::PointerConflict(
                            "active ontology epoch has no retained executable package".into(),
                        )
                    })?;
                let package = crate::ontology_program::load_installed_ontology_program_package(
                    &store.artifact_root(),
                    &package_identity,
                )
                .map_err(|error| SnapshotRuntimeError::PointerConflict(error.to_string()))?;
                let policy =
                    crate::domain_conformance::DomainOperationPolicy::from_package(&package)
                        .map_err(|error| {
                            SnapshotRuntimeError::PointerConflict(error.to_string())
                        })?;
                let runtime = crate::ontology_program::EpochRuntimeAuthority::decode(&package)
                    .map_err(|error| SnapshotRuntimeError::PointerConflict(error.to_string()))?;
                if pin.package_identity != package.manifest.package_identity
                    || pin.epoch_runtime_authority_identity != runtime.identity()
                    || pin.program_identity != package.manifest.logical_program_identity
                    || pin.function_catalog_identity
                        != package.manifest.member_identities["program.calculation_contract"]
                    || pin.query_form_identity
                        != package.manifest.member_identities["program.phrase_binding"]
                    || pin.policy_identity != policy.result_policy_identity()
                    || pin.checksum_version
                        != crate::ontology_program::result_checksum_version(&package).map_err(
                            |error| SnapshotRuntimeError::PointerConflict(error.to_string()),
                        )?
                {
                    return Err(SnapshotRuntimeError::PointerConflict(
                        "ordinary candidate does not validate under the active retained package"
                            .into(),
                    ));
                }
            }
            _ => {
                return Err(SnapshotRuntimeError::PointerConflict(
                    "ordinary candidate changes ontology/result authority presence".into(),
                ));
            }
        }
        Ok(OrdinarySnapshotActivationPermit {
            workspace: workspace_id,
            snapshot: snapshot_id,
            predecessor_snapshot: predecessor_snapshot_id,
        })
    }

    /// Atomically activate after the durable pointer transaction commits.
    ///
    /// # Errors
    ///
    /// Rejects stale predecessor/generation observations, invalid manifest bytes, missing
    /// workspaces, or an injected crash seam.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)] // AC-G-26 fixes one auditable transaction order.
    fn commit_permitted_fact_snapshot(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        permit: OrdinarySnapshotActivationPermit,
        expected_predecessor: Option<[u8; 16]>,
        expected_active_pointer_generation: u64,
        observed_durable_pointer_generation: u64,
        now: u64,
        fault: Option<SnapshotActivationFaultPoint>,
    ) -> Result<Vec<SnapshotActivationStage>, SnapshotActivationError> {
        let mut trace = vec![SnapshotActivationStage::CandidateValidated];
        if permit.workspace
            != candidate.manifest.raw_workspace_id().map_err(|source| {
                ErrorEnvelope::new(
                    Phase::SnapshotActivation,
                    "INTERNAL_INVARIANT_VIOLATION",
                    trace.clone(),
                    SnapshotRuntimeError::Manifest(source),
                )
            })?
            || permit.snapshot
                != candidate.manifest.raw_snapshot_id().map_err(|source| {
                    ErrorEnvelope::new(
                        Phase::SnapshotActivation,
                        "INTERNAL_INVARIANT_VIOLATION",
                        trace.clone(),
                        SnapshotRuntimeError::Manifest(source),
                    )
                })?
            || permit.predecessor_snapshot != expected_predecessor
        {
            return Err(ErrorEnvelope::new(
                Phase::SnapshotActivation,
                "INTERNAL_INVARIANT_VIOLATION",
                trace,
                SnapshotRuntimeError::PointerConflict(
                    "activation permit is not bound to this candidate transition".into(),
                ),
            ));
        }
        self.activate_unphased(
            store,
            candidate,
            permit,
            expected_active_pointer_generation,
            observed_durable_pointer_generation,
            now,
            fault,
            &mut trace,
        )
        .map_err(|source| {
            ErrorEnvelope::new(
                Phase::SnapshotActivation,
                snapshot_runtime_public_code(&source),
                trace,
                source,
            )
        })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn commit_fact_snapshot(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        expected_predecessor: Option<[u8; 16]>,
        expected_active_pointer_generation: u64,
        observed_durable_pointer_generation: u64,
        now: u64,
        fault: Option<SnapshotActivationFaultPoint>,
    ) -> Result<Vec<SnapshotActivationStage>, SnapshotActivationError> {
        let permit = OrdinarySnapshotActivationPermit {
            workspace: candidate.manifest.raw_workspace_id().map_err(|source| {
                ErrorEnvelope::new(
                    Phase::SnapshotActivation,
                    "INTERNAL_INVARIANT_VIOLATION",
                    vec![SnapshotActivationStage::CandidateValidated],
                    SnapshotRuntimeError::Manifest(source),
                )
            })?,
            snapshot: candidate.manifest.raw_snapshot_id().map_err(|source| {
                ErrorEnvelope::new(
                    Phase::SnapshotActivation,
                    "INTERNAL_INVARIANT_VIOLATION",
                    vec![SnapshotActivationStage::CandidateValidated],
                    SnapshotRuntimeError::Manifest(source),
                )
            })?,
            predecessor_snapshot: expected_predecessor,
        };
        self.commit_permitted_fact_snapshot(
            store,
            candidate,
            permit,
            expected_predecessor,
            expected_active_pointer_generation,
            observed_durable_pointer_generation,
            now,
            fault,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn activate_unphased(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        permit: OrdinarySnapshotActivationPermit,
        expected_active_pointer_generation: u64,
        observed_durable_pointer_generation: u64,
        now: u64,
        fault: Option<SnapshotActivationFaultPoint>,
        trace: &mut Vec<SnapshotActivationStage>,
    ) -> Result<Vec<SnapshotActivationStage>, SnapshotRuntimeError> {
        candidate.manifest.validate()?;
        let publication_id = candidate.manifest.raw_publication_id()?;
        let manifest_body = candidate.manifest.body.canonical_body()?;
        let manifest_json = serde_json::to_vec(&candidate.manifest)
            .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?;
        let manifest_digest = candidate.manifest.raw_manifest_digest()?;
        let expected_generation = sql_u64(expected_active_pointer_generation)?;
        let observed_generation = sql_u64(observed_durable_pointer_generation)?;
        let now_sql = sql_u64(now)?;
        trace.extend([
            SnapshotActivationStage::ReadyInserted,
            SnapshotActivationStage::PredecessorVerified,
            SnapshotActivationStage::PriorRetired,
            SnapshotActivationStage::CandidateActivated,
        ]);
        store
            .commit_classified_ordinary_serving_pointer(
                &permit,
                publication_id,
                &manifest_body,
                &manifest_json,
                manifest_digest,
                expected_generation,
                observed_generation,
                now_sql,
                fault == Some(SnapshotActivationFaultPoint::BeforeSqlCommit),
            )
            .map_err(|error| {
                if fault == Some(SnapshotActivationFaultPoint::BeforeSqlCommit)
                    && error
                        .to_string()
                        .contains("injected ordinary serving-pointer fault")
                {
                    SnapshotRuntimeError::InjectedFault(
                        SnapshotActivationFaultPoint::BeforeSqlCommit,
                    )
                } else {
                    SnapshotRuntimeError::PointerConflict(error.to_string())
                }
            })?;
        trace.push(SnapshotActivationStage::DurablePointerCommitted);
        if fault == Some(SnapshotActivationFaultPoint::AfterSqlCommitBeforeMemorySwap) {
            return Err(SnapshotRuntimeError::InjectedFault(
                SnapshotActivationFaultPoint::AfterSqlCommitBeforeMemorySwap,
            ));
        }
        self.active.store(Some(candidate));
        trace.push(SnapshotActivationStage::MemoryPointerSwapped);
        Ok(trace.clone())
    }

    /// Reconstruct the process-local pointer only from matching, fully validating durable state.
    ///
    /// # Errors
    ///
    /// Rejects invalid candidates, malformed durable rows, or identity/byte mismatches.
    #[cfg(test)]
    fn recover_with_candidate(
        &self,
        store: &OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
    ) -> Result<bool, SnapshotRuntimeError> {
        candidate.manifest.validate()?;
        let snapshot_id = candidate.manifest.raw_snapshot_id()?;
        let workspace_id = candidate.manifest.raw_workspace_id()?;
        let expected_body = candidate.manifest.body.canonical_body()?;
        let expected_json = serde_json::to_vec(&candidate.manifest)
            .map_err(|error| SnapshotRuntimeError::Candidate(error.to_string()))?;
        let expected_digest = candidate.manifest.raw_manifest_digest()?;
        let found = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT manifest_body_bytes, manifest_json_bytes, manifest_digest
                     FROM serving_snapshot_manifest JOIN active_snapshot USING(snapshot_id)
                     WHERE active_snapshot.workspace_id=?1 AND snapshot_id=?2
                       AND serving_snapshot_manifest.state_code=?3",
                        params![
                            workspace_id.as_slice(),
                            snapshot_id.as_slice(),
                            i64::from(ServingActivationState::Active as u16),
                        ],
                        |row| {
                            Ok((
                                row.get::<_, Vec<u8>>(0)?,
                                row.get::<_, Vec<u8>>(1)?,
                                row.get::<_, Vec<u8>>(2)?,
                            ))
                        },
                    )
                    .optional()
            })?;
        let Some((body, json, digest)) = found else {
            return Ok(false);
        };
        if body != expected_body || json != expected_json || digest != expected_digest {
            return Err(SnapshotRuntimeError::Candidate(
                "durable active manifest does not match frozen candidate".into(),
            ));
        }
        self.active.store(Some(candidate));
        Ok(true)
    }

    #[cfg(test)]
    fn recover(
        &self,
        store: &OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
    ) -> Result<bool, SnapshotRuntimeError> {
        self.recover_with_candidate(store, candidate)
    }

    /// Reconstruct and install the active process-local pointer using only durable state and
    /// exact-version Delta providers.
    ///
    /// # Errors
    ///
    /// Rejects an absent active pointer or any manifest/provider reconstruction failure.
    pub async fn recover_durable(
        &self,
        store: &OperationalStore,
        workspace_id: [u8; 16],
    ) -> Result<bool, SnapshotRuntimeError> {
        let snapshot_id = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT snapshot_id FROM active_snapshot WHERE workspace_id=?1",
                        [workspace_id.as_slice()],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()
            })?
            .map(|bytes| {
                bytes.try_into().map_err(|bytes: Vec<u8>| {
                    SnapshotRuntimeError::Candidate(format!(
                        "active snapshot id has {} bytes",
                        bytes.len()
                    ))
                })
            })
            .transpose()?;
        let Some(snapshot_id) = snapshot_id else {
            return Ok(false);
        };
        let candidate = ServingSnapshotCandidate::reconstruct_durable(store, snapshot_id).await?;
        self.active.store(Some(candidate));
        Ok(true)
    }
}

/// Read-only durable snapshot lease record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotLeaseRecord {
    pub lease_id: [u8; 16],
    pub kind: SnapshotLeaseKind,
    pub workspace_id: [u8; 16],
    pub snapshot_id: [u8; 16],
    pub publication_id: [u8; 16],
    pub created_at: u64,
    pub last_heartbeat_at: u64,
    pub expires_at: u64,
    pub state: SnapshotLeaseState,
    pub process_instance_id: [u8; 16],
    pub source_blob_lease_id: Option<[u8; 16]>,
    pub ontology_epoch_identity: Option<String>,
    pub result_authority: Option<ResultAuthorityPin>,
}

fn byte_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn legacy_result_authority_pin(candidate: &ServingSnapshotCandidate) -> ResultAuthorityPin {
    let manifest = candidate.manifest();
    let body = &manifest.body;
    let program_identity = crate::integrity::framed_digest(
        format!(
            "legacy-program.v1\0{}\0{}\0{}",
            body.bundles.ontology_bundle_id,
            body.bundles.schema_bundle_id,
            body.bundles.query_language_bundle_id
        )
        .as_bytes(),
    );
    let function_catalog_identity = crate::integrity::framed_digest(
        format!(
            "legacy-function-catalog.v1\0{}\0{}",
            body.bundles.query_language_bundle_id, body.bundles.model_pack_bundle_id
        )
        .as_bytes(),
    );
    let policy_identity = crate::integrity::framed_digest(
        format!(
            "legacy-result-policy.v1\0{}\0{}",
            body.limits_profile_digest, body.manifest_version
        )
        .as_bytes(),
    );
    let query_form_identity = crate::integrity::framed_digest(
        format!(
            "legacy-query-form.v1\0{}",
            body.bundles.query_language_bundle_id
        )
        .as_bytes(),
    );
    let exact_table_set_identity = crate::integrity::framed_digest(
        format!(
            "legacy-exact-table-set.v1\0{}\0{}",
            manifest.manifest_digest, body.base_publication.publication_id
        )
        .as_bytes(),
    );
    let result_authority_identity = crate::integrity::framed_digest(
        format!(
            "legacy-result-authority.v1\0{program_identity}\0{function_catalog_identity}\0{policy_identity}\0{query_form_identity}\0{exact_table_set_identity}\0ResultChecksumV1"
        )
        .as_bytes(),
    );
    let epoch_runtime_authority_identity = legacy_epoch_runtime_authority_identity(body);
    let package_identity = crate::integrity::framed_digest(
        format!(
            "legacy-epoch-package.v1\0{}\0{}",
            body.bundles.ontology_bundle_id, body.bundles.schema_bundle_id
        )
        .as_bytes(),
    );
    ResultAuthorityPin {
        result_authority_identity,
        package_identity,
        epoch_runtime_authority_identity,
        program_identity,
        function_catalog_identity,
        policy_identity,
        query_form_identity,
        checksum_version: "ResultChecksumV1".into(),
        exact_table_set_identity,
    }
}

fn legacy_epoch_runtime_authority_identity(body: &ServingSnapshotManifestBody) -> String {
    crate::integrity::framed_digest(
        format!(
            "legacy-epoch-runtime-authority.v1\0{}\0{}",
            body.bundles.schema_bundle_id, body.bundles.query_language_bundle_id
        )
        .as_bytes(),
    )
}

/// In-process lease guard retaining the exact immutable snapshot graph.
#[derive(Debug)]
pub struct SnapshotLeaseGuard {
    record: SnapshotLeaseRecord,
    snapshot: Arc<ServingSnapshotCandidate>,
}

impl SnapshotLeaseGuard {
    #[must_use]
    pub const fn record(&self) -> &SnapshotLeaseRecord {
        &self.record
    }

    #[must_use]
    pub fn snapshot(&self) -> Arc<ServingSnapshotCandidate> {
        Arc::clone(&self.snapshot)
    }

    #[must_use]
    pub const fn result_authority(&self) -> Option<&ResultAuthorityPin> {
        self.record.result_authority.as_ref()
    }
}

/// Process-scoped lease manager; durable state remains authoritative across restarts.
#[derive(Clone, Copy, Debug)]
pub struct SnapshotLeaseManager {
    process_instance_id: [u8; 16],
}

impl SnapshotLeaseManager {
    #[must_use]
    pub const fn new(process_instance_id: [u8; 16]) -> Self {
        Self {
            process_instance_id,
        }
    }

    /// Acquire a snapshot lease and its coupled source-artifact holder.
    ///
    /// # Errors
    ///
    /// Rejects inactive snapshots, invalid identities/expiry, or persistence/coupling failures.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn acquire(
        &self,
        store: &mut OperationalStore,
        source_images: &mut SourceImageStore,
        candidate: Arc<ServingSnapshotCandidate>,
        kind: SnapshotLeaseKind,
        agent_instance_id: Option<&[u8; 16]>,
        now: u64,
        ttl: Duration,
        artifact_expires_at: Option<u64>,
    ) -> Result<SnapshotLeaseGuard, SnapshotRuntimeError> {
        let lease_id = random_registration_nonce()?;
        let workspace_id = candidate.manifest.raw_workspace_id()?;
        let snapshot_id = candidate.manifest.raw_snapshot_id()?;
        let publication_id = candidate.manifest.raw_publication_id()?;
        let active_authority = store.active_ontology_authority(workspace_id)?;
        let mut ontology_epoch_identity = active_authority
            .as_ref()
            .map(|authority| authority.epoch_identity.clone());
        let result_authority = match (
            active_authority.as_ref(),
            candidate.manifest.body.result_authority.as_ref(),
        ) {
            (Some(active), Some(pin))
                if pin.package_identity == active.package_identity
                    && pin.epoch_runtime_authority_identity
                        == active.epoch_runtime_authority_identity
                    && pin.program_identity == active.program_identity
                    && pin.function_catalog_identity == active.function_catalog_identity
                    && pin.policy_identity == active.policy_identity
                    && pin.query_form_identity == active.query_form_identity
                    && pin.checksum_version == active.checksum_version =>
            {
                pin.clone()
            }
            (None, None) => legacy_result_authority_pin(candidate.as_ref()),
            _ => {
                return Err(SnapshotRuntimeError::Lease(
                    "snapshot semantic authority does not match the active ontology epoch".into(),
                ));
            }
        };
        if ontology_epoch_identity.is_none() {
            ontology_epoch_identity = Some(crate::integrity::framed_digest(
                format!(
                    "legacy-ontology-epoch.v1\0{}\0{}",
                    byte_hex(&workspace_id),
                    result_authority.result_authority_identity
                )
                .as_bytes(),
            ));
        }
        if candidate.manifest.body.result_authority.is_some()
            && result_authority.exact_table_set_identity
                != candidate.providers.exact_table_set_identity()
        {
            return Err(SnapshotRuntimeError::Lease(
                "snapshot result authority does not match its exact provider set".into(),
            ));
        }
        let expires_at = now
            .checked_add(ttl.as_secs())
            .ok_or_else(|| SnapshotRuntimeError::Lease("expiry overflow".into()))?;
        let versions = candidate
            .manifest
            .body
            .base_publication
            .tables
            .iter()
            .map(|table| (table.table_code, table.delta_version))
            .collect::<Vec<_>>();
        let version_bytes = serde_json::to_vec(&versions)
            .map_err(|error| SnapshotRuntimeError::Lease(error.to_string()))?;
        let source_lease_id = source_images.acquire_serving_snapshot_lease(
            store,
            workspace_id,
            candidate.manifest.body.source.source_generation,
            lease_id,
            candidate.source_blob_digests(),
            ttl,
        )?;
        let insert = store.write_transaction(|transaction| {
            let active = transaction
                .query_row(
                    "SELECT snapshot_id FROM active_snapshot WHERE workspace_id=?1",
                    [workspace_id.as_slice()],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()?;
            if active.as_deref() != Some(snapshot_id.as_slice()) {
                return Err(SnapshotRuntimeError::Lease(
                    "candidate is not the durable active snapshot".into(),
                ));
            }
            transaction.execute(
                "INSERT INTO ontology_result_authority(
                   result_authority_identity, workspace_id, program_identity,
                   function_catalog_identity, policy_identity, query_form_identity,
                   checksum_version, exact_table_set_identity, created_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(result_authority_identity) DO NOTHING",
                params![
                    result_authority.result_authority_identity,
                    workspace_id.as_slice(),
                    result_authority.program_identity,
                    result_authority.function_catalog_identity,
                    result_authority.policy_identity,
                    result_authority.query_form_identity,
                    result_authority.checksum_version,
                    result_authority.exact_table_set_identity,
                    sql_u64(now)?,
                ],
            )?;
            transaction.execute(
                "INSERT INTO snapshot_lease(lease_id, lease_kind_code, workspace_id,
                 snapshot_id, base_publication_id, required_delta_versions_bytes,
                 requires_overlay, agent_instance_id, created_at, last_heartbeat_at,
                 expires_at, state_code, process_instance_id, orphaned_at,
                 artifact_expires_at, source_blob_lease_id, ontology_epoch_identity,
                 result_authority_identity)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11,
                         ?12, NULL, ?13, ?14, ?15, ?16)",
                params![
                    lease_id.as_slice(),
                    i64::from(kind as u16),
                    workspace_id.as_slice(),
                    snapshot_id.as_slice(),
                    publication_id.as_slice(),
                    version_bytes,
                    i64::from(candidate.manifest.body.overlay.overlay_generation != 0),
                    agent_instance_id.map(<[u8; 16]>::as_slice),
                    sql_u64(now)?,
                    sql_u64(expires_at)?,
                    i64::from(SnapshotLeaseState::Active as u16),
                    self.process_instance_id.as_slice(),
                    artifact_expires_at.map(sql_u64).transpose()?,
                    source_lease_id.as_ref().map(<[u8; 16]>::as_slice),
                    ontology_epoch_identity.as_deref(),
                    result_authority.result_authority_identity.as_str(),
                ],
            )?;
            let updated = transaction.execute(
                "UPDATE active_snapshot SET lease_count=lease_count+1
                 WHERE workspace_id=?1 AND snapshot_id=?2",
                params![workspace_id.as_slice(), snapshot_id.as_slice()],
            )?;
            if updated != 1 {
                return Err(SnapshotRuntimeError::Lease(
                    "active snapshot disappeared during lease acquisition".into(),
                ));
            }
            Ok::<_, SnapshotRuntimeError>(())
        });
        if let Err(error) = insert {
            if let Some(source_lease_id) = source_lease_id {
                source_images.release(store, source_lease_id)?;
            }
            return Err(error);
        }
        Ok(SnapshotLeaseGuard {
            record: SnapshotLeaseRecord {
                lease_id,
                kind,
                workspace_id,
                snapshot_id,
                publication_id,
                created_at: now,
                last_heartbeat_at: now,
                expires_at,
                state: SnapshotLeaseState::Active,
                process_instance_id: self.process_instance_id,
                source_blob_lease_id: source_lease_id,
                ontology_epoch_identity,
                result_authority: Some(result_authority),
            },
            snapshot: candidate,
        })
    }

    /// Rehydrate one crash-orphaned durable lease against the exact reconstructed snapshot and
    /// retained ontology package.
    ///
    /// # Errors
    ///
    /// Rejects an absent/expired/foreign active lease, snapshot or authority mismatch, missing
    /// executable package, or failed ownership transition.
    #[allow(clippy::too_many_lines)] // Rehydration authenticates every durable lease/epoch/provider binding.
    fn rehydrate_with_candidate(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        lease_id: [u8; 16],
        now: u64,
    ) -> Result<SnapshotLeaseGuard, SnapshotRuntimeError> {
        let workspace_id = candidate.manifest.raw_workspace_id()?;
        let snapshot_id = candidate.manifest.raw_snapshot_id()?;
        let publication_id = candidate.manifest.raw_publication_id()?;
        let mut record = Self::list(store, workspace_id)?
            .into_iter()
            .find(|record| record.lease_id == lease_id)
            .ok_or_else(|| SnapshotRuntimeError::Lease("durable lease is absent".into()))?;
        if record.snapshot_id != snapshot_id
            || record.publication_id != publication_id
            || record.expires_at < now
            || !matches!(
                record.state,
                SnapshotLeaseState::Active | SnapshotLeaseState::Orphaned
            )
            || (record.state == SnapshotLeaseState::Active
                && record.process_instance_id != self.process_instance_id)
        {
            return Err(SnapshotRuntimeError::Lease(
                "durable lease cannot be rehydrated by this process".into(),
            ));
        }
        let reconstructed_authority = candidate
            .manifest
            .body
            .result_authority
            .clone()
            .unwrap_or_else(|| legacy_result_authority_pin(&candidate));
        if record.result_authority.as_ref() != Some(&reconstructed_authority) {
            return Err(SnapshotRuntimeError::Lease(
                "durable lease authority differs from the reconstructed snapshot".into(),
            ));
        }
        if candidate.manifest.body.result_authority.is_some() {
            let epoch = record.ontology_epoch_identity.as_deref().ok_or_else(|| {
                SnapshotRuntimeError::Lease("governed lease has no ontology epoch".into())
            })?;
            let readers = store.reader_factory();
            let package_identity =
                readers
                    .ontology_epoch_package_identity(epoch)?
                    .ok_or_else(|| {
                        SnapshotRuntimeError::Lease(
                            "governed lease epoch has no executable package".into(),
                        )
                    })?;
            let package = crate::ontology_program::load_installed_ontology_program_package(
                &readers.artifact_root(),
                &package_identity,
            )
            .map_err(|error| SnapshotRuntimeError::Lease(error.to_string()))?;
            let policy = crate::domain_conformance::DomainOperationPolicy::from_package(&package)
                .map_err(|error| SnapshotRuntimeError::Lease(error.to_string()))?;
            let runtime = crate::ontology_program::EpochRuntimeAuthority::decode(&package)
                .map_err(|error| SnapshotRuntimeError::Lease(error.to_string()))?;
            let authority = record.result_authority.as_ref().ok_or_else(|| {
                SnapshotRuntimeError::Lease("governed lease has no result authority".into())
            })?;
            if authority.package_identity != package.manifest.package_identity
                || authority.epoch_runtime_authority_identity != runtime.identity()
                || authority.program_identity != package.manifest.logical_program_identity
                || authority.policy_identity != policy.result_policy_identity()
                || authority.function_catalog_identity
                    != package.manifest.member_identities["program.calculation_contract"]
                || authority.query_form_identity
                    != package.manifest.member_identities["program.phrase_binding"]
                || authority.checksum_version != runtime.checksum_version()
                || authority.exact_table_set_identity
                    != candidate.providers().exact_table_set_identity()
                || authority.result_authority_identity
                    != crate::snapshot::governed_result_authority_identity(authority)
            {
                return Err(SnapshotRuntimeError::Lease(
                    "retained package does not reconstruct the lease authority".into(),
                ));
            }
        }
        let now_sql = sql_u64(now)?;
        let changed = store.write_transaction(|transaction| {
            Ok::<_, SnapshotRuntimeError>(transaction.execute(
                "UPDATE snapshot_lease SET state_code=?2, process_instance_id=?3,
                 last_heartbeat_at=?4, orphaned_at=NULL
                 WHERE lease_id=?1 AND state_code IN (?5, ?6)",
                params![
                    lease_id.as_slice(),
                    i64::from(SnapshotLeaseState::Active as u16),
                    self.process_instance_id.as_slice(),
                    now_sql,
                    i64::from(SnapshotLeaseState::Active as u16),
                    i64::from(SnapshotLeaseState::Orphaned as u16),
                ],
            )?)
        })?;
        if changed != 1 {
            return Err(SnapshotRuntimeError::Lease(
                "durable lease ownership transition lost its compare-and-swap".into(),
            ));
        }
        record.state = SnapshotLeaseState::Active;
        record.process_instance_id = self.process_instance_id;
        record.last_heartbeat_at = now;
        Ok(SnapshotLeaseGuard {
            record,
            snapshot: candidate,
        })
    }

    #[cfg(test)]
    fn rehydrate(
        &self,
        store: &mut OperationalStore,
        candidate: Arc<ServingSnapshotCandidate>,
        lease_id: [u8; 16],
        now: u64,
    ) -> Result<SnapshotLeaseGuard, SnapshotRuntimeError> {
        self.rehydrate_with_candidate(store, candidate, lease_id, now)
    }

    /// Rehydrate a durable lease without accepting caller-retained snapshot/provider state.
    ///
    /// # Errors
    ///
    /// Rejects an absent lease, missing retained snapshot/package, exact-version reopen drift,
    /// or lease ownership conflict.
    pub async fn rehydrate_durable(
        &self,
        store: &mut OperationalStore,
        lease_id: [u8; 16],
        now: u64,
    ) -> Result<SnapshotLeaseGuard, SnapshotRuntimeError> {
        let record = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT snapshot_id FROM snapshot_lease WHERE lease_id=?1",
                        [lease_id.as_slice()],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()
            })?
            .ok_or_else(|| SnapshotRuntimeError::Lease("durable lease is absent".into()))?;
        let snapshot_id: [u8; 16] = record.try_into().map_err(|record: Vec<u8>| {
            SnapshotRuntimeError::Lease(format!(
                "durable lease snapshot id has {} bytes",
                record.len()
            ))
        })?;
        let candidate = ServingSnapshotCandidate::reconstruct_durable(store, snapshot_id).await?;
        self.rehydrate_with_candidate(store, candidate, lease_id, now)
    }

    /// Heartbeat an ACTIVE lease, extending its expiry.
    ///
    /// # Errors
    ///
    /// Rejects inactive, foreign-process, overflowing, or unpersistable leases.
    pub fn heartbeat(
        &self,
        store: &mut OperationalStore,
        lease_id: [u8; 16],
        now: u64,
        ttl: Duration,
    ) -> Result<(), SnapshotRuntimeError> {
        let expires_at = now
            .checked_add(ttl.as_secs())
            .ok_or_else(|| SnapshotRuntimeError::Lease("expiry overflow".into()))?;
        let changed = store.write_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE snapshot_lease SET last_heartbeat_at=?2, expires_at=?3
                     WHERE lease_id=?1 AND state_code=?4 AND process_instance_id=?5",
                    params![
                        lease_id.as_slice(),
                        sql_u64(now)?,
                        sql_u64(expires_at)?,
                        i64::from(SnapshotLeaseState::Active as u16),
                        self.process_instance_id.as_slice(),
                    ],
                )
                .map_err(SnapshotRuntimeError::from)
        })?;
        if changed != 1 {
            return Err(SnapshotRuntimeError::Lease(
                "lease is not active for this process".into(),
            ));
        }
        Ok(())
    }

    /// Release a lease through ACTIVE → RELEASING → RELEASED, idempotently.
    ///
    /// # Errors
    ///
    /// Rejects illegal orphan release or persistence/source-holder failures.
    pub fn release(
        &self,
        store: &mut OperationalStore,
        source_images: &mut SourceImageStore,
        lease_id: [u8; 16],
    ) -> Result<(), SnapshotRuntimeError> {
        let release = store.write_transaction(|transaction| {
            let record = transaction
                .query_row(
                    "SELECT workspace_id, snapshot_id, state_code, source_blob_lease_id
                     FROM snapshot_lease WHERE lease_id=?1",
                    [lease_id.as_slice()],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Option<Vec<u8>>>(3)?,
                        ))
                    },
                )
                .optional()?;
            let Some((workspace, snapshot, state, source_lease)) = record else {
                return Ok::<_, SnapshotRuntimeError>(None);
            };
            if state == i64::from(SnapshotLeaseState::Released as u16)
                || state == i64::from(SnapshotLeaseState::Expired as u16)
            {
                return Ok(None);
            }
            if state != i64::from(SnapshotLeaseState::Active as u16)
                && state != i64::from(SnapshotLeaseState::Releasing as u16)
            {
                return Err(SnapshotRuntimeError::Lease(
                    "orphaned leases expire after crash grace".into(),
                ));
            }
            transaction.execute(
                "UPDATE snapshot_lease SET state_code=?2 WHERE lease_id=?1",
                params![
                    lease_id.as_slice(),
                    i64::from(SnapshotLeaseState::Releasing as u16),
                ],
            )?;
            Ok(Some((workspace, snapshot, source_lease)))
        })?;
        let Some((workspace, snapshot, source_lease)) = release else {
            return Ok(());
        };
        if let Some(source_lease) = source_lease {
            source_images.release(store, fixed_blob::<16>(&source_lease, "source lease")?)?;
        }
        store.write_transaction(|transaction| {
            let changed = transaction.execute(
                "UPDATE snapshot_lease SET state_code=?2, source_blob_lease_id=NULL
                 WHERE lease_id=?1 AND state_code=?3",
                params![
                    lease_id.as_slice(),
                    i64::from(SnapshotLeaseState::Released as u16),
                    i64::from(SnapshotLeaseState::Releasing as u16),
                ],
            )?;
            if changed == 1 {
                transaction.execute(
                    "UPDATE active_snapshot SET lease_count=MAX(lease_count-1, 0)
                     WHERE workspace_id=?1 AND snapshot_id=?2",
                    params![workspace, snapshot],
                )?;
            }
            Ok::<_, SnapshotRuntimeError>(())
        })
    }

    /// Mark ACTIVE leases owned by prior processes ORPHANED; repeated sweeps are no-ops.
    ///
    /// # Errors
    ///
    /// Returns conversion or persistence failures.
    pub fn orphan_after_restart(
        &self,
        store: &mut OperationalStore,
        now: u64,
    ) -> Result<u64, SnapshotRuntimeError> {
        let changed = store.write_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE snapshot_lease SET state_code=?1, orphaned_at=?2
                     WHERE state_code=?3 AND process_instance_id != ?4",
                    params![
                        i64::from(SnapshotLeaseState::Orphaned as u16),
                        sql_u64(now)?,
                        i64::from(SnapshotLeaseState::Active as u16),
                        self.process_instance_id.as_slice(),
                    ],
                )
                .map_err(SnapshotRuntimeError::from)
        })?;
        Ok(u64::try_from(changed).unwrap_or(u64::MAX))
    }

    /// Expire heartbeat-dead ACTIVE leases and ORPHANED leases after the 24-hour grace.
    ///
    /// # Errors
    ///
    /// Returns read, transition, or source-holder release failures.
    pub fn expire(
        &self,
        store: &mut OperationalStore,
        source_images: &mut SourceImageStore,
        now: u64,
    ) -> Result<u64, SnapshotRuntimeError> {
        let now_sql = sql_u64(now)?;
        let grace_sql = sql_u64(SNAPSHOT_ORPHAN_GRACE.as_secs())?;
        let candidates = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT lease_id, workspace_id, snapshot_id, source_blob_lease_id
                 FROM snapshot_lease
                 WHERE (state_code=?1 AND expires_at<=?3)
                    OR (state_code=?2 AND expires_at<=?3 AND orphaned_at+?4<=?3)
                 ORDER BY lease_id",
                )?;
                statement
                    .query_map(
                        params![
                            i64::from(SnapshotLeaseState::Active as u16),
                            i64::from(SnapshotLeaseState::Orphaned as u16),
                            now_sql,
                            grace_sql,
                        ],
                        |row| {
                            Ok((
                                row.get::<_, Vec<u8>>(0)?,
                                row.get::<_, Vec<u8>>(1)?,
                                row.get::<_, Vec<u8>>(2)?,
                                row.get::<_, Option<Vec<u8>>>(3)?,
                            ))
                        },
                    )?
                    .collect::<Result<Vec<_>, _>>()
            })?;
        let mut expired = 0_u64;
        for (lease, workspace, snapshot, source_lease) in candidates {
            let changed = store.write_transaction(|transaction| {
                let changed = transaction.execute(
                    "UPDATE snapshot_lease SET state_code=?2, source_blob_lease_id=NULL
                     WHERE lease_id=?1 AND state_code IN (?3, ?4)",
                    params![
                        lease,
                        i64::from(SnapshotLeaseState::Expired as u16),
                        i64::from(SnapshotLeaseState::Active as u16),
                        i64::from(SnapshotLeaseState::Orphaned as u16),
                    ],
                )?;
                if changed == 1 {
                    transaction.execute(
                        "UPDATE active_snapshot SET lease_count=MAX(lease_count-1, 0)
                         WHERE workspace_id=?1 AND snapshot_id=?2",
                        params![workspace, snapshot],
                    )?;
                }
                Ok::<_, SnapshotRuntimeError>(changed)
            })?;
            if changed == 1 {
                if let Some(source_lease) = source_lease {
                    source_images
                        .release(store, fixed_blob::<16>(&source_lease, "source lease")?)?;
                }
                expired = expired.saturating_add(1);
            }
        }
        Ok(expired)
    }

    /// Read leases through an independent query-only `SQLite` connection.
    ///
    /// # Errors
    ///
    /// Rejects malformed durable registry codes/blob widths and read failures.
    pub fn list(
        store: &OperationalStore,
        workspace_id: [u8; 16],
    ) -> Result<Vec<SnapshotLeaseRecord>, SnapshotRuntimeError> {
        store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT lease.lease_id, lease.lease_kind_code, lease.workspace_id,
                 lease.snapshot_id, lease.base_publication_id, lease.created_at,
                 lease.last_heartbeat_at, lease.expires_at, lease.state_code,
                 lease.process_instance_id, lease.source_blob_lease_id,
                 lease.ontology_epoch_identity, authority.result_authority_identity,
                 authority.program_identity, authority.function_catalog_identity,
                 authority.policy_identity, authority.query_form_identity,
                 authority.checksum_version, authority.exact_table_set_identity,
                 manifest.manifest_json_bytes
                 FROM snapshot_lease AS lease
                 LEFT JOIN ontology_result_authority AS authority
                   ON authority.result_authority_identity=lease.result_authority_identity
                 JOIN serving_snapshot_manifest AS manifest
                   ON manifest.snapshot_id=lease.snapshot_id
                 WHERE lease.workspace_id=?1 ORDER BY lease.lease_id",
                )?;
                statement
                    .query_map([workspace_id.as_slice()], |row| {
                        let kind = SnapshotLeaseKind::try_from(row.get::<_, u16>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let state = SnapshotLeaseState::try_from(row.get::<_, u16>(8)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let result_authority_identity = row.get::<_, Option<String>>(12)?;
                        let result_authority = result_authority_identity
                            .map(|result_authority_identity| {
                                let manifest: ServingSnapshotManifest = serde_json::from_slice(
                                    &row.get::<_, Vec<u8>>(19)?,
                                )
                                .map_err(|error| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        19,
                                        rusqlite::types::Type::Blob,
                                        Box::new(error),
                                    )
                                })?;
                                let epoch_runtime_authority_identity =
                                    manifest.body.result_authority.as_ref().map_or_else(
                                        || legacy_epoch_runtime_authority_identity(&manifest.body),
                                        |pin| pin.epoch_runtime_authority_identity.clone(),
                                    );
                                let package_identity =
                                    manifest.body.result_authority.as_ref().map_or_else(
                                        || {
                                            crate::integrity::framed_digest(
                                                format!(
                                                    "legacy-epoch-package.v1\0{}\0{}",
                                                    manifest.body.bundles.ontology_bundle_id,
                                                    manifest.body.bundles.schema_bundle_id
                                                )
                                                .as_bytes(),
                                            )
                                        },
                                        |pin| pin.package_identity.clone(),
                                    );
                                Ok::<_, rusqlite::Error>(ResultAuthorityPin {
                                    result_authority_identity,
                                    package_identity,
                                    epoch_runtime_authority_identity,
                                    program_identity: row.get(13)?,
                                    function_catalog_identity: row.get(14)?,
                                    policy_identity: row.get(15)?,
                                    query_form_identity: row.get(16)?,
                                    checksum_version: row.get(17)?,
                                    exact_table_set_identity: row.get(18)?,
                                })
                            })
                            .transpose()?;
                        Ok(SnapshotLeaseRecord {
                            lease_id: fixed_blob_sql(row.get(0)?)?,
                            kind,
                            workspace_id: fixed_blob_sql(row.get(2)?)?,
                            snapshot_id: fixed_blob_sql(row.get(3)?)?,
                            publication_id: fixed_blob_sql(row.get(4)?)?,
                            created_at: sql_timestamp(row.get(5)?)?,
                            last_heartbeat_at: sql_timestamp(row.get(6)?)?,
                            expires_at: sql_timestamp(row.get(7)?)?,
                            state,
                            process_instance_id: fixed_blob_sql(row.get(9)?)?,
                            source_blob_lease_id: row
                                .get::<_, Option<Vec<u8>>>(10)?
                                .map(fixed_blob_sql)
                                .transpose()?,
                            ontology_epoch_identity: row.get(11)?,
                            result_authority,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(Into::into)
    }
}

/// Explicit five-way retention-set input.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SnapshotRetentionInput {
    pub current_publication: Option<[u8; 16]>,
    pub recovery_eligible_publications: BTreeSet<[u8; 16]>,
    pub minimum_window_publications: BTreeSet<[u8; 16]>,
}

/// Non-timing evidence for the five union sources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SnapshotRetentionMetrics {
    pub current_count: usize,
    pub active_snapshot_count: usize,
    pub lease_count: usize,
    pub recovery_count: usize,
    pub minimum_window_count: usize,
    pub provenance_count: usize,
    pub retained_count: usize,
}

/// Closed publication set that no vacuum operation may invalidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRetentionSet {
    publications: BTreeSet<[u8; 16]>,
    metrics: SnapshotRetentionMetrics,
}

impl SnapshotRetentionSet {
    /// Build the exact five-source retention union from durable state and caller-owned
    /// publication/recovery windows.
    ///
    /// # Errors
    ///
    /// Rejects malformed durable publication identities and read/conversion failures.
    pub fn build(
        store: &OperationalStore,
        input: SnapshotRetentionInput,
        now: u64,
    ) -> Result<Self, SnapshotRuntimeError> {
        let now_sql = sql_u64(now)?;
        let grace_sql = sql_u64(SNAPSHOT_ORPHAN_GRACE.as_secs())?;
        let (active, leases, provenance) =
            store
                .reader_factory()
                .open()?
                .with_connection(|connection| {
                    let active = {
                        let mut statement = connection.prepare(
                            "SELECT publication_id FROM serving_snapshot_manifest
                     WHERE state_code=?1 ORDER BY publication_id",
                        )?;
                        statement
                            .query_map([i64::from(ServingActivationState::Active as u16)], |row| {
                                row.get::<_, Vec<u8>>(0)
                            })?
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    let leases = {
                        let mut statement = connection.prepare(
                            "SELECT DISTINCT base_publication_id FROM snapshot_lease
                     WHERE (state_code IN (?1, ?2) AND expires_at>?4)
                        OR (state_code=?3 AND orphaned_at+?5>?4)
                     ORDER BY base_publication_id",
                        )?;
                        statement
                            .query_map(
                                params![
                                    i64::from(SnapshotLeaseState::Active as u16),
                                    i64::from(SnapshotLeaseState::Releasing as u16),
                                    i64::from(SnapshotLeaseState::Orphaned as u16),
                                    now_sql,
                                    grace_sql,
                                ],
                                |row| row.get::<_, Vec<u8>>(0),
                            )?
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    let provenance = {
                        let mut statement = connection.prepare(
                            "SELECT DISTINCT publication_id FROM serving_snapshot_manifest
                         ORDER BY publication_id",
                        )?;
                        statement
                            .query_map([], |row| row.get::<_, Vec<u8>>(0))?
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    Ok::<_, rusqlite::Error>((active, leases, provenance))
                })?;
        let active = active
            .into_iter()
            .map(|bytes| fixed_blob::<16>(&bytes, "active publication"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let leases = leases
            .into_iter()
            .map(|bytes| fixed_blob::<16>(&bytes, "lease publication"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let provenance = provenance
            .into_iter()
            .map(|bytes| fixed_blob::<16>(&bytes, "provenance publication"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let metrics = SnapshotRetentionMetrics {
            current_count: usize::from(input.current_publication.is_some()),
            active_snapshot_count: active.len(),
            lease_count: leases.len(),
            recovery_count: input.recovery_eligible_publications.len(),
            minimum_window_count: input.minimum_window_publications.len(),
            provenance_count: provenance.len(),
            retained_count: 0,
        };
        let mut publications = BTreeSet::new();
        publications.extend(input.current_publication);
        publications.extend(active);
        publications.extend(leases);
        publications.extend(input.recovery_eligible_publications);
        publications.extend(input.minimum_window_publications);
        publications.extend(provenance);
        Ok(Self {
            metrics: SnapshotRetentionMetrics {
                retained_count: publications.len(),
                ..metrics
            },
            publications,
        })
    }

    #[must_use]
    pub fn contains(&self, publication_id: &[u8; 16]) -> bool {
        self.publications.contains(publication_id)
    }

    #[must_use]
    pub const fn metrics(&self) -> SnapshotRetentionMetrics {
        self.metrics
    }
}

/// One file proposed by a Delta vacuum dry-run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VacuumFileCandidate {
    pub publication_id: [u8; 16],
    pub table_code: i16,
    pub delta_version: u64,
    pub file_uri: String,
}

/// Validate library-native vacuum dry-run output before any destructive execution.
///
/// # Errors
///
/// Rejects any candidate reachable from the closed retention set.
pub fn validate_vacuum_dry_run(
    retention: &SnapshotRetentionSet,
    candidates: &[VacuumFileCandidate],
) -> Result<(), SnapshotRuntimeError> {
    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| retention.contains(&candidate.publication_id))
    {
        return Err(SnapshotRuntimeError::Candidate(format!(
            "vacuum dry-run listed retained publication for {}",
            candidate.file_uri
        )));
    }
    Ok(())
}

fn framed_digest(digest: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(67);
    encoded.push_str("b3:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to String is infallible");
    }
    encoded
}

fn sql_u64(value: u64) -> Result<i64, SnapshotRuntimeError> {
    i64::try_from(value).map_err(|_| SnapshotRuntimeError::Lease("value exceeds i64".into()))
}

fn fixed_blob<const N: usize>(
    bytes: &[u8],
    field: &'static str,
) -> Result<[u8; N], SnapshotRuntimeError> {
    bytes
        .try_into()
        .map_err(|_| SnapshotRuntimeError::Lease(format!("{field} has the wrong width")))
}

fn fixed_blob_sql<const N: usize>(bytes: Vec<u8>) -> rusqlite::Result<[u8; N]> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn sql_timestamp(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::SnapshotProviderCatalog;
    use crate::governed_session::GovernedSession;
    use crate::ontology_candidate::CandidateClosureRunner;
    use crate::ontology_gate::GateResourceEnvelope;
    use crate::ontology_program::{OntologyPackagingProfile, build_ontology_program_package};
    use crate::snapshot::{
        SnapshotBundles, SnapshotContextRecord, SnapshotContexts, SnapshotIndexes, SnapshotOverlay,
        SnapshotSource,
    };
    use arrow_array::{ArrayRef, Int64Array, RecordBatch};
    use datafusion::prelude::SessionConfig;
    use rusqlite::params;
    use tempfile::tempdir;

    const WORKSPACE: [u8; 16] = [0x11; 16];
    const PUBLICATION: [u8; 16] = [0x22; 16];
    const OVERLAY: [u8; 32] = [0x33; 32];
    const CONTEXT: [u8; 16] = [0x44; 16];

    fn publication_scope(source_generation: u64) -> crate::fabric::PublicationScope {
        let analysis_context_ids = vec![CONTEXT];
        crate::fabric::PublicationScope {
            workspace_id: WORKSPACE,
            source_generation: i64::try_from(source_generation).unwrap(),
            analysis_context_set_id: crate::identity::context_set_identity(
                WORKSPACE,
                &analysis_context_ids,
            )
            .unwrap()
            .id,
            analysis_context_ids,
        }
    }

    fn digest(byte: u8) -> String {
        framed_digest([byte; 32])
    }

    fn body(source_generation: u64) -> ServingSnapshotManifestBody {
        ServingSnapshotManifestBody {
            manifest_version: "1.0".into(),
            workspace_id: encode_public_id(IdentityDomain::Workspace, None, WORKSPACE).unwrap(),
            repository_id: None,
            worktree_id: None,
            registration_revision: 1,
            source: SnapshotSource {
                source_generation,
                admitted_event_sequence: source_generation,
                reconciled_event_sequence: source_generation,
                inventory_digest: digest(1),
                authorization_fingerprint: digest(2),
                inclusion_policy_fingerprint: digest(3),
                path_profile_version: "1".into(),
                source_trust_state: "CURRENT".into(),
                event_stream_health: "HEALTHY".into(),
                git_acceleration_status: "AVAILABLE".into(),
                git_state_fingerprint: Some(digest(4)),
            },
            contexts: SnapshotContexts {
                context_set_id: encode_public_id(
                    IdentityDomain::ContextSet,
                    None,
                    publication_scope(source_generation).analysis_context_set_id,
                )
                .unwrap(),
                default_python_context_id: None,
                default_rust_context_id: None,
                records: vec![SnapshotContextRecord {
                    analysis_context_id: encode_public_id(
                        IdentityDomain::AnalysisContext,
                        None,
                        CONTEXT,
                    )
                    .unwrap(),
                    context_manifest_digest: digest(9),
                    capability_partition_digest: digest(10),
                }],
            },
            base_publication: SnapshotBasePublication {
                publication_id: String::new(),
                tables: Vec::new(),
            },
            overlay: SnapshotOverlay {
                overlay_generation: 0,
                overlay_digest: digest(0),
                total_memory_bytes: 99,
                tables: Vec::new(),
            },
            indexes: SnapshotIndexes {
                capability_index_digest: digest(5),
                diagnostic_index_digest: digest(6),
                dependency_graph_digest: digest(7),
            },
            bundles: SnapshotBundles {
                ontology_bundle_id: "ontology:1.0".into(),
                schema_bundle_id: "schema:1.0".into(),
                provider_bundle_id: "provider:1.0".into(),
                derivation_bundle_id: "derivation:1.0".into(),
                query_language_bundle_id: "query:1.0".into(),
                model_pack_bundle_id: "model:1.0".into(),
                toolchain_bundle_id: "toolchain:1.0".into(),
                sandbox_profile_digests: std::collections::BTreeMap::new(),
            },
            result_authority: None,
            limits_profile_digest: digest(8),
            source_blob_digests: Vec::new(),
        }
    }

    fn candidate(source_generation: u64) -> Arc<ServingSnapshotCandidate> {
        let catalog = Arc::new(SnapshotProviderCatalog::empty_for_snapshot_tests(
            PUBLICATION,
            0,
            OVERLAY,
            publication_scope(source_generation),
        ));
        Arc::new(ServingSnapshotCandidate::build(body(source_generation), catalog, &[]).unwrap())
    }

    async fn ontology_report(
        predecessor: Option<String>,
    ) -> (
        crate::ontology_candidate::CandidateClosureReport,
        Arc<SnapshotProviderCatalog>,
    ) {
        let publication_id = PUBLICATION;
        let catalog = Arc::new(SnapshotProviderCatalog::single_for_snapshot_tests(
            publication_id,
            WORKSPACE,
            1,
            OVERLAY,
            2,
            vec![CONTEXT],
        ));
        let publication = crate::fabric::PublicationOutcome {
            publication_id,
            scope: catalog.scope().clone(),
            pointer: crate::fabric::CurrentPublicationRecord {
                workspace_id: WORKSPACE,
                publication_id,
                pointer_generation: 1,
                updated_at_micros: 100,
            },
            tables: BTreeMap::from([(1, catalog.provider_record(1).unwrap().manifest.clone())]),
        };
        let runner = CandidateClosureRunner::new_for_epoch(
            build_ontology_program_package(&OntologyPackagingProfile::default()).unwrap(),
            publication,
            GovernedSession::new(SessionConfig::new()).unwrap(),
            predecessor,
            100_000,
        )
        .unwrap();
        let batches = crate::schema_registry::table_specs()
            .iter()
            .map(|spec| {
                (
                    spec.table_code,
                    RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let providers = crate::ontology_relational_program::candidate_batch_providers(&batches)
            .expect("schema-faithful lease providers");
        let report = runner
            .execute_with_test_providers(&GateResourceEnvelope::default(), &providers)
            .await
            .unwrap();
        (report, catalog)
    }

    fn candidate_with_table() -> Arc<ServingSnapshotCandidate> {
        let catalog = Arc::new(SnapshotProviderCatalog::single_for_snapshot_tests(
            PUBLICATION,
            WORKSPACE,
            100,
            OVERLAY,
            1,
            vec![CONTEXT],
        ));
        Arc::new(ServingSnapshotCandidate::build(body(1), catalog, &[]).unwrap())
    }

    fn reject_table_mutation(
        candidate: &ServingSnapshotCandidate,
        mutate: impl FnOnce(&mut SnapshotBaseTable),
    ) {
        let mut body = candidate.manifest().body.clone();
        mutate(&mut body.base_publication.tables[0]);
        let manifest = body.derive().unwrap();
        assert!(
            ServingSnapshotCandidate::validate_and_bind(manifest, candidate.providers(), &[])
                .is_err()
        );
    }

    fn assert_recovery_rejects_corrupt_durable_bytes(
        store: &mut OperationalStore,
        candidate: &Arc<ServingSnapshotCandidate>,
    ) {
        let snapshot_id = candidate.manifest().raw_snapshot_id().unwrap();
        let expected_body = candidate.manifest().body.canonical_body().unwrap();
        let expected_json = serde_json::to_vec(candidate.manifest()).unwrap();
        let expected_digest = candidate.manifest().raw_manifest_digest().unwrap();
        for (column, expected) in [
            ("manifest_body_bytes", expected_body),
            ("manifest_json_bytes", expected_json),
            ("manifest_digest", expected_digest.to_vec()),
        ] {
            store
                .write_transaction(|transaction| {
                    transaction.execute(
                        &format!(
                            "UPDATE serving_snapshot_manifest SET {column}=X'00' WHERE snapshot_id=?1"
                        ),
                        [snapshot_id.as_slice()],
                    )?;
                    Ok::<_, OperationalStoreError>(())
                })
                .unwrap();
            assert!(
                ServingSnapshotRuntime::default()
                    .recover(store, Arc::clone(candidate))
                    .is_err()
            );
            store
                .write_transaction(|transaction| {
                    transaction.execute(
                        &format!(
                            "UPDATE serving_snapshot_manifest SET {column}=?2 WHERE snapshot_id=?1"
                        ),
                        params![snapshot_id.as_slice(), expected],
                    )?;
                    Ok::<_, OperationalStoreError>(())
                })
                .unwrap();
        }
    }

    fn assert_candidate_rejects_identity_and_table_mismatches(
        candidate: &Arc<ServingSnapshotCandidate>,
    ) {
        let mut corrupt = candidate.manifest().clone();
        corrupt.manifest_digest = digest(0xff);
        assert!(
            ServingSnapshotCandidate::validate_and_bind(corrupt, candidate.providers(), &[])
                .is_err()
        );
        let manifest = candidate.manifest().clone();
        let mut wrong_generation = manifest.body.clone();
        wrong_generation.source.source_generation += 1;
        assert!(
            ServingSnapshotCandidate::validate_and_bind(
                wrong_generation.derive().unwrap(),
                candidate.providers(),
                &[],
            )
            .is_err()
        );
        let mut wrong_context = manifest.body.clone();
        let replacement = [0x45; 16];
        wrong_context.contexts.records[0].analysis_context_id =
            encode_public_id(IdentityDomain::AnalysisContext, None, replacement).unwrap();
        wrong_context.contexts.context_set_id = encode_public_id(
            IdentityDomain::ContextSet,
            None,
            crate::identity::context_set_identity(WORKSPACE, &[replacement])
                .unwrap()
                .id,
        )
        .unwrap();
        assert!(
            ServingSnapshotCandidate::validate_and_bind(
                wrong_context.derive().unwrap(),
                candidate.providers(),
                &[],
            )
            .is_err()
        );
        for catalog in [
            SnapshotProviderCatalog::empty_for_snapshot_tests(
                [0x23; 16],
                0,
                OVERLAY,
                publication_scope(1),
            ),
            SnapshotProviderCatalog::empty_for_snapshot_tests(
                PUBLICATION,
                1,
                OVERLAY,
                publication_scope(1),
            ),
            SnapshotProviderCatalog::empty_for_snapshot_tests(
                PUBLICATION,
                0,
                [0x34; 32],
                publication_scope(1),
            ),
        ] {
            assert!(
                ServingSnapshotCandidate::validate_and_bind(
                    manifest.clone(),
                    Arc::new(catalog),
                    &[],
                )
                .is_err()
            );
        }

        let table_candidate = candidate_with_table();
        let mut missing_table = table_candidate.manifest().body.clone();
        missing_table.base_publication.tables.clear();
        assert!(
            ServingSnapshotCandidate::validate_and_bind(
                missing_table.derive().unwrap(),
                table_candidate.providers(),
                &[],
            )
            .is_err()
        );
        reject_table_mutation(&table_candidate, |table| table.table_code = u16::MAX);
        reject_table_mutation(&table_candidate, |table| table.table_code = 110);
        reject_table_mutation(&table_candidate, |table| table.table_uri.push_str("-other"));
        reject_table_mutation(&table_candidate, |table| table.delta_version += 1);
        reject_table_mutation(&table_candidate, |table| table.schema_digest = digest(0x91));
        reject_table_mutation(&table_candidate, |table| table.row_count += 1);
        reject_table_mutation(&table_candidate, |table| {
            table.primary_key_digest = digest(0x92);
        });
        reject_table_mutation(&table_candidate, |table| {
            table.effective_content_digest = digest(0x93);
        });
        let wrong_workspace = Arc::new(SnapshotProviderCatalog::single_for_snapshot_tests(
            PUBLICATION,
            [0x12; 16],
            100,
            OVERLAY,
            1,
            vec![CONTEXT],
        ));
        assert!(ServingSnapshotCandidate::build(body(1), wrong_workspace, &[]).is_err());
    }

    fn assert_vacuum_respects_retention(store: &OperationalStore) {
        let retention = SnapshotRetentionSet::build(
            store,
            SnapshotRetentionInput {
                current_publication: Some(PUBLICATION),
                ..SnapshotRetentionInput::default()
            },
            30,
        )
        .unwrap();
        let pinned = VacuumFileCandidate {
            publication_id: PUBLICATION,
            table_code: 100,
            delta_version: 1,
            file_uri: "file:///pinned.parquet".into(),
        };
        assert!(validate_vacuum_dry_run(&retention, &[pinned]).is_err());
        let unpinned = VacuumFileCandidate {
            publication_id: [0x99; 16],
            table_code: 100,
            delta_version: 1,
            file_uri: "file:///old.parquet".into(),
        };
        validate_vacuum_dry_run(&retention, &[unpinned]).unwrap();
    }

    fn store() -> (tempfile::TempDir, OperationalStore) {
        let directory = tempdir().unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO workspace_registration(workspace_id,
                     workspace_registration_nonce, registration_revision,
                     administrative_key, root_path_bytes, root_path_display,
                     root_directory_file_identity, platform_code,
                     case_sensitivity_mode, authorization_revision,
                     allowed_source_disclosure_rules, repository_id, worktree_id,
                     authorization_fingerprint, context_fingerprint, status_code,
                     created_at, updated_at)
                     VALUES (?1, ?2, 1, ?3, X'2f', '/', ?4, 10, 'sensitive', 1,
                             X'', NULL, NULL, ?5, ?6, ?7, '0', '0')",
                    params![
                        WORKSPACE.as_slice(),
                        [1_u8; 16].as_slice(),
                        b"test".as_slice(),
                        [2_u8; 16].as_slice(),
                        [3_u8; 32].as_slice(),
                        [4_u8; 32].as_slice(),
                        i64::from(WorkspaceRegistryLifecycle::Bootstrapping as u16),
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO worktree_state(workspace_id, worktree_id, repository_id,
                     work_dir_path_bytes, work_dir_path_display, git_dir_path_bytes,
                     git_dir_path_display, lifecycle_state_code, source_trust_state_code,
                     event_stream_health_code, git_acceleration_status_code,
                     active_snapshot_id, analysis_context_set_id, source_generation,
                     event_watermark, newest_dirty_generation, durable_generation,
                     reconcile_required, updated_at, last_diagnostic_id, inventory_digest)
                     VALUES (?1, NULL, NULL, X'2f', '/', NULL, NULL, 30, 30, 10, 10,
                             NULL, ?2, 1, 1, 0, 1, 0, '0', NULL, ?3)",
                    params![
                        WORKSPACE.as_slice(),
                        [0x44_u8; 16].as_slice(),
                        [0x55_u8; 32].as_slice(),
                    ],
                )?;
                Ok::<_, OperationalStoreError>(())
            })
            .unwrap();
        (directory, store)
    }

    fn source_store(directory: &tempfile::TempDir) -> SourceImageStore {
        SourceImageStore::open(
            &directory.path().join("source-images"),
            crate::source_image::SourceCapturePolicy::default(),
        )
        .unwrap()
    }

    #[test]
    fn wp24_behavioral_acceptance() {
        let (directory, mut store) = store();
        let runtime = ServingSnapshotRuntime::default();
        let first = candidate(1);
        let trace = runtime
            .commit_fact_snapshot(&mut store, Arc::clone(&first), None, 0, 7, 100, None)
            .unwrap();
        assert_eq!(
            trace,
            [
                SnapshotActivationStage::CandidateValidated,
                SnapshotActivationStage::ReadyInserted,
                SnapshotActivationStage::PredecessorVerified,
                SnapshotActivationStage::PriorRetired,
                SnapshotActivationStage::CandidateActivated,
                SnapshotActivationStage::DurablePointerCommitted,
                SnapshotActivationStage::MemoryPointerSwapped,
            ]
        );
        assert_eq!(
            runtime.active().unwrap().manifest().snapshot_id,
            first.manifest().snapshot_id
        );
        let status: u16 = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT status_code FROM workspace_registration WHERE workspace_id=?1",
                    [WORKSPACE.as_slice()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(status, WorkspaceRegistryLifecycle::Ready as u16);

        let mut sources = source_store(&directory);
        let manager = SnapshotLeaseManager::new([0x66; 16]);
        let lease = manager
            .acquire(
                &mut store,
                &mut sources,
                Arc::clone(&first),
                SnapshotLeaseKind::Query,
                Some(&[0xa1; 16]),
                101,
                SNAPSHOT_LEASE_TTL,
                None,
            )
            .unwrap();
        manager
            .heartbeat(&mut store, lease.record().lease_id, 110, SNAPSHOT_LEASE_TTL)
            .unwrap();
        assert_eq!(
            SnapshotLeaseManager::list(&store, WORKSPACE).unwrap().len(),
            1
        );
        manager
            .release(&mut store, &mut sources, lease.record().lease_id)
            .unwrap();
        assert_eq!(
            SnapshotLeaseManager::list(&store, WORKSPACE).unwrap()[0].state,
            SnapshotLeaseState::Released
        );
    }

    #[test]
    fn wp24_structural_acceptance() {
        let candidate = candidate(1);
        candidate.manifest().validate().unwrap();
        assert!(
            candidate
                .providers()
                .trace()
                .ends_with(&[crate::fabric::SnapshotConstructionStage::Freeze])
        );
        let json = serde_json::to_value(candidate.manifest()).unwrap();
        assert!(json.get("created_at").is_none());
        assert!(json.get("active_pointer_generation").is_none());
        assert_eq!(candidate.manifest().body.base_publication.tables.len(), 0);
        assert_eq!(candidate.manifest().body.overlay.total_memory_bytes, 0);
    }

    #[test]
    fn wp24_negative_zero_state() {
        let (directory, mut store) = store();
        let runtime = ServingSnapshotRuntime::default();
        let first = candidate(1);
        assert!(matches!(
            runtime.commit_fact_snapshot(&mut store, Arc::clone(&first), None, 1, 1, 9, None),
            Err(error)
                if matches!(error.source_error(), SnapshotRuntimeError::PointerConflict(_))
        ));
        assert!(runtime.active().is_none());
        runtime
            .commit_fact_snapshot(&mut store, Arc::clone(&first), None, 0, 1, 10, None)
            .unwrap();
        let second = candidate(2);
        let first_id = first.manifest().raw_snapshot_id().unwrap();
        assert!(matches!(
            runtime.commit_fact_snapshot(
                &mut store,
                Arc::clone(&second),
                Some([0xaa; 16]),
                1,
                2,
                18,
                None,
            ),
            Err(error)
                if matches!(error.source_error(), SnapshotRuntimeError::PointerConflict(_))
        ));
        assert!(matches!(
            runtime.commit_fact_snapshot(
                &mut store,
                Arc::clone(&second),
                Some(first_id),
                2,
                2,
                19,
                None,
            ),
            Err(error)
                if matches!(error.source_error(), SnapshotRuntimeError::PointerConflict(_))
        ));
        assert!(matches!(
            runtime.commit_fact_snapshot(
                &mut store,
                Arc::clone(&second),
                Some(first_id),
                1,
                2,
                20,
                Some(SnapshotActivationFaultPoint::BeforeSqlCommit),
            ),
            Err(error)
                if matches!(
                    error.source_error(),
                    SnapshotRuntimeError::InjectedFault(
                        SnapshotActivationFaultPoint::BeforeSqlCommit
                    )
                )
        ));
        assert_eq!(
            runtime.active().unwrap().manifest().snapshot_id,
            first.manifest().snapshot_id
        );
        assert!(matches!(
            runtime.commit_fact_snapshot(
                &mut store,
                Arc::clone(&second),
                Some(first_id),
                1,
                2,
                21,
                Some(SnapshotActivationFaultPoint::AfterSqlCommitBeforeMemorySwap),
            ),
            Err(error)
                if matches!(
                    error.source_error(),
                    SnapshotRuntimeError::InjectedFault(
                        SnapshotActivationFaultPoint::AfterSqlCommitBeforeMemorySwap
                    )
                )
        ));
        assert_eq!(
            runtime.active().unwrap().manifest().snapshot_id,
            first.manifest().snapshot_id
        );
        let recovered = ServingSnapshotRuntime::default();
        assert_recovery_rejects_corrupt_durable_bytes(&mut store, &second);
        assert!(recovered.recover(&store, Arc::clone(&second)).unwrap());
        assert_eq!(
            recovered.active().unwrap().manifest().snapshot_id,
            second.manifest().snapshot_id
        );

        assert_vacuum_respects_retention(&store);

        assert_candidate_rejects_identity_and_table_mismatches(&second);
        drop(directory);
    }

    #[test]
    fn wp24_operational_acceptance() {
        let (directory, mut store) = store();
        let runtime = ServingSnapshotRuntime::default();
        let active = candidate(1);
        runtime
            .commit_fact_snapshot(&mut store, Arc::clone(&active), None, 0, 1, 10, None)
            .unwrap();
        let mut sources = source_store(&directory);
        let prior = SnapshotLeaseManager::new([0x70; 16]);
        prior
            .acquire(
                &mut store,
                &mut sources,
                Arc::clone(&active),
                SnapshotLeaseKind::ResourceRead,
                None,
                11,
                Duration::from_secs(1),
                None,
            )
            .unwrap();
        let restarted = SnapshotLeaseManager::new([0x71; 16]);
        assert_eq!(restarted.orphan_after_restart(&mut store, 20).unwrap(), 1);
        assert_eq!(restarted.orphan_after_restart(&mut store, 20).unwrap(), 0);
        assert_eq!(restarted.expire(&mut store, &mut sources, 21).unwrap(), 0);
        assert_eq!(
            restarted
                .expire(
                    &mut store,
                    &mut sources,
                    20 + SNAPSHOT_ORPHAN_GRACE.as_secs(),
                )
                .unwrap(),
            1
        );
        let leases = SnapshotLeaseManager::list(&store, WORKSPACE).unwrap();
        assert_eq!(leases[0].state, SnapshotLeaseState::Expired);
        let metrics = SnapshotRetentionSet::build(
            &store,
            SnapshotRetentionInput {
                current_publication: Some(PUBLICATION),
                recovery_eligible_publications: BTreeSet::from([[0x80; 16]]),
                minimum_window_publications: BTreeSet::from([[0x81; 16]]),
            },
            30,
        )
        .unwrap()
        .metrics();
        assert_eq!(metrics.current_count, 1);
        assert_eq!(metrics.active_snapshot_count, 1);
        assert_eq!(metrics.recovery_count, 1);
        assert_eq!(metrics.minimum_window_count, 1);
        assert_eq!(metrics.retained_count, 3);
    }

    #[allow(clippy::too_many_lines)] // The test intentionally exercises the complete lease matrix.
    #[tokio::test]
    async fn ontology_result_authority_lease_matrix() {
        let (directory, mut store) = store();
        let database_path = directory.path().join("state.sqlite3");
        let runtime = ServingSnapshotRuntime::default();
        let legacy = candidate(1);
        runtime
            .commit_fact_snapshot(&mut store, Arc::clone(&legacy), None, 0, 1, 10, None)
            .unwrap();
        let mut sources = source_store(&directory);
        let manager = SnapshotLeaseManager::new([0x72; 16]);
        let old_lease = manager
            .acquire(
                &mut store,
                &mut sources,
                Arc::clone(&legacy),
                SnapshotLeaseKind::ResourceRead,
                None,
                11,
                Duration::from_secs(100),
                None,
            )
            .unwrap();
        assert_eq!(
            old_lease
                .result_authority()
                .expect("legacy lease has an explicit authority")
                .checksum_version,
            "ResultChecksumV1"
        );

        let (report, target_catalog) = ontology_report(None).await;
        let retained_package = crate::ontology_program::build_ontology_program_package(
            &crate::ontology_program::OntologyPackagingProfile::default(),
        )
        .unwrap();
        crate::ontology_program::install_ontology_program_package(
            &store.artifact_root(),
            &retained_package,
        )
        .unwrap();
        let evidence = report.durable_evidence();
        let authority = ResultAuthorityPin {
            result_authority_identity: evidence.result_authority_identity.clone(),
            package_identity: evidence.package_identity.clone(),
            epoch_runtime_authority_identity: evidence.epoch_runtime_authority_identity.clone(),
            program_identity: evidence.program_identity.clone(),
            function_catalog_identity: evidence.function_catalog_identity.clone(),
            policy_identity: evidence.result_policy_identity.clone(),
            query_form_identity: evidence.query_form_identity.clone(),
            checksum_version: evidence.checksum_version.clone(),
            exact_table_set_identity: evidence.exact_table_set_identity.clone(),
        };
        let mut target_body = body(2);
        target_body.manifest_version = "2.0".into();
        target_body.result_authority = Some(authority.clone());
        let target =
            Arc::new(ServingSnapshotCandidate::build(target_body, target_catalog, &[]).unwrap());
        let staged = target.staged_record().unwrap();
        store
            .persist_proved_ontology_candidate_with_snapshot(&report, &staged, 12)
            .unwrap();
        let decision = crate::operational_store::OntologyOwnerDecision::new_for_test(
            report.candidate_identity(),
            "owner:ontology-release",
            report.durable_evidence().policy_identity.clone(),
            13,
        )
        .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let request = store
            .resolve_ontology_activation_request(
                WORKSPACE,
                report.candidate_identity(),
                decision.identity(),
                "lease-authority-activation",
                &crate::integrity::framed_digest(report.candidate_identity().as_bytes()),
                13,
            )
            .unwrap();
        store.activate_ontology_candidate(&request).unwrap();
        let new_lease = manager
            .acquire(
                &mut store,
                &mut sources,
                target,
                SnapshotLeaseKind::ResourceRead,
                None,
                21,
                Duration::from_secs(100),
                None,
            )
            .unwrap();
        let new_authority = new_lease.result_authority().unwrap();
        assert_eq!(
            new_authority.result_authority_identity,
            authority.result_authority_identity
        );
        assert_eq!(new_authority.program_identity, authority.program_identity);
        assert_eq!(
            new_authority.exact_table_set_identity,
            new_lease.snapshot().providers().exact_table_set_identity()
        );
        assert_eq!(new_authority.checksum_version, "ResultChecksumV2");
        assert_ne!(
            old_lease.record().snapshot_id,
            new_lease.record().snapshot_id
        );
        let query_batch = RecordBatch::try_from_iter([(
            "value",
            Arc::new(Int64Array::from(vec![7_i64, 3, 7])) as ArrayRef,
        )])
        .unwrap();
        let old_result = crate::fabric::result_checksum_for_version(
            &old_lease.result_authority().unwrap().checksum_version,
            query_batch.schema().as_ref(),
            std::slice::from_ref(&query_batch),
            1_048_576,
        )
        .unwrap();
        let new_result = crate::fabric::result_checksum_for_version(
            &new_lease.result_authority().unwrap().checksum_version,
            query_batch.schema().as_ref(),
            std::slice::from_ref(&query_batch),
            1_048_576,
        )
        .unwrap();
        let (
            crate::fabric::VersionedResultChecksum::V1(old_result),
            crate::fabric::VersionedResultChecksum::V2(new_result),
        ) = (old_result, new_result)
        else {
            panic!("lease-pinned checksum dispatch selected the wrong encoder");
        };
        assert_eq!(old_result.canonical_schema, new_result.canonical_schema);
        assert_eq!(old_result.row_count, new_result.row_count);
        assert_ne!(old_result.checksum, new_result.checksum);

        let old_lease_id = old_lease.record().lease_id;
        let old_snapshot = old_lease.snapshot();
        let new_lease_id = new_lease.record().lease_id;
        let expected_new_result_identity = new_lease
            .result_authority()
            .unwrap()
            .result_authority_identity
            .clone();
        let new_snapshot = new_lease.snapshot();
        drop(old_lease);
        drop(new_lease);

        drop(store);
        let mut reopened = OperationalStore::open(&database_path).unwrap();
        let leases = SnapshotLeaseManager::list(&reopened, WORKSPACE).unwrap();
        assert_eq!(leases.len(), 2);
        assert_eq!(
            leases
                .iter()
                .filter(|lease| lease.result_authority.is_some())
                .count(),
            2
        );
        assert_eq!(
            leases
                .iter()
                .filter_map(|lease| lease.result_authority.as_ref())
                .map(|pin| pin.checksum_version.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["ResultChecksumV1", "ResultChecksumV2"])
        );
        assert!(leases.iter().any(|lease| {
            lease.result_authority.as_ref().is_some_and(|pin| {
                pin.program_identity == authority.program_identity
                    && pin.function_catalog_identity == authority.function_catalog_identity
                    && pin.policy_identity == authority.policy_identity
                    && pin.query_form_identity == authority.query_form_identity
                    && pin.checksum_version == "ResultChecksumV2"
                    && pin.exact_table_set_identity
                        == new_snapshot.providers().exact_table_set_identity()
            })
        }));
        let restarted = SnapshotLeaseManager::new([0x73; 16]);
        assert_eq!(
            restarted.orphan_after_restart(&mut reopened, 30).unwrap(),
            2
        );
        let rehydrated_old = restarted
            .rehydrate(&mut reopened, old_snapshot, old_lease_id, 31)
            .expect("rehydrate legacy lease after restart");
        let rehydrated_new = restarted
            .rehydrate(&mut reopened, new_snapshot, new_lease_id, 31)
            .expect("rehydrate retained-package lease after restart");
        assert_eq!(
            rehydrated_old.result_authority().unwrap().checksum_version,
            "ResultChecksumV1"
        );
        assert_eq!(
            rehydrated_new
                .result_authority()
                .unwrap()
                .result_authority_identity,
            expected_new_result_identity
        );
        let package_manifest = reopened
            .artifact_root()
            .join("ontology-programs")
            .join(
                retained_package
                    .manifest
                    .package_identity
                    .trim_start_matches("b3:"),
            )
            .join("manifest.json");
        let missing_manifest = package_manifest.with_extension("missing");
        std::fs::rename(&package_manifest, &missing_manifest).unwrap();
        assert!(
            restarted
                .rehydrate(&mut reopened, rehydrated_new.snapshot(), new_lease_id, 32)
                .is_err(),
            "missing retained package must reject lease rehydration"
        );
        std::fs::rename(missing_manifest, package_manifest).unwrap();
    }
}
