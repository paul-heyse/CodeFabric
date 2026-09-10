use super::*;
use crate::pyrefly_service::PyreflyRelation;
use arrow_array::{BinaryArray, BooleanArray};

#[allow(
    clippy::too_many_lines,
    reason = "explicit Arrow inputs exercise identity, graph dependency and validity boundaries independently"
)]
async fn fixture(generation: u64, run: u8, context: u8) -> datafusion::prelude::SessionContext {
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
            ("context_id", ids16(&[context])),
            ("provider", strings(&["pyrefly"])),
        ],
    );
    provider(
        &mut builder,
        DECLARATION,
        vec![
            ("entity_id", ids16(&[70, 80, 81, 99])),
            ("workspace_id", ids16(&[6; 4])),
            ("context_id", ids16(&[context, context, context, 99])),
            ("file_id", ids16(&[8; 4])),
            ("content_digest", digests(&[17; 4])),
            ("source_generation", numbers(&[generation; 4])),
            ("language", strings(&["python"; 4])),
            ("start_byte", numbers(&[10, 20, 20, 30])),
            ("end_byte", numbers(&[13, 23, 23, 33])),
        ],
    );
    let file =
        |byte| identity::encode_public_id(IdentityDomain::SourceFile, None, [byte; 16]).unwrap();
    let owner = file(7);
    let target = file(8);
    let scope = |count: usize| {
        vec![
            ("provider_run_id", strings(&vec!["native-run"; count])),
            ("source_generation", numbers(&vec![generation; count])),
            ("file_id", strings(&vec![owner.as_str(); count])),
            ("content_digest", digests(&vec![17; count])),
        ]
    };
    let mut nodes = scope(10);
    // Last two rows have stale owner bytes/generation and never enter the accepted graph.
    nodes[1].1 = numbers(&[
        generation,
        generation,
        generation,
        generation,
        generation,
        generation,
        generation,
        generation,
        generation,
        generation - 1,
    ]);
    nodes[3].1 = digests(&[17, 17, 17, 17, 17, 17, 17, 17, 99, 17]);
    nodes.extend([
        ("local_type_index", numbers(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])),
        (
            "type_kind",
            strings(&[
                "nominal",
                "nominal",
                "nominal",
                "nominal",
                "tuple",
                "unsupported",
                "error",
                "tuple",
                "never",
                "never",
            ]),
        ),
        (
            "intrinsic",
            Arc::new(StringArray::from(vec![
                Some("int"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ])) as ArrayRef,
        ),
        (
            "style",
            Arc::new(StringArray::from(vec![
                None,
                None,
                None,
                None,
                Some("concrete"),
                None,
                None,
                Some("concrete"),
                None,
                None,
            ])),
        ),
        ("definition_file_id", strings(&[target.as_str(); 10])),
        ("definition_content_digest", digests(&[17; 10])),
        (
            "definition_start_byte",
            numbers(&[0, 10, 20, 30, 0, 0, 0, 0, 0, 0]),
        ),
        (
            "definition_end_byte",
            numbers(&[0, 13, 23, 33, 0, 0, 0, 0, 0, 0]),
        ),
        (
            "literal_kind",
            Arc::new(StringArray::from(vec![None::<&str>; 10])),
        ),
        (
            "literal_text",
            Arc::new(StringArray::from(vec![None::<&str>; 10])),
        ),
        (
            "literal_bytes",
            Arc::new(BinaryArray::from(vec![None::<&[u8]>; 10])),
        ),
        (
            "literal_boolean",
            Arc::new(BooleanArray::from(vec![None; 10])),
        ),
    ]);
    provider(&mut builder, PyreflyRelation::TypeNode.relation_id(), nodes);
    let mut edges = scope(3);
    edges.extend([
        ("owner_local_type_index", numbers(&[4, 4, 7])),
        ("referenced_local_type_index", numbers(&[0, 1, 900])),
        ("component_role", strings(&["element"; 3])),
        ("component_ordinal", numbers(&[0, 1, 0])),
        (
            "parameter_kind",
            Arc::new(StringArray::from(vec![None::<&str>; 3])) as ArrayRef,
        ),
        (
            "parameter_name",
            Arc::new(StringArray::from(vec![None::<&str>; 3])),
        ),
        (
            "parameter_required",
            Arc::new(BooleanArray::from(vec![None; 3])),
        ),
    ]);
    provider(&mut builder, PyreflyRelation::TypeEdge.relation_id(), edges);
    let mut observations = scope(5);
    observations.extend([
        ("occurrence_ordinal", numbers(&[0, 1, 2, 3, 4])),
        ("start_byte", numbers(&[0, 2, 4, 6, 99])),
        ("end_byte", numbers(&[1, 3, 5, 7, 101])),
        (
            "local_type_index",
            Arc::new(UInt64Array::from(vec![
                Some(4),
                Some(2),
                None,
                Some(900),
                Some(0),
            ])) as ArrayRef,
        ),
        ("type_role", strings(&["checker-observed"; 5])),
    ]);
    provider(
        &mut builder,
        PyreflyRelation::TypeObservation.relation_id(),
        observations,
    );
    for relation in [
        super::super::types::Relation::Graph,
        super::super::types::Relation::Type,
        super::super::types::Relation::Observation,
        super::super::types::Relation::Component,
    ] {
        builder
            .add_transformation(Arc::new(Canonical::new(
                Kind::Type {
                    relation,
                    inputs: super::super::types::Inputs::new(true, RustInputs::from_relations([])),
                },
                &inventory,
            )))
            .unwrap();
    }
    builder
        .add_transformation(Arc::new(Canonical::new(
            Kind::Type {
                relation: super::super::types::Relation::RustGraph,
                inputs: super::super::types::Inputs::new(false, RustInputs::from_relations([])),
            },
            &inventory,
        )))
        .unwrap();
    let mut assembly = builder.into_assembly_parts().3;
    assembly.install_transformations().await.unwrap();
    assembly.candidate_context()
}

pub(super) async fn rows(
    context: &datafusion::prelude::SessionContext,
    table: &str,
) -> Vec<serde_json::Value> {
    let batches = context.table(table).await.unwrap().collect().await.unwrap();
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
async fn type_graph_fences_anchors_inputs_and_preserves_unknown_observations() {
    let context = fixture(3, 4, 9).await;
    let graph = rows(&context, super::super::types::GRAPH).await;
    assert_eq!(graph.len(), 8);
    let node = |index| {
        graph
            .iter()
            .find(|row| row["local_type_index"] == index)
            .unwrap()
    };
    for index in [0, 1, 4, 6] {
        assert!(!node(index)["type_id"].is_null(), "{}", node(index));
    }
    for index in [2, 3, 5, 7] {
        assert!(node(index)["type_id"].is_null());
        assert!(!node(index)["unknown_reason"].is_null());
    }
    assert_eq!(node(2)["unknown_reason"], "nominal_definition_unavailable");
    assert_eq!(node(3)["unknown_reason"], "nominal_definition_unavailable");
    let observations = rows(&context, super::super::types::OBSERVATION).await;
    assert_eq!(observations.len(), 5);
    let observation = |index| {
        observations
            .iter()
            .find(|row| row["provider_occurrence_ordinal"] == index)
            .unwrap()
    };
    assert_eq!(observation(0)["type_id"], node(4)["type_id"]);
    assert_eq!(observation(2)["unknown_reason"], "native_type_graph_limit");
    assert_eq!(
        observation(3)["unknown_reason"],
        "native_type_node_unavailable"
    );
    assert_eq!(
        observation(4)["unknown_reason"],
        "type_location_unavailable"
    );
    assert!(observation(4)["type_occurrence_id"].is_null());
    let components = rows(&context, super::super::types::COMPONENT).await;
    assert_eq!(components.len(), 3);
    assert!(
        components
            .iter()
            .any(|row| row["owner_type_id"] == node(4)["type_id"]
                && row["referenced_type_id"] == node(1)["type_id"]
                && row["component_ordinal"] == 1)
    );
    assert!(
        components
            .iter()
            .any(|row| row["referenced_local_type_index"] == 900
                && row["referenced_type_id"].is_null()
                && !row["unknown_reason"].is_null())
    );
}

#[tokio::test]
async fn canonical_type_keys_survive_runs_and_generations_with_contextual_ids() {
    let first = fixture(3, 4, 9).await;
    let next = fixture(4, 5, 9).await;
    let other = fixture(3, 4, 10).await;
    let first = rows(&first, super::super::types::TYPE).await;
    let next = rows(&next, super::super::types::TYPE).await;
    let other = rows(&other, super::super::types::TYPE).await;
    assert_eq!(first.len(), 4);
    for value in &first {
        let next = next
            .iter()
            .find(|row| row["canonical_key"] == value["canonical_key"])
            .unwrap();
        assert_eq!(value["type_id"], next["type_id"]);
        assert!(other.iter().all(|row| row["type_id"] != value["type_id"]));
    }
}
