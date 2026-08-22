//! Persisted AC-G-09/10 workspace registrations driven by generated lifecycle tables.

use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension as _, Transaction, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::analysis_context::{AnalysisContext, AnalysisContextError, AnalysisContextKind};
use crate::identity::{
    CaseSensitivityMode, IdentityDomain, IdentityError, RootAuthorizationInput, SOURCE_CONTEXT_ID,
    context_set_identity, encode_public_id, probe_case_sensitivity, random_registration_nonce,
    repository_registration_identity, root_authorization_fingerprint,
    workspace_registration_identity, worktree_registration_identity,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::registries::{
    EventStreamHealth, GitAccelerationStatus, SnapshotLeaseState, SourceTrustState,
    WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS, WORKSPACE_REGISTRY_LIFECYCLE_VALUES,
    WorkspaceLifecycle, WorkspaceRegistryLifecycle, generated_transition, registry_state_name,
};

const DEFAULT_DISCLOSURE_RULES: [&str; 1] = ["metadata"];

/// Verified source topology supplied by the registration boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceSourceRegistration {
    /// A directory with no synthetic Git identity.
    Directory,
    /// The first worktree registered for a newly registered repository.
    NewGitRepository {
        worktree_administrative_key: Vec<u8>,
        worktree_kind: String,
    },
    /// A linked worktree whose repository identity is already registered.
    ExistingGitRepository {
        repository_id: [u8; 16],
        worktree_administrative_key: Vec<u8>,
        worktree_kind: String,
    },
}

/// Proof supplied when the operator moves an existing registration to a new root.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "proof", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelinkProof {
    NonGit {
        operator_acknowledged: bool,
        inventory_matches: bool,
    },
    Git {
        repository_id: [u8; 16],
        worktree_id: [u8; 16],
        worktree_administrative_key: Vec<u8>,
    },
}

/// Persisted workspace registration projected from the generated operational schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecord {
    #[serde(with = "workspace_id_serde")]
    pub workspace_id: [u8; 16],
    pub workspace_registration_nonce: [u8; 16],
    pub registration_revision: u64,
    pub administrative_key: Vec<u8>,
    pub root_path_bytes: Vec<u8>,
    pub root_path_display: String,
    pub root_directory_file_identity: Vec<u8>,
    pub platform_code: u8,
    pub case_sensitivity_mode: String,
    pub authorization_revision: u64,
    pub allowed_source_disclosure_rules: Vec<String>,
    #[serde(with = "optional_repository_id_serde")]
    pub repository_id: Option<[u8; 16]>,
    #[serde(with = "optional_worktree_id_serde")]
    pub worktree_id: Option<[u8; 16]>,
    pub authorization_fingerprint: [u8; 32],
    pub context_fingerprint: [u8; 32],
    pub status: WorkspaceRegistryLifecycle,
    pub created_at: String,
    pub updated_at: String,
}

mod workspace_id_serde {
    use serde::{Deserialize as _, Deserializer, Serializer};

    use crate::identity::{IdentityDomain, encode_public_id};

    pub fn serialize<S>(id: &[u8; 16], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(
            &encode_public_id(IdentityDomain::Workspace, None, *id)
                .map_err(serde::ser::Error::custom)?,
        )
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 16], D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        crate::identity::decode_public_id(IdentityDomain::Workspace, None, &value)
            .map_err(serde::de::Error::custom)
    }
}

macro_rules! optional_public_id_serde {
    ($module:ident, $domain:expr) => {
        mod $module {
            use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer};

            use crate::identity::{IdentityDomain, encode_public_id};

            #[allow(clippy::ref_option)] // Serde's `with` serializer ABI passes `&FieldType`.
            pub fn serialize<S>(id: &Option<[u8; 16]>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                id.map(|value| encode_public_id($domain, None, value))
                    .transpose()
                    .map_err(serde::ser::Error::custom)?
                    .serialize(serializer)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 16]>, D::Error>
            where
                D: Deserializer<'de>,
            {
                Option::<String>::deserialize(deserializer)?
                    .map(|value| crate::identity::decode_public_id($domain, None, &value))
                    .transpose()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

optional_public_id_serde!(optional_repository_id_serde, IdentityDomain::Repository);
optional_public_id_serde!(optional_worktree_id_serde, IdentityDomain::Worktree);

impl Serialize for WorkspaceRegistryLifecycle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(*self as u16)
    }
}

impl<'de> Deserialize<'de> for WorkspaceRegistryLifecycle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let code = u16::deserialize(deserializer)?;
        Self::try_from(code).map_err(|_| serde::de::Error::custom("unknown workspace status"))
    }
}

impl WorkspaceRecord {
    /// Stable symbolic public workspace ID.
    #[must_use]
    pub fn public_id(&self) -> String {
        encode_public_id(IdentityDomain::Workspace, None, self.workspace_id)
            .unwrap_or_else(|_| "workspace:<invalid>".to_owned())
    }
}

/// Result of one destructive-retention choice.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovalPolicy {
    RetainData,
    PurgeData,
}

/// Stable workspace-registry failures.
#[derive(Debug, Error)]
pub enum WorkspaceRegistryError {
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    AnalysisContext(#[from] AnalysisContextError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("workspace root is invalid: {0}")]
    Root(String),
    #[error("workspace {0} is not registered")]
    NotFound(String),
    #[error("duplicate active worktree administrative key")]
    DuplicateAdministrativeKey,
    #[error("STATE_TRANSITION_VIOLATION: {prior_state} + {event} + {guard}")]
    StateTransitionViolation {
        prior_state: String,
        event: String,
        guard: String,
    },
    #[error("workspace relink proof does not match the persisted source identity")]
    RelinkProof,
    #[error("purge requires two explicit confirmations")]
    PurgeConfirmation,
    #[error("workspace has active snapshot or result-artifact leases")]
    ActiveLease,
    #[error("workspace record has invalid persisted data: {0}")]
    Persisted(String),
}

/// The sole administrative mutation port over the coordinator-owned store writer.
pub struct WorkspaceRegistry<'store> {
    store: &'store mut OperationalStore,
}

#[derive(Clone, Copy)]
struct RegistrationNonces {
    workspace: [u8; 16],
    repository: [u8; 16],
    worktree: [u8; 16],
}

struct AuthorizedRoot {
    bytes: Vec<u8>,
    display: String,
    file_identity: Vec<u8>,
    platform_code: u8,
    case_mode: String,
    disclosure_rules: Vec<String>,
}

impl<'store> WorkspaceRegistry<'store> {
    /// Bind the administrative API to the sole writer.
    pub fn new(store: &'store mut OperationalStore) -> Self {
        Self { store }
    }

    /// Register one explicit root and persist its initial disabled state.
    ///
    /// # Errors
    ///
    /// Returns a root, entropy, identity, duplicate, transition, or store error.
    pub fn add(
        &mut self,
        root: &Path,
        source: WorkspaceSourceRegistration,
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        self.add_with_nonces(
            root,
            source,
            RegistrationNonces {
                workspace: random_registration_nonce()?,
                repository: random_registration_nonce()?,
                worktree: random_registration_nonce()?,
            },
        )
    }

    #[allow(clippy::too_many_lines)] // One transaction keeps registration invariants atomic.
    fn add_with_nonces(
        &mut self,
        root: &Path,
        source: WorkspaceSourceRegistration,
        nonces: RegistrationNonces,
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        let authorized = authorize_root(root)?;
        let workspace_kind = match &source {
            WorkspaceSourceRegistration::Directory => "directory",
            WorkspaceSourceRegistration::NewGitRepository { .. }
            | WorkspaceSourceRegistration::ExistingGitRepository { .. } => "git-worktree",
        };
        let workspace = workspace_registration_identity(nonces.workspace, workspace_kind)?;
        let (repository_id, worktree_id, administrative_key, repository_nonce, worktree_kind) =
            match source {
                WorkspaceSourceRegistration::Directory => (
                    None,
                    None,
                    directory_administrative_key(&authorized.bytes),
                    None,
                    None,
                ),
                WorkspaceSourceRegistration::NewGitRepository {
                    worktree_administrative_key,
                    worktree_kind,
                } => {
                    let repository = repository_registration_identity(nonces.repository)?;
                    let worktree = worktree_registration_identity(
                        repository.id,
                        nonces.worktree,
                        &worktree_kind,
                    )?;
                    (
                        Some(repository.id),
                        Some(worktree.id),
                        git_administrative_key(repository.id, &worktree_administrative_key),
                        Some(nonces.repository),
                        Some((nonces.worktree, worktree_kind, worktree_administrative_key)),
                    )
                }
                WorkspaceSourceRegistration::ExistingGitRepository {
                    repository_id,
                    worktree_administrative_key,
                    worktree_kind,
                } => {
                    let worktree = worktree_registration_identity(
                        repository_id,
                        nonces.worktree,
                        &worktree_kind,
                    )?;
                    (
                        Some(repository_id),
                        Some(worktree.id),
                        git_administrative_key(repository_id, &worktree_administrative_key),
                        None,
                        Some((nonces.worktree, worktree_kind, worktree_administrative_key)),
                    )
                }
            };
        let authorization_fingerprint = authorization_fingerprint(workspace.id, &authorized, 1)?;
        let context_set_id = context_set_identity(workspace.id, &[SOURCE_CONTEXT_ID])?.id;
        let workspace_public_id = encode_public_id(IdentityDomain::Workspace, None, workspace.id)?;
        let context_fingerprint = AnalysisContext::new(
            &workspace_public_id,
            AnalysisContextKind::Source,
            "1.0",
            "source",
            None,
            true,
        )?
        .fingerprint_bytes()?;
        let now = timestamp()?;
        let disclosure_bytes = serde_json::to_vec(&authorized.disclosure_rules)
            .map_err(|error| WorkspaceRegistryError::Persisted(error.to_string()))?;
        self.store.write_transaction(|transaction| {
            reject_duplicate_administrative_key(transaction, &administrative_key)?;
            if let Some(repository_id) = repository_id {
                if let Some(repository_nonce) = repository_nonce {
                    transaction.execute(
                        "INSERT INTO repository_registration(repository_id, repository_registration_nonce, created_at) VALUES (?1, ?2, ?3)",
                        params![repository_id.as_slice(), repository_nonce.as_slice(), &now],
                    )?;
                } else if !repository_exists(transaction, repository_id)? {
                    return Err(WorkspaceRegistryError::RelinkProof);
                }
            }
            if let (Some(repository_id), Some(worktree_id), Some((nonce, kind, key))) =
                (repository_id, worktree_id, worktree_kind.as_ref())
            {
                reject_duplicate_worktree_key(transaction, repository_id, key)?;
                transaction.execute(
                    "INSERT INTO worktree_registration(worktree_id, repository_id, worktree_registration_nonce, worktree_kind, administrative_key, created_at, removed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                    params![worktree_id.as_slice(), repository_id.as_slice(), nonce.as_slice(), kind, key, &now],
                )?;
            }
            transaction.execute(
                "INSERT INTO workspace_registration(workspace_id, workspace_registration_nonce, registration_revision, administrative_key, root_path_bytes, root_path_display, root_directory_file_identity, platform_code, case_sensitivity_mode, authorization_revision, allowed_source_disclosure_rules, repository_id, worktree_id, authorization_fingerprint, context_fingerprint, status_code, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![workspace.id.as_slice(), nonces.workspace.as_slice(), &administrative_key, &authorized.bytes, &authorized.display, &authorized.file_identity, authorized.platform_code, &authorized.case_mode, &disclosure_bytes, repository_id.as_ref().map(<[u8; 16]>::as_slice), worktree_id.as_ref().map(<[u8; 16]>::as_slice), authorization_fingerprint.as_slice(), context_fingerprint.as_slice(), WorkspaceRegistryLifecycle::Registering as u16, &now],
            )?;
            transaction.execute(
                "INSERT INTO workspace_generation(workspace_id, source_generation, admitted_event_sequence, reconciled_event_sequence, durable_generation, active_pointer_generation, updated_at) VALUES (?1, 0, 0, 0, 0, 0, ?2)",
                params![workspace.id.as_slice(), &now],
            )?;
            transaction.execute(
                "INSERT INTO worktree_state(workspace_id, worktree_id, repository_id, work_dir_path_bytes, work_dir_path_display, git_dir_path_bytes, git_dir_path_display, lifecycle_state_code, source_trust_state_code, event_stream_health_code, git_acceleration_status_code, active_snapshot_id, analysis_context_set_id, source_generation, event_watermark, newest_dirty_generation, durable_generation, reconcile_required, updated_at, last_diagnostic_id) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, ?8, ?9, NULL, ?10, 0, 0, 0, 0, 0, ?11, NULL)",
                params![workspace.id.as_slice(), worktree_id.as_ref().map(<[u8; 16]>::as_slice), repository_id.as_ref().map(<[u8; 16]>::as_slice), &authorized.bytes, &authorized.display, WorkspaceLifecycle::Bootstrapping as u16, SourceTrustState::Unverified as u16, EventStreamHealth::Healthy as u16, if worktree_id.is_some() { GitAccelerationStatus::GitScanning as u16 } else { GitAccelerationStatus::NotAGitWorktree as u16 }, context_set_id.as_slice(), &now],
            )?;
            transition_workspace(
                transaction,
                workspace.id,
                "registration-created",
                "root-authorized",
                &now,
            )?;
            insert_nested_exclusions(transaction, workspace.id, &authorized.bytes, authorization_fingerprint, &now)?;
            audit(transaction, Some(workspace.id), 1_000, "workspace-add", &now)?;
            read_workspace(transaction, workspace.id)
        })
    }

    /// List every non-removed registration in stable public-ID order.
    ///
    /// # Errors
    ///
    /// Returns a store or persisted-data error.
    pub fn list(&self) -> Result<Vec<WorkspaceRecord>, WorkspaceRegistryError> {
        let reader = self.store.reader_factory().open()?;
        reader
            .with_connection(|connection| {
                let mut statement = connection.prepare(&format!(
                    "{} WHERE status_code != ?1 ORDER BY workspace_id",
                    workspace_select()
                ))?;
                statement
                    .query_map(
                        [WorkspaceRegistryLifecycle::Removed as u16],
                        row_to_workspace,
                    )?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(OperationalStoreError::from)
            .map_err(Into::into)
    }

    /// Show one registration, including a retired record.
    ///
    /// # Errors
    ///
    /// Returns a store, not-found, or persisted-data error.
    pub fn show(&self, workspace_id: [u8; 16]) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        let reader = self.store.reader_factory().open()?;
        reader
            .with_connection(|connection| {
                connection
                    .query_row(
                        &format!("{} WHERE workspace_id = ?1", workspace_select()),
                        [workspace_id.as_slice()],
                        row_to_workspace,
                    )
                    .optional()
            })
            .map_err(OperationalStoreError::from)?
            .ok_or_else(|| WorkspaceRegistryError::NotFound(public_workspace_id(workspace_id)))
    }

    /// Enable a disabled registration through generated OPENING into BOOTSTRAPPING.
    ///
    /// # Errors
    ///
    /// Returns a generated transition or store error.
    pub fn enable(
        &mut self,
        workspace_id: [u8; 16],
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        self.mutate(
            workspace_id,
            1_010,
            "workspace-enable",
            |transaction, now| {
                transition_workspace(
                    transaction,
                    workspace_id,
                    "enable",
                    "operator-authorized",
                    now,
                )?;
                transition_workspace(
                    transaction,
                    workspace_id,
                    "root-opened",
                    "root-identity-matches",
                    now,
                )
            },
        )
    }

    /// Disable an opening, bootstrapping, ready, degraded, or failed registration.
    ///
    /// # Errors
    ///
    /// Returns a generated transition or store error.
    pub fn disable(
        &mut self,
        workspace_id: [u8; 16],
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        self.mutate(
            workspace_id,
            1_020,
            "workspace-disable",
            |transaction, now| {
                let record = read_workspace(transaction, workspace_id)?;
                if record.status == WorkspaceRegistryLifecycle::Failed {
                    transition_workspace(
                        transaction,
                        workspace_id,
                        "disable",
                        "operator-authorized",
                        now,
                    )
                } else {
                    transition_workspace(
                        transaction,
                        workspace_id,
                        "disable",
                        "operator-authorized",
                        now,
                    )?;
                    transition_workspace(
                        transaction,
                        workspace_id,
                        "stopped",
                        "no-provider-work",
                        now,
                    )
                }
            },
        )
    }

    /// Relink a registration after proving the same source instance.
    ///
    /// # Errors
    ///
    /// Returns a proof, root, identity, not-found, or store error.
    pub fn relink(
        &mut self,
        workspace_id: [u8; 16],
        new_root: &Path,
        proof: &RelinkProof,
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        let root = authorize_root(new_root)?;
        let now = timestamp()?;
        self.store.write_transaction(|transaction| {
            let record = read_workspace(transaction, workspace_id)?;
            validate_relink_proof(transaction, &record, proof)?;
            let registration_revision = record.registration_revision + 1;
            let authorization_revision = record.authorization_revision + 1;
            let registration_revision_sql = sqlite_u64(registration_revision, "registration_revision")?;
            let authorization_revision_sql = sqlite_u64(authorization_revision, "authorization_revision")?;
            let authorization = authorization_fingerprint(
                workspace_id,
                &root,
                authorization_revision,
            )?;
            let disclosure = serde_json::to_vec(&root.disclosure_rules)
                .map_err(|error| WorkspaceRegistryError::Persisted(error.to_string()))?;
            transaction.execute(
                "UPDATE workspace_registration SET registration_revision=?2, root_path_bytes=?3, root_path_display=?4, root_directory_file_identity=?5, platform_code=?6, case_sensitivity_mode=?7, authorization_revision=?8, allowed_source_disclosure_rules=?9, authorization_fingerprint=?10, updated_at=?11 WHERE workspace_id=?1",
                params![workspace_id.as_slice(), registration_revision_sql, &root.bytes, &root.display, &root.file_identity, root.platform_code, &root.case_mode, authorization_revision_sql, disclosure, authorization.as_slice(), &now],
            )?;
            insert_nested_exclusions(transaction, workspace_id, &root.bytes, authorization, &now)?;
            audit(transaction, Some(workspace_id), 1_030, "workspace-relink", &now)?;
            read_workspace(transaction, workspace_id)
        })
    }

    /// Apply a profile fingerprint and monotonically advance registration revisions.
    ///
    /// # Errors
    ///
    /// Returns an identity, not-found, persisted-data, or store error.
    pub fn configure(
        &mut self,
        workspace_id: [u8; 16],
        profile_fingerprint: [u8; 32],
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        let now = timestamp()?;
        self.store.write_transaction(|transaction| {
            let record = read_workspace(transaction, workspace_id)?;
            let registration_revision = record.registration_revision + 1;
            let authorization_revision = record.authorization_revision + 1;
            let registration_revision_sql = sqlite_u64(registration_revision, "registration_revision")?;
            let authorization_revision_sql = sqlite_u64(authorization_revision, "authorization_revision")?;
            let root = authorized_root_from_record(&record);
            let authorization = authorization_fingerprint(
                workspace_id,
                &root,
                authorization_revision,
            )?;
            transaction.execute(
                "UPDATE workspace_registration SET registration_revision=?2, authorization_revision=?3, authorization_fingerprint=?4, context_fingerprint=?5, updated_at=?6 WHERE workspace_id=?1",
                params![workspace_id.as_slice(), registration_revision_sql, authorization_revision_sql, authorization.as_slice(), profile_fingerprint.as_slice(), &now],
            )?;
            audit(transaction, Some(workspace_id), 1_040, "workspace-configure", &now)?;
            read_workspace(transaction, workspace_id)
        })
    }

    /// Mark reconciliation requested without fabricating a lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns a not-found or store error.
    pub fn reconcile(
        &mut self,
        workspace_id: [u8; 16],
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        self.mutate(workspace_id, 1_050, "workspace-reconcile", |transaction, _| {
            let exists: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM workspace_registration WHERE workspace_id=?1 AND status_code != ?2",
                params![workspace_id.as_slice(), WorkspaceRegistryLifecycle::Removed as u16],
                |row| row.get(0),
            )?;
            if exists != 1 {
                return Err(WorkspaceRegistryError::NotFound(public_workspace_id(workspace_id)));
            }
            let updated = transaction.execute(
                "UPDATE worktree_state SET reconcile_required=1 WHERE workspace_id=?1",
                [workspace_id.as_slice()],
            )?;
            if updated != 1 {
                return Err(WorkspaceRegistryError::Persisted(
                    "workspace registration has no worktree_state row".into(),
                ));
            }
            Ok(())
        })
    }

    /// Retire a disabled/failed registration, optionally purging unleased operational data.
    ///
    /// # Errors
    ///
    /// Returns a confirmation, lease, transition, not-found, or store error.
    pub fn remove(
        &mut self,
        workspace_id: [u8; 16],
        policy: RemovalPolicy,
        purge_confirmations: u8,
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        if policy == RemovalPolicy::PurgeData && purge_confirmations != 2 {
            return Err(WorkspaceRegistryError::PurgeConfirmation);
        }
        let now = timestamp()?;
        self.store.write_transaction(|transaction| {
            if policy == RemovalPolicy::PurgeData && active_lease_count(transaction, workspace_id)? != 0 {
                return Err(WorkspaceRegistryError::ActiveLease);
            }
            transition_workspace(transaction, workspace_id, "remove", "no-active-leases", &now)?;
            if policy == RemovalPolicy::PurgeData {
                purge_workspace_operational_data(transaction, workspace_id)?;
            }
            transition_workspace(
                transaction,
                workspace_id,
                "removal-complete",
                "retention-policy-applied",
                &now,
            )?;
            transaction.execute(
                "UPDATE worktree_registration SET removed_at=?2 WHERE worktree_id=(SELECT worktree_id FROM workspace_registration WHERE workspace_id=?1)",
                params![workspace_id.as_slice(), &now],
            )?;
            audit(transaction, Some(workspace_id), 1_060, "workspace-remove", &now)?;
            read_workspace(transaction, workspace_id)
        })
    }

    fn mutate(
        &mut self,
        workspace_id: [u8; 16],
        event_code: u16,
        event_name: &'static str,
        operation: impl FnOnce(&Transaction<'_>, &str) -> Result<(), WorkspaceRegistryError>,
    ) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
        let now = timestamp()?;
        self.store.write_transaction(|transaction| {
            operation(transaction, &now)?;
            audit(
                transaction,
                Some(workspace_id),
                event_code,
                event_name,
                &now,
            )?;
            read_workspace(transaction, workspace_id)
        })
    }
}

fn workspace_select() -> &'static str {
    "SELECT workspace_id, workspace_registration_nonce, registration_revision, administrative_key, root_path_bytes, root_path_display, root_directory_file_identity, platform_code, case_sensitivity_mode, authorization_revision, allowed_source_disclosure_rules, repository_id, worktree_id, authorization_fingerprint, context_fingerprint, status_code, created_at, updated_at FROM workspace_registration"
}

fn row_to_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceRecord> {
    let workspace_id = fixed_blob::<16>(row.get(0)?, "workspace_id")?;
    let workspace_registration_nonce = fixed_blob::<16>(row.get(1)?, "workspace nonce")?;
    let registration_revision = u64::try_from(row.get::<_, i64>(2)?)
        .map_err(|_| conversion_error("registration_revision"))?;
    let platform_code =
        u8::try_from(row.get::<_, i64>(7)?).map_err(|_| conversion_error("platform_code"))?;
    let authorization_revision = u64::try_from(row.get::<_, i64>(9)?)
        .map_err(|_| conversion_error("authorization_revision"))?;
    let disclosure_bytes: Vec<u8> = row.get(10)?;
    let allowed_source_disclosure_rules = serde_json::from_slice(&disclosure_bytes)
        .map_err(|_| conversion_error("allowed_source_disclosure_rules"))?;
    let repository_id = optional_fixed_blob::<16>(row.get(11)?, "repository_id")?;
    let worktree_id = optional_fixed_blob::<16>(row.get(12)?, "worktree_id")?;
    let authorization_fingerprint = fixed_blob::<32>(row.get(13)?, "authorization fingerprint")?;
    let context_fingerprint = fixed_blob::<32>(row.get(14)?, "context fingerprint")?;
    let status_code =
        u16::try_from(row.get::<_, i64>(15)?).map_err(|_| conversion_error("status_code"))?;
    let status = WorkspaceRegistryLifecycle::try_from(status_code)
        .map_err(|_| conversion_error("status_code"))?;
    Ok(WorkspaceRecord {
        workspace_id,
        workspace_registration_nonce,
        registration_revision,
        administrative_key: row.get(3)?,
        root_path_bytes: row.get(4)?,
        root_path_display: row.get(5)?,
        root_directory_file_identity: row.get(6)?,
        platform_code,
        case_sensitivity_mode: row.get(8)?,
        authorization_revision,
        allowed_source_disclosure_rules,
        repository_id,
        worktree_id,
        authorization_fingerprint,
        context_fingerprint,
        status,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn read_workspace(
    transaction: &Transaction<'_>,
    workspace_id: [u8; 16],
) -> Result<WorkspaceRecord, WorkspaceRegistryError> {
    transaction
        .query_row(
            &format!("{} WHERE workspace_id=?1", workspace_select()),
            [workspace_id.as_slice()],
            row_to_workspace,
        )
        .optional()?
        .ok_or_else(|| WorkspaceRegistryError::NotFound(public_workspace_id(workspace_id)))
}

fn transition_workspace(
    transaction: &Transaction<'_>,
    workspace_id: [u8; 16],
    event: &str,
    guard: &str,
    now: &str,
) -> Result<(), WorkspaceRegistryError> {
    let status_code: u16 = transaction
        .query_row(
            "SELECT status_code FROM workspace_registration WHERE workspace_id=?1",
            [workspace_id.as_slice()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| WorkspaceRegistryError::NotFound(public_workspace_id(workspace_id)))?;
    let prior_state = registry_state_name(WORKSPACE_REGISTRY_LIFECYCLE_VALUES, status_code)
        .ok_or_else(|| WorkspaceRegistryError::Persisted("unknown workspace status code".into()))?;
    let transition = generated_transition(
        WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS,
        prior_state,
        event,
        guard,
    )
    .map_err(|error| WorkspaceRegistryError::StateTransitionViolation {
        prior_state: error.prior_state,
        event: error.event,
        guard: error.guard,
    })?;
    let next_code = WORKSPACE_REGISTRY_LIFECYCLE_VALUES
        .iter()
        .find(|state| state.name == transition.to)
        .map(|state| state.code)
        .ok_or_else(|| WorkspaceRegistryError::Persisted("transition target is absent".into()))?;
    transaction.execute(
        "UPDATE workspace_registration SET status_code=?2, updated_at=?3 WHERE workspace_id=?1",
        params![workspace_id.as_slice(), next_code, now],
    )?;
    Ok(())
}

fn authorize_root(root: &Path) -> Result<AuthorizedRoot, WorkspaceRegistryError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| WorkspaceRegistryError::Root(error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(WorkspaceRegistryError::Root(
            "root must be a non-symlink directory".into(),
        ));
    }
    let path =
        fs::canonicalize(root).map_err(|error| WorkspaceRegistryError::Root(error.to_string()))?;
    let bytes = path.as_os_str().as_bytes().to_vec();
    let mut file_identity = Vec::with_capacity(16);
    file_identity.extend(metadata.dev().to_be_bytes());
    file_identity.extend(metadata.ino().to_be_bytes());
    let case_mode = match probe_case_sensitivity(&path)? {
        CaseSensitivityMode::Sensitive => "sensitive",
        CaseSensitivityMode::Insensitive => "insensitive",
    }
    .to_owned();
    Ok(AuthorizedRoot {
        display: path.to_string_lossy().into_owned(),
        bytes,
        file_identity,
        platform_code: platform_code(),
        case_mode,
        disclosure_rules: DEFAULT_DISCLOSURE_RULES
            .iter()
            .map(ToString::to_string)
            .collect(),
    })
}

fn authorized_root_from_record(record: &WorkspaceRecord) -> AuthorizedRoot {
    AuthorizedRoot {
        bytes: record.root_path_bytes.clone(),
        display: record.root_path_display.clone(),
        file_identity: record.root_directory_file_identity.clone(),
        platform_code: record.platform_code,
        case_mode: record.case_sensitivity_mode.clone(),
        disclosure_rules: record.allowed_source_disclosure_rules.clone(),
    }
}

fn authorization_fingerprint(
    workspace_id: [u8; 16],
    root: &AuthorizedRoot,
    revision: u64,
) -> Result<[u8; 32], WorkspaceRegistryError> {
    root_authorization_fingerprint(&RootAuthorizationInput {
        workspace_id,
        root_path_bytes: root.bytes.clone(),
        root_directory_file_identity: root.file_identity.clone(),
        platform_code: root.platform_code,
        case_sensitivity_mode: root.case_mode.clone(),
        authorization_revision: revision,
        allowed_source_disclosure_rules: root.disclosure_rules.clone(),
    })
    .map_err(Into::into)
}

fn reject_duplicate_administrative_key(
    transaction: &Transaction<'_>,
    key: &[u8],
) -> Result<(), WorkspaceRegistryError> {
    let count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM workspace_registration WHERE administrative_key=?1 AND status_code != ?2",
        params![key, WorkspaceRegistryLifecycle::Removed as u16],
        |row| row.get(0),
    )?;
    if count == 0 {
        Ok(())
    } else {
        Err(WorkspaceRegistryError::DuplicateAdministrativeKey)
    }
}

fn reject_duplicate_worktree_key(
    transaction: &Transaction<'_>,
    repository_id: [u8; 16],
    key: &[u8],
) -> Result<(), WorkspaceRegistryError> {
    let count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM worktree_registration WHERE repository_id=?1 AND administrative_key=?2 AND removed_at IS NULL",
        params![repository_id.as_slice(), key],
        |row| row.get(0),
    )?;
    if count == 0 {
        Ok(())
    } else {
        Err(WorkspaceRegistryError::DuplicateAdministrativeKey)
    }
}

fn repository_exists(
    transaction: &Transaction<'_>,
    repository_id: [u8; 16],
) -> Result<bool, WorkspaceRegistryError> {
    Ok(transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM repository_registration WHERE repository_id=?1)",
        [repository_id.as_slice()],
        |row| row.get(0),
    )?)
}

fn validate_relink_proof(
    transaction: &Transaction<'_>,
    record: &WorkspaceRecord,
    proof: &RelinkProof,
) -> Result<(), WorkspaceRegistryError> {
    match (record.repository_id, record.worktree_id, proof) {
        (
            None,
            None,
            RelinkProof::NonGit {
                operator_acknowledged: true,
                inventory_matches: true,
            },
        ) => Ok(()),
        (
            Some(repository_id),
            Some(worktree_id),
            RelinkProof::Git {
                repository_id: proof_repository,
                worktree_id: proof_worktree,
                worktree_administrative_key,
            },
        ) if repository_id == *proof_repository && worktree_id == *proof_worktree => {
            let stored: Vec<u8> = transaction.query_row(
                "SELECT administrative_key FROM worktree_registration WHERE worktree_id=?1 AND removed_at IS NULL",
                [worktree_id.as_slice()],
                |row| row.get(0),
            )?;
            if stored == *worktree_administrative_key {
                Ok(())
            } else {
                Err(WorkspaceRegistryError::RelinkProof)
            }
        }
        _ => Err(WorkspaceRegistryError::RelinkProof),
    }
}

fn insert_nested_exclusions(
    transaction: &Transaction<'_>,
    workspace_id: [u8; 16],
    root: &[u8],
    authorization_fingerprint: [u8; 32],
    now: &str,
) -> Result<(), WorkspaceRegistryError> {
    let mut statement = transaction.prepare(
        "SELECT workspace_id, root_path_bytes, authorization_fingerprint FROM workspace_registration WHERE workspace_id != ?1 AND status_code != ?2",
    )?;
    let existing = statement
        .query_map(
            params![
                workspace_id.as_slice(),
                WorkspaceRegistryLifecycle::Removed as u16
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (other_id, other_root, other_fingerprint) in existing {
        let other_id = fixed_blob::<16>(other_id, "nested workspace_id")?;
        let other_fingerprint = fixed_blob::<32>(other_fingerprint, "nested authorization")?;
        let relation = if let Some(relative) = strict_descendant(root, &other_root) {
            Some((workspace_id, other_id, relative, authorization_fingerprint))
        } else {
            strict_descendant(&other_root, root)
                .map(|relative| (other_id, workspace_id, relative, other_fingerprint))
        };
        if let Some((parent, child, relative, fingerprint)) = relation {
            transaction.execute(
                "INSERT OR REPLACE INTO nested_root_exclusion(parent_workspace_id, child_workspace_id, relative_path_bytes, relative_path_display, authorization_fingerprint, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![parent.as_slice(), child.as_slice(), &relative, String::from_utf8_lossy(&relative), fingerprint.as_slice(), now],
            )?;
        }
    }
    Ok(())
}

fn strict_descendant(parent: &[u8], child: &[u8]) -> Option<Vec<u8>> {
    let remainder = child.strip_prefix(parent)?;
    remainder
        .strip_prefix(b"/")
        .filter(|relative| !relative.is_empty())
        .map(ToOwned::to_owned)
}

fn active_lease_count(
    transaction: &Transaction<'_>,
    workspace_id: [u8; 16],
) -> Result<i64, WorkspaceRegistryError> {
    Ok(transaction.query_row(
        "SELECT COUNT(*) FROM snapshot_lease
         WHERE workspace_id=?1 AND state_code IN (?2, ?3, ?4)",
        params![
            workspace_id.as_slice(),
            SnapshotLeaseState::Active as u16,
            SnapshotLeaseState::Releasing as u16,
            SnapshotLeaseState::Orphaned as u16,
        ],
        |row| row.get(0),
    )?)
}

fn purge_workspace_operational_data(
    transaction: &Transaction<'_>,
    workspace_id: [u8; 16],
) -> Result<(), WorkspaceRegistryError> {
    for table in [
        "git_state_vector",
        "git_operation_run",
        "provider_run",
        "update_wave_item",
        "update_wave",
        "hot_overlay_manifest",
        "serving_snapshot_manifest",
        "active_snapshot",
        "worktree_state",
        "workspace_generation",
        "credential_metadata",
    ] {
        let sql = if table == "update_wave_item" {
            "DELETE FROM update_wave_item WHERE wave_id IN (SELECT wave_id FROM update_wave WHERE workspace_id=?1)".to_owned()
        } else {
            format!("DELETE FROM {table} WHERE workspace_id=?1")
        };
        transaction.execute(&sql, [workspace_id.as_slice()])?;
    }
    Ok(())
}

fn audit(
    transaction: &Transaction<'_>,
    workspace_id: Option<[u8; 16]>,
    event_code: u16,
    event_name: &str,
    now: &str,
) -> Result<(), WorkspaceRegistryError> {
    let event_id = random_registration_nonce()?;
    let details_digest = *blake3::hash(event_name.as_bytes()).as_bytes();
    transaction.execute(
        "INSERT INTO audit_event(event_id, workspace_id, event_code, actor_id, occurred_at, details_digest, diagnostic_id) VALUES (?1, ?2, ?3, 'local-admin', ?4, ?5, NULL)",
        params![event_id.as_slice(), workspace_id.as_ref().map(<[u8; 16]>::as_slice), event_code, now, details_digest.as_slice()],
    )?;
    Ok(())
}

fn directory_administrative_key(root: &[u8]) -> Vec<u8> {
    let mut key = b"directory\0".to_vec();
    key.extend(root);
    key
}

fn git_administrative_key(repository_id: [u8; 16], worktree_key: &[u8]) -> Vec<u8> {
    let mut key = b"git\0".to_vec();
    key.extend(repository_id);
    key.push(0);
    key.extend(worktree_key);
    key
}

fn timestamp() -> Result<String, WorkspaceRegistryError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| WorkspaceRegistryError::Root(error.to_string()))?
        .as_millis();
    Ok(format!("{millis:020}"))
}

fn platform_code() -> u8 {
    if cfg!(target_os = "macos") { 2 } else { 1 }
}

fn fixed_blob<const N: usize>(bytes: Vec<u8>, field: &str) -> rusqlite::Result<[u8; N]> {
    bytes.try_into().map_err(|_| conversion_error(field))
}

fn optional_fixed_blob<const N: usize>(
    bytes: Option<Vec<u8>>,
    field: &str,
) -> rusqlite::Result<Option<[u8; N]>> {
    bytes.map(|value| fixed_blob(value, field)).transpose()
}

fn conversion_error(field: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Blob,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            field.to_owned(),
        )),
    )
}

fn sqlite_u64(value: u64, field: &str) -> Result<i64, WorkspaceRegistryError> {
    i64::try_from(value)
        .map_err(|_| WorkspaceRegistryError::Persisted(format!("{field} exceeds SQLite INTEGER")))
}

fn public_workspace_id(workspace_id: [u8; 16]) -> String {
    encode_public_id(IdentityDomain::Workspace, None, workspace_id)
        .unwrap_or_else(|_| "workspace:<invalid>".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{
        repository_registration_identity, workspace_registration_identity,
        worktree_registration_identity,
    };

    fn store() -> (tempfile::TempDir, OperationalStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        (directory, store)
    }

    fn nonces(value: u8) -> RegistrationNonces {
        RegistrationNonces {
            workspace: [value; 16],
            repository: [value.wrapping_add(1); 16],
            worktree: [value.wrapping_add(2); 16],
        }
    }

    #[test]
    fn wp14_behavioral_acceptance() {
        let (directory, mut store) = store();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let workspace_id = {
            let mut registry = WorkspaceRegistry::new(&mut store);
            let record = registry
                .add_with_nonces(&root, WorkspaceSourceRegistration::Directory, nonces(1))
                .unwrap();
            assert_eq!(record.status, WorkspaceRegistryLifecycle::Disabled);
            let record = registry.enable(record.workspace_id).unwrap();
            assert_eq!(record.status, WorkspaceRegistryLifecycle::Bootstrapping);
            assert!(
                generated_transition(
                    WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS,
                    "BOOTSTRAPPING",
                    "not-a-valid-snapshot-event",
                    "snapshot-valid",
                )
                .is_err()
            );
            assert_eq!(
                generated_transition(
                    WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS,
                    "BOOTSTRAPPING",
                    "first-snapshot-active",
                    "snapshot-valid",
                )
                .unwrap()
                .to,
                "READY"
            );
            let record = registry.disable(record.workspace_id).unwrap();
            assert_eq!(record.status, WorkspaceRegistryLifecycle::Disabled);
            record.workspace_id
        };
        drop(store);
        let mut reopened = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let mut registry = WorkspaceRegistry::new(&mut reopened);
        assert_eq!(
            registry.show(workspace_id).unwrap().status,
            WorkspaceRegistryLifecycle::Disabled
        );
        assert!(matches!(
            registry.remove(workspace_id, RemovalPolicy::PurgeData, 1),
            Err(WorkspaceRegistryError::PurgeConfirmation)
        ));
        registry
            .store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO snapshot_lease(
                       lease_id, lease_kind_code, workspace_id, snapshot_id,
                       base_publication_id, required_delta_versions_bytes,
                       requires_overlay, agent_instance_id, created_at,
                       last_heartbeat_at, expires_at, state_code,
                       process_instance_id, orphaned_at, artifact_expires_at,
                       source_blob_lease_id
                     ) VALUES (?1, 10, ?2, ?3, ?4, X'7b7d', 0, NULL,
                               1, 1, 300, 10, ?5, NULL, NULL, NULL)",
                    params![
                        [0x81_u8; 16].as_slice(),
                        workspace_id.as_slice(),
                        [0x82_u8; 16].as_slice(),
                        [0x83_u8; 16].as_slice(),
                        [0x84_u8; 16].as_slice()
                    ],
                )?;
                Ok::<(), WorkspaceRegistryError>(())
            })
            .unwrap();
        assert!(matches!(
            registry.remove(workspace_id, RemovalPolicy::PurgeData, 2),
            Err(WorkspaceRegistryError::ActiveLease)
        ));
        registry
            .store
            .write_transaction(|transaction| {
                transaction.execute(
                    "DELETE FROM snapshot_lease WHERE workspace_id=?1",
                    [workspace_id.as_slice()],
                )?;
                Ok::<(), WorkspaceRegistryError>(())
            })
            .unwrap();
        let removed = registry
            .remove(workspace_id, RemovalPolicy::PurgeData, 2)
            .unwrap();
        assert_eq!(removed.status, WorkspaceRegistryLifecycle::Removed);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One oracle covers the coupled AC-G-09 identity outcomes.
    fn wp14_structural_acceptance() {
        let workspace = workspace_registration_identity([0x11; 16], "Git-Worktree").unwrap();
        assert_eq!(
            workspace.id,
            [
                0xeb, 0x8e, 0x71, 0x6f, 0xd6, 0xee, 0xe2, 0x49, 0x90, 0xb7, 0x20, 0x74, 0xfd, 0x40,
                0x65, 0xad,
            ]
        );
        let repository = repository_registration_identity([0x12; 16]).unwrap();
        let worktree = worktree_registration_identity(repository.id, [0x13; 16], "Linked").unwrap();
        assert_eq!(
            worktree.id,
            [
                0x3d, 0x4b, 0x7a, 0x79, 0x0f, 0x2d, 0x54, 0xbb, 0xad, 0xa6, 0x3e, 0x24, 0x8f, 0x09,
                0xaf, 0x41,
            ]
        );

        let (directory, mut store) = store();
        let first_root = directory.path().join("first");
        let second_root = directory.path().join("second");
        fs::create_dir(&first_root).unwrap();
        fs::create_dir(&second_root).unwrap();
        let mut registry = WorkspaceRegistry::new(&mut store);
        let first = registry
            .add_with_nonces(
                &first_root,
                WorkspaceSourceRegistration::NewGitRepository {
                    worktree_administrative_key: b"MAIN".to_vec(),
                    worktree_kind: "main".into(),
                },
                nonces(10),
            )
            .unwrap();
        let second = registry
            .add_with_nonces(
                &second_root,
                WorkspaceSourceRegistration::ExistingGitRepository {
                    repository_id: first.repository_id.unwrap(),
                    worktree_administrative_key: b"linked".to_vec(),
                    worktree_kind: "linked".into(),
                },
                nonces(20),
            )
            .unwrap();
        let moved_first_root = directory.path().join("first-moved");
        fs::create_dir(&moved_first_root).unwrap();
        let relinked = registry
            .relink(
                first.workspace_id,
                &moved_first_root,
                &RelinkProof::Git {
                    repository_id: first.repository_id.unwrap(),
                    worktree_id: first.worktree_id.unwrap(),
                    worktree_administrative_key: b"MAIN".to_vec(),
                },
            )
            .unwrap();
        assert_eq!(relinked.workspace_id, first.workspace_id);
        assert_eq!(relinked.repository_id, first.repository_id);
        assert_eq!(relinked.worktree_id, first.worktree_id);
        assert_eq!(first.repository_id, second.repository_id);
        assert_ne!(first.worktree_id, second.worktree_id);
        assert_ne!(first.workspace_id, second.workspace_id);
        let public_record = serde_json::to_value(&first).unwrap();
        assert_eq!(public_record["workspace_id"], first.public_id());
        assert!(
            public_record["repository_id"]
                .as_str()
                .unwrap()
                .starts_with("repository:")
        );
        assert!(
            public_record["worktree_id"]
                .as_str()
                .unwrap()
                .starts_with("worktree:")
        );
        let query_protocols = [
            include_str!("../contracts/rpc/cpg_query_service.proto"),
            include_str!("../contracts/rpc/provider_control.proto"),
            include_str!("../contracts/rpc/pyrefly_sidecar.proto"),
            include_str!("../contracts/rpc/rustc_extractor.proto"),
        ]
        .join("\n");
        for admin_verb in [
            "WorkspaceAdmin",
            "AddWorkspace",
            "RemoveWorkspace",
            "RelinkWorkspace",
        ] {
            assert!(!query_protocols.contains(admin_verb));
        }
        let non_git_root = directory.path().join("non-git");
        fs::create_dir(&non_git_root).unwrap();
        let non_git = registry
            .add_with_nonces(
                &non_git_root,
                WorkspaceSourceRegistration::Directory,
                nonces(30),
            )
            .unwrap();
        assert_eq!((non_git.repository_id, non_git.worktree_id), (None, None));

        let parent_root = directory.path().join("parent");
        let child_root = parent_root.join("nested");
        fs::create_dir(&parent_root).unwrap();
        fs::create_dir(&child_root).unwrap();
        let parent = registry
            .add_with_nonces(
                &parent_root,
                WorkspaceSourceRegistration::Directory,
                nonces(31),
            )
            .unwrap();
        let child = registry
            .add_with_nonces(
                &child_root,
                WorkspaceSourceRegistration::Directory,
                nonces(32),
            )
            .unwrap();
        let nested_count = registry
            .store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM nested_root_exclusion WHERE parent_workspace_id=?1 AND child_workspace_id=?2 AND relative_path_bytes=?3",
                    params![parent.workspace_id.as_slice(), child.workspace_id.as_slice(), b"nested"],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(nested_count, 1);
    }

    #[test]
    fn wp14_negative_zero_state() {
        let (directory, mut store) = store();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut registry = WorkspaceRegistry::new(&mut store);
        let first = registry
            .add_with_nonces(&root, WorkspaceSourceRegistration::Directory, nonces(40))
            .unwrap();
        assert!(matches!(
            registry.add_with_nonces(&root, WorkspaceSourceRegistration::Directory, nonces(50)),
            Err(WorkspaceRegistryError::DuplicateAdministrativeKey)
        ));
        assert!(matches!(
            registry.enable([0xff; 16]),
            Err(WorkspaceRegistryError::NotFound(_)
                | WorkspaceRegistryError::StateTransitionViolation { .. })
        ));
        registry
            .remove(first.workspace_id, RemovalPolicy::RetainData, 0)
            .unwrap();
        let second = registry
            .add_with_nonces(&root, WorkspaceSourceRegistration::Directory, nonces(60))
            .unwrap();
        assert_ne!(first.workspace_id, second.workspace_id);
    }

    #[test]
    fn wp14_operational_acceptance() {
        let (directory, mut store) = store();
        let root = directory.path().join("workspace");
        let moved = directory.path().join("moved");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&moved).unwrap();
        let mut registry = WorkspaceRegistry::new(&mut store);
        let record = registry
            .add_with_nonces(&root, WorkspaceSourceRegistration::Directory, nonces(70))
            .unwrap();
        registry.configure(record.workspace_id, [0x44; 32]).unwrap();
        registry
            .relink(
                record.workspace_id,
                &moved,
                &RelinkProof::NonGit {
                    operator_acknowledged: true,
                    inventory_matches: true,
                },
            )
            .unwrap();
        registry.reconcile(record.workspace_id).unwrap();
        registry.enable(record.workspace_id).unwrap();
        registry.disable(record.workspace_id).unwrap();
        registry
            .remove(record.workspace_id, RemovalPolicy::RetainData, 0)
            .unwrap();
        let audit_count = registry
            .store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM audit_event WHERE workspace_id=?1",
                    [record.workspace_id.as_slice()],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(audit_count, 7);

        for transition in WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS {
            assert_eq!(
                generated_transition(
                    WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS,
                    transition.from,
                    transition.event,
                    transition.guard,
                )
                .unwrap(),
                transition
            );
        }
    }
}
