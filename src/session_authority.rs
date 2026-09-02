//! Launch-grant and daemon-session authority for the local v2 boundary.
//!
//! Grants arrive only on the inherited supervisor control channel. A successful handshake
//! consumes one grant and mints one expiring session bound to the kernel-supplied UDS peer.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::fabric::command::{PrincipalId, WorkspaceId};
use crate::rpc::VerifiedPeerIdentity;

pub const SESSION_METADATA_KEY: &str = "codefabric-session-bin";
/// Upper bound projected to the adapter so guarded-input authority cannot outlive a session.
pub const MAXIMUM_CHALLENGE_TTL_SECONDS: u64 = 60;

/// Opaque operator-selected identity of one immutable launch policy.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LaunchPolicyId(String);

impl LaunchPolicyId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SessionAuthorityError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':')
            })
        {
            return Err(SessionAuthorityError::InvalidPolicyIdentity);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn is_valid(&self) -> bool {
        Self::try_new(self.0.clone()).is_ok()
    }
}

/// Monotone immutable revision of one operator-owned launch policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LaunchPolicyRevision(u64);

impl LaunchPolicyRevision {
    pub const fn new(value: u64) -> Result<Self, SessionAuthorityError> {
        if value == 0 {
            return Err(SessionAuthorityError::InvalidPolicyIdentity);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotone operator-provided revocation horizon carried by grants and sessions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RevocationGeneration(u64);

impl RevocationGeneration {
    pub const fn new(value: u64) -> Result<Self, SessionAuthorityError> {
        if value == 0 {
            return Err(SessionAuthorityError::InvalidPolicyIdentity);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOperation {
    Status,
    Reference,
    Validate,
    Start,
    Watch,
    Cancel,
    ReadResource,
    ReleaseResource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredLaunchGrant {
    pub grant_id: String,
    pub grant_digest: [u8; 32],
    pub policy_id: LaunchPolicyId,
    pub policy_revision: LaunchPolicyRevision,
    pub revocation_generation: RevocationGeneration,
    pub principal_id: [u8; 16],
    pub workspace_ids: Vec<[u8; 16]>,
    pub operations: BTreeSet<SessionOperation>,
    pub semantic_profiles: BTreeSet<String>,
    pub maximum_resource_chunk_bytes: u64,
    pub maximum_result_bytes: u64,
    pub maximum_result_pages: u64,
    pub maximum_request_state_ttl_seconds: u64,
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub daemon_generation: u64,
    pub supervisor_generation: u64,
    pub peer_uid: u32,
    pub peer_pid: Option<u32>,
    pub peer_start_identity: Option<String>,
}

impl RegisteredLaunchGrant {
    fn validate(&self) -> Result<(), SessionAuthorityError> {
        if self.grant_id.is_empty()
            || self.grant_digest == [0; 32]
            || !self.policy_id.is_valid()
            || self.policy_revision.get() == 0
            || self.revocation_generation.get() == 0
            || self.principal_id == [0; 16]
            || self.workspace_ids.is_empty()
            || self.operations.is_empty()
            || self.semantic_profiles.is_empty()
            || self.maximum_resource_chunk_bytes == 0
            || self.maximum_result_bytes == 0
            || self.maximum_result_pages == 0
            || self.maximum_request_state_ttl_seconds == 0
            || self.maximum_request_state_ttl_seconds > MAXIMUM_CHALLENGE_TTL_SECONDS
            || self.issued_at_unix_ms <= 0
            || self.expires_at_unix_ms <= self.issued_at_unix_ms
            || self.daemon_generation == 0
            || self.supervisor_generation == 0
            || self.workspace_ids.iter().any(|id| *id == [0; 16])
            || self.peer_start_identity.as_ref().is_some_and(|identity| {
                identity.is_empty() || identity.len() > 128 || !identity.is_ascii()
            })
            || self
                .semantic_profiles
                .iter()
                .any(|profile| profile.is_empty() || profile.len() > 128 || !profile.is_ascii())
        {
            return Err(SessionAuthorityError::InvalidGrant);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AuthorizedSession {
    token: [u8; 32],
    session_id: Arc<str>,
    session_generation: u64,
    principal_id: PrincipalId,
    workspace_ids: Arc<BTreeSet<WorkspaceId>>,
    operations: Arc<BTreeSet<SessionOperation>>,
    semantic_profile: Arc<str>,
    maximum_resource_chunk_bytes: u64,
    maximum_result_bytes: u64,
    maximum_result_pages: u64,
    maximum_request_state_ttl_seconds: u64,
    expires_at_unix_ms: i64,
    daemon_generation: u64,
    supervisor_generation: u64,
    policy_id: LaunchPolicyId,
    policy_revision: LaunchPolicyRevision,
    revocation_generation: RevocationGeneration,
}

impl AuthorizedSession {
    #[must_use]
    pub const fn token(&self) -> &[u8; 32] {
        &self.token
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub const fn session_generation(&self) -> u64 {
        self.session_generation
    }

    #[must_use]
    pub const fn principal_id(&self) -> PrincipalId {
        self.principal_id
    }

    #[must_use]
    pub fn workspace_ids(&self) -> &BTreeSet<WorkspaceId> {
        &self.workspace_ids
    }

    #[must_use]
    pub fn semantic_profile(&self) -> &str {
        &self.semantic_profile
    }

    #[must_use]
    pub const fn maximum_resource_chunk_bytes(&self) -> u64 {
        self.maximum_resource_chunk_bytes
    }

    #[must_use]
    pub const fn maximum_result_bytes(&self) -> u64 {
        self.maximum_result_bytes
    }

    #[must_use]
    pub const fn maximum_result_pages(&self) -> u64 {
        self.maximum_result_pages
    }

    #[must_use]
    pub const fn maximum_request_state_ttl_seconds(&self) -> u64 {
        self.maximum_request_state_ttl_seconds
    }

    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> i64 {
        self.expires_at_unix_ms
    }

    #[must_use]
    pub const fn daemon_generation(&self) -> u64 {
        self.daemon_generation
    }

    #[must_use]
    pub const fn supervisor_generation(&self) -> u64 {
        self.supervisor_generation
    }

    #[must_use]
    pub const fn policy_id(&self) -> &LaunchPolicyId {
        &self.policy_id
    }

    #[must_use]
    pub const fn policy_revision(&self) -> LaunchPolicyRevision {
        self.policy_revision
    }

    /// Wire-compatible projection of the immutable operator policy revision.
    #[must_use]
    pub const fn policy_generation(&self) -> u64 {
        self.policy_revision.get()
    }

    #[must_use]
    pub const fn revocation_generation(&self) -> u64 {
        self.revocation_generation.get()
    }

    #[must_use]
    pub const fn typed_revocation_generation(&self) -> RevocationGeneration {
        self.revocation_generation
    }

    #[must_use]
    pub fn permits(&self, operation: SessionOperation) -> bool {
        self.operations.contains(&operation)
    }

    #[must_use]
    pub fn permits_workspace(&self, workspace_id: WorkspaceId) -> bool {
        self.workspace_ids.contains(&workspace_id)
    }
}

#[derive(Clone, Debug)]
struct SessionRecord {
    session: AuthorizedSession,
    grant_id: Arc<str>,
    grant_digest: [u8; 32],
    peer_uid: u32,
    peer_pid: Option<u32>,
    peer_start_identity: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct PolicyAuthorityState {
    latest_revision: LaunchPolicyRevision,
    revocation_generation: RevocationGeneration,
}

#[derive(Debug)]
struct AuthorityState {
    daemon_generation: u64,
    supervisor_generation: u64,
    next_session_generation: u64,
    grants: BTreeMap<[u8; 32], RegisteredLaunchGrant>,
    sessions: BTreeMap<[u8; 32], SessionRecord>,
    policies: BTreeMap<LaunchPolicyId, PolicyAuthorityState>,
    principal_revocations: BTreeMap<PrincipalId, RevocationGeneration>,
}

/// Volatile authority populated only by the inherited supervisor control stream.
#[derive(Debug)]
pub struct LaunchGrantAuthority {
    session_secret: [u8; 32],
    maximum_grants: usize,
    maximum_sessions: usize,
    state: Mutex<AuthorityState>,
}

impl LaunchGrantAuthority {
    pub fn try_new(
        daemon_generation: u64,
        supervisor_generation: u64,
        maximum_grants: usize,
        maximum_sessions: usize,
    ) -> Result<Self, SessionAuthorityError> {
        if daemon_generation == 0
            || supervisor_generation == 0
            || maximum_grants == 0
            || maximum_sessions == 0
        {
            return Err(SessionAuthorityError::InvalidAuthority);
        }
        let first = crate::identity::random_registration_nonce()
            .map_err(|_| SessionAuthorityError::Entropy)?;
        let second = crate::identity::random_registration_nonce()
            .map_err(|_| SessionAuthorityError::Entropy)?;
        let mut secret = [0_u8; 32];
        secret[..16].copy_from_slice(&first);
        secret[16..].copy_from_slice(&second);
        Ok(Self {
            session_secret: secret,
            maximum_grants,
            maximum_sessions,
            state: Mutex::new(AuthorityState {
                daemon_generation,
                supervisor_generation,
                next_session_generation: 1,
                grants: BTreeMap::new(),
                sessions: BTreeMap::new(),
                policies: BTreeMap::new(),
                principal_revocations: BTreeMap::new(),
            }),
        })
    }

    pub async fn register(
        &self,
        grant: RegisteredLaunchGrant,
    ) -> Result<(), SessionAuthorityError> {
        grant.validate()?;
        let mut state = self.state.lock().await;
        if grant.daemon_generation != state.daemon_generation
            || grant.supervisor_generation != state.supervisor_generation
        {
            return Err(SessionAuthorityError::GenerationMismatch);
        }
        if state.grants.len() >= self.maximum_grants {
            return Err(SessionAuthorityError::Capacity);
        }
        if state.grants.contains_key(&grant.grant_digest) {
            return Err(SessionAuthorityError::Replay);
        }
        let current = state.policies.get(&grant.policy_id).copied();
        if current.is_some_and(|authority| grant.policy_revision < authority.latest_revision) {
            return Err(SessionAuthorityError::StalePolicyRevision);
        }
        if current
            .is_some_and(|authority| grant.revocation_generation < authority.revocation_generation)
        {
            return Err(SessionAuthorityError::StaleRevocation);
        }
        if current.is_some_and(|authority| {
            grant.policy_revision > authority.latest_revision
                || grant.revocation_generation > authority.revocation_generation
        }) {
            state.grants.retain(|_, existing| {
                existing.policy_id != grant.policy_id
                    || (existing.policy_revision >= grant.policy_revision
                        && existing.revocation_generation >= grant.revocation_generation)
            });
        }
        let latest_revision = current.map_or(grant.policy_revision, |authority| {
            authority.latest_revision.max(grant.policy_revision)
        });
        state.policies.insert(
            grant.policy_id.clone(),
            PolicyAuthorityState {
                latest_revision,
                revocation_generation: grant.revocation_generation,
            },
        );
        state.grants.insert(grant.grant_digest, grant);
        Ok(())
    }

    pub async fn revoke_principal(
        &self,
        principal_id: PrincipalId,
        revocation_generation: RevocationGeneration,
    ) -> Result<(), SessionAuthorityError> {
        let mut state = self.state.lock().await;
        if state
            .principal_revocations
            .get(&principal_id)
            .is_some_and(|current| revocation_generation <= *current)
        {
            return Err(SessionAuthorityError::StaleRevocation);
        }
        state
            .principal_revocations
            .insert(principal_id, revocation_generation);
        state
            .grants
            .retain(|_, grant| grant.principal_id != *principal_id.as_bytes());
        state
            .sessions
            .retain(|_, session| session.session.principal_id != principal_id);
        Ok(())
    }

    /// Revoke the one pending grant or authorized session owned by an exact supervised launch.
    ///
    /// The launch ID alone is not authority: the daemon also requires the immutable grant digest
    /// and kernel-bound adapter PID recorded at registration/consumption. A substituted child or
    /// grant leaves both the grant and any resulting session intact.
    pub async fn revoke_launch(
        &self,
        grant_id: &str,
        grant_digest: [u8; 32],
        adapter_pid: u32,
        adapter_start_identity: Option<&str>,
    ) -> Result<(), SessionAuthorityError> {
        if grant_id.is_empty() || grant_digest == [0; 32] || adapter_pid <= 1 {
            return Err(SessionAuthorityError::InvalidGrant);
        }
        let mut state = self.state.lock().await;
        let exact_grant = state.grants.get(&grant_digest).is_some_and(|grant| {
            grant.grant_id == grant_id
                && grant.peer_pid == Some(adapter_pid)
                && grant.peer_start_identity.as_deref() == adapter_start_identity
        });
        let exact_session = state.sessions.values().any(|record| {
            record.grant_id.as_ref() == grant_id
                && record.grant_digest == grant_digest
                && record.peer_pid == Some(adapter_pid)
                && record.peer_start_identity.as_deref() == adapter_start_identity
        });
        let substituted = state.grants.values().any(|grant| {
            grant.grant_id == grant_id
                && (grant.grant_digest != grant_digest
                    || grant.peer_pid != Some(adapter_pid)
                    || grant.peer_start_identity.as_deref() != adapter_start_identity)
        }) || state.sessions.values().any(|record| {
            record.grant_id.as_ref() == grant_id
                && (record.grant_digest != grant_digest
                    || record.peer_pid != Some(adapter_pid)
                    || record.peer_start_identity.as_deref() != adapter_start_identity)
        });
        if substituted {
            return Err(SessionAuthorityError::GrantBinding);
        }
        if !exact_grant && !exact_session {
            return Err(SessionAuthorityError::UnknownGrant);
        }
        if exact_grant {
            state.grants.remove(&grant_digest);
        }
        state.sessions.retain(|_, record| {
            record.grant_id.as_ref() != grant_id
                || record.grant_digest != grant_digest
                || record.peer_pid != Some(adapter_pid)
                || record.peer_start_identity.as_deref() != adapter_start_identity
        });
        Ok(())
    }

    pub async fn advance_generation(
        &self,
        daemon_generation: u64,
        supervisor_generation: u64,
    ) -> Result<(), SessionAuthorityError> {
        let mut state = self.state.lock().await;
        if daemon_generation <= state.daemon_generation
            || supervisor_generation < state.supervisor_generation
        {
            return Err(SessionAuthorityError::GenerationMismatch);
        }
        state.daemon_generation = daemon_generation;
        state.supervisor_generation = supervisor_generation;
        state.next_session_generation = 1;
        state.grants.clear();
        state.sessions.clear();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn consume_grant(
        &self,
        raw_grant: &[u8],
        peer: VerifiedPeerIdentity,
        desired_profiles: &[String],
        client_maximum_resource_chunk_bytes: u64,
        observed_at_unix_ms: i64,
    ) -> Result<AuthorizedSession, SessionAuthorityError> {
        if raw_grant.len() < 32 || raw_grant.len() > 512 {
            return Err(SessionAuthorityError::InvalidGrant);
        }
        let digest = *blake3::hash(raw_grant).as_bytes();
        let observed_peer_start_identity = peer.pid().and_then(observed_process_start_identity);
        let mut state = self.state.lock().await;
        let grant = state
            .grants
            .remove(&digest)
            .ok_or(SessionAuthorityError::UnknownGrant)?;
        let policy = state
            .policies
            .get(&grant.policy_id)
            .copied()
            .ok_or(SessionAuthorityError::GrantBinding)?;
        if grant.peer_uid != peer.uid()
            || grant.peer_pid.is_some_and(|pid| Some(pid) != peer.pid())
            || grant.peer_start_identity != observed_peer_start_identity
            || grant.daemon_generation != state.daemon_generation
            || grant.supervisor_generation != state.supervisor_generation
            || observed_at_unix_ms < grant.issued_at_unix_ms
            || observed_at_unix_ms >= grant.expires_at_unix_ms
            || state
                .principal_revocations
                .get(&PrincipalId::from_bytes(grant.principal_id))
                .copied()
                .is_some_and(|generation| generation > grant.revocation_generation)
            || policy.latest_revision > grant.policy_revision
            || policy.revocation_generation > grant.revocation_generation
        {
            return Err(SessionAuthorityError::GrantBinding);
        }
        if state.sessions.len() >= self.maximum_sessions {
            return Err(SessionAuthorityError::Capacity);
        }
        let profile = desired_profiles
            .iter()
            .find(|profile| grant.semantic_profiles.contains(*profile))
            .or_else(|| grant.semantic_profiles.iter().next())
            .cloned()
            .ok_or(SessionAuthorityError::ProfileUnavailable)?;
        let resource_bound = grant
            .maximum_resource_chunk_bytes
            .min(client_maximum_resource_chunk_bytes);
        if resource_bound == 0 {
            return Err(SessionAuthorityError::InvalidGrant);
        }
        let nonce = crate::identity::random_registration_nonce()
            .map_err(|_| SessionAuthorityError::Entropy)?;
        let mut hasher = blake3::Hasher::new_keyed(&self.session_secret);
        frame(&mut hasher, b"codefabric.daemon-session.v2");
        frame(&mut hasher, &digest);
        frame(&mut hasher, &nonce);
        frame(&mut hasher, &peer.uid().to_be_bytes());
        frame(&mut hasher, &peer.pid().unwrap_or_default().to_be_bytes());
        frame(&mut hasher, &state.daemon_generation.to_be_bytes());
        frame(&mut hasher, grant.policy_id.as_str().as_bytes());
        frame(&mut hasher, &grant.policy_revision.get().to_be_bytes());
        frame(
            &mut hasher,
            &grant.revocation_generation.get().to_be_bytes(),
        );
        let token = *hasher.finalize().as_bytes();
        let session_generation = state.next_session_generation;
        state.next_session_generation = state
            .next_session_generation
            .checked_add(1)
            .ok_or(SessionAuthorityError::Capacity)?;
        let public_session_id = blake3::keyed_hash(&self.session_secret, &token);
        let session = AuthorizedSession {
            token,
            session_id: Arc::from(format!("session:b3:{}", hex(public_session_id.as_bytes()))),
            session_generation,
            principal_id: PrincipalId::from_bytes(grant.principal_id),
            workspace_ids: Arc::new(
                grant
                    .workspace_ids
                    .into_iter()
                    .map(WorkspaceId::from_bytes)
                    .collect(),
            ),
            operations: Arc::new(grant.operations),
            semantic_profile: Arc::from(profile),
            maximum_resource_chunk_bytes: resource_bound,
            maximum_result_bytes: grant.maximum_result_bytes,
            maximum_result_pages: grant.maximum_result_pages,
            maximum_request_state_ttl_seconds: grant.maximum_request_state_ttl_seconds,
            expires_at_unix_ms: grant.expires_at_unix_ms,
            daemon_generation: state.daemon_generation,
            supervisor_generation: state.supervisor_generation,
            policy_id: grant.policy_id,
            policy_revision: grant.policy_revision,
            revocation_generation: grant.revocation_generation,
        };
        state.sessions.insert(
            token,
            SessionRecord {
                session: session.clone(),
                grant_id: Arc::from(grant.grant_id),
                grant_digest: digest,
                peer_uid: peer.uid(),
                peer_pid: peer.pid(),
                peer_start_identity: observed_peer_start_identity,
            },
        );
        Ok(session)
    }

    pub async fn authorize(
        &self,
        token: &[u8],
        peer: VerifiedPeerIdentity,
        operation: SessionOperation,
        workspace_id: Option<WorkspaceId>,
        observed_at_unix_ms: i64,
    ) -> Result<AuthorizedSession, SessionAuthorityError> {
        let token: [u8; 32] = token
            .try_into()
            .map_err(|_| SessionAuthorityError::InvalidSession)?;
        let state = self.state.lock().await;
        let record = state
            .sessions
            .get(&token)
            .ok_or(SessionAuthorityError::InvalidSession)?;
        let policy = state
            .policies
            .get(record.session.policy_id())
            .copied()
            .ok_or(SessionAuthorityError::SessionBinding)?;
        if record.peer_uid != peer.uid()
            || record.peer_pid.is_some_and(|pid| Some(pid) != peer.pid())
            || record.peer_start_identity != peer.pid().and_then(observed_process_start_identity)
            || record.session.daemon_generation != state.daemon_generation
            || record.session.supervisor_generation != state.supervisor_generation
            || observed_at_unix_ms >= record.session.expires_at_unix_ms
            || state
                .principal_revocations
                .get(&record.session.principal_id)
                .copied()
                .is_some_and(|generation| generation > record.session.typed_revocation_generation())
            || policy.latest_revision > record.session.policy_revision()
            || policy.revocation_generation > record.session.typed_revocation_generation()
        {
            return Err(SessionAuthorityError::SessionBinding);
        }
        if !record.session.permits(operation)
            || workspace_id.is_some_and(|workspace| !record.session.permits_workspace(workspace))
        {
            return Err(SessionAuthorityError::OperationDenied);
        }
        Ok(record.session.clone())
    }

    pub async fn generations(&self) -> (u64, u64) {
        let state = self.state.lock().await;
        (state.daemon_generation, state.supervisor_generation)
    }
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

#[cfg(target_os = "linux")]
pub(crate) fn observed_process_start_identity(pid: u32) -> Option<String> {
    let bytes = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    if bytes.len() > 4_096 {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    let close = text.rfind(')')?;
    let start_ticks = text.get(close + 2..)?.split_whitespace().nth(19)?;
    Some(format!("linux-proc-start:{start_ticks}"))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn observed_process_start_identity(_pid: u32) -> Option<String> {
    None
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SessionAuthorityError {
    #[error("launch/session authority configuration is invalid")]
    InvalidAuthority,
    #[error("launch policy identity, revision, or revocation generation is invalid")]
    InvalidPolicyIdentity,
    #[error("launch/session entropy is unavailable")]
    Entropy,
    #[error("launch grant is invalid")]
    InvalidGrant,
    #[error("launch/session authority reached capacity")]
    Capacity,
    #[error("launch grant or session was replayed")]
    Replay,
    #[error("launch grant was not registered")]
    UnknownGrant,
    #[error("launch grant binding is invalid")]
    GrantBinding,
    #[error("daemon or supervisor generation is invalid")]
    GenerationMismatch,
    #[error("principal revocation is stale")]
    StaleRevocation,
    #[error("launch policy revision is stale")]
    StalePolicyRevision,
    #[error("semantic profile is unavailable")]
    ProfileUnavailable,
    #[error("daemon session is invalid")]
    InvalidSession,
    #[error("daemon session binding is invalid")]
    SessionBinding,
    #[error("daemon session does not authorize this operation")]
    OperationDenied,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdUnixStream;

    use crate::rpc::SameUserInterceptor;

    fn grant(raw: &[u8], peer: VerifiedPeerIdentity) -> RegisteredLaunchGrant {
        RegisteredLaunchGrant {
            grant_id: "launch:test".to_owned(),
            grant_digest: *blake3::hash(raw).as_bytes(),
            policy_id: LaunchPolicyId::try_new("policy-one").unwrap(),
            policy_revision: LaunchPolicyRevision::new(4).unwrap(),
            revocation_generation: RevocationGeneration::new(7).unwrap(),
            principal_id: [0x11; 16],
            workspace_ids: vec![[0x22; 16]],
            operations: BTreeSet::from([SessionOperation::Status, SessionOperation::Start]),
            semantic_profiles: BTreeSet::from(["codefabric.semantic-query.v2".to_owned()]),
            maximum_resource_chunk_bytes: 1_024,
            maximum_result_bytes: 8_192,
            maximum_result_pages: 8,
            maximum_request_state_ttl_seconds: 30,
            issued_at_unix_ms: 100,
            expires_at_unix_ms: 1_000,
            daemon_generation: 7,
            supervisor_generation: 9,
            peer_uid: peer.uid(),
            peer_pid: peer.pid(),
            peer_start_identity: peer.pid().and_then(observed_process_start_identity),
        }
    }

    fn current_peer() -> VerifiedPeerIdentity {
        let (peer, _other) = StdUnixStream::pair().expect("peer socketpair");
        peer.set_nonblocking(true).expect("peer nonblocking");
        let stream = tokio::net::UnixStream::from_std(peer).expect("Tokio peer stream");
        SameUserInterceptor::new(rustix::process::geteuid().as_raw())
            .authenticate_stream(&stream)
            .expect("kernel peer identity")
    }

    #[tokio::test]
    async fn wp44_int_registered_grant_mints_one_bounded_authorized_session() {
        let peer = current_peer();
        let raw = [0x5a; 32];
        let authority = LaunchGrantAuthority::try_new(7, 9, 4, 4).expect("authority");
        authority.register(grant(&raw, peer)).await.expect("grant");
        let session = authority
            .consume_grant(
                &raw,
                peer,
                &["codefabric.semantic-query.v2".to_owned()],
                512,
                200,
            )
            .await
            .expect("one session");
        assert_eq!(session.maximum_resource_chunk_bytes(), 512);
        assert_eq!(session.maximum_result_bytes(), 8_192);
        assert_eq!(session.maximum_result_pages(), 8);
        assert_eq!(session.daemon_generation(), 7);
        assert_eq!(session.supervisor_generation(), 9);
        assert_eq!(session.policy_id().as_str(), "policy-one");
        assert_eq!(session.policy_revision().get(), 4);
        assert_eq!(session.policy_generation(), 4);
        assert_eq!(session.revocation_generation(), 7);
        assert!(session.permits(SessionOperation::Status));
        assert!(session.permits_workspace(WorkspaceId::from_bytes([0x22; 16])));
        authority
            .authorize(
                session.token(),
                peer,
                SessionOperation::Status,
                Some(WorkspaceId::from_bytes([0x22; 16])),
                300,
            )
            .await
            .expect("authorized request");
        assert!(matches!(
            authority.consume_grant(&raw, peer, &[], 512, 200).await,
            Err(SessionAuthorityError::UnknownGrant)
        ));
    }

    #[tokio::test]
    async fn wp44_neg_session_operation_expiry_and_generation_fail_closed() {
        let peer = current_peer();
        let raw = [0x6b; 32];
        let authority = LaunchGrantAuthority::try_new(7, 9, 4, 4).expect("authority");
        authority.register(grant(&raw, peer)).await.expect("grant");
        let session = authority
            .consume_grant(&raw, peer, &[], 512, 200)
            .await
            .expect("session");
        assert!(matches!(
            authority
                .authorize(
                    session.token(),
                    peer,
                    SessionOperation::ReleaseResource,
                    None,
                    300,
                )
                .await,
            Err(SessionAuthorityError::OperationDenied)
        ));
        assert!(matches!(
            authority
                .authorize(session.token(), peer, SessionOperation::Status, None, 1_000,)
                .await,
            Err(SessionAuthorityError::SessionBinding)
        ));
        authority
            .advance_generation(8, 9)
            .await
            .expect("advance generation");
        assert_eq!(authority.generations().await, (8, 9));
        assert!(matches!(
            authority
                .authorize(session.token(), peer, SessionOperation::Status, None, 300,)
                .await,
            Err(SessionAuthorityError::InvalidSession)
        ));
    }

    #[tokio::test]
    async fn wp44_neg_launch_revocation_requires_exact_grant_and_child_binding() {
        let peer = current_peer();
        let adapter_pid = peer.pid().expect("kernel peer PID");
        let adapter_start_identity = peer.pid().and_then(observed_process_start_identity);
        let raw = [0x70; 32];
        let digest = *blake3::hash(&raw).as_bytes();
        let authority = LaunchGrantAuthority::try_new(7, 9, 4, 4).expect("authority");
        authority
            .register(grant(&raw, peer))
            .await
            .expect("launch grant");
        let session = authority
            .consume_grant(&raw, peer, &[], 512, 200)
            .await
            .expect("launch session");

        assert!(matches!(
            authority
                .revoke_launch(
                    "launch:test",
                    digest,
                    adapter_pid.saturating_add(1),
                    adapter_start_identity.as_deref(),
                )
                .await,
            Err(SessionAuthorityError::GrantBinding)
        ));
        authority
            .authorize(session.token(), peer, SessionOperation::Status, None, 300)
            .await
            .expect("substituted child cannot revoke the session");

        authority
            .revoke_launch(
                "launch:test",
                digest,
                adapter_pid,
                adapter_start_identity.as_deref(),
            )
            .await
            .expect("exact launch revocation");
        assert!(matches!(
            authority
                .authorize(session.token(), peer, SessionOperation::Status, None, 300)
                .await,
            Err(SessionAuthorityError::InvalidSession)
        ));
    }

    #[tokio::test]
    async fn wp44_neg_registered_and_live_adapter_start_identity_mismatch_fail_closed() {
        let peer = current_peer();
        let forged_raw = [0x73; 32];
        let forged_authority = LaunchGrantAuthority::try_new(7, 9, 4, 4).unwrap();
        let mut forged = grant(&forged_raw, peer);
        forged.peer_start_identity = Some("linux-proc-start:substituted".to_owned());
        forged_authority.register(forged).await.unwrap();
        assert!(matches!(
            forged_authority
                .consume_grant(&forged_raw, peer, &[], 512, 200)
                .await,
            Err(SessionAuthorityError::GrantBinding)
        ));

        let raw = [0x74; 32];
        let authority = LaunchGrantAuthority::try_new(7, 9, 4, 4).unwrap();
        authority.register(grant(&raw, peer)).await.unwrap();
        let session = authority
            .consume_grant(&raw, peer, &[], 512, 200)
            .await
            .unwrap();
        authority
            .state
            .lock()
            .await
            .sessions
            .get_mut(session.token())
            .unwrap()
            .peer_start_identity = Some("linux-proc-start:substituted".to_owned());
        assert!(matches!(
            authority
                .authorize(session.token(), peer, SessionOperation::Status, None, 300)
                .await,
            Err(SessionAuthorityError::SessionBinding)
        ));
    }

    #[tokio::test]
    async fn wp44_neg_policy_revision_and_revocation_substitution_fail_closed() {
        let peer = current_peer();
        let first_raw = [0x71; 32];
        let authority = LaunchGrantAuthority::try_new(7, 9, 8, 8).expect("authority");
        authority
            .register(grant(&first_raw, peer))
            .await
            .expect("initial policy grant");
        let session = authority
            .consume_grant(&first_raw, peer, &[], 512, 200)
            .await
            .expect("policy-bound session");

        let mut advanced = grant(&[0x72; 32], peer);
        advanced.policy_revision = LaunchPolicyRevision::new(5).unwrap();
        advanced.revocation_generation = RevocationGeneration::new(7).unwrap();
        authority
            .register(advanced)
            .await
            .expect("monotone policy revision");
        assert!(matches!(
            authority
                .authorize(session.token(), peer, SessionOperation::Status, None, 300)
                .await,
            Err(SessionAuthorityError::SessionBinding)
        ));
        let revision_session = authority
            .consume_grant(&[0x72; 32], peer, &[], 512, 300)
            .await
            .expect("replacement-revision session");

        let mut revoked = grant(&[0x73; 32], peer);
        revoked.policy_revision = LaunchPolicyRevision::new(5).unwrap();
        revoked.revocation_generation = RevocationGeneration::new(8).unwrap();
        authority
            .register(revoked)
            .await
            .expect("monotone policy revocation");
        assert!(matches!(
            authority
                .authorize(
                    revision_session.token(),
                    peer,
                    SessionOperation::Status,
                    None,
                    300
                )
                .await,
            Err(SessionAuthorityError::SessionBinding)
        ));

        let mut stale_revision = grant(&[0x74; 32], peer);
        stale_revision.policy_revision = LaunchPolicyRevision::new(4).unwrap();
        stale_revision.revocation_generation = RevocationGeneration::new(8).unwrap();
        assert!(matches!(
            authority.register(stale_revision).await,
            Err(SessionAuthorityError::StalePolicyRevision)
        ));

        let mut stale_revocation = grant(&[0x75; 32], peer);
        stale_revocation.policy_revision = LaunchPolicyRevision::new(5).unwrap();
        stale_revocation.revocation_generation = RevocationGeneration::new(7).unwrap();
        assert!(matches!(
            authority.register(stale_revocation).await,
            Err(SessionAuthorityError::StaleRevocation)
        ));
    }
}
