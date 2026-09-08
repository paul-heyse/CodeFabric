#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use arrow::{array::{ArrayRef, RecordBatch, StringArray}, datatypes::{DataType as ArrowType, Field, Schema}};
    use arrow::json::resource::*;
    use buoyant_kernel::{engine::{arrow_data::ArrowEngineData, arrow_utils::parse_json}, schema::{DataType, StructField, StructType}};
    #[derive(Debug)] struct Admission;
    #[derive(Debug)] struct Receipt(usize);
    impl ResourceReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
    impl ResourceAdmission for Admission {
        fn try_reserve(&self, request: ResourceRequest) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
            if request.kind == "JSON primitive values" { Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }) }
            else { Ok(Arc::new(Receipt(request.bytes))) }
        }
    }
    #[test]
    fn kernel_parse_json_inherits_required_native_reader_policy() {
        let policy = ReaderResourcePolicy::try_new(Arc::new(Admission), ReaderResourceLimits { allocations: 1024, collection_entries: 65536, string_bytes: 65536, nesting: 128 }).unwrap();
        let _worker = policy.enter_thread().unwrap();
        let schema = Arc::new(Schema::new(vec![Field::new("json", ArrowType::Utf8, false)]));
        let values: ArrayRef = Arc::new(StringArray::from(vec!["{\"v\":1}"]));
        let batch = RecordBatch::try_new(schema, vec![values]).unwrap();
        let target = Arc::new(StructType::try_new([StructField::new("v", DataType::LONG, true)]).unwrap());
        let result = parse_json(Box::new(ArrowEngineData::new(batch)), target);
        let error = match result { Ok(_) => panic!("native allocation admission was bypassed"), Err(error) => error };
        assert!(error.is_resource_exhausted(), "{error:?}");
    }
}
