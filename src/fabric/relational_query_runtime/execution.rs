//! One ready block uses the admitted child and native shared-pool retention.

use super::{
    Arc, BTreeSet, Cancellation, CompilationObservations, Instant, RelationalProgramCompiler,
    RelationalQueryRuntimeError, RequestOwnedRelationCollection, ResultProvenance,
    SelectedQueryOutput, StreamedRelationInput, StreamedResultPackageError, compilation_provenance,
    isolated_child_execution_error,
};
use crate::fabric::child_session::AuthorizedChildSession;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpoch;
use crate::fabric::query_result_input::{QueryResultInput, RetainedBlockResult};

pub(super) struct BlockExecution<'a> {
    pub(super) epoch: &'a ProgrammaticFabricEpoch,
    pub(super) child: &'a AuthorizedChildSession,
    pub(super) max_output_rows: u64,
    pub(super) cancellation: &'a Cancellation,
    pub(super) deadline: Instant,
    pub(super) isolate_errors: bool,
}

pub(super) struct ExecutedBlock {
    pub(super) relation: StreamedRelationInput,
    pub(super) compilation: CompilationObservations,
    pub(super) retained: Option<Arc<RetainedBlockResult>>,
}

impl BlockExecution<'_> {
    // Keep the schema/authority checks and their native execution adjacent.
    #[allow(
        clippy::too_many_lines,
        clippy::result_large_err,
        reason = "preserve the existing structured query error boundary beside its native execution"
    )]
    pub(super) async fn execute(
        &self,
        output: SelectedQueryOutput,
        request_inputs: Option<&RequestOwnedRelationCollection>,
        prior_inputs: Vec<QueryResultInput>,
        reusable: bool,
    ) -> Result<Option<ExecutedBlock>, RelationalQueryRuntimeError> {
        let coverage = output.coverage.ok_or_else(|| {
            RelationalQueryRuntimeError::CompletenessUndeclared(output.relation_id.as_str().into())
        })?;
        let query_bindings = if let Some(binding) = &output.program_result_binding {
            self.epoch
                .program_bindings()
                .with_supplemental_relations([binding.clone()])?
        } else {
            self.epoch.program_bindings().as_ref().clone()
        };
        let session_relation = RelationalProgramCompiler::resolve_output_relation_with_bindings(
            &query_bindings,
            &output.program,
        )?;
        if session_relation != output.relation_id {
            return Err(RelationalQueryRuntimeError::OutputRelationMismatch {
                advertised: output.relation_id.as_str().into(),
                session_bound: session_relation.as_str().into(),
            });
        }
        let streamed = match self
            .child
            .execute_relational_program_stream_with_query_local_bindings(
                &output.program,
                request_inputs,
                output.program_result_binding.as_ref(),
                &prior_inputs,
            )
            .await
        {
            Ok(streamed) => streamed,
            Err(error) if self.isolate_errors && isolated_child_execution_error(&error) => {
                tracing::warn!(relation = output.relation_id.as_str(), %error, "query block execution failed");
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let schema = Arc::clone(streamed.schema());
        let compilation = streamed.observations().clone();
        let mut provenance = compilation_provenance(&compilation);
        provenance.extend(
            output
                .prior_results
                .iter()
                .flat_map(|slot| &slot.producers)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|producer| ResultProvenance {
                    kind: "prior_result_block".into(),
                    identity: producer.as_str().into(),
                }),
        );
        let (stream, retained) = if reusable {
            let selected_rows = output
                .row_selection
                .as_ref()
                .map_or(self.max_output_rows, |selection| selection.maximum_rows);
            let result = match RetainedBlockResult::collect(
                schema.clone(),
                streamed.into_stream(),
                self.child.reserve_prior_result_memory(),
                self.max_output_rows,
                usize::try_from(selected_rows)
                    .map_err(|_| RelationalQueryRuntimeError::ResultCountOverflow)?,
                self.cancellation,
                self.deadline,
            )
            .await
            {
                Ok(result) => result,
                Err(StreamedResultPackageError::DataFusion(error))
                    if self.isolate_errors
                        && super::super::query_block_outcomes::isolated_execution_error(&error) =>
                {
                    tracing::warn!(relation = output.relation_id.as_str(), %error, "reusable query block execution failed");
                    return Ok(None);
                }
                Err(error) => return Err(error.into()),
            };
            (result.stream(), Some(result))
        } else {
            (streamed.into_stream(), None)
        };
        Ok(Some(ExecutedBlock {
            relation: StreamedRelationInput {
                relation_id: output.relation_id,
                schema,
                stream,
                max_rows: self.max_output_rows,
                coverage,
                provenance,
                row_selection: output.row_selection,
            },
            compilation,
            retained,
        }))
    }
}
