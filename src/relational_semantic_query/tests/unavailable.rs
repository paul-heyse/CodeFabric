use super::*;

fn failed(query: &str, producer: Option<&str>) -> EpochBoundUnavailableBlockRow {
    EpochBoundUnavailableBlockRow {
        query_id: Arc::from(query),
        compatibility_form: ReleasedSemanticForm::FindCodeEntities,
        disposition: if producer.is_some() {
            SemanticBlockDisposition::NotExecutedDependency
        } else {
            SemanticBlockDisposition::SemanticUnavailable
        },
        issues: vec![SemanticCompilationIssue {
            code: if producer.is_some() {
                "NOT_EXECUTED_DEPENDENCY"
            } else {
                "SEMANTIC_REFERENCE_UNAVAILABLE"
            },
            subject_id: Arc::from(query),
            related_id: producer.map(Arc::from),
        }],
    }
}

#[test]
fn unavailable_ingress_compiles_without_fabricated_bindings_or_request_inputs() {
    let mut request = epoch_ingress();
    request.blocks.clear();
    request.selections.clear();
    request.returns.clear();
    request.request_inputs.clear();
    request.dependencies.clear();
    request.dependency_order.clear();
    request.unavailable_blocks = vec![failed("dependent", Some("root")), failed("root", None)];
    let validated =
        validate_epoch_bound_semantic_ingress(request, &epoch_ingress_catalog()).unwrap();
    assert_eq!(validated.consumption().blocks, 2);
    assert!(validated.execution_programs().is_empty());
    let compiled = compile_epoch_bound_semantic_request(
        &validated,
        &epoch_execution_catalog(),
        &epoch_runtime_closure(),
    )
    .unwrap();
    assert!(compiled.handoff().request_inputs.is_empty());
    assert!(compiled.handoff().prior_results.is_empty());
    let blocks = compiled.compiled().blocks();
    assert_eq!(
        blocks
            .iter()
            .map(|row| row.query_id().as_ref())
            .collect::<Vec<_>>(),
        ["root", "dependent"]
    );
    assert!(blocks.iter().all(|row| row.output().is_none()));
    assert_eq!(blocks[1].issues()[0].related_id.as_deref(), Some("root"));
}

#[test]
fn unavailable_ingress_rejects_duplicate_active_ids_unknown_dependencies_and_cycles() {
    for rows in [
        vec![failed("query-entities", None)],
        vec![failed("a", None), failed("a", None)],
        vec![failed("a", Some("query-entities"))],
        vec![failed("a", Some("a"))],
        vec![failed("a", Some("b")), failed("b", Some("a"))],
    ] {
        let mut request = epoch_ingress();
        request.unavailable_blocks = rows;
        assert!(validate_epoch_bound_semantic_ingress(request, &epoch_ingress_catalog()).is_err());
    }
    let mut request = epoch_ingress();
    request.unavailable_blocks = vec![failed("a", None)];
    request.unavailable_blocks[0].disposition = SemanticBlockDisposition::Compiled;
    assert!(validate_epoch_bound_semantic_ingress(request, &epoch_ingress_catalog()).is_err());

    let mut request = epoch_ingress();
    request.unavailable_blocks = (0..request.limits.compiler().max_blocks())
        .map(|index| failed(&format!("unavailable-{index}"), None))
        .collect();
    assert!(matches!(
        validate_epoch_bound_semantic_ingress(request, &epoch_ingress_catalog()),
        Err(EpochBoundSemanticIngressError::Limit {
            limit: "max_blocks",
            ..
        })
    ));
}
