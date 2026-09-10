use super::*;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mixed native publication checks independent denotations, source positions, namespace semantics and exact reopen"
)]
fn pragmatic_rust_canonical_imports_references_and_reopen() {
    let fixture = ProductionFixture::with_source(b"def py_target():\n    pass\npy_target()\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"hir_references\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"hir_references\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = "mod inner;\nuse inner::{target as selected, Token};\npub use inner::target as exported;\nuse inner::*;\npub fn run(value: u8) -> u8 { let local = value; selected(local) + Token::new().method() }\n";
    fs::write(root.join("src/lib.rs"), source).unwrap();
    fs::write(root.join("src/inner.rs"), "pub struct Token;\nimpl Token { pub fn new() -> Self { Token } pub fn method(&self) -> u8 { 2 } }\npub fn target(value: u8) -> u8 { value }\n").unwrap();
    let supervisor = fixture.start_supervisor();
    let declarations = canonical_diagnostic_rows(&fixture, "fact.code_declaration");
    let target = |suffix: &str| {
        declarations
            .iter()
            .find(|row| {
                row["language"] == "rust"
                    && row["name"]
                        .as_str()
                        .is_some_and(|name| name.ends_with(suffix))
            })
            .unwrap()
    };
    let references = canonical_diagnostic_rows(&fixture, "fact.code_semantic_reference");
    let imports = canonical_diagnostic_rows(&fixture, "fact.code_import");
    assert!(references.iter().any(|row| row["language"] == "python"
        && row["name"] == "py_target"
        && row["resolution"] == "resolved"));
    assert!(
        references.iter().any(|row| row["language"] == "rust"),
        "missing Rust references; preparation={:?}",
        canonical_diagnostic_rows(&fixture, "system.rust_target_progress")
    );
    for (name, kind, suffix) in [
        ("selected", "value-path", "inner::target"),
        ("method", "method-call", "Token::method"),
        ("new", "associated-path", "Token::new"),
        ("target", "import", "inner::target"),
    ] {
        let selected = references
            .iter()
            .filter(|row| {
                row["language"] == "rust" && row["name"] == name && row["reference_kind"] == kind
            })
            .collect::<Vec<_>>();
        assert!(!selected.is_empty(), "missing {name}/{kind}");
        for row in selected {
            assert_eq!(
                row["target_entity_id"],
                target(suffix)["entity_id"],
                "{row}"
            );
            assert_eq!(
                row["target_declaration_id"],
                target(suffix)["declaration_id"]
            );
            assert_eq!(row["target_file_id"], target(suffix)["file_id"]);
            assert_eq!(row["context_id"], target(suffix)["context_id"]);
            assert_eq!(row["resolution"], "resolved", "{row}");
            assert!(!row["reference_id"].is_null());
            assert!(row["unknown_reason"].is_null());
        }
    }
    let method = references
        .iter()
        .find(|row| row["language"] == "rust" && row["reference_kind"] == "method-call")
        .unwrap();
    let start = source.find(".method()").unwrap() + 1;
    assert_eq!(
        (method["start_byte"].as_u64(), method["end_byte"].as_u64()),
        (Some(start as u64), Some(start as u64 + 6))
    );
    assert_eq!(
        method["target_native_definition_kind"],
        "associated-function"
    );
    assert_eq!(method["target_definition_kind"], "function");
    for alias in ["selected", "exported"] {
        let row = imports
            .iter()
            .find(|row| row["language"] == "rust" && row["alias_name"] == alias)
            .unwrap();
        assert_eq!(
            row["target_entity_id"],
            target("inner::target")["entity_id"]
        );
        assert_eq!(row["resolution"], "resolved", "{row}");
        assert_eq!(row["join_method"], "compiler-reference-ordinal");
        assert!(!row["import_id"].is_null());
        assert!(!row["semantic_reference_id"].is_null());
        assert!(row["syntax_observation_id"].is_null());
        assert!(row["syntax_provider_run_id"].is_null());
        assert_eq!(row["is_public"], alias == "exported");
    }
    let token = imports
        .iter()
        .filter(|row| row["language"] == "rust" && row["alias_name"] == "Token")
        .collect::<Vec<_>>();
    assert_eq!(token.len(), 2);
    assert_ne!(token[0]["target_namespace"], token[1]["target_namespace"]);
    assert_eq!(token[0]["import_id"], token[1]["import_id"]);
    assert!(references.iter().any(|row| row["language"] == "rust"
        && row["raw_resolution"] == "local-binding"
        && row["target_entity_id"].is_null()
        && row["resolution"] == "unknown"));
    assert!(imports.iter().any(|row| row["language"] == "rust"
        && row["import_id"].is_null()
        && row["unknown_reason"] == "compiler_import_location_unavailable"));
    let coverage = canonical_diagnostic_rows(&fixture, "system.entity_processing_scope");
    for family in ["imports", "semantic-references"] {
        assert!(coverage.iter().any(|row| row["language"] == "rust"
            && row["family"] == family
            && row["processing_state"] == "partial"));
    }
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor();
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    for (relation, expected) in [
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
