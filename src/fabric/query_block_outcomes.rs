//! Typed block outcomes shared by execution, sealed manifests and retained result delivery.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic_query_contract::{
    QueryBlockExecutionState as State, QueryBlockIssue, QueryBlockOutcome,
};

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
        self.outcomes
            .iter()
            .all(|row| row.execution_state != State::Complete)
    }

    pub(crate) fn query_id(&self, relation: &str) -> Option<&str> {
        self.bindings
            .iter()
            .find(|(_, selected)| selected.as_str() == relation)
            .map(|(query, _)| query.as_str())
    }

    pub(crate) fn failed(&self, relation: &str) -> bool {
        self.query_id(relation)
            .is_some_and(|query| !self.complete(query))
    }

    fn complete(&self, query: &str) -> bool {
        self.outcomes
            .iter()
            .any(|row| row.query_id == query && row.execution_state == State::Complete)
    }

    pub(crate) fn fail_execution(
        &mut self,
        relation: &str,
    ) -> Result<(), StreamedResultPackageError> {
        self.fail(relation, &[])
    }

    pub(crate) fn fail_dependencies(
        &mut self,
        relation: &str,
        dependencies: &[&str],
    ) -> Result<(), StreamedResultPackageError> {
        if dependencies.is_empty() {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        self.fail(relation, dependencies)
    }

    fn fail(
        &mut self,
        relation: &str,
        dependencies: &[&str],
    ) -> Result<(), StreamedResultPackageError> {
        let query = self
            .query_id(relation)
            .ok_or(StreamedResultPackageError::ManifestShape)?
            .to_owned();
        let errors = if dependencies.is_empty() {
            vec![QueryBlockIssue {
                code: "QUERY_EXECUTION_FAILED".into(),
                subject_id: query.clone(),
                related_id: None,
            }]
        } else {
            dependencies
                .iter()
                .map(|relation| {
                    let related = self
                        .query_id(relation)
                        .ok_or(StreamedResultPackageError::ManifestShape)?;
                    if self.complete(related) {
                        return Err(StreamedResultPackageError::ManifestShape);
                    }
                    Ok(QueryBlockIssue {
                        code: "NOT_EXECUTED_DEPENDENCY".into(),
                        subject_id: query.clone(),
                        related_id: Some(related.to_owned()),
                    })
                })
                .collect::<Result<Vec<_>, StreamedResultPackageError>>()?
        };
        let outcome = self
            .outcomes
            .iter_mut()
            .find(|row| row.query_id == query)
            .ok_or(StreamedResultPackageError::ManifestShape)?;
        if outcome.execution_state != State::Complete {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        outcome.execution_state = if dependencies.is_empty() {
            State::Failed
        } else {
            State::NotExecutedDependency
        };
        outcome.errors = errors;
        Ok(())
    }

    pub(crate) fn write_response(
        &self,
        response: &mut serde_json::Value,
    ) -> Result<(), StreamedResultPackageError> {
        response
            .get_mut("queries")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(StreamedResultPackageError::ManifestShape)?
            .retain(|binding| {
                binding["query_id"]
                    .as_str()
                    .is_some_and(|query| self.complete(query))
            });
        response["query_results"] = serde_json::to_value(&self.outcomes)
            .map_err(StreamedResultPackageError::CanonicalResponse)?;
        Ok(())
    }

    pub(crate) fn retain_processing(
        &self,
        processing: &mut Vec<super::processing_status::QueryProcessing>,
    ) {
        processing.retain(|summary| self.complete(&summary.query_id));
    }

    pub(crate) fn validate_relations<'a>(
        &self,
        relations: impl Iterator<Item = &'a str>,
    ) -> Result<(), StreamedResultPackageError> {
        if self
            .bindings
            .iter()
            .filter(|(query, _)| self.complete(query))
            .map(|(_, relation)| relation.as_str())
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

/// Native execution errors describe block-local malformed computation inputs. Resource, storage,
/// cancellation, source-hard-limit, internal and task-join failures retain request-level handling.
pub(crate) fn isolated_execution_error(error: &datafusion::common::DataFusionError) -> bool {
    use arrow_schema::ArrowError;
    use datafusion::common::DataFusionError;
    match error.find_root() {
        DataFusionError::Execution(_) => true,
        DataFusionError::ArrowError(error, _) => matches!(
            error.as_ref(),
            ArrowError::DivideByZero
                | ArrowError::ArithmeticOverflow(_)
                | ArrowError::CastError(_)
                | ArrowError::ComputeError(_)
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_computation_errors_preserve_fatal_resource_and_integrity_failures() {
        use arrow_schema::ArrowError;
        use datafusion::common::DataFusionError as Error;
        for error in [
            Error::Execution("invalid input".into()),
            ArrowError::DivideByZero.into(),
            ArrowError::CastError("invalid cast".into()).into(),
        ] {
            assert!(isolated_execution_error(&error.context("native plan")));
        }
        for error in [
            Error::ResourcesExhausted("memory".into()),
            Error::Internal("invariant".into()),
            Error::External(Box::new(crate::fabric::source_context::SourceContextMaterializationError::HardOutputLimitExceeded)),
            ArrowError::MemoryError("allocation".into()).into(),
            ArrowError::SchemaError("wrong schema".into()).into(),
            ArrowError::IpcError("corrupt page".into()).into(),
        ] {
            let wrapped = Error::from(ArrowError::ExternalError(Box::new(
                error.context("upstream"),
            )));
            assert!(!isolated_execution_error(&wrapped));
        }
    }

    #[test]
    fn execution_failure_only_removes_its_branch_bindings() {
        let mut response = json!({"queries":[
            {"query_id":"consumer","relation_id":"r.consumer"},
            {"query_id":"producer","relation_id":"r.producer"},
            {"query_id":"independent","relation_id":"r.independent"}],"query_results":[
            {"query_id":"consumer","execution_state":"COMPLETE","errors":[]},
            {"query_id":"producer","execution_state":"COMPLETE","errors":[]},
            {"query_id":"independent","execution_state":"COMPLETE","errors":[]}]});
        let mut outcomes = QueryBlockOutcomes::parse(&response).unwrap().unwrap();
        assert!(
            outcomes
                .fail_dependencies("r.consumer", &["r.producer"])
                .is_err()
        );
        outcomes.fail_execution("r.producer").unwrap();
        outcomes
            .fail_dependencies("r.consumer", &["r.producer"])
            .unwrap();
        assert!(outcomes.fail_execution("r.producer").is_err());
        outcomes.write_response(&mut response).unwrap();
        let parsed = QueryBlockOutcomes::parse(&response).unwrap().unwrap();
        parsed
            .validate_relations(["r.independent"].into_iter())
            .unwrap();
        assert_eq!(
            response["query_results"][0]["errors"][0]["related_id"],
            "producer"
        );
        assert_eq!(
            response["query_results"][1]["errors"][0]["code"],
            "QUERY_EXECUTION_FAILED"
        );
        assert_eq!(response["query_results"][2]["execution_state"], "COMPLETE");
    }

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
