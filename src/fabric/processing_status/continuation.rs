//! Query-bound continuation over the original exact processing relation.

use super::*;
use crate::cancellation::StructuredCancellationScope;
use crate::fabric::command::EpochId;
use crate::fabric::delta_exact::ExactDeltaPin;
use crate::fabric::workspace_resources::ProductionWorkspaceResources;
use crate::resource_budget::{ChargedSlice, ResourceClass};
use datafusion::common::ScalarValue;
use datafusion::dataframe::DataFrame;
use datafusion::execution::SessionStateBuilder;
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::SessionContext;
use std::collections::BTreeMap;
use std::time::Instant;

const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

/// Retained privately with a result, never projected into a public manifest or response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessingSelection {
    table_root: String,
    table_version: u64,
    workspace: [u8; 16],
    family: String,
    languages: BTreeSet<String>,
    contexts: BTreeSet<[u8; 16]>,
}

impl ProcessingSelection {
    pub(crate) fn for_query(
        epoch: &ProgrammaticFabricEpoch,
        scope: &EntityQueryScope,
        summary: &EntityProcessingSummary,
        workspace: [u8; 16],
    ) -> Result<Option<Self>, String> {
        if summary.next_offset.is_none() {
            return Ok(None);
        }
        let pin = epoch
            .relation_publication()
            .table_version_map()
            .get(&ProgrammaticRelationId::new(ENTITY_PROCESSING_RELATION))
            .ok_or("processing relation has no exact retained selection")?;
        Ok(Some(Self {
            table_root: pin.canonical_root().to_string(),
            table_version: pin.version(),
            workspace,
            family: scope.family.to_owned(),
            languages: scope.languages.clone(),
            contexts: scope.contexts.clone(),
        }))
    }

    pub(super) fn validate(&self, summary: &EntityProcessingSummary) -> bool {
        !self.table_root.is_empty()
            && self.workspace != [0; 16]
            && self.languages.iter().eq(summary.languages.iter())
            && matches!(
                self.family.as_str(),
                "function-declarations"
                    | "call-targets"
                    | "lexical-references"
                    | "function-source-context"
            )
            && self.contexts.len() <= 4096
    }

    fn scope(&self) -> Result<EntityQueryScope, String> {
        let family = match self.family.as_str() {
            "function-declarations" => "function-declarations",
            "function-source-context" => "function-source-context",
            "call-targets" => "call-targets",
            "lexical-references" => "lexical-references",
            _ => return Err("invalid retained processing family".to_owned()),
        };
        Ok(EntityQueryScope {
            family,
            languages: self.languages.clone(),
            contexts: self.contexts.clone(),
        })
    }
}

pub(super) fn ordered(frame: DataFrame) -> Result<DataFrame, String> {
    let mut names = vec![
        "family",
        "language",
        "relative_path",
        "scope_kind",
        "target_name",
        "context_id",
        "processing_state",
        "reason",
    ];
    if frame
        .schema()
        .field_with_unqualified_name("target_kind")
        .is_ok()
    {
        names.push("target_kind");
    }
    frame
        .sort(
            names
                .into_iter()
                .map(|name| col(name).sort(true, true))
                .collect(),
        )
        .map_err(|error| error.to_string())
}

fn selected_remainder(
    frame: DataFrame,
    selection: &ProcessingSelection,
    generation: u64,
) -> Result<DataFrame, String> {
    let mut predicate = col("family")
        .eq(lit(selection.family.clone()))
        .and(col("language").in_list(
            selection.languages.iter().cloned().map(lit).collect(),
            false,
        ))
        .and(col("workspace_id").eq(lit(ScalarValue::FixedSizeBinary(
            16,
            Some(selection.workspace.to_vec()),
        ))))
        .and(col("source_generation").eq(lit(generation)))
        .and(col("processing_state").not_eq(lit("complete")));
    if !selection.contexts.is_empty() {
        predicate = predicate.and(
            col("context_id").is_null().or(col("context_id").in_list(
                selection
                    .contexts
                    .iter()
                    .map(|id| lit(ScalarValue::FixedSizeBinary(16, Some(id.to_vec()))))
                    .collect(),
                false,
            )),
        );
    }
    frame.filter(predicate).map_err(|error| error.to_string())
}

/// Uses the workspace's shared native resources and joined daemon lifecycle.
#[derive(Clone)]
pub(crate) struct ProcessingPageReader {
    resources: ProductionWorkspaceResources,
    scope: StructuredCancellationScope,
}

impl std::fmt::Debug for ProcessingPageReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingPageReader")
            .finish_non_exhaustive()
    }
}

impl ProcessingPageReader {
    pub(crate) fn new(
        resources: ProductionWorkspaceResources,
        scope: StructuredCancellationScope,
    ) -> Self {
        Self { resources, scope }
    }

    pub(crate) async fn read(
        &self,
        epoch: EpochId,
        summary: QueryProcessing,
        offset: usize,
        deadline: Instant,
    ) -> Result<ChargedSlice<u8>, String> {
        if !summary.validate()
            || offset == 0
            || !offset.is_multiple_of(REMAINDER_PAGE_SIZE)
            || offset as u64 >= summary.processing.remaining_partitions
        {
            return Err("invalid processing continuation offset".to_owned());
        }
        let selection = summary
            .selection
            .clone()
            .ok_or("processing continuation unavailable")?;
        let resources = self.resources.clone();
        self.resources
            .native_execution(&self.scope)
            .map_err(|error| error.to_string())?
            .run_read(
                "processing-remainder",
                ResourceClass::Data,
                deadline,
                move |cancellation, _| async move {
                    read_page(resources, selection, epoch, summary, offset, cancellation).await
                },
            )
            .await
            .map_err(|error| error.to_string())
    }
}

async fn read_page(
    resources: ProductionWorkspaceResources,
    selection: ProcessingSelection,
    epoch: EpochId,
    mut summary: QueryProcessing,
    offset: usize,
    cancellation: crate::cancellation::Cancellation,
) -> Result<ChargedSlice<u8>, String> {
    // Bounded row/string projection and JSON scratch share this operation's workspace budget.
    let _projection =
        crate::inventory::reserve_memory(resources.budget(), MAX_PAGE_BYTES as u64 * 8)
            .map_err(|error| error.to_string())?;
    let session = SessionContext::new_with_state(
        SessionStateBuilder::new()
            .with_default_features()
            .with_query_planner(deltalake::delta_datafusion::planner::DeltaPlanner::new())
            .with_runtime_env(resources.native().runtime_env())
            .build(),
    );
    let pin = ExactDeltaPin::new(
        &url::Url::parse(&selection.table_root).map_err(|e| e.to_string())?,
        selection.table_version,
    )
    .map_err(|e| e.to_string())?;
    let (_, mut providers) =
        crate::fabric::programmatic_relation_delta::reopen_programmatic_relation_snapshots(
            Arc::new(session.state()),
            epoch,
            BTreeMap::from([(ProgrammaticRelationId::new(ENTITY_PROCESSING_RELATION), pin)]),
        )
        .await
        .map_err(|e| e.to_string())?;
    let provider = providers.pop().ok_or("missing processing provider")?;
    let frame = session
        .read_table(provider.provider)
        .map_err(|e| e.to_string())?;
    let frame = selected_remainder(frame, &selection, summary.processing.source_generation)?;
    let mut stream = ordered(frame)?
        .limit(offset, Some(REMAINDER_PAGE_SIZE))
        .map_err(|e| e.to_string())?
        .execute_stream()
        .await
        .map_err(|e| e.to_string())?;
    let mut batches = Vec::new();
    while let Some(batch) = stream.next().await {
        if cancellation.is_cancelled() {
            return Err("processing continuation cancelled".to_owned());
        }
        let batch = batch.map_err(|e| e.to_string())?;
        validate(
            &batch,
            selection.workspace,
            summary.processing.source_generation,
        )?;
        let charge = crate::inventory::reserve_memory(
            resources.budget(),
            batch.get_array_memory_size() as u64 + 512,
        )
        .map_err(|e| e.to_string())?;
        batches.push(charge.into_charged_value(batch));
    }
    let snapshot = EntityProcessingSnapshot {
        batches,
        generation: summary.processing.source_generation,
        workspace: selection.workspace,
        epoch: crate::fabric::epoch_runtime::FabricEpochId::from_bytes(*epoch.as_bytes()),
    };
    let remainder = snapshot.summarize(&selection.scope()?, 0).remainder;
    let expected =
        (summary.processing.remaining_partitions - offset as u64).min(REMAINDER_PAGE_SIZE as u64);
    if remainder.len() as u64 != expected {
        return Err("retained processing count mismatch".to_owned());
    }
    let end = offset + remainder.len();
    summary.processing.remainder = remainder;
    summary.processing.next_offset =
        ((end as u64) < summary.processing.remaining_partitions).then_some(end);
    summary.selection = None;
    if !summary.validate_page(offset) {
        return Err("invalid processing page".to_owned());
    }
    let charge = crate::inventory::reserve_memory(resources.budget(), MAX_PAGE_BYTES as u64)
        .map_err(|e| e.to_string())?;
    let mut counter = crate::fabric::bounded_encoding::CappedWriter::counting(MAX_PAGE_BYTES);
    serde_json::to_writer(&mut counter, &summary).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&summary).map_err(|e| e.to_string())?;
    charge.into_charged_vec(bytes).map_err(|e| e.to_string())
}
