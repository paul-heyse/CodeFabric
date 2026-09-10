use super::*;
use arrow_array::BinaryArray;

#[tokio::test]
async fn occurrence_sources_require_exact_pins_valid_spans_and_provenance() {
    let inventory = ProviderSourceInventory::try_new(
        [6; 16],
        3,
        [25; 32],
        &[b"7.py".to_vec()],
        vec![ProviderInventoryMember {
            relative_path: b"7.py".to_vec(),
            selected_for_provider: true,
            disposition: ProviderInputDisposition::Captured {
                file_id: [7; 16],
                digest: [17; 32],
                byte_length: 100,
            },
        }],
        vec![],
        None,
    )
    .unwrap();
    let mut builder = ProgrammaticFabricEpochBuilder::try_new(
        FabricEpochId::from_bytes([42; 16]),
        FabricEpochRuntimeConfig::default(),
    )
    .unwrap();
    provider(
        &mut builder,
        SOURCE,
        vec![
            ("workspace_id", ids16(&[6])),
            ("source_generation", numbers(&[3])),
            ("file_id", ids16(&[7])),
            ("content_digest", digests(&[17])),
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"7.py"])),
            ),
            ("byte_length", numbers(&[100])),
        ],
    );
    let occurrence_ids = [Some([40; 16]); 9]
        .into_iter()
        .chain([None])
        .collect::<Vec<_>>();
    let run_ids = [Some([4; 16]); 8]
        .into_iter()
        .chain([None, Some([4; 16])])
        .collect::<Vec<_>>();
    provider(
        &mut builder,
        super::super::calls::RELATION,
        vec![
            (
                "call_site_id",
                crate::fabric::id16_array(occurrence_ids.iter().map(Option::as_ref)),
            ),
            (
                "provider_run_id",
                crate::fabric::id16_array(run_ids.iter().map(Option::as_ref)),
            ),
            ("context_id", ids16(&[8; 10])),
            ("file_id", ids16(&[7; 10])),
            ("workspace_id", ids16(&[6, 6, 6, 6, 99, 6, 6, 6, 6, 6])),
            (
                "source_generation",
                numbers(&[3, 3, 3, 99, 3, 3, 3, 3, 3, 3]),
            ),
            (
                "content_digest",
                digests(&[17, 17, 99, 17, 17, 17, 17, 17, 17, 17]),
            ),
            (
                "start_byte",
                Arc::new(UInt64Array::from(vec![
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                    None,
                    Some(2),
                    Some(5),
                    Some(2),
                    Some(2),
                ])),
            ),
            ("end_byte", numbers(&[4, 4, 4, 4, 4, 4, 101, 4, 4, 4])),
            ("language", strings(&["python"; 10])),
            ("provider", strings(&["pyrefly"; 10])),
            ("raw_dispatch_kind", strings(&["direct"; 10])),
        ],
    );
    for (relation, fields) in [
        (
            super::super::syntax::RELATION,
            super::super::syntax::fields(),
        ),
        (DECLARATION, declaration_fields()),
        (REFERENCE, reference_fields()),
        (
            super::super::semantic_references::RELATION,
            super::super::semantic_references::fields(),
        ),
        (
            super::super::imports::RELATION,
            super::super::imports::fields(),
        ),
        (
            super::super::modules::RELATION,
            super::super::modules::fields(),
        ),
    ] {
        provider(
            &mut builder,
            relation,
            fields
                .into_iter()
                .map(|(name, data_type, _)| (name, arrow_array::new_empty_array(&data_type)))
                .collect(),
        );
    }
    builder
        .add_transformation(Arc::new(Canonical::new(
            Kind::SourceContext {
                python: false,
                rust: false,
            },
            &inventory,
        )))
        .unwrap();
    let mut assembly = builder.into_assembly_parts().3;
    assembly.install_transformations().await.unwrap();
    let batches = assembly
        .candidate_context()
        .table(super::super::source_context::RELATION)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(
        batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
        3,
        "one valid occurrence, deduplicated across candidates, in each source context meaning"
    );
    for batch in batches {
        assert_eq!(
            batch.column_by_name("declaration_id").unwrap().null_count(),
            batch.num_rows()
        );
        let start = batch
            .column_by_name("start_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let end = batch
            .column_by_name("end_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert_eq!((start.value(row), end.value(row)), (2, 4));
        }
    }
}
