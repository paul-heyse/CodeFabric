use super::*;
use arrow::array::Int64Array;
use arrow::record_batch::RecordBatch;
use arrow_schema::{Field, Schema};
use datafusion::prelude::{SessionContext, col};

#[tokio::test]
async fn native_walk_union_keeps_metadata_across_empty_and_named_seeds() {
    let context = SessionContext::new();
    let edges = context
        .read_batch(
            RecordBatch::try_new(
                Arc::new(Schema::new_with_metadata(
                    vec![
                        Field::new("source", DataType::Int64, false),
                        Field::new("target", DataType::Int64, false),
                    ],
                    std::collections::HashMap::from([("relation".into(), "edges".into())]),
                )),
                vec![
                    Arc::new(Int64Array::from(vec![0, 1, 1, 2])),
                    Arc::new(Int64Array::from(vec![1, 2, 3, 1])),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .into_unoptimized_plan();
    let seed = storage_seed(&context).await;
    assert_aggregate(&context, &seed).await;
    let subject = LogicalPlanBuilder::from(edges.clone())
        .join_on(
            seed,
            datafusion::logical_expr::JoinType::LeftSemi,
            [col("source").eq(col("seed"))],
        )
        .unwrap()
        .build()
        .unwrap();
    let empty = LogicalPlanBuilder::from(subject.clone())
        .filter(datafusion::prelude::lit(false))
        .unwrap()
        .build()
        .unwrap();
    let first = LogicalPlanBuilder::from(empty)
        .union_distinct(subject)
        .unwrap()
        .build()
        .unwrap();
    let mut steps = vec![first];
    for _ in 2..=3 {
        let frontier = LogicalPlanBuilder::from(steps.last().unwrap().clone())
            .project([col("target").alias("next")])
            .unwrap()
            .build()
            .unwrap();
        steps.push(
            LogicalPlanBuilder::from(edges.clone())
                .join_on(
                    frontier,
                    datafusion::logical_expr::JoinType::LeftSemi,
                    [col("source").eq(col("next"))],
                )
                .unwrap()
                .build()
                .unwrap(),
        );
    }
    let mut result = LogicalPlanBuilder::from(steps.remove(0));
    for step in steps {
        result = result.union_distinct(step).unwrap();
    }
    let batches = context
        .execute_logical_plan(result.build().unwrap())
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 4);
    let observed = batches
        .iter()
        .flat_map(|batch| {
            let source = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            let target = batch
                .column(1)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            (0..batch.num_rows())
                .map(|row| (source.value(row), target.value(row)))
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        observed,
        std::collections::BTreeSet::from([(0, 1), (1, 2), (1, 3), (2, 1)])
    );
}

async fn storage_seed(context: &SessionContext) -> LogicalPlan {
    let storage = Arc::new(Schema::new(vec![Field::new(
        "seed",
        DataType::Int64,
        false,
    )]));
    let logical = Arc::new(Schema::new_with_metadata(
        vec![
            storage
                .field(0)
                .clone()
                .with_metadata(std::collections::HashMap::from([(
                    crate::schema_contract::FIELD_ID_METADATA_KEY.into(),
                    "test.logical.seed".into(),
                )])),
        ],
        std::collections::HashMap::from([
            ("selector".into(), "named".into()),
            (
                crate::schema_contract::RELATION_ID_METADATA_KEY.into(),
                "test.walk-seed".into(),
            ),
        ]),
    ));
    let contract = Arc::new(
        SchemaContract::try_new(
            "test.walk-seed",
            TableReference::bare("seeds"),
            logical,
            Arc::clone(&storage),
            vec![crate::schema_contract::FieldIndexMapping::direct(0, 0)],
        )
        .unwrap(),
    );
    let batch = RecordBatch::try_new(
        Arc::clone(&storage),
        vec![Arc::new(Int64Array::from(vec![0]))],
    )
    .unwrap();
    let native =
        Arc::new(datafusion::datasource::MemTable::try_new(storage, vec![vec![batch]]).unwrap());
    context
        .register_table(
            "seeds",
            Arc::new(
                crate::fabric::provider::SchemaContractStorageProvider::try_new(contract, native)
                    .unwrap(),
            ),
        )
        .unwrap();
    context
        .table("seeds")
        .await
        .unwrap()
        .into_unoptimized_plan()
}

async fn assert_aggregate(context: &SessionContext, seed: &LogicalPlan) {
    let grouped = LogicalPlanBuilder::from(seed.clone())
        .aggregate(
            [col("seed")],
            [datafusion::functions_aggregate::expr_fn::count(col("seed"))],
        )
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        context
            .execute_logical_plan(grouped)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap()
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        1
    );
}
