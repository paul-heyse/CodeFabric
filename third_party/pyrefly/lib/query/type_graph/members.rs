/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Native associated fields, independent from effective inherited or instance lookup.

use ruff_python_ast::Identifier;
use ruff_text_size::Ranged;
use starlark_map::Hashed;
use std::collections::VecDeque;

use super::{CalleeDefinition, GraphBuilder, NativeTypeKind, Type, TypeShapeContext};
use crate::alt::answers::Answers;
use crate::binding::binding::{ClassFieldDefinition, KeyClassField};
use crate::binding::bindings::Bindings;

#[derive(Clone, Debug)]
pub struct NativeMember {
    pub name: String,
    pub ordinal: usize,
    pub definition: Option<CalleeDefinition>,
    pub definition_kind: &'static str,
    pub native_kind: Option<&'static str>,
    pub declared_type_index: Option<usize>,
    pub computed_type_index: Option<usize>,
    pub annotation_present: Option<bool>,
    pub is_property: Option<bool>,
    pub is_class_var: Option<bool>,
    pub is_final: Option<bool>,
    pub is_abstract: Option<bool>,
    pub descriptor_has_set: Option<bool>,
    pub descriptor_has_delete: Option<bool>,
    pub annotation_rendering: Option<String>,
    pub unknown_reason: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct NativeClassMembers {
    pub name: String,
    pub definition: CalleeDefinition,
    pub expected_members: Option<usize>,
    pub members: Vec<NativeMember>,
    pub complete: bool,
    pub unknown_reason: Option<&'static str>,
}

/// Structural identities can represent error types. Propagate incomplete components once so
/// an error-bearing signature or container cannot masquerade as a fully known member type.
pub(super) fn qualify(graph: &mut GraphBuilder) {
    if graph.classes.iter().all(|class| class.members.is_empty()) {
        return;
    }
    let mut parents = vec![Vec::new(); graph.nodes.len()];
    let mut incomplete = vec![false; graph.nodes.len()];
    for (index, node) in graph.nodes.iter().enumerate() {
        incomplete[index] = matches!(
            node.kind,
            NativeTypeKind::Unknown | NativeTypeKind::Error | NativeTypeKind::Unsupported
        );
        for component in &node.components {
            if let Some(parent_list) = component.target.and_then(|target| parents.get_mut(target)) {
                parent_list.push(index);
            } else {
                incomplete[index] = true;
            }
        }
    }
    let mut pending = incomplete
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(index) = pending.pop_front() {
        for &parent in &parents[index] {
            if !incomplete[parent] {
                incomplete[parent] = true;
                pending.push_back(parent);
            }
        }
    }
    let unknown = |index: Option<usize>| {
        index
            .and_then(|index| incomplete.get(index))
            .is_none_or(|value| *value)
    };
    for class in &mut graph.classes {
        for member in &mut class.members {
            if member.unknown_reason.is_none() {
                if unknown(member.computed_type_index) {
                    member.unknown_reason = Some("native_member_computed_type_incomplete");
                } else if member.annotation_present == Some(true)
                    && unknown(member.declared_type_index)
                {
                    member.unknown_reason = Some("native_member_declared_type_incomplete");
                }
            }
        }
    }
}

pub(super) fn collect(
    context: &TypeShapeContext,
    name: &Identifier,
    ty: Option<&Type>,
    bindings: &Bindings,
    answers: &Answers,
    graph: &mut GraphBuilder,
) {
    if graph.member_rows >= graph.maximum_nodes {
        graph.member_census_complete = false;
        return;
    }
    graph.member_rows += 1;
    let mut scope = NativeClassMembers {
        name: name.id.to_string(),
        definition: CalleeDefinition {
            path: context.source_handle.path().as_path().to_path_buf(),
            start_byte: name.start().to_u32(),
            end_byte: name.end().to_u32(),
        },
        expected_members: None,
        members: vec![],
        complete: false,
        unknown_reason: Some("native_class_definition_unavailable"),
    };
    if let Some(Type::ClassDef(class)) = ty
        && class.module_path() == context.source_handle.path()
        && class.qname().range() == name.range()
        && let Some(fields) = bindings.get_class_fields(class.index())
    {
        scope.expected_members = Some(fields.len());
        for (ordinal, field_name) in fields.names().enumerate() {
            if graph.member_rows >= graph.maximum_nodes {
                graph.member_census_complete = false;
                break;
            }
            graph.member_rows += 1;
            let key = KeyClassField(class.index(), field_name.clone());
            let mut row = NativeMember {
                name: field_name.to_string(),
                ordinal,
                definition: None,
                definition_kind: "unknown",
                native_kind: None,
                declared_type_index: None,
                computed_type_index: None,
                annotation_present: None,
                is_property: None,
                is_class_var: None,
                is_final: None,
                is_abstract: None,
                descriptor_has_set: None,
                descriptor_has_delete: None,
                annotation_rendering: None,
                unknown_reason: Some("native_member_binding_unavailable"),
            };
            if let Some(index) = bindings.key_to_idx_hashed_opt(Hashed::new(&key)) {
                let binding = bindings.get(index);
                row.definition = (!binding.range.is_empty()).then(|| CalleeDefinition {
                    path: scope.definition.path.clone(),
                    start_byte: binding.range.start().to_u32(),
                    end_byte: binding.range.end().to_u32(),
                });
                let (kind, annotation) = match &binding.definition {
                    ClassFieldDefinition::DeclaredByAnnotation { annotation, .. } => {
                        ("declared-by-annotation", Some(*annotation))
                    }
                    ClassFieldDefinition::DeclaredWithoutAnnotation => {
                        ("declared-without-annotation", None)
                    }
                    ClassFieldDefinition::AssignedInBody { annotation, .. } => {
                        ("assigned-in-body", *annotation)
                    }
                    ClassFieldDefinition::MethodLike { annotation, .. } => {
                        ("method-like", *annotation)
                    }
                    ClassFieldDefinition::NestedClass { .. } => ("nested-class", None),
                    ClassFieldDefinition::DefinedWithoutAssign { .. } => {
                        ("defined-without-assignment", None)
                    }
                    ClassFieldDefinition::DefinedInMethod { annotation, .. } => {
                        ("defined-in-method", *annotation)
                    }
                };
                row.definition_kind = kind;
                row.annotation_present = Some(annotation.is_some());
                if let Some(ty) = annotation
                    .and_then(|index| answers.get_idx(index))
                    .and_then(|annotation| annotation.annotation.ty.clone())
                {
                    row.declared_type_index =
                        graph.index(context, &answers.solver().for_export_boundary(ty));
                }
                if let Some(field) = answers.get_idx(index) {
                    let ty = answers.solver().for_export_boundary(field.ty());
                    row.computed_type_index = graph.index(context, &ty);
                    row.annotation_rendering = Some(super::super::type_to_string(&ty));
                    row.is_property = Some(field.is_property());
                    row.is_class_var = Some(field.is_class_var());
                    row.is_final = Some(field.is_final());
                    row.is_abstract = Some(field.is_abstract());
                    row.native_kind = Some(field.query_kind());
                    if let Some((set, delete)) = field.query_descriptor_write_hooks() {
                        row.descriptor_has_set = Some(set);
                        row.descriptor_has_delete = Some(delete);
                    }
                    row.unknown_reason = if row.computed_type_index.is_none() {
                        Some("native_member_type_graph_limit")
                    } else if row.annotation_present == Some(true)
                        && row.declared_type_index.is_none()
                    {
                        Some("native_member_annotation_unavailable")
                    } else {
                        None
                    };
                } else {
                    row.unknown_reason = Some("native_member_type_unavailable");
                }
            }
            scope.members.push(row);
        }
        scope.complete = scope.members.len() == fields.len();
        scope.unknown_reason = (!scope.complete).then_some("native_member_census_limit");
    }
    graph.classes.push(scope);
}
