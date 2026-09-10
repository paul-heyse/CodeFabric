//! Application type terms over a bounded native graph. Tags are version-one constructor fields:
//! 1 is language, 2 is constructor data, and 3 is ordered application arguments where present.
//! Native names/hashes, graph indices, runs and source generations do not become type identity.

use std::collections::BTreeMap;

use crate::identity::{
    CbefField, CbefValue, InternedType, StringNormalization, TypeConstructor, TypeInterner,
    TypeTerm,
};
use petgraph::algo::kosaraju_scc;
use petgraph::graph::DiGraph;

pub(super) struct Node<'a> {
    pub index: u64,
    pub kind: &'a str,
    pub intrinsic: Option<&'a str>,
    pub definition: Option<[u8; 16]>,
    pub style: Option<&'a str>,
    pub literal: Option<CbefValue>,
    pub components: Vec<Component<'a>>,
}

pub(super) struct Component<'a> {
    pub role: &'a str,
    pub ordinal: u64,
    pub target: Option<u64>,
    pub parameter_kind: Option<&'a str>,
    pub parameter_name: Option<&'a str>,
    pub parameter_required: Option<bool>,
}

#[derive(Clone, Debug)]
pub(super) struct NodeIdentity {
    pub index: u64,
    pub identity: Option<InternedType>,
    pub unknown_reason: Option<&'static str>,
}

pub(super) fn normalize(
    workspace: [u8; 16],
    context: [u8; 16],
    nodes: &[Node<'_>],
) -> Result<Vec<NodeIdentity>, String> {
    if nodes.len() > 1_000_000 {
        return Err("canonical type graph exceeds the node bound".to_owned());
    }
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.index, index))
        .collect::<BTreeMap<_, _>>();
    if indices.len() != nodes.len() {
        return Err("duplicate native type graph index".to_owned());
    }
    let edge_count = nodes
        .iter()
        .map(|node| node.components.len())
        .sum::<usize>();
    if edge_count > 1_000_000 {
        return Err("canonical type graph exceeds the edge bound".to_owned());
    }
    let mut graph = DiGraph::<(), ()>::with_capacity(nodes.len(), edge_count);
    let vertices = nodes.iter().map(|_| graph.add_node(())).collect::<Vec<_>>();
    for (index, node) in nodes.iter().enumerate() {
        for component in &node.components {
            if let Some(target) = component.target.and_then(|target| indices.get(&target)) {
                graph.add_edge(vertices[index], vertices[*target], ());
            }
        }
    }
    let mut output = nodes
        .iter()
        .map(|node| NodeIdentity {
            index: node.index,
            identity: None,
            unknown_reason: None,
        })
        .collect::<Vec<_>>();
    let mut interner = TypeInterner::default();
    for component in kosaraju_scc(&graph) {
        if component.len() != 1 || graph.contains_edge(component[0], component[0]) {
            for vertex in component {
                output[vertex.index()].unknown_reason =
                    Some("recursive_type_normalization_unavailable");
            }
            continue;
        }
        let index = component[0].index();
        let node = &nodes[index];
        let term = term(node, |target| {
            indices
                .get(&target)
                .and_then(|index| output[*index].identity.as_ref())
                .map(|identity| identity.type_id)
        });
        match term {
            Ok(term) => {
                output[index].identity = Some(
                    interner
                        .intern_type(workspace, context, &term)
                        .map_err(|error| error.to_string())?,
                );
                output[index].unknown_reason = match node.kind {
                    "error" => Some("native_type_error"),
                    "unknown" => Some("native_type_unknown"),
                    _ => node
                        .components
                        .iter()
                        .any(|component| {
                            component
                                .target
                                .and_then(|target| indices.get(&target))
                                .is_some_and(|index| output[*index].unknown_reason.is_some())
                        })
                        .then_some("type_component_unknown"),
                };
            }
            Err(reason) => output[index].unknown_reason = Some(reason),
        }
    }
    Ok(output)
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed constructor mapping keeps versioned CBEF fields and validation together"
)]
fn term(
    node: &Node<'_>,
    target: impl Fn(u64) -> Option<[u8; 16]>,
) -> Result<TypeTerm, &'static str> {
    use TypeConstructor as C;
    let mut components = node.components.iter().collect::<Vec<_>>();
    components.sort_by_key(|component| (component.role, component.ordinal));
    if components
        .windows(2)
        .any(|pair| (pair[0].role, pair[0].ordinal) == (pair[1].role, pair[1].ordinal))
    {
        return Err("duplicate_type_component");
    }
    let allowed: &[&str] = match (node.kind, node.style) {
        ("nominal", _) => &["argument"],
        ("type-object", _) | ("tuple", Some("concrete")) => &["element"],
        ("union", _) => &["member"],
        ("intersection", _) => &["member", "fallback"],
        ("tuple", Some("unbounded")) => &["variadic"],
        ("tuple", Some("unpacked")) => &["prefix", "variadic", "suffix"],
        ("callable" | "function", _) => &["parameter", "return", "parameter-specification"],
        _ => &[],
    };
    let mut previous = None;
    for component in &components {
        if !allowed.contains(&component.role) {
            return Err("unexpected_type_component_role");
        }
        let expected = previous
            .filter(|(role, _)| *role == component.role)
            .map_or(0, |(_, ordinal): (&str, u64)| ordinal + 1);
        if component.ordinal != expected {
            return Err("type_component_ordinal_gap");
        }
        previous = Some((component.role, component.ordinal));
    }
    let child = |component: &Component<'_>| {
        component
            .target
            .and_then(&target)
            .map(CbefValue::Id)
            .ok_or("type_component_unavailable")
    };
    let children = |role| {
        components
            .iter()
            .filter(|component| component.role == role)
            .map(|component| child(component))
            .collect::<Result<Vec<_>, _>>()
    };
    let one = |role| {
        let values = children(role)?;
        if values.len() == 1 {
            Ok(values[0].clone())
        } else {
            Err("type_component_cardinality")
        }
    };
    let mut fields = vec![field(1, text("python"))];
    let constructor = match node.kind {
        "any" => C::AnyDynamic,
        "error" => C::Error,
        "unknown" => C::Unknown,
        "never" => C::NeverBottom,
        "none" => C::NullNone,
        "nominal" | "class-object" => {
            let definition = if let Some(intrinsic) = node.intrinsic {
                if ![
                    "bool",
                    "int",
                    "float",
                    "complex",
                    "str",
                    "bytes",
                    "bytearray",
                    "object",
                    "list",
                    "dict",
                    "tuple",
                    "set",
                    "frozenset",
                    "type",
                    "slice",
                    "memoryview",
                ]
                .contains(&intrinsic)
                {
                    return Err("unknown_intrinsic_type");
                }
                CbefValue::TaggedUnion {
                    variant: 1,
                    value: Box::new(text(intrinsic)),
                }
            } else {
                CbefValue::TaggedUnion {
                    variant: 2,
                    value: Box::new(CbefValue::Id(
                        node.definition.ok_or("nominal_definition_unavailable")?,
                    )),
                }
            };
            fields.push(field(2, definition));
            let arguments = children("argument")?;
            if node.kind == "class-object" {
                C::ClassObject
            } else if !arguments.is_empty() {
                fields.push(field(3, CbefValue::OrderedList(arguments)));
                C::Generic
            } else if node.intrinsic.is_some_and(|kind| {
                ["bool", "int", "float", "complex", "str", "bytes"].contains(&kind)
            }) {
                C::Primitive
            } else {
                C::Nominal
            }
        }
        "type-object" => {
            fields.push(field(2, one("element")?));
            C::TypeObject
        }
        "union" | "intersection" => {
            fields.push(field(2, CbefValue::Set(children("member")?)));
            if node.kind == "union" {
                C::Union
            } else {
                C::Intersection
            }
        }
        "tuple" => {
            let style = node.style.ok_or("tuple_style_unavailable")?;
            let roles: &[&str] = match style {
                "concrete" => &["element"],
                "unbounded" => &["variadic"],
                "unpacked" => &["prefix", "variadic", "suffix"],
                _ => return Err("tuple_style_unavailable"),
            };
            let mut values = vec![text(style)];
            for role in roles {
                if *role == "variadic" {
                    values.push(one(role)?);
                } else {
                    values.push(CbefValue::OrderedList(children(role)?));
                }
            }
            fields.push(field(2, CbefValue::OrderedList(values)));
            C::Tuple
        }
        "callable" | "function" => {
            let style = node.style.ok_or("callable_parameter_style_unavailable")?;
            if !["list", "ellipsis"].contains(&style) {
                return Err("callable_parameter_style_unavailable");
            }
            let parameters = components
                .iter()
                .filter(|component| component.role == "parameter")
                .map(|component| {
                    let kind = component
                        .parameter_kind
                        .ok_or("callable_parameter_kind_unavailable")?;
                    if ![
                        "positional-only",
                        "positional-or-keyword",
                        "keyword-only",
                        "variadic-positional",
                        "variadic-keyword",
                    ]
                    .contains(&kind)
                    {
                        return Err("callable_parameter_kind_unavailable");
                    }
                    let name = if matches!(kind, "positional-or-keyword" | "keyword-only") {
                        text(
                            component
                                .parameter_name
                                .ok_or("callable_parameter_name_unavailable")?,
                        )
                    } else {
                        CbefValue::Absent
                    };
                    if !kind.starts_with("variadic-") && component.parameter_required.is_none() {
                        return Err("callable_parameter_requiredness_unavailable");
                    }
                    Ok(CbefValue::OrderedList(vec![
                        text(kind),
                        name,
                        component
                            .parameter_required
                            .map_or(CbefValue::Absent, CbefValue::Boolean),
                        child(component)?,
                    ]))
                })
                .collect::<Result<Vec<_>, _>>()?;
            fields.push(field(
                2,
                CbefValue::OrderedList(vec![
                    text(style),
                    CbefValue::OrderedList(parameters),
                    one("return")?,
                ]),
            ));
            if node.kind == "function" {
                fields.push(field(
                    3,
                    CbefValue::Id(
                        node.definition
                            .ok_or("function_type_definition_unavailable")?,
                    ),
                ));
                C::FunctionDefinition
            } else {
                C::Callable
            }
        }
        "literal" => {
            fields.push(field(
                2,
                node.literal.clone().ok_or("literal_value_unavailable")?,
            ));
            C::Literal
        }
        _ => return Err("native_type_constructor_unavailable"),
    };
    Ok(TypeTerm {
        constructor,
        fields,
    })
}

fn field(tag: u16, value: CbefValue) -> CbefField {
    CbefField { tag, value }
}

pub(super) fn text(value: &str) -> CbefValue {
    CbefValue::Utf8 {
        value: value.to_owned(),
        normalization: StringNormalization::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(index: u64, kind: &'static str) -> Node<'static> {
        Node {
            index,
            kind,
            intrinsic: None,
            definition: None,
            style: None,
            literal: None,
            components: vec![],
        }
    }
    fn edge(role: &'static str, ordinal: u64, target: u64) -> Component<'static> {
        Component {
            role,
            ordinal,
            target: Some(target),
            parameter_kind: None,
            parameter_name: None,
            parameter_required: None,
        }
    }
    fn primitive(index: u64, name: &'static str) -> Node<'static> {
        Node {
            intrinsic: Some(name),
            ..node(index, "nominal")
        }
    }
    fn ids(nodes: &[Node<'_>], context: u8) -> BTreeMap<u64, NodeIdentity> {
        normalize([6; 16], [context; 16], nodes)
            .unwrap()
            .into_iter()
            .map(|node| (node.index, node))
            .collect()
    }
    fn id(nodes: &BTreeMap<u64, NodeIdentity>, index: u64) -> [u8; 16] {
        nodes[&index].identity.as_ref().unwrap().type_id
    }

    #[test]
    fn canonical_type_keys_preserve_structure_and_ignore_graph_order() {
        let mut nodes = vec![primitive(1, "int"), primitive(2, "str")];
        for (index, kind, style, role, targets) in [
            (3, "union", None, "member", vec![1, 2]),
            (4, "union", None, "member", vec![2, 1]),
            (5, "tuple", Some("concrete"), "element", vec![1, 2]),
            (6, "tuple", Some("concrete"), "element", vec![2, 1]),
            (7, "tuple", Some("unbounded"), "variadic", vec![1]),
            (8, "tuple", Some("concrete"), "element", vec![1]),
            (9, "type-object", None, "element", vec![1]),
        ] {
            nodes.push(Node {
                style,
                components: targets
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, target)| edge(role, ordinal as u64, target))
                    .collect(),
                ..node(index, kind)
            });
        }
        nodes.push(Node {
            intrinsic: Some("int"),
            ..node(10, "class-object")
        });
        nodes.extend([node(11, "any"), node(12, "error"), node(13, "unknown")]);
        nodes.push(Node {
            literal: Some(CbefValue::TaggedUnion {
                variant: 1,
                value: Box::new(text("1")),
            }),
            ..node(14, "literal")
        });
        nodes.push(Node {
            literal: Some(CbefValue::TaggedUnion {
                variant: 3,
                value: Box::new(text("1")),
            }),
            ..node(15, "literal")
        });
        let result = ids(&nodes, 9);
        assert_eq!(id(&result, 3), id(&result, 4));
        for (left, right) in [
            (1, 2),
            (5, 6),
            (7, 8),
            (9, 10),
            (11, 12),
            (11, 13),
            (14, 15),
        ] {
            assert_ne!(id(&result, left), id(&result, right));
        }
        nodes.reverse();
        let reordered = ids(&nodes, 9);
        let another_context = ids(&nodes, 10);
        for index in 1..=15 {
            assert_eq!(id(&result, index), id(&reordered, index));
            assert_ne!(id(&result, index), id(&another_context, index));
        }
    }

    #[test]
    fn cyclic_and_missing_type_components_do_not_discard_independent_types() {
        let nodes = vec![
            primitive(1, "int"),
            Node {
                style: Some("concrete"),
                components: vec![edge("element", 0, 2)],
                ..node(2, "tuple")
            },
            Node {
                style: Some("concrete"),
                components: vec![edge("element", 0, 2)],
                ..node(3, "tuple")
            },
            Node {
                style: Some("concrete"),
                components: vec![edge("element", 0, 99)],
                ..node(4, "tuple")
            },
            Node {
                style: Some("concrete"),
                components: vec![edge("element", 0, 1)],
                ..node(5, "tuple")
            },
            node(6, "nominal"),
            Node {
                style: Some("concrete"),
                components: vec![edge("element", 1, 1)],
                ..node(7, "tuple")
            },
        ];
        let result = ids(&nodes, 9);
        assert!(result[&1].identity.is_some() && result[&5].identity.is_some());
        assert_eq!(
            result[&2].unknown_reason,
            Some("recursive_type_normalization_unavailable")
        );
        for index in [3, 4] {
            assert_eq!(
                result[&index].unknown_reason,
                Some("type_component_unavailable")
            );
        }
        assert_eq!(
            result[&6].unknown_reason,
            Some("nominal_definition_unavailable")
        );
        assert_eq!(
            result[&7].unknown_reason,
            Some("type_component_ordinal_gap")
        );
        assert!(result[&2].identity.is_none());
    }

    #[test]
    fn callable_parameter_semantics_and_function_identity_remain_distinct() {
        let callable = |index, name, kind, required| Node {
            style: Some("list"),
            components: vec![
                Component {
                    parameter_kind: Some(kind),
                    parameter_name: Some(name),
                    parameter_required: Some(required),
                    ..edge("parameter", 0, 1)
                },
                edge("return", 0, 1),
            ],
            ..node(index, "callable")
        };
        let mut function = callable(7, "value", "positional-only", true);
        function.kind = "function";
        function.definition = Some([8; 16]);
        let nodes = vec![
            primitive(1, "int"),
            callable(2, "value", "positional-only", true),
            callable(3, "renamed", "positional-only", true),
            callable(4, "value", "keyword-only", true),
            callable(5, "renamed", "keyword-only", true),
            callable(6, "value", "positional-only", false),
            function,
        ];
        let result = ids(&nodes, 9);
        assert_eq!(id(&result, 2), id(&result, 3));
        for (left, right) in [(2, 4), (4, 5), (2, 6), (2, 7)] {
            assert_ne!(id(&result, left), id(&result, right));
        }
    }
}
