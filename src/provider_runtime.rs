//! Bounded accepted-handle execution for all fact providers.

#[cfg(any(test, feature = "compatibility-probes"))]
pub(crate) mod fixture;

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rayon::ThreadPool;
use thiserror::Error;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

use crate::cancellation::Cancellation;
use crate::fact_ingest::{ProviderFactMessage, StreamTerminal, bounded_provider_fact_channel};
use crate::identity::{IdentityDomain, decode_public_id};
use crate::operational_store::{OperationalReaderFactory, OperationalStore, ProviderRunRecord};
use crate::registries::{
    PROVIDER_ENTRIES, PROVIDER_EVENT_MAPPINGS, PROVIDER_RESOURCE_PROFILES,
    PROVIDER_RUN_STATE_TRANSITIONS, PROVIDER_RUN_STATE_VALUES, ProviderEntry,
    ProviderResourceProfileEntry, ProviderRunState, generated_transition, registry_state_name,
};
use crate::rpc::generated::codefabric::provider::v1 as wire;
use crate::rpc::generated::codefabric::provider::v1::{
    CancelAcknowledgement, CancelAcknowledgementState, ProviderScopeKind,
};

const EVENT_CHANNEL_CAPACITY: usize = 32;
const OBSERVATION_CHANNEL_CAPACITY: usize = 4;
const ADMISSION_AUDIT_CAPACITY: usize = 64;
const MAX_DIAGNOSTIC_BYTES: usize = 1_024;
const INTENT_NONE: u8 = 0;
const INTENT_CANCEL: u8 = 1;
const INTENT_SUPERSEDE: u8 = 2;

/// Deterministic semantic-provider fault seams governed by AC-G-32/GI-15.
pub const SEMANTIC_PROVIDER_FAULT_POINT_CODES: [&str; 12] = [
    "PROVIDER_ADMISSION",
    "PROVIDER_CHILD_LAUNCH",
    "PROVIDER_HANDSHAKE",
    "PROVIDER_STAGE_CREATION",
    "PROVIDER_CHUNK_WRITE",
    "PROVIDER_CHUNK_ACCEPT",
    "PROVIDER_CHUNK_REJECT",
    "PROVIDER_TERMINAL_VERIFY",
    "PROVIDER_CANCELLATION",
    "PROVIDER_KILL",
    "PROVIDER_CLEANUP",
    "PROVIDER_JOURNAL_TRANSITION",
];

/// Bounded observability fields; all labels come from closed registries or lifecycle phases.
pub const SEMANTIC_PROVIDER_TELEMETRY_FIELDS: [(&str, &str, &str); 10] = [
    ("provider_phase", "code", "provider-run"),
    ("input_bytes", "bytes", "provider-run"),
    ("output_bytes", "bytes", "provider-run"),
    ("memory_high_water", "bytes", "provider-run"),
    ("queue_depth", "jobs", "runtime-sample"),
    ("chunk_count", "chunks", "provider-run"),
    ("cache_hits", "entries", "provider-run"),
    ("cancellation_count", "requests", "provider-run"),
    ("failure_count", "failures", "provider-run"),
    ("wall_time", "microseconds", "provider-run"),
];

/// Domain-owned immutable blob reference used by provider admission.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderBlobReference {
    pub blob_id: String,
    pub content_digest: String,
    pub byte_length: u64,
    pub read_only_uri: String,
}

/// Domain-owned source snapshot lease; no Protobuf type crosses this seam.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderSourceSnapshotLease {
    pub lease_id: String,
    pub workspace_id: String,
    pub source_generation: u64,
    pub source_manifest_digest: String,
    pub expires_at_unix_ms: i64,
    pub blobs: Vec<ProviderBlobReference>,
}

/// Domain-owned provider resource estimate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderResourceEstimate {
    pub input_bytes: u64,
    pub expected_output_bytes: u64,
    pub cpu_weight: u32,
    pub memory_mib: u32,
}

/// Domain-owned provider scope. Numeric values retain the governed wire allocation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderScope {
    pub scope_kind: u16,
    pub scope_id: String,
}

/// Lane-neutral invocation selected after immutable inputs and containment are pinned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticProviderWork {
    pub provider_id: String,
    pub capability_family: String,
    pub workspace_view: crate::source_image::ProviderWorkspaceView,
    pub trust_profile: crate::provider_sandbox::ProviderTrustProfile,
    pub invocation_manifest: Arc<[u8]>,
}

/// In-process payload selected only after the common wire job has crossed the RPC adapter.
#[derive(Clone, Debug, Default)]
pub enum ProviderDirectWork {
    #[default]
    None,
    TreeSitter {
        revision: u64,
        text: crate::provider_types::ProviderText,
    },
    RuffPython {
        revision: u64,
        text: crate::provider_types::ProviderText,
        tree_sitter: crate::tree_sitter_adapter::TreeSitterSnapshot,
    },
    SemanticProcess(SemanticProviderWork),
}

/// Application-owned accepted-provider job. Protobuf conversion lives in [`rpc_adapter`].
#[derive(Clone, Debug, Default)]
pub struct ProviderJob {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub source_generation: u64,
    pub source_snapshot_lease: Option<ProviderSourceSnapshotLease>,
    pub requested_capability_codes: Vec<u32>,
    pub scopes: Vec<ProviderScope>,
    pub priority_class: u16,
    pub resource_estimate: Option<ProviderResourceEstimate>,
    pub deadline_unix_ms: i64,
    pub supersession_key: String,
    pub required_bundle_digests: Vec<String>,
    pub required_schema_digests: Vec<String>,
    pub idempotency_key: String,
    pub resource_profile_id: String,
    pub sandbox_profile_digest: String,
    pub direct_work: ProviderDirectWork,
}

impl ProviderJob {
    /// Closed scope/family portion of the common semantic supersession tuple.
    #[must_use]
    pub fn semantic_supersession_key(scope: &ProviderScope, capability_family: &str) -> String {
        format!(
            "{}:{}:{}",
            scope.scope_kind, scope.scope_id, capability_family
        )
    }

    fn fingerprint(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        for value in [
            self.provider_run_id.as_bytes(),
            self.workspace_id.as_bytes(),
            self.analysis_context_id.as_bytes(),
            self.supersession_key.as_bytes(),
            self.idempotency_key.as_bytes(),
            self.resource_profile_id.as_bytes(),
            self.sandbox_profile_digest.as_bytes(),
        ] {
            bytes.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
            bytes.extend_from_slice(value);
        }
        bytes.extend_from_slice(&self.source_generation.to_be_bytes());
        bytes.extend_from_slice(&self.deadline_unix_ms.to_be_bytes());
        for capability in &self.requested_capability_codes {
            bytes.extend_from_slice(&capability.to_be_bytes());
        }
        if let Some(estimate) = self.resource_estimate {
            bytes.extend_from_slice(&estimate.input_bytes.to_be_bytes());
            bytes.extend_from_slice(&estimate.expected_output_bytes.to_be_bytes());
            bytes.extend_from_slice(&estimate.cpu_weight.to_be_bytes());
            bytes.extend_from_slice(&estimate.memory_mib.to_be_bytes());
        }
        crate::integrity::digest_bytes(&bytes)
    }
}

/// The only Protobuf-to-domain provider job conversion boundary.
pub mod rpc_adapter {
    use super::{
        ProviderBlobReference, ProviderDirectWork, ProviderJob, ProviderResourceEstimate,
        ProviderScope, ProviderSourceSnapshotLease, wire,
    };

    /// Decode an admitted wire job without retaining generated message types.
    #[must_use]
    pub fn decode_job(spec: wire::ProviderJobSpec) -> ProviderJob {
        ProviderJob {
            provider_run_id: spec.provider_run_id,
            workspace_id: spec.workspace_id,
            analysis_context_id: spec.analysis_context_id,
            source_generation: spec.source_generation,
            source_snapshot_lease: spec.source_snapshot_lease.map(|lease| {
                ProviderSourceSnapshotLease {
                    lease_id: lease.lease_id,
                    workspace_id: lease.workspace_id,
                    source_generation: lease.source_generation,
                    source_manifest_digest: lease.source_manifest_digest,
                    expires_at_unix_ms: lease.expires_at_unix_ms,
                    blobs: lease
                        .blobs
                        .into_iter()
                        .map(|blob| ProviderBlobReference {
                            blob_id: blob.blob_id,
                            content_digest: blob.content_digest,
                            byte_length: blob.byte_length,
                            read_only_uri: blob.read_only_uri,
                        })
                        .collect(),
                }
            }),
            requested_capability_codes: spec.requested_capability_codes,
            scopes: spec
                .scopes
                .into_iter()
                .map(|scope| ProviderScope {
                    scope_kind: u16::try_from(scope.scope_kind).unwrap_or_default(),
                    scope_id: scope.scope_id,
                })
                .collect(),
            priority_class: u16::try_from(spec.priority_class).unwrap_or_default(),
            resource_estimate: spec
                .resource_estimate
                .map(|estimate| ProviderResourceEstimate {
                    input_bytes: estimate.input_bytes,
                    expected_output_bytes: estimate.expected_output_bytes,
                    cpu_weight: estimate.cpu_weight,
                    memory_mib: estimate.memory_mib,
                }),
            deadline_unix_ms: spec.deadline_unix_ms,
            supersession_key: spec.supersession_key,
            required_bundle_digests: spec.required_bundle_digests,
            required_schema_digests: spec.required_schema_digests,
            idempotency_key: spec.idempotency_key,
            resource_profile_id: spec.resource_profile_id,
            sandbox_profile_digest: spec.sandbox_profile_digest,
            direct_work: ProviderDirectWork::None,
        }
    }
}

/// Stable provider-runtime failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderRuntimeError {
    #[error("provider job is invalid: {0}")]
    InvalidJob(String),
    #[error("provider admission rejected check {check} for profile {profile_id}")]
    AdmissionRejected {
        profile_id: String,
        check: &'static str,
    },
    #[error("provider run already exists: {0}")]
    DuplicateRun(String),
    #[error("provider run was not found: {0}")]
    RunNotFound(String),
    #[error("provider runtime state transition failed: {0}")]
    StateTransition(String),
    #[error("provider run journal failed: {0}")]
    Journal(String),
    #[error("provider event receiver closed")]
    EventReceiverClosed,
    #[error("provider worker closed without a terminal result")]
    WorkerClosed,
    #[error("provider adapter failed: {code}")]
    Adapter { code: String },
    #[error("provider protocol violation: {0}")]
    Protocol(String),
    #[error("provider runtime could not construct its bounded Rayon pool: {0}")]
    ThreadPool(String),
}

/// One named profile check retained in the bounded admission audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionLimitOutcome {
    pub check: &'static str,
    pub allowed: bool,
}

/// One bounded, inspectable admission decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionAuditRecord {
    pub provider_run_id: String,
    pub provider_id: &'static str,
    pub resource_profile_id: String,
    pub outcomes: Vec<AdmissionLimitOutcome>,
}

/// Snapshot of counters; atomics remain private runtime mechanics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderRuntimeMetrics {
    pub accepted: u64,
    pub rejected: u64,
    pub started: u64,
    pub terminal: u64,
    pub superseded: u64,
    pub cancelled: u64,
    pub timed_out: u64,
    pub stale_results: u64,
    pub backpressure_waits: u64,
}

#[derive(Default)]
struct RuntimeCounters {
    accepted: AtomicU64,
    rejected: AtomicU64,
    started: AtomicU64,
    terminal: AtomicU64,
    superseded: AtomicU64,
    cancelled: AtomicU64,
    timed_out: AtomicU64,
    stale_results: AtomicU64,
    backpressure_waits: AtomicU64,
}

#[derive(Clone, Copy)]
struct ValidatedJobIds {
    run: [u8; 16],
    workspace: [u8; 16],
    context: [u8; 16],
}

impl RuntimeCounters {
    fn snapshot(&self) -> ProviderRuntimeMetrics {
        ProviderRuntimeMetrics {
            accepted: self.accepted.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            started: self.started.load(Ordering::Relaxed),
            terminal: self.terminal.load(Ordering::Relaxed),
            superseded: self.superseded.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
            timed_out: self.timed_out.load(Ordering::Relaxed),
            stale_results: self.stale_results.load(Ordering::Relaxed),
            backpressure_waits: self.backpressure_waits.load(Ordering::Relaxed),
        }
    }
}

/// Identity and generation fence carried structurally by every application event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEventIdentity {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub source_generation: u64,
    pub sequence: u64,
    pub input_fingerprint: [u8; 32],
}

/// Application-owned provider event taxonomy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderEvent {
    Accepted {
        identity: ProviderEventIdentity,
    },
    Progress {
        identity: ProviderEventIdentity,
        completed_units: u64,
        total_units: u64,
        phase: String,
    },
    ScopeBegin {
        identity: ProviderEventIdentity,
        scope: ProviderScope,
    },
    ArrowIpcChunk {
        identity: ProviderEventIdentity,
        observation_family_code: u32,
        arrow_ipc: Vec<u8>,
        schema_digest: String,
        row_count: u64,
        chunk_digest: String,
    },
    ScopeEnd {
        identity: ProviderEventIdentity,
        scope: ProviderScope,
        family_counts: BTreeMap<u32, u64>,
        scope_digest: String,
    },
    Diagnostic {
        identity: ProviderEventIdentity,
        code: String,
        detail: String,
    },
    Completed {
        identity: ProviderEventIdentity,
        state: ProviderRunState,
        output_fingerprint: [u8; 32],
    },
    Failed {
        identity: ProviderEventIdentity,
        state: ProviderRunState,
        code: String,
    },
    CancelAcknowledged {
        identity: ProviderEventIdentity,
        state: ProviderRunState,
    },
}

/// Accepted handle returned before the job waits for admission permits.
pub struct AcceptedProviderJob {
    pub run_id: String,
    pub accepted_generation: u64,
    pub events: mpsc::Receiver<ProviderEvent>,
    pub provider_facts: mpsc::Receiver<ProviderFactMessage>,
    pub cancellation: Cancellation,
}

/// Bounded sink used only by provider adapters running on dedicated CPU capacity.
#[derive(Clone)]
pub struct ProviderEventSink {
    events: mpsc::Sender<ProviderEvent>,
    provider_facts: mpsc::Sender<ProviderFactMessage>,
    counters: Arc<RuntimeCounters>,
    identity: ProviderEventIdentity,
    validated_ids: ValidatedJobIds,
    provider_code: i16,
    next_sequence: Arc<AtomicU64>,
}

impl ProviderEventSink {
    fn next_identity(&self) -> ProviderEventIdentity {
        ProviderEventIdentity {
            sequence: self.next_sequence.fetch_add(1, Ordering::AcqRel),
            ..self.identity.clone()
        }
    }

    /// Emit a typed progress event without allowing an adapter to drift identity.
    ///
    /// # Errors
    ///
    /// Returns `EventReceiverClosed` after the accepted handle is dropped.
    pub fn send_progress(
        &self,
        completed_units: u64,
        total_units: u64,
        phase: impl Into<String>,
    ) -> Result<(), ProviderRuntimeError> {
        self.send_event(ProviderEvent::Progress {
            identity: self.next_identity(),
            completed_units,
            total_units,
            phase: phase.into(),
        })
    }

    /// Emit one application event with observable bounded-channel backpressure.
    ///
    /// # Errors
    ///
    /// Returns `EventReceiverClosed` after the accepted handle is dropped.
    pub fn send_event(&self, event: ProviderEvent) -> Result<(), ProviderRuntimeError> {
        match self.events.try_send(event) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(event)) => {
                self.counters
                    .backpressure_waits
                    .fetch_add(1, Ordering::Relaxed);
                self.events
                    .blocking_send(event)
                    .map_err(|_| ProviderRuntimeError::EventReceiverClosed)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(ProviderRuntimeError::EventReceiverClosed)
            }
        }
    }

    /// Emit one fact-ingest protocol message through its existing bounded channel.
    ///
    /// # Errors
    ///
    /// Returns `EventReceiverClosed` after the observation consumer is dropped.
    pub fn send_provider_fact(
        &self,
        message: ProviderFactMessage,
    ) -> Result<(), ProviderRuntimeError> {
        if let ProviderFactMessage::Manifest(manifest) = &message
            && (manifest.workspace_id != self.validated_ids.workspace
                || manifest.analysis_context_id != self.validated_ids.context
                || manifest.provider_run_id != self.validated_ids.run
                || manifest.source_generation
                    != i64::try_from(self.identity.source_generation).unwrap_or(i64::MAX)
                || manifest.provider_code != self.provider_code)
        {
            return Err(ProviderRuntimeError::Protocol(
                "observation manifest identity differs from accepted job".into(),
            ));
        }
        match self.provider_facts.try_send(message) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(message)) => {
                self.counters
                    .backpressure_waits
                    .fetch_add(1, Ordering::Relaxed);
                self.provider_facts
                    .blocking_send(message)
                    .map_err(|_| ProviderRuntimeError::EventReceiverClosed)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(ProviderRuntimeError::EventReceiverClosed)
            }
        }
    }

    /// Begin the common direct provider-fact stream with runtime-owned identity.
    ///
    /// # Errors
    ///
    /// Returns the same bounded-channel and identity failures as [`Self::send_provider_fact`].
    pub fn begin_provider_facts(
        &self,
        provider_version: impl Into<String>,
        schema_fingerprints: BTreeMap<i16, String>,
        declared_rows: usize,
    ) -> Result<(), ProviderRuntimeError> {
        self.send_provider_fact(ProviderFactMessage::Manifest(
            crate::fact_ingest::ProviderFactManifest {
                stream_id: self.validated_ids.run,
                workspace_id: self.validated_ids.workspace,
                analysis_context_id: self.validated_ids.context,
                source_generation: i64::try_from(self.identity.source_generation)
                    .unwrap_or(i64::MAX),
                provider_code: self.provider_code,
                provider_version: provider_version.into(),
                provider_run_id: self.validated_ids.run,
                emitted_at_micros: system_now_unix_millis().saturating_mul(1_000),
                schema_fingerprints,
                declared_rows,
            },
        ))
    }
}

/// Terminal result returned by a provider adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCompletion {
    pub state: ProviderRunState,
    pub output_fingerprint: [u8; 32],
    pub diagnostic_code: Option<String>,
}

/// Application port implemented by each provider placement adapter.
pub trait ProviderAdapter: Send + Sync {
    /// Run one accepted job on the runtime-selected provider placement.
    ///
    /// # Errors
    ///
    /// Returns a closed provider failure; the runtime maps it to generated lifecycle
    /// state and a bounded diagnostic.
    fn run(
        &self,
        spec: ProviderJob,
        events: ProviderEventSink,
        cancellation: Cancellation,
    ) -> Result<ProviderCompletion, ProviderRuntimeError>;
}

/// Lane implementation port installed behind the shared semantic adapter registrations.
pub trait SemanticProviderDriver: Send + Sync {
    /// Execute one already-admitted, immutable, contained provider invocation.
    ///
    /// # Errors
    ///
    /// Returns only application-owned runtime errors; provider-library types stay behind the lane.
    fn execute(
        &self,
        work: SemanticProviderWork,
        events: ProviderEventSink,
        cancellation: Cancellation,
    ) -> Result<ProviderCompletion, ProviderRuntimeError>;
}

/// Shared adapter registered for each out-of-process semantic provider.
pub struct RegisteredSemanticProviderAdapter {
    provider_id: &'static str,
    driver: Arc<dyn SemanticProviderDriver>,
}

impl RegisteredSemanticProviderAdapter {
    /// Bind one registry provider identity to its lane-owned execution driver.
    ///
    /// # Errors
    ///
    /// Rejects in-process or unknown provider identities.
    pub fn new(
        provider_id: &'static str,
        driver: Arc<dyn SemanticProviderDriver>,
    ) -> Result<Self, ProviderRuntimeError> {
        let provider = PROVIDER_ENTRIES
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .ok_or_else(|| ProviderRuntimeError::InvalidJob("unknown provider_id".into()))?;
        if provider.placement == "IN_PROCESS" {
            return Err(ProviderRuntimeError::InvalidJob(
                "semantic adapter requires out-of-process placement".into(),
            ));
        }
        Ok(Self {
            provider_id,
            driver,
        })
    }
}

impl ProviderAdapter for RegisteredSemanticProviderAdapter {
    fn run(
        &self,
        spec: ProviderJob,
        events: ProviderEventSink,
        cancellation: Cancellation,
    ) -> Result<ProviderCompletion, ProviderRuntimeError> {
        let ProviderDirectWork::SemanticProcess(work) = spec.direct_work else {
            return Err(ProviderRuntimeError::InvalidJob(
                "semantic provider work is absent".into(),
            ));
        };
        let scope = spec.scopes.first().ok_or_else(|| {
            ProviderRuntimeError::InvalidJob("semantic provider scope is absent".into())
        })?;
        if work.provider_id != self.provider_id
            || work.workspace_view.workspace_id
                != decode_public_id(IdentityDomain::Workspace, None, &spec.workspace_id)
                    .map_err(|error| ProviderRuntimeError::InvalidJob(error.to_string()))?
            || work.workspace_view.source_generation != spec.source_generation
            || work.workspace_view.sandbox_profile_digest != spec.sandbox_profile_digest
            || spec.supersession_key
                != ProviderJob::semantic_supersession_key(scope, &work.capability_family)
        {
            return Err(ProviderRuntimeError::InvalidJob(
                "semantic invocation identity differs from accepted job".into(),
            ));
        }
        events.send_progress(0, 1, "sandbox-admission")?;
        self.driver.execute(work, events, cancellation)
    }
}

/// Exact semantic adapter census. Lanes supply drivers; shared consumers resolve by registry ID.
#[derive(Clone)]
pub struct SemanticProviderAdapterRegistry {
    adapters: BTreeMap<&'static str, Arc<dyn ProviderAdapter>>,
}

impl SemanticProviderAdapterRegistry {
    /// Register both governed semantic placements exactly once.
    ///
    /// # Errors
    ///
    /// Rejects a registry identity or placement mismatch.
    pub fn new(
        pyrefly: Arc<dyn SemanticProviderDriver>,
        rustc: Arc<dyn SemanticProviderDriver>,
    ) -> Result<Self, ProviderRuntimeError> {
        let mut adapters = BTreeMap::<&'static str, Arc<dyn ProviderAdapter>>::new();
        for (provider_id, driver) in [("pyrefly-python", pyrefly), ("rustc-mir", rustc)] {
            let adapter = RegisteredSemanticProviderAdapter::new(provider_id, driver)?;
            if adapters.insert(provider_id, Arc::new(adapter)).is_some() {
                return Err(ProviderRuntimeError::InvalidJob(
                    "duplicate semantic adapter registration".into(),
                ));
            }
        }
        Ok(Self { adapters })
    }

    #[must_use]
    pub fn provider_ids(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }

    #[must_use]
    pub fn adapter(&self, provider_id: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters.get(provider_id).cloned()
    }
}

/// Source-generation fence consulted after provider work completes.
pub trait SourceGenerationOracle: Send + Sync {
    /// Return the current stable source generation for one workspace.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when current generation cannot be established.
    fn current_generation(&self, workspace_id: &str) -> Result<u64, ProviderRuntimeError>;
}

/// Time port used by deadline, persistence, and acknowledgement decisions.
pub trait ProviderClock: Send + Sync {
    fn now_unix_millis(&self) -> i64;
}

#[derive(Clone, Copy, Debug, Default)]
struct SystemProviderClock;

impl ProviderClock for SystemProviderClock {
    fn now_unix_millis(&self) -> i64 {
        system_now_unix_millis()
    }
}

/// Production generation fence backed by a read-only operational-store connection.
#[derive(Clone, Debug)]
pub struct OperationalSourceGenerationOracle {
    reader: OperationalReaderFactory,
}

impl OperationalSourceGenerationOracle {
    #[must_use]
    pub const fn new(reader: OperationalReaderFactory) -> Self {
        Self { reader }
    }
}

impl SourceGenerationOracle for OperationalSourceGenerationOracle {
    fn current_generation(&self, workspace_id: &str) -> Result<u64, ProviderRuntimeError> {
        let identity = decode_public_id(IdentityDomain::Workspace, None, workspace_id)
            .map_err(|error| ProviderRuntimeError::InvalidJob(error.to_string()))?;
        let generation = self
            .reader
            .open()
            .map_err(|error| ProviderRuntimeError::Journal(error.to_string()))?
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
                    [identity.as_slice()],
                    |row| row.get::<_, i64>(0),
                )
            })
            .map_err(|error| ProviderRuntimeError::Journal(error.to_string()))?;
        u64::try_from(generation)
            .map_err(|_| ProviderRuntimeError::Journal("negative source generation".into()))
    }
}

/// Operational journal port; providers themselves never receive it.
pub trait ProviderRunJournal: Send + Sync {
    /// Persist the latest generated-state projection for one provider run.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when the operational projection cannot be committed.
    fn record(&self, record: &ProviderRunRecord) -> Result<(), ProviderRuntimeError>;
}

/// The production journal adapter around the sole logical operational-store writer.
pub struct OperationalProviderRunJournal {
    store: Mutex<OperationalStore>,
}

impl OperationalProviderRunJournal {
    #[must_use]
    pub const fn new(store: OperationalStore) -> Self {
        Self {
            store: Mutex::new(store),
        }
    }
}

impl ProviderRunJournal for OperationalProviderRunJournal {
    fn record(&self, record: &ProviderRunRecord) -> Result<(), ProviderRuntimeError> {
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record_provider_run(record)
            .map_err(|error| ProviderRuntimeError::Journal(error.to_string()))
    }
}

type WorkspaceSemaphores = Arc<Mutex<BTreeMap<String, Arc<Semaphore>>>>;
type ContextSemaphores = Arc<Mutex<BTreeMap<(String, String), Arc<Semaphore>>>>;

#[derive(Clone)]
struct AdmissionController {
    global: Arc<Semaphore>,
    workspaces: WorkspaceSemaphores,
    contexts: ContextSemaphores,
    workspace_limit: usize,
    context_limit: usize,
}

impl AdmissionController {
    fn new(profile: &ProviderResourceProfileEntry) -> Self {
        Self {
            global: Arc::new(Semaphore::new(usize::from(
                profile.max_parallel_jobs_global,
            ))),
            workspaces: Arc::new(Mutex::new(BTreeMap::new())),
            contexts: Arc::new(Mutex::new(BTreeMap::new())),
            workspace_limit: usize::from(profile.max_parallel_jobs_per_workspace),
            context_limit: usize::from(profile.max_parallel_jobs_per_context),
        }
    }

    fn workspace(&self, workspace_id: &str) -> Arc<Semaphore> {
        Arc::clone(
            self.workspaces
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry(workspace_id.to_owned())
                .or_insert_with(|| Arc::new(Semaphore::new(self.workspace_limit))),
        )
    }

    fn context(&self, workspace_id: &str, context_id: &str) -> Arc<Semaphore> {
        Arc::clone(
            self.contexts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry((workspace_id.to_owned(), context_id.to_owned()))
                .or_insert_with(|| Arc::new(Semaphore::new(self.context_limit))),
        )
    }

    fn evict_workspace(&self, workspace_id: &str) {
        self.workspaces
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(workspace_id);
        self.contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|(workspace, _), _| workspace != workspace_id);
    }

    #[cfg(test)]
    fn scope_counts(&self) -> (usize, usize) {
        let workspaces = self
            .workspaces
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        let contexts = self
            .contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        (workspaces, contexts)
    }
}

struct RunControl {
    state: AtomicU16,
    intent: AtomicU8,
    cancelled: Arc<AtomicBool>,
    request_notify: Notify,
    terminal_notify: Notify,
    record: Mutex<ProviderRunRecord>,
}

enum RunCompletion {
    Adapter(Result<ProviderCompletion, ProviderRuntimeError>),
    Requested(u8),
    Deadline,
    Closed,
}

/// Runtime context that enriches a wire event with its accepted input fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireEventContext {
    pub provider_id: String,
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub source_generation: u64,
    pub input_fingerprint: [u8; 32],
}

fn mapped_event(
    context: &WireEventContext,
    wire_event: &str,
    application_event: &str,
) -> Result<(), ProviderRuntimeError> {
    let mapping_version = PROVIDER_ENTRIES
        .iter()
        .find(|provider| provider.provider_id == context.provider_id)
        .map(|provider| provider.event_mapping_version)
        .ok_or_else(|| ProviderRuntimeError::Protocol("unknown provider identity".into()))?;
    PROVIDER_EVENT_MAPPINGS
        .iter()
        .any(|mapping| {
            mapping.wire_event == wire_event
                && mapping.application_event == application_event
                && mapping.mapping_version == mapping_version
        })
        .then_some(())
        .ok_or_else(|| {
            ProviderRuntimeError::Protocol(format!(
                "unregistered provider event mapping {wire_event}->{application_event}"
            ))
        })
}

fn wire_identity(
    header: Option<wire::ProviderEventHeader>,
    context: &WireEventContext,
) -> Result<ProviderEventIdentity, ProviderRuntimeError> {
    let header =
        header.ok_or_else(|| ProviderRuntimeError::Protocol("event header absent".into()))?;
    if header.provider_run_id != context.provider_run_id
        || header.workspace_id != context.workspace_id
        || header.analysis_context_id != context.analysis_context_id
        || header.source_generation != context.source_generation
        || header.event_checksum.is_empty()
    {
        return Err(ProviderRuntimeError::Protocol(
            "event identity or checksum differs from accepted job".into(),
        ));
    }
    Ok(ProviderEventIdentity {
        provider_run_id: header.provider_run_id,
        workspace_id: header.workspace_id,
        analysis_context_id: header.analysis_context_id,
        source_generation: header.source_generation,
        sequence: header.sequence,
        input_fingerprint: context.input_fingerprint,
    })
}

/// Map one common wire event through the generated feature-registry taxonomy.
///
/// # Errors
///
/// Rejects unregistered event mappings, missing or mismatched identity headers,
/// unknown terminal states, and malformed terminal digests.
#[allow(clippy::too_many_lines)] // The closed wire event union is mapped exhaustively at one DTO fence.
pub fn map_wire_event(
    event: wire::ProviderEvent,
    context: &WireEventContext,
) -> Result<ProviderEvent, ProviderRuntimeError> {
    match event
        .event
        .ok_or_else(|| ProviderRuntimeError::Protocol("event payload absent".into()))?
    {
        wire::provider_event::Event::Accepted(event) => {
            mapped_event(context, "ACCEPTED", "ACCEPTED")?;
            if event.accepted_generation != context.source_generation {
                return Err(ProviderRuntimeError::Protocol(
                    "accepted generation differs".into(),
                ));
            }
            Ok(ProviderEvent::Accepted {
                identity: wire_identity(event.header, context)?,
            })
        }
        wire::provider_event::Event::Progress(event) => {
            mapped_event(context, "PROGRESS", "PROGRESS")?;
            Ok(ProviderEvent::Progress {
                identity: wire_identity(event.header, context)?,
                completed_units: u64::from(event.completed_units),
                total_units: u64::from(event.total_units),
                phase: event.phase,
            })
        }
        wire::provider_event::Event::ScopeBegin(event) => {
            mapped_event(context, "SCOPE_BEGIN", "SCOPE_BEGIN")?;
            let scope = event
                .scope
                .ok_or_else(|| ProviderRuntimeError::Protocol("scope absent".into()))?;
            Ok(ProviderEvent::ScopeBegin {
                identity: wire_identity(event.header, context)?,
                scope: ProviderScope {
                    scope_kind: u16::try_from(scope.scope_kind).map_err(|_| {
                        ProviderRuntimeError::Protocol("scope kind exceeds domain range".into())
                    })?,
                    scope_id: scope.scope_id,
                },
            })
        }
        wire::provider_event::Event::ObservationChunk(event) => {
            mapped_event(context, "OBSERVATION_CHUNK", "OBSERVATION_CHUNK")?;
            if event.arrow_ipc.is_empty()
                || event.schema_digest.is_empty()
                || event.chunk_digest.is_empty()
            {
                return Err(ProviderRuntimeError::Protocol(
                    "observation chunk payload identity is incomplete".into(),
                ));
            }
            Ok(ProviderEvent::ArrowIpcChunk {
                identity: wire_identity(event.header, context)?,
                observation_family_code: event.observation_family_code,
                arrow_ipc: event.arrow_ipc,
                schema_digest: event.schema_digest,
                row_count: event.row_count,
                chunk_digest: event.chunk_digest,
            })
        }
        wire::provider_event::Event::ScopeEnd(event) => {
            mapped_event(context, "SCOPE_END", "SCOPE_END")?;
            let scope = event
                .scope
                .ok_or_else(|| ProviderRuntimeError::Protocol("scope absent".into()))?;
            Ok(ProviderEvent::ScopeEnd {
                identity: wire_identity(event.header, context)?,
                scope: ProviderScope {
                    scope_kind: u16::try_from(scope.scope_kind).map_err(|_| {
                        ProviderRuntimeError::Protocol("scope kind exceeds domain range".into())
                    })?,
                    scope_id: scope.scope_id,
                },
                family_counts: event.family_counts.into_iter().collect(),
                scope_digest: event.scope_digest,
            })
        }
        wire::provider_event::Event::Terminal(event) => {
            mapped_event(context, "TERMINAL", "TERMINAL")?;
            let identity = wire_identity(event.header, context)?;
            let state = u16::try_from(event.state)
                .ok()
                .and_then(|code| ProviderRunState::try_from(code).ok())
                .ok_or_else(|| ProviderRuntimeError::Protocol("terminal state unknown".into()))?;
            if matches!(
                state,
                ProviderRunState::Succeeded | ProviderRunState::Partial
            ) {
                Ok(ProviderEvent::Completed {
                    identity,
                    state,
                    output_fingerprint: parse_blake3_digest(&event.overall_digest)?,
                })
            } else {
                Ok(ProviderEvent::Failed {
                    identity,
                    state,
                    code: event.error_code.unwrap_or_else(|| state_name(state).into()),
                })
            }
        }
        wire::provider_event::Event::CancelAcknowledged(event) => {
            mapped_event(context, "CANCEL_ACKNOWLEDGED", "CANCEL_ACKNOWLEDGED")?;
            Ok(ProviderEvent::CancelAcknowledged {
                identity: wire_identity(event.header, context)?,
                state: ProviderRunState::Cancelled,
            })
        }
    }
}

impl RunControl {
    fn new(record: ProviderRunRecord) -> Self {
        Self {
            state: AtomicU16::new(ProviderRunState::Queued as u16),
            intent: AtomicU8::new(INTENT_NONE),
            cancelled: Arc::new(AtomicBool::new(false)),
            request_notify: Notify::new(),
            terminal_notify: Notify::new(),
            record: Mutex::new(record),
        }
    }

    fn request(&self, intent: u8) {
        self.intent.fetch_max(intent, Ordering::AcqRel);
        self.cancelled.store(true, Ordering::Release);
        self.request_notify.notify_waiters();
    }

    fn state(&self) -> ProviderRunState {
        ProviderRunState::try_from(self.state.load(Ordering::Acquire))
            .expect("runtime only stores generated provider-run states")
    }
}

/// Common bounded provider runtime. The provider identity is selected at construction;
/// it is not duplicated in every wire job.
#[derive(Clone)]
pub struct ProviderRuntime {
    provider: &'static ProviderEntry,
    profile: &'static ProviderResourceProfileEntry,
    wave_id: Arc<[u8]>,
    adapter: Arc<dyn ProviderAdapter>,
    generation_oracle: Arc<dyn SourceGenerationOracle>,
    journal: Arc<dyn ProviderRunJournal>,
    clock: Arc<dyn ProviderClock>,
    cpu_pool: Arc<ThreadPool>,
    admission: AdmissionController,
    runs: Arc<Mutex<BTreeMap<String, Arc<RunControl>>>>,
    supersession: Arc<Mutex<BTreeMap<String, String>>>,
    admission_audit: Arc<Mutex<VecDeque<AdmissionAuditRecord>>>,
    counters: Arc<RuntimeCounters>,
}

impl ProviderRuntime {
    /// Construct one provider-class runtime from generated registry authorities.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown provider/profile or an invalid Rayon pool.
    pub fn new(
        provider_id: &str,
        wave_id: Vec<u8>,
        adapter: Arc<dyn ProviderAdapter>,
        generation_oracle: Arc<dyn SourceGenerationOracle>,
        journal: Arc<dyn ProviderRunJournal>,
    ) -> Result<Self, ProviderRuntimeError> {
        Self::new_with_clock(
            provider_id,
            wave_id,
            adapter,
            generation_oracle,
            journal,
            Arc::new(SystemProviderClock),
        )
    }

    /// Construct a provider runtime with an explicit deterministic clock.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`].
    pub fn new_with_clock(
        provider_id: &str,
        wave_id: Vec<u8>,
        adapter: Arc<dyn ProviderAdapter>,
        generation_oracle: Arc<dyn SourceGenerationOracle>,
        journal: Arc<dyn ProviderRunJournal>,
        clock: Arc<dyn ProviderClock>,
    ) -> Result<Self, ProviderRuntimeError> {
        let provider = PROVIDER_ENTRIES
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .ok_or_else(|| ProviderRuntimeError::InvalidJob("unknown provider_id".into()))?;
        let profile = PROVIDER_RESOURCE_PROFILES
            .iter()
            .find(|profile| profile.profile_id == provider.resource_profile_id)
            .ok_or_else(|| ProviderRuntimeError::InvalidJob("provider profile is absent".into()))?;
        if !profile.provider_ids.contains(&provider.provider_id) || wave_id.is_empty() {
            return Err(ProviderRuntimeError::InvalidJob(
                "provider profile or wave identity is invalid".into(),
            ));
        }
        let thread_name_provider = provider_id.to_owned();
        let cpu_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(usize::from(profile.max_parser_workers))
            .thread_name(move |index| format!("codefabric-{thread_name_provider}-{index}"))
            .build()
            .map_err(|error| ProviderRuntimeError::ThreadPool(error.to_string()))?;
        Ok(Self {
            provider,
            profile,
            wave_id: Arc::from(wave_id),
            adapter,
            generation_oracle,
            journal,
            clock,
            cpu_pool: Arc::new(cpu_pool),
            admission: AdmissionController::new(profile),
            runs: Arc::default(),
            supersession: Arc::default(),
            admission_audit: Arc::default(),
            counters: Arc::default(),
        })
    }

    /// Current metrics snapshot.
    #[must_use]
    pub fn metrics(&self) -> ProviderRuntimeMetrics {
        self.counters.snapshot()
    }

    /// Bounded admission records, oldest first.
    #[must_use]
    pub fn admission_audit(&self) -> Vec<AdmissionAuditRecord> {
        self.admission_audit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// Evict bounded admission state when the workspace lifecycle closes.
    pub fn evict_workspace(&self, workspace_id: &str) {
        self.admission.evict_workspace(workspace_id);
        self.supersession
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|key, _| !key.contains(workspace_id));
    }

    fn validate_job(&self, spec: &ProviderJob) -> Result<ValidatedJobIds, ProviderRuntimeError> {
        if spec.supersession_key.is_empty() {
            return Err(ProviderRuntimeError::InvalidJob(
                "supersession identity is required".into(),
            ));
        }
        if !valid_sandbox_profile_digest(&spec.sandbox_profile_digest) {
            return Err(ProviderRuntimeError::InvalidJob(
                "sandbox profile digest is invalid".into(),
            ));
        }
        let validated_ids = validated_job_ids(spec)?;
        let mut outcomes = Vec::new();
        let profile_matches = spec.resource_profile_id == self.profile.profile_id;
        outcomes.push(AdmissionLimitOutcome {
            check: "resource-profile-exact",
            allowed: profile_matches,
        });
        let provider_allowed = self
            .profile
            .provider_ids
            .contains(&self.provider.provider_id);
        outcomes.push(AdmissionLimitOutcome {
            check: "provider-profile-compatible",
            allowed: provider_allowed,
        });
        let estimate = spec.resource_estimate.as_ref();
        outcomes.push(AdmissionLimitOutcome {
            check: "resource-estimate-present",
            allowed: estimate.is_some(),
        });
        let input_allowed =
            estimate.is_some_and(|value| value.input_bytes <= self.profile.max_input_bytes);
        outcomes.push(AdmissionLimitOutcome {
            check: "max-input-bytes",
            allowed: input_allowed,
        });
        let output_allowed = estimate
            .is_some_and(|value| value.expected_output_bytes <= self.profile.max_output_bytes);
        outcomes.push(AdmissionLimitOutcome {
            check: "max-output-bytes",
            allowed: output_allowed,
        });
        let cpu_allowed =
            estimate.is_some_and(|value| value.cpu_weight <= self.profile.max_cpu_weight);
        outcomes.push(AdmissionLimitOutcome {
            check: "max-cpu-weight",
            allowed: cpu_allowed,
        });
        let memory_allowed =
            estimate.is_some_and(|value| value.memory_mib <= self.profile.max_memory_mib);
        outcomes.push(AdmissionLimitOutcome {
            check: "max-memory-mib",
            allowed: memory_allowed,
        });
        let now = self.clock.now_unix_millis();
        let lease_allowed = spec.source_snapshot_lease.as_ref().is_some_and(|lease| {
            lease.workspace_id == spec.workspace_id
                && lease.source_generation == spec.source_generation
                && lease.expires_at_unix_ms >= spec.deadline_unix_ms
                && lease
                    .blobs
                    .iter()
                    .try_fold(0_u64, |total, blob| total.checked_add(blob.byte_length))
                    .is_some_and(|total| total <= self.profile.max_input_bytes)
        });
        outcomes.push(AdmissionLimitOutcome {
            check: "source-snapshot-lease",
            allowed: lease_allowed,
        });
        let deadline_allowed = spec.deadline_unix_ms > now
            && u64::try_from(spec.deadline_unix_ms - now)
                .is_ok_and(|duration| duration <= self.profile.max_wall_millis);
        outcomes.push(AdmissionLimitOutcome {
            check: "max-wall-millis",
            allowed: deadline_allowed,
        });
        let audit = AdmissionAuditRecord {
            provider_run_id: spec.provider_run_id.clone(),
            provider_id: self.provider.provider_id,
            resource_profile_id: spec.resource_profile_id.clone(),
            outcomes: outcomes.clone(),
        };
        let mut records = self
            .admission_audit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if records.len() == ADMISSION_AUDIT_CAPACITY {
            records.pop_front();
        }
        records.push_back(audit);
        drop(records);
        if let Some(outcome) = outcomes.iter().find(|outcome| !outcome.allowed) {
            self.counters.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(ProviderRuntimeError::AdmissionRejected {
                profile_id: spec.resource_profile_id.clone(),
                check: outcome.check,
            });
        }
        Ok(validated_ids)
    }

    fn supersession_key(&self, spec: &ProviderJob) -> String {
        format!(
            "{}\0{}\0{}\0{}",
            spec.workspace_id,
            spec.analysis_context_id,
            self.provider.provider_id,
            spec.supersession_key
        )
    }

    fn initial_record(
        &self,
        spec: &ProviderJob,
        ids: ValidatedJobIds,
        accepted_at: i64,
    ) -> ProviderRunRecord {
        let owner_id = spec
            .scopes
            .iter()
            .find(|scope| scope.scope_kind == ProviderScopeKind::SemanticOwner as u16)
            .map(|scope| scope.scope_id.as_bytes().to_vec());
        let build_unit_id = spec
            .scopes
            .iter()
            .find(|scope| scope.scope_kind == ProviderScopeKind::BuildUnit as u16)
            .map(|scope| scope.scope_id.as_bytes().to_vec());
        ProviderRunRecord {
            provider_run_id: ids.run.to_vec(),
            workspace_id: ids.workspace.to_vec(),
            analysis_context_id: ids.context.to_vec(),
            wave_id: self.wave_id.to_vec(),
            provider_code: i64::from(self.provider.provider_code),
            owner_id,
            build_unit_id,
            source_generation: i64::try_from(spec.source_generation).unwrap_or(i64::MAX),
            input_fingerprint: spec.fingerprint().to_vec(),
            output_fingerprint: None,
            sandbox_profile_digest: Some(spec.sandbox_profile_digest.clone()),
            state_code: i64::from(ProviderRunState::Queued as u16),
            accepted_at: accepted_at.to_string(),
            terminal_at: None,
            diagnostic_id: None,
        }
    }

    fn transition(
        &self,
        control: &RunControl,
        event: &str,
        guard: &str,
        output_fingerprint: Option<[u8; 32]>,
        diagnostic: Option<&str>,
    ) -> Result<ProviderRunState, ProviderRuntimeError> {
        let prior_code = control.state.load(Ordering::Acquire);
        let prior = registry_state_name(PROVIDER_RUN_STATE_VALUES, prior_code)
            .ok_or_else(|| ProviderRuntimeError::StateTransition("unknown prior state".into()))?;
        let transition = generated_transition(PROVIDER_RUN_STATE_TRANSITIONS, prior, event, guard)
            .map_err(|error| {
                ProviderRuntimeError::StateTransition(format!(
                    "{} + {} + {}",
                    error.prior_state, error.event, error.guard
                ))
            })?;
        let next = PROVIDER_RUN_STATE_VALUES
            .iter()
            .find(|entry| entry.name == transition.to)
            .and_then(|entry| ProviderRunState::try_from(entry.code).ok())
            .ok_or_else(|| ProviderRuntimeError::StateTransition("unknown next state".into()))?;
        control
            .state
            .compare_exchange(prior_code, next as u16, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ProviderRuntimeError::StateTransition("concurrent transition".into()))?;
        let mut record = control
            .record
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        record.state_code = i64::from(next as u16);
        record.output_fingerprint = output_fingerprint.map(|value| value.to_vec());
        if terminal(next) {
            record.terminal_at = Some(self.clock.now_unix_millis().to_string());
            record.diagnostic_id = diagnostic.map(diagnostic_id);
        }
        self.journal.record(&record)?;
        drop(record);
        if next == ProviderRunState::Running {
            self.counters.started.fetch_add(1, Ordering::Relaxed);
        }
        if terminal(next) {
            self.counters.terminal.fetch_add(1, Ordering::Relaxed);
            match next {
                ProviderRunState::Superseded => {
                    self.counters.superseded.fetch_add(1, Ordering::Relaxed);
                }
                ProviderRunState::Cancelled => {
                    self.counters.cancelled.fetch_add(1, Ordering::Relaxed);
                }
                ProviderRunState::TimedOut => {
                    self.counters.timed_out.fetch_add(1, Ordering::Relaxed);
                }
                ProviderRunState::StaleResult => {
                    self.counters.stale_results.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
            control.terminal_notify.notify_waiters();
        }
        Ok(next)
    }

    async fn acquire(
        semaphore: Arc<Semaphore>,
        control: &RunControl,
    ) -> Result<OwnedSemaphorePermit, u8> {
        let intent = control.intent.load(Ordering::Acquire);
        if intent != INTENT_NONE {
            return Err(intent);
        }
        tokio::select! {
            biased;
            () = control.request_notify.notified() => Err(control.intent.load(Ordering::Acquire)),
            permit = semaphore.acquire_owned() => permit.map_err(|_| INTENT_CANCEL),
        }
    }

    async fn acquire_all(&self, spec: &ProviderJob, control: &RunControl) -> Result<PermitSet, u8> {
        let global = Self::acquire(Arc::clone(&self.admission.global), control).await?;
        let workspace =
            Self::acquire(self.admission.workspace(&spec.workspace_id), control).await?;
        let context = Self::acquire(
            self.admission
                .context(&spec.workspace_id, &spec.analysis_context_id),
            control,
        )
        .await?;
        Ok(PermitSet {
            _global: global,
            _workspace: workspace,
            _context: context,
        })
    }

    async fn emit_terminal(
        &self,
        sink: &ProviderEventSink,
        state: ProviderRunState,
        output: Option<[u8; 32]>,
        code: &str,
    ) {
        let event = match (state, output) {
            (ProviderRunState::Succeeded | ProviderRunState::Partial, Some(output_fingerprint)) => {
                ProviderEvent::Completed {
                    identity: sink.next_identity(),
                    state,
                    output_fingerprint,
                }
            }
            _ => ProviderEvent::Failed {
                identity: sink.next_identity(),
                state,
                code: code.to_owned(),
            },
        };
        let _ = sink.events.send(event).await;
    }

    async fn finish_requested(
        &self,
        control: &RunControl,
        sink: &ProviderEventSink,
        prior: ProviderRunState,
        intent: u8,
    ) {
        let (event, guard, code) = if intent == INTENT_SUPERSEDE {
            ("superseded", "newer-generation", "SUPERSEDED")
        } else if prior == ProviderRunState::Queued {
            ("cancelled", "cancellation-active", "CANCELLED")
        } else {
            ("cancelled", "cancellation-acknowledged", "CANCELLED")
        };
        if let Ok(state) = self.transition(control, event, guard, None, Some(code)) {
            let _ = sink
                .events
                .send(ProviderEvent::CancelAcknowledged {
                    identity: sink.next_identity(),
                    state,
                })
                .await;
            self.emit_terminal(sink, state, None, code).await;
        }
    }

    fn spawn_adapter(
        &self,
        spec: &ProviderJob,
        control: &RunControl,
        sink: &ProviderEventSink,
    ) -> oneshot::Receiver<Result<ProviderCompletion, ProviderRuntimeError>> {
        let (result_sender, result_receiver) = oneshot::channel();
        let adapter = Arc::clone(&self.adapter);
        let adapter_spec = spec.clone();
        let adapter_sink = sink.clone();
        let cancellation = Cancellation::from_shared(
            Arc::clone(&control.cancelled),
            self.profile.cancellation_check_interval,
        );
        self.cpu_pool.spawn(move || {
            let _ = result_sender.send(adapter.run(adapter_spec, adapter_sink, cancellation));
        });
        result_receiver
    }

    async fn await_completion(
        &self,
        spec: &ProviderJob,
        control: &RunControl,
        result_receiver: &mut oneshot::Receiver<Result<ProviderCompletion, ProviderRuntimeError>>,
    ) -> RunCompletion {
        let now = self.clock.now_unix_millis();
        let wall_millis = u64::try_from(spec.deadline_unix_ms.saturating_sub(now))
            .unwrap_or_default()
            .min(self.profile.max_wall_millis);
        let deadline = tokio::time::sleep(Duration::from_millis(wall_millis));
        tokio::pin!(deadline);
        tokio::select! {
            result = result_receiver => match result {
                Ok(result) => RunCompletion::Adapter(result),
                Err(_) => RunCompletion::Closed,
            },
            () = control.request_notify.notified() => {
                RunCompletion::Requested(control.intent.load(Ordering::Acquire))
            },
            () = &mut deadline => RunCompletion::Deadline,
        }
    }

    async fn await_adapter_stop(
        &self,
        permits: PermitSet,
        mut result_receiver: oneshot::Receiver<Result<ProviderCompletion, ProviderRuntimeError>>,
    ) {
        if tokio::time::timeout(
            Duration::from_millis(u64::from(self.profile.cancellation_ack_millis)),
            &mut result_receiver,
        )
        .await
        .is_err()
        {
            tokio::spawn(async move {
                let _permits = permits;
                let _ = result_receiver.await;
            });
        }
    }

    async fn finish_deadline(&self, control: &RunControl, sink: &ProviderEventSink) {
        if let Ok(state) = self.transition(
            control,
            "deadline-expired",
            "deadline-reached",
            None,
            Some("TIMED_OUT"),
        ) {
            self.emit_terminal(sink, state, None, "TIMED_OUT").await;
        }
    }

    async fn finish_closed(&self, control: &RunControl, sink: &ProviderEventSink) {
        if let Ok(state) = self.transition(
            control,
            "process-exited",
            "unexpected-exit",
            None,
            Some("WORKER_CLOSED"),
        ) {
            self.emit_terminal(sink, state, None, "WORKER_CLOSED").await;
        }
    }

    async fn run_job(self, spec: ProviderJob, control: Arc<RunControl>, sink: ProviderEventSink) {
        let permits = match self.acquire_all(&spec, &control).await {
            Ok(permits) => permits,
            Err(intent) => {
                self.finish_requested(&control, &sink, ProviderRunState::Queued, intent)
                    .await;
                return;
            }
        };
        if self
            .transition(&control, "permit-granted", "capacity-available", None, None)
            .is_err()
        {
            return;
        }
        let mut result_receiver = self.spawn_adapter(&spec, &control, &sink);
        let completion = self
            .await_completion(&spec, &control, &mut result_receiver)
            .await;
        match completion {
            RunCompletion::Adapter(result) => {
                drop(permits);
                self.finish_adapter_result(&spec, &control, &sink, result)
                    .await;
            }
            RunCompletion::Requested(intent) => {
                control.cancelled.store(true, Ordering::Release);
                self.await_adapter_stop(permits, result_receiver).await;
                self.finish_requested(&control, &sink, ProviderRunState::Running, intent)
                    .await;
            }
            RunCompletion::Deadline => {
                control.cancelled.store(true, Ordering::Release);
                self.await_adapter_stop(permits, result_receiver).await;
                self.finish_deadline(&control, &sink).await;
            }
            RunCompletion::Closed => {
                drop(permits);
                self.finish_closed(&control, &sink).await;
            }
        }
    }

    async fn finish_adapter_result(
        &self,
        spec: &ProviderJob,
        control: &RunControl,
        sink: &ProviderEventSink,
        result: Result<ProviderCompletion, ProviderRuntimeError>,
    ) {
        let Ok(completion) = result else {
            if let Ok(state) = self.transition(
                control,
                "domain-failure",
                "failure-valid",
                None,
                Some("PROVIDER_FAILED"),
            ) {
                self.emit_terminal(sink, state, None, "PROVIDER_FAILED")
                    .await;
            }
            return;
        };
        let current_generation = self
            .generation_oracle
            .current_generation(&spec.workspace_id);
        if current_generation != Ok(spec.source_generation) {
            if let Ok(state) = self.transition(
                control,
                "stale-result",
                "source-generation-changed",
                None,
                Some("STALE_RESULT"),
            ) {
                self.emit_terminal(sink, state, None, "STALE_RESULT").await;
            }
            return;
        }
        let (event, guard, stream_terminal) = match completion.state {
            ProviderRunState::Succeeded => (
                "terminal-manifest-complete",
                "manifest-valid",
                StreamTerminal::Completed,
            ),
            ProviderRunState::Partial => (
                "terminal-manifest-partial",
                "manifest-valid-and-partial",
                StreamTerminal::Partial,
            ),
            ProviderRunState::Failed => ("domain-failure", "failure-valid", StreamTerminal::Failed),
            _ => {
                if let Ok(state) = self.transition(
                    control,
                    "protocol-violated",
                    "framing-or-credit-invalid",
                    None,
                    Some("INVALID_TERMINAL_STATE"),
                ) {
                    self.emit_terminal(sink, state, None, "INVALID_TERMINAL_STATE")
                        .await;
                }
                return;
            }
        };
        let _ = sink
            .provider_facts
            .send(ProviderFactMessage::Terminal(stream_terminal))
            .await;
        let diagnostic = completion.diagnostic_code.as_deref();
        if let Ok(state) = self.transition(
            control,
            event,
            guard,
            Some(completion.output_fingerprint),
            diagnostic,
        ) {
            self.emit_terminal(
                sink,
                state,
                Some(completion.output_fingerprint),
                diagnostic.unwrap_or("PROVIDER_FAILED"),
            )
            .await;
        }
    }
}

/// Registry-selected shared dispatch port used by continuous serving and compatibility verticals.
#[derive(Clone)]
pub struct ProviderRuntimeDispatch {
    runtimes: BTreeMap<&'static str, ProviderRuntime>,
}

impl ProviderRuntimeDispatch {
    /// Construct the two semantic runtimes from the generated provider/resource registries.
    ///
    /// # Errors
    ///
    /// Returns a runtime construction error or missing adapter registration.
    pub fn semantic(
        wave_id: &[u8],
        adapters: &SemanticProviderAdapterRegistry,
        generation_oracle: &Arc<dyn SourceGenerationOracle>,
        journal: &Arc<dyn ProviderRunJournal>,
    ) -> Result<Self, ProviderRuntimeError> {
        let mut runtimes = BTreeMap::new();
        for provider_id in ["pyrefly-python", "rustc-mir"] {
            let adapter = adapters.adapter(provider_id).ok_or_else(|| {
                ProviderRuntimeError::InvalidJob("semantic adapter registration is absent".into())
            })?;
            let runtime = ProviderRuntime::new(
                provider_id,
                wave_id.to_owned(),
                adapter,
                Arc::clone(generation_oracle),
                Arc::clone(journal),
            )?;
            runtimes.insert(provider_id, runtime);
        }
        Ok(Self { runtimes })
    }

    #[must_use]
    pub fn provider_ids(&self) -> Vec<&'static str> {
        self.runtimes.keys().copied().collect()
    }

    /// Submit one provider job only after registry selection.
    ///
    /// # Errors
    ///
    /// Rejects unknown provider IDs and forwards the selected runtime's admission result.
    pub async fn submit(
        &self,
        provider_id: &str,
        spec: ProviderJob,
    ) -> Result<AcceptedProviderJob, ProviderRuntimeError> {
        self.runtimes
            .get(provider_id)
            .ok_or_else(|| ProviderRuntimeError::InvalidJob("provider is not registered".into()))?
            .submit(spec)
            .await
    }

    /// Cancel through the same selected runtime that admitted the run.
    ///
    /// # Errors
    ///
    /// Rejects unknown provider IDs and forwards runtime cancellation errors.
    pub async fn cancel(
        &self,
        provider_id: &str,
        run_id: &str,
        reason: &str,
    ) -> Result<CancelAcknowledgement, ProviderRuntimeError> {
        self.runtimes
            .get(provider_id)
            .ok_or_else(|| ProviderRuntimeError::InvalidJob("provider is not registered".into()))?
            .cancel(run_id, reason)
            .await
    }
}

#[async_trait]
pub trait ProviderExecutor: Send + Sync {
    async fn submit(&self, spec: ProviderJob) -> Result<AcceptedProviderJob, ProviderRuntimeError>;
    async fn cancel(
        &self,
        run_id: &str,
        reason: &str,
    ) -> Result<CancelAcknowledgement, ProviderRuntimeError>;
}

#[async_trait]
impl ProviderExecutor for ProviderRuntime {
    async fn submit(&self, spec: ProviderJob) -> Result<AcceptedProviderJob, ProviderRuntimeError> {
        let validated_ids = self.validate_job(&spec)?;
        let accepted_at = self.clock.now_unix_millis();
        let record = self.initial_record(&spec, validated_ids, accepted_at);
        let input_fingerprint: [u8; 32] = record
            .input_fingerprint
            .as_slice()
            .try_into()
            .expect("BLAKE3 fingerprint is 32 bytes");
        let control = Arc::new(RunControl::new(record));
        {
            let mut runs = self
                .runs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if runs.contains_key(&spec.provider_run_id) {
                return Err(ProviderRuntimeError::DuplicateRun(
                    spec.provider_run_id.clone(),
                ));
            }
            self.journal.record(
                &control
                    .record
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )?;
            runs.insert(spec.provider_run_id.clone(), Arc::clone(&control));
        }
        let key = self.supersession_key(&spec);
        if let Some(prior_run_id) = self
            .supersession
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, spec.provider_run_id.clone())
            && let Some(prior) = self
                .runs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&prior_run_id)
        {
            prior.request(INTENT_SUPERSEDE);
        }
        let (events_sender, events) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (provider_facts_sender, provider_facts) =
            bounded_provider_fact_channel(OBSERVATION_CHANNEL_CAPACITY);
        let identity = ProviderEventIdentity {
            provider_run_id: spec.provider_run_id.clone(),
            workspace_id: spec.workspace_id.clone(),
            analysis_context_id: spec.analysis_context_id.clone(),
            source_generation: spec.source_generation,
            sequence: 0,
            input_fingerprint,
        };
        events_sender
            .send(ProviderEvent::Accepted {
                identity: identity.clone(),
            })
            .await
            .map_err(|_| ProviderRuntimeError::EventReceiverClosed)?;
        let sink = ProviderEventSink {
            events: events_sender,
            provider_facts: provider_facts_sender,
            counters: Arc::clone(&self.counters),
            identity,
            validated_ids,
            provider_code: self.provider.provider_code,
            next_sequence: Arc::new(AtomicU64::new(1)),
        };
        self.counters.accepted.fetch_add(1, Ordering::Relaxed);
        let accepted = AcceptedProviderJob {
            run_id: spec.provider_run_id.clone(),
            accepted_generation: spec.source_generation,
            events,
            provider_facts,
            cancellation: Cancellation::from_shared(
                Arc::clone(&control.cancelled),
                self.profile.cancellation_check_interval,
            ),
        };
        tokio::spawn(self.clone().run_job(spec, control, sink));
        Ok(accepted)
    }

    async fn cancel(
        &self,
        run_id: &str,
        _reason: &str,
    ) -> Result<CancelAcknowledgement, ProviderRuntimeError> {
        let control = self
            .runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(run_id)
            .cloned()
            .ok_or_else(|| ProviderRuntimeError::RunNotFound(run_id.to_owned()))?;
        let prior = control.state();
        if !terminal(prior) {
            control.request(INTENT_CANCEL);
            let wait = async {
                loop {
                    if terminal(control.state()) {
                        break;
                    }
                    control.terminal_notify.notified().await;
                }
            };
            tokio::time::timeout(
                Duration::from_millis(
                    u64::from(self.profile.cancellation_ack_millis).saturating_add(50),
                ),
                wait,
            )
            .await
            .map_err(|_| ProviderRuntimeError::WorkerClosed)?;
        }
        let state = control.state();
        let acknowledgement_state = if terminal(prior) {
            CancelAcknowledgementState::AlreadyTerminal
        } else {
            CancelAcknowledgementState::Cancelled
        };
        Ok(CancelAcknowledgement {
            provider_run_id: run_id.to_owned(),
            state: acknowledgement_state as i32,
            acknowledged_at_unix_ms: self.clock.now_unix_millis(),
            terminal_state: Some(state as i32),
            cleaning_up_components: Vec::new(),
            forced_termination: false,
        })
    }
}

struct PermitSet {
    _global: OwnedSemaphorePermit,
    _workspace: OwnedSemaphorePermit,
    _context: OwnedSemaphorePermit,
}

fn terminal(state: ProviderRunState) -> bool {
    !matches!(state, ProviderRunState::Queued | ProviderRunState::Running)
}

fn state_name(state: ProviderRunState) -> &'static str {
    registry_state_name(PROVIDER_RUN_STATE_VALUES, state as u16)
        .expect("generated state has a generated name")
}

fn parse_blake3_digest(value: &str) -> Result<[u8; 32], ProviderRuntimeError> {
    let payload = value
        .strip_prefix("b3:")
        .filter(|payload| payload.len() == 64)
        .ok_or_else(|| ProviderRuntimeError::Protocol("terminal digest is malformed".into()))?;
    let mut digest = [0_u8; 32];
    for (index, chunk) in payload.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| ProviderRuntimeError::Protocol("terminal digest is malformed".into()))?;
        digest[index] = u8::from_str_radix(text, 16)
            .map_err(|_| ProviderRuntimeError::Protocol("terminal digest is malformed".into()))?;
    }
    Ok(digest)
}

fn valid_sandbox_profile_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .or_else(|| value.strip_prefix("b3:"))
        .is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
}

fn decode_lower_hex_id16(value: &str) -> Result<[u8; 16], ()> {
    if value.len() != 32
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(());
    }
    let mut output = [0_u8; 16];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        output[index] =
            u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| ())?, 16).map_err(|_| ())?;
    }
    Ok(output)
}

fn validated_job_ids(spec: &ProviderJob) -> Result<ValidatedJobIds, ProviderRuntimeError> {
    let run = decode_lower_hex_id16(&spec.provider_run_id).map_err(|()| {
        ProviderRuntimeError::InvalidJob(
            "provider_run_id must be exactly 32 lowercase hexadecimal characters".into(),
        )
    })?;
    let workspace = decode_public_id(IdentityDomain::Workspace, None, &spec.workspace_id)
        .map_err(|error| ProviderRuntimeError::InvalidJob(error.to_string()))?;
    let context = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &spec.analysis_context_id,
    )
    .map_err(|error| ProviderRuntimeError::InvalidJob(error.to_string()))?;
    Ok(ValidatedJobIds {
        run,
        workspace,
        context,
    })
}

fn system_now_unix_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn diagnostic_id(value: &str) -> Vec<u8> {
    let bounded = value
        .as_bytes()
        .get(..MAX_DIAGNOSTIC_BYTES)
        .unwrap_or(value.as_bytes());
    crate::identity::unframed_semantic_id(bounded).to_vec()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    use std::sync::atomic::AtomicU64;
    use std::thread;
    use std::time::Instant;

    use tempfile::tempdir;

    use super::*;
    use crate::fact_ingest::{ProviderFactManifest, receive_provider_fact_stream};

    const TEST_NOW: i64 = 1_800_000_000_000;

    #[derive(Clone, Copy)]
    enum FakeMode {
        Immediate,
        Slow,
        Backpressure,
        Partial,
        Failed,
    }

    struct FakeAdapter {
        mode: FakeMode,
    }

    struct FakeSemanticDriver;

    impl SemanticProviderDriver for FakeSemanticDriver {
        fn execute(
            &self,
            _work: SemanticProviderWork,
            _events: ProviderEventSink,
            _cancellation: Cancellation,
        ) -> Result<ProviderCompletion, ProviderRuntimeError> {
            Ok(ProviderCompletion {
                state: ProviderRunState::Succeeded,
                output_fingerprint: [0x44; 32],
                diagnostic_code: None,
            })
        }
    }

    impl ProviderAdapter for FakeAdapter {
        fn run(
            &self,
            spec: ProviderJob,
            events: ProviderEventSink,
            cancellation: Cancellation,
        ) -> Result<ProviderCompletion, ProviderRuntimeError> {
            assert_eq!(cancellation.check_interval(), 1_024);
            if !matches!(self.mode, FakeMode::Slow) {
                assert!(!cancellation.is_cancelled());
            }
            events.send_provider_fact(ProviderFactMessage::Manifest(ProviderFactManifest {
                stream_id: [1; 16],
                workspace_id: decode_public_id(IdentityDomain::Workspace, None, &spec.workspace_id)
                    .unwrap(),
                analysis_context_id: decode_public_id(
                    IdentityDomain::AnalysisContext,
                    None,
                    &spec.analysis_context_id,
                )
                .unwrap(),
                source_generation: i64::try_from(spec.source_generation).unwrap(),
                provider_code: 10,
                provider_version: "test".into(),
                provider_run_id: decode_lower_hex_id16(&spec.provider_run_id).unwrap(),
                emitted_at_micros: TEST_NOW,
                schema_fingerprints: BTreeMap::new(),
                declared_rows: 0,
            }))?;
            if matches!(self.mode, FakeMode::Backpressure) {
                for completed_units in 0..40 {
                    events.send_progress(completed_units, 40, "fake")?;
                }
            }
            if matches!(self.mode, FakeMode::Slow) {
                for _ in 0..100 {
                    if cancellation.is_cancelled() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
            }
            let state = match self.mode {
                FakeMode::Partial => ProviderRunState::Partial,
                FakeMode::Failed => ProviderRunState::Failed,
                _ => ProviderRunState::Succeeded,
            };
            Ok(ProviderCompletion {
                state,
                output_fingerprint: [9; 32],
                diagnostic_code: (state == ProviderRunState::Failed).then(|| "FAKE_FAILURE".into()),
            })
        }
    }

    struct Generation(AtomicU64);

    impl SourceGenerationOracle for Generation {
        fn current_generation(&self, _workspace_id: &str) -> Result<u64, ProviderRuntimeError> {
            Ok(self.0.load(Ordering::Acquire))
        }
    }

    struct FixedClock;

    impl ProviderClock for FixedClock {
        fn now_unix_millis(&self) -> i64 {
            TEST_NOW
        }
    }

    struct RuntimeFixture {
        _directory: tempfile::TempDir,
        runtime: ProviderRuntime,
        reader: crate::operational_store::OperationalReaderFactory,
        generation: Arc<Generation>,
    }

    fn fixture(mode: FakeMode) -> RuntimeFixture {
        fixture_with_adapter("tree-sitter", Arc::new(FakeAdapter { mode }))
    }

    fn fixture_with_adapter(
        provider_id: &str,
        adapter: Arc<dyn ProviderAdapter>,
    ) -> RuntimeFixture {
        let directory = tempdir().unwrap();
        let store = OperationalStore::open(&directory.path().join("operational.sqlite")).unwrap();
        let reader = store.reader_factory();
        let generation = Arc::new(Generation(AtomicU64::new(7)));
        let runtime = ProviderRuntime::new_with_clock(
            provider_id,
            [5; 16].to_vec(),
            adapter,
            generation.clone(),
            Arc::new(OperationalProviderRunJournal::new(store)),
            Arc::new(FixedClock),
        )
        .unwrap();
        RuntimeFixture {
            _directory: directory,
            runtime,
            reader,
            generation,
        }
    }

    fn provider_text(source: &str) -> crate::provider_types::ProviderText {
        crate::provider_types::ProviderText {
            text: Arc::from(source),
            original_byte_offsets: Arc::from(
                source
                    .char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap())
                    .chain(std::iter::once(u64::try_from(source.len()).unwrap()))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn run_id(label: &str) -> String {
        let digest = crate::identity::unframed_semantic_id(label.as_bytes());
        let mut encoded = String::with_capacity(32);
        for byte in &digest {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        encoded
    }

    fn job(run_label: &str, supersession_key: &str) -> ProviderJob {
        let workspace_id =
            crate::identity::encode_public_id(IdentityDomain::Workspace, None, [2; 16]).unwrap();
        ProviderJob {
            provider_run_id: run_id(run_label),
            workspace_id: workspace_id.clone(),
            analysis_context_id: "context:source".into(),
            source_generation: 7,
            source_snapshot_lease: Some(ProviderSourceSnapshotLease {
                lease_id: "lease:test".into(),
                workspace_id,
                source_generation: 7,
                source_manifest_digest: "b3:test".into(),
                expires_at_unix_ms: TEST_NOW + 120_000,
                blobs: Vec::new(),
            }),
            resource_estimate: Some(ProviderResourceEstimate {
                input_bytes: 128,
                expected_output_bytes: 256,
                cpu_weight: 1,
                memory_mib: 64,
            }),
            deadline_unix_ms: TEST_NOW + 1_000,
            supersession_key: supersession_key.into(),
            resource_profile_id: "in-process-syntax-standard".into(),
            sandbox_profile_digest: format!("b3:{}", "11".repeat(32)),
            ..ProviderJob::default()
        }
    }

    fn semantic_job(
        provider_id: &'static str,
        run_label: &str,
        workspace_view: crate::source_image::ProviderWorkspaceView,
    ) -> ProviderJob {
        let workspace_id =
            crate::identity::encode_public_id(IdentityDomain::Workspace, None, [2; 16]).unwrap();
        let scope = ProviderScope {
            scope_kind: ProviderScopeKind::SemanticOwner as u16,
            scope_id: "0000000000000000".into(),
        };
        let capability_family = match provider_id {
            "pyrefly-python" => "PYTHON_SEMANTIC",
            "rustc-mir" => "RUST_SEMANTIC",
            _ => unreachable!("test only constructs governed semantic providers"),
        };
        let provider = PROVIDER_ENTRIES
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .unwrap();
        let deadline_unix_ms = system_now_unix_millis() + 60_000;
        ProviderJob {
            provider_run_id: run_id(run_label),
            workspace_id: workspace_id.clone(),
            analysis_context_id: "context:source".into(),
            source_generation: 7,
            source_snapshot_lease: Some(ProviderSourceSnapshotLease {
                lease_id: format!("lease:{provider_id}"),
                workspace_id,
                source_generation: 7,
                source_manifest_digest: format!("b3:{}", "21".repeat(32)),
                expires_at_unix_ms: deadline_unix_ms,
                blobs: Vec::new(),
            }),
            requested_capability_codes: Vec::new(),
            scopes: vec![scope.clone()],
            priority_class: 10,
            resource_estimate: Some(ProviderResourceEstimate {
                input_bytes: 1,
                expected_output_bytes: 1,
                cpu_weight: 1,
                memory_mib: 64,
            }),
            deadline_unix_ms,
            supersession_key: ProviderJob::semantic_supersession_key(&scope, capability_family),
            required_bundle_digests: Vec::new(),
            required_schema_digests: Vec::new(),
            idempotency_key: format!("idempotency:{provider_id}"),
            resource_profile_id: provider.resource_profile_id.into(),
            sandbox_profile_digest: workspace_view.sandbox_profile_digest.clone(),
            direct_work: ProviderDirectWork::SemanticProcess(SemanticProviderWork {
                provider_id: provider_id.into(),
                capability_family: capability_family.into(),
                workspace_view,
                trust_profile: crate::provider_sandbox::ProviderTrustProfile::TrustedLocal,
                invocation_manifest: Arc::from(&b"{}"[..]),
            }),
        }
    }

    async fn terminal_event_with_identity(
        events: &mut mpsc::Receiver<ProviderEvent>,
    ) -> (ProviderRunState, ProviderEventIdentity) {
        while let Some(event) = events.recv().await {
            match event {
                ProviderEvent::Completed {
                    state, identity, ..
                }
                | ProviderEvent::Failed {
                    state, identity, ..
                } => return (state, identity),
                _ => {}
            }
        }
        panic!("provider event stream closed before terminal state")
    }

    async fn terminal_event(events: &mut mpsc::Receiver<ProviderEvent>) -> ProviderRunState {
        terminal_event_with_identity(events).await.0
    }

    async fn assert_runtime_limit_rejections(runtime: &ProviderRuntime) {
        let mut missing_estimate = job("run:missing-estimate", "scope:limits");
        missing_estimate.resource_estimate = None;
        let mut input = job("run:input", "scope:limits");
        input.resource_estimate.as_mut().unwrap().input_bytes = 16_777_217;
        let mut output = job("run:output", "scope:limits");
        output
            .resource_estimate
            .as_mut()
            .unwrap()
            .expected_output_bytes = 268_435_457;
        let mut cpu = job("run:cpu", "scope:limits");
        cpu.resource_estimate.as_mut().unwrap().cpu_weight = 5;
        let mut workspace_lease = job("run:lease-workspace", "scope:limits");
        workspace_lease
            .source_snapshot_lease
            .as_mut()
            .unwrap()
            .workspace_id = "workspace:other".into();
        let mut generation_lease = job("run:lease-generation", "scope:limits");
        generation_lease
            .source_snapshot_lease
            .as_mut()
            .unwrap()
            .source_generation = 8;
        let mut oversized_lease = job("run:lease-bytes", "scope:limits");
        oversized_lease
            .source_snapshot_lease
            .as_mut()
            .unwrap()
            .blobs
            .push(ProviderBlobReference {
                byte_length: 16_777_217,
                ..ProviderBlobReference::default()
            });
        let mut expired_lease = job("run:lease-expired", "scope:limits");
        expired_lease
            .source_snapshot_lease
            .as_mut()
            .unwrap()
            .expires_at_unix_ms = TEST_NOW;
        let mut expired = job("run:expired", "scope:limits");
        expired.deadline_unix_ms = TEST_NOW - 1;
        let mut elapsed = job("run:elapsed", "scope:limits");
        elapsed.deadline_unix_ms = TEST_NOW;
        let mut overlong = job("run:overlong", "scope:limits");
        overlong.deadline_unix_ms = TEST_NOW + 30_100;
        for (invalid, expected_check) in [
            (missing_estimate, "resource-estimate-present"),
            (input, "max-input-bytes"),
            (output, "max-output-bytes"),
            (cpu, "max-cpu-weight"),
            (workspace_lease, "source-snapshot-lease"),
            (generation_lease, "source-snapshot-lease"),
            (oversized_lease, "source-snapshot-lease"),
            (expired_lease, "source-snapshot-lease"),
            (expired, "max-wall-millis"),
            (elapsed, "max-wall-millis"),
            (overlong, "max-wall-millis"),
        ] {
            assert!(matches!(
                runtime.submit(invalid).await,
                Err(ProviderRuntimeError::AdmissionRejected { check, .. })
                    if check == expected_check
            ));
        }
    }

    #[tokio::test]
    async fn wp29_behavioral_acceptance() {
        let fixture = fixture(FakeMode::Slow);
        let mut accepted = fixture
            .runtime
            .submit(job("run:success", "scope:a"))
            .await
            .unwrap();
        assert_eq!(accepted.run_id, run_id("run:success"));
        assert_eq!(accepted.accepted_generation, 7);
        let accepted_identity = match accepted.events.recv().await {
            Some(ProviderEvent::Accepted { identity }) => identity,
            other => panic!("expected accepted event, got {other:?}"),
        };
        assert_eq!(accepted_identity.sequence, 0);
        tokio::time::timeout(Duration::from_millis(100), async {
            while fixture.runtime.metrics().started == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("RUNNING transition must be observable before terminal");
        assert_eq!(fixture.runtime.metrics().started, 1);
        assert_eq!(fixture.runtime.metrics().terminal, 0);
        let provider_facts =
            receive_provider_fact_stream(&mut accepted.provider_facts, &accepted.cancellation)
                .await
                .unwrap();
        assert_eq!(provider_facts.terminal, StreamTerminal::Completed);
        let (state, terminal_identity) = terminal_event_with_identity(&mut accepted.events).await;
        assert_eq!(state, ProviderRunState::Succeeded);
        assert_eq!(terminal_identity.sequence, 1);
        assert_eq!(
            terminal_identity.input_fingerprint,
            accepted_identity.input_fingerprint
        );
        let state = fixture
            .reader
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT state_code FROM provider_run WHERE provider_run_id = ?1",
                    [decode_lower_hex_id16(&run_id("run:success"))
                        .unwrap()
                        .as_slice()],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(state, i64::from(ProviderRunState::Succeeded as u16));
    }

    #[tokio::test]
    async fn wp29_structural_acceptance() {
        let fixture = fixture(FakeMode::Slow);
        let mut first = fixture
            .runtime
            .submit(job("run:first", "scope:a"))
            .await
            .unwrap();
        assert!(matches!(
            first.provider_facts.recv().await,
            Some(ProviderFactMessage::Manifest(_))
        ));
        let mut second = fixture
            .runtime
            .submit(job("run:second", "scope:a"))
            .await
            .unwrap();
        assert_eq!(
            terminal_event(&mut first.events).await,
            ProviderRunState::Superseded
        );
        assert_eq!(
            terminal_event(&mut second.events).await,
            ProviderRunState::Succeeded
        );
        let mut independent_a = fixture
            .runtime
            .submit(job("run:independent-a", "scope:b"))
            .await
            .unwrap();
        let mut independent_b = fixture
            .runtime
            .submit(job("run:independent-b", "scope:c"))
            .await
            .unwrap();
        assert_eq!(
            terminal_event(&mut independent_a.events).await,
            ProviderRunState::Succeeded
        );
        assert_eq!(
            terminal_event(&mut independent_b.events).await,
            ProviderRunState::Succeeded
        );
        let metrics = fixture.runtime.metrics();
        assert_eq!(metrics.accepted, 4);
        assert_eq!(metrics.started, 4);
        assert_eq!(metrics.terminal, 4);
        assert_eq!(metrics.superseded, 1);
        assert!(
            generated_transition(
                PROVIDER_RUN_STATE_TRANSITIONS,
                "QUEUED",
                "terminal-manifest-complete",
                "manifest-valid"
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn wp29_negative_zero_state() {
        let fixture = fixture(FakeMode::Immediate);
        let mut missing_run = job("run:missing-run", "scope:required");
        missing_run.provider_run_id.clear();
        let mut missing_workspace = job("run:missing-workspace", "scope:required");
        missing_workspace.workspace_id.clear();
        let mut missing_context = job("run:missing-context", "scope:required");
        missing_context.analysis_context_id.clear();
        let mut missing_supersession = job("run:missing-supersession", "scope:required");
        missing_supersession.supersession_key.clear();
        let mut malformed_run = job("run:malformed-run", "scope:required");
        malformed_run.provider_run_id = "ABC".into();
        let mut malformed_workspace = job("run:malformed-workspace", "scope:required");
        malformed_workspace.workspace_id = "workspace:ABC".into();
        let mut malformed_context = job("run:malformed-context", "scope:required");
        malformed_context.analysis_context_id = "context:ABC".into();
        for invalid in [
            missing_run,
            missing_workspace,
            missing_context,
            missing_supersession,
            malformed_run,
            malformed_workspace,
            malformed_context,
        ] {
            assert!(matches!(
                fixture.runtime.submit(invalid).await,
                Err(ProviderRuntimeError::InvalidJob(_))
            ));
        }
        for (profile, expected_check) in [
            ("", "resource-profile-exact"),
            ("missing-profile", "resource-profile-exact"),
            ("sidecar-semantic-standard", "resource-profile-exact"),
        ] {
            let mut invalid = job(&format!("run:{profile}"), "scope:invalid");
            invalid.resource_profile_id = profile.into();
            assert!(matches!(
                fixture.runtime.submit(invalid).await,
                Err(ProviderRuntimeError::AdmissionRejected { check, .. })
                    if check == expected_check
            ));
        }
        let mut excessive = job("run:excessive", "scope:invalid");
        excessive.resource_estimate.as_mut().unwrap().memory_mib = 1_025;
        assert!(matches!(
            fixture.runtime.submit(excessive).await,
            Err(ProviderRuntimeError::AdmissionRejected {
                check: "max-memory-mib",
                ..
            })
        ));
        assert_eq!(fixture.runtime.admission_audit().len(), 4);
        assert_eq!(fixture.runtime.metrics().rejected, 4);

        assert_runtime_limit_rejections(&fixture.runtime).await;
        assert_eq!(fixture.runtime.admission_audit().len(), 15);
        assert_eq!(fixture.runtime.metrics().rejected, 15);

        let (events, _event_receiver) = mpsc::channel(1);
        let (provider_facts, _provider_fact_receiver) = bounded_provider_fact_channel(1);
        let valid = job("run:manifest-fence", "scope:required");
        let validated_ids = fixture.runtime.validate_job(&valid).unwrap();
        let sink = ProviderEventSink {
            events,
            provider_facts,
            counters: Arc::new(RuntimeCounters::default()),
            identity: ProviderEventIdentity {
                provider_run_id: valid.provider_run_id,
                workspace_id: valid.workspace_id,
                analysis_context_id: valid.analysis_context_id,
                source_generation: valid.source_generation,
                sequence: 0,
                input_fingerprint: [0; 32],
            },
            validated_ids,
            provider_code: 10,
            next_sequence: Arc::new(AtomicU64::new(1)),
        };
        let mismatched = ProviderFactManifest {
            stream_id: [1; 16],
            workspace_id: [9; 16],
            analysis_context_id: validated_ids.context,
            source_generation: 7,
            provider_code: 10,
            provider_version: "test".into(),
            provider_run_id: validated_ids.run,
            emitted_at_micros: TEST_NOW,
            schema_fingerprints: BTreeMap::new(),
            declared_rows: 0,
        };
        assert!(matches!(
            sink.send_provider_fact(ProviderFactMessage::Manifest(mismatched)),
            Err(ProviderRuntimeError::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn wp29_operational_acceptance() {
        let fixture = fixture(FakeMode::Slow);
        let mut cancelled = fixture
            .runtime
            .submit(job("run:cancel", "scope:cancel"))
            .await
            .unwrap();
        assert!(matches!(
            cancelled.provider_facts.recv().await,
            Some(ProviderFactMessage::Manifest(_))
        ));
        let started = Instant::now();
        let acknowledgement = fixture
            .runtime
            .cancel(&run_id("run:cancel"), "test")
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(
            acknowledgement.terminal_state,
            Some(ProviderRunState::Cancelled as i32)
        );
        assert_eq!(
            terminal_event(&mut cancelled.events).await,
            ProviderRunState::Cancelled
        );

        let mut completed = fixture
            .runtime
            .submit(job("run:already-terminal", "scope:already-terminal"))
            .await
            .unwrap();
        assert_eq!(
            terminal_event(&mut completed.events).await,
            ProviderRunState::Succeeded
        );
        let acknowledgement = fixture
            .runtime
            .cancel(&run_id("run:already-terminal"), "test")
            .await
            .unwrap();
        assert_eq!(
            acknowledgement.state,
            CancelAcknowledgementState::AlreadyTerminal as i32
        );
        assert_eq!(
            acknowledgement.terminal_state,
            Some(ProviderRunState::Succeeded as i32)
        );

        fixture.generation.0.store(8, Ordering::Release);
        let mut stale = fixture
            .runtime
            .submit(job("run:stale", "scope:stale"))
            .await
            .unwrap();
        assert_eq!(
            terminal_event(&mut stale.events).await,
            ProviderRunState::StaleResult
        );
        let metrics = fixture.runtime.metrics();
        assert_eq!(metrics.cancelled, 1);
        assert_eq!(metrics.stale_results, 1);
    }

    #[tokio::test]
    async fn slow_consumer_makes_backpressure_observable() {
        let fixture = fixture(FakeMode::Backpressure);
        let mut accepted = fixture
            .runtime
            .submit(job("run:backpressure", "scope:backpressure"))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            terminal_event(&mut accepted.events).await,
            ProviderRunState::Succeeded
        );
        assert!(fixture.runtime.metrics().backpressure_waits > 0);
    }

    #[tokio::test]
    async fn provider_terminal_variants_and_deadline_use_generated_transitions() {
        for (mode, expected_state) in [
            (FakeMode::Partial, ProviderRunState::Partial),
            (FakeMode::Failed, ProviderRunState::Failed),
        ] {
            let fixture = fixture(mode);
            let mut accepted = fixture
                .runtime
                .submit(job("run:terminal", "scope:terminal"))
                .await
                .unwrap();
            assert_eq!(terminal_event(&mut accepted.events).await, expected_state);
        }

        let fixture = fixture(FakeMode::Slow);
        let mut deadline_job = job("run:timeout", "scope:timeout");
        deadline_job.deadline_unix_ms = TEST_NOW + 20;
        let mut accepted = fixture.runtime.submit(deadline_job).await.unwrap();
        assert_eq!(
            terminal_event(&mut accepted.events).await,
            ProviderRunState::TimedOut
        );
        assert_eq!(fixture.runtime.metrics().timed_out, 1);
    }

    #[test]
    fn provider_wire_mapping_is_generated_total_and_identity_fenced() {
        assert_eq!(
            PROVIDER_EVENT_MAPPINGS
                .iter()
                .map(|mapping| mapping.wire_event)
                .collect::<Vec<_>>(),
            [
                "ACCEPTED",
                "PROGRESS",
                "SCOPE_BEGIN",
                "OBSERVATION_CHUNK",
                "SCOPE_END",
                "TERMINAL",
                "CANCEL_ACKNOWLEDGED",
            ]
        );
        let context = WireEventContext {
            provider_id: "tree-sitter".into(),
            provider_run_id: "run:wire".into(),
            workspace_id: "workspace:test".into(),
            analysis_context_id: "context:source".into(),
            source_generation: 7,
            input_fingerprint: [5; 32],
        };
        let header = wire::ProviderEventHeader {
            provider_run_id: context.provider_run_id.clone(),
            workspace_id: context.workspace_id.clone(),
            analysis_context_id: context.analysis_context_id.clone(),
            source_generation: context.source_generation,
            sequence: 1,
            event_at_unix_ms: TEST_NOW,
            event_checksum: "b3:event".into(),
        };
        let scope = wire::ProviderScope {
            scope_kind: ProviderScopeKind::SourceFile as i32,
            scope_id: "file:test".into(),
        };
        let events = [
            wire::provider_event::Event::Accepted(wire::ProviderAcceptedEvent {
                header: Some(header.clone()),
                accepted_generation: 7,
            }),
            wire::provider_event::Event::Progress(wire::ProviderProgressEvent {
                header: Some(header.clone()),
                completed_units: 1,
                total_units: 2,
                phase: "parse".into(),
            }),
            wire::provider_event::Event::ScopeBegin(wire::ProviderScopeBeginEvent {
                header: Some(header.clone()),
                scope: Some(scope.clone()),
            }),
            wire::provider_event::Event::ObservationChunk(wire::ProviderObservationChunkEvent {
                header: Some(header.clone()),
                scope: Some(scope.clone()),
                observation_family_code: 10,
                arrow_ipc: vec![1],
                payload_reference: None,
                schema_digest: "b3:schema".into(),
                row_count: 1,
                chunk_digest: "b3:chunk".into(),
            }),
            wire::provider_event::Event::ScopeEnd(wire::ProviderScopeEndEvent {
                header: Some(header.clone()),
                scope: Some(scope),
                family_counts: std::collections::HashMap::from([(10, 1)]),
                scope_digest: "b3:scope".into(),
            }),
            wire::provider_event::Event::Terminal(wire::ProviderTerminalEvent {
                header: Some(header.clone()),
                state: wire::ProviderRunState::Succeeded as i32,
                capability_outcomes: Vec::new(),
                overall_digest: format!("b3:{}", "09".repeat(32)),
                error_code: None,
            }),
            wire::provider_event::Event::CancelAcknowledged(wire::CancelAcknowledgedEvent {
                header: Some(header),
                state: CancelAcknowledgementState::Cancelled as i32,
            }),
        ];
        for event in events {
            assert!(map_wire_event(wire::ProviderEvent { event: Some(event) }, &context).is_ok());
        }

        let mismatched = wire::ProviderEvent {
            event: Some(wire::provider_event::Event::Progress(
                wire::ProviderProgressEvent {
                    header: Some(wire::ProviderEventHeader {
                        provider_run_id: "run:other".into(),
                        ..wire::ProviderEventHeader::default()
                    }),
                    ..wire::ProviderProgressEvent::default()
                },
            )),
        };
        assert!(matches!(
            map_wire_event(mismatched, &context),
            Err(ProviderRuntimeError::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn wp60_behavioral_acceptance() {
        fn implements_common_contract<T: ProviderAdapter>() {}

        implements_common_contract::<crate::tree_sitter_adapter::TreeSitterAdapter>();
        implements_common_contract::<crate::ruff_adapter::RuffAdapter>();
        let in_process = PROVIDER_ENTRIES
            .iter()
            .filter(|provider| provider.placement == "IN_PROCESS")
            .map(|provider| provider.provider_id)
            .collect::<Vec<_>>();
        assert!(in_process.contains(&"tree-sitter"));
        assert!(in_process.contains(&"ruff-python"));

        let text = provider_text("value = 1\n");
        let tree_fixture = fixture_with_adapter(
            "tree-sitter",
            Arc::new(
                crate::tree_sitter_adapter::TreeSitterAdapter::new(
                    crate::tree_sitter_adapter::TreeSitterLanguage::Python,
                )
                .unwrap(),
            ),
        );
        let mut tree_job = job("run:wp60-tree", "scope:wp60-tree");
        tree_job.direct_work = ProviderDirectWork::TreeSitter {
            revision: 7,
            text: text.clone(),
        };
        let mut accepted = tree_fixture.runtime.submit(tree_job).await.unwrap();
        let tree_stream =
            receive_provider_fact_stream(&mut accepted.provider_facts, &accepted.cancellation)
                .await
                .unwrap();
        assert_eq!(tree_stream.manifest.provider_code, 10);
        assert_eq!(tree_stream.manifest.declared_rows, 0);
        assert_eq!(
            terminal_event(&mut accepted.events).await,
            ProviderRunState::Succeeded
        );

        let mut parser = crate::tree_sitter_adapter::TreeSitterAdapter::new(
            crate::tree_sitter_adapter::TreeSitterLanguage::Python,
        )
        .unwrap();
        let tree = parser
            .parse_full(7, text.clone(), &Cancellation::default())
            .unwrap();
        let ruff_fixture = fixture_with_adapter(
            "ruff-python",
            Arc::new(crate::ruff_adapter::RuffAdapter::new().unwrap()),
        );
        let mut ruff_job = job("run:wp60-ruff", "scope:wp60-ruff");
        ruff_job.direct_work = ProviderDirectWork::RuffPython {
            revision: 7,
            text,
            tree_sitter: tree,
        };
        let mut accepted = ruff_fixture.runtime.submit(ruff_job).await.unwrap();
        let ruff_stream =
            receive_provider_fact_stream(&mut accepted.provider_facts, &accepted.cancellation)
                .await
                .unwrap();
        assert_eq!(ruff_stream.manifest.provider_code, 20);
        assert_eq!(
            terminal_event(&mut accepted.events).await,
            ProviderRunState::Succeeded
        );
    }

    #[test]
    fn wp60_structural_acceptance() {
        use crate::registries::SyntaxFieldRole;

        assert_eq!(
            crate::registries::provider_field_role_code("tree-sitter-python-0-25-0", "name"),
            Some(SyntaxFieldRole::Name as u16)
        );
        assert_eq!(
            crate::registries::provider_field_role_code("ruff-python-0-0-7", "Callee"),
            Some(SyntaxFieldRole::Callee as u16)
        );
        assert!(
            PROVIDER_ENTRIES
                .iter()
                .all(|provider| !provider.provider_id.is_empty())
        );
        let fixture = fixture(FakeMode::Immediate);
        let workspace = job("run:wp60-evict", "scope:wp60-evict").workspace_id;
        fixture.runtime.admission.workspace(&workspace);
        fixture
            .runtime
            .admission
            .context(&workspace, "context:source");
        assert_eq!(fixture.runtime.admission.scope_counts(), (1, 1));
        fixture.runtime.evict_workspace(&workspace);
        assert_eq!(fixture.runtime.admission.scope_counts(), (0, 0));
    }

    #[tokio::test]
    async fn wp60_negative_zero_state() {
        let fixture = fixture(FakeMode::Backpressure);
        let run = job("run:wp60-duplicate", "scope:wp60-duplicate");
        let expected_run_id = run.provider_run_id.clone();
        let accepted = fixture.runtime.submit(run.clone()).await.unwrap();
        assert!(matches!(
            fixture.runtime.submit(run).await,
            Err(ProviderRuntimeError::DuplicateRun(id)) if id == expected_run_id
        ));
        accepted.cancellation.cancel();
        assert!(accepted.cancellation.is_cancelled());
        let missing_run_id = run_id("run:missing");
        assert!(matches!(
            fixture.runtime.cancel(&missing_run_id, "negative").await,
            Err(ProviderRuntimeError::RunNotFound(id)) if id == missing_run_id
        ));
    }

    #[tokio::test]
    async fn wp60_operational_acceptance() {
        let fixture = fixture(FakeMode::Slow);
        let mut accepted = fixture
            .runtime
            .submit(job("run:wp60-cancel", "scope:wp60-cancel"))
            .await
            .unwrap();
        let receive =
            receive_provider_fact_stream(&mut accepted.provider_facts, &accepted.cancellation);
        let cancel = async {
            tokio::task::yield_now().await;
            fixture
                .runtime
                .cancel(&run_id("run:wp60-cancel"), "stream-poll")
                .await
                .unwrap()
        };
        let (stream_result, acknowledgement) = tokio::join!(receive, cancel);
        assert!(matches!(
            stream_result,
            Err(crate::fact_ingest::FactIngestError::Protocol(message))
                if message == "provider fact stream cancelled"
        ));
        assert_eq!(
            acknowledgement.terminal_state,
            Some(ProviderRunState::Cancelled as i32)
        );
        assert_eq!(
            terminal_event(&mut accepted.events).await,
            ProviderRunState::Cancelled
        );
        assert_eq!(fixture.runtime.metrics().cancelled, 1);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One parity matrix covers both registered provider adapters.
    async fn provider_adapter_registration_parity() {
        let registry = SemanticProviderAdapterRegistry::new(
            Arc::new(FakeSemanticDriver),
            Arc::new(FakeSemanticDriver),
        )
        .unwrap();
        assert_eq!(registry.provider_ids(), ["pyrefly-python", "rustc-mir"]);
        for provider_id in registry.provider_ids() {
            let provider = PROVIDER_ENTRIES
                .iter()
                .find(|entry| entry.provider_id == provider_id)
                .expect("registered adapter must resolve through provider registry");
            assert_ne!(provider.placement, "IN_PROCESS");
            assert!(registry.adapter(provider_id).is_some());
            assert!(PROVIDER_RESOURCE_PROFILES.iter().any(|profile| {
                profile.profile_id == provider.resource_profile_id
                    && profile.provider_ids.contains(&provider.provider_id)
            }));
        }
        let mappings = PROVIDER_EVENT_MAPPINGS
            .iter()
            .map(|mapping| mapping.wire_event)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            mappings,
            BTreeSet::from([
                "ACCEPTED",
                "PROGRESS",
                "SCOPE_BEGIN",
                "OBSERVATION_CHUNK",
                "SCOPE_END",
                "TERMINAL",
                "CANCEL_ACKNOWLEDGED",
            ])
        );
        assert!(
            PROVIDER_EVENT_MAPPINGS
                .iter()
                .all(|mapping| mapping.mapping_version == "1.0")
        );

        let directory = tempdir().unwrap();
        let workspace_view = crate::source_image::ProviderWorkspaceView {
            workspace_id: [2; 16],
            source_generation: 7,
            workspace_root: directory.path().join("view"),
            dependency_root: directory.path().join("dependencies"),
            output_root: directory.path().join("output"),
            manifest_path: directory.path().join("manifest.json"),
            manifest_digest: [0x31; 32],
            dependency_manifest_digest: [0x32; 32],
            sandbox_profile_digest: format!("b3:{}", "33".repeat(32)),
            entries: Vec::new(),
        };
        let store = OperationalStore::open(&directory.path().join("operational.sqlite")).unwrap();
        let reader = store.reader_factory();
        let generation = Arc::new(Generation(AtomicU64::new(7)));
        let generation_oracle: Arc<dyn SourceGenerationOracle> = generation.clone();
        let journal: Arc<dyn ProviderRunJournal> =
            Arc::new(OperationalProviderRunJournal::new(store));
        let dispatch =
            ProviderRuntimeDispatch::semantic(&[0x41; 16], &registry, &generation_oracle, &journal)
                .unwrap();
        assert_eq!(dispatch.provider_ids(), ["pyrefly-python", "rustc-mir"]);
        for provider_id in dispatch.provider_ids() {
            let mut accepted = dispatch
                .submit(
                    provider_id,
                    semantic_job(
                        provider_id,
                        &format!("runtime-registration:{provider_id}"),
                        workspace_view.clone(),
                    ),
                )
                .await
                .unwrap();
            assert_eq!(
                terminal_event(&mut accepted.events).await,
                ProviderRunState::Succeeded
            );
        }
        generation.0.store(8, Ordering::Release);
        let mut stale = dispatch
            .submit(
                "pyrefly-python",
                semantic_job(
                    "pyrefly-python",
                    "runtime-registration:stale",
                    workspace_view,
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            terminal_event(&mut stale.events).await,
            ProviderRunState::StaleResult
        );
        let persisted = reader
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*),
                            SUM(state_code=?1),
                            SUM(state_code=?2),
                            COUNT(DISTINCT sandbox_profile_digest)
                     FROM provider_run",
                    rusqlite::params![
                        i64::from(ProviderRunState::Succeeded as u16),
                        i64::from(ProviderRunState::StaleResult as u16)
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
            })
            .unwrap();
        assert_eq!(persisted, (3, 2, 1, 1));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Runtime, registry, mapping, journal, and zero-state parity are one structural oracle.
    async fn pyrefly_provider_runtime_parity() {
        fn implements_semantic_driver<T: SemanticProviderDriver>() {}
        implements_semantic_driver::<crate::pyrefly_service::PyreflyProviderDriver>();

        let registry = SemanticProviderAdapterRegistry::new(
            Arc::new(FakeSemanticDriver),
            Arc::new(FakeSemanticDriver),
        )
        .unwrap();
        let provider = PROVIDER_ENTRIES
            .iter()
            .find(|entry| entry.provider_id == "pyrefly-python")
            .unwrap();
        assert_eq!(provider.placement, "SIDECAR");
        assert_eq!(provider.resource_profile_id, "sidecar-semantic-standard");
        assert_eq!(provider.event_mapping_version, "1.0");

        let directory = tempdir().unwrap();
        let view = crate::source_image::ProviderWorkspaceView {
            workspace_id: [2; 16],
            source_generation: 7,
            workspace_root: directory.path().join("view"),
            dependency_root: directory.path().join("dependencies"),
            output_root: directory.path().join("output"),
            manifest_path: directory.path().join("manifest.json"),
            manifest_digest: [0x51; 32],
            dependency_manifest_digest: [0x52; 32],
            sandbox_profile_digest: format!("b3:{}", "53".repeat(32)),
            entries: Vec::new(),
        };
        let store = OperationalStore::open(&directory.path().join("journal.sqlite")).unwrap();
        let reader = store.reader_factory();
        let generation: Arc<dyn SourceGenerationOracle> = Arc::new(Generation(AtomicU64::new(7)));
        let journal: Arc<dyn ProviderRunJournal> =
            Arc::new(OperationalProviderRunJournal::new(store));
        let dispatch =
            ProviderRuntimeDispatch::semantic(&[0x61; 16], &registry, &generation, &journal)
                .unwrap();
        let mut accepted = dispatch
            .submit(
                "pyrefly-python",
                semantic_job("pyrefly-python", "pyrefly-runtime-parity", view.clone()),
            )
            .await
            .unwrap();
        let mut observed = Vec::new();
        while let Some(event) = accepted.events.recv().await {
            match event {
                ProviderEvent::Accepted { .. } => observed.push("ACCEPTED"),
                ProviderEvent::Progress { .. } => observed.push("PROGRESS"),
                ProviderEvent::Completed { state, .. } => {
                    assert_eq!(state, ProviderRunState::Succeeded);
                    observed.push("TERMINAL");
                    break;
                }
                ProviderEvent::Failed { state, code, .. } => {
                    panic!("Pyrefly runtime parity failed as {state:?}: {code}")
                }
                _ => {}
            }
        }
        assert_eq!(observed, ["ACCEPTED", "PROGRESS", "TERMINAL"]);
        let persisted = reader
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT provider_code, state_code, sandbox_profile_digest FROM provider_run",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
            })
            .unwrap();
        assert_eq!(
            persisted,
            (
                i64::from(provider.provider_code),
                i64::from(ProviderRunState::Succeeded as u16),
                view.sandbox_profile_digest,
            )
        );

        let provider_registry = include_str!("../contracts/registry/provider-registry.yaml");
        assert!(provider_registry.contains("protocol_package: codefabric.pyrefly.v1"));
        assert!(provider_registry.contains("protocol_service: PyreflySidecar"));
        let feature_registry = include_str!("../contracts/rpc/feature-registry.yaml");
        for mapping in [
            "ACCEPTED",
            "PROGRESS",
            "SCOPE_BEGIN",
            "OBSERVATION_CHUNK",
            "SCOPE_END",
            "TERMINAL",
            "CANCEL_ACKNOWLEDGED",
        ] {
            assert!(feature_registry.contains(&format!("wire_event: {mapping}")));
        }
    }
}
