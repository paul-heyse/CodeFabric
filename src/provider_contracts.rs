//! Application-owned contracts at every provider boundary.
//!
//! Provider implementations consume immutable jobs and return owned Arrow batches plus explicit
//! coverage, gaps, diagnostics, provenance, trust, resource, and terminal outcomes. Concrete
//! provider libraries, generated transport messages, DataFusion, Delta, and daemon state do not
//! cross this boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::resource_budget::{
    ResourceBudget, ResourceBudgetError, ResourceOwner, ResourceScopeKind,
};

mod inputs;
pub use inputs::*;
pub(crate) mod allocation;

const MAX_IDENTITY_BYTES: usize = 512;
const MAX_DIAGNOSTIC_BYTES: usize = 8 * 1024;
const MAX_REQUESTED_FAMILIES: usize = 4_096;
const MAX_RELATIONS: usize = 4_096;
const MAX_BATCHES_PER_RELATION: usize = 65_536;
const MAX_ROWS: u64 = 1_000_000_000;
const MAX_BYTES: u64 = 1 << 40;
const MAX_DIAGNOSTICS: usize = 65_536;
const MAX_WORK_UNITS_BETWEEN_POLLS: usize = 4_096;
const MAX_INPUT_BYTES: u64 = 1 << 34;
const MAX_WORK_UNITS: u64 = 1_000_000_000;
const MAX_WALL_MILLIS: u64 = 3_600_000;
const MAX_VISITED_NODES: u64 = 1_000_000_000;
const MAX_TRAVERSAL_DEPTH: u16 = 16_384;
const MAX_WORKERS: u16 = 4_096;
const MAX_RETAINED_REVISIONS: u16 = 4_096;
const MAX_CANCELLATION_ACK_MILLIS: u64 = 60_000;

/// Arrow metadata key for a relation's application-owned semantic role.
pub const RELATION_SEMANTIC_ROLE_METADATA_KEY: &str = "codefabric.semantic_relation_role";
/// Arrow metadata key for a field's application-owned semantic role.
pub const SEMANTIC_ROLE_METADATA_KEY: &str = "codefabric.semantic_role";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct BoundedIdentity(Arc<str>);

impl BoundedIdentity {
    fn try_new(
        value: impl Into<Arc<str>>,
        kind: &'static str,
    ) -> Result<Self, ProviderContractError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_IDENTITY_BYTES
            || value.trim() != value.as_ref()
            || value.chars().any(char::is_control)
        {
            return Err(ProviderContractError::InvalidIdentity { kind });
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! categorical_identity {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Categorical ", $kind, "; it cannot be substituted for another identity class.")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(BoundedIdentity);

        impl $name {
            /// Construct a bounded, non-empty categorical identity.
            ///
            /// # Errors
            ///
            /// Rejects empty, surrounding-whitespace, control-character, and oversized values.
            pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, ProviderContractError> {
                BoundedIdentity::try_new(value, $kind).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

categorical_identity!(SuiteIdentity, "suite identity");
categorical_identity!(ProviderIdentity, "provider identity");
categorical_identity!(ProviderProtocolIdentity, "provider protocol identity");
categorical_identity!(ProviderSchemaIdentity, "provider schema identity");
categorical_identity!(SourceIdentity, "source identity");
categorical_identity!(ContextIdentity, "context identity");
categorical_identity!(ProviderRunIdentity, "provider run identity");
categorical_identity!(ProviderRelationIdentity, "provider relation identity");
categorical_identity!(ProviderFamilyIdentity, "provider family identity");
categorical_identity!(ProviderScopeIdentity, "provider scope identity");
categorical_identity!(ProviderBuildIdentity, "provider build identity");
categorical_identity!(ProviderPolicyIdentity, "provider policy identity");
categorical_identity!(ProviderProgramIdentity, "provider program identity");
categorical_identity!(RustToolchainIdentity, "Rust toolchain identity");
categorical_identity!(
    RustCompilationUnitIdentity,
    "Rust compilation-unit identity"
);
categorical_identity!(RustOwnerIdentity, "Rust owner identity");
categorical_identity!(CanonicalEntityIdentity, "canonical entity identity");
categorical_identity!(DiagnosticCode, "diagnostic code");

/// Exact immutable source pins carried by a provider job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSourceBinding {
    identity: SourceIdentity,
    workspace_id: [u8; 16],
    generation: u64,
    selection: ProviderSourceSelection,
}

impl ProviderSourceBinding {
    /// Construct a source binding whose binary pins can be repeated in Arrow rows.
    ///
    /// # Errors
    ///
    /// Rejects zero file or content identities.
    pub fn try_file(
        identity: SourceIdentity,
        workspace_id: [u8; 16],
        file_id: [u8; 16],
        generation: u64,
        content_digest: [u8; 32],
    ) -> Result<Self, ProviderContractError> {
        require_nonzero_bytes(&workspace_id)?;
        require_nonzero_bytes(&file_id)?;
        require_nonzero_bytes(&content_digest)?;
        Ok(Self {
            identity,
            workspace_id,
            generation,
            selection: ProviderSourceSelection::File {
                file_id,
                content_digest,
            },
        })
    }

    /// Bind a context-wide job to the complete captured inventory, never a dirty subset.
    #[must_use]
    pub fn from_inventory(
        identity: SourceIdentity,
        inventory: crate::resource_budget::ChargedValue<ProviderSourceInventory>,
    ) -> Self {
        Self {
            identity,
            workspace_id: inventory.workspace_id(),
            generation: inventory.source_generation(),
            selection: ProviderSourceSelection::Inventory(inventory),
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }

    #[must_use]
    pub const fn file_id(&self) -> Option<[u8; 16]> {
        match &self.selection {
            ProviderSourceSelection::File { file_id, .. } => Some(*file_id),
            ProviderSourceSelection::Inventory(_) => None,
        }
    }

    #[must_use]
    pub const fn selection(&self) -> &ProviderSourceSelection {
        &self.selection
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn content_digest(&self) -> [u8; 32] {
        match &self.selection {
            ProviderSourceSelection::File { content_digest, .. } => *content_digest,
            ProviderSourceSelection::Inventory(inventory) => inventory.identity(),
        }
    }
}

/// Exact analysis and semantic-environment pins carried by a provider job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderContextBinding {
    identity: ContextIdentity,
    analysis_context_id: [u8; 16],
    context_fingerprint: [u8; 32],
    semantic_environment_id: [u8; 32],
    support_obligations: Arc<[ProviderSupportDependency]>,
    modules: Arc<[ProviderModuleBinding]>,
    python_version: Option<(u16, u16)>,
    effective_input_identity: [u8; 32],
}

impl ProviderContextBinding {
    /// Construct a context binding whose binary pins can be repeated in Arrow rows.
    ///
    /// # Errors
    ///
    /// Rejects zero analysis-context or semantic-environment identities.
    pub fn try_new(
        identity: ContextIdentity,
        analysis_context_id: [u8; 16],
        context_fingerprint: [u8; 32],
        semantic_environment_id: [u8; 32],
    ) -> Result<Self, ProviderContractError> {
        require_nonzero_bytes(&analysis_context_id)?;
        require_nonzero_bytes(&context_fingerprint)?;
        require_nonzero_bytes(&semantic_environment_id)?;
        let mut context = Self {
            identity,
            analysis_context_id,
            context_fingerprint,
            semantic_environment_id,
            support_obligations: Arc::from([]),
            modules: Arc::from([]),
            python_version: None,
            effective_input_identity: [0; 32],
        };
        context.refresh_effective_input_identity()?;
        Ok(context)
    }

    #[must_use]
    pub const fn identity(&self) -> &ContextIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn analysis_context_id(&self) -> [u8; 16] {
        self.analysis_context_id
    }

    #[must_use]
    pub const fn context_fingerprint(&self) -> [u8; 32] {
        self.context_fingerprint
    }

    /// Content identity of the exact selected invocation inputs, not a canonical context ID.
    #[must_use]
    pub const fn effective_input_identity(&self) -> [u8; 32] {
        self.effective_input_identity
    }

    fn refresh_effective_input_identity(&mut self) -> Result<(), ProviderContractError> {
        self.effective_input_identity = inputs::effective_context_input_identity(self)?;
        Ok(())
    }

    /// Install the actual selected configuration and search observations as required support.
    ///
    /// # Errors
    /// Rejects malformed or duplicate observations, including absence without a closed search.
    pub fn with_support_obligations(
        mut self,
        mut dependencies: Vec<ProviderSupportDependency>,
    ) -> Result<Self, ProviderContractError> {
        inputs::validate_dependencies(&dependencies)?;
        if dependencies
            .iter()
            .any(|dependency| dependency.scope.effective_context != self.context_fingerprint)
        {
            return Err(ProviderContractError::SupportMismatch);
        }
        dependencies.sort();
        self.support_obligations = dependencies.into();
        self.refresh_effective_input_identity()?;
        Ok(self)
    }

    #[must_use]
    pub fn support_obligations(&self) -> &[ProviderSupportDependency] {
        &self.support_obligations
    }

    /// Retained context containers and nested allocation capacities; native state is separate.
    ///
    /// # Errors
    /// Rejects arithmetic overflow or a metadata envelope violation.
    pub fn memory_bytes(&self) -> Result<u64, ProviderContractError> {
        inputs::context_memory_bytes(self)
    }

    /// Bind the selected module map without inventing names from file identities.
    ///
    /// # Errors
    /// Rejects duplicate files and malformed module/path identities.
    pub fn with_modules(
        mut self,
        mut modules: Vec<ProviderModuleBinding>,
    ) -> Result<Self, ProviderContractError> {
        inputs::validate_modules(&modules)?;
        modules.sort_by_key(|module| module.file_id);
        self.modules = modules.into();
        self.refresh_effective_input_identity()?;
        Ok(self)
    }

    #[must_use]
    pub fn module_for_file(&self, file_id: [u8; 16]) -> Option<&ProviderModuleBinding> {
        self.modules
            .binary_search_by_key(&file_id, |module| module.file_id)
            .ok()
            .map(|index| &self.modules[index])
    }

    /// Select the real language version consumed by Ruff. Unsupported versions fail closed.
    ///
    /// # Errors
    /// Rejects versions outside the pinned provider's supported Python 3.7-3.15 range.
    pub fn with_python_version(
        mut self,
        major: u16,
        minor: u16,
    ) -> Result<Self, ProviderContractError> {
        if major != 3 || !(7..=15).contains(&minor) {
            return Err(ProviderContractError::SupportMismatch);
        }
        self.python_version = Some((major, minor));
        self.refresh_effective_input_identity()?;
        Ok(self)
    }

    #[must_use]
    pub const fn python_version(&self) -> Option<(u16, u16)> {
        self.python_version
    }

    #[must_use]
    pub const fn semantic_environment_id(&self) -> [u8; 32] {
        self.semantic_environment_id
    }
}

/// Exact run identity and binary row pin carried by a provider job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRunBinding {
    identity: ProviderRunIdentity,
    provider_run_id: [u8; 16],
}

impl ProviderRunBinding {
    /// Construct a run binding.
    ///
    /// # Errors
    ///
    /// Rejects a zero binary run identity.
    pub fn try_new(
        identity: ProviderRunIdentity,
        provider_run_id: [u8; 16],
    ) -> Result<Self, ProviderContractError> {
        require_nonzero_bytes(&provider_run_id)?;
        Ok(Self {
            identity,
            provider_run_id,
        })
    }

    #[must_use]
    pub const fn identity(&self) -> &ProviderRunIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn provider_run_id(&self) -> [u8; 16] {
        self.provider_run_id
    }
}

fn require_nonzero_bytes(bytes: &[u8]) -> Result<(), ProviderContractError> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(ProviderContractError::ZeroInvocationPin)
    } else {
        Ok(())
    }
}

/// Exact provider lane selected by a release-prepared job.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderLane {
    TreeSitter,
    TreeSitterRust,
    Ruff,
    Pyrefly,
    Rustc,
}

impl ProviderLane {
    pub const ALL: [Self; 5] = [
        Self::TreeSitter,
        Self::TreeSitterRust,
        Self::Ruff,
        Self::Pyrefly,
        Self::Rustc,
    ];
}

/// Trust posture resolved before provider execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTrustPosture {
    InProcessConstrained,
    LocalSidecarConstrained,
    CompilerSubprocessConstrained,
}

/// Effective per-run ceilings after release policy and operational budgets are reduced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderResourceCeilings {
    relations: NonZeroUsize,
    batches_per_relation: NonZeroUsize,
    input_bytes: NonZeroU64,
    rows: NonZeroU64,
    bytes: NonZeroU64,
    diagnostics: NonZeroUsize,
    work_units: NonZeroU64,
    wall_millis: NonZeroU64,
    visited_nodes: NonZeroU64,
    traversal_depth: u16,
    workers: u16,
    retained_revisions: u16,
    cancellation_poll_work_units: NonZeroUsize,
    cancellation_ack_millis: NonZeroU64,
}

/// Arguments for one complete provider resource envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderResourceCeilingSpec {
    pub max_relations: usize,
    pub max_batches_per_relation: usize,
    pub max_input_bytes: u64,
    pub max_rows: u64,
    pub max_bytes: u64,
    pub max_diagnostics: usize,
    pub max_work_units: u64,
    pub max_wall_millis: u64,
    pub max_visited_nodes: u64,
    pub max_traversal_depth: u16,
    pub max_workers: u16,
    pub max_retained_revisions: u16,
    pub cancellation_poll_work_units: usize,
    pub cancellation_ack_millis: u64,
}

impl ProviderResourceCeilings {
    /// Construct bounded effective ceilings.
    ///
    /// # Errors
    ///
    /// Rejects zero values and values wider than the application hard limits.
    pub fn try_new(spec: ProviderResourceCeilingSpec) -> Result<Self, ProviderContractError> {
        let value = Self {
            relations: NonZeroUsize::new(spec.max_relations)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            batches_per_relation: NonZeroUsize::new(spec.max_batches_per_relation)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            input_bytes: NonZeroU64::new(spec.max_input_bytes)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            rows: NonZeroU64::new(spec.max_rows)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            bytes: NonZeroU64::new(spec.max_bytes)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            diagnostics: NonZeroUsize::new(spec.max_diagnostics)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            work_units: NonZeroU64::new(spec.max_work_units)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            wall_millis: NonZeroU64::new(spec.max_wall_millis)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            visited_nodes: NonZeroU64::new(spec.max_visited_nodes)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            traversal_depth: spec.max_traversal_depth,
            workers: spec.max_workers,
            retained_revisions: spec.max_retained_revisions,
            cancellation_poll_work_units: NonZeroUsize::new(spec.cancellation_poll_work_units)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
            cancellation_ack_millis: NonZeroU64::new(spec.cancellation_ack_millis)
                .ok_or(ProviderContractError::InvalidResourceCeiling)?,
        };
        if value.relations.get() > MAX_RELATIONS
            || value.batches_per_relation.get() > MAX_BATCHES_PER_RELATION
            || value.input_bytes.get() > MAX_INPUT_BYTES
            || value.rows.get() > MAX_ROWS
            || value.bytes.get() > MAX_BYTES
            || value.diagnostics.get() > MAX_DIAGNOSTICS
            || value.work_units.get() > MAX_WORK_UNITS
            || value.wall_millis.get() > MAX_WALL_MILLIS
            || value.visited_nodes.get() > MAX_VISITED_NODES
            || value.traversal_depth == 0
            || value.traversal_depth > MAX_TRAVERSAL_DEPTH
            || value.workers == 0
            || value.workers > MAX_WORKERS
            || value.retained_revisions == 0
            || value.retained_revisions > MAX_RETAINED_REVISIONS
            || value.cancellation_poll_work_units.get() > MAX_WORK_UNITS_BETWEEN_POLLS
            || value.cancellation_ack_millis.get() > MAX_CANCELLATION_ACK_MILLIS
        {
            return Err(ProviderContractError::InvalidResourceCeiling);
        }
        Ok(value)
    }

    /// Intersect compiled and operational envelopes without widening either one.
    ///
    /// # Errors
    ///
    /// Returns an error only if the resulting envelope violates an application hard bound.
    pub fn intersect(self, other: Self) -> Result<Self, ProviderContractError> {
        Self::try_new(ProviderResourceCeilingSpec {
            max_relations: self.max_relations().min(other.max_relations()),
            max_batches_per_relation: self
                .max_batches_per_relation()
                .min(other.max_batches_per_relation()),
            max_input_bytes: self.max_input_bytes().min(other.max_input_bytes()),
            max_rows: self.max_rows().min(other.max_rows()),
            max_bytes: self.max_bytes().min(other.max_bytes()),
            max_diagnostics: self.max_diagnostics().min(other.max_diagnostics()),
            max_work_units: self.max_work_units().min(other.max_work_units()),
            max_wall_millis: self.max_wall_millis().min(other.max_wall_millis()),
            max_visited_nodes: self.max_visited_nodes().min(other.max_visited_nodes()),
            max_traversal_depth: self.max_traversal_depth().min(other.max_traversal_depth()),
            max_workers: self.max_workers().min(other.max_workers()),
            max_retained_revisions: self
                .max_retained_revisions()
                .min(other.max_retained_revisions()),
            cancellation_poll_work_units: self
                .cancellation_poll_work_units()
                .min(other.cancellation_poll_work_units()),
            cancellation_ack_millis: self
                .cancellation_ack_millis()
                .min(other.cancellation_ack_millis()),
        })
    }

    #[must_use]
    pub const fn max_relations(self) -> usize {
        self.relations.get()
    }

    #[must_use]
    pub const fn max_batches_per_relation(self) -> usize {
        self.batches_per_relation.get()
    }

    #[must_use]
    pub const fn max_input_bytes(self) -> u64 {
        self.input_bytes.get()
    }

    #[must_use]
    pub const fn max_rows(self) -> u64 {
        self.rows.get()
    }

    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        self.bytes.get()
    }

    #[must_use]
    pub const fn max_diagnostics(self) -> usize {
        self.diagnostics.get()
    }

    #[must_use]
    pub const fn max_work_units(self) -> u64 {
        self.work_units.get()
    }

    #[must_use]
    pub const fn max_wall_millis(self) -> u64 {
        self.wall_millis.get()
    }

    #[must_use]
    pub const fn max_visited_nodes(self) -> u64 {
        self.visited_nodes.get()
    }

    #[must_use]
    pub const fn max_traversal_depth(self) -> u16 {
        self.traversal_depth
    }

    #[must_use]
    pub const fn max_workers(self) -> u16 {
        self.workers
    }

    #[must_use]
    pub const fn max_retained_revisions(self) -> u16 {
        self.retained_revisions
    }

    #[must_use]
    pub const fn cancellation_poll_work_units(self) -> usize {
        self.cancellation_poll_work_units.get()
    }

    #[must_use]
    pub const fn cancellation_ack_millis(self) -> u64 {
        self.cancellation_ack_millis.get()
    }
}

/// Owner-side cancellation handle. It is not carried by a provider job.
#[derive(Clone, Debug)]
pub struct CancellationHandle(Cancellation);

impl CancellationHandle {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

/// Bounded synchronous view of an outward-owned cancellation source.
#[derive(Clone, Debug)]
pub struct CancellationProbe {
    cancellation: Cancellation,
    max_work_units_between_polls: NonZeroUsize,
}

impl CancellationProbe {
    /// Create the owner handle and provider-facing probe.
    ///
    /// # Errors
    ///
    /// Rejects a zero or application-unbounded polling interval.
    pub fn pair(
        max_work_units_between_polls: usize,
    ) -> Result<(CancellationHandle, Self), ProviderContractError> {
        let interval = NonZeroUsize::new(max_work_units_between_polls)
            .filter(|value| value.get() <= MAX_WORK_UNITS_BETWEEN_POLLS)
            .ok_or(ProviderContractError::InvalidCancellationProbe)?;
        let cancellation = Cancellation::with_check_interval(
            u32::try_from(max_work_units_between_polls)
                .map_err(|_| ProviderContractError::InvalidCancellationProbe)?,
        );
        Ok((
            CancellationHandle(cancellation.clone()),
            Self {
                cancellation,
                max_work_units_between_polls: interval,
            },
        ))
    }

    /// Bind the provider probe to the caller's existing cancellation authority.
    ///
    /// # Errors
    /// Rejects a zero or application-unbounded polling interval.
    pub fn from_cancellation(
        cancellation: Cancellation,
        max_work_units_between_polls: usize,
    ) -> Result<Self, ProviderContractError> {
        let interval = NonZeroUsize::new(max_work_units_between_polls)
            .filter(|value| value.get() <= MAX_WORK_UNITS_BETWEEN_POLLS)
            .ok_or(ProviderContractError::InvalidCancellationProbe)?;
        Ok(Self {
            cancellation,
            max_work_units_between_polls: interval,
        })
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    #[must_use]
    pub const fn max_work_units_between_polls(&self) -> usize {
        self.max_work_units_between_polls.get()
    }

    pub(crate) fn restricted_to(&self, maximum: usize) -> Result<Self, ProviderContractError> {
        let interval = self.max_work_units_between_polls().min(maximum);
        Ok(Self {
            cancellation: self.cancellation.clone(),
            max_work_units_between_polls: NonZeroUsize::new(interval)
                .ok_or(ProviderContractError::InvalidCancellationProbe)?,
        })
    }
}

/// One exact requested provider family, output relation, schema, and scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFamilyRequest {
    family: ProviderFamilyIdentity,
    relation: ProviderRelationIdentity,
    schema_identity: ProviderSchemaIdentity,
    schema: SchemaRef,
    scope: ProviderScopeIdentity,
    requested_units: u64,
}

impl ProviderFamilyRequest {
    /// Construct a schema-bearing family request. A zero unit census is lawful only when
    /// the containing job binds a proved empty selected inventory.
    ///
    /// # Errors
    ///
    /// Rejects an empty Arrow schema; job construction validates zero-unit scope.
    pub fn try_new(
        family: ProviderFamilyIdentity,
        relation: ProviderRelationIdentity,
        schema_identity: ProviderSchemaIdentity,
        schema: SchemaRef,
        scope: ProviderScopeIdentity,
        requested_units: u64,
    ) -> Result<Self, ProviderContractError> {
        if schema.fields().is_empty() {
            return Err(ProviderContractError::ArrowSchemaMismatch);
        }
        Ok(Self {
            family,
            relation,
            schema_identity,
            schema,
            scope,
            requested_units,
        })
    }

    #[must_use]
    pub const fn requested_units(&self) -> u64 {
        self.requested_units
    }

    #[must_use]
    pub const fn family(&self) -> &ProviderFamilyIdentity {
        &self.family
    }

    #[must_use]
    pub const fn relation(&self) -> &ProviderRelationIdentity {
        &self.relation
    }

    #[must_use]
    pub const fn schema_identity(&self) -> &ProviderSchemaIdentity {
        &self.schema_identity
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn scope(&self) -> &ProviderScopeIdentity {
        &self.scope
    }
}

/// Release and provider-build identities recorded on one run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRunProvenance {
    provider_build: ProviderBuildIdentity,
    policy: ProviderPolicyIdentity,
    program: ProviderProgramIdentity,
}

impl ProviderRunProvenance {
    #[must_use]
    pub const fn new(
        provider_build: ProviderBuildIdentity,
        policy: ProviderPolicyIdentity,
        program: ProviderProgramIdentity,
    ) -> Self {
        Self {
            provider_build,
            policy,
            program,
        }
    }

    #[must_use]
    pub const fn provider_build(&self) -> &ProviderBuildIdentity {
        &self.provider_build
    }

    #[must_use]
    pub const fn policy(&self) -> &ProviderPolicyIdentity {
        &self.policy
    }

    #[must_use]
    pub const fn program(&self) -> &ProviderProgramIdentity {
        &self.program
    }
}

/// Immutable, release-prepared provider execution job.
#[derive(Clone, Debug)]
pub struct ProviderJob {
    suite: SuiteIdentity,
    provider: ProviderIdentity,
    protocol: ProviderProtocolIdentity,
    source: ProviderSourceBinding,
    context: crate::resource_budget::ChargedValue<ProviderContextBinding>,
    run: ProviderRunBinding,
    lane: ProviderLane,
    trust: ProviderTrustPosture,
    requests: crate::resource_budget::ChargedSlice<ProviderFamilyRequest>,
    ceilings: ProviderResourceCeilings,
    resource_budget: ResourceBudget,
    deadline: Instant,
    cancellation: CancellationProbe,
    provenance: ProviderRunProvenance,
    producer_release_identity: [u8; 32],
}

/// Arguments kept together so job construction has one validation boundary.
#[derive(Clone, Debug)]
pub struct ProviderJobSpec {
    pub suite: SuiteIdentity,
    pub provider: ProviderIdentity,
    pub protocol: ProviderProtocolIdentity,
    pub source: ProviderSourceBinding,
    pub context: crate::resource_budget::ChargedValue<ProviderContextBinding>,
    pub run: ProviderRunBinding,
    pub lane: ProviderLane,
    pub trust: ProviderTrustPosture,
    pub requests: Vec<ProviderFamilyRequest>,
    pub ceilings: ProviderResourceCeilings,
    /// Exact operation scope under the selected workspace's existing aggregate budget.
    pub resource_budget: ResourceBudget,
    pub deadline: Instant,
    pub cancellation: CancellationProbe,
    pub provenance: ProviderRunProvenance,
}

impl ProviderJob {
    /// Validate and construct one immutable job.
    ///
    /// # Errors
    ///
    /// Rejects expired jobs, empty/oversized request sets, duplicate family/relation identities,
    /// and requests that exceed the effective relation ceiling.
    pub fn try_new(spec: ProviderJobSpec) -> Result<Self, ProviderContractError> {
        if spec.resource_budget.owner()
            != (ResourceOwner {
                kind: ResourceScopeKind::Operation,
                id: spec.run.provider_run_id(),
            })
            || spec
                .resource_budget
                .ancestor_owner(ResourceScopeKind::Workspace)
                != Some(ResourceOwner {
                    kind: ResourceScopeKind::Workspace,
                    id: spec.source.workspace_id(),
                })
        {
            return Err(ProviderContractError::ResourceOwnerMismatch);
        }
        allocation::require_native_workspace(
            spec.context.reservation().owner(),
            &spec.resource_budget,
        )?;
        if let ProviderSourceSelection::Inventory(inventory) = spec.source.selection() {
            allocation::require_native_workspace(
                inventory.reservation().owner(),
                &spec.resource_budget,
            )?;
        }
        if spec.deadline <= Instant::now() {
            return Err(ProviderContractError::ExpiredJob);
        }
        if spec.requests.is_empty()
            || spec.requests.len() > MAX_REQUESTED_FAMILIES
            || spec.requests.len() > spec.ceilings.max_relations()
        {
            return Err(ProviderContractError::EmptyOrOversizedRequestSet);
        }
        let mut families = BTreeSet::new();
        let mut relations = BTreeSet::new();
        for request in &spec.requests {
            if request.requested_units == 0
                && !matches!(spec.source.selection(), ProviderSourceSelection::Inventory(inventory) if inventory.selected_files().next().is_none())
            {
                return Err(ProviderContractError::EmptyRequest);
            }
            if !families.insert(request.family.clone()) {
                return Err(ProviderContractError::DuplicateFamily);
            }
            if !relations.insert(request.relation.clone()) {
                return Err(ProviderContractError::DuplicateRelation);
            }
        }
        inputs::validate_job_input_bounds(&spec)?;
        let producer_release_identity = spec.provenance.producer_release_identity()?;
        let metadata = spec
            .requests
            .capacity()
            .checked_mul(std::mem::size_of::<ProviderFamilyRequest>())
            .ok_or(ProviderContractError::ResourceOverflow)?;
        let mut allocation =
            allocation::ProviderAllocation::try_new(&spec.resource_budget, metadata as u64)?;
        let requests = allocation.retain_measured_vec(spec.requests, |_| 0)?;
        Ok(Self {
            suite: spec.suite,
            provider: spec.provider,
            protocol: spec.protocol,
            source: spec.source,
            context: spec.context,
            run: spec.run,
            lane: spec.lane,
            trust: spec.trust,
            requests,
            ceilings: spec.ceilings,
            resource_budget: spec.resource_budget,
            deadline: spec.deadline,
            cancellation: spec.cancellation,
            provenance: spec.provenance,
            producer_release_identity,
        })
    }

    #[must_use]
    pub const fn resource_budget(&self) -> &ResourceBudget {
        &self.resource_budget
    }

    #[must_use]
    pub const fn cancellation(&self) -> &CancellationProbe {
        &self.cancellation
    }

    #[must_use]
    pub const fn suite(&self) -> &SuiteIdentity {
        &self.suite
    }

    #[must_use]
    pub const fn provider(&self) -> &ProviderIdentity {
        &self.provider
    }

    #[must_use]
    pub const fn protocol(&self) -> &ProviderProtocolIdentity {
        &self.protocol
    }

    #[must_use]
    pub const fn source(&self) -> &ProviderSourceBinding {
        &self.source
    }

    #[must_use]
    pub fn context(&self) -> &ProviderContextBinding {
        &self.context
    }

    #[must_use]
    pub const fn run(&self) -> &ProviderRunBinding {
        &self.run
    }

    #[must_use]
    pub fn requests(&self) -> &[ProviderFamilyRequest] {
        &self.requests
    }

    #[must_use]
    pub const fn lane(&self) -> ProviderLane {
        self.lane
    }

    #[must_use]
    pub const fn trust(&self) -> ProviderTrustPosture {
        self.trust
    }

    #[must_use]
    pub const fn ceilings(&self) -> ProviderResourceCeilings {
        self.ceilings
    }

    #[must_use]
    pub const fn provenance(&self) -> &ProviderRunProvenance {
        &self.provenance
    }

    #[must_use]
    pub const fn producer_release_identity(&self) -> [u8; 32] {
        self.producer_release_identity
    }

    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }
}

/// Why requested coverage remains unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderUnknownCause {
    MissingOutput,
    Unsupported,
    Timeout,
    Cancelled,
    Corruption,
    Oversized,
    ProviderFailure,
    TrustLoss,
}

/// Why a provider deliberately completed less than the requested scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRemainderReason {
    BudgetExhausted,
    DeadlineReached,
    ProviderDeclaredScope,
}

/// Terminal coverage for one requested family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCoverageState {
    Complete {
        completed_units: u64,
    },
    IntentionalRemainder {
        completed_units: u64,
        reason: ProviderRemainderReason,
    },
    Unknown {
        completed_units: u64,
        cause: ProviderUnknownCause,
    },
}

impl ProviderCoverageState {
    #[must_use]
    pub const fn completed_units(&self) -> u64 {
        match self {
            Self::Complete { completed_units }
            | Self::IntentionalRemainder {
                completed_units, ..
            }
            | Self::Unknown {
                completed_units, ..
            } => *completed_units,
        }
    }
}

/// Coverage observation for one exact requested family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCoverage {
    family: ProviderFamilyIdentity,
    state: ProviderCoverageState,
}

impl ProviderCoverage {
    #[must_use]
    pub const fn new(family: ProviderFamilyIdentity, state: ProviderCoverageState) -> Self {
        Self { family, state }
    }

    #[must_use]
    pub const fn family(&self) -> &ProviderFamilyIdentity {
        &self.family
    }

    #[must_use]
    pub const fn state(&self) -> &ProviderCoverageState {
        &self.state
    }
}

/// Explicit detail for one unknown family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderGap {
    family: ProviderFamilyIdentity,
    cause: ProviderUnknownCause,
    detail: Arc<str>,
}

impl ProviderGap {
    #[must_use]
    pub fn family(&self) -> &ProviderFamilyIdentity {
        &self.family
    }
    #[must_use]
    pub const fn cause(&self) -> ProviderUnknownCause {
        self.cause
    }
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Construct one bounded explicit gap.
    ///
    /// # Errors
    ///
    /// Rejects empty, whitespace-padded, control-character, or oversized detail.
    pub fn try_new(
        family: ProviderFamilyIdentity,
        cause: ProviderUnknownCause,
        detail: impl Into<Arc<str>>,
    ) -> Result<Self, ProviderContractError> {
        let detail = detail.into();
        validate_evidence_detail(&detail)?;
        Ok(Self {
            family,
            cause,
            detail,
        })
    }
}

/// Diagnostic severity without policy judgment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderDiagnosticSeverity {
    Information,
    Warning,
    Error,
}

/// Bounded provider diagnostic retained as evidence, not semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDiagnostic {
    code: DiagnosticCode,
    severity: ProviderDiagnosticSeverity,
    message: Arc<str>,
}

impl ProviderDiagnostic {
    /// Construct one bounded diagnostic.
    ///
    /// # Errors
    ///
    /// Rejects empty or oversized/control-bearing text.
    pub fn try_new(
        code: DiagnosticCode,
        severity: ProviderDiagnosticSeverity,
        message: impl Into<Arc<str>>,
    ) -> Result<Self, ProviderContractError> {
        let message = message.into();
        validate_evidence_detail(&message)?;
        Ok(Self {
            code,
            severity,
            message,
        })
    }
}

/// Owned Arrow batches for one application relation identity.
#[derive(Clone, Debug)]
pub struct ProviderRelationOutput {
    relation: ProviderRelationIdentity,
    schema_identity: ProviderSchemaIdentity,
    schema: SchemaRef,
    batches: crate::resource_budget::ChargedSlice<RecordBatch>,
}

impl ProviderRelationOutput {
    /// Construct a relation whose batches all carry one exact Arrow schema.
    ///
    /// # Errors
    ///
    /// Rejects an empty batch list or a mismatched Arrow schema.
    pub fn try_new(
        relation: ProviderRelationIdentity,
        schema_identity: ProviderSchemaIdentity,
        schema: SchemaRef,
        batches: Vec<RecordBatch>,
        resource_budget: &ResourceBudget,
    ) -> Result<Self, ProviderContractError> {
        if batches.is_empty() {
            return Err(ProviderContractError::EmptyRelationOutput);
        }
        if batches.iter().any(|batch| batch.schema() != schema) {
            return Err(ProviderContractError::ArrowSchemaMismatch);
        }
        let metadata_bytes = batches
            .capacity()
            .saturating_mul(std::mem::size_of::<RecordBatch>())
            .saturating_add(
                schema
                    .fields()
                    .iter()
                    .map(|field| field.size())
                    .sum::<usize>(),
            )
            .saturating_add(
                schema
                    .metadata()
                    .iter()
                    .map(|(key, value)| key.capacity().saturating_add(value.capacity()))
                    .sum::<usize>(),
            )
            .saturating_add(std::mem::size_of::<Self>());
        let mut allocation = allocation::ProviderAllocation::try_new(
            resource_budget,
            u64::try_from(metadata_bytes).map_err(|_| ProviderContractError::ResourceOverflow)?,
        )?;
        let batches = allocation.retain_vec(metadata_bytes as u64, batches)?;
        Ok(Self {
            relation,
            schema_identity,
            schema,
            batches,
        })
    }

    #[must_use]
    pub const fn relation(&self) -> &ProviderRelationIdentity {
        &self.relation
    }

    #[must_use]
    pub const fn schema_identity(&self) -> &ProviderSchemaIdentity {
        &self.schema_identity
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    fn rows(&self) -> u64 {
        self.batches
            .iter()
            .map(|batch| u64::try_from(batch.num_rows()).unwrap_or(u64::MAX))
            .sum()
    }

    fn bytes(&self) -> u64 {
        self.batches
            .iter()
            .map(|batch| u64::try_from(batch.get_array_memory_size()).unwrap_or(u64::MAX))
            .sum()
    }
}

/// Trust result retained independently from provider terminal status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderTrustOutcome {
    Trusted,
    Degraded { detail: Arc<str> },
    Rejected { detail: Arc<str> },
}

fn validate_evidence_detail(detail: &str) -> Result<(), ProviderContractError> {
    if detail.is_empty()
        || detail.len() > MAX_DIAGNOSTIC_BYTES
        || detail.trim() != detail
        || detail.chars().any(char::is_control)
    {
        return Err(ProviderContractError::InvalidDiagnostic);
    }
    Ok(())
}

/// Closed run terminal; it must agree with coverage and gaps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTerminalStatus {
    Complete,
    Partial,
    Unknown,
    TimedOut,
    Cancelled,
    Corrupt,
    Oversized,
    Failed,
}

/// Derived resource consumption observed from owned result values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderResourceOutcome {
    pub relations: usize,
    pub batches: usize,
    pub rows: u64,
    pub bytes: u64,
    pub diagnostics: usize,
}

/// Provider-emitted result before release-owned admission joins it to a job.
#[derive(Clone, Debug)]
pub struct ProviderRunResult {
    suite: SuiteIdentity,
    provider: ProviderIdentity,
    protocol: ProviderProtocolIdentity,
    source: ProviderSourceBinding,
    context: crate::resource_budget::ChargedValue<ProviderContextBinding>,
    run: ProviderRunBinding,
    provenance: ProviderRunProvenance,
    relations: crate::resource_budget::ChargedSlice<ProviderRelationOutput>,
    coverage: crate::resource_budget::ChargedSlice<ProviderCoverage>,
    gaps: crate::resource_budget::ChargedSlice<ProviderGap>,
    diagnostics: crate::resource_budget::ChargedSlice<ProviderDiagnostic>,
    trust: ProviderTrustOutcome,
    terminal: ProviderTerminalStatus,
    resources: ProviderResourceOutcome,
    support: crate::resource_budget::ChargedValue<ProviderRunSupport>,
}

/// Identity and evidence arguments for one provider result.
#[derive(Clone, Debug)]
pub struct ProviderRunResultSpec {
    pub resource_budget: ResourceBudget,
    pub suite: SuiteIdentity,
    pub provider: ProviderIdentity,
    pub protocol: ProviderProtocolIdentity,
    pub source: ProviderSourceBinding,
    pub context: crate::resource_budget::ChargedValue<ProviderContextBinding>,
    pub run: ProviderRunBinding,
    pub provenance: ProviderRunProvenance,
    pub relations: Vec<ProviderRelationOutput>,
    pub coverage: Vec<ProviderCoverage>,
    pub gaps: Vec<ProviderGap>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub trust: ProviderTrustOutcome,
    pub terminal: ProviderTerminalStatus,
    pub support: ProviderRunSupport,
}

/// Provider-authored evidence whose categorical and binary invocation pins are copied from a job.
#[derive(Clone, Debug)]
pub struct ProviderRunEvidenceSpec {
    pub relations: Vec<ProviderRelationOutput>,
    pub coverage: Vec<ProviderCoverage>,
    pub gaps: Vec<ProviderGap>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub trust: ProviderTrustOutcome,
    pub terminal: ProviderTerminalStatus,
    pub support: ProviderRunSupport,
}

impl ProviderRunResult {
    /// Construct provider evidence bound to the exact immutable job invocation.
    ///
    /// This does not grant admission authority; release-owned admission still joins every
    /// requested family, schema, coverage value, trust outcome, and resource ceiling.
    ///
    /// # Errors
    ///
    /// Rejects internally incoherent provider evidence.
    pub fn try_from_job(
        job: &ProviderJob,
        evidence: ProviderRunEvidenceSpec,
    ) -> Result<Self, ProviderContractError> {
        Self::try_new(ProviderRunResultSpec {
            resource_budget: job.resource_budget.clone(),
            suite: job.suite.clone(),
            provider: job.provider.clone(),
            protocol: job.protocol.clone(),
            source: job.source.clone(),
            context: job.context.clone(),
            run: job.run.clone(),
            provenance: job.provenance.clone(),
            relations: evidence.relations,
            coverage: evidence.coverage,
            gaps: evidence.gaps,
            diagnostics: evidence.diagnostics,
            trust: evidence.trust,
            terminal: evidence.terminal,
            support: evidence.support,
        })
    }

    /// Validate internal result coherence without granting admission authority.
    ///
    /// # Errors
    ///
    /// Rejects duplicate relations/coverage, missing or contradictory gaps, false terminal status,
    /// and unbounded arithmetic.
    pub fn try_new(spec: ProviderRunResultSpec) -> Result<Self, ProviderContractError> {
        allocation::require_native_workspace(
            spec.context.reservation().owner(),
            &spec.resource_budget,
        )?;
        if spec.resource_budget.owner()
            != (ResourceOwner {
                kind: ResourceScopeKind::Operation,
                id: spec.run.provider_run_id(),
            })
        {
            return Err(ProviderContractError::ResourceOwnerMismatch);
        }
        for relation in &spec.relations {
            if !relation
                .batches
                .reservation()
                .owner()
                .same_scope(&spec.resource_budget)
            {
                return Err(ProviderContractError::ResourceOwnerMismatch);
            }
        }
        if spec.coverage.is_empty() || spec.coverage.len() > MAX_REQUESTED_FAMILIES {
            return Err(ProviderContractError::MissingCoverage);
        }
        match &spec.trust {
            ProviderTrustOutcome::Trusted => {}
            ProviderTrustOutcome::Degraded { detail }
            | ProviderTrustOutcome::Rejected { detail } => validate_evidence_detail(detail)?,
        }
        let mut relation_ids = BTreeSet::new();
        let mut batches = 0_usize;
        let mut rows = 0_u64;
        // Include owned typed support as well as Arrow buffers in the result envelope.
        let mut bytes = spec.support.memory_bytes()?;
        for relation in &spec.relations {
            if !relation_ids.insert(relation.relation.clone()) {
                return Err(ProviderContractError::DuplicateRelation);
            }
            batches = batches
                .checked_add(relation.batches.len())
                .ok_or(ProviderContractError::ResourceOverflow)?;
            rows = rows
                .checked_add(relation.rows())
                .ok_or(ProviderContractError::ResourceOverflow)?;
            bytes = bytes
                .checked_add(relation.bytes())
                .ok_or(ProviderContractError::ResourceOverflow)?;
        }

        let mut coverage = BTreeMap::new();
        for observation in &spec.coverage {
            if coverage
                .insert(observation.family.clone(), observation.state.clone())
                .is_some()
            {
                return Err(ProviderContractError::DuplicateFamily);
            }
        }
        let mut gaps = BTreeMap::new();
        for gap in &spec.gaps {
            if gaps.insert(gap.family.clone(), gap.cause).is_some() {
                return Err(ProviderContractError::DuplicateGap);
            }
        }
        for (family, state) in &coverage {
            match state {
                ProviderCoverageState::Unknown { cause, .. } if gaps.get(family) == Some(cause) => {
                }
                ProviderCoverageState::Unknown { .. } => {
                    return Err(ProviderContractError::MissingOrContradictoryGap);
                }
                ProviderCoverageState::Complete { .. }
                | ProviderCoverageState::IntentionalRemainder { .. }
                    if !gaps.contains_key(family) => {}
                _ => return Err(ProviderContractError::UnexpectedGap),
            }
        }
        if gaps.keys().any(|family| !coverage.contains_key(family)) {
            return Err(ProviderContractError::UnexpectedGap);
        }
        if terminal_for_coverage(coverage.values()) != spec.terminal {
            return Err(ProviderContractError::FalseTerminal);
        }

        let resources = ProviderResourceOutcome {
            relations: spec.relations.len(),
            batches,
            rows,
            bytes,
            diagnostics: spec.diagnostics.len(),
        };
        Self::retain_owned(spec, resources)
    }

    fn retain_owned(
        spec: ProviderRunResultSpec,
        resources: ProviderResourceOutcome,
    ) -> Result<Self, ProviderContractError> {
        let support_bytes = spec.support.memory_bytes()?;
        let metadata = support_bytes
            .checked_add(
                (spec
                    .relations
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ProviderRelationOutput>())
                    + spec
                        .coverage
                        .capacity()
                        .saturating_mul(std::mem::size_of::<ProviderCoverage>())
                    + spec
                        .gaps
                        .capacity()
                        .saturating_mul(std::mem::size_of::<ProviderGap>())
                    + spec
                        .diagnostics
                        .capacity()
                        .saturating_mul(std::mem::size_of::<ProviderDiagnostic>()))
                    as u64,
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    spec.gaps
                        .iter()
                        .map(|gap| gap.detail.len() as u64)
                        .sum::<u64>(),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    spec.diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.len() as u64)
                        .sum::<u64>(),
                )
            })
            .ok_or(ProviderContractError::ResourceOverflow)?;
        let mut allocation =
            allocation::ProviderAllocation::try_new(&spec.resource_budget, metadata)?;
        let relations = allocation.retain_measured_vec(spec.relations, |_| 0)?;
        let coverage = allocation.retain_measured_vec(spec.coverage, |_| 0)?;
        let gaps = allocation.retain_measured_vec(spec.gaps, |gap| gap.detail.len())?;
        let diagnostics = allocation
            .retain_measured_vec(spec.diagnostics, |diagnostic| diagnostic.message.len())?;
        let support = allocation.retain_value(support_bytes, spec.support)?;
        Ok(Self {
            suite: spec.suite,
            provider: spec.provider,
            protocol: spec.protocol,
            source: spec.source,
            context: spec.context,
            run: spec.run,
            provenance: spec.provenance,
            relations,
            coverage,
            gaps,
            diagnostics,
            trust: spec.trust,
            terminal: spec.terminal,
            resources,
            support,
        })
    }

    #[must_use]
    pub const fn resources(&self) -> ProviderResourceOutcome {
        self.resources
    }

    #[must_use]
    pub fn support(&self) -> &ProviderRunSupport {
        &self.support
    }

    #[must_use]
    pub fn relations(&self) -> &[ProviderRelationOutput] {
        &self.relations
    }

    #[must_use]
    pub fn coverage(&self) -> &[ProviderCoverage] {
        &self.coverage
    }

    #[must_use]
    pub fn gaps(&self) -> &[ProviderGap] {
        &self.gaps
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ProviderDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn terminal(&self) -> ProviderTerminalStatus {
        self.terminal
    }

    #[must_use]
    pub const fn trust(&self) -> &ProviderTrustOutcome {
        &self.trust
    }
}

fn terminal_for_coverage<'a>(
    states: impl Iterator<Item = &'a ProviderCoverageState>,
) -> ProviderTerminalStatus {
    let mut terminal = ProviderTerminalStatus::Complete;
    for state in states {
        let candidate = match state {
            ProviderCoverageState::Complete { .. } => ProviderTerminalStatus::Complete,
            ProviderCoverageState::IntentionalRemainder { .. } => ProviderTerminalStatus::Partial,
            ProviderCoverageState::Unknown { cause, .. } => match cause {
                ProviderUnknownCause::MissingOutput | ProviderUnknownCause::Unsupported => {
                    ProviderTerminalStatus::Unknown
                }
                ProviderUnknownCause::Timeout => ProviderTerminalStatus::TimedOut,
                ProviderUnknownCause::Cancelled => ProviderTerminalStatus::Cancelled,
                ProviderUnknownCause::Corruption => ProviderTerminalStatus::Corrupt,
                ProviderUnknownCause::Oversized => ProviderTerminalStatus::Oversized,
                ProviderUnknownCause::ProviderFailure | ProviderUnknownCause::TrustLoss => {
                    ProviderTerminalStatus::Failed
                }
            },
        };
        if terminal_rank(candidate) > terminal_rank(terminal) {
            terminal = candidate;
        }
    }
    terminal
}

const fn terminal_rank(value: ProviderTerminalStatus) -> u8 {
    match value {
        ProviderTerminalStatus::Complete => 0,
        ProviderTerminalStatus::Partial => 1,
        ProviderTerminalStatus::Unknown => 2,
        ProviderTerminalStatus::TimedOut => 3,
        ProviderTerminalStatus::Cancelled => 4,
        ProviderTerminalStatus::Failed => 5,
        ProviderTerminalStatus::Oversized => 6,
        ProviderTerminalStatus::Corrupt => 7,
    }
}

/// Result admitted only after exact job/result, coverage, schema, trust, and resource joins.
#[derive(Clone, Debug)]
pub struct AdmittedProviderResult {
    job: ProviderJob,
    result: ProviderRunResult,
}

impl AdmittedProviderResult {
    #[must_use]
    pub const fn job(&self) -> &ProviderJob {
        &self.job
    }

    #[must_use]
    pub const fn result(&self) -> &ProviderRunResult {
        &self.result
    }

    /// Derive a bounded contract observation from the admitted values.
    #[must_use]
    pub fn observation(&self) -> ProviderContractObservation {
        ProviderContractObservation {
            suite: self.job.suite.clone(),
            provider: self.job.provider.clone(),
            run: self.job.run.identity.clone(),
            lane: self.job.lane,
            requested_families: self.job.requests.len(),
            emitted_relations: self.result.relations.len(),
            terminal: self.result.terminal,
            resources: self.result.resources,
        }
    }
}

/// Derived observation of a constructed and admitted provider contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderContractObservation {
    pub suite: SuiteIdentity,
    pub provider: ProviderIdentity,
    pub run: ProviderRunIdentity,
    pub lane: ProviderLane,
    pub requested_families: usize,
    pub emitted_relations: usize,
    pub terminal: ProviderTerminalStatus,
    pub resources: ProviderResourceOutcome,
}

/// Join an untrusted provider result to the exact release-prepared job.
///
/// # Errors
///
/// Rejects wrong categorical identities/provenance, missing or extra coverage, unrequested or
/// schema-mismatched relations, false completion, rejected trust, and any effective bound breach.
pub fn admit_provider_result(
    job: ProviderJob,
    result: ProviderRunResult,
) -> Result<AdmittedProviderResult, ProviderContractError> {
    if job.suite != result.suite
        || job.provider != result.provider
        || job.protocol != result.protocol
        || job.source != result.source
        || job.context != result.context
        || job.run != result.run
        || job.provenance != result.provenance
    {
        return Err(ProviderContractError::IdentityMismatch);
    }
    if matches!(result.trust, ProviderTrustOutcome::Rejected { .. }) {
        return Err(ProviderContractError::RejectedTrust);
    }
    result.support.validate_for_job(&job)?;

    let requests = job
        .requests
        .iter()
        .map(|request| (request.family.clone(), request))
        .collect::<BTreeMap<_, _>>();
    let coverage = result
        .coverage
        .iter()
        .map(|observation| (observation.family.clone(), &observation.state))
        .collect::<BTreeMap<_, _>>();
    if requests.keys().ne(coverage.keys()) {
        return Err(ProviderContractError::CoverageSetMismatch);
    }
    let emitted_relations = result
        .relations
        .iter()
        .map(|relation| &relation.relation)
        .collect::<BTreeSet<_>>();
    for (family, request) in &requests {
        let state = coverage[family];
        if state.completed_units() > request.requested_units()
            || matches!(state, ProviderCoverageState::Complete { .. })
                && state.completed_units() != request.requested_units()
            || matches!(state, ProviderCoverageState::IntentionalRemainder { .. })
                && state.completed_units() >= request.requested_units()
        {
            return Err(ProviderContractError::FalseCoverage);
        }
        if matches!(state, ProviderCoverageState::Complete { .. })
            && !emitted_relations.contains(&request.relation)
        {
            return Err(ProviderContractError::MissingCompleteRelation);
        }
    }

    let requested_relations = job
        .requests
        .iter()
        .map(|request| {
            (
                request.relation.clone(),
                (&request.schema_identity, &request.schema),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for relation in &result.relations {
        let (expected_identity, expected_schema) = requested_relations
            .get(&relation.relation)
            .ok_or(ProviderContractError::UnrequestedRelation)?;
        if *expected_identity != &relation.schema_identity || *expected_schema != &relation.schema {
            return Err(ProviderContractError::ArrowSchemaMismatch);
        }
    }

    let resources = result.resources;
    if resources.relations > job.ceilings.max_relations()
        || result
            .relations
            .iter()
            .any(|relation| relation.batches.len() > job.ceilings.max_batches_per_relation())
        || resources.rows > job.ceilings.max_rows()
        || resources.bytes > job.ceilings.max_bytes()
        || resources.diagnostics > job.ceilings.max_diagnostics()
    {
        return Err(ProviderContractError::ResourceCeilingExceeded);
    }
    Ok(AdmittedProviderResult { job, result })
}

/// Application-owned projection of a rustc compilation-begin control event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcCompilationHeader {
    pub run: ProviderRunIdentity,
    pub compilation_unit: RustCompilationUnitIdentity,
    pub protocol: ProviderProtocolIdentity,
    pub source: SourceIdentity,
    pub context: ContextIdentity,
    pub compiler_build: ProviderBuildIdentity,
    pub toolchain: RustToolchainIdentity,
    pub requested_capability_count: u64,
}

/// Application-owned projection of a rustc owner-begin control event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcOwnerHeader {
    pub owner: RustOwnerIdentity,
    pub canonical_owner: CanonicalEntityIdentity,
    pub expected_relation_count: u64,
}

/// Application-owned projection of a rustc owner terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcOwnerTerminal {
    pub owner: RustOwnerIdentity,
    pub relation_count: u64,
    pub row_count: u64,
    pub coverage: ProviderCoverageState,
}

/// Application-owned projection of a rustc compilation terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcCompilationTerminal {
    pub run: ProviderRunIdentity,
    pub compilation_unit: RustCompilationUnitIdentity,
    pub compiler_exit_status: i32,
    pub owner_count: u64,
    pub relation_count: u64,
    pub terminal: ProviderTerminalStatus,
    pub diagnostics_count: u64,
}

/// One validated owner control projection with no generated transport value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcOwnerControl {
    pub header: RustcOwnerHeader,
    pub terminal: RustcOwnerTerminal,
}

impl RustcOwnerControl {
    /// Join one owner header to its exact terminal.
    ///
    /// # Errors
    ///
    /// Rejects an owner mismatch or a terminal relation count wider than the declared count.
    pub fn try_new(
        header: RustcOwnerHeader,
        terminal: RustcOwnerTerminal,
    ) -> Result<Self, ProviderContractError> {
        if header.owner != terminal.owner
            || terminal.relation_count > header.expected_relation_count
        {
            return Err(ProviderContractError::RustcControlMismatch);
        }
        Ok(Self { header, terminal })
    }
}

/// Complete application-owned rustc control projection used by later admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcCompilationControl {
    pub header: RustcCompilationHeader,
    pub owners: Vec<RustcOwnerControl>,
    pub terminal: RustcCompilationTerminal,
}

impl RustcCompilationControl {
    /// Join application-owned headers and terminals after transport conversion.
    ///
    /// # Errors
    ///
    /// Rejects run/count mismatches and duplicate owner identities.
    pub fn try_new(
        header: RustcCompilationHeader,
        owners: Vec<RustcOwnerControl>,
        terminal: RustcCompilationTerminal,
    ) -> Result<Self, ProviderContractError> {
        let owner_count = u64::try_from(owners.len()).unwrap_or(u64::MAX);
        let relation_count = owners.iter().try_fold(0_u64, |count, owner| {
            count.checked_add(owner.terminal.relation_count)
        });
        let distinct_owners = owners
            .iter()
            .map(|owner| owner.header.owner.clone())
            .collect::<BTreeSet<_>>();
        if header.run != terminal.run
            || header.compilation_unit != terminal.compilation_unit
            || terminal.owner_count != owner_count
            || relation_count != Some(terminal.relation_count)
            || distinct_owners.len() != owners.len()
        {
            return Err(ProviderContractError::RustcControlMismatch);
        }
        Ok(Self {
            header,
            owners,
            terminal,
        })
    }
}

/// Closed provider-contract validation failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderContractError {
    #[error("provider resource budget differs from its exact operation or workspace")]
    ResourceOwnerMismatch,
    #[error(transparent)]
    ResourceBudget(#[from] ResourceBudgetError),
    #[error(
        "provider input dispositions or changed/withdrawn membership do not close the authorized inventory"
    )]
    SourceInventoryMismatch,
    #[error("provider support or replacement partitions differ from the exact job obligations")]
    SupportMismatch,
    #[error("invalid {kind}")]
    InvalidIdentity { kind: &'static str },
    #[error("provider invocation carries an all-zero binary pin")]
    ZeroInvocationPin,
    #[error("provider resource ceiling is zero or wider than the application hard limit")]
    InvalidResourceCeiling,
    #[error("cancellation polling interval is zero or application-unbounded")]
    InvalidCancellationProbe,
    #[error("provider family request has a zero unit census")]
    EmptyRequest,
    #[error("provider job is already expired")]
    ExpiredJob,
    #[error("provider job request set is empty or oversized")]
    EmptyOrOversizedRequestSet,
    #[error("provider family occurs more than once")]
    DuplicateFamily,
    #[error("provider relation occurs more than once")]
    DuplicateRelation,
    #[error("provider gap occurs more than once")]
    DuplicateGap,
    #[error("provider diagnostic or gap detail is invalid")]
    InvalidDiagnostic,
    #[error("provider relation contains no Arrow batch")]
    EmptyRelationOutput,
    #[error("provider Arrow schema identity or value is inconsistent")]
    ArrowSchemaMismatch,
    #[error("provider result omitted terminal coverage")]
    MissingCoverage,
    #[error("unknown coverage lacks its exact explicit gap")]
    MissingOrContradictoryGap,
    #[error("gap exists for a complete, remainder, or absent coverage family")]
    UnexpectedGap,
    #[error("provider terminal contradicts coverage")]
    FalseTerminal,
    #[error("provider resource accounting overflowed")]
    ResourceOverflow,
    #[error("provider result categorical identity or provenance differs from its job")]
    IdentityMismatch,
    #[error("provider result trust was rejected")]
    RejectedTrust,
    #[error("provider result coverage families differ from requested families")]
    CoverageSetMismatch,
    #[error("provider result falsely reports completed or remainder coverage")]
    FalseCoverage,
    #[error("complete provider coverage lacks its schema-bearing relation output")]
    MissingCompleteRelation,
    #[error("provider emitted an unrequested relation")]
    UnrequestedRelation,
    #[error("provider result exceeds its effective resource ceiling")]
    ResourceCeilingExceeded,
    #[error("application-owned rustc control projections are inconsistent")]
    RustcControlMismatch,
}

#[cfg(test)]
pub(crate) fn fixture_provider_budget(workspace: [u8; 16], run: [u8; 16]) -> ResourceBudget {
    use crate::resource_budget::{ResourceAmounts, ResourceBudgetPolicy};
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: 1 << 30,
            disk_bytes: 1 << 30,
            running_jobs: 64,
            queued_jobs: 64,
            retained_generations: 128,
            retained_bytes: 1 << 30,
            rows: 1_000_000_000,
            pages: 65_536,
        },
        control_reserve: ResourceAmounts::default(),
    };
    thread_local! {
        static SCOPES: std::cell::RefCell<BTreeMap<([u8; 16], [u8; 16]), ResourceBudget>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    }
    SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        if let Some(scope) = scopes.get(&(workspace, run)) {
            return scope.clone();
        }
        let workspace_scope = if let Some(scope) = scopes.get(&(workspace, [0; 16])) {
            scope.clone()
        } else {
            let scope = ResourceBudget::try_process([231; 16], policy)
                .unwrap()
                .workspace(workspace, policy)
                .unwrap();
            scopes.insert((workspace, [0; 16]), scope.clone());
            scope
        };
        let operation = workspace_scope.operation(run, policy).unwrap();
        scopes.insert((workspace, run), operation.clone());
        operation
    })
}

#[cfg(test)]
pub(crate) fn fixture_provider_context(
    workspace: [u8; 16],
    context: ProviderContextBinding,
) -> crate::resource_budget::ChargedValue<ProviderContextBinding> {
    let budget = fixture_provider_budget(workspace, [254; 16]);
    budget
        .try_reserve(
            crate::resource_budget::ResourceClass::Data,
            crate::resource_budget::ResourceAmounts {
                memory_bytes: context.memory_bytes().unwrap(),
                ..Default::default()
            },
        )
        .unwrap()
        .into_charged_value(context)
}

#[cfg(test)]
impl ProviderSourceBinding {
    pub(crate) fn from_inventory_fixture(
        identity: SourceIdentity,
        inventory: ProviderSourceInventory,
    ) -> Self {
        let budget = fixture_provider_budget(inventory.workspace_id(), [254; 16]);
        let charged = budget
            .try_reserve(
                crate::resource_budget::ResourceClass::Data,
                crate::resource_budget::ResourceAmounts {
                    memory_bytes: inventory.memory_bytes().unwrap(),
                    ..Default::default()
                },
            )
            .unwrap()
            .into_charged_value(inventory);
        Self::from_inventory(identity, charged)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};

    use super::*;

    fn input_member(path: &[u8], file: u8) -> ProviderInventoryMember {
        ProviderInventoryMember {
            relative_path: path.to_vec(),
            disposition: ProviderInputDisposition::Captured {
                file_id: [file; 16],
                digest: [17; 32],
                byte_length: 12,
            },
            selected_for_provider: true,
        }
    }

    #[test]
    fn provider_selections_preserve_capture_and_reject_substitution() {
        let census = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [25; 32],
            &[b"src/a.py".to_vec(), b"src/b.rs".to_vec()],
            vec![input_member(b"src/a.py", 7), input_member(b"src/b.rs", 8)],
            vec![b"src/a.py".to_vec()],
            None,
        )
        .unwrap();
        let python = census.select_files(&BTreeSet::from([[7; 16]])).unwrap();
        let rust = census.select_files(&BTreeSet::from([[8; 16]])).unwrap();
        assert!(python.has_same_capture(&rust));
        assert!(rust.has_same_capture(&census));
        assert_ne!(python.identity(), rust.identity());
        assert_eq!(
            rust.selected_files()
                .map(|(file, _)| file)
                .collect::<Vec<_>>(),
            vec![[8; 16]]
        );
        assert_eq!(rust.members().len(), 2);
        assert_eq!(rust.changed_paths(), census.changed_paths());
        assert_eq!(rust.select_files(&BTreeSet::from([[8; 16]])).unwrap(), rust);
        assert_eq!(
            python.select_files(&BTreeSet::from([[8; 16]])).unwrap(),
            rust
        );
        assert!(rust.select_files(&BTreeSet::from([[9; 16]])).is_err());
        let changed = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [25; 32],
            &[b"src/a.py".to_vec(), b"src/b.rs".to_vec()],
            vec![input_member(b"src/a.py", 9), input_member(b"src/b.rs", 8)],
            vec![b"src/a.py".to_vec()],
            None,
        )
        .unwrap();
        assert!(!rust.has_same_capture(&changed));
    }

    fn missing_import() -> ProviderSupportDependency {
        ProviderSupportDependency {
            key: ProviderLookupKey::Import {
                qualified_name: b"optional_dependency".to_vec(),
            },
            scope: ProviderSearchScope {
                namespace: b"python-project".to_vec(),
                ordered_roots: vec![b"src".to_vec(), b"stubs".to_vec()],
                policy_identity: [21; 32],
                effective_context: [3; 32],
            },
            outcome: ProviderLookupOutcome::Absent {
                closed_universe: [22; 32],
            },
        }
    }

    fn complete_with_support(job: &ProviderJob, support: ProviderRunSupport) -> ProviderRunResult {
        let request = &job.requests()[0];
        ProviderRunResult::try_from_job(
            job,
            ProviderRunEvidenceSpec {
                relations: vec![
                    ProviderRelationOutput::try_new(
                        request.relation().clone(),
                        request.schema_identity().clone(),
                        Arc::clone(request.schema()),
                        vec![RecordBatch::new_empty(Arc::clone(request.schema()))],
                        &crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
                    )
                    .unwrap(),
                ],
                coverage: vec![ProviderCoverage::new(
                    request.family().clone(),
                    ProviderCoverageState::Complete {
                        completed_units: request.requested_units(),
                    },
                )],
                gaps: vec![],
                diagnostics: vec![],
                trust: ProviderTrustOutcome::Trusted,
                terminal: ProviderTerminalStatus::Complete,
                support,
            },
        )
        .unwrap()
    }

    #[test]
    fn rt_cpg_wp78_behavior() {
        let (_, mut original) = job();
        original.context = fixture_provider_context(
            [6; 16],
            (*original.context)
                .clone()
                .with_support_obligations(vec![missing_import()])
                .unwrap(),
        );
        let old = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [25; 32],
            &[b"src/a.py".to_vec(), b"src/b.py".to_vec()],
            vec![input_member(b"src/a.py", 7), input_member(b"src/b.py", 8)],
            vec![b"src/a.py".to_vec()],
            None,
        )
        .unwrap();
        assert_eq!(old.selected_files().count(), 2);
        assert_eq!(old.changed_paths().len(), 1);
        original.source = ProviderSourceBinding::from_inventory_fixture(
            SourceIdentity::try_new("closed-inventory-7").unwrap(),
            old.clone(),
        );
        let support = ProviderRunSupport::from_job_inputs(&original, true);
        assert!(!support.requires_context_invalidation());
        let admitted =
            admit_provider_result(original.clone(), complete_with_support(&original, support))
                .unwrap();
        assert_eq!(admitted.result().support().partitions.len(), 3);
        assert!(admitted.result().support().partitions.iter().all(|entry| {
            admitted
                .result()
                .support()
                .dependencies_for(entry)
                .any(|dependency| *dependency == missing_import())
        }));

        let mut changed = original.clone();
        let mut lookup = missing_import();
        lookup.scope.ordered_roots.reverse();
        lookup.scope.effective_context = [32; 32];
        changed.context = fixture_provider_context(
            [6; 16],
            ProviderContextBinding::try_new(
                ContextIdentity::try_new("selected-stubs-first").unwrap(),
                [31; 16],
                [32; 32],
                [33; 32],
            )
            .unwrap()
            .with_support_obligations(vec![lookup])
            .unwrap(),
        );
        assert_eq!(
            original.source, changed.source,
            "source bytes did not change"
        );
        assert_ne!(original.context, changed.context);
        assert_ne!(
            ProviderRunSupport::from_job_inputs(&original, true),
            ProviderRunSupport::from_job_inputs(&changed, true)
        );
        assert_eq!(
            admit_provider_result(changed, admitted.result().clone()).unwrap_err(),
            ProviderContractError::IdentityMismatch
        );

        let empty =
            ProviderSourceInventory::try_new([6; 16], 8, [26; 32], &[], vec![], vec![], Some(&old))
                .unwrap();
        assert_eq!(empty.withdrawn().len(), 2);
        let mut deleted = original;
        deleted.source = ProviderSourceBinding::from_inventory_fixture(
            SourceIdentity::try_new("closed-empty-8").unwrap(),
            empty,
        );
        let mut requests = deleted.requests.to_vec();
        requests[0].requested_units = 0;
        deleted.requests = crate::resource_budget::ChargedSlice::for_test(requests);
        let support = ProviderRunSupport::from_job_inputs(&deleted, true);
        assert_eq!(
            support
                .partitions
                .iter()
                .filter(|entry| entry.partition.action == ProviderPartitionAction::Withdraw)
                .count(),
            2
        );
        let admitted =
            admit_provider_result(deleted.clone(), complete_with_support(&deleted, support))
                .unwrap();
        assert_eq!(admitted.result().resources().rows, 0);
        assert_eq!(admitted.result().resources().relations, 1);
        let current = match deleted.source.selection() {
            ProviderSourceSelection::Inventory(current) => current,
            _ => unreachable!(),
        };
        let recreated = ProviderSourceInventory::try_new(
            [6; 16],
            9,
            [27; 32],
            &[b"src/a.py".to_vec()],
            vec![input_member(b"src/a.py", 7)],
            vec![b"src/a.py".to_vec()],
            Some(current),
        )
        .unwrap();
        assert_eq!(recreated.selected_files().count(), 1);
        assert!(recreated.withdrawn().is_empty());
    }

    #[test]
    fn rt_cpg_wp78_faults() {
        let paths = vec![b"a.py".to_vec(), b"b.py".to_vec()];
        for incomplete in [vec![input_member(b"a.py", 7)], vec![]] {
            assert_eq!(
                ProviderSourceInventory::try_new(
                    [6; 16],
                    7,
                    [25; 32],
                    &paths,
                    incomplete,
                    vec![b"a.py".to_vec()],
                    None
                )
                .unwrap_err(),
                ProviderContractError::SourceInventoryMismatch,
                "dirty work is not the full inventory"
            );
        }
        let (_, mut job) = job();
        job.context = fixture_provider_context(
            [6; 16],
            (*job.context)
                .clone()
                .with_support_obligations(vec![missing_import()])
                .unwrap(),
        );
        let mut incomplete = ProviderRunSupport::from_job_inputs(&job, true);
        incomplete.common_dependencies = incomplete
            .common_dependencies
            .iter()
            .filter(|dependency| !matches!(dependency.key, ProviderLookupKey::Import { .. }))
            .cloned()
            .collect();
        assert_eq!(
            admit_provider_result(job.clone(), complete_with_support(&job, incomplete))
                .unwrap_err(),
            ProviderContractError::SupportMismatch
        );
        let mut missing_partition = ProviderRunSupport::from_job_inputs(&job, true);
        missing_partition.partitions.clear();
        assert_eq!(
            admit_provider_result(job.clone(), complete_with_support(&job, missing_partition))
                .unwrap_err(),
            ProviderContractError::SupportMismatch
        );
        let mut wrong_context = ProviderRunSupport::from_job_inputs(&job, true);
        wrong_context.partitions[0].partition.key.context_id = [99; 16];
        assert_eq!(
            admit_provider_result(job.clone(), complete_with_support(&job, wrong_context))
                .unwrap_err(),
            ProviderContractError::SupportMismatch
        );
        assert!(ProviderRunSupport::conservative(&job).requires_context_invalidation());
        let mut unknown_lookup = missing_import();
        unknown_lookup.outcome = ProviderLookupOutcome::Incomplete {
            observed_universe: [22; 32],
        };
        job.context = fixture_provider_context(
            [6; 16],
            (*job.context)
                .clone()
                .with_support_obligations(vec![unknown_lookup])
                .unwrap(),
        );
        let support = ProviderRunSupport::from_job_inputs(&job, true);
        assert!(support.requires_context_invalidation());
        admit_provider_result(job.clone(), complete_with_support(&job, support)).unwrap();
        assert_ne!(
            job.context.analysis_context_id().len(),
            job.context.context_fingerprint().len()
        );
        assert_ne!(job.source.workspace_id(), job.source.file_id().unwrap());
    }

    #[test]
    fn rt_cpg_wp78_path_owned_file_identity_rejects_drift() {
        let previous = ProviderSourceInventory::try_new(
            [6; 16],
            7,
            [25; 32],
            &[b"a.py".to_vec()],
            vec![input_member(b"a.py", 7)],
            vec![],
            None,
        )
        .unwrap();
        for member in [input_member(b"a.py", 8), input_member(b"renamed.py", 7)] {
            assert_eq!(
                ProviderSourceInventory::try_new(
                    [6; 16],
                    8,
                    [26; 32],
                    &[member.relative_path.clone()],
                    vec![member],
                    vec![],
                    Some(&previous),
                )
                .unwrap_err(),
                ProviderContractError::SourceInventoryMismatch
            );
        }
        let renamed = ProviderSourceInventory::try_new(
            [6; 16],
            8,
            [26; 32],
            &[b"renamed.py".to_vec()],
            vec![input_member(b"renamed.py", 8)],
            vec![],
            Some(&previous),
        )
        .unwrap();
        assert_eq!(renamed.withdrawn(), &[([7; 16], b"a.py".to_vec())]);
    }

    #[test]
    fn rt_cpg_wp78_producer_program_and_effective_inputs_bind_support() {
        let (_, original) = job();
        // Independently computed with the pinned Python rfc8785 + blake3 implementations.
        assert_eq!(
            blake3::Hash::from(original.producer_release_identity())
                .to_hex()
                .as_str(),
            "4a69ad52e3d79556ce8824446cbd5fd42037333543f125f9809e6e2fe1d39899"
        );
        let support = ProviderRunSupport::from_job_inputs(&original, true);
        let mut changed = original.clone();
        changed.provenance = ProviderRunProvenance::new(
            original.provenance.provider_build().clone(),
            original.provenance.policy().clone(),
            ProviderProgramIdentity::try_new("provider-program.changed").unwrap(),
        );
        changed = rebuild_job(changed).unwrap();
        assert_ne!(
            original.provenance.producer_release_identity(),
            changed.provenance.producer_release_identity()
        );
        assert_ne!(
            original.replacement_obligations(),
            changed.replacement_obligations()
        );
        assert_eq!(
            admit_provider_result(
                changed.clone(),
                complete_with_support(&changed, support.clone())
            )
            .unwrap_err(),
            ProviderContractError::SupportMismatch
        );

        let module = ProviderModuleBinding {
            file_id: [7; 16],
            qualified_name: "a".into(),
            relative_path: b"a.py".to_vec(),
        };
        let with_module = (*original.context)
            .clone()
            .with_modules(vec![module.clone()])
            .unwrap();
        assert!(Arc::ptr_eq(
            &with_module.modules,
            &with_module.clone().modules
        ));
        let with_version = (*original.context)
            .clone()
            .with_python_version(3, 14)
            .unwrap();
        let mut renamed = module;
        renamed.qualified_name = "different_module".into();
        assert_ne!(
            with_module.effective_input_identity(),
            (*original.context)
                .clone()
                .with_modules(vec![renamed])
                .unwrap()
                .effective_input_identity()
        );
        assert_ne!(
            with_version.effective_input_identity(),
            (*original.context)
                .clone()
                .with_python_version(3, 13)
                .unwrap()
                .effective_input_identity()
        );
        for context in [with_module, with_version] {
            assert_eq!(
                context.analysis_context_id(),
                original.context.analysis_context_id()
            );
            assert_eq!(
                context.context_fingerprint(),
                original.context.context_fingerprint()
            );
            assert_ne!(
                context.effective_input_identity(),
                original.context.effective_input_identity()
            );
            changed = original.clone();
            changed.context = fixture_provider_context([6; 16], context);
            assert_eq!(
                admit_provider_result(
                    changed.clone(),
                    complete_with_support(&changed, support.clone())
                )
                .unwrap_err(),
                ProviderContractError::SupportMismatch
            );
        }
    }

    #[test]
    fn rt_cpg_wp78_extra_lookup_authority_is_not_self_attested() {
        let (_, mut job) = job();
        job.context = fixture_provider_context(
            [6; 16],
            (*job.context)
                .clone()
                .with_support_obligations(vec![missing_import()])
                .unwrap(),
        );
        let mut extra = missing_import();
        extra.key = ProviderLookupKey::Import {
            qualified_name: b"another_dependency".to_vec(),
        };
        for mutation in 0..4 {
            let mut bad = extra.clone();
            match mutation {
                0 => bad.scope.effective_context = [99; 32],
                1 => bad.scope.policy_identity = [99; 32],
                2 => bad.scope.ordered_roots.reverse(),
                _ => {
                    bad.outcome = ProviderLookupOutcome::Absent {
                        closed_universe: [99; 32],
                    }
                }
            }
            for common in [false, true] {
                let mut support = ProviderRunSupport::from_job_inputs(&job, true);
                if common {
                    let mut shared = support.common_dependencies.to_vec();
                    shared.push(bad.clone());
                    support.common_dependencies = shared.into();
                } else {
                    support.partitions[0].dependencies.push(bad.clone());
                }
                assert_eq!(
                    admit_provider_result(job.clone(), complete_with_support(&job, support))
                        .unwrap_err(),
                    ProviderContractError::SupportMismatch
                );
            }
        }
        let mut authorized = ProviderRunSupport::from_job_inputs(&job, true);
        authorized.partitions[0].dependencies.push(extra.clone());
        let admitted =
            admit_provider_result(job.clone(), complete_with_support(&job, authorized)).unwrap();
        assert!(!admitted.result().support().requires_context_invalidation());
        let mut tight = job.clone();
        tight.ceilings.work_units = NonZeroU64::new(5).unwrap();
        let tight = rebuild_job(tight).unwrap();
        assert_eq!(
            admit_provider_result(tight, admitted.result().clone()).unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        extra.scope.ordered_roots = vec![b"unobserved_root".to_vec()];
        extra.outcome = ProviderLookupOutcome::Incomplete {
            observed_universe: [99; 32],
        };
        let mut conservative = ProviderRunSupport::from_job_inputs(&job, true);
        conservative.partitions[0].dependencies.push(extra);
        let admitted =
            admit_provider_result(job.clone(), complete_with_support(&job, conservative)).unwrap();
        assert!(admitted.result().support().requires_context_invalidation());
    }

    #[test]
    fn rt_cpg_wp78_consumed_source_support_requires_exact_captured_authority() {
        let (_, original) = job();
        for inventory_source in [false, true] {
            let mut job = original.clone();
            job.context = fixture_provider_context(
                [6; 16],
                (*job.context)
                    .clone()
                    .with_support_obligations(vec![missing_import()])
                    .unwrap(),
            );
            if inventory_source {
                let mut configuration = input_member(b"config.py", 8);
                configuration.selected_for_provider = false;
                job.source = ProviderSourceBinding::from_inventory_fixture(
                    SourceIdentity::try_new("inventory").unwrap(),
                    ProviderSourceInventory::try_new(
                        [6; 16],
                        7,
                        [25; 32],
                        &[
                            b"a.py".to_vec(),
                            b"config.py".to_vec(),
                            b"unreadable.py".to_vec(),
                        ],
                        vec![
                            input_member(b"a.py", 7),
                            configuration,
                            ProviderInventoryMember {
                                relative_path: b"unreadable.py".to_vec(),
                                disposition: ProviderInputDisposition::Unreadable,
                                selected_for_provider: false,
                            },
                        ],
                        vec![],
                        None,
                    )
                    .unwrap(),
                );
            }
            let job = rebuild_job(job).unwrap();
            for (file, digest, accepted) in [
                (7, 17, true),
                (7, 99, false),
                (8, 17, inventory_source),
                (9, 17, false),
                (99, 17, false),
            ] {
                for common in [false, true] {
                    let mut dependency = missing_import();
                    dependency.key = ProviderLookupKey::SourceBytes {
                        file_id: [file; 16],
                    };
                    dependency.outcome = ProviderLookupOutcome::Consumed {
                        revision: [digest; 32],
                        candidates: vec![],
                    };
                    let mut support = ProviderRunSupport::from_job_inputs(&job, true);
                    if common {
                        let mut shared = support.common_dependencies.to_vec();
                        shared.push(dependency);
                        support.common_dependencies = shared.into();
                    } else {
                        support.partitions[0].dependencies.push(dependency);
                    }
                    let result =
                        admit_provider_result(job.clone(), complete_with_support(&job, support));
                    assert_eq!(
                        result.is_ok(),
                        accepted,
                        "inventory={inventory_source}, common={common}, file={file}, digest={digest}"
                    );
                    if let Err(error) = result {
                        assert_eq!(error, ProviderContractError::SupportMismatch);
                    }
                }
            }
            let mut dependency = missing_import();
            dependency.key = ProviderLookupKey::SourceBytes { file_id: [99; 16] };
            let mut absent = ProviderRunSupport::from_job_inputs(&job, true);
            absent.partitions[0].dependencies.push(dependency.clone());
            assert_eq!(
                admit_provider_result(job.clone(), complete_with_support(&job, absent))
                    .unwrap_err(),
                ProviderContractError::SupportMismatch,
            );
            dependency.outcome = ProviderLookupOutcome::Incomplete {
                observed_universe: [22; 32],
            };
            let mut incomplete = ProviderRunSupport::from_job_inputs(&job, true);
            incomplete.partitions[0].dependencies.push(dependency);
            let admitted =
                admit_provider_result(job.clone(), complete_with_support(&job, incomplete))
                    .unwrap();
            assert!(admitted.result().support().requires_context_invalidation());
        }
    }

    #[test]
    fn rt_cpg_wp78_support_bundle_identity_is_canonical_and_root_ordered() {
        let first = missing_import();
        // Independent Python rfc8785/blake3 oracle for the externally tagged typed record.
        assert_eq!(
            blake3::Hash::from(
                provider_support_bundle_identity(std::slice::from_ref(&first)).unwrap()
            )
            .to_hex()
            .as_str(),
            "b1a16696c4b7a4dd33de6df184bc71d2567260cc288e6481a6445a85c499732b"
        );
        let mut second = first.clone();
        second.key = ProviderLookupKey::Name {
            name: b"name".to_vec(),
        };
        let baseline = provider_support_bundle_identity(&[first.clone(), second.clone()]).unwrap();
        assert_eq!(
            baseline,
            provider_support_bundle_identity(&[second.clone(), first.clone()]).unwrap()
        );
        second.scope.ordered_roots.reverse();
        assert_ne!(
            baseline,
            provider_support_bundle_identity(&[first.clone(), second]).unwrap()
        );
        let mut contradiction = first.clone();
        contradiction.outcome = ProviderLookupOutcome::Absent {
            closed_universe: [99; 32],
        };
        assert_eq!(
            provider_support_bundle_identity(&[first, contradiction]).unwrap_err(),
            ProviderContractError::SupportMismatch
        );
    }

    fn rebuild_job(job: ProviderJob) -> Result<ProviderJob, ProviderContractError> {
        ProviderJob::try_new(ProviderJobSpec {
            suite: job.suite,
            provider: job.provider,
            protocol: job.protocol,
            source: job.source,
            context: job.context,
            run: job.run,
            lane: job.lane,
            trust: job.trust,
            requests: job.requests.to_vec(),
            ceilings: job.ceilings,
            resource_budget: job.resource_budget,
            deadline: job.deadline,
            cancellation: job.cancellation,
            provenance: job.provenance,
        })
    }

    #[test]
    fn rt_cpg_wp78_shared_support_is_linear_and_charged() {
        let build = |count: u8| {
            let (_, mut job) = job();
            let members = (1..=count)
                .map(|file| input_member(format!("file{file}.py").as_bytes(), file))
                .collect::<Vec<_>>();
            let paths = members
                .iter()
                .map(|member| member.relative_path.clone())
                .collect::<Vec<_>>();
            job.source = ProviderSourceBinding::from_inventory_fixture(
                SourceIdentity::try_new("inventory").unwrap(),
                ProviderSourceInventory::try_new(
                    [6; 16],
                    7,
                    [25; 32],
                    &paths,
                    members,
                    vec![],
                    None,
                )
                .unwrap(),
            );
            let mut requests = job.requests.to_vec();
            requests.push(
                ProviderFamilyRequest::try_new(
                    ProviderFamilyIdentity::try_new("syntax.names").unwrap(),
                    ProviderRelationIdentity::try_new("raw.names").unwrap(),
                    ProviderSchemaIdentity::try_new("raw.names.v1").unwrap(),
                    relation_schema(),
                    ProviderScopeIdentity::try_new("workspace").unwrap(),
                    u64::from(count),
                )
                .unwrap(),
            );
            job.requests = crate::resource_budget::ChargedSlice::for_test(requests);
            rebuild_job(job).unwrap()
        };
        let small = ProviderRunSupport::from_job_inputs(&build(16), true);
        let large_job = build(32);
        let large = ProviderRunSupport::from_job_inputs(&large_job, true);
        assert_eq!(large.common_dependencies().len(), 34);
        assert_eq!(large.partitions.len(), 66);
        assert!(
            large
                .partitions
                .iter()
                .all(|partition| partition.dependencies.is_empty())
        );
        assert!(large.memory_bytes().unwrap() <= 2 * small.memory_bytes().unwrap());
        assert!(Arc::ptr_eq(
            &large.common_dependencies,
            &large.clone().common_dependencies
        ));
        let (_, job) = job();
        let support = ProviderRunSupport::from_job_inputs(&job, true);
        let support_bytes = support.memory_bytes().unwrap();
        let result = complete_with_support(&job, support);
        let arrow_bytes = result
            .relations()
            .iter()
            .flat_map(ProviderRelationOutput::batches)
            .map(|batch| u64::try_from(batch.get_array_memory_size()).unwrap())
            .sum::<u64>();
        assert_eq!(result.resources().bytes, support_bytes + arrow_bytes);
        let mut too_small = job;
        too_small.ceilings.bytes = NonZeroU64::new(support_bytes - 1).unwrap();
        assert_eq!(
            admit_provider_result(too_small, result).unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
    }

    #[test]
    fn rt_cpg_wp78_resource_bounds_precede_support_expansion() {
        assert_eq!(
            ProviderSourceInventory::try_new(
                [6; 16],
                7,
                [25; 32],
                &vec![Vec::new(); 262_145],
                vec![],
                vec![],
                None
            )
            .unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        let oversized = vec![b'x'; 16_385];
        let mut dependency = missing_import();
        dependency.scope.namespace = oversized.clone();
        assert_eq!(
            provider_support_bundle_identity(&[dependency]).unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        let mut dependency = missing_import();
        dependency.outcome = ProviderLookupOutcome::Consumed {
            revision: [1; 32],
            candidates: vec![[1; 16]; 65_537],
        };
        assert_eq!(
            provider_support_bundle_identity(&[dependency]).unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        assert_eq!(
            context_binding()
                .with_modules(vec![ProviderModuleBinding {
                    file_id: [7; 16],
                    qualified_name: "a".into(),
                    relative_path: oversized
                }])
                .unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        let (_, original) = job();
        for resource in 0..3 {
            let mut limited = original.clone();
            match resource {
                0 => limited.ceilings.work_units = NonZeroU64::new(1).unwrap(),
                1 => limited.ceilings.bytes = NonZeroU64::new(1).unwrap(),
                _ => {
                    limited.ceilings.input_bytes = NonZeroU64::new(1).unwrap();
                    limited.source = ProviderSourceBinding::from_inventory_fixture(
                        SourceIdentity::try_new("inventory").unwrap(),
                        ProviderSourceInventory::try_new(
                            [6; 16],
                            7,
                            [25; 32],
                            &[b"a.py".to_vec()],
                            vec![input_member(b"a.py", 7)],
                            vec![],
                            None,
                        )
                        .unwrap(),
                    );
                }
            }
            assert_eq!(
                rebuild_job(limited).unwrap_err(),
                ProviderContractError::ResourceCeilingExceeded
            );
        }
        let mut source_limit = input_member(b"a.py", 7);
        source_limit.disposition = ProviderInputDisposition::Captured {
            file_id: [7; 16],
            digest: [17; 32],
            byte_length: MAX_INPUT_BYTES,
        };
        assert_eq!(
            ProviderSourceInventory::try_new(
                [6; 16],
                7,
                [25; 32],
                &[b"a.py".to_vec(), b"b.py".to_vec()],
                vec![source_limit, input_member(b"b.py", 8)],
                vec![],
                None
            )
            .unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
    }

    fn identity<T>(
        value: &str,
        constructor: impl FnOnce(Arc<str>) -> Result<T, ProviderContractError>,
    ) -> T {
        constructor(Arc::<str>::from(value)).unwrap()
    }

    fn request() -> ProviderFamilyRequest {
        ProviderFamilyRequest::try_new(
            identity("syntax.calls", ProviderFamilyIdentity::try_new),
            identity("raw.calls", ProviderRelationIdentity::try_new),
            identity("raw.calls.v1", ProviderSchemaIdentity::try_new),
            relation_schema(),
            identity("workspace", ProviderScopeIdentity::try_new),
            2,
        )
        .unwrap()
    }

    fn relation_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]))
    }

    fn provenance() -> ProviderRunProvenance {
        ProviderRunProvenance::new(
            identity("tree-sitter-python-0.25", ProviderBuildIdentity::try_new),
            identity("policy.v1", ProviderPolicyIdentity::try_new),
            identity("provider-program.v2.3", ProviderProgramIdentity::try_new),
        )
    }

    fn source_binding() -> ProviderSourceBinding {
        ProviderSourceBinding::try_file(
            identity("source-generation-7", SourceIdentity::try_new),
            [6; 16],
            [7; 16],
            7,
            [17; 32],
        )
        .unwrap()
    }

    fn context_binding() -> ProviderContextBinding {
        ProviderContextBinding::try_new(
            identity("python-context-3", ContextIdentity::try_new),
            [3; 16],
            [3; 32],
            [4; 32],
        )
        .unwrap()
    }

    fn run_binding(value: &str, pin: u8) -> ProviderRunBinding {
        ProviderRunBinding::try_new(identity(value, ProviderRunIdentity::try_new), [pin; 16])
            .unwrap()
    }

    fn ceilings(max_relations: usize, max_rows: u64) -> ProviderResourceCeilings {
        ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
            max_relations,
            max_batches_per_relation: 2,
            max_input_bytes: 65_536,
            max_rows,
            max_bytes: 65_536,
            max_diagnostics: 4,
            max_work_units: 10_000,
            max_wall_millis: 30_000,
            max_visited_nodes: 10_000,
            max_traversal_depth: 128,
            max_workers: 1,
            max_retained_revisions: 2,
            cancellation_poll_work_units: 128,
            cancellation_ack_millis: 2_000,
        })
        .unwrap()
    }

    fn job() -> (CancellationHandle, ProviderJob) {
        let (handle, probe) = CancellationProbe::pair(128).unwrap();
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: identity(
                "codefabric-relational-data-fabric@2.3.0",
                SuiteIdentity::try_new,
            ),
            provider: identity("tree-sitter", ProviderIdentity::try_new),
            protocol: identity("in-process-arrow@1", ProviderProtocolIdentity::try_new),
            source: source_binding(),
            context: crate::provider_contracts::fixture_provider_context(
                [6; 16],
                context_binding(),
            ),
            run: run_binding("run-19", 19),
            lane: ProviderLane::TreeSitter,
            trust: ProviderTrustPosture::InProcessConstrained,
            requests: vec![request()],
            ceilings: ceilings(2, 8),
            resource_budget: fixture_provider_budget([6; 16], [19; 16]),
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation: probe,
            provenance: provenance(),
        })
        .unwrap();
        (handle, job)
    }

    fn relation() -> ProviderRelationOutput {
        let schema = relation_schema();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![1_i64, 2]))],
        )
        .unwrap();
        ProviderRelationOutput::try_new(
            identity("raw.calls", ProviderRelationIdentity::try_new),
            identity("raw.calls.v1", ProviderSchemaIdentity::try_new),
            schema,
            vec![batch],
            &crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
        )
        .unwrap()
    }

    fn result(
        run: &str,
        coverage: ProviderCoverageState,
        terminal: ProviderTerminalStatus,
    ) -> ProviderRunResult {
        let family = identity("syntax.calls", ProviderFamilyIdentity::try_new);
        let gaps = match coverage {
            ProviderCoverageState::Unknown { cause, .. } => vec![
                ProviderGap::try_new(family.clone(), cause, "provider could not finish").unwrap(),
            ],
            _ => Vec::new(),
        };
        ProviderRunResult::try_new(ProviderRunResultSpec {
            resource_budget: crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
            support: ProviderRunSupport::conservative(&job().1),
            suite: identity(
                "codefabric-relational-data-fabric@2.3.0",
                SuiteIdentity::try_new,
            ),
            provider: identity("tree-sitter", ProviderIdentity::try_new),
            protocol: identity("in-process-arrow@1", ProviderProtocolIdentity::try_new),
            source: source_binding(),
            context: crate::provider_contracts::fixture_provider_context(
                [6; 16],
                context_binding(),
            ),
            run: run_binding(run, 19),
            provenance: provenance(),
            relations: vec![relation()],
            coverage: vec![ProviderCoverage::new(family, coverage)],
            gaps,
            diagnostics: Vec::new(),
            trust: ProviderTrustOutcome::Trusted,
            terminal,
        })
        .unwrap()
    }

    #[test]
    fn provider_job_result_terminal_semantics() {
        let (_, job) = job();
        let admitted = admit_provider_result(
            job,
            result(
                "run-19",
                ProviderCoverageState::Complete { completed_units: 2 },
                ProviderTerminalStatus::Complete,
            ),
        )
        .unwrap();
        assert_eq!(admitted.result().resources().rows, 2);
        assert_eq!(admitted.job().requests()[0].scope().as_str(), "workspace");
        assert_eq!(
            admitted.observation(),
            ProviderContractObservation {
                suite: identity(
                    "codefabric-relational-data-fabric@2.3.0",
                    SuiteIdentity::try_new,
                ),
                provider: identity("tree-sitter", ProviderIdentity::try_new),
                run: identity("run-19", ProviderRunIdentity::try_new),
                lane: ProviderLane::TreeSitter,
                requested_families: 1,
                emitted_relations: 1,
                terminal: ProviderTerminalStatus::Complete,
                resources: ProviderResourceOutcome {
                    relations: 1,
                    batches: 1,
                    rows: 2,
                    bytes: admitted.result().resources().bytes,
                    diagnostics: 0,
                },
            }
        );
    }

    #[test]
    fn provider_complete_coverage_requires_real_or_empty_relation() {
        let (_, job) = job();
        let complete = |relations| {
            ProviderRunResult::try_from_job(
                &job,
                ProviderRunEvidenceSpec {
                    support: ProviderRunSupport::conservative(&job),
                    relations,
                    coverage: vec![ProviderCoverage::new(
                        job.requests()[0].family().clone(),
                        ProviderCoverageState::Complete { completed_units: 2 },
                    )],
                    gaps: Vec::new(),
                    diagnostics: Vec::new(),
                    trust: ProviderTrustOutcome::Trusted,
                    terminal: ProviderTerminalStatus::Complete,
                },
            )
            .unwrap()
        };
        assert_eq!(
            admit_provider_result(job.clone(), complete(Vec::new())).unwrap_err(),
            ProviderContractError::MissingCompleteRelation
        );
        let request = &job.requests()[0];
        let empty = ProviderRelationOutput::try_new(
            request.relation().clone(),
            request.schema_identity().clone(),
            Arc::clone(request.schema()),
            vec![RecordBatch::new_empty(Arc::clone(request.schema()))],
            &crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
        )
        .unwrap();
        let admitted = admit_provider_result(job.clone(), complete(vec![empty])).unwrap();
        assert_eq!(admitted.result().resources().relations, 1);
        assert_eq!(admitted.result().resources().rows, 0);
        assert!(admitted.result().gaps().is_empty());
        assert_eq!(
            admitted.result().terminal(),
            ProviderTerminalStatus::Complete
        );
    }

    #[test]
    fn provider_unavailable_requires_unknown_not_empty_success() {
        let (_, job) = job();
        let family = job.requests()[0].family().clone();
        let evidence = ProviderRunEvidenceSpec {
            support: ProviderRunSupport::conservative(&job),
            relations: Vec::new(),
            coverage: vec![ProviderCoverage::new(
                family.clone(),
                ProviderCoverageState::Unknown {
                    completed_units: 0,
                    cause: ProviderUnknownCause::Unsupported,
                },
            )],
            gaps: vec![
                ProviderGap::try_new(
                    family,
                    ProviderUnknownCause::Unsupported,
                    "required extractor is not implemented",
                )
                .unwrap(),
            ],
            diagnostics: Vec::new(),
            trust: ProviderTrustOutcome::Trusted,
            terminal: ProviderTerminalStatus::Unknown,
        };
        let unavailable = ProviderRunResult::try_from_job(&job, evidence.clone()).unwrap();
        let admitted = admit_provider_result(job.clone(), unavailable).unwrap();
        assert!(admitted.result().relations().is_empty());
        assert_eq!(admitted.result().gaps().len(), 1);
        let mut missing_gap = evidence;
        missing_gap.gaps.clear();
        assert_eq!(
            ProviderRunResult::try_from_job(&job, missing_gap).unwrap_err(),
            ProviderContractError::MissingOrContradictoryGap
        );
    }

    #[test]
    fn categorical_run_mismatch_is_rejected() {
        let (_, job) = job();
        let error = admit_provider_result(
            job,
            result(
                "run-20",
                ProviderCoverageState::Complete { completed_units: 2 },
                ProviderTerminalStatus::Complete,
            ),
        )
        .unwrap_err();
        assert_eq!(error, ProviderContractError::IdentityMismatch);
    }

    #[test]
    fn categorical_schema_identity_cannot_mask_an_arrow_schema_mismatch() {
        let (_, job) = job();
        let mut candidate = result(
            "run-19",
            ProviderCoverageState::Complete { completed_units: 2 },
            ProviderTerminalStatus::Complete,
        );
        let different_schema = Arc::new(Schema::new(vec![Field::new(
            "different_value",
            DataType::Int64,
            false,
        )]));
        let different_batch = RecordBatch::try_new(
            Arc::clone(&different_schema),
            vec![Arc::new(Int64Array::from(vec![1_i64, 2]))],
        )
        .unwrap();
        candidate.relations = crate::resource_budget::ChargedSlice::for_test(vec![
            ProviderRelationOutput::try_new(
                identity("raw.calls", ProviderRelationIdentity::try_new),
                identity("raw.calls.v1", ProviderSchemaIdentity::try_new),
                different_schema,
                vec![different_batch],
                &crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
            )
            .unwrap(),
        ]);

        assert_eq!(
            admit_provider_result(job, candidate).unwrap_err(),
            ProviderContractError::ArrowSchemaMismatch
        );
    }

    #[test]
    fn false_complete_coverage_is_rejected() {
        let (_, job) = job();
        let error = admit_provider_result(
            job,
            result(
                "run-19",
                ProviderCoverageState::Complete { completed_units: 1 },
                ProviderTerminalStatus::Complete,
            ),
        )
        .unwrap_err();
        assert_eq!(error, ProviderContractError::FalseCoverage);
    }

    #[test]
    fn unknown_output_requires_an_explicit_matching_gap_and_terminal() {
        let family = identity("syntax.calls", ProviderFamilyIdentity::try_new);
        let error = ProviderRunResult::try_new(ProviderRunResultSpec {
            resource_budget: crate::provider_contracts::fixture_provider_budget([6; 16], [19; 16]),
            support: ProviderRunSupport::conservative(&job().1),
            suite: identity(
                "codefabric-relational-data-fabric@2.3.0",
                SuiteIdentity::try_new,
            ),
            provider: identity("tree-sitter", ProviderIdentity::try_new),
            protocol: identity("in-process-arrow@1", ProviderProtocolIdentity::try_new),
            source: source_binding(),
            context: crate::provider_contracts::fixture_provider_context(
                [6; 16],
                context_binding(),
            ),
            run: run_binding("run-19", 19),
            provenance: provenance(),
            relations: Vec::new(),
            coverage: vec![ProviderCoverage::new(
                family,
                ProviderCoverageState::Unknown {
                    completed_units: 0,
                    cause: ProviderUnknownCause::MissingOutput,
                },
            )],
            gaps: Vec::new(),
            diagnostics: Vec::new(),
            trust: ProviderTrustOutcome::Trusted,
            terminal: ProviderTerminalStatus::Unknown,
        })
        .unwrap_err();
        assert_eq!(error, ProviderContractError::MissingOrContradictoryGap);
    }

    #[test]
    fn cancellation_probe_is_bounded_and_owner_driven() {
        assert_eq!(
            CancellationProbe::pair(MAX_WORK_UNITS_BETWEEN_POLLS + 1).unwrap_err(),
            ProviderContractError::InvalidCancellationProbe
        );
        let (handle, job) = job();
        assert!(!job.cancellation().is_cancelled());
        handle.cancel();
        assert!(job.cancellation().is_cancelled());
        assert_eq!(job.cancellation().max_work_units_between_polls(), 128);
    }

    #[test]
    fn hard_resource_ceiling_cannot_be_widened() {
        assert_eq!(
            ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
                max_relations: MAX_RELATIONS + 1,
                max_batches_per_relation: 1,
                max_input_bytes: 1,
                max_rows: 1,
                max_bytes: 1,
                max_diagnostics: 1,
                max_work_units: 1,
                max_wall_millis: 1,
                max_visited_nodes: 1,
                max_traversal_depth: 1,
                max_workers: 1,
                max_retained_revisions: 1,
                cancellation_poll_work_units: 1,
                cancellation_ack_millis: 1,
            })
            .unwrap_err(),
            ProviderContractError::InvalidResourceCeiling
        );
    }

    #[test]
    fn every_unknown_cause_selects_a_distinct_closed_terminal() {
        let cases = [
            (
                ProviderUnknownCause::MissingOutput,
                ProviderTerminalStatus::Unknown,
            ),
            (
                ProviderUnknownCause::Unsupported,
                ProviderTerminalStatus::Unknown,
            ),
            (
                ProviderUnknownCause::Timeout,
                ProviderTerminalStatus::TimedOut,
            ),
            (
                ProviderUnknownCause::Cancelled,
                ProviderTerminalStatus::Cancelled,
            ),
            (
                ProviderUnknownCause::Corruption,
                ProviderTerminalStatus::Corrupt,
            ),
            (
                ProviderUnknownCause::Oversized,
                ProviderTerminalStatus::Oversized,
            ),
            (
                ProviderUnknownCause::ProviderFailure,
                ProviderTerminalStatus::Failed,
            ),
            (
                ProviderUnknownCause::TrustLoss,
                ProviderTerminalStatus::Failed,
            ),
        ];
        for (cause, expected) in cases {
            let state = ProviderCoverageState::Unknown {
                completed_units: 0,
                cause,
            };
            assert_eq!(terminal_for_coverage([state].iter()), expected);
        }
        let remainder = ProviderCoverageState::IntentionalRemainder {
            completed_units: 1,
            reason: ProviderRemainderReason::BudgetExhausted,
        };
        assert_eq!(
            terminal_for_coverage([remainder].iter()),
            ProviderTerminalStatus::Partial
        );
    }

    #[test]
    fn provider_job_validation_fault_matrix() {
        let (_, mut bounded_job) = job();
        bounded_job.ceilings = ceilings(1, 1);
        let error = admit_provider_result(
            bounded_job,
            result(
                "run-19",
                ProviderCoverageState::Complete { completed_units: 2 },
                ProviderTerminalStatus::Complete,
            ),
        )
        .unwrap_err();
        assert_eq!(error, ProviderContractError::ResourceCeilingExceeded);

        let (_, trusted_job) = job();
        let mut rejected = result(
            "run-19",
            ProviderCoverageState::Complete { completed_units: 2 },
            ProviderTerminalStatus::Complete,
        );
        rejected.trust = ProviderTrustOutcome::Rejected {
            detail: Arc::from("launcher receipt mismatch"),
        };
        assert_eq!(
            admit_provider_result(trusted_job, rejected).unwrap_err(),
            ProviderContractError::RejectedTrust
        );
    }

    #[test]
    fn provider_contract_type_boundary_integrity() {
        let header = RustcCompilationHeader {
            run: identity("run-19", ProviderRunIdentity::try_new),
            compilation_unit: identity("crate-a:lib", RustCompilationUnitIdentity::try_new),
            protocol: identity("rustc-extractor@1", ProviderProtocolIdentity::try_new),
            source: identity("source-generation-7", SourceIdentity::try_new),
            context: identity("rust-toolchain-context", ContextIdentity::try_new),
            compiler_build: identity("rustc-extractor-build", ProviderBuildIdentity::try_new),
            toolchain: identity("nightly-2026-08-18", RustToolchainIdentity::try_new),
            requested_capability_count: 2,
        };
        assert_eq!(header.toolchain.as_str(), "nightly-2026-08-18");

        let owner = RustOwnerIdentity::try_new("owner-a").unwrap();
        let owner_control = RustcOwnerControl::try_new(
            RustcOwnerHeader {
                owner: owner.clone(),
                canonical_owner: CanonicalEntityIdentity::try_new("crate::owner_a").unwrap(),
                expected_relation_count: 1,
            },
            RustcOwnerTerminal {
                owner,
                relation_count: 1,
                row_count: 2,
                coverage: ProviderCoverageState::Complete { completed_units: 1 },
            },
        )
        .unwrap();
        let terminal = RustcCompilationTerminal {
            run: header.run.clone(),
            compilation_unit: header.compilation_unit.clone(),
            compiler_exit_status: 0,
            owner_count: 1,
            relation_count: 1,
            terminal: ProviderTerminalStatus::Complete,
            diagnostics_count: 0,
        };
        let control =
            RustcCompilationControl::try_new(header.clone(), vec![owner_control], terminal.clone())
                .unwrap();
        assert_eq!(control.terminal.owner_count, 1);

        let mut wrong_terminal = terminal;
        wrong_terminal.run = ProviderRunIdentity::try_new("another-run").unwrap();
        assert_eq!(
            RustcCompilationControl::try_new(header, Vec::new(), wrong_terminal).unwrap_err(),
            ProviderContractError::RustcControlMismatch
        );
    }

    #[test]
    fn wp79_provider_job_charge_clones_and_same_name_foreign_root_are_causal() {
        let (_, original) = job();
        let budget = original.resource_budget().clone();
        let charged = budget.observation().used.memory_bytes;
        assert!(charged > 0);
        let clone = original.clone();
        assert_eq!(budget.observation().used.memory_bytes, charged);
        let mut foreign = original.clone();
        let policy = budget.policy();
        foreign.resource_budget = ResourceBudget::try_process([231; 16], policy)
            .unwrap()
            .workspace([6; 16], policy)
            .unwrap()
            .operation([19; 16], policy)
            .unwrap();
        assert!(matches!(
            rebuild_job(foreign),
            Err(ProviderContractError::ResourceOwnerMismatch)
        ));
        drop(original);
        assert_eq!(budget.observation().used.memory_bytes, charged);
        drop(clone);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }
}
