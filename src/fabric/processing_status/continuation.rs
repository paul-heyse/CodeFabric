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
    #[serde(default, skip_serializing_if = "SourceBoundaries::is_empty")]
    boundaries: SourceBoundaries,
    table_root: String,
    table_version: u64,
    workspace: [u8; 16],
    family: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    families: BTreeSet<String>,
    languages: BTreeSet<String>,
    contexts: BTreeSet<[u8; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owners: Option<BTreeSet<[u8; 16]>>,
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
            boundaries: scope.boundaries.clone(),
            table_root: pin.canonical_root().to_string(),
            table_version: pin.version(),
            workspace,
            family: scope.family.to_owned(),
            families: scope
                .families
                .iter()
                .map(|family| (*family).to_owned())
                .collect(),
            languages: scope.languages.clone(),
            contexts: scope.contexts.clone(),
            owners: scope.owners.clone(),
        }))
    }

    pub(super) fn validate(&self, summary: &EntityProcessingSummary) -> bool {
        !self.table_root.is_empty()
            && self.boundaries.valid()
            && self.workspace != [0; 16]
            && self.languages.iter().eq(summary.languages.iter())
            && super::canonical_processing_family(&self.family).is_some()
            && self
                .families
                .iter()
                .all(|family| super::canonical_processing_family(family).is_some())
            && (self.families.is_empty() || self.families.contains(&self.family))
            && self.contexts.len() <= 4096
            && self.owners.as_ref().is_none_or(|owners| {
                !owners.is_empty()
                    && owners.len() <= 4096
                    && self.family == "call-targets"
                    && (self.families.is_empty() || self.families.len() == 1)
            })
    }

    fn scope(&self) -> Result<EntityQueryScope, String> {
        if !self.boundaries.valid() {
            return Err("invalid retained source boundaries".into());
        }
        let family = super::canonical_processing_family(&self.family)
            .ok_or("invalid retained processing family")?;
        Ok(EntityQueryScope {
            boundaries: self.boundaries.clone(),
            family,
            families: self
                .families
                .iter()
                .map(|family| {
                    super::canonical_processing_family(family)
                        .ok_or_else(|| "invalid retained dependency family".to_owned())
                })
                .collect::<Result<_, _>>()?,
            languages: self.languages.clone(),
            contexts: self.contexts.clone(),
            owners: self.owners.clone(),
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
    if frame
        .schema()
        .field_with_unqualified_name("target_platform")
        .is_ok()
    {
        names.push("target_platform");
    }
    if frame
        .schema()
        .field_with_unqualified_name("owner_entity_id")
        .is_ok()
    {
        names.push("owner_entity_id");
    }
    for name in ["build_profile", "build_features", "default_features"] {
        if frame.schema().field_with_unqualified_name(name).is_ok() {
            names.push(name);
        }
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
        .in_list(
            if selection.families.is_empty() {
                vec![lit(selection.family.clone())]
            } else {
                selection.families.iter().cloned().map(lit).collect()
            },
            false,
        )
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
    if !selection.boundaries.is_empty() {
        // Rust partitions cover compilation targets; no file dependency closure is available yet.
        predicate = predicate.and(
            col("language")
                .not_eq(lit("python"))
                .and(col("family").not_eq(lit("syntax-nodes")))
                .or(selection.boundaries.native_predicate(&col("relative_path"))),
        );
    }
    if let Some(owners) = &selection.owners {
        predicate = predicate.and(
            col("owner_entity_id").in_list(
                owners
                    .iter()
                    .map(|id| lit(ScalarValue::FixedSizeBinary(16, Some(id.to_vec()))))
                    .collect(),
                false,
            ),
        );
    } else if frame
        .schema()
        .field_with_unqualified_name("owner_entity_id")
        .is_ok()
    {
        predicate = predicate.and(col("owner_entity_id").is_null());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retained_owner_remainder_selects_owners_without_double_counting_contexts() {
        let context = SessionContext::new();
        let batch = RecordBatch::try_from_iter([
            (
                "family",
                Arc::new(StringArray::from(vec!["call-targets"; 4])) as arrow_array::ArrayRef,
            ),
            ("language", Arc::new(StringArray::from(vec!["rust"; 4]))),
            (
                "workspace_id",
                crate::fabric::id16_array([Some(&[1; 16]); 4]),
            ),
            ("source_generation", Arc::new(UInt64Array::from(vec![3; 4]))),
            (
                "context_id",
                crate::fabric::id16_array([
                    Some(&[2; 16]),
                    Some(&[2; 16]),
                    Some(&[2; 16]),
                    Some(&[3; 16]),
                ]),
            ),
            (
                "processing_state",
                Arc::new(StringArray::from(vec![
                    "partial", "complete", "partial", "partial",
                ])),
            ),
            (
                "owner_entity_id",
                crate::fabric::id16_array([
                    None,
                    Some(&[10; 16]),
                    Some(&[11; 16]),
                    Some(&[12; 16]),
                ]),
            ),
        ])
        .unwrap();
        let frame = context.read_batch(batch).unwrap();
        let mut selection = ProcessingSelection {
            boundaries: SourceBoundaries::default(),
            table_root: "file:///owned/processing".into(),
            table_version: 0,
            workspace: [1; 16],
            family: "call-targets".into(),
            families: BTreeSet::new(),
            languages: BTreeSet::from(["rust".into()]),
            contexts: BTreeSet::new(),
            owners: None,
        };
        for (owners, contexts, expected) in [
            (None, BTreeSet::new(), vec![None]),
            (
                Some(BTreeSet::from([[10; 16], [11; 16]])),
                BTreeSet::new(),
                vec![Some([11; 16])],
            ),
            (
                Some(BTreeSet::from([[12; 16]])),
                BTreeSet::from([[2; 16]]),
                vec![],
            ),
            (
                Some(BTreeSet::from([[12; 16]])),
                BTreeSet::new(),
                vec![Some([12; 16])],
            ),
        ] {
            selection.owners = owners;
            selection.contexts = contexts;
            let batches = selected_remainder(frame.clone(), &selection, 3)
                .unwrap()
                .collect()
                .await
                .unwrap();
            let mut actual = Vec::new();
            for batch in batches {
                let ids = batch
                    .column_by_name("owner_entity_id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .unwrap();
                for row in 0..batch.num_rows() {
                    actual.push(
                        (!ids.is_null(row)).then(|| <[u8; 16]>::try_from(ids.value(row)).unwrap()),
                    );
                }
            }
            assert_eq!(actual, expected);
        }
        let mut serialized = serde_json::to_value(&selection).unwrap();
        serialized.as_object_mut().unwrap().remove("owners");
        assert!(
            serde_json::from_value::<ProcessingSelection>(serialized)
                .unwrap()
                .owners
                .is_none()
        );
    }

    #[tokio::test]
    async fn retained_family_set_filters_native_rows_and_reads_legacy_selections() {
        let snapshot = super::super::tests::fixture();
        let original = &snapshot.batches[0];
        let mut batches = vec![(**original).clone()];
        for family in ["call-targets", "types"] {
            let mut columns = original.columns().to_vec();
            columns[original.schema().index_of("family").unwrap()] =
                Arc::new(StringArray::from(vec![family; original.num_rows()]));
            batches.push(RecordBatch::try_new(original.schema(), columns).unwrap());
        }
        let context = SessionContext::new();
        let provider = Arc::new(
            datafusion::datasource::MemTable::try_new(original.schema(), vec![batches]).unwrap(),
        );
        let frame = context.read_table(provider).unwrap();
        let legacy = serde_json::json!({"table_root":"/private/exact/processing", "table_version":7,
            "workspace":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1], "family":"function-declarations", "languages":["rust"], "contexts":[]});
        for (families, expected) in [
            (None, 130),
            (Some(vec!["function-declarations", "call-targets"]), 260),
        ] {
            let mut value = legacy.clone();
            if let Some(families) = families {
                value["families"] = serde_json::json!(families);
            }
            let selected: ProcessingSelection = serde_json::from_value(value).unwrap();
            let restored: ProcessingSelection =
                serde_json::from_slice(&serde_json::to_vec(&selected).unwrap()).unwrap();
            assert_eq!(selected, restored);
            let rows = selected_remainder(frame.clone(), &restored, 3)
                .unwrap()
                .collect()
                .await
                .unwrap();
            assert_eq!(
                rows.iter().map(RecordBatch::num_rows).sum::<usize>(),
                expected
            );
            let scope = restored.scope().unwrap();
            assert!(scope.selects_family("function-declarations"));
            assert_eq!(scope.selects_family("call-targets"), expected == 260);
            assert!(!scope.selects_family("types"));
            for batch in rows {
                assert!(
                    strings(&batch, "family")
                        .unwrap()
                        .iter()
                        .flatten()
                        .all(|family| scope.selects_family(family))
                );
            }
        }
        let mut invalid = legacy;
        invalid["families"] = serde_json::json!(["unknown-family"]);
        let invalid: ProcessingSelection = serde_json::from_value(invalid).unwrap();
        assert!(invalid.scope().is_err());
    }
    #[tokio::test]
    async fn retained_source_boundaries_match_initial_counts_and_pages() {
        for family in ["function-declarations", "syntax-nodes"] {
            let rust_partitions = if family == "syntax-nodes" { 0 } else { 2 };
            let mut snapshot = super::super::tests::fixture();
            let original = &snapshot.batches[0];
            let mut columns = original.columns().to_vec();
            let count = original.num_rows();
            let replace = |columns: &mut Vec<arrow_array::ArrayRef>, name: &str, array| {
                columns[original.schema().index_of(name).unwrap()] = array;
            };
            replace(
                &mut columns,
                "family",
                Arc::new(StringArray::from(vec![family; count])),
            );
            replace(
                &mut columns,
                "language",
                Arc::new(StringArray::from_iter_values(
                    (0..count).map(|row| if row < 129 { "python" } else { "rust" }),
                )),
            );
            replace(
                &mut columns,
                "relative_path",
                Arc::new(BinaryArray::from_iter_values((0..count).map(|row| {
                    if row < 100 {
                        b"src/in.py".as_slice()
                    } else {
                        b"src_other/out.py".as_slice()
                    }
                }))),
            );
            let batch = RecordBatch::try_new(original.schema(), columns).unwrap();
            snapshot.batches = ordered(SessionContext::new().read_batch(batch.clone()).unwrap())
                .unwrap()
                .collect()
                .await
                .unwrap()
                .into_iter()
                .map(ChargedValue::for_test)
                .collect();
            let selection: ProcessingSelection = serde_json::from_value(serde_json::json!({
                "table_root":"/private/exact/processing", "table_version":7,
                "workspace":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1], "family":family,
                "languages":["python","rust"], "contexts":[], "boundaries":["src"]
            }))
            .unwrap();
            let selection: ProcessingSelection =
                serde_json::from_slice(&serde_json::to_vec(&selection).unwrap()).unwrap();
            let scope = selection.scope().unwrap();
            let first = snapshot.summarize(&scope, 0);
            assert_eq!(
                (
                    first.requested_partitions,
                    first.completed_partitions,
                    first.remaining_partitions
                ),
                (100 + rust_partitions, 1, 99 + rust_partitions)
            );
            assert_eq!(first.next_offset, Some(64));
            assert!(selection.validate(&first));
            let later = snapshot.summarize(&scope, 64);
            assert_eq!(later.remainder.len() as u64, 35 + rust_partitions);
            assert_eq!(
                later
                    .remainder
                    .iter()
                    .filter(|row| row.language == "rust")
                    .count() as u64,
                rust_partitions
            );
            let frame = SessionContext::new().read_batch(batch).unwrap();
            let selected = ordered(selected_remainder(frame, &selection, 3).unwrap())
                .unwrap()
                .limit(64, Some(64))
                .unwrap()
                .collect()
                .await
                .unwrap();
            let selected_snapshot = EntityProcessingSnapshot {
                batches: selected.into_iter().map(ChargedValue::for_test).collect(),
                generation: snapshot.generation,
                workspace: snapshot.workspace,
                epoch: snapshot.epoch,
            };
            assert_eq!(
                selected_snapshot.summarize(&scope, 0).remainder,
                later.remainder
            );
        }
    }
}
