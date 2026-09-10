//! A prepared query plan starts native execution only when its owned consumer polls it.

use std::sync::Arc;

use datafusion::execution::TaskContext;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{ExecutionPlan, SendableRecordBatchStream, execute_stream};
use futures::TryStreamExt as _;

pub(super) fn program_stream(
    plan: Arc<dyn ExecutionPlan>,
    context: Arc<TaskContext>,
) -> SendableRecordBatchStream {
    let schema = plan.schema();
    let start = futures::stream::once(async move { execute_stream(plan, context) });
    // Bind before polling: native tasks created by execute_stream inherit this query's owner.
    // Dropping an unpolled leaf only drops its prepared plan and never starts native tasks.
    crate::fabric::native_operations::bind_stream(Box::pin(RecordBatchStreamAdapter::new(
        schema,
        start.try_flatten(),
    )))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::{DataFusionError, Result, tree_node::TreeNodeRecursion};
    use datafusion::physical_plan::empty::EmptyExec;
    use datafusion::physical_plan::{DisplayAs, DisplayFormatType, PhysicalExpr, PlanProperties};
    use futures::StreamExt as _;

    use super::*;

    #[derive(Debug)]
    struct ObservedExecution {
        native: EmptyExec,
        executions: Arc<AtomicUsize>,
        fail: bool,
    }

    impl DisplayAs for ObservedExecution {
        fn fmt_as(
            &self,
            _: DisplayFormatType,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            f.write_str("ObservedExecution")
        }
    }

    impl ExecutionPlan for ObservedExecution {
        fn name(&self) -> &'static str {
            "ObservedExecution"
        }
        fn properties(&self) -> &Arc<PlanProperties> {
            self.native.properties()
        }
        fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
            vec![]
        }
        fn with_new_children(
            self: Arc<Self>,
            children: Vec<Arc<dyn ExecutionPlan>>,
        ) -> Result<Arc<dyn ExecutionPlan>> {
            assert!(children.is_empty());
            Ok(self)
        }
        fn apply_expressions(
            &self,
            _: &mut dyn FnMut(&Arc<dyn PhysicalExpr>) -> Result<TreeNodeRecursion>,
        ) -> Result<TreeNodeRecursion> {
            Ok(TreeNodeRecursion::Continue)
        }
        fn execute(
            &self,
            partition: usize,
            context: Arc<TaskContext>,
        ) -> Result<SendableRecordBatchStream> {
            self.executions.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                return Err(DataFusionError::Execution("native input failure".into()));
            }
            self.native.execute(partition, context)
        }
    }

    #[tokio::test]
    async fn unpolled_leaf_does_not_start_native_execution_and_polled_errors_are_preserved() {
        for fail in [false, true] {
            let executions = Arc::new(AtomicUsize::new(0));
            let schema = Arc::new(Schema::new(vec![Field::new(
                "value",
                DataType::Int64,
                false,
            )]));
            let plan: Arc<dyn ExecutionPlan> = Arc::new(ObservedExecution {
                native: EmptyExec::new(schema.clone()),
                executions: executions.clone(),
                fail,
            });
            let stream = program_stream(plan.clone(), Arc::new(TaskContext::default()));
            assert_eq!(stream.schema(), schema);
            assert_eq!(executions.load(Ordering::Relaxed), 0);
            drop(stream);
            assert_eq!(executions.load(Ordering::Relaxed), 0);
            let mut stream = program_stream(plan, Arc::new(TaskContext::default()));
            let first = stream.next().await;
            assert_eq!(executions.load(Ordering::Relaxed), 1);
            if fail {
                assert!(matches!(first, Some(Err(DataFusionError::Execution(_)))));
            } else {
                assert!(first.is_none());
            }
            assert!(stream.next().await.is_none());
            assert_eq!(executions.load(Ordering::Relaxed), 1);
        }
    }
}
