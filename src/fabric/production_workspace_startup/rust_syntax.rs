//! Source-context Rust parsing and publication, independent of semantic compilation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use datafusion::catalog::MemTable;
use datafusion::common::TableReference;

use super::inputs::PreparedSourceInputs;
use super::{
    CompiledSemanticRelease, ProductionWorkspaceStartupError, digest16, digest32, lower_hex, step,
};
use crate::analysis_context::{AnalysisContext, AnalysisContextKind};
use crate::cancellation::Cancellation;
use crate::fabric::epoch_runtime::{FABRIC_CATALOG, FabricSchemaRole};
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::programmatic_schema::{ProgrammaticRelationId, ProviderInput};
use crate::identity::SOURCE_CONTEXT_ID;
use crate::provider_contracts::{
    AdmittedProviderResult, CancellationProbe, ContextIdentity, ProviderContextBinding,
    ProviderLane, ProviderRunBinding, ProviderRunIdentity, ProviderScopeIdentity,
    ProviderSourceBinding, SourceIdentity,
};
use crate::provider_native_rust_syntax::{RELEASE, RustSyntaxRelation};
use crate::provider_native_syntax::ProviderNativeSourceImage;
use crate::schema_contract::{FieldIndexMapping, SchemaContract};
use crate::semantic_release::ProviderJobInput;
use crate::source_image::SourceLanguage;
use crate::workspace_registry::WorkspaceRecord;

#[allow(
    clippy::too_many_lines,
    reason = "keep one source job's preparation, execution and admission together"
)]
pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inputs: &PreparedSourceInputs,
    record: &WorkspaceRecord,
    release: &CompiledSemanticRelease,
    cancellation: &Cancellation,
    resources: &crate::fabric::workspace_resources::ProductionWorkspaceResources,
) -> Result<Vec<AdmittedProviderResult>, ProductionWorkspaceStartupError> {
    if !inputs
        .capture()?
        .images()
        .iter()
        .any(|image| image.language == SourceLanguage::Rust)
    {
        return Ok(Vec::new());
    }
    let (context, context_fingerprint) = source_context(inputs, record)?;
    let mut runs = Vec::new();
    for image in inputs
        .capture()?
        .images()
        .iter()
        .filter(|image| image.language == SourceLanguage::Rust)
    {
        let source = ProviderNativeSourceImage::try_from(image)
            .map_err(|error| step("rust-syntax-source", error))?;
        let run_id = digest16(
            b"codefabric.rust-syntax-run.v2\0",
            &[
                &record.workspace_id,
                builder.identity().as_bytes(),
                &source.file_id,
                &source.content_digest,
                &source.source_generation.to_be_bytes(),
                &context_fingerprint,
            ],
        );
        let source_binding = ProviderSourceBinding::try_file(
            SourceIdentity::try_new(format!(
                "codefabric.source.{}.{}",
                lower_hex(&source.file_id),
                source.source_generation
            ))
            .map_err(|error| step("rust-syntax-source", error))?,
            record.workspace_id,
            source.file_id,
            source.source_generation,
            source.content_digest,
        )
        .map_err(|error| step("rust-syntax-source", error))?;
        let prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::TreeSitterRust,
                    source: source_binding,
                    context: context.clone(),
                    run: ProviderRunBinding::try_new(
                        ProviderRunIdentity::try_new(format!(
                            "codefabric.rust-syntax-run.{}",
                            lower_hex(&run_id)
                        ))
                        .map_err(|error| step("rust-syntax-run", error))?,
                        run_id,
                    )
                    .map_err(|error| step("rust-syntax-run", error))?,
                    scope: ProviderScopeIdentity::try_new(format!(
                        "codefabric.source-scope.{}",
                        lower_hex(&source.file_id)
                    ))
                    .map_err(|error| step("rust-syntax-scope", error))?,
                    requested_families: release
                        .providers()
                        .families(ProviderLane::TreeSitterRust)
                        .map_err(|error| step("rust-syntax-families", error))?
                        .into_iter()
                        .map(|family| (family, 1))
                        .collect(),
                    operational_ceilings: super::inprocess_operational_ceilings()
                        .map_err(|error| step("rust-syntax-ceilings", error))?,
                    deadline: Instant::now() + Duration::from_secs(30),
                    cancellation: CancellationProbe::from_cancellation(cancellation.clone(), 1024)
                        .map_err(|error| step("rust-syntax-cancellation", error))?,
                    resource_budget: inputs
                        .budget()
                        .operation(run_id, inputs.budget().policy())
                        .map_err(|error| step("rust-syntax-budget", error))?,
                },
            )
            .map_err(|error| step("rust-syntax-job", error))?;
        let result = resources
            .syntax_cache()
            .lock()
            .map_err(|error| step("rust-syntax-cache-owner", error))?
            .rust(prepared.job(), &source)
            .map_err(|error| step("rust-syntax-parse", error))?;
        runs.push(
            release
                .providers()
                .admit(prepared, result)
                .map_err(|error| step("rust-syntax-admission", error))?,
        );
    }
    for relation in RustSyntaxRelation::ALL {
        register(builder, relation, &runs)?;
    }
    Ok(runs)
}

fn source_context(
    inputs: &PreparedSourceInputs,
    record: &WorkspaceRecord,
) -> Result<
    (
        crate::resource_budget::ChargedValue<ProviderContextBinding>,
        [u8; 32],
    ),
    ProductionWorkspaceStartupError,
> {
    let source_context = AnalysisContext::new(
        &record.public_id(),
        AnalysisContextKind::Source,
        RELEASE,
        "rust-tree-sitter",
        None,
        true,
    )
    .map_err(|error| step("rust-syntax-context", error))?;
    let context_fingerprint = source_context
        .fingerprint_bytes()
        .map_err(|error| step("rust-syntax-context", error))?;
    let context = ProviderContextBinding::try_new(
        ContextIdentity::try_new("context:source")
            .map_err(|error| step("rust-syntax-context", error))?,
        SOURCE_CONTEXT_ID,
        context_fingerprint,
        digest32(
            b"codefabric.rust-syntax-environment.v1\0",
            &[&record.workspace_id, &context_fingerprint],
        ),
    )
    .map_err(|error| step("rust-syntax-context", error))?;
    let context = crate::inventory::reserve_memory(
        inputs.budget(),
        context
            .memory_bytes()
            .map_err(|error| step("rust-syntax-context-memory", error))?,
    )
    .map_err(|error| step("rust-syntax-context-memory", error))?
    .into_charged_value(context);
    Ok((context, context_fingerprint))
}

fn register(
    builder: &mut ProgrammaticFabricEpochBuilder,
    relation: RustSyntaxRelation,
    runs: &[AdmittedProviderResult],
) -> Result<(), ProductionWorkspaceStartupError> {
    let schema = relation.schema();
    let batches = runs
        .iter()
        .flat_map(|run| run.result().relations())
        .filter(|output| output.relation().as_str() == relation.name())
        .flat_map(|output| output.batches().iter().cloned())
        .collect::<Vec<_>>();
    if batches.is_empty() {
        return Err(step(
            "rust-syntax-registration",
            "admitted Rust syntax run omitted a requested relation",
        ));
    }
    let reference = TableReference::full(
        FABRIC_CATALOG,
        FabricSchemaRole::RawTreeSitter.as_str(),
        relation.name().replace('.', "_"),
    );
    let contract = SchemaContract::try_new(
        format!("codefabric.provider-schema.v2.3.{}", relation.name()),
        reference.clone(),
        Arc::clone(&schema),
        Arc::clone(&schema),
        (0..schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect(),
    )
    .map_err(|error| step("rust-syntax-schema", error))?;
    let provider = MemTable::try_new(schema, vec![batches])
        .map_err(|error| step("rust-syntax-table", error))?;
    builder
        .register_provider(ProviderInput::new(
            ProgrammaticRelationId::new(relation.name()),
            reference,
            Arc::new(contract),
            Arc::new(provider),
        ))
        .map_err(|error| step("rust-syntax-registration", error))
}
