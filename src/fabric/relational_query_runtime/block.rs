//! Block-owned output schemas distinguish repeated forms without changing canonical fact identity.

use super::{
    BTreeMap, RelationId, RelationalProgramError, SelectedQueryOutput,
    SupplementalProgramRelationBinding,
};
use crate::relational_program::FieldId;
use datafusion::common::TableReference;

#[derive(Clone, Debug)]
pub(super) struct PriorResultInputSelection {
    pub(super) binding: SupplementalProgramRelationBinding,
    pub(super) producers: Vec<RelationId>,
}

impl SelectedQueryOutput {
    pub(crate) fn with_prior_result_input(
        mut self,
        binding: SupplementalProgramRelationBinding,
        producers: Vec<RelationId>,
    ) -> Self {
        self.prior_results
            .push(PriorResultInputSelection { binding, producers });
        self
    }

    pub(crate) fn bind_block_output(
        mut self,
        request: [u8; 32],
        catalog: [u8; 32],
        query_id: &str,
    ) -> Result<Self, RelationalProgramError> {
        // Legacy epoch outputs can be direct retained relations rather than transient schemas.
        let Some(binding) = self.program_result_binding.as_ref() else {
            return Ok(self);
        };
        let mut hash = blake3::Hasher::new();
        hash.update(b"codefabric.query.block-output.v1\0");
        hash.update(&request);
        hash.update(&catalog);
        hash.update(&binding.authority_pin());
        for value in [query_id, self.relation_id.as_str()] {
            hash.update(&(value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        let authority = *hash.finalize().as_bytes();
        let relation_id = RelationId::new(format!(
            "query.block.{}",
            blake3::Hash::from_bytes(authority).to_hex()
        ))?;
        let fields = binding
            .field_ids()
            .iter()
            .enumerate()
            .map(|(ordinal, _)| FieldId::new(format!("{}.f{ordinal}", relation_id.as_str())))
            .collect::<Result<Vec<_>, _>>()?;
        let mapping = binding
            .field_ids()
            .iter()
            .cloned()
            .zip(fields.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let rebound = SupplementalProgramRelationBinding::try_new(
            relation_id.clone(),
            TableReference::full("codefabric", "query_result", relation_id.as_str()),
            binding.schema().clone(),
            fields,
            authority,
        )?;
        self.program.remap_fields(&mapping);
        self.relation_id = relation_id;
        self.program_result_binding = Some(rebound);
        Ok(self)
    }
}

pub(super) fn dependency_order(
    outputs: Vec<SelectedQueryOutput>,
) -> Result<Vec<SelectedQueryOutput>, RelationalProgramError> {
    let mut ordered = Vec::with_capacity(outputs.len());
    let mut ready = ReadyBlocks::try_new(outputs)?;
    while let Some(output) = ready.pop_ready() {
        ready.complete(&output.relation_id)?;
        ordered.push(output);
    }
    Ok(ordered)
}

/// Native graph indices remain private. Completed producers unlock only their own consumers;
/// an unrelated running root never creates a wave barrier. Relation IDs break ready-set ties.
pub(super) struct ReadyBlocks {
    graph: petgraph::graph::DiGraph<RelationId, ()>,
    nodes: BTreeMap<RelationId, petgraph::graph::NodeIndex>,
    pending: BTreeMap<RelationId, SelectedQueryOutput>,
    remaining: Vec<usize>,
    ready: std::collections::BTreeSet<RelationId>,
    running: std::collections::BTreeSet<RelationId>,
}

impl ReadyBlocks {
    pub(super) fn try_new(
        outputs: Vec<SelectedQueryOutput>,
    ) -> Result<Self, RelationalProgramError> {
        use petgraph::Direction::Incoming;
        use std::collections::BTreeSet;

        let mut pending = BTreeMap::new();
        for output in outputs {
            if pending.insert(output.relation_id.clone(), output).is_some() {
                return Err(RelationalProgramError::InvalidProgram(
                    "duplicate query output".into(),
                ));
            }
        }
        let mut graph = petgraph::graph::DiGraph::with_capacity(pending.len(), 0);
        let nodes = pending
            .keys()
            .map(|id| (id.clone(), graph.add_node(id.clone())))
            .collect::<BTreeMap<_, _>>();
        for output in pending.values() {
            for producer in output
                .prior_results
                .iter()
                .flat_map(|slot| &slot.producers)
                .collect::<BTreeSet<_>>()
            {
                let Some(&producer) = nodes.get(producer) else {
                    return Err(RelationalProgramError::InvalidProgram(
                        "prior result is outside the query transaction".into(),
                    ));
                };
                graph.add_edge(producer, nodes[&output.relation_id], ());
            }
        }
        petgraph::algo::toposort(&graph, None).map_err(|_| {
            RelationalProgramError::InvalidProgram("prior result dependency cycle".into())
        })?;
        let remaining = graph
            .node_indices()
            .map(|node| graph.neighbors_directed(node, Incoming).count())
            .collect::<Vec<_>>();
        let ready = nodes
            .iter()
            .filter(|(_, node)| remaining[node.index()] == 0)
            .map(|(id, _)| id.clone())
            .collect();
        Ok(Self {
            graph,
            nodes,
            pending,
            remaining,
            ready,
            running: BTreeSet::new(),
        })
    }

    pub(super) fn pop_ready(&mut self) -> Option<SelectedQueryOutput> {
        let id = self.ready.pop_first()?;
        self.running.insert(id.clone());
        self.pending.remove(&id)
    }

    pub(super) fn reusable(&self, id: &RelationId) -> bool {
        self.graph
            .neighbors_directed(self.nodes[id], petgraph::Direction::Outgoing)
            .next()
            .is_some()
    }

    pub(super) fn complete(&mut self, id: &RelationId) -> Result<(), RelationalProgramError> {
        if !self.running.remove(id) {
            return Err(RelationalProgramError::InvalidProgram(
                "query output is not running".into(),
            ));
        }
        for consumer in self
            .graph
            .neighbors_directed(self.nodes[id], petgraph::Direction::Outgoing)
        {
            self.remaining[consumer.index()] -= 1;
            if self.remaining[consumer.index()] == 0 {
                self.ready.insert(self.graph[consumer].clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::arrow_result_resource::ResultCoverage;
    use crate::relational_program::{RelationalExpression, RelationalProgram};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn output(id: &str, producers: &[&str]) -> SelectedQueryOutput {
        let relation_id = RelationId::new(id).unwrap();
        let field = FieldId::new(format!("{id}.value")).unwrap();
        let program = RelationalProgram {
            root: RelationalExpression::Input(relation_id.clone()),
            output_fields: vec![field.clone()],
        };
        let mut output =
            SelectedQueryOutput::new(relation_id, program, Some(ResultCoverage::complete(0)));
        for (ordinal, producer) in producers.iter().enumerate() {
            let relation_id = RelationId::new(format!("query.prior.{id}.{ordinal}")).unwrap();
            let binding = SupplementalProgramRelationBinding::try_new(
                relation_id.clone(),
                TableReference::full("codefabric", "query_input", relation_id.as_str()),
                Arc::new(Schema::new(vec![Field::new(
                    "value",
                    DataType::Utf8,
                    false,
                )])),
                vec![field.clone()],
                [1; 32],
            )
            .unwrap();
            output =
                output.with_prior_result_input(binding, vec![RelationId::new(*producer).unwrap()]);
        }
        output
    }

    #[test]
    fn ready_consumers_do_not_wait_for_unrelated_roots_and_repeated_slots_count_once() {
        let outputs = vec![
            output("d", &["b", "c"]),
            output("c", &["a", "a"]),
            output("b", &[]),
            output("a", &[]),
        ];
        let ordered = dependency_order(outputs.clone()).unwrap();
        assert_eq!(
            ordered
                .iter()
                .map(|row| row.relation_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
        let mut ready = ReadyBlocks::try_new(outputs).unwrap();
        let a = ready.pop_ready().unwrap();
        let b = ready.pop_ready().unwrap();
        assert!(ready.reusable(&a.relation_id));
        assert!(ready.pop_ready().is_none());
        ready.complete(&a.relation_id).unwrap();
        let c = ready.pop_ready().unwrap();
        assert_eq!(c.relation_id.as_str(), "c");
        ready.complete(&c.relation_id).unwrap();
        assert!(ready.pop_ready().is_none(), "fan-in still requires b");
        assert!(ready.complete(&a.relation_id).is_err());
        ready.complete(&b.relation_id).unwrap();
        let d = ready.pop_ready().unwrap();
        assert_eq!(d.relation_id.as_str(), "d");
        assert!(!ready.reusable(&d.relation_id));
        ready.complete(&d.relation_id).unwrap();
        assert!(ready.pop_ready().is_none());
    }

    #[test]
    fn invalid_dependency_graphs_fail_before_any_block_runs() {
        for outputs in [
            vec![output("a", &["missing"])],
            vec![output("a", &["a"])],
            vec![output("a", &["b"]), output("b", &["a"])],
            vec![output("a", &[]), output("a", &[])],
        ] {
            assert!(ReadyBlocks::try_new(outputs).is_err());
        }
    }
}
