use super::block_queries::{block_rows, resource_bytes};
use super::*;

fn occurrence_id(row: &Value, field: &str, kind: &str) -> String {
    let value = row[field].as_str().unwrap();
    let id = (0..16)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    codefabric::identity::encode_public_id(
        codefabric::identity::IdentityDomain::Entity,
        Some(kind),
        id,
    )
    .unwrap()
}

#[allow(clippy::too_many_lines)]
fn relationships(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    let references = canonical_diagnostic_rows(fixture, "fact.code_semantic_reference");
    let imports = canonical_diagnostic_rows(fixture, "fact.code_import");
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
    let mut queries = Vec::new();
    for language in ["python", "rust"] {
        for (meaning, rows, occurrence, kind) in [
            (
                "semantic references",
                &references,
                "reference_id",
                "reference",
            ),
            ("imports", &imports, "import_id", "import-occurrence"),
        ] {
            let row = rows
                .iter()
                .find(|row| {
                    row["language"] == language
                        && row["target_entity_id"] == target(language)["entity_id"]
                        && !row[occurrence].is_null()
                })
                .unwrap();
            for direction in ["incoming", "outgoing"] {
                let subject = if direction == "incoming" {
                    target(language)["public_entity_id"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                } else {
                    occurrence_id(row, occurrence, kind)
                };
                queries.push(json!({"request":"follow code relationships", "query_id":format!("{language}-{}-{direction}", meaning.replace(' ', "-")),
                    "starting_from":[{"entity_id":subject},{"entity_id":subject}], "relationship":meaning, "direction":direction, "distance":"one step"}));
            }
        }
    }
    let missing = imports
        .iter()
        .find(|row| row["language"] == "python" && row["module_name"] == "missing_package")
        .unwrap();
    queries.push(json!({"request":"follow code relationships", "query_id":"unknown-import", "starting_from":[{"entity_id":occurrence_id(missing,"import_id","import-occurrence")}], "relationship":"imports", "direction":"outgoing", "distance":"one step"}));
    queries.push(json!({"request":"find code entities","query_id":"functions","looking_for":"Python function declarations"}));
    queries.push(json!({"request":"follow code relationships","query_id":"prior-references", "starting_from":[{"results_of":"functions","select":"entities"}], "relationship":"semantic references", "direction":"incoming", "distance":"one step"}));
    queries.push(json!({"request":"follow code relationships", "query_id":"empty", "starting_from":[{"entity_id":codefabric::identity::encode_public_id(codefabric::identity::IdentityDomain::Entity, Some("reference"), [0x33;16]).unwrap()}], "relationship":"semantic references", "direction":"outgoing", "distance":"one step"}));
    queries.push(json!({"request":"follow code relationships", "query_id":"limited", "starting_from":[{"entity_id":target("python")["public_entity_id"]}], "relationship":"semantic references", "direction":"incoming", "distance":"one step", "return":{"limit":{"maximum_results":1}}}));
    // Resolve an unknown family while retaining the incoming direction program.
    queries[2]["relationship"] = json!("show imported endpoints");
    let count = queries.len();
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:relationships-{phase}"),
        "unused",
    );
    request["queries"] = json!(queries);
    let mut steps = vec![
        json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    for page in 0..count {
        steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
    }
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([{"message":"input.selection-resolution.description","action":"accept","content":{"value":{"$requested_schema_presentation":"Imports"}}}]),
        json!(steps),
    );
    let path = write_modern_client_scenario(fixture, &format!("relationships-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED");
    assert_eq!(result["processing"].as_array().unwrap().len(), count);
    let processing = result["processing"].as_array().unwrap();
    let status = |query: &str| processing.iter().find(|p| p["query_id"] == query).unwrap();
    assert_eq!(status("limited")["additional_rows"], true);
    assert_eq!(status("unknown-import")["family"], "imports");
    assert!(
        status("unknown-import")["remaining_partitions"]
            .as_u64()
            .unwrap()
            > 0
    );
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let rows = |query: &str| {
        let binding = manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["query_id"] == query)
            .unwrap();
        let entry = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["relation_id"] == binding["relation_id"])
            .unwrap();
        assert_eq!(entry["page_count"], 1);
        block_rows(
            &report,
            usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap(),
        )
    };
    for language in ["python", "rust"] {
        for (meaning, source, occurrence, kind) in [
            (
                "semantic references",
                &references,
                "reference_id",
                "reference",
            ),
            ("imports", &imports, "import_id", "import-occurrence"),
        ] {
            for direction in ["incoming", "outgoing"] {
                let output = rows(&format!(
                    "{language}-{}-{direction}",
                    meaning.replace(' ', "-")
                ));
                assert!(!output.is_empty());
                if direction == "incoming" {
                    assert_eq!(
                        output.len(),
                        source
                            .iter()
                            .filter(
                                |row| row["target_entity_id"] == target(language)["entity_id"]
                                    && row["context_id"] == target(language)["context_id"]
                            )
                            .count()
                    );
                }
                for row in output {
                    assert_eq!(
                        row["public_target_entity_id"],
                        target(language)["public_entity_id"]
                    );
                    assert_eq!(row["public_source_entity_id"], row["public_occurrence_id"]);
                    assert_eq!(
                        row["public_occurrence_id"],
                        occurrence_id(&row, occurrence, kind)
                    );
                    assert_eq!(row["resolution"], "resolved");
                    assert!(row["unknown_reason"].is_null());
                    assert!(!row["start_byte"].is_null());
                    assert!(!row["end_byte"].is_null());
                }
            }
        }
    }
    assert!(rows("empty").is_empty());
    assert_eq!(rows("limited").len(), 1);
    let unknown = rows("unknown-import");
    assert_eq!(unknown.len(), 1);
    assert!(unknown[0]["public_target_entity_id"].is_null());
    assert!(unknown[0]["target_entity_kind"].is_null());
    assert!(!unknown[0]["unknown_reason"].is_null());
    assert_ne!(unknown[0]["resolution"], "resolved");
    assert_eq!(
        rows("prior-references"),
        rows("python-semantic-references-incoming")
    );
}

#[allow(clippy::too_many_lines)]
fn find_occurrences(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    for (language, label) in [("python", "Python"), ("rust", "Rust")] {
        let families = [
            (
                "calls",
                "call",
                "call occurrences",
                "call targets",
                "fact.code_call_site",
                "call_site_id",
                "call-targets",
            ),
            (
                "references",
                "reference",
                "semantic reference occurrences",
                "semantic references",
                "fact.code_semantic_reference",
                "reference_id",
                "semantic-references",
            ),
            (
                "imports",
                "import-occurrence",
                "import occurrences",
                "imports",
                "fact.code_import",
                "import_id",
                "imports",
            ),
        ];
        let mut queries = vec![];
        for (query, _, phrase, meaning, _, _, _) in families {
            queries.push(json!({"request":"find code entities","query_id":query,"looking_for":format!("{label} {phrase}")}));
            queries.push(json!({"request":"follow code relationships","query_id":format!("{query}-facts"),
                "starting_from":[{"results_of":query,"select":"entities"}], "relationship":meaning,"direction":"outgoing","distance":"one step"}));
            queries.push(json!({"request":"retrieve source and syntax context","query_id":format!("{query}-source"),
                "about":[{"results_of":query,"select":"entities"}], "context":"exact source span", "return":{"maximum_source_bytes":4096}}));
        }
        queries.push(json!({"request":"retrieve source and syntax context","query_id":"mixed-source",
            "about":[{"results_of":"calls","select":"entities"},{"results_of":"imports","select":"entities"}], "context":"surrounding lines", "return":{"maximum_source_bytes":4096,"source_lines_before":1,"source_lines_after":1}}));
        if language == "python" {
            queries.push(json!({"request":"find code entities","query_id":"modules","looking_for":"Python modules"}));
            queries.push(json!({"request":"retrieve source and syntax context","query_id":"modules-source",
                "about":[{"results_of":"modules","select":"entities"}], "context":"exact source span", "return":{"maximum_source_bytes":4096}}));
        }
        let count = queries.len();
        let mut request = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:occurrences-{language}-{phase}"),
            "unused",
        );
        request["scope"]["languages"] = json!([language]);
        request["queries"] = json!(queries);
        let mut steps = vec![
            json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
            json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
        ];
        for page in 0..count {
            steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
        }
        let scenario =
            modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
        let path = write_modern_client_scenario(
            fixture,
            &format!("occurrences-{language}-{phase}"),
            &scenario,
        );
        let report = modern_client_report(&run_modern_client(stack, &path));
        let result = modern_structured(modern_step(&report, "query"));
        assert_eq!(result["execution_state"], "SUCCEEDED");
        let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
        let rows = |query: &str| {
            let binding = manifest["canonical_semantic_response"]["queries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["query_id"] == query)
                .unwrap();
            let entry = manifest["relations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["relation_id"] == binding["relation_id"])
                .unwrap();
            assert_eq!(entry["page_count"], 1);
            block_rows(
                &report,
                usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap(),
            )
        };
        for (query, kind, _, _, relation, id, coverage) in families {
            let expected = canonical_diagnostic_rows(fixture, relation)
                .into_iter()
                .filter(|row| row["language"] == language && !row[id].is_null())
                .collect::<Vec<_>>();
            assert!(!expected.is_empty(), "{language}/{query}");
            let expected_ids = expected
                .iter()
                .map(|row| occurrence_id(row, id, kind))
                .collect::<BTreeSet<_>>();
            let entities = rows(query);
            let observed_ids = entities
                .iter()
                .map(|row| row["public_entity_id"].as_str().unwrap().to_owned())
                .collect::<BTreeSet<_>>();
            assert_eq!(observed_ids, expected_ids);
            assert_eq!(
                entities.len(),
                observed_ids.len(),
                "target candidates cannot duplicate an occurrence entity"
            );
            assert!(entities.iter().all(|row| row["entity_kind"] == kind));
            let status = result["processing"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["query_id"] == query)
                .unwrap();
            assert_eq!(status["family"], coverage);
            let sources = rows(&format!("{query}-source"));
            assert_eq!(
                sources.len(),
                expected_ids.len(),
                "one {language}/{query} source per occurrence"
            );
            let mut source_ids = BTreeSet::new();
            for source in &sources {
                verify_exact_source(fixture, source);
                assert!(source["declaration_id"].is_null());
                assert_eq!(source["source_mapping"], "exact-occurrence-span");
                source_ids.insert(source["public_entity_id"].as_str().unwrap().to_owned());
            }
            assert_eq!(source_ids, expected_ids);
            let source_status = result["processing"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["query_id"] == format!("{query}-source"))
                .unwrap();
            assert_eq!(source_status["family"], "source-context");
            assert_eq!(
                source_status["requested_partitions"],
                status["requested_partitions"]
            );
            assert_eq!(source_status["languages"], json!([language]));
            let facts = rows(&format!("{query}-facts"));
            assert_eq!(facts.len(), expected.len());
            let source_fields = expected[0]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut projected = facts
                .into_iter()
                .map(|row| {
                    serde_json::to_string(
                        &row.as_object()
                            .unwrap()
                            .iter()
                            .filter(|(key, _)| source_fields.contains(*key))
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect::<serde_json::Map<_, _>>(),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let mut expected = expected
                .iter()
                .map(|row| serde_json::to_string(row).unwrap())
                .collect::<Vec<_>>();
            projected.sort();
            expected.sort();
            assert_eq!(projected, expected, "{language}/{query}");
        }
        let mixed = rows("mixed-source");
        assert_eq!(
            mixed.len(),
            rows("calls-source").len() + rows("imports-source").len()
        );
        for source in &mixed {
            verify_exact_source(fixture, source);
        }
        let processing = result["processing"].as_array().unwrap();
        let status = |query| processing.iter().find(|p| p["query_id"] == query).unwrap();
        assert_eq!(
            status("mixed-source")["requested_partitions"]
                .as_u64()
                .unwrap(),
            status("calls-source")["requested_partitions"]
                .as_u64()
                .unwrap()
                + status("imports-source")["requested_partitions"]
                    .as_u64()
                    .unwrap()
        );
        let reasons = status("mixed-source")["remainder"].as_array().unwrap();
        if language == "python" {
            assert!(!reasons.is_empty());
        }
        assert!(
            reasons
                .iter()
                .all(|r| matches!(r["fact_family"].as_str(), Some("call-targets" | "imports")))
        );
        if language == "python" {
            let modules = rows("modules-source");
            assert_eq!(modules.len(), rows("modules").len());
            assert!(!modules.is_empty());
            for source in modules {
                verify_exact_source(fixture, &source);
                assert_eq!(source["source_mapping"], "exact-module-file");
                assert_eq!(source["start_byte"], 0);
                assert!(source["declaration_id"].is_null());
            }
        }
    }
}

#[allow(
    clippy::naive_bytecount,
    reason = "independent line oracle over a small authored fixture"
)]
fn verify_exact_source(fixture: &ProductionFixture, row: &Value) {
    let hex = row["relative_path"].as_str().unwrap();
    let path = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let source = fs::read(
        Path::new(&fixture.workspace.root_path_display).join(String::from_utf8(path).unwrap()),
    )
    .unwrap();
    let context = &row["source_context"];
    let start = usize::try_from(context["start_byte"].as_u64().unwrap()).unwrap();
    let end = usize::try_from(context["end_byte"].as_u64().unwrap()).unwrap();
    assert_eq!(
        context["text"],
        std::str::from_utf8(&source[start..end]).unwrap()
    );
    assert_eq!(context["complete"], true);
    assert_eq!(context["anchor_start_byte"], row["start_byte"]);
    assert_eq!(context["anchor_end_byte"], row["end_byte"]);
    assert_eq!(
        context["start_line"].as_u64().unwrap(),
        1 + source[..start].iter().filter(|b| **b == b'\n').count() as u64
    );
}

#[test]
fn pragmatic_semantic_relationships_through_installed_clients_and_reopen() {
    let fixture = ProductionFixture::with_source(b"from b import target as alias\nimport missing_package\ndef subject(value: int) -> int:\n    return alias(value)\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::write(
        root.join("b.py"),
        b"def target(value: int) -> int:\n    return value\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"public_relationships\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"public_relationships\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), "mod inner;\nuse inner::target as alias;\npub fn subject(value: u8) -> u8 { alias(value) }\n").unwrap();
    fs::write(
        root.join("src/inner.rs"),
        "pub fn target(value: u8) -> u8 { value }\n",
    )
    .unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    relationships(&fixture, &stack, "initial");
    find_occurrences(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    relationships(&fixture, &stack, "reopened");
    find_occurrences(&fixture, &stack, "reopened");
    supervisor.stop();
}
