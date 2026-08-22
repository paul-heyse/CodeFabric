//! Bounded accepted-handle execution for all fact providers.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use prost::Message as _;
use rayon::ThreadPool;
use thiserror::Error;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

use crate::fact_ingest::{ObservationMessage, StreamTerminal, bounded_observation_channel};
use crate::identity::{IdentityDomain, decode_public_id};
use crate::operational_store::{OperationalReaderFactory, OperationalStore, ProviderRunRecord};
use crate::registries::{
    PROVIDER_ENTRIES, PROVIDER_EVENT_MAPPINGS, PROVIDER_RESOURCE_PROFILES,
    PROVIDER_RUN_STATE_TRANSITIONS, PROVIDER_RUN_STATE_VALUES, ProviderEntry,
    ProviderResourceProfileEntry, ProviderRunState, generated_transition, registry_state_name,
};
use crate::rpc::generated::codefabric::provider::v1 as wire;
use crate::rpc::generated::codefabric::provider::v1::{
    CancelAcknowledgement, CancelAcknowledgementState, ProviderJobSpec, ProviderScopeKind,
};

const EVENT_CHANNEL_CAPACITY: usize = 32;
const OBSERVATION_CHANNEL_CAPACITY: usize = 4;
const ADMISSION_AUDIT_CAPACITY: usize = 64;
const MAX_DIAGNOSTIC_BYTES: usize = 1_024;
const INTENT_NONE: u8 = 0;
const INTENT_CANCEL: u8 = 1;
const INTENT_SUPERSEDE: u8 = 2;

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
    ArrowIpcChunk {
        identity: ProviderEventIdentity,
        observation_family_code: u32,
        arrow_ipc: Vec<u8>,
        schema_digest: String,
        row_count: u64,
        chunk_digest: String,
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
    pub observations: mpsc::Receiver<ObservationMessage>,
}

/// Cooperative cancellation signal available to synchronous provider adapters.
#[derive(Clone, Debug)]
pub struct ProviderCancellation {
    cancelled: Arc<AtomicBool>,
    check_interval: u32,
}

impl ProviderCancellation {
    /// Whether the runtime has requested cancellation or supersession.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Registry-defined work interval at which a provider must poll cancellation.
    #[must_use]
    pub const fn check_interval(&self) -> u32 {
        self.check_interval
    }
}

/// Bounded sink used only by provider adapters running on dedicated CPU capacity.
#[derive(Clone)]
pub struct ProviderEventSink {
    events: mpsc::Sender<ProviderEvent>,
    observations: mpsc::Sender<ObservationMessage>,
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
    pub fn send_observation(
        &self,
        message: ObservationMessage,
    ) -> Result<(), ProviderRuntimeError> {
        if let ObservationMessage::Manifest(manifest) = &message
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
        match self.observations.try_send(message) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(message)) => {
                self.counters
                    .backpressure_waits
                    .fetch_add(1, Ordering::Relaxed);
                self.observations
                    .blocking_send(message)
                    .map_err(|_| ProviderRuntimeError::EventReceiverClosed)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(ProviderRuntimeError::EventReceiverClosed)
            }
        }
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
        spec: ProviderJobSpec,
        events: ProviderEventSink,
        cancellation: ProviderCancellation,
    ) -> Result<ProviderCompletion, ProviderRuntimeError>;
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
            Ok(ProviderEvent::Progress {
                identity: wire_identity(event.header, context)?,
                completed_units: 0,
                total_units: 0,
                phase: format!("scope-begin:{}:{}", scope.scope_kind, scope.scope_id),
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
            Ok(ProviderEvent::Progress {
                identity: wire_identity(event.header, context)?,
                completed_units: event.family_counts.values().copied().sum(),
                total_units: event.family_counts.values().copied().sum(),
                phase: format!("scope-end:{}:{}", scope.scope_kind, scope.scope_id),
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

    fn validate_job(
        &self,
        spec: &ProviderJobSpec,
    ) -> Result<ValidatedJobIds, ProviderRuntimeError> {
        if spec.supersession_key.is_empty() {
            return Err(ProviderRuntimeError::InvalidJob(
                "supersession identity is required".into(),
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

    fn supersession_key(&self, spec: &ProviderJobSpec) -> String {
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
        spec: &ProviderJobSpec,
        ids: ValidatedJobIds,
        accepted_at: i64,
    ) -> ProviderRunRecord {
        let owner_id = spec
            .scopes
            .iter()
            .find(|scope| scope.scope_kind == ProviderScopeKind::SemanticOwner as i32)
            .map(|scope| scope.scope_id.as_bytes().to_vec());
        let build_unit_id = spec
            .scopes
            .iter()
            .find(|scope| scope.scope_kind == ProviderScopeKind::BuildUnit as i32)
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
            input_fingerprint: blake3::hash(&spec.encode_to_vec()).as_bytes().to_vec(),
            output_fingerprint: None,
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

    async fn acquire_all(
        &self,
        spec: &ProviderJobSpec,
        control: &RunControl,
    ) -> Result<PermitSet, u8> {
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
        spec: &ProviderJobSpec,
        control: &RunControl,
        sink: &ProviderEventSink,
    ) -> oneshot::Receiver<Result<ProviderCompletion, ProviderRuntimeError>> {
        let (result_sender, result_receiver) = oneshot::channel();
        let adapter = Arc::clone(&self.adapter);
        let adapter_spec = spec.clone();
        let adapter_sink = sink.clone();
        let cancellation = ProviderCancellation {
            cancelled: Arc::clone(&control.cancelled),
            check_interval: self.profile.cancellation_check_interval,
        };
        self.cpu_pool.spawn(move || {
            let _ = result_sender.send(adapter.run(adapter_spec, adapter_sink, cancellation));
        });
        result_receiver
    }

    async fn await_completion(
        &self,
        spec: &ProviderJobSpec,
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

    async fn run_job(
        self,
        spec: ProviderJobSpec,
        control: Arc<RunControl>,
        sink: ProviderEventSink,
    ) {
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
        spec: &ProviderJobSpec,
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
            .observations
            .send(ObservationMessage::Terminal(stream_terminal))
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

#[async_trait]
pub trait ProviderExecutor: Send + Sync {
    async fn submit(
        &self,
        spec: ProviderJobSpec,
    ) -> Result<AcceptedProviderJob, ProviderRuntimeError>;
    async fn cancel(
        &self,
        run_id: &str,
        reason: &str,
    ) -> Result<CancelAcknowledgement, ProviderRuntimeError>;
}

#[async_trait]
impl ProviderExecutor for ProviderRuntime {
    async fn submit(
        &self,
        spec: ProviderJobSpec,
    ) -> Result<AcceptedProviderJob, ProviderRuntimeError> {
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
        let (observations_sender, observations) =
            bounded_observation_channel(OBSERVATION_CHANNEL_CAPACITY);
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
            observations: observations_sender,
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
            observations,
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
    for (index, chunk) in payload.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| ProviderRuntimeError::Protocol("terminal digest is malformed".into()))?;
        digest[index] = u8::from_str_radix(text, 16)
            .map_err(|_| ProviderRuntimeError::Protocol("terminal digest is malformed".into()))?;
    }
    Ok(digest)
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
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] =
            u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| ())?, 16).map_err(|_| ())?;
    }
    Ok(output)
}

fn validated_job_ids(spec: &ProviderJobSpec) -> Result<ValidatedJobIds, ProviderRuntimeError> {
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
    blake3::hash(bounded).as_bytes()[..16].to_vec()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::sync::atomic::AtomicU64;
    use std::thread;
    use std::time::Instant;

    use tempfile::tempdir;

    use super::*;
    use crate::fact_ingest::{ObservationManifest, receive_observation_stream};
    use crate::rpc::generated::codefabric::provider::v1::{ResourceEstimate, SourceSnapshotLease};

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

    impl ProviderAdapter for FakeAdapter {
        fn run(
            &self,
            spec: ProviderJobSpec,
            events: ProviderEventSink,
            cancellation: ProviderCancellation,
        ) -> Result<ProviderCompletion, ProviderRuntimeError> {
            assert_eq!(cancellation.check_interval(), 1_024);
            if !matches!(self.mode, FakeMode::Slow) {
                assert!(!cancellation.is_cancelled());
            }
            events.send_observation(ObservationMessage::Manifest(ObservationManifest {
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
        let directory = tempdir().unwrap();
        let store = OperationalStore::open(&directory.path().join("operational.sqlite")).unwrap();
        let reader = store.reader_factory();
        let generation = Arc::new(Generation(AtomicU64::new(7)));
        let runtime = ProviderRuntime::new_with_clock(
            "tree-sitter",
            [5; 16].to_vec(),
            Arc::new(FakeAdapter { mode }),
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

    fn run_id(label: &str) -> String {
        let digest = blake3::hash(label.as_bytes());
        let mut encoded = String::with_capacity(32);
        for byte in &digest.as_bytes()[..16] {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        encoded
    }

    fn job(run_label: &str, supersession_key: &str) -> ProviderJobSpec {
        let workspace_id =
            crate::identity::encode_public_id(IdentityDomain::Workspace, None, [2; 16]).unwrap();
        ProviderJobSpec {
            provider_run_id: run_id(run_label),
            workspace_id: workspace_id.clone(),
            analysis_context_id: "context:source".into(),
            source_generation: 7,
            source_snapshot_lease: Some(SourceSnapshotLease {
                lease_id: "lease:test".into(),
                workspace_id,
                source_generation: 7,
                source_manifest_digest: "b3:test".into(),
                expires_at_unix_ms: TEST_NOW + 120_000,
                blobs: Vec::new(),
            }),
            resource_estimate: Some(ResourceEstimate {
                input_bytes: 128,
                expected_output_bytes: 256,
                cpu_weight: 1,
                memory_mib: 64,
            }),
            deadline_unix_ms: TEST_NOW + 1_000,
            supersession_key: supersession_key.into(),
            resource_profile_id: "in-process-syntax-standard".into(),
            ..ProviderJobSpec::default()
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
            .push(wire::BlobReference {
                byte_length: 16_777_217,
                ..wire::BlobReference::default()
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
        let observations = receive_observation_stream(&mut accepted.observations)
            .await
            .unwrap();
        assert_eq!(observations.terminal, StreamTerminal::Completed);
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
            first.observations.recv().await,
            Some(ObservationMessage::Manifest(_))
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
        let (observations, _observation_receiver) = bounded_observation_channel(1);
        let valid = job("run:manifest-fence", "scope:required");
        let validated_ids = fixture.runtime.validate_job(&valid).unwrap();
        let sink = ProviderEventSink {
            events,
            observations,
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
        let mismatched = ObservationManifest {
            stream_id: [1; 16],
            workspace_id: [9; 16],
            analysis_context_id: validated_ids.context,
            source_generation: 7,
            provider_code: 10,
            provider_version: "test".into(),
            provider_run_id: validated_ids.run,
            schema_fingerprints: BTreeMap::new(),
            declared_rows: 0,
        };
        assert!(matches!(
            sink.send_observation(ObservationMessage::Manifest(mismatched)),
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
            cancelled.observations.recv().await,
            Some(ObservationMessage::Manifest(_))
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
}
