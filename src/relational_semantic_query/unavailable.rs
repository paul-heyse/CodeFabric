//! Unavailable ingress blocks retain identity and dependency failure without inventing a plan.

use super::{
    Arc, BTreeMap, BTreeSet, EpochBoundSemanticIngress, EpochBoundSemanticIngressError as Error,
    ReleasedSemanticForm, SemanticBlockDisposition as State, SemanticCompilationIssue,
    epoch_duplicate, validate_epoch_identity, validate_epoch_limit,
};

/// An application-resolved semantic failure before an executable program can be selected.
#[derive(Clone, Debug)]
pub struct EpochBoundUnavailableBlockRow {
    pub query_id: Arc<str>,
    pub compatibility_form: ReleasedSemanticForm,
    pub disposition: State,
    pub issues: Vec<SemanticCompilationIssue>,
}

pub(super) fn validate(ingress: &mut EpochBoundSemanticIngress) -> Result<Vec<Arc<str>>, Error> {
    let active = ingress
        .blocks
        .iter()
        .map(|row| row.query_id.as_ref())
        .collect::<BTreeSet<_>>();
    let mut graph = petgraph::graph::DiGraph::<Arc<str>, ()>::new();
    let mut nodes = BTreeMap::new();
    ingress
        .unavailable_blocks
        .sort_by(|a, b| a.query_id.cmp(&b.query_id));
    for row in &mut ingress.unavailable_blocks {
        validate_epoch_identity("unavailable query", &row.query_id)?;
        if active.contains(row.query_id.as_ref()) || nodes.contains_key(row.query_id.as_ref()) {
            return Err(epoch_duplicate("query", row.query_id.to_string()));
        }
        if !matches!(
            row.disposition,
            State::SemanticUnavailable | State::NotExecutedDependency
        ) || row.issues.is_empty()
        {
            return Err(Error::InvalidUnavailableBlock(row.query_id.to_string()));
        }
        validate_epoch_limit("unavailable block issues", row.issues.len(), 128)?;
        for issue in &row.issues {
            validate_epoch_identity("unavailable issue", issue.code)?;
            validate_epoch_identity("unavailable subject", &issue.subject_id)?;
            if row.disposition == State::NotExecutedDependency {
                if issue.code != "NOT_EXECUTED_DEPENDENCY" || issue.related_id.is_none() {
                    return Err(Error::InvalidUnavailableBlock(row.query_id.to_string()));
                }
            } else if issue.related_id.is_some() {
                return Err(Error::InvalidUnavailableBlock(row.query_id.to_string()));
            }
        }
        row.issues.sort();
        row.issues.dedup();
        nodes.insert(row.query_id.clone(), graph.add_node(row.query_id.clone()));
    }
    for row in &ingress.unavailable_blocks {
        for issue in &row.issues {
            let Some(producer) = &issue.related_id else {
                continue;
            };
            validate_epoch_identity("unavailable predecessor", producer)?;
            let Some(&node) = nodes.get(producer) else {
                return Err(Error::UnknownDependencyQuery(producer.to_string()));
            };
            graph.update_edge(node, nodes[&row.query_id], ());
        }
    }
    validate_epoch_limit(
        "max_dependencies",
        ingress
            .dependencies
            .len()
            .saturating_add(graph.edge_count()),
        ingress.limits.compiler().max_dependencies(),
    )?;
    for node in graph.node_indices() {
        for (direction, name, maximum) in [
            (
                petgraph::Direction::Incoming,
                "max_fanin",
                ingress.limits.compiler().max_fanin(),
            ),
            (
                petgraph::Direction::Outgoing,
                "max_fanout",
                ingress.limits.compiler().max_fanout(),
            ),
        ] {
            validate_epoch_limit(
                name,
                graph.neighbors_directed(node, direction).count(),
                maximum,
            )?;
        }
    }
    // Native iterative validation includes self-cycles. Application IDs remain node weights;
    // private indices never become public identities or result ordering.
    let order = petgraph::algo::toposort(&graph, None).map_err(|_| Error::DependencyCycle)?;
    Ok(order.into_iter().map(|node| graph[node].clone()).collect())
}
