use super::*;
use arrow_array::BinaryArray;

fn entity(hash: u8) -> [u8; 16] {
    let key = [51_u64.to_be_bytes().as_slice(), &[hash; 16]].concat();
    let owner =
        identity::semantic_owner_identity([6; 16], [9; 16], "rust-item", key.clone()).unwrap();
    identity::semantic_entity_identity([6; 16], [9; 16], 1, owner.id, key)
        .unwrap()
        .id
}

#[allow(
    clippy::too_many_lines,
    reason = "independent Arrow inputs isolate compiler owner, location, target and namespace binding boundaries"
)]
async fn fixture(generation: u64, run: u8) -> datafusion::prelude::SessionContext {
    let inventory = ProviderSourceInventory::try_new(
        [6; 16],
        generation,
        [25; 32],
        &[b"7.rs".to_vec(), b"8.rs".to_vec(), b"9.rs".to_vec()],
        [7, 8, 9]
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
            ("workspace_id", ids16(&[6; 3])),
            ("source_generation", numbers(&[generation; 3])),
            ("file_id", ids16(&[7, 8, 9])),
            ("content_digest", digests(&[17; 3])),
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"7.rs", b"8.rs", b"9.rs"])),
            ),
            ("byte_length", numbers(&[100; 3])),
            ("disposition", strings(&["captured"; 3])),
        ],
    );
    provider(
        &mut builder,
        RUN,
        vec![
            ("provider_run_id", ids16(&[run])),
            ("provider_run_identity", strings(&["run"])),
            ("source_generation", numbers(&[generation])),
            ("context_id", ids16(&[9])),
            ("provider", strings(&["rustc"])),
        ],
    );
    let entities = [entity(88), entity(88), entity(89), entity(89), entity(90)];
    provider(
        &mut builder,
        DECLARATION,
        vec![
            (
                "entity_id",
                crate::fabric::id16_array(entities.iter().map(Some)),
            ),
            ("declaration_id", ids16(&[80, 81, 82, 83, 84])),
            ("context_id", ids16(&[9, 9, 9, 99, 9])),
            ("file_id", ids16(&[8; 5])),
            ("content_digest", digests(&[17, 17, 17, 17, 99])),
            ("source_generation", numbers(&[generation; 5])),
            ("language", strings(&["rust"; 5])),
        ],
    );
    let file = |id| identity::encode_public_id(IdentityDomain::SourceFile, None, [id; 16]).unwrap();
    let owner = file(7);
    let location = file(9);
    let scope = |count| {
        vec![
            ("provider_run_id", strings(&vec!["run"; count])),
            ("compilation_unit_id", strings(&vec!["unit"; count])),
            ("owner_id", strings(&vec!["owner"; count])),
            ("source_generation", numbers(&vec![generation; count])),
            ("source_file_id", strings(&vec![owner.as_str(); count])),
            ("source_content_digest", digests(&vec![17; count])),
            ("location_file_id", strings(&vec![location.as_str(); count])),
            ("location_content_digest", digests(&vec![17; count])),
            ("location_state", strings(&vec!["captured-source"; count])),
            ("expansion_kind", strings(&vec!["source-authored"; count])),
        ]
    };
    let mut refs = scope(9);
    refs[0].1 = strings(&[
        "run",
        "run",
        "run",
        "run",
        "run",
        "run",
        "unaccepted",
        "run",
        "run",
    ]);
    refs[3].1 = numbers(&[
        generation,
        generation,
        generation,
        generation,
        generation,
        generation - 1,
        generation,
        generation,
        generation,
    ]);
    refs[5].1 = digests(&[17, 17, 17, 17, 99, 17, 17, 17, 17]);
    refs[7].1 = digests(&[17, 17, 17, 99, 17, 17, 17, 17, 17]);
    refs.extend([
        ("reference_ordinal", numbers(&[0, 1, 2, 3, 4, 5, 6, 7, 8])),
        (
            "target_ordinal",
            Arc::new(UInt64Array::from(vec![
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                None,
                Some(0),
            ])),
        ),
        (
            "name",
            strings(&[
                "candidate",
                "exact",
                "stale-target",
                "stale-location",
                "stale-owner",
                "old-generation",
                "unaccepted",
                "local",
                "invalid-range",
            ]),
        ),
        ("reference_kind", strings(&["import"; 9])),
        (
            "resolution_kind",
            strings(&[
                "definition",
                "definition",
                "definition",
                "definition",
                "definition",
                "definition",
                "definition",
                "unresolved",
                "definition",
            ]),
        ),
        (
            "span_start_byte",
            numbers(&[10, 20, 30, 40, 50, 60, 70, 80, 99]),
        ),
        (
            "span_end_byte",
            numbers(&[11, 21, 31, 41, 51, 61, 71, 81, 101]),
        ),
        ("target_stable_crate_id", numbers(&[51; 9])),
        (
            "target_def_path_hash",
            crate::fabric::id16_array([
                Some(&[88; 16]),
                Some(&[89; 16]),
                Some(&[90; 16]),
                Some(&[89; 16]),
                Some(&[89; 16]),
                Some(&[89; 16]),
                Some(&[89; 16]),
                None,
                Some(&[89; 16]),
            ]),
        ),
        ("target_definition_kind", strings(&["function"; 9])),
        ("target_native_definition_kind", strings(&["function"; 9])),
        ("target_namespace", strings(&["value"; 9])),
    ]);
    provider(
        &mut builder,
        RustcRelation::HirReference.relation_id(),
        refs,
    );
    let mut imports = scope(3);
    imports[1].1 = strings(&["unit", "unit", "another-unit"]);
    imports.extend([
        ("import_ordinal", numbers(&[0, 1, 2])),
        ("reference_ordinal", numbers(&[0, 1, 1])),
        ("span_start_byte", numbers(&[10, 20, 30])),
        ("span_end_byte", numbers(&[11, 21, 31])),
        ("import_kind", strings(&["single"; 3])),
        ("path", strings(&["target"; 3])),
        ("alias", strings(&["a", "b", "c"])),
        (
            "is_public",
            Arc::new(arrow_array::BooleanArray::from(vec![false; 3])) as ArrayRef,
        ),
    ]);
    provider(
        &mut builder,
        RustcRelation::HirImport.relation_id(),
        imports,
    );
    for kind in [
        Kind::SemanticReference {
            pyrefly: false,
            rust: true,
        },
        Kind::Import {
            python: false,
            rust: true,
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

// One fixture compares source, target, namespace and coverage fences together.
#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn rust_references_fence_owner_location_target_and_import_unit() {
    let context = fixture(3, 4).await;
    let rows = super::types::rows(&context, super::super::semantic_references::RELATION).await;
    assert_eq!(rows.len(), 7);
    let matching = |ordinal| {
        rows.iter()
            .filter(move |row| row["provider_occurrence_ordinal"] == ordinal)
            .collect::<Vec<_>>()
    };
    let candidates = matching(0);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0]["reference_id"], candidates[1]["reference_id"]);
    assert_ne!(
        candidates[0]["target_declaration_id"],
        candidates[1]["target_declaration_id"]
    );
    assert!(
        candidates
            .iter()
            .all(|row| row["resolution"] == "candidates")
    );
    assert_eq!(matching(1)[0]["resolution"], "resolved");
    assert_eq!(
        matching(2)[0]["unknown_reason"],
        "canonical_definition_unavailable"
    );
    assert!(matching(2)[0]["target_entity_id"].is_null());
    for ordinal in [3, 8] {
        let row = matching(ordinal)[0];
        for field in [
            "reference_id",
            "file_id",
            "content_digest",
            "start_byte",
            "end_byte",
        ] {
            assert!(
                row[field].is_null(),
                "invalid location must not borrow compilation-owner coordinates: {row}"
            );
        }
        assert_eq!(
            row["unknown_reason"],
            "compiler_reference_location_unavailable"
        );
    }
    assert_eq!(
        matching(7)[0]["unknown_reason"],
        "compiler_reference_kind_not_normalized"
    );
    let imports = super::types::rows(&context, super::super::imports::RELATION).await;
    assert_eq!(imports.len(), 4);
    let other = imports.iter().find(|row| row["alias_name"] == "c").unwrap();
    assert_eq!(
        other["unknown_reason"],
        "compiler_import_reference_unavailable"
    );
    assert!(other["target_entity_id"].is_null());
    for (native, relation, expected) in [
        (
            RustcRelation::HirReference,
            super::super::semantic_references::RELATION,
            1,
        ),
        (RustcRelation::HirImport, super::super::imports::RELATION, 0),
    ] {
        let raw = context
            .table(TableReference::full(
                FABRIC_CATALOG,
                "raw_ruff",
                native.relation_id().replace('.', "_"),
            ))
            .await
            .unwrap()
            .into_unoptimized_plan();
        let runs = context
            .table(TableReference::full(
                FABRIC_CATALOG,
                "raw_ruff",
                RUN.replace('.', "_"),
            ))
            .await
            .unwrap()
            .into_unoptimized_plan();
        let canonical = context
            .table(relation)
            .await
            .unwrap()
            .into_unoptimized_plan();
        let missing =
            super::super::rust_references::missing_binding_rows(raw, runs, canonical, native)
                .unwrap();
        let batches = context
            .execute_logical_plan(missing)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            expected,
            "only the stale owner disappeared; a retained null-target observation still counts as represented"
        );
    }
    let next = fixture(4, 5).await;
    let keys = |rows: Vec<serde_json::Value>| {
        rows.into_iter()
            .map(|row| {
                serde_json::to_string(&[
                    &row["reference_id"],
                    &row["target_entity_id"],
                    &row["target_declaration_id"],
                    &row["resolution"],
                ])
                .unwrap()
            })
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        keys(rows),
        keys(super::types::rows(&next, super::super::semantic_references::RELATION).await)
    );
}
