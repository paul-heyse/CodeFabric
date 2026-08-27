//! Ruff AST callable, member, call-site, and argument-binding projection.
//!
//! This module preserves syntax facts without pretending they are cross-module
//! semantic targets. Local argument binding is attempted only when Ruff's
//! lexical binding connects a callee name to one source-declared callable, or
//! when an explicit `self`/`cls` receiver selects a method in the current class.

use std::collections::{BTreeMap, BTreeSet};

use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{ArgOrKeyword, Expr, Parameter, Parameters, Stmt};
use ruff_text_size::{Ranged, TextRange};

use super::semantic::{
    PythonBindingFact, PythonReferenceFact, PythonScopeFact, PythonScopeKind, PythonSemanticId,
    PythonTargetForm, semantic_id,
};

pub(super) const CALLABLE_FLAG_ASYNC: i64 = 1 << 0;
pub(super) const CALLABLE_FLAG_GENERATOR: i64 = 1 << 1;
const CALLABLE_FLAG_LAMBDA: i64 = 1 << 2;
const CALLABLE_FLAG_METHOD: i64 = 1 << 3;
const CALLABLE_FLAG_CLASS_METHOD: i64 = 1 << 4;
const CALLABLE_FLAG_STATIC_METHOD: i64 = 1 << 5;
const CALLABLE_FLAG_PROPERTY: i64 = 1 << 6;
const CALLABLE_FLAG_MODULE_BODY: i64 = 1 << 7;
const PARAMETER_FLAG_HAS_ANNOTATION: i64 = 1 << 0;
const PARAMETER_FLAG_HAS_DEFAULT: i64 = 1 << 1;
const PARAMETER_FLAG_IMPLICIT_RECEIVER: i64 = 1 << 2;
const CALL_FLAG_RESOLUTION_PLACEHOLDER: i64 = 1 << 0;
const CALL_FLAG_DYNAMIC_SPLAT: i64 = 1 << 1;
const CALL_FLAG_BINDING_DIAGNOSTIC: i64 = 1 << 2;

/// Python parameter categories in source-signature order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

/// Source-only dispatch shape. Target resolution remains a WP13 placeholder.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonDispatchKind {
    DirectName,
    Attribute,
    Unknown,
}

/// Per-argument result of the application-owned Python binder.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonArgumentBindingStatus {
    Bound,
    BoundReceiver,
    Defaulted,
    MissingRequired,
    Duplicate,
    PositionalOnlyKeyword,
    TooManyPositional,
    UnmatchedKeyword,
    UnresolvedTarget,
    UnknownArgumentSet,
}

/// Whether an argument is explicit, expanded, or implicit.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonArgumentSpreadKind {
    None,
    PositionalStatic,
    KeywordStatic,
    PositionalDynamic,
    KeywordDynamic,
    BoundReceiver,
    Default,
    Missing,
}

/// Source-declared class member candidates owned by WP05.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonMemberKind {
    Method,
    ClassVariable,
    PropertyCandidate,
    NestedType,
    InstanceVariable,
}

/// Syntax roles retained beside callable semantic facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonCallableSyntaxRole {
    CallExpression,
    CalleeExpression,
    Receiver,
    Argument,
    Decorator,
    ReturnAnnotation,
    ParameterAnnotation,
    ParameterDefault,
    TypeParameter,
}

/// One source callable, including the synthetic module body callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCallableFact {
    pub callable_id: PythonSemanticId,
    pub owner_scope_id: PythonSemanticId,
    pub declared_binding_id: Option<PythonSemanticId>,
    pub class_id: Option<PythonSemanticId>,
    pub name: String,
    pub qualified_name: String,
    pub parameter_count: i32,
    pub generic_parameter_count: i32,
    pub flags: i64,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One formal parameter in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonParameterFact {
    pub parameter_id: PythonSemanticId,
    pub callable_id: PythonSemanticId,
    pub ordinal: i32,
    pub name: String,
    pub kind: PythonParameterKind,
    pub annotation_syntax_id: Option<PythonSemanticId>,
    pub default_syntax_id: Option<PythonSemanticId>,
    pub flags: i64,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One syntax occurrence retained by the semantic profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCallableSyntaxFact {
    pub syntax_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub role: PythonCallableSyntaxRole,
    pub ordinal: Option<i32>,
    pub text: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One first-class call-site entity. Target counts stay unresolved until WP13.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCallSiteFact {
    pub call_site_id: PythonSemanticId,
    pub caller_id: PythonSemanticId,
    pub syntax_id: PythonSemanticId,
    pub callee_syntax_id: PythonSemanticId,
    pub receiver_syntax_id: Option<PythonSemanticId>,
    pub declared_target_id: Option<PythonSemanticId>,
    pub dispatch_kind: PythonDispatchKind,
    pub resolved_target_count: i32,
    pub flags: i64,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One explicit or binder-synthesized argument occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCallArgumentFact {
    pub argument_id: PythonSemanticId,
    pub call_site_id: PythonSemanticId,
    pub ordinal: i32,
    pub keyword_name: Option<String>,
    pub argument_syntax_id: Option<PythonSemanticId>,
    pub parameter_id: Option<PythonSemanticId>,
    pub binding_status: PythonArgumentBindingStatus,
    pub spread_kind: PythonArgumentSpreadKind,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
}

/// One explicit unknown target for an unexpanded dynamic argument set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonUnknownArgumentSetFact {
    pub unknown_argument_set_id: PythonSemanticId,
    pub call_site_id: PythonSemanticId,
    pub spread_kind: PythonArgumentSpreadKind,
}

/// One class-body or `self.x` source member candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonMemberFact {
    pub member_id: PythonSemanticId,
    pub class_id: PythonSemanticId,
    pub declared_entity_id: Option<PythonSemanticId>,
    pub name: String,
    pub kind: PythonMemberKind,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// Deterministic binder diagnostic; no actual argument row is dropped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCallDiagnosticFact {
    pub diagnostic_id: PythonSemanticId,
    pub call_site_id: PythonSemanticId,
    pub argument_id: PythonSemanticId,
    pub code: &'static str,
    pub message: String,
}

pub(super) struct PythonCallableProjection {
    pub callables: Vec<PythonCallableFact>,
    pub parameters: Vec<PythonParameterFact>,
    pub syntax: Vec<PythonCallableSyntaxFact>,
    pub call_sites: Vec<PythonCallSiteFact>,
    pub arguments: Vec<PythonCallArgumentFact>,
    pub unknown_argument_sets: Vec<PythonUnknownArgumentSetFact>,
    pub members: Vec<PythonMemberFact>,
    pub diagnostics: Vec<PythonCallDiagnosticFact>,
}

#[derive(Clone)]
struct RawArgument {
    argument_id: PythonSemanticId,
    keyword_name: Option<String>,
    syntax_id: PythonSemanticId,
    spread_kind: PythonArgumentSpreadKind,
    start_byte: u64,
    end_byte: u64,
}

struct CallDraft {
    fact: PythonCallSiteFact,
    callee_name: Option<String>,
    callee_range: TextRange,
    current_class_id: Option<PythonSemanticId>,
    receiver_is_self_or_cls: bool,
    raw_arguments: Vec<RawArgument>,
}

struct ClassContext {
    class_id: PythonSemanticId,
    qualified_name: String,
    callable_depth: usize,
}

struct CallablePass<'a> {
    source: &'a str,
    module_name: &'a str,
    fingerprint: &'a str,
    scopes: &'a [PythonScopeFact],
    bindings: &'a [PythonBindingFact],
    references: &'a [PythonReferenceFact],
    callables: Vec<PythonCallableFact>,
    parameters: Vec<PythonParameterFact>,
    syntax: Vec<PythonCallableSyntaxFact>,
    calls: Vec<CallDraft>,
    members: Vec<PythonMemberFact>,
    callable_stack: Vec<PythonSemanticId>,
    class_stack: Vec<ClassContext>,
}

#[allow(clippy::too_many_arguments)] // The adapter boundary keeps every pinned input explicit.
pub(super) fn project_python_callables(
    source: &str,
    suite: &[Stmt],
    module_name: &str,
    fingerprint: &str,
    module_id: PythonSemanticId,
    scopes: &[PythonScopeFact],
    bindings: &[PythonBindingFact],
    references: &[PythonReferenceFact],
) -> PythonCallableProjection {
    let module_scope = scopes
        .iter()
        .find(|scope| scope.kind == PythonScopeKind::Module)
        .expect("semantic projection always has a module scope");
    let module_callable = semantic_id(
        fingerprint,
        "callable-module-body",
        0,
        u64::try_from(source.len()).unwrap_or(u64::MAX),
        module_name,
        0,
    );
    let mut pass = CallablePass {
        source,
        module_name,
        fingerprint,
        scopes,
        bindings,
        references,
        callables: vec![PythonCallableFact {
            callable_id: module_callable,
            owner_scope_id: module_scope.scope_id,
            declared_binding_id: None,
            class_id: None,
            name: "<module>".into(),
            qualified_name: format!("{module_name}.<module>"),
            parameter_count: 0,
            generic_parameter_count: 0,
            flags: CALLABLE_FLAG_MODULE_BODY,
            start_byte: 0,
            end_byte: u64::try_from(source.len()).unwrap_or(u64::MAX),
        }],
        parameters: Vec::new(),
        syntax: Vec::new(),
        calls: Vec::new(),
        members: Vec::new(),
        callable_stack: vec![module_callable],
        class_stack: Vec::new(),
    };
    // The module entity is intentionally distinct from its executable body.
    let _ = module_id;
    visitor::walk_body(&mut pass, suite);
    pass.finish()
}

impl CallablePass<'_> {
    fn current_callable(&self) -> PythonSemanticId {
        *self.callable_stack.last().expect("module callable remains")
    }

    fn parent_scope(&self, start: u64, end: u64) -> PythonSemanticId {
        self.scopes
            .iter()
            .filter(|scope| scope.start_byte <= start && end <= scope.end_byte)
            .min_by_key(|scope| scope.end_byte.saturating_sub(scope.start_byte))
            .or_else(|| self.scopes.first())
            .expect("module scope exists")
            .scope_id
    }

    fn binding_at(
        &self,
        name: &str,
        range: TextRange,
        form: PythonTargetForm,
    ) -> Option<PythonSemanticId> {
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        self.bindings
            .iter()
            .find(|binding| {
                binding.name == name
                    && binding.start_byte == start
                    && binding.end_byte == end
                    && binding.target_form == form
            })
            .map(|binding| binding.binding_id)
    }

    fn add_syntax(
        &mut self,
        owner_id: PythonSemanticId,
        role: PythonCallableSyntaxRole,
        ordinal: Option<i32>,
        range: TextRange,
    ) -> PythonSemanticId {
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let syntax_id = semantic_id(
            self.fingerprint,
            "callable-syntax",
            start,
            end,
            callable_syntax_role_name(role),
            ordinal
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
        );
        if !self.syntax.iter().any(|fact| fact.syntax_id == syntax_id) {
            self.syntax.push(PythonCallableSyntaxFact {
                syntax_id,
                owner_id,
                role,
                ordinal,
                text: source_text(self.source, range),
                start_byte: start,
                end_byte: end,
            });
        }
        syntax_id
    }

    fn qualified_callable_name(&self, local_name: &str, direct_class_member: bool) -> String {
        if let Some(class) = self.class_stack.last().filter(|_| direct_class_member) {
            format!("{}.{}", class.qualified_name, local_name)
        } else {
            format!("{}.{}", self.module_name, local_name)
        }
    }

    fn add_parameters(
        &mut self,
        callable_id: PythonSemanticId,
        parameters: &Parameters,
        method: bool,
    ) {
        let mut ordinal = 0_i32;
        for parameter in &parameters.posonlyargs {
            self.add_parameter(
                callable_id,
                ordinal,
                &parameter.parameter,
                parameter.default(),
                PythonParameterKind::PositionalOnly,
                method && ordinal == 0,
            );
            ordinal += 1;
        }
        for parameter in &parameters.args {
            self.add_parameter(
                callable_id,
                ordinal,
                &parameter.parameter,
                parameter.default(),
                PythonParameterKind::PositionalOrKeyword,
                method && ordinal == 0,
            );
            ordinal += 1;
        }
        if let Some(parameter) = parameters.vararg.as_deref() {
            self.add_parameter(
                callable_id,
                ordinal,
                parameter,
                None,
                PythonParameterKind::VarPositional,
                false,
            );
            ordinal += 1;
        }
        for parameter in &parameters.kwonlyargs {
            self.add_parameter(
                callable_id,
                ordinal,
                &parameter.parameter,
                parameter.default(),
                PythonParameterKind::KeywordOnly,
                false,
            );
            ordinal += 1;
        }
        if let Some(parameter) = parameters.kwarg.as_deref() {
            self.add_parameter(
                callable_id,
                ordinal,
                parameter,
                None,
                PythonParameterKind::VarKeyword,
                false,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_parameter(
        &mut self,
        callable_id: PythonSemanticId,
        ordinal: i32,
        parameter: &Parameter,
        default: Option<&Expr>,
        kind: PythonParameterKind,
        implicit_receiver: bool,
    ) {
        let start = u64::from(u32::from(parameter.start()));
        let end = u64::from(u32::from(parameter.end()));
        let parameter_id = semantic_id(
            self.fingerprint,
            "parameter",
            start,
            end,
            parameter.name.as_str(),
            u8::try_from(ordinal).unwrap_or(u8::MAX),
        );
        let annotation_syntax_id = parameter.annotation().map(|annotation| {
            self.add_syntax(
                parameter_id,
                PythonCallableSyntaxRole::ParameterAnnotation,
                None,
                annotation.range(),
            )
        });
        let default_syntax_id = default.map(|expression| {
            self.add_syntax(
                parameter_id,
                PythonCallableSyntaxRole::ParameterDefault,
                None,
                expression.range(),
            )
        });
        let mut flags = 0;
        if annotation_syntax_id.is_some() {
            flags |= PARAMETER_FLAG_HAS_ANNOTATION;
        }
        if default_syntax_id.is_some() {
            flags |= PARAMETER_FLAG_HAS_DEFAULT;
        }
        if implicit_receiver {
            flags |= PARAMETER_FLAG_IMPLICIT_RECEIVER;
        }
        self.parameters.push(PythonParameterFact {
            parameter_id,
            callable_id,
            ordinal,
            name: parameter.name.to_string(),
            kind,
            annotation_syntax_id,
            default_syntax_id,
            flags,
            start_byte: start,
            end_byte: end,
        });
    }

    #[allow(clippy::too_many_lines)] // Callable syntax, flags, and member projection are one source transaction.
    fn add_function(&mut self, node: &ruff_python_ast::StmtFunctionDef) -> PythonSemanticId {
        let start = u64::from(u32::from(node.start()));
        let end = u64::from(u32::from(node.end()));
        let callable_id = semantic_id(
            self.fingerprint,
            "callable",
            start,
            end,
            node.name.as_str(),
            0,
        );
        let class_id = self
            .class_stack
            .last()
            .filter(|class| self.callable_stack.len() == class.callable_depth)
            .map(|class| class.class_id);
        let decorators = node
            .decorator_list
            .iter()
            .filter_map(|decorator| expression_name(&decorator.expression))
            .collect::<BTreeSet<_>>();
        let mut flags = 0;
        if node.is_async {
            flags |= CALLABLE_FLAG_ASYNC;
        }
        if contains_yield(&node.body) {
            flags |= CALLABLE_FLAG_GENERATOR;
        }
        if class_id.is_some() {
            flags |= CALLABLE_FLAG_METHOD;
        }
        if decorators.iter().any(|name| name.ends_with("classmethod")) {
            flags |= CALLABLE_FLAG_CLASS_METHOD;
        }
        if decorators.iter().any(|name| name.ends_with("staticmethod")) {
            flags |= CALLABLE_FLAG_STATIC_METHOD;
        }
        let property = decorators.iter().any(|name| {
            name.ends_with("property") || name.ends_with("setter") || name.ends_with("deleter")
        });
        if property {
            flags |= CALLABLE_FLAG_PROPERTY;
        }
        self.callables.push(PythonCallableFact {
            callable_id,
            owner_scope_id: self.parent_scope(start, end),
            declared_binding_id: self.binding_at(
                node.name.as_str(),
                node.name.range(),
                PythonTargetForm::FunctionName,
            ),
            class_id,
            name: node.name.to_string(),
            qualified_name: self.qualified_callable_name(node.name.as_str(), class_id.is_some()),
            parameter_count: i32::try_from(node.parameters.len()).unwrap_or(i32::MAX),
            generic_parameter_count: node
                .type_params
                .as_deref()
                .map_or(0, |params| i32::try_from(params.len()).unwrap_or(i32::MAX)),
            flags,
            start_byte: start,
            end_byte: end,
        });
        self.add_parameters(
            callable_id,
            &node.parameters,
            class_id.is_some() && flags & CALLABLE_FLAG_STATIC_METHOD == 0,
        );
        for (ordinal, decorator) in node.decorator_list.iter().enumerate() {
            self.add_syntax(
                callable_id,
                PythonCallableSyntaxRole::Decorator,
                i32::try_from(ordinal).ok(),
                decorator.expression.range(),
            );
        }
        if let Some(annotation) = node.returns.as_deref() {
            self.add_syntax(
                callable_id,
                PythonCallableSyntaxRole::ReturnAnnotation,
                None,
                annotation.range(),
            );
        }
        if let Some(params) = node.type_params.as_deref() {
            for (ordinal, parameter) in params.iter().enumerate() {
                self.add_syntax(
                    callable_id,
                    PythonCallableSyntaxRole::TypeParameter,
                    i32::try_from(ordinal).ok(),
                    parameter.range(),
                );
            }
        }
        if let Some(class_id) = class_id {
            self.members.push(PythonMemberFact {
                member_id: semantic_id(
                    self.fingerprint,
                    "member",
                    start,
                    end,
                    node.name.as_str(),
                    if property { 2 } else { 1 },
                ),
                class_id,
                declared_entity_id: Some(callable_id),
                name: node.name.to_string(),
                kind: if property {
                    PythonMemberKind::PropertyCandidate
                } else {
                    PythonMemberKind::Method
                },
                start_byte: start,
                end_byte: end,
            });
        }
        callable_id
    }

    fn add_lambda(&mut self, node: &ruff_python_ast::ExprLambda) -> PythonSemanticId {
        let start = u64::from(u32::from(node.start()));
        let end = u64::from(u32::from(node.end()));
        let callable_id = semantic_id(self.fingerprint, "lambda-callable", start, end, "lambda", 0);
        let parameter_count = node
            .parameters
            .as_deref()
            .map_or(0, |params| i32::try_from(params.len()).unwrap_or(i32::MAX));
        self.callables.push(PythonCallableFact {
            callable_id,
            owner_scope_id: self.parent_scope(start, end),
            declared_binding_id: None,
            class_id: None,
            name: format!("<lambda@{start}>"),
            qualified_name: format!("{}.<lambda@{start}>", self.module_name),
            parameter_count,
            generic_parameter_count: 0,
            flags: CALLABLE_FLAG_LAMBDA,
            start_byte: start,
            end_byte: end,
        });
        if let Some(parameters) = node.parameters.as_deref() {
            self.add_parameters(callable_id, parameters, false);
        }
        callable_id
    }

    fn add_call(&mut self, node: &ruff_python_ast::ExprCall) {
        let caller_id = self.current_callable();
        let start = u64::from(u32::from(node.start()));
        let end = u64::from(u32::from(node.end()));
        let call_site_id = semantic_id(self.fingerprint, "call-site", start, end, "call", 0);
        let syntax_id = self.add_syntax(
            call_site_id,
            PythonCallableSyntaxRole::CallExpression,
            None,
            node.range,
        );
        let callee_syntax_id = self.add_syntax(
            call_site_id,
            PythonCallableSyntaxRole::CalleeExpression,
            None,
            node.func.range(),
        );
        let (callee_name, receiver_range, receiver_is_self_or_cls, dispatch_kind) = match node
            .func
            .as_ref()
        {
            Expr::Name(name) => (
                Some(name.id.to_string()),
                None,
                false,
                PythonDispatchKind::DirectName,
            ),
            Expr::Attribute(attribute) => (
                Some(attribute.attr.to_string()),
                Some(attribute.value.range()),
                matches!(attribute.value.as_ref(), Expr::Name(name) if matches!(name.id.as_str(), "self" | "cls")),
                PythonDispatchKind::Attribute,
            ),
            _ => (None, None, false, PythonDispatchKind::Unknown),
        };
        let receiver_syntax_id = receiver_range.map(|range| {
            self.add_syntax(
                call_site_id,
                PythonCallableSyntaxRole::Receiver,
                None,
                range,
            )
        });
        let raw_arguments = self.raw_arguments(call_site_id, &node.arguments);
        self.calls.push(CallDraft {
            fact: PythonCallSiteFact {
                call_site_id,
                caller_id,
                syntax_id,
                callee_syntax_id,
                receiver_syntax_id,
                declared_target_id: None,
                dispatch_kind,
                resolved_target_count: 0,
                flags: CALL_FLAG_RESOLUTION_PLACEHOLDER,
                start_byte: start,
                end_byte: end,
            },
            callee_name,
            callee_range: node.func.range(),
            current_class_id: self.class_stack.last().map(|class| class.class_id),
            receiver_is_self_or_cls,
            raw_arguments,
        });
    }

    fn raw_arguments(
        &mut self,
        call_site_id: PythonSemanticId,
        arguments: &ruff_python_ast::Arguments,
    ) -> Vec<RawArgument> {
        let mut output = Vec::new();
        for argument in arguments.iter_source_order() {
            match argument {
                ArgOrKeyword::Arg(Expr::Starred(starred)) => {
                    let values = static_positional_splat(&starred.value);
                    if let Some(values) = values {
                        for value in values {
                            self.push_raw_argument(
                                &mut output,
                                call_site_id,
                                None,
                                value,
                                PythonArgumentSpreadKind::PositionalStatic,
                            );
                        }
                    } else {
                        self.push_raw_argument(
                            &mut output,
                            call_site_id,
                            None,
                            &starred.value,
                            PythonArgumentSpreadKind::PositionalDynamic,
                        );
                    }
                }
                ArgOrKeyword::Arg(expression) => self.push_raw_argument(
                    &mut output,
                    call_site_id,
                    None,
                    expression,
                    PythonArgumentSpreadKind::None,
                ),
                ArgOrKeyword::Keyword(keyword) if keyword.arg.is_none() => {
                    if let Some(values) = static_keyword_splat(&keyword.value) {
                        for (name, value) in values {
                            self.push_raw_argument(
                                &mut output,
                                call_site_id,
                                Some(name),
                                value,
                                PythonArgumentSpreadKind::KeywordStatic,
                            );
                        }
                    } else {
                        self.push_raw_argument(
                            &mut output,
                            call_site_id,
                            None,
                            &keyword.value,
                            PythonArgumentSpreadKind::KeywordDynamic,
                        );
                    }
                }
                ArgOrKeyword::Keyword(keyword) => self.push_raw_argument(
                    &mut output,
                    call_site_id,
                    keyword.arg.as_ref().map(ToString::to_string),
                    &keyword.value,
                    PythonArgumentSpreadKind::None,
                ),
            }
        }
        output
    }

    fn push_raw_argument(
        &mut self,
        output: &mut Vec<RawArgument>,
        call_site_id: PythonSemanticId,
        keyword_name: Option<String>,
        expression: &Expr,
        spread_kind: PythonArgumentSpreadKind,
    ) {
        let ordinal = i32::try_from(output.len()).unwrap_or(i32::MAX);
        let start = u64::from(u32::from(expression.start()));
        let end = u64::from(u32::from(expression.end()));
        let argument_id = semantic_id(
            self.fingerprint,
            "call-argument",
            start,
            end,
            keyword_name.as_deref().unwrap_or(""),
            u8::try_from(ordinal).unwrap_or(u8::MAX),
        );
        let syntax_id = self.add_syntax(
            argument_id,
            PythonCallableSyntaxRole::Argument,
            Some(ordinal),
            expression.range(),
        );
        output.push(RawArgument {
            argument_id,
            keyword_name,
            syntax_id,
            spread_kind,
            start_byte: start,
            end_byte: end,
        });
        let _ = call_site_id;
    }

    fn add_class_variable(&mut self, name: &str, range: TextRange) {
        let Some(class) = self.class_stack.last() else {
            return;
        };
        // A nested function keeps the class context but assignments in its body
        // are not class variables.
        if self.callable_stack.len() != class.callable_depth {
            return;
        }
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        self.members.push(PythonMemberFact {
            member_id: semantic_id(self.fingerprint, "member", start, end, name, 3),
            class_id: class.class_id,
            declared_entity_id: self
                .bindings
                .iter()
                .find(|binding| binding.name == name && binding.start_byte == start)
                .map(|binding| binding.binding_id),
            name: name.to_owned(),
            kind: PythonMemberKind::ClassVariable,
            start_byte: start,
            end_byte: end,
        });
    }

    fn add_instance_members(&mut self, target: &Expr) {
        let Some(class) = self.class_stack.last() else {
            return;
        };
        if self.callable_stack.len() <= class.callable_depth {
            return;
        }
        let mut attributes = Vec::new();
        collect_self_attributes(target, &mut attributes);
        for attribute in attributes {
            let start = u64::from(u32::from(attribute.start()));
            let end = u64::from(u32::from(attribute.end()));
            self.members.push(PythonMemberFact {
                member_id: semantic_id(
                    self.fingerprint,
                    "member",
                    start,
                    end,
                    attribute.attr.as_str(),
                    5,
                ),
                class_id: class.class_id,
                declared_entity_id: None,
                name: attribute.attr.to_string(),
                kind: PythonMemberKind::InstanceVariable,
                start_byte: start,
                end_byte: end,
            });
        }
    }

    fn finish(mut self) -> PythonCallableProjection {
        let binding_to_callable = self
            .callables
            .iter()
            .filter_map(|callable| {
                callable
                    .declared_binding_id
                    .map(|binding| (binding, callable.callable_id))
            })
            .collect::<BTreeMap<_, _>>();
        let callable_by_id = self
            .callables
            .iter()
            .map(|callable| (callable.callable_id, callable.clone()))
            .collect::<BTreeMap<_, _>>();
        let params_by_callable = self.parameters.iter().fold(
            BTreeMap::<PythonSemanticId, Vec<PythonParameterFact>>::new(),
            |mut map, parameter| {
                map.entry(parameter.callable_id)
                    .or_default()
                    .push(parameter.clone());
                map
            },
        );
        let mut call_sites = Vec::new();
        let mut arguments = Vec::new();
        let mut unknown_argument_sets = Vec::new();
        let mut diagnostics = Vec::new();
        for mut draft in self.calls {
            let target = resolve_local_target(
                &draft,
                self.references,
                &binding_to_callable,
                &self.callables,
            );
            draft.fact.declared_target_id = target;
            let parameters = target.and_then(|id| params_by_callable.get(&id));
            let (mut bound, mut unknowns, mut call_diagnostics) = bind_arguments(
                self.fingerprint,
                &draft,
                parameters,
                target.and_then(|id| callable_by_id.get(&id)),
            );
            if !unknowns.is_empty() {
                draft.fact.flags |= CALL_FLAG_DYNAMIC_SPLAT;
            }
            if !call_diagnostics.is_empty() {
                draft.fact.flags |= CALL_FLAG_BINDING_DIAGNOSTIC;
            }
            arguments.append(&mut bound);
            unknown_argument_sets.append(&mut unknowns);
            diagnostics.append(&mut call_diagnostics);
            call_sites.push(draft.fact);
        }
        self.members.sort_by_key(|member| {
            (
                member.start_byte,
                member.end_byte,
                member.name.clone(),
                member.kind,
            )
        });
        self.members.dedup_by_key(|member| member.member_id);
        self.callables.sort_by_key(|callable| callable.callable_id);
        self.parameters
            .sort_by_key(|parameter| (parameter.callable_id, parameter.ordinal));
        self.syntax.sort_by_key(|fact| fact.syntax_id);
        self.syntax.dedup_by_key(|fact| fact.syntax_id);
        call_sites.sort_by_key(|fact| fact.call_site_id);
        arguments.sort_by_key(|fact| (fact.call_site_id, fact.ordinal, fact.argument_id));
        PythonCallableProjection {
            callables: self.callables,
            parameters: self.parameters,
            syntax: self.syntax,
            call_sites,
            arguments,
            unknown_argument_sets,
            members: self.members,
            diagnostics,
        }
    }
}

impl<'a> Visitor<'a> for CallablePass<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                visit_parameter_expressions(self, &node.parameters);
                if let Some(returns) = node.returns.as_deref() {
                    self.visit_expr(returns);
                }
                if let Some(params) = node.type_params.as_deref() {
                    visitor::walk_type_params(self, params);
                }
                let callable_id = self.add_function(node);
                self.callable_stack.push(callable_id);
                visitor::walk_body(self, &node.body);
                self.callable_stack.pop();
            }
            Stmt::ClassDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                if let Some(arguments) = node.arguments.as_deref() {
                    visitor::walk_arguments(self, arguments);
                }
                let start = u64::from(u32::from(node.start()));
                let end = u64::from(u32::from(node.end()));
                let class_id = self
                    .binding_at(
                        node.name.as_str(),
                        node.name.range(),
                        PythonTargetForm::ClassName,
                    )
                    .unwrap_or_else(|| {
                        semantic_id(
                            self.fingerprint,
                            "class-declaration",
                            start,
                            end,
                            node.name.as_str(),
                            0,
                        )
                    });
                if let Some(parent) = self.class_stack.last() {
                    self.members.push(PythonMemberFact {
                        member_id: semantic_id(
                            self.fingerprint,
                            "member",
                            start,
                            end,
                            node.name.as_str(),
                            4,
                        ),
                        class_id: parent.class_id,
                        declared_entity_id: Some(class_id),
                        name: node.name.to_string(),
                        kind: PythonMemberKind::NestedType,
                        start_byte: start,
                        end_byte: end,
                    });
                }
                let qualified_name = self.class_stack.last().map_or_else(
                    || format!("{}.{}", self.module_name, node.name),
                    |parent| format!("{}.{}", parent.qualified_name, node.name),
                );
                self.class_stack.push(ClassContext {
                    class_id,
                    qualified_name,
                    callable_depth: self.callable_stack.len(),
                });
                visitor::walk_body(self, &node.body);
                self.class_stack.pop();
            }
            Stmt::Assign(node) => {
                for target in &node.targets {
                    if let Expr::Name(name) = target {
                        self.add_class_variable(name.id.as_str(), name.range);
                    }
                    self.add_instance_members(target);
                }
                visitor::walk_stmt(self, stmt);
            }
            Stmt::AnnAssign(node) => {
                if let Expr::Name(name) = node.target.as_ref() {
                    self.add_class_variable(name.id.as_str(), name.range);
                }
                self.add_instance_members(&node.target);
                visitor::walk_stmt(self, stmt);
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Lambda(node) => {
                if let Some(parameters) = node.parameters.as_deref() {
                    visit_parameter_expressions(self, parameters);
                }
                let callable_id = self.add_lambda(node);
                self.callable_stack.push(callable_id);
                self.visit_expr(&node.body);
                self.callable_stack.pop();
            }
            Expr::Call(node) => {
                self.add_call(node);
                visitor::walk_expr(self, expr);
            }
            _ => visitor::walk_expr(self, expr),
        }
    }
}

fn visit_parameter_expressions<'a>(visitor: &mut CallablePass<'a>, parameters: &'a Parameters) {
    for parameter in parameters {
        if let Some(annotation) = parameter.annotation() {
            visitor.visit_expr(annotation);
        }
        if let Some(default) = parameter.default() {
            visitor.visit_expr(default);
        }
    }
}

fn resolve_local_target(
    draft: &CallDraft,
    references: &[PythonReferenceFact],
    binding_to_callable: &BTreeMap<PythonSemanticId, PythonSemanticId>,
    callables: &[PythonCallableFact],
) -> Option<PythonSemanticId> {
    if draft.fact.dispatch_kind == PythonDispatchKind::DirectName {
        let start = u64::from(u32::from(draft.callee_range.start()));
        let end = u64::from(u32::from(draft.callee_range.end()));
        return references
            .iter()
            .find(|reference| {
                reference.start_byte == start
                    && reference.end_byte == end
                    && draft.callee_name.as_deref() == Some(reference.name.as_str())
            })
            .and_then(|reference| binding_to_callable.get(&reference.target_id))
            .copied();
    }
    if draft.receiver_is_self_or_cls {
        let candidates = callables
            .iter()
            .filter(|callable| {
                callable.class_id == draft.current_class_id
                    && draft.callee_name.as_deref() == Some(callable.name.as_str())
            })
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            return Some(candidates[0].callable_id);
        }
    }
    None
}

#[allow(clippy::too_many_lines)] // The binder keeps every Python argument state in one deterministic state machine.
fn bind_arguments(
    fingerprint: &str,
    draft: &CallDraft,
    parameters: Option<&Vec<PythonParameterFact>>,
    target: Option<&PythonCallableFact>,
) -> (
    Vec<PythonCallArgumentFact>,
    Vec<PythonUnknownArgumentSetFact>,
    Vec<PythonCallDiagnosticFact>,
) {
    let mut output = Vec::new();
    let mut unknowns = Vec::new();
    let mut diagnostics = Vec::new();
    let mut bound_parameters = BTreeSet::new();
    let mut positional_cursor = 0_usize;
    let params = parameters.map_or(&[][..], Vec::as_slice);
    let has_dynamic_splat = draft.raw_arguments.iter().any(|argument| {
        matches!(
            argument.spread_kind,
            PythonArgumentSpreadKind::PositionalDynamic | PythonArgumentSpreadKind::KeywordDynamic
        )
    });

    if target.is_some_and(|target| {
        target.class_id.is_some() && target.flags & CALLABLE_FLAG_STATIC_METHOD == 0
    }) && draft.fact.dispatch_kind == PythonDispatchKind::Attribute
        && let Some(receiver) = params.first()
    {
        bound_parameters.insert(receiver.parameter_id);
        output.push(synthetic_argument(
            fingerprint,
            draft.fact.call_site_id,
            i32::try_from(output.len()).unwrap_or(i32::MAX),
            receiver.parameter_id,
            PythonArgumentBindingStatus::BoundReceiver,
            PythonArgumentSpreadKind::BoundReceiver,
        ));
        positional_cursor = 1;
    }

    for raw in &draft.raw_arguments {
        let ordinal = i32::try_from(output.len()).unwrap_or(i32::MAX);
        if matches!(
            raw.spread_kind,
            PythonArgumentSpreadKind::PositionalDynamic | PythonArgumentSpreadKind::KeywordDynamic
        ) {
            let sentinel = semantic_id(
                fingerprint,
                "unknown-argument-set",
                raw.start_byte,
                raw.end_byte,
                "UNKNOWN_ARGUMENT_SET",
                u8::try_from(ordinal).unwrap_or(u8::MAX),
            );
            unknowns.push(PythonUnknownArgumentSetFact {
                unknown_argument_set_id: sentinel,
                call_site_id: draft.fact.call_site_id,
                spread_kind: raw.spread_kind,
            });
            output.push(argument_fact(
                raw,
                draft.fact.call_site_id,
                ordinal,
                Some(sentinel),
                PythonArgumentBindingStatus::UnknownArgumentSet,
            ));
            continue;
        }

        let (parameter_id, status) = if params.is_empty() {
            (None, PythonArgumentBindingStatus::UnresolvedTarget)
        } else if let Some(keyword) = raw.keyword_name.as_deref() {
            bind_keyword(keyword, params, &bound_parameters)
        } else {
            bind_positional(params, &mut positional_cursor, &bound_parameters)
        };
        if let Some(parameter_id) = parameter_id
            && matches!(
                status,
                PythonArgumentBindingStatus::Bound | PythonArgumentBindingStatus::BoundReceiver
            )
        {
            bound_parameters.insert(parameter_id);
        }
        if matches!(
            status,
            PythonArgumentBindingStatus::Duplicate
                | PythonArgumentBindingStatus::PositionalOnlyKeyword
                | PythonArgumentBindingStatus::TooManyPositional
                | PythonArgumentBindingStatus::UnmatchedKeyword
        ) {
            diagnostics.push(PythonCallDiagnosticFact {
                diagnostic_id: semantic_id(
                    fingerprint,
                    "call-diagnostic",
                    raw.start_byte,
                    raw.end_byte,
                    binding_status_name(status),
                    0,
                ),
                call_site_id: draft.fact.call_site_id,
                argument_id: raw.argument_id,
                code: binding_status_name(status),
                message: format!(
                    "{} at call argument {}",
                    binding_status_name(status),
                    ordinal
                ),
            });
        }
        output.push(argument_fact(
            raw,
            draft.fact.call_site_id,
            ordinal,
            parameter_id,
            status,
        ));
    }

    if !params.is_empty() && !has_dynamic_splat {
        for parameter in params {
            if bound_parameters.contains(&parameter.parameter_id)
                || matches!(
                    parameter.kind,
                    PythonParameterKind::VarPositional | PythonParameterKind::VarKeyword
                )
            {
                continue;
            }
            let status = if parameter.default_syntax_id.is_some() {
                PythonArgumentBindingStatus::Defaulted
            } else {
                PythonArgumentBindingStatus::MissingRequired
            };
            output.push(synthetic_argument(
                fingerprint,
                draft.fact.call_site_id,
                i32::try_from(output.len()).unwrap_or(i32::MAX),
                parameter.parameter_id,
                status,
                if status == PythonArgumentBindingStatus::Defaulted {
                    PythonArgumentSpreadKind::Default
                } else {
                    PythonArgumentSpreadKind::Missing
                },
            ));
        }
    }
    (output, unknowns, diagnostics)
}

fn bind_keyword(
    keyword: &str,
    parameters: &[PythonParameterFact],
    bound: &BTreeSet<PythonSemanticId>,
) -> (Option<PythonSemanticId>, PythonArgumentBindingStatus) {
    if let Some(parameter) = parameters
        .iter()
        .find(|parameter| parameter.name == keyword)
    {
        if parameter.kind == PythonParameterKind::PositionalOnly {
            if let Some(var_keyword) = parameters
                .iter()
                .find(|candidate| candidate.kind == PythonParameterKind::VarKeyword)
            {
                return (
                    Some(var_keyword.parameter_id),
                    PythonArgumentBindingStatus::Bound,
                );
            }
            return (
                Some(parameter.parameter_id),
                PythonArgumentBindingStatus::PositionalOnlyKeyword,
            );
        }
        if bound.contains(&parameter.parameter_id) {
            return (
                Some(parameter.parameter_id),
                PythonArgumentBindingStatus::Duplicate,
            );
        }
        return (
            Some(parameter.parameter_id),
            PythonArgumentBindingStatus::Bound,
        );
    }
    if let Some(parameter) = parameters
        .iter()
        .find(|parameter| parameter.kind == PythonParameterKind::VarKeyword)
    {
        return (
            Some(parameter.parameter_id),
            PythonArgumentBindingStatus::Bound,
        );
    }
    (None, PythonArgumentBindingStatus::UnmatchedKeyword)
}

fn bind_positional(
    parameters: &[PythonParameterFact],
    cursor: &mut usize,
    bound: &BTreeSet<PythonSemanticId>,
) -> (Option<PythonSemanticId>, PythonArgumentBindingStatus) {
    if let Some(parameter) = parameters.get(*cursor) {
        match parameter.kind {
            PythonParameterKind::PositionalOnly | PythonParameterKind::PositionalOrKeyword => {
                *cursor += 1;
                if bound.contains(&parameter.parameter_id) {
                    return (
                        Some(parameter.parameter_id),
                        PythonArgumentBindingStatus::Duplicate,
                    );
                }
                return (
                    Some(parameter.parameter_id),
                    PythonArgumentBindingStatus::Bound,
                );
            }
            PythonParameterKind::VarPositional => {
                return (
                    Some(parameter.parameter_id),
                    PythonArgumentBindingStatus::Bound,
                );
            }
            PythonParameterKind::KeywordOnly | PythonParameterKind::VarKeyword => {}
        }
    }
    (None, PythonArgumentBindingStatus::TooManyPositional)
}

fn argument_fact(
    raw: &RawArgument,
    call_site_id: PythonSemanticId,
    ordinal: i32,
    parameter_id: Option<PythonSemanticId>,
    binding_status: PythonArgumentBindingStatus,
) -> PythonCallArgumentFact {
    PythonCallArgumentFact {
        argument_id: raw.argument_id,
        call_site_id,
        ordinal,
        keyword_name: raw.keyword_name.clone(),
        argument_syntax_id: Some(raw.syntax_id),
        parameter_id,
        binding_status,
        spread_kind: raw.spread_kind,
        start_byte: Some(raw.start_byte),
        end_byte: Some(raw.end_byte),
    }
}

fn synthetic_argument(
    fingerprint: &str,
    call_site_id: PythonSemanticId,
    ordinal: i32,
    parameter_id: PythonSemanticId,
    binding_status: PythonArgumentBindingStatus,
    spread_kind: PythonArgumentSpreadKind,
) -> PythonCallArgumentFact {
    let call_site_prefix = u64::from_be_bytes(
        call_site_id[..8]
            .try_into()
            .expect("a semantic identifier has eight prefix bytes"),
    );
    let call_site_suffix = u64::from_be_bytes(
        call_site_id[8..]
            .try_into()
            .expect("a semantic identifier has eight suffix bytes"),
    );
    let identity_name = format!(
        "{}:{parameter_id:02x?}",
        binding_status_name(binding_status)
    );
    PythonCallArgumentFact {
        argument_id: semantic_id(
            fingerprint,
            "implicit-call-argument",
            call_site_prefix,
            call_site_suffix,
            &identity_name,
            u8::try_from(ordinal).unwrap_or(u8::MAX),
        ),
        call_site_id,
        ordinal,
        keyword_name: None,
        argument_syntax_id: None,
        parameter_id: Some(parameter_id),
        binding_status,
        spread_kind,
        start_byte: None,
        end_byte: None,
    }
}

fn static_positional_splat(expression: &Expr) -> Option<Vec<&Expr>> {
    let elements = match expression {
        Expr::Tuple(node) => &node.elts,
        Expr::List(node) => &node.elts,
        _ => return None,
    };
    let mut output = Vec::new();
    for element in elements {
        if let Expr::Starred(starred) = element {
            output.extend(static_positional_splat(&starred.value)?);
        } else {
            output.push(element);
        }
    }
    Some(output)
}

fn static_keyword_splat(expression: &Expr) -> Option<Vec<(String, &Expr)>> {
    let Expr::Dict(dict) = expression else {
        return None;
    };
    let mut output = Vec::new();
    let mut positions = BTreeMap::new();
    for item in &dict.items {
        let key = item.key.as_ref()?.as_string_literal_expr()?;
        let key = key.value.to_str().to_owned();
        if let Some(position) = positions.get(&key).copied() {
            output[position] = (key, &item.value);
        } else {
            positions.insert(key.clone(), output.len());
            output.push((key, &item.value));
        }
    }
    Some(output)
}

fn collect_self_attributes<'a>(
    expression: &'a Expr,
    output: &mut Vec<&'a ruff_python_ast::ExprAttribute>,
) {
    match expression {
        Expr::Attribute(attribute) if matches!(attribute.value.as_ref(), Expr::Name(name) if name.id.as_str() == "self") =>
        {
            output.push(attribute);
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                collect_self_attributes(element, output);
            }
        }
        Expr::List(list) => {
            for element in &list.elts {
                collect_self_attributes(element, output);
            }
        }
        _ => {}
    }
}

fn expression_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => {
            expression_name(&attribute.value).map(|prefix| format!("{prefix}.{}", attribute.attr))
        }
        Expr::Call(call) => expression_name(&call.func),
        _ => None,
    }
}

fn source_text(source: &str, range: TextRange) -> String {
    let start = usize::try_from(u32::from(range.start())).unwrap_or(source.len());
    let end = usize::try_from(u32::from(range.end())).unwrap_or(source.len());
    source.get(start..end).unwrap_or("").to_owned()
}

fn contains_yield(body: &[Stmt]) -> bool {
    struct YieldDetector(bool);
    impl<'a> Visitor<'a> for YieldDetector {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            if !matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
                visitor::walk_stmt(self, stmt);
            }
        }

        fn visit_expr(&mut self, expression: &'a Expr) {
            match expression {
                Expr::Yield(_) | Expr::YieldFrom(_) => self.0 = true,
                Expr::Lambda(_) => {}
                _ => visitor::walk_expr(self, expression),
            }
        }
    }
    let mut detector = YieldDetector(false);
    visitor::walk_body(&mut detector, body);
    detector.0
}

pub(crate) const fn binding_status_name(status: PythonArgumentBindingStatus) -> &'static str {
    match status {
        PythonArgumentBindingStatus::Bound => "BOUND",
        PythonArgumentBindingStatus::BoundReceiver => "BOUND_RECEIVER",
        PythonArgumentBindingStatus::Defaulted => "DEFAULTED",
        PythonArgumentBindingStatus::MissingRequired => "MISSING_REQUIRED",
        PythonArgumentBindingStatus::Duplicate => "DUPLICATE_ARGUMENT",
        PythonArgumentBindingStatus::PositionalOnlyKeyword => "POSITIONAL_ONLY_KEYWORD",
        PythonArgumentBindingStatus::TooManyPositional => "TOO_MANY_POSITIONAL",
        PythonArgumentBindingStatus::UnmatchedKeyword => "UNMATCHED_KEYWORD",
        PythonArgumentBindingStatus::UnresolvedTarget => "UNRESOLVED_TARGET",
        PythonArgumentBindingStatus::UnknownArgumentSet => "UNKNOWN_ARGUMENT_SET",
    }
}

pub(crate) const fn callable_syntax_role_name(role: PythonCallableSyntaxRole) -> &'static str {
    match role {
        PythonCallableSyntaxRole::CallExpression => "CALL_EXPRESSION",
        PythonCallableSyntaxRole::CalleeExpression => "CALLEE_EXPRESSION",
        PythonCallableSyntaxRole::Receiver => "RECEIVER",
        PythonCallableSyntaxRole::Argument => "ARGUMENT",
        PythonCallableSyntaxRole::Decorator => "DECORATOR",
        PythonCallableSyntaxRole::ReturnAnnotation => "RETURN_ANNOTATION",
        PythonCallableSyntaxRole::ParameterAnnotation => "PARAMETER_ANNOTATION",
        PythonCallableSyntaxRole::ParameterDefault => "PARAMETER_DEFAULT",
        PythonCallableSyntaxRole::TypeParameter => "TYPE_PARAMETER",
    }
}
