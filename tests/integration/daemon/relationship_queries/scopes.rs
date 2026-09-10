use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn run(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> BTreeMap<String, Vec<Value>> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:reference-scopes-{phase}"),
        "unused",
    );
    let prior = |query: &str| json!({"results_of":query,"select":"entities"});
    let mut queries = Vec::new();
    for language in ["Python", "Rust"] {
        let target = format!("{language}-target");
        let references = format!("{language}-references");
        let named = format!("{language} function named `target`");
        queries.extend([
            json!({"request":"find code entities","query_id":target,"looking_for":named}),
            json!({"request":"find code entities","query_id":references,"looking_for":format!("{language} semantic reference occurrences"),"within":[prior(&target),prior(&target)]}),
            json!({"request":"find code entities","query_id":format!("{language}-named"),"looking_for":format!("{language} semantic reference occurrences"),"within":[named]}),
            json!({"request":"follow code relationships","query_id":format!("{language}-targets"),"starting_from":[prior(&references)],"relationship":"semantic references","direction":"outgoing"}),
            json!({"request":"retrieve source and syntax context","query_id":format!("{language}-source"),"about":[prior(&references)],"context":"exact source span","return":{"maximum_source_bytes":4096}}),
        ]);
    }
    queries.extend([
        json!({"request":"find code entities","query_id":"empty-target","looking_for":"Python function named `absent_target`"}),
        json!({"request":"find code entities","query_id":"empty-references","looking_for":"Python semantic reference occurrences","within":[prior("empty-target")]}),
        json!({"request":"find code entities","query_id":"limited","looking_for":"Python semantic reference occurrences","within":[prior("Python-target")],"return":{"limit":{"maximum_results":1}}}),
    ]);
    let count = queries.len();
    request["queries"] = json!(queries);
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
    let path =
        write_modern_client_scenario(fixture, &format!("reference-scopes-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    assert_eq!(
        result["processing"].as_array().unwrap().len(),
        count,
        "{result}"
    );
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let pages = result["pages"].as_array().unwrap();
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!(
            pages
                .iter()
                .enumerate()
                .map(|(index, page)| json!({
                    "id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]
                }))
                .collect::<Vec<_>>()
        ),
    );
    let path = write_modern_client_scenario(
        fixture,
        &format!("reference-scope-pages-{phase}"),
        &scenario,
    );
    let report = modern_client_report(&run_modern_client(stack, &path));
    let mut outputs = BTreeMap::new();
    for binding in manifest["canonical_semantic_response"]["queries"]
        .as_array()
        .unwrap()
    {
        let relation = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["relation_id"] == binding["relation_id"])
            .unwrap();
        let start = usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(relation["page_count"].as_u64().unwrap()).unwrap();
        outputs.insert(
            binding["query_id"].as_str().unwrap().to_owned(),
            (start..start + count)
                .flat_map(|page| block_rows(&report, page))
                .collect::<Vec<_>>(),
        );
    }
    for language in ["Python", "Rust"] {
        let target = &outputs[&format!("{language}-target")];
        assert_eq!(target.len(), 1);
        let rows = &outputs[&format!("{language}-references")];
        assert!(
            !rows.is_empty(),
            "{language} references to the selected target"
        );
        assert_eq!(rows, &outputs[&format!("{language}-named")]);
        assert!(rows.iter().any(|row| row["name"] == "alias"), "{rows:?}");
        assert!(rows.iter().all(|row| row["entity_kind"] == "reference"));
        let unique = rows
            .iter()
            .map(|row| row["public_entity_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            rows.len(),
            unique.len(),
            "repeated subjects must not multiply occurrences"
        );
        let facts = &outputs[&format!("{language}-targets")];
        assert_eq!(facts.len(), rows.len());
        assert!(
            facts
                .iter()
                .all(|row| row["public_target_entity_id"] == target[0]["public_entity_id"])
        );
        let sources = &outputs[&format!("{language}-source")];
        assert_eq!(sources.len(), rows.len());
        assert!(
            sources
                .iter()
                .any(|row| row["source_context"]["text"] == "alias")
        );
        let status = result["processing"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["query_id"] == format!("{language}-references"))
            .unwrap();
        assert_eq!(status["family"], "semantic-references");
    }
    assert!(outputs["empty-target"].is_empty());
    assert!(
        outputs["empty-references"].is_empty(),
        "an empty prior cannot become an unscoped census"
    );
    assert_eq!(outputs["limited"], outputs["Python-references"][..1]);
    // A fresh request after reopen has its own snapshot/disclosure-bound source handle.
    // Compare every canonical fact and delivered byte/range, preserving provider identity too.
    for rows in outputs.values_mut() {
        for row in rows {
            if let Some(context) = row.get_mut("source_context") {
                assert!(
                    context["source_context_id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("context:"))
                );
                context.as_object_mut().unwrap().remove("source_context_id");
            }
        }
    }
    outputs
}
