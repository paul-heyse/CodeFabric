//! Discard partial failed projections and retain their typed non-executable DAG outcomes.

use super::{
    Arc, BTreeMap, BTreeSet, EpochBoundBlockBindingRow, IngressProjection,
    ProgrammaticQueryPortError, SemanticInputRequirement, SemanticQueryClause,
    SemanticQueryRequest, dependency_order_from_ids, released_form,
};
use crate::relational_semantic_query::{
    EpochBoundUnavailableBlockRow, SemanticBlockDisposition as State, SemanticCompilationIssue,
};

#[derive(Default)]
pub(super) struct ProjectionFailures {
    blocks: BTreeMap<Arc<str>, EpochBoundUnavailableBlockRow>,
    requirement_queries: BTreeMap<String, String>,
}

impl ProjectionFailures {
    pub(super) fn track_requirements(
        &mut self,
        query: &str,
        requirements: &[SemanticInputRequirement],
    ) {
        self.requirement_queries.extend(
            requirements
                .iter()
                .map(|row| (row.semantic_field_id.clone(), query.to_owned())),
        );
    }

    pub(super) fn reject(&mut self, clause: &SemanticQueryClause, subject: String) {
        let query_id = Arc::from(clause.query_id());
        self.blocks.insert(
            Arc::clone(&query_id),
            EpochBoundUnavailableBlockRow {
                query_id,
                compatibility_form: released_form(clause),
                disposition: State::SemanticUnavailable,
                issues: vec![SemanticCompilationIssue {
                    code: "SEMANTIC_REFERENCE_UNAVAILABLE",
                    subject_id: Arc::from(subject),
                    related_id: None,
                }],
            },
        );
    }

    pub(super) fn finish(
        mut self,
        request: &SemanticQueryRequest,
        active: &mut Vec<EpochBoundBlockBindingRow>,
        projection: &mut IngressProjection,
    ) -> Result<Vec<EpochBoundUnavailableBlockRow>, ProgrammaticQueryPortError> {
        if self.blocks.is_empty() {
            return Ok(Vec::new());
        }
        let clauses = request
            .queries
            .iter()
            .map(|clause| (clause.query_id(), clause))
            .collect::<BTreeMap<_, _>>();
        let mut incoming = BTreeMap::<&str, BTreeSet<&str>>::new();
        for clause in &request.queries {
            for prior in clause.result_references() {
                if clauses.contains_key(prior.results_of.as_str()) {
                    incoming
                        .entry(clause.query_id())
                        .or_default()
                        .insert(&prior.results_of);
                }
            }
        }
        let order = dependency_order_from_ids(
            clauses.keys().copied(),
            incoming.iter().flat_map(|(consumer, producers)| {
                producers.iter().map(move |producer| (*producer, *consumer))
            }),
        )?;
        for query in order {
            if self.blocks.contains_key(query.as_str()) {
                continue;
            }
            let issues = incoming
                .get(query.as_str())
                .into_iter()
                .flatten()
                .filter(|producer| self.blocks.contains_key(**producer))
                .map(|producer| SemanticCompilationIssue {
                    code: "NOT_EXECUTED_DEPENDENCY",
                    subject_id: Arc::from(query.as_str()),
                    related_id: Some(Arc::from(*producer)),
                })
                .collect::<Vec<_>>();
            if !issues.is_empty() {
                self.blocks.insert(
                    Arc::from(query.as_str()),
                    EpochBoundUnavailableBlockRow {
                        query_id: Arc::from(query.as_str()),
                        compatibility_form: released_form(clauses[query.as_str()]),
                        disposition: State::NotExecutedDependency,
                        issues,
                    },
                );
            }
        }
        let available = |query: &str| !self.blocks.contains_key(query);
        active.retain(|row| available(&row.query_id));
        projection.selections.retain(|row| available(&row.query_id));
        projection.returns.retain(|row| available(&row.query_id));
        projection
            .request_inputs
            .retain(|row| available(&row.query_id));
        projection
            .dependencies
            .retain(|row| available(&row.producer_query_id) && available(&row.consumer_query_id));
        projection
            .requirements
            .retain(|row| available(&self.requirement_queries[&row.semantic_field_id]));
        Ok(self.blocks.into_values().collect())
    }
}
