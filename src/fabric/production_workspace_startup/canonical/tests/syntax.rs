use super::*;
use arrow_array::{BinaryArray, BooleanArray};
use std::collections::BTreeSet;

#[tokio::test]
async fn native_python_cst_preserves_nodes_flags_and_exact_source_identity() {
    for (text, stale) in [
        (
            "# café\r\ndef target(x: int) -> int:\r\n    return x + 1\r\n",
            false,
        ),
        ("def broken(:\n    return (\n", false),
        ("def target():\n    return 1\n", true),
    ] {
        let run = crate::provider_native_syntax::job_tests::run_fixture(text, 3, 7);
        let raw = run.relation(NativeSyntaxRelation::TreeSitterCstNode);
        let digest = crate::integrity::digest_bytes(text.as_bytes());
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
                    digest,
                    byte_length: text.len() as u64,
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
                ("source_generation", numbers(&[if stale { 4 } else { 3 }])),
                ("file_id", ids16(&[7])),
                (
                    "content_digest",
                    crate::fabric::hash32_array([Some(&digest)]),
                ),
                (
                    "relative_path",
                    Arc::new(BinaryArray::from_vec(vec![b"7.py"])),
                ),
                ("byte_length", numbers(&[text.len() as u64])),
                ("disposition", strings(&["captured"])),
            ],
        );
        provider(
            &mut builder,
            NativeSyntaxRelation::TreeSitterCstNode.as_str(),
            raw.schema()
                .fields()
                .iter()
                .zip(raw.columns())
                .map(|(field, array)| (field.name().as_str(), array.clone()))
                .collect(),
        );
        builder
            .add_transformation(Arc::new(Canonical::new(
                Kind::Syntax {
                    python: true,
                    rust: false,
                },
                &inventory,
            )))
            .unwrap();
        let mut assembly = builder.into_assembly_parts().3;
        assembly.install_transformations().await.unwrap();
        let batches = assembly
            .candidate_context()
            .table(super::super::syntax::RELATION)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            if stale { 0 } else { raw.num_rows() }
        );
        assert_native_rows(raw, &batches);
    }
}

fn assert_native_rows(raw: &RecordBatch, batches: &[RecordBatch]) {
    let mut ids = BTreeSet::new();
    let raw_ids = raw
        .column_by_name("provider_local_node_id")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    for batch in batches {
        let local = batch
            .column_by_name("provider_local_node_id")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let entities = batch
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        let contexts = batch
            .column_by_name("context_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert!(ids.insert(entities.value(row).to_vec()));
            assert_eq!(contexts.value(row), crate::identity::SOURCE_CONTEXT_ID);
            let raw_row = (0..raw.num_rows())
                .find(|index| raw_ids.value(*index) == local.value(row))
                .unwrap();
            for name in ["named", "extra", "error", "missing"] {
                let expected = raw
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .value(raw_row);
                let actual = batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .value(row);
                assert_eq!(actual, expected);
            }
            for name in ["start_byte", "end_byte"] {
                let expected = raw
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(raw_row);
                let actual = batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(row);
                assert_eq!(actual, expected);
            }
        }
    }
}
