use super::*;

pub(super) fn run(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Value> {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    let target = |language: &str| {
        declarations
            .iter()
            .find(|row| {
                row["language"] == language
                    && row["name"]
                        .as_str()
                        .is_some_and(|name| name == "target" || name.ends_with("::target"))
            })
            .unwrap()
    };
    let all = declarations
        .iter()
        .filter(|row| row["entity_kind"] == "function")
        .map(|row| json!({"entity_id":row["public_entity_id"]}))
        .collect::<Vec<_>>();
    let mut queries = vec![
        json!({"request":"find code entities", "query_id":"functions", "looking_for":"function declarations"}),
    ];
    for language in ["python", "rust"] {
        let subject = json!({"entity_id":target(language)["public_entity_id"]});
        queries.push(
            json!({"request":"retrieve source and syntax context", "query_id":language,
            "about":[subject, subject], "context":"related occurrence"}),
        );
    }
    queries.extend([
        json!({"request":"retrieve source and syntax context", "query_id":"all", "about":all, "context":"related occurrence"}),
        json!({"request":"retrieve source and syntax context", "query_id":"prior", "about":[{"results_of":"functions","select":"entities"}], "context":"related occurrence"}),
        json!({"request":"retrieve source and syntax context", "query_id":"location", "about":[{"source_location":{"source_file":"b.py","start_byte":4,"semantic_location":"Python function"}}], "context":"related occurrence"}),
    ]);
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:related-{phase}"),
        "unused",
    );
    request["queries"] = json!(queries);
    let (report, rows) = query(fixture, stack, &format!("related-{phase}"), &request);
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(rows["all"], rows["prior"]);
    assert_eq!(rows["location"], rows["python"]);
    for language in ["python", "rust"] {
        assert_related_target(
            fixture,
            &rows[language],
            &target(language)["public_entity_id"],
        );
    }
    let processing = result["processing"].as_array().unwrap();
    let location = processing
        .iter()
        .find(|r| r["query_id"] == "location")
        .unwrap();
    assert!(
        location["remaining_partitions"].as_u64().unwrap() > 0,
        "the unresolved import in the occurrence file remains in scope"
    );
    assert_boundaries(fixture, stack, phase, request, &rows);
    rows["all"].clone()
}

fn assert_boundaries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    mut request: Value,
    rows: &BTreeMap<String, Vec<Value>>,
) {
    request["semantic_request_id"] = json!(format!("request:related-boundary-{phase}"));
    request["scope"]["source_boundaries"] =
        json!([{"kind":"path","root":"b.py"},{"kind":"path","root":"src/inner.rs"}]);
    request["queries"] = json!([request["queries"][1], request["queries"][2]]);
    let (report, bounded) = query(
        fixture,
        stack,
        &format!("related-boundary-{phase}"),
        &request,
    );
    for (language, path) in [("python", "b.py"), ("rust", "src/inner.rs")] {
        let encoded = super::super::block_queries::hex_bytes(path.as_bytes());
        let expected = rows[language]
            .iter()
            .filter(|row| row["relative_path"] == encoded)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            bounded[language], expected,
            "exact authorized occurrence subset"
        );
        assert!(
            bounded[language]
                .iter()
                .all(|row| row["related_relationship"] != "incoming call occurrence"),
            "neither authored target file contains a call: {:?}",
            bounded[language]
        );
    }
    let result = modern_structured(modern_step(&report, "query"));
    let summaries = result["processing"].as_array().unwrap();
    for language in ["python", "rust"] {
        let summary = summaries
            .iter()
            .find(|s| s["query_id"] == language)
            .unwrap();
        assert_eq!(summary["languages"], json!([language]));
        assert!(
            summary["remaining_partitions"].as_u64().unwrap() > 0,
            "{summary}"
        );
        for gap in summary["remainder"].as_array().unwrap() {
            assert_eq!(gap["language"], language);
            if language == "python" {
                assert_eq!(gap["path"], "b.py");
                assert_eq!(gap["reason_code"], "native_semantic_coverage_incomplete");
            }
        }
    }
}

fn assert_related_target(fixture: &ProductionFixture, related: &[Value], subject: &Value) {
    assert!(!related.is_empty(), "related occurrences");
    let calls = related
        .iter()
        .filter(|r| r["related_relationship"] == "incoming call occurrence")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one authored call to each target");
    assert_eq!(calls[0]["source_context"]["text"], "alias(value)");
    for relationship in ["semantic reference occurrence", "import occurrence"] {
        assert!(
            related
                .iter()
                .any(|r| r["related_relationship"] == relationship)
        );
    }
    for row in related {
        verify_exact_source(fixture, row);
        assert_eq!(row["context_kind"], "related occurrence");
        assert_eq!(&row["related_subject_id"], subject);
        assert_ne!(row["public_entity_id"], row["related_subject_id"]);
        assert!(row["declaration_id"].is_null());
        assert!(row.get("source_bytes").is_none());
        assert!(row["related_resolution"].is_string());
    }
}

fn query(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    request: &Value,
) -> (Value, BTreeMap<String, Vec<Value>>) {
    let (report, mut rows) =
        super::super::block_queries::resource_query(fixture, stack, phase, request);
    // Disclosure scope and serving snapshot participate in this handle's identity. A new
    // authorized request may issue a different handle for identical canonical facts and bytes.
    for block in rows.values_mut() {
        for row in block {
            let Some(context) = row.get_mut("source_context") else {
                continue;
            };
            let context = context.as_object_mut().unwrap();
            let handle = context.remove("source_context_id").unwrap();
            codefabric::identity::decode_public_id(
                codefabric::identity::IdentityDomain::QuerySourceContext,
                None,
                handle.as_str().unwrap(),
            )
            .unwrap();
        }
    }
    (report, rows)
}
