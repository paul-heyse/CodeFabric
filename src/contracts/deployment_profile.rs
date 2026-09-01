//! Narrow runtime contract for the released local-workstation deployment profile.
//!
//! The daemon consumes this operational profile directly. Historical bundle, registry,
//! traceability, model-pack, ontology, and scaffold document models are deliberately absent.

use std::collections::BTreeSet;

use serde::Deserialize;

/// Artifact kind admitted at the runtime deployment-profile boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentArtifactKind {
    YamlContract,
}

/// Release state admitted at the runtime deployment-profile boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentArtifactStatus {
    Released,
}

/// Canonical projection admitted for the released YAML profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum DeploymentDigestProjection {
    #[serde(rename = "yaml-ac-g-53-v1")]
    YamlAcG53V1,
}

/// Strict identity envelope for the released deployment profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentArtifactHeader {
    pub artifact_id: String,
    pub artifact_kind: DeploymentArtifactKind,
    pub version: String,
    pub compatible_suite_major: u16,
    pub status: DeploymentArtifactStatus,
    pub canonical_digest: String,
    pub digest_projection: DeploymentDigestProjection,
    pub generator_revision: String,
}

/// One supported deployment platform code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeploymentPlatform {
    #[serde(rename = "linux-x86_64")]
    LinuxX86_64,
    #[serde(rename = "linux-aarch64")]
    LinuxAarch64,
    #[serde(rename = "macos-aarch64")]
    MacosAarch64,
    #[serde(rename = "macos-x86_64")]
    MacosX86_64,
}

/// Platform-specific root selection and private-mode contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlatformRootProfile {
    pub platform_family: String,
    pub state_root_options: Vec<String>,
    pub runtime_root_options: Vec<String>,
    pub config_root_options: Vec<String>,
    pub directory_mode: String,
    pub private_file_mode: String,
}

/// Bounded source-image capture defaults.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceImageLimitProfile {
    pub ordinary_maximum_bytes: u64,
    pub explicit_maximum_bytes: u64,
    pub stable_read_retry_count: u8,
    pub orphan_grace_seconds: u64,
    pub garbage_collection_batch_size: u32,
}

/// One magic-byte prefix admitted by source classification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BinarySignatureProfile {
    pub name: String,
    pub prefix_hex: String,
}

/// Source admission and classification policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceAdmissionProfile {
    pub binary_sample_bytes: u32,
    pub maximum_single_line_bytes: u32,
    pub maximum_path_components: u16,
    pub maximum_path_bytes: u16,
    pub excluded_directory_names: BTreeSet<String>,
    pub vendored_directory_names: BTreeSet<String>,
    pub generated_directory_names: BTreeSet<String>,
    pub binary_signatures: Vec<BinarySignatureProfile>,
}

/// Independently enforceable generic-inventory bounds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InventoryLimitProfile {
    pub maximum_file_count: u64,
    pub maximum_directory_count: u64,
    pub maximum_directory_depth: u32,
    pub maximum_total_bytes_considered: u64,
    pub maximum_duration_ms: u64,
    pub maximum_entries_per_directory: u64,
}

/// Bounded continuous-update configuration owned by the deployment profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LifecycleLimitProfile {
    pub watch_debounce_timeout_ms: u64,
    pub watch_tick_rate_ms: u64,
    pub watch_ingress_capacity: u16,
    pub maximum_watch_paths_per_batch: u32,
    pub gather_window_ms: u64,
    pub dirty_path_bulk_threshold: u32,
    pub default_await_current_timeout_ms: u64,
    pub overlay_flush_maximum_rows: u64,
    pub overlay_flush_maximum_bytes: u64,
    pub overlay_flush_maximum_touched_owners: u64,
    pub overlay_flush_maximum_generations: u64,
}

/// Strict released local-workstation deployment profile consumed by the daemon.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentProfileDocument {
    #[serde(flatten)]
    pub header: DeploymentArtifactHeader,
    pub profile_id: String,
    pub supported_platforms: BTreeSet<DeploymentPlatform>,
    pub windows_support: String,
    pub network_listeners: String,
    pub workspace_registration: String,
    pub operational_store: String,
    pub fact_store: String,
    pub object_store: String,
    pub hot_overlay_journal: String,
    pub source_blob_persistence: String,
    pub result_artifact_ttl_seconds: u32,
    pub source_result_artifact_ttl_seconds: u32,
    pub coordinator_command_capacity: u16,
    pub maximum_concurrent_source_reads: u16,
    pub maximum_concurrent_gix_jobs: u16,
    pub source_image_limits: SourceImageLimitProfile,
    pub source_admission: SourceAdmissionProfile,
    pub inventory_limits: InventoryLimitProfile,
    pub lifecycle_limits: LifecycleLimitProfile,
    pub default_query_freshness: String,
    pub provider_sandbox: String,
    pub follow_directory_symlinks: bool,
    pub follow_internal_file_symlinks: bool,
    pub index_external_dependency_bodies: bool,
    pub semantic_query_language: String,
    pub canonical_json: String,
    pub platform_roots: Vec<PlatformRootProfile>,
}
