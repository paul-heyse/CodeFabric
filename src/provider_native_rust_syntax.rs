//! Rust CST observations independent of Cargo and compiler availability.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{Schema, SchemaRef};

use crate::provider_contracts::{
    ProviderContractError, ProviderCoverage, ProviderCoverageState, ProviderJob, ProviderLane,
    ProviderRelationOutput, ProviderRunEvidenceSpec, ProviderRunResult, ProviderRunSupport,
    ProviderTerminalStatus, ProviderTrustOutcome,
};
use crate::provider_native_syntax::{
    NativeSyntaxRelation, ProviderNativeSourceImage, ProviderNativeSyntaxError,
    SyntaxProviderRunPin,
};
use crate::tree_sitter_adapter::{TreeSitterAdapter, TreeSitterLanguage};

pub(crate) const PROVIDER: &str = "tree-sitter-rust";
pub(crate) const RELEASE: &str = "tree-sitter=0.26.12;tree-sitter-rust=0.24.2";
pub(crate) const GRAMMAR_RELEASE: &str = "0.24.2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RustSyntaxRelation {
    Run,
    Coverage,
    Remainder,
    CstNode,
    ChangedRange,
    RecoveryDiagnostic,
}

impl RustSyntaxRelation {
    pub(crate) const ALL: [Self; 6] = [
        Self::Run,
        Self::Coverage,
        Self::Remainder,
        Self::CstNode,
        Self::ChangedRange,
        Self::RecoveryDiagnostic,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Run => "provider.tree_sitter_rust.run",
            Self::Coverage => "provider.tree_sitter_rust.coverage",
            Self::Remainder => "provider.tree_sitter_rust.remainder",
            Self::CstNode => "provider.tree_sitter_rust.cst_node",
            Self::ChangedRange => "provider.tree_sitter_rust.changed_range",
            Self::RecoveryDiagnostic => "provider.tree_sitter_rust.recovery_diagnostic",
        }
    }

    const fn native(self) -> NativeSyntaxRelation {
        match self {
            Self::Run => NativeSyntaxRelation::TreeSitterRun,
            Self::Coverage => NativeSyntaxRelation::TreeSitterCoverage,
            Self::Remainder => NativeSyntaxRelation::TreeSitterRemainder,
            Self::CstNode => NativeSyntaxRelation::TreeSitterCstNode,
            Self::ChangedRange => NativeSyntaxRelation::TreeSitterChangedRange,
            Self::RecoveryDiagnostic => NativeSyntaxRelation::TreeSitterRecoveryDiagnostic,
        }
    }

    fn indices(self) -> Vec<usize> {
        self.native()
            .schema()
            .fields()
            .iter()
            .enumerate()
            .filter(|(_, field)| !field.name().starts_with("python_target_"))
            .map(|(index, _)| index)
            .collect()
    }

    /// Reuse the exact CST shape and coordinate semantics with Rust's own identity.
    pub(crate) fn schema(self) -> SchemaRef {
        let original = self.native().schema();
        let fields = self
            .indices()
            .into_iter()
            .map(|index| {
                let field = original.field(index).clone();
                let mut metadata = field.metadata().clone();
                metadata.insert(
                    "codefabric.field_id".to_owned(),
                    format!("{}.{}", self.name(), field.name()),
                );
                field.with_metadata(metadata)
            })
            .collect::<Vec<_>>();
        Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([
                ("codefabric.relation_id".to_owned(), self.name().to_owned()),
                ("codefabric.relation".to_owned(), self.name().to_owned()),
                (
                    "codefabric.schema_contract_id".to_owned(),
                    format!("rust-native-syntax-v1:{}", self.name()),
                ),
                ("codefabric.provider_release".to_owned(), RELEASE.to_owned()),
                (
                    "codefabric.semantic_encoding".to_owned(),
                    "typed-arrow-fields-only".to_owned(),
                ),
            ]),
        ))
    }

    fn project(self, batch: &RecordBatch) -> Result<RecordBatch, ProviderNativeSyntaxError> {
        Ok(RecordBatch::try_new(
            self.schema(),
            self.indices()
                .into_iter()
                .map(|index| Arc::clone(batch.column(index)))
                .collect(),
        )?)
    }
}

/// A retained parser owns all native trees. Only application-owned Arrow leaves this module.
pub(crate) struct ExactRustSyntaxRunner {
    parser: TreeSitterAdapter,
}

impl ExactRustSyntaxRunner {
    pub(crate) fn new(job: &ProviderJob) -> Result<Self, ProviderNativeSyntaxError> {
        validate_job(job)?;
        Ok(Self {
            parser: TreeSitterAdapter::new(TreeSitterLanguage::Rust, job)?,
        })
    }

    pub(crate) fn run_captured(
        &mut self,
        job: &ProviderJob,
        source: &ProviderNativeSourceImage,
    ) -> Result<ProviderRunResult, ProviderNativeSyntaxError> {
        validate_job(job)?;
        crate::provider_native_syntax::validate_single_job_source(job, source)?;
        let text = crate::provider_native_syntax::validated_provider_text(source, false)?;
        let tree = self.parser.parse_captured(job, text)?;
        Self::finish(job, source, &tree)
    }

    pub(crate) fn native_reservations(&self) -> crate::resource_budget::ResourceAmounts {
        self.parser.native_reservations()
    }

    fn finish(
        job: &ProviderJob,
        source: &ProviderNativeSourceImage,
        tree: &crate::tree_sitter_adapter::TreeSitterSnapshot,
    ) -> Result<ProviderRunResult, ProviderNativeSyntaxError> {
        if tree.catalog_id != "tree-sitter-rust-0-24-2" {
            return Err(ProviderNativeSyntaxError::SnapshotMismatch(
                "Rust grammar catalog",
            ));
        }
        let context = job.context();
        let pin = SyntaxProviderRunPin {
            provider_run_id: job.run().provider_run_id(),
            analysis_context_id: context.analysis_context_id(),
            context_fingerprint: context.context_fingerprint(),
            semantic_environment_id: context.semantic_environment_id(),
            python_version: None,
        };
        let mut allocation = crate::provider_contracts::allocation::ProviderAllocation::try_new(
            job.resource_budget(),
            job.ceilings().max_bytes(),
        )?;
        let raw = crate::provider_native_syntax::project_tree_relations(
            source,
            pin,
            tree,
            PROVIDER,
            RELEASE,
            GRAMMAR_RELEASE,
        )?;
        let mut outputs = Vec::new();
        let mut coverage = Vec::new();
        let projected = RustSyntaxRelation::ALL
            .into_iter()
            .map(|relation| Ok((relation.name(), relation.project(&raw[&relation.native()])?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, ProviderNativeSyntaxError>>()?;
        allocation.claim_new_batches(projected.values(), 4096)?;
        for request in job.requests() {
            let relation = RustSyntaxRelation::ALL
                .into_iter()
                .find(|relation| relation.name() == request.relation().as_str())
                .ok_or(ProviderContractError::UnrequestedRelation)?;
            if request.schema().as_ref() != relation.schema().as_ref() {
                return Err(ProviderContractError::ArrowSchemaMismatch.into());
            }
            let batch = projected[relation.name()].clone();
            outputs.push(ProviderRelationOutput::try_new(
                request.relation().clone(),
                request.schema_identity().clone(),
                Arc::clone(request.schema()),
                vec![batch],
                job.resource_budget(),
            )?);
            coverage.push(ProviderCoverage::new(
                request.family().clone(),
                ProviderCoverageState::Complete {
                    completed_units: request.requested_units(),
                },
            ));
        }
        Ok(ProviderRunResult::try_from_job(
            job,
            ProviderRunEvidenceSpec {
                relations: outputs,
                coverage,
                gaps: Vec::new(),
                diagnostics: Vec::new(),
                trust: ProviderTrustOutcome::Trusted,
                terminal: ProviderTerminalStatus::Complete,
                support: ProviderRunSupport::from_job_inputs(job, true),
            },
        )?)
    }
}

fn validate_job(job: &ProviderJob) -> Result<(), ProviderNativeSyntaxError> {
    if job.lane() != ProviderLane::TreeSitterRust
        || job.provider().as_str() != PROVIDER
        || job.context().python_version().is_some()
        || job.context().analysis_context_id() != crate::identity::SOURCE_CONTEXT_ID
        || job.provenance().provider_build().as_str() != RELEASE
    {
        return Err(ProviderNativeSyntaxError::SnapshotMismatch(
            "Rust syntax job identity",
        ));
    }
    Ok(())
}
