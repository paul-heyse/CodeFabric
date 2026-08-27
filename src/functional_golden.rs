//! Behavior-first Gate B expectation contracts.
//!
//! This module owns strict ingress and structural validation for human-authored semantic
//! expectations. It deliberately contains no provider, reconciliation, query-engine, graph,
//! lifecycle, candidate, or release imports. Runtime identities and integrity digests are not
//! valid expectation material.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

mod evaluator;

pub use evaluator::{
    ClaimResult, ClaimStatus, ComparisonReport, FixtureClaimEvaluator, LogicalObservation,
    PublicObservationDecoder, QueryEvaluation, ReferenceQueryEvaluator,
    ScenarioTransitionEvaluator, TransitionReport,
};

/// Repository-relative root containing the authored functional authority.
pub const FUNCTIONAL_AUTHORITY_ROOT: &str = "tests/golden/codefabric-golden-v4";
/// Contract member below [`FUNCTIONAL_AUTHORITY_ROOT`].
pub const FUNCTIONAL_CONTRACT_FILE: &str = "functional-expectations.json";

const REQUIRED_SCENARIOS: [&str; 16] = [
    "000_clean_bootstrap",
    "010_python_local_edit",
    "020_python_import_surface_change",
    "030_python_parse_failure_and_recovery",
    "040_rust_body_edit",
    "050_rust_public_signature_change",
    "060_rust_compile_failure_and_recovery",
    "070_rename_and_case_change",
    "080_multi_file_logical_save",
    "090_context_change",
    "100_generated_source_change",
    "110_watcher_loss_reconciliation",
    "120_hot_overlay_flush",
    "130_daemon_restart",
    "140_capability_withdrawal",
    "150_source_acl_redaction",
];

const REQUIRED_QUERY_FORMS: [&str; 8] = [
    "find code entities",
    "retrieve facts about code",
    "follow code relationships",
    "find connecting fact paths",
    "match a code fact pattern",
    "combine result sets",
    "summarize objective facts",
    "retrieve source and syntax context",
];

#[derive(Debug, Error)]
pub enum FunctionalGoldenError {
    #[error("functional golden I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("functional golden JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("functional golden strict ingress failed: {0}")]
    Strict(#[from] crate::contracts::jcs::CanonicalJsonError),
    #[error("functional golden invariant failed: {0}")]
    Invariant(String),
}

fn invariant(message: impl std::fmt::Display) -> FunctionalGoldenError {
    FunctionalGoldenError::Invariant(message.to_string())
}

fn read_functional_authority_input(path: &Path) -> Result<Vec<u8>, FunctionalGoldenError> {
    fs::read(path).map_err(|source| FunctionalGoldenError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionalGoldenContract {
    pub schema_version: u16,
    pub authority_id: String,
    pub normative_scope: Vec<String>,
    pub sources: Vec<SourceFixture>,
    pub logical_records: Vec<LogicalRecord>,
    pub claims: Vec<SemanticClaim>,
    pub queries: Vec<QueryExpectation>,
    pub scenarios: Vec<Scenario>,
    pub mutants: Vec<SemanticMutant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFixture {
    pub source_name: String,
    pub language: String,
    pub path: String,
    pub anchors: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSelector {
    pub source_name: String,
    pub anchor: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalRecordKind {
    Entity,
    Property,
    Relation,
    Derived,
    Unknown,
    Capability,
    Provenance,
    Diagnostic,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalRelation {
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub certainty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalRecord {
    pub name: String,
    pub checkpoint: String,
    pub kind: LogicalRecordKind,
    pub language: String,
    pub source: Option<SourceSelector>,
    pub attributes: BTreeMap<String, String>,
    pub relation: Option<LogicalRelation>,
    pub multiplicity: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSelector {
    pub kind: LogicalRecordKind,
    pub logical_name: Option<String>,
    pub source: Option<SourceSelector>,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimPredicate {
    Present,
    Absent,
    ProvenEmpty,
    Equals {
        field: String,
        value: String,
    },
    Relation {
        source: String,
        target: String,
        relationship: String,
        certainty: String,
    },
    Capability {
        capability: String,
        state: String,
    },
    Terminal {
        state: String,
    },
}

impl ClaimPredicate {
    const fn requires_complete_universe(&self) -> bool {
        matches!(self, Self::Absent | Self::ProvenEmpty)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardinalityPolicy {
    Exact,
    AtLeast,
    AtMost,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cardinality {
    pub policy: CardinalityPolicy,
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderingPolicy {
    Exact,
    Bag,
    Set,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProofUniverse {
    pub profile: String,
    pub owner: String,
    pub context: String,
    pub families: BTreeSet<String>,
    pub capability: String,
    pub currentness: String,
    pub closed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicSurface {
    CanonicalRows,
    UdsResponse,
    ResultArtifact,
    FastmcpStdio,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticClaim {
    pub claim_id: String,
    pub normative_reference: String,
    pub checkpoint: String,
    pub selector: ClaimSelector,
    pub predicate: ClaimPredicate,
    pub cardinality: Cardinality,
    pub ordering: OrderingPolicy,
    pub proof_universe: Option<ProofUniverse>,
    pub surfaces: BTreeSet<PublicSurface>,
    pub allowed_observations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceQuery {
    FindEntities {
        entity_kind: String,
    },
    RetrieveFacts {
        about: Vec<String>,
        fact_kind: String,
    },
    FollowRelationships {
        starting_from: Vec<String>,
        relationship: String,
        direction: String,
        maximum_depth: u16,
    },
    FindPaths {
        starting_from: Vec<String>,
        ending_at: Vec<String>,
        through: String,
        maximum_length: u16,
    },
    MatchPattern {
        source_kind: String,
        relationship: String,
        target_kind: String,
    },
    CombineResults {
        inputs: Vec<String>,
        combination: String,
    },
    SummarizeFacts {
        input: String,
        group_by: String,
    },
    RetrieveSourceContext {
        input: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryExpectation {
    pub query_id: String,
    pub request_form: String,
    pub operation: ReferenceQuery,
    pub expected_records: Vec<String>,
    pub ordering: OrderingPolicy,
    pub completeness: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenarioOperation {
    Barrier {
        name: String,
    },
    WriteFile {
        path: String,
        expected_previous: Option<String>,
        contents: String,
    },
    RemoveFile {
        path: String,
        expected_previous: String,
    },
    RenameFile {
        from: String,
        to: String,
        expected_contents: String,
    },
    SetContext {
        path: String,
        contents: String,
    },
    ProviderFault {
        provider: String,
        fault: String,
    },
    DropWatchHint {
        path: String,
    },
    ReconcileInventory,
    FlushOverlay,
    RestartDaemon,
    SetSourceAcl {
        path: String,
        visibility: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionExpectation {
    pub added: BTreeSet<String>,
    pub removed: BTreeSet<String>,
    pub changed: BTreeSet<String>,
    pub preserved: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioCheckpoint {
    pub checkpoint: String,
    pub after_operation: usize,
    pub terminal: String,
    pub claims: Vec<String>,
    pub queries: Vec<String>,
    pub transition: TransitionExpectation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub scenario_id: String,
    pub operations: Vec<ScenarioOperation>,
    pub checkpoints: Vec<ScenarioCheckpoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticMutant {
    pub mutant_id: String,
    pub semantic_axis: String,
    pub intervention: String,
    pub must_fail_claims: BTreeSet<String>,
    pub must_preserve_claims: BTreeSet<String>,
}

/// Decode a contract after recursive duplicate-key detection, then validate its semantic closure.
///
/// # Errors
///
/// Returns an error for invalid JSON, duplicate keys, unknown fields, forbidden captured-output
/// material, unresolved references, incomplete negative proof, or invalid scenario/query closure.
pub fn decode_contract(
    bytes: &[u8],
    authority_root: &Path,
) -> Result<FunctionalGoldenContract, FunctionalGoldenError> {
    let raw = crate::contracts::jcs::decode_strict(bytes)?;
    reject_captured_output(&raw, "$")?;
    let contract: FunctionalGoldenContract = serde_json::from_value(raw)?;
    contract.validate(authority_root)?;
    Ok(contract)
}

/// Load the repository's behavior-first expectation contract.
///
/// # Errors
///
/// Returns the same failures as [`decode_contract`] plus input I/O failures.
pub fn load_contract(
    repository_root: &Path,
) -> Result<FunctionalGoldenContract, FunctionalGoldenError> {
    let authority_root = repository_root.join(FUNCTIONAL_AUTHORITY_ROOT);
    let contract_path = authority_root.join(FUNCTIONAL_CONTRACT_FILE);
    decode_contract(
        &read_functional_authority_input(&contract_path)?,
        &authority_root,
    )
}

fn reject_captured_output(value: &Value, location: &str) -> Result<(), FunctionalGoldenError> {
    const FORBIDDEN_KEYS: [&str; 10] = [
        "expected_digest",
        "candidate_digest",
        "canonical_row_hex",
        "governed_key_hex",
        "response_bytes_hex",
        "descriptor_identity",
        "registry_count",
        "runtime_id",
        "matches",
        "requirement_checks",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if FORBIDDEN_KEYS.contains(&key.as_str()) {
                    return Err(invariant(format!(
                        "captured-output field {location}.{key} is forbidden"
                    )));
                }
                reject_captured_output(child, &format!("{location}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                reject_captured_output(child, &format!("{location}[{index}]"))?;
            }
        }
        Value::String(text) if text.starts_with("b3:") => {
            return Err(invariant(format!(
                "integrity digest at {location} is not a semantic expectation"
            )));
        }
        _ => {}
    }
    Ok(())
}

fn safe_relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn exactly_one(haystack: &str, needle: &str) -> bool {
    !needle.is_empty() && haystack.match_indices(needle).take(2).count() == 1
}

impl FunctionalGoldenContract {
    fn validate(&self, authority_root: &Path) -> Result<(), FunctionalGoldenError> {
        if self.schema_version != 1
            || self.authority_id != "codefabric.functional-golden.gate-b-v1"
            || self.normative_scope.is_empty()
        {
            return Err(invariant("functional authority header differs"));
        }
        let anchors = validate_sources(&self.sources, authority_root)?;
        let logical_names = validate_logical_records(&self.logical_records, &anchors)?;
        let claim_ids = validate_claims(&self.claims, &anchors, &logical_names)?;
        let query_ids = validate_queries(&self.queries, &logical_names)?;
        validate_scenarios(&self.scenarios, &claim_ids, &query_ids, &logical_names)?;
        validate_mutants(&self.mutants, &claim_ids)?;
        Ok(())
    }
}

fn validate_sources(
    sources: &[SourceFixture],
    authority_root: &Path,
) -> Result<BTreeSet<(String, String)>, FunctionalGoldenError> {
    let mut source_names = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    for source in sources {
        if !source_names.insert(source.source_name.as_str())
            || !safe_relative(&source.path)
            || source.anchors.is_empty()
        {
            return Err(invariant(format!(
                "invalid or duplicate source fixture {}",
                source.source_name
            )));
        }
        let text = String::from_utf8(read_functional_authority_input(
            &authority_root.join(&source.path),
        )?)
        .map_err(|error| {
            invariant(format!(
                "source fixture {} is not UTF-8: {error}",
                source.path
            ))
        })?;
        for (anchor, needle) in &source.anchors {
            if !anchors.insert((source.source_name.clone(), anchor.clone()))
                || !exactly_one(&text, needle)
            {
                return Err(invariant(format!(
                    "source anchor {}.{} does not resolve exactly once",
                    source.source_name, anchor
                )));
            }
        }
    }
    Ok(anchors)
}

fn selector_resolves(selector: &SourceSelector, anchors: &BTreeSet<(String, String)>) -> bool {
    anchors.contains(&(selector.source_name.clone(), selector.anchor.clone()))
}

fn validate_logical_records(
    records: &[LogicalRecord],
    anchors: &BTreeSet<(String, String)>,
) -> Result<BTreeSet<String>, FunctionalGoldenError> {
    let mut identities = BTreeSet::new();
    let mut names = BTreeSet::new();
    for record in records {
        let relation_shape = match record.kind {
            LogicalRecordKind::Relation => record.relation.is_some(),
            LogicalRecordKind::Derived => true,
            _ => record.relation.is_none(),
        };
        if record.multiplicity == 0
            || !identities.insert((record.checkpoint.as_str(), record.name.as_str()))
            || record
                .source
                .as_ref()
                .is_some_and(|selector| !selector_resolves(selector, anchors))
            || !relation_shape
        {
            return Err(invariant(format!("invalid logical record {}", record.name)));
        }
        names.insert(record.name.clone());
    }
    Ok(names)
}

fn universe_is_complete(universe: &ProofUniverse) -> bool {
    universe.closed
        && !universe.profile.is_empty()
        && !universe.owner.is_empty()
        && !universe.context.is_empty()
        && !universe.families.is_empty()
        && !universe.capability.is_empty()
        && !universe.currentness.is_empty()
}

fn validate_claims(
    claims: &[SemanticClaim],
    anchors: &BTreeSet<(String, String)>,
    logical_names: &BTreeSet<String>,
) -> Result<BTreeSet<String>, FunctionalGoldenError> {
    let mut claim_ids = BTreeSet::new();
    let mut predicates = BTreeMap::<(&str, &str), BTreeSet<&'static str>>::new();
    for claim in claims {
        if !claim_ids.insert(claim.claim_id.clone())
            || claim.normative_reference.trim().is_empty()
            || claim.surfaces.is_empty()
            || claim
                .selector
                .source
                .as_ref()
                .is_some_and(|selector| !selector_resolves(selector, anchors))
            || claim
                .selector
                .logical_name
                .as_ref()
                .is_some_and(|name| !logical_names.contains(name))
        {
            return Err(invariant(format!(
                "invalid semantic claim {}",
                claim.claim_id
            )));
        }
        if claim.predicate.requires_complete_universe()
            && claim
                .proof_universe
                .as_ref()
                .is_none_or(|universe| !universe_is_complete(universe))
        {
            return Err(invariant(format!(
                "negative claim {} lacks a complete proof universe",
                claim.claim_id
            )));
        }
        let class = match claim.predicate {
            ClaimPredicate::Present => "present",
            ClaimPredicate::Absent | ClaimPredicate::ProvenEmpty => "negative",
            _ => "qualified",
        };
        predicates
            .entry((
                claim.checkpoint.as_str(),
                claim.selector.logical_name.as_deref().unwrap_or("*"),
            ))
            .or_default()
            .insert(class);
    }
    if predicates
        .values()
        .any(|classes| classes.contains("present") && classes.contains("negative"))
    {
        return Err(invariant("contradictory present and negative claims"));
    }
    Ok(claim_ids)
}

fn validate_queries(
    queries: &[QueryExpectation],
    logical_names: &BTreeSet<String>,
) -> Result<BTreeSet<String>, FunctionalGoldenError> {
    let mut query_ids = BTreeSet::new();
    let mut query_forms = BTreeSet::new();
    for query in queries {
        if !query_ids.insert(query.query_id.clone())
            || !query_forms.insert(query.request_form.as_str())
            || query
                .expected_records
                .iter()
                .any(|name| !logical_names.contains(name))
        {
            return Err(invariant(format!(
                "invalid query expectation {}",
                query.query_id
            )));
        }
    }
    if query_forms != REQUIRED_QUERY_FORMS.into_iter().collect::<BTreeSet<_>>() {
        return Err(invariant(
            "functional contract does not own exactly eight query forms",
        ));
    }
    Ok(query_ids)
}

fn validate_scenarios(
    scenarios: &[Scenario],
    claim_ids: &BTreeSet<String>,
    query_ids: &BTreeSet<String>,
    logical_names: &BTreeSet<String>,
) -> Result<(), FunctionalGoldenError> {
    let mut scenario_ids = BTreeSet::new();
    for scenario in scenarios {
        if !scenario_ids.insert(scenario.scenario_id.as_str())
            || scenario.operations.is_empty()
            || scenario.checkpoints.is_empty()
            || scenario
                .operations
                .iter()
                .flat_map(ScenarioOperation::paths)
                .any(|path| !safe_relative(path))
        {
            return Err(invariant(format!(
                "invalid scenario {}",
                scenario.scenario_id
            )));
        }
        for checkpoint in &scenario.checkpoints {
            if checkpoint.after_operation > scenario.operations.len()
                || checkpoint.claims.iter().any(|id| !claim_ids.contains(id))
                || checkpoint.queries.iter().any(|id| !query_ids.contains(id))
                || checkpoint
                    .transition
                    .names()
                    .any(|name| !logical_names.contains(name))
            {
                return Err(invariant(format!(
                    "scenario {} checkpoint {} is not closed",
                    scenario.scenario_id, checkpoint.checkpoint
                )));
            }
        }
    }
    if scenario_ids != REQUIRED_SCENARIOS.into_iter().collect::<BTreeSet<_>>() {
        return Err(invariant(
            "functional contract does not own the sixteen Gate B scenarios",
        ));
    }
    Ok(())
}

fn validate_mutants(
    mutants: &[SemanticMutant],
    claim_ids: &BTreeSet<String>,
) -> Result<(), FunctionalGoldenError> {
    let mut mutant_ids = BTreeSet::new();
    let mut axes = BTreeSet::new();
    for mutant in mutants {
        if !mutant_ids.insert(mutant.mutant_id.as_str())
            || !axes.insert(mutant.semantic_axis.as_str())
            || mutant.must_fail_claims.is_empty()
            || mutant
                .must_fail_claims
                .iter()
                .chain(&mutant.must_preserve_claims)
                .any(|id| !claim_ids.contains(id))
        {
            return Err(invariant(format!(
                "invalid semantic mutant {}",
                mutant.mutant_id
            )));
        }
    }
    if axes.len() < 8 {
        return Err(invariant(
            "semantic mutant registry covers fewer than eight axes",
        ));
    }
    Ok(())
}

impl TransitionExpectation {
    fn names(&self) -> impl Iterator<Item = &String> {
        self.added
            .iter()
            .chain(&self.removed)
            .chain(&self.changed)
            .chain(&self.preserved)
    }
}

impl ScenarioOperation {
    fn paths(&self) -> Vec<&str> {
        match self {
            Self::WriteFile { path, .. }
            | Self::RemoveFile { path, .. }
            | Self::DropWatchHint { path }
            | Self::SetContext { path, .. }
            | Self::SetSourceAcl { path, .. } => vec![path],
            Self::RenameFile { from, to, .. } => vec![from, to],
            Self::Barrier { .. }
            | Self::ProviderFault { .. }
            | Self::ReconcileInventory
            | Self::FlushOverlay
            | Self::RestartDaemon => Vec::new(),
        }
    }
}

/// Prove a candidate/staging target cannot overlap the authored expectation root.
///
/// This function is intentionally read-only. Candidate writers call it before any output create.
///
/// # Errors
///
/// Returns an error if `output` is the expectation root, is beneath it, or lexically traverses.
pub fn assert_output_isolated(
    repository_root: &Path,
    output: &Path,
) -> Result<(), FunctionalGoldenError> {
    if output
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invariant("candidate output contains parent traversal"));
    }
    let output = if output.is_absolute() {
        output.to_owned()
    } else {
        repository_root.join(output)
    };
    let authority = repository_root.join(FUNCTIONAL_AUTHORITY_ROOT);
    if output == authority || output.starts_with(&authority) {
        return Err(invariant(
            "candidate output overlaps functional expectation authority",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn functional_golden_claim_schema_conformance() {
        let contract = load_contract(&root()).expect("authored functional contract must validate");
        assert_eq!(contract.scenarios.len(), 16);
        assert_eq!(contract.queries.len(), 8);
        assert!(contract.claims.len() >= 16);
        assert!(contract.mutants.len() >= 8);
    }

    #[test]
    fn functional_golden_source_anchor_closure() {
        let contract = load_contract(&root()).expect("source anchors must close");
        let anchored = contract
            .logical_records
            .iter()
            .filter(|record| record.source.is_some())
            .count();
        assert!(
            anchored >= 8,
            "semantic expectations need reviewable source anchors"
        );
        assert!(assert_output_isolated(&root(), Path::new("target/gate-b-v4-dry-run")).is_ok());
        assert!(
            assert_output_isolated(
                &root(),
                Path::new("tests/golden/codefabric-golden-v4/candidate.json")
            )
            .is_err()
        );
    }

    #[test]
    fn functional_golden_negative_claim_requires_complete_universe() {
        let path = root()
            .join(FUNCTIONAL_AUTHORITY_ROOT)
            .join(FUNCTIONAL_CONTRACT_FILE);
        let valid =
            String::from_utf8(read_functional_authority_input(&path).expect("contract input"))
                .expect("UTF-8");
        let weakened = valid.replacen("\"closed\":true", "\"closed\":false", 1);
        let error = decode_contract(weakened.as_bytes(), path.parent().expect("parent"))
            .expect_err("a weakened universe must not prove absence");
        assert!(error.to_string().contains("complete proof universe"));
    }

    #[test]
    fn functional_golden_duplicate_key_rejected() {
        let top = br#"{"schema_version":1,"schema_version":1}"#;
        let nested = br#"{"outer":{"selector":{"kind":"entity","kind":"property"}}}"#;
        for bytes in [top.as_slice(), nested.as_slice()] {
            let error = decode_contract(bytes, Path::new("."))
                .expect_err("duplicate keys must fail before typed DTO construction");
            assert!(error.to_string().contains("duplicate object key"));
        }
    }

    #[test]
    fn functional_golden_contract_operational_gate() {
        let contract = load_contract(&root()).expect("operational load");
        let forms = contract
            .queries
            .iter()
            .map(|query| query.request_form.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(forms, REQUIRED_QUERY_FORMS.into_iter().collect());
        for mutant in &contract.mutants {
            assert!(
                mutant
                    .must_fail_claims
                    .is_disjoint(&mutant.must_preserve_claims),
                "mutant {} cannot both fail and preserve one claim",
                mutant.mutant_id
            );
        }
    }
}
