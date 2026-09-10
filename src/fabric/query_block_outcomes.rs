//! Typed block outcomes shared by execution, sealed manifests and retained result delivery.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic_query_contract::{QueryBlockExecutionState as State, QueryBlockOutcome};

use super::streamed_result_package::StreamedResultPackageError;

pub(crate) struct QueryBlockOutcomes {
    outcomes: Vec<QueryBlockOutcome>,
    bindings: BTreeMap<String, String>,
}

impl QueryBlockOutcomes {
    pub(crate) fn parse(
        response: &serde_json::Value,
    ) -> Result<Option<Self>, StreamedResultPackageError> {
        let Some(value) = response.get("query_results") else {
            return Ok(None);
        };
        let outcomes: Vec<QueryBlockOutcome> = serde_json::from_value(value.clone())
            .map_err(StreamedResultPackageError::CanonicalResponse)?;
        let ids = outcomes
            .iter()
            .map(|row| row.query_id.as_str())
            .collect::<BTreeSet<_>>();
        if outcomes.is_empty()
            || ids.len() != outcomes.len()
            || outcomes.iter().any(|row| !row.valid())
        {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        let complete = outcomes
            .iter()
            .filter(|row| row.execution_state == State::Complete)
            .map(|row| row.query_id.as_str())
            .collect::<BTreeSet<_>>();
        // Native iterative topological validation is linear in this retained dependency graph.
        // Node indices remain private; public request order and identities are unchanged.
        let mut graph = petgraph::graph::DiGraph::<State, ()>::with_capacity(
            outcomes.len(),
            outcomes.iter().map(|row| row.errors.len()).sum(),
        );
        let nodes = outcomes
            .iter()
            .map(|row| (row.query_id.as_str(), graph.add_node(row.execution_state)))
            .collect::<BTreeMap<_, _>>();
        for row in &outcomes {
            if row.execution_state != State::NotExecutedDependency {
                continue;
            }
            for error in &row.errors {
                let related = error.related_id.as_deref().expect("validated dependency");
                let Some(&producer) = nodes.get(related) else {
                    return Err(StreamedResultPackageError::ManifestShape);
                };
                if graph[producer] == State::Complete {
                    return Err(StreamedResultPackageError::ManifestShape);
                }
                graph.add_edge(producer, nodes[row.query_id.as_str()], ());
            }
        }
        petgraph::algo::toposort(&graph, None)
            .map_err(|_| StreamedResultPackageError::ManifestShape)?;
        let bindings = response["queries"]
            .as_array()
            .ok_or(StreamedResultPackageError::ManifestShape)?;
        let mut by_query = BTreeMap::new();
        let mut relations = BTreeSet::new();
        for binding in bindings {
            let query = binding["query_id"]
                .as_str()
                .ok_or(StreamedResultPackageError::ManifestShape)?;
            let relation = binding["relation_id"]
                .as_str()
                .ok_or(StreamedResultPackageError::ManifestShape)?;
            if by_query
                .insert(query.to_owned(), relation.to_owned())
                .is_some()
                || !relations.insert(relation)
            {
                return Err(StreamedResultPackageError::ManifestShape);
            }
        }
        if by_query.keys().map(String::as_str).collect::<BTreeSet<_>>() != complete {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        Ok(Some(Self {
            outcomes,
            bindings: by_query,
        }))
    }

    pub(crate) fn all_failed(&self) -> bool {
        self.bindings.is_empty()
    }

    pub(crate) fn validate_relations<'a>(
        &self,
        relations: impl Iterator<Item = &'a str>,
    ) -> Result<(), StreamedResultPackageError> {
        if self
            .bindings
            .values()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != relations.collect()
        {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        Ok(())
    }

    pub(crate) fn into_outcomes(self) -> Vec<QueryBlockOutcome> {
        self.outcomes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn failed_dependency_chains_require_an_actual_failed_block() {
        let response = json!({"queries":[],"query_results":[
            {"query_id":"third","execution_state":"NOT_EXECUTED_DEPENDENCY","errors":[{"code":"NOT_EXECUTED_DEPENDENCY","subject_id":"third","related_id":"second"}]},
            {"query_id":"second","execution_state":"NOT_EXECUTED_DEPENDENCY","errors":[{"code":"NOT_EXECUTED_DEPENDENCY","subject_id":"second","related_id":"first"}]},
            {"query_id":"first","execution_state":"FAILED","errors":[{"code":"EXECUTION_FAILED","subject_id":"first"}]}
        ]});
        let parsed = QueryBlockOutcomes::parse(&response).unwrap().unwrap();
        assert!(parsed.all_failed());
        assert_eq!(parsed.into_outcomes()[0].query_id, "third");
        for related in ["third", "second", "missing"] {
            let mut invalid = response.clone();
            invalid["query_results"][1]["errors"][0]["related_id"] = json!(related);
            assert!(QueryBlockOutcomes::parse(&invalid).is_err());
        }
    }
}
