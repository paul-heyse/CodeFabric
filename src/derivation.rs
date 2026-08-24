//! Registered deterministic fact derivations.
//!
//! Derivations consume application-owned canonical rows, never provider-native objects. A single
//! typed registry record owns the algorithm/version/dependency contract and every emitted row is
//! bound to that identity.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use crate::fact_ingest::{RelationRow, SyntaxDetailRow};
use crate::registries::{DERIVATION_ENTRIES, DerivationEntry};

pub const SYNTAX_TREE_DERIVATION_ID: &str = "SYNTAX_TREE_V1";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SyntaxTreeProjectionRow {
    pub owner_id: [u8; 16],
    pub parent_id: [u8; 16],
    pub child_id: [u8; 16],
    pub ordinal: u32,
    pub derivation_id: &'static str,
    pub algorithm_version: &'static str,
    pub input_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxNodeInput {
    pub owner_id: [u8; 16],
    pub entity_id: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AstChildInput {
    pub owner_id: [u8; 16],
    pub parent_id: [u8; 16],
    pub child_id: [u8; 16],
    pub ordinal: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivationOutput {
    pub rows: Vec<SyntaxTreeProjectionRow>,
    pub state_digest: [u8; 32],
    pub owners: BTreeSet<[u8; 16]>,
}

#[derive(Debug, Error)]
pub enum DerivationError {
    #[error("DERIVATION_REGISTRY_MISSING:{0}")]
    RegistryMissing(&'static str),
    #[error("DERIVATION_REGISTRY_INVALID:{0}")]
    RegistryInvalid(&'static str),
    #[error("DERIVATION_INPUT_INVALID:{0}")]
    InputInvalid(String),
}

fn syntax_tree_contract() -> Result<&'static DerivationEntry, DerivationError> {
    let contract = DERIVATION_ENTRIES
        .iter()
        .find(|entry| entry.derivation_id == SYNTAX_TREE_DERIVATION_ID)
        .ok_or(DerivationError::RegistryMissing(SYNTAX_TREE_DERIVATION_ID))?;
    if contract.owner_kind != "source-file"
        || contract.input_fact_families != ["syntax-detail", "relation"]
        || contract.output_fact_families != ["syntax-projection"]
        || contract.projection_id != SYNTAX_TREE_DERIVATION_ID
        || contract.precision_profile != "CORE_SOURCE_V1"
        || contract.algorithm_version != "1.0"
        || contract.replacement_scope != "OWNER_REPLACE"
        || contract.dependency_rule != "source-file->syntax-detail+AST_CHILD"
    {
        return Err(DerivationError::RegistryInvalid(SYNTAX_TREE_DERIVATION_ID));
    }
    Ok(contract)
}

fn input_digest(
    contract: &DerivationEntry,
    detail: SyntaxNodeInput,
    relation: AstChildInput,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derivation.syntax-tree.input.v1\0");
    hasher.update(contract.derivation_id.as_bytes());
    hasher.update(contract.algorithm_version.as_bytes());
    hasher.update(&detail.owner_id);
    hasher.update(&detail.entity_id);
    hasher.update(&relation.parent_id);
    hasher.update(&relation.child_id);
    hasher.update(&relation.ordinal.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn state_digest(contract: &DerivationEntry, rows: &[SyntaxTreeProjectionRow]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.derivation.syntax-tree.state.v1\0");
    hasher.update(contract.derivation_id.as_bytes());
    hasher.update(contract.algorithm_version.as_bytes());
    for row in rows {
        hasher.update(&row.owner_id);
        hasher.update(&row.parent_id);
        hasher.update(&row.child_id);
        hasher.update(&row.ordinal.to_be_bytes());
        hasher.update(&row.input_digest);
    }
    *hasher.finalize().as_bytes()
}

/// Execute the registered `SYNTAX_TREE_V1` owner-replace derivation.
///
/// # Errors
///
/// Returns an error when the generated registry contract is absent or inconsistent, or when the
/// supplied syntax rows violate the registered tree invariants.
pub fn derive_syntax_tree(
    details: &[SyntaxDetailRow],
    ast_child_relations: &[RelationRow],
) -> Result<DerivationOutput, DerivationError> {
    let nodes = details
        .iter()
        .map(|detail| SyntaxNodeInput {
            owner_id: detail.scope.owner_id,
            entity_id: detail.entity_id,
        })
        .collect::<Vec<_>>();
    let relations = ast_child_relations
        .iter()
        .map(|relation| {
            let ordinal = relation.ordinal.ok_or_else(|| {
                DerivationError::InputInvalid("AST_CHILD relation lacks an ordinal".to_owned())
            })?;
            Ok(AstChildInput {
                owner_id: relation.scope.owner_id,
                parent_id: relation.source_id,
                child_id: relation.target_id,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    DerivationError::InputInvalid("AST_CHILD ordinal is negative".to_owned())
                })?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    derive_syntax_tree_inputs(&nodes, &relations)
}

fn derive_syntax_tree_inputs(
    details: &[SyntaxNodeInput],
    ast_child_relations: &[AstChildInput],
) -> Result<DerivationOutput, DerivationError> {
    let contract = syntax_tree_contract()?;
    let details_by_entity = details
        .iter()
        .map(|detail| (detail.entity_id, detail))
        .collect::<BTreeMap<_, _>>();
    if details_by_entity.len() != details.len() {
        return Err(DerivationError::InputInvalid(
            "syntax detail entity IDs are not unique".to_owned(),
        ));
    }
    let mut keys = BTreeSet::new();
    let mut parents = BTreeMap::new();
    let mut children = BTreeMap::<[u8; 16], Vec<[u8; 16]>>::new();
    let mut rows = Vec::with_capacity(ast_child_relations.len());
    for relation in ast_child_relations {
        let parent = details_by_entity.get(&relation.parent_id).ok_or_else(|| {
            DerivationError::InputInvalid("AST_CHILD subject lacks syntax detail".to_owned())
        })?;
        let child = details_by_entity.get(&relation.child_id).ok_or_else(|| {
            DerivationError::InputInvalid("AST_CHILD object lacks syntax detail".to_owned())
        })?;
        if parent.owner_id != child.owner_id {
            return Err(DerivationError::InputInvalid(
                "SYNTAX_TREE_V1 cannot cross source-file owners".to_owned(),
            ));
        }
        if relation.owner_id != parent.owner_id {
            return Err(DerivationError::InputInvalid(
                "AST_CHILD relation owner differs from syntax owner".to_owned(),
            ));
        }
        let key = (parent.owner_id, relation.parent_id, relation.ordinal);
        if !keys.insert(key) {
            return Err(DerivationError::InputInvalid(
                "duplicate AST_CHILD ordinal for parent".to_owned(),
            ));
        }
        if relation.parent_id == relation.child_id {
            return Err(DerivationError::InputInvalid(
                "SYNTAX_TREE_V1 contains a self cycle".to_owned(),
            ));
        }
        if parents
            .insert(relation.child_id, relation.parent_id)
            .is_some_and(|existing| existing != relation.parent_id)
        {
            return Err(DerivationError::InputInvalid(
                "SYNTAX_TREE_V1 child has multiple parents".to_owned(),
            ));
        }
        children
            .entry(relation.parent_id)
            .or_default()
            .push(relation.child_id);
        rows.push(SyntaxTreeProjectionRow {
            owner_id: parent.owner_id,
            parent_id: relation.parent_id,
            child_id: relation.child_id,
            ordinal: relation.ordinal,
            derivation_id: contract.derivation_id,
            algorithm_version: contract.algorithm_version,
            input_digest: input_digest(contract, **parent, *relation),
        });
    }
    ensure_acyclic(&children)?;
    rows.sort();
    let owners = rows.iter().map(|row| row.owner_id).collect();
    Ok(DerivationOutput {
        state_digest: state_digest(contract, &rows),
        rows,
        owners,
    })
}

fn ensure_acyclic(children: &BTreeMap<[u8; 16], Vec<[u8; 16]>>) -> Result<(), DerivationError> {
    fn visit(
        node: [u8; 16],
        children: &BTreeMap<[u8; 16], Vec<[u8; 16]>>,
        visiting: &mut BTreeSet<[u8; 16]>,
        visited: &mut BTreeSet<[u8; 16]>,
    ) -> Result<(), DerivationError> {
        if visited.contains(&node) {
            return Ok(());
        }
        if !visiting.insert(node) {
            return Err(DerivationError::InputInvalid(
                "SYNTAX_TREE_V1 contains a cycle".to_owned(),
            ));
        }
        if let Some(next) = children.get(&node) {
            for child in next {
                visit(*child, children, visiting, visited)?;
            }
        }
        visiting.remove(&node);
        visited.insert(node);
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in children.keys() {
        visit(*node, children, &mut visiting, &mut visited)?;
    }
    Ok(())
}

impl DerivationOutput {
    /// Query the immutable derived projection independently by owner identity.
    #[must_use]
    pub fn rows_for_owner(&self, owner_id: [u8; 16]) -> Vec<&SyntaxTreeProjectionRow> {
        self.rows
            .iter()
            .filter(|row| row.owner_id == owner_id)
            .collect()
    }
}

/// Recompute only changed owners while preserving the same deterministic global ordering.
///
/// # Errors
///
/// Returns the same registry or input-invariant failures as [`derive_syntax_tree`].
pub fn derive_syntax_tree_incremental(
    previous: &[SyntaxTreeProjectionRow],
    changed_owners: &BTreeSet<[u8; 16]>,
    details: &[SyntaxDetailRow],
    ast_child_relations: &[RelationRow],
) -> Result<DerivationOutput, DerivationError> {
    let mut retained = previous
        .iter()
        .filter(|row| !changed_owners.contains(&row.owner_id))
        .cloned()
        .collect::<Vec<_>>();
    let changed_details = details
        .iter()
        .filter(|row| changed_owners.contains(&row.scope.owner_id))
        .cloned()
        .collect::<Vec<_>>();
    let detail_ids = changed_details
        .iter()
        .map(|row| row.entity_id)
        .collect::<BTreeSet<_>>();
    let changed_relations = ast_child_relations
        .iter()
        .filter(|row| detail_ids.contains(&row.source_id))
        .cloned()
        .collect::<Vec<_>>();
    retained.extend(derive_syntax_tree(&changed_details, &changed_relations)?.rows);
    retained.sort();
    let contract = syntax_tree_contract()?;
    Ok(DerivationOutput {
        state_digest: state_digest(contract, &retained),
        owners: retained.iter().map(|row| row.owner_id).collect(),
        rows: retained,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(owner: u8, entity: u8) -> SyntaxNodeInput {
        SyntaxNodeInput {
            owner_id: [owner; 16],
            entity_id: [entity; 16],
        }
    }

    fn relation(owner: u8, parent: u8, child: u8, ordinal: u32) -> AstChildInput {
        AstChildInput {
            owner_id: [owner; 16],
            parent_id: [parent; 16],
            child_id: [child; 16],
            ordinal,
        }
    }

    #[test]
    fn wp37_behavioral_acceptance() {
        let details = vec![detail(1, 1), detail(1, 2), detail(2, 3), detail(2, 4)];
        let relations = vec![relation(2, 3, 4, 0), relation(1, 1, 2, 0)];
        let full = derive_syntax_tree_inputs(&details, &relations).unwrap();
        let initial = derive_syntax_tree_inputs(&details[..2], &relations[1..]).unwrap();
        let mut retained = initial.rows;
        retained.extend(
            derive_syntax_tree_inputs(&details[2..], &relations[..1])
                .unwrap()
                .rows,
        );
        retained.sort();
        let incremental = DerivationOutput {
            state_digest: state_digest(syntax_tree_contract().unwrap(), &retained),
            owners: retained.iter().map(|row| row.owner_id).collect(),
            rows: retained,
        };
        assert_eq!(incremental, full);
        assert_eq!(full.rows_for_owner([1; 16]).len(), 1);
    }

    #[test]
    fn wp37_negative_zero_state() {
        assert!(derive_syntax_tree_inputs(&[detail(1, 1)], &[relation(1, 1, 2, 0)]).is_err());
        assert!(
            derive_syntax_tree_inputs(&[detail(1, 1), detail(2, 2)], &[relation(1, 1, 2, 0)])
                .is_err()
        );
        assert!(
            derive_syntax_tree_inputs(
                &[detail(1, 1), detail(1, 2), detail(1, 3)],
                &[
                    relation(1, 1, 2, 0),
                    relation(1, 2, 3, 0),
                    relation(1, 3, 1, 0),
                ],
            )
            .is_err()
        );
    }

    #[test]
    fn wp37_structural_acceptance() {
        let contract = syntax_tree_contract().unwrap();
        assert_eq!(contract.derivation_id, SYNTAX_TREE_DERIVATION_ID);
        assert_eq!(contract.algorithm_version, "1.0");
        assert_eq!(contract.precision_profile, "CORE_SOURCE_V1");
        assert_eq!(contract.replacement_scope, "OWNER_REPLACE");
        assert_eq!(
            contract.dependency_rule,
            "source-file->syntax-detail+AST_CHILD"
        );
    }

    #[test]
    fn wp37_operational_acceptance() {
        let output = derive_syntax_tree_inputs(
            &[detail(1, 1), detail(1, 2), detail(2, 3), detail(2, 4)],
            &[relation(2, 3, 4, 0), relation(1, 1, 2, 0)],
        )
        .unwrap();
        assert_eq!(output.rows_for_owner([1; 16]).len(), 1);
        assert_eq!(output.rows_for_owner([2; 16]).len(), 1);
        assert!(output.rows_for_owner([9; 16]).is_empty());
        assert!(output.rows.iter().all(|row| {
            row.derivation_id == SYNTAX_TREE_DERIVATION_ID
                && row.algorithm_version == "1.0"
                && row.input_digest != [0; 32]
        }));
    }
}
