-- @generated from codefabric.schema.contract-ir semantic=b3:b333477366c2b31c2852f3a69d73ec769e3022e15cab90ec935df0db0fea03fb source=b3:2c6f24ac53ea1b07fce5bcfe04acf2a8e0806b528009ec37d1a26be09e2d53a4; schema-contract-driver-v1; do not edit.
-- Cross-store Arrow/Delta foreign keys are generated as application contracts, not SQLite reference clauses.

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
  inventory_digest BLOB,
  PRIMARY KEY (workspace_id),
  UNIQUE (worktree_id)
) STRICT;

CREATE TABLE source_inventory (
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  path_bytes BLOB NOT NULL,
  path_display TEXT NOT NULL,
  comparison_key_bytes BLOB NOT NULL,
  file_id BLOB,
  content_digest BLOB,
  byte_length INTEGER NOT NULL,
  file_kind_code INTEGER NOT NULL,
  language_code TEXT,
  inventory_classification_code INTEGER NOT NULL,
  inclusion_state_code INTEGER NOT NULL,
  git_repo_path_bytes BLOB,
  git_blob_oid BLOB,
  current_file_owner BLOB,
  PRIMARY KEY (workspace_id, source_generation, path_bytes)
) STRICT;

CREATE TABLE source_blob (
  blob_digest BLOB NOT NULL,
  byte_length INTEGER NOT NULL,
  line_index_digest BLOB NOT NULL,
  encoding_code INTEGER NOT NULL,
  newline_code INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (blob_digest)
) STRICT;

CREATE TABLE source_blob_lease (
  lease_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  holder_kind_code INTEGER NOT NULL,
  holder_id BLOB NOT NULL,
  state_code INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  orphaned_at INTEGER,
  PRIMARY KEY (lease_id),
  UNIQUE (workspace_id, source_generation, holder_kind_code, holder_id)
) STRICT;

CREATE TABLE source_blob_lease_member (
  lease_id BLOB NOT NULL,
  blob_digest BLOB NOT NULL,
  PRIMARY KEY (lease_id, blob_digest)
) STRICT;

CREATE TABLE git_state_vector (
  workspace_id BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  repository_id BLOB NOT NULL,
  worktree_id BLOB NOT NULL,
  head_kind_code INTEGER NOT NULL,
  head_target BLOB,
  head_tree BLOB,
  index_fingerprint BLOB,
  index_entry_count INTEGER,
  has_conflict_stages INTEGER NOT NULL,
  repository_state_code INTEGER NOT NULL,
  inclusion_policy_fingerprint BLOB NOT NULL,
  attributes_fingerprint BLOB NOT NULL,
  worktree_inventory_digest BLOB NOT NULL,
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

CREATE TABLE operational_dependency_edge (
  workspace_id BLOB NOT NULL,
  source_owner_id BLOB NOT NULL,
  dependent_owner_id BLOB NOT NULL,
  edge_kind_code INTEGER NOT NULL,
  derivation_id TEXT,
  source_generation INTEGER NOT NULL,
  input_digest BLOB NOT NULL,
  active INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, source_owner_id, dependent_owner_id, edge_kind_code)
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
  sandbox_profile_digest TEXT,
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

CREATE TABLE git_candidate_cache (
  workspace_id BLOB NOT NULL,
  worktree_id BLOB NOT NULL,
  state_vector_digest BLOB NOT NULL,
  topology_digest BLOB NOT NULL,
  mode_code INTEGER NOT NULL,
  candidate_payload BLOB NOT NULL,
  payload_digest BLOB NOT NULL,
  source_generation INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, worktree_id, state_vector_digest, topology_digest, mode_code)
) STRICT;

CREATE TABLE table_mutation_operation (
  operation_id BLOB NOT NULL,
  table_code INTEGER NOT NULL,
  mutation_phase TEXT NOT NULL,
  application_id TEXT NOT NULL,
  application_version INTEGER NOT NULL,
  publication_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  analysis_context_id BLOB,
  source_generation INTEGER NOT NULL,
  owner_set_fingerprint BLOB NOT NULL,
  input_checksum BLOB NOT NULL,
  expected_output_checksum BLOB NOT NULL,
  expected_predecessor INTEGER,
  state_code INTEGER NOT NULL,
  delta_version INTEGER,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  PRIMARY KEY (operation_id, table_code, mutation_phase),
  UNIQUE (application_id, application_version)
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
  lease_kind_code INTEGER NOT NULL,
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  base_publication_id BLOB NOT NULL,
  required_delta_versions_bytes BLOB NOT NULL,
  requires_overlay INTEGER NOT NULL,
  agent_instance_id BLOB,
  created_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  state_code INTEGER NOT NULL,
  process_instance_id BLOB NOT NULL,
  orphaned_at INTEGER,
  artifact_expires_at INTEGER,
  source_blob_lease_id BLOB,
  ontology_epoch_identity TEXT,
  result_authority_identity TEXT,
  PRIMARY KEY (lease_id)
) STRICT;

CREATE TABLE result_artifact_lease (
  lease_id BLOB NOT NULL,
  artifact_uri TEXT NOT NULL,
  checksum BLOB NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY (lease_id)
) STRICT;

CREATE TABLE query_execution_terminal (
  execution_id TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  semantic_request_id TEXT NOT NULL,
  mcp_call_id TEXT NOT NULL,
  terminal_phase TEXT NOT NULL,
  failing_stage TEXT,
  bundle_checksum TEXT NOT NULL,
  primary_payload_uri TEXT,
  payload_status TEXT NOT NULL,
  fallback_envelope_bytes BLOB,
  snapshot_id TEXT,
  publication_id TEXT,
  source_table_versions_bytes BLOB NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY (execution_id)
) STRICT;

CREATE TABLE serving_snapshot_manifest (
  snapshot_id BLOB NOT NULL,
  workspace_id BLOB NOT NULL,
  publication_id BLOB NOT NULL,
  state_code INTEGER NOT NULL,
  manifest_body_bytes BLOB NOT NULL,
  manifest_json_bytes BLOB NOT NULL,
  manifest_digest BLOB NOT NULL,
  created_at INTEGER NOT NULL,
  activated_at INTEGER,
  retired_at INTEGER,
  PRIMARY KEY (snapshot_id)
) STRICT;

CREATE TABLE active_snapshot (
  workspace_id BLOB NOT NULL,
  snapshot_id BLOB NOT NULL,
  created_at INTEGER NOT NULL,
  activated_at INTEGER NOT NULL,
  observed_durable_pointer_generation INTEGER NOT NULL,
  active_pointer_generation INTEGER NOT NULL,
  lease_count INTEGER NOT NULL,
  PRIMARY KEY (workspace_id)
) STRICT;

CREATE TABLE ontology_candidate (
  candidate_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  state TEXT NOT NULL,
  manifest_bytes BLOB NOT NULL,
  manifest_digest TEXT NOT NULL,
  program_identity TEXT NOT NULL,
  package_identity TEXT NOT NULL,
  config_identity TEXT NOT NULL,
  policy_identity TEXT NOT NULL,
  exact_table_set_identity TEXT NOT NULL,
  publication_id BLOB NOT NULL,
  serving_snapshot_id BLOB,
  observed_durable_pointer_generation INTEGER NOT NULL,
  predecessor_epoch_identity TEXT,
  rollback_retain_until INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (candidate_identity)
) STRICT;

CREATE TABLE ontology_candidate_exact_table (
  candidate_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  table_code INTEGER NOT NULL,
  table_uri TEXT NOT NULL,
  delta_version INTEGER NOT NULL,
  schema_identity BLOB NOT NULL,
  content_identity BLOB NOT NULL,
  PRIMARY KEY (candidate_identity, table_code)
) STRICT;

CREATE TABLE ontology_gate_execution (
  execution_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  semantic_checksum TEXT NOT NULL,
  artifact_identity TEXT NOT NULL,
  receipt_identity TEXT NOT NULL,
  completed_at INTEGER NOT NULL,
  PRIMARY KEY (execution_identity),
  UNIQUE (candidate_identity, operation_id)
) STRICT;

CREATE TABLE ontology_gate_receipt (
  receipt_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  receipt_bytes BLOB NOT NULL,
  semantic_checksum TEXT NOT NULL,
  expected_result_contract TEXT NOT NULL,
  artifact_identity TEXT NOT NULL,
  PRIMARY KEY (receipt_identity),
  UNIQUE (candidate_identity, operation_id)
) STRICT;

CREATE TABLE ontology_gate_artifact (
  artifact_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  artifact_bytes BLOB NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (artifact_identity),
  UNIQUE (candidate_identity, operation_id)
) STRICT;

CREATE TABLE ontology_owner_decision (
  decision_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  owner_identity TEXT NOT NULL,
  policy_identity TEXT NOT NULL,
  decision_bytes BLOB NOT NULL,
  accepted_at INTEGER NOT NULL,
  PRIMARY KEY (decision_identity),
  UNIQUE (candidate_identity)
) STRICT;

CREATE TABLE ontology_activation_request (
  request_key TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  decision_identity TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  submission_digest TEXT NOT NULL,
  expected_predecessor_identity TEXT,
  expected_pointer_generation INTEGER NOT NULL,
  state TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  completed_at INTEGER,
  PRIMARY KEY (request_key)
) STRICT;

CREATE TABLE ontology_acceptance (
  candidate_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  request_key TEXT NOT NULL,
  decision_identity TEXT NOT NULL,
  receipt_set_identity TEXT NOT NULL,
  pointer_generation INTEGER NOT NULL,
  accepted_at INTEGER NOT NULL,
  PRIMARY KEY (candidate_identity)
) STRICT;

CREATE TABLE ontology_active_pointer (
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  epoch_identity TEXT NOT NULL,
  predecessor_epoch_identity TEXT,
  pointer_generation INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (workspace_id)
) STRICT;

CREATE TABLE ontology_recovery (
  request_key TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  outcome TEXT NOT NULL,
  observed_pointer_generation INTEGER NOT NULL,
  reconciled_at INTEGER NOT NULL,
  PRIMARY KEY (request_key)
) STRICT;

CREATE TABLE ontology_result_authority (
  result_authority_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  program_identity TEXT NOT NULL,
  function_catalog_identity TEXT NOT NULL,
  policy_identity TEXT NOT NULL,
  query_form_identity TEXT NOT NULL,
  checksum_version TEXT NOT NULL,
  exact_table_set_identity TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (result_authority_identity)
) STRICT;

CREATE TABLE ontology_serving_epoch (
  epoch_identity TEXT NOT NULL,
  workspace_id BLOB NOT NULL,
  candidate_identity TEXT NOT NULL,
  predecessor_epoch_identity TEXT,
  result_authority_identity TEXT NOT NULL,
  state TEXT NOT NULL,
  activated_at INTEGER NOT NULL,
  retained_until INTEGER NOT NULL,
  PRIMARY KEY (epoch_identity)
) STRICT;

CREATE VIEW workspace_update_state AS
SELECT workspace_id, lifecycle_state_code, source_generation, event_watermark, newest_dirty_generation, durable_generation, reconcile_required, updated_at
FROM worktree_state;

CREATE VIEW source_trust_state AS
SELECT workspace_id, source_trust_state_code, event_stream_health_code, git_acceleration_status_code, last_diagnostic_id, updated_at
FROM worktree_state;

