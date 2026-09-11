//! Complete, read-only Arrow inputs. Native IPC supplies the content encoding; native
//! DataFusion supplies schema validation, projection and scan execution.

use super::*;

#[derive(Debug)]
pub(super) struct ImmutableArrowTable {
    // Do not expose MemTable's public mutable partitions or its insert operation. An identity
    // derived during construction remains valid for every subsequent scan of these arrays.
    inner: MemTable,
}

impl ImmutableArrowTable {
    pub(super) fn try_new(
        schema: SchemaRef,
        partitions: Vec<Vec<RecordBatch>>,
    ) -> Result<Self, DataFusionError> {
        Ok(Self {
            inner: MemTable::try_new(schema, partitions)?,
        })
    }

    pub(super) fn content_identity(&self) -> Option<[u8; 32]> {
        match self.ipc_identity() {
            Ok(identity) => Some(identity),
            Err(error) => {
                // An IPC representation limitation must not remove an otherwise supported
                // Arrow input. Its ordinary candidate-owned Delta write remains authoritative.
                tracing::debug!(%error, "materialized Arrow input is ineligible for content reuse");
                None
            }
        }
    }

    fn ipc_identity(&self) -> Result<[u8; 32], ArrowError> {
        let mut digest = DigestWriter(crate::integrity::IntegrityHasher::new());
        digest.0.update(b"codefabric.materialized-arrow-ipc.v1\0");
        digest
            .0
            .update(&(self.inner.batches.len() as u64).to_be_bytes());
        for partition in &self.inner.batches {
            // The table is private, never inserted into and only exposes shared read scans.
            let batches = partition
                .try_read()
                .expect("immutable Arrow partition has no writer");
            digest.0.update(&(batches.len() as u64).to_be_bytes());
            let mut stream = arrow_ipc::writer::StreamWriter::try_new(
                &mut digest,
                self.inner.schema().as_ref(),
            )?;
            for batch in batches.iter() {
                stream.write(batch)?;
            }
            stream.finish()?;
        }
        Ok(digest.0.finalize())
    }
}

struct DigestWriter(crate::integrity::IntegrityHasher);

impl std::io::Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl TableProvider for ImmutableArrowTable {
    fn schema(&self) -> SchemaRef {
        self.inner.schema()
    }

    fn constraints(&self) -> Option<&Constraints> {
        self.inner.constraints()
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>, DataFusionError> {
        self.inner.supports_filters_pushdown(filters)
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
        self.inner.scan(state, projection, filters, limit).await
    }

    async fn scan_with_args<'a>(
        &self,
        state: &dyn Session,
        args: ScanArgs<'a>,
    ) -> Result<ScanResult, DataFusionError> {
        self.inner.scan_with_args(state, args).await
    }

    fn statistics(&self) -> Option<Statistics> {
        self.inner.statistics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{DictionaryArray, Int8Array, types::Int8Type};

    fn table(array: ArrayRef) -> ImmutableArrowTable {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            array.data_type().clone(),
            true,
        )]));
        let batch = RecordBatch::try_new(Arc::clone(&schema), vec![array]).unwrap();
        ImmutableArrowTable::try_new(schema, vec![vec![batch]]).unwrap()
    }

    #[test]
    fn content_identity_includes_nulls_dictionary_values_and_multiplicity() {
        let values = |values: Vec<Option<i64>>| table(Arc::new(Int64Array::from(values)));
        let first = values(vec![Some(1), None, Some(1)])
            .content_identity()
            .unwrap();
        assert_eq!(
            first,
            values(vec![Some(1), None, Some(1)])
                .content_identity()
                .unwrap()
        );
        for changed in [
            vec![Some(1), Some(0), Some(1)],
            vec![Some(1), None],
            vec![Some(2), None, Some(1)],
        ] {
            assert_ne!(Some(first), values(changed).content_identity());
        }
        let dictionary = |value| {
            table(Arc::new(
                DictionaryArray::<Int8Type>::try_new(
                    Int8Array::from(vec![Some(0), None, Some(0)]),
                    Arc::new(StringArray::from(vec![value])),
                )
                .unwrap(),
            ))
        };
        assert_eq!(
            dictionary("first").content_identity(),
            dictionary("first").content_identity()
        );
        assert_ne!(
            dictionary("first").content_identity(),
            dictionary("second").content_identity()
        );
    }

    #[tokio::test]
    async fn native_scans_keep_projection_and_rows_but_cannot_insert_or_mutate_partitions() {
        let provider: Arc<dyn TableProvider> =
            Arc::new(table(Arc::new(Int64Array::from(vec![1, 2, 2]))));
        assert!(provider.downcast_ref::<MemTable>().is_none());
        let context = SessionContext::new();
        context
            .register_table("immutable", Arc::clone(&provider))
            .unwrap();
        let rows = context
            .sql("SELECT value FROM immutable WHERE value = 2 LIMIT 1")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(rows.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
        assert_eq!(
            rows[0]
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(0),
            2
        );
        let state = context.state();
        let scan = provider
            .scan_with_args(&state, ScanArgs::default().with_projection(Some(&[0])))
            .await
            .unwrap()
            .into_inner();
        assert!(
            provider
                .insert_into(
                    &state,
                    scan,
                    datafusion::logical_expr::dml::InsertOp::Append
                )
                .await
                .is_err()
        );
        assert_eq!(
            context
                .table("immutable")
                .await
                .unwrap()
                .count()
                .await
                .unwrap(),
            3
        );
    }

    #[test]
    fn unsupported_ipc_dictionary_nesting_disables_reuse_without_rejecting_input() {
        let inner = DictionaryArray::<Int8Type>::try_new(
            Int8Array::from(vec![0]),
            Arc::new(StringArray::from(vec!["nested"])),
        )
        .unwrap();
        let outer = DictionaryArray::<Int8Type>::try_new(Int8Array::from(vec![0]), Arc::new(inner))
            .unwrap();
        let input = table(Arc::new(outer));
        assert!(input.content_identity().is_none());
        assert_eq!(input.inner.batches[0].try_read().unwrap()[0].num_rows(), 1);
    }
}
