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

mod callables;

use arrow_array::builder::{BooleanBuilder, FixedSizeBinaryBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use crate::provider_contracts::{
    ProviderContractError, ProviderCoverage, ProviderCoverageState, ProviderJob, ProviderLane,
    ProviderRelationOutput, ProviderRunEvidenceSpec, ProviderRunResult, ProviderRunSupport,
    ProviderTerminalStatus, ProviderTrustOutcome, RELATION_SEMANTIC_ROLE_METADATA_KEY,
    SEMANTIC_ROLE_METADATA_KEY,
};
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
pub const PROVIDER_NATIVE_SYNTAX_SCHEMA_RELEASE: &str = "2";

const TREE_SITTER_PROVIDER_ID: &str = "tree-sitter-python";
const RUFF_PROVIDER_ID: &str = "ruff-python";
const TREE_SITTER_PROVIDER_RELEASE: &str = "tree-sitter=0.26.12;tree-sitter-python=0.25.0";
const RUFF_PROVIDER_RELEASE: &str = "ruff-python-ast=0.0.7;ruff-python-parser=0.0.7";

/// Immutable run pins repeated by every provider-native relation row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxProviderRunPin {
    pub provider_run_id: [u8; 16],
    /// Application-owned canonical identity, never a shortened fingerprint.
    pub analysis_context_id: [u8; 16],
    /// Exact effective context manifest fingerprint used by lane admission.
    pub context_fingerprint: [u8; 32],
    pub semantic_environment_id: [u8; 32],
    pub python_version: Option<(u16, u16)>,
}

/// The two exact in-process provider runs that observe one immutable source image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PythonSyntaxRunPins {
    tree_sitter: SyntaxProviderRunPin,
    ruff: SyntaxProviderRunPin,
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
    pub bytes: crate::resource_budget::ChargedSlice<u8>,
    pub content_digest: [u8; 32],
    pub provider_text: ProviderText,
}

impl ProviderNativeSourceImage {
    /// Construct and validate an exact source image.
    ///
    /// # Errors
    ///
    /// Rejects digest drift, text/byte drift, or an incomplete/non-monotonic original-byte map.
    pub fn new(
        file_id: [u8; 16],
        source_generation: u64,
        bytes: crate::resource_budget::ChargedSlice<u8>,
        content_digest: [u8; 32],
        provider_text: ProviderText,
    ) -> Result<Self, ProviderNativeSyntaxError> {
        Self::new_for_language(
            file_id,
            source_generation,
            bytes,
            content_digest,
            provider_text,
            false,
        )
    }

    fn new_for_language(
        file_id: [u8; 16],
        source_generation: u64,
        bytes: crate::resource_budget::ChargedSlice<u8>,
        content_digest: [u8; 32],
        provider_text: ProviderText,
        python: bool,
    ) -> Result<Self, ProviderNativeSyntaxError> {
        let source = Self {
            file_id,
            source_generation,
            bytes,
            content_digest,
            provider_text,
        };
        validated_provider_text(&source, python)?;
        Ok(source)
    }
}

#[cfg(feature = "daemon")]
impl TryFrom<&SourceImage> for ProviderNativeSourceImage {
    type Error = ProviderNativeSyntaxError;

    fn try_from(source: &SourceImage) -> Result<Self, Self::Error> {
        if !matches!(
            source.language,
            SourceLanguage::Python | SourceLanguage::Rust
        ) {
            return Err(ProviderNativeSyntaxError::InvalidSource(
                "language has no supported syntax provider",
            ));
        }
        Self::new_for_language(
            source.file_id,
            source.source_generation,
            source.bytes.clone(),
            source.digest,
            source
                .provider_text
                .clone()
                .ok_or(ProviderNativeSyntaxError::InvalidSource(
                    "provider UTF-8 text is unavailable",
                ))?,
            source.language == SourceLanguage::Python,
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
    RuffCallable,
    RuffCallSite,
    RuffCallableSyntax,
}

impl NativeSyntaxRelation {
    pub const ALL: [Self; 28] = [
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
        Self::RuffCallable,
        Self::RuffCallSite,
        Self::RuffCallableSyntax,
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
            Self::RuffCallable => "provider.ruff.callable",
            Self::RuffCallSite => "provider.ruff.call_site",
            Self::RuffCallableSyntax => "provider.ruff.callable_syntax",
        }
    }

    /// Return the exact application-owned Arrow schema compiled for this relation.
    ///
    /// Batch construction consumes the same schema, so schema-only contract compilation cannot
    /// drift from a provider-emitted relation or require executing a provider on fabricated source.
    #[must_use]
    pub fn schema(self) -> SchemaRef {
        native_relation_schema(self)
    }
}

/// Two validated in-process jobs observing one immutable source/context generation.
#[derive(Clone, Copy, Debug)]
pub struct InProcessProviderJobs<'a> {
    tree_sitter: &'a ProviderJob,
    ruff: &'a ProviderJob,
}

impl<'a> InProcessProviderJobs<'a> {
    /// Join one Tree-sitter job and one Ruff job before provider execution.
    ///
    /// # Errors
    ///
    /// Rejects wrong lanes or source/context/suite drift between the two jobs.
    pub fn try_new(
        tree_sitter: &'a ProviderJob,
        ruff: &'a ProviderJob,
    ) -> Result<Self, ProviderNativeSyntaxError> {
        if tree_sitter.lane() != ProviderLane::TreeSitter
            || ruff.lane() != ProviderLane::Ruff
            || tree_sitter.suite() != ruff.suite()
            || tree_sitter.source() != ruff.source()
            || tree_sitter.context() != ruff.context()
        {
            return Err(ProviderNativeSyntaxError::MixedRunContext);
        }
        Ok(Self { tree_sitter, ruff })
    }

    fn pins(self) -> Result<PythonSyntaxRunPins, ProviderNativeSyntaxError> {
        let context = self.tree_sitter.context();
        let python_version =
            context
                .python_version()
                .ok_or(ProviderNativeSyntaxError::InvalidModule(
                    "effective context does not select a Python language version",
                ))?;
        Ok(PythonSyntaxRunPins {
            tree_sitter: SyntaxProviderRunPin {
                provider_run_id: self.tree_sitter.run().provider_run_id(),
                analysis_context_id: context.analysis_context_id(),
                context_fingerprint: context.context_fingerprint(),
                semantic_environment_id: context.semantic_environment_id(),
                python_version: Some(python_version),
            },
            ruff: SyntaxProviderRunPin {
                provider_run_id: self.ruff.run().provider_run_id(),
                analysis_context_id: context.analysis_context_id(),
                context_fingerprint: context.context_fingerprint(),
                semantic_environment_id: context.semantic_environment_id(),
                python_version: Some(python_version),
            },
        })
    }
}

/// One complete application-owned result set for the two exact in-process lanes.
#[derive(Clone, Debug)]
pub struct ProviderNativeSyntaxRun {
    pub relations: BTreeMap<NativeSyntaxRelation, RecordBatch>,
    tree_sitter: ProviderRunResult,
    ruff: ProviderRunResult,
}

impl ProviderNativeSyntaxRun {
    /// Fetch one typed relation. Every declared family is present, including empty ones.
    #[must_use]
    pub fn relation(&self, relation: NativeSyntaxRelation) -> &RecordBatch {
        &self.relations[&relation]
    }

    #[must_use]
    pub const fn tree_sitter_result(&self) -> &ProviderRunResult {
        &self.tree_sitter
    }

    #[must_use]
    pub const fn ruff_result(&self) -> &ProviderRunResult {
        &self.ruff
    }
}

/// Stable failures at the exact-provider/Arrow boundary.
#[derive(Debug, Error)]
pub enum ProviderNativeSyntaxError {
    #[error("source image is not an exact valid source image: {0}")]
    InvalidSource(&'static str),
    #[error("module input differs from the selected effective context: {0}")]
    InvalidModule(&'static str),
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
    #[error(transparent)]
    Contract(#[from] ProviderContractError),
}

/// Stateful exact-current Python syntax lane.
///
/// Tree-sitter keeps its bounded revision cache for changed-range evidence. Ruff reparses each
/// source image in full, exactly matching the current library's capabilities.
pub struct ExactPythonSyntaxRunner {
    tree_sitter: TreeSitterAdapter,
    ruff: RuffAdapter,
}

/// Bounded native-state observation derived from adapter-owned lifecycle values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InProcessProviderLifecycleObservation {
    pub tree_sitter_retained_revisions: u16,
    pub ruff_retained_revisions: u16,
    pub tree_sitter_completed_runs: u64,
    pub ruff_completed_runs: u64,
}

impl ExactPythonSyntaxRunner {
    /// Construct the exact providers and execute a compile/runtime probe against their pinned APIs.
    ///
    /// # Errors
    ///
    /// Returns provider/API errors when the current pinned runtime cannot execute its documented
    /// exact parser, grammar, token, trivia, index, or typed-AST surfaces.
    pub fn new(jobs: InProcessProviderJobs<'_>) -> Result<Self, ProviderNativeSyntaxError> {
        Ok(Self {
            tree_sitter: TreeSitterAdapter::new(TreeSitterLanguage::Python, jobs.tree_sitter)?,
            ruff: RuffAdapter::new(jobs.ruff)?,
        })
    }

    /// Parse one source image without Tree-sitter reuse and emit all typed native relations.
    ///
    /// # Errors
    ///
    /// Rejects invalid source/pin/snapshot state, provider failures, and Arrow schema violations.
    pub fn run_full(
        &mut self,
        jobs: InProcessProviderJobs<'_>,
        revision: u64,
        source: &ProviderNativeSourceImage,
        module: PythonModuleInput<'_>,
    ) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
        validate_job_source(jobs, source)?;
        validate_job_module(jobs, source.file_id, module)?;
        let text = validated_provider_text(source, true)?;
        let pins = jobs.pins()?;
        validate_run_pins(pins)?;
        let tree = self
            .tree_sitter
            .parse_full(jobs.tree_sitter, revision, text.clone())?;
        let ruff = self.ruff.parse(jobs.ruff, revision, text, &tree)?;
        let semantics = semantic_result(&self.ruff, jobs.ruff, revision, module)?;
        finish_run(jobs, source, pins, &tree, &ruff, semantics.as_deref())
    }

    /// Apply one exact edit to the retained Tree-sitter tree, reparse Ruff in full, and emit the
    /// changed-range relation alongside the complete current provider relations.
    ///
    /// # Errors
    ///
    /// In addition to [`Self::run_full`] failures, rejects a stale or geometrically invalid edit.
    pub fn run_incremental(
        &mut self,
        jobs: InProcessProviderJobs<'_>,
        revision: u64,
        source: &ProviderNativeSourceImage,
        edit: TreeSitterEdit,
        module: PythonModuleInput<'_>,
    ) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
        validate_job_source(jobs, source)?;
        validate_job_module(jobs, source.file_id, module)?;
        let text = validated_provider_text(source, true)?;
        let pins = jobs.pins()?;
        validate_run_pins(pins)?;
        let tree =
            self.tree_sitter
                .parse_incremental(jobs.tree_sitter, revision, text.clone(), edit)?;
        let ruff = self.ruff.parse(jobs.ruff, revision, text, &tree)?;
        let semantics = semantic_result(&self.ruff, jobs.ruff, revision, module)?;
        finish_run(jobs, source, pins, &tree, &ruff, semantics.as_deref())
    }

    #[must_use]
    pub fn lifecycle_observation(&self) -> InProcessProviderLifecycleObservation {
        let tree = self.tree_sitter.metrics();
        let ruff = self.ruff.metrics();
        InProcessProviderLifecycleObservation {
            tree_sitter_retained_revisions: tree.retained_revisions,
            ruff_retained_revisions: ruff.retained_revisions,
            tree_sitter_completed_runs: tree.completed_runs,
            ruff_completed_runs: ruff.completed_runs,
        }
    }
}

fn validate_job_source(
    jobs: InProcessProviderJobs<'_>,
    source: &ProviderNativeSourceImage,
) -> Result<(), ProviderNativeSyntaxError> {
    crate::provider_contracts::allocation::require_native_workspace(
        jobs.ruff.resource_budget(),
        jobs.tree_sitter.resource_budget(),
    )?;
    validate_single_job_source(jobs.tree_sitter, source)
}

pub(crate) fn validate_single_job_source(
    job: &ProviderJob,
    source: &ProviderNativeSourceImage,
) -> Result<(), ProviderNativeSyntaxError> {
    let binding = job.source();
    for budget in [
        source.bytes.reservation().owner(),
        source.provider_text.text.reservation().owner(),
        source
            .provider_text
            .original_byte_offsets
            .reservation()
            .owner(),
    ] {
        crate::provider_contracts::allocation::require_native_workspace(
            budget,
            job.resource_budget(),
        )?;
    }
    if binding.file_id() != Some(source.file_id)
        || binding.generation() != source.source_generation
        || binding.content_digest() != source.content_digest
    {
        return Err(ProviderNativeSyntaxError::InvalidSource(
            "provider job source pins differ from the immutable source image",
        ));
    }
    Ok(())
}

fn validate_job_module(
    jobs: InProcessProviderJobs<'_>,
    file_id: [u8; 16],
    module: PythonModuleInput<'_>,
) -> Result<(), ProviderNativeSyntaxError> {
    let selected = jobs.tree_sitter.context().module_for_file(file_id).ok_or(
        ProviderNativeSyntaxError::InvalidModule("source has no selected module binding"),
    )?;
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStrExt;
        module.module_path.as_os_str().as_bytes()
    };
    #[cfg(not(unix))]
    let path = module
        .module_path
        .to_str()
        .ok_or(ProviderNativeSyntaxError::InvalidModule(
            "module path is not representable",
        ))?
        .as_bytes();
    if selected.qualified_name != module.module_name || selected.relative_path != path {
        return Err(ProviderNativeSyntaxError::InvalidModule(
            "module name or byte-native path differs from the selected context",
        ));
    }
    Ok(())
}

fn semantic_result(
    ruff: &RuffAdapter,
    job: &ProviderJob,
    revision: u64,
    module: PythonModuleInput<'_>,
) -> Result<
    Option<crate::resource_budget::ChargedValue<PythonFrontendBatch>>,
    ProviderNativeSyntaxError,
> {
    match ruff.semantic_batch(job, revision, module.module_name, module.module_path, false) {
        Ok(batch) => Ok(Some(batch)),
        Err(PythonSemanticError::UnavailableParse(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn validate_run_pins(pins: PythonSyntaxRunPins) -> Result<(), ProviderNativeSyntaxError> {
    if pins.tree_sitter.python_version.is_none()
        || pins.tree_sitter.analysis_context_id != pins.ruff.analysis_context_id
        || pins.tree_sitter.context_fingerprint != pins.ruff.context_fingerprint
        || pins.tree_sitter.semantic_environment_id != pins.ruff.semantic_environment_id
        || pins.tree_sitter.python_version != pins.ruff.python_version
    {
        return Err(ProviderNativeSyntaxError::MixedRunContext);
    }
    Ok(())
}

pub(crate) fn validated_provider_text(
    source: &ProviderNativeSourceImage,
    python: bool,
) -> Result<ProviderText, ProviderNativeSyntaxError> {
    if crate::integrity::digest_bytes(&source.bytes) != source.content_digest {
        return Err(ProviderNativeSyntaxError::InvalidSource(
            "content digest differs from source bytes",
        ));
    }
    let text = source.provider_text.clone();
    let decoded = crate::source_encoding::DecodedSource::select(&source.bytes, python)
        .map_err(|_| ProviderNativeSyntaxError::InvalidSource("unsupported source encoding"))?;
    if !decoded.characters().map(|(_, ch)| ch).eq(text.text.chars())
        || !decoded
            .characters()
            .map(|(offset, _)| offset as u64)
            .chain(std::iter::once(decoded.original_len() as u64))
            .eq(text.original_byte_offsets.iter().copied())
    {
        return Err(ProviderNativeSyntaxError::InvalidSource(
            "provider text or boundary map differs from immutable source decoding",
        ));
    }
    Ok(text)
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

pub(crate) fn project_tree_relations(
    source: &ProviderNativeSourceImage,
    run: SyntaxProviderRunPin,
    tree: &TreeSitterSnapshot,
    provider_id: &'static str,
    provider_release: &'static str,
    grammar_release: &'static str,
) -> Result<BTreeMap<NativeSyntaxRelation, RecordBatch>, ProviderNativeSyntaxError> {
    if tree.provider_image_fingerprint != source.provider_text.provider_image_fingerprint() {
        return Err(ProviderNativeSyntaxError::SnapshotMismatch(
            "provider image fingerprint",
        ));
    }
    let tree_pin = RelationPin {
        run,
        provider_id,
        provider_release,
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
            Some(grammar_release),
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

    Ok(relations)
}

#[allow(clippy::too_many_lines)] // One projection keeps the declared native relation surface together.
fn project_relations(
    source: &ProviderNativeSourceImage,
    pins: PythonSyntaxRunPins,
    tree: &TreeSitterSnapshot,
    ruff: &RuffSnapshot,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<BTreeMap<NativeSyntaxRelation, RecordBatch>, ProviderNativeSyntaxError> {
    validate_snapshots(source, tree, ruff)?;
    let ruff_pin = RelationPin {
        run: pins.ruff,
        provider_id: RUFF_PROVIDER_ID,
        provider_release: ruff.provider_version,
        source,
    };
    let mut relations = project_tree_relations(
        source,
        pins.tree_sitter,
        tree,
        TREE_SITTER_PROVIDER_ID,
        TREE_SITTER_PROVIDER_RELEASE,
        TREE_SITTER_PYTHON_GRAMMAR_RELEASE,
    )?;

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
        "ruff.callable",
        "ruff.call_site",
        "ruff.callable_syntax",
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
    Ok(relations)
}

fn finish_run(
    jobs: InProcessProviderJobs<'_>,
    source: &ProviderNativeSourceImage,
    pins: PythonSyntaxRunPins,
    tree: &TreeSitterSnapshot,
    ruff: &RuffSnapshot,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
    // Both lane envelopes are admitted before constructing any Arrow output. Claims below are
    // private, one-time claims of newly built buffers, never a reclaim of arbitrary input arrays.
    let mut tree_allocation = crate::provider_contracts::allocation::ProviderAllocation::try_new(
        jobs.tree_sitter.resource_budget(),
        jobs.tree_sitter.ceilings().max_bytes(),
    )?;
    let mut ruff_allocation = crate::provider_contracts::allocation::ProviderAllocation::try_new(
        jobs.ruff.resource_budget(),
        jobs.ruff.ceilings().max_bytes(),
    )?;
    let relations = project_relations(source, pins, tree, ruff, semantics)?;
    let is_tree =
        |relation: &NativeSyntaxRelation| relation.as_str().starts_with("provider.tree_sitter.");
    tree_allocation.claim_new_batches(
        relations
            .iter()
            .filter(|(key, _)| is_tree(key))
            .map(|(_, batch)| batch),
        65_536,
    )?;
    ruff_allocation.claim_new_batches(
        relations
            .iter()
            .filter(|(key, _)| !is_tree(key))
            .map(|(_, batch)| batch),
        65_536,
    )?;
    let tree_sitter = provider_result(jobs.tree_sitter, &relations, true)?;
    let ruff = provider_result(jobs.ruff, &relations, semantics.is_some())?;
    Ok(ProviderNativeSyntaxRun {
        relations,
        tree_sitter,
        ruff,
    })
}

fn provider_result(
    job: &ProviderJob,
    relations: &BTreeMap<NativeSyntaxRelation, RecordBatch>,
    semantics_available: bool,
) -> Result<ProviderRunResult, ProviderNativeSyntaxError> {
    let mut outputs = Vec::with_capacity(job.requests().len());
    let mut coverage = Vec::with_capacity(job.requests().len());
    let mut gaps = Vec::new();
    for request in job.requests() {
        let relation = NativeSyntaxRelation::ALL
            .into_iter()
            .find(|relation| relation.as_str() == request.relation().as_str())
            .ok_or(ProviderContractError::UnrequestedRelation)?;
        let expected_lane = match relation {
            NativeSyntaxRelation::TreeSitterRun
            | NativeSyntaxRelation::TreeSitterCoverage
            | NativeSyntaxRelation::TreeSitterRemainder
            | NativeSyntaxRelation::TreeSitterCstNode
            | NativeSyntaxRelation::TreeSitterChangedRange
            | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => ProviderLane::TreeSitter,
            _ => ProviderLane::Ruff,
        };
        if expected_lane != job.lane() || relation.schema() != *request.schema() {
            return Err(ProviderContractError::ArrowSchemaMismatch.into());
        }
        outputs.push(ProviderRelationOutput::try_new(
            request.relation().clone(),
            request.schema_identity().clone(),
            Arc::clone(request.schema()),
            vec![relations[&relation].clone()],
            job.resource_budget(),
        )?);
        let state = if !semantics_available
            && matches!(
                relation,
                NativeSyntaxRelation::RuffScope
                    | NativeSyntaxRelation::RuffBinding
                    | NativeSyntaxRelation::RuffReference
                    | NativeSyntaxRelation::RuffUnknownSymbol
                    | NativeSyntaxRelation::RuffSemanticEdge
                    | NativeSyntaxRelation::RuffImport
                    | NativeSyntaxRelation::RuffExport
                    | NativeSyntaxRelation::RuffCallable
                    | NativeSyntaxRelation::RuffCallSite
                    | NativeSyntaxRelation::RuffCallableSyntax
            ) {
            let cause = crate::provider_contracts::ProviderUnknownCause::ProviderFailure;
            gaps.push(crate::provider_contracts::ProviderGap::try_new(
                request.family().clone(),
                cause,
                "Ruff local semantics unavailable because the source parse is invalid",
            )?);
            ProviderCoverageState::Unknown {
                completed_units: 0,
                cause,
            }
        } else {
            ProviderCoverageState::Complete {
                completed_units: request.requested_units(),
            }
        };
        coverage.push(ProviderCoverage::new(request.family().clone(), state));
    }
    let terminal = if gaps.is_empty() {
        ProviderTerminalStatus::Complete
    } else {
        ProviderTerminalStatus::Failed
    };
    Ok(ProviderRunResult::try_from_job(
        job,
        ProviderRunEvidenceSpec {
            relations: outputs,
            coverage,
            gaps,
            diagnostics: Vec::new(),
            trust: ProviderTrustOutcome::Trusted,
            terminal,
            support: ProviderRunSupport::from_job_inputs(job, true),
        },
    )?)
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
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.normalized_kind.0),
            )),
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
            Arc::new(UInt16Array::from_iter_values(
                rows.iter().map(|row| row.category.registry_code()),
            )),
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
    for relation in [
        NativeSyntaxRelation::RuffCallable,
        NativeSyntaxRelation::RuffCallSite,
        NativeSyntaxRelation::RuffCallableSyntax,
    ] {
        insert(
            relations,
            relation,
            callables::project(pin, relation, semantics)?,
        );
    }
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
        NativeSyntaxRelation::RuffCallable
        | NativeSyntaxRelation::RuffCallSite
        | NativeSyntaxRelation::RuffCallableSyntax => callables::fields(relation),
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
            typed_field(
                "normalized_kind_code",
                DataType::UInt16,
                false,
                "application-normalized-kind",
            ),
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
            typed_field(
                "normalized_kind_code",
                DataType::UInt16,
                false,
                "application-normalized-kind",
            ),
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
            if let Some(semantic_role) = native_field_semantic_role(relation, field.name()) {
                metadata.insert(
                    SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                    semantic_role.to_owned(),
                );
            }
            field.with_metadata(metadata)
        })
        .collect::<Vec<_>>();
    let mut metadata = HashMap::from([
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
    if let Some(semantic_role) = native_relation_semantic_role(relation) {
        metadata.insert(
            RELATION_SEMANTIC_ROLE_METADATA_KEY.to_owned(),
            semantic_role.to_owned(),
        );
    }
    Arc::new(Schema::new_with_metadata(fields, metadata))
}

/// Query semantics carried by the exact provider contract, not inferred from table/column names.
const fn native_relation_semantic_role(relation: NativeSyntaxRelation) -> Option<&'static str> {
    match relation {
        NativeSyntaxRelation::RuffBinding => Some("semantic.entity-source"),
        _ => None,
    }
}

fn native_field_semantic_role(
    relation: NativeSyntaxRelation,
    field_name: &str,
) -> Option<&'static str> {
    match (relation, field_name) {
        (NativeSyntaxRelation::RuffBinding, "binding_id") => Some("semantic.entity.identity"),
        (NativeSyntaxRelation::RuffBinding, "binding_kind") => Some("semantic.entity.kind"),
        (NativeSyntaxRelation::RuffBinding, "target_form") => {
            Some("semantic.entity.declaration-form")
        }
        (NativeSyntaxRelation::RuffBinding, "name") => Some("semantic.entity.name"),
        (NativeSyntaxRelation::RuffBinding, "analysis_context_id") => {
            Some("semantic.provenance.analysis-context")
        }
        (NativeSyntaxRelation::RuffBinding, "file_id") => Some("semantic.provenance.source-file"),
        (NativeSyntaxRelation::RuffBinding, "source_generation") => {
            Some("semantic.provenance.source-generation")
        }
        (NativeSyntaxRelation::RuffBinding, "start_byte") => Some("semantic.source.start-byte"),
        (NativeSyntaxRelation::RuffBinding, "end_byte") => Some("semantic.source.end-byte"),
        _ => None,
    }
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
            DataType::FixedSizeBinary(16),
            false,
            "canonical-analysis-context-id",
        ),
        typed_field(
            "context_fingerprint",
            DataType::FixedSizeBinary(32),
            false,
            "effective-context-manifest-fingerprint",
        ),
        typed_field(
            "semantic_environment_id",
            DataType::FixedSizeBinary(32),
            false,
            "semantic-environment-id",
        ),
        typed_field(
            "python_target_major",
            DataType::UInt16,
            true,
            "selected-python-major",
        ),
        typed_field(
            "python_target_minor",
            DataType::UInt16,
            true,
            "selected-python-minor",
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
        fixed16_repeat(&pin.run.analysis_context_id, row_count),
        fixed32_repeat(&pin.run.context_fingerprint, row_count),
        fixed32_repeat(&pin.run.semantic_environment_id, row_count),
        Arc::new(UInt16Array::from_iter(std::iter::repeat_n(
            pin.run.python_version.map(|version| version.0),
            row_count,
        ))),
        Arc::new(UInt16Array::from_iter(std::iter::repeat_n(
            pin.run.python_version.map(|version| version.1),
            row_count,
        ))),
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
pub(crate) mod job_tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::provider_contracts::{
        CancellationHandle, CancellationProbe, ContextIdentity, ProviderBuildIdentity,
        ProviderContextBinding, ProviderFamilyIdentity, ProviderFamilyRequest, ProviderIdentity,
        ProviderJobSpec, ProviderModuleBinding, ProviderPolicyIdentity, ProviderProgramIdentity,
        ProviderProtocolIdentity, ProviderRelationIdentity, ProviderResourceCeilingSpec,
        ProviderResourceCeilings, ProviderRunBinding, ProviderRunIdentity, ProviderRunProvenance,
        ProviderSchemaIdentity, ProviderScopeIdentity, ProviderSourceBinding, ProviderTrustPosture,
        SourceIdentity, SuiteIdentity, admit_provider_result,
    };

    struct FixtureJobs {
        tree_owner: CancellationHandle,
        ruff_owner: CancellationHandle,
        tree: ProviderJob,
        ruff: ProviderJob,
    }

    impl FixtureJobs {
        fn borrowed(&self) -> InProcessProviderJobs<'_> {
            InProcessProviderJobs::try_new(&self.tree, &self.ruff).unwrap()
        }
    }

    fn source(text: &str, generation: u64) -> ProviderNativeSourceImage {
        source_with_marker(text, generation, 1)
    }

    fn source_with_marker(text: &str, generation: u64, marker: u8) -> ProviderNativeSourceImage {
        let budget = crate::provider_contracts::fixture_provider_budget([6; 16], [254; 16]);
        let bytes = budget
            .try_reserve(
                crate::resource_budget::ResourceClass::Data,
                crate::resource_budget::ResourceAmounts {
                    memory_bytes: text.len() as u64,
                    ..Default::default()
                },
            )
            .unwrap()
            .into_charged_vec(text.as_bytes().to_vec())
            .unwrap();
        ProviderNativeSourceImage::new(
            [marker; 16],
            generation,
            bytes.clone(),
            crate::integrity::digest_bytes(&bytes),
            ProviderText::from_validated_utf8(text, &budget).unwrap(),
        )
        .unwrap()
    }

    fn limits() -> ProviderResourceCeilingSpec {
        ProviderResourceCeilingSpec {
            max_relations: 64,
            max_batches_per_relation: 8,
            max_input_bytes: 1 << 20,
            max_rows: 2_000_000,
            max_bytes: 1 << 26,
            max_diagnostics: 10_000,
            max_work_units: 10_000_000,
            max_wall_millis: 30_000,
            max_visited_nodes: 2_000_000,
            max_traversal_depth: 256,
            max_workers: 4,
            max_retained_revisions: 2,
            cancellation_poll_work_units: 1,
            cancellation_ack_millis: 2_000,
        }
    }

    fn lane(relation: NativeSyntaxRelation) -> ProviderLane {
        match relation {
            NativeSyntaxRelation::TreeSitterRun
            | NativeSyntaxRelation::TreeSitterCoverage
            | NativeSyntaxRelation::TreeSitterRemainder
            | NativeSyntaxRelation::TreeSitterCstNode
            | NativeSyntaxRelation::TreeSitterChangedRange
            | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => ProviderLane::TreeSitter,
            _ => ProviderLane::Ruff,
        }
    }

    fn requests(target: ProviderLane) -> Vec<ProviderFamilyRequest> {
        NativeSyntaxRelation::ALL
            .into_iter()
            .filter(|relation| lane(*relation) == target)
            .map(|relation| {
                ProviderFamilyRequest::try_new(
                    ProviderFamilyIdentity::try_new(format!(
                        "codefabric.provider-family.v2.3.{}",
                        relation.as_str()
                    ))
                    .unwrap(),
                    ProviderRelationIdentity::try_new(relation.as_str()).unwrap(),
                    ProviderSchemaIdentity::try_new(format!(
                        "codefabric.provider-schema.v2.3.{}",
                        relation.as_str()
                    ))
                    .unwrap(),
                    relation.schema(),
                    ProviderScopeIdentity::try_new("fixture.source").unwrap(),
                    1,
                )
                .unwrap()
            })
            .collect()
    }

    fn job(
        target: ProviderLane,
        source: &ProviderNativeSourceImage,
        run_pin: [u8; 16],
        spec: ProviderResourceCeilingSpec,
    ) -> (CancellationHandle, ProviderJob) {
        job_with_modules(
            target,
            source,
            run_pin,
            spec,
            vec![ProviderModuleBinding {
                file_id: source.file_id,
                qualified_name: "fixture.module".into(),
                relative_path: b"fixture/module.py".to_vec(),
            }],
        )
    }

    fn job_with_modules(
        target: ProviderLane,
        source: &ProviderNativeSourceImage,
        run_pin: [u8; 16],
        spec: ProviderResourceCeilingSpec,
        modules: Vec<ProviderModuleBinding>,
    ) -> (CancellationHandle, ProviderJob) {
        let (owner, cancellation) =
            CancellationProbe::pair(spec.cancellation_poll_work_units).unwrap();
        let provider = match target {
            ProviderLane::TreeSitter => "tree-sitter-python",
            ProviderLane::Ruff => "ruff-python",
            _ => unreachable!(),
        };
        let context = ProviderContextBinding::try_new(
            ContextIdentity::try_new("fixture.context").unwrap(),
            [5; 16],
            [3; 32],
            [4; 32],
        )
        .unwrap()
        .with_modules(modules)
        .unwrap()
        .with_python_version(3, 14)
        .unwrap();
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3.0").unwrap(),
            provider: ProviderIdentity::try_new(provider).unwrap(),
            protocol: ProviderProtocolIdentity::try_new("in-process-arrow@1").unwrap(),
            source: ProviderSourceBinding::try_file(
                SourceIdentity::try_new("fixture.source").unwrap(),
                [6; 16],
                source.file_id,
                source.source_generation,
                source.content_digest,
            )
            .unwrap(),
            context: crate::provider_contracts::fixture_provider_context([6; 16], context),
            run: ProviderRunBinding::try_new(
                ProviderRunIdentity::try_new(format!("{provider}.run")).unwrap(),
                run_pin,
            )
            .unwrap(),
            lane: target,
            trust: ProviderTrustPosture::InProcessConstrained,
            requests: requests(target),
            ceilings: ProviderResourceCeilings::try_new(spec).unwrap(),
            resource_budget: crate::provider_contracts::fixture_provider_budget([6; 16], run_pin),
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation,
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new(format!("{provider}.build")).unwrap(),
                ProviderPolicyIdentity::try_new("codefabric.policy-program.v2.3").unwrap(),
                ProviderProgramIdentity::try_new("codefabric.provider-program.v2.3").unwrap(),
            ),
        })
        .unwrap();
        (owner, job)
    }

    fn jobs(source: &ProviderNativeSourceImage) -> FixtureJobs {
        jobs_with_marker(source, 1)
    }

    fn jobs_with_marker(source: &ProviderNativeSourceImage, marker: u8) -> FixtureJobs {
        let (tree_owner, tree) = job(ProviderLane::TreeSitter, source, [marker; 16], limits());
        let (ruff_owner, ruff) = job(
            ProviderLane::Ruff,
            source,
            [marker.wrapping_add(64); 16],
            limits(),
        );
        FixtureJobs {
            tree_owner,
            ruff_owner,
            tree,
            ruff,
        }
    }

    fn module() -> PythonModuleInput<'static> {
        PythonModuleInput {
            module_name: "fixture.module",
            module_path: Path::new("fixture/module.py"),
        }
    }

    pub(crate) fn run_fixture(text: &str, generation: u64, marker: u8) -> ProviderNativeSyntaxRun {
        let source = source_with_marker(text, generation, marker);
        let jobs = jobs_with_marker(&source, marker);
        ExactPythonSyntaxRunner::new(jobs.borrowed())
            .unwrap()
            .run_full(jobs.borrowed(), 1, &source, module())
            .unwrap()
    }

    #[test]
    fn decoded_sources_preserve_native_ranges_and_reject_forged_maps() {
        for bytes in [
            b"# coding: latin-1\r\n# \xe9\r\ndef caf\xe9():\r\n    return caf\xe9()\r\n".as_slice(),
            b"\xef\xbb\xbf# \xc3\xa9\r\ndef caf\xc3\xa9():\r\n    return caf\xc3\xa9()\r\n",
        ] {
            let budget = crate::provider_contracts::fixture_provider_budget([6; 16], [254; 16]);
            let decoded = crate::source_encoding::DecodedSource::select(bytes, true).unwrap();
            let text: String = decoded.characters().map(|(_, ch)| ch).collect();
            let mut mapped = ProviderText::from_validated_utf8(&text, &budget).unwrap();
            mapped.original_byte_offsets = crate::resource_budget::ChargedSlice::try_from_fn(
                &budget,
                crate::resource_budget::ResourceClass::Data,
                text.chars().count() + 1,
                || {
                    let mut offsets = Vec::with_capacity(text.chars().count() + 1);
                    offsets.extend(decoded.characters().map(|(offset, _)| offset as u64));
                    offsets.push(bytes.len() as u64);
                    offsets
                },
            )
            .unwrap();
            let source = ProviderNativeSourceImage::new_for_language(
                [1; 16],
                1,
                crate::resource_budget::ChargedSlice::try_from_fn(
                    &budget,
                    crate::resource_budget::ResourceClass::Data,
                    bytes.len(),
                    || bytes.to_vec(),
                )
                .unwrap(),
                crate::integrity::digest_bytes(bytes),
                mapped,
                true,
            )
            .unwrap();
            let jobs = jobs(&source);
            let mut runner = ExactPythonSyntaxRunner::new(jobs.borrowed()).unwrap();
            let run = runner
                .run_full(jobs.borrowed(), 1, &source, module())
                .unwrap();
            let bindings = run.relation(NativeSyntaxRelation::RuffBinding);
            let names = bindings
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let starts = bindings
                .column_by_name("start_byte")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            let ends = bindings
                .column_by_name("end_byte")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            let row = (0..bindings.num_rows())
                .find(|row| names.value(*row) == "café")
                .unwrap();
            let start = bytes
                .windows(4)
                .position(|window| window == b"def ")
                .unwrap()
                + 4;
            let end = bytes[start..]
                .iter()
                .position(|byte| *byte == b'(')
                .unwrap()
                + start;
            assert_eq!(
                (starts.value(row), ends.value(row)),
                (start as u64, end as u64)
            );
            let mut forged = source.clone();
            forged.provider_text = ProviderText::from_validated_utf8(&text, &budget).unwrap();
            assert!(validated_provider_text(&forged, true).is_err());
        }
    }

    #[test]
    fn inprocess_provider_boundary_integrity() {
        assert_eq!(requests(ProviderLane::TreeSitter).len(), 6);
        assert_eq!(requests(ProviderLane::Ruff).len(), 22);
        assert!(
            crate::provider_raw_kinds::RUFF_PYTHON_FRONTEND
                .catalog_id
                .starts_with("ruff-")
        );
        for relation in NativeSyntaxRelation::ALL {
            assert!(!relation.schema().fields().is_empty());
        }
    }

    #[test]
    fn malformed_python_keeps_syntax_but_not_complete_semantic_coverage() {
        let run = run_fixture("def broken(:\n    return 1\n", 1, 44);
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterCstNode)
                .num_rows()
                > 0
        );
        assert!(
            run.relation(NativeSyntaxRelation::RuffParseDiagnostic)
                .num_rows()
                > 0
        );
        assert_eq!(
            run.tree_sitter_result().terminal(),
            ProviderTerminalStatus::Complete
        );
        assert_eq!(run.ruff_result().terminal(), ProviderTerminalStatus::Failed);
        let coverage = run.ruff_result().coverage();
        let binding = coverage
            .iter()
            .find(|row| row.family().as_str().ends_with("provider.ruff.binding"))
            .unwrap();
        assert!(matches!(
            binding.state(),
            ProviderCoverageState::Unknown {
                completed_units: 0,
                ..
            }
        ));
    }

    #[test]
    fn wp79_native_raw_arrow_clone_retains_backing_after_result_and_runner_drop() {
        let input = source("answer = 42\n", 1);
        let jobs = jobs(&input);
        let budget = jobs.ruff.resource_budget().clone();
        let baseline = budget.observation().used.memory_bytes;
        let mut runner = ExactPythonSyntaxRunner::new(jobs.borrowed()).unwrap();
        let run = runner
            .run_full(jobs.borrowed(), 1, &input, module())
            .unwrap();
        let raw = run.relation(NativeSyntaxRelation::RuffToken).slice(0, 1);
        drop(run);
        drop(runner);
        let retained = budget.observation().used.memory_bytes;
        assert!(
            retained > baseline,
            "raw Arrow slice must retain its native claims"
        );
        assert!(
            retained < u128::from(jobs.ruff.ceilings().max_bytes()),
            "tiny finished output must not retain the whole work envelope"
        );
        drop(raw);
        assert_eq!(budget.observation().used.memory_bytes, baseline);
    }

    #[test]
    fn wp79_native_admission_rejects_pressure_and_foreign_source_authority() {
        use crate::resource_budget::{ResourceAmounts, ResourceClass};
        let input = source("answer = 42\n", 1);
        let jobs = jobs(&input);
        let budget = jobs.ruff.resource_budget();
        let pressure = budget
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 1_000_000_000,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(ExactPythonSyntaxRunner::new(jobs.borrowed()).is_err());
        drop(pressure);
        let mut runner = ExactPythonSyntaxRunner::new(jobs.borrowed()).unwrap();
        let mut foreign = input.clone();
        foreign.bytes = crate::resource_budget::ChargedSlice::for_test(input.bytes.to_vec());
        assert!(matches!(
            runner.run_full(jobs.borrowed(), 1, &foreign, module()),
            Err(ProviderNativeSyntaxError::Contract(
                ProviderContractError::ResourceOwnerMismatch
            ))
        ));
        assert_eq!(runner.lifecycle_observation().tree_sitter_completed_runs, 0);
        assert_eq!(runner.lifecycle_observation().ruff_completed_runs, 0);
    }

    #[test]
    fn native_context_identity_and_fingerprint_remain_distinct() {
        use arrow_array::{Array, FixedSizeBinaryArray};

        let run = run_fixture("value = 1\n", 1, 7);
        for relation in NativeSyntaxRelation::ALL {
            let batch = run.relation(relation);
            let schema = batch.schema_ref();
            assert_eq!(
                schema.metadata()["codefabric.provider_native_schema_release"],
                "2"
            );
            for (name, width, marker) in [
                ("analysis_context_id", 16, 5),
                ("context_fingerprint", 32, 3),
                ("semantic_environment_id", 32, 4),
            ] {
                assert_eq!(
                    schema.field_with_name(name).unwrap().data_type(),
                    &DataType::FixedSizeBinary(width),
                );
                let column = batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap();
                assert_eq!(column.null_count(), 0);
                for row in 0..batch.num_rows() {
                    assert_eq!(
                        column.value(row),
                        vec![marker; usize::try_from(width).unwrap()]
                    );
                }
            }
        }
        let canonical_field = NativeSyntaxRelation::RuffBinding
            .schema()
            .field_with_name("analysis_context_id")
            .unwrap()
            .clone();
        assert_eq!(
            canonical_field.metadata()[SEMANTIC_ROLE_METADATA_KEY],
            "semantic.provenance.analysis-context",
        );

        let pin = SyntaxProviderRunPin {
            provider_run_id: [1; 16],
            analysis_context_id: [5; 16],
            context_fingerprint: [3; 32],
            semantic_environment_id: [4; 32],
            python_version: Some((3, 14)),
        };
        for other in [
            SyntaxProviderRunPin {
                analysis_context_id: [3; 16],
                ..pin
            },
            SyntaxProviderRunPin {
                context_fingerprint: [5; 32],
                ..pin
            },
        ] {
            assert!(matches!(
                validate_run_pins(PythonSyntaxRunPins {
                    tree_sitter: pin,
                    ruff: other,
                }),
                Err(ProviderNativeSyntaxError::MixedRunContext),
            ));
        }
    }

    #[test]
    fn native_context_module_binding_is_a_causal_provider_input() {
        let input = source("value = 1\n", 1);
        let jobs = jobs(&input);
        let mut runner = ExactPythonSyntaxRunner::new(jobs.borrowed()).unwrap();
        for module in [
            PythonModuleInput {
                module_name: "wrong.module",
                ..module()
            },
            PythonModuleInput {
                module_path: Path::new("wrong/module.py"),
                ..module()
            },
        ] {
            assert!(matches!(
                runner.run_full(jobs.borrowed(), 1, &input, module),
                Err(ProviderNativeSyntaxError::InvalidModule(_)),
            ));
        }
        assert_eq!(runner.lifecycle_observation().tree_sitter_completed_runs, 0);
        assert_eq!(runner.lifecycle_observation().ruff_completed_runs, 0);

        let (_tree_owner, tree) = job_with_modules(
            ProviderLane::TreeSitter,
            &input,
            [1; 16],
            limits(),
            Vec::new(),
        );
        let (_ruff_owner, ruff) =
            job_with_modules(ProviderLane::Ruff, &input, [2; 16], limits(), Vec::new());
        assert!(matches!(
            runner.run_full(
                InProcessProviderJobs::try_new(&tree, &ruff).unwrap(),
                1,
                &input,
                module()
            ),
            Err(ProviderNativeSyntaxError::InvalidModule(_)),
        ));

        let run = runner
            .run_full(jobs.borrowed(), 1, &input, module())
            .unwrap();
        assert!(
            !run.tree_sitter_result()
                .support()
                .requires_context_invalidation()
        );
        let mut support = run.tree_sitter_result().support().clone();
        support.common_dependencies = Arc::from([]);
        let detached = ProviderRunResult::try_from_job(
            &jobs.tree,
            ProviderRunEvidenceSpec {
                relations: run.tree_sitter_result().relations().to_vec(),
                coverage: run.tree_sitter_result().coverage().to_vec(),
                gaps: Vec::new(),
                diagnostics: Vec::new(),
                trust: ProviderTrustOutcome::Trusted,
                terminal: ProviderTerminalStatus::Complete,
                support,
            },
        )
        .unwrap();
        assert!(matches!(
            admit_provider_result(jobs.tree.clone(), detached),
            Err(ProviderContractError::SupportMismatch),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn native_context_module_paths_preserve_non_utf8_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let input = source("value = 1\n", 1);
        let raw_path = b"fixture/non-utf8-\xff.py";
        let module_binding = ProviderModuleBinding {
            file_id: input.file_id,
            qualified_name: "fixture.module".into(),
            relative_path: raw_path.to_vec(),
        };
        let (_tree_owner, tree) = job_with_modules(
            ProviderLane::TreeSitter,
            &input,
            [1; 16],
            limits(),
            vec![module_binding.clone()],
        );
        let (_ruff_owner, ruff) = job_with_modules(
            ProviderLane::Ruff,
            &input,
            [2; 16],
            limits(),
            vec![module_binding],
        );
        let jobs = InProcessProviderJobs::try_new(&tree, &ruff).unwrap();
        let mut runner = ExactPythonSyntaxRunner::new(jobs).unwrap();
        let run = runner
            .run_full(
                jobs,
                1,
                &input,
                PythonModuleInput {
                    module_name: "fixture.module",
                    module_path: Path::new(OsStr::from_bytes(raw_path)),
                },
            )
            .unwrap();
        assert!(run.relation(NativeSyntaxRelation::RuffBinding).num_rows() > 0);
        assert!(matches!(
            runner.run_incremental(
                jobs,
                2,
                &input,
                TreeSitterEdit {
                    start_byte: 0,
                    old_end_byte: 0,
                    new_end_byte: 0
                },
                module()
            ),
            Err(ProviderNativeSyntaxError::InvalidModule(_)),
        ));
    }

    #[test]
    fn tree_sitter_ruff_arrow_job_semantics() {
        let source = source("from pkg import value\nresult = value + 1\n", 1);
        let jobs = jobs(&source);
        let run = ExactPythonSyntaxRunner::new(jobs.borrowed())
            .unwrap()
            .run_full(jobs.borrowed(), 1, &source, module())
            .unwrap();
        assert_eq!(run.relations.len(), NativeSyntaxRelation::ALL.len());
        assert_eq!(run.tree_sitter_result().relations().len(), 6);
        assert_eq!(run.ruff_result().relations().len(), 22);
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterCstNode)
                .num_rows()
                > 0
        );
        assert!(run.relation(NativeSyntaxRelation::RuffToken).num_rows() > 0);
        assert!(run.relation(NativeSyntaxRelation::RuffAstNode).num_rows() > 0);
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterCstNode)
                .schema()
                .field_with_name("raw_kind")
                .is_ok()
        );
        assert!(
            run.relation(NativeSyntaxRelation::TreeSitterCstNode)
                .schema()
                .field_with_name("normalized_kind_code")
                .is_ok()
        );
        assert!(
            run.relation(NativeSyntaxRelation::RuffAstNode)
                .schema()
                .field_with_name("normalized_kind_code")
                .is_ok()
        );

        let tree =
            admit_provider_result(jobs.tree.clone(), run.tree_sitter_result().clone()).unwrap();
        let ruff = admit_provider_result(jobs.ruff.clone(), run.ruff_result().clone()).unwrap();
        assert_eq!(tree.observation().emitted_relations, 6);
        assert_eq!(ruff.observation().emitted_relations, 22);
    }

    #[test]
    fn inprocess_provider_admission_faults() {
        let input = source("value = 1\n", 1);
        let jobs = jobs(&input);
        jobs.tree_owner.cancel();
        let error = ExactPythonSyntaxRunner::new(jobs.borrowed())
            .unwrap()
            .run_full(jobs.borrowed(), 1, &input, module())
            .unwrap_err();
        assert!(matches!(
            error,
            ProviderNativeSyntaxError::TreeSitter(TreeSitterAdapterError::Cancelled)
        ));

        let wrong_source = source("value = 2\n", 2);
        let error = ExactPythonSyntaxRunner::new(jobs.borrowed())
            .unwrap()
            .run_full(jobs.borrowed(), 1, &wrong_source, module())
            .unwrap_err();
        assert!(matches!(error, ProviderNativeSyntaxError::InvalidSource(_)));
        let _ruff_owner_remains_live = &jobs.ruff_owner;
    }

    #[test]
    fn inprocess_provider_incremental_lifecycle() {
        let first_source = source("value = 1\n", 1);
        let first_jobs = jobs(&first_source);
        let mut incremental = ExactPythonSyntaxRunner::new(first_jobs.borrowed()).unwrap();
        incremental
            .run_full(first_jobs.borrowed(), 1, &first_source, module())
            .unwrap();

        let second_source = source("value = 2\n", 2);
        let second_jobs = jobs(&second_source);
        let incremental_run = incremental
            .run_incremental(
                second_jobs.borrowed(),
                2,
                &second_source,
                TreeSitterEdit {
                    start_byte: 8,
                    old_end_byte: 9,
                    new_end_byte: 9,
                },
                module(),
            )
            .unwrap();
        let clean_run = ExactPythonSyntaxRunner::new(second_jobs.borrowed())
            .unwrap()
            .run_full(second_jobs.borrowed(), 2, &second_source, module())
            .unwrap();
        for relation in [
            NativeSyntaxRelation::TreeSitterCstNode,
            NativeSyntaxRelation::RuffToken,
            NativeSyntaxRelation::RuffAstNode,
        ] {
            assert_eq!(
                incremental_run.relation(relation),
                clean_run.relation(relation)
            );
        }
        let lifecycle = incremental.lifecycle_observation();
        assert_eq!(lifecycle.tree_sitter_retained_revisions, 2);
        assert_eq!(lifecycle.ruff_retained_revisions, 1);
        assert_eq!(lifecycle.tree_sitter_completed_runs, 2);
        assert_eq!(lifecycle.ruff_completed_runs, 2);
    }

    #[test]
    fn wp65_measure_inprocess_tree_sitter_ruff() {
        let mut first_text = String::new();
        for index in 0..256 {
            use std::fmt::Write as _;
            writeln!(
                first_text,
                "def function_{index}(value: int) -> int:\n    return value + 1\n"
            )
            .unwrap();
        }
        let edit_start = first_text.rfind('1').expect("frozen one-byte edit");
        let mut second_text = first_text.clone().into_bytes();
        second_text[edit_start] = b'2';
        let second_text = String::from_utf8(second_text).unwrap();

        let first_source = source_with_marker(&first_text, 1, 0x65);
        let first_jobs = jobs_with_marker(&first_source, 0x65);
        let mut retained = ExactPythonSyntaxRunner::new(first_jobs.borrowed()).unwrap();
        let initial_started = Instant::now();
        let initial = retained
            .run_full(first_jobs.borrowed(), 1, &first_source, module())
            .unwrap();
        let initial_millis = initial_started.elapsed().as_secs_f64() * 1_000.0;

        let second_source = source_with_marker(&second_text, 2, 0x66);
        let second_jobs = jobs_with_marker(&second_source, 0x66);
        let incremental_started = Instant::now();
        let incremental = retained
            .run_incremental(
                second_jobs.borrowed(),
                2,
                &second_source,
                TreeSitterEdit {
                    start_byte: edit_start,
                    old_end_byte: edit_start + 1,
                    new_end_byte: edit_start + 1,
                },
                module(),
            )
            .unwrap();
        let incremental_millis = incremental_started.elapsed().as_secs_f64() * 1_000.0;

        let clean_started = Instant::now();
        let clean = ExactPythonSyntaxRunner::new(second_jobs.borrowed())
            .unwrap()
            .run_full(second_jobs.borrowed(), 2, &second_source, module())
            .unwrap();
        let clean_millis = clean_started.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(incremental.relations, clean.relations);
        let output_rows = initial
            .relations
            .values()
            .chain(incremental.relations.values())
            .map(RecordBatch::num_rows)
            .sum::<usize>();
        let output_bytes = initial
            .relations
            .values()
            .chain(incremental.relations.values())
            .map(RecordBatch::get_array_memory_size)
            .sum::<usize>();
        let elapsed_seconds = ((initial_millis + incremental_millis) / 1_000.0).max(f64::EPSILON);
        let lifecycle = retained.lifecycle_observation();
        assert_eq!(lifecycle.tree_sitter_retained_revisions, 2);
        assert_eq!(lifecycle.ruff_retained_revisions, 1);

        if std::env::var_os("CODEFABRIC_WP65_MEASURE").is_some() {
            println!(
                "CODEFABRIC_WP65_OBSERVATION={}",
                serde_json::json!({
                    "workload_id": "inprocess_tree_sitter_ruff",
                    "initial_millis": initial_millis,
                    "incremental_millis": incremental_millis,
                    "clean_millis": clean_millis,
                    "input_bytes": first_text.len() + second_text.len(),
                    "output_rows": output_rows,
                    "output_bytes": output_bytes,
                    "throughput_bytes_per_second": output_bytes as f64 / elapsed_seconds,
                    "incremental_equals_clean": true,
                    "tree_sitter_retained_revisions": lifecycle.tree_sitter_retained_revisions,
                    "ruff_retained_revisions": lifecycle.ruff_retained_revisions,
                })
            );
        }
    }
}
