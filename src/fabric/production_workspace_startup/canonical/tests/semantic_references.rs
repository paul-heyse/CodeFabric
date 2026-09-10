use super::*;
use crate::pyrefly_service::PyreflyRelation;
use arrow_array::{BinaryArray, BooleanArray};

#[allow(
    clippy::too_many_lines,
    reason = "independent raw inputs exercise source, target, context and candidate validity"
)]
async fn fixture(generation: u64, run: u8) -> datafusion::prelude::SessionContext {
    let inventory = ProviderSourceInventory::try_new(
        [6; 16],
        generation,
        [25; 32],
        &[b"7.py".to_vec(), b"8.py".to_vec()],
        [7, 8]
            .into_iter()
            .map(|file| ProviderInventoryMember {
                relative_path: format!("{file}.py").into_bytes(),
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
                Arc::new(BinaryArray::from_vec(vec![b"7.py", b"8.py"])),
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
            ("provider", strings(&["pyrefly"])),
        ],
    );
    let file =
        |byte| identity::encode_public_id(IdentityDomain::SourceFile, None, [byte; 16]).unwrap();
    let owner = file(7);
    let target = file(8);
    provider(
        &mut builder,
        PyreflyRelation::ModuleContext.relation_id(),
        vec![
            ("provider_run_id", strings(&["native-run"; 2])),
            ("source_generation", numbers(&[generation; 2])),
            ("file_id", strings(&[&owner, &target])),
            ("content_digest", digests(&[17; 2])),
            ("module_name", strings(&["seven", "eight"])),
            ("module_id", strings(&["native-seven", "native-eight"])),
        ],
    );
    provider(
        &mut builder,
        DECLARATION,
        vec![
            ("entity_id", ids16(&[40, 41, 42, 99])),
            ("declaration_id", ids16(&[50, 51, 52, 99])),
            ("context_id", ids16(&[9, 9, 9, 10])),
            ("language", strings(&["python"; 4])),
            ("file_id", ids16(&[8; 4])),
            ("content_digest", digests(&[17; 4])),
            ("source_generation", numbers(&[generation; 4])),
            ("start_byte", numbers(&[20, 20, 30, 30])),
            ("end_byte", numbers(&[26, 26, 36, 36])),
        ],
    );
    provider(
        &mut builder,
        PyreflyRelation::Reference.relation_id(),
        vec![
            ("provider_run_id", strings(&["native-run"; 7])),
            (
                "source_generation",
                numbers(&[
                    generation,
                    generation,
                    generation,
                    generation,
                    generation - 1,
                    generation,
                    generation,
                ]),
            ),
            ("file_id", strings(&[owner.as_str(); 7])),
            ("content_digest", digests(&[17, 17, 17, 99, 17, 17, 17])),
            ("start_byte", numbers(&[0, 10, 20, 30, 40, 50, 99])),
            ("end_byte", numbers(&[1, 11, 21, 31, 41, 51, 101])),
            ("occurrence_ordinal", numbers(&[0, 1, 2, 3, 4, 5, 6])),
            (
                "name",
                strings(&[
                    "ambiguous",
                    "exact",
                    "stale-target",
                    "stale-owner",
                    "stale-generation",
                    "module",
                    "invalid-range",
                ]),
            ),
            ("reference_kind", strings(&["read"; 7])),
            ("resolution_state", strings(&["resolved"; 7])),
            (
                "definition_mapping",
                strings(&["exact_checker_definition"; 7]),
            ),
            ("target_ordinal", numbers(&[0; 7])),
            ("target_file_id", strings(&[target.as_str(); 7])),
            (
                "target_content_digest",
                digests(&[17, 17, 99, 17, 17, 17, 17]),
            ),
            ("target_start_byte", numbers(&[20, 30, 20, 20, 20, 0, 20])),
            ("target_end_byte", numbers(&[26, 36, 26, 26, 26, 0, 26])),
            (
                "target_is_module",
                Arc::new(BooleanArray::from(vec![
                    false, false, false, false, false, true, false,
                ])),
            ),
        ],
    );
    for kind in [
        Kind::Module { pyrefly: true },
        Kind::SemanticReference { pyrefly: true, rust: false },
    ] {
        builder
            .add_transformation(Arc::new(Canonical::new(kind, &inventory)))
            .unwrap();
    }
    let mut assembly = builder.into_assembly_parts().3;
    assembly.install_transformations().await.unwrap();
    assembly.candidate_context()
}

async fn references(generation: u64, run: u8) -> Vec<serde_json::Value> {
    let context = fixture(generation, run).await;
    let batches = context
        .table(super::super::semantic_references::RELATION)
        .await
        .unwrap()
        .sort(vec![
            col("provider_occurrence_ordinal").sort(true, false),
            col("target_entity_id").sort(true, false),
        ])
        .unwrap()
        .collect()
        .await
        .unwrap();
    let mut writer = arrow::json::WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, arrow::json::writer::JsonArray>(Vec::new());
    writer
        .write_batches(&batches.iter().collect::<Vec<_>>())
        .unwrap();
    writer.finish().unwrap();
    serde_json::from_slice(&writer.into_inner()).unwrap()
}

#[tokio::test]
async fn semantic_references_keep_candidates_and_fence_context_source_and_target() {
    let rows = references(3, 4).await;
    assert_eq!(
        rows.len(),
        5,
        "stale owners, generations and invalid ranges are excluded"
    );
    assert_eq!(rows[0]["name"], "ambiguous");
    assert_eq!(rows[1]["name"], "ambiguous");
    for row in &rows[..2] {
        assert_eq!(row["resolution"], "candidates");
        assert_eq!(row["unknown_reason"], "multiple_canonical_definitions");
    }
    assert_ne!(rows[0]["target_entity_id"], rows[1]["target_entity_id"]);
    assert_eq!(rows[0]["reference_id"], rows[1]["reference_id"]);
    assert_eq!(rows[2]["name"], "exact");
    assert_eq!(
        rows[2]["resolution"], "resolved",
        "another analysis context is not a candidate"
    );
    assert_eq!(rows[3]["name"], "stale-target");
    assert_eq!(rows[3]["resolution"], "unknown");
    assert!(rows[3]["target_entity_id"].is_null());
    assert_eq!(rows[4]["name"], "module");
    assert_eq!(rows[4]["resolution"], "resolved");
    assert!(!rows[4]["target_entity_id"].is_null());
    assert!(
        rows[4]["target_declaration_id"].is_null(),
        "module has no invented declaration"
    );
    let next = references(4, 5).await;
    for (old, new) in rows.iter().zip(next) {
        for identity in ["reference_id", "target_entity_id", "target_declaration_id"] {
            assert_eq!(
                old[identity], new[identity],
                "{identity} is independent of run and generation"
            );
        }
        assert_ne!(old["provider_run_id"], new["provider_run_id"]);
    }
}
