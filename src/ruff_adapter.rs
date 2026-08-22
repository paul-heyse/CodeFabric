//! Bounded Ruff Python lexical and typed-AST adapter.
//!
//! Ruff owns parsing and indexing inside this module. Every value crossing the
//! public boundary is an application-owned observation over authoritative source
//! byte coordinates.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{AnyNodeRef, NodeKind, PySourceType, PythonVersion, Stmt};
use ruff_python_index::Indexer;
use ruff_python_parser::{ParseOptions, Parsed, parse_unchecked};
use ruff_python_trivia::{CommentLinePosition, SuppressionKind, TriviaRanges, is_pragma_comment};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextRange, TextSize};
use thiserror::Error;

use crate::provider_raw_kinds::{
    ProviderRawKindDisposition, RUFF_PYTHON_FRONTEND, RuffPythonInventory,
    ruff_python_node_kind_entry, ruff_python_token_kind_entry,
};
use crate::provider_types::{ProviderBoundaryError, ProviderBoundaryMap, ProviderText};
use crate::registries::{PROVIDER_RESOURCE_PROFILES, ProviderResourceProfileEntry};
use crate::tree_sitter_adapter::{RawSyntaxFact, SyntaxOccurrenceId, TreeSitterSnapshot};

/// Stable occurrence identity within one Ruff parse result.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuffOccurrenceId(pub u64);

/// The closed language-neutral typed-AST categories owned by CodeFabric.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffAstCategory {
    SyntaxNode,
    Statement,
    Expression,
    Pattern,
    DeclarationSyntax,
    TypeSyntax,
    ParameterSyntax,
    ArgumentSyntax,
    Block,
    Literal,
    Operation,
    AttributeAccess,
    SubscriptAccess,
    CallExpression,
    Assignment,
    Branch,
    Loop,
    Return,
    Yield,
    Await,
    RaiseSyntax,
    ImportSyntax,
}

impl RuffAstCategory {
    const fn registry_code(self) -> u16 {
        match self {
            Self::SyntaxNode => 10,
            Self::Statement => 20,
            Self::Expression => 30,
            Self::Pattern => 40,
            Self::DeclarationSyntax => 50,
            Self::TypeSyntax => 60,
            Self::ParameterSyntax => 70,
            Self::ArgumentSyntax => 80,
            Self::Block => 90,
            Self::Literal => 100,
            Self::Operation => 110,
            Self::AttributeAccess => 120,
            Self::SubscriptAccess => 140,
            Self::CallExpression => 160,
            Self::Assignment => 170,
            Self::Branch => 180,
            Self::Loop => 190,
            Self::Return => 200,
            Self::Yield => 210,
            Self::Await => 220,
            Self::RaiseSyntax => 230,
            Self::ImportSyntax => 240,
        }
    }

    const fn from_registry_code(code: u16) -> Option<Self> {
        match code {
            10 => Some(Self::SyntaxNode),
            20 => Some(Self::Statement),
            30 => Some(Self::Expression),
            40 => Some(Self::Pattern),
            50 => Some(Self::DeclarationSyntax),
            60 => Some(Self::TypeSyntax),
            70 => Some(Self::ParameterSyntax),
            80 => Some(Self::ArgumentSyntax),
            90 => Some(Self::Block),
            100 => Some(Self::Literal),
            110 => Some(Self::Operation),
            120 => Some(Self::AttributeAccess),
            140 => Some(Self::SubscriptAccess),
            160 => Some(Self::CallExpression),
            170 => Some(Self::Assignment),
            180 => Some(Self::Branch),
            190 => Some(Self::Loop),
            200 => Some(Self::Return),
            210 => Some(Self::Yield),
            220 => Some(Self::Await),
            230 => Some(Self::RaiseSyntax),
            240 => Some(Self::ImportSyntax),
            _ => None,
        }
    }
}

/// Closed normalized relation name for one source-order AST child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffChildRole {
    Body,
    Decorator,
    Name,
    TypeParameter,
    Parameter,
    Argument,
    KeywordArgument,
    Callee,
    Condition,
    Target,
    Value,
    Annotation,
    Iterable,
    Pattern,
    Handler,
    Clause,
    Item,
    Segment,
    Child,
}

/// Closed lexical category derived from Ruff's exact token enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffTokenClass {
    Identifier,
    Keyword,
    Operator,
    Literal,
    Comment,
    Newline,
    Indentation,
    EndOfFile,
    Unknown,
}

/// Contract-governed token spelling retention policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuffTokenSpelling {
    /// Exact source spelling for identifiers and keywords.
    Slice(String),
    /// Domain-separated BLAKE3 spelling identity for literals.
    Blake3(String),
}

/// Source member of one Python frontend batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffSourceFact {
    pub provider_image_fingerprint: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub line_count: u64,
}

/// One application-owned Ruff token observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffTokenFact {
    pub ordinal: u32,
    pub raw_kind_id: u16,
    pub raw_kind: &'static str,
    pub class: RuffTokenClass,
    pub start_byte: u64,
    pub end_byte: u64,
    pub line: u32,
    pub column: u32,
    pub spelling: Option<RuffTokenSpelling>,
    pub syntax_id: Option<RuffOccurrenceId>,
}

/// One application-owned typed-AST observation in source-containment order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffAstFact {
    pub id: RuffOccurrenceId,
    pub raw_kind_id: u16,
    pub raw_kind: &'static str,
    pub category: RuffAstCategory,
    pub disposition: ProviderRawKindDisposition,
    pub start_byte: u64,
    pub end_byte: u64,
    pub line: u32,
    pub column: u32,
    pub parent: Option<RuffOccurrenceId>,
    pub child_role: Option<RuffChildRole>,
    pub child_ordinal: u32,
    pub source_ordinal: u32,
    pub evaluation_ordinal: Option<u32>,
    pub explicit_parenthesized: bool,
}

/// Placement of one Python comment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffCommentPlacement {
    OwnLine,
    EndOfLine,
}

/// One comment observation derived from Ruff's token/trivia index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffCommentFact {
    pub start_byte: u64,
    pub end_byte: u64,
    pub placement: RuffCommentPlacement,
    pub block_member: bool,
}

/// Closed directive classes required by the Python lexical contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffDirectiveKind {
    Noqa,
    TypeIgnore,
    TypeComment,
    Formatter,
    OtherPragma,
}

/// One directive found in a Ruff-authenticated comment token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffDirectiveFact {
    pub kind: RuffDirectiveKind,
    pub start_byte: u64,
    pub end_byte: u64,
    pub target: Option<RuffOccurrenceId>,
}

/// One source string region indexed by Ruff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffStringRegion {
    pub start_byte: u64,
    pub end_byte: u64,
    pub multiline: bool,
    pub interpolated: bool,
    pub syntax_id: Option<RuffOccurrenceId>,
}

/// One module, class, or function docstring statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffDocstringFact {
    pub start_byte: u64,
    pub end_byte: u64,
    pub owner: RuffOccurrenceId,
}

/// Closed Ruff diagnostic source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuffDiagnosticKind {
    Parse,
    UnsupportedSyntax,
}

/// One Ruff recovery diagnostic with overlapping Tree-sitter evidence retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffDiagnosticFact {
    pub kind: RuffDiagnosticKind,
    pub message: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub tree_sitter_recovery_ids: Arc<[SyntaxOccurrenceId]>,
}

/// Smallest compatible named Tree-sitter node for one Ruff AST occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuffTreeCorrespondence {
    pub ruff_id: RuffOccurrenceId,
    pub tree_sitter_id: SyntaxOccurrenceId,
}

/// Per-run operational observations. They are not benchmark thresholds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuffRunMetrics {
    pub parse_duration: Duration,
    pub projection_duration: Duration,
    pub visited_nodes: u64,
    pub token_count: u64,
    pub output_records: u64,
    pub output_bytes: u64,
    pub work_units: u64,
}

/// Aggregate counters, including candidates discarded before publication.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuffAdapterMetrics {
    pub completed_runs: u64,
    pub rejected_runs: u64,
    pub cancelled_runs: u64,
    pub retained_revisions: u16,
    pub last_run: Option<RuffRunMetrics>,
}

/// Application-owned projection of one complete Ruff parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffSnapshot {
    pub revision: u64,
    pub source: RuffSourceFact,
    pub catalog_id: &'static str,
    pub provider_version: &'static str,
    pub runtime_inventory_fingerprint: &'static str,
    pub tokens: Arc<[RuffTokenFact]>,
    pub ast: Arc<[RuffAstFact]>,
    pub comments: Arc<[RuffCommentFact]>,
    pub directives: Arc<[RuffDirectiveFact]>,
    pub strings: Arc<[RuffStringRegion]>,
    pub docstrings: Arc<[RuffDocstringFact]>,
    pub continuation_line_starts: Arc<[u64]>,
    pub diagnostics: Arc<[RuffDiagnosticFact]>,
    pub correspondences: Arc<[RuffTreeCorrespondence]>,
    pub metrics: RuffRunMetrics,
}

/// Cooperative cancellation boundary for the in-process Ruff adapter.
pub trait RuffCancellation {
    fn is_cancelled(&self) -> bool;
    fn check_interval(&self) -> u32;
}

/// Cancellation probe for direct adapter use.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverRuffCancelled;

impl RuffCancellation for NeverRuffCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn check_interval(&self) -> u32 {
        u32::MAX
    }
}

#[cfg(feature = "daemon")]
impl RuffCancellation for crate::provider_runtime::ProviderCancellation {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()
    }

    fn check_interval(&self) -> u32 {
        self.check_interval()
    }
}

/// Closed adapter errors; no Ruff-owned error escapes this module.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuffAdapterError {
    #[error("Ruff provider version mismatch: {0}")]
    ProviderVersionMismatch(String),
    #[error("provider text boundary map is invalid: {0}")]
    InvalidBoundaryMap(String),
    #[error("Ruff source exceeds the provider input limit")]
    InputLimit,
    #[error("Ruff provider was cancelled")]
    Cancelled,
    #[error("Ruff provider exceeded its wall-clock deadline")]
    Deadline,
    #[error("Ruff provider exceeded its work limit")]
    WorkLimit,
    #[error("Ruff provider exceeded its visited-node limit")]
    NodeLimit,
    #[error("Ruff provider exceeded its traversal-depth limit")]
    DepthLimit,
    #[error("Ruff provider exceeded its output-record limit")]
    OutputRecordLimit,
    #[error("Ruff provider exceeded its output-byte limit")]
    OutputByteLimit,
    #[error("Ruff provider exceeded its diagnostic limit")]
    DiagnosticLimit,
    #[error("Ruff returned an out-of-bounds or non-boundary span")]
    InvalidSpan,
    #[error("Ruff projection invariant failed: {0}")]
    ProjectionInvariant(String),
    #[error("Tree-sitter evidence does not describe this Python source revision")]
    MismatchedTreeSitterEvidence,
    #[error("parse revision must advance monotonically")]
    StaleRevision,
}

impl From<ProviderBoundaryError> for RuffAdapterError {
    fn from(error: ProviderBoundaryError) -> Self {
        match error {
            ProviderBoundaryError::InvalidMap(message) => Self::InvalidBoundaryMap(message),
            ProviderBoundaryError::InvalidOffset(_) => Self::InvalidSpan,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RuffLimits {
    max_input_bytes: u64,
    max_work_units: u64,
    max_wall_millis: u64,
    max_visited_nodes: u64,
    max_traversal_depth: u16,
    max_output_records: u64,
    max_output_bytes: u64,
    max_diagnostics: u16,
    cancellation_check_interval: u32,
}

impl RuffLimits {
    const fn from_profile(profile: &ProviderResourceProfileEntry) -> Self {
        Self {
            max_input_bytes: profile.max_input_bytes,
            max_work_units: profile.max_work_units,
            max_wall_millis: profile.max_wall_millis,
            max_visited_nodes: profile.max_visited_nodes,
            max_traversal_depth: profile.max_traversal_depth,
            max_output_records: profile.max_output_records,
            max_output_bytes: profile.max_output_bytes,
            max_diagnostics: profile.max_diagnostics,
            cancellation_check_interval: profile.cancellation_check_interval,
        }
    }
}

struct RetainedRuffRevision {
    revision: u64,
    text: ProviderText,
    parsed: Parsed<ruff_python_ast::ModModule>,
    trivia: TriviaRanges,
    indexer: Indexer,
    line_index: LineIndex,
    snapshot: RuffSnapshot,
}

/// One worker-owned Ruff frontend with exactly one atomically published parse.
pub struct RuffAdapter {
    inventory: &'static RuffPythonInventory,
    limits: RuffLimits,
    retained: Option<RetainedRuffRevision>,
    metrics: RuffAdapterMetrics,
}

impl RuffAdapter {
    /// Validate the generated exact-version inventory and resource profile.
    ///
    /// # Errors
    ///
    /// Returns a version mismatch if the generated inventory or profile is not
    /// the exact supported Ruff frontend.
    pub fn new() -> Result<Self, RuffAdapterError> {
        validate_runtime_inventory(&RUFF_PYTHON_FRONTEND)?;
        let profile = PROVIDER_RESOURCE_PROFILES
            .iter()
            .find(|profile| profile.profile_id == "in-process-syntax-standard")
            .ok_or_else(|| {
                RuffAdapterError::ProviderVersionMismatch(
                    "in-process-syntax-standard profile absent".into(),
                )
            })?;
        if !profile.provider_ids.contains(&"ruff-python") || profile.max_parser_workers == 0 {
            return Err(RuffAdapterError::ProviderVersionMismatch(
                "Ruff resource profile is not runnable".into(),
            ));
        }
        Ok(Self {
            inventory: &RUFF_PYTHON_FRONTEND,
            limits: RuffLimits::from_profile(profile),
            retained: None,
            metrics: RuffAdapterMetrics::default(),
        })
    }

    /// Parse a Python source image once, build all Ruff indexes once, then
    /// atomically replace the active revision only after every bound succeeds.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, invalid source mappings, cancellation, deadlines,
    /// and every configured resource limit without changing the active revision.
    #[allow(clippy::too_many_lines)] // The atomic candidate pipeline keeps every retained Ruff value visibly single-build.
    pub fn parse(
        &mut self,
        revision: u64,
        text: ProviderText,
        tree_sitter: &TreeSitterSnapshot,
        cancellation: &impl RuffCancellation,
    ) -> Result<RuffSnapshot, RuffAdapterError> {
        if self
            .retained
            .as_ref()
            .is_some_and(|retained| revision <= retained.revision)
        {
            return self.reject(RuffAdapterError::StaleRevision);
        }
        if u64::try_from(text.text.len()).unwrap_or(u64::MAX) > self.limits.max_input_bytes {
            return self.reject(RuffAdapterError::InputLimit);
        }
        let provider_image_fingerprint = text.provider_image_fingerprint();
        if tree_sitter.revision != revision
            || tree_sitter.catalog_id != "tree-sitter-python-0-25-0"
            || tree_sitter.provider_image_fingerprint != provider_image_fingerprint
        {
            return self.reject(RuffAdapterError::MismatchedTreeSitterEvidence);
        }
        let boundary_map = match ProviderBoundaryMap::new(&text) {
            Ok(map) => map,
            Err(error) => return self.reject(error.into()),
        };
        if cancellation.is_cancelled() {
            return self.reject(RuffAdapterError::Cancelled);
        }

        let started = Instant::now();
        // Ruff parsing is not incrementally interruptible. We therefore check on
        // both sides and discard the complete result if cancellation or deadline
        // wins while the library call is running.
        let parse_started = Instant::now();
        let Some(parsed) = parse_unchecked(
            &text.text,
            ParseOptions::from(PySourceType::Python).with_target_version(PythonVersion::PY314),
        )
        .try_into_module() else {
            return self.reject(RuffAdapterError::ProjectionInvariant(
                "Python module parse options produced a non-module root".into(),
            ));
        };
        let parse_duration = parse_started.elapsed();
        if cancellation.is_cancelled() {
            return self.reject(RuffAdapterError::Cancelled);
        }
        self.check_progress(started, 1, cancellation)?;

        let trivia = TriviaRanges::from(parsed.tokens());
        let indexer = Indexer::from_tokens(parsed.tokens(), &text.text);
        let line_index = LineIndex::from_source_text(&text.text);
        let projection_started = Instant::now();
        let mut work_units = 1_u64;

        let mut tokens = match project_tokens(
            &parsed,
            &line_index,
            &boundary_map,
            &text.text,
            &mut work_units,
            self.limits,
            started,
            cancellation,
        ) {
            Ok(tokens) => tokens,
            Err(error) => return self.reject(error),
        };
        let evaluation_ordinals = evaluation_ordinals(&parsed);
        let mut visitor = AstProjectionVisitor::new(
            &trivia,
            &line_index,
            &boundary_map,
            &text.text,
            &evaluation_ordinals,
            self.limits,
            started,
            cancellation,
            work_units,
        );
        source_order::walk_node(&mut visitor, AnyNodeRef::from(parsed.syntax()));
        let (ast, ast_work_units) = match visitor.finish() {
            Ok(output) => output,
            Err(error) => return self.reject(error),
        };
        work_units = ast_work_units;
        link_tokens_to_ast(&mut tokens, &ast);

        let comments = match project_comments(&indexer, &boundary_map, &text.text) {
            Ok(comments) => comments,
            Err(error) => return self.reject(error),
        };
        let directives =
            match project_directives(&indexer, &boundary_map, &line_index, &text.text, &ast) {
                Ok(directives) => directives,
                Err(error) => return self.reject(error),
            };
        let strings = match project_strings(&parsed, &indexer, &boundary_map, &ast) {
            Ok(strings) => strings,
            Err(error) => return self.reject(error),
        };
        let docstrings = match project_docstrings(&parsed, &boundary_map, &ast) {
            Ok(docstrings) => docstrings,
            Err(error) => return self.reject(error),
        };
        let continuation_line_starts = match indexer
            .continuation_line_starts()
            .iter()
            .map(|offset| boundary_map.original(usize::from(*offset)))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(offsets) => offsets,
            Err(error) => return self.reject(error.into()),
        };
        let diagnostics = match project_diagnostics(&parsed, tree_sitter, &boundary_map) {
            Ok(diagnostics) => diagnostics,
            Err(error) => return self.reject(error),
        };
        let correspondences = project_correspondences(&ast, &tree_sitter.facts);
        work_units = work_units
            .saturating_add(u64::try_from(comments.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(directives.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(strings.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(docstrings.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(continuation_line_starts.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(diagnostics.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(correspondences.len()).unwrap_or(u64::MAX));
        self.check_progress(started, work_units, cancellation)?;

        let output_records = sum_lengths(&[
            1,
            tokens.len(),
            ast.len(),
            comments.len(),
            directives.len(),
            strings.len(),
            docstrings.len(),
            continuation_line_starts.len(),
            diagnostics.len(),
            correspondences.len(),
        ]);
        if output_records > self.limits.max_output_records {
            return self.reject(RuffAdapterError::OutputRecordLimit);
        }
        if diagnostics.len() > usize::from(self.limits.max_diagnostics) {
            return self.reject(RuffAdapterError::DiagnosticLimit);
        }
        let output_bytes = estimate_output_bytes(
            &tokens,
            &ast,
            &comments,
            &directives,
            &strings,
            &docstrings,
            &continuation_line_starts,
            &diagnostics,
            &correspondences,
            &provider_image_fingerprint,
        );
        if output_bytes > self.limits.max_output_bytes {
            return self.reject(RuffAdapterError::OutputByteLimit);
        }
        let run_metrics = RuffRunMetrics {
            parse_duration,
            projection_duration: projection_started.elapsed(),
            visited_nodes: u64::try_from(ast.len()).unwrap_or(u64::MAX),
            token_count: u64::try_from(tokens.len()).unwrap_or(u64::MAX),
            output_records,
            output_bytes,
            work_units,
        };
        let source_start = match boundary_map.original(0) {
            Ok(source_start) => source_start,
            Err(error) => return self.reject(error.into()),
        };
        let source_end = match boundary_map.original(text.text.len()) {
            Ok(source_end) => source_end,
            Err(error) => return self.reject(error.into()),
        };
        let snapshot = RuffSnapshot {
            revision,
            source: RuffSourceFact {
                provider_image_fingerprint,
                start_byte: source_start,
                end_byte: source_end,
                line_count: u64::try_from(line_index.line_count()).unwrap_or(u64::MAX),
            },
            catalog_id: self.inventory.catalog_id,
            provider_version: self.inventory.provider_version,
            runtime_inventory_fingerprint: self.inventory.runtime_inventory_fingerprint,
            tokens: tokens.into(),
            ast: ast.into(),
            comments: comments.into(),
            directives: directives.into(),
            strings: strings.into(),
            docstrings: docstrings.into(),
            continuation_line_starts: continuation_line_starts.into(),
            diagnostics: diagnostics.into(),
            correspondences: correspondences.into(),
            metrics: run_metrics,
        };
        self.retained = Some(RetainedRuffRevision {
            revision,
            text,
            parsed,
            trivia,
            indexer,
            line_index,
            snapshot: snapshot.clone(),
        });
        self.metrics.completed_runs = self.metrics.completed_runs.saturating_add(1);
        self.metrics.retained_revisions = 1;
        self.metrics.last_run = Some(run_metrics);
        Ok(snapshot)
    }

    /// Last atomically committed complete revision.
    #[must_use]
    pub fn active_snapshot(&self) -> Option<&RuffSnapshot> {
        self.retained.as_ref().map(|retained| &retained.snapshot)
    }

    /// Application-owned proof that the one retained parse and its three indexes
    /// correspond to the active source. No Ruff type crosses this boundary.
    #[must_use]
    pub fn active_index_summary(&self) -> Option<RuffIndexSummary> {
        self.retained.as_ref().map(|retained| RuffIndexSummary {
            source_bytes: u64::try_from(retained.text.text.len()).unwrap_or(u64::MAX),
            token_count: u64::try_from(retained.parsed.tokens().len()).unwrap_or(u64::MAX),
            comment_count: u64::try_from(retained.trivia.comments().len()).unwrap_or(u64::MAX),
            indexed_comment_count: u64::try_from(retained.indexer.comment_ranges().len())
                .unwrap_or(u64::MAX),
            line_count: u64::try_from(retained.line_index.line_count()).unwrap_or(u64::MAX),
        })
    }

    /// Current operational counters.
    #[must_use]
    pub const fn metrics(&self) -> RuffAdapterMetrics {
        self.metrics
    }

    /// Exact generated Ruff inventory validated at startup.
    #[must_use]
    pub const fn inventory(&self) -> &'static RuffPythonInventory {
        self.inventory
    }

    fn check_progress(
        &mut self,
        started: Instant,
        work_units: u64,
        cancellation: &impl RuffCancellation,
    ) -> Result<(), RuffAdapterError> {
        let effective_interval = self
            .limits
            .cancellation_check_interval
            .min(cancellation.check_interval())
            .max(1);
        if work_units.is_multiple_of(u64::from(effective_interval)) && cancellation.is_cancelled() {
            return self.reject(RuffAdapterError::Cancelled);
        }
        if deadline_exceeded(started, self.limits.max_wall_millis) {
            return self.reject(RuffAdapterError::Deadline);
        }
        if work_units > self.limits.max_work_units {
            return self.reject(RuffAdapterError::WorkLimit);
        }
        Ok(())
    }

    fn reject<T>(&mut self, error: RuffAdapterError) -> Result<T, RuffAdapterError> {
        self.metrics.rejected_runs = self.metrics.rejected_runs.saturating_add(1);
        if error == RuffAdapterError::Cancelled {
            self.metrics.cancelled_runs = self.metrics.cancelled_runs.saturating_add(1);
        }
        Err(error)
    }
}

/// Retained parse/index counts without exposing Ruff ownership types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuffIndexSummary {
    pub source_bytes: u64,
    pub token_count: u64,
    pub comment_count: u64,
    pub indexed_comment_count: u64,
    pub line_count: u64,
}

fn validate_runtime_inventory(inventory: &RuffPythonInventory) -> Result<(), RuffAdapterError> {
    if inventory.catalog_id != RUFF_PYTHON_FRONTEND.catalog_id
        || inventory.provider_version != RUFF_PYTHON_FRONTEND.provider_version
        || inventory.runtime_inventory_fingerprint
            != RUFF_PYTHON_FRONTEND.runtime_inventory_fingerprint
        || inventory.node_kinds != RUFF_PYTHON_FRONTEND.node_kinds
        || inventory.token_kinds != RUFF_PYTHON_FRONTEND.token_kinds
    {
        return Err(RuffAdapterError::ProviderVersionMismatch(
            "generated Ruff inventory identity drifted".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // These are borrowed, once-built parse indexes and packet limits, not independent options.
fn project_tokens(
    parsed: &Parsed<ruff_python_ast::ModModule>,
    line_index: &LineIndex,
    boundary_map: &ProviderBoundaryMap,
    source: &str,
    work_units: &mut u64,
    limits: RuffLimits,
    started: Instant,
    cancellation: &impl RuffCancellation,
) -> Result<Vec<RuffTokenFact>, RuffAdapterError> {
    let mut output = Vec::with_capacity(parsed.tokens().len());
    let interval = limits
        .cancellation_check_interval
        .min(cancellation.check_interval())
        .max(1);
    for (ordinal, token) in parsed.tokens().iter().enumerate() {
        *work_units = work_units.saturating_add(1);
        if work_units.is_multiple_of(u64::from(interval)) && cancellation.is_cancelled() {
            return Err(RuffAdapterError::Cancelled);
        }
        if deadline_exceeded(started, limits.max_wall_millis) {
            return Err(RuffAdapterError::Deadline);
        }
        let entry = ruff_python_token_kind_entry(token.kind());
        let start = usize::from(token.start());
        let end = usize::from(token.end());
        let location = line_index.line_column(token.start(), source);
        let class = token_class(token.kind());
        output.push(RuffTokenFact {
            ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
            raw_kind_id: entry.raw_kind_id,
            raw_kind: entry.raw_name,
            class,
            start_byte: boundary_map.original(start)?,
            end_byte: boundary_map.original(end)?,
            line: u32::try_from(location.line.get()).unwrap_or(u32::MAX),
            column: u32::try_from(location.column.get()).unwrap_or(u32::MAX),
            spelling: token_spelling(
                class,
                source
                    .get(start..end)
                    .ok_or(RuffAdapterError::InvalidSpan)?,
            ),
            syntax_id: None,
        });
    }
    Ok(output)
}

fn token_spelling(class: RuffTokenClass, spelling: &str) -> Option<RuffTokenSpelling> {
    match class {
        RuffTokenClass::Identifier | RuffTokenClass::Keyword => {
            Some(RuffTokenSpelling::Slice(spelling.to_owned()))
        }
        RuffTokenClass::Literal => {
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"codefabric:python-literal-token-spelling:v1\0");
            hasher.update(
                &u64::try_from(spelling.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(spelling.as_bytes());
            Some(RuffTokenSpelling::Blake3(format!(
                "b3:{}",
                hasher.finalize()
            )))
        }
        _ => None,
    }
}

fn link_tokens_to_ast(tokens: &mut [RuffTokenFact], ast: &[RuffAstFact]) {
    for token in tokens {
        token.syntax_id = ast
            .iter()
            .filter(|fact| {
                fact.start_byte <= token.start_byte
                    && fact.end_byte >= token.end_byte
                    && token_ast_compatible(token.class, fact)
            })
            .min_by_key(|fact| {
                (
                    u8::from(
                        fact.start_byte != token.start_byte || fact.end_byte != token.end_byte,
                    ),
                    fact.end_byte.saturating_sub(fact.start_byte),
                    fact.id,
                )
            })
            .map(|fact| fact.id);
    }
}

fn token_ast_compatible(class: RuffTokenClass, fact: &RuffAstFact) -> bool {
    match class {
        RuffTokenClass::Identifier => matches!(fact.raw_kind, "Identifier" | "ExprName"),
        RuffTokenClass::Literal => fact.category == RuffAstCategory::Literal,
        RuffTokenClass::Operator => matches!(
            fact.category,
            RuffAstCategory::Operation
                | RuffAstCategory::Assignment
                | RuffAstCategory::AttributeAccess
                | RuffAstCategory::SubscriptAccess
                | RuffAstCategory::CallExpression
        ),
        RuffTokenClass::Keyword => matches!(
            fact.category,
            RuffAstCategory::Statement
                | RuffAstCategory::DeclarationSyntax
                | RuffAstCategory::Branch
                | RuffAstCategory::Loop
                | RuffAstCategory::Return
                | RuffAstCategory::Yield
                | RuffAstCategory::Await
                | RuffAstCategory::RaiseSyntax
                | RuffAstCategory::ImportSyntax
        ),
        _ => false,
    }
}

fn token_class(kind: TokenKind) -> RuffTokenClass {
    if kind.is_operator() {
        RuffTokenClass::Operator
    } else {
        match kind {
            TokenKind::Name => RuffTokenClass::Identifier,
            TokenKind::Int
            | TokenKind::Float
            | TokenKind::Complex
            | TokenKind::String
            | TokenKind::FStringStart
            | TokenKind::FStringMiddle
            | TokenKind::FStringEnd
            | TokenKind::TStringStart
            | TokenKind::TStringMiddle
            | TokenKind::TStringEnd => RuffTokenClass::Literal,
            TokenKind::Comment => RuffTokenClass::Comment,
            TokenKind::Newline | TokenKind::NonLogicalNewline => RuffTokenClass::Newline,
            TokenKind::Indent | TokenKind::Dedent => RuffTokenClass::Indentation,
            TokenKind::EndOfFile => RuffTokenClass::EndOfFile,
            _ => {
                if kind.is_keyword() {
                    RuffTokenClass::Keyword
                } else {
                    RuffTokenClass::Unknown
                }
            }
        }
    }
}

type NodeKey = (u32, u32, &'static str);

fn node_key(node: AnyNodeRef<'_>) -> NodeKey {
    let entry = ruff_python_node_kind_entry(node.kind());
    (node.start().to_u32(), node.end().to_u32(), entry.raw_name)
}

fn evaluation_ordinals(parsed: &Parsed<ruff_python_ast::ModModule>) -> BTreeMap<NodeKey, u32> {
    struct EvalVisitor {
        next: u32,
        ordinals: BTreeMap<NodeKey, u32>,
    }

    impl EvalVisitor {
        fn record(&mut self, node: AnyNodeRef<'_>) {
            self.ordinals.entry(node_key(node)).or_insert_with(|| {
                let current = self.next;
                self.next = self.next.saturating_add(1);
                current
            });
        }
    }

    impl<'a> Visitor<'a> for EvalVisitor {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            self.record(AnyNodeRef::from(stmt));
            visitor::walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &'a ruff_python_ast::Expr) {
            self.record(AnyNodeRef::from(expr));
            visitor::walk_expr(self, expr);
        }
    }

    let mut visitor = EvalVisitor {
        next: 0,
        ordinals: BTreeMap::new(),
    };
    visitor.visit_body(parsed.suite());
    visitor.ordinals
}

struct AstProjectionVisitor<'a, C> {
    trivia: &'a TriviaRanges,
    line_index: &'a LineIndex,
    boundary_map: &'a ProviderBoundaryMap,
    source: &'a str,
    evaluation_ordinals: &'a BTreeMap<NodeKey, u32>,
    limits: RuffLimits,
    started: Instant,
    cancellation: &'a C,
    stack: Vec<RuffOccurrenceId>,
    parent_nodes: Vec<AnyNodeRef<'a>>,
    child_counts: Vec<u32>,
    output: Vec<RuffAstFact>,
    work_units: u64,
    error: Option<RuffAdapterError>,
}

impl<'a, C: RuffCancellation> AstProjectionVisitor<'a, C> {
    #[allow(clippy::too_many_arguments)] // The visitor borrows the complete one-run projection context without cloning it.
    fn new(
        trivia: &'a TriviaRanges,
        line_index: &'a LineIndex,
        boundary_map: &'a ProviderBoundaryMap,
        source: &'a str,
        evaluation_ordinals: &'a BTreeMap<NodeKey, u32>,
        limits: RuffLimits,
        started: Instant,
        cancellation: &'a C,
        work_units: u64,
    ) -> Self {
        Self {
            trivia,
            line_index,
            boundary_map,
            source,
            evaluation_ordinals,
            limits,
            started,
            cancellation,
            stack: Vec::new(),
            parent_nodes: Vec::new(),
            child_counts: Vec::new(),
            output: Vec::new(),
            work_units,
            error: None,
        }
    }

    fn finish(self) -> Result<(Vec<RuffAstFact>, u64), RuffAdapterError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok((self.output, self.work_units)),
        }
    }

    fn fail(&mut self, error: RuffAdapterError) -> TraversalSignal {
        self.error.get_or_insert(error);
        TraversalSignal::Skip
    }
}

impl<'a, C: RuffCancellation> SourceOrderVisitor<'a> for AstProjectionVisitor<'a, C> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        if self.error.is_some() {
            return TraversalSignal::Skip;
        }
        self.work_units = self.work_units.saturating_add(1);
        let interval = self
            .limits
            .cancellation_check_interval
            .min(self.cancellation.check_interval())
            .max(1);
        if self.work_units.is_multiple_of(u64::from(interval)) && self.cancellation.is_cancelled() {
            return self.fail(RuffAdapterError::Cancelled);
        }
        if deadline_exceeded(self.started, self.limits.max_wall_millis) {
            return self.fail(RuffAdapterError::Deadline);
        }
        if self.work_units > self.limits.max_work_units {
            return self.fail(RuffAdapterError::WorkLimit);
        }
        if self.output.len() >= usize::try_from(self.limits.max_visited_nodes).unwrap_or(usize::MAX)
        {
            return self.fail(RuffAdapterError::NodeLimit);
        }
        if self.stack.len() >= usize::from(self.limits.max_traversal_depth) {
            return self.fail(RuffAdapterError::DepthLimit);
        }

        let entry = ruff_python_node_kind_entry(node.kind());
        let id = RuffOccurrenceId(u64::try_from(self.output.len()).unwrap_or(u64::MAX));
        let location = self.line_index.line_column(node.start(), self.source);
        let start_byte = match self.boundary_map.original(usize::from(node.start())) {
            Ok(value) => value,
            Err(error) => return self.fail(error.into()),
        };
        let end_byte = match self.boundary_map.original(usize::from(node.end())) {
            Ok(value) => value,
            Err(error) => return self.fail(error.into()),
        };
        let Some(category) = RuffAstCategory::from_registry_code(entry.normalized_kind_code) else {
            return self.fail(RuffAdapterError::ProjectionInvariant(format!(
                "generated Ruff raw kind {} resolves outside the GEN 16.1 syntax set",
                entry.raw_name
            )));
        };
        if entry.disposition != ProviderRawKindDisposition::Normalize {
            return self.fail(RuffAdapterError::ProjectionInvariant(format!(
                "generated Ruff raw kind {} is not normalized",
                entry.raw_name
            )));
        }
        let parent_node = self.parent_nodes.last().copied();
        let child_ordinal = self.child_counts.last().copied().unwrap_or(0);
        if let Some(next_ordinal) = self.child_counts.last_mut() {
            *next_ordinal = next_ordinal.saturating_add(1);
        }
        self.output.push(RuffAstFact {
            id,
            raw_kind_id: entry.raw_kind_id,
            raw_kind: entry.raw_name,
            category,
            disposition: entry.disposition,
            start_byte,
            end_byte,
            line: u32::try_from(location.line.get()).unwrap_or(u32::MAX),
            column: u32::try_from(location.column.get()).unwrap_or(u32::MAX),
            parent: self.stack.last().copied(),
            child_role: parent_node.map(|parent| child_role(parent, node, category)),
            child_ordinal,
            source_ordinal: u32::try_from(id.0).unwrap_or(u32::MAX),
            evaluation_ordinal: self.evaluation_ordinals.get(&node_key(node)).copied(),
            explicit_parenthesized: self.trivia.parenthesized().contains(node.range()),
        });
        self.stack.push(id);
        self.parent_nodes.push(node);
        self.child_counts.push(0);
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, _node: AnyNodeRef<'a>) {
        if self.error.is_some() {
            return;
        }
        self.stack.pop();
        self.parent_nodes.pop();
        self.child_counts.pop();
    }
}

fn child_role(
    parent: AnyNodeRef<'_>,
    child: AnyNodeRef<'_>,
    child_category: RuffAstCategory,
) -> RuffChildRole {
    let child_kind = child.kind();
    match child_kind {
        NodeKind::Decorator => RuffChildRole::Decorator,
        NodeKind::Identifier => RuffChildRole::Name,
        NodeKind::TypeParams
        | NodeKind::TypeParamTypeVar
        | NodeKind::TypeParamTypeVarTuple
        | NodeKind::TypeParamParamSpec => RuffChildRole::TypeParameter,
        NodeKind::Parameters | NodeKind::Parameter | NodeKind::ParameterWithDefault => {
            RuffChildRole::Parameter
        }
        NodeKind::Arguments => RuffChildRole::Argument,
        NodeKind::Keyword => RuffChildRole::KeywordArgument,
        NodeKind::ExceptHandlerExceptHandler => RuffChildRole::Handler,
        NodeKind::ElifElseClause => RuffChildRole::Clause,
        NodeKind::WithItem => RuffChildRole::Item,
        NodeKind::InterpolatedElement
        | NodeKind::InterpolatedStringLiteralElement
        | NodeKind::InterpolatedStringFormatSpec
        | NodeKind::FString
        | NodeKind::TString
        | NodeKind::StringLiteral
        | NodeKind::BytesLiteral => RuffChildRole::Segment,
        _ => match child_category {
            RuffAstCategory::Pattern => RuffChildRole::Pattern,
            _ if node_kind_is_statement(child_kind) => RuffChildRole::Body,
            _ if is_target_child(parent, child) => RuffChildRole::Target,
            _ if is_condition_child(parent, child) => RuffChildRole::Condition,
            _ if is_callee_child(parent, child) => RuffChildRole::Callee,
            _ if is_annotation_child(parent, child) => RuffChildRole::Annotation,
            _ if is_iterable_child(parent, child) => RuffChildRole::Iterable,
            _ if is_value_child(parent, child) => RuffChildRole::Value,
            _ => RuffChildRole::Child,
        },
    }
}

fn same_node(left: AnyNodeRef<'_>, right: AnyNodeRef<'_>) -> bool {
    left.as_ptr() == right.as_ptr()
}

fn is_target_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    match parent {
        AnyNodeRef::StmtAssign(node) => node
            .targets
            .iter()
            .any(|target| same_node(child, AnyNodeRef::from(target))),
        AnyNodeRef::StmtAugAssign(node) => same_node(child, AnyNodeRef::from(node.target.as_ref())),
        AnyNodeRef::StmtAnnAssign(node) => same_node(child, AnyNodeRef::from(node.target.as_ref())),
        AnyNodeRef::StmtFor(node) => same_node(child, AnyNodeRef::from(node.target.as_ref())),
        AnyNodeRef::StmtDelete(node) => node
            .targets
            .iter()
            .any(|target| same_node(child, AnyNodeRef::from(target))),
        AnyNodeRef::StmtTypeAlias(node) => same_node(child, AnyNodeRef::from(node.name.as_ref())),
        _ => false,
    }
}

fn is_condition_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    match parent {
        AnyNodeRef::StmtIf(node) => same_node(child, AnyNodeRef::from(node.test.as_ref())),
        AnyNodeRef::StmtWhile(node) => same_node(child, AnyNodeRef::from(node.test.as_ref())),
        AnyNodeRef::StmtAssert(node) => same_node(child, AnyNodeRef::from(node.test.as_ref())),
        AnyNodeRef::ExprIf(node) => same_node(child, AnyNodeRef::from(node.test.as_ref())),
        AnyNodeRef::StmtMatch(node) => same_node(child, AnyNodeRef::from(node.subject.as_ref())),
        _ => false,
    }
}

fn is_callee_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    matches!(parent, AnyNodeRef::ExprCall(node) if same_node(child, AnyNodeRef::from(node.func.as_ref())))
}

fn is_annotation_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    match parent {
        AnyNodeRef::StmtAnnAssign(node) => {
            same_node(child, AnyNodeRef::from(node.annotation.as_ref()))
        }
        AnyNodeRef::StmtFunctionDef(node) => node
            .returns
            .as_deref()
            .is_some_and(|returns| same_node(child, AnyNodeRef::from(returns))),
        AnyNodeRef::Parameter(node) => node
            .annotation
            .as_deref()
            .is_some_and(|annotation| same_node(child, AnyNodeRef::from(annotation))),
        _ => false,
    }
}

fn is_iterable_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    matches!(parent, AnyNodeRef::StmtFor(node) if same_node(child, AnyNodeRef::from(node.iter.as_ref())))
}

fn is_value_child(parent: AnyNodeRef<'_>, child: AnyNodeRef<'_>) -> bool {
    match parent {
        AnyNodeRef::StmtAssign(node) => same_node(child, AnyNodeRef::from(node.value.as_ref())),
        AnyNodeRef::StmtAugAssign(node) => same_node(child, AnyNodeRef::from(node.value.as_ref())),
        AnyNodeRef::StmtAnnAssign(node) => node
            .value
            .as_deref()
            .is_some_and(|value| same_node(child, AnyNodeRef::from(value))),
        AnyNodeRef::StmtReturn(node) => node
            .value
            .as_deref()
            .is_some_and(|value| same_node(child, AnyNodeRef::from(value))),
        AnyNodeRef::StmtRaise(node) => node
            .exc
            .as_deref()
            .is_some_and(|value| same_node(child, AnyNodeRef::from(value))),
        AnyNodeRef::StmtTypeAlias(node) => same_node(child, AnyNodeRef::from(node.value.as_ref())),
        AnyNodeRef::ExprAwait(node) => same_node(child, AnyNodeRef::from(node.value.as_ref())),
        AnyNodeRef::ExprYield(node) => node
            .value
            .as_deref()
            .is_some_and(|value| same_node(child, AnyNodeRef::from(value))),
        AnyNodeRef::ExprYieldFrom(node) => same_node(child, AnyNodeRef::from(node.value.as_ref())),
        _ => false,
    }
}

const fn node_kind_is_statement(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StmtFunctionDef
            | NodeKind::StmtClassDef
            | NodeKind::StmtReturn
            | NodeKind::StmtDelete
            | NodeKind::StmtTypeAlias
            | NodeKind::StmtAssign
            | NodeKind::StmtAugAssign
            | NodeKind::StmtAnnAssign
            | NodeKind::StmtFor
            | NodeKind::StmtWhile
            | NodeKind::StmtIf
            | NodeKind::StmtWith
            | NodeKind::StmtMatch
            | NodeKind::StmtRaise
            | NodeKind::StmtTry
            | NodeKind::StmtAssert
            | NodeKind::StmtImport
            | NodeKind::StmtImportFrom
            | NodeKind::StmtGlobal
            | NodeKind::StmtNonlocal
            | NodeKind::StmtExpr
            | NodeKind::StmtPass
            | NodeKind::StmtBreak
            | NodeKind::StmtContinue
            | NodeKind::StmtIpyEscapeCommand
    )
}

fn project_comments(
    indexer: &Indexer,
    boundary_map: &ProviderBoundaryMap,
    source: &str,
) -> Result<Vec<RuffCommentFact>, RuffAdapterError> {
    let all = TextRange::new(
        TextSize::new(0),
        TextSize::try_from(source.len()).map_err(|_| RuffAdapterError::InputLimit)?,
    );
    let block_starts = indexer.comment_ranges().block_comments(source);
    indexer
        .comment_ranges()
        .comments_in_range(all)
        .iter()
        .map(|range| {
            let placement = match CommentLinePosition::for_range(*range, source) {
                CommentLinePosition::OwnLine => RuffCommentPlacement::OwnLine,
                CommentLinePosition::EndOfLine => RuffCommentPlacement::EndOfLine,
            };
            Ok(RuffCommentFact {
                start_byte: boundary_map.original(usize::from(range.start()))?,
                end_byte: boundary_map.original(usize::from(range.end()))?,
                placement,
                block_member: block_starts.binary_search(&range.start()).is_ok(),
            })
        })
        .collect()
}

fn project_directives(
    indexer: &Indexer,
    boundary_map: &ProviderBoundaryMap,
    line_index: &LineIndex,
    source: &str,
    ast: &[RuffAstFact],
) -> Result<Vec<RuffDirectiveFact>, RuffAdapterError> {
    let all = TextRange::new(
        TextSize::new(0),
        TextSize::try_from(source.len()).map_err(|_| RuffAdapterError::InputLimit)?,
    );
    let mut output = Vec::new();
    for range in indexer.comment_ranges().comments_in_range(all) {
        let comment = &source[*range];
        let lower = comment.to_ascii_lowercase();
        let kind = if lower
            .trim_start_matches('#')
            .trim_start()
            .starts_with("noqa")
        {
            Some(RuffDirectiveKind::Noqa)
        } else if lower.contains("type: ignore") {
            Some(RuffDirectiveKind::TypeIgnore)
        } else if lower
            .trim_start_matches('#')
            .trim_start()
            .starts_with("type:")
        {
            Some(RuffDirectiveKind::TypeComment)
        } else if SuppressionKind::from_comment(comment).is_some() {
            Some(RuffDirectiveKind::Formatter)
        } else if is_pragma_comment(comment) {
            Some(RuffDirectiveKind::OtherPragma)
        } else {
            None
        };
        if let Some(kind) = kind {
            let start_byte = boundary_map.original(usize::from(range.start()))?;
            let end_byte = boundary_map.original(usize::from(range.end()))?;
            let line =
                u32::try_from(line_index.line_index(range.start()).get()).unwrap_or(u32::MAX);
            let placement = CommentLinePosition::for_range(*range, source);
            output.push(RuffDirectiveFact {
                kind,
                start_byte,
                end_byte,
                target: directive_target(ast, placement, line, start_byte, end_byte),
            });
        }
    }
    Ok(output)
}

fn directive_target(
    ast: &[RuffAstFact],
    placement: CommentLinePosition,
    line: u32,
    start_byte: u64,
    end_byte: u64,
) -> Option<RuffOccurrenceId> {
    match placement {
        CommentLinePosition::EndOfLine => ast
            .iter()
            .filter(|fact| fact.line == line && fact.end_byte <= start_byte)
            .max_by_key(|fact| {
                (
                    fact.end_byte,
                    std::cmp::Reverse(fact.end_byte.saturating_sub(fact.start_byte)),
                )
            })
            .map(|fact| fact.id),
        CommentLinePosition::OwnLine => ast
            .iter()
            .filter(|fact| fact.start_byte >= end_byte)
            .min_by_key(|fact| {
                (
                    fact.start_byte,
                    fact.end_byte.saturating_sub(fact.start_byte),
                    fact.id,
                )
            })
            .map(|fact| fact.id),
    }
}

fn project_strings(
    parsed: &Parsed<ruff_python_ast::ModModule>,
    indexer: &Indexer,
    boundary_map: &ProviderBoundaryMap,
    ast: &[RuffAstFact],
) -> Result<Vec<RuffStringRegion>, RuffAdapterError> {
    let mut output = Vec::new();
    for token in parsed.tokens() {
        if token.kind() == TokenKind::String {
            let start_byte = boundary_map.original(usize::from(token.start()))?;
            let end_byte = boundary_map.original(usize::from(token.end()))?;
            output.push(RuffStringRegion {
                start_byte,
                end_byte,
                multiline: indexer.multiline_ranges().contains_range(token.range()),
                interpolated: false,
                syntax_id: string_syntax_id(ast, start_byte, end_byte),
            });
        }
    }
    for range in indexer.interpolated_string_ranges().values() {
        let start_byte = boundary_map.original(usize::from(range.start()))?;
        let end_byte = boundary_map.original(usize::from(range.end()))?;
        output.push(RuffStringRegion {
            start_byte,
            end_byte,
            multiline: indexer.multiline_ranges().intersects(*range),
            interpolated: true,
            syntax_id: string_syntax_id(ast, start_byte, end_byte),
        });
    }
    output.sort_by_key(|region| (region.start_byte, region.end_byte));
    Ok(output)
}

fn string_syntax_id(
    ast: &[RuffAstFact],
    start_byte: u64,
    end_byte: u64,
) -> Option<RuffOccurrenceId> {
    ast.iter()
        .filter(|fact| {
            fact.category == RuffAstCategory::Literal
                && fact.start_byte <= start_byte
                && fact.end_byte >= end_byte
        })
        .min_by_key(|fact| (fact.end_byte.saturating_sub(fact.start_byte), fact.id))
        .map(|fact| fact.id)
}

fn project_docstrings(
    parsed: &Parsed<ruff_python_ast::ModModule>,
    boundary_map: &ProviderBoundaryMap,
    ast: &[RuffAstFact],
) -> Result<Vec<RuffDocstringFact>, RuffAdapterError> {
    struct DocstringVisitor {
        output: Vec<(TextRange, NodeKey)>,
    }

    impl<'a> SourceOrderVisitor<'a> for DocstringVisitor {
        fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
            let first_statement = match node {
                AnyNodeRef::ModModule(module) => module.body.first(),
                AnyNodeRef::StmtFunctionDef(function) => function.body.first(),
                AnyNodeRef::StmtClassDef(class) => class.body.first(),
                _ => None,
            };
            if let Some(statement) =
                first_statement.filter(|stmt| ruff_python_ast::helpers::is_docstring_stmt(stmt))
            {
                self.output.push((statement.range(), node_key(node)));
            }
            TraversalSignal::Traverse
        }
    }

    let mut visitor = DocstringVisitor { output: Vec::new() };
    source_order::walk_node(&mut visitor, AnyNodeRef::from(parsed.syntax()));
    visitor
        .output
        .into_iter()
        .map(|(range, owner_key)| {
            let owner_start = boundary_map.original(
                usize::try_from(owner_key.0).map_err(|_| RuffAdapterError::InvalidSpan)?,
            )?;
            let owner_end = boundary_map.original(
                usize::try_from(owner_key.1).map_err(|_| RuffAdapterError::InvalidSpan)?,
            )?;
            let owner = ast
                .iter()
                .find(|fact| {
                    fact.start_byte == owner_start
                        && fact.end_byte == owner_end
                        && fact.raw_kind == owner_key.2
                })
                .map(|fact| fact.id)
                .ok_or_else(|| {
                    RuffAdapterError::ProjectionInvariant(
                        "docstring semantic owner is absent from the AST projection".into(),
                    )
                })?;
            Ok(RuffDocstringFact {
                start_byte: boundary_map.original(usize::from(range.start()))?,
                end_byte: boundary_map.original(usize::from(range.end()))?,
                owner,
            })
        })
        .collect()
}

fn project_diagnostics(
    parsed: &Parsed<ruff_python_ast::ModModule>,
    tree_sitter: &TreeSitterSnapshot,
    boundary_map: &ProviderBoundaryMap,
) -> Result<Vec<RuffDiagnosticFact>, RuffAdapterError> {
    let mut output = Vec::new();
    for error in parsed.errors() {
        output.push(diagnostic(
            RuffDiagnosticKind::Parse,
            error.to_string(),
            error.range(),
            tree_sitter,
            boundary_map,
        )?);
    }
    for error in parsed.unsupported_syntax_errors() {
        output.push(diagnostic(
            RuffDiagnosticKind::UnsupportedSyntax,
            format!("{:?}", error.kind),
            error.range(),
            tree_sitter,
            boundary_map,
        )?);
    }
    Ok(output)
}

fn diagnostic(
    kind: RuffDiagnosticKind,
    message: String,
    range: TextRange,
    tree_sitter: &TreeSitterSnapshot,
    boundary_map: &ProviderBoundaryMap,
) -> Result<RuffDiagnosticFact, RuffAdapterError> {
    let start_byte = boundary_map.original(usize::from(range.start()))?;
    let end_byte = boundary_map.original(usize::from(range.end()))?;
    let tree_sitter_recovery_ids = tree_sitter
        .facts
        .iter()
        .filter(|fact| {
            (fact.error || fact.missing)
                && ranges_overlap(start_byte, end_byte, fact.start_byte, fact.end_byte)
        })
        .map(|fact| fact.id)
        .collect::<Vec<_>>();
    Ok(RuffDiagnosticFact {
        kind,
        message,
        start_byte,
        end_byte,
        tree_sitter_recovery_ids: tree_sitter_recovery_ids.into(),
    })
}

fn project_correspondences(
    ast: &[RuffAstFact],
    tree: &[RawSyntaxFact],
) -> Vec<RuffTreeCorrespondence> {
    ast.iter()
        .filter_map(|ruff| {
            tree.iter()
                .filter(|fact| {
                    fact.named
                        && !fact.extra
                        && !fact.error
                        && !fact.missing
                        && fact.start_byte <= ruff.start_byte
                        && fact.end_byte >= ruff.end_byte
                        && (fact.normalized_kind.0 == ruff.category.registry_code()
                            || fact.normalized_kind.0
                                == RuffAstCategory::SyntaxNode.registry_code())
                        && tree_field_compatible(ruff.child_role, fact.field_name.as_deref())
                })
                .min_by_key(|fact| {
                    (
                        fact.end_byte.saturating_sub(fact.start_byte),
                        u8::from(fact.normalized_kind.0 != ruff.category.registry_code()),
                        fact.id,
                    )
                })
                .map(|fact| RuffTreeCorrespondence {
                    ruff_id: ruff.id,
                    tree_sitter_id: fact.id,
                })
        })
        .collect()
}

fn tree_field_compatible(role: Option<RuffChildRole>, field_name: Option<&str>) -> bool {
    let Some(role) = role else {
        return field_name.is_none();
    };
    match role {
        RuffChildRole::Body => matches!(
            field_name,
            None | Some("body" | "consequence" | "alternative")
        ),
        RuffChildRole::Decorator => matches!(field_name, None | Some("decorator")),
        RuffChildRole::Name => matches!(field_name, None | Some("name")),
        RuffChildRole::TypeParameter => {
            matches!(field_name, None | Some("type_parameters" | "type"))
        }
        RuffChildRole::Parameter => matches!(field_name, None | Some("parameters")),
        RuffChildRole::Argument | RuffChildRole::KeywordArgument => {
            matches!(field_name, None | Some("arguments"))
        }
        RuffChildRole::Callee => matches!(field_name, None | Some("function")),
        RuffChildRole::Condition => matches!(field_name, None | Some("condition")),
        RuffChildRole::Target => matches!(field_name, None | Some("left" | "target")),
        RuffChildRole::Value => matches!(field_name, None | Some("right" | "value")),
        RuffChildRole::Annotation => {
            matches!(field_name, None | Some("return_type" | "type"))
        }
        RuffChildRole::Iterable => matches!(field_name, None | Some("right" | "iterable")),
        RuffChildRole::Pattern => matches!(field_name, None | Some("pattern")),
        RuffChildRole::Handler => matches!(field_name, None | Some("body")),
        RuffChildRole::Clause => matches!(field_name, None | Some("alternative")),
        RuffChildRole::Item | RuffChildRole::Segment | RuffChildRole::Child => true,
    }
}

const fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    if a_start == a_end {
        b_start <= a_start && a_start <= b_end
    } else if b_start == b_end {
        a_start <= b_start && b_start <= a_end
    } else {
        a_start < b_end && b_start < a_end
    }
}

fn sum_lengths(lengths: &[usize]) -> u64 {
    lengths.iter().fold(0_u64, |total, length| {
        total.saturating_add(u64::try_from(*length).unwrap_or(u64::MAX))
    })
}

#[allow(clippy::too_many_arguments)] // Every disjoint public record family contributes to the one output-byte budget.
fn estimate_output_bytes(
    tokens: &[RuffTokenFact],
    ast: &[RuffAstFact],
    comments: &[RuffCommentFact],
    directives: &[RuffDirectiveFact],
    strings: &[RuffStringRegion],
    docstrings: &[RuffDocstringFact],
    continuation_line_starts: &[u64],
    diagnostics: &[RuffDiagnosticFact],
    correspondences: &[RuffTreeCorrespondence],
    provider_image_fingerprint: &str,
) -> u64 {
    let fixed = tokens
        .len()
        .saturating_mul(std::mem::size_of::<RuffTokenFact>())
        .saturating_add(ast.len().saturating_mul(std::mem::size_of::<RuffAstFact>()))
        .saturating_add(
            comments
                .len()
                .saturating_mul(std::mem::size_of::<RuffCommentFact>()),
        )
        .saturating_add(
            directives
                .len()
                .saturating_mul(std::mem::size_of::<RuffDirectiveFact>()),
        )
        .saturating_add(
            strings
                .len()
                .saturating_mul(std::mem::size_of::<RuffStringRegion>()),
        )
        .saturating_add(
            docstrings
                .len()
                .saturating_mul(std::mem::size_of::<RuffDocstringFact>()),
        )
        .saturating_add(
            continuation_line_starts
                .len()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
        .saturating_add(
            correspondences
                .len()
                .saturating_mul(std::mem::size_of::<RuffTreeCorrespondence>()),
        )
        .saturating_add(std::mem::size_of::<RuffSourceFact>())
        .saturating_add(provider_image_fingerprint.len());
    let token_spelling_bytes = tokens.iter().fold(0_usize, |total, token| {
        total.saturating_add(match &token.spelling {
            Some(RuffTokenSpelling::Slice(value) | RuffTokenSpelling::Blake3(value)) => value.len(),
            None => 0,
        })
    });
    diagnostics.iter().fold(
        u64::try_from(fixed.saturating_add(token_spelling_bytes)).unwrap_or(u64::MAX),
        |total, diagnostic| {
            total
                .saturating_add(
                    u64::try_from(std::mem::size_of::<RuffDiagnosticFact>()).unwrap_or(u64::MAX),
                )
                .saturating_add(u64::try_from(diagnostic.message.len()).unwrap_or(u64::MAX))
                .saturating_add(
                    u64::try_from(diagnostic.tree_sitter_recovery_ids.len())
                        .unwrap_or(u64::MAX)
                        .saturating_mul(
                            u64::try_from(std::mem::size_of::<SyntaxOccurrenceId>())
                                .unwrap_or(u64::MAX),
                        ),
                )
        },
    )
}

fn deadline_exceeded(started: Instant, max_wall_millis: u64) -> bool {
    elapsed_exceeds_deadline(started.elapsed(), max_wall_millis)
}

const fn elapsed_exceeds_deadline(elapsed: Duration, max_wall_millis: u64) -> bool {
    elapsed.as_millis() > max_wall_millis as u128
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::tree_sitter_adapter::{NeverCancelled, TreeSitterAdapter, TreeSitterLanguage};

    type ConfigureBound = fn(&mut RuffLimits);

    fn provider_text(source: &str) -> ProviderText {
        ProviderText {
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

    fn latin1_provider_text() -> ProviderText {
        const SOURCE: &str = "\"\"\"café\"\"\"\nname = 1\n";
        ProviderText {
            text: Arc::from(SOURCE),
            original_byte_offsets: Arc::from(
                (0_u64..=u64::try_from(SOURCE.chars().count()).unwrap()).collect::<Vec<_>>(),
            ),
        }
    }

    fn tree_snapshot(revision: u64, source: &ProviderText) -> TreeSitterSnapshot {
        TreeSitterAdapter::new(TreeSitterLanguage::Python)
            .unwrap()
            .parse_full(revision, source.clone(), &NeverCancelled)
            .unwrap()
    }

    fn ast_fact(
        id: u64,
        raw_kind: &'static str,
        category: RuffAstCategory,
        start_byte: u64,
        end_byte: u64,
        line: u32,
    ) -> RuffAstFact {
        RuffAstFact {
            id: RuffOccurrenceId(id),
            raw_kind_id: 0,
            raw_kind,
            category,
            disposition: ProviderRawKindDisposition::Normalize,
            start_byte,
            end_byte,
            line,
            column: 0,
            parent: None,
            child_role: None,
            child_ordinal: 0,
            source_ordinal: u32::try_from(id).unwrap(),
            evaluation_ordinal: None,
            explicit_parenthesized: false,
        }
    }

    fn parse_with_limits(
        source: &ProviderText,
        tree: &TreeSitterSnapshot,
        configure: impl FnOnce(&mut RuffLimits),
    ) -> Result<RuffSnapshot, RuffAdapterError> {
        let mut adapter = RuffAdapter::new().unwrap();
        configure(&mut adapter.limits);
        adapter.parse(1, source.clone(), tree, &NeverRuffCancelled)
    }

    fn has_parent_role(
        snapshot: &RuffSnapshot,
        parent_raw_kind: &str,
        child_raw_kind: &str,
        role: RuffChildRole,
    ) -> bool {
        snapshot.ast.iter().any(|child| {
            child.raw_kind == child_raw_kind
                && child.child_role == Some(role)
                && child.parent.is_some_and(|parent_id| {
                    snapshot
                        .ast
                        .iter()
                        .any(|parent| parent.id == parent_id && parent.raw_kind == parent_raw_kind)
                })
        })
    }

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../contracts/fixtures/ruff/adapter-cases-v1.json"
        ))
        .unwrap()
    }

    #[allow(clippy::too_many_lines)] // The KAT frames every public semantic field explicitly.
    fn snapshot_digest(snapshot: &RuffSnapshot) -> String {
        fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
            hasher.update(&u64::try_from(bytes.len()).unwrap().to_le_bytes());
            hasher.update(bytes);
        }
        fn option_u64(hasher: &mut blake3::Hasher, value: Option<u64>) {
            match value {
                Some(value) => {
                    hasher.update(&[1]);
                    hasher.update(&value.to_le_bytes());
                }
                None => {
                    hasher.update(&[0]);
                }
            }
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"codefabric:ruff-python-frontend-projection:v1\0");
        hasher.update(&snapshot.revision.to_le_bytes());
        frame(&mut hasher, snapshot.catalog_id.as_bytes());
        frame(&mut hasher, snapshot.provider_version.as_bytes());
        frame(
            &mut hasher,
            snapshot.runtime_inventory_fingerprint.as_bytes(),
        );
        frame(
            &mut hasher,
            snapshot.source.provider_image_fingerprint.as_bytes(),
        );
        hasher.update(&snapshot.source.start_byte.to_le_bytes());
        hasher.update(&snapshot.source.end_byte.to_le_bytes());
        hasher.update(&snapshot.source.line_count.to_le_bytes());
        hasher.update(&u64::try_from(snapshot.tokens.len()).unwrap().to_le_bytes());
        for fact in snapshot.tokens.iter() {
            hasher.update(&fact.ordinal.to_le_bytes());
            hasher.update(&fact.raw_kind_id.to_le_bytes());
            frame(&mut hasher, fact.raw_kind.as_bytes());
            hasher.update(&[fact.class as u8]);
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&fact.line.to_le_bytes());
            hasher.update(&fact.column.to_le_bytes());
            match &fact.spelling {
                Some(RuffTokenSpelling::Slice(value)) => {
                    hasher.update(&[1]);
                    frame(&mut hasher, value.as_bytes());
                }
                Some(RuffTokenSpelling::Blake3(value)) => {
                    hasher.update(&[2]);
                    frame(&mut hasher, value.as_bytes());
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            option_u64(&mut hasher, fact.syntax_id.map(|id| id.0));
        }
        hasher.update(&u64::try_from(snapshot.ast.len()).unwrap().to_le_bytes());
        for fact in snapshot.ast.iter() {
            hasher.update(&fact.id.0.to_le_bytes());
            hasher.update(&fact.raw_kind_id.to_le_bytes());
            frame(&mut hasher, fact.raw_kind.as_bytes());
            hasher.update(&[fact.category as u8]);
            hasher.update(&[match fact.disposition {
                ProviderRawKindDisposition::Normalize => 0,
                ProviderRawKindDisposition::Ignore => 1,
                ProviderRawKindDisposition::Unsupported => 2,
            }]);
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&fact.line.to_le_bytes());
            hasher.update(&fact.column.to_le_bytes());
            option_u64(&mut hasher, fact.parent.map(|id| id.0));
            hasher.update(&[fact.child_role.map_or(u8::MAX, |role| role as u8)]);
            hasher.update(&fact.child_ordinal.to_le_bytes());
            hasher.update(&fact.source_ordinal.to_le_bytes());
            option_u64(&mut hasher, fact.evaluation_ordinal.map(u64::from));
            hasher.update(&[u8::from(fact.explicit_parenthesized)]);
        }
        hasher.update(
            &u64::try_from(snapshot.comments.len())
                .unwrap()
                .to_le_bytes(),
        );
        for fact in snapshot.comments.iter() {
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&[fact.placement as u8, u8::from(fact.block_member)]);
        }
        hasher.update(
            &u64::try_from(snapshot.directives.len())
                .unwrap()
                .to_le_bytes(),
        );
        for fact in snapshot.directives.iter() {
            hasher.update(&[fact.kind as u8]);
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            option_u64(&mut hasher, fact.target.map(|id| id.0));
        }
        hasher.update(&u64::try_from(snapshot.strings.len()).unwrap().to_le_bytes());
        for fact in snapshot.strings.iter() {
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&[u8::from(fact.multiline), u8::from(fact.interpolated)]);
            option_u64(&mut hasher, fact.syntax_id.map(|id| id.0));
        }
        hasher.update(
            &u64::try_from(snapshot.docstrings.len())
                .unwrap()
                .to_le_bytes(),
        );
        for fact in snapshot.docstrings.iter() {
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(&fact.owner.0.to_le_bytes());
        }
        hasher.update(
            &u64::try_from(snapshot.continuation_line_starts.len())
                .unwrap()
                .to_le_bytes(),
        );
        for offset in snapshot.continuation_line_starts.iter() {
            hasher.update(&offset.to_le_bytes());
        }
        hasher.update(
            &u64::try_from(snapshot.diagnostics.len())
                .unwrap()
                .to_le_bytes(),
        );
        for fact in snapshot.diagnostics.iter() {
            hasher.update(&[fact.kind as u8]);
            frame(&mut hasher, fact.message.as_bytes());
            hasher.update(&fact.start_byte.to_le_bytes());
            hasher.update(&fact.end_byte.to_le_bytes());
            hasher.update(
                &u64::try_from(fact.tree_sitter_recovery_ids.len())
                    .unwrap()
                    .to_le_bytes(),
            );
            for id in fact.tree_sitter_recovery_ids.iter() {
                hasher.update(&id.0.to_le_bytes());
            }
        }
        hasher.update(
            &u64::try_from(snapshot.correspondences.len())
                .unwrap()
                .to_le_bytes(),
        );
        for edge in snapshot.correspondences.iter() {
            hasher.update(&edge.ruff_id.0.to_le_bytes());
            hasher.update(&edge.tree_sitter_id.0.to_le_bytes());
        }
        format!("b3:{}", hasher.finalize())
    }

    const RICH_SOURCE: &str = concat!(
        "\"\"\"module docs\"\"\"\n",
        "# own-line block\n",
        "# second block line\n",
        "@decorate(flag())\n",
        "def render(x: int = default()) -> str:\n",
        "    \"\"\"function docs\"\"\"\n",
        "    total = (x + 1)\n",
        "    text = f\"value {total}\"  # noqa: F401\n",
        "    # fmt: off\n",
        "    legacy = total  # type: int\n",
        "    # fmt: on\n",
        "    if total:\n",
        "        return text\n",
        "    return \"\"  # type: ignore[return-value]\n",
        "result[index()] = render(1) \\\n",
        "    + \"!\"\n",
    );

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture oracle covers every GEN section 15 projection family together.
    fn wp31_behavioral_acceptance() {
        let mut projection_digests = Vec::new();
        for case in fixture()["cases"].as_array().unwrap() {
            let revision = case["revision"].as_u64().unwrap();
            let text = provider_text(case["source"].as_str().unwrap());
            let tree = tree_snapshot(revision, &text);
            let snapshot = RuffAdapter::new()
                .unwrap()
                .parse(revision, text, &tree, &NeverRuffCancelled)
                .unwrap();
            projection_digests.push((
                case["case_id"].as_str().unwrap().to_owned(),
                snapshot_digest(&snapshot),
                case["expected_projection_digest"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ));
            assert_eq!(
                u64::try_from(snapshot.tokens.len()).unwrap(),
                case["expected_token_count"].as_u64().unwrap()
            );
            assert_eq!(
                u64::try_from(snapshot.ast.len()).unwrap(),
                case["expected_ast_count"].as_u64().unwrap()
            );
            assert_eq!(
                !snapshot.diagnostics.is_empty(),
                case["expected_recovery"].as_bool().unwrap()
            );
            if case["case_id"] == "python-pattern-and-type-parameters" {
                assert!(
                    snapshot
                        .ast
                        .iter()
                        .any(|fact| fact.category == RuffAstCategory::Pattern)
                );
                assert!(
                    snapshot
                        .ast
                        .iter()
                        .any(|fact| fact.category == RuffAstCategory::TypeSyntax)
                );
            }
        }
        assert!(
            projection_digests
                .iter()
                .all(|(_, actual, expected)| actual == expected),
            "Ruff independent projection KATs drifted: {projection_digests:#?}"
        );

        let text = provider_text(RICH_SOURCE);
        let tree = tree_snapshot(1, &text);
        let mut adapter = RuffAdapter::new().unwrap();
        let snapshot = adapter.parse(1, text, &tree, &NeverRuffCancelled).unwrap();

        assert_eq!(snapshot.catalog_id, "ruff-python-0-0-7");
        assert!(snapshot.provider_version.ends_with("python-target=3.14"));
        assert_eq!(snapshot.source.start_byte, 0);
        assert_eq!(
            snapshot.source.end_byte,
            u64::try_from(RICH_SOURCE.len()).unwrap()
        );
        assert!(!snapshot.tokens.is_empty());
        assert!(!snapshot.ast.is_empty());
        assert!(snapshot.comments.len() >= 4);
        assert_eq!(snapshot.directives.len(), 5);
        for kind in [
            RuffDirectiveKind::Noqa,
            RuffDirectiveKind::TypeIgnore,
            RuffDirectiveKind::TypeComment,
            RuffDirectiveKind::Formatter,
        ] {
            assert!(snapshot.directives.iter().any(|fact| fact.kind == kind));
        }
        assert_eq!(snapshot.docstrings.len(), 2);
        assert!(snapshot.strings.iter().any(|region| region.interpolated));
        assert_eq!(snapshot.continuation_line_starts.len(), 1);
        assert!(snapshot.ast.iter().any(|fact| fact.explicit_parenthesized));
        assert!(
            snapshot
                .tokens
                .iter()
                .enumerate()
                .all(|(ordinal, fact)| fact.ordinal == u32::try_from(ordinal).unwrap())
        );
        assert!(snapshot.tokens.iter().any(|fact| {
            fact.class == RuffTokenClass::Keyword
                && matches!(&fact.spelling, Some(RuffTokenSpelling::Slice(value)) if value == "def")
        }));
        assert!(snapshot.tokens.iter().any(|fact| {
            fact.class == RuffTokenClass::Literal
                && matches!(&fact.spelling, Some(RuffTokenSpelling::Blake3(value)) if value.starts_with("b3:"))
        }));
        assert!(
            snapshot.tokens.iter().any(|fact| {
                fact.class == RuffTokenClass::Identifier && fact.syntax_id.is_some()
            })
        );
        assert!(snapshot.ast.iter().any(|fact| {
            fact.evaluation_ordinal
                .is_some_and(|ordinal| ordinal != fact.source_ordinal)
        }));
        for needle in ["decorate(flag())", "default()"] {
            let start = u64::try_from(RICH_SOURCE.find(needle).unwrap()).unwrap();
            assert!(snapshot.ast.iter().any(|fact| {
                fact.category == RuffAstCategory::CallExpression
                    && fact.start_byte == start
                    && fact.evaluation_ordinal.is_some()
            }));
        }
        for role in [
            RuffChildRole::Body,
            RuffChildRole::Parameter,
            RuffChildRole::Annotation,
            RuffChildRole::Condition,
            RuffChildRole::Target,
            RuffChildRole::Value,
            RuffChildRole::Callee,
        ] {
            assert!(
                snapshot
                    .ast
                    .iter()
                    .any(|fact| fact.child_role == Some(role))
            );
        }
        assert!(
            snapshot
                .ast
                .iter()
                .any(|fact| fact.category == RuffAstCategory::CallExpression)
        );
        assert!(
            !snapshot
                .ast
                .iter()
                .any(|fact| fact.category == RuffAstCategory::ImportSyntax)
        );
        assert!(!snapshot.correspondences.is_empty());
        assert!(snapshot.directives.iter().all(|fact| fact.target.is_some()));
        assert!(snapshot.strings.iter().all(|fact| fact.syntax_id.is_some()));
        assert!(snapshot.docstrings.iter().all(|fact| {
            snapshot.ast.iter().any(|owner| {
                owner.id == fact.owner
                    && matches!(
                        owner.category,
                        RuffAstCategory::Block | RuffAstCategory::DeclarationSyntax
                    )
            })
        }));
        assert!(snapshot.diagnostics.is_empty());

        let summary = adapter.active_index_summary().unwrap();
        assert_eq!(summary.token_count, snapshot.metrics.token_count);
        assert_eq!(summary.comment_count, summary.indexed_comment_count);
        assert_eq!(adapter.metrics().retained_revisions, 1);
    }

    #[test]
    fn wp31_structural_acceptance() {
        let text = provider_text(RICH_SOURCE);
        let tree = tree_snapshot(1, &text);
        let snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, text, &tree, &NeverRuffCancelled)
            .unwrap();

        assert!(snapshot.tokens.iter().all(|fact| {
            RUFF_PYTHON_FRONTEND
                .token_kinds
                .get(usize::from(fact.raw_kind_id))
                .is_some_and(|entry| entry.raw_name == fact.raw_kind)
        }));
        assert!(snapshot.ast.iter().all(|fact| {
            RUFF_PYTHON_FRONTEND
                .node_kinds
                .get(usize::from(fact.raw_kind_id))
                .is_some_and(|entry| {
                    entry.raw_name == fact.raw_kind && entry.disposition == fact.disposition
                })
        }));
        assert!(RUFF_PYTHON_FRONTEND.node_kinds.iter().all(|entry| {
            entry.disposition == ProviderRawKindDisposition::Normalize
                && RuffAstCategory::from_registry_code(entry.normalized_kind_code).is_some()
        }));
        assert_eq!(snapshot.ast[0].category, RuffAstCategory::Block);
        assert!(
            snapshot
                .ast
                .iter()
                .skip(1)
                .all(|fact| fact.parent.is_some())
        );
        assert!(snapshot.ast.windows(2).all(|pair| {
            pair[0].source_ordinal < pair[1].source_ordinal
                && pair[0].start_byte <= pair[0].end_byte
        }));
        let mut child_ordinals = BTreeMap::<RuffOccurrenceId, Vec<u32>>::new();
        for (parent, child_ordinal) in snapshot
            .ast
            .iter()
            .filter_map(|fact| fact.parent.map(|parent| (parent, fact.child_ordinal)))
        {
            child_ordinals
                .entry(parent)
                .or_default()
                .push(child_ordinal);
        }
        assert!(child_ordinals.values().all(|ordinals| {
            ordinals
                .iter()
                .copied()
                .eq(0..u32::try_from(ordinals.len()).unwrap())
        }));
        assert!(snapshot.correspondences.iter().all(|edge| {
            snapshot
                .ast
                .iter()
                .find(|fact| fact.id == edge.ruff_id)
                .is_some_and(|ruff| {
                    tree.facts
                        .iter()
                        .find(|fact| fact.id == edge.tree_sitter_id)
                        .is_some_and(|fact| {
                            fact.named
                                && tree_field_compatible(
                                    ruff.child_role,
                                    fact.field_name.as_deref(),
                                )
                        })
                })
        }));

        let latin1 = latin1_provider_text();
        let latin1_tree = tree_snapshot(1, &latin1);
        let latin1_snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, latin1, &latin1_tree, &NeverRuffCancelled)
            .unwrap();
        assert!(
            latin1_snapshot
                .tokens
                .iter()
                .all(|token| token.end_byte <= 20)
        );
        assert_eq!(latin1_snapshot.docstrings.len(), 1);
        assert_eq!(
            latin1_snapshot.docstrings[0].owner,
            latin1_snapshot.ast[0].id
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One matrix protects every application-owned GEN 16.2 field role.
    fn wp31_semantic_edge_acceptance() {
        const ROLE_SOURCE: &str = concat!(
            "async def async_roles(task):\n",
            "    result = await task()\n",
            "    return result\n",
            "def roles(items, ready, condition):\n",
            "    annotated: int = 1\n",
            "    annotated += 2\n",
            "    for item in items:\n",
            "        del item\n",
            "    while ready:\n",
            "        assert check()\n",
            "        ready = False\n",
            "    choice = left if condition else right\n",
            "    if condition:\n",
            "        pass\n",
            "    else:\n",
            "        pass\n",
            "    with manager() as handle:\n",
            "        call(name=annotated)\n",
            "    try:\n",
            "        raise Error()\n",
            "    except Error:\n",
            "        pass\n",
            "    yield annotated\n",
            "    yield from items\n",
        );
        const DOC_SOURCE: &str = concat!(
            "\"\"\"module docs\"\"\"\n",
            "class Container:\n",
            "    \"\"\"class docs\"\"\"\n",
            "    def method(self):\n",
            "        \"\"\"method docs\"\"\"\n",
            "        return 1\n",
        );
        let text = provider_text(ROLE_SOURCE);
        let tree = tree_snapshot(1, &text);
        let snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, text, &tree, &NeverRuffCancelled)
            .unwrap();
        for (parent, child, role) in [
            ("StmtAugAssign", "ExprName", RuffChildRole::Target),
            ("StmtAnnAssign", "ExprName", RuffChildRole::Target),
            ("StmtAnnAssign", "ExprName", RuffChildRole::Annotation),
            ("StmtAnnAssign", "ExprNumberLiteral", RuffChildRole::Value),
            ("StmtAugAssign", "ExprNumberLiteral", RuffChildRole::Value),
            ("StmtFor", "ExprName", RuffChildRole::Target),
            ("StmtFor", "ExprName", RuffChildRole::Iterable),
            ("StmtDelete", "ExprName", RuffChildRole::Target),
            ("StmtWhile", "ExprName", RuffChildRole::Condition),
            ("StmtAssert", "ExprCall", RuffChildRole::Condition),
            ("ExprIf", "ExprName", RuffChildRole::Condition),
            ("ExprAwait", "ExprCall", RuffChildRole::Value),
            ("StmtRaise", "ExprCall", RuffChildRole::Value),
            ("ExprYield", "ExprName", RuffChildRole::Value),
            ("ExprYieldFrom", "ExprName", RuffChildRole::Value),
            ("Arguments", "Keyword", RuffChildRole::KeywordArgument),
            (
                "StmtTry",
                "ExceptHandlerExceptHandler",
                RuffChildRole::Handler,
            ),
            ("StmtIf", "ElifElseClause", RuffChildRole::Clause),
            ("StmtWith", "WithItem", RuffChildRole::Item),
        ] {
            assert!(
                has_parent_role(&snapshot, parent, child, role),
                "missing {parent} -> {child} role {role:?}"
            );
        }

        assert_eq!(token_class(TokenKind::EndOfFile), RuffTokenClass::EndOfFile);
        let mut token = RuffTokenFact {
            ordinal: 0,
            raw_kind_id: 0,
            raw_kind: "Name",
            class: RuffTokenClass::Identifier,
            start_byte: 5,
            end_byte: 6,
            line: 1,
            column: 0,
            spelling: Some(RuffTokenSpelling::Slice("x".into())),
            syntax_id: None,
        };
        let candidates = [
            ast_fact(1, "ExprName", RuffAstCategory::Expression, 5, 10, 1),
            ast_fact(2, "ExprName", RuffAstCategory::Expression, 4, 7, 1),
        ];
        link_tokens_to_ast(std::slice::from_mut(&mut token), &candidates);
        assert_eq!(token.syntax_id, Some(RuffOccurrenceId(2)));

        let eol_candidates = [
            ast_fact(1, "ExprName", RuffAstCategory::Expression, 1, 19, 2),
            ast_fact(2, "ExprName", RuffAstCategory::Expression, 20, 21, 3),
            ast_fact(3, "ExprName", RuffAstCategory::Expression, 1, 19, 3),
            ast_fact(4, "ExprName", RuffAstCategory::Expression, 10, 19, 3),
        ];
        assert_eq!(
            directive_target(&eol_candidates, CommentLinePosition::EndOfLine, 3, 20, 30),
            Some(RuffOccurrenceId(4))
        );
        let own_line_candidates = [
            ast_fact(5, "ExprName", RuffAstCategory::Expression, 21, 40, 4),
            ast_fact(6, "ExprName", RuffAstCategory::Expression, 21, 23, 4),
        ];
        assert_eq!(
            directive_target(
                &own_line_candidates,
                CommentLinePosition::OwnLine,
                3,
                10,
                20
            ),
            Some(RuffOccurrenceId(6))
        );
        assert_eq!(string_syntax_id(&own_line_candidates, 21, 22), None);
        let string_candidates = [
            ast_fact(7, "ExprStringLiteral", RuffAstCategory::Literal, 10, 30, 1),
            ast_fact(8, "StringLiteral", RuffAstCategory::Literal, 10, 20, 1),
        ];
        assert_eq!(
            string_syntax_id(&string_candidates, 12, 18),
            Some(RuffOccurrenceId(8))
        );

        for (left, right, expected) in [
            ((1, 3), (2, 4), true),
            ((1, 2), (2, 3), false),
            ((2, 3), (1, 2), false),
            ((1, 3), (1, 3), true),
            ((2, 2), (1, 3), true),
            ((1, 3), (3, 3), true),
            ((4, 4), (1, 3), false),
            ((1, 3), (0, 0), false),
            ((2, 2), (2, 2), true),
        ] {
            assert_eq!(
                ranges_overlap(left.0, left.1, right.0, right.1),
                expected,
                "overlap drift for {left:?} and {right:?}"
            );
        }

        let doc_text = provider_text(DOC_SOURCE);
        let doc_tree = tree_snapshot(1, &doc_text);
        let doc_snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, doc_text.clone(), &doc_tree, &NeverRuffCancelled)
            .unwrap();
        let parsed = parse_unchecked(
            &doc_text.text,
            ParseOptions::from(PySourceType::Python).with_target_version(PythonVersion::PY314),
        )
        .try_into_module()
        .unwrap();
        let boundary_map = ProviderBoundaryMap::new(&doc_text).unwrap();
        let mut ast_with_decoys = doc_snapshot.ast.to_vec();
        let class = ast_with_decoys
            .iter()
            .find(|fact| fact.raw_kind == "StmtClassDef")
            .unwrap()
            .clone();
        let mut same_start = class.clone();
        same_start.id = RuffOccurrenceId(900);
        same_start.end_byte = same_start.end_byte.saturating_sub(1);
        same_start.raw_kind = "Identifier";
        let mut same_raw = class;
        same_raw.id = RuffOccurrenceId(901);
        same_raw.start_byte = same_raw.start_byte.saturating_add(1);
        same_raw.end_byte = same_raw.end_byte.saturating_sub(1);
        ast_with_decoys.insert(0, same_raw);
        ast_with_decoys.insert(0, same_start);
        let docstrings = project_docstrings(&parsed, &boundary_map, &ast_with_decoys).unwrap();
        assert_eq!(docstrings.len(), 3);
        assert!(docstrings.iter().all(|docstring| docstring.owner.0 < 900));
        for owner_kind in ["ModModule", "StmtClassDef", "StmtFunctionDef"] {
            assert!(docstrings.iter().any(|docstring| {
                doc_snapshot
                    .ast
                    .iter()
                    .any(|fact| fact.id == docstring.owner && fact.raw_kind == owner_kind)
            }));
        }
    }

    #[test]
    fn wp31_exact_limit_and_identity_acceptance() {
        let source = provider_text("value = call(1)\n");
        let tree = tree_snapshot(1, &source);
        let baseline = RuffAdapter::new()
            .unwrap()
            .parse(1, source.clone(), &tree, &NeverRuffCancelled)
            .unwrap();
        let input_bytes = u64::try_from(source.text.len()).unwrap();
        assert!(
            parse_with_limits(&source, &tree, |limits| {
                limits.max_input_bytes = input_bytes;
            })
            .is_ok()
        );
        assert!(
            parse_with_limits(&source, &tree, |limits| {
                limits.max_output_records = baseline.metrics.output_records;
            })
            .is_ok()
        );
        assert!(
            parse_with_limits(&source, &tree, |limits| {
                limits.max_output_bytes = baseline.metrics.output_bytes;
            })
            .is_ok()
        );
        assert!(
            parse_with_limits(&source, &tree, |limits| {
                limits.max_work_units = baseline.metrics.work_units;
            })
            .is_ok()
        );
        assert!(
            parse_with_limits(&source, &tree, |limits| {
                limits.max_diagnostics = 0;
            })
            .is_ok()
        );

        for mutate in [
            |evidence: &mut TreeSitterSnapshot| evidence.revision = 2,
            |evidence: &mut TreeSitterSnapshot| evidence.catalog_id = "wrong-catalog",
            |evidence: &mut TreeSitterSnapshot| {
                evidence.provider_image_fingerprint = "b3:wrong-image".into();
            },
        ] {
            let mut evidence = tree.clone();
            mutate(&mut evidence);
            assert_eq!(
                RuffAdapter::new().unwrap().parse(
                    1,
                    source.clone(),
                    &evidence,
                    &NeverRuffCancelled
                ),
                Err(RuffAdapterError::MismatchedTreeSitterEvidence)
            );
        }

        let mut progress = RuffAdapter::new().unwrap();
        progress.limits.max_work_units = 5;
        assert_eq!(
            progress.check_progress(Instant::now(), 5, &NeverRuffCancelled),
            Ok(())
        );
        assert_eq!(
            progress.check_progress(Instant::now(), 6, &NeverRuffCancelled),
            Err(RuffAdapterError::WorkLimit)
        );
        let cancellation = CancelAtInterval(2);
        let mut interval = RuffAdapter::new().unwrap();
        interval.limits.cancellation_check_interval = 2;
        assert_eq!(
            interval.check_progress(Instant::now(), 1, &cancellation),
            Ok(())
        );
        assert_eq!(
            interval.check_progress(Instant::now(), 2, &cancellation),
            Err(RuffAdapterError::Cancelled)
        );

        let mut rejected = RuffAdapter::new().unwrap();
        assert_eq!(
            rejected.reject::<()>(RuffAdapterError::InputLimit),
            Err(RuffAdapterError::InputLimit)
        );
        assert_eq!(rejected.metrics().rejected_runs, 1);
        assert_eq!(rejected.metrics().cancelled_runs, 0);
        assert_eq!(
            rejected.reject::<()>(RuffAdapterError::Cancelled),
            Err(RuffAdapterError::Cancelled)
        );
        assert_eq!(rejected.metrics().rejected_runs, 2);
        assert_eq!(rejected.metrics().cancelled_runs, 1);

        assert!(!elapsed_exceeds_deadline(Duration::from_millis(5), 5));
        assert!(elapsed_exceeds_deadline(Duration::from_millis(6), 5));
        assert!(!deadline_exceeded(Instant::now(), 1_000));
        assert!(deadline_exceeded(
            Instant::now()
                .checked_sub(Duration::from_millis(2))
                .unwrap(),
            1
        ));
    }

    struct CancelAfter {
        checks: AtomicUsize,
        after: usize,
    }

    impl RuffCancellation for CancelAfter {
        fn is_cancelled(&self) -> bool {
            self.checks.fetch_add(1, Ordering::Relaxed) >= self.after
        }

        fn check_interval(&self) -> u32 {
            1
        }
    }

    struct CancelAtInterval(u32);

    impl RuffCancellation for CancelAtInterval {
        fn is_cancelled(&self) -> bool {
            true
        }

        fn check_interval(&self) -> u32 {
            self.0
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One negative oracle isolates every independent publication and resource boundary.
    fn wp31_negative_zero_state() {
        for mutate in [
            |inventory: &mut RuffPythonInventory| inventory.catalog_id = "wrong-catalog",
            |inventory: &mut RuffPythonInventory| inventory.provider_version = "wrong-version",
            |inventory: &mut RuffPythonInventory| {
                inventory.runtime_inventory_fingerprint = "b3:drift";
            },
            |inventory: &mut RuffPythonInventory| inventory.node_kinds = &[],
            |inventory: &mut RuffPythonInventory| inventory.token_kinds = &[],
        ] {
            let mut drifted = RUFF_PYTHON_FRONTEND;
            mutate(&mut drifted);
            assert!(matches!(
                validate_runtime_inventory(&drifted),
                Err(RuffAdapterError::ProviderVersionMismatch(_))
            ));
        }

        let malformed = provider_text("def broken(:\n    pass\n");
        let malformed_tree = tree_snapshot(1, &malformed);
        let malformed_snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, malformed, &malformed_tree, &NeverRuffCancelled)
            .unwrap();
        assert!(!malformed_snapshot.diagnostics.is_empty());
        assert!(
            malformed_tree
                .facts
                .iter()
                .any(|fact| fact.error || fact.missing)
        );
        for diagnostic in malformed_snapshot.diagnostics.iter() {
            assert!(diagnostic.tree_sitter_recovery_ids.iter().all(|id| {
                malformed_tree
                    .facts
                    .iter()
                    .any(|fact| fact.id == *id && (fact.error || fact.missing))
            }));
        }

        let good = provider_text("value = 1\n");
        let good_tree = tree_snapshot(1, &good);
        let mut adapter = RuffAdapter::new().unwrap();
        let accepted = adapter
            .parse(1, good, &good_tree, &NeverRuffCancelled)
            .unwrap();
        let replacement = provider_text("value = 2\n");
        let replacement_tree = tree_snapshot(2, &replacement);
        let mut wrong_image_tree = good_tree.clone();
        wrong_image_tree.revision = 2;
        assert_eq!(
            adapter.parse(
                2,
                replacement.clone(),
                &wrong_image_tree,
                &NeverRuffCancelled
            ),
            Err(RuffAdapterError::MismatchedTreeSitterEvidence)
        );
        assert_eq!(adapter.active_snapshot(), Some(&accepted));
        assert_eq!(
            adapter.parse(
                1,
                replacement.clone(),
                &replacement_tree,
                &NeverRuffCancelled
            ),
            Err(RuffAdapterError::StaleRevision)
        );
        assert_eq!(adapter.active_snapshot(), Some(&accepted));

        let cancelled = CancelAfter {
            checks: AtomicUsize::new(0),
            after: 1,
        };
        assert_eq!(
            adapter.parse(2, replacement.clone(), &replacement_tree, &cancelled),
            Err(RuffAdapterError::Cancelled)
        );
        assert_eq!(adapter.active_snapshot(), Some(&accepted));

        let mut bounded = RuffAdapter::new().unwrap();
        bounded.limits.max_output_records = 1;
        assert_eq!(
            bounded.parse(2, replacement, &replacement_tree, &NeverRuffCancelled),
            Err(RuffAdapterError::OutputRecordLimit)
        );
        assert!(bounded.active_snapshot().is_none());

        let bound_cases: [(ConfigureBound, RuffAdapterError); 5] = [
            (
                |limits| limits.max_input_bytes = 1,
                RuffAdapterError::InputLimit,
            ),
            (
                |limits| limits.max_visited_nodes = 1,
                RuffAdapterError::NodeLimit,
            ),
            (
                |limits| limits.max_traversal_depth = 1,
                RuffAdapterError::DepthLimit,
            ),
            (
                |limits| limits.max_work_units = 1,
                RuffAdapterError::WorkLimit,
            ),
            (
                |limits| limits.max_output_bytes = 1,
                RuffAdapterError::OutputByteLimit,
            ),
        ];
        for (configure, expected) in bound_cases {
            let source = provider_text("value = call(1)\n");
            let tree = tree_snapshot(1, &source);
            let mut bounded = RuffAdapter::new().unwrap();
            configure(&mut bounded.limits);
            assert_eq!(
                bounded.parse(1, source, &tree, &NeverRuffCancelled),
                Err(expected)
            );
            assert!(bounded.active_snapshot().is_none());
        }

        let source = provider_text("def broken(:\n    pass\n");
        let tree = tree_snapshot(1, &source);
        let mut diagnostic_bounded = RuffAdapter::new().unwrap();
        diagnostic_bounded.limits.max_diagnostics = 0;
        assert_eq!(
            diagnostic_bounded.parse(1, source, &tree, &NeverRuffCancelled),
            Err(RuffAdapterError::DiagnosticLimit)
        );
    }

    #[test]
    fn wp31_operational_acceptance() {
        let text = provider_text(RICH_SOURCE);
        let tree = tree_snapshot(1, &text);
        let mut adapter = RuffAdapter::new().unwrap();
        let snapshot = adapter.parse(1, text, &tree, &NeverRuffCancelled).unwrap();
        assert_eq!(adapter.metrics().completed_runs, 1);
        assert_eq!(adapter.metrics().rejected_runs, 0);
        assert_eq!(snapshot.metrics.visited_nodes, snapshot.ast.len() as u64);
        assert_eq!(snapshot.metrics.token_count, snapshot.tokens.len() as u64);
        assert!(snapshot.metrics.output_records >= snapshot.metrics.visited_nodes);
        assert!(snapshot.metrics.output_bytes > 0);
        assert!(snapshot.metrics.work_units >= snapshot.metrics.visited_nodes);

        let mut repetitive_source = String::with_capacity(64 * 2_048);
        for index in 0..2_048 {
            writeln!(&mut repetitive_source, "value_{index} = call({index})").unwrap();
        }
        let repetitive = provider_text(&repetitive_source);
        let tree = tree_snapshot(1, &repetitive);
        let repetitive_snapshot = RuffAdapter::new()
            .unwrap()
            .parse(1, repetitive, &tree, &NeverRuffCancelled)
            .unwrap();
        assert!(repetitive_snapshot.tokens.len() > 10_000);
        assert!(repetitive_snapshot.ast.len() > 8_000);
    }
}
