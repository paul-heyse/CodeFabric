use super::*;
use arrow_schema::{Field, Schema};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one snapshot checks class ID encoding, exact family ownership and legacy broad fallback"
)]
fn member_owner_selection_keeps_class_ids_family_isolation_and_broad_fallback() {
    let mut snapshot = super::tests::fixture();
    let original = snapshot.batches[0].slice(0, 1);
    let mut fields = original.schema().fields().to_vec();
    fields.push(Arc::new(Field::new(
        "owner_entity_id",
        arrow_schema::DataType::FixedSizeBinary(16),
        true,
    )));
    let schema = Arc::new(Schema::new(fields));
    let row = |family: &str, owner: Option<&[u8; 16]>, state: &str| {
        let mut columns = original.columns().to_vec();
        let (language, kind) = if family == "members" {
            ("python", "member_owner")
        } else {
            ("rust", "call_owner")
        };
        for (name, value) in [
            ("family", family),
            ("language", language),
            (
                "scope_kind",
                if owner.is_some() { kind } else { "source_file" },
            ),
            ("processing_state", state),
            (
                "reason",
                if state == "complete" {
                    ""
                } else {
                    "unknown_member_type"
                },
            ),
        ] {
            columns[original.schema().index_of(name).unwrap()] =
                Arc::new(StringArray::from(vec![value]));
        }
        columns.push(crate::fabric::id16_array([owner]));
        let batch = RecordBatch::try_new(Arc::clone(&schema), columns).unwrap();
        validate(&batch, [1; 16], 3).unwrap();
        ChargedValue::for_test(batch)
    };
    snapshot.batches = vec![
        row("members", None, "partial"),
        row("members", Some(&[10; 16]), "complete"),
        row("members", Some(&[11; 16]), "partial"),
        row("call-targets", Some(&[12; 16]), "complete"),
    ];
    let scope = || EntityQueryScope {
        boundaries: SourceBoundaries::default(),
        families: BTreeSet::new(),
        family: "members",
        languages: ["python".to_owned(), "rust".to_owned()].into(),
        contexts: BTreeSet::new(),
        owners: None,
    };
    let clause = |ids: &[u8]| {
        serde_json::from_value::<crate::semantic_query_contract::SemanticQueryClause>(serde_json::json!({
        "request":"retrieve facts about code", "query_id":"q", "facts":["associated member observations"],
        "about":ids.iter().map(|id| serde_json::json!({"entity_id":crate::identity::encode_public_id(
            crate::identity::IdentityDomain::Entity, Some("class"), [*id; 16]).unwrap()})).collect::<Vec<_>>()
    })).unwrap()
    };
    let mut selected = scope();
    snapshot
        .select_subject_owners(&mut selected, &clause(&[10, 10]))
        .unwrap();
    let summary = snapshot.summarize(&selected, 0);
    assert_eq!(
        (summary.requested_partitions, summary.remaining_partitions),
        (1, 0)
    );
    assert_eq!(summary.scope, "selected_python_member_owners");
    let mut selected = scope();
    snapshot
        .select_subject_owners(&mut selected, &clause(&[10, 11]))
        .unwrap();
    let summary = snapshot.summarize(&selected, 0);
    assert_eq!(
        (summary.requested_partitions, summary.remaining_partitions),
        (2, 1)
    );
    assert_eq!(
        summary.remainder[0].entity_id.as_deref(),
        Some(
            crate::identity::encode_public_id(
                crate::identity::IdentityDomain::Entity,
                Some("class"),
                [11; 16]
            )
            .unwrap()
            .as_str()
        )
    );
    let result = QueryProcessing {
        query_id: "q".into(),
        processing: summary,
        maximum_rows: None,
        additional_rows: None,
        selection: None,
    };
    assert!(result.validate());
    for ids in [&[12][..], &[10, 99][..]] {
        let mut fallback = scope();
        snapshot
            .select_subject_owners(&mut fallback, &clause(ids))
            .unwrap();
        assert!(
            fallback.owners.is_none(),
            "another family's owner is not evidence of member coverage"
        );
        let summary = snapshot.summarize(&fallback, 0);
        assert_eq!(
            (summary.requested_partitions, summary.remaining_partitions),
            (1, 1)
        );
    }
}
