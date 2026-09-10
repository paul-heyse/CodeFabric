//! Structural source identity uses canonical parents and sibling order, never native node IDs.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{
    SOURCE_CONTEXT_ID, SourceOccurrenceIdentityInput, semantic_owner_identity,
    source_occurrence_identity,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct Anchor {
    pub local_id: u64,
    pub parent: Option<u64>,
    pub start: u64,
    pub end: u64,
    pub normalized_kind: u16,
    pub ordinal: u32,
    pub depth: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    clippy::struct_field_names,
    reason = "explicit names distinguish the provider join ID from canonical node and parent IDs"
)]
pub(super) struct Identity {
    pub local_id: u64,
    pub entity_id: [u8; 16],
    pub parent_id: Option<[u8; 16]>,
}

pub(super) fn normalize(
    workspace: [u8; 16],
    file: [u8; 16],
    digest: [u8; 32],
    byte_length: u64,
    language: &str,
    nodes: &[Anchor],
) -> Result<Vec<Identity>, String> {
    if !matches!(language, "python" | "rust")
        || nodes.len() as u64 > super::super::super::INPROCESS_MAX_VISITED_NODES
    {
        return Err("syntax identity input exceeds its language or node bound".into());
    }
    if nodes.is_empty() {
        return Ok(Vec::new());
    }
    let mut owner_key = language.as_bytes().to_vec();
    owner_key.push(0);
    owner_key.extend_from_slice(&file);
    owner_key.extend_from_slice(&digest);
    let owner = semantic_owner_identity(workspace, SOURCE_CONTEXT_ID, "syntax-tree", owner_key)
        .map_err(|error| error.to_string())?
        .id;
    let mut ordered = nodes.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|node| (node.depth, node.ordinal));
    let mut normalized = BTreeMap::<u64, (&Anchor, [u8; 16])>::new();
    let mut siblings = BTreeSet::new();
    let mut roots = 0;
    let mut result = Vec::with_capacity(nodes.len());
    for node in ordered {
        if node.local_id == 0
            || node.normalized_kind == 0
            || node.start > node.end
            || node.end > byte_length
            || !siblings.insert((node.parent, node.ordinal))
            || normalized.contains_key(&node.local_id)
        {
            return Err("invalid syntax occurrence coordinates or sibling identity".into());
        }
        let parent_id = if let Some(parent) = node.parent {
            let (anchor, id) = normalized
                .get(&parent)
                .ok_or("syntax parent missing or cyclic")?;
            if anchor.depth.checked_add(1) != Some(node.depth)
                || node.start < anchor.start
                || node.end > anchor.end
            {
                return Err("syntax parent does not contain its child".into());
            }
            Some(*id)
        } else {
            roots += 1;
            if roots != 1 || node.depth != 0 || node.ordinal != 0 {
                return Err("syntax tree requires one canonical root".into());
            }
            None
        };
        let entity_id = source_occurrence_identity(SourceOccurrenceIdentityInput {
            workspace_id: workspace,
            file_id: file,
            source_digest: digest,
            start_byte: node.start,
            end_byte: node.end,
            owner_id: owner,
            entity_kind_code: 105,
            occurrence_family_code: 6,
            normalized_kind_code: u32::from(node.normalized_kind),
            parent_id,
            role_code: None,
            ordinal: node.ordinal,
        })
        .map_err(|error| error.to_string())?
        .id;
        normalized.insert(node.local_id, (node, entity_id));
        result.push(Identity {
            local_id: node.local_id,
            entity_id,
            parent_id,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_ids_ignore_provider_numbering_and_separate_zero_width_siblings() {
        let root = Anchor {
            local_id: 1,
            parent: None,
            start: 0,
            end: 7,
            normalized_kind: 10,
            ordinal: 0,
            depth: 0,
        };
        let first = Anchor {
            local_id: 2,
            parent: Some(1),
            start: 7,
            end: 7,
            normalized_kind: 10,
            ordinal: 0,
            depth: 1,
        };
        let second = Anchor {
            local_id: 3,
            ordinal: 1,
            ..first
        };
        let run =
            |nodes: &[Anchor]| normalize([1; 16], [2; 16], [3; 32], 7, "python", nodes).unwrap();
        let before = run(&[root, first, second]);
        let after = run(&[
            Anchor {
                local_id: 12,
                parent: Some(91),
                ..second
            },
            Anchor {
                local_id: 91,
                ..root
            },
            Anchor {
                local_id: 5,
                parent: Some(91),
                ..first
            },
        ]);
        assert_eq!(
            before
                .iter()
                .map(|node| (node.entity_id, node.parent_id))
                .collect::<Vec<_>>(),
            after
                .iter()
                .map(|node| (node.entity_id, node.parent_id))
                .collect::<Vec<_>>()
        );
        assert_ne!(before[1].entity_id, before[2].entity_id);
        assert_eq!(before[1].parent_id, Some(before[0].entity_id));
        for invalid in [
            Anchor {
                parent: Some(999),
                ..first
            },
            Anchor { end: 8, ..first },
            Anchor { depth: 0, ..first },
            Anchor {
                local_id: 1,
                ..first
            },
        ] {
            assert!(normalize([1; 16], [2; 16], [3; 32], 7, "python", &[root, invalid]).is_err());
        }
        assert!(
            normalize(
                [1; 16],
                [2; 16],
                [3; 32],
                7,
                "python",
                &[
                    root,
                    first,
                    Anchor {
                        ordinal: 0,
                        ..second
                    }
                ]
            )
            .is_err()
        );
        assert_ne!(
            before[0].entity_id,
            normalize([1; 16], [2; 16], [3; 32], 7, "rust", &[root]).unwrap()[0].entity_id
        );
    }
}
