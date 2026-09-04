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

use crate::provider_contracts::{
    CancellationProbe, ProviderJob, ProviderLane, ProviderTrustPosture,
};
use crate::provider_raw_kinds::{
    ProviderRawKindDisposition, RUFF_PYTHON_FRONTEND, RuffNodeKindEntry, RuffPythonInventory,
    RuffTokenKindEntry,
};
use crate::provider_types::{ProviderBoundaryError, ProviderBoundaryMap, ProviderText};
use crate::tree_sitter_adapter::{RawSyntaxFact, SyntaxOccurrenceId, TreeSitterSnapshot};

mod callables;
mod cfg;
mod dataflow;
mod imports;
mod semantic;

pub use callables::{
    PythonArgumentBindingStatus, PythonArgumentSpreadKind, PythonCallArgumentFact,
    PythonCallDiagnosticFact, PythonCallSiteFact, PythonCallableFact, PythonCallableSyntaxFact,
    PythonCallableSyntaxRole, PythonDispatchKind, PythonMemberFact, PythonMemberKind,
    PythonParameterFact, PythonParameterKind, PythonUnknownArgumentSetFact,
};
pub use cfg::{
    PythonCfgEdgeFact, PythonCfgEdgeKind, PythonCfgFact, PythonCfgKind, PythonCfgNodeFact,
    PythonCfgNodeKind, PythonCfgValidationError, validate_python_cfg,
};
pub use dataflow::{
    PYTHON_DATAFLOW_BUNDLE_ID, PYTHON_DATAFLOW_DERIVATION_ID, PYTHON_DATAFLOW_PRECISION_PROFILE,
    PythonAccessPathComponentFact, PythonAccessProjectionKind, PythonDataflowEventFact,
    PythonDataflowEventKind, PythonDataflowRelationFact, PythonDataflowRelationKind,
    PythonLocationKind, PythonMemoryLocationFact, PythonOperationFact, PythonOperationKind,
    PythonValueFact, PythonValueKind,
};
pub use semantic::{
    PythonBindingFact, PythonBindingKind, PythonExportFact, PythonExportStatus,
    PythonFrontendBatch, PythonImportFact, PythonImportKind, PythonReferenceClass,
    PythonReferenceFact, PythonResolution, PythonScopeFact, PythonScopeKind, PythonSemanticEdge,
    PythonSemanticEdgeKind, PythonSemanticError, PythonSemanticMetrics, PythonSemanticTerminal,
    PythonTargetForm, PythonUnknownSymbolFact,
};

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
    pub(crate) const fn registry_code(self) -> u16 {
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

impl RuffChildRole {
    /// Exact provider-normalization registry key for this application-owned role.
    #[must_use]
    pub const fn registry_name(self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Decorator => "Decorator",
            Self::Name => "Name",
            Self::TypeParameter => "TypeParameter",
            Self::Parameter => "Parameter",
            Self::Argument => "Argument",
            Self::KeywordArgument => "KeywordArgument",
            Self::Callee => "Callee",
            Self::Condition => "Condition",
            Self::Target => "Target",
            Self::Value => "Value",
            Self::Annotation => "Annotation",
            Self::Iterable => "Iterable",
            Self::Pattern => "Pattern",
            Self::Handler => "Handler",
            Self::Clause => "Clause",
            Self::Item => "Item",
            Self::Segment => "Segment",
            Self::Child => "Child",
        }
    }
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
    pub raw_kind: String,
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
    pub raw_kind: String,
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
    fn from_job(job: &ProviderJob) -> Result<Self, RuffAdapterError> {
        validate_ruff_job(job)?;
        let ceilings = job.ceilings();
        let remaining = job.remaining().ok_or(RuffAdapterError::Deadline)?;
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
            cancellation_check_interval: u32::try_from(ceilings.cancellation_poll_work_units())
                .unwrap_or(u32::MAX),
        })
    }
}

fn validate_ruff_job(job: &ProviderJob) -> Result<(), RuffAdapterError> {
    if job.lane() != ProviderLane::Ruff
        || job.trust() != ProviderTrustPosture::InProcessConstrained
        || job.protocol().as_str() != "in-process-arrow@1"
        || job.requests().is_empty()
    {
        return Err(RuffAdapterError::ProviderVersionMismatch(
            "job is not an exact in-process Ruff invocation".into(),
        ));
    }
    Ok(())
}

fn ruff_python_node_kind_entry(kind: NodeKind) -> RuffNodeKindEntry {
    RuffNodeKindEntry {
        raw_kind_id: kind as u16,
        raw_name: format!("{kind:?}"),
        disposition: ProviderRawKindDisposition::Normalize,
        normalized_kind_code: ruff_python_normalized_kind_code(kind),
    }
}

fn ruff_python_token_kind_entry(kind: TokenKind) -> RuffTokenKindEntry {
    RuffTokenKindEntry {
        raw_kind_id: kind as u16,
        raw_name: format!("{kind:?}"),
    }
}

#[allow(clippy::too_many_lines)] // Exhaustiveness is the deliberate Ruff upgrade sentinel.
const fn ruff_python_normalized_kind_code(kind: NodeKind) -> u16 {
    use NodeKind::*;

    match kind {
        ModModule | ModExpression => 90,
        StmtFunctionDef | StmtClassDef | StmtTypeAlias => 50,
        StmtReturn => 200,
        StmtDelete | StmtWith | StmtTry | StmtAssert | StmtGlobal | StmtNonlocal | StmtExpr
        | StmtPass | StmtBreak | StmtContinue | StmtIpyEscapeCommand => 20,
        StmtAssign | StmtAugAssign | StmtAnnAssign => 170,
        StmtFor | StmtWhile | Comprehension => 190,
        StmtIf | StmtMatch | ExprIf | MatchCase => 180,
        StmtRaise => 230,
        StmtImport | StmtImportFrom | Alias => 240,
        ExprBoolOp | ExprNamed | ExprBinOp | ExprUnaryOp | ExprCompare => 110,
        ExprLambda | ExprDict | ExprSet | ExprListComp | ExprSetComp | ExprDictComp
        | ExprGenerator | ExprStarred | ExprName | ExprList | ExprTuple | ExprSlice
        | ExprIpyEscapeCommand => 30,
        ExprAwait => 220,
        ExprYield | ExprYieldFrom => 210,
        ExprCall => 160,
        ExprFString | ExprTString | ExprStringLiteral | ExprBytesLiteral | ExprNumberLiteral
        | ExprBooleanLiteral | ExprNoneLiteral | ExprEllipsisLiteral | FString | TString
        | StringLiteral | BytesLiteral => 100,
        ExprAttribute => 120,
        ExprSubscript => 140,
        ExceptHandlerExceptHandler
        | InterpolatedElement
        | InterpolatedStringLiteralElement
        | InterpolatedStringFormatSpec
        | WithItem
        | Decorator
        | ElifElseClause
        | Identifier => 10,
        PatternMatchValue
        | PatternMatchSingleton
        | PatternMatchSequence
        | PatternMatchMapping
        | PatternMatchClass
        | PatternMatchStar
        | PatternMatchAs
        | PatternMatchOr
        | PatternArguments
        | PatternKeyword => 40,
        TypeParamTypeVar | TypeParamTypeVarTuple | TypeParamParamSpec | TypeParams => 60,
        Arguments | Keyword => 80,
        Parameters | Parameter | ParameterWithDefault => 70,
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
    retained: Option<RetainedRuffRevision>,
    metrics: RuffAdapterMetrics,
}

impl RuffAdapter {
    /// Validate the application-owned exact-version identity and resource profile.
    ///
    /// # Errors
    ///
    /// Returns a version mismatch if the release identity or profile is not the exact supported
    /// Ruff frontend.
    pub(crate) fn new() -> Result<Self, RuffAdapterError> {
        validate_runtime_inventory(&RUFF_PYTHON_FRONTEND)?;
        Ok(Self {
            inventory: &RUFF_PYTHON_FRONTEND,
            retained: None,
            metrics: RuffAdapterMetrics::default(),
        })
    }

    /// Project the retained Ruff AST through the pinned semantic model.
    ///
    /// Ruff arena identifiers and borrowed nodes remain inside this adapter
    /// module. The returned batch is entirely application owned.
    ///
    /// # Errors
    ///
    /// Rejects a missing/stale retained revision or an injected cleanup fault.
    pub fn semantic_batch(
        &self,
        revision: u64,
        module_name: &str,
        module_path: &std::path::Path,
        inject_cleanup_failure: bool,
    ) -> Result<PythonFrontendBatch, PythonSemanticError> {
        let retained = self
            .retained
            .as_ref()
            .filter(|retained| retained.revision == revision)
            .ok_or(PythonSemanticError::MissingRevision(revision))?;
        let parse_diagnostic_count = retained
            .snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == RuffDiagnosticKind::Parse)
            .count();
        if parse_diagnostic_count > 0 {
            return Err(PythonSemanticError::UnavailableParse(
                parse_diagnostic_count,
            ));
        }
        semantic::project_python_semantics(
            &retained.text.text,
            retained.parsed.syntax().body.as_slice(),
            module_name,
            module_path,
            &retained.snapshot.source.provider_image_fingerprint,
            inject_cleanup_failure,
        )
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
        job: &ProviderJob,
        revision: u64,
        text: ProviderText,
        tree_sitter: &TreeSitterSnapshot,
    ) -> Result<RuffSnapshot, RuffAdapterError> {
        let limits = RuffLimits::from_job(job)?;
        let (major, minor) = job.context().python_version().ok_or_else(|| {
            RuffAdapterError::ProjectionInvariant(
                "Python language version is absent from the effective provider context".into(),
            )
        })?;
        let target_version = PythonVersion {
            major: u8::try_from(major).map_err(|_| {
                RuffAdapterError::ProjectionInvariant(
                    "Python major version is outside the provider representation".into(),
                )
            })?,
            minor: u8::try_from(minor).map_err(|_| {
                RuffAdapterError::ProjectionInvariant(
                    "Python minor version is outside the provider representation".into(),
                )
            })?,
        };
        let cancellation = job.cancellation();
        if self
            .retained
            .as_ref()
            .is_some_and(|retained| revision <= retained.revision)
        {
            return self.reject(RuffAdapterError::StaleRevision);
        }
        if u64::try_from(text.text.len()).unwrap_or(u64::MAX) > limits.max_input_bytes {
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
            ParseOptions::from(PySourceType::Python).with_target_version(target_version),
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
        self.check_progress(limits, started, 1, cancellation)?;

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
            limits,
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
            limits,
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
        self.check_progress(limits, started, work_units, cancellation)?;

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
        if output_records > limits.max_output_records {
            return self.reject(RuffAdapterError::OutputRecordLimit);
        }
        if diagnostics.len() > usize::from(limits.max_diagnostics) {
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
        if output_bytes > limits.max_output_bytes {
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

    /// Exact application-owned Ruff release identity validated at startup.
    #[must_use]
    pub const fn inventory(&self) -> &'static RuffPythonInventory {
        self.inventory
    }

    fn check_progress(
        &mut self,
        limits: RuffLimits,
        started: Instant,
        work_units: u64,
        cancellation: &CancellationProbe,
    ) -> Result<(), RuffAdapterError> {
        let effective_interval = limits
            .cancellation_check_interval
            .min(u32::try_from(cancellation.max_work_units_between_polls()).unwrap_or(u32::MAX))
            .max(1);
        if work_units.is_multiple_of(u64::from(effective_interval)) && cancellation.is_cancelled() {
            return self.reject(RuffAdapterError::Cancelled);
        }
        if deadline_exceeded(started, limits.max_wall_millis) {
            return self.reject(RuffAdapterError::Deadline);
        }
        if work_units > limits.max_work_units {
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
    {
        return Err(RuffAdapterError::ProviderVersionMismatch(
            "application Ruff release identity drifted".into(),
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
    cancellation: &CancellationProbe,
) -> Result<Vec<RuffTokenFact>, RuffAdapterError> {
    let mut output = Vec::with_capacity(parsed.tokens().len());
    let interval = limits
        .cancellation_check_interval
        .min(u32::try_from(cancellation.max_work_units_between_polls()).unwrap_or(u32::MAX))
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
            let mut hasher = crate::identity::semantic_fingerprint(
                crate::identity::SemanticFingerprintDomain::PythonLiteralTokenSpelling,
            );
            hasher.update(
                &u64::try_from(spelling.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(spelling.as_bytes());
            Some(RuffTokenSpelling::Blake3(crate::integrity::frame_digest(
                hasher.finalize(),
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
        RuffTokenClass::Identifier => {
            matches!(fact.raw_kind.as_str(), "Identifier" | "ExprName")
        }
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

type NodeKey = (u32, u32, u16);

fn node_key(node: AnyNodeRef<'_>) -> NodeKey {
    (
        node.start().to_u32(),
        node.end().to_u32(),
        node.kind() as u16,
    )
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

struct AstProjectionVisitor<'a> {
    trivia: &'a TriviaRanges,
    line_index: &'a LineIndex,
    boundary_map: &'a ProviderBoundaryMap,
    source: &'a str,
    evaluation_ordinals: &'a BTreeMap<NodeKey, u32>,
    limits: RuffLimits,
    started: Instant,
    cancellation: &'a CancellationProbe,
    stack: Vec<RuffOccurrenceId>,
    parent_nodes: Vec<AnyNodeRef<'a>>,
    child_counts: Vec<u32>,
    output: Vec<RuffAstFact>,
    work_units: u64,
    error: Option<RuffAdapterError>,
}

impl<'a> AstProjectionVisitor<'a> {
    #[allow(clippy::too_many_arguments)] // The visitor borrows the complete one-run projection context without cloning it.
    fn new(
        trivia: &'a TriviaRanges,
        line_index: &'a LineIndex,
        boundary_map: &'a ProviderBoundaryMap,
        source: &'a str,
        evaluation_ordinals: &'a BTreeMap<NodeKey, u32>,
        limits: RuffLimits,
        started: Instant,
        cancellation: &'a CancellationProbe,
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

impl<'a> SourceOrderVisitor<'a> for AstProjectionVisitor<'a> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        if self.error.is_some() {
            return TraversalSignal::Skip;
        }
        self.work_units = self.work_units.saturating_add(1);
        let interval = self
            .limits
            .cancellation_check_interval
            .min(
                u32::try_from(self.cancellation.max_work_units_between_polls()).unwrap_or(u32::MAX),
            )
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
                "Ruff raw kind {} resolves outside the application syntax set",
                entry.raw_name
            )));
        };
        if entry.disposition != ProviderRawKindDisposition::Normalize {
            return self.fail(RuffAdapterError::ProjectionInvariant(format!(
                "Ruff raw kind {} is not normalized",
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
                        && fact.raw_kind_id == owner_key.2
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
    use crate::tree_sitter_adapter::{TreeSitterAdapter, TreeSitterLanguage};

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

    fn limits() -> ProviderResourceCeilingSpec {
        ProviderResourceCeilingSpec {
            max_relations: 32,
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

    fn job_for_lane(
        lane: ProviderLane,
        spec: ProviderResourceCeilingSpec,
    ) -> (CancellationHandle, ProviderJob) {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let (owner, cancellation) =
            CancellationProbe::pair(spec.cancellation_poll_work_units).unwrap();
        let (provider, relation) = match lane {
            ProviderLane::TreeSitter => ("tree-sitter-python", "provider.tree_sitter.cst_node"),
            ProviderLane::Ruff => ("ruff-python", "provider.ruff.ast_node"),
            _ => ("wrong-provider", "provider.wrong"),
        };
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3.0").unwrap(),
            provider: ProviderIdentity::try_new(provider).unwrap(),
            protocol: ProviderProtocolIdentity::try_new("in-process-arrow@1").unwrap(),
            source: ProviderSourceBinding::try_file(
                SourceIdentity::try_new("source-1").unwrap(),
                [6; 16],
                [1; 16],
                1,
                [2; 32],
            )
            .unwrap(),
            context: ProviderContextBinding::try_new(
                ContextIdentity::try_new("context-1").unwrap(),
                [3; 16],
                [3; 32],
                [4; 32],
            )
            .unwrap()
            .with_python_version(3, 14)
            .unwrap(),
            run: ProviderRunBinding::try_new(
                ProviderRunIdentity::try_new(format!("{provider}.run-1")).unwrap(),
                if lane == ProviderLane::Ruff {
                    [6; 16]
                } else {
                    [5; 16]
                },
            )
            .unwrap(),
            lane,
            trust: ProviderTrustPosture::InProcessConstrained,
            requests: vec![
                ProviderFamilyRequest::try_new(
                    ProviderFamilyIdentity::try_new(format!("{provider}.family")).unwrap(),
                    ProviderRelationIdentity::try_new(relation).unwrap(),
                    ProviderSchemaIdentity::try_new(format!("{provider}.schema")).unwrap(),
                    schema,
                    ProviderScopeIdentity::try_new("source-1").unwrap(),
                    1,
                )
                .unwrap(),
            ],
            ceilings: ProviderResourceCeilings::try_new(spec).unwrap(),
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation,
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new(format!("{provider}.build")).unwrap(),
                ProviderPolicyIdentity::try_new("policy.v2.3").unwrap(),
                ProviderProgramIdentity::try_new("provider-program.v2.3").unwrap(),
            ),
        })
        .unwrap();
        (owner, job)
    }

    fn tree(text: &str, revision: u64) -> TreeSitterSnapshot {
        let (_, job) = job_for_lane(ProviderLane::TreeSitter, limits());
        TreeSitterAdapter::new(TreeSitterLanguage::Python)
            .unwrap()
            .parse_full(&job, revision, provider_text(text))
            .unwrap()
    }

    #[test]
    fn ruff_job_drives_owned_snapshot_and_honest_whole_file_reparse() {
        let mut adapter = RuffAdapter::new().unwrap();
        let (_, first_job) = job_for_lane(ProviderLane::Ruff, limits());
        let first_text = provider_text("value = 1\n");
        let first = adapter
            .parse(&first_job, 1, first_text, &tree("value = 1\n", 1))
            .unwrap();
        assert!(!first.tokens.is_empty());
        assert!(!first.ast.is_empty());

        let (_, second_job) = job_for_lane(ProviderLane::Ruff, limits());
        let second = adapter
            .parse(
                &second_job,
                2,
                provider_text("value = 2\nother = value\n"),
                &tree("value = 2\nother = value\n", 2),
            )
            .unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(adapter.metrics().retained_revisions, 1);
        assert!(adapter.active_index_summary().unwrap().token_count > 0);
    }

    #[test]
    fn ruff_job_limits_cancellation_and_lane_are_causal() {
        let text = "value = 1\n";
        let evidence = tree(text, 1);
        let mut adapter = RuffAdapter::new().unwrap();
        let (owner, cancelled_job) = job_for_lane(ProviderLane::Ruff, limits());
        owner.cancel();
        assert_eq!(
            adapter.parse(&cancelled_job, 1, provider_text(text), &evidence),
            Err(RuffAdapterError::Cancelled)
        );

        let mut tiny = limits();
        tiny.max_input_bytes = 1;
        let (_, tiny_job) = job_for_lane(ProviderLane::Ruff, tiny);
        assert_eq!(
            adapter.parse(&tiny_job, 1, provider_text(text), &evidence),
            Err(RuffAdapterError::InputLimit)
        );

        let (_, wrong_job) = job_for_lane(ProviderLane::TreeSitter, limits());
        assert!(matches!(
            adapter.parse(&wrong_job, 1, provider_text(text), &evidence),
            Err(RuffAdapterError::ProviderVersionMismatch(_))
        ));
    }

    #[test]
    fn ruff_native_kinds_are_projected_only_inside_the_adapter() {
        let node = ruff_python_node_kind_entry(NodeKind::ExprCall);
        let token = ruff_python_token_kind_entry(TokenKind::Name);
        assert_eq!(node.raw_name, "ExprCall");
        assert_eq!(node.normalized_kind_code, 160);
        assert_eq!(token.raw_name, "Name");
    }
}
