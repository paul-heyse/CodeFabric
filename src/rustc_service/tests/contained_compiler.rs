//! Real Cargo, compiler callback, Arrow and containment in one selected-context run.

use super::*;
use crate::analysis_context::rust_context::{
    RustContextDiscoveryOutcome, RustContextDiscoveryRequest, RustContextSelection,
    discover_rust_context,
};
use crate::analysis_context::{
    ContextArtifactInput, ContextFileInput, ContextSearchRoot, ContextSearchScope,
    ContextSearchUniverse, RustTargetKind, RustTargetSettings, RustToolchainSettings,
};
use crate::identity::{IdentityDomain, decode_public_id, encode_public_id};
use crate::rust_compilation_trust::{
    RustCompilationTerminalState, SelectedRustCompilationPreparation,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contained_cargo_extracts_real_selected_rust_call() {
    contained_cargo_observations(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contained_cargo_retains_structured_diagnostics_without_mir() {
    contained_cargo_observations(true).await;
}

async fn contained_cargo_observations(compile_failure: bool) {
    let mut harness = lifecycle_harness();
    harness.protocol_policy.supported_feature_bits =
        crate::rustc_relation_schema::RUSTC_INVOCATION_CENSUS_FEATURE;
    let workspace = harness.inputs.workspace_view.clone();
    let dependencies = harness.inputs.dependency_view.clone();
    fs::create_dir(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub mod other;\npub fn caller() -> u32 { other::caller() }\n",
    )
    .unwrap();
    fs::write(
        workspace.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = 'fixture'\nversion = '0.0.0'\n",
    )
    .unwrap();

    let diagnostic_source = if compile_failure {
        "\u{feff}// original CRLF bytes\r\npub fn target(v: u32) -> u32 { v + 1 }\r\npub fn caller() -> u32 { missing_function(4) }\r\n"
    } else {
        "\u{feff}// original CRLF bytes\r\npub fn target(v: u32) -> u32 { v + 1 }\r\npub fn caller() -> u32 { target(4) }\r\npub fn Uppercase() {}\r\n"
    };
    fs::write(workspace.join("src/other.rs"), diagnostic_source).unwrap();

    let sysroot = std::process::Command::new("rustup")
        .args(["run", "nightly-2026-08-18", "rustc", "--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let sysroot = PathBuf::from(String::from_utf8(sysroot.stdout).unwrap().trim());
    let compiler_version = std::process::Command::new(sysroot.join("bin/rustc"))
        .arg("-vV")
        .output()
        .unwrap();
    assert!(compiler_version.status.success());
    let compiler_version = String::from_utf8(compiler_version.stdout).unwrap();
    let host = compiler_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();
    // A private read-only dependency bundle avoids exposing the host Rustup or Cargo homes.
    let copy = std::process::Command::new("cp")
        .args(["-a", "--reflink=auto"])
        .arg(sysroot.join("lib"))
        .arg(dependencies.join("toolchain/lib"))
        .status()
        .unwrap();
    assert!(copy.success());
    for executable in ["cargo", "rustc"] {
        fs::copy(
            sysroot.join("bin").join(executable),
            dependencies.join("toolchain/bin").join(executable),
        )
        .unwrap();
    }
    fs::create_dir(dependencies.join("extractor")).unwrap();
    let extractor = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/extractor/debug/codefabric-rustc-extractor");
    fs::copy(
        &extractor,
        dependencies.join("extractor/codefabric-rustc-extractor"),
    )
    .expect("build the dated-nightly extractor with just extractor-identity");
    fs::write(&dependencies.join("toolchain/bin/codefabric-rustc-extractor"),
        "#!/bin/sh\nexport LD_LIBRARY_PATH=/dependencies/toolchain/lib\nexec /dependencies/extractor/codefabric-rustc-extractor \"$@\"\n").unwrap();
    let identity = include_bytes!("../../../rustc-extractor/toolchain-identity.json");
    let identity_digest = digest(identity);
    harness.inputs = RustCompilationInputs::inspect(
        &workspace,
        &dependencies,
        &dependencies.join("toolchain/bin/cargo"),
        &dependencies.join("toolchain/bin/rustc"),
        &dependencies.join("toolchain/bin/codefabric-rustc-extractor"),
        &harness.inputs.source_snapshot_digest,
        &harness.inputs.dependency_snapshot_digest,
        &identity_digest,
        "rustc-1.100.0-nightly-2026-08-18",
    )
    .unwrap();
    let files: Vec<_> = ["Cargo.toml", "Cargo.lock", "src/lib.rs", "src/other.rs"]
        .into_iter()
        .map(|path| {
            let contents = fs::read(workspace.join(path)).unwrap();
            ContextFileInput {
                file_id: encode_public_id(
                    IdentityDomain::SourceFile,
                    None,
                    crate::integrity::digest_bytes(path.as_bytes())[..16]
                        .try_into()
                        .unwrap(),
                )
                .unwrap(),
                relative_path: path.as_bytes().to_vec(),
                digest: crate::integrity::digest_bytes(&contents),
                contents,
            }
        })
        .collect();
    let source_manifest = crate::rustc_source_files::RustSourceFileManifest {
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, [1; 16]).unwrap(),
        source_generation: 7,
        files: files
            .iter()
            .map(|file| {
                (
                    file.relative_path.clone(),
                    crate::rustc_source_files::CapturedRustSourceFile {
                        file_id: file.file_id.clone(),
                        content_digest: file.digest,
                    },
                )
            })
            .collect(),
    };
    let source_manifest_path = dependencies.join("source-files.json");
    fs::write(
        &source_manifest_path,
        serde_json::to_vec(&source_manifest).unwrap(),
    )
    .unwrap();
    let discovered = discover_rust_context(&RustContextDiscoveryRequest {
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, [1; 16]).unwrap(),
        source_generation: 7,
        provider_bundle_version: "rust-contained-fixture".into(),
        files,
        search_scope: ContextSearchScope {
            namespace: b".".to_vec(),
            ordered_roots: vec![ContextSearchRoot {
                root_id: "root".into(),
                relative_path: b".".to_vec(),
            }],
            policy_identity: [6; 32],
            universe: ContextSearchUniverse::Closed {
                inventory_identity: [7; 32],
            },
        },
        selection: RustContextSelection {
            target: Some(RustTargetSettings {
                name: "fixture".into(),
                kind: RustTargetKind::Library,
                crate_root: b"src/lib.rs".to_vec(),
            }),
            default_features: true,
            target_triple: Some(host.into()),
            profile: Some("dev".into()),
            toolchain: Some(RustToolchainSettings {
                release: harness.inputs.exact_toolchain_release.clone(),
                commit_hash: harness.protocol_policy.rustc_commit.clone(),
                artifact_digest: crate::identity::decode_b3_digest(&identity_digest).unwrap(),
            }),
            sysroot: Some(ContextArtifactInput {
                file_id: "sysroot:selected".into(),
                digest: crate::integrity::digest_bytes(identity),
            }),
            dependency_inputs: Some(Vec::new()),
            build_inputs: Some(Vec::new()),
            ..RustContextSelection::default()
        },
    })
    .unwrap();
    let RustContextDiscoveryOutcome::Prepared(product) = discovered else {
        panic!("selected context missing: {discovered:?}");
    };
    harness.request.preparation =
        SelectedRustCompilationPreparation::from_discovered(&product).unwrap();
    harness.request.build_scripts_present = false;
    harness.request.procedural_macros_present = false;
    harness.request.context.workspace_id = product.context.workspace_id.clone();
    harness.request.context.analysis_context_id = product.context.analysis_context_id.clone();
    harness.request.context.context_manifest_digest = product.context.context_fingerprint.clone();
    harness.request.context.cargo_lock_digest =
        digest(&fs::read(workspace.join("Cargo.lock")).unwrap());
    harness.request.context.cargo_config_digest = digest(b"");
    harness.capabilities = SandboxCapabilityMatrix::probe_current_host();
    let metadata_paths =
        RustCompilationPrivatePaths::prepare(harness.paths.run_root.parent().unwrap(), "metadata")
            .unwrap();
    let metadata_profile = GeneratedSandboxProfile::generate(
        ProviderTrustProfile::UntrustedSandboxed,
        SandboxMechanism::LinuxBubblewrap,
        &workspace,
        &dependencies,
        &metadata_paths.run_root,
    )
    .unwrap();
    let metadata_plan = crate::rust_compilation_trust::compile_rust_metadata_launch_plan(
        &harness.trust_policy,
        &harness.capabilities,
        &metadata_profile,
        &harness.inputs,
        &metadata_paths,
        &harness.request,
    )
    .unwrap();
    assert!(metadata_plan.protocol_binding().is_err());
    let metadata_capabilities = harness.capabilities.clone();
    let metadata = tokio::task::spawn_blocking(move || {
        let seccomp = crate::provider_sandbox::CompiledProviderSeccomp::compile().unwrap();
        let receipt = supervise_rust_compilation(
            &metadata_plan,
            &metadata_paths,
            &ProviderSandboxLauncher::new(metadata_capabilities),
            &metadata_profile,
            ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
            &RustCompilationCancellationSignal::default(),
        )
        .unwrap();
        assert_eq!(
            receipt.terminal().terminal_state,
            RustCompilationTerminalState::Succeeded,
            "{}",
            fs::read_to_string(&metadata_paths.stderr_path).unwrap()
        );
        let metadata = metadata_plan
            .read_metadata(&metadata_paths, &receipt)
            .unwrap();
        fs::write(&metadata_paths.stdout_path, b"substituted metadata").unwrap();
        assert!(
            metadata_plan
                .read_metadata(&metadata_paths, &receipt)
                .is_err()
        );
        metadata
    })
    .await
    .unwrap();
    let metadata_json: serde_json::Value = serde_json::from_slice(&metadata).unwrap();
    assert_eq!(metadata_json["workspace_root"], "/workspace");
    assert!(metadata_json["resolve"]["nodes"].is_array());
    harness.request.context.cargo_metadata_digest = digest(&metadata);
    harness.request.preparation = harness
        .request
        .preparation
        .with_cargo_metadata(&harness.inputs, &metadata)
        .unwrap()
        .with_source_file_manifest(&harness.inputs, &source_manifest_path)
        .unwrap();
    harness.admission.workspace_id = product.context.workspace_id.clone();
    harness.admission.analysis_context_id = product.context.analysis_context_id.clone();
    harness.admission.canonical_analysis_context_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &product.context.analysis_context_id,
    )
    .unwrap();
    harness.admission.context_manifest_digest = product.context.context_fingerprint.clone();
    harness.capabilities = SandboxCapabilityMatrix::probe_current_host();
    harness.profile = GeneratedSandboxProfile::generate(
        ProviderTrustProfile::UntrustedSandboxed,
        SandboxMechanism::LinuxBubblewrap,
        &workspace,
        &dependencies,
        &harness.paths.run_root,
    )
    .unwrap();
    harness.protocol_policy.sandbox_profile_digest = harness.profile.sha256_digest.clone();
    harness.protocol_policy.toolchain_identity_digest = identity_digest;
    harness.protocol_policy.provider_deadline_unix_ms = now_millis() + 120_000;
    harness.trust_policy.limits = RustCompilationResourceLimits {
        cpu_workers: 2,
        wall_time_millis: 120_000,
        stdout_bytes: 64 * 1024 * 1024,
        stderr_bytes: 64 * 1024 * 1024,
        artifact_bytes: 1024 * 1024 * 1024,
        process_count: 256,
        file_count: 100_000,
        single_file_bytes: 1024 * 1024 * 1024,
        cpu_seconds: 600,
        memory_bytes: 16 * 1024 * 1024 * 1024,
        open_files: 4096,
    };
    let seccomp = crate::provider_sandbox::CompiledProviderSeccomp::compile().unwrap();
    let job = provider_job(
        &harness.protocol_policy,
        &harness.admission,
        &RustcRelation::ALL,
    );
    let result = run_untrusted_rustc_provider_lifecycle(UntrustedRustcProviderLifecycle {
        cpu_lease: None,
        task_scope: task_scope(),
        provider_job: &job,
        trust_policy: &harness.trust_policy,
        sandbox_capabilities: &harness.capabilities,
        sandbox_profile: &harness.profile,
        compilation_inputs: &harness.inputs,
        private_paths: &harness.paths,
        compilation_request: &harness.request,
        protocol_policy: harness.protocol_policy.clone(),
        run_admission: harness.admission.clone(),
        allowed_uid: harness.allowed_uid,
        launch_material: ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
    })
    .await;
    let stderr = fs::read_to_string(&harness.paths.stderr_path).unwrap_or_default();
    let result = result.unwrap_or_else(|error| panic!("{error:?}\n{stderr}"));
    assert!(
        !result.compilations().is_empty(),
        "{:?}\n{stderr}",
        result.result().gaps().first()
    );
    assert_native_diagnostic_details(&result, diagnostic_source, compile_failure);
    let cargo = result
        .cargo_output()
        .expect("joined native Cargo JSON census");
    assert_eq!(cargo.succeeded, !compile_failure);
    assert!(cargo.artifacts.iter().all(|artifact| !artifact.fresh));
    assert!(cargo.reservation().amounts().memory_bytes > 0);
    for compilation in result.compilations() {
        let census = compilation
            .accepted()
            .invocation
            .as_ref()
            .expect("negotiated native invocation census");
        assert!(census.compiler_path.ends_with(b"toolchain/bin/rustc"));
        assert_eq!(census.source_path, b"src/lib.rs");
        assert_eq!(census.working_directory, b"/workspace");
        assert_eq!(
            census.source_content_digest,
            digest(b"pub mod other;\npub fn caller() -> u32 { other::caller() }\n")
        );
        assert!(
            census
                .arguments
                .iter()
                .any(|argument| argument == b"--crate-name")
        );
        assert!(census.reservation().amounts().memory_bytes > 0);
    }
    if compile_failure {
        assert_eq!(result.result().terminal(), ProviderTerminalStatus::Failed);
        assert!(
            result
                .result()
                .coverage()
                .iter()
                .all(|coverage| matches!(coverage.state(), ProviderCoverageState::Unknown { .. }))
        );
        assert!(result.compilations().iter().all(|compilation| {
            compilation.trust_proof().terminal().terminal_state
                == RustCompilationTerminalState::CompilerFailed
        }));
        let relations = result
            .compilations()
            .iter()
            .flat_map(|compilation| &compilation.accepted().owners)
            .flat_map(|owner| &owner.relations)
            .collect::<Vec<_>>();
        assert!(
            !relations
                .iter()
                .any(|relation| relation.relation == RustcRelation::MirBody)
        );
        assert!(
            relations
                .iter()
                .filter(|relation| relation.relation == RustcRelation::Diagnostic)
                .any(|relation| {
                    let strings = |name| {
                        relation
                            .batch
                            .column_by_name(name)
                            .unwrap()
                            .as_any()
                            .downcast_ref::<arrow_array::StringArray>()
                            .unwrap()
                    };
                    strings("reason_code")
                        .iter()
                        .zip(strings("message").iter())
                        .any(|(code, message)| {
                            code == Some("E0425")
                                && message
                                    .is_some_and(|message| message.contains("missing_function"))
                        })
                }),
            "expected the actual unresolved-function diagnostic: {stderr}"
        );
        assert!(!harness.paths.extractor_socket_path.exists());
        return;
    }
    assert_eq!(result.result().terminal(), ProviderTerminalStatus::Complete);
    assert!(result.result().gaps().is_empty());
    let diagnostics = result
        .result()
        .coverage()
        .iter()
        .find(|coverage| {
            coverage
                .family()
                .as_str()
                .ends_with("provider.rustc.diagnostic.v1")
        })
        .unwrap();
    assert!(matches!(
        diagnostics.state(),
        ProviderCoverageState::Complete { completed_units: 1 }
    ));
    assert!(
        result
            .compilations()
            .iter()
            .flat_map(|compilation| &compilation.accepted().owners)
            .flat_map(|owner| &owner.relations)
            .filter(|relation| relation.relation == RustcRelation::Diagnostic)
            .any(|relation| {
                let strings = |name| {
                    relation
                        .batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>()
                        .unwrap()
                };
                strings("reason_code")
                    .iter()
                    .zip(strings("severity").iter())
                    .any(|(code, severity)| {
                        code == Some("non_snake_case") && severity == Some("warning")
                    })
            }),
        "a successful compiler invocation retains actual lint diagnostics too"
    );
    let targets = result
        .compilations()
        .iter()
        .flat_map(|compilation| &compilation.accepted().owners)
        .flat_map(|owner| &owner.relations)
        .filter(|relation| relation.relation == RustcRelation::Call)
        .flat_map(|relation| {
            relation
                .batch
                .column_by_name("declared_target")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::StringArray>()
                .unwrap()
                .iter()
                .flatten()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        targets
            .iter()
            .any(|target| target == "other::target" || target == "fixture::other::target"),
        "{targets:?}"
    );
    let mut saw_other_owner = false;
    for compilation in result.compilations() {
        for owner in &compilation.accepted().owners {
            for relation in &owner.relations {
                if relation.relation != RustcRelation::PublicItem {
                    continue;
                }
                let strings = |name| {
                    relation
                        .batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>()
                        .unwrap()
                };
                for row in 0..relation.batch.num_rows() {
                    if strings("qualified_name")
                        .value(row)
                        .ends_with("other::target")
                    {
                        assert_eq!(
                            strings("source_file_id").value(row),
                            source_manifest.files[b"src/other.rs".as_slice()].file_id
                        );
                        saw_other_owner = true;
                    }
                }
            }
        }
    }
    assert!(
        saw_other_owner,
        "nested module owner must retain its captured file identity"
    );
    assert!(!harness.paths.extractor_socket_path.exists());
    assert_fresh_cargo_does_not_admit_unobserved_facts(&harness).await;
}

async fn assert_fresh_cargo_does_not_admit_unobserved_facts(harness: &LifecycleHarness) {
    let paths = RustCompilationPrivatePaths::prepare(
        harness.paths.run_root.parent().unwrap(),
        "fresh-census",
    )
    .unwrap();
    // This fixture deliberately supplies retained target files. Production still uses fresh
    // private targets; a cache hit cannot be enabled there until fact replay/admission exists.
    assert!(
        std::process::Command::new("cp")
            .args(["-a", "--reflink=auto"])
            .arg(harness.paths.target_root.join("."))
            .arg(&paths.target_root)
            .status()
            .unwrap()
            .success()
    );
    let profile = GeneratedSandboxProfile::generate(
        ProviderTrustProfile::UntrustedSandboxed,
        SandboxMechanism::LinuxBubblewrap,
        &harness.inputs.workspace_view,
        &harness.inputs.dependency_view,
        &paths.run_root,
    )
    .unwrap();
    let mut policy = harness.protocol_policy.clone();
    policy.sandbox_profile_digest = profile.sha256_digest.clone();
    policy.provider_deadline_unix_ms = now_millis() + 120_000;
    let job = provider_job(&policy, &harness.admission, &RustcRelation::ALL);
    let seccomp = crate::provider_sandbox::CompiledProviderSeccomp::compile().unwrap();
    let result = run_untrusted_rustc_provider_lifecycle(UntrustedRustcProviderLifecycle {
        cpu_lease: None,
        task_scope: task_scope(),
        provider_job: &job,
        trust_policy: &harness.trust_policy,
        sandbox_capabilities: &harness.capabilities,
        sandbox_profile: &profile,
        compilation_inputs: &harness.inputs,
        private_paths: &paths,
        compilation_request: &harness.request,
        protocol_policy: policy,
        run_admission: harness.admission.clone(),
        allowed_uid: harness.allowed_uid,
        launch_material: ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
    })
    .await
    .unwrap();
    let cargo = result
        .cargo_output()
        .expect("Fresh still has a native Cargo census");
    assert!(cargo.succeeded);
    assert!(!cargo.artifacts.is_empty());
    assert!(cargo.artifacts.iter().all(|artifact| artifact.fresh));
    assert!(
        result.compilations().is_empty(),
        "Fresh did not execute the wrapper"
    );
    assert!(
        result
            .result()
            .coverage()
            .iter()
            .all(|coverage| matches!(coverage.state(), ProviderCoverageState::Unknown { .. }))
    );
}

/// The compiler owner is the crate root, while this diagnostic points into another captured file.
fn assert_native_diagnostic_details(result: &RustcProviderRunResult, source: &str, failed: bool) {
    use arrow_array::{BooleanArray, FixedSizeBinaryArray, StringArray, UInt64Array};
    let (code, witness) = if failed {
        ("E0425", "missing_function")
    } else {
        ("non_snake_case", "Uppercase")
    };
    let mut found = false;
    for owner in result
        .compilations()
        .iter()
        .flat_map(|run| &run.accepted().owners)
    {
        let Some(diagnostics) = owner
            .relations
            .iter()
            .find(|item| item.relation == RustcRelation::Diagnostic)
        else {
            continue;
        };
        let strings = |batch: &RecordBatch, name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .clone()
        };
        let integers = |batch: &RecordBatch, name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .clone()
        };
        let Some(index) = strings(&diagnostics.batch, "reason_code")
            .iter()
            .position(|value| value == Some(code))
        else {
            continue;
        };
        found = true;
        let ordinal = integers(&diagnostics.batch, "diagnostic_ordinal").value(index);
        let spans = &owner
            .relations
            .iter()
            .find(|item| item.relation == RustcRelation::DiagnosticSpan)
            .unwrap()
            .batch;
        let primary = spans
            .column_by_name("is_primary")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap();
        let span = integers(spans, "diagnostic_ordinal")
            .iter()
            .enumerate()
            .find(|(row, value)| *value == Some(ordinal) && primary.value(*row))
            .unwrap()
            .0;
        assert_eq!(
            strings(spans, "location_state").value(span),
            "captured-source"
        );
        let expected_file = encode_public_id(
            IdentityDomain::SourceFile,
            None,
            crate::integrity::digest_bytes(b"src/other.rs")[..16]
                .try_into()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            strings(spans, "location_file_id").value(span),
            expected_file
        );
        assert_ne!(strings(spans, "source_file_id").value(span), expected_file);
        let start = source.find(witness).unwrap() as u64;
        assert_eq!(integers(spans, "span_start_byte").value(span), start);
        assert_eq!(
            integers(spans, "span_end_byte").value(span),
            start + witness.len() as u64
        );
        let digests = spans
            .column_by_name("location_content_digest")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(
            digests.value(span),
            blake3::hash(source.as_bytes()).as_bytes()
        );
        if !failed {
            assert_native_lint_edit(owner, ordinal, start);
        }
    }
    assert!(found, "expected the exact native diagnostic code {code}");
}

fn assert_native_lint_edit(owner: &AcceptedRustcOwner, ordinal: u64, start: u64) {
    use arrow_array::{StringArray, UInt64Array};
    let relation = |kind| {
        &owner
            .relations
            .iter()
            .find(|item| item.relation == kind)
            .unwrap()
            .batch
    };
    let children = relation(RustcRelation::DiagnosticChild);
    assert!(
        children
            .column_by_name("message")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .any(|value| value.is_some_and(|value| value.contains("warn(non_snake_case)")))
    );
    let suggestions = relation(RustcRelation::DiagnosticSuggestion);
    assert!(
        suggestions
            .column_by_name("applicability")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .any(|value| value == Some("MaybeIncorrect"))
    );
    let edits = relation(RustcRelation::DiagnosticEdit);
    let integers = |name| {
        edits
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap()
    };
    let strings = |name| {
        edits
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
    };
    let row = integers("diagnostic_ordinal")
        .iter()
        .position(|value| value == Some(ordinal))
        .unwrap();
    assert_eq!(strings("replacement_text").value(row), "uppercase");
    assert_eq!(integers("span_start_byte").value(row), start);
    assert_eq!(integers("span_end_byte").value(row), start + 9);
    assert_eq!(strings("location_state").value(row), "captured-source");
}
