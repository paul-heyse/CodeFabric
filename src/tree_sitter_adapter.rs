//! Bounded Tree-sitter adapters for Python and Rust complete-CST observations.
//!
//! This is the only production module allowed to traffic in Tree-sitter parser,
//! tree, node, query, or edit types. Everything crossing its public boundary is
//! application-owned data.

use std::collections::{BTreeSet, VecDeque};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use thiserror::Error;
use tree_sitter::{
    InputEdit, Language, ParseOptions, Parser, Point, Query, QueryCursor, QueryCursorOptions,
    StreamingIterator as _, Tree,
};

use crate::provider_raw_kinds::{
    ProviderGrammarInventory, ProviderRawKindDisposition, TREE_SITTER_PYTHON_GRAMMAR,
    TREE_SITTER_RECOVERY_QUERY, TREE_SITTER_RUST_GRAMMAR,
};
use crate::provider_types::ProviderText;
use crate::registries::{PROVIDER_RESOURCE_PROFILES, ProviderResourceProfileEntry};

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
    pub catalog_id: &'static str,
    pub grammar_fingerprint: &'static str,
    pub facts: Arc<[RawSyntaxFact]>,
    pub changed_ranges: Arc<[ChangedRange]>,
    pub metrics: TreeSitterRunMetrics,
}

/// Cooperative cancellation boundary accepted by the in-process adapter.
pub trait TreeSitterCancellation {
    fn is_cancelled(&self) -> bool;
    fn check_interval(&self) -> u32;
}

/// Cancellation probe for direct, non-runtime adapter use.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl TreeSitterCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn check_interval(&self) -> u32 {
        u32::MAX
    }
}

#[cfg(feature = "daemon")]
impl TreeSitterCancellation for crate::provider_runtime::ProviderCancellation {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()
    }

    fn check_interval(&self) -> u32 {
        self.check_interval()
    }
}

/// Closed adapter failures; no parser-owned error type escapes this boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TreeSitterAdapterError {
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
    fn from_profile(profile: &ProviderResourceProfileEntry) -> Self {
        Self {
            max_input_bytes: profile.max_input_bytes,
            max_work_units: profile.max_work_units,
            max_wall_millis: profile.max_wall_millis,
            max_visited_nodes: profile.max_visited_nodes,
            max_traversal_depth: profile.max_traversal_depth,
            max_output_records: profile.max_output_records,
            max_output_bytes: profile.max_output_bytes,
            max_diagnostics: profile.max_diagnostics,
            max_retained_tree_revisions: profile.max_retained_tree_revisions,
            cancellation_check_interval: profile.cancellation_check_interval,
        }
    }
}

#[derive(Clone)]
struct BoundaryMap {
    provider_offsets: Arc<[usize]>,
    original_offsets: Arc<[u64]>,
}

impl BoundaryMap {
    fn new(text: &ProviderText) -> Result<Self, TreeSitterAdapterError> {
        let provider_offsets = text
            .text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.text.len()))
            .collect::<Vec<_>>();
        if provider_offsets.len() != text.original_byte_offsets.len() {
            return Err(TreeSitterAdapterError::InvalidBoundaryMap(format!(
                "{} provider boundaries but {} original boundaries",
                provider_offsets.len(),
                text.original_byte_offsets.len()
            )));
        }
        if text
            .original_byte_offsets
            .windows(2)
            .any(|window| window[0] > window[1])
        {
            return Err(TreeSitterAdapterError::InvalidBoundaryMap(
                "original offsets are not monotonic".into(),
            ));
        }
        Ok(Self {
            provider_offsets: provider_offsets.into(),
            original_offsets: Arc::clone(&text.original_byte_offsets),
        })
    }

    fn original(&self, provider_offset: usize) -> Result<u64, TreeSitterAdapterError> {
        self.provider_offsets
            .binary_search(&provider_offset)
            .ok()
            .and_then(|index| self.original_offsets.get(index).copied())
            .ok_or(TreeSitterAdapterError::InvalidSpan)
    }
}

struct RetainedRevision {
    revision: u64,
    text: ProviderText,
    tree: Tree,
    snapshot: TreeSitterSnapshot,
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
    query: Query,
    query_cursor: QueryCursor,
    inventory: &'static ProviderGrammarInventory,
    limits: TreeSitterLimits,
    retained: VecDeque<RetainedRevision>,
    metrics: TreeSitterAdapterMetrics,
}

impl TreeSitterAdapter {
    /// Construct and fully validate the selected runtime grammar against the
    /// generated provider catalog before accepting source.
    ///
    /// # Errors
    ///
    /// Returns a version mismatch for any ABI, node, field, source metadata, or
    /// fingerprint drift, and `InvalidQuery` if the governed recovery query no
    /// longer compiles for the exact grammar.
    pub fn new(language_choice: TreeSitterLanguage) -> Result<Self, TreeSitterAdapterError> {
        let (language, node_types, inventory) = language_choice.runtime();
        validate_runtime_inventory(&language, node_types, inventory)?;
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|error| TreeSitterAdapterError::ProviderVersionMismatch(error.to_string()))?;
        let query = Query::new(&language, TREE_SITTER_RECOVERY_QUERY)
            .map_err(|error| TreeSitterAdapterError::InvalidQuery(error.to_string()))?;
        let profile = PROVIDER_RESOURCE_PROFILES
            .iter()
            .find(|profile| profile.profile_id == "in-process-syntax-standard")
            .ok_or_else(|| {
                TreeSitterAdapterError::ProviderVersionMismatch(
                    "in-process-syntax-standard profile absent".into(),
                )
            })?;
        if !profile.provider_ids.contains(&"tree-sitter")
            || profile.max_parser_workers == 0
            || profile.max_retained_tree_revisions == 0
        {
            return Err(TreeSitterAdapterError::ProviderVersionMismatch(
                "Tree-sitter resource profile is not runnable".into(),
            ));
        }
        Ok(Self {
            parser,
            query,
            query_cursor: QueryCursor::new(),
            inventory,
            limits: TreeSitterLimits::from_profile(profile),
            retained: VecDeque::with_capacity(usize::from(profile.max_retained_tree_revisions)),
            metrics: TreeSitterAdapterMetrics::default(),
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
        revision: u64,
        text: ProviderText,
        cancellation: &impl TreeSitterCancellation,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        self.parse_candidate(revision, text, None, cancellation)
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
        revision: u64,
        text: ProviderText,
        edit: TreeSitterEdit,
        cancellation: &impl TreeSitterCancellation,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
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
        self.parse_candidate(revision, text, Some(&edited_tree), cancellation)
    }

    /// Last atomically committed complete revision.
    #[must_use]
    pub fn active_snapshot(&self) -> Option<&TreeSitterSnapshot> {
        self.retained.back().map(|revision| &revision.snapshot)
    }

    /// Current operational counters.
    #[must_use]
    pub const fn metrics(&self) -> TreeSitterAdapterMetrics {
        self.metrics
    }

    /// The exact generated grammar identity validated at startup.
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
        revision: u64,
        text: ProviderText,
        edited_old_tree: Option<&Tree>,
        cancellation: &impl TreeSitterCancellation,
    ) -> Result<TreeSitterSnapshot, TreeSitterAdapterError> {
        if self
            .retained
            .back()
            .is_some_and(|active| revision <= active.revision)
        {
            return self.reject(TreeSitterAdapterError::StaleRevision);
        }
        if u64::try_from(text.text.len()).unwrap_or(u64::MAX) > self.limits.max_input_bytes {
            return self.reject(TreeSitterAdapterError::InputLimit);
        }
        let boundaries = match BoundaryMap::new(&text) {
            Ok(boundaries) => boundaries,
            Err(error) => return self.reject(error),
        };
        if cancellation.is_cancelled() {
            self.parser.reset();
            return self.reject(TreeSitterAdapterError::Cancelled);
        }

        let started = Instant::now();
        let mut work_units = 0_u64;
        let mut abort_reason = None;
        let check_interval = cancellation
            .check_interval()
            .min(self.limits.cancellation_check_interval)
            .max(1);
        let mut callbacks = 0_u32;
        let bytes = text.text.as_bytes();
        let tree = {
            let mut progress = |_: &tree_sitter::ParseState| {
                work_units = work_units.saturating_add(1);
                callbacks = callbacks.saturating_add(1);
                let reason = progress_abort_reason(
                    work_units,
                    self.limits.max_work_units,
                    deadline_exceeded(started.elapsed(), self.limits.max_wall_millis),
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
            self.inventory,
            &boundaries,
            self.limits,
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
            self.limits,
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
            catalog_id: self.inventory.catalog_id,
            grammar_fingerprint: self.inventory.runtime_inventory_fingerprint,
            facts: facts.into(),
            changed_ranges: changed_ranges.into(),
            metrics,
        };
        self.retained.push_back(RetainedRevision {
            revision,
            text,
            tree,
            snapshot: snapshot.clone(),
        });
        while exceeds_limit(
            u64::try_from(self.retained.len()).unwrap_or(u64::MAX),
            u64::from(self.limits.max_retained_tree_revisions),
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
    if language.abi_version() != inventory.grammar_abi {
        return Err(version_mismatch("grammar ABI"));
    }
    if language.node_kind_count().saturating_add(2) != inventory.raw_kinds.len() {
        return Err(version_mismatch("raw kind count"));
    }
    let runtime_kind_ids = (0..language.node_kind_count())
        .map(|id| u16::try_from(id).map_err(|_| version_mismatch("raw kind ID")))
        .chain([Ok(u16::MAX - 1), Ok(u16::MAX)]);
    for (id, entry) in runtime_kind_ids.zip(inventory.raw_kinds) {
        let id = id?;
        if entry.raw_kind_id != id
            || language.node_kind_for_id(id) != Some(entry.raw_name)
            || language.node_kind_is_named(id) != entry.named
            || language.node_kind_is_visible(id) != entry.visible
            || language.node_kind_is_supertype(id) != entry.supertype
        {
            return Err(version_mismatch("raw kind inventory"));
        }
    }
    if language.field_count() != inventory.fields.len() {
        return Err(version_mismatch("field count"));
    }
    for entry in inventory.fields {
        if language.field_name_for_id(entry.field_id) != Some(entry.field_name) {
            return Err(version_mismatch("field inventory"));
        }
    }
    if checksum(node_types.as_bytes()) != inventory.node_types_digest {
        return Err(version_mismatch("NODE_TYPES digest"));
    }
    if checksum(inventory.query_bundle_canonical_json) != inventory.query_bundle_digest {
        return Err(version_mismatch("query bundle digest"));
    }
    Ok(())
}

fn version_mismatch(part: &str) -> TreeSitterAdapterError {
    TreeSitterAdapterError::ProviderVersionMismatch(part.to_owned())
}

fn checksum(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

type RecoveryNode = (usize, usize, bool, bool, u16);

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // Iterative cursor ownership and all budget checks remain in one traversal.
fn walk_tree(
    tree: &Tree,
    inventory: &ProviderGrammarInventory,
    boundaries: &BoundaryMap,
    limits: TreeSitterLimits,
    cancellation: &impl TreeSitterCancellation,
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
            u64::from(cancellation.check_interval()),
            cancellation.is_cancelled(),
        ) {
            return Err(TreeSitterAdapterError::Cancelled);
        }
        let node = cursor.node();
        let entry = inventory
            .raw_kinds
            .binary_search_by_key(&node.kind_id(), |entry| entry.raw_kind_id)
            .ok()
            .and_then(|index| inventory.raw_kinds.get(index))
            .ok_or_else(|| version_mismatch("runtime node kind absent"))?;
        if !runtime_node_matches(node.kind(), node.is_named(), entry.raw_name, entry.named) {
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
        let raw_kind = entry.raw_name.to_owned();
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
    cancellation: &impl TreeSitterCancellation,
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
        .check_interval()
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
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn provider_text(text: &str) -> ProviderText {
        ProviderText {
            text: Arc::from(text),
            original_byte_offsets: Arc::from(
                text.char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap())
                    .chain(std::iter::once(u64::try_from(text.len()).unwrap()))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn latin1_provider_text() -> ProviderText {
        ProviderText {
            text: Arc::from("# coding: latin-1\nname = 'é'\n"),
            original_byte_offsets: Arc::from(
                (0_u64..="# coding: latin-1\nname = 'é'\n".chars().count() as u64)
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../contracts/fixtures/tree-sitter/adapter-cases-v1.json"
        ))
        .unwrap()
    }

    fn fixture_language(value: &serde_json::Value) -> TreeSitterLanguage {
        match value.as_str().unwrap() {
            "python" => TreeSitterLanguage::Python,
            "rust" => TreeSitterLanguage::Rust,
            other => panic!("unknown fixture language {other}"),
        }
    }

    fn fact_stream_digest(facts: &[RawSyntaxFact]) -> String {
        fn frame_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
            hasher.update(&u64::try_from(value.len()).unwrap().to_le_bytes());
            hasher.update(value);
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric:tree-sitter-raw-syntax-facts:v1\0");
        hasher.update(&u64::try_from(facts.len()).unwrap().to_le_bytes());
        for fact in facts {
            hasher.update(&fact.id.0.to_le_bytes());
            hasher.update(&fact.raw_kind_id.to_le_bytes());
            frame_bytes(&mut hasher, fact.raw_kind.as_bytes());
            hasher.update(&fact.normalized_kind.0.to_le_bytes());
            hasher.update(&[match fact.disposition {
                ProviderRawKindDisposition::Normalize => 0,
                ProviderRawKindDisposition::Ignore => 1,
                ProviderRawKindDisposition::Unsupported => 2,
            }]);
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&[u8::from(fact.named)
                | (u8::from(fact.extra) << 1)
                | (u8::from(fact.error) << 2)
                | (u8::from(fact.missing) << 3)]);
            match fact.parent {
                Some(parent) => {
                    hasher.update(&[1]);
                    hasher.update(&parent.0.to_le_bytes());
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            match &fact.field_name {
                Some(field_name) => {
                    hasher.update(&[1]);
                    frame_bytes(&mut hasher, field_name.as_bytes());
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            hasher.update(&fact.ordinal.to_le_bytes());
            hasher.update(&fact.depth.to_le_bytes());
        }
        format!("b3:{}", hasher.finalize())
    }

    struct CancelAfter {
        checks: AtomicUsize,
        after: usize,
    }

    impl TreeSitterCancellation for CancelAfter {
        fn is_cancelled(&self) -> bool {
            self.checks.fetch_add(1, Ordering::Relaxed) >= self.after
        }

        fn check_interval(&self) -> u32 {
            1
        }
    }

    #[test]
    fn wp30_behavioral_acceptance() {
        for case in fixture()["cases"].as_array().unwrap() {
            let language = fixture_language(&case["language"]);
            let mut adapter = TreeSitterAdapter::new(language).unwrap();
            let inventory = *adapter.inventory();
            let snapshot = adapter
                .parse_full(
                    1,
                    provider_text(case["source"].as_str().unwrap()),
                    &NeverCancelled,
                )
                .unwrap();
            assert_eq!(
                fact_stream_digest(&snapshot.facts),
                case["expected_fact_digest"].as_str().unwrap(),
                "{} exact fact projection drifted",
                case["case_id"].as_str().unwrap()
            );
            assert!(!snapshot.facts.is_empty());
            assert!(snapshot.facts.iter().all(|fact| {
                inventory
                    .raw_kinds
                    .binary_search_by_key(&fact.raw_kind_id, |entry| entry.raw_kind_id)
                    .ok()
                    .and_then(|index| inventory.raw_kinds.get(index))
                    .is_some_and(|entry| {
                        entry.raw_name == fact.raw_kind
                            && entry.normalized_kind_code == fact.normalized_kind.0
                    })
            }));
            assert_eq!(
                snapshot.facts.iter().any(|fact| fact.error || fact.missing),
                case["expected_recovery"].as_bool().unwrap()
            );
            if let Some(expected_count) = case["expected_fact_count"].as_u64() {
                assert_eq!(u64::try_from(snapshot.facts.len()).unwrap(), expected_count);
                assert!(snapshot.facts.iter().any(|fact| !fact.named));
            }
            if let Some(expected_root) = case.get("expected_root") {
                let root = snapshot.facts.first().unwrap();
                assert_eq!(
                    u64::from(root.raw_kind_id),
                    expected_root["raw_kind_id"].as_u64().unwrap()
                );
                assert_eq!(root.raw_kind, expected_root["raw_kind"].as_str().unwrap());
                assert_eq!(
                    root.start_byte,
                    expected_root["start_byte"].as_u64().unwrap()
                );
                assert_eq!(root.end_byte, expected_root["end_byte"].as_u64().unwrap());
            }
            if let Some(required) = case["required_raw_kinds"].as_array() {
                for raw_kind in required {
                    assert!(
                        snapshot
                            .facts
                            .iter()
                            .any(|fact| fact.raw_kind == raw_kind.as_str().unwrap())
                    );
                }
            }
        }
    }

    #[test]
    fn wp30_structural_acceptance() {
        for edit_case in fixture()["edits"].as_array().unwrap() {
            let language = fixture_language(&edit_case["language"]);
            let old = edit_case["old_source"].as_str().unwrap();
            let new = edit_case["new_source"].as_str().unwrap();
            let old_fragment = edit_case["old_fragment"].as_str().unwrap();
            let new_fragment = edit_case["new_fragment"].as_str().unwrap();
            let start = old.rfind(old_fragment).unwrap();
            assert_eq!(start, new.rfind(new_fragment).unwrap());
            let mut incremental = TreeSitterAdapter::new(language).unwrap();
            incremental
                .parse_full(1, provider_text(old), &NeverCancelled)
                .unwrap();
            let incrementally_parsed = incremental
                .parse_incremental(
                    2,
                    provider_text(new),
                    TreeSitterEdit {
                        start_byte: start,
                        old_end_byte: start + old_fragment.len(),
                        new_end_byte: start + new_fragment.len(),
                    },
                    &NeverCancelled,
                )
                .unwrap();
            let fully_parsed = TreeSitterAdapter::new(language)
                .unwrap()
                .parse_full(2, provider_text(new), &NeverCancelled)
                .unwrap();
            assert_eq!(incrementally_parsed.facts, fully_parsed.facts);
            assert!(!incrementally_parsed.changed_ranges.is_empty());
            assert!(
                incrementally_parsed
                    .facts
                    .iter()
                    .all(|fact| fact.start_byte <= fact.end_byte)
            );
        }

        let latin1 = TreeSitterAdapter::new(TreeSitterLanguage::Python)
            .unwrap()
            .parse_full(1, latin1_provider_text(), &NeverCancelled)
            .unwrap();
        assert_eq!(latin1.facts.first().unwrap().end_byte, 29);
        assert!(latin1.facts.iter().all(|fact| fact.end_byte <= 29));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One negative oracle isolates every independent boundary predicate.
    fn wp30_negative_zero_state() {
        let (language, node_types, expected) = TreeSitterLanguage::Python.runtime();
        assert!(validate_runtime_inventory(&language, node_types, expected).is_ok());
        let mut drifted = *expected;
        drifted.grammar_abi = drifted.grammar_abi.saturating_add(1);
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        drifted = *expected;
        drifted.raw_kinds = &expected.raw_kinds[1..];
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        let raw_mutations: [fn(&mut crate::provider_raw_kinds::ProviderRawKindEntry); 5] = [
            |entry: &mut crate::provider_raw_kinds::ProviderRawKindEntry| {
                entry.raw_kind_id = entry.raw_kind_id.saturating_add(1);
            },
            |entry: &mut crate::provider_raw_kinds::ProviderRawKindEntry| {
                entry.raw_name = "catalog-drift";
            },
            |entry: &mut crate::provider_raw_kinds::ProviderRawKindEntry| {
                entry.named = !entry.named;
            },
            |entry: &mut crate::provider_raw_kinds::ProviderRawKindEntry| {
                entry.visible = !entry.visible;
            },
            |entry: &mut crate::provider_raw_kinds::ProviderRawKindEntry| {
                entry.supertype = !entry.supertype;
            },
        ];
        for mutate in raw_mutations {
            let mut raw_kinds = expected.raw_kinds.to_vec();
            mutate(&mut raw_kinds[0]);
            drifted = *expected;
            drifted.raw_kinds = Box::leak(raw_kinds.into_boxed_slice());
            assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        }
        drifted = *expected;
        drifted.fields = &expected.fields[1..];
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        let mut fields = expected.fields.to_vec();
        fields[0].field_id = fields[0].field_id.saturating_add(1);
        drifted = *expected;
        drifted.fields = Box::leak(fields.into_boxed_slice());
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        drifted = *expected;
        drifted.node_types_digest = "b3:catalog-drift";
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());
        drifted = *expected;
        drifted.query_bundle_canonical_json = b"{}";
        assert!(validate_runtime_inventory(&language, node_types, &drifted).is_err());

        let bad_lengths = ProviderText {
            text: Arc::from("é"),
            original_byte_offsets: Arc::from([0]),
        };
        assert!(matches!(
            BoundaryMap::new(&bad_lengths),
            Err(TreeSitterAdapterError::InvalidBoundaryMap(_))
        ));
        let bad_order = ProviderText {
            text: Arc::from("ab"),
            original_byte_offsets: Arc::from([0, 2, 1]),
        };
        assert!(matches!(
            BoundaryMap::new(&bad_order),
            Err(TreeSitterAdapterError::InvalidBoundaryMap(_))
        ));
        let unicode = BoundaryMap::new(&provider_text("é")).unwrap();
        assert_eq!(unicode.original(0), Ok(0));
        assert_eq!(unicode.original(2), Ok(2));
        assert_eq!(
            unicode.original(1),
            Err(TreeSitterAdapterError::InvalidSpan)
        );

        let valid_edit = TreeSitterEdit {
            start_byte: 1,
            old_end_byte: 2,
            new_end_byte: 2,
        };
        assert!(edit_geometry_valid(3, 3, valid_edit));
        for invalid in [
            TreeSitterEdit {
                start_byte: 2,
                old_end_byte: 1,
                new_end_byte: 2,
            },
            TreeSitterEdit {
                start_byte: 2,
                old_end_byte: 2,
                new_end_byte: 1,
            },
            TreeSitterEdit {
                start_byte: 1,
                old_end_byte: 4,
                new_end_byte: 2,
            },
            TreeSitterEdit {
                start_byte: 1,
                old_end_byte: 2,
                new_end_byte: 4,
            },
        ] {
            assert!(!edit_geometry_valid(3, 3, invalid));
        }
        assert!(edit_boundaries_valid("abc", "aXc", valid_edit));
        for (old, new, edit) in [
            (
                "éa",
                "abc",
                TreeSitterEdit {
                    start_byte: 1,
                    old_end_byte: 2,
                    new_end_byte: 2,
                },
            ),
            (
                "aé",
                "abc",
                TreeSitterEdit {
                    start_byte: 0,
                    old_end_byte: 2,
                    new_end_byte: 2,
                },
            ),
            (
                "abc",
                "éa",
                TreeSitterEdit {
                    start_byte: 1,
                    old_end_byte: 2,
                    new_end_byte: 2,
                },
            ),
            (
                "abc",
                "aé",
                TreeSitterEdit {
                    start_byte: 0,
                    old_end_byte: 2,
                    new_end_byte: 2,
                },
            ),
        ] {
            assert!(!edit_boundaries_valid(old, new, edit));
        }
        assert!(edit_unchanged_regions_match("abc", "aXc", valid_edit));
        assert!(!edit_unchanged_regions_match(
            "abc",
            "xbc",
            TreeSitterEdit {
                start_byte: 1,
                old_end_byte: 1,
                new_end_byte: 1,
            }
        ));
        assert!(!edit_unchanged_regions_match(
            "abc",
            "ab!",
            TreeSitterEdit {
                start_byte: 1,
                old_end_byte: 1,
                new_end_byte: 1,
            }
        ));
        assert!(validate_edit("abc", "aXc", valid_edit).is_ok());
        assert!(validate_edit("abc", "xbc", valid_edit).is_err());

        assert_eq!(point_at("ab\ncde", 0), Ok(Point { row: 0, column: 0 }));
        assert_eq!(point_at("ab\ncde", 2), Ok(Point { row: 0, column: 2 }));
        assert_eq!(point_at("ab\ncde", 3), Ok(Point { row: 1, column: 0 }));
        assert_eq!(point_at("ab\ncde", 5), Ok(Point { row: 1, column: 2 }));
        assert!(point_at("é", 1).is_err());

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("def broken(:\n", None).unwrap();
        let query = Query::new(&language, TREE_SITTER_RECOVERY_QUERY).unwrap();
        let mut cursor = QueryCursor::new();
        let limits = TreeSitterLimits::from_profile(
            PROVIDER_RESOURCE_PROFILES
                .iter()
                .find(|profile| profile.profile_id == "in-process-syntax-standard")
                .unwrap(),
        );
        let mut work_units = 0;
        assert!(matches!(
            run_recovery_query(
                &mut cursor,
                &query,
                &tree,
                "def broken(:\n",
                &BTreeSet::new(),
                limits,
                &NeverCancelled,
                Instant::now(),
                &mut work_units,
            ),
            Err(TreeSitterAdapterError::ProviderVersionMismatch(_))
        ));
        let mut query_limited = limits;
        query_limited.max_work_units = 0;
        work_units = 0;
        assert_eq!(
            run_recovery_query(
                &mut cursor,
                &query,
                &tree,
                "def broken(:\n",
                &BTreeSet::new(),
                query_limited,
                &NeverCancelled,
                Instant::now(),
                &mut work_units,
            ),
            Err(TreeSitterAdapterError::WorkLimit)
        );

        let malformed = String::from_utf8_lossy(&[b'f', b'n', b' ', 0xff, b'(']).into_owned();
        for language in [TreeSitterLanguage::Python, TreeSitterLanguage::Rust] {
            let result = std::panic::catch_unwind(|| {
                TreeSitterAdapter::new(language).unwrap().parse_full(
                    1,
                    provider_text(&malformed),
                    &NeverCancelled,
                )
            });
            assert!(result.is_ok());
            assert!(result.unwrap().is_ok());
        }
        let mut adapter = TreeSitterAdapter::new(TreeSitterLanguage::Rust).unwrap();
        adapter
            .parse_full(1, provider_text("fn main() {}\n"), &NeverCancelled)
            .unwrap();
        let active = adapter.active_snapshot().unwrap().clone();
        assert!(matches!(
            adapter.parse_incremental(
                2,
                provider_text("fn changed() {}\n"),
                TreeSitterEdit {
                    start_byte: 3,
                    old_end_byte: 6,
                    new_end_byte: 10,
                },
                &NeverCancelled,
            ),
            Err(TreeSitterAdapterError::InvalidEdit(_))
        ));
        assert_eq!(adapter.active_snapshot(), Some(&active));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One oracle proves every profile boundary preserves atomic publication.
    fn wp30_operational_acceptance() {
        assert!(!exceeds_limit(10, 10));
        assert!(exceeds_limit(11, 10));
        assert!(!deadline_exceeded(Duration::from_millis(10), 10));
        assert!(deadline_exceeded(Duration::from_millis(11), 10));
        assert!(!cancellation_due(1, 2, true));
        assert!(cancellation_due(2, 2, true));
        assert!(!cancellation_due(2, 2, false));
        assert!(runtime_node_matches("node", true, "node", true));
        assert!(!runtime_node_matches("other", true, "node", true));
        assert!(!runtime_node_matches("node", false, "node", true));
        assert_eq!(progress_abort_reason(10, 10, false, 1, 2, false), None);
        assert_eq!(
            progress_abort_reason(11, 10, false, 1, 2, false),
            Some(AbortReason::Work)
        );
        assert_eq!(
            progress_abort_reason(10, 10, true, 1, 2, false),
            Some(AbortReason::Deadline)
        );
        assert_eq!(progress_abort_reason(10, 10, false, 1, 2, true), None);
        assert_eq!(
            progress_abort_reason(10, 10, false, 2, 2, true),
            Some(AbortReason::Cancelled)
        );

        let mut adapter = TreeSitterAdapter::new(TreeSitterLanguage::Python).unwrap();
        let complete = adapter
            .parse_full(1, provider_text("value = 1\n"), &NeverCancelled)
            .unwrap();
        let cancellation = CancelAfter {
            checks: AtomicUsize::new(0),
            after: 1,
        };
        let cancelled_source = "value = 2\n".repeat(10_000);
        assert_eq!(
            adapter.parse_full(2, provider_text(&cancelled_source), &cancellation),
            Err(TreeSitterAdapterError::Cancelled)
        );
        assert_eq!(adapter.active_snapshot(), Some(&complete));
        assert_eq!(adapter.metrics().completed_runs, 1);
        assert_eq!(adapter.metrics().cancelled_runs, 1);

        let deep = format!("{}value{}\n", "(".repeat(300), ")".repeat(300));
        assert_eq!(
            adapter.parse_full(3, provider_text(&deep), &NeverCancelled),
            Err(TreeSitterAdapterError::DepthLimit)
        );
        assert_eq!(adapter.active_snapshot(), Some(&complete));

        let standard_limits = adapter.limits;
        let exact_input = "x = 1\n";
        let mut exact_input_adapter = TreeSitterAdapter::new(TreeSitterLanguage::Python).unwrap();
        exact_input_adapter.limits.max_input_bytes = u64::try_from(exact_input.len()).unwrap();
        assert!(
            exact_input_adapter
                .parse_full(1, provider_text(exact_input), &NeverCancelled)
                .is_ok()
        );
        let mut over_input_adapter = TreeSitterAdapter::new(TreeSitterLanguage::Python).unwrap();
        over_input_adapter.limits.max_input_bytes =
            u64::try_from(exact_input.len().saturating_sub(1)).unwrap();
        assert_eq!(
            over_input_adapter.parse_full(1, provider_text(exact_input), &NeverCancelled),
            Err(TreeSitterAdapterError::InputLimit)
        );
        adapter.limits.max_input_bytes = 1;
        assert_eq!(
            adapter.parse_full(4, provider_text("value = 4\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::InputLimit)
        );
        adapter.limits = standard_limits;
        adapter.limits.max_work_units = 0;
        assert_eq!(
            adapter.parse_full(4, provider_text("value = 4\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::WorkLimit)
        );
        adapter.limits = standard_limits;
        adapter.limits.max_visited_nodes = 1;
        assert_eq!(
            adapter.parse_full(4, provider_text("value = 4\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::NodeLimit)
        );
        adapter.limits = standard_limits;
        adapter.limits.max_output_records = 1;
        assert_eq!(
            adapter.parse_full(4, provider_text("value = 4\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::OutputRecordLimit)
        );
        adapter.limits = standard_limits;
        adapter.limits.max_output_bytes = 1;
        assert_eq!(
            adapter.parse_full(4, provider_text("value = 4\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::OutputByteLimit)
        );
        adapter.limits = standard_limits;
        adapter.limits.max_diagnostics = 0;
        assert_eq!(
            adapter.parse_full(4, provider_text("def broken(:\n"), &NeverCancelled),
            Err(TreeSitterAdapterError::DiagnosticLimit)
        );
        adapter.limits = standard_limits;
        assert_eq!(adapter.active_snapshot(), Some(&complete));

        adapter
            .parse_full(4, provider_text("value = 4\n"), &NeverCancelled)
            .unwrap();
        let measured_source = "value = 5\n".repeat(10_000);
        adapter
            .parse_full(5, provider_text(&measured_source), &NeverCancelled)
            .unwrap();
        assert_eq!(adapter.metrics().retained_revisions, 2);
        let metrics = adapter.metrics().last_run.unwrap();
        assert!(metrics.visited_nodes > 0);
        assert!(metrics.parse_work_units > 0);
        assert_eq!(
            metrics.work_units,
            metrics
                .parse_work_units
                .saturating_add(metrics.visited_nodes)
                .saturating_add(metrics.query_work_units)
        );
        assert!(metrics.parse_duration > Duration::ZERO);
    }
}
