//! Exact Tree-sitter/Ruff observations encoded as typed provider-native Arrow relations.
//!
//! The pinned provider APIs execute in [`crate::tree_sitter_adapter`] and
//! [`crate::ruff_adapter`]. This module is the first relational publication boundary: every
//! batch repeats the exact provider run and immutable source pins, raw provider kinds remain
//! queryable, and incomplete semantic work is represented by coverage/remainder rows. No
//! Tree-sitter or Ruff borrowed value crosses this boundary.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, FixedSizeBinaryBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::provider_raw_kinds::ProviderRawKindDisposition;
use crate::provider_types::ProviderText;
use crate::ruff_adapter::{
    PythonBindingKind, PythonExportStatus, PythonFrontendBatch, PythonImportKind,
    PythonReferenceClass, PythonResolution, PythonScopeKind, PythonSemanticEdgeKind,
    PythonSemanticError, PythonTargetForm, RuffAdapter, RuffAdapterError, RuffAstCategory,
    RuffChildRole, RuffCommentPlacement, RuffDiagnosticKind, RuffDirectiveKind, RuffSnapshot,
    RuffTokenClass, RuffTokenSpelling,
};
#[cfg(feature = "daemon")]
use crate::source_image::{SourceImage, SourceLanguage};
use crate::tree_sitter_adapter::{
    TreeSitterAdapter, TreeSitterAdapterError, TreeSitterEdit, TreeSitterLanguage,
    TreeSitterSnapshot,
};

/// Exact stable-root provider release identities compiled into this adapter.
pub const TREE_SITTER_RUNTIME_RELEASE: &str = "0.26.12";
pub const TREE_SITTER_PYTHON_GRAMMAR_RELEASE: &str = "0.25.0";
pub const RUFF_COMPONENT_RELEASE: &str = "0.0.7";
pub const PROVIDER_NATIVE_SYNTAX_SCHEMA_RELEASE: &str = "1";

const TREE_SITTER_PROVIDER_ID: &str = "tree-sitter-python";
const RUFF_PROVIDER_ID: &str = "ruff-python";
const TREE_SITTER_PROVIDER_RELEASE: &str = "tree-sitter=0.26.12;tree-sitter-python=0.25.0";
const RUFF_PROVIDER_RELEASE: &str =
    "ruff-python-ast=0.0.7;ruff-python-parser=0.0.7;python-target=3.14";

/// Immutable run pins repeated by every provider-native relation row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxProviderRunPin {
    pub provider_run_id: [u8; 16],
    pub analysis_context_id: [u8; 32],
    pub semantic_environment_id: [u8; 32],
}

/// The two exact in-process provider runs that observe one immutable source image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythonSyntaxRunPins {
    pub tree_sitter: SyntaxProviderRunPin,
    pub ruff: SyntaxProviderRunPin,
}

/// Module identity used only while populating Ruff's exact local semantic model.
#[derive(Clone, Copy, Debug)]
pub struct PythonModuleInput<'a> {
    pub module_name: &'a str,
    pub module_path: &'a Path,
}

/// Narrow immutable source image consumed by the exact in-process syntax providers.
///
/// It preserves the authoritative source digest and original-byte boundary map without pulling
/// daemon storage/path state into the provider/Arrow feature slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderNativeSourceImage {
    pub file_id: [u8; 16],
    pub source_generation: u64,
    pub bytes: Arc<[u8]>,
    pub content_digest: [u8; 32],
    pub provider_text: ProviderText,
}

impl ProviderNativeSourceImage {
    /// Construct and validate an exact Python source image.
    ///
    /// # Errors
    ///
    /// Rejects digest drift, text/byte drift, or an incomplete/non-monotonic original-byte map.
    pub fn new(
        file_id: [u8; 16],
        source_generation: u64,
        bytes: Arc<[u8]>,
        content_digest: [u8; 32],
        provider_text: ProviderText,
    ) -> Result<Self, ProviderNativeSyntaxError> {
        let source = Self {
            file_id,
            source_generation,
            bytes,
            content_digest,
            provider_text,
        };
        validated_provider_text(&source)?;
        Ok(source)
    }
}

#[cfg(feature = "daemon")]
impl TryFrom<&SourceImage> for ProviderNativeSourceImage {
    type Error = ProviderNativeSyntaxError;

    fn try_from(source: &SourceImage) -> Result<Self, Self::Error> {
        if source.language != SourceLanguage::Python {
            return Err(ProviderNativeSyntaxError::InvalidSource(
                "language is not Python",
            ));
        }
        Self::new(
            source.file_id,
            source.source_generation,
            Arc::clone(&source.bytes),
            source.digest,
            source
                .provider_text
                .clone()
                .ok_or(ProviderNativeSyntaxError::InvalidSource(
                    "provider UTF-8 text is unavailable",
                ))?,
        )
    }
}

/// Closed provider-native relation identities emitted by this exact adapter release.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeSyntaxRelation {
    TreeSitterRun,
    TreeSitterCoverage,
    TreeSitterRemainder,
    TreeSitterCstNode,
    TreeSitterChangedRange,
    TreeSitterRecoveryDiagnostic,
    RuffRun,
    RuffCoverage,
    RuffRemainder,
    RuffToken,
    RuffComment,
    RuffDirective,
    RuffStringRegion,
    RuffDocstring,
    RuffContinuationLine,
    RuffAstNode,
    RuffParseDiagnostic,
    RuffDiagnosticRecoveryEvidence,
    RuffScope,
    RuffBinding,
    RuffReference,
    RuffUnknownSymbol,
    RuffSemanticEdge,
    RuffImport,
    RuffExport,
}

impl NativeSyntaxRelation {
    pub const ALL: [Self; 25] = [
        Self::TreeSitterRun,
        Self::TreeSitterCoverage,
        Self::TreeSitterRemainder,
        Self::TreeSitterCstNode,
        Self::TreeSitterChangedRange,
        Self::TreeSitterRecoveryDiagnostic,
        Self::RuffRun,
        Self::RuffCoverage,
        Self::RuffRemainder,
        Self::RuffToken,
        Self::RuffComment,
        Self::RuffDirective,
        Self::RuffStringRegion,
        Self::RuffDocstring,
        Self::RuffContinuationLine,
        Self::RuffAstNode,
        Self::RuffParseDiagnostic,
        Self::RuffDiagnosticRecoveryEvidence,
        Self::RuffScope,
        Self::RuffBinding,
        Self::RuffReference,
        Self::RuffUnknownSymbol,
        Self::RuffSemanticEdge,
        Self::RuffImport,
        Self::RuffExport,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TreeSitterRun => "provider.tree_sitter.run",
            Self::TreeSitterCoverage => "provider.tree_sitter.coverage",
            Self::TreeSitterRemainder => "provider.tree_sitter.remainder",
            Self::TreeSitterCstNode => "provider.tree_sitter.cst_node",
            Self::TreeSitterChangedRange => "provider.tree_sitter.changed_range",
            Self::TreeSitterRecoveryDiagnostic => "provider.tree_sitter.recovery_diagnostic",
            Self::RuffRun => "provider.ruff.run",
            Self::RuffCoverage => "provider.ruff.coverage",
            Self::RuffRemainder => "provider.ruff.remainder",
            Self::RuffToken => "provider.ruff.token",
            Self::RuffComment => "provider.ruff.comment",
            Self::RuffDirective => "provider.ruff.directive",
            Self::RuffStringRegion => "provider.ruff.string_region",
            Self::RuffDocstring => "provider.ruff.docstring",
            Self::RuffContinuationLine => "provider.ruff.continuation_line",
            Self::RuffAstNode => "provider.ruff.ast_node",
            Self::RuffParseDiagnostic => "provider.ruff.parse_diagnostic",
            Self::RuffDiagnosticRecoveryEvidence => "provider.ruff.diagnostic_tree_sitter_evidence",
            Self::RuffScope => "provider.ruff.scope",
            Self::RuffBinding => "provider.ruff.binding",
            Self::RuffReference => "provider.ruff.reference",
            Self::RuffUnknownSymbol => "provider.ruff.unknown_symbol",
            Self::RuffSemanticEdge => "provider.ruff.semantic_edge",
            Self::RuffImport => "provider.ruff.import",
            Self::RuffExport => "provider.ruff.export",
        }
    }

    /// Return the exact application-owned Arrow schema compiled for this relation.
    ///
    /// Batch construction consumes the same schema, so schema-only contract compilation cannot
    /// drift from a provider-emitted relation or require executing a provider on fabricated source.
    #[must_use]
    pub(crate) fn schema(self) -> SchemaRef {
        native_relation_schema(self)
    }
}

/// One complete relation set for an immutable source image and exact provider runs.
#[derive(Debug)]
pub struct ProviderNativeSyntaxRun {
    pub relations: BTreeMap<NativeSyntaxRelation, RecordBatch>,
}

impl ProviderNativeSyntaxRun {
    /// Fetch one typed relation. All 25 relation families are present, including empty ones.
    #[must_use]
    pub fn relation(&self, relation: NativeSyntaxRelation) -> &RecordBatch {
        &self.relations[&relation]
    }
}

/// Stable failures at the exact-provider/Arrow boundary.
#[derive(Debug, Error)]
pub enum ProviderNativeSyntaxError {
    #[error("source image is not an exact valid Python source image: {0}")]
    InvalidSource(&'static str),
    #[error("Tree-sitter and Ruff runs do not share one analysis context and semantic environment")]
    MixedRunContext,
    #[error("provider snapshot does not match the immutable source image: {0}")]
    SnapshotMismatch(&'static str),
    #[error("exact Tree-sitter API probe failed: {0}")]
    TreeSitterApi(String),
    #[error(transparent)]
    TreeSitter(#[from] TreeSitterAdapterError),
    #[error(transparent)]
    Ruff(#[from] RuffAdapterError),
    #[error(transparent)]
    RuffSemantic(#[from] PythonSemanticError),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

/// Stateful exact-current Python syntax lane.
///
/// Tree-sitter keeps its bounded revision cache for changed-range evidence. Ruff reparses each
/// source image in full, exactly matching the current library's capabilities.
pub struct ExactPythonSyntaxRunner {
    tree_sitter: TreeSitterAdapter,
    ruff: RuffAdapter,
}

impl ExactPythonSyntaxRunner {
    /// Construct the exact providers and execute a compile/runtime probe against their pinned APIs.
    ///
    /// # Errors
    ///
    /// Returns provider/API errors when the current pinned runtime cannot execute its documented
    /// exact parser, grammar, token, trivia, index, or typed-AST surfaces.
    pub fn new() -> Result<Self, ProviderNativeSyntaxError> {
        exact_syntax_api_probe()?;
        Ok(Self {
            tree_sitter: TreeSitterAdapter::new(TreeSitterLanguage::Python)?,
            ruff: RuffAdapter::new()?,
        })
    }

    /// Parse one source image without Tree-sitter reuse and emit all typed native relations.
    ///
    /// # Errors
    ///
    /// Rejects invalid source/pin/snapshot state, provider failures, and Arrow schema violations.
    pub fn run_full(
        &mut self,
        revision: u64,
        source: &ProviderNativeSourceImage,
        pins: PythonSyntaxRunPins,
        module: PythonModuleInput<'_>,
        cancellation: &Cancellation,
    ) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
        let text = validated_provider_text(source)?;
        validate_run_pins(pins)?;
        let tree = self
            .tree_sitter
            .parse_full(revision, text.clone(), cancellation)?;
        let ruff = self.ruff.parse(revision, text, &tree, cancellation)?;
        let semantics = semantic_result(&self.ruff, revision, module)?;
        project_relations(source, pins, &tree, &ruff, semantics.as_ref())
    }

    /// Apply one exact edit to the retained Tree-sitter tree, reparse Ruff in full, and emit the
    /// changed-range relation alongside the complete current provider relations.
    ///
    /// # Errors
    ///
    /// In addition to [`Self::run_full`] failures, rejects a stale or geometrically invalid edit.
    pub fn run_incremental(
        &mut self,
        revision: u64,
        source: &ProviderNativeSourceImage,
        edit: TreeSitterEdit,
        pins: PythonSyntaxRunPins,
        module: PythonModuleInput<'_>,
        cancellation: &Cancellation,
    ) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
        let text = validated_provider_text(source)?;
        validate_run_pins(pins)?;
        let tree =
            self.tree_sitter
                .parse_incremental(revision, text.clone(), edit, cancellation)?;
        let ruff = self.ruff.parse(revision, text, &tree, cancellation)?;
        let semantics = semantic_result(&self.ruff, revision, module)?;
        project_relations(source, pins, &tree, &ruff, semantics.as_ref())
    }
}

fn semantic_result(
    ruff: &RuffAdapter,
    revision: u64,
    module: PythonModuleInput<'_>,
) -> Result<Option<PythonFrontendBatch>, ProviderNativeSyntaxError> {
    match ruff.semantic_batch(revision, module.module_name, module.module_path, false) {
        Ok(batch) => Ok(Some(batch)),
        Err(PythonSemanticError::UnavailableParse(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn validate_run_pins(pins: PythonSyntaxRunPins) -> Result<(), ProviderNativeSyntaxError> {
    if pins.tree_sitter.analysis_context_id != pins.ruff.analysis_context_id
        || pins.tree_sitter.semantic_environment_id != pins.ruff.semantic_environment_id
    {
        return Err(ProviderNativeSyntaxError::MixedRunContext);
    }
    Ok(())
}

fn validated_provider_text(
    source: &ProviderNativeSourceImage,
) -> Result<ProviderText, ProviderNativeSyntaxError> {
    if crate::integrity::digest_bytes(&source.bytes) != source.content_digest {
        return Err(ProviderNativeSyntaxError::InvalidSource(
            "content digest differs from source bytes",
        ));
    }
    let text = source.provider_text.clone();
    if text.text.as_bytes() != source.bytes.as_ref() {
        return Err(ProviderNativeSyntaxError::InvalidSource(
            "provider text differs from immutable source bytes",
        ));
    }
    Ok(text)
}

/// Compile and execute the exact APIs selected by GEN §2 for this provider lane.
///
/// This deliberately names current symbols instead of hiding them behind a future-version facade.
///
/// # Errors
///
/// Returns an error if the exact grammar cannot be assigned or a trivial parse is cancelled.
pub fn exact_syntax_api_probe() -> Result<(), ProviderNativeSyntaxError> {
    use ruff_python_ast::{PySourceType, PythonVersion};
    use ruff_python_index::Indexer;
    use ruff_python_parser::{ParseOptions, parse_unchecked};
    use ruff_python_trivia::TriviaRanges;
    use ruff_source_file::LineIndex;
    use tree_sitter::{Language, Parser};

    let language: Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| ProviderNativeSyntaxError::TreeSitterApi(error.to_string()))?;
    let tree = parser
        .parse(b"value = 1\n", None)
        .ok_or_else(|| ProviderNativeSyntaxError::TreeSitterApi("parse cancelled".into()))?;
    let root = tree.root_node();
    let _native_root_kind = root.kind();
    let _native_root_kind_id = root.kind_id();
    let _changed_range_count = tree.changed_ranges(&tree).count();

    let parsed = parse_unchecked(
        "value = 1\n",
        ParseOptions::from(PySourceType::Python).with_target_version(PythonVersion::PY314),
    )
    .try_into_module()
    .ok_or_else(|| {
        ProviderNativeSyntaxError::TreeSitterApi(
            "Ruff module parse options produced a non-module root".into(),
        )
    })?;
    let _line_index = LineIndex::from_source_text("value = 1\n");
    let _trivia = TriviaRanges::from(parsed.tokens());
    let _index = Indexer::from_tokens(parsed.tokens(), "value = 1\n");
    let _typed_ast = parsed.syntax();
    Ok(())
}

#[derive(Clone, Copy)]
struct RelationPin<'a> {
    run: SyntaxProviderRunPin,
    provider_id: &'static str,
    provider_release: &'static str,
    source: &'a ProviderNativeSourceImage,
}

#[derive(Clone, Copy)]
struct CoverageRow {
    family: &'static str,
    requested_units: u64,
    completed_units: u64,
    terminal: &'static str,
    remainder_reason: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct RemainderRow {
    family: &'static str,
    reason: &'static str,
    detail: &'static str,
}

#[allow(clippy::too_many_lines)] // One closed projection makes the exact 25-relation surface auditable.
fn project_relations(
    source: &ProviderNativeSourceImage,
    pins: PythonSyntaxRunPins,
    tree: &TreeSitterSnapshot,
    ruff: &RuffSnapshot,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
    validate_snapshots(source, tree, ruff)?;
    let tree_pin = RelationPin {
        run: pins.tree_sitter,
        provider_id: TREE_SITTER_PROVIDER_ID,
        provider_release: TREE_SITTER_PROVIDER_RELEASE,
        source,
    };
    let ruff_pin = RelationPin {
        run: pins.ruff,
        provider_id: RUFF_PROVIDER_ID,
        provider_release: ruff.provider_version,
        source,
    };
    let mut relations = BTreeMap::new();

    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterRun,
        run_batch(
            tree_pin,
            NativeSyntaxRelation::TreeSitterRun,
            tree.revision,
            tree.catalog_id,
            tree.grammar_fingerprint,
            Some(TREE_SITTER_PYTHON_GRAMMAR_RELEASE),
        )?,
    );
    let tree_coverage = [
        complete_coverage("tree_sitter.cst_node"),
        complete_coverage("tree_sitter.changed_range"),
        complete_coverage("tree_sitter.recovery_diagnostic"),
    ];
    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterCoverage,
        coverage_batch(
            tree_pin,
            NativeSyntaxRelation::TreeSitterCoverage,
            &tree_coverage,
        )?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterRemainder,
        remainder_batch(tree_pin, NativeSyntaxRelation::TreeSitterRemainder, &[])?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterCstNode,
        tree_node_batch(tree_pin, tree)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterChangedRange,
        tree_changed_range_batch(tree_pin, tree)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::TreeSitterRecoveryDiagnostic,
        tree_recovery_batch(tree_pin, tree)?,
    );

    insert(
        &mut relations,
        NativeSyntaxRelation::RuffRun,
        run_batch(
            ruff_pin,
            NativeSyntaxRelation::RuffRun,
            ruff.revision,
            ruff.catalog_id,
            ruff.runtime_inventory_fingerprint,
            None,
        )?,
    );
    let mut ruff_coverage = vec![
        complete_coverage("ruff.token"),
        complete_coverage("ruff.comment"),
        complete_coverage("ruff.directive"),
        complete_coverage("ruff.string_region"),
        complete_coverage("ruff.docstring"),
        complete_coverage("ruff.continuation_line"),
        complete_coverage("ruff.ast_node"),
        complete_coverage("ruff.parse_diagnostic"),
    ];
    let semantic_families = [
        "ruff.scope",
        "ruff.binding",
        "ruff.reference",
        "ruff.unknown_symbol",
        "ruff.semantic_edge",
        "ruff.import",
        "ruff.export",
    ];
    let mut ruff_remainders = Vec::new();
    if semantics.is_some() {
        ruff_coverage.extend(semantic_families.map(complete_coverage));
    } else {
        for family in semantic_families {
            ruff_coverage.push(CoverageRow {
                family,
                requested_units: 1,
                completed_units: 0,
                terminal: "unknown",
                remainder_reason: Some("source-invalid"),
            });
            ruff_remainders.push(RemainderRow {
                family,
                reason: "source-invalid",
                detail: "Ruff local semantic model is unavailable for a recovered parse",
            });
        }
    }
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffCoverage,
        coverage_batch(ruff_pin, NativeSyntaxRelation::RuffCoverage, &ruff_coverage)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffRemainder,
        remainder_batch(
            ruff_pin,
            NativeSyntaxRelation::RuffRemainder,
            &ruff_remainders,
        )?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffToken,
        ruff_token_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffComment,
        ruff_comment_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffDirective,
        ruff_directive_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffStringRegion,
        ruff_string_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffDocstring,
        ruff_docstring_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffContinuationLine,
        ruff_continuation_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffAstNode,
        ruff_ast_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffParseDiagnostic,
        ruff_diagnostic_batch(ruff_pin, ruff)?,
    );
    insert(
        &mut relations,
        NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence,
        ruff_diagnostic_evidence_batch(ruff_pin, ruff)?,
    );
    insert_semantic_relations(&mut relations, ruff_pin, semantics)?;
    debug_assert_eq!(relations.len(), NativeSyntaxRelation::ALL.len());
    Ok(ProviderNativeSyntaxRun { relations })
}

fn validate_snapshots(
    source: &ProviderNativeSourceImage,
    tree: &TreeSitterSnapshot,
    ruff: &RuffSnapshot,
) -> Result<(), ProviderNativeSyntaxError> {
    let expected = source.provider_text.provider_image_fingerprint();
    if tree.provider_image_fingerprint != expected
        || ruff.source.provider_image_fingerprint != expected
    {
        return Err(ProviderNativeSyntaxError::SnapshotMismatch(
            "provider image fingerprint",
        ));
    }
    if tree.revision != ruff.revision {
        return Err(ProviderNativeSyntaxError::SnapshotMismatch("revision"));
    }
    if tree.catalog_id != "tree-sitter-python-0-25-0"
        || ruff.provider_version != RUFF_PROVIDER_RELEASE
    {
        return Err(ProviderNativeSyntaxError::SnapshotMismatch(
            "provider release/catalog",
        ));
    }
    Ok(())
}

fn insert(
    relations: &mut BTreeMap<NativeSyntaxRelation, RecordBatch>,
    relation: NativeSyntaxRelation,
    batch: RecordBatch,
) {
    let prior = relations.insert(relation, batch);
    debug_assert!(prior.is_none());
}

const fn complete_coverage(family: &'static str) -> CoverageRow {
    CoverageRow {
        family,
        requested_units: 1,
        completed_units: 1,
        terminal: "complete",
        remainder_reason: None,
    }
}

fn run_batch(
    pin: RelationPin<'_>,
    relation: NativeSyntaxRelation,
    provider_revision: u64,
    catalog_id: &str,
    inventory_fingerprint: &str,
    grammar_release: Option<&str>,
) -> Result<RecordBatch, ArrowError> {
    batch(
        pin,
        relation,
        1,
        vec![
            Arc::new(UInt64Array::from(vec![provider_revision])),
            Arc::new(StringArray::from(vec![catalog_id])),
            Arc::new(StringArray::from(vec![inventory_fingerprint])),
            Arc::new(StringArray::from(vec![grammar_release])),
        ],
    )
}

fn coverage_batch(
    pin: RelationPin<'_>,
    relation: NativeSyntaxRelation,
    rows: &[CoverageRow],
) -> Result<RecordBatch, ArrowError> {
    batch(
        pin,
        relation,
        rows.len(),
        vec![
            utf8(rows, |row| Some(row.family)),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.requested_units),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.completed_units),
            )),
            utf8(rows, |row| Some(row.terminal)),
            utf8(rows, |row| row.remainder_reason),
        ],
    )
}

fn remainder_batch(
    pin: RelationPin<'_>,
    relation: NativeSyntaxRelation,
    rows: &[RemainderRow],
) -> Result<RecordBatch, ArrowError> {
    batch(
        pin,
        relation,
        rows.len(),
        vec![
            utf8(rows, |row| Some(row.family)),
            utf8(rows, |row| Some(row.reason)),
            utf8(rows, |row| Some(row.detail)),
        ],
    )
}

fn tree_node_batch(
    pin: RelationPin<'_>,
    tree: &TreeSitterSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = tree.facts.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::TreeSitterCstNode,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.id.0),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.parent.map(|id| id.0))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.raw_kind_id),
            )),
            utf8(rows, |row| Some(row.raw_kind.as_str())),
            utf8(rows, |row| row.field_name.as_deref()),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            bools(rows, |row| row.named),
            bools(rows, |row| row.extra),
            bools(rows, |row| row.error),
            bools(rows, |row| row.missing),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.depth),
            )),
            utf8(rows, |row| Some(raw_kind_disposition(row.disposition))),
        ],
    )
}

fn tree_changed_range_batch(
    pin: RelationPin<'_>,
    tree: &TreeSitterSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = tree.changed_ranges.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::TreeSitterChangedRange,
        rows.len(),
        vec![
            Arc::new(UInt32Array::from_iter_values(
                (0..rows.len()).map(|value| u32::try_from(value).unwrap_or(u32::MAX)),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn tree_recovery_batch(
    pin: RelationPin<'_>,
    tree: &TreeSitterSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = tree
        .facts
        .iter()
        .filter(|row| row.error || row.missing)
        .collect::<Vec<_>>();
    batch(
        pin,
        NativeSyntaxRelation::TreeSitterRecoveryDiagnostic,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.id.0),
            )),
            utf8(&rows, |row| {
                Some(if row.missing { "MISSING" } else { "ERROR" })
            }),
            utf8(&rows, |row| Some(row.raw_kind.as_str())),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn ruff_token_batch(pin: RelationPin<'_>, ruff: &RuffSnapshot) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.tokens.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffToken,
        rows.len(),
        vec![
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.raw_kind_id),
            )),
            utf8(rows, |row| Some(row.raw_kind.as_str())),
            utf8(rows, |row| Some(ruff_token_class(row.class))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.line),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.column),
            )),
            utf8(rows, |row| match &row.spelling {
                Some(RuffTokenSpelling::Slice(_)) => Some("source-slice"),
                Some(RuffTokenSpelling::Blake3(_)) => Some("blake3-digest"),
                None => None,
            }),
            utf8(rows, |row| match &row.spelling {
                Some(RuffTokenSpelling::Slice(value) | RuffTokenSpelling::Blake3(value)) => {
                    Some(value.as_str())
                }
                None => None,
            }),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.syntax_id.map(|id| id.0))
                    .collect::<Vec<_>>(),
            )),
        ],
    )
}

fn ruff_comment_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.comments.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffComment,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            utf8(rows, |row| Some(ruff_comment_placement(row.placement))),
            bools(rows, |row| row.block_member),
        ],
    )
}

fn ruff_directive_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.directives.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffDirective,
        rows.len(),
        vec![
            utf8(rows, |row| Some(ruff_directive_kind(row.kind))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.target.map(|id| id.0))
                    .collect::<Vec<_>>(),
            )),
        ],
    )
}

fn ruff_string_batch(pin: RelationPin<'_>, ruff: &RuffSnapshot) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.strings.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffStringRegion,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            bools(rows, |row| row.multiline),
            bools(rows, |row| row.interpolated),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.syntax_id.map(|id| id.0))
                    .collect::<Vec<_>>(),
            )),
        ],
    )
}

fn ruff_docstring_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.docstrings.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffDocstring,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.owner.0),
            )),
        ],
    )
}

fn ruff_continuation_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.continuation_line_starts.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffContinuationLine,
        rows.len(),
        vec![Arc::new(UInt64Array::from_iter_values(
            rows.iter().copied(),
        ))],
    )
}

fn ruff_ast_batch(pin: RelationPin<'_>, ruff: &RuffSnapshot) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.ast.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffAstNode,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.id.0),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.parent.map(|id| id.0))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.raw_kind_id),
            )),
            utf8(rows, |row| Some(row.raw_kind.as_str())),
            utf8(rows, |row| Some(ruff_ast_category(row.category))),
            utf8(rows, |row| row.child_role.map(ruff_child_role)),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.line),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.column),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.child_ordinal),
            )),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|row| row.source_ordinal),
            )),
            Arc::new(UInt32Array::from(
                rows.iter()
                    .map(|row| row.evaluation_ordinal)
                    .collect::<Vec<_>>(),
            )),
            bools(rows, |row| row.explicit_parenthesized),
            utf8(rows, |row| Some(raw_kind_disposition(row.disposition))),
        ],
    )
}

fn ruff_diagnostic_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff.diagnostics.as_ref();
    batch(
        pin,
        NativeSyntaxRelation::RuffParseDiagnostic,
        rows.len(),
        vec![
            Arc::new(UInt32Array::from_iter_values(
                (0..rows.len()).map(|value| u32::try_from(value).unwrap_or(u32::MAX)),
            )),
            utf8(rows, |row| Some(ruff_diagnostic_kind(row.kind))),
            utf8(rows, |row| Some(row.message.as_str())),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn ruff_diagnostic_evidence_batch(
    pin: RelationPin<'_>,
    ruff: &RuffSnapshot,
) -> Result<RecordBatch, ArrowError> {
    let rows = ruff
        .diagnostics
        .iter()
        .enumerate()
        .flat_map(|(diagnostic, row)| {
            row.tree_sitter_recovery_ids
                .iter()
                .map(move |tree_id| (diagnostic, tree_id.0))
        })
        .collect::<Vec<_>>();
    batch(
        pin,
        NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence,
        rows.len(),
        vec![
            Arc::new(UInt32Array::from_iter_values(
                rows.iter()
                    .map(|(ordinal, _)| u32::try_from(*ordinal).unwrap_or(u32::MAX)),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|(_, tree_id)| *tree_id),
            )),
        ],
    )
}

fn insert_semantic_relations(
    relations: &mut BTreeMap<NativeSyntaxRelation, RecordBatch>,
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<(), ArrowError> {
    insert(
        relations,
        NativeSyntaxRelation::RuffScope,
        ruff_scope_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffBinding,
        ruff_binding_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffReference,
        ruff_reference_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffUnknownSymbol,
        ruff_unknown_symbol_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffSemanticEdge,
        ruff_semantic_edge_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffImport,
        ruff_import_batch(pin, semantics)?,
    );
    insert(
        relations,
        NativeSyntaxRelation::RuffExport,
        ruff_export_batch(pin, semantics)?,
    );
    Ok(())
}

fn ruff_scope_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.scopes.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffScope,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.scope_id)),
            fixed16(rows, |row| row.parent_scope_id.as_ref()),
            utf8(rows, |row| Some(python_scope_kind(row.kind))),
            utf8(rows, |row| row.name.as_deref()),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn ruff_binding_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.bindings.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffBinding,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.binding_id)),
            fixed16(rows, |row| Some(&row.scope_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            utf8(rows, |row| Some(python_binding_kind(row.kind))),
            utf8(rows, |row| Some(python_target_form(row.target_form))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn ruff_reference_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.references.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffReference,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.reference_id)),
            fixed16(rows, |row| Some(&row.scope_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            utf8(rows, |row| Some(python_reference_class(row.class))),
            utf8(rows, |row| Some(python_resolution(row.resolution))),
            fixed16(rows, |row| Some(&row.target_id)),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            utf8(rows, |row| row.unknown_reason_code.as_deref()),
        ],
    )
}

fn ruff_unknown_symbol_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.unknown_symbols.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffUnknownSymbol,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.unknown_symbol_id)),
            fixed16(rows, |row| Some(&row.scope_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            utf8(rows, |row| Some(row.reason_code.as_str())),
        ],
    )
}

fn ruff_semantic_edge_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.edges.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffSemanticEdge,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.subject_id)),
            fixed16(rows, |row| Some(&row.object_id)),
            utf8(rows, |row| Some(python_semantic_edge_kind(row.kind))),
        ],
    )
}

fn ruff_import_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.imports.as_slice());
    batch(
        pin,
        NativeSyntaxRelation::RuffImport,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.import_id)),
            fixed16(rows, |row| Some(&row.scope_id)),
            utf8(rows, |row| Some(python_import_kind(row.kind))),
            Arc::new(UInt16Array::from(
                rows.iter()
                    .map(|row| {
                        row.relative_level
                            .and_then(|level| u16::try_from(level).ok())
                    })
                    .collect::<Vec<_>>(),
            )),
            utf8(rows, |row| Some(row.source_name.as_str())),
            utf8(rows, |row| row.alias_name.as_deref()),
            bools(rows, |row| row.star_import),
            fixed16(rows, |row| Some(&row.target_module_id)),
            utf8(rows, |row| row.target_module_name.as_deref()),
            utf8(rows, |row| row.ruff_qualified_name.as_deref()),
            utf8(rows, |row| Some(python_resolution(row.resolution))),
            fixed16(rows, |row| row.imported_entity_id.as_ref()),
            utf8(rows, |row| row.imported_name.as_deref()),
            fixed16(rows, |row| row.local_binding_id.as_ref()),
            utf8(rows, |row| row.unknown_reason_code.as_deref()),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn ruff_export_batch(
    pin: RelationPin<'_>,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    let rows = semantics.map_or(&[][..], |batch| batch.exports.as_slice());
    let status = semantics.map(|batch| python_export_status(batch.export_status));
    batch(
        pin,
        NativeSyntaxRelation::RuffExport,
        rows.len(),
        vec![
            fixed16(rows, |row| Some(&row.export_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            fixed16(rows, |row| Some(&row.target_id)),
            bools(rows, |row| row.reexport),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|_| status.unwrap_or("unknown")),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
        ],
    )
}

fn native_relation_specific_fields(relation: NativeSyntaxRelation) -> Vec<Field> {
    match relation {
        NativeSyntaxRelation::TreeSitterRun | NativeSyntaxRelation::RuffRun => vec![
            typed_field(
                "provider_revision",
                DataType::UInt64,
                false,
                "provider-local-revision",
            ),
            typed_field("catalog_id", DataType::Utf8, false, "provider-catalog-id"),
            typed_field(
                "inventory_fingerprint",
                DataType::Utf8,
                false,
                "provider-inventory-fingerprint",
            ),
            typed_field(
                "grammar_release",
                DataType::Utf8,
                true,
                "provider-grammar-release",
            ),
        ],
        NativeSyntaxRelation::TreeSitterCoverage | NativeSyntaxRelation::RuffCoverage => vec![
            typed_field("family", DataType::Utf8, false, "provider-api-family"),
            typed_field(
                "requested_units",
                DataType::UInt64,
                false,
                "coverage-requested",
            ),
            typed_field(
                "completed_units",
                DataType::UInt64,
                false,
                "coverage-completed",
            ),
            typed_field(
                "terminal_status",
                DataType::Utf8,
                false,
                "coverage-terminal",
            ),
            typed_field(
                "remainder_reason",
                DataType::Utf8,
                true,
                "coverage-remainder-reason",
            ),
        ],
        NativeSyntaxRelation::TreeSitterRemainder | NativeSyntaxRelation::RuffRemainder => vec![
            typed_field("family", DataType::Utf8, false, "provider-api-family"),
            typed_field("reason", DataType::Utf8, false, "remainder-reason"),
            typed_field("detail", DataType::Utf8, false, "bounded-diagnostic"),
        ],
        NativeSyntaxRelation::TreeSitterCstNode => vec![
            typed_field(
                "provider_local_node_id",
                DataType::UInt64,
                false,
                "provider-local-id",
            ),
            typed_field(
                "parent_provider_local_node_id",
                DataType::UInt64,
                true,
                "provider-local-id",
            ),
            typed_field(
                "raw_kind_id",
                DataType::UInt16,
                false,
                "provider-native-kind-id",
            ),
            typed_field("raw_kind", DataType::Utf8, false, "provider-native-kind"),
            typed_field("field_name", DataType::Utf8, true, "provider-native-field"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field("named", DataType::Boolean, false, "provider-native-flag"),
            typed_field("extra", DataType::Boolean, false, "provider-native-flag"),
            typed_field("error", DataType::Boolean, false, "provider-native-flag"),
            typed_field("missing", DataType::Boolean, false, "provider-native-flag"),
            typed_field("ordinal", DataType::UInt32, false, "provider-local-ordinal"),
            typed_field("depth", DataType::UInt16, false, "provider-local-depth"),
            typed_field(
                "raw_kind_disposition",
                DataType::Utf8,
                false,
                "raw-kind-disposition",
            ),
        ],
        NativeSyntaxRelation::TreeSitterChangedRange => vec![
            typed_field(
                "range_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => vec![
            typed_field(
                "provider_local_node_id",
                DataType::UInt64,
                false,
                "provider-local-id",
            ),
            typed_field(
                "recovery_kind",
                DataType::Utf8,
                false,
                "provider-native-recovery-kind",
            ),
            typed_field("raw_kind", DataType::Utf8, false, "provider-native-kind"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::RuffToken => vec![
            typed_field(
                "token_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field(
                "raw_kind_id",
                DataType::UInt16,
                false,
                "provider-native-kind-id",
            ),
            typed_field("raw_kind", DataType::Utf8, false, "provider-native-kind"),
            typed_field("token_class", DataType::Utf8, false, "provider-token-class"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "line",
                DataType::UInt32,
                false,
                "provider-native-coordinate",
            ),
            typed_field(
                "column",
                DataType::UInt32,
                false,
                "provider-native-coordinate",
            ),
            typed_field(
                "spelling_kind",
                DataType::Utf8,
                true,
                "provider-spelling-kind",
            ),
            typed_field(
                "spelling_value",
                DataType::Utf8,
                true,
                "provider-spelling-or-digest",
            ),
            typed_field(
                "provider_local_ast_id",
                DataType::UInt64,
                true,
                "provider-local-id",
            ),
        ],
        NativeSyntaxRelation::RuffComment => vec![
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "placement",
                DataType::Utf8,
                false,
                "provider-comment-placement",
            ),
            typed_field(
                "block_member",
                DataType::Boolean,
                false,
                "provider-native-flag",
            ),
        ],
        NativeSyntaxRelation::RuffDirective => vec![
            typed_field(
                "directive_kind",
                DataType::Utf8,
                false,
                "provider-directive-kind",
            ),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "provider_local_target_id",
                DataType::UInt64,
                true,
                "provider-local-id",
            ),
        ],
        NativeSyntaxRelation::RuffStringRegion => vec![
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "multiline",
                DataType::Boolean,
                false,
                "provider-native-flag",
            ),
            typed_field(
                "interpolated",
                DataType::Boolean,
                false,
                "provider-native-flag",
            ),
            typed_field(
                "provider_local_ast_id",
                DataType::UInt64,
                true,
                "provider-local-id",
            ),
        ],
        NativeSyntaxRelation::RuffDocstring => vec![
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "provider_local_owner_id",
                DataType::UInt64,
                false,
                "provider-local-id",
            ),
        ],
        NativeSyntaxRelation::RuffContinuationLine => vec![typed_field(
            "start_byte",
            DataType::UInt64,
            false,
            "source-byte-start",
        )],
        NativeSyntaxRelation::RuffAstNode => vec![
            typed_field(
                "provider_local_ast_id",
                DataType::UInt64,
                false,
                "provider-local-id",
            ),
            typed_field(
                "parent_provider_local_ast_id",
                DataType::UInt64,
                true,
                "provider-local-id",
            ),
            typed_field(
                "raw_kind_id",
                DataType::UInt16,
                false,
                "provider-native-kind-id",
            ),
            typed_field("raw_kind", DataType::Utf8, false, "provider-native-kind"),
            typed_field("ast_category", DataType::Utf8, false, "typed-ast-category"),
            typed_field("child_role", DataType::Utf8, true, "typed-ast-child-role"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field(
                "line",
                DataType::UInt32,
                false,
                "provider-native-coordinate",
            ),
            typed_field(
                "column",
                DataType::UInt32,
                false,
                "provider-native-coordinate",
            ),
            typed_field(
                "child_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field(
                "source_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field(
                "evaluation_ordinal",
                DataType::UInt32,
                true,
                "provider-local-ordinal",
            ),
            typed_field(
                "explicit_parenthesized",
                DataType::Boolean,
                false,
                "provider-native-flag",
            ),
            typed_field(
                "raw_kind_disposition",
                DataType::Utf8,
                false,
                "raw-kind-disposition",
            ),
        ],
        NativeSyntaxRelation::RuffParseDiagnostic => vec![
            typed_field(
                "diagnostic_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field(
                "diagnostic_kind",
                DataType::Utf8,
                false,
                "provider-diagnostic-kind",
            ),
            typed_field("message", DataType::Utf8, false, "bounded-diagnostic"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence => vec![
            typed_field(
                "diagnostic_ordinal",
                DataType::UInt32,
                false,
                "provider-local-ordinal",
            ),
            typed_field(
                "tree_sitter_provider_local_node_id",
                DataType::UInt64,
                false,
                "provider-local-id",
            ),
        ],
        NativeSyntaxRelation::RuffScope => vec![
            typed_field(
                "scope_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "parent_scope_id",
                DataType::FixedSizeBinary(16),
                true,
                "application-owned-observation-id",
            ),
            typed_field("scope_kind", DataType::Utf8, false, "ruff-local-scope-kind"),
            typed_field("name", DataType::Utf8, true, "source-name"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::RuffBinding => vec![
            typed_field(
                "binding_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "scope_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("name", DataType::Utf8, false, "source-name"),
            typed_field(
                "binding_kind",
                DataType::Utf8,
                false,
                "ruff-local-binding-kind",
            ),
            typed_field(
                "target_form",
                DataType::Utf8,
                false,
                "ruff-local-target-form",
            ),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::RuffReference => vec![
            typed_field(
                "reference_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "scope_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("name", DataType::Utf8, false, "source-name"),
            typed_field(
                "reference_class",
                DataType::Utf8,
                false,
                "ruff-local-reference-class",
            ),
            typed_field(
                "resolution",
                DataType::Utf8,
                false,
                "provider-resolution-state",
            ),
            typed_field(
                "target_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
            typed_field("unknown_reason", DataType::Utf8, true, "unknown-reason"),
        ],
        NativeSyntaxRelation::RuffUnknownSymbol => vec![
            typed_field(
                "unknown_symbol_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "scope_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("name", DataType::Utf8, false, "source-name"),
            typed_field("reason", DataType::Utf8, false, "unknown-reason"),
        ],
        NativeSyntaxRelation::RuffSemanticEdge => vec![
            typed_field(
                "subject_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "object_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "edge_kind",
                DataType::Utf8,
                false,
                "ruff-local-semantic-edge-kind",
            ),
        ],
        NativeSyntaxRelation::RuffImport => vec![
            typed_field(
                "import_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "scope_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "import_kind",
                DataType::Utf8,
                false,
                "ruff-local-import-kind",
            ),
            typed_field(
                "relative_level",
                DataType::UInt16,
                true,
                "source-relative-import-level",
            ),
            typed_field("source_name", DataType::Utf8, false, "source-name"),
            typed_field("alias_name", DataType::Utf8, true, "source-name"),
            typed_field(
                "star_import",
                DataType::Boolean,
                false,
                "provider-native-flag",
            ),
            typed_field(
                "target_module_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field(
                "target_module_name",
                DataType::Utf8,
                true,
                "provider-module-name",
            ),
            typed_field(
                "ruff_qualified_name",
                DataType::Utf8,
                true,
                "ruff-qualified-name",
            ),
            typed_field(
                "resolution",
                DataType::Utf8,
                false,
                "provider-resolution-state",
            ),
            typed_field(
                "imported_entity_id",
                DataType::FixedSizeBinary(16),
                true,
                "application-owned-observation-id",
            ),
            typed_field("imported_name", DataType::Utf8, true, "source-name"),
            typed_field(
                "local_binding_id",
                DataType::FixedSizeBinary(16),
                true,
                "application-owned-observation-id",
            ),
            typed_field("unknown_reason", DataType::Utf8, true, "unknown-reason"),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
        NativeSyntaxRelation::RuffExport => vec![
            typed_field(
                "export_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("name", DataType::Utf8, false, "source-name"),
            typed_field(
                "target_id",
                DataType::FixedSizeBinary(16),
                false,
                "application-owned-observation-id",
            ),
            typed_field("reexport", DataType::Boolean, false, "provider-native-flag"),
            typed_field(
                "export_status",
                DataType::Utf8,
                false,
                "provider-completeness-state",
            ),
            typed_field("start_byte", DataType::UInt64, false, "source-byte-start"),
            typed_field("end_byte", DataType::UInt64, false, "source-byte-end"),
        ],
    }
}

fn native_relation_schema(relation: NativeSyntaxRelation) -> SchemaRef {
    let mut fields = common_fields();
    fields.extend(native_relation_specific_fields(relation));
    let fields = fields
        .into_iter()
        .map(|field| {
            let mut metadata = field.metadata().clone();
            metadata.insert(
                "codefabric.field_id".to_owned(),
                format!("{}.{}", relation.as_str(), field.name()),
            );
            field.with_metadata(metadata)
        })
        .collect::<Vec<_>>();
    let metadata = HashMap::from([
        (
            "codefabric.relation_id".to_owned(),
            relation.as_str().to_owned(),
        ),
        (
            "codefabric.relation".to_owned(),
            relation.as_str().to_owned(),
        ),
        (
            "codefabric.provider_native_schema_release".to_owned(),
            PROVIDER_NATIVE_SYNTAX_SCHEMA_RELEASE.to_owned(),
        ),
        (
            "codefabric.schema_contract_id".to_owned(),
            format!(
                "provider-native-syntax-v{PROVIDER_NATIVE_SYNTAX_SCHEMA_RELEASE}:{}",
                relation.as_str()
            ),
        ),
        (
            "codefabric.semantic_encoding".to_owned(),
            "typed-arrow-fields-only".to_owned(),
        ),
    ]);
    Arc::new(Schema::new_with_metadata(fields, metadata))
}

fn batch(
    pin: RelationPin<'_>,
    relation: NativeSyntaxRelation,
    row_count: usize,
    extra_columns: Vec<ArrayRef>,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_columns(pin, row_count);
    columns.extend(extra_columns);
    RecordBatch::try_new(relation.schema(), columns)
}

fn common_fields() -> Vec<Field> {
    vec![
        typed_field(
            "provider_run_id",
            DataType::FixedSizeBinary(16),
            false,
            "provider-run-id",
        ),
        typed_field("provider_id", DataType::Utf8, false, "provider-id"),
        typed_field(
            "provider_release",
            DataType::Utf8,
            false,
            "provider-release",
        ),
        typed_field(
            "analysis_context_id",
            DataType::FixedSizeBinary(32),
            false,
            "analysis-context-id",
        ),
        typed_field(
            "semantic_environment_id",
            DataType::FixedSizeBinary(32),
            false,
            "semantic-environment-id",
        ),
        typed_field("file_id", DataType::FixedSizeBinary(16), false, "file-id"),
        typed_field(
            "content_digest",
            DataType::FixedSizeBinary(32),
            false,
            "content-digest",
        ),
        typed_field(
            "source_generation",
            DataType::UInt64,
            false,
            "source-generation",
        ),
    ]
}

fn common_columns(pin: RelationPin<'_>, row_count: usize) -> Vec<ArrayRef> {
    vec![
        fixed16_repeat(&pin.run.provider_run_id, row_count),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            pin.provider_id,
            row_count,
        ))),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            pin.provider_release,
            row_count,
        ))),
        fixed32_repeat(&pin.run.analysis_context_id, row_count),
        fixed32_repeat(&pin.run.semantic_environment_id, row_count),
        fixed16_repeat(&pin.source.file_id, row_count),
        fixed32_repeat(&pin.source.content_digest, row_count),
        Arc::new(UInt64Array::from_iter_values(std::iter::repeat_n(
            pin.source.source_generation,
            row_count,
        ))),
    ]
}

fn typed_field(name: &str, data_type: DataType, nullable: bool, meaning: &str) -> Field {
    Field::new(name, data_type, nullable).with_metadata(HashMap::from([
        ("codefabric.meaning".to_owned(), meaning.to_owned()),
        (
            "codefabric.semantic_representation".to_owned(),
            "typed-arrow-field".to_owned(),
        ),
    ]))
}

fn fixed16_repeat(value: &[u8; 16], count: usize) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::with_capacity(count, 16);
    for _ in 0..count {
        builder
            .append_value(value)
            .expect("Id16 has the exact Arrow storage width");
    }
    Arc::new(builder.finish())
}

fn fixed32_repeat(value: &[u8; 32], count: usize) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::with_capacity(count, 32);
    for _ in 0..count {
        builder
            .append_value(value)
            .expect("Hash32 has the exact Arrow storage width");
    }
    Arc::new(builder.finish())
}

fn fixed16<T>(
    rows: &[T],
    mut value: impl for<'a> FnMut(&'a T) -> Option<&'a [u8; 16]>,
) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows.len(), 16);
    for row in rows {
        if let Some(value) = value(row) {
            builder
                .append_value(value)
                .expect("Id16 has the exact Arrow storage width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn utf8<T>(rows: &[T], mut value: impl for<'a> FnMut(&'a T) -> Option<&'a str>) -> ArrayRef {
    let capacity = rows
        .iter()
        .filter_map(&mut value)
        .map(str::len)
        .sum::<usize>();
    let mut builder = StringBuilder::with_capacity(rows.len(), capacity);
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn bools<T>(rows: &[T], mut value: impl FnMut(&T) -> bool) -> ArrayRef {
    let mut builder = BooleanBuilder::with_capacity(rows.len());
    for row in rows {
        builder.append_value(value(row));
    }
    Arc::new(builder.finish())
}

const fn raw_kind_disposition(value: ProviderRawKindDisposition) -> &'static str {
    match value {
        ProviderRawKindDisposition::Normalize => "normalize",
        ProviderRawKindDisposition::Ignore => "ignore",
        ProviderRawKindDisposition::Unsupported => "unsupported",
    }
}

const fn ruff_token_class(value: RuffTokenClass) -> &'static str {
    match value {
        RuffTokenClass::Identifier => "identifier",
        RuffTokenClass::Keyword => "keyword",
        RuffTokenClass::Operator => "operator",
        RuffTokenClass::Literal => "literal",
        RuffTokenClass::Comment => "comment",
        RuffTokenClass::Newline => "newline",
        RuffTokenClass::Indentation => "indentation",
        RuffTokenClass::EndOfFile => "end-of-file",
        RuffTokenClass::Unknown => "unknown",
    }
}

const fn ruff_ast_category(value: RuffAstCategory) -> &'static str {
    match value {
        RuffAstCategory::SyntaxNode => "syntax-node",
        RuffAstCategory::Statement => "statement",
        RuffAstCategory::Expression => "expression",
        RuffAstCategory::Pattern => "pattern",
        RuffAstCategory::DeclarationSyntax => "declaration-syntax",
        RuffAstCategory::TypeSyntax => "type-syntax",
        RuffAstCategory::ParameterSyntax => "parameter-syntax",
        RuffAstCategory::ArgumentSyntax => "argument-syntax",
        RuffAstCategory::Block => "block",
        RuffAstCategory::Literal => "literal",
        RuffAstCategory::Operation => "operation",
        RuffAstCategory::AttributeAccess => "attribute-access",
        RuffAstCategory::SubscriptAccess => "subscript-access",
        RuffAstCategory::CallExpression => "call-expression",
        RuffAstCategory::Assignment => "assignment",
        RuffAstCategory::Branch => "branch",
        RuffAstCategory::Loop => "loop",
        RuffAstCategory::Return => "return",
        RuffAstCategory::Yield => "yield",
        RuffAstCategory::Await => "await",
        RuffAstCategory::RaiseSyntax => "raise-syntax",
        RuffAstCategory::ImportSyntax => "import-syntax",
    }
}

const fn ruff_child_role(value: RuffChildRole) -> &'static str {
    match value {
        RuffChildRole::Body => "body",
        RuffChildRole::Decorator => "decorator",
        RuffChildRole::Name => "name",
        RuffChildRole::TypeParameter => "type-parameter",
        RuffChildRole::Parameter => "parameter",
        RuffChildRole::Argument => "argument",
        RuffChildRole::KeywordArgument => "keyword-argument",
        RuffChildRole::Callee => "callee",
        RuffChildRole::Condition => "condition",
        RuffChildRole::Target => "target",
        RuffChildRole::Value => "value",
        RuffChildRole::Annotation => "annotation",
        RuffChildRole::Iterable => "iterable",
        RuffChildRole::Pattern => "pattern",
        RuffChildRole::Handler => "handler",
        RuffChildRole::Clause => "clause",
        RuffChildRole::Item => "item",
        RuffChildRole::Segment => "segment",
        RuffChildRole::Child => "child",
    }
}

const fn ruff_comment_placement(value: RuffCommentPlacement) -> &'static str {
    match value {
        RuffCommentPlacement::OwnLine => "own-line",
        RuffCommentPlacement::EndOfLine => "end-of-line",
    }
}

const fn ruff_directive_kind(value: RuffDirectiveKind) -> &'static str {
    match value {
        RuffDirectiveKind::Noqa => "noqa",
        RuffDirectiveKind::TypeIgnore => "type-ignore",
        RuffDirectiveKind::TypeComment => "type-comment",
        RuffDirectiveKind::Formatter => "formatter",
        RuffDirectiveKind::OtherPragma => "other-pragma",
    }
}

const fn ruff_diagnostic_kind(value: RuffDiagnosticKind) -> &'static str {
    match value {
        RuffDiagnosticKind::Parse => "parse",
        RuffDiagnosticKind::UnsupportedSyntax => "unsupported-syntax",
    }
}

const fn python_scope_kind(value: PythonScopeKind) -> &'static str {
    match value {
        PythonScopeKind::Module => "module",
        PythonScopeKind::Function => "function",
        PythonScopeKind::Class => "class",
        PythonScopeKind::Lambda => "lambda",
        PythonScopeKind::Comprehension => "comprehension",
        PythonScopeKind::Annotation => "annotation",
        PythonScopeKind::TypeParameter => "type-parameter",
    }
}

const fn python_binding_kind(value: PythonBindingKind) -> &'static str {
    match value {
        PythonBindingKind::Local => "local",
        PythonBindingKind::Parameter => "parameter",
        PythonBindingKind::Global => "global",
        PythonBindingKind::Nonlocal => "nonlocal",
        PythonBindingKind::Import => "import",
        PythonBindingKind::ClassAttribute => "class-attribute",
        PythonBindingKind::InstanceAttribute => "instance-attribute",
        PythonBindingKind::Comprehension => "comprehension",
        PythonBindingKind::Loop => "loop",
        PythonBindingKind::With => "with",
        PythonBindingKind::Exception => "exception",
        PythonBindingKind::Match => "match",
        PythonBindingKind::Walrus => "walrus",
        PythonBindingKind::TypeParameter => "type-parameter",
        PythonBindingKind::TypeAlias => "type-alias",
        PythonBindingKind::Free => "free",
        PythonBindingKind::Cell => "cell",
        PythonBindingKind::Builtin => "builtin",
        PythonBindingKind::Function => "function",
        PythonBindingKind::Class => "class",
    }
}

const fn python_target_form(value: PythonTargetForm) -> &'static str {
    match value {
        PythonTargetForm::FunctionName => "function-name",
        PythonTargetForm::ClassName => "class-name",
        PythonTargetForm::Parameter => "parameter",
        PythonTargetForm::Assignment => "assignment",
        PythonTargetForm::AnnotatedAssignment => "annotated-assignment",
        PythonTargetForm::AugmentedAssignment => "augmented-assignment",
        PythonTargetForm::NamedExpression => "named-expression",
        PythonTargetForm::ImportAlias => "import-alias",
        PythonTargetForm::LoopTarget => "loop-target",
        PythonTargetForm::WithTarget => "with-target",
        PythonTargetForm::ExceptionTarget => "exception-target",
        PythonTargetForm::MatchCapture => "match-capture",
        PythonTargetForm::ComprehensionTarget => "comprehension-target",
        PythonTargetForm::GlobalDeclaration => "global-declaration",
        PythonTargetForm::NonlocalDeclaration => "nonlocal-declaration",
        PythonTargetForm::TypeParameter => "type-parameter",
        PythonTargetForm::TypeAlias => "type-alias",
    }
}

const fn python_reference_class(value: PythonReferenceClass) -> &'static str {
    match value {
        PythonReferenceClass::Read => "read",
        PythonReferenceClass::Write => "write",
        PythonReferenceClass::ReadWrite => "read-write",
        PythonReferenceClass::Delete => "delete",
        PythonReferenceClass::TypeReference => "type-reference",
        PythonReferenceClass::CallReference => "call-reference",
        PythonReferenceClass::ImportReference => "import-reference",
    }
}

const fn python_resolution(value: PythonResolution) -> &'static str {
    match value {
        PythonResolution::Resolved => "resolved",
        PythonResolution::MayReferTo => "may-refer-to",
        PythonResolution::UnknownSymbol => "unknown-symbol",
        PythonResolution::UnboundLocal => "unbound-local",
    }
}

const fn python_semantic_edge_kind(value: PythonSemanticEdgeKind) -> &'static str {
    match value {
        PythonSemanticEdgeKind::RefersTo => "refers-to",
        PythonSemanticEdgeKind::MayReferTo => "may-refer-to",
        PythonSemanticEdgeKind::Shadows => "shadows",
        PythonSemanticEdgeKind::Rebinds => "rebinds",
        PythonSemanticEdgeKind::GlobalResolution => "global-resolution",
        PythonSemanticEdgeKind::NonlocalResolution => "nonlocal-resolution",
        PythonSemanticEdgeKind::Captures => "captures",
        PythonSemanticEdgeKind::CapturedFrom => "captured-from",
    }
}

const fn python_import_kind(value: PythonImportKind) -> &'static str {
    match value {
        PythonImportKind::Module => "module",
        PythonImportKind::FromName => "from-name",
        PythonImportKind::Star => "star",
        PythonImportKind::Dynamic => "dynamic",
    }
}

const fn python_export_status(value: PythonExportStatus) -> &'static str {
    match value {
        PythonExportStatus::Complete => "complete",
        PythonExportStatus::IncompleteDynamic => "incomplete-dynamic",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{Array as _, FixedSizeBinaryArray};

    use super::*;
    use crate::provider_types::ProviderText;

    fn source_image(text: &str) -> ProviderNativeSourceImage {
        let bytes = text.as_bytes().to_vec();
        let digest = crate::integrity::digest_bytes(&bytes);
        let provider_text = ProviderText {
            text: Arc::from(text),
            original_byte_offsets: Arc::from(
                text.char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap())
                    .chain(std::iter::once(u64::try_from(text.len()).unwrap()))
                    .collect::<Vec<_>>(),
            ),
        };
        ProviderNativeSourceImage::new([2; 16], 7, Arc::from(bytes), digest, provider_text).unwrap()
    }

    const fn pins() -> PythonSyntaxRunPins {
        PythonSyntaxRunPins {
            tree_sitter: SyntaxProviderRunPin {
                provider_run_id: [10; 16],
                analysis_context_id: [12; 32],
                semantic_environment_id: [13; 32],
            },
            ruff: SyntaxProviderRunPin {
                provider_run_id: [20; 16],
                analysis_context_id: [12; 32],
                semantic_environment_id: [13; 32],
            },
        }
    }

    fn run(text: &str) -> ProviderNativeSyntaxRun {
        let source = source_image(text);
        ExactPythonSyntaxRunner::new()
            .unwrap()
            .run_full(
                1,
                &source,
                pins(),
                PythonModuleInput {
                    module_name: "pkg.sample",
                    module_path: Path::new("pkg/sample.py"),
                },
                &Cancellation::default(),
            )
            .unwrap()
    }

    #[test]
    fn exact_provider_native_relations_are_typed_and_source_pinned() {
        let run = run(
            "from pkg import value as item\n\ndef f(arg: int):\n    # noqa\n    return item(arg)\n",
        );
        assert_eq!(run.relations.len(), NativeSyntaxRelation::ALL.len());
        for relation in NativeSyntaxRelation::ALL {
            let batch = run.relation(relation);
            assert_eq!(
                batch.schema().metadata()["codefabric.semantic_encoding"],
                "typed-arrow-fields-only"
            );
            assert!(batch.column_by_name("provider_run_id").is_some());
            assert!(batch.column_by_name("file_id").is_some());
            assert!(batch.column_by_name("content_digest").is_some());
            assert!(
                batch.column_by_name("model_epoch_id").is_none(),
                "provider-native observations must not claim predecessor model authority"
            );
            assert!(batch.schema().fields().iter().all(|field| !matches!(
                field.data_type(),
                DataType::Binary | DataType::LargeBinary
            )));
        }
        let tree = run.relation(NativeSyntaxRelation::TreeSitterCstNode);
        assert!(tree.num_rows() > 0);
        assert!(tree.column_by_name("raw_kind").is_some());
        let ruff_ast = run.relation(NativeSyntaxRelation::RuffAstNode);
        assert!(ruff_ast.num_rows() > 0);
        assert!(ruff_ast.column_by_name("raw_kind").is_some());
        assert!(run.relation(NativeSyntaxRelation::RuffToken).num_rows() > 0);
        assert!(run.relation(NativeSyntaxRelation::RuffScope).num_rows() > 0);
        assert!(run.relation(NativeSyntaxRelation::RuffBinding).num_rows() > 0);
        assert!(run.relation(NativeSyntaxRelation::RuffImport).num_rows() > 0);

        let file_ids = ruff_ast
            .column_by_name("file_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert!(file_ids.iter().flatten().all(|value| value == [2; 16]));
    }

    #[test]
    fn compiled_native_relation_schemas_exactly_match_every_emitted_batch() {
        let run = run("from pkg import value\nresult = value + 1\n");
        assert_eq!(run.relations.len(), NativeSyntaxRelation::ALL.len());
        for relation in NativeSyntaxRelation::ALL {
            assert_eq!(
                relation.schema().as_ref(),
                run.relation(relation).schema().as_ref(),
                "compiled schema drifted for {}",
                relation.as_str()
            );
        }
    }

    #[test]
    fn invalid_source_keeps_syntax_and_materializes_semantic_remainders() {
        let run = run("def incomplete(value:\n    return value\n");
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterCstNode)
                .num_rows()
                > 0
        );
        assert!(run.relation(NativeSyntaxRelation::RuffToken).num_rows() > 0);
        assert!(
            run.relation(NativeSyntaxRelation::RuffParseDiagnostic)
                .num_rows()
                > 0
        );
        assert_eq!(run.relation(NativeSyntaxRelation::RuffScope).num_rows(), 0);
        assert_eq!(
            run.relation(NativeSyntaxRelation::RuffRemainder).num_rows(),
            7
        );
    }

    #[test]
    fn incremental_run_emits_structural_changed_ranges() {
        let source_v1 = source_image("value = 1\n");
        let source_v2 = source_image("value = foo(2)\n");
        let mut runner = ExactPythonSyntaxRunner::new().unwrap();
        runner
            .run_full(
                1,
                &source_v1,
                pins(),
                PythonModuleInput {
                    module_name: "pkg.sample",
                    module_path: Path::new("pkg/sample.py"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        let run = runner
            .run_incremental(
                2,
                &source_v2,
                TreeSitterEdit {
                    start_byte: 8,
                    old_end_byte: 9,
                    new_end_byte: 14,
                },
                pins(),
                PythonModuleInput {
                    module_name: "pkg.sample",
                    module_path: Path::new("pkg/sample.py"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterChangedRange)
                .num_rows()
                > 0
        );
    }

    #[test]
    fn provider_authority_does_not_claim_cfg_or_dataflow() {
        for relation in NativeSyntaxRelation::ALL {
            let name = relation.as_str();
            assert!(!name.contains("cfg"));
            assert!(!name.contains("dataflow"));
            assert!(!name.contains("semantic_json"));
        }
    }
}
