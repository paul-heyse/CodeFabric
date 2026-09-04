use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::{Array, StringArray};
use codefabric::integrity::digest_bytes;
use codefabric::provider_contracts::{
    CancellationProbe, ContextIdentity, ProviderBuildIdentity, ProviderContextBinding,
    ProviderFamilyIdentity, ProviderFamilyRequest, ProviderIdentity, ProviderJob, ProviderJobSpec,
    ProviderLane, ProviderPolicyIdentity, ProviderProgramIdentity, ProviderProtocolIdentity,
    ProviderRelationIdentity, ProviderResourceCeilingSpec, ProviderResourceCeilings,
    ProviderRunBinding, ProviderRunIdentity, ProviderRunProvenance, ProviderSchemaIdentity,
    ProviderScopeIdentity, ProviderSourceBinding, ProviderTrustPosture, SourceIdentity,
    SuiteIdentity, admit_provider_result,
};
use codefabric::provider_native_syntax::{
    ExactPythonSyntaxRunner, InProcessProviderJobs, NativeSyntaxRelation,
    ProviderNativeSourceImage, PythonModuleInput,
};
use codefabric::provider_types::ProviderText;

fn lane(relation: NativeSyntaxRelation) -> ProviderLane {
    match relation {
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::TreeSitterCstNode
        | NativeSyntaxRelation::TreeSitterChangedRange
        | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => ProviderLane::TreeSitter,
        _ => ProviderLane::Ruff,
    }
}

fn ceilings() -> ProviderResourceCeilings {
    ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
        max_relations: 64,
        max_batches_per_relation: 8,
        max_input_bytes: 1 << 20,
        max_rows: 2_000_000,
        max_bytes: 1 << 28,
        max_diagnostics: 10_000,
        max_work_units: 10_000_000,
        max_wall_millis: 30_000,
        max_visited_nodes: 2_000_000,
        max_traversal_depth: 256,
        max_workers: 4,
        max_retained_revisions: 2,
        cancellation_poll_work_units: 64,
        cancellation_ack_millis: 2_000,
    })
    .unwrap()
}

fn requests(target: ProviderLane) -> Vec<ProviderFamilyRequest> {
    NativeSyntaxRelation::ALL
        .into_iter()
        .filter(|relation| lane(*relation) == target)
        .map(|relation| {
            ProviderFamilyRequest::try_new(
                ProviderFamilyIdentity::try_new(format!("family.{}", relation.as_str())).unwrap(),
                ProviderRelationIdentity::try_new(relation.as_str()).unwrap(),
                ProviderSchemaIdentity::try_new(format!("schema.{}", relation.as_str())).unwrap(),
                relation.schema(),
                ProviderScopeIdentity::try_new("integration.source").unwrap(),
                1,
            )
            .unwrap()
        })
        .collect()
}

fn job(
    target: ProviderLane,
    source: &ProviderNativeSourceImage,
    context: &ProviderContextBinding,
    pin: u8,
) -> ProviderJob {
    let (_, cancellation) = CancellationProbe::pair(64).unwrap();
    let provider = match target {
        ProviderLane::TreeSitter => "tree-sitter-python",
        ProviderLane::Ruff => "ruff-python",
        _ => unreachable!(),
    };
    ProviderJob::try_new(ProviderJobSpec {
        suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3.0").unwrap(),
        provider: ProviderIdentity::try_new(provider).unwrap(),
        protocol: ProviderProtocolIdentity::try_new("in-process-arrow@1").unwrap(),
        source: ProviderSourceBinding::try_file(
            SourceIdentity::try_new("integration.source").unwrap(),
            [6; 16],
            source.file_id,
            source.source_generation,
            source.content_digest,
        )
        .unwrap(),
        context: context.clone(),
        run: ProviderRunBinding::try_new(
            ProviderRunIdentity::try_new(format!("{provider}.integration-run")).unwrap(),
            [pin; 16],
        )
        .unwrap(),
        lane: target,
        trust: ProviderTrustPosture::InProcessConstrained,
        requests: requests(target),
        ceilings: ceilings(),
        deadline: Instant::now() + Duration::from_secs(30),
        cancellation,
        provenance: ProviderRunProvenance::new(
            ProviderBuildIdentity::try_new(format!("{provider}.pinned-build")).unwrap(),
            ProviderPolicyIdentity::try_new("integration.policy").unwrap(),
            ProviderProgramIdentity::try_new("integration.provider-program").unwrap(),
        ),
    })
    .unwrap()
}

#[test]
fn isolated_fact_generation_executes_provider_jobs_to_arrow() {
    let text = "from package import value\nresult = value + 1\n";
    let bytes = Arc::<[u8]>::from(text.as_bytes());
    let source = ProviderNativeSourceImage::new(
        [7; 16],
        11,
        Arc::clone(&bytes),
        digest_bytes(&bytes),
        ProviderText {
            text: Arc::from(text),
            original_byte_offsets: Arc::from(
                text.char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap())
                    .chain(std::iter::once(u64::try_from(text.len()).unwrap()))
                    .collect::<Vec<_>>(),
            ),
        },
    )
    .unwrap();
    let context = ProviderContextBinding::try_new(
        ContextIdentity::try_new("integration.context").unwrap(),
        [9; 16],
        [9; 32],
        [10; 32],
    )
    .unwrap()
    .with_python_version(3, 14)
    .unwrap()
    .with_modules(vec![
        codefabric::provider_contracts::ProviderModuleBinding {
            file_id: source.file_id,
            qualified_name: "integration.module".to_owned(),
            relative_path: b"integration/module.py".to_vec(),
        },
    ])
    .unwrap();
    let tree_job = job(ProviderLane::TreeSitter, &source, &context, 17);
    let ruff_job = job(ProviderLane::Ruff, &source, &context, 18);

    let mut runner = ExactPythonSyntaxRunner::new().unwrap();
    let result = runner
        .run_full(
            InProcessProviderJobs::try_new(&tree_job, &ruff_job).unwrap(),
            11,
            &source,
            PythonModuleInput {
                module_name: "integration.module",
                module_path: Path::new("integration/module.py"),
            },
        )
        .unwrap();

    let cst = result.relation(NativeSyntaxRelation::TreeSitterCstNode);
    let raw_kind_index = cst.schema().index_of("raw_kind").unwrap();
    let raw_kinds = cst
        .column(raw_kind_index)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert!(!raw_kinds.is_empty());
    assert!(!raw_kinds.value(0).is_empty());
    assert!(cst.schema().index_of("normalized_kind_code").is_ok());
    assert!(
        result
            .relation(NativeSyntaxRelation::RuffAstNode)
            .schema()
            .index_of("normalized_kind_code")
            .is_ok()
    );

    let tree = admit_provider_result(tree_job, result.tree_sitter_result().clone()).unwrap();
    let ruff = admit_provider_result(ruff_job, result.ruff_result().clone()).unwrap();
    assert_eq!(tree.observation().emitted_relations, 6);
    assert_eq!(ruff.observation().emitted_relations, 19);
    assert_eq!(tree.result().coverage().len(), 6);
    assert_eq!(ruff.result().coverage().len(), 19);
    assert!(tree.result().gaps().is_empty());
    assert!(ruff.result().gaps().is_empty());
}
