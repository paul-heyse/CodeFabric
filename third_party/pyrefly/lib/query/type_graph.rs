/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Native type structure alongside the existing presentation table, from one checker traversal.
//! Local indices are graph references, never application identities. Unsupported native shapes
//! retain their discriminant; they are not silently projected to Any or a similarly named class.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_types::callable::{Callable, FunctionKind, Param, Params, Required};
use pyrefly_types::class::Class;
use pyrefly_types::literal::{Lit, LitStyle};
use pyrefly_types::tuple::Tuple;
use pyrefly_types::types::{AnyStyle, NeverStyle, Type};
use pyrefly_util::lined_buffer::PythonASTRange;
use starlark_map::Hashed;

use super::Handle;
use super::{
    CalleeDefinition, Query, TypeShapeContext, TypeTableBuilder, TypeTableResponseData,
    located_type_table_refs, qname_to_string, type_to_indexed_shape,
};
use crate::binding::binding::KeyUndecoratedFunctionRange;
use ruff_text_size::Ranged;

mod declarations;
mod members;
pub use members::{NativeClassMembers, NativeMember};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTypeKind {
    Any,
    Error,
    Unknown,
    Never,
    None,
    Nominal,
    ClassObject,
    TypeObject,
    Union,
    Intersection,
    Tuple,
    Callable,
    Function,
    Literal,
    Unsupported,
}

#[derive(Clone, Debug)]
pub enum NativeTypeLiteral {
    String(String),
    Bytes(Vec<u8>),
    Integer(String),
    Boolean(bool),
}

#[derive(Clone, Debug)]
pub struct NativeTypeComponent {
    pub role: &'static str,
    pub ordinal: usize,
    pub target: Option<usize>,
    pub parameter_kind: Option<&'static str>,
    pub parameter_name: Option<String>,
    pub parameter_required: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct NativeTypeNode {
    pub kind: NativeTypeKind,
    pub native_kind: &'static str,
    pub name: Option<String>,
    pub intrinsic: Option<&'static str>,
    pub definition: Option<CalleeDefinition>,
    pub literal: Option<NativeTypeLiteral>,
    pub style: Option<&'static str>,
    pub components: Vec<NativeTypeComponent>,
}

impl NativeTypeNode {
    fn new(kind: NativeTypeKind, native_kind: &'static str) -> Self {
        Self {
            kind,
            native_kind,
            name: None,
            intrinsic: None,
            definition: None,
            literal: None,
            style: None,
            components: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct NativeTypeOccurrence {
    pub location: PythonASTRange,
    pub type_index: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct NativeTypeGraph {
    pub nodes: Vec<NativeTypeNode>,
    pub types: Vec<NativeTypeOccurrence>,
    pub complete: bool,
    pub classes: Vec<NativeClassMembers>,
    pub member_census_complete: bool,
}

pub struct TypeFactsResponseData {
    pub presentation: TypeTableResponseData,
    pub structural: NativeTypeGraph,
}

impl Query {
    /// Preserve the existing shape observations while exposing native structure for canonical
    /// normalization. Both exports share the same transaction, answers and occurrence census.
    pub fn get_type_facts_in_file(
        &self,
        name: ModuleName,
        path: ModulePath,
        maximum_nodes: usize,
    ) -> Option<TypeFactsResponseData> {
        let presentation = RefCell::new(TypeTableBuilder::new());
        let structural = RefCell::new(GraphBuilder::new(maximum_nodes));
        let finish = |context: &TypeShapeContext, body: &[ruff_python_ast::Stmt]| {
            declarations::collect(context, body, &mut structural.borrow_mut());
        };
        let types = self.get_types_in_file_with_optional_timing(
            name,
            path,
            None,
            |context, ty| {
                (
                    type_to_indexed_shape(context, ty, &mut presentation.borrow_mut()),
                    structural.borrow_mut().index(context, ty),
                )
            },
            None,
            Some(&finish),
        )?;
        let (old, native): (Vec<_>, Vec<_>) = types
            .into_iter()
            .map(|(location, (old, native))| {
                (
                    (location.clone(), old),
                    NativeTypeOccurrence {
                        location,
                        type_index: native,
                    },
                )
            })
            .unzip();
        let mut structural = structural.into_inner();
        members::qualify(&mut structural);
        Some(TypeFactsResponseData {
            presentation: TypeTableResponseData {
                type_table: presentation.into_inner().into_type_table(),
                types: located_type_table_refs(old),
            },
            structural: NativeTypeGraph {
                nodes: structural.nodes,
                types: native,
                complete: structural.complete,
                classes: structural.classes,
                member_census_complete: structural.member_census_complete,
            },
        })
    }
}

struct GraphBuilder {
    seen: HashMap<Type, usize>,
    pending: VecDeque<(usize, Type)>,
    nodes: Vec<NativeTypeNode>,
    maximum_nodes: usize,
    complete: bool,
    classes: Vec<NativeClassMembers>,
    member_rows: usize,
    member_census_complete: bool,
}

impl GraphBuilder {
    fn new(maximum_nodes: usize) -> Self {
        Self {
            seen: HashMap::new(),
            pending: VecDeque::new(),
            nodes: vec![],
            maximum_nodes,
            complete: true,
            classes: vec![],
            member_rows: 0,
            member_census_complete: true,
        }
    }

    fn reserve(&mut self, ty: &Type) -> Option<usize> {
        if let Some(index) = self.seen.get(ty) {
            return Some(*index);
        }
        if self.nodes.len() >= self.maximum_nodes {
            self.complete = false;
            return None;
        }
        let index = self.nodes.len();
        self.seen.insert(ty.clone(), index);
        self.nodes
            .push(NativeTypeNode::new(NativeTypeKind::Unknown, "Pending"));
        self.pending.push_back((index, ty.clone()));
        Some(index)
    }

    fn index(&mut self, context: &TypeShapeContext, ty: &Type) -> Option<usize> {
        let root = self.reserve(ty);
        // Reserving before expansion permits recursive native edges without recursive encoding.
        while let Some((index, ty)) = self.pending.pop_front() {
            self.nodes[index] = self.describe(context, &ty);
        }
        root
    }

    fn component(
        &mut self,
        node: &mut NativeTypeNode,
        role: &'static str,
        ordinal: usize,
        ty: &Type,
    ) {
        node.components.push(NativeTypeComponent {
            role,
            ordinal,
            target: self.reserve(ty),
            parameter_kind: None,
            parameter_name: None,
            parameter_required: None,
        });
    }

    fn class(&mut self, node: &mut NativeTypeNode, class: &Class) {
        node.name = Some(qname_to_string(class.qname()));
        node.definition = Some(CalleeDefinition {
            path: class.module_path().as_path().to_path_buf(),
            start_byte: class.qname().range().start().to_u32(),
            end_byte: class.qname().range().end().to_u32(),
        });
        node.intrinsic = [
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
        .into_iter()
        .find(|name| class.is_builtin(name));
    }

    fn describe(&mut self, context: &TypeShapeContext, ty: &Type) -> NativeTypeNode {
        use NativeTypeKind as K;
        let mut node = NativeTypeNode::new(K::Unsupported, native_kind(ty));
        match ty {
            Type::Any(style) => {
                node.kind = if *style == AnyStyle::Error {
                    K::Error
                } else {
                    K::Any
                };
                node.style = Some(match style {
                    AnyStyle::Explicit => "explicit",
                    AnyStyle::Implicit => "implicit",
                    AnyStyle::Error => "error",
                });
            }
            Type::Var(_) => node.kind = K::Unknown,
            Type::Never(style) => {
                node.kind = K::Never;
                node.style = Some(match style {
                    NeverStyle::NoReturn => "no-return",
                    NeverStyle::Never => "never",
                });
            }
            Type::None => node.kind = K::None,
            Type::ClassType(class) => {
                node.kind = K::Nominal;
                self.class(&mut node, class.class_object());
                for (i, arg) in class.targs().as_slice().iter().enumerate() {
                    self.component(&mut node, "argument", i, arg);
                }
            }
            Type::ClassDef(class) => {
                node.kind = K::ClassObject;
                self.class(&mut node, class);
            }
            Type::Type(inner) => {
                node.kind = K::TypeObject;
                self.component(&mut node, "element", 0, inner);
            }
            Type::Union(union) => {
                node.kind = K::Union;
                for (i, member) in union.members.iter().enumerate() {
                    self.component(&mut node, "member", i, member);
                }
            }
            Type::Intersect(intersection) => {
                node.kind = K::Intersection;
                for (i, member) in intersection.0.iter().enumerate() {
                    self.component(&mut node, "member", i, member);
                }
                self.component(&mut node, "fallback", 0, &intersection.1);
            }
            Type::Tuple(tuple) => {
                node.kind = K::Tuple;
                match tuple {
                    Tuple::Concrete(elements) => {
                        node.style = Some("concrete");
                        for (i, element) in elements.iter().enumerate() {
                            self.component(&mut node, "element", i, element);
                        }
                    }
                    Tuple::Unbounded(element) => {
                        node.style = Some("unbounded");
                        self.component(&mut node, "variadic", 0, element);
                    }
                    Tuple::Unpacked(unpacked) => {
                        node.style = Some("unpacked");
                        for (i, element) in unpacked.0.iter().enumerate() {
                            self.component(&mut node, "prefix", i, element);
                        }
                        self.component(&mut node, "variadic", 0, &unpacked.1);
                        for (i, element) in unpacked.2.iter().enumerate() {
                            self.component(&mut node, "suffix", i, element);
                        }
                    }
                }
            }
            Type::Callable(callable) => {
                node.kind = K::Callable;
                self.callable(&mut node, callable);
            }
            Type::Function(function) => {
                node.kind = K::Function;
                node.definition = function_definition(context, &function.metadata.kind);
                self.callable(&mut node, &function.signature);
            }
            Type::Literal(literal) => {
                node.kind = K::Literal;
                node.style = Some(match literal.style {
                    LitStyle::Explicit => "explicit",
                    LitStyle::Implicit => "implicit",
                });
                node.literal = match &literal.value {
                    Lit::Str(value) => Some(NativeTypeLiteral::String(value.to_string())),
                    Lit::Bytes(value) => Some(NativeTypeLiteral::Bytes(value.to_vec())),
                    // Exact arbitrary precision integer scalar; never a type display rendering.
                    Lit::Int(value) => Some(NativeTypeLiteral::Integer(value.to_string())),
                    Lit::Bool(value) => Some(NativeTypeLiteral::Boolean(*value)),
                    Lit::Enum(_) => {
                        node.kind = K::Unsupported;
                        None
                    }
                };
            }
            _ => {}
        }
        node
    }

    fn callable(&mut self, node: &mut NativeTypeNode, callable: &Callable) {
        node.style = Some(match &callable.params {
            Params::List(params) | Params::Partial(params) => {
                for (i, param) in params.items().iter().enumerate() {
                    self.parameter(node, i, param);
                }
                if matches!(callable.params, Params::Partial(_)) {
                    "partial"
                } else {
                    "list"
                }
            }
            Params::Ellipsis => "ellipsis",
            Params::Materialization => "materialization",
            Params::ParamSpec(prefix, spec) => {
                for (i, param) in prefix.iter().enumerate() {
                    self.parameter(node, i, &param.to_param_preserve_name());
                }
                self.component(node, "parameter-specification", 0, spec);
                "parameter-specification"
            }
        });
        self.component(node, "return", 0, &callable.ret);
    }

    fn parameter(&mut self, node: &mut NativeTypeNode, ordinal: usize, param: &Param) {
        let (kind, name, ty, required) = match param {
            Param::PosOnly(name, ty, required) => {
                ("positional-only", name.as_ref(), ty, Some(required))
            }
            Param::Pos(name, ty, required) => {
                ("positional-or-keyword", Some(name), ty, Some(required))
            }
            Param::KwOnly(name, ty, required) => ("keyword-only", Some(name), ty, Some(required)),
            Param::Varargs(name, ty) => ("variadic-positional", name.as_ref(), ty, None),
            Param::Kwargs(name, ty) => ("variadic-keyword", name.as_ref(), ty, None),
        };
        node.components.push(NativeTypeComponent {
            role: "parameter",
            ordinal,
            target: self.reserve(ty),
            parameter_kind: Some(kind),
            parameter_name: name.map(ToString::to_string),
            parameter_required: required.map(|value| matches!(value, Required::Required)),
        });
    }
}

fn function_definition(
    context: &TypeShapeContext,
    kind: &FunctionKind,
) -> Option<CalleeDefinition> {
    let definition = kind.definition_id()?;
    let handle = Handle::new(
        definition.module.name(),
        definition.module.path().clone(),
        context.source_handle.sys_info().clone(),
    );
    let bindings = context.transaction.get_bindings(&handle)?;
    let index = bindings.key_to_idx_hashed_opt(Hashed::new(&KeyUndecoratedFunctionRange(
        definition.def_index?,
    )))?;
    let range = bindings.get(index).0.range();
    Some(CalleeDefinition {
        path: definition.module.path().as_path().to_path_buf(),
        start_byte: range.start().to_u32(),
        end_byte: range.end().to_u32(),
    })
}

// Exhaustive native discriminants make a library upgrade review the exported type universe.
fn native_kind(ty: &Type) -> &'static str {
    match ty {
        Type::Literal(_) => "Literal",
        Type::LiteralString(_) => "LiteralString",
        Type::Callable(_) => "Callable",
        Type::CallableResidual(_) => "CallableResidual",
        Type::TypeLevelDslCall(_) => "TypeLevelDslCall",
        Type::Function(_) => "Function",
        Type::BoundMethod(_) => "BoundMethod",
        Type::Overload(_) => "Overload",
        Type::Union(_) => "Union",
        Type::Intersect(_) => "Intersect",
        Type::ClassDef(_) => "ClassDef",
        Type::ClassType(_) => "ClassType",
        Type::TypedDict(_) => "TypedDict",
        Type::PartialTypedDict(_) => "PartialTypedDict",
        Type::ShapedArray(_) => "ShapedArray",
        Type::IntTuple(_) => "IntTuple",
        Type::NNModule(_) => "NNModule",
        Type::DataFrame(_) => "DataFrame",
        Type::Series(_) => "Series",
        Type::Int(_) => "Int",
        Type::Tuple(_) => "Tuple",
        Type::Module(_) => "Module",
        Type::Forall(_) => "Forall",
        Type::Var(_) => "Var",
        Type::Quantified(_) => "Quantified",
        Type::QuantifiedValue(_) => "QuantifiedValue",
        Type::ElementOfTypeVarTuple(_) => "ElementOfTypeVarTuple",
        Type::TypeGuard(_) => "TypeGuard",
        Type::TypeIs(_) => "TypeIs",
        Type::Annotated(..) => "Annotated",
        Type::Unpack(_) => "Unpack",
        Type::TypeVar(_) => "TypeVar",
        Type::ParamSpec(_) => "ParamSpec",
        Type::TypeVarTuple(_) => "TypeVarTuple",
        Type::SpecialForm(_) => "SpecialForm",
        Type::Concatenate(..) => "Concatenate",
        Type::ParamSpecValue(_) => "ParamSpecValue",
        Type::Args(_) => "Args",
        Type::Kwargs(_) => "Kwargs",
        Type::ArgsValue(_) => "ArgsValue",
        Type::KwargsValue(_) => "KwargsValue",
        Type::Type(_) => "Type",
        Type::TypeForm(_) => "TypeForm",
        Type::Ellipsis => "Ellipsis",
        Type::Any(_) => "Any",
        Type::Never(_) => "Never",
        Type::TypeAlias(_) => "TypeAlias",
        Type::UntypedAlias(_) => "UntypedAlias",
        Type::Sentinel(_) => "Sentinel",
        Type::SuperInstance(_) => "SuperInstance",
        Type::SelfType(_) => "SelfType",
        Type::KwCall(_) => "KwCall",
        Type::Materialization => "Materialization",
        Type::None => "None",
    }
}
