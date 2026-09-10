use super::super::types as type_facts;
use super::*;
use arrow_array::{BinaryArray, BooleanArray};

#[allow(
    clippy::too_many_lines,
    reason = "independent Arrow records exercise source, context and compiler-owner boundaries"
)]
async fn fixture(generation: u64, run: u8) -> datafusion::prelude::SessionContext {
    let inventory = ProviderSourceInventory::try_new(
        [6; 16],
        generation,
        [25; 32],
        &[b"7.rs".to_vec()],
        vec![ProviderInventoryMember {
            relative_path: b"7.rs".to_vec(),
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
            ("source_generation", numbers(&[generation])),
            ("file_id", ids16(&[7])),
            ("content_digest", digests(&[17])),
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"7.rs"])),
            ),
            ("byte_length", numbers(&[100])),
            ("disposition", strings(&["captured"])),
        ],
    );
    provider(
        &mut builder,
        RUN,
        vec![
            ("provider_run_id", ids16(&[run, 88])),
            (
                "provider_run_identity",
                strings(&["native-run", "other-context"]),
            ),
            ("source_generation", numbers(&[generation; 2])),
            ("context_id", ids16(&[9, 88])),
            ("provider", strings(&["rustc"; 2])),
        ],
    );
    provider(
        &mut builder,
        DECLARATION,
        vec![
            ("entity_id", ids16(&[])),
            ("context_id", ids16(&[])),
            ("source_generation", numbers(&[])),
            ("language", strings(&[])),
        ],
    );
    let file = identity::encode_public_id(IdentityDomain::SourceFile, None, [7; 16]).unwrap();
    // A second compiler owner has the same array key but no primitive node. It must
    // retain an unknown child even though another owner's graph contains that key.
    let mut raw = vec![
        (
            "provider_run_id",
            strings(&[
                "native-run",
                "native-run",
                "native-run",
                "native-run",
                "native-run",
                "other-context",
                "native-run",
                "native-run",
                "unaccepted",
            ]),
        ),
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
                generation,
                generation,
            ]),
        ),
        ("source_file_id", strings(&[file.as_str(); 9])),
        (
            "source_content_digest",
            digests(&[17, 17, 17, 99, 17, 17, 17, 17, 17]),
        ),
        ("compilation_unit_id", strings(&["unit"; 9])),
        (
            "owner_id",
            strings(&["a", "a", "a", "a", "a", "a", "b", "b", "a"]),
        ),
        ("type_key", digests(&[0, 1, 1, 2, 3, 0, 1, 1, 4])),
        (
            "type_kind",
            strings(&[
                "Uint", "Array", "Array", "Never", "Never", "Uint", "Array", "Array", "Never",
            ]),
        ),
        (
            "component_role",
            strings(&[
                "self", "self", "element", "self", "self", "self", "self", "element", "self",
            ]),
        ),
        ("component_ordinal", numbers(&[0, 0, 1, 0, 0, 0, 0, 1, 0])),
        (
            "component_type_key",
            crate::fabric::hash32_array([
                None,
                None,
                Some(&[0; 32]),
                None,
                None,
                None,
                None,
                Some(&[0; 32]),
                None,
            ]),
        ),
        (
            "primitive_kind",
            Arc::new(StringArray::from(vec![
                Some("u8"),
                None,
                None,
                None,
                None,
                Some("u8"),
                None,
                None,
                None,
            ])) as ArrayRef,
        ),
        ("generic_argument_count", numbers(&[0; 9])),
        (
            "array_length",
            Arc::new(UInt64Array::from(vec![
                None,
                Some(2),
                Some(2),
                None,
                None,
                None,
                Some(2),
                Some(2),
                None,
            ])),
        ),
        ("bound_variable_count", numbers(&[0; 9])),
    ];
    for name in [
        "definition_kind",
        "mutability",
        "region_kind",
        "function_abi",
    ] {
        raw.push((name, Arc::new(StringArray::from(vec![None::<&str>; 9]))));
    }
    raw.push((
        "definition_stable_crate_id",
        Arc::new(UInt64Array::from(vec![None; 9])),
    ));
    raw.push((
        "definition_def_path_hash",
        crate::fabric::id16_array([None; 9]),
    ));
    for name in [
        "function_abi_unwind",
        "function_unsafe",
        "function_variadic",
    ] {
        raw.push((name, Arc::new(BooleanArray::from(vec![None; 9]))));
    }
    provider(&mut builder, RustcRelation::Type.relation_id(), raw);
    for relation in [
        type_facts::Relation::Graph,
        type_facts::Relation::RustGraph,
        type_facts::Relation::Type,
        type_facts::Relation::Component,
    ] {
        builder
            .add_transformation(Arc::new(Canonical::new(
                Kind::Type {
                    relation,
                    inputs: type_facts::Inputs::new(
                        false,
                        RustInputs::from_relations([RustcRelation::Type]),
                    ),
                },
                &inventory,
            )))
            .unwrap();
    }
    let mut assembly = builder.into_assembly_parts().3;
    assembly.install_transformations().await.unwrap();
    assembly.candidate_context()
}

#[tokio::test]
async fn rust_graph_fences_source_context_owner_and_stable_identity() {
    let context = fixture(3, 4).await;
    let graph = super::types::rows(&context, type_facts::RUST_GRAPH).await;
    assert_eq!(graph.len(), 4, "stale and unaccepted inputs are excluded");
    let owner_b = graph
        .iter()
        .find(|row| row["provider_owner"] == "b")
        .unwrap();
    assert!(owner_b["type_id"].is_null());
    assert!(
        owner_b["unknown_reason"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    let types = super::types::rows(&context, type_facts::TYPE).await;
    assert_eq!(
        types.len(),
        3,
        "two context-specific primitives and one array"
    );
    let primitives = types
        .iter()
        .filter(|row| row["type_kind_code"] == identity::TypeConstructor::Primitive.code())
        .collect::<Vec<_>>();
    assert_eq!(primitives.len(), 2);
    assert_eq!(
        primitives[0]["canonical_key"],
        primitives[1]["canonical_key"]
    );
    assert_ne!(primitives[0]["type_id"], primitives[1]["type_id"]);
    let components = super::types::rows(&context, type_facts::COMPONENT).await;
    assert_eq!(components.len(), 2);
    assert!(
        components
            .iter()
            .any(|row| !row["owner_type_id"].is_null() && !row["referenced_type_id"].is_null())
    );
    assert!(
        components
            .iter()
            .any(|row| row["owner_type_id"].is_null() && row["referenced_type_id"].is_null())
    );
    let changed = fixture(4, 5).await;
    assert_eq!(types, super::types::rows(&changed, type_facts::TYPE).await);
}
