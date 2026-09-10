use super::super::block_queries::{block_rows, resource_bytes};
use super::*;

const SOURCE: &str = "from typing import ClassVar, Final\nclass Empty:\n    pass\nclass Café:\n    counter: ClassVar[int] = 0\n    label: Final[str] = 'café'\n    def __init__(self):\n        self.instance = 1\n    @property\n    def value(self) -> int:\n        return self.instance\n    class Nested:\n        item: bytes\n";

fn public_members(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Vec<Value>> {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    let owner = |name: &str| {
        declarations
            .iter()
            .find(|row| row["name"] == name && row["entity_kind"] == "class")
            .unwrap()
    };
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:members-{phase}"),
        "unused",
    );
    request["queries"] = json!([
        {"request":"find code entities","query_id":"classes","looking_for":"Python class declarations"},
        {"request":"retrieve facts about code","query_id":"all","about":[{"results_of":"classes","select":"entities"},{"results_of":"classes","select":"entities"}],"facts":["associated member observations"]},
        {"request":"retrieve facts about code","query_id":"empty","about":[{"entity_id":owner("Empty")["public_entity_id"]}],"facts":["associated member observations"]},
        {"request":"retrieve facts about code","query_id":"cafe","about":[{"entity_id":owner("Café")["public_entity_id"]}],"facts":["associated member observations"]},
        {"request":"find code entities","query_id":"absent","looking_for":"Python function named `absent`"},
        {"request":"retrieve facts about code","query_id":"none","about":[{"results_of":"absent","select":"entities"}],"facts":["associated member observations"]}
    ]);
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}},
            {"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, &format!("members-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let steps = result["pages"].as_array().unwrap().iter().enumerate().map(|(index, page)| json!({"id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]})).collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("member-pages-{phase}"), &scenario);
    let pages = modern_client_report(&run_modern_client(stack, &path));
    let rows = ["all", "empty", "cafe", "none"].map(|query| {
        let binding = manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["query_id"] == query)
            .unwrap();
        let relation = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["relation_id"] == binding["relation_id"])
            .unwrap();
        let start = usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(relation["page_count"].as_u64().unwrap()).unwrap();
        let mut rows = (start..start + count)
            .flat_map(|page| block_rows(&pages, page))
            .collect::<Vec<_>>();
        rows.sort_by_cached_key(Value::to_string);
        rows
    });
    let expected = canonical_diagnostic_rows(fixture, "fact.code_member_observation");
    assert_eq!(rows[0], expected);
    assert_eq!(rows[1].len(), 1);
    assert_eq!(rows[1][0]["record_kind"], "class");
    assert_eq!(
        rows[1][0]["class_expected_members"],
        0,
        "empty class census: {:?}; processing: {:?}",
        rows[1],
        canonical_diagnostic_rows(fixture, "system.entity_processing_scope")
    );
    assert_eq!(rows[1][0]["class_census_complete"], true);
    assert!(rows[1][0]["unknown_reason"].is_null());
    assert!(
        rows[2]
            .iter()
            .all(|row| row["owner_entity_id"] == owner("Café")["entity_id"])
    );
    let member = |name: &str| {
        rows[2]
            .iter()
            .find(|row| row["member_name"] == name)
            .unwrap()
    };
    let counter = member("counter");
    let primitive = super::callables::primitive(counter, "python");
    assert_eq!(counter["declared_type_id"], primitive);
    assert_eq!(counter["computed_type_id"], primitive);
    assert_eq!(counter["is_class_var"], true);
    assert_eq!(member("label")["is_final"], true);
    assert_eq!(member("value")["is_property"], true);
    assert_eq!(member("value")["native_kind"], "property");
    assert_eq!(member("instance")["definition_kind"], "defined-in-method");
    assert!(rows[3].is_empty());
    for row in &rows[0] {
        let start = usize::try_from(row["class_start_byte"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(row["class_end_byte"].as_u64().unwrap()).unwrap();
        assert_eq!(&SOURCE[start..end], row["class_name"].as_str().unwrap());
    }
    rows.to_vec()
}

#[test]
fn pragmatic_native_members_preserve_ownership_empty_scopes_types_and_reopen() {
    let fixture = ProductionFixture::with_source(SOURCE.as_bytes());
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let selected = wait_for_semantic_activation(&fixture);
    let first = public_members(&fixture, &stack, "initial");
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(first, public_members(&fixture, &stack, "reopened"));
    supervisor.stop();
}
