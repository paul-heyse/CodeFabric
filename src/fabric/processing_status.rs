//! Immutable processing projection of the selected epoch, shared by query and status delivery.

use super::arrow_result_resource::{ResultCompleteness, ResultCoverage, ResultUnknownCause};
use super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::programmatic_schema::ProgrammaticRelationId;
use crate::resource_budget::{ChargedValue, ResourceBudget};
use arrow_array::{
    Array, BinaryArray, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array,
};
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

mod continuation;
pub(crate) use continuation::{ProcessingPageReader, ProcessingSelection};

pub(crate) const ENTITY_PROCESSING_RELATION: &str = "system.entity_processing_scope";
const MAX_PARTITIONS: usize = 4_000_000;
const REMAINDER_PAGE_SIZE: usize = 64;

/// Arrow buffers are retained once with the epoch. This is not independently mutable state.
pub(crate) struct EntityProcessingSnapshot {
    batches: Vec<ChargedValue<RecordBatch>>,
    generation: u64,
    workspace: [u8; 16],
    epoch: super::epoch_runtime::FabricEpochId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityProcessingSummary {
    pub source_generation: u64,
    pub requested_partitions: u64,
    pub completed_partitions: u64,
    pub remaining_partitions: u64,
    pub scope: String,
    pub family: String,
    pub remainder: Vec<ProcessingRemainder>,
    pub next_offset: Option<usize>,
    pub languages: Vec<String>,
}

/// Query-selected processing and output bounds retained with the immutable result package.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryProcessing {
    pub query_id: String,
    pub processing: EntityProcessingSummary,
    pub maximum_rows: Option<u64>,
    /// None means exhaustion was not observed; false is an observed complete result.
    pub additional_rows: Option<bool>,
    /// Private exact relation selection. Public projections must omit storage addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selection: Option<ProcessingSelection>,
}

impl QueryProcessing {
    pub(crate) fn public_copy(&self) -> Self {
        let mut value = self.clone();
        value.selection = None;
        value
    }

    pub(crate) fn validate(&self) -> bool {
        self.validate_page(0)
    }

    pub(crate) fn validate_page(&self, offset: usize) -> bool {
        let value = &self.processing;
        let end = offset.checked_add(value.remainder.len());
        !self.query_id.is_empty()
            && value.source_generation > 0
            && !value.scope.is_empty()
            && !value.family.is_empty()
            && value
                .completed_partitions
                .checked_add(value.remaining_partitions)
                == Some(value.requested_partitions)
            && value.remainder.len() <= REMAINDER_PAGE_SIZE
            && end.is_some_and(|n| n as u64 <= value.remaining_partitions)
            && offset.is_multiple_of(REMAINDER_PAGE_SIZE)
            && value.remainder.len() as u64
                == value
                    .remaining_partitions
                    .saturating_sub(offset as u64)
                    .min(REMAINDER_PAGE_SIZE as u64)
            && value.next_offset == end.filter(|n| (*n as u64) < value.remaining_partitions)
            && self
                .selection
                .as_ref()
                .is_none_or(|selection| selection.validate(value))
            && self.maximum_rows != Some(0)
            && value
                .languages
                .iter()
                .all(|language| matches!(language.as_str(), "python" | "rust"))
            && value.remainder.iter().all(|row| {
                value.languages.contains(&row.language)
                    && !row.scope_kind.is_empty()
                    && !row.reason.is_empty()
                    && row.path.as_deref() == std::str::from_utf8(&row.path_bytes).ok()
                    && matches!(
                        row.state.as_str(),
                        "pending"
                            | "running"
                            | "partial"
                            | "unknown"
                            | "unavailable"
                            | "excluded"
                            | "limited"
                            | "unsupported"
                            | "failed"
                            | "cancelled"
                    )
            })
    }
}

pub(crate) struct EntityQueryScope {
    family: &'static str,
    languages: BTreeSet<String>,
    contexts: BTreeSet<[u8; 16]>,
}

impl EntityQueryScope {
    pub(crate) fn from_request(
        request: &crate::semantic_query_contract::SemanticQueryRequest,
        selector: &str,
    ) -> Result<Self, String> {
        if !request.source_boundaries.is_empty()
            || request
                .representations
                .iter()
                .any(|value| value != "semantic")
            || request
                .external_entity_policy
                .as_deref()
                .is_some_and(|value| value != "exclude")
        {
            return Err("canonical entity source-boundary, representation or external-entity scope is not implemented".to_owned());
        }
        let mut languages = if request.languages.is_empty() {
            BTreeSet::from(["python".to_owned(), "rust".to_owned()])
        } else {
            request
                .languages
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect()
        };
        if languages
            .iter()
            .any(|language| language != "python" && language != "rust")
        {
            return Err("unsupported entity language".to_owned());
        }
        match selector {
            "function"
            | "declarations"
            | "calls"
            | "lexical-references"
            | "function-source-context" => {}
            selected
                if crate::production_query_recipe::CANONICAL_ENTITY_SELECTORS
                    .iter()
                    .any(|(_, value)| *value == selected) =>
            {
                let (selected_language, _) =
                    selected.split_once(':').expect("closed language selector");
                languages.retain(|language| language == selected_language);
            }
            _ => {
                return Err("entity selector is not a compiled declaration meaning".to_owned());
            }
        }
        let contexts = request
            .analysis_context_ids
            .iter()
            .map(|value| {
                crate::identity::decode_public_id(
                    crate::identity::IdentityDomain::AnalysisContext,
                    None,
                    value,
                )
                .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            family: match selector {
                "function-source-context" => "function-source-context",
                "calls" => "call-targets",
                "lexical-references" => "lexical-references",
                _ => "function-declarations",
            },
            languages,
            contexts,
        })
    }

    pub(crate) fn predicate(&self) -> Result<crate::relational_program::ScalarExpression, String> {
        self.predicate_for(
            "query.result.semantic-entities",
            "entity-language",
            "analysis-context-id",
        )
    }

    pub(crate) fn predicate_for(
        &self,
        relation: &str,
        language_field: &str,
        context_field: &str,
    ) -> Result<crate::relational_program::ScalarExpression, String> {
        use crate::relational_program::{FieldId, ScalarExpression as E, ScalarOperator as O};
        use datafusion::common::ScalarValue;
        let any = |name: &str, values: Vec<ScalarValue>| -> Result<E, String> {
            let field = FieldId::new(format!("{relation}.{name}")).map_err(|e| e.to_string())?;
            Ok(values
                .into_iter()
                .map(|value| E::Call {
                    operator: O::Equal,
                    arguments: vec![E::Field(field.clone()), E::Literal(value)],
                })
                .reduce(|left, right| E::Call {
                    operator: O::Or,
                    arguments: vec![left, right],
                })
                .unwrap_or(E::Literal(ScalarValue::Boolean(Some(false)))))
        };
        let language = any(
            language_field,
            self.languages
                .iter()
                .map(|value| ScalarValue::Utf8(Some(value.clone())))
                .collect(),
        )?;
        if self.contexts.is_empty() {
            return Ok(language);
        }
        Ok(E::Call {
            operator: O::And,
            arguments: vec![
                language,
                any(
                    context_field,
                    self.contexts
                        .iter()
                        .map(|value| ScalarValue::FixedSizeBinary(16, Some(value.to_vec())))
                        .collect(),
                )?,
            ],
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessingRemainder {
    pub language: String,
    pub scope_kind: String,
    pub path: Option<String>,
    pub path_bytes: Vec<u8>,
    pub target: Option<String>,
    #[serde(default)]
    pub target_kind: Option<String>,
    #[serde(default)]
    pub analysis_context_id: Option<String>,
    pub state: String,
    pub reason: String,
}

impl EntityProcessingSnapshot {
    pub(crate) async fn load(
        epoch: &ProgrammaticFabricEpoch,
        workspace: [u8; 16],
        generation: u64,
        budget: &ResourceBudget,
    ) -> Result<Option<Arc<Self>>, String> {
        let id = ProgrammaticRelationId::new(ENTITY_PROCESSING_RELATION);
        let Some(binding) = epoch.relation(&id) else {
            return Ok(None);
        };
        let frame = epoch
            .context()
            .table(binding.table_reference.clone())
            .await
            .map_err(|e| e.to_string())?;
        let mut stream = continuation::ordered(frame)?
            .limit(0, Some(MAX_PARTITIONS + 1))
            .map_err(|e| e.to_string())?
            .execute_stream()
            .await
            .map_err(|e| e.to_string())?;
        let mut batches = Vec::new();
        let mut rows = 0;
        while let Some(batch) = stream.next().await {
            let batch = batch.map_err(|e| e.to_string())?;
            rows += batch.num_rows();
            if rows > MAX_PARTITIONS {
                return Err("processing partition limit exceeded".to_owned());
            }
            validate(&batch, workspace, generation)?;
            let bytes = u64::try_from(batch.get_array_memory_size()).map_err(|e| e.to_string())?;
            let retained = crate::inventory::reserve_memory(budget, bytes.saturating_add(512))
                .map_err(|e| e.to_string())?;
            batches.push(retained.into_charged_value(batch));
        }
        Ok(Some(Arc::new(Self {
            batches,
            generation,
            workspace,
            epoch: *epoch.identity(),
        })))
    }

    pub(crate) fn matches(
        &self,
        workspace: [u8; 16],
        epoch: super::epoch_runtime::FabricEpochId,
        generation: u64,
    ) -> bool {
        self.workspace == workspace && self.epoch == epoch && self.generation == generation
    }

    /// Current query profile counts source-file and selected-Cargo-target work partitions.
    /// Pagination limits explanation size; it never changes the reported remainder count.
    pub(crate) fn summarize(
        &self,
        scope: &EntityQueryScope,
        offset: usize,
    ) -> EntityProcessingSummary {
        let mut summary = EntityProcessingSummary {
            source_generation: self.generation,
            requested_partitions: 0,
            completed_partitions: 0,
            remaining_partitions: 0,
            scope: "requested_python_sources_and_selected_cargo_targets".to_owned(),
            family: scope.family.to_owned(),
            remainder: Vec::new(),
            next_offset: None,
            languages: scope.languages.iter().cloned().collect(),
        };
        for batch in &self.batches {
            let families = strings(batch, "family").expect("validated processing schema");
            let languages = strings(batch, "language").expect("validated processing schema");
            let states = strings(batch, "processing_state").expect("validated processing schema");
            let reasons = strings(batch, "reason").expect("validated processing schema");
            let kinds = strings(batch, "scope_kind").expect("validated processing schema");
            let targets = strings(batch, "target_name").expect("validated processing schema");
            let target_kinds = batch
                .column_by_name("target_kind")
                .and_then(|array| array.as_any().downcast_ref::<StringArray>());
            let paths = binary(batch, "relative_path").expect("validated processing schema");
            let contexts = batch
                .column_by_name("context_id")
                .and_then(|a| a.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .expect("validated context ID");
            for row in 0..batch.num_rows() {
                if families.value(row) != scope.family
                    || !scope.languages.contains(languages.value(row))
                {
                    continue;
                }
                if !scope.contexts.is_empty()
                    && !contexts.is_null(row)
                    && !scope
                        .contexts
                        .iter()
                        .any(|value| contexts.value(row) == value)
                {
                    continue;
                }
                summary.requested_partitions += 1;
                if states.value(row) == "complete" {
                    summary.completed_partitions += 1;
                    continue;
                }
                let index = usize::try_from(summary.remaining_partitions)
                    .expect("bounded processing partition count");
                summary.remaining_partitions += 1;
                if index < offset || summary.remainder.len() == REMAINDER_PAGE_SIZE {
                    continue;
                }
                summary.remainder.push(ProcessingRemainder {
                    language: languages.value(row).to_owned(),
                    scope_kind: kinds.value(row).to_owned(),
                    path: String::from_utf8(paths.value(row).to_vec()).ok(),
                    path_bytes: paths.value(row).to_vec(),
                    target: (!targets.is_null(row)).then(|| targets.value(row).to_owned()),
                    target_kind: target_kinds
                        .filter(|array| !array.is_null(row))
                        .map(|array| array.value(row).to_owned()),
                    analysis_context_id: (!contexts.is_null(row)).then(|| {
                        crate::identity::encode_public_id(
                            crate::identity::IdentityDomain::AnalysisContext,
                            None,
                            contexts
                                .value(row)
                                .try_into()
                                .expect("validated context width"),
                        )
                        .expect("context identities have no kind slug")
                    }),
                    state: states.value(row).to_owned(),
                    reason: reasons.value(row).to_owned(),
                });
            }
        }
        let next = offset.saturating_add(summary.remainder.len());
        summary.next_offset = (next
            < usize::try_from(summary.remaining_partitions)
                .expect("bounded processing partition count"))
        .then_some(next);
        summary
    }
}

impl EntityProcessingSummary {
    pub(crate) fn coverage(&self) -> Result<ResultCoverage, String> {
        if self.remaining_partitions == 0 {
            return Ok(ResultCoverage::complete(self.requested_partitions));
        }
        // These are processing partitions. Result limits are described separately in the response.
        let remainder = self.remaining_partitions;
        ResultCoverage::try_new(
            if self.completed_partitions == 0 {
                ResultCompleteness::Unknown
            } else {
                ResultCompleteness::Partial
            },
            self.completed_partitions + remainder,
            self.completed_partitions,
            remainder,
            Some(
                ResultUnknownCause::try_new("PROCESSING_SCOPE_INCOMPLETE")
                    .map_err(|e| e.to_string())?,
            ),
        )
        .map_err(|e| e.to_string())
    }
}

fn strings<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray, String> {
    batch
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref())
        .ok_or_else(|| format!("invalid processing column {name}"))
}
fn binary<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a BinaryArray, String> {
    batch
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref())
        .ok_or_else(|| format!("invalid processing column {name}"))
}
fn validate(batch: &RecordBatch, workspace: [u8; 16], generation: u64) -> Result<(), String> {
    let workspace_ids = batch
        .column_by_name("workspace_id")
        .and_then(|a| a.as_any().downcast_ref::<FixedSizeBinaryArray>())
        .ok_or("invalid processing workspace")?;
    let generations = batch
        .column_by_name("source_generation")
        .and_then(|a| a.as_any().downcast_ref::<UInt64Array>())
        .ok_or("invalid processing generation")?;
    let contexts = batch
        .column_by_name("context_id")
        .and_then(|a| a.as_any().downcast_ref::<FixedSizeBinaryArray>())
        .ok_or("invalid processing context")?;
    if workspace_ids.value_length() != 16 || contexts.value_length() != 16 {
        return Err("invalid processing identity width".to_owned());
    }
    let family = strings(batch, "family")?;
    let language = strings(batch, "language")?;
    let state = strings(batch, "processing_state")?;
    let reason = strings(batch, "reason")?;
    let _ = strings(batch, "target_name")?;
    if batch.column_by_name("target_kind").is_some() {
        let _ = strings(batch, "target_kind")?;
    }
    let kinds = strings(batch, "scope_kind")?;
    let paths = binary(batch, "relative_path")?;
    for required in [family as &dyn Array, language, state, reason, kinds, paths] {
        if required.null_count() != 0 {
            return Err("null required processing value".to_owned());
        }
    }
    for row in 0..batch.num_rows() {
        if workspace_ids.is_null(row)
            || workspace_ids.value(row) != workspace
            || generations.is_null(row)
            || generations.value(row) != generation
            || !matches!(
                family.value(row),
                "function-declarations"
                    | "call-targets"
                    | "lexical-references"
                    | "function-source-context"
            )
            || !matches!(language.value(row), "python" | "rust")
            || !matches!(
                state.value(row),
                "complete"
                    | "pending"
                    | "running"
                    | "partial"
                    | "unknown"
                    | "unavailable"
                    | "excluded"
                    | "limited"
                    | "unsupported"
                    | "failed"
                    | "cancelled"
            )
            || ((state.value(row) == "complete") != reason.value(row).is_empty())
        {
            return Err(
                "processing row differs from the selected epoch or coverage contract".to_owned(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::ArrayRef;
    use arrow_schema::{Field, Schema};

    fn fixture() -> EntityProcessingSnapshot {
        let rows = 131;
        let columns: Vec<(&str, ArrayRef)> = vec![
            (
                "workspace_id",
                super::super::id16_array((0..rows).map(|_| Some(&[1; 16]))),
            ),
            (
                "source_generation",
                Arc::new(UInt64Array::from(vec![3; rows])),
            ),
            (
                "context_id",
                super::super::id16_array(
                    (0..rows).map(|row| if row == 0 { Some(&[2; 16]) } else { None }),
                ),
            ),
            (
                "family",
                Arc::new(StringArray::from(vec!["function-declarations"; rows])),
            ),
            (
                "language",
                Arc::new(StringArray::from_iter_values(
                    (0..rows).map(|row| if row == 0 { "python" } else { "rust" }),
                )),
            ),
            (
                "processing_state",
                Arc::new(StringArray::from_iter_values(
                    (0..rows).map(|row| if row == 0 { "complete" } else { "unavailable" }),
                )),
            ),
            (
                "reason",
                Arc::new(StringArray::from_iter_values((0..rows).map(|row| {
                    if row == 0 {
                        ""
                    } else {
                        "compiler_target_unavailable"
                    }
                }))),
            ),
            (
                "scope_kind",
                Arc::new(StringArray::from(vec!["cargo_target"; rows])),
            ),
            (
                "target_name",
                Arc::new(StringArray::from_iter_values(
                    (0..rows).map(|row| format!("target_{row}")),
                )),
            ),
            (
                "relative_path",
                Arc::new(BinaryArray::from_iter_values(
                    (0..rows).map(|_| &b"invalid-\xff/Cargo.toml"[..]),
                )),
            ),
        ];
        let schema = Arc::new(Schema::new(
            columns
                .iter()
                .map(|(name, array)| Field::new(*name, array.data_type().clone(), true))
                .collect::<Vec<_>>(),
        ));
        let batch =
            RecordBatch::try_new(schema, columns.into_iter().map(|(_, a)| a).collect()).unwrap();
        validate(&batch, [1; 16], 3).unwrap();
        EntityProcessingSnapshot {
            batches: vec![ChargedValue::for_test(batch)],
            generation: 3,
            workspace: [1; 16],
            epoch: super::super::epoch_runtime::FabricEpochId::from_bytes([9; 16]),
        }
    }

    #[test]
    fn processing_scope_keeps_python_complete_and_paginates_rust_remainder() {
        let processing = fixture();
        let python = EntityQueryScope {
            family: "function-declarations",
            languages: BTreeSet::from(["python".to_owned()]),
            contexts: BTreeSet::new(),
        };
        let summary = processing.summarize(&python, 0);
        assert_eq!(
            (
                summary.requested_partitions,
                summary.completed_partitions,
                summary.remaining_partitions
            ),
            (1, 1, 0)
        );
        assert_eq!(
            summary.coverage().unwrap().state(),
            ResultCompleteness::Complete
        );
        let rust = EntityQueryScope {
            family: "function-declarations",
            languages: BTreeSet::from(["rust".to_owned()]),
            contexts: BTreeSet::new(),
        };
        let first = processing.summarize(&rust, 0);
        let second = processing.summarize(&rust, first.next_offset.unwrap());
        let last = processing.summarize(&rust, second.next_offset.unwrap());
        assert_eq!(first.remaining_partitions, 130);
        assert_eq!(
            (
                first.remainder.len(),
                second.remainder.len(),
                last.remainder.len()
            ),
            (64, 64, 2)
        );
        assert_eq!(last.next_offset, None);
        assert_eq!(first.remainder[0].target.as_deref(), Some("target_1"));
        assert_eq!(second.remainder[0].target.as_deref(), Some("target_65"));
        assert!(first.remainder[0].path.is_none());
        assert_eq!(first.remainder[0].path_bytes, b"invalid-\xff/Cargo.toml");
        assert_eq!(
            first.coverage().unwrap().state(),
            ResultCompleteness::Unknown
        );
    }

    #[test]
    fn pending_and_running_scope_reopen_without_becoming_complete() {
        let original = fixture().batches[0].slice(0, 1);
        for state in ["pending", "running"] {
            let mut columns = original.columns().to_vec();
            columns[original.schema().index_of("processing_state").unwrap()] =
                Arc::new(StringArray::from(vec![state]));
            columns[original.schema().index_of("reason").unwrap()] =
                Arc::new(StringArray::from(vec!["semantic_work_pending"]));
            let batch = RecordBatch::try_new(original.schema(), columns).unwrap();
            validate(&batch, [1; 16], 3).unwrap();
            let processing = EntityProcessingSnapshot {
                batches: vec![ChargedValue::for_test(batch)],
                generation: 3,
                workspace: [1; 16],
                epoch: fixture().epoch,
            };
            let summary = processing.summarize(
                &EntityQueryScope {
                    family: "function-declarations",
                    languages: BTreeSet::from(["python".to_owned()]),
                    contexts: BTreeSet::new(),
                },
                0,
            );
            assert_eq!(summary.completed_partitions, 0);
            assert_eq!(summary.remaining_partitions, 1);
            assert_eq!(summary.remainder[0].state, state);
        }
    }

    #[test]
    fn processing_scope_counts_only_the_requested_family() {
        let mut processing = fixture();
        let original = processing.batches[0].slice(0, 1);
        let mut columns = original.columns().to_vec();
        let family = original.schema().index_of("family").unwrap();
        let state = original.schema().index_of("processing_state").unwrap();
        let reason = original.schema().index_of("reason").unwrap();
        columns[family] = Arc::new(StringArray::from(vec!["call-targets"]));
        columns[state] = Arc::new(StringArray::from(vec!["partial"]));
        columns[reason] = Arc::new(StringArray::from(vec!["unresolved_targets"]));
        let calls = RecordBatch::try_new(original.schema(), columns).unwrap();
        validate(&calls, [1; 16], 3).unwrap();
        processing.batches.push(ChargedValue::for_test(calls));
        let mut scope = EntityQueryScope {
            family: "function-declarations",
            languages: BTreeSet::from(["python".to_owned()]),
            contexts: BTreeSet::new(),
        };
        let declarations = processing.summarize(&scope, 0);
        assert_eq!(
            (
                declarations.requested_partitions,
                declarations.completed_partitions
            ),
            (1, 1)
        );
        scope.family = "call-targets";
        let calls = processing.summarize(&scope, 0);
        assert_eq!(
            (
                calls.requested_partitions,
                calls.completed_partitions,
                calls.remaining_partitions
            ),
            (1, 0, 1)
        );
        assert_eq!(calls.remainder[0].reason, "unresolved_targets");
    }

    #[test]
    fn processing_scope_rejects_another_epoch_and_preserves_unknown_contexts() {
        let processing = fixture();
        assert!(!processing.matches([2; 16], processing.epoch, 3));
        assert!(!processing.matches([1; 16], processing.epoch, 4));
        assert!(validate(&processing.batches[0], [1; 16], 4).is_err());
        let scope = EntityQueryScope {
            family: "function-declarations",
            languages: BTreeSet::from(["python".to_owned(), "rust".to_owned()]),
            contexts: BTreeSet::from([[8; 16]]),
        };
        let summary = processing.summarize(&scope, 0);
        assert_eq!(
            (summary.completed_partitions, summary.remaining_partitions),
            (0, 130)
        );
        let empty = EntityQueryScope {
            family: "function-declarations",
            languages: BTreeSet::new(),
            contexts: BTreeSet::new(),
        };
        assert_eq!(
            processing.summarize(&empty, 0).coverage().unwrap(),
            ResultCoverage::complete(0)
        );
    }
}
