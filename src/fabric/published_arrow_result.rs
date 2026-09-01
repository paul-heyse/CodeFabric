//! Owner-bound publication and lease enforcement for immutable Arrow result packages.
//!
//! [`ArrowResultResourcePackage`] owns exact Arrow schemas, IPC bytes, checksums, and its
//! application-internal lease. This module adds the serving boundary: public identities bind the
//! owning workspace and agent, opaque tokens never become an internal [`LeaseId`], and the exact
//! [`FabricQueryLease`] keeps the admitted epoch alive until release or expiry. Semantic relation
//! IDs remain typed descriptor data; they are never a dispatch registry.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::relational_program::RelationId;

use super::admission::FabricQueryLease;
use super::arrow_result_resource::{
    ARROW_STREAM_MEDIA_TYPE, ArrowResultResourceError, ArrowResultResourcePackage,
    QueryExecutionPin, ResultCompleteness, ResultResourceId,
};
use super::child_session::resource_governance::EpochResultLeasePermit;
use super::command::{EpochId, LeaseId, PrincipalId, WorkspaceId};

const PUBLISHED_RESULT_FORMAT: &str = "codefabric.published-arrow-result.v1";
const CANONICAL_JSON_MEDIA_TYPE: &str = "application/json";

macro_rules! public_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Stable `b3:` control-plane representation.
            #[must_use]
            pub fn public_id(self) -> String {
                format!("b3:{}", hex(&self.0))
            }

            /// Decode the stable control-plane identity without conflating it with content IDs.
            pub fn try_from_public_id(
                value: &str,
            ) -> Result<Self, PublishedResultRegistryError> {
                let payload = value
                    .strip_prefix("b3:")
                    .ok_or(PublishedResultRegistryError::InvalidPublicIdentity)?;
                decode_hex(payload)
                    .map(Self)
                    .ok_or(PublishedResultRegistryError::InvalidPublicIdentity)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.public_id())
            }
        }
    };
}

public_identity!(
    /// Owner-bound identity of one externally advertised result artifact.
    PublishedArtifactId
);
public_identity!(
    /// Owner-bound identity of the immutable Arrow package behind an artifact.
    PublishedPackageId
);
public_identity!(
    /// Owner-bound identity of one relation-scoped Arrow IPC subresource.
    PublishedResultResourceId
);

/// Exact authenticated owner of one published result.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PublishedResultOwner {
    workspace_id: WorkspaceId,
    agent_id: PrincipalId,
}

impl PublishedResultOwner {
    #[must_use]
    pub const fn new(workspace_id: WorkspaceId, agent_id: PrincipalId) -> Self {
        Self {
            workspace_id,
            agent_id,
        }
    }

    #[must_use]
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn agent_id(self) -> PrincipalId {
        self.agent_id
    }

    /// Decode the two fixed-width public owner fields projected into the descriptor.
    pub fn try_from_public_ids(
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<Self, PublishedResultRegistryError> {
        Ok(Self::new(
            WorkspaceId::from_bytes(
                decode_hex(workspace_id)
                    .ok_or(PublishedResultRegistryError::InvalidPublicIdentity)?,
            ),
            PrincipalId::from_bytes(
                decode_hex(agent_id).ok_or(PublishedResultRegistryError::InvalidPublicIdentity)?,
            ),
        ))
    }
}

/// Opaque serving credential. It is intentionally a different type and width from [`LeaseId`].
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OpaqueResultLeaseToken([u8; 32]);

impl OpaqueResultLeaseToken {
    /// Construct a non-sentinel token supplied by the authenticated control plane.
    ///
    /// # Errors
    ///
    /// The all-zero value is reserved and rejected.
    pub const fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, PublishedResultRegistryError> {
        if all_zero(&bytes) {
            Err(PublishedResultRegistryError::InvalidOpaqueToken)
        } else {
            Ok(Self(bytes))
        }
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encode the secret as an opaque transport token without reusing an internal lease ID.
    #[must_use]
    pub fn public_token(&self) -> String {
        hex(&self.0)
    }

    /// Decode an opaque transport token into its distinct serving-credential type.
    pub fn try_from_public_token(value: &str) -> Result<Self, PublishedResultRegistryError> {
        let bytes = decode_hex(value).ok_or(PublishedResultRegistryError::InvalidOpaqueToken)?;
        Self::try_from_bytes(bytes)
    }
}

impl fmt::Debug for OpaqueResultLeaseToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueResultLeaseToken(REDACTED)")
    }
}

/// Exact public coverage attached to one relation descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedResultCoverage {
    pub state: ResultCompleteness,
    pub requested_units: u64,
    pub completed_units: u64,
    pub remainder_units: u64,
    pub unknown_cause: Option<String>,
}

/// Owner-bound descriptor for one application-owned Arrow relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRelationDescriptor {
    pub relation_id: RelationId,
    /// Owner-bound handle accepted for reads.
    pub authorization_resource_id: PublishedResultResourceId,
    /// Content identity carried by the canonical package manifest; never an authorization token.
    pub content_resource_id: ResultResourceId,
    pub media_type: &'static str,
    pub schema_checksum: [u8; 32],
    pub schema_byte_length: u64,
    pub content_checksum: [u8; 32],
    pub row_count: u64,
    pub batch_count: u64,
    pub byte_length: u64,
    pub coverage: PublishedResultCoverage,
}

/// Explicit mapping from the canonical manifest's content identity to its read authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedManifestDescriptor {
    /// Owner-bound handle accepted for reads.
    pub authorization_resource_id: PublishedResultResourceId,
    /// Content identity produced by the immutable Arrow package.
    pub content_resource_id: ResultResourceId,
    pub media_type: &'static str,
    pub content_checksum: [u8; 32],
    pub byte_length: u64,
}

/// Deterministic control-plane description of one owner-bound immutable package.
///
/// The package's internal manifest checksum and content identities remain visible as causal
/// evidence. Only the separately labeled owner-bound IDs may be used for authorization at this
/// serving boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedArrowResultDescriptor {
    pub format: &'static str,
    pub artifact_id: PublishedArtifactId,
    /// Owner-bound package identity used by the serving control plane.
    pub package_id: PublishedPackageId,
    /// Immutable package content identity; never accepted for authorization.
    pub content_package_id: ResultResourceId,
    pub owner: PublishedResultOwner,
    pub epoch_id: EpochId,
    pub query_execution: QueryExecutionPin,
    pub source_manifest_checksum: [u8; 32],
    pub source_manifest_byte_length: u64,
    pub completion: ResultCompleteness,
    pub total_rows: u64,
    pub total_batches: u64,
    pub total_schema_bytes: u64,
    pub total_ipc_bytes: u64,
    pub lease_expires_at_unix_ms: i64,
    pub manifest: PublishedManifestDescriptor,
    pub relations: Arc<[PublishedRelationDescriptor]>,
}

impl PublishedArrowResultDescriptor {
    /// Encode the strict presentation descriptor as RFC 8785 canonical JSON.
    ///
    /// This control projection contains identities, bounds, checksums, and completeness only;
    /// semantic rows remain in the separately authorized Arrow IPC resources.
    pub fn canonical_control_bytes(&self) -> Result<Vec<u8>, PublishedResultRegistryError> {
        serde_json_canonicalizer::to_vec(&PublishedDescriptorProjection::from(self))
            .map_err(PublishedResultRegistryError::CanonicalDescriptor)
    }
}

#[derive(Serialize)]
struct PublishedDescriptorProjection {
    format: &'static str,
    artifact_id: String,
    package_id: String,
    content_package_id: String,
    owner: PublishedOwnerProjection,
    epoch_id: String,
    query_execution: String,
    source_manifest_checksum: String,
    source_manifest_byte_length: u64,
    completion: &'static str,
    total_rows: u64,
    total_batches: u64,
    total_schema_bytes: u64,
    total_ipc_bytes: u64,
    lease_expires_at_unix_ms: i64,
    manifest: PublishedManifestProjection,
    relations: Vec<PublishedRelationProjection>,
}

#[derive(Serialize)]
struct PublishedOwnerProjection {
    workspace_id: String,
    agent_id: String,
}

#[derive(Serialize)]
struct PublishedManifestProjection {
    authorization_resource_id: String,
    content_resource_id: String,
    media_type: &'static str,
    content_checksum: String,
    byte_length: u64,
}

#[derive(Serialize)]
struct PublishedRelationProjection {
    relation_id: String,
    authorization_resource_id: String,
    content_resource_id: String,
    media_type: &'static str,
    schema_checksum: String,
    schema_byte_length: u64,
    content_checksum: String,
    row_count: u64,
    batch_count: u64,
    byte_length: u64,
    coverage: PublishedCoverageProjection,
}

#[derive(Serialize)]
struct PublishedCoverageProjection {
    state: &'static str,
    requested_units: u64,
    completed_units: u64,
    remainder_units: u64,
    unknown_cause: Option<String>,
}

impl From<&PublishedArrowResultDescriptor> for PublishedDescriptorProjection {
    fn from(value: &PublishedArrowResultDescriptor) -> Self {
        Self {
            format: value.format,
            artifact_id: value.artifact_id.public_id(),
            package_id: value.package_id.public_id(),
            content_package_id: value.content_package_id.public_id(),
            owner: PublishedOwnerProjection {
                workspace_id: hex(value.owner.workspace_id().as_bytes()),
                agent_id: hex(value.owner.agent_id().as_bytes()),
            },
            epoch_id: hex(value.epoch_id.as_bytes()),
            query_execution: framed_id(value.query_execution.as_bytes()),
            source_manifest_checksum: framed_id(&value.source_manifest_checksum),
            source_manifest_byte_length: value.source_manifest_byte_length,
            completion: completeness_name(value.completion),
            total_rows: value.total_rows,
            total_batches: value.total_batches,
            total_schema_bytes: value.total_schema_bytes,
            total_ipc_bytes: value.total_ipc_bytes,
            lease_expires_at_unix_ms: value.lease_expires_at_unix_ms,
            manifest: PublishedManifestProjection {
                authorization_resource_id: value.manifest.authorization_resource_id.public_id(),
                content_resource_id: value.manifest.content_resource_id.public_id(),
                media_type: value.manifest.media_type,
                content_checksum: framed_id(&value.manifest.content_checksum),
                byte_length: value.manifest.byte_length,
            },
            relations: value
                .relations
                .iter()
                .map(|relation| PublishedRelationProjection {
                    relation_id: relation.relation_id.as_str().to_owned(),
                    authorization_resource_id: relation.authorization_resource_id.public_id(),
                    content_resource_id: relation.content_resource_id.public_id(),
                    media_type: relation.media_type,
                    schema_checksum: framed_id(&relation.schema_checksum),
                    schema_byte_length: relation.schema_byte_length,
                    content_checksum: framed_id(&relation.content_checksum),
                    row_count: relation.row_count,
                    batch_count: relation.batch_count,
                    byte_length: relation.byte_length,
                    coverage: PublishedCoverageProjection {
                        state: completeness_name(relation.coverage.state),
                        requested_units: relation.coverage.requested_units,
                        completed_units: relation.coverage.completed_units,
                        remainder_units: relation.coverage.remainder_units,
                        unknown_cause: relation.coverage.unknown_cause.clone(),
                    },
                })
                .collect(),
        }
    }
}

/// Credentials and owner identity presented for a result operation.
#[derive(Clone, Copy, Debug)]
pub struct PublishedResultAccess {
    pub artifact_id: PublishedArtifactId,
    pub owner: PublishedResultOwner,
    pub lease_token: OpaqueResultLeaseToken,
}

/// One bounded read against an owner-bound relation resource.
#[derive(Clone, Copy, Debug)]
pub struct PublishedResultReadRequest {
    pub access: PublishedResultAccess,
    pub resource_id: PublishedResultResourceId,
    pub observed_at_unix_ms: i64,
    pub offset: u64,
    pub max_bytes: usize,
}

/// Public chunk whose identity remains owner-bound while bytes/checksum come from the package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedResultChunk {
    pub resource_id: PublishedResultResourceId,
    pub media_type: &'static str,
    pub offset: u64,
    pub next_offset: u64,
    pub total_length: u64,
    pub content_checksum: [u8; 32],
    pub bytes: Arc<[u8]>,
    pub complete: bool,
    pub lease_expires_at_unix_ms: i64,
}

/// Idempotent public release outcome. Reads still distinguish a released tombstone as failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishedReleaseOutcome {
    Released,
    AlreadyReleased,
}

/// Immutable package registry with bounded, explicit lifecycle state.
#[derive(Debug, Default)]
pub struct PublishedArrowResultRegistry {
    entries: Mutex<BTreeMap<PublishedArtifactId, PublishedEntry>>,
}

#[derive(Debug)]
struct PublishedEntry {
    owner: PublishedResultOwner,
    lease_token: OpaqueResultLeaseToken,
    internal_lease_id: LeaseId,
    package: Arc<ArrowResultResourcePackage>,
    epoch_lease: Option<FabricQueryLease>,
    resource_lease: Option<EpochResultLeasePermit>,
    descriptor: PublishedArrowResultDescriptor,
    resources: BTreeMap<PublishedResultResourceId, ResultResourceId>,
}

impl PublishedArrowResultRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// Publish one package under an exact owner and opaque serving token.
    ///
    /// # Errors
    ///
    /// Rejects an invalid token, epoch-pin disagreement, publication outside the package lease
    /// window, public identity collision, or unavailable registry state.
    pub fn publish(
        &self,
        owner: PublishedResultOwner,
        lease_token: OpaqueResultLeaseToken,
        epoch_lease: FabricQueryLease,
        resource_lease: EpochResultLeasePermit,
        package: Arc<ArrowResultResourcePackage>,
        observed_at_unix_ms: i64,
    ) -> Result<PublishedArrowResultDescriptor, PublishedResultRegistryError> {
        let metadata = package.metadata();
        if metadata.epoch_id() != epoch_lease.epoch_id() {
            return Err(PublishedResultRegistryError::EpochPinMismatch {
                package: metadata.epoch_id(),
                lease: epoch_lease.epoch_id(),
            });
        }
        let lease = package.lease();
        if resource_lease.epoch_id() != metadata.epoch_id() {
            return Err(PublishedResultRegistryError::ResourcePermitEpochMismatch);
        }
        if resource_lease.principal_id() != owner.agent_id() {
            return Err(PublishedResultRegistryError::ResourcePermitOwnerMismatch);
        }
        if resource_lease.lease_id() != lease.lease_id() {
            return Err(PublishedResultRegistryError::ResourcePermitLeaseMismatch);
        }
        if observed_at_unix_ms < lease.issued_at_unix_ms()
            || observed_at_unix_ms >= lease.expires_at_unix_ms()
        {
            return Err(PublishedResultRegistryError::PublicationOutsideLease);
        }

        let artifact_id = PublishedArtifactId(owner_bound_identity(
            b"published-artifact.v1",
            owner,
            metadata.package_id().as_bytes(),
        ));
        let package_id = PublishedPackageId(owner_bound_identity(
            b"published-package.v1",
            owner,
            metadata.package_id().as_bytes(),
        ));
        let mut resources = BTreeMap::new();
        let manifest_authorization_id = PublishedResultResourceId(owner_bound_identity(
            b"published-manifest-resource.v1",
            owner,
            metadata.manifest_resource_id().as_bytes(),
        ));
        resources.insert(manifest_authorization_id, metadata.manifest_resource_id());
        let mut relations = Vec::with_capacity(metadata.relations().len());
        for relation in metadata.relations() {
            let public_resource_id = PublishedResultResourceId(owner_bound_identity(
                b"published-relation-resource.v1",
                owner,
                relation.resource_id().as_bytes(),
            ));
            if resources
                .insert(public_resource_id, relation.resource_id())
                .is_some()
            {
                return Err(PublishedResultRegistryError::PublicIdentityCollision);
            }
            let coverage = relation.coverage();
            relations.push(PublishedRelationDescriptor {
                relation_id: relation.relation_id().clone(),
                authorization_resource_id: public_resource_id,
                content_resource_id: relation.resource_id(),
                media_type: ARROW_STREAM_MEDIA_TYPE,
                schema_checksum: *relation.schema_checksum(),
                schema_byte_length: relation.schema_byte_length(),
                content_checksum: *relation.content_checksum(),
                row_count: relation.row_count(),
                batch_count: relation.batch_count(),
                byte_length: relation.byte_length(),
                coverage: PublishedResultCoverage {
                    state: coverage.state(),
                    requested_units: coverage.requested_units(),
                    completed_units: coverage.completed_units(),
                    remainder_units: coverage.remainder_units(),
                    unknown_cause: coverage
                        .unknown_cause()
                        .map(|cause| cause.as_str().to_owned()),
                },
            });
        }
        let descriptor = PublishedArrowResultDescriptor {
            format: PUBLISHED_RESULT_FORMAT,
            artifact_id,
            package_id,
            content_package_id: metadata.package_id(),
            owner,
            epoch_id: metadata.epoch_id(),
            query_execution: metadata.query_execution(),
            source_manifest_checksum: *metadata.manifest_checksum(),
            source_manifest_byte_length: metadata.manifest_byte_length(),
            completion: metadata.completion(),
            total_rows: metadata.total_rows(),
            total_batches: metadata.total_batches(),
            total_schema_bytes: metadata.total_schema_bytes(),
            total_ipc_bytes: metadata.total_ipc_bytes(),
            lease_expires_at_unix_ms: lease.expires_at_unix_ms(),
            manifest: PublishedManifestDescriptor {
                authorization_resource_id: manifest_authorization_id,
                content_resource_id: metadata.manifest_resource_id(),
                media_type: CANONICAL_JSON_MEDIA_TYPE,
                content_checksum: *metadata.manifest_checksum(),
                byte_length: metadata.manifest_byte_length(),
            },
            relations: Arc::from(relations),
        };
        let entry = PublishedEntry {
            owner,
            lease_token,
            internal_lease_id: lease.lease_id(),
            package,
            epoch_lease: Some(epoch_lease),
            resource_lease: Some(resource_lease),
            descriptor: descriptor.clone(),
            resources,
        };
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| PublishedResultRegistryError::RegistryStateUnavailable)?;
        if entries.contains_key(&artifact_id) {
            return Err(PublishedResultRegistryError::ArtifactIdentityCollision(
                artifact_id,
            ));
        }
        entries.insert(artifact_id, entry);
        Ok(descriptor)
    }

    /// Read one exact range after owner, token, lifetime, and public resource authorization.
    pub fn read_chunk(
        &self,
        request: PublishedResultReadRequest,
    ) -> Result<PublishedResultChunk, PublishedResultRegistryError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| PublishedResultRegistryError::RegistryStateUnavailable)?;
        let entry = entries.get(&request.access.artifact_id).ok_or(
            PublishedResultRegistryError::UnknownArtifact(request.access.artifact_id),
        )?;
        authorize_entry(entry, request.access)?;
        validate_live_entry(entry, request.observed_at_unix_ms)?;
        if entry.epoch_lease.is_none() || entry.resource_lease.is_none() {
            return Err(PublishedResultRegistryError::Released);
        }
        let internal_resource_id = *entry.resources.get(&request.resource_id).ok_or(
            PublishedResultRegistryError::UnknownResource(request.resource_id),
        )?;
        let media_type =
            if request.resource_id == entry.descriptor.manifest.authorization_resource_id {
                entry.descriptor.manifest.media_type
            } else {
                entry
                    .descriptor
                    .relations
                    .iter()
                    .find(|relation| relation.authorization_resource_id == request.resource_id)
                    .map_or(ARROW_STREAM_MEDIA_TYPE, |relation| relation.media_type)
            };
        let chunk = entry
            .package
            .read_chunk(
                entry.internal_lease_id,
                request.observed_at_unix_ms,
                internal_resource_id,
                request.offset,
                request.max_bytes,
            )
            .map_err(translate_package_error)?;
        Ok(PublishedResultChunk {
            resource_id: request.resource_id,
            media_type,
            offset: chunk.offset,
            next_offset: chunk.next_offset,
            total_length: chunk.total_length,
            content_checksum: chunk.content_checksum,
            bytes: chunk.bytes,
            complete: chunk.complete,
            lease_expires_at_unix_ms: entry.descriptor.lease_expires_at_unix_ms,
        })
    }

    /// Release the result while retaining an owner/token-checked tombstone until expiry.
    pub fn release(
        &self,
        access: PublishedResultAccess,
        observed_at_unix_ms: i64,
    ) -> Result<PublishedReleaseOutcome, PublishedResultRegistryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| PublishedResultRegistryError::RegistryStateUnavailable)?;
        let entry = entries.get_mut(&access.artifact_id).ok_or(
            PublishedResultRegistryError::UnknownArtifact(access.artifact_id),
        )?;
        authorize_entry(entry, access)?;
        validate_live_entry(entry, observed_at_unix_ms)?;
        if entry.epoch_lease.is_none() {
            return Ok(PublishedReleaseOutcome::AlreadyReleased);
        }
        match entry
            .package
            .release(entry.internal_lease_id, observed_at_unix_ms)
        {
            Ok(()) => {
                entry.epoch_lease.take();
                entry.resource_lease.take();
                Ok(PublishedReleaseOutcome::Released)
            }
            Err(ArrowResultResourceError::Released) => {
                entry.epoch_lease.take();
                entry.resource_lease.take();
                Ok(PublishedReleaseOutcome::AlreadyReleased)
            }
            Err(error) => Err(translate_package_error(error)),
        }
    }

    /// Remove every expired live entry or released tombstone and drop any remaining epoch pins.
    ///
    /// Returns the exact number removed.
    pub fn collect_expired(
        &self,
        observed_at_unix_ms: i64,
    ) -> Result<usize, PublishedResultRegistryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| PublishedResultRegistryError::RegistryStateUnavailable)?;
        let before = entries.len();
        entries.retain(|_, entry| entry.descriptor.lease_expires_at_unix_ms > observed_at_unix_ms);
        Ok(before - entries.len())
    }
}

fn authorize_entry(
    entry: &PublishedEntry,
    access: PublishedResultAccess,
) -> Result<(), PublishedResultRegistryError> {
    if entry.owner != access.owner {
        return Err(PublishedResultRegistryError::WrongOwner);
    }
    if !constant_time_equal(entry.lease_token.as_bytes(), access.lease_token.as_bytes()) {
        return Err(PublishedResultRegistryError::WrongOpaqueToken);
    }
    Ok(())
}

fn validate_live_entry(
    entry: &PublishedEntry,
    observed_at_unix_ms: i64,
) -> Result<(), PublishedResultRegistryError> {
    let lease = entry.package.lease();
    if observed_at_unix_ms < lease.issued_at_unix_ms()
        || observed_at_unix_ms >= lease.expires_at_unix_ms()
    {
        return Err(PublishedResultRegistryError::Expired);
    }
    Ok(())
}

fn translate_package_error(error: ArrowResultResourceError) -> PublishedResultRegistryError {
    match error {
        ArrowResultResourceError::Released => PublishedResultRegistryError::Released,
        ArrowResultResourceError::Expired => PublishedResultRegistryError::Expired,
        ArrowResultResourceError::WrongLease | ArrowResultResourceError::UnknownResource(_) => {
            PublishedResultRegistryError::InternalPackageInvariant(error)
        }
        other => PublishedResultRegistryError::Package(other),
    }
}

fn owner_bound_identity(
    domain: &[u8],
    owner: PublishedResultOwner,
    internal_id: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    update_framed(&mut hasher, domain);
    update_framed(&mut hasher, owner.workspace_id.as_bytes());
    update_framed(&mut hasher, owner.agent_id.as_bytes());
    update_framed(&mut hasher, internal_id);
    *hasher.finalize().as_bytes()
}

fn update_framed(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

const fn all_zero(value: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn framed_id(bytes: &[u8]) -> String {
    format!("b3:{}", hex(bytes))
}

const fn completeness_name(value: ResultCompleteness) -> &'static str {
    match value {
        ResultCompleteness::Complete => "complete",
        ResultCompleteness::Partial => "partial",
        ResultCompleteness::Unknown => "unknown",
    }
}

fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut decoded = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_nibble(pair[0])?;
        let low = decode_nibble(pair[1])?;
        decoded[index] = (high << 4) | low;
    }
    Some(decoded)
}

const fn decode_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Stable failures at the owner-bound publication boundary.
#[derive(Debug, thiserror::Error)]
pub enum PublishedResultRegistryError {
    #[error("INVALID_REQUEST_SCHEMA:RESULT_PUBLIC_IDENTITY")]
    InvalidPublicIdentity,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_OPAQUE_TOKEN")]
    InvalidOpaqueToken,
    #[error("RESULT_EPOCH_PIN_MISMATCH:package={package:?}:lease={lease:?}")]
    EpochPinMismatch { package: EpochId, lease: EpochId },
    #[error("RESULT_PUBLICATION_OUTSIDE_LEASE")]
    PublicationOutsideLease,
    #[error("RESULT_RESOURCE_PERMIT_EPOCH_MISMATCH")]
    ResourcePermitEpochMismatch,
    #[error("RESULT_RESOURCE_PERMIT_OWNER_MISMATCH")]
    ResourcePermitOwnerMismatch,
    #[error("RESULT_RESOURCE_PERMIT_LEASE_MISMATCH")]
    ResourcePermitLeaseMismatch,
    #[error("RESULT_PUBLIC_IDENTITY_COLLISION")]
    PublicIdentityCollision,
    #[error("RESULT_ARTIFACT_IDENTITY_COLLISION:{0:?}")]
    ArtifactIdentityCollision(PublishedArtifactId),
    #[error("RESULT_ARTIFACT_UNKNOWN:{0:?}")]
    UnknownArtifact(PublishedArtifactId),
    #[error("RESULT_RESOURCE_UNKNOWN:{0:?}")]
    UnknownResource(PublishedResultResourceId),
    #[error("RESULT_RESOURCE_WRONG_OWNER")]
    WrongOwner,
    #[error("RESULT_RESOURCE_WRONG_OPAQUE_TOKEN")]
    WrongOpaqueToken,
    #[error("RESULT_RESOURCE_RELEASED")]
    Released,
    #[error("RESULT_RESOURCE_EXPIRED")]
    Expired,
    #[error("INTERNAL_INVARIANT_VIOLATION:PUBLISHED_RESULT_REGISTRY_STATE")]
    RegistryStateUnavailable,
    #[error("INTERNAL_INVARIANT_VIOLATION:PUBLISHED_RESULT_DESCRIPTOR:{0}")]
    CanonicalDescriptor(#[source] serde_json::Error),
    #[error("INTERNAL_INVARIANT_VIOLATION:PUBLISHED_RESULT_PACKAGE:{0}")]
    InternalPackageInvariant(#[source] ArrowResultResourceError),
    #[error(transparent)]
    Package(#[from] ArrowResultResourceError),
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use arrow_array::{RecordBatch, StringArray};
    use arrow_ipc::reader::StreamReader;
    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::fabric::activation::{
        ActivationAttempt, ActivationChain, ActivationCommit, ActivationEvent, ActivationEventId,
        ActivationOrdinal, ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef,
        FabricEpochPins, OverlaySegmentSetRef, PolicySetRef, TableVersionSetRef,
    };
    use crate::fabric::admission::FabricAdmissionRuntime;
    use crate::fabric::arrow_result_resource::{
        ArrowResultResourceLimits, QueryExecutionPin, ResultCoverage, ResultRelationInput,
        ResultResourceLease,
    };
    use crate::fabric::child_session::ChildResourceLimits;
    use crate::fabric::child_session::resource_governance::{
        EpochResourceCoordinator, EpochResourceError, EpochResourcePolicy,
        test_lifecycle_work_class_policies,
    };
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins, ExecutionOwner,
        ExpectedHead, FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef,
        OperationId, OperationSelectionRef, ProgramReleaseRef, ProofReceiptRef, ProviderSetRef,
        ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration, TransactionRef, WriterFence,
        WriterGeneration,
    };
    use crate::fabric::epoch_runtime::FabricEpochRuntimeConfig;
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn command(
        operation_seed: u8,
        workspace: WorkspaceId,
        predecessor: ExpectedHead,
        target: EpochId,
        generation: u64,
    ) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(operation_seed)),
                idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed)),
            },
            ownership: CommandOwnership {
                workspace_id: workspace,
                principal_id: PrincipalId::from_bytes(id16(2)),
                authorization: AuthorizationRef::from_bytes(id32(3)),
            },
            expected_head: predecessor,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(4)),
                generation: WriterGeneration::new(generation).unwrap(),
            },
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(id32(5)),
                program_release: ProgramReleaseRef::from_bytes(id32(6)),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    id32(6),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(6)),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(6)),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes(id32(8)),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(9)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: target,
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
        }
    }

    fn activation_event(
        event_seed: u8,
        command: &FabricCommand,
        predecessor_event_id: Option<ActivationEventId>,
        ordinal: u64,
        target: EpochId,
    ) -> ActivationEvent {
        ActivationEvent::try_from_attempt(
            ActivationEventId::from_bytes(id32(event_seed)),
            ActivationAttempt::for_test(
                *command,
                1,
                ExecutionOwner {
                    actor_id: ActorId::from_bytes(id16(33)),
                    fence: command.writer_fence,
                },
            ),
            predecessor_event_id,
            ActivationOrdinal::new(ordinal).unwrap(),
            FabricEpochPins {
                epoch: target,
                input_release: command.pins.input_release,
                program_release: command.pins.program_release,
                application_release: command.pins.application_release,
                source_authority: command.pins.source_authority,
                provider_release: command.pins.provider_release,
                source_generation: command.pins.source_generation,
                provider_set: command.pins.provider_set,
                table_versions: TableVersionSetRef::from_bytes(id32(11)),
                overlay_segments: OverlaySegmentSetRef::from_bytes(id32(12)),
                policy_set: PolicySetRef::from_bytes(id32(13)),
                resource_envelope: command.resources,
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            ActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(event_seed + 30)),
                transaction: TransactionRef::from_bytes(id32(event_seed + 60)),
                backend_commit: BackendCommitRef::from_bytes(id32(event_seed + 90)),
                readback: ActivationReadbackRef::from_bytes(id32(event_seed + 120)),
            },
        )
        .unwrap()
    }

    async fn epoch(epoch_id: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let config = FabricEpochRuntimeConfig::default();
        Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, config)
                .unwrap()
                .seal_for_test()
                .await
                .unwrap(),
        )
    }

    fn package(epoch_id: EpochId, lease_seed: u8) -> Arc<ArrowResultResourcePackage> {
        let schema = Arc::new(Schema::new(vec![Field::new("name", DataType::Utf8, false)]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(StringArray::from(vec!["Ada", "Grace", "Barbara"]))],
        )
        .unwrap();
        let relation = ResultRelationInput::new(
            RelationId::new("public.people").unwrap(),
            schema,
            vec![batch],
            ResultCoverage::complete(3),
        );
        Arc::new(
            ArrowResultResourcePackage::try_new(
                epoch_id,
                QueryExecutionPin::from_bytes(id32(0x55)),
                vec![relation],
                ResultResourceLease::try_new(LeaseId::from_bytes(id16(lease_seed)), 1_000, 2_000)
                    .unwrap(),
                ArrowResultResourceLimits::try_new(
                    4,
                    4,
                    100,
                    8,
                    200,
                    4 * 1024,
                    8 * 1024,
                    128 * 1024,
                    256 * 1024,
                    64 * 1024,
                    17,
                )
                .unwrap(),
            )
            .unwrap(),
        )
    }

    fn owner(workspace: WorkspaceId, agent_seed: u8) -> PublishedResultOwner {
        PublishedResultOwner::new(workspace, PrincipalId::from_bytes(id16(agent_seed)))
    }

    fn resource_coordinator(epoch_id: EpochId) -> EpochResourceCoordinator {
        EpochResourceCoordinator::try_new(
            epoch_id,
            id32(0x33),
            EpochResourcePolicy::try_new(
                ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1)
                    .unwrap(),
                test_lifecycle_work_class_policies(),
                4,
                1,
                8,
                30_000,
                1,
                2,
                8,
                64 * 1024 * 1024,
                60_000,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn publish_governed(
        registry: &PublishedArrowResultRegistry,
        result_owner: PublishedResultOwner,
        lease_token: OpaqueResultLeaseToken,
        epoch_lease: FabricQueryLease,
        package: Arc<ArrowResultResourcePackage>,
        observed_at_unix_ms: i64,
    ) -> Result<PublishedArrowResultDescriptor, PublishedResultRegistryError> {
        let resources = resource_coordinator(package.metadata().epoch_id());
        let resource_lease = resources
            .retain_result(
                result_owner.agent_id(),
                package.lease(),
                &package,
                observed_at_unix_ms,
            )
            .unwrap();
        registry.publish(
            result_owner,
            lease_token,
            epoch_lease,
            resource_lease,
            package,
            observed_at_unix_ms,
        )
    }

    fn token(seed: u8) -> OpaqueResultLeaseToken {
        OpaqueResultLeaseToken::try_from_bytes(id32(seed)).unwrap()
    }

    fn read_all(
        registry: &PublishedArrowResultRegistry,
        access: PublishedResultAccess,
        resource_id: PublishedResultResourceId,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut offset = 0;
        loop {
            let chunk = registry
                .read_chunk(PublishedResultReadRequest {
                    access,
                    resource_id,
                    observed_at_unix_ms: 1_500,
                    offset,
                    max_bytes: 17,
                })
                .unwrap();
            assert_eq!(chunk.offset, offset);
            bytes.extend_from_slice(&chunk.bytes);
            offset = chunk.next_offset;
            if chunk.complete {
                assert_eq!(offset, chunk.total_length);
                break;
            }
        }
        bytes
    }

    #[tokio::test]
    async fn descriptor_maps_owner_authorization_handles_to_content_identities() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch_id = EpochId::from_bytes(id16(20));
        let first_epoch = epoch(epoch_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, epoch_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, epoch_id);
        let chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let runtime =
            FabricAdmissionRuntime::recover(&chain, |_| Some(Arc::clone(&first_epoch))).unwrap();
        let admitted = runtime.admit_selected(Arc::clone(&first_epoch)).unwrap();
        let first_package = package(epoch_id, 0x61);
        let result_owner = owner(workspace, 0x31);
        let registry = PublishedArrowResultRegistry::new();
        let descriptor = publish_governed(
            &registry,
            result_owner,
            token(0x71),
            admitted.clone(),
            Arc::clone(&first_package),
            1_500,
        )
        .unwrap();

        assert_ne!(
            descriptor.package_id.as_bytes(),
            descriptor.content_package_id.as_bytes()
        );
        assert_eq!(
            descriptor.manifest.content_resource_id,
            first_package.metadata().manifest_resource_id()
        );
        assert_ne!(
            descriptor.manifest.authorization_resource_id.as_bytes(),
            descriptor.manifest.content_resource_id.as_bytes()
        );
        assert_eq!(
            descriptor.relations[0].content_resource_id,
            first_package.metadata().relations()[0].resource_id()
        );
        assert_ne!(
            descriptor.relations[0].authorization_resource_id.as_bytes(),
            descriptor.relations[0].content_resource_id.as_bytes()
        );
        let control_bytes = descriptor.canonical_control_bytes().unwrap();
        let control: serde_json::Value = serde_json::from_slice(&control_bytes).unwrap();
        assert_eq!(
            serde_json_canonicalizer::to_vec(&control).unwrap(),
            control_bytes
        );
        assert_eq!(control["format"], PUBLISHED_RESULT_FORMAT);
        assert_eq!(control["artifact_id"], descriptor.artifact_id.public_id());
        assert_eq!(
            control["content_package_id"],
            descriptor.content_package_id.public_id()
        );
        assert_eq!(control["owner"]["workspace_id"], hex(workspace.as_bytes()));
        assert_eq!(control["owner"]["agent_id"], hex(id16(0x31).as_slice()));
        assert_eq!(control["relations"][0]["relation_id"], "public.people");
        assert_eq!(
            PublishedArtifactId::try_from_public_id(&descriptor.artifact_id.public_id()).unwrap(),
            descriptor.artifact_id
        );
        assert_eq!(
            PublishedResultOwner::try_from_public_ids(
                control["owner"]["workspace_id"].as_str().unwrap(),
                control["owner"]["agent_id"].as_str().unwrap(),
            )
            .unwrap(),
            result_owner
        );
        let opaque_token = token(0x71);
        let event = crate::query_service::published_arrow_artifact_ready_event(
            crate::rpc::generated::codefabric::cpgd::v1::QueryEventHeader {
                daemon_query_id: "query:arrow-transport".to_owned(),
                sequence: 2,
                snapshot_id: Some("epoch:arrow-transport".to_owned()),
                event_at_unix_ms: 1_500,
                event_checksum: String::new(),
            },
            &descriptor,
            &opaque_token,
        )
        .unwrap();
        assert_eq!(event.artifact_id, descriptor.artifact_id.public_id());
        assert_eq!(event.canonical_result_descriptor_json, control_bytes);
        assert_eq!(
            event.result_descriptor_checksum,
            crate::integrity::framed_digest(&event.canonical_result_descriptor_json)
        );
        assert_eq!(event.artifact_checksum, event.result_descriptor_checksum);
        assert_eq!(event.result_contract_version, PUBLISHED_RESULT_FORMAT);
        assert_eq!(
            event.arrow_release,
            crate::fabric::arrow_result_resource::ARROW_RELEASE
        );
        assert_eq!(event.lease_token, opaque_token.public_token());
        assert_eq!(
            OpaqueResultLeaseToken::try_from_public_token(&opaque_token.public_token()).unwrap(),
            opaque_token
        );
        assert!(PublishedArtifactId::try_from_public_id("b3:NOT-LOWER-HEX").is_err());

        let second_registry = PublishedArrowResultRegistry::new();
        let rebuilt = publish_governed(
            &second_registry,
            result_owner,
            token(0x72),
            admitted.clone(),
            package(epoch_id, 0x62),
            1_500,
        )
        .unwrap();
        assert_eq!(descriptor, rebuilt);

        let other_registry = PublishedArrowResultRegistry::new();
        let other_owner = publish_governed(
            &other_registry,
            owner(workspace, 0x32),
            token(0x73),
            admitted,
            package(epoch_id, 0x63),
            1_500,
        )
        .unwrap();
        assert_eq!(
            descriptor.content_package_id,
            other_owner.content_package_id
        );
        assert_ne!(descriptor.package_id, other_owner.package_id);
        assert_ne!(
            descriptor.relations[0].authorization_resource_id,
            other_owner.relations[0].authorization_resource_id
        );
    }

    #[tokio::test]
    async fn owner_token_resource_release_tombstone_and_expiry_are_explicit() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch_id = EpochId::from_bytes(id16(20));
        let first_epoch = epoch(epoch_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, epoch_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, epoch_id);
        let chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let runtime =
            FabricAdmissionRuntime::recover(&chain, |_| Some(Arc::clone(&first_epoch))).unwrap();
        let registry = PublishedArrowResultRegistry::new();
        let result_owner = owner(workspace, 0x31);
        let lease_token = token(0x71);
        let descriptor = publish_governed(
            &registry,
            result_owner,
            lease_token,
            runtime.admit_selected(Arc::clone(&first_epoch)).unwrap(),
            package(epoch_id, 0x61),
            1_500,
        )
        .unwrap();
        let access = PublishedResultAccess {
            artifact_id: descriptor.artifact_id,
            owner: result_owner,
            lease_token,
        };
        let relation_resource = descriptor.relations[0].authorization_resource_id;

        let request = |access, resource_id| PublishedResultReadRequest {
            access,
            resource_id,
            observed_at_unix_ms: 1_500,
            offset: 0,
            max_bytes: 17,
        };
        assert!(matches!(
            registry.read_chunk(request(
                PublishedResultAccess {
                    owner: owner(workspace, 0x32),
                    ..access
                },
                relation_resource,
            )),
            Err(PublishedResultRegistryError::WrongOwner)
        ));
        assert!(matches!(
            registry.read_chunk(request(
                PublishedResultAccess {
                    lease_token: token(0x72),
                    ..access
                },
                relation_resource,
            )),
            Err(PublishedResultRegistryError::WrongOpaqueToken)
        ));
        assert!(matches!(
            registry.read_chunk(request(access, PublishedResultResourceId(id32(0x99)),)),
            Err(PublishedResultRegistryError::UnknownResource(_))
        ));

        let manifest = read_all(
            &registry,
            access,
            descriptor.manifest.authorization_resource_id,
        );
        assert_eq!(manifest.len() as u64, descriptor.manifest.byte_length);
        assert!(serde_json::from_slice::<serde_json::Value>(&manifest).is_ok());
        let relation = read_all(&registry, access, relation_resource);
        let reader = StreamReader::try_new(Cursor::new(relation), None).unwrap();
        let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 3);

        assert_eq!(
            registry.release(access, 1_500).unwrap(),
            PublishedReleaseOutcome::Released
        );
        assert_eq!(
            registry.release(access, 1_500).unwrap(),
            PublishedReleaseOutcome::AlreadyReleased
        );
        assert!(matches!(
            registry.read_chunk(request(access, relation_resource)),
            Err(PublishedResultRegistryError::Released)
        ));
        assert_eq!(registry.collect_expired(1_999).unwrap(), 0);
        assert_eq!(registry.entries.lock().unwrap().len(), 1);
        assert_eq!(registry.collect_expired(2_000).unwrap(), 1);
        assert!(matches!(
            registry.read_chunk(request(access, relation_resource)),
            Err(PublishedResultRegistryError::UnknownArtifact(_))
        ));

        let expired = publish_governed(
            &registry,
            result_owner,
            token(0x73),
            runtime.admit_selected(Arc::clone(&first_epoch)).unwrap(),
            package(epoch_id, 0x62),
            1_500,
        )
        .unwrap();
        assert!(matches!(
            registry.read_chunk(PublishedResultReadRequest {
                access: PublishedResultAccess {
                    artifact_id: expired.artifact_id,
                    owner: result_owner,
                    lease_token: token(0x73),
                },
                resource_id: expired.relations[0].authorization_resource_id,
                observed_at_unix_ms: 2_000,
                offset: 0,
                max_bytes: 17,
            }),
            Err(PublishedResultRegistryError::Expired)
        ));
    }

    #[tokio::test]
    async fn result_release_returns_epoch_capacity_after_causal_backpressure() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch_id = EpochId::from_bytes(id16(20));
        let fabric_epoch = epoch(epoch_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, epoch_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, epoch_id);
        let chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let admission =
            FabricAdmissionRuntime::recover(&chain, |_| Some(Arc::clone(&fabric_epoch))).unwrap();
        let resources = EpochResourceCoordinator::try_new(
            epoch_id,
            id32(0x33),
            EpochResourcePolicy::try_new(
                ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1)
                    .unwrap(),
                test_lifecycle_work_class_policies(),
                4,
                1,
                8,
                30_000,
                1,
                2,
                1,
                64 * 1024 * 1024,
                60_000,
            )
            .unwrap(),
        )
        .unwrap();
        let registry = PublishedArrowResultRegistry::new();
        let result_owner = owner(workspace, 0x31);
        let first_package = package(epoch_id, 0x61);
        let retained_bytes = first_package.retained_resource_bytes().unwrap();
        let resource_lease = resources
            .retain_result(
                result_owner.agent_id(),
                first_package.lease(),
                &first_package,
                1_500,
            )
            .unwrap();
        let descriptor = registry
            .publish(
                result_owner,
                token(0x71),
                admission.admit_selected(Arc::clone(&fabric_epoch)).unwrap(),
                resource_lease,
                first_package,
                1_500,
            )
            .unwrap();
        let observation = resources.observation().unwrap();
        assert_eq!(observation.live_result_leases, 1);
        assert_eq!(observation.retained_result_bytes, retained_bytes);

        let second_package = package(epoch_id, 0x62);
        assert!(matches!(
            resources.retain_result(
                result_owner.agent_id(),
                second_package.lease(),
                &second_package,
                1_500,
            ),
            Err(EpochResourceError::ResultLeaseBackpressure { live: 1, limit: 1 })
        ));

        assert_eq!(
            registry
                .release(
                    PublishedResultAccess {
                        artifact_id: descriptor.artifact_id,
                        owner: result_owner,
                        lease_token: token(0x71),
                    },
                    1_500,
                )
                .unwrap(),
            PublishedReleaseOutcome::Released
        );
        let observation = resources.observation().unwrap();
        assert_eq!(observation.live_result_leases, 0);
        assert_eq!(observation.retained_result_bytes, 0);
    }

    #[tokio::test]
    async fn registry_epoch_lease_pins_predecessor_across_activation_swap() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_id = EpochId::from_bytes(id16(20));
        let second_id = EpochId::from_bytes(id16(21));
        let first = epoch(first_id).await;
        let first_weak = Arc::downgrade(&first);
        let second = epoch(second_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, first_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, first_id);
        let first_chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let runtime =
            FabricAdmissionRuntime::recover(&first_chain, |_| Some(Arc::clone(&first))).unwrap();
        let registry = PublishedArrowResultRegistry::new();
        let result_owner = owner(workspace, 0x31);
        let lease_token = token(0x71);
        let descriptor = publish_governed(
            &registry,
            result_owner,
            lease_token,
            runtime.admit_selected(Arc::clone(&first)).unwrap(),
            package(first_id, 0x61),
            1_500,
        )
        .unwrap();

        let second_command = command(2, workspace, ExpectedHead::Epoch(first_id), second_id, 1);
        let barrier = runtime
            .close_admission(second_command.expected_head, second_command.writer_fence)
            .unwrap();
        let second_event = activation_event(
            2,
            &second_command,
            Some(first_event.event_id()),
            2,
            second_id,
        );
        let second_chain = ActivationChain::derive(workspace, [second_event, first_event]).unwrap();
        runtime
            .publish_selected_epoch(barrier, &second_chain, Arc::clone(&second))
            .unwrap();
        runtime
            .reopen_after_reconciliation(barrier, ExpectedHead::Epoch(second_id))
            .unwrap();
        drop(first);

        assert!(first_weak.upgrade().is_some());
        assert_eq!(
            runtime
                .admit_selected(Arc::clone(&second))
                .unwrap()
                .epoch_id(),
            second_id
        );
        registry
            .release(
                PublishedResultAccess {
                    artifact_id: descriptor.artifact_id,
                    owner: result_owner,
                    lease_token,
                },
                1_500,
            )
            .unwrap();
        assert!(first_weak.upgrade().is_none());
    }
}
