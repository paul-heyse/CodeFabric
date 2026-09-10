use super::*;

#[test]
fn absent_selection_return_and_reference_targets_are_block_local_semantic_gaps() {
    let port = port();
    for (target, field, value) in [
        ("selection.where", "where", serde_json::json!(["known"])),
        (
            "return.include",
            "return",
            serde_json::json!({"include":["name"]}),
        ),
        ("input.within", "within", serde_json::json!(["workspace"])),
    ] {
        let mut catalog = catalog(&port);
        let program = catalog
            .program_bindings
            .iter()
            .find(|row| row.compatibility_form == ReleasedSemanticForm::FindCodeEntities)
            .unwrap()
            .program_binding_id
            .clone();
        catalog
            .selections
            .retain(|row| row.program_binding_id != program || row.selection_id.as_ref() != target);
        catalog
            .returns
            .retain(|row| row.program_binding_id != program || row.return_id.as_ref() != target);
        catalog
            .request_inputs
            .retain(|row| row.program_binding_id != program || row.input_id.as_ref() != target);
        let mut clause = serde_json::json!({"request":"find code entities","query_id":"unavailable","looking_for":"functions"});
        clause[field] = value;
        let request = request(serde_json::json!([clause]));
        let IngressPreparation::Ready(ingress) = port
            .prepare_against_catalog(&request, &[], &catalog)
            .unwrap()
        else {
            panic!("missing target needs no guarded choice")
        };
        assert!(ingress.blocks.is_empty());
        assert!(ingress.selections.is_empty());
        assert!(ingress.returns.is_empty());
        assert!(ingress.request_inputs.is_empty());
        assert_eq!(
            ingress.unavailable_blocks[0].issues[0].subject_id.as_ref(),
            target
        );
    }
}

fn request(queries: serde_json::Value) -> ParsedSemanticRequest {
    let mut value: serde_json::Value =
        serde_json::from_slice(&eight_form_request().canonical_bytes).unwrap();
    value["queries"] = queries;
    parse_request(&serde_json::to_vec(&value).unwrap()).unwrap()
}

#[test]
fn missing_form_program_preserves_independent_projection_and_typed_failed_descendants() {
    let port = port();
    let mut catalog = catalog(&port);
    // An absent form is a semantic gap. An inconsistent partially installed catalog remains
    // a global error, so remove every binding belonging to this program deliberately.
    let program = catalog
        .program_bindings
        .iter()
        .find(|row| row.compatibility_form == ReleasedSemanticForm::FindCodeEntities)
        .unwrap()
        .program_binding_id
        .clone();
    catalog
        .program_bindings
        .retain(|row| row.program_binding_id != program);
    catalog
        .selections
        .retain(|row| row.program_binding_id != program);
    catalog
        .returns
        .retain(|row| row.program_binding_id != program);
    catalog
        .request_inputs
        .retain(|row| row.program_binding_id != program);
    catalog
        .consumer_slots
        .retain(|row| row.program_binding_id != program);
    let request = request(serde_json::json!([
        {"request":"retrieve facts about code","query_id":"dependent","about":[{"results_of":"root","select":"entities"}],"facts":["declarations"]},
        {"request":"find code entities","query_id":"root","looking_for":"functions"},
        {"request":"retrieve facts about code","query_id":"good","about":["a subject"],"facts":["declarations"]}
    ]));
    let IngressPreparation::Ready(ingress) = port
        .prepare_against_catalog(&request, &[], &catalog)
        .unwrap()
    else {
        panic!("no guard is needed")
    };
    assert_eq!(
        ingress
            .blocks
            .iter()
            .map(|row| row.query_id.as_ref())
            .collect::<Vec<_>>(),
        ["good"]
    );
    assert_eq!(ingress.unavailable_blocks.len(), 2);
    assert!(ingress.dependencies.is_empty());
    assert!(
        ingress
            .request_inputs
            .iter()
            .all(|row| row.query_id.as_ref() == "good")
    );
    assert!(
        ingress
            .selections
            .iter()
            .all(|row| row.query_id.as_ref() == "good")
    );
    let dependent = ingress
        .unavailable_blocks
        .iter()
        .find(|row| row.query_id.as_ref() == "dependent")
        .unwrap();
    assert_eq!(dependent.issues[0].related_id.as_deref(), Some("root"));
}

#[test]
fn missing_consumer_slot_discards_its_partial_projection_and_guard_requirements() {
    let port = port();
    let mut catalog = catalog(&port);
    let program = catalog
        .program_bindings
        .iter()
        .find(|row| row.compatibility_form == ReleasedSemanticForm::FindCodeEntities)
        .unwrap()
        .program_binding_id
        .clone();
    catalog
        .consumer_slots
        .retain(|row| row.program_binding_id != program);
    let selection = catalog
        .selections
        .iter_mut()
        .find(|row| {
            row.program_binding_id == program
                && row.selection_id.as_ref() == "selection.looking-for"
        })
        .unwrap();
    selection.resolutions = vec![EpochBoundSelectionValueResolution {
        request_value: text("functions").unwrap(),
        execution_value: text("function").unwrap(),
    }];
    let request = request(serde_json::json!([
        {"request":"find code entities","query_id":"root","looking_for":"functions"},
        {"request":"find code entities","query_id":"blocked","looking_for":"choose a meaning","within":[{"results_of":"root","select":"entities"}]}
    ]));
    let IngressPreparation::Ready(ingress) = port
        .prepare_against_catalog(&request, &[], &catalog)
        .unwrap()
    else {
        panic!("failed block's guard must be removed")
    };
    assert_eq!(ingress.blocks.len(), 1);
    assert_eq!(ingress.blocks[0].query_id.as_ref(), "root");
    assert_eq!(ingress.unavailable_blocks[0].query_id.as_ref(), "blocked");
    assert_eq!(
        ingress.unavailable_blocks[0].issues[0].subject_id.as_ref(),
        "slot.within"
    );
}
