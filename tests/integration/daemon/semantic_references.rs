use super::*;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one real checker publication and exact reopen verifies cross-file denotations and scoped unknowns"
)]
fn pragmatic_python_canonical_modules_imports_references_and_reopen() {
    let fixture = ProductionFixture::with_source(
        b"from b import chosen as alias, Service\nimport b\ndef chosen() -> str:\n    return 'local'\nvalue = alias()\nservice = Service()\nother = service.method()\nmodule_value = b.chosen()\nmissing()\n",
    );
    fs::write(
        Path::new(&fixture.workspace.root_path_display).join("b.py"),
        b"def chosen() -> int:\n    return 7\nclass Service:\n    def method(self) -> int:\n        return chosen()\n",
    ).unwrap();
    fs::write(
        Path::new(&fixture.workspace.root_path_display).join("broken.py"),
        b"from absent import orphan\norphan()\n",
    )
    .unwrap();
    let supervisor = fixture.start_supervisor();
    let modules = canonical_diagnostic_rows(&fixture, "fact.code_module");
    let target_module = modules.iter().find(|row| row["name"] == "b").unwrap();
    assert_eq!(target_module["entity_kind"], "module");
    assert!(!target_module["public_entity_id"].is_null());
    let declarations = canonical_diagnostic_rows(&fixture, "fact.code_declaration");
    let target = |name| {
        declarations
            .iter()
            .find(|row| row["name"] == name && row["file_id"] == target_module["file_id"])
            .unwrap()
    };
    let chosen = target("chosen");
    let method = target("method");
    assert_eq!(
        (chosen["start_byte"].as_u64(), chosen["end_byte"].as_u64()),
        (Some(4), Some(10))
    );
    assert_eq!(
        (method["start_byte"].as_u64(), method["end_byte"].as_u64()),
        (Some(57), Some(63))
    );
    let references = canonical_diagnostic_rows(&fixture, "fact.code_semantic_reference");
    let mut checked = BTreeSet::new();
    for row in &references {
        let name = row["name"].as_str().unwrap();
        let kind = row["reference_kind"].as_str().unwrap();
        let expected = match (name, kind) {
            ("chosen", "import") | ("alias" | "chosen", "read") => chosen,
            ("method", "read") => method,
            ("b", "import" | "read") => target_module,
            ("missing", "read") => {
                assert_eq!(row["resolution"], "unknown");
                assert_eq!(row["unknown_reason"], "checker_definition_unavailable");
                assert!(row["target_entity_id"].is_null());
                checked.insert((name, kind));
                continue;
            }
            _ => continue,
        };
        assert_eq!(row["target_entity_id"], expected["entity_id"], "{row}");
        assert_eq!(row["resolution"], "resolved", "{row}");
        assert_eq!(
            row["target_declaration_id"], expected["declaration_id"],
            "{row}"
        );
        assert!(!row["reference_id"].is_null());
        checked.insert((name, kind));
    }
    assert_eq!(
        checked,
        BTreeSet::from([
            ("chosen", "import"),
            ("alias", "read"),
            ("chosen", "read"),
            ("method", "read"),
            ("b", "import"),
            ("b", "read"),
            ("missing", "read"),
        ])
    );
    let imports = canonical_diagnostic_rows(&fixture, "fact.code_import");
    assert_eq!(imports.len(), 4);
    let sample = modules.iter().find(|row| row["name"] == "sample").unwrap();
    let broken = modules.iter().find(|row| row["name"] == "broken").unwrap();
    let unresolved = references
        .iter()
        .filter(|row| row["name"] == "orphan")
        .collect::<Vec<_>>();
    assert_eq!(unresolved.len(), 2);
    assert!(
        unresolved
            .iter()
            .all(|row| row["resolution"] == "unknown" && row["target_entity_id"].is_null()),
        "an editor landing fallback is not a denoted entity"
    );
    assert!(
        imports
            .iter()
            .filter(|row| row["file_id"] == sample["file_id"])
            .all(|row| row["resolution"] == "resolved"
                && !row["semantic_reference_id"].is_null()
                && row["join_method"] == "checker-name-within-ruff-alias"),
        "{imports:?}"
    );
    let alias = imports
        .iter()
        .find(|row| row["alias_name"] == "alias")
        .unwrap();
    assert_eq!(alias["target_entity_id"], chosen["entity_id"]);
    assert_eq!(
        (alias["start_byte"].as_u64(), alias["end_byte"].as_u64()),
        (Some(14), Some(29))
    );
    let coverage = canonical_diagnostic_rows(&fixture, "system.entity_processing_scope");
    assert!(imports.iter().any(|row| row["file_id"] == broken["file_id"]
        && row["resolution"] == "unknown"
        && row["target_entity_id"].is_null()));
    assert!(
        coverage
            .iter()
            .any(|row| row["file_id"] == broken["file_id"]
                && row["family"] == "imports"
                && row["processing_state"] == "partial")
    );
    for (family, state) in [
        ("modules", "complete"),
        ("imports", "complete"),
        ("semantic-references", "partial"),
    ] {
        assert!(
            coverage
                .iter()
                .any(|row| row["file_id"] == sample["file_id"]
                    && row["context_id"] == sample["context_id"]
                    && row["family"] == family
                    && row["processing_state"] == state),
            "{family}: {coverage:?}"
        );
    }
    let selected = wait_for_semantic_activation(&fixture);
    let entities = canonical_diagnostic_rows(&fixture, "fact.code_entity");
    assert!(
        entities
            .iter()
            .any(|row| row["entity_id"] == target_module["entity_id"])
    );
    supervisor.stop();
    let supervisor = fixture.start_supervisor();
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    for (relation, expected) in [
        ("fact.code_module", modules),
        ("fact.code_semantic_reference", references),
        ("fact.code_import", imports),
        ("system.entity_processing_scope", coverage),
    ] {
        assert_eq!(
            canonical_diagnostic_rows(&fixture, relation),
            expected,
            "{relation}"
        );
    }
    supervisor.stop();
}
