//! Reusable returned Arrow rows, with shared-pool ownership retained through every consumer stream.

use std::sync::Arc;
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use datafusion::catalog::TableProvider;
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::execution::memory_pool::MemoryReservation;
use datafusion::logical_expr::LogicalPlanBuilder;
use datafusion::physical_plan::{SendableRecordBatchStream, stream::RecordBatchStreamAdapter};
use futures::StreamExt as _;

use crate::cancellation::Cancellation;
use crate::relational_program::{RelationInput, SupplementalProgramRelationBinding};

use super::streamed_result_package::{StreamedResultPackageError, next_batch};

#[derive(Debug)]
pub(crate) struct RetainedBlockResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    selected_rows: usize,
    _reservation: MemoryReservation,
}

impl RetainedBlockResult {
    pub(crate) async fn collect(
        schema: SchemaRef,
        mut stream: SendableRecordBatchStream,
        reservation: MemoryReservation,
        max_rows: u64,
        selected_rows: usize,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<Arc<Self>, StreamedResultPackageError> {
        let mut rows = 0_u64;
        let mut batches = Vec::new();
        while let Some(batch) = next_batch(&mut stream, cancellation, deadline).await? {
            let batch = batch.map_err(StreamedResultPackageError::DataFusion)?;
            if batch.schema_ref() != &schema {
                return Err(StreamedResultPackageError::StreamSchemaDrift(
                    "prior-result".into(),
                ));
            }
            rows = rows
                .checked_add(batch.num_rows() as u64)
                .ok_or(StreamedResultPackageError::CounterOverflow)?;
            if rows > max_rows {
                return Err(StreamedResultPackageError::RelationRowLimit {
                    relation: "prior-result".into(),
                    observed: rows,
                    limit: max_rows,
                });
            }
            // Charge received native storage before retaining it. Slices and consumers share
            // this reservation; this is conservative residency accounting, not unique RSS.
            reservation
                .try_grow(
                    batch
                        .get_array_memory_size()
                        .saturating_add(std::mem::size_of::<RecordBatch>()),
                )
                .map_err(StreamedResultPackageError::DataFusion)?;
            batches.push(batch);
        }
        Ok(Arc::new(Self {
            schema,
            batches,
            selected_rows,
            _reservation: reservation,
        }))
    }

    pub(crate) fn stream(self: &Arc<Self>) -> SendableRecordBatchStream {
        let stream = futures::stream::unfold((Arc::clone(self), 0), |(owner, index)| async move {
            let batch = owner.batches.get(index)?.clone();
            Some((Ok(batch), (owner, index + 1)))
        });
        Box::pin(RecordBatchStreamAdapter::new(
            Arc::clone(&self.schema),
            stream,
        ))
    }

    fn selected_batches(&self) -> Vec<RecordBatch> {
        let mut remaining = self.selected_rows;
        self.batches
            .iter()
            .filter_map(|batch| {
                let count = batch.num_rows().min(remaining);
                remaining -= count;
                (count > 0).then(|| batch.slice(0, count))
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QueryResultInput {
    pub(crate) binding: SupplementalProgramRelationBinding,
    pub(crate) input: RelationInput,
    pub(crate) provider: Arc<dyn TableProvider>,
    owners: Vec<Arc<RetainedBlockResult>>,
}

impl QueryResultInput {
    pub(crate) fn try_new(
        binding: SupplementalProgramRelationBinding,
        owners: Vec<Arc<RetainedBlockResult>>,
    ) -> Result<Self, StreamedResultPackageError> {
        let mut batches = Vec::new();
        for owner in &owners {
            if owner.schema.fields() != binding.schema().fields() {
                return Err(StreamedResultPackageError::StreamSchemaDrift(format!(
                    "prior-result field contract {}",
                    binding.relation_id().as_str(),
                )));
            }
            // DataFusion carries source-level schema metadata through projection. A transient
            // input has its own declared relation envelope; retain exact field annotations and
            // Arrow buffers, without relabeling this input as the original provider table.
            batches.extend(
                owner
                    .selected_batches()
                    .into_iter()
                    .map(|batch| {
                        RecordBatch::try_new(Arc::clone(binding.schema()), batch.columns().to_vec())
                            .map_err(StreamedResultPackageError::Arrow)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let provider: Arc<dyn TableProvider> = Arc::new(
            MemTable::try_new(Arc::clone(binding.schema()), vec![batches])
                .map_err(StreamedResultPackageError::DataFusion)?,
        );
        let plan = LogicalPlanBuilder::scan(
            binding.table_reference().clone(),
            provider_as_source(Arc::clone(&provider)),
            None,
        )
        .and_then(LogicalPlanBuilder::build)
        .map_err(StreamedResultPackageError::DataFusion)?;
        Ok(Self {
            input: RelationInput {
                relation_id: binding.relation_id().clone(),
                plan,
            },
            binding,
            provider,
            owners,
        })
    }

    pub(crate) fn retain_stream(
        inputs: &[Self],
        stream: SendableRecordBatchStream,
    ) -> SendableRecordBatchStream {
        let owners = inputs
            .iter()
            .flat_map(|input| input.owners.iter().cloned())
            .collect::<Vec<_>>();
        let schema = stream.schema();
        let stream = futures::stream::unfold((stream, owners), |(mut stream, owners)| async move {
            stream.next().await.map(|batch| (batch, (stream, owners)))
        });
        Box::pin(RecordBatchStreamAdapter::new(schema, stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_program::{FieldId, RelationId};
    use arrow_array::UInt64Array;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;
    use datafusion::execution::memory_pool::{GreedyMemoryPool, MemoryConsumer, MemoryPool};
    use datafusion::prelude::SessionContext;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn source(polls: Arc<AtomicUsize>) -> (SchemaRef, SendableRecordBatchStream) {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("id", DataType::UInt64, false).with_metadata(
                std::collections::HashMap::from([("meaning".into(), "canonical-id".into())]),
            )],
            std::collections::HashMap::from([(
                "source-relation".into(),
                "provider.original".into(),
            )]),
        ));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(UInt64Array::from(vec![1, 2, 3]))],
        )
        .unwrap();
        let stream = futures::stream::iter(vec![Ok(batch)]).inspect(move |_| {
            polls.fetch_add(1, Ordering::Relaxed);
        });
        (
            Arc::clone(&schema),
            Box::pin(RecordBatchStreamAdapter::new(schema, stream)),
        )
    }

    fn binding(schema: &SchemaRef) -> SupplementalProgramRelationBinding {
        let schema = Arc::new(Schema::new(schema.fields().clone()));
        SupplementalProgramRelationBinding::try_new(
            RelationId::new("query.prior").unwrap(),
            TableReference::bare("prior"),
            schema,
            vec![FieldId::new("query.prior.id").unwrap()],
            [1; 32],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn retained_result_is_polled_once_and_selected_rows_share_owned_memory() {
        let pool: Arc<dyn MemoryPool> = Arc::new(GreedyMemoryPool::new(4096));
        let polls = Arc::new(AtomicUsize::new(0));
        let (schema, stream) = source(Arc::clone(&polls));
        let owner = RetainedBlockResult::collect(
            Arc::clone(&schema),
            stream,
            MemoryConsumer::new("test.prior").register(&pool),
            3,
            2,
            &Cancellation::with_check_interval(1),
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
        let retained_bytes = pool.reserved();
        assert!(retained_bytes > 0);
        let mut output_stream = owner.stream();
        let left = QueryResultInput::try_new(binding(&schema), vec![Arc::clone(&owner)]).unwrap();
        let right = QueryResultInput::try_new(binding(&schema), vec![Arc::clone(&owner)]).unwrap();
        assert!(left.provider.schema().metadata().is_empty());
        assert_eq!(
            left.provider.schema().field(0).metadata()["meaning"],
            "canonical-id"
        );
        let incompatible = Arc::new(Schema::new(vec![Field::new("id", DataType::UInt64, false)]));
        assert!(
            QueryResultInput::try_new(binding(&incompatible), vec![Arc::clone(&owner)]).is_err(),
            "field semantics may not be discarded with relation-level metadata"
        );
        assert_eq!(
            pool.reserved(),
            retained_bytes,
            "fan-out shares the producer reservation"
        );
        drop(owner);
        let context = SessionContext::new();
        for input in [&left, &right] {
            let frame =
                datafusion::dataframe::DataFrame::new(context.state(), input.input.plan.clone());
            let batches = frame.collect().await.unwrap();
            assert_eq!(
                batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
                2,
                "probe rows do not enter consumers"
            );
        }
        assert_eq!(
            polls.load(Ordering::Relaxed),
            1,
            "consumers never rerun the source"
        );
        drop((left, right));
        assert_eq!(
            pool.reserved(),
            retained_bytes,
            "delivery still owns the producer"
        );
        assert_eq!(
            output_stream.next().await.unwrap().unwrap().num_rows(),
            3,
            "delivery retains the exhaustion probe"
        );
        drop(output_stream);
        assert_eq!(pool.reserved(), 0);
    }

    #[tokio::test]
    async fn retained_result_limits_cancel_and_deadline_release_memory() {
        for mode in ["rows", "memory", "cancel", "deadline"] {
            let pool: Arc<dyn MemoryPool> = Arc::new(GreedyMemoryPool::new(if mode == "memory" {
                1
            } else {
                4096
            }));
            let (schema, stream) = source(Arc::new(AtomicUsize::new(0)));
            let cancellation = Cancellation::with_check_interval(1);
            if mode == "cancel" {
                cancellation.cancel();
            }
            let deadline = Instant::now()
                + if mode == "deadline" {
                    Duration::ZERO
                } else {
                    Duration::from_secs(5)
                };
            let result = RetainedBlockResult::collect(
                schema,
                stream,
                MemoryConsumer::new("test.prior.failure").register(&pool),
                if mode == "rows" { 2 } else { 3 },
                2,
                &cancellation,
                deadline,
            )
            .await;
            assert!(result.is_err(), "{mode} must fail explicitly");
            assert_eq!(pool.reserved(), 0, "{mode} must release retained memory");
        }
    }
}
