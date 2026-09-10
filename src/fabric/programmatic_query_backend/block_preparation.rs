//! Block-local application checks precede native planning and preserve compiled DAG ownership.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::relational_semantic_query::EpochBoundDependencyRow;
use crate::semantic_query_contract::{
    QueryBlockExecutionState as State, QueryBlockIssue, QueryBlockOutcome,
};

pub(super) struct PreparedBlocks {
    outcomes: Vec<QueryBlockOutcome>,
    indices: BTreeMap<String, usize>,
}

impl PreparedBlocks {
    pub(super) fn new(outcomes: Vec<QueryBlockOutcome>) -> Self {
        let indices = outcomes
            .iter()
            .enumerate()
            .map(|(index, row)| (row.query_id.clone(), index))
            .collect();
        Self { outcomes, indices }
    }

    pub(super) fn complete(&self, query: &str) -> bool {
        self.outcomes[self.indices[query]].execution_state == State::Complete
    }

    pub(super) fn fail(&mut self, query: &str, code: &str, subject: &str) {
        let row = &mut self.outcomes[self.indices[query]];
        if row.execution_state != State::Complete {
            return;
        }
        row.execution_state = State::Failed;
        row.errors.push(QueryBlockIssue {
            code: code.into(),
            subject_id: subject.into(),
            related_id: None,
        });
    }

    pub(super) fn propagate(&mut self, order: &[Arc<str>], edges: &[EpochBoundDependencyRow]) {
        let mut incoming = BTreeMap::<&str, Vec<&str>>::new();
        for edge in edges {
            incoming
                .entry(&edge.consumer_query_id)
                .or_default()
                .push(&edge.producer_query_id);
        }
        // The ingress validator already proves this deterministic order against every edge.
        // Iterate it once; an unavailable ancestor therefore reaches every downstream block.
        for query in order {
            if !self.complete(query) {
                continue;
            }
            let mut failed = incoming
                .get(query.as_ref())
                .into_iter()
                .flatten()
                .copied()
                .filter(|producer| !self.complete(producer))
                .collect::<Vec<_>>();
            failed.sort_unstable();
            failed.dedup();
            if failed.is_empty() {
                continue;
            }
            let row = &mut self.outcomes[self.indices[query.as_ref()]];
            row.execution_state = State::NotExecutedDependency;
            row.errors = failed
                .into_iter()
                .map(|producer| QueryBlockIssue {
                    code: "NOT_EXECUTED_DEPENDENCY".into(),
                    subject_id: query.to_string(),
                    related_id: Some(producer.into()),
                })
                .collect();
        }
    }

    pub(super) fn into_outcomes(self) -> Vec<QueryBlockOutcome> {
        self.outcomes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_failure_reaches_descendants_without_changing_original_order_or_own_errors() {
        let queries = ["leaf", "independent", "join", "a", "b", "own-error"];
        let mut blocks = PreparedBlocks::new(
            queries
                .iter()
                .map(|query| QueryBlockOutcome {
                    query_id: (*query).into(),
                    execution_state: State::Complete,
                    errors: vec![],
                })
                .collect(),
        );
        blocks.fail("a", "SOURCE_ACCESS_DENIED", "a");
        blocks.fail("b", "SEMANTIC_REFERENCE_UNAVAILABLE", "b");
        blocks.fail("own-error", "INVALID_RETURN_DIRECTIVE", "return.order-by");
        let edges = [
            ("a", "join"),
            ("b", "join"),
            ("a", "join"),
            ("join", "leaf"),
            ("a", "own-error"),
        ]
        .into_iter()
        .map(|(producer, consumer)| EpochBoundDependencyRow {
            producer_query_id: Arc::from(producer),
            consumer_query_id: Arc::from(consumer),
            producer_role_id: Arc::from("entities"),
            consumer_role_id: Arc::from("entities"),
            consumer_slot_id: Arc::from("subjects"),
            ordinal: 0,
        })
        .collect::<Vec<_>>();
        let order = ["a", "b", "independent", "join", "leaf", "own-error"].map(Arc::from);
        blocks.propagate(&order, &edges);
        // Applying the same immutable dependency selection is idempotent.
        blocks.propagate(&order, &edges);
        let outcomes = blocks.into_outcomes();
        assert_eq!(
            outcomes
                .iter()
                .map(|row| row.query_id.as_str())
                .collect::<Vec<_>>(),
            queries
        );
        assert_eq!(outcomes[0].execution_state, State::NotExecutedDependency);
        assert_eq!(outcomes[0].errors[0].related_id.as_deref(), Some("join"));
        assert_eq!(outcomes[1].execution_state, State::Complete);
        assert_eq!(
            outcomes[2]
                .errors
                .iter()
                .map(|issue| issue.related_id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(outcomes[5].errors[0].code, "INVALID_RETURN_DIRECTIVE");
        assert!(outcomes.iter().all(QueryBlockOutcome::valid));
    }
}
