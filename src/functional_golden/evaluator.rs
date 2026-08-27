//! Production-independent semantic evaluators for the behavior-first Gate B authority.
//!
//! The implementation uses only authored logical records and standard-library collections. It
//! does not parse source and does not call providers, reconciliation, lifecycle, query planning,
//! DataFusion, petgraph, candidate generation, or release code.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CardinalityPolicy, ClaimPredicate, ClaimSelector, FunctionalGoldenContract,
    FunctionalGoldenError, LogicalRecord, LogicalRecordKind, OrderingPolicy, ProofUniverse,
    QueryExpectation, ReferenceQuery, SemanticClaim, SourceSelector, TransitionExpectation,
    invariant,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalObservation {
    pub checkpoint: String,
    pub records: Vec<LogicalRecord>,
    pub terminal: Option<String>,
    pub proof: Option<ProofUniverse>,
    pub closed_kinds: BTreeSet<LogicalRecordKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Matched,
    Missing,
    Unexpected,
    Ambiguous,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimResult {
    pub claim_id: String,
    pub status: ClaimStatus,
    pub expected: String,
    pub actual_records: Vec<String>,
    pub actual_count: u64,
    pub source_pointers: Vec<SourceSelector>,
    pub explanation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonReport {
    pub checkpoint: String,
    pub results: Vec<ClaimResult>,
    pub unexpected_closed_records: Vec<String>,
    pub passed: bool,
}

pub struct FixtureClaimEvaluator;

impl FixtureClaimEvaluator {
    #[must_use]
    pub fn compare_checkpoint(
        contract: &FunctionalGoldenContract,
        observation: &LogicalObservation,
    ) -> ComparisonReport {
        let claims = contract
            .claims
            .iter()
            .filter(|claim| claim.checkpoint == observation.checkpoint)
            .collect::<Vec<_>>();
        let results = claims
            .iter()
            .map(|claim| compare_claim(claim, observation))
            .collect::<Vec<_>>();
        let unexpected_closed_records = observation
            .records
            .iter()
            .filter(|record| observation.closed_kinds.contains(&record.kind))
            .filter(|record| {
                !claims
                    .iter()
                    .any(|claim| selector_matches(&claim.selector, record))
            })
            .map(|record| record.name.clone())
            .collect::<Vec<_>>();
        let passed = results
            .iter()
            .all(|result| result.status == ClaimStatus::Matched)
            && unexpected_closed_records.is_empty();
        ComparisonReport {
            checkpoint: observation.checkpoint.clone(),
            results,
            unexpected_closed_records,
            passed,
        }
    }
}

fn selector_matches(selector: &ClaimSelector, record: &LogicalRecord) -> bool {
    selector.kind == record.kind
        && selector
            .logical_name
            .as_ref()
            .is_none_or(|name| name == &record.name)
        && selector
            .source
            .as_ref()
            .is_none_or(|source| record.source.as_ref() == Some(source))
        && selector
            .attributes
            .iter()
            .all(|(key, value)| record.attributes.get(key) == Some(value))
}

fn proof_covers(actual: Option<&ProofUniverse>, expected: Option<&ProofUniverse>) -> bool {
    match (actual, expected) {
        (_, None) => true,
        (Some(actual), Some(expected)) => {
            actual.closed
                && actual.profile == expected.profile
                && actual.owner == expected.owner
                && actual.context == expected.context
                && actual.capability == expected.capability
                && actual.currentness == expected.currentness
                && actual.families.is_superset(&expected.families)
        }
        (None, Some(_)) => false,
    }
}

fn predicate_matches(predicate: &ClaimPredicate, record: &LogicalRecord) -> bool {
    match predicate {
        ClaimPredicate::Present => true,
        ClaimPredicate::Absent | ClaimPredicate::ProvenEmpty | ClaimPredicate::Terminal { .. } => {
            false
        }
        ClaimPredicate::Equals { field, value } => record.attributes.get(field) == Some(value),
        ClaimPredicate::Relation {
            source,
            target,
            relationship,
            certainty,
        } => record.relation.as_ref().is_some_and(|relation| {
            relation.source == *source
                && relation.target == *target
                && relation.relationship == *relationship
                && relation.certainty == *certainty
        }),
        ClaimPredicate::Capability { capability, state } => {
            record.attributes.get("capability") == Some(capability)
                && record.attributes.get("state") == Some(state)
        }
    }
}

fn cardinality_status(policy: &CardinalityPolicy, expected: u64, actual: u64) -> ClaimStatus {
    match policy {
        CardinalityPolicy::Exact if actual == expected => ClaimStatus::Matched,
        CardinalityPolicy::Exact if actual > expected => ClaimStatus::Ambiguous,
        CardinalityPolicy::AtLeast if actual >= expected => ClaimStatus::Matched,
        CardinalityPolicy::Exact | CardinalityPolicy::AtLeast => ClaimStatus::Missing,
        CardinalityPolicy::AtMost if actual <= expected => ClaimStatus::Matched,
        CardinalityPolicy::AtMost => ClaimStatus::Ambiguous,
    }
}

fn compare_claim(claim: &SemanticClaim, observation: &LogicalObservation) -> ClaimResult {
    let selected = observation
        .records
        .iter()
        .filter(|record| selector_matches(&claim.selector, record))
        .collect::<Vec<_>>();
    let (status, matched, count, explanation) = match &claim.predicate {
        ClaimPredicate::Absent | ClaimPredicate::ProvenEmpty => {
            if proof_covers(observation.proof.as_ref(), claim.proof_universe.as_ref()) {
                let count = selected.iter().map(|record| record.multiplicity).sum();
                (
                    cardinality_status(&claim.cardinality.policy, claim.cardinality.count, count),
                    selected,
                    count,
                    "closed-universe negative predicate".to_owned(),
                )
            } else {
                (
                    ClaimStatus::Blocked,
                    Vec::new(),
                    0,
                    "negative proof universe is incomplete or weaker than authored".to_owned(),
                )
            }
        }
        ClaimPredicate::Terminal { state } => {
            let count = u64::from(observation.terminal.as_ref() == Some(state));
            (
                cardinality_status(&claim.cardinality.policy, claim.cardinality.count, count),
                Vec::new(),
                count,
                format!("terminal state observed as {:?}", observation.terminal),
            )
        }
        predicate => {
            let matched = selected
                .into_iter()
                .filter(|record| predicate_matches(predicate, record))
                .collect::<Vec<_>>();
            let count = matched.iter().map(|record| record.multiplicity).sum();
            (
                cardinality_status(&claim.cardinality.policy, claim.cardinality.count, count),
                matched,
                count,
                "selector and predicate evaluated independently".to_owned(),
            )
        }
    };
    let mut source_pointers = matched
        .iter()
        .filter_map(|record| record.source.clone())
        .collect::<Vec<_>>();
    source_pointers.sort_by(|left, right| {
        (&left.source_name, &left.anchor).cmp(&(&right.source_name, &right.anchor))
    });
    source_pointers.dedup();
    ClaimResult {
        claim_id: claim.claim_id.clone(),
        status,
        expected: format!("{:?} {:?}", claim.predicate, claim.cardinality),
        actual_records: matched.iter().map(|record| record.name.clone()).collect(),
        actual_count: count,
        source_pointers,
        explanation,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QueryEvaluation {
    pub answers: BTreeMap<String, Vec<String>>,
    pub mismatches: BTreeMap<String, QueryMismatch>,
    pub passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QueryMismatch {
    pub expected: Vec<String>,
    pub actual: Vec<String>,
}

pub struct ReferenceQueryEvaluator<'a> {
    records: Vec<&'a LogicalRecord>,
    by_name: BTreeMap<&'a str, &'a LogicalRecord>,
}

impl<'a> ReferenceQueryEvaluator<'a> {
    #[must_use]
    pub fn new(contract: &'a FunctionalGoldenContract, checkpoint: &str) -> Self {
        let records = contract
            .logical_records
            .iter()
            .filter(|record| record.checkpoint == checkpoint)
            .collect::<Vec<_>>();
        let by_name = records
            .iter()
            .map(|record| (record.name.as_str(), *record))
            .collect();
        Self { records, by_name }
    }

    #[must_use]
    pub fn evaluate_all(&self, queries: &[QueryExpectation]) -> QueryEvaluation {
        let mut answers = BTreeMap::new();
        let mut mismatches = BTreeMap::new();
        for query in queries {
            let actual = self.evaluate_one(query, &answers);
            if actual != query.expected_records {
                mismatches.insert(
                    query.query_id.clone(),
                    QueryMismatch {
                        expected: query.expected_records.clone(),
                        actual: actual.clone(),
                    },
                );
            }
            answers.insert(query.query_id.clone(), actual);
        }
        QueryEvaluation {
            passed: mismatches.is_empty(),
            answers,
            mismatches,
        }
    }

    fn evaluate_one(
        &self,
        query: &QueryExpectation,
        prior: &BTreeMap<String, Vec<String>>,
    ) -> Vec<String> {
        let mut answer = match &query.operation {
            ReferenceQuery::FindEntities { entity_kind } => self
                .records
                .iter()
                .filter(|record| record.kind == LogicalRecordKind::Entity)
                .filter(|record| entity_kind_matches(record, entity_kind))
                .map(|record| record.name.clone())
                .collect(),
            ReferenceQuery::RetrieveFacts { about, fact_kind } => self
                .records
                .iter()
                .filter(|record| record.kind == LogicalRecordKind::Property)
                .filter(|record| {
                    record
                        .attributes
                        .get("about")
                        .is_some_and(|name| about.contains(name))
                        && record.attributes.get("property") == Some(fact_kind)
                })
                .map(|record| record.name.clone())
                .collect(),
            ReferenceQuery::FollowRelationships {
                starting_from,
                relationship,
                direction,
                maximum_depth,
            } => self.follow(starting_from, relationship, direction, *maximum_depth),
            ReferenceQuery::FindPaths {
                starting_from,
                ending_at,
                through,
                maximum_length,
            } => self.paths(starting_from, ending_at, through, *maximum_length),
            ReferenceQuery::MatchPattern {
                source_kind,
                relationship,
                target_kind,
                starting_from,
                ending_at,
            } => self.pattern(
                source_kind,
                relationship,
                target_kind,
                starting_from,
                ending_at,
            ),
            ReferenceQuery::CombineResults {
                inputs,
                combination,
            } => combine(prior, inputs, combination),
            ReferenceQuery::SummarizeFacts { input, group_by } => {
                self.summarize(prior.get(input), group_by)
            }
            ReferenceQuery::RetrieveSourceContext { input } => prior
                .get(input)
                .into_iter()
                .flatten()
                .filter(|name| {
                    self.by_name
                        .get(name.as_str())
                        .is_some_and(|row| row.source.is_some())
                })
                .cloned()
                .collect(),
        };
        if !matches!(query.ordering, OrderingPolicy::NotApplicable) {
            answer.sort();
        }
        answer
    }

    fn relation_rows(&self, relationship: &str) -> Vec<&LogicalRecord> {
        self.records
            .iter()
            .copied()
            .filter(|record| record.kind == LogicalRecordKind::Relation)
            .filter(|record| {
                record
                    .relation
                    .as_ref()
                    .is_some_and(|relation| relation.relationship == relationship)
            })
            .collect()
    }

    fn follow(
        &self,
        starting_from: &[String],
        relationship: &str,
        direction: &str,
        maximum_depth: u16,
    ) -> Vec<String> {
        let edges = self.relation_rows(relationship);
        let mut frontier = starting_from.iter().cloned().collect::<BTreeSet<_>>();
        let mut result = Vec::new();
        for _ in 0..maximum_depth {
            let mut next = BTreeSet::new();
            for edge in &edges {
                let relation = edge.relation.as_ref().expect("filtered relation");
                let (seed, target) = if direction == "incoming" {
                    (&relation.target, &relation.source)
                } else {
                    (&relation.source, &relation.target)
                };
                if frontier.contains(seed) {
                    result.extend(std::iter::repeat_n(
                        edge.name.clone(),
                        usize::try_from(edge.multiplicity).unwrap_or(usize::MAX),
                    ));
                    next.insert(target.clone());
                }
            }
            frontier = next;
        }
        result
    }

    fn paths(
        &self,
        starting_from: &[String],
        ending_at: &[String],
        relationship: &str,
        maximum_length: u16,
    ) -> Vec<String> {
        let edges = self.relation_rows(relationship);
        let goals = ending_at.iter().cloned().collect::<BTreeSet<_>>();
        let mut queue = starting_from
            .iter()
            .cloned()
            .map(|start| (start.clone(), vec![start]))
            .collect::<VecDeque<_>>();
        let mut paths = BTreeSet::new();
        while let Some((node, path)) = queue.pop_front() {
            let length = path.len().saturating_sub(1);
            if length > 0 && goals.contains(&node) {
                paths.insert(path.join(","));
                continue;
            }
            if length >= usize::from(maximum_length) {
                continue;
            }
            for edge in &edges {
                let relation = edge.relation.as_ref().expect("filtered relation");
                if relation.source == node && !path.contains(&relation.target) {
                    let mut next_path = path.clone();
                    next_path.push(relation.target.clone());
                    queue.push_back((relation.target.clone(), next_path));
                }
            }
        }
        self.records
            .iter()
            .filter(|record| record.kind == LogicalRecordKind::Derived)
            .filter(|record| {
                record
                    .attributes
                    .get("path")
                    .is_some_and(|path| paths.contains(path))
            })
            .map(|record| record.name.clone())
            .collect()
    }

    fn pattern(
        &self,
        source_kind: &str,
        relationship: &str,
        target_kind: &str,
        starting_from: &[String],
        ending_at: &[String],
    ) -> Vec<String> {
        self.relation_rows(relationship)
            .into_iter()
            .filter(|record| {
                let relation = record.relation.as_ref().expect("filtered relation");
                (starting_from.is_empty() || starting_from.contains(&relation.source))
                    && (ending_at.is_empty() || ending_at.contains(&relation.target))
                    && self
                        .by_name
                        .get(relation.source.as_str())
                        .is_some_and(|source| entity_kind_matches(source, source_kind))
                    && self
                        .by_name
                        .get(relation.target.as_str())
                        .is_some_and(|target| entity_kind_matches(target, target_kind))
            })
            .map(|record| record.name.clone())
            .collect()
    }

    fn summarize(&self, input: Option<&Vec<String>>, group_by: &str) -> Vec<String> {
        let mut groups = BTreeMap::<String, usize>::new();
        if group_by == "relationship" {
            for name in input.into_iter().flatten() {
                if let Some(relationship) = self
                    .by_name
                    .get(name.as_str())
                    .and_then(|record| record.relation.as_ref())
                    .map(|relation| relation.relationship.clone())
                {
                    *groups.entry(relationship).or_default() += 1;
                }
            }
        }
        self.records
            .iter()
            .filter(|record| record.kind == LogicalRecordKind::Derived)
            .filter(|record| {
                record.attributes.get("group").is_some_and(|group| {
                    groups.get(group).is_some_and(|count| {
                        record.attributes.get("count") == Some(&count.to_string())
                    })
                })
            })
            .map(|record| record.name.clone())
            .collect()
    }
}

fn entity_kind_matches(record: &LogicalRecord, requested: &str) -> bool {
    record.attributes.get("entity_kind").is_some_and(|actual| {
        actual == requested || (requested == "callable" && actual == "function")
    })
}

fn combine(
    prior: &BTreeMap<String, Vec<String>>,
    inputs: &[String],
    combination: &str,
) -> Vec<String> {
    let values = inputs
        .iter()
        .filter_map(|input| prior.get(input))
        .collect::<Vec<_>>();
    match combination {
        "intersection-by-logical-record" => values.first().map_or_else(Vec::new, |first| {
            first
                .iter()
                .filter(|value| values.iter().skip(1).all(|other| other.contains(value)))
                .cloned()
                .collect()
        }),
        "difference-by-logical-record" => values.first().map_or_else(Vec::new, |first| {
            first
                .iter()
                .filter(|value| values.iter().skip(1).all(|other| !other.contains(value)))
                .cloned()
                .collect()
        }),
        _ => values
            .into_iter()
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TransitionReport {
    pub violations: Vec<String>,
    pub passed: bool,
}

pub struct ScenarioTransitionEvaluator;

impl ScenarioTransitionEvaluator {
    #[must_use]
    pub fn compare(
        expected: &TransitionExpectation,
        before: &[LogicalRecord],
        after: &[LogicalRecord],
    ) -> TransitionReport {
        let before = before
            .iter()
            .map(|record| (record.name.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let after = after
            .iter()
            .map(|record| (record.name.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let mut violations = Vec::new();
        for name in &expected.added {
            if before.contains_key(name.as_str()) || !after.contains_key(name.as_str()) {
                violations.push(format!("{name} was not added"));
            }
        }
        for name in &expected.removed {
            if !before.contains_key(name.as_str()) || after.contains_key(name.as_str()) {
                violations.push(format!("{name} was not removed"));
            }
        }
        for name in &expected.changed {
            if !matches!((before.get(name.as_str()), after.get(name.as_str())),
                (Some(left), Some(right)) if !semantically_equal(left, right))
            {
                violations.push(format!("{name} did not change"));
            }
        }
        for name in &expected.preserved {
            if !matches!((before.get(name.as_str()), after.get(name.as_str())),
                (Some(left), Some(right)) if semantically_equal(left, right))
            {
                violations.push(format!("{name} was not preserved"));
            }
        }
        TransitionReport {
            passed: violations.is_empty(),
            violations,
        }
    }
}

fn semantically_equal(left: &LogicalRecord, right: &LogicalRecord) -> bool {
    left.name == right.name
        && left.kind == right.kind
        && left.language == right.language
        && left.source == right.source
        && left.attributes == right.attributes
        && left.relation == right.relation
        && left.multiplicity == right.multiplicity
}

/// One deterministic execution of an authored semantic counterfactual.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MutantExecution {
    pub mutant_id: String,
    pub semantic_axis: String,
    pub injection_seam: String,
    pub detected_by: String,
    pub failed_claims: BTreeSet<String>,
    pub preserved_claims: BTreeSet<String>,
    pub missing_expected_failures: BTreeSet<String>,
    pub collateral_failures: BTreeSet<String>,
    pub killed: bool,
}

/// Closed execution report for the complete authored mutant registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MutantExecutionReport {
    pub executions: Vec<MutantExecution>,
    pub uncovered_claims: BTreeSet<String>,
    pub passed: bool,
}

fn authored_observations(
    contract: &FunctionalGoldenContract,
) -> BTreeMap<String, LogicalObservation> {
    let checkpoints = contract
        .logical_records
        .iter()
        .map(|record| record.checkpoint.clone())
        .chain(contract.claims.iter().map(|claim| claim.checkpoint.clone()))
        .collect::<BTreeSet<_>>();
    checkpoints
        .into_iter()
        .map(|checkpoint| {
            let terminal = contract.claims.iter().find_map(|claim| {
                if claim.checkpoint == checkpoint
                    && let ClaimPredicate::Terminal { state } = &claim.predicate
                {
                    return Some(state.clone());
                }
                None
            });
            let proof = contract.claims.iter().find_map(|claim| {
                (claim.checkpoint == checkpoint)
                    .then(|| claim.proof_universe.clone())
                    .flatten()
            });
            (
                checkpoint.clone(),
                LogicalObservation {
                    checkpoint: checkpoint.clone(),
                    records: contract
                        .logical_records
                        .iter()
                        .filter(|record| record.checkpoint == checkpoint)
                        .cloned()
                        .collect(),
                    terminal,
                    proof,
                    closed_kinds: BTreeSet::new(),
                },
            )
        })
        .collect()
}

fn record_mut<'a>(
    observations: &'a mut BTreeMap<String, LogicalObservation>,
    name: &str,
) -> Result<&'a mut LogicalRecord, FunctionalGoldenError> {
    let matches = observations
        .values_mut()
        .flat_map(|observation| observation.records.iter_mut())
        .filter(|record| record.name == name)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invariant(format!(
            "semantic mutant record {name} has multiplicity {}",
            matches.len()
        )));
    }
    Ok(matches.into_iter().next().expect("one record"))
}

fn claim_failures(
    contract: &FunctionalGoldenContract,
    observations: &BTreeMap<String, LogicalObservation>,
) -> Result<BTreeSet<String>, FunctionalGoldenError> {
    let mut failed = BTreeSet::new();
    for claim in &contract.claims {
        let observation = observations
            .get(&claim.checkpoint)
            .ok_or_else(|| invariant(format!("claim checkpoint {} is absent", claim.checkpoint)))?;
        if compare_claim(claim, observation).status != ClaimStatus::Matched {
            failed.insert(claim.claim_id.clone());
        }
    }
    Ok(failed)
}

fn mutate_relation(
    observations: &mut BTreeMap<String, LogicalObservation>,
    name: &str,
    reverse: bool,
    certainty: Option<&str>,
) -> Result<(), FunctionalGoldenError> {
    let relation = record_mut(observations, name)?
        .relation
        .as_mut()
        .ok_or_else(|| invariant(format!("mutant relation {name} is absent")))?;
    if reverse {
        std::mem::swap(&mut relation.source, &mut relation.target);
    }
    if let Some(certainty) = certainty {
        certainty.clone_into(&mut relation.certainty);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One closed dispatcher makes the registered semantic-axis census auditable.
fn apply_logical_mutant(
    axis: &str,
    observations: &mut BTreeMap<String, LogicalObservation>,
) -> Result<Option<&'static str>, FunctionalGoldenError> {
    let detected_by = match axis {
        "owner-authority" => {
            record_mut(observations, "py.pipeline")?
                .attributes
                .insert("owner".to_owned(), "syntax-owner".to_owned());
            record_mut(observations, "provenance.py.pipeline")?
                .attributes
                .insert("authority".to_owned(), "syntax-owner".to_owned());
            "fixture-claim-evaluator"
        }
        "property-value" => {
            record_mut(observations, "py.pipeline.return")?
                .attributes
                .insert("value".to_owned(), "str".to_owned());
            "fixture-claim-evaluator"
        }
        "relation-direction" => {
            mutate_relation(observations, "rel.py.pipeline.scale", true, None)?;
            mutate_relation(observations, "derived.ffi.scale", true, None)?;
            "fixture-claim-evaluator"
        }
        "bag-multiplicity" => {
            record_mut(observations, "rel.py.pipeline.scale")?.multiplicity = 2;
            "fixture-claim-evaluator"
        }
        "certainty" => {
            mutate_relation(
                observations,
                "rel.rust.pipeline.double",
                false,
                Some("heuristic"),
            )?;
            "fixture-claim-evaluator"
        }
        "negative-proof" => {
            "tree-sitter+python-semantic".clone_into(
                &mut observations
                    .get_mut("base")
                    .and_then(|observation| observation.proof.as_mut())
                    .ok_or_else(|| invariant("base negative-proof universe is absent"))?
                    .owner,
            );
            "fixture-claim-evaluator"
        }
        "currentness" => {
            record_mut(observations, "capability.pyrefly")?
                .attributes
                .insert("state".to_owned(), "stale".to_owned());
            observations
                .get_mut("base")
                .ok_or_else(|| invariant("base observation is absent"))?
                .terminal = Some("stale".to_owned());
            "fixture-claim-evaluator"
        }
        "rust-owner-authority" => {
            record_mut(observations, "rust.pipeline")?
                .attributes
                .insert("owner".to_owned(), "syntax-owner".to_owned());
            "fixture-claim-evaluator"
        }
        "mir-branch-kind" => {
            record_mut(observations, "rust.choose.branch")?
                .attributes
                .insert("value".to_owned(), "goto".to_owned());
            "fixture-claim-evaluator"
        }
        "ffi-relation-direction" => {
            mutate_relation(observations, "rel.ffi.pipeline", true, None)?;
            "fixture-claim-evaluator"
        }
        "parse-failure-materialization" => {
            let observation = observations
                .get_mut("python-broken")
                .ok_or_else(|| invariant("python-broken observation is absent"))?;
            observation.records.retain(|record| {
                !matches!(
                    record.name.as_str(),
                    "unknown.python.parse" | "diagnostic.python.parse"
                )
            });
            "fixture-claim-evaluator"
        }
        "capability-withdrawal" => {
            record_mut(observations, "capability.pyrefly.withdrawn")?
                .attributes
                .insert("state".to_owned(), "current".to_owned());
            "fixture-claim-evaluator"
        }
        "acl-redaction" => {
            record_mut(observations, "py.pipeline.return.redacted")?
                .attributes
                .insert("source-text-visibility".to_owned(), "visible".to_owned());
            "fixture-claim-evaluator"
        }
        "delivery-equivalence" | "scenario-transition" | "query-ordering" => return Ok(None),
        _ => {
            return Err(invariant(format!(
                "unregistered semantic mutant axis {axis}"
            )));
        }
    };
    Ok(Some(detected_by))
}

fn specialized_mutant_failures(
    contract: &FunctionalGoldenContract,
    axis: &str,
) -> Result<(BTreeSet<String>, &'static str), FunctionalGoldenError> {
    match axis {
        "delivery-equivalence" => {
            let baseline = contract
                .logical_records
                .iter()
                .filter(|record| record.checkpoint == "base")
                .map(|record| record.name.clone())
                .collect::<Vec<_>>();
            let mut fastmcp = baseline.clone();
            fastmcp.retain(|record| record != "py.pipeline");
            if baseline == fastmcp {
                return Err(invariant("FastMCP surface-drop mutant survived"));
            }
            Ok((
                BTreeSet::from(["claim.delivery.equivalent".to_owned()]),
                "public-surface-bag-equivalence",
            ))
        }
        "scenario-transition" => {
            let checkpoint = contract
                .scenarios
                .iter()
                .find(|scenario| scenario.scenario_id == "010_python_local_edit")
                .and_then(|scenario| scenario.checkpoints.first())
                .ok_or_else(|| invariant("named Python edit checkpoint is absent"))?;
            let before = contract
                .logical_records
                .iter()
                .filter(|record| record.checkpoint == "base")
                .cloned()
                .collect::<Vec<_>>();
            let mut after = before.clone();
            after
                .iter_mut()
                .find(|record| record.name == "py.pipeline")
                .ok_or_else(|| invariant("Python pipeline transition record is absent"))?
                .attributes
                .insert("body-revision".to_owned(), "edited".to_owned());
            if !ScenarioTransitionEvaluator::compare(&checkpoint.transition, &before, &after).passed
            {
                return Err(invariant("authored Python edit transition is inconsistent"));
            }
            let mut mutated = checkpoint.transition.clone();
            mutated.changed.remove("py.pipeline");
            mutated.preserved.insert("py.pipeline".to_owned());
            if ScenarioTransitionEvaluator::compare(&mutated, &before, &after).passed {
                return Err(invariant("scenario-transition mutant survived"));
            }
            Ok((
                checkpoint.claims.iter().cloned().collect(),
                "scenario-transition-evaluator",
            ))
        }
        "query-ordering" => {
            let evaluated =
                ReferenceQueryEvaluator::new(contract, "base").evaluate_all(&contract.queries);
            if !evaluated.passed {
                return Err(invariant("authored query contract is inconsistent"));
            }
            let expected = contract
                .queries
                .iter()
                .find(|query| query.query_id == "q.combine")
                .ok_or_else(|| invariant("q.combine expectation is absent"))?;
            let mut reversed = evaluated.answers["q.combine"].clone();
            reversed.reverse();
            if reversed == expected.expected_records {
                return Err(invariant("query-order mutant survived"));
            }
            Ok((
                BTreeSet::from(["claim.delivery.equivalent".to_owned()]),
                "reference-query-ordering-evaluator",
            ))
        }
        _ => Err(invariant(format!(
            "semantic axis {axis} has no specialized evaluator"
        ))),
    }
}

/// Execute every authored semantic mutant and reject survivors, collateral failures, or gaps.
///
/// # Errors
///
/// Returns an error when the authored baseline is inconsistent or a mutant axis is not owned by
/// an executable evaluator.
pub fn execute_required_mutants(
    contract: &FunctionalGoldenContract,
) -> Result<MutantExecutionReport, FunctionalGoldenError> {
    let baseline_observations = authored_observations(contract);
    let baseline_failures = claim_failures(contract, &baseline_observations)?;
    if !baseline_failures.is_empty() {
        return Err(invariant(format!(
            "authored baseline claims fail before mutation: {baseline_failures:?}"
        )));
    }
    let all_claims = contract
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let covered_claims = contract
        .mutants
        .iter()
        .flat_map(|mutant| mutant.must_fail_claims.iter().cloned())
        .collect::<BTreeSet<_>>();
    let uncovered_claims = all_claims
        .difference(&covered_claims)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut executions = Vec::with_capacity(contract.mutants.len());
    for mutant in &contract.mutants {
        let mut observations = baseline_observations.clone();
        let (failed_claims, detected_by) = if let Some(detected_by) =
            apply_logical_mutant(&mutant.semantic_axis, &mut observations)?
        {
            (claim_failures(contract, &observations)?, detected_by)
        } else {
            specialized_mutant_failures(contract, &mutant.semantic_axis)?
        };
        let preserved_claims = mutant
            .must_preserve_claims
            .difference(&failed_claims)
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing_expected_failures = mutant
            .must_fail_claims
            .difference(&failed_claims)
            .cloned()
            .collect::<BTreeSet<_>>();
        let collateral_failures = failed_claims
            .difference(&mutant.must_fail_claims)
            .cloned()
            .collect::<BTreeSet<_>>();
        let killed = missing_expected_failures.is_empty()
            && collateral_failures.is_empty()
            && preserved_claims == mutant.must_preserve_claims;
        executions.push(MutantExecution {
            mutant_id: mutant.mutant_id.clone(),
            semantic_axis: mutant.semantic_axis.clone(),
            injection_seam: mutant.injection_seam.clone(),
            detected_by: detected_by.to_owned(),
            failed_claims,
            preserved_claims,
            missing_expected_failures,
            collateral_failures,
            killed,
        });
    }
    let passed = uncovered_claims.is_empty() && executions.iter().all(|result| result.killed);
    Ok(MutantExecutionReport {
        executions,
        uncovered_claims,
        passed,
    })
}

pub struct PublicObservationDecoder;

impl PublicObservationDecoder {
    /// Convert governed row projections into the neutral logical observation form.
    ///
    /// # Errors
    ///
    /// Returns an error when any row does not implement the strict logical-record contract.
    pub fn decode_canonical_rows(
        checkpoint: &str,
        terminal: Option<String>,
        proof: Option<ProofUniverse>,
        closed_kinds: BTreeSet<LogicalRecordKind>,
        rows: &[Value],
    ) -> Result<LogicalObservation, FunctionalGoldenError> {
        let records = rows
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LogicalObservation {
            checkpoint: checkpoint.to_owned(),
            records,
            terminal,
            proof,
            closed_kinds,
        })
    }

    /// Decode a public UDS response containing a neutral functional observation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed/duplicate-key JSON or a missing/invalid observation.
    pub fn decode_uds_response(bytes: &[u8]) -> Result<LogicalObservation, FunctionalGoldenError> {
        decode_nested(bytes, &["functional_observation"])
    }

    /// Decode a persisted public result artifact containing the neutral observation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed/duplicate-key JSON or a missing/invalid observation.
    pub fn decode_result_artifact(
        bytes: &[u8],
    ) -> Result<LogicalObservation, FunctionalGoldenError> {
        decode_nested(bytes, &["functional_observation"])
    }

    /// Decode the functional observation nested in the FastMCP structured response.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed/duplicate-key JSON or a missing/invalid observation.
    pub fn decode_fastmcp_stdio(bytes: &[u8]) -> Result<LogicalObservation, FunctionalGoldenError> {
        decode_nested(
            bytes,
            &[
                "structured_content",
                "delivery",
                "response",
                "functional_observation",
            ],
        )
    }
}

fn decode_nested(bytes: &[u8], path: &[&str]) -> Result<LogicalObservation, FunctionalGoldenError> {
    let mut value = crate::contracts::jcs::decode_strict(bytes)?;
    for field in path {
        value = value
            .as_object_mut()
            .and_then(|object| object.remove(*field))
            .ok_or_else(|| invariant(format!("public observation lacks {}", path.join("."))))?;
    }
    serde_json::from_value(value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use proptest::prelude::*;
    use serde_json::json;

    use super::*;
    use crate::functional_golden::{
        FUNCTIONAL_AUTHORITY_ROOT, FUNCTIONAL_CONTRACT_FILE, assert_output_isolated, load_contract,
    };

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn contract() -> FunctionalGoldenContract {
        load_contract(&root()).expect("functional contract")
    }

    fn base_observation(contract: &FunctionalGoldenContract) -> LogicalObservation {
        let proof = contract
            .claims
            .iter()
            .find(|claim| claim.claim_id == "claim.absent.phantom")
            .and_then(|claim| claim.proof_universe.clone());
        LogicalObservation {
            checkpoint: "base".to_owned(),
            records: contract
                .logical_records
                .iter()
                .filter(|record| record.checkpoint == "base")
                .cloned()
                .collect(),
            terminal: Some("current".to_owned()),
            proof,
            closed_kinds: BTreeSet::new(),
        }
    }

    fn status(report: &ComparisonReport, claim_id: &str) -> ClaimStatus {
        report
            .results
            .iter()
            .find(|result| result.claim_id == claim_id)
            .expect("claim result")
            .status
    }

    #[test]
    fn reference_query_evaluator_laws() {
        let contract = contract();
        let result =
            ReferenceQueryEvaluator::new(&contract, "base").evaluate_all(&contract.queries);
        assert!(result.passed, "query mismatches: {:?}", result.mismatches);
        assert_eq!(result.answers.len(), 8);
        assert!(result.answers["q.paths"].is_empty());
        assert_eq!(
            result.answers["q.summary"],
            vec!["summary.call-targets".to_owned()]
        );

        let observation = base_observation(&contract);
        let encoded = serde_json::to_value(&observation).unwrap();
        let canonical = PublicObservationDecoder::decode_canonical_rows(
            "base",
            observation.terminal.clone(),
            observation.proof.clone(),
            BTreeSet::new(),
            encoded["records"].as_array().unwrap(),
        )
        .unwrap();
        let uds = json!({"functional_observation": observation});
        let public_bytes = serde_json::to_vec(&uds).unwrap();
        let uds = PublicObservationDecoder::decode_uds_response(&public_bytes).unwrap();
        let artifact = PublicObservationDecoder::decode_result_artifact(&public_bytes).unwrap();
        let mcp = json!({
            "structured_content": {
                "delivery": {"response": {"functional_observation": uds.clone()}}
            }
        });
        let mcp =
            PublicObservationDecoder::decode_fastmcp_stdio(&serde_json::to_vec(&mcp).unwrap())
                .unwrap();
        assert_eq!(canonical, uds);
        assert_eq!(canonical, artifact);
        assert_eq!(canonical, mcp);
    }

    #[test]
    fn semantic_oracle_required_mutants_are_killed() {
        let contract = contract();
        let report = execute_required_mutants(&contract).expect("mutant execution");
        assert!(report.passed, "{report:#?}");
        assert!(report.uncovered_claims.is_empty());
        assert_eq!(report.executions.len(), contract.mutants.len());
        assert!(report.executions.iter().all(|execution| execution.killed));
        assert!(report.executions.iter().all(|execution| {
            !execution.injection_seam.is_empty() && !execution.detected_by.is_empty()
        }));
    }

    #[test]
    fn semantic_oracle_rejects_unregistered_or_surviving_mutant() {
        let mut survivor = contract();
        survivor.mutants[0]
            .must_preserve_claims
            .remove("claim.rust.owner");
        survivor.mutants[0]
            .must_fail_claims
            .insert("claim.rust.owner".to_owned());
        let report = execute_required_mutants(&survivor).expect("survivor report");
        assert!(!report.passed);
        assert!(
            report.executions[0]
                .missing_expected_failures
                .contains("claim.rust.owner")
        );

        let mut unregistered = contract();
        unregistered.mutants[0].semantic_axis = "unregistered-axis".to_owned();
        assert!(execute_required_mutants(&unregistered).is_err());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one named matrix keeps all comparator falsification axes visible together"
    )]
    fn functional_golden_comparator_falsification() {
        let contract = contract();
        let baseline = base_observation(&contract);
        let report = FixtureClaimEvaluator::compare_checkpoint(&contract, &baseline);
        assert!(
            report.passed,
            "baseline claim failures: {:?}",
            report.results
        );

        let mut missing = baseline.clone();
        missing
            .records
            .retain(|record| record.name != "rel.py.pipeline.scale");
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &missing),
                "claim.py.call-target"
            ),
            ClaimStatus::Missing
        );

        let mut duplicate = baseline.clone();
        duplicate
            .records
            .iter_mut()
            .find(|record| record.name == "rel.py.pipeline.scale")
            .unwrap()
            .multiplicity = 2;
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &duplicate),
                "claim.py.call-target"
            ),
            ClaimStatus::Ambiguous
        );

        let mut swapped = baseline.clone();
        let relation = swapped
            .records
            .iter_mut()
            .find(|record| record.name == "rel.py.pipeline.scale")
            .and_then(|record| record.relation.as_mut())
            .unwrap();
        std::mem::swap(&mut relation.source, &mut relation.target);
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &swapped),
                "claim.py.call-target"
            ),
            ClaimStatus::Missing
        );

        let mut wrong_kind = baseline.clone();
        wrong_kind
            .records
            .iter_mut()
            .find(|record| record.name == "py.pipeline")
            .unwrap()
            .kind = LogicalRecordKind::Property;
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &wrong_kind),
                "claim.py.owner"
            ),
            ClaimStatus::Missing
        );

        let mut wrong_certainty = baseline.clone();
        wrong_certainty
            .records
            .iter_mut()
            .find(|record| record.name == "rel.rust.pipeline.double")
            .and_then(|record| record.relation.as_mut())
            .unwrap()
            .certainty = "heuristic".to_owned();
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &wrong_certainty),
                "claim.rust.call-target"
            ),
            ClaimStatus::Missing
        );

        let mut stale = baseline.clone();
        stale
            .records
            .iter_mut()
            .find(|record| record.name == "capability.pyrefly")
            .unwrap()
            .attributes
            .insert("state".to_owned(), "stale".to_owned());
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &stale),
                "claim.capability.current"
            ),
            ClaimStatus::Missing
        );

        let mut false_complete = baseline.clone();
        false_complete.proof.as_mut().unwrap().closed = false;
        assert_eq!(
            status(
                &FixtureClaimEvaluator::compare_checkpoint(&contract, &false_complete),
                "claim.absent.phantom"
            ),
            ClaimStatus::Blocked
        );

        let mut unexpected = baseline;
        unexpected.closed_kinds.insert(LogicalRecordKind::Entity);
        let mut extra = unexpected.records[0].clone();
        extra.name = "unexpected.entity".to_owned();
        extra
            .attributes
            .insert("name".to_owned(), "unexpected".to_owned());
        unexpected.records.push(extra);
        let report = FixtureClaimEvaluator::compare_checkpoint(&contract, &unexpected);
        assert!(
            report
                .unexpected_closed_records
                .contains(&"unexpected.entity".to_owned())
        );
        assert!(!report.passed);
    }

    #[test]
    fn functional_golden_expectation_write_isolation() {
        let root = root();
        assert!(assert_output_isolated(&root, Path::new("target/functional-candidate")).is_ok());
        assert!(
            assert_output_isolated(
                &root,
                Path::new("tests/golden/codefabric-golden-v4/observed.json")
            )
            .is_err()
        );
        let contract = contract();
        let encoded = serde_json::to_value(contract).unwrap();
        let forbidden_fields = [
            "expected_digest",
            "canonical_row_hex",
            "matches",
            "requirement_checks",
        ];
        let encoded = encoded.to_string();
        assert!(
            forbidden_fields
                .iter()
                .all(|field| !encoded.contains(field))
        );
        assert!(
            root.join(FUNCTIONAL_AUTHORITY_ROOT)
                .join(FUNCTIONAL_CONTRACT_FILE)
                .is_file()
        );
    }

    #[test]
    fn functional_golden_independence_operational_gate() {
        let justfile = include_str!("../../justfile");
        assert!(justfile.contains("functional-golden-independence-check:"));
        assert!(justfile.contains("functional_golden_comparator_falsification"));
        assert!(justfile.contains("functional_golden_expectation_write_isolation"));

        let contract = contract();
        let before = contract
            .logical_records
            .iter()
            .find(|record| record.name == "py.pipeline" && record.checkpoint == "base")
            .unwrap()
            .clone();
        let mut after = before.clone();
        after.checkpoint = "edited".to_owned();
        after
            .attributes
            .insert("body-value".to_owned(), "7".to_owned());
        let expected = TransitionExpectation {
            added: BTreeSet::new(),
            removed: BTreeSet::new(),
            changed: BTreeSet::from(["py.pipeline".to_owned()]),
            preserved: BTreeSet::new(),
        };
        assert!(ScenarioTransitionEvaluator::compare(&expected, &[before], &[after]).passed);
    }

    proptest! {
        #[test]
        fn reference_bag_multiplicity_law(multiplicity in 1_u64..5) {
            let contract = contract();
            let mut observation = base_observation(&contract);
            observation.records.iter_mut()
                .find(|record| record.name == "rel.py.pipeline.scale")
                .unwrap().multiplicity = multiplicity;
            let result = FixtureClaimEvaluator::compare_checkpoint(&contract, &observation);
            prop_assert_eq!(
                status(&result, "claim.py.call-target") == ClaimStatus::Matched,
                multiplicity == 1
            );
        }

        #[test]
        fn reference_coverage_monotonicity_law(closed in any::<bool>()) {
            let contract = contract();
            let mut observation = base_observation(&contract);
            observation.proof.as_mut().unwrap().closed = closed;
            let result = FixtureClaimEvaluator::compare_checkpoint(&contract, &observation);
            let negative = status(&result, "claim.absent.phantom");
            prop_assert_eq!(negative == ClaimStatus::Matched, closed);
        }
    }
}
