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
    let mut pending = outputs
        .into_iter()
        .map(|output| (output.relation_id.clone(), output))
        .collect::<BTreeMap<_, _>>();
    let all = pending
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if pending
        .values()
        .flat_map(|output| &output.prior_results)
        .flat_map(|slot| &slot.producers)
        .any(|producer| !all.contains(producer))
    {
        return Err(RelationalProgramError::InvalidProgram(
            "prior result is outside the query transaction".into(),
        ));
    }
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .find(|(_, output)| {
                output
                    .prior_results
                    .iter()
                    .flat_map(|slot| &slot.producers)
                    .all(|producer| !pending.contains_key(producer))
            })
            .map(|(id, _)| id.clone());
        let Some(ready) = ready else {
            return Err(RelationalProgramError::InvalidProgram(
                "prior result dependency cycle".into(),
            ));
        };
        ordered.push(pending.remove(&ready).expect("selected pending output"));
    }
    Ok(ordered)
}
