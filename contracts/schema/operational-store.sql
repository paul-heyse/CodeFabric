-- @generated from codefabric.schema.contract-ir b3:8f9e1ed9f532a518bfd4e7b7ec398a119845c2f8cdb26da691b6e1227c2b1c30; codefabric-schema-contracts-v1; do not edit.
PRAGMA foreign_keys = ON;

CREATE TABLE workspace_registration (
  workspace_id BLOB NOT NULL,
  workspace_registration_nonce BLOB NOT NULL,
  registration_revision INTEGER NOT NULL,
  administrative_key BLOB NOT NULL,
  root_path_bytes BLOB NOT NULL,
  root_path_display TEXT NOT NULL,
  root_directory_file_identity BLOB NOT NULL,
  platform_code INTEGER NOT NULL,
  case_sensitivity_mode TEXT NOT NULL,
  authorization_revision INTEGER NOT NULL,
  allowed_source_disclosure_rules BLOB NOT NULL,
  repository_id BLOB,
  worktree_id BLOB,
  authorization_fingerprint BLOB NOT NULL,
  context_fingerprint BLOB NOT NULL,
  status_code INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id)
) STRICT;

CREATE TABLE workspace_generation (
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  admitted_event_sequence INTEGER NOT NULL,
  reconciled_event_sequence INTEGER NOT NULL,
  durable_generation INTEGER NOT NULL,
  active_pointer_generation INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id)
) STRICT;

CREATE TABLE nested_root_exclusion (
  parent_workspace_id BLOB NOT NULL,
  child_workspace_id BLOB NOT NULL,
  relative_path_bytes BLOB NOT NULL,
  relative_path_display TEXT NOT NULL,
  authorization_fingerprint BLOB NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (parent_workspace_id, child_workspace_id)
) STRICT;

CREATE TABLE credential_metadata (
  credential_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  agent_id TEXT NOT NULL,
  credential_hash BLOB NOT NULL,
  operations_mask INTEGER NOT NULL,
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT,
  revocation_reason_code INTEGER,
  PRIMARY KEY (credential_id)
) STRICT;

CREATE TABLE audit_event (
  event_id BLOB NOT NULL,
  workspace_id BLOB,
  event_code INTEGER NOT NULL,
  actor_id TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  details_digest BLOB NOT NULL,
  diagnostic_id BLOB,
  PRIMARY KEY (event_id)
) STRICT;

CREATE TABLE repository_registration (
  repository_id BLOB NOT NULL,
  repository_registration_nonce BLOB NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (repository_id)
) STRICT;

CREATE TABLE worktree_registration (
  worktree_id BLOB NOT NULL,
  repository_id BLOB NOT NULL,
  worktree_registration_nonce BLOB NOT NULL,
  worktree_kind TEXT NOT NULL,
  administrative_key BLOB NOT NULL,
  created_at TEXT NOT NULL,
  removed_at TEXT,
  PRIMARY KEY (worktree_id)
) STRICT;

CREATE TABLE common_repository_state (
  repository_id BLOB NOT NULL,
  common_dir_path_bytes BLOB NOT NULL,
  common_dir_path_display TEXT NOT NULL,
  object_format_code INTEGER NOT NULL,
  gix_version TEXT NOT NULL,
  trust_policy_fingerprint BLOB NOT NULL,
  worktree_count INTEGER NOT NULL,
  git_health_code INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  last_diagnostic_id BLOB,
  PRIMARY KEY (repository_id)
) STRICT;

CREATE TABLE worktree_state (
  workspace_id BLOB NOT NULL,
  worktree_id BLOB,
  repository_id BLOB,
  work_dir_path_bytes BLOB NOT NULL,
  work_dir_path_display TEXT NOT NULL,
  git_dir_path_bytes BLOB,
  git_dir_path_display TEXT,
  lifecycle_state_code INTEGER NOT NULL,
  source_trust_state_code INTEGER NOT NULL,
  event_stream_health_code INTEGER NOT NULL,
  git_acceleration_status_code INTEGER NOT NULL,
  active_snapshot_id BLOB,
  analysis_context_set_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  event_watermark INTEGER NOT NULL,
  newest_dirty_generation INTEGER NOT NULL,
  durable_generation INTEGER NOT NULL,
  reconcile_required INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  last_diagnostic_id BLOB,
  PRIMARY KEY (workspace_id),
  UNIQUE (worktree_id)
) STRICT;

CREATE TABLE git_state_vector (
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  head_oid BLOB,
  head_tree_oid BLOB,
  index_fingerprint BLOB NOT NULL,
  worktree_fingerprint BLOB NOT NULL,
  inclusion_fingerprint BLOB NOT NULL,
  captured_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, source_generation)
) STRICT;

CREATE TABLE update_wave (
  wave_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  event_watermark INTEGER NOT NULL,
  state_code INTEGER NOT NULL,
  candidate_strategy_code INTEGER NOT NULL,
  input_fingerprint BLOB NOT NULL,
  candidate_count INTEGER NOT NULL,
  started_at TEXT NOT NULL,
  terminal_at TEXT,
  diagnostic_id BLOB,
  PRIMARY KEY (wave_id)
) STRICT;

CREATE TABLE update_wave_item (
  wave_id BLOB NOT NULL,
  item_ordinal INTEGER NOT NULL,
  path_bytes BLOB NOT NULL,
  path_display TEXT NOT NULL,
  path_encoding_code INTEGER NOT NULL,
  state_code INTEGER NOT NULL,
  input_fingerprint BLOB NOT NULL,
  output_fingerprint BLOB,
  diagnostic_id BLOB,
  PRIMARY KEY (wave_id, item_ordinal)
) STRICT;

CREATE TABLE provider_run (
  provider_run_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  analysis_context_id BLOB NOT NULL,
  wave_id BLOB NOT NULL,
  provider_code INTEGER NOT NULL,
  owner_id BLOB,
  build_unit_id BLOB,
  source_generation INTEGER NOT NULL,
  input_fingerprint BLOB NOT NULL,
  output_fingerprint BLOB,
  state_code INTEGER NOT NULL,
  accepted_at TEXT NOT NULL,
  terminal_at TEXT,
  diagnostic_id BLOB,
  PRIMARY KEY (provider_run_id)
) STRICT;

CREATE TABLE git_operation_run (
  git_operation_run_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  baseline_fingerprint BLOB NOT NULL,
  result_fingerprint BLOB,
  candidate_count INTEGER NOT NULL,
  verified_count INTEGER NOT NULL,
  state_code INTEGER NOT NULL,
  started_at TEXT NOT NULL,
  terminal_at TEXT,
  diagnostic_id BLOB,
  PRIMARY KEY (git_operation_run_id)
) STRICT;

CREATE TABLE hot_overlay_manifest (
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  base_publication_id BLOB NOT NULL,
  overlay_generation INTEGER NOT NULL,
  analysis_context_set_id BLOB NOT NULL,
  overlay_checksum BLOB NOT NULL,
  table_manifest_bytes BLOB NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, overlay_generation)
) STRICT;

CREATE TABLE snapshot_lease (
  lease_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  owner_id TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY (lease_id)
) STRICT;

CREATE TABLE result_artifact_lease (
  lease_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  artifact_uri TEXT NOT NULL,
  checksum BLOB NOT NULL,
  owner_id TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY (lease_id)
) STRICT;

CREATE TABLE serving_snapshot_manifest (
  snapshot_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  repository_id BLOB,
  worktree_id BLOB,
  registration_revision INTEGER NOT NULL,
  source_generation INTEGER NOT NULL,
  admitted_event_sequence INTEGER NOT NULL,
  reconciled_event_sequence INTEGER NOT NULL,
  inventory_digest BLOB NOT NULL,
  authorization_fingerprint BLOB NOT NULL,
  inclusion_policy_fingerprint BLOB NOT NULL,
  path_profile_version TEXT NOT NULL,
  source_trust_state_code INTEGER NOT NULL,
  event_stream_health_code INTEGER NOT NULL,
  git_acceleration_status_code INTEGER NOT NULL,
  git_state_fingerprint BLOB,
  context_set_id BLOB NOT NULL,
  contexts_manifest_bytes BLOB NOT NULL,
  publication_id BLOB NOT NULL,
  base_tables_manifest_bytes BLOB NOT NULL,
  overlay_generation INTEGER NOT NULL,
  overlay_digest BLOB NOT NULL,
  overlay_total_memory_bytes INTEGER NOT NULL,
  overlay_tables_manifest_bytes BLOB NOT NULL,
  capability_index_digest BLOB NOT NULL,
  diagnostic_index_digest BLOB NOT NULL,
  dependency_graph_digest BLOB NOT NULL,
  bundle_ids_bytes BLOB NOT NULL,
  limits_profile_digest BLOB NOT NULL,
  manifest_digest BLOB NOT NULL,
  PRIMARY KEY (snapshot_id)
) STRICT;

CREATE TABLE active_snapshot (
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  created_at TEXT NOT NULL,
  activated_at TEXT NOT NULL,
  observed_durable_pointer_generation INTEGER NOT NULL,
  active_pointer_generation INTEGER NOT NULL,
  lease_count INTEGER NOT NULL,
  PRIMARY KEY (workspace_id)
) STRICT;

CREATE VIEW workspace_update_state AS
SELECT workspace_id, lifecycle_state_code, source_generation, event_watermark, newest_dirty_generation, durable_generation, reconcile_required, updated_at
FROM worktree_state;
