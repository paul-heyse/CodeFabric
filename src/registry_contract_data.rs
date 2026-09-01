// Application-owned released-wire and operational contract data.
//
// This module is intentionally handwritten target input. It is not generated from, selected by,
// or replayed through the retired repository model/ontology compiler.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownRegistryCode {
    pub domain: &'static str,
    pub code: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub code: u16,
    pub name: &'static str,
    pub slug: &'static str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpgdFeatureMask(u64);
impl CpgdFeatureMask {
    pub const NONE: Self = Self(0);
    pub const QUERY_RESUME: Self = Self(0x0000_0000_0000_0001);
    pub const RESULT_RESOURCES: Self = Self(0x0000_0000_0000_0002);
    pub const ZSTD_PAYLOADS: Self = Self(0x0000_0000_0000_0004);
    pub const TRACE_CONTEXT: Self = Self(0x0000_0000_0000_0008);
    pub const SUPPORTED: Self = Self(0x0000_0000_0000_000f);
    pub const REQUIRED: Self = Self(0x0000_0000_0000_0001);
    #[must_use]
    pub const fn from_wire(bits: u64) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn missing_from(self, available: Self) -> Self {
        Self(self.0 & !available.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderFeatureMask(u64);
impl ProviderFeatureMask {
    pub const NONE: Self = Self(0);
    pub const ACCEPTED_HANDLE_EVENTS: Self = Self(0x0000_0000_0001_0000);
    pub const CREDIT_CONTROL: Self = Self(0x0000_0000_0002_0000);
    pub const SUPPORTED: Self = Self(0x0000_0000_0003_0000);
    pub const REQUIRED: Self = Self(0x0000_0000_0003_0000);
    #[must_use]
    pub const fn from_wire(bits: u64) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn missing_from(self, available: Self) -> Self {
        Self(self.0 & !available.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PyreflyFeatureMask(u64);
impl PyreflyFeatureMask {
    pub const NONE: Self = Self(0);
    pub const ARROW_IPC_OBSERVATIONS: Self = Self(0x0000_0001_0000_0000);
    pub const MULTI_CONTEXT: Self = Self(0x0000_0002_0000_0000);
    pub const SUPPORTED: Self = Self(0x0000_0003_0000_0000);
    pub const REQUIRED: Self = Self(0x0000_0001_0000_0000);
    #[must_use]
    pub const fn from_wire(bits: u64) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn missing_from(self, available: Self) -> Self {
        Self(self.0 & !available.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RustcFeatureMask(u64);
impl RustcFeatureMask {
    pub const NONE: Self = Self(0);
    pub const CLOSED_OWNER_STREAM: Self = Self(0x0001_0000_0000_0000);
    pub const PARTIAL_COMPILATION: Self = Self(0x0002_0000_0000_0000);
    pub const SUPPORTED: Self = Self(0x0001_0000_0000_0000);
    pub const REQUIRED: Self = Self(0x0001_0000_0000_0000);
    #[must_use]
    pub const fn from_wire(bits: u64) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn missing_from(self, available: Self) -> Self {
        Self(self.0 & !available.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum EventStreamHealth {
    Healthy = 10,
    RescanRequired = 20,
    Degraded = 30,
    Unavailable = 40,
}
impl TryFrom<u16> for EventStreamHealth {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Healthy),
            20 => Ok(Self::RescanRequired),
            30 => Ok(Self::Degraded),
            40 => Ok(Self::Unavailable),
            _ => Err(UnknownRegistryCode {
                domain: "EVENT_STREAM_HEALTH",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum FreshnessState {
    Current = 10,
    PotentiallyStale = 20,
    Unavailable = 30,
}
impl TryFrom<u16> for FreshnessState {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Current),
            20 => Ok(Self::PotentiallyStale),
            30 => Ok(Self::Unavailable),
            _ => Err(UnknownRegistryCode {
                domain: "FRESHNESS_STATE",
                code,
            }),
        }
    }
}
pub const FRESHNESS_STATE_VALUES: &[RegistryEntry] = &[
    RegistryEntry {
        code: 10,
        name: "CURRENT",
        slug: "current",
    },
    RegistryEntry {
        code: 20,
        name: "POTENTIALLY_STALE",
        slug: "potentially-stale",
    },
    RegistryEntry {
        code: 30,
        name: "UNAVAILABLE",
        slug: "unavailable",
    },
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitAccelerationStatus {
    NotAGitWorktree = 10,
    GitUnavailable = 20,
    GitReady = 30,
    GitMetadataDirty = 40,
    GitScanning = 50,
    GitOperationInProgress = 60,
    GitBulkReconciling = 70,
    GitDegraded = 80,
}
impl TryFrom<u16> for GitAccelerationStatus {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::NotAGitWorktree),
            20 => Ok(Self::GitUnavailable),
            30 => Ok(Self::GitReady),
            40 => Ok(Self::GitMetadataDirty),
            50 => Ok(Self::GitScanning),
            60 => Ok(Self::GitOperationInProgress),
            70 => Ok(Self::GitBulkReconciling),
            80 => Ok(Self::GitDegraded),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_ACCELERATION_STATUS",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitCandidateMode {
    Status = 10,
    HeadTree = 20,
}
impl TryFrom<u16> for GitCandidateMode {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Status),
            20 => Ok(Self::HeadTree),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_CANDIDATE_MODE",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitCandidateOrigin {
    IndexWorktree = 10,
    HeadIndex = 20,
    HeadTree = 30,
}
impl TryFrom<u16> for GitCandidateOrigin {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::IndexWorktree),
            20 => Ok(Self::HeadIndex),
            30 => Ok(Self::HeadTree),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_CANDIDATE_ORIGIN",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitHashAlgorithm {
    Sha1 = 10,
    Sha256 = 20,
}
impl TryFrom<u16> for GitHashAlgorithm {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Sha1),
            20 => Ok(Self::Sha256),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_HASH_ALGORITHM",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitHeadKind {
    Symbolic = 10,
    Detached = 20,
    Unborn = 30,
}
impl TryFrom<u16> for GitHeadKind {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Symbolic),
            20 => Ok(Self::Detached),
            30 => Ok(Self::Unborn),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_HEAD_KIND",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitInventoryClassification {
    Tracked = 10,
    UntrackedNotIgnored = 20,
    UntrackedIgnored = 30,
    TrackedButIgnoredPatternMatches = 40,
    ExcludedByCodeFabricPolicy = 50,
    SubmoduleGitlink = 60,
    NestedRepository = 70,
    SpecialFile = 80,
}
impl TryFrom<u16> for GitInventoryClassification {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Tracked),
            20 => Ok(Self::UntrackedNotIgnored),
            30 => Ok(Self::UntrackedIgnored),
            40 => Ok(Self::TrackedButIgnoredPatternMatches),
            50 => Ok(Self::ExcludedByCodeFabricPolicy),
            60 => Ok(Self::SubmoduleGitlink),
            70 => Ok(Self::NestedRepository),
            80 => Ok(Self::SpecialFile),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_INVENTORY_CLASSIFICATION",
                code,
            }),
        }
    }
}
pub const GIT_INVENTORY_CLASSIFICATION_VALUES: &[RegistryEntry] = &[
    RegistryEntry {
        code: 10,
        name: "TRACKED",
        slug: "tracked",
    },
    RegistryEntry {
        code: 20,
        name: "UNTRACKED_NOT_IGNORED",
        slug: "untracked-not-ignored",
    },
    RegistryEntry {
        code: 30,
        name: "UNTRACKED_IGNORED",
        slug: "untracked-ignored",
    },
    RegistryEntry {
        code: 40,
        name: "TRACKED_BUT_IGNORED_PATTERN_MATCHES",
        slug: "tracked-but-ignored-pattern-matches",
    },
    RegistryEntry {
        code: 50,
        name: "EXCLUDED_BY_CODE_FABRIC_POLICY",
        slug: "excluded-by-code-fabric-policy",
    },
    RegistryEntry {
        code: 60,
        name: "SUBMODULE_GITLINK",
        slug: "submodule-gitlink",
    },
    RegistryEntry {
        code: 70,
        name: "NESTED_REPOSITORY",
        slug: "nested-repository",
    },
    RegistryEntry {
        code: 80,
        name: "SPECIAL_FILE",
        slug: "special-file",
    },
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitOperationState {
    Clean = 10,
    Merge = 20,
    Rebase = 30,
    CherryPick = 40,
    Revert = 50,
    Bisect = 60,
    Apply = 70,
    OtherOperation = 80,
    Unknown = 90,
}
impl TryFrom<u16> for GitOperationState {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Clean),
            20 => Ok(Self::Merge),
            30 => Ok(Self::Rebase),
            40 => Ok(Self::CherryPick),
            50 => Ok(Self::Revert),
            60 => Ok(Self::Bisect),
            70 => Ok(Self::Apply),
            80 => Ok(Self::OtherOperation),
            90 => Ok(Self::Unknown),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_OPERATION_STATE",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum GitRepositoryKind {
    Common = 10,
    LinkedWorktree = 20,
    Submodule = 30,
}
impl TryFrom<u16> for GitRepositoryKind {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Common),
            20 => Ok(Self::LinkedWorktree),
            30 => Ok(Self::Submodule),
            _ => Err(UnknownRegistryCode {
                domain: "GIT_REPOSITORY_KIND",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum InventoryFileKind {
    Regular = 10,
    Symlink = 20,
    Special = 30,
}
impl TryFrom<u16> for InventoryFileKind {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Regular),
            20 => Ok(Self::Symlink),
            30 => Ok(Self::Special),
            _ => Err(UnknownRegistryCode {
                domain: "INVENTORY_FILE_KIND",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum InventoryInclusionState {
    Included = 10,
    ExcludedPolicy = 20,
    ExcludedSpecialFile = 30,
    ExcludedSizeLimit = 40,
}
impl TryFrom<u16> for InventoryInclusionState {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Included),
            20 => Ok(Self::ExcludedPolicy),
            30 => Ok(Self::ExcludedSpecialFile),
            40 => Ok(Self::ExcludedSizeLimit),
            _ => Err(UnknownRegistryCode {
                domain: "INVENTORY_INCLUSION_STATE",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum NewlineKind {
    None = 10,
    Lf = 20,
    Crlf = 30,
    Cr = 40,
    Mixed = 50,
}
impl TryFrom<u16> for NewlineKind {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::None),
            20 => Ok(Self::Lf),
            30 => Ok(Self::Crlf),
            40 => Ok(Self::Cr),
            50 => Ok(Self::Mixed),
            _ => Err(UnknownRegistryCode {
                domain: "NEWLINE_KIND",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum Phase {
    InputValidation = 10,
    SchemaBinding = 20,
    LogicalPlanning = 30,
    PolicyValidation = 40,
    PhysicalPlanning = 50,
    Execution = 60,
    WriteValidation = 70,
    Commit = 80,
    SnapshotConstruction = 90,
    SnapshotActivation = 100,
    Shutdown = 110,
}
impl TryFrom<u16> for Phase {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::InputValidation),
            20 => Ok(Self::SchemaBinding),
            30 => Ok(Self::LogicalPlanning),
            40 => Ok(Self::PolicyValidation),
            50 => Ok(Self::PhysicalPlanning),
            60 => Ok(Self::Execution),
            70 => Ok(Self::WriteValidation),
            80 => Ok(Self::Commit),
            90 => Ok(Self::SnapshotConstruction),
            100 => Ok(Self::SnapshotActivation),
            110 => Ok(Self::Shutdown),
            _ => Err(UnknownRegistryCode {
                domain: "PHASE",
                code,
            }),
        }
    }
}
pub const PHASE_VALUES: &[RegistryEntry] = &[
    RegistryEntry {
        code: 10,
        name: "INPUT_VALIDATION",
        slug: "input-validation",
    },
    RegistryEntry {
        code: 20,
        name: "SCHEMA_BINDING",
        slug: "schema-binding",
    },
    RegistryEntry {
        code: 30,
        name: "LOGICAL_PLANNING",
        slug: "logical-planning",
    },
    RegistryEntry {
        code: 40,
        name: "POLICY_VALIDATION",
        slug: "policy-validation",
    },
    RegistryEntry {
        code: 50,
        name: "PHYSICAL_PLANNING",
        slug: "physical-planning",
    },
    RegistryEntry {
        code: 60,
        name: "EXECUTION",
        slug: "execution",
    },
    RegistryEntry {
        code: 70,
        name: "WRITE_VALIDATION",
        slug: "write-validation",
    },
    RegistryEntry {
        code: 80,
        name: "COMMIT",
        slug: "commit",
    },
    RegistryEntry {
        code: 90,
        name: "SNAPSHOT_CONSTRUCTION",
        slug: "snapshot-construction",
    },
    RegistryEntry {
        code: 100,
        name: "SNAPSHOT_ACTIVATION",
        slug: "snapshot-activation",
    },
    RegistryEntry {
        code: 110,
        name: "SHUTDOWN",
        slug: "shutdown",
    },
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum ProviderRunState {
    Queued = 10,
    Running = 20,
    Succeeded = 30,
    Partial = 40,
    Failed = 50,
    TimedOut = 60,
    Cancelled = 70,
    Superseded = 80,
    Crashed = 90,
    ProtocolError = 100,
    StaleResult = 110,
    StaleGitBaseline = 120,
}
impl TryFrom<u16> for ProviderRunState {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Queued),
            20 => Ok(Self::Running),
            30 => Ok(Self::Succeeded),
            40 => Ok(Self::Partial),
            50 => Ok(Self::Failed),
            60 => Ok(Self::TimedOut),
            70 => Ok(Self::Cancelled),
            80 => Ok(Self::Superseded),
            90 => Ok(Self::Crashed),
            100 => Ok(Self::ProtocolError),
            110 => Ok(Self::StaleResult),
            120 => Ok(Self::StaleGitBaseline),
            _ => Err(UnknownRegistryCode {
                domain: "PROVIDER_RUN_STATE",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SourceTrustState {
    Unverified = 10,
    Verifying = 20,
    Current = 30,
    PotentiallyStale = 40,
    Unavailable = 50,
}
impl TryFrom<u16> for SourceTrustState {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Unverified),
            20 => Ok(Self::Verifying),
            30 => Ok(Self::Current),
            40 => Ok(Self::PotentiallyStale),
            50 => Ok(Self::Unavailable),
            _ => Err(UnknownRegistryCode {
                domain: "SOURCE_TRUST_STATE",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum UpdateCandidateStrategy {
    IsolatedPaths = 10,
    GitStatusIndex = 20,
    HeadTreeAndStatus = 30,
    GenericInventory = 40,
}
impl TryFrom<u16> for UpdateCandidateStrategy {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::IsolatedPaths),
            20 => Ok(Self::GitStatusIndex),
            30 => Ok(Self::HeadTreeAndStatus),
            40 => Ok(Self::GenericInventory),
            _ => Err(UnknownRegistryCode {
                domain: "UPDATE_CANDIDATE_STRATEGY",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum WorkspaceLifecycle {
    Bootstrapping = 10,
    Ready = 20,
    Degraded = 30,
    Disabled = 40,
    Failed = 50,
}
impl TryFrom<u16> for WorkspaceLifecycle {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Bootstrapping),
            20 => Ok(Self::Ready),
            30 => Ok(Self::Degraded),
            40 => Ok(Self::Disabled),
            50 => Ok(Self::Failed),
            _ => Err(UnknownRegistryCode {
                domain: "WorkspaceLifecycle",
                code,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum WorkspaceRegistryLifecycle {
    Registering = 10,
    Disabled = 20,
    Opening = 30,
    Bootstrapping = 40,
    Ready = 50,
    Degraded = 60,
    Disabling = 70,
    Removing = 80,
    Removed = 90,
    Failed = 100,
}
impl TryFrom<u16> for WorkspaceRegistryLifecycle {
    type Error = UnknownRegistryCode;
    fn try_from(code: u16) -> Result<Self, UnknownRegistryCode> {
        match code {
            10 => Ok(Self::Registering),
            20 => Ok(Self::Disabled),
            30 => Ok(Self::Opening),
            40 => Ok(Self::Bootstrapping),
            50 => Ok(Self::Ready),
            60 => Ok(Self::Degraded),
            70 => Ok(Self::Disabling),
            80 => Ok(Self::Removing),
            90 => Ok(Self::Removed),
            100 => Ok(Self::Failed),
            _ => Err(UnknownRegistryCode {
                domain: "WorkspaceRegistryLifecycle",
                code,
            }),
        }
    }
}
pub const WORKSPACE_REGISTRY_LIFECYCLE_VALUES: &[RegistryEntry] = &[
    RegistryEntry {
        code: 10,
        name: "REGISTERING",
        slug: "registering",
    },
    RegistryEntry {
        code: 20,
        name: "DISABLED",
        slug: "disabled",
    },
    RegistryEntry {
        code: 30,
        name: "OPENING",
        slug: "opening",
    },
    RegistryEntry {
        code: 40,
        name: "BOOTSTRAPPING",
        slug: "bootstrapping",
    },
    RegistryEntry {
        code: 50,
        name: "READY",
        slug: "ready",
    },
    RegistryEntry {
        code: 60,
        name: "DEGRADED",
        slug: "degraded",
    },
    RegistryEntry {
        code: 70,
        name: "DISABLING",
        slug: "disabling",
    },
    RegistryEntry {
        code: 80,
        name: "REMOVING",
        slug: "removing",
    },
    RegistryEntry {
        code: 90,
        name: "REMOVED",
        slug: "removed",
    },
    RegistryEntry {
        code: 100,
        name: "FAILED",
        slug: "failed",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateTransitionEntry {
    pub from: &'static str,
    pub event: &'static str,
    pub guard: &'static str,
    pub to: &'static str,
    pub actions: &'static [&'static str],
    pub idempotency_key: &'static str,
    pub error_on_illegal: &'static str,
}

pub const WORKSPACE_REGISTRY_LIFECYCLE_TRANSITIONS: &[StateTransitionEntry] = &[
    StateTransitionEntry {
        from: "REGISTERING",
        event: "registration-created",
        guard: "root-authorized",
        to: "DISABLED",
        actions: &["persist-registration"],
        idempotency_key: "registry:registered",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "DISABLED",
        event: "enable",
        guard: "operator-authorized",
        to: "OPENING",
        actions: &["open-root"],
        idempotency_key: "registry:open",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "DISABLED",
        event: "remove",
        guard: "no-active-leases",
        to: "REMOVING",
        actions: &["stop-workspace"],
        idempotency_key: "registry:remove",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "OPENING",
        event: "root-opened",
        guard: "root-identity-matches",
        to: "BOOTSTRAPPING",
        actions: &["start-inventory"],
        idempotency_key: "registry:bootstrap",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "OPENING",
        event: "open-failed",
        guard: "terminal-root-error",
        to: "FAILED",
        actions: &["publish-diagnostic"],
        idempotency_key: "registry:failed",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "OPENING",
        event: "disable",
        guard: "operator-authorized",
        to: "DISABLING",
        actions: &["cancel-open", "stop-watchers", "stop-providers"],
        idempotency_key: "registry:disable",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "BOOTSTRAPPING",
        event: "first-snapshot-active",
        guard: "snapshot-valid",
        to: "READY",
        actions: &["publish-readiness"],
        idempotency_key: "registry:ready",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "BOOTSTRAPPING",
        event: "bootstrap-failed",
        guard: "terminal-build-error",
        to: "FAILED",
        actions: &["publish-diagnostic"],
        idempotency_key: "registry:failed",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "BOOTSTRAPPING",
        event: "disable",
        guard: "operator-authorized",
        to: "DISABLING",
        actions: &["cancel-bootstrap", "stop-watchers", "stop-providers"],
        idempotency_key: "registry:disable",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "READY",
        event: "source-degraded",
        guard: "source-not-current",
        to: "DEGRADED",
        actions: &["preserve-last-snapshot"],
        idempotency_key: "registry:degraded",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "READY",
        event: "disable",
        guard: "operator-authorized",
        to: "DISABLING",
        actions: &["stop-watchers", "stop-providers"],
        idempotency_key: "registry:disable",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "DEGRADED",
        event: "source-current",
        guard: "reconciliation-complete",
        to: "READY",
        actions: &["publish-readiness"],
        idempotency_key: "registry:ready",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "DEGRADED",
        event: "disable",
        guard: "operator-authorized",
        to: "DISABLING",
        actions: &["stop-watchers", "stop-providers"],
        idempotency_key: "registry:disable",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "DISABLING",
        event: "stopped",
        guard: "no-provider-work",
        to: "DISABLED",
        actions: &["persist-disabled"],
        idempotency_key: "registry:disabled",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "REMOVING",
        event: "removal-complete",
        guard: "retention-policy-applied",
        to: "REMOVED",
        actions: &["retire-registration"],
        idempotency_key: "registry:removed",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "FAILED",
        event: "disable",
        guard: "operator-authorized",
        to: "DISABLED",
        actions: &["persist-disabled"],
        idempotency_key: "registry:disabled",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
    StateTransitionEntry {
        from: "FAILED",
        event: "remove",
        guard: "no-active-leases",
        to: "REMOVING",
        actions: &["stop-workspace"],
        idempotency_key: "registry:remove",
        error_on_illegal: "STATE_TRANSITION_VIOLATION",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicErrorEntry {
    pub code: u16,
    pub name: &'static str,
    pub owning_layer: &'static str,
    pub severity: &'static str,
    pub retryability: &'static str,
    pub scope: &'static str,
    pub public_message_template: &'static str,
    pub diagnostic_linkage: &'static str,
    pub grpc_status: &'static str,
    pub mcp_mapping: &'static str,
}

pub const PUBLIC_ERROR_ENTRIES: &[PublicErrorEntry] = &[
    PublicErrorEntry {
        code: 7000,
        name: "INCOMPATIBLE_MAJOR",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Incompatible protocol major",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7010,
        name: "UNSUPPORTED_MINOR",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Unsupported protocol minor",
        diagnostic_linkage: "required",
        grpc_status: "UNIMPLEMENTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7020,
        name: "BUNDLE_DIGEST_MISMATCH",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Contract bundle mismatch",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 2000,
        name: "WORKSPACE_NOT_AUTHORIZED",
        owning_layer: "authorization",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Workspace is not authorized",
        diagnostic_linkage: "required",
        grpc_status: "PERMISSION_DENIED",
        mcp_mapping: "authorization-error",
    },
    PublicErrorEntry {
        code: 2010,
        name: "PATH_OUTSIDE_AUTHORIZED_ROOT",
        owning_layer: "authorization",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Path is outside the authorized root",
        diagnostic_linkage: "required",
        grpc_status: "PERMISSION_DENIED",
        mcp_mapping: "authorization-error",
    },
    PublicErrorEntry {
        code: 2020,
        name: "SOURCE_ACCESS_DENIED",
        owning_layer: "authorization",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Source access denied",
        diagnostic_linkage: "required",
        grpc_status: "PERMISSION_DENIED",
        mcp_mapping: "authorization-error",
    },
    PublicErrorEntry {
        code: 2030,
        name: "BLOCKED_PATH_COLLISION",
        owning_layer: "identity",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "WORKSPACE",
        public_message_template: "Workspace paths collide under the registered comparison profile",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3000,
        name: "FRESHNESS_DEADLINE_EXCEEDED",
        owning_layer: "freshness",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Freshness deadline exceeded",
        diagnostic_linkage: "required",
        grpc_status: "DEADLINE_EXCEEDED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3010,
        name: "CAPABILITY_UNAVAILABLE",
        owning_layer: "capability",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Required capability unavailable",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3020,
        name: "NEGATIVE_PROOF_INDETERMINATE",
        owning_layer: "completeness",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Negative proof is indeterminate",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3030,
        name: "SOURCE_SNAPSHOT_MISMATCH",
        owning_layer: "context",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "PROVIDER_RUN",
        public_message_template: "Provider output references another source snapshot",
        diagnostic_linkage: "required",
        grpc_status: "ABORTED",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 4000,
        name: "PROVIDER_PROTOCOL_ERROR",
        owning_layer: "provider",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "PROVIDER_RUN",
        public_message_template: "Provider protocol violation",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 4010,
        name: "SANDBOX_UNAVAILABLE",
        owning_layer: "provider",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "PROVIDER_RUN",
        public_message_template: "Required sandbox unavailable",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 4020,
        name: "RUFF_SEMANTIC_UNAVAILABLE_PARSE",
        owning_layer: "provider",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "PROVIDER_RUN",
        public_message_template: "Ruff semantic facts are unavailable because the source is invalid",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 4030,
        name: "RUFF_SEMANTIC_CLEANUP_FAILED",
        owning_layer: "provider",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "PROVIDER_RUN",
        public_message_template: "Ruff semantic cleanup failed",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 4040,
        name: "SEMANTIC_LANE_FAILED",
        owning_layer: "provider",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "WORKSPACE",
        public_message_template: "Semantic lane execution failed",
        diagnostic_linkage: "required",
        grpc_status: "UNAVAILABLE",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 5000,
        name: "QUERY_HARD_LIMIT_EXCEEDED",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Query exceeds a hard limit",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 1000,
        name: "ENTITY_AMBIGUOUS",
        owning_layer: "binding",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Entity reference is ambiguous",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1010,
        name: "SEMANTIC_PHRASE_AMBIGUOUS",
        owning_layer: "controlled-language",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Semantic phrase is ambiguous",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1020,
        name: "SEMANTIC_PHRASE_UNRECOGNIZED",
        owning_layer: "controlled-language",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Semantic phrase is not recognized",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 6000,
        name: "CURRENT_POINTER_CONFLICT",
        owning_layer: "publication",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "WORKSPACE",
        public_message_template: "Active snapshot pointer changed",
        diagnostic_linkage: "required",
        grpc_status: "ABORTED",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 9000,
        name: "ID_COLLISION",
        owning_layer: "identity",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Canonical identity collision detected",
        diagnostic_linkage: "required",
        grpc_status: "DATA_LOSS",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 6010,
        name: "OVERLAY_GENERATION_CONFLICT",
        owning_layer: "storage",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "WORKSPACE",
        public_message_template: "Overlay generation conflicts with existing data",
        diagnostic_linkage: "required",
        grpc_status: "ABORTED",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 7030,
        name: "CREDENTIAL_REPLAY_DETECTED",
        owning_layer: "credential",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Credential replay detected",
        diagnostic_linkage: "required",
        grpc_status: "UNAUTHENTICATED",
        mcp_mapping: "authorization-error",
    },
    PublicErrorEntry {
        code: 7040,
        name: "IDEMPOTENCY_CONFLICT",
        owning_layer: "transport",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Idempotency key conflicts with prior request",
        diagnostic_linkage: "required",
        grpc_status: "ALREADY_EXISTS",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7050,
        name: "RESUME_WINDOW_EXPIRED",
        owning_layer: "transport",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Resume window expired",
        diagnostic_linkage: "required",
        grpc_status: "OUT_OF_RANGE",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 8000,
        name: "RESULT_TOO_LARGE_FOR_HOST",
        owning_layer: "delivery",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Result is too large for inline delivery",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 8010,
        name: "ARTIFACT_ID_COLLISION",
        owning_layer: "artifact",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Artifact identity collision detected",
        diagnostic_linkage: "required",
        grpc_status: "DATA_LOSS",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 8020,
        name: "RESOURCE_EXPIRED",
        owning_layer: "artifact",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Resource has expired",
        diagnostic_linkage: "required",
        grpc_status: "NOT_FOUND",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6020,
        name: "STATE_TRANSITION_VIOLATION",
        owning_layer: "lifecycle",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "WORKSPACE",
        public_message_template: "Illegal lifecycle state transition",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 9010,
        name: "INTERNAL_INVARIANT_VIOLATION",
        owning_layer: "internal",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Internal invariant violated",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 1030,
        name: "INVALID_REQUEST_SCHEMA",
        owning_layer: "request",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Request does not match its schema",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1040,
        name: "CONTEXT_NOT_INDEXED",
        owning_layer: "binding",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Requested context is not indexed",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 1050,
        name: "COMPOSITE_SNAPSHOT_UNSUPPORTED",
        owning_layer: "request",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Composite snapshot operation is unsupported",
        diagnostic_linkage: "required",
        grpc_status: "UNIMPLEMENTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1060,
        name: "RESOURCE_LIMIT_REJECTED",
        owning_layer: "request",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Requested resource limit is rejected",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1070,
        name: "CANCELLED",
        owning_layer: "execution",
        severity: "INFO",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Request was cancelled",
        diagnostic_linkage: "required",
        grpc_status: "CANCELLED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 1080,
        name: "ADAPTER_INPUT_NOT_JSON",
        owning_layer: "adapter",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Adapter input must be JSON-compatible",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1090,
        name: "ADAPTER_INPUT_LIMIT",
        owning_layer: "adapter",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Adapter input exceeds a limit",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1100,
        name: "ADAPTER_INPUT_VALIDATION",
        owning_layer: "adapter",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Adapter input validation failed",
        diagnostic_linkage: "required",
        grpc_status: "INVALID_ARGUMENT",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1110,
        name: "ADAPTER_OUTPUT_CONTRACT",
        owning_layer: "adapter",
        severity: "FATAL",
        retryability: "TRANSIENT",
        scope: "DAEMON",
        public_message_template: "Adapter output contract failed",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 1120,
        name: "UNSUPPORTED_BINARY",
        owning_layer: "source",
        severity: "WARNING",
        retryability: "AFTER_RECONFIGURATION",
        scope: "WORKSPACE",
        public_message_template: "Binary source is unsupported",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 1130,
        name: "UNSUPPORTED_CONTENT",
        owning_layer: "source",
        severity: "WARNING",
        retryability: "AFTER_RECONFIGURATION",
        scope: "WORKSPACE",
        public_message_template: "Source content is unsupported",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3040,
        name: "CURRENT_FACTS_UNAVAILABLE",
        owning_layer: "freshness",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Current facts are not yet available",
        diagnostic_linkage: "required",
        grpc_status: "UNAVAILABLE",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3050,
        name: "DEFAULT_CONTEXT_UNAVAILABLE",
        owning_layer: "context",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "No default context is available",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 3060,
        name: "WORKSPACE_BOOTSTRAPPING",
        owning_layer: "freshness",
        severity: "INFO",
        retryability: "NEW_SNAPSHOT",
        scope: "WORKSPACE",
        public_message_template: "Workspace is building its first snapshot",
        diagnostic_linkage: "required",
        grpc_status: "UNAVAILABLE",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3070,
        name: "FACT_FAMILY_UNAVAILABLE",
        owning_layer: "capability",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Fact family is unavailable",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 3080,
        name: "SNAPSHOT_FRESHNESS_MISMATCH",
        owning_layer: "freshness",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Snapshot freshness domains differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 5030,
        name: "QUERY_LOST_DAEMON_RESTART",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Query was lost during daemon restart",
        diagnostic_linkage: "required",
        grpc_status: "ABORTED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6030,
        name: "COMPARISON_DOMAIN_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Comparison domains differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 6040,
        name: "SCHEMA_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Compared schemas differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6050,
        name: "TABLE_SET_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Compared table sets differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6060,
        name: "ROW_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Compared rows differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6070,
        name: "CAPABILITY_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Compared capability evidence differs",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6080,
        name: "SNAPSHOT_METADATA_MISMATCH",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Compared snapshot metadata differs",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 6090,
        name: "ID_COLLISION_DETECTED",
        owning_layer: "comparison",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Comparison detected an identity collision",
        diagnostic_linkage: "required",
        grpc_status: "DATA_LOSS",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 6100,
        name: "COMPARATOR_ERROR",
        owning_layer: "comparison",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "QUERY_BLOCK",
        public_message_template: "Snapshot comparison failed",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 6110,
        name: "PUBLICATION_REFERENTIAL_INTEGRITY",
        owning_layer: "publication",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "WORKSPACE",
        public_message_template: "Candidate publication contains an unresolved reference",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 7060,
        name: "REQUIRED_FEATURE_UNSUPPORTED",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Required feature is unsupported",
        diagnostic_linkage: "required",
        grpc_status: "UNIMPLEMENTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7070,
        name: "SCHEMA_DIGEST_MISMATCH",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Schema digest mismatch",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7080,
        name: "TOOLCHAIN_MISMATCH",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Toolchain identity mismatch",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 7100,
        name: "PLATFORM_UNSUPPORTED",
        owning_layer: "transport",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "DAEMON",
        public_message_template: "Platform is unsupported",
        diagnostic_linkage: "required",
        grpc_status: "UNIMPLEMENTED",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 7110,
        name: "DAEMON_UNAVAILABLE",
        owning_layer: "transport",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "DAEMON",
        public_message_template: "Local daemon is unavailable",
        diagnostic_linkage: "required",
        grpc_status: "UNAVAILABLE",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 7120,
        name: "CONTRACT_MISMATCH",
        owning_layer: "compatibility",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "DAEMON",
        public_message_template: "Adapter and daemon contracts differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 9020,
        name: "INTERNAL",
        owning_layer: "internal",
        severity: "FATAL",
        retryability: "TRANSIENT",
        scope: "DAEMON",
        public_message_template: "Internal failure",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 1140,
        name: "GOVERNED_PLAN_INGRESS_REJECTED",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "QUERY_BLOCK",
        public_message_template: "Governed plan ingress rejected",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 1150,
        name: "SEMANTIC_PHRASE_UNSUPPORTED",
        owning_layer: "controlled-language",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "REQUEST",
        public_message_template: "Semantic phrase has no supported compiled binding",
        diagnostic_linkage: "required",
        grpc_status: "UNIMPLEMENTED",
        mcp_mapping: "invalid-request",
    },
    PublicErrorEntry {
        code: 4050,
        name: "INVALID_EPOCH_RESOURCE_POLICY",
        owning_layer: "configuration",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "WORKSPACE",
        public_message_template: "Epoch resource policy is invalid",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 4060,
        name: "INVALID_EPOCH_WORK_CLASS_POLICY",
        owning_layer: "configuration",
        severity: "ERROR",
        retryability: "AFTER_RECONFIGURATION",
        scope: "WORKSPACE",
        public_message_template: "Epoch work-class policy is invalid",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 6120,
        name: "EPOCH_RESOURCE_PIN_MISMATCH",
        owning_layer: "context",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Epoch resource pin does not match the query epoch",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 5110,
        name: "EPOCH_RESOURCE_BACKPRESSURE",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "QUERY_BLOCK",
        public_message_template: "Epoch resource admission is backpressured",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 5120,
        name: "EPOCH_RESOURCE_DEADLINE_EXCEEDED",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "QUERY_BLOCK",
        public_message_template: "Epoch resource admission deadline was exceeded",
        diagnostic_linkage: "required",
        grpc_status: "DEADLINE_EXCEEDED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 5130,
        name: "EPOCH_RESULT_LEASE_BACKPRESSURE",
        owning_layer: "execution",
        severity: "ERROR",
        retryability: "TRANSIENT",
        scope: "QUERY_BLOCK",
        public_message_template: "Epoch result-lease admission is backpressured",
        diagnostic_linkage: "required",
        grpc_status: "RESOURCE_EXHAUSTED",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 9030,
        name: "EPOCH_RESOURCE_COUNTER_OVERFLOW",
        owning_layer: "internal",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Epoch resource accounting overflowed",
        diagnostic_linkage: "required",
        grpc_status: "INTERNAL",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 8030,
        name: "RESULT_ARTIFACT_IDENTITY_COLLISION",
        owning_layer: "artifact",
        severity: "FATAL",
        retryability: "NEVER",
        scope: "DAEMON",
        public_message_template: "Result artifact identity collision detected",
        diagnostic_linkage: "required",
        grpc_status: "DATA_LOSS",
        mcp_mapping: "internal-error",
    },
    PublicErrorEntry {
        code: 8040,
        name: "RESULT_ARTIFACT_UNKNOWN",
        owning_layer: "artifact",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Result artifact is unknown",
        diagnostic_linkage: "required",
        grpc_status: "NOT_FOUND",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 8050,
        name: "RESULT_EPOCH_PIN_MISMATCH",
        owning_layer: "context",
        severity: "ERROR",
        retryability: "NEW_SNAPSHOT",
        scope: "QUERY_BLOCK",
        public_message_template: "Result package and lease epochs differ",
        diagnostic_linkage: "required",
        grpc_status: "FAILED_PRECONDITION",
        mcp_mapping: "execution-error",
    },
    PublicErrorEntry {
        code: 8060,
        name: "RESULT_RESOURCE_UNKNOWN",
        owning_layer: "artifact",
        severity: "ERROR",
        retryability: "NEVER",
        scope: "REQUEST",
        public_message_template: "Result resource is unknown",
        diagnostic_linkage: "required",
        grpc_status: "NOT_FOUND",
        mcp_mapping: "execution-error",
    },
];

pub const CAPABILITY_IDS: &[&str] = &[
    "SOURCE_BYTES",
    "SOURCE_INVENTORY",
    "TOKENS",
    "CST",
    "TYPED_AST",
    "SCOPES_BINDINGS",
    "IMPORT_RESOLUTION",
    "DECLARED_TYPES",
    "COMPUTED_TYPES",
    "MEMBER_RESOLUTION",
    "CALL_TARGETS",
    "RUST_MIR",
    "BORROW_LOANS",
    "CFG",
    "DOMINANCE",
    "CONTROL_DEPENDENCE",
    "DEF_USE",
    "LIVENESS",
    "POINTS_TO_ALIAS",
    "EFFECTS",
    "CONCURRENCY",
    "CALLABLE_SUMMARIES",
];

pub const CAPABILITY_CODES: &[u16] = &[
    10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 210,
    220,
];
