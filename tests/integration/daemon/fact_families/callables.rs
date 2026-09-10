use super::super::block_queries::resource_bytes;
use super::*;
use codefabric::identity::{
    CbefField, CbefValue, StringNormalization, TypeConstructor, TypeInterner, TypeTerm,
};
use std::fmt::Write as _;

pub(super) fn primitive(owner: &Value, language: &str) -> String {
    let text = |value: &str| CbefValue::Utf8 {
        value: value.to_owned(),
        normalization: StringNormalization::None,
    };
    let decode = |name: &str| {
        let value = owner[name].as_str().unwrap();
        let bytes = (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        <[u8; 16]>::try_from(bytes).unwrap()
    };
    let scalar = if language == "python" {
        CbefValue::TaggedUnion {
            variant: 1,
            value: Box::new(text("int")),
        }
    } else {
        text("u8")
    };
    let identity = TypeInterner::default()
        .intern_type(
            decode("workspace_id"),
            decode("context_id"),
            &TypeTerm {
                constructor: TypeConstructor::Primitive,
                fields: vec![
                    CbefField {
                        tag: 1,
                        value: text(language),
                    },
                    CbefField {
                        tag: 2,
                        value: scalar,
                    },
                ],
            },
        )
        .unwrap();
    let mut encoded = String::with_capacity(32);
    for byte in identity.type_id {
        write!(encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn is_named(row: &Value, language: &str, expected: &str) -> bool {
    row["language"] == language
        && row["name"].as_str().is_some_and(|name| {
            name == expected || (language == "rust" && name.rsplit("::").next() == Some(expected))
        })
}

pub(super) fn assert_facts(fixture: &ProductionFixture, rows: &[Value]) {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    for (language, name) in [
        ("python", "target"),
        ("rust", "target"),
        ("python", "subject"),
    ] {
        let owner = declarations
            .iter()
            .find(|row| is_named(row, language, name))
            .unwrap();
        let types = rows
            .iter()
            .filter(|row| {
                row["owner_entity_id"] == owner["entity_id"]
                    && row["context_id"] == owner["context_id"]
            })
            .collect::<Vec<_>>();
        let expected = primitive(owner, language);
        for role in ["parameter", "return"] {
            let matches = types
                .iter()
                .filter(|row| row["type_role"] == role)
                .collect::<Vec<_>>();
            assert!(!matches.is_empty(), "{language} {role}: {types:?}");
            assert!(
                matches.iter().all(|row| row["type_id"] == expected),
                "{matches:?}"
            );
            assert!(matches.iter().all(|row| row["component_ordinal"] == 0));
            if role == "parameter" {
                assert!(matches.iter().all(|row| row["parameter_required"] == true));
                if language == "python" {
                    assert!(matches.iter().all(|row| row["parameter_name"] == "value"
                        && row["parameter_kind"] == "positional-or-keyword"));
                }
            }
        }
        assert!(types.iter().all(|row| row["evidence_kind"]
            == if language == "python" {
                "checker-function-type"
            } else {
                "compiler-mir-slot"
            }));
    }
    // No emitted callable evidence does not assert an empty signature for any canonical function.
    for owner in declarations
        .iter()
        .filter(|row| row["entity_kind"] == "function")
    {
        assert!(
            rows.iter()
                .any(|row| row["owner_entity_id"] == owner["entity_id"]
                    && row["context_id"] == owner["context_id"])
        );
    }
}

pub(super) fn scoped(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Vec<Value>> {
    let mut all = Vec::new();
    for (language, name) in [
        ("Python", "target"),
        ("Rust", "target"),
        ("Python", "subject"),
    ] {
        let mut request = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:callable-{language}-{name}-{phase}"),
            &format!("{language} function named `{name}`"),
        );
        let producer = request["queries"][0]["query_id"].clone();
        request["queries"].as_array_mut().unwrap().push(json!({
            "request":"retrieve facts about code", "query_id":"signature",
            "about":[{"results_of":producer,"select":"entities"}],
            "facts":["parameter and return type observations"]
        }));
        let scenario = modern_client_scenario(
            fixture,
            stack,
            "policy-one",
            json!([]),
            json!([
                {"id":"query", "operation":"call_tool", "name":"query_code_graph", "arguments":{"request":request,"delivery":"resource"}},
                {"id":"manifest", "operation":"read_resource", "uri":{"$ref":"query.structured_content.manifest.uri"}}
            ]),
        );
        let path = write_modern_client_scenario(
            fixture,
            &format!("callable-{language}-{name}-{phase}"),
            &scenario,
        );
        let report = modern_client_report(&run_modern_client(stack, &path));
        let result = modern_structured(modern_step(&report, "query"));
        assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
        let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
        let binding = manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["query_id"] == "signature")
            .unwrap();
        let relation = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["relation_id"] == binding["relation_id"])
            .unwrap();
        let start = usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(relation["page_count"].as_u64().unwrap()).unwrap();
        let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(
            (start..start + count).map(|page| json!({"id":format!("p{page}"), "operation":"read_resource", "uri":result["pages"][page]["uri"]})).collect::<Vec<_>>()
        ));
        let path = write_modern_client_scenario(
            fixture,
            &format!("callable-pages-{language}-{name}-{phase}"),
            &scenario,
        );
        let report = modern_client_report(&run_modern_client(stack, &path));
        let mut rows = (start..start + count)
            .flat_map(|page| page_rows(&report, &format!("p{page}")))
            .collect::<Vec<_>>();
        rows.sort_by_cached_key(Value::to_string);
        let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
        let owner = declarations
            .iter()
            .find(|row| is_named(row, &language.to_ascii_lowercase(), name))
            .unwrap();
        assert!(rows.len() >= 2, "{rows:?}");
        assert!(
            rows.iter()
                .all(|row| row["owner_entity_id"] == owner["entity_id"])
        );
        all.push(rows);
    }
    all
}
