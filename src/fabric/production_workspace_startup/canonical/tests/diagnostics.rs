use super::*;
use arrow_array::{BinaryArray, BooleanArray};

#[allow(
    clippy::too_many_lines,
    reason = "independent raw Arrow inputs cover both owner and location validity"
)]
async fn diagnostic_fixture(generation: u64, run: u8) -> datafusion::prelude::SessionContext {
    let inventory = ProviderSourceInventory::try_new(
        [6; 16],
        generation,
        [25; 32],
        &[b"7.rs".to_vec(), b"8.rs".to_vec()],
        [7, 8]
            .into_iter()
            .map(|file| ProviderInventoryMember {
                relative_path: format!("{file}.rs").into_bytes(),
                selected_for_provider: true,
                disposition: ProviderInputDisposition::Captured {
                    file_id: [file; 16],
                    digest: [17; 32],
                    byte_length: 100,
                },
            })
            .collect(),
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
            ("workspace_id", ids16(&[6; 2])),
            ("source_generation", numbers(&[generation; 2])),
            ("file_id", ids16(&[7, 8])),
            ("content_digest", digests(&[17; 2])),
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"7.rs", b"8.rs"])),
            ),
            ("byte_length", numbers(&[100; 2])),
            ("disposition", strings(&["captured"; 2])),
        ],
    );
    provider(
        &mut builder,
        RUN,
        vec![
            ("provider_run_id", ids16(&[run])),
            ("provider_run_identity", strings(&["native-run"])),
            ("source_generation", numbers(&[generation])),
            ("context_id", ids16(&[9])),
            ("provider", strings(&["rustc"])),
        ],
    );
    let owner = identity::encode_public_id(IdentityDomain::SourceFile, None, [7; 16]).unwrap();
    let location = identity::encode_public_id(IdentityDomain::SourceFile, None, [8; 16]).unwrap();
    provider(
        &mut builder,
        RustcRelation::Diagnostic.relation_id(),
        vec![
            ("provider_run_id", strings(&["native-run"; 4])),
            (
                "source_generation",
                numbers(&[generation, generation, generation, generation - 1]),
            ),
            ("source_file_id", strings(&[owner.as_str(); 4])),
            ("source_content_digest", digests(&[17, 17, 99, 17])),
            ("compilation_unit_id", strings(&["native-unit"; 4])),
            ("diagnostic_ordinal", numbers(&[0, 1, 2, 3])),
            ("severity", strings(&["error"; 4])),
            ("reason_code", strings(&["E0425"; 4])),
            (
                "message",
                strings(&[
                    "missing function",
                    "another missing function",
                    "stale bytes",
                    "stale generation",
                ]),
            ),
            (
                "structured_compiler_diagnostic",
                Arc::new(BooleanArray::from(vec![true; 4])),
            ),
            ("suggestions_state", strings(&["unavailable"; 4])),
        ],
    );
    provider(
        &mut builder,
        RustcRelation::DiagnosticSpan.relation_id(),
        vec![
            ("provider_run_id", strings(&["native-run"; 3])),
            ("source_generation", numbers(&[generation; 3])),
            ("source_file_id", strings(&[owner.as_str(); 3])),
            ("source_content_digest", digests(&[17; 3])),
            ("compilation_unit_id", strings(&["native-unit"; 3])),
            ("diagnostic_ordinal", numbers(&[0; 3])),
            ("child_ordinal", Arc::new(UInt64Array::from(vec![None; 3]))),
            ("span_ordinal", numbers(&[0, 1, 2])),
            (
                "is_primary",
                Arc::new(BooleanArray::from(vec![true, false, false])),
            ),
            ("label", strings(&["missing"; 3])),
            ("span_file", strings(&["8.rs"; 3])),
            (
                "span_file_bytes",
                Arc::new(BinaryArray::from_vec(vec![b"8.rs"; 3])),
            ),
            ("span_start_byte", numbers(&[20; 3])),
            ("span_end_byte", numbers(&[22, 22, 101])),
            ("expansion_kind", strings(&["root"; 3])),
            ("location_state", strings(&["captured-source"; 3])),
            ("location_file_id", strings(&[location.as_str(); 3])),
            ("location_content_digest", digests(&[17, 99, 17])),
        ],
    );
    for kind in [
        Kind::Diagnostic {
            pyrefly: false,
            rust: true,
        },
        Kind::DiagnosticDetail {
            detail: super::super::diagnostics::Detail::Span,
            available: true,
        },
    ] {
        builder
            .add_transformation(Arc::new(Canonical::new(kind, &inventory)))
            .unwrap();
    }
    let mut assembly = builder.into_assembly_parts().3;
    assembly.install_transformations().await.unwrap();
    assembly.candidate_context()
}

async fn relation(
    context: &datafusion::prelude::SessionContext,
    name: &str,
    order: &str,
) -> RecordBatch {
    let batches = context
        .table(name)
        .await
        .unwrap()
        .sort(vec![col(order).sort(true, false)])
        .unwrap()
        .collect()
        .await
        .unwrap();
    arrow::compute::concat_batches(&batches[0].schema(), &batches).unwrap()
}

#[tokio::test]
async fn canonical_diagnostics_fence_owner_and_location_independently_and_keep_stable_ids() {
    let context = diagnostic_fixture(3, 4).await;
    let messages = relation(&context, "fact.code_diagnostic", "diagnostic_ordinal").await;
    assert_eq!(
        messages.num_rows(),
        2,
        "stale owner bytes and generation cannot enter current diagnostics"
    );
    let spans = relation(&context, "fact.code_diagnostic_span", "span_ordinal").await;
    assert_eq!(spans.num_rows(), 3, "unmapped evidence remains available");
    let file = spans
        .column_by_name("file_id")
        .unwrap()
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .unwrap();
    assert_eq!(
        file.value(0),
        &[8; 16],
        "location is not the compilation owner"
    );
    assert!(file.is_null(1), "changed location content is excluded");
    assert!(file.is_null(2), "out-of-bounds location is excluded");
    let starts = spans
        .column_by_name("start_byte")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(starts.value(0), 20);
    assert!(starts.is_null(1) && starts.is_null(2));
    assert_eq!(
        spans
            .column_by_name("native_start_byte")
            .unwrap()
            .null_count(),
        0
    );
    let states = spans
        .column_by_name("location_state")
        .unwrap()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(
        states.iter().collect::<Vec<_>>(),
        vec![
            Some("captured-source"),
            Some("invalidated-source-location"),
            Some("invalidated-source-location")
        ]
    );
    let next = diagnostic_fixture(5, 8).await;
    let repeated = relation(&next, "fact.code_diagnostic", "diagnostic_ordinal").await;
    assert_eq!(
        messages.column_by_name("diagnostic_id"),
        repeated.column_by_name("diagnostic_id"),
        "run and generation identify validity, not unchanged diagnostic content"
    );
    assert_ne!(
        messages.column_by_name("provider_run_id"),
        repeated.column_by_name("provider_run_id")
    );
}
