//! Bounded Tree-sitter adapters for Python and Rust complete-CST observations.
//!
//! This is the only production module allowed to traffic in Tree-sitter parser,
//! tree, node, query, or edit types. Everything crossing its public boundary is
//! application-owned data.

use std::collections::{BTreeSet, VecDeque};
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use thiserror::Error;
use tree_sitter::{
    InputEdit, Language, ParseOptions, Parser, Point, Query, QueryCursor, QueryCursorOptions,
    StreamingIterator as _, Tree,
};

use crate::provider_contracts::{
    CancellationProbe, ProviderJob, ProviderLane, ProviderTrustPosture,
};
use crate::provider_raw_kinds::{
    ProviderGrammarInventory, ProviderGrammarKind, ProviderRawKindDisposition,
    ProviderRawKindEntry, TREE_SITTER_PYTHON_GRAMMAR, TREE_SITTER_RECOVERY_QUERY,
    TREE_SITTER_RUST_GRAMMAR, tree_sitter_normalization,
};
use crate::provider_types::{ProviderBoundaryError, ProviderBoundaryMap, ProviderText};

/// Closed language selection for the two Wave-4 complete-CST adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeSitterLanguage {
    Python,
    Rust,
}

impl TreeSitterLanguage {
    fn runtime(self) -> (Language, &'static str, &'static ProviderGrammarInventory) {
        match self {
            Self::Python => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::NODE_TYPES,
                &TREE_SITTER_PYTHON_GRAMMAR,
            ),
            Self::Rust => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::NODE_TYPES,
                &TREE_SITTER_RUST_GRAMMAR,
            ),
        }
    }
}

/// Application-local complete-CST occurrence identity.
///
/// It is a deterministic preorder identity for one immutable parse result, not a
/// canonical graph identity and never a provider-owned node handle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SyntaxOccurrenceId(pub u64);

/// Application-owned normalized syntax-kind registry code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedSyntaxKind(pub u16);

/// Application-owned complete-CST observation.
#[allow(clippy::struct_excessive_bools)] // The four flags are independent Tree-sitter facts required by GEN 7.1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawSyntaxFact {
    pub id: SyntaxOccurrenceId,
    pub raw_kind_id: u16,
    pub raw_kind: String,
    pub normalized_kind: NormalizedSyntaxKind,
    pub disposition: ProviderRawKindDisposition,
    pub start_byte: u64,
    pub end_byte: u64,
    pub named: bool,
    pub extra: bool,
    pub error: bool,
    pub missing: bool,
    pub parent: Option<SyntaxOccurrenceId>,
    pub field_name: Option<String>,
    pub ordinal: u32,
    pub depth: u16,
}

/// One changed byte interval surfaced by incremental Tree-sitter parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChangedRange {
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One exact source edit in provider UTF-8 byte coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeSitterEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
}

impl TreeSitterEdit {
    /// Describe arbitrary captured replacements with one exact UTF-8 boundary-aligned edit.
    /// The unchanged prefix/suffix come from the two authoritative texts, never watcher ranges.
    #[must_use]
    pub fn between(old: &str, new: &str) -> Self {
        let mut start = old
            .bytes()
            .zip(new.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
            start -= 1;
        }
        let mut suffix = old.as_bytes()[start..]
            .iter()
            .rev()
            .zip(new.as_bytes()[start..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix)
        {
            suffix -= 1;
        }
        Self {
            start_byte: start,
            old_end_byte: old.len() - suffix,
            new_end_byte: new.len() - suffix,
        }
    }
}

/// Per-completed-run operational measurements. Durations are observations, not
/// acceptance thresholds or benchmark claims.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeSitterRunMetrics {
    pub parse_duration: Duration,
    pub query_duration: Duration,
    pub parse_work_units: u64,
    pub query_work_units: u64,
    pub visited_nodes: u64,
    pub query_matches: u64,
    pub error_nodes: u64,
    pub missing_nodes: u64,
    pub output_bytes: u64,
    pub work_units: u64,
    pub changed_ranges: u64,
}

/// Aggregate adapter measurements, including rejected candidates that never
/// became the active complete revision.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeSitterAdapterMetrics {
    pub completed_runs: u64,
    pub rejected_runs: u64,
    pub cancelled_runs: u64,
    pub retained_revisions: u16,
    pub last_run: Option<TreeSitterRunMetrics>,
}

/// A complete application-owned parse revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeSitterSnapshot {
    pub revision: u64,
    pub provider_image_fingerprint: String,
    pub catalog_id: &'static str,
    pub grammar_fingerprint: &'static str,
    pub facts: crate::resource_budget::ChargedSlice<RawSyntaxFact>,
    pub changed_ranges: crate::resource_budget::ChargedSlice<ChangedRange>,
    pub metrics: TreeSitterRunMetrics,
}

/// Closed adapter failures; no parser-owned error type escapes this boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TreeSitterAdapterError {
    #[error(transparent)]
    Resource(#[from] crate::provider_contracts::ProviderContractError),
    #[error("Tree-sitter provider version mismatch: {0}")]
    ProviderVersionMismatch(String),
    #[error("Tree-sitter recovery query is invalid: {0}")]
    InvalidQuery(String),
    #[error("provider text boundary map is invalid: {0}")]
    InvalidBoundaryMap(String),
    #[error("incremental edit is invalid: {0}")]
    InvalidEdit(String),
    #[error("source exceeds the provider input limit")]
    InputLimit,
    #[error("Tree-sitter provider was cancelled")]
    Cancelled,
    #[error("Tree-sitter provider exceeded its wall-clock deadline")]
    Deadline,
    #[error("Tree-sitter provider exceeded its work limit")]
    WorkLimit,
    #[error("Tree-sitter provider exceeded its visited-node limit")]
    NodeLimit,
    #[error("Tree-sitter provider exceeded its traversal-depth limit")]
    DepthLimit,
    #[error("Tree-sitter provider exceeded its output-record limit")]
    OutputRecordLimit,
    #[error("Tree-sitter provider exceeded its output-byte limit")]
    OutputByteLimit,
    #[error("Tree-sitter provider exceeded its diagnostic limit")]
    DiagnosticLimit,
    #[error("Tree-sitter recovery query exceeded its match limit")]
    QueryMatchLimit,
    #[error("Tree-sitter returned an out-of-bounds or non-boundary span")]
    InvalidSpan,
    #[error("Tree-sitter parser stopped without a classified limit")]
    ParserStopped,
    #[error("parse revision must advance monotonically")]
    StaleRevision,
}

impl From<ProviderBoundaryError> for TreeSitterAdapterError {
    fn from(error: ProviderBoundaryError) -> Self {
        match error {
            ProviderBoundaryError::InvalidMap(message) => Self::InvalidBoundaryMap(message),
            ProviderBoundaryError::InvalidOffset(_) => Self::InvalidSpan,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TreeSitterLimits {
    max_input_bytes: u64,
    max_work_units: u64,
    max_wall_millis: u64,
    max_visited_nodes: u64,
    max_traversal_depth: u16,
    max_output_records: u64,
    max_output_bytes: u64,
    max_diagnostics: u16,
    max_retained_tree_revisions: u16,
    cancellation_check_interval: u32,
}

impl TreeSitterLimits {
    fn from_job(job: &ProviderJob) -> Result<Self, TreeSitterAdapterError> {
        validate_tree_sitter_job(job)?;
        let ceilings = job.ceilings();
        let remaining = job.remaining().ok_or(TreeSitterAdapterError::Deadline)?;
        let remaining_millis = u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX);
        Ok(Self {
            max_input_bytes: ceilings.max_input_bytes(),
            max_work_units: ceilings.max_work_units(),
            max_wall_millis: ceilings.max_wall_millis().min(remaining_millis),
            max_visited_nodes: ceilings.max_visited_nodes(),
            max_traversal_depth: ceilings.max_traversal_depth(),
            max_output_records: ceilings.max_rows(),
            max_output_bytes: ceilings.max_bytes(),
            max_diagnostics: u16::try_from(ceilings.max_diagnostics()).unwrap_or(u16::MAX),
            max_retained_tree_revisions: ceilings.max_retained_revisions(),
            cancellation_check_interval: u32::try_from(ceilings.cancellation_poll_work_units())
                .unwrap_or(u32::MAX),
        })
    }
}

fn validate_tree_sitter_job(job: &ProviderJob) -> Result<(), TreeSitterAdapterError> {
    if !matches!(
        job.lane(),
        ProviderLane::TreeSitter | ProviderLane::TreeSitterRust
    ) || job.trust() != ProviderTrustPosture::InProcessConstrained
        || job.protocol().as_str() != "in-process-arrow@1"
        || job.requests().is_empty()
    {
        return Err(TreeSitterAdapterError::ProviderVersionMismatch(
            "job is not an exact in-process Tree-sitter invocation".into(),
        ));
    }
    Ok(())
}

fn tree_sitter_raw_kind_entry(
    language: &Language,
    inventory: &ProviderGrammarInventory,
    raw_kind_id: u16,
) -> Option<ProviderRawKindEntry> {
    let raw_name = language.node_kind_for_id(raw_kind_id)?.to_owned();
    let (disposition, normalized_kind_code) =
        tree_sitter_normalization(inventory.grammar, &raw_name);
    Some(ProviderRawKindEntry {
        raw_kind_id,
        raw_name,
        named: language.node_kind_is_named(raw_kind_id),
        visible: language.node_kind_is_visible(raw_kind_id),
        supertype: language.node_kind_is_supertype(raw_kind_id),
        disposition,
        normalized_kind_code,
    })
}

struct RetainedRevision {
    revision: u64,
    text: ProviderText,
    tree: Tree,
    snapshot: TreeSitterSnapshot,
    // Dropped after its native tree and DTO owners, never on mere result observation.
    native_envelope: crate::resource_budget::ResourceReservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AbortReason {
    Cancelled,
    Deadline,
    Work,
}

fn progress_abort_reason(
    work_units: u64,
    max_work_units: u64,
    deadline_exceeded: bool,
    callbacks: u32,
    check_interval: u32,
    cancelled: bool,
) -> Option<AbortReason> {
    if exceeds_limit(work_units, max_work_units) {
        Some(AbortReason::Work)
    } else if deadline_exceeded {
        Some(AbortReason::Deadline)
    } else if cancellation_due(u64::from(callbacks), u64::from(check_interval), cancelled) {
        Some(AbortReason::Cancelled)
    } else {
        None
    }
}

const fn exceeds_limit(value: u64, maximum: u64) -> bool {
    value > maximum
}

fn deadline_exceeded(elapsed: Duration, maximum_millis: u64) -> bool {
    elapsed > Duration::from_millis(maximum_millis)
}

fn cancellation_due(work_units: u64, interval: u64, cancelled: bool) -> bool {
    work_units.is_multiple_of(interval.max(1)) && cancelled
}

fn runtime_node_matches(
    raw_name: &str,
    named: bool,
    expected_name: &str,
    expected_named: bool,
) -> bool {
    raw_name == expected_name && named == expected_named
}

impl AbortReason {
    const fn error(self) -> TreeSitterAdapterError {
        match self {
            Self::Cancelled => TreeSitterAdapterError::Cancelled,
            Self::Deadline => TreeSitterAdapterError::Deadline,
            Self::Work => TreeSitterAdapterError::WorkLimit,
        }
    }
}

/// One worker-owned parser/query-cursor pair and its bounded revision cache.
pub struct TreeSitterAdapter {
    parser: Parser,
    language: Language,
    query: Query,
    query_cursor: QueryCursor,
    inventory: &'static ProviderGrammarInventory,
    retained: VecDeque<RetainedRevision>,
    metrics: TreeSitterAdapterMetrics,
    // Covers parser/query/cursor state, including capacity retained after a failed parse.
    native_envelope: crate::resource_budget::ResourceReservation,
}

impl TreeSitterAdapter {
    /// Construct and fully validate the selected exact-pinned runtime grammar before accepting
    /// source.
    ///
    /// # Errors
    ///
    /// Returns a version mismatch for any ABI, node, field, source metadata, or
    /// fingerprint drift, and `InvalidQuery` if the governed recovery query no
    /// longer compiles for the exact grammar.
    pub(crate) fn new(
        language_choice: TreeSitterLanguage,
        job: &ProviderJob,
    ) -> Result<Self, TreeSitterAdapterError> {
        let native_envelope = crate::provider_contracts::allocation::reserve_native_state(job)?;
        let (language, node_types, inventory) = language_choice.runtime();
        validate_runtime_inventory(&language, node_types, inventory)?;
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|error| TreeSitterAdapterError::ProviderVersionMismatch(error.to_string()))?;
        let query = Query::new(&language, TREE_SITTER_RECOVERY_QUERY)
            .map_err(|error| TreeSitterAdapterError::InvalidQuery(error.to_string()))?;
        Ok(Self {
            parser,
            language,
            query,
            query_cursor: QueryCursor::new(),
            inventory,
            retained: VecDeque::new(),
            metrics: TreeSitterAdapterMetrics::default(),
            native_envelope,
        })
    }

    /// Parse and atomically commit a complete source revision without reuse.
    ///
    /// # Errors
    ///
    /// Rejects invalid source mappings and every configured resource, deadline,
    /// cancellation, version, or output-limit breach. A rejection never changes
    /// the active revision.
    pub fn parse_full(
        &mut self,
        job: &ProviderJob,
        revision: u64,
        text: ProviderText,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        for owner in [
            self.native_envelope.owner(),
            text.text.reservation().owner(),
            text.original_byte_offsets.reservation().owner(),
        ] {
            crate::provider_contracts::allocation::require_native_workspace(
                owner,
                job.resource_budget(),
            )?;
        }
        let envelope = crate::provider_contracts::allocation::reserve_native_state(job)?;
        self.parse_candidate(job, revision, text, None, envelope)
    }

    /// Apply one exact edit to the active tree, parse incrementally, surface
    /// changed ranges, and atomically commit only the validated complete result.
    ///
    /// # Errors
    ///
    /// In addition to `parse_full` failures, rejects stale revisions and edits
    /// whose unchanged prefix/suffix do not match the active source.
    pub fn parse_incremental(
        &mut self,
        job: &ProviderJob,
        revision: u64,
        text: ProviderText,
        edit: TreeSitterEdit,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        for owner in [
            self.native_envelope.owner(),
            text.text.reservation().owner(),
            text.original_byte_offsets.reservation().owner(),
        ] {
            crate::provider_contracts::allocation::require_native_workspace(
                owner,
                job.resource_budget(),
            )?;
        }
        let envelope = crate::provider_contracts::allocation::reserve_native_state(job)?;
        let prior = self
            .retained
            .back()
            .ok_or_else(|| TreeSitterAdapterError::InvalidEdit("no active revision".into()))?;
        if revision <= prior.revision {
            return self.reject(TreeSitterAdapterError::StaleRevision);
        }
        validate_edit(&prior.text.text, &text.text, edit)?;
        let mut edited_tree = prior.tree.clone();
        edited_tree.edit(&InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: point_at(&prior.text.text, edit.start_byte)?,
            old_end_position: point_at(&prior.text.text, edit.old_end_byte)?,
            new_end_position: point_at(&text.text, edit.new_end_byte)?,
        });
        self.parse_candidate(job, revision, text, Some(&edited_tree), envelope)
    }

    /// Parse captured text using an exact edit against this file's last complete native tree.
    /// The caller owns one adapter per compatible file/context and drops it after a failed run.
    ///
    /// # Errors
    /// Returns the same admission/cancellation/parser failures as full parsing, or revision overflow.
    pub fn parse_captured(
        &mut self,
        job: &ProviderJob,
        text: ProviderText,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        if let Some(prior) = self.retained.back() {
            let revision = prior
                .revision
                .checked_add(1)
                .ok_or(TreeSitterAdapterError::StaleRevision)?;
            let edit = TreeSitterEdit::between(&prior.text.text, &text.text);
            self.parse_incremental(job, revision, text, edit)
        } else {
            self.parse_full(job, 1, text)
        }
    }

    /// Last atomically committed complete revision.
    #[must_use]
    pub fn active_snapshot(&self) -> Option<&TreeSitterSnapshot> {
        self.retained.back().map(|revision| &revision.snapshot)
    }

    /// Declared native reservations retained by this owner, separately from charged DTO backing.
    pub(crate) fn native_reservations(&self) -> crate::resource_budget::ResourceAmounts {
        self.retained
            .iter()
            .fold(self.native_envelope.amounts(), |mut cost, revision| {
                let native = revision.native_envelope.amounts();
                cost.memory_bytes = cost.memory_bytes.saturating_add(native.memory_bytes);
                cost.retained_bytes = cost.retained_bytes.saturating_add(native.retained_bytes);
                cost.retained_generations = cost
                    .retained_generations
                    .saturating_add(native.retained_generations);
                cost
            })
    }

    /// Current operational counters.
    #[must_use]
    pub const fn metrics(&self) -> TreeSitterAdapterMetrics {
        self.metrics
    }

    /// The exact application-owned grammar identity validated at startup.
    #[must_use]
    pub const fn inventory(&self) -> &'static ProviderGrammarInventory {
        self.inventory
    }

    fn reject<T>(&mut self, error: TreeSitterAdapterError) -> Result<T, TreeSitterAdapterError> {
        self.metrics.rejected_runs = self.metrics.rejected_runs.saturating_add(1);
        if error == TreeSitterAdapterError::Cancelled {
            self.parser.reset();
            self.metrics.cancelled_runs = self.metrics.cancelled_runs.saturating_add(1);
        }
        Err(error)
    }

    #[allow(clippy::too_many_lines)] // One candidate transaction keeps partial parser output from escaping.
    fn parse_candidate(
        &mut self,
        job: &ProviderJob,
        revision: u64,
        text: ProviderText,
        edited_old_tree: Option<&Tree>,
        native_envelope: crate::resource_budget::ResourceReservation,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        crate::provider_contracts::allocation::require_native_workspace(
            text.text.reservation().owner(),
            job.resource_budget(),
        )?;
        crate::provider_contracts::allocation::require_native_workspace(
            text.original_byte_offsets.reservation().owner(),
            job.resource_budget(),
        )?;
        let limits = TreeSitterLimits::from_job(job)?;
        crate::provider_contracts::allocation::require_native_workspace(
            self.native_envelope.owner(),
            job.resource_budget(),
        )?;
        let mut dto_allocation =
            crate::provider_contracts::allocation::ProviderAllocation::try_new(
                job.resource_budget(),
                job.ceilings().max_bytes(),
            )?;
        let cancellation = job.cancellation();
        if self
            .retained
            .back()
            .is_some_and(|active| revision <= active.revision)
        {
            return self.reject(TreeSitterAdapterError::StaleRevision);
        }
        if u64::try_from(text.text.len()).unwrap_or(u64::MAX) > limits.max_input_bytes {
            return self.reject(TreeSitterAdapterError::InputLimit);
        }
        let boundaries = match ProviderBoundaryMap::new(&text) {
            Ok(boundaries) => boundaries,
            Err(error) => return self.reject(error.into()),
        };
        if cancellation.is_cancelled() {
            self.parser.reset();
            return self.reject(TreeSitterAdapterError::Cancelled);
        }

        let started = Instant::now();
        let mut work_units = 0_u64;
        let mut abort_reason = None;
        let check_interval = cancellation
            .max_work_units_between_polls()
            .try_into()
            .unwrap_or(u32::MAX)
            .min(limits.cancellation_check_interval)
            .max(1);
        let mut callbacks = 0_u32;
        let bytes = text.text.as_bytes();
        let tree = {
            let mut progress = |_: &tree_sitter::ParseState| {
                work_units = work_units.saturating_add(1);
                callbacks = callbacks.saturating_add(1);
                let reason = progress_abort_reason(
                    work_units,
                    limits.max_work_units,
                    deadline_exceeded(started.elapsed(), limits.max_wall_millis),
                    callbacks,
                    check_interval,
                    cancellation.is_cancelled(),
                );
                if let Some(reason) = reason {
                    abort_reason = Some(reason);
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            };
            let mut reader = |offset: usize, _: Point| bytes.get(offset..).unwrap_or_default();
            self.parser.parse_with_options(
                &mut reader,
                edited_old_tree,
                Some(ParseOptions::new().progress_callback(&mut progress)),
            )
        };
        let parse_duration = started.elapsed();
        let Some(tree) = tree else {
            self.parser.reset();
            return self.reject(
                abort_reason.map_or(TreeSitterAdapterError::ParserStopped, AbortReason::error),
            );
        };
        if let Some(reason) = abort_reason {
            self.parser.reset();
            return self.reject(reason.error());
        }
        if cancellation.is_cancelled() {
            self.parser.reset();
            return self.reject(TreeSitterAdapterError::Cancelled);
        }

        let changed_ranges = match edited_old_tree {
            Some(old_tree) => {
                let ranges = old_tree
                    .changed_ranges(&tree)
                    .map(|range| {
                        Ok(ChangedRange {
                            start_byte: boundaries.original(range.start_byte)?,
                            end_byte: boundaries.original(range.end_byte)?,
                        })
                    })
                    .collect::<Result<Vec<_>, TreeSitterAdapterError>>();
                match ranges {
                    Ok(ranges) => ranges,
                    Err(error) => return self.reject(error),
                }
            }
            None => Vec::new(),
        };
        let (facts, mut metrics, recovery_nodes) = match walk_tree(
            &tree,
            &self.language,
            self.inventory,
            &boundaries,
            limits,
            cancellation,
            started,
            work_units,
        ) {
            Ok(result) => result,
            Err(error) => return self.reject(error),
        };
        let query_started = Instant::now();
        let pre_query_work_units = metrics.work_units;
        let query_result = run_recovery_query(
            &mut self.query_cursor,
            &self.query,
            &tree,
            &text.text,
            &recovery_nodes,
            limits,
            cancellation,
            started,
            &mut metrics.work_units,
        );
        metrics.query_duration = query_started.elapsed();
        metrics.query_work_units = metrics.work_units.saturating_sub(pre_query_work_units);
        if let Err(error) = query_result {
            return self.reject(error);
        }
        metrics.query_matches = u64::try_from(recovery_nodes.len()).unwrap_or(u64::MAX);
        metrics.parse_duration = parse_duration;
        metrics.changed_ranges = u64::try_from(changed_ranges.len()).unwrap_or(u64::MAX);
        let snapshot = TreeSitterSnapshot {
            revision,
            provider_image_fingerprint: text.provider_image_fingerprint(),
            catalog_id: self.inventory.catalog_id,
            grammar_fingerprint: self.inventory.runtime_inventory_fingerprint,
            facts: dto_allocation.retain_measured_vec(facts, |fact| {
                fact.raw_kind
                    .capacity()
                    .saturating_add(fact.field_name.as_ref().map_or(0, String::capacity))
            })?,
            changed_ranges: dto_allocation.retain_measured_vec(changed_ranges, |_| 0)?,
            metrics,
        };
        self.retained.push_back(RetainedRevision {
            revision,
            text,
            tree,
            snapshot: snapshot.clone(),
            native_envelope,
        });
        while exceeds_limit(
            u64::try_from(self.retained.len()).unwrap_or(u64::MAX),
            u64::from(limits.max_retained_tree_revisions),
        ) {
            self.retained.pop_front();
        }
        self.metrics.completed_runs = self.metrics.completed_runs.saturating_add(1);
        self.metrics.retained_revisions = u16::try_from(self.retained.len()).unwrap_or(u16::MAX);
        self.metrics.last_run = Some(metrics);
        Ok(snapshot)
    }
}

fn validate_runtime_inventory(
    language: &Language,
    node_types: &str,
    inventory: &ProviderGrammarInventory,
) -> Result<(), TreeSitterAdapterError> {
    let expected = match inventory.grammar {
        ProviderGrammarKind::Python => &TREE_SITTER_PYTHON_GRAMMAR,
        ProviderGrammarKind::Rust => &TREE_SITTER_RUST_GRAMMAR,
    };
    if inventory != expected {
        return Err(version_mismatch("application grammar identity"));
    }
    if language.abi_version() != inventory.grammar_abi {
        return Err(version_mismatch("grammar ABI"));
    }
    if language.node_kind_count() == 0 || language.node_kind_count() > usize::from(u16::MAX) {
        return Err(version_mismatch("live raw kind count"));
    }
    for id in 0..language.node_kind_count() {
        let id = u16::try_from(id).map_err(|_| version_mismatch("live raw kind ID"))?;
        let entry = tree_sitter_raw_kind_entry(language, inventory, id)
            .ok_or_else(|| version_mismatch("live raw kind"))?;
        if entry.raw_name.is_empty() || entry.raw_kind_id != id {
            return Err(version_mismatch("live raw kind identity"));
        }
    }
    for (id, expected_name) in [(u16::MAX - 1, "_ERROR"), (u16::MAX, "ERROR")] {
        let entry = tree_sitter_raw_kind_entry(language, inventory, id)
            .ok_or_else(|| version_mismatch("live recovery kind"))?;
        if entry.raw_name != expected_name {
            return Err(version_mismatch("live recovery kind identity"));
        }
    }
    if language.field_count() == 0 || language.field_count() > usize::from(u16::MAX) {
        return Err(version_mismatch("live field count"));
    }
    for id in 1..=language.field_count() {
        let id = u16::try_from(id).map_err(|_| version_mismatch("live field ID"))?;
        let name = language
            .field_name_for_id(id)
            .ok_or_else(|| version_mismatch("live field"))?;
        if name.is_empty()
            || language
                .field_id_for_name(name)
                .is_none_or(|observed| observed.get() != id)
        {
            return Err(version_mismatch("live field identity"));
        }
    }
    if checksum(node_types.as_bytes()) != inventory.node_types_digest {
        return Err(version_mismatch("NODE_TYPES digest"));
    }
    if checksum(TREE_SITTER_RECOVERY_QUERY.as_bytes()) != inventory.recovery_query_digest {
        return Err(version_mismatch("recovery query digest"));
    }
    Ok(())
}

fn version_mismatch(part: &str) -> TreeSitterAdapterError {
    TreeSitterAdapterError::ProviderVersionMismatch(part.to_owned())
}

fn checksum(bytes: &[u8]) -> String {
    crate::integrity::framed_digest(bytes)
}

type RecoveryNode = (usize, usize, bool, bool, u16);

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // Iterative cursor ownership and all budget checks remain in one traversal.
fn walk_tree(
    tree: &Tree,
    language: &Language,
    inventory: &ProviderGrammarInventory,
    boundaries: &ProviderBoundaryMap,
    limits: TreeSitterLimits,
    cancellation: &CancellationProbe,
    started: Instant,
    initial_work_units: u64,
) -> Result<
    (
        Vec<RawSyntaxFact>,
        TreeSitterRunMetrics,
        BTreeSet<RecoveryNode>,
    ),
    TreeSitterAdapterError,
> {
    let mut facts = Vec::new();
    let mut recovery_nodes = BTreeSet::new();
    let mut cursor = tree.walk();
    let mut parent_ids = Vec::new();
    let mut ordinals = vec![0_u32];
    let mut depth = 0_u16;
    let mut metrics = TreeSitterRunMetrics {
        work_units: initial_work_units,
        parse_work_units: initial_work_units,
        ..TreeSitterRunMetrics::default()
    };
    loop {
        metrics.visited_nodes = metrics.visited_nodes.saturating_add(1);
        metrics.work_units = metrics.work_units.saturating_add(1);
        if exceeds_limit(metrics.visited_nodes, limits.max_visited_nodes) {
            return Err(TreeSitterAdapterError::NodeLimit);
        }
        if exceeds_limit(metrics.work_units, limits.max_work_units) {
            return Err(TreeSitterAdapterError::WorkLimit);
        }
        if exceeds_limit(u64::from(depth), u64::from(limits.max_traversal_depth)) {
            return Err(TreeSitterAdapterError::DepthLimit);
        }
        if deadline_exceeded(started.elapsed(), limits.max_wall_millis) {
            return Err(TreeSitterAdapterError::Deadline);
        }
        if cancellation_due(
            metrics.visited_nodes,
            u64::try_from(cancellation.max_work_units_between_polls()).unwrap_or(u64::MAX),
            cancellation.is_cancelled(),
        ) {
            return Err(TreeSitterAdapterError::Cancelled);
        }
        let node = cursor.node();
        let entry = tree_sitter_raw_kind_entry(language, inventory, node.kind_id())
            .ok_or_else(|| version_mismatch("runtime node kind absent"))?;
        if !runtime_node_matches(node.kind(), node.is_named(), &entry.raw_name, entry.named) {
            return Err(version_mismatch("runtime node identity"));
        }
        let start_byte = boundaries.original(node.start_byte())?;
        let end_byte = boundaries.original(node.end_byte())?;
        if start_byte > end_byte {
            return Err(TreeSitterAdapterError::InvalidSpan);
        }
        let id = SyntaxOccurrenceId(
            u64::try_from(facts.len())
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        );
        let error = node.is_error();
        let missing = node.is_missing();
        if error {
            metrics.error_nodes = metrics.error_nodes.saturating_add(1);
        }
        if missing {
            metrics.missing_nodes = metrics.missing_nodes.saturating_add(1);
        }
        if error || missing {
            recovery_nodes.insert((
                node.start_byte(),
                node.end_byte(),
                error,
                missing,
                node.kind_id(),
            ));
        }
        if exceeds_limit(
            metrics.error_nodes.saturating_add(metrics.missing_nodes),
            u64::from(limits.max_diagnostics),
        ) {
            return Err(TreeSitterAdapterError::DiagnosticLimit);
        }
        let field_name = cursor.field_name().map(str::to_owned);
        let raw_kind = entry.raw_name;
        let fact_bytes = u64::try_from(
            std::mem::size_of::<RawSyntaxFact>()
                .saturating_add(raw_kind.len())
                .saturating_add(field_name.as_ref().map_or(0, String::len)),
        )
        .unwrap_or(u64::MAX);
        metrics.output_bytes = metrics.output_bytes.saturating_add(fact_bytes);
        if exceeds_limit(metrics.output_bytes, limits.max_output_bytes) {
            return Err(TreeSitterAdapterError::OutputByteLimit);
        }
        facts.push(RawSyntaxFact {
            id,
            raw_kind_id: entry.raw_kind_id,
            raw_kind,
            normalized_kind: NormalizedSyntaxKind(entry.normalized_kind_code),
            disposition: entry.disposition,
            start_byte,
            end_byte,
            named: node.is_named(),
            extra: node.is_extra(),
            error,
            missing,
            parent: parent_ids.last().copied(),
            field_name,
            ordinal: *ordinals.last().expect("root ordinal exists"),
            depth,
        });
        if exceeds_limit(
            u64::try_from(facts.len()).unwrap_or(u64::MAX),
            limits.max_output_records,
        ) {
            return Err(TreeSitterAdapterError::OutputRecordLimit);
        }

        if cursor.goto_first_child() {
            parent_ids.push(id);
            ordinals.push(0);
            depth = depth.saturating_add(1);
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                let ordinal = ordinals.last_mut().expect("current ordinal exists");
                *ordinal = ordinal.saturating_add(1);
                break;
            }
            if !cursor.goto_parent() {
                return Ok((facts, metrics, recovery_nodes));
            }
            parent_ids.pop();
            ordinals.pop();
            depth = depth.saturating_sub(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_recovery_query(
    cursor: &mut QueryCursor,
    query: &Query,
    tree: &Tree,
    text: &str,
    expected: &BTreeSet<RecoveryNode>,
    limits: TreeSitterLimits,
    cancellation: &CancellationProbe,
    started: Instant,
    work_units: &mut u64,
) -> Result<(), TreeSitterAdapterError> {
    cursor.set_match_limit(
        u32::try_from(u64::from(limits.max_diagnostics).saturating_add(1)).unwrap_or(u32::MAX),
    );
    cursor.set_byte_range(0..text.len());
    cursor.set_max_start_depth(Some(u32::from(limits.max_traversal_depth)));
    let mut abort_reason = None;
    let mut iteration_abort_reason = None;
    let mut callback_work_units = *work_units;
    let mut iteration_work_units = 0_u64;
    let mut callbacks = 0_u32;
    let check_interval = cancellation
        .max_work_units_between_polls()
        .try_into()
        .unwrap_or(u32::MAX)
        .min(limits.cancellation_check_interval)
        .max(1);
    let mut found = BTreeSet::new();
    {
        let mut progress = |_: &tree_sitter::QueryCursorState| {
            callback_work_units = callback_work_units.saturating_add(1);
            callbacks = callbacks.saturating_add(1);
            let reason = progress_abort_reason(
                callback_work_units,
                limits.max_work_units,
                deadline_exceeded(started.elapsed(), limits.max_wall_millis),
                callbacks,
                check_interval,
                cancellation.is_cancelled(),
            );
            if let Some(reason) = reason {
                abort_reason = Some(reason);
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = QueryCursorOptions::new().progress_callback(&mut progress);
        let mut matches =
            cursor.matches_with_options(query, tree.root_node(), text.as_bytes(), options);
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let node = capture.node;
                found.insert((
                    node.start_byte(),
                    node.end_byte(),
                    node.is_error(),
                    node.is_missing(),
                    node.kind_id(),
                ));
            }
            iteration_work_units = iteration_work_units.saturating_add(1);
            if exceeds_limit(iteration_work_units, limits.max_work_units) {
                iteration_abort_reason = Some(AbortReason::Work);
                break;
            }
        }
    }
    *work_units = callback_work_units.saturating_add(iteration_work_units);
    if let Some(reason) = abort_reason.or(iteration_abort_reason) {
        return Err(reason.error());
    }
    if cursor.did_exceed_match_limit() {
        return Err(TreeSitterAdapterError::QueryMatchLimit);
    }
    if cancellation.is_cancelled() {
        return Err(TreeSitterAdapterError::Cancelled);
    }
    if &found != expected {
        return Err(version_mismatch("recovery query/traversal disagreement"));
    }
    Ok(())
}

fn validate_edit(old: &str, new: &str, edit: TreeSitterEdit) -> Result<(), TreeSitterAdapterError> {
    if !edit_geometry_valid(old.len(), new.len(), edit) {
        return Err(invalid_edit());
    }
    if !edit_boundaries_valid(old, new, edit) {
        return Err(invalid_edit());
    }
    if !edit_unchanged_regions_match(old, new, edit) {
        return Err(invalid_edit());
    }
    Ok(())
}

fn invalid_edit() -> TreeSitterAdapterError {
    TreeSitterAdapterError::InvalidEdit(
        "edit does not describe the exact old/new source delta".into(),
    )
}

const fn edit_geometry_valid(old_length: usize, new_length: usize, edit: TreeSitterEdit) -> bool {
    edit.start_byte <= edit.old_end_byte
        && edit.start_byte <= edit.new_end_byte
        && edit.old_end_byte <= old_length
        && edit.new_end_byte <= new_length
}

fn edit_boundaries_valid(old: &str, new: &str, edit: TreeSitterEdit) -> bool {
    old.is_char_boundary(edit.start_byte)
        && old.is_char_boundary(edit.old_end_byte)
        && new.is_char_boundary(edit.start_byte)
        && new.is_char_boundary(edit.new_end_byte)
}

fn edit_unchanged_regions_match(old: &str, new: &str, edit: TreeSitterEdit) -> bool {
    old.get(..edit.start_byte) == new.get(..edit.start_byte)
        && old.get(edit.old_end_byte..) == new.get(edit.new_end_byte..)
}

fn point_at(text: &str, byte: usize) -> Result<Point, TreeSitterAdapterError> {
    let prefix = text
        .get(..byte)
        .ok_or_else(|| TreeSitterAdapterError::InvalidEdit("edit splits UTF-8".into()))?;
    let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column = prefix
        .rfind('\n')
        .map_or(prefix.len(), |last_newline| prefix.len() - last_newline - 1);
    Ok(Point { row, column })
}

#[cfg(test)]
mod job_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::provider_contracts::{
        CancellationHandle, CancellationProbe, ContextIdentity, ProviderBuildIdentity,
        ProviderContextBinding, ProviderFamilyIdentity, ProviderFamilyRequest, ProviderIdentity,
        ProviderJobSpec, ProviderPolicyIdentity, ProviderProgramIdentity, ProviderProtocolIdentity,
        ProviderRelationIdentity, ProviderResourceCeilingSpec, ProviderResourceCeilings,
        ProviderRunBinding, ProviderRunIdentity, ProviderRunProvenance, ProviderSchemaIdentity,
        ProviderScopeIdentity, ProviderSourceBinding, SourceIdentity, SuiteIdentity,
    };

    fn provider_text(text: &str) -> ProviderText {
        ProviderText::from_validated_utf8(
            text,
            &crate::provider_contracts::fixture_provider_budget([6; 16], [254; 16]),
        )
        .unwrap()
    }

    fn limits() -> ProviderResourceCeilingSpec {
        ProviderResourceCeilingSpec {
            max_relations: 8,
            max_batches_per_relation: 8,
            max_input_bytes: 1 << 20,
            max_rows: 100_000,
            max_bytes: 1 << 24,
            max_diagnostics: 1_000,
            max_work_units: 1_000_000,
            max_wall_millis: 30_000,
            max_visited_nodes: 100_000,
            max_traversal_depth: 256,
            max_workers: 1,
            max_retained_revisions: 2,
            cancellation_poll_work_units: 1,
            cancellation_ack_millis: 2_000,
        }
    }

    fn job(spec: ProviderResourceCeilingSpec) -> (CancellationHandle, ProviderJob) {
        job_for_lane(spec, ProviderLane::TreeSitter)
    }

    fn job_for_lane(
        spec: ProviderResourceCeilingSpec,
        lane: ProviderLane,
    ) -> (CancellationHandle, ProviderJob) {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let (owner, cancellation) =
            CancellationProbe::pair(spec.cancellation_poll_work_units).unwrap();
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3.0").unwrap(),
            provider: ProviderIdentity::try_new("tree-sitter-python").unwrap(),
            protocol: ProviderProtocolIdentity::try_new("in-process-arrow@1").unwrap(),
            source: ProviderSourceBinding::try_file(
                SourceIdentity::try_new("source-1").unwrap(),
                [6; 16],
                [1; 16],
                1,
                [2; 32],
            )
            .unwrap(),
            context: crate::provider_contracts::fixture_provider_context(
                [6; 16],
                ProviderContextBinding::try_new(
                    ContextIdentity::try_new("context-1").unwrap(),
                    [3; 16],
                    [3; 32],
                    [4; 32],
                )
                .unwrap(),
            ),
            run: ProviderRunBinding::try_new(
                ProviderRunIdentity::try_new("tree-run-1").unwrap(),
                [5; 16],
            )
            .unwrap(),
            lane,
            trust: ProviderTrustPosture::InProcessConstrained,
            requests: vec![
                ProviderFamilyRequest::try_new(
                    ProviderFamilyIdentity::try_new("tree-sitter.cst").unwrap(),
                    ProviderRelationIdentity::try_new("provider.tree_sitter.cst_node").unwrap(),
                    ProviderSchemaIdentity::try_new("tree-sitter.cst.schema").unwrap(),
                    schema,
                    ProviderScopeIdentity::try_new("source-1").unwrap(),
                    1,
                )
                .unwrap(),
            ],
            ceilings: ProviderResourceCeilings::try_new(spec).unwrap(),
            resource_budget: crate::provider_contracts::fixture_provider_budget([6; 16], [5; 16]),
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation,
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new("tree-sitter=0.26.12").unwrap(),
                ProviderPolicyIdentity::try_new("policy.v2.3").unwrap(),
                ProviderProgramIdentity::try_new("provider-program.v2.3").unwrap(),
            ),
        })
        .unwrap();
        (owner, job)
    }

    #[test]
    fn tree_sitter_job_drives_exact_parse_and_bounded_revisions() {
        let (_, first_job) = job(limits());
        let mut adapter = TreeSitterAdapter::new(TreeSitterLanguage::Python, &first_job).unwrap();
        let first = adapter
            .parse_full(&first_job, 1, provider_text("value = 1\n"))
            .unwrap();
        assert!(!first.facts.is_empty());

        let (_, second_job) = job(limits());
        let second = adapter
            .parse_incremental(
                &second_job,
                2,
                provider_text("value = 2\n"),
                TreeSitterEdit {
                    start_byte: 8,
                    old_end_byte: 9,
                    new_end_byte: 9,
                },
            )
            .unwrap();
        assert_eq!(second.revision, 2);
        assert!(adapter.metrics().retained_revisions <= 2);
    }

    #[test]
    fn captured_unicode_and_disjoint_edits_match_independent_full_trees() {
        let (_, selected) = job(limits());
        let mut retained = TreeSitterAdapter::new(TreeSitterLanguage::Python, &selected).unwrap();
        for (index, source) in [
            "def café():\r\n    return 'α'\r\n",
            "def cafè():\r\n    return 'β'\r\n",
            "# prefix\nvalue = 3\ndef cafè():\n    return value\n",
            "# prefix\nvalue = 4\ndef cafè():\n    return value\n",
            "# prefix\nvalue = 4\ndef cafè():\n    return value\n",
            "",
            "def restored():\n    return 1\n",
        ]
        .into_iter()
        .enumerate()
        {
            let incremental = retained
                .parse_captured(&selected, provider_text(source))
                .unwrap();
            let full = TreeSitterAdapter::new(TreeSitterLanguage::Python, &selected)
                .unwrap()
                .parse_full(&selected, 1, provider_text(source))
                .unwrap();
            assert_eq!(&*incremental.facts, &*full.facts, "captured edit {index}");
            assert_eq!(
                incremental.provider_image_fingerprint,
                full.provider_image_fingerprint
            );
            assert_eq!(incremental.revision, index as u64 + 1);
        }
    }

    #[test]
    fn tree_sitter_job_limits_and_cancellation_are_causal() {
        let (owner, cancelled_job) = job(limits());
        let mut adapter =
            TreeSitterAdapter::new(TreeSitterLanguage::Python, &cancelled_job).unwrap();
        owner.cancel();
        assert_eq!(
            adapter.parse_full(&cancelled_job, 1, provider_text("value = 1\n")),
            Err(TreeSitterAdapterError::Cancelled)
        );

        let mut tiny = limits();
        tiny.max_input_bytes = 1;
        let (_, tiny_job) = job(tiny);
        assert_eq!(
            adapter.parse_full(&tiny_job, 1, provider_text("value = 1\n")),
            Err(TreeSitterAdapterError::InputLimit)
        );
    }

    #[test]
    fn tree_sitter_job_rejects_wrong_lane_before_library_execution() {
        let (_, wrong) = job_for_lane(limits(), ProviderLane::Ruff);
        assert!(matches!(
            TreeSitterAdapter::new(TreeSitterLanguage::Python, &wrong)
                .unwrap()
                .parse_full(&wrong, 1, provider_text("value = 1\n")),
            Err(TreeSitterAdapterError::ProviderVersionMismatch(_))
        ));
    }
}
