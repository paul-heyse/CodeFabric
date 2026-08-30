//! Immutable serving-snapshot manifest identities from the AC-G-19 CBEF contract.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::{
    CbefField, CbefRecord, CbefValue, IdentityDomain, IdentityError, StringNormalization,
    context_set_identity, decode_public_id, derive_identity, encode_public_id, encode_record,
};

/// Snapshot source observation frozen at construction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotSource {
    pub source_generation: u64,
    pub admitted_event_sequence: u64,
    pub reconciled_event_sequence: u64,
    pub inventory_digest: String,
    pub authorization_fingerprint: String,
    pub inclusion_policy_fingerprint: String,
    pub path_profile_version: String,
    pub source_trust_state: String,
    pub event_stream_health: String,
    pub git_acceleration_status: String,
    pub git_state_fingerprint: Option<String>,
}

/// One analysis-context identity in a serving snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotContextRecord {
    pub analysis_context_id: String,
    pub context_manifest_digest: String,
    pub capability_partition_digest: String,
}

/// Context selection frozen into a serving snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotContexts {
    pub context_set_id: String,
    pub default_python_context_id: Option<String>,
    pub default_rust_context_id: Option<String>,
    pub records: Vec<SnapshotContextRecord>,
}

/// One exact Delta table version in a durable publication.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBaseTable {
    pub table_code: u16,
    pub table_uri: String,
    pub delta_version: u64,
    pub schema_digest: String,
    pub row_count: u64,
    pub primary_key_digest: String,
    pub effective_content_digest: String,
}

/// Durable publication pinned by the snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBasePublication {
    pub publication_id: String,
    pub tables: Vec<SnapshotBaseTable>,
}

/// One immutable hot-overlay table manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotOverlayTable {
    pub table_code: u16,
    pub mutation_policy: String,
    pub replacement_row_count: u64,
    pub owner_tombstone_count: u64,
    pub key_tombstone_count: u64,
    pub table_replacement: bool,
    pub row_digest: String,
    pub tombstone_digest: String,
}

/// Consolidated hot-overlay identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotOverlay {
    pub overlay_generation: u64,
    pub overlay_digest: String,
    pub total_memory_bytes: u64,
    pub tables: Vec<SnapshotOverlayTable>,
}

/// Snapshot-scoped index identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotIndexes {
    pub capability_index_digest: String,
    pub diagnostic_index_digest: String,
    pub dependency_graph_digest: String,
}

/// Exact interpretation bundle identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBundles {
    pub ontology_bundle_id: String,
    pub schema_bundle_id: String,
    pub provider_bundle_id: String,
    pub derivation_bundle_id: String,
    pub query_language_bundle_id: String,
    pub model_pack_bundle_id: String,
    pub toolchain_bundle_id: String,
    /// Provider ID to exact generated containment profile digest.
    #[serde(default)]
    pub sandbox_profile_digests: BTreeMap<String, String>,
}

/// Versioned interpretation authority pinned by a serving snapshot and every derived lease.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultAuthorityPin {
    pub result_authority_identity: String,
    pub package_identity: String,
    pub epoch_runtime_authority_identity: String,
    pub program_identity: String,
    pub function_catalog_identity: String,
    pub policy_identity: String,
    pub query_form_identity: String,
    pub checksum_version: String,
    pub exact_table_set_identity: String,
}

impl ResultAuthorityPin {
    /// Rebind the data-dependent portion of a governed result authority to one exact provider
    /// set. Program, function, policy, query-form, and checksum authority remain unchanged.
    pub(crate) fn rebind_exact_table_set(&mut self, exact_table_set_identity: String) {
        self.exact_table_set_identity = exact_table_set_identity;
        self.result_authority_identity = governed_result_authority_identity(self);
    }
}

/// Recompute the content identity used by every governed result-authority producer.
pub(crate) fn governed_result_authority_identity(authority: &ResultAuthorityPin) -> String {
    let mut bytes = Vec::new();
    for part in [
        b"ontology-result-authority.v1".as_slice(),
        authority.package_identity.as_bytes(),
        authority.epoch_runtime_authority_identity.as_bytes(),
        authority.program_identity.as_bytes(),
        authority.function_catalog_identity.as_bytes(),
        authority.policy_identity.as_bytes(),
        authority.query_form_identity.as_bytes(),
        authority.checksum_version.as_bytes(),
        authority.exact_table_set_identity.as_bytes(),
    ] {
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

/// Canonical READY-snapshot bytes passed only between the trusted proof coordinator and the
/// durable activation kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StagedServingSnapshot {
    pub snapshot_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub publication_id: [u8; 16],
    pub manifest_body: Vec<u8>,
    pub manifest_json: Vec<u8>,
    pub manifest_digest: [u8; 32],
    pub result_authority: Option<ResultAuthorityPin>,
}

/// Immutable AC-G-19 manifest body; mutable activation observations are absent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServingSnapshotManifestBody {
    pub manifest_version: String,
    pub workspace_id: String,
    pub repository_id: Option<String>,
    pub worktree_id: Option<String>,
    pub registration_revision: u64,
    pub source: SnapshotSource,
    pub contexts: SnapshotContexts,
    pub base_publication: SnapshotBasePublication,
    pub overlay: SnapshotOverlay,
    pub indexes: SnapshotIndexes,
    pub bundles: SnapshotBundles,
    /// Absent only for deterministically decoded legacy manifests with no ontology epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_authority: Option<ResultAuthorityPin>,
    pub limits_profile_digest: String,
    /// Ordered unique source-image digests that make snapshot-to-bytes provenance resolvable.
    pub source_blob_digests: Vec<String>,
}

/// Content-addressed serving-snapshot manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServingSnapshotManifest {
    pub snapshot_id: String,
    #[serde(flatten)]
    pub body: ServingSnapshotManifestBody,
    pub manifest_digest: String,
}

/// Mutable activation state deliberately excluded from snapshot identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotActivationRecord {
    pub snapshot_id: String,
    pub created_at: String,
    pub activated_at: Option<String>,
    pub observed_durable_pointer_generation: u64,
    pub active_pointer_generation: u64,
    pub lease_count: u64,
}

/// Snapshot manifest validation or identity failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SnapshotManifestError {
    #[error("invalid serving-snapshot field: {0}")]
    InvalidField(&'static str),
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

impl ServingSnapshotManifest {
    /// Recompute and verify both immutable identities from the typed body.
    ///
    /// # Errors
    ///
    /// Rejects any malformed body field, digest, or public identity.
    pub fn validate(&self) -> Result<(), SnapshotManifestError> {
        let expected = self.body.clone().derive()?;
        if expected.snapshot_id != self.snapshot_id
            || expected.manifest_digest != self.manifest_digest
        {
            return Err(SnapshotManifestError::InvalidField(
                "snapshot_id or manifest_digest",
            ));
        }
        Ok(())
    }

    /// Decode the public snapshot identity to its durable 16-byte key.
    ///
    /// # Errors
    ///
    /// Rejects malformed or wrongly typed public identity text.
    pub fn raw_snapshot_id(&self) -> Result<[u8; 16], SnapshotManifestError> {
        decode_public_id(IdentityDomain::ServingSnapshot, None, &self.snapshot_id)
            .map_err(|_| SnapshotManifestError::InvalidField("snapshot_id"))
    }

    /// Decode the workspace identity bound by this manifest.
    ///
    /// # Errors
    ///
    /// Rejects malformed or wrongly typed public identity text.
    pub fn raw_workspace_id(&self) -> Result<[u8; 16], SnapshotManifestError> {
        decode_public_id(IdentityDomain::Workspace, None, &self.body.workspace_id)
            .map_err(|_| SnapshotManifestError::InvalidField("workspace_id"))
    }

    /// Decode the durable publication identity bound by this manifest.
    ///
    /// # Errors
    ///
    /// Rejects malformed or wrongly typed public identity text.
    pub fn raw_publication_id(&self) -> Result<[u8; 16], SnapshotManifestError> {
        decode_public_id(
            IdentityDomain::Publication,
            None,
            &self.body.base_publication.publication_id,
        )
        .map_err(|_| SnapshotManifestError::InvalidField("base_publication.publication_id"))
    }

    /// Decode the raw manifest digest after validating its framing.
    ///
    /// # Errors
    ///
    /// Rejects a digest without valid BLAKE3 framing and lowercase hex.
    pub fn raw_manifest_digest(&self) -> Result<[u8; 32], SnapshotManifestError> {
        decode_digest(&self.manifest_digest, "manifest_digest")
    }

    /// Decode and validate the ordered context membership bound by the context-set ID.
    ///
    /// # Errors
    ///
    /// Rejects an empty, unsorted, duplicate, malformed, or identity-inconsistent set.
    pub fn raw_analysis_context_ids(&self) -> Result<Vec<[u8; 16]>, SnapshotManifestError> {
        self.body.validated_context_ids()
    }

    /// Decode the ordered source-image digest set bound into snapshot identity.
    ///
    /// # Errors
    ///
    /// Rejects malformed, duplicate, or non-canonical digest order.
    pub fn raw_source_blob_digests(&self) -> Result<Vec<[u8; 32]>, SnapshotManifestError> {
        let digests = self
            .body
            .source_blob_digests
            .iter()
            .map(|value| decode_digest(value, "source_blob_digests"))
            .collect::<Result<Vec<_>, _>>()?;
        if !digests.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(SnapshotManifestError::InvalidField(
                "source_blob_digests order",
            ));
        }
        Ok(digests)
    }
}

fn text(value: &str) -> CbefValue {
    CbefValue::Utf8 {
        value: value.to_owned(),
        normalization: StringNormalization::None,
    }
}

fn unsigned(value: u64) -> CbefValue {
    CbefValue::Unsigned(value.to_be_bytes().to_vec())
}

fn map(entries: Vec<(&str, CbefValue)>) -> CbefValue {
    CbefValue::Map(
        entries
            .into_iter()
            .map(|(key, value)| (text(key), value))
            .collect(),
    )
}

fn optional(value: Option<CbefValue>) -> CbefValue {
    value.unwrap_or(CbefValue::Absent)
}

fn decode_digest(value: &str, field: &'static str) -> Result<[u8; 32], SnapshotManifestError> {
    let payload = value
        .strip_prefix("b3:")
        .or_else(|| value.strip_prefix("blake3:"))
        .ok_or(SnapshotManifestError::InvalidField(field))?;
    if payload.len() != 64
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(SnapshotManifestError::InvalidField(field));
    }
    let mut digest = [0; 32];
    for (index, pair) in payload.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        digest[index] = u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
            .map_err(|_| SnapshotManifestError::InvalidField(field))?;
    }
    Ok(digest)
}

fn digest(value: &str, field: &'static str) -> Result<CbefValue, SnapshotManifestError> {
    Ok(CbefValue::Digest(decode_digest(value, field)?))
}

fn sandbox_profile_digests(
    values: &BTreeMap<String, String>,
) -> Result<CbefValue, SnapshotManifestError> {
    values
        .iter()
        .map(|(provider_id, value)| {
            let payload = value
                .strip_prefix("sha256:")
                .or_else(|| value.strip_prefix("b3:"))
                .ok_or(SnapshotManifestError::InvalidField(
                    "bundles.sandbox_profile_digests",
                ))?;
            if provider_id.is_empty()
                || payload.len() != 64
                || !payload
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(SnapshotManifestError::InvalidField(
                    "bundles.sandbox_profile_digests",
                ));
            }
            Ok((text(provider_id), text(value)))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(CbefValue::Map)
}

fn public_id(
    value: &str,
    domain: IdentityDomain,
    field: &'static str,
) -> Result<CbefValue, SnapshotManifestError> {
    decode_public_id(domain, None, value)
        .map(CbefValue::Id)
        .map_err(|_| SnapshotManifestError::InvalidField(field))
}

fn context_record(record: &SnapshotContextRecord) -> Result<CbefValue, SnapshotManifestError> {
    Ok(map(vec![
        (
            "analysis_context_id",
            public_id(
                &record.analysis_context_id,
                IdentityDomain::AnalysisContext,
                "contexts.records.analysis_context_id",
            )?,
        ),
        (
            "context_manifest_digest",
            digest(
                &record.context_manifest_digest,
                "contexts.records.context_manifest_digest",
            )?,
        ),
        (
            "capability_partition_digest",
            digest(
                &record.capability_partition_digest,
                "contexts.records.capability_partition_digest",
            )?,
        ),
    ]))
}

fn base_table(table: &SnapshotBaseTable) -> Result<CbefValue, SnapshotManifestError> {
    Ok(map(vec![
        ("table_code", unsigned(u64::from(table.table_code))),
        ("table_uri", text(&table.table_uri)),
        ("delta_version", unsigned(table.delta_version)),
        (
            "schema_digest",
            digest(
                &table.schema_digest,
                "base_publication.tables.schema_digest",
            )?,
        ),
        ("row_count", unsigned(table.row_count)),
        (
            "primary_key_digest",
            digest(
                &table.primary_key_digest,
                "base_publication.tables.primary_key_digest",
            )?,
        ),
        (
            "effective_content_digest",
            digest(
                &table.effective_content_digest,
                "base_publication.tables.effective_content_digest",
            )?,
        ),
    ]))
}

fn overlay_table(table: &SnapshotOverlayTable) -> Result<CbefValue, SnapshotManifestError> {
    Ok(map(vec![
        ("table_code", unsigned(u64::from(table.table_code))),
        ("mutation_policy", text(&table.mutation_policy)),
        (
            "replacement_row_count",
            unsigned(table.replacement_row_count),
        ),
        (
            "owner_tombstone_count",
            unsigned(table.owner_tombstone_count),
        ),
        ("key_tombstone_count", unsigned(table.key_tombstone_count)),
        (
            "table_replacement",
            CbefValue::Boolean(table.table_replacement),
        ),
        (
            "row_digest",
            digest(&table.row_digest, "overlay.tables.row_digest")?,
        ),
        (
            "tombstone_digest",
            digest(&table.tombstone_digest, "overlay.tables.tombstone_digest")?,
        ),
    ]))
}

impl ServingSnapshotManifestBody {
    fn validated_context_ids(&self) -> Result<Vec<[u8; 16]>, SnapshotManifestError> {
        let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &self.workspace_id)
            .map_err(|_| SnapshotManifestError::InvalidField("workspace_id"))?;
        let context_set_id = decode_public_id(
            IdentityDomain::ContextSet,
            None,
            &self.contexts.context_set_id,
        )
        .map_err(|_| SnapshotManifestError::InvalidField("contexts.context_set_id"))?;
        let ids = self
            .contexts
            .records
            .iter()
            .map(|record| {
                decode_public_id(
                    IdentityDomain::AnalysisContext,
                    None,
                    &record.analysis_context_id,
                )
                .map_err(|_| {
                    SnapshotManifestError::InvalidField("contexts.records.analysis_context_id")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ids.is_empty() || !ids.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(SnapshotManifestError::InvalidField(
                "contexts.records membership order",
            ));
        }
        if context_set_identity(workspace_id, &ids)?.id != context_set_id {
            return Err(SnapshotManifestError::InvalidField(
                "contexts.context_set_id membership",
            ));
        }
        for default in [
            self.contexts.default_python_context_id.as_deref(),
            self.contexts.default_rust_context_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let id = decode_public_id(IdentityDomain::AnalysisContext, None, default)
                .map_err(|_| SnapshotManifestError::InvalidField("contexts.default_context"))?;
            if !ids.contains(&id) {
                return Err(SnapshotManifestError::InvalidField(
                    "contexts.default_context membership",
                ));
            }
        }
        Ok(ids)
    }

    /// Encode the exact immutable manifest body in the AC-G-19 field order.
    ///
    /// # Errors
    ///
    /// Returns a bounded field error for malformed IDs/digests or a CBEF error.
    #[allow(clippy::too_many_lines)] // AC-G-19 fixes one auditable top-level field order.
    pub fn canonical_body(&self) -> Result<Vec<u8>, SnapshotManifestError> {
        if !matches!(
            (
                self.manifest_version.as_str(),
                self.result_authority.as_ref()
            ),
            ("1.0", None) | ("2.0", Some(_))
        ) {
            return Err(SnapshotManifestError::InvalidField("manifest_version"));
        }
        self.validated_context_ids()?;
        let raw_source_blob_digests = self
            .source_blob_digests
            .iter()
            .map(|value| decode_digest(value, "source_blob_digests"))
            .collect::<Result<Vec<_>, _>>()?;
        if raw_source_blob_digests
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(SnapshotManifestError::InvalidField(
                "source_blob_digests order",
            ));
        }
        let source_blob_digests = raw_source_blob_digests
            .into_iter()
            .map(CbefValue::Digest)
            .collect();
        let source = map(vec![
            ("source_generation", unsigned(self.source.source_generation)),
            (
                "admitted_event_sequence",
                unsigned(self.source.admitted_event_sequence),
            ),
            (
                "reconciled_event_sequence",
                unsigned(self.source.reconciled_event_sequence),
            ),
            (
                "inventory_digest",
                digest(&self.source.inventory_digest, "source.inventory_digest")?,
            ),
            (
                "authorization_fingerprint",
                digest(
                    &self.source.authorization_fingerprint,
                    "source.authorization_fingerprint",
                )?,
            ),
            (
                "inclusion_policy_fingerprint",
                digest(
                    &self.source.inclusion_policy_fingerprint,
                    "source.inclusion_policy_fingerprint",
                )?,
            ),
            (
                "path_profile_version",
                text(&self.source.path_profile_version),
            ),
            ("source_trust_state", text(&self.source.source_trust_state)),
            (
                "event_stream_health",
                text(&self.source.event_stream_health),
            ),
            (
                "git_acceleration_status",
                text(&self.source.git_acceleration_status),
            ),
            (
                "git_state_fingerprint",
                optional(
                    self.source
                        .git_state_fingerprint
                        .as_deref()
                        .map(|value| digest(value, "source.git_state_fingerprint"))
                        .transpose()?,
                ),
            ),
        ]);
        let contexts = map(vec![
            (
                "context_set_id",
                public_id(
                    &self.contexts.context_set_id,
                    IdentityDomain::ContextSet,
                    "contexts.context_set_id",
                )?,
            ),
            (
                "default_python_context_id",
                optional(
                    self.contexts
                        .default_python_context_id
                        .as_deref()
                        .map(|value| {
                            public_id(
                                value,
                                IdentityDomain::AnalysisContext,
                                "contexts.default_python_context_id",
                            )
                        })
                        .transpose()?,
                ),
            ),
            (
                "default_rust_context_id",
                optional(
                    self.contexts
                        .default_rust_context_id
                        .as_deref()
                        .map(|value| {
                            public_id(
                                value,
                                IdentityDomain::AnalysisContext,
                                "contexts.default_rust_context_id",
                            )
                        })
                        .transpose()?,
                ),
            ),
            (
                "records",
                CbefValue::OrderedList(
                    self.contexts
                        .records
                        .iter()
                        .map(context_record)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ),
        ]);
        let base = map(vec![
            (
                "publication_id",
                public_id(
                    &self.base_publication.publication_id,
                    IdentityDomain::Publication,
                    "base_publication.publication_id",
                )?,
            ),
            (
                "tables",
                CbefValue::OrderedList(
                    self.base_publication
                        .tables
                        .iter()
                        .map(base_table)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ),
        ]);
        let overlay = map(vec![
            (
                "overlay_generation",
                unsigned(self.overlay.overlay_generation),
            ),
            (
                "overlay_digest",
                digest(&self.overlay.overlay_digest, "overlay.overlay_digest")?,
            ),
            (
                "total_memory_bytes",
                unsigned(self.overlay.total_memory_bytes),
            ),
            (
                "tables",
                CbefValue::OrderedList(
                    self.overlay
                        .tables
                        .iter()
                        .map(overlay_table)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ),
        ]);
        let indexes = map(vec![
            (
                "capability_index_digest",
                digest(
                    &self.indexes.capability_index_digest,
                    "indexes.capability_index_digest",
                )?,
            ),
            (
                "diagnostic_index_digest",
                digest(
                    &self.indexes.diagnostic_index_digest,
                    "indexes.diagnostic_index_digest",
                )?,
            ),
            (
                "dependency_graph_digest",
                digest(
                    &self.indexes.dependency_graph_digest,
                    "indexes.dependency_graph_digest",
                )?,
            ),
        ]);
        let bundles = map(vec![
            ("ontology_bundle_id", text(&self.bundles.ontology_bundle_id)),
            ("schema_bundle_id", text(&self.bundles.schema_bundle_id)),
            ("provider_bundle_id", text(&self.bundles.provider_bundle_id)),
            (
                "derivation_bundle_id",
                text(&self.bundles.derivation_bundle_id),
            ),
            (
                "query_language_bundle_id",
                text(&self.bundles.query_language_bundle_id),
            ),
            (
                "model_pack_bundle_id",
                text(&self.bundles.model_pack_bundle_id),
            ),
            (
                "toolchain_bundle_id",
                text(&self.bundles.toolchain_bundle_id),
            ),
            (
                "sandbox_profile_digests",
                sandbox_profile_digests(&self.bundles.sandbox_profile_digests)?,
            ),
        ]);
        let mut fields = vec![
            CbefField {
                tag: 1,
                value: text(&self.manifest_version),
            },
            CbefField {
                tag: 2,
                value: public_id(
                    &self.workspace_id,
                    IdentityDomain::Workspace,
                    "workspace_id",
                )?,
            },
            CbefField {
                tag: 3,
                value: optional(
                    self.repository_id
                        .as_deref()
                        .map(|value| public_id(value, IdentityDomain::Repository, "repository_id"))
                        .transpose()?,
                ),
            },
            CbefField {
                tag: 4,
                value: optional(
                    self.worktree_id
                        .as_deref()
                        .map(|value| public_id(value, IdentityDomain::Worktree, "worktree_id"))
                        .transpose()?,
                ),
            },
            CbefField {
                tag: 5,
                value: unsigned(self.registration_revision),
            },
            CbefField {
                tag: 6,
                value: source,
            },
            CbefField {
                tag: 7,
                value: contexts,
            },
            CbefField {
                tag: 8,
                value: base,
            },
            CbefField {
                tag: 9,
                value: overlay,
            },
            CbefField {
                tag: 10,
                value: indexes,
            },
            CbefField {
                tag: 11,
                value: bundles,
            },
            CbefField {
                tag: 12,
                value: digest(&self.limits_profile_digest, "limits_profile_digest")?,
            },
            CbefField {
                tag: 13,
                value: CbefValue::OrderedList(source_blob_digests),
            },
        ];
        if let Some(authority) = &self.result_authority {
            if authority.checksum_version != "ResultChecksumV1"
                && authority.checksum_version != "ResultChecksumV2"
            {
                return Err(SnapshotManifestError::InvalidField(
                    "result_authority.checksum_version",
                ));
            }
            fields.push(CbefField {
                tag: 14,
                value: map(vec![
                    (
                        "result_authority_identity",
                        digest(
                            &authority.result_authority_identity,
                            "result_authority.result_authority_identity",
                        )?,
                    ),
                    (
                        "package_identity",
                        digest(
                            &authority.package_identity,
                            "result_authority.package_identity",
                        )?,
                    ),
                    (
                        "epoch_runtime_authority_identity",
                        digest(
                            &authority.epoch_runtime_authority_identity,
                            "result_authority.epoch_runtime_authority_identity",
                        )?,
                    ),
                    (
                        "program_identity",
                        digest(
                            &authority.program_identity,
                            "result_authority.program_identity",
                        )?,
                    ),
                    (
                        "function_catalog_identity",
                        digest(
                            &authority.function_catalog_identity,
                            "result_authority.function_catalog_identity",
                        )?,
                    ),
                    (
                        "policy_identity",
                        digest(
                            &authority.policy_identity,
                            "result_authority.policy_identity",
                        )?,
                    ),
                    (
                        "query_form_identity",
                        digest(
                            &authority.query_form_identity,
                            "result_authority.query_form_identity",
                        )?,
                    ),
                    ("checksum_version", text(&authority.checksum_version)),
                    (
                        "exact_table_set_identity",
                        digest(
                            &authority.exact_table_set_identity,
                            "result_authority.exact_table_set_identity",
                        )?,
                    ),
                ]),
            });
        }
        Ok(encode_record(&CbefRecord {
            domain: IdentityDomain::ServingSnapshot,
            fields,
        })?)
    }

    /// Derive the manifest digest and public snapshot identity.
    ///
    /// # Errors
    ///
    /// Returns a field or CBEF validation error.
    pub fn derive(self) -> Result<ServingSnapshotManifest, SnapshotManifestError> {
        let body = self.canonical_body()?;
        let mut hasher = crate::integrity::IntegrityHasher::for_domain(
            crate::integrity::IntegrityDomain::ServingSnapshotManifest,
        );
        hasher.update(&body);
        let manifest_digest = hasher.finalize();
        let identity = derive_identity(&CbefRecord {
            domain: IdentityDomain::ServingSnapshot,
            fields: vec![CbefField {
                tag: 1,
                value: CbefValue::Digest(manifest_digest),
            }],
        })?;
        Ok(ServingSnapshotManifest {
            snapshot_id: encode_public_id(IdentityDomain::ServingSnapshot, None, identity.id)?,
            body: self,
            manifest_digest: format!("b3:{}", hex(&manifest_digest)),
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut encoded, byte| {
            write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
            encoded
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_value(byte: u8) -> String {
        format!("b3:{}", hex(&[byte; 32]))
    }

    fn body() -> ServingSnapshotManifestBody {
        let id = |prefix: &str, byte: u8| format!("{prefix}:{}", hex(&[byte; 16]));
        let context_ids = [[5; 16]];
        let context_set_id = encode_public_id(
            IdentityDomain::ContextSet,
            None,
            context_set_identity([1; 16], &context_ids).unwrap().id,
        )
        .unwrap();
        ServingSnapshotManifestBody {
            manifest_version: "1.0".to_owned(),
            workspace_id: id("workspace", 1),
            repository_id: Some(id("repository", 2)),
            worktree_id: Some(id("worktree", 3)),
            registration_revision: 7,
            source: SnapshotSource {
                source_generation: 11,
                admitted_event_sequence: 12,
                reconciled_event_sequence: 12,
                inventory_digest: digest_value(1),
                authorization_fingerprint: digest_value(2),
                inclusion_policy_fingerprint: digest_value(3),
                path_profile_version: "1.0".to_owned(),
                source_trust_state: "CURRENT".to_owned(),
                event_stream_health: "HEALTHY".to_owned(),
                git_acceleration_status: "GIT_READY".to_owned(),
                git_state_fingerprint: Some(digest_value(4)),
            },
            contexts: SnapshotContexts {
                context_set_id,
                default_python_context_id: Some(id("context", 5)),
                default_rust_context_id: None,
                records: vec![SnapshotContextRecord {
                    analysis_context_id: id("context", 5),
                    context_manifest_digest: digest_value(5),
                    capability_partition_digest: digest_value(6),
                }],
            },
            base_publication: SnapshotBasePublication {
                publication_id: id("publication", 6),
                tables: vec![SnapshotBaseTable {
                    table_code: 100,
                    table_uri: "file:///tmp/entity".to_owned(),
                    delta_version: 9,
                    schema_digest: digest_value(7),
                    row_count: 1,
                    primary_key_digest: digest_value(8),
                    effective_content_digest: digest_value(9),
                }],
            },
            overlay: SnapshotOverlay {
                overlay_generation: 3,
                overlay_digest: digest_value(10),
                total_memory_bytes: 128,
                tables: vec![SnapshotOverlayTable {
                    table_code: 100,
                    mutation_policy: "OWNER_REPLACE".to_owned(),
                    replacement_row_count: 1,
                    owner_tombstone_count: 0,
                    key_tombstone_count: 0,
                    table_replacement: false,
                    row_digest: digest_value(11),
                    tombstone_digest: digest_value(12),
                }],
            },
            indexes: SnapshotIndexes {
                capability_index_digest: digest_value(13),
                diagnostic_index_digest: digest_value(14),
                dependency_graph_digest: digest_value(15),
            },
            bundles: SnapshotBundles {
                ontology_bundle_id: "ontology:1.0".to_owned(),
                schema_bundle_id: "schema:1.0".to_owned(),
                provider_bundle_id: "provider:1.0".to_owned(),
                derivation_bundle_id: "derivation:1.0".to_owned(),
                query_language_bundle_id: "query:1.0".to_owned(),
                model_pack_bundle_id: "model-pack:1.0".to_owned(),
                toolchain_bundle_id: "toolchain:1.0".to_owned(),
                sandbox_profile_digests: BTreeMap::new(),
            },
            result_authority: None,
            limits_profile_digest: digest_value(16),
            source_blob_digests: vec![digest_value(17)],
        }
    }

    #[test]
    fn wp09_serving_snapshot_identity_is_content_addressed() {
        let first = body().derive().unwrap();
        let second = body().derive().unwrap();
        assert_eq!(first, second);
        let mut changed = body();
        changed.source.reconciled_event_sequence += 1;
        let changed = changed.derive().unwrap();
        assert_ne!(first.manifest_digest, changed.manifest_digest);
        assert_ne!(first.snapshot_id, changed.snapshot_id);
    }

    #[test]
    fn sandbox_profile_digest_is_snapshot_identity_material() {
        let mut first_body = body();
        first_body.bundles.sandbox_profile_digests.insert(
            "pyrefly-python".into(),
            format!("sha256:{}", "11".repeat(32)),
        );
        let first = first_body.derive().unwrap();

        let mut changed_body = body();
        changed_body.bundles.sandbox_profile_digests.insert(
            "pyrefly-python".into(),
            format!("sha256:{}", "22".repeat(32)),
        );
        let changed = changed_body.clone().derive().unwrap();
        assert_ne!(first.snapshot_id, changed.snapshot_id);
        assert_ne!(first.manifest_digest, changed.manifest_digest);

        changed_body
            .bundles
            .sandbox_profile_digests
            .insert("rustc-mir".into(), format!("sha256:{}", "AA".repeat(32)));
        assert!(matches!(
            changed_body.derive(),
            Err(SnapshotManifestError::InvalidField(
                "bundles.sandbox_profile_digests"
            ))
        ));
    }

    #[test]
    fn activation_observations_do_not_change_snapshot_identity() {
        let manifest = body().derive().unwrap();
        let before = SnapshotActivationRecord {
            snapshot_id: manifest.snapshot_id.clone(),
            created_at: "2026-08-21T00:00:00Z".to_owned(),
            activated_at: None,
            observed_durable_pointer_generation: 1,
            active_pointer_generation: 1,
            lease_count: 0,
        };
        let after = SnapshotActivationRecord {
            activated_at: Some("2026-08-21T00:00:01Z".to_owned()),
            lease_count: 9,
            ..before.clone()
        };
        assert_eq!(before.snapshot_id, after.snapshot_id);
        assert_eq!(manifest.snapshot_id, after.snapshot_id);
    }
}
