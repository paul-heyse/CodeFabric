//! Pinned Ruff semantic-model traversal.
//!
//! This is deliberately a child of `ruff_adapter`: provider-owned nodes and
//! arena IDs never cross the adapter boundary. The traversal follows Ruff's
//! checker order: collect bindings, resolve references, then clean up into
//! application-owned records.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Duration, Instant};

use ruff_python_ast::helpers::{collect_import_from_member, from_relative_import};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{
    self as ast, Expr, ExprContext, Pattern, PySourceType, PythonVersion, Stmt, TypeParam,
};
use ruff_python_semantic::{
    BindingFlags, BindingId, BindingKind, FromImport, GeneratorKind, Import, Module, ModuleKind,
    ModuleSource, ReadResult, ScopeId, ScopeKind, SemanticModel, StarImport, SubmoduleImport,
};
use ruff_text_size::{Ranged, TextRange};
use thiserror::Error;

use super::callables::{
    PythonCallArgumentFact, PythonCallDiagnosticFact, PythonCallSiteFact, PythonCallableFact,
    PythonCallableSyntaxFact, PythonMemberFact, PythonParameterFact, PythonUnknownArgumentSetFact,
};

/// Application-owned stable identifier for one semantic observation.
pub type PythonSemanticId = [u8; 16];

/// The seven governed Python scope kinds from ONT section 33.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PythonScopeKind {
    Module,
    Function,
    Class,
    Lambda,
    Comprehension,
    Annotation,
    TypeParameter,
}

/// Governed Python binding categories. These deliberately describe ontology
/// semantics rather than Ruff's version-local enum layout.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonBindingKind {
    Local,
    Parameter,
    Global,
    Nonlocal,
    Import,
    ClassAttribute,
    InstanceAttribute,
    Comprehension,
    Loop,
    With,
    Exception,
    Match,
    Walrus,
    TypeParameter,
    TypeAlias,
    Free,
    Cell,
    Builtin,
    Function,
    Class,
}

/// The fifteen governed target-form families from GEN section 18.2.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonTargetForm {
    FunctionName,
    ClassName,
    Parameter,
    Assignment,
    AnnotatedAssignment,
    AugmentedAssignment,
    NamedExpression,
    ImportAlias,
    LoopTarget,
    WithTarget,
    ExceptionTarget,
    MatchCapture,
    ComprehensionTarget,
    GlobalDeclaration,
    NonlocalDeclaration,
    TypeParameter,
    TypeAlias,
}

/// Governed reference classifications from GEN section 18.3.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonReferenceClass {
    Read,
    Write,
    ReadWrite,
    Delete,
    TypeReference,
    CallReference,
    ImportReference,
}

/// Resolution state is explicit; absence is not overloaded as unresolved.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonResolution {
    Resolved,
    MayReferTo,
    UnknownSymbol,
    UnboundLocal,
}

/// Governed import occurrence forms from FAB section 25.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonImportKind {
    Module,
    FromName,
    Star,
    Dynamic,
}

/// Whether the module export surface was fully established without execution.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonExportStatus {
    Complete,
    IncompleteDynamic,
}

/// Governed semantic relation kinds emitted by this pass.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonSemanticEdgeKind {
    RefersTo,
    MayReferTo,
    Shadows,
    Rebinds,
    GlobalResolution,
    NonlocalResolution,
    Captures,
    CapturedFrom,
}

/// One application-owned scope observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonScopeFact {
    pub scope_id: PythonSemanticId,
    pub parent_scope_id: Option<PythonSemanticId>,
    pub kind: PythonScopeKind,
    pub name: Option<String>,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One application-owned binding observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonBindingFact {
    pub binding_id: PythonSemanticId,
    pub scope_id: PythonSemanticId,
    pub name: String,
    pub kind: PythonBindingKind,
    pub target_form: PythonTargetForm,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One explicit unknown-symbol entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonUnknownSymbolFact {
    pub unknown_symbol_id: PythonSemanticId,
    pub scope_id: PythonSemanticId,
    pub name: String,
    pub reason_code: String,
}

/// One application-owned reference observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonReferenceFact {
    pub reference_id: PythonSemanticId,
    pub scope_id: PythonSemanticId,
    pub name: String,
    pub class: PythonReferenceClass,
    pub resolution: PythonResolution,
    pub target_id: PythonSemanticId,
    pub start_byte: u64,
    pub end_byte: u64,
    pub unknown_reason_code: Option<String>,
}

/// One semantic edge between application-owned IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythonSemanticEdge {
    pub subject_id: PythonSemanticId,
    pub object_id: PythonSemanticId,
    pub kind: PythonSemanticEdgeKind,
}

/// One source import occurrence with distinct syntax, binding, module, and symbol IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonImportFact {
    pub import_id: PythonSemanticId,
    pub scope_id: PythonSemanticId,
    pub kind: PythonImportKind,
    pub relative_level: Option<i16>,
    pub source_name: String,
    pub alias_name: Option<String>,
    pub star_import: bool,
    pub target_module_id: PythonSemanticId,
    pub target_module_name: Option<String>,
    /// Fully-qualified import target established by Ruff's local semantic model.
    /// Source spelling remains separately preserved in `source_name`.
    pub ruff_qualified_name: Option<String>,
    pub resolution: PythonResolution,
    pub imported_entity_id: Option<PythonSemanticId>,
    pub imported_name: Option<String>,
    pub local_binding_id: Option<PythonSemanticId>,
    pub unknown_reason_code: Option<String>,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// One statically visible export or re-export occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonExportFact {
    pub export_id: PythonSemanticId,
    pub name: String,
    pub target_id: PythonSemanticId,
    pub reexport: bool,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// Bounded operational measurements for one deterministic traversal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PythonSemanticMetrics {
    pub binding_pass_duration: Duration,
    pub traversal_pass_duration: Duration,
    pub cleanup_duration: Duration,
    pub visited_nodes: u64,
    pub scope_count: u64,
    pub binding_count: u64,
    pub reference_count: u64,
    pub unresolved_reference_count: u64,
    pub import_count: u64,
    pub export_count: u64,
    pub unresolved_module_count: u64,
    pub callable_count: u64,
    pub parameter_count: u64,
    pub call_site_count: u64,
    pub call_argument_count: u64,
    pub member_count: u64,
}

/// Registered terminal observation for the Ruff traversal pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonSemanticTerminal {
    pub pass_id: &'static str,
    pub provider_id: &'static str,
    pub terminal_state: &'static str,
    pub failure_code: Option<&'static str>,
}

/// Complete application-owned output of one Ruff semantic traversal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFrontendBatch {
    pub module_id: PythonSemanticId,
    pub module_name: String,
    pub provider_image_fingerprint: String,
    pub scopes: Vec<PythonScopeFact>,
    pub bindings: Vec<PythonBindingFact>,
    pub references: Vec<PythonReferenceFact>,
    pub unknown_symbols: Vec<PythonUnknownSymbolFact>,
    pub edges: Vec<PythonSemanticEdge>,
    pub imports: Vec<PythonImportFact>,
    pub exports: Vec<PythonExportFact>,
    pub export_status: PythonExportStatus,
    pub callables: Vec<PythonCallableFact>,
    pub parameters: Vec<PythonParameterFact>,
    pub callable_syntax: Vec<PythonCallableSyntaxFact>,
    pub call_sites: Vec<PythonCallSiteFact>,
    pub call_arguments: Vec<PythonCallArgumentFact>,
    pub unknown_argument_sets: Vec<PythonUnknownArgumentSetFact>,
    pub members: Vec<PythonMemberFact>,
    pub call_diagnostics: Vec<PythonCallDiagnosticFact>,
    pub metrics: PythonSemanticMetrics,
    pub terminal: PythonSemanticTerminal,
}

/// Closed adapter failures. Ruff-owned failure types never escape.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PythonSemanticError {
    #[error("Ruff semantic projection has no retained revision {0}")]
    MissingRevision(u64),
    #[error("RUFF_SEMANTIC_CLEANUP_FAILED: terminal_state=failed failure_code=cleanup-failure")]
    CleanupFailure,
    #[error("Ruff semantic projection invariant failed: {0}")]
    Invariant(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ScopeKey {
    start: u32,
    end: u32,
    kind: PythonScopeKind,
}

impl ScopeKey {
    fn new(range: TextRange, kind: PythonScopeKind) -> Self {
        Self {
            start: range.start().into(),
            end: range.end().into(),
            kind,
        }
    }
}

struct ScopeState {
    fact: PythonScopeFact,
    ruff_id: ScopeId,
    parent_index: Option<usize>,
    bindings: BTreeMap<String, usize>,
    uses_star_import: bool,
}

struct BindingPass<'a> {
    semantic: SemanticModel<'a>,
    fingerprint: &'a str,
    scopes: Vec<ScopeState>,
    scope_by_key: HashMap<ScopeKey, usize>,
    scope_stack: Vec<usize>,
    bindings: Vec<PythonBindingFact>,
    binding_by_ruff: HashMap<BindingId, usize>,
    edges: Vec<PythonSemanticEdge>,
    visited_nodes: u64,
}

impl<'a> BindingPass<'a> {
    fn new(
        suite: &'a [Stmt],
        module_name: &'a str,
        module_path: &'a Path,
        fingerprint: &'a str,
        source_len: u64,
    ) -> Self {
        let module = Module {
            kind: ModuleKind::Module,
            source: ModuleSource::File(module_path),
            python_ast: suite,
            name: Some(module_name),
        };
        let semantic = SemanticModel::new(
            &[],
            &[],
            PythonVersion::PY314,
            PySourceType::Python,
            module_path,
            module,
        );
        let fact = PythonScopeFact {
            scope_id: semantic_id(
                fingerprint,
                "scope",
                0,
                source_len,
                module_name,
                PythonScopeKind::Module as u8,
            ),
            parent_scope_id: None,
            kind: PythonScopeKind::Module,
            name: Some(module_name.to_owned()),
            start_byte: 0,
            end_byte: source_len,
        };
        let state = ScopeState {
            fact,
            ruff_id: ScopeId::global(),
            parent_index: None,
            bindings: BTreeMap::new(),
            uses_star_import: false,
        };
        Self {
            semantic,
            fingerprint,
            scopes: vec![state],
            scope_by_key: HashMap::new(),
            scope_stack: vec![0],
            bindings: Vec::new(),
            binding_by_ruff: HashMap::new(),
            edges: Vec::new(),
            visited_nodes: 0,
        }
    }

    fn current_scope_index(&self) -> usize {
        *self
            .scope_stack
            .last()
            .expect("module scope remains active")
    }

    fn register_current_scope(
        &mut self,
        range: TextRange,
        kind: PythonScopeKind,
        name: Option<String>,
    ) -> usize {
        let parent_index = self.current_scope_index();
        let ruff_id = self.semantic.scope_id;
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let scope_id = semantic_id(
            self.fingerprint,
            "scope",
            start,
            end,
            name.as_deref().unwrap_or(""),
            kind as u8,
        );
        let index = self.scopes.len();
        self.scopes.push(ScopeState {
            fact: PythonScopeFact {
                scope_id,
                parent_scope_id: Some(self.scopes[parent_index].fact.scope_id),
                kind,
                name,
                start_byte: start,
                end_byte: end,
            },
            ruff_id,
            parent_index: Some(parent_index),
            bindings: BTreeMap::new(),
            uses_star_import: false,
        });
        self.scope_by_key.insert(ScopeKey::new(range, kind), index);
        index
    }

    fn enter_existing(&mut self, index: usize) {
        self.semantic.scope_id = self.scopes[index].ruff_id;
        self.scope_stack.push(index);
    }

    fn leave_scope(&mut self) {
        let index = self.scope_stack.pop().expect("active nested scope");
        let parent = self.scopes[index]
            .parent_index
            .expect("nested scope has parent");
        self.semantic.scope_id = self.scopes[parent].ruff_id;
    }

    fn create_scope(
        &mut self,
        range: TextRange,
        kind: PythonScopeKind,
        name: Option<String>,
        ruff_kind: ScopeKind<'a>,
    ) -> usize {
        self.semantic.push_scope(ruff_kind);
        let index = self.register_current_scope(range, kind, name);
        self.semantic.pop_scope();
        index
    }

    fn nearest_binding(&self, name: &str, include_current: bool) -> Option<usize> {
        let mut cursor = if include_current {
            Some(self.current_scope_index())
        } else {
            self.scopes[self.current_scope_index()].parent_index
        };
        while let Some(index) = cursor {
            if let Some(binding) = self.scopes[index].bindings.get(name) {
                return Some(*binding);
            }
            cursor = self.scopes[index].parent_index;
        }
        None
    }

    fn bind(
        &mut self,
        name: &'a str,
        range: TextRange,
        kind: PythonBindingKind,
        target_form: PythonTargetForm,
        ruff_kind: BindingKind<'a>,
    ) -> usize {
        let scope_index = self.current_scope_index();
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let binding_id = semantic_id(
            self.fingerprint,
            "binding",
            start,
            end,
            name,
            target_form as u8,
        );
        let ruff_id = self
            .semantic
            .push_binding(name, range, ruff_kind, BindingFlags::empty());
        let same_scope_previous = self.semantic.current_scope_mut().add(name, ruff_id);
        let index = self.bindings.len();
        self.bindings.push(PythonBindingFact {
            binding_id,
            scope_id: self.scopes[scope_index].fact.scope_id,
            name: name.to_owned(),
            kind,
            target_form,
            start_byte: start,
            end_byte: end,
        });
        self.binding_by_ruff.insert(ruff_id, index);
        if let Some(previous) =
            same_scope_previous.and_then(|id| self.binding_by_ruff.get(&id).copied())
        {
            self.edges.push(PythonSemanticEdge {
                subject_id: binding_id,
                object_id: self.bindings[previous].binding_id,
                kind: PythonSemanticEdgeKind::Rebinds,
            });
        } else if let Some(outer) = self.nearest_binding(name, false) {
            self.semantic.shadowed_bindings.insert(
                ruff_id,
                self.binding_by_ruff
                    .iter()
                    .find_map(|(id, candidate)| (*candidate == outer).then_some(*id))
                    .expect("outer binding has Ruff ID"),
            );
            self.edges.push(PythonSemanticEdge {
                subject_id: binding_id,
                object_id: self.bindings[outer].binding_id,
                kind: PythonSemanticEdgeKind::Shadows,
            });
        }
        self.scopes[scope_index]
            .bindings
            .insert(name.to_owned(), index);
        index
    }

    fn bind_identifier(
        &mut self,
        identifier: &'a ast::Identifier,
        kind: PythonBindingKind,
        form: PythonTargetForm,
    ) {
        let ruff_kind = simple_ruff_binding_kind(kind, form);
        self.bind(
            identifier.as_str(),
            identifier.range(),
            kind,
            form,
            ruff_kind,
        );
    }

    fn bind_target(&mut self, target: &'a Expr, kind: PythonBindingKind, form: PythonTargetForm) {
        match target {
            Expr::Name(name) if name.ctx.is_store() => {
                self.bind(
                    name.id.as_str(),
                    name.range,
                    kind,
                    form,
                    simple_ruff_binding_kind(kind, form),
                );
            }
            Expr::List(node) => {
                for element in &node.elts {
                    self.bind_target(element, kind, form);
                }
            }
            Expr::Tuple(node) => {
                for element in &node.elts {
                    self.bind_target(element, kind, form);
                }
            }
            Expr::Starred(node) => self.bind_target(&node.value, kind, form),
            _ => {}
        }
    }

    fn bind_type_params(&mut self, params: &'a ast::TypeParams) {
        for parameter in params {
            let identifier = match parameter {
                TypeParam::TypeVar(node) => &node.name,
                TypeParam::TypeVarTuple(node) => &node.name,
                TypeParam::ParamSpec(node) => &node.name,
            };
            self.bind_identifier(
                identifier,
                PythonBindingKind::TypeParameter,
                PythonTargetForm::TypeParameter,
            );
        }
    }

    fn collect_type_param_scope(&mut self, params: &'a ast::TypeParams) {
        let index = self.create_scope(
            params.range,
            PythonScopeKind::TypeParameter,
            None,
            ScopeKind::Type,
        );
        self.enter_existing(index);
        self.bind_type_params(params);
        visitor::walk_type_params(self, params);
        self.leave_scope();
    }

    fn collect_annotation_scope(&mut self, expression: &'a Expr) {
        let index = self.create_scope(
            expression.range(),
            PythonScopeKind::Annotation,
            None,
            ScopeKind::Type,
        );
        self.enter_existing(index);
        visitor::walk_expr(self, expression);
        self.leave_scope();
    }
}

impl<'a> Visitor<'a> for BindingPass<'a> {
    #[allow(clippy::too_many_lines)] // The governed target-form census is clearer as one exhaustive visitor.
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        self.visited_nodes = self.visited_nodes.saturating_add(1);
        match stmt {
            Stmt::FunctionDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                if let Some(params) = &node.type_params {
                    self.collect_type_param_scope(params);
                }
                for parameter in &node.parameters {
                    if let Some(annotation) = parameter.annotation() {
                        self.visit_annotation(annotation);
                    }
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
                if let Some(returns) = &node.returns {
                    self.visit_annotation(returns);
                }
                let child = self.create_scope(
                    node.range,
                    PythonScopeKind::Function,
                    Some(node.name.to_string()),
                    ScopeKind::Function(node),
                );
                let child_ruff = self.scopes[child].ruff_id;
                self.bind(
                    node.name.as_str(),
                    node.name.range(),
                    PythonBindingKind::Function,
                    PythonTargetForm::FunctionName,
                    BindingKind::FunctionDefinition(child_ruff),
                );
                self.enter_existing(child);
                for parameter in &node.parameters {
                    self.bind_identifier(
                        parameter.name(),
                        PythonBindingKind::Parameter,
                        PythonTargetForm::Parameter,
                    );
                }
                visitor::walk_body(self, &node.body);
                self.leave_scope();
            }
            Stmt::ClassDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                if let Some(arguments) = &node.arguments {
                    self.visit_arguments(arguments);
                }
                if let Some(params) = &node.type_params {
                    self.collect_type_param_scope(params);
                }
                let child = self.create_scope(
                    node.range,
                    PythonScopeKind::Class,
                    Some(node.name.to_string()),
                    ScopeKind::Class(node),
                );
                let child_ruff = self.scopes[child].ruff_id;
                self.bind(
                    node.name.as_str(),
                    node.name.range(),
                    PythonBindingKind::Class,
                    PythonTargetForm::ClassName,
                    BindingKind::ClassDefinition(child_ruff),
                );
                self.enter_existing(child);
                visitor::walk_body(self, &node.body);
                self.leave_scope();
            }
            Stmt::Assign(node) => {
                for target in &node.targets {
                    self.bind_target(
                        target,
                        PythonBindingKind::Local,
                        PythonTargetForm::Assignment,
                    );
                }
                self.visit_expr(&node.value);
            }
            Stmt::AnnAssign(node) => {
                self.bind_target(
                    &node.target,
                    PythonBindingKind::Local,
                    PythonTargetForm::AnnotatedAssignment,
                );
                self.visit_annotation(&node.annotation);
                if let Some(value) = &node.value {
                    self.visit_expr(value);
                }
            }
            Stmt::AugAssign(node) => {
                self.bind_target(
                    &node.target,
                    PythonBindingKind::Local,
                    PythonTargetForm::AugmentedAssignment,
                );
                self.visit_expr(&node.value);
            }
            Stmt::TypeAlias(node) => {
                self.bind_target(
                    &node.name,
                    PythonBindingKind::TypeAlias,
                    PythonTargetForm::TypeAlias,
                );
                if let Some(params) = &node.type_params {
                    self.collect_type_param_scope(params);
                }
                self.visit_annotation(&node.value);
            }
            Stmt::For(node) => {
                self.bind_target(
                    &node.target,
                    PythonBindingKind::Loop,
                    PythonTargetForm::LoopTarget,
                );
                self.visit_expr(&node.iter);
                visitor::walk_body(self, &node.body);
                visitor::walk_body(self, &node.orelse);
            }
            Stmt::With(node) => {
                for item in &node.items {
                    self.visit_expr(&item.context_expr);
                    if let Some(target) = &item.optional_vars {
                        self.bind_target(
                            target,
                            PythonBindingKind::With,
                            PythonTargetForm::WithTarget,
                        );
                    }
                }
                visitor::walk_body(self, &node.body);
            }
            Stmt::Import(node) => {
                for alias in &node.names {
                    let identifier = alias.asname.as_ref().unwrap_or(&alias.name);
                    let name = alias.asname.as_ref().map_or_else(
                        || {
                            alias
                                .name
                                .as_str()
                                .split('.')
                                .next()
                                .unwrap_or(alias.name.as_str())
                        },
                        ast::Identifier::as_str,
                    );
                    let qualified_name = alias.name.as_str().split('.').collect();
                    let ruff_kind = if alias.asname.is_none() && alias.name.as_str().contains('.') {
                        BindingKind::SubmoduleImport(SubmoduleImport {
                            qualified_name: Box::new(qualified_name),
                        })
                    } else {
                        BindingKind::Import(Import {
                            qualified_name: Box::new(qualified_name),
                        })
                    };
                    self.bind(
                        name,
                        identifier.range(),
                        PythonBindingKind::Import,
                        PythonTargetForm::ImportAlias,
                        ruff_kind,
                    );
                }
            }
            Stmt::ImportFrom(node) => {
                for alias in &node.names {
                    if alias.name.as_str() == "*" {
                        self.semantic
                            .current_scope_mut()
                            .add_star_import(StarImport {
                                level: node.level,
                                module: node.module.as_ref().map(ast::Identifier::as_str),
                            });
                        let scope_index = self.current_scope_index();
                        self.scopes[scope_index].uses_star_import = true;
                    } else {
                        let identifier = alias.asname.as_ref().unwrap_or(&alias.name);
                        let qualified_name = collect_import_from_member(
                            node.level,
                            node.module.as_ref().map(ast::Identifier::as_str),
                            alias.name.as_str(),
                        );
                        self.bind(
                            identifier.as_str(),
                            identifier.range(),
                            PythonBindingKind::Import,
                            PythonTargetForm::ImportAlias,
                            BindingKind::FromImport(FromImport {
                                qualified_name: Box::new(qualified_name),
                            }),
                        );
                    }
                }
            }
            Stmt::Global(node) => {
                for name in &node.names {
                    self.bind(
                        name.as_str(),
                        name.range(),
                        PythonBindingKind::Global,
                        PythonTargetForm::GlobalDeclaration,
                        BindingKind::Global(None),
                    );
                }
            }
            Stmt::Nonlocal(node) => {
                for name in &node.names {
                    if let Some(target) = self.nearest_binding(name.as_str(), false) {
                        let target_ruff = self
                            .binding_by_ruff
                            .iter()
                            .find_map(|(id, candidate)| (*candidate == target).then_some(*id));
                        if let Some(target_ruff) = target_ruff {
                            let target_scope = self.scopes
                                [self.bindings[target].scope_id_index(&self.scopes)]
                            .ruff_id;
                            self.bind(
                                name.as_str(),
                                name.range(),
                                PythonBindingKind::Nonlocal,
                                PythonTargetForm::NonlocalDeclaration,
                                BindingKind::Nonlocal(target_ruff, target_scope),
                            );
                        }
                    } else {
                        self.bind(
                            name.as_str(),
                            name.range(),
                            PythonBindingKind::Nonlocal,
                            PythonTargetForm::NonlocalDeclaration,
                            BindingKind::Global(None),
                        );
                    }
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        self.visited_nodes = self.visited_nodes.saturating_add(1);
        match expr {
            Expr::Lambda(node) => {
                let child = self.create_scope(
                    node.range,
                    PythonScopeKind::Lambda,
                    None,
                    ScopeKind::Lambda(node),
                );
                self.enter_existing(child);
                if let Some(parameters) = &node.parameters {
                    for parameter in parameters {
                        self.bind_identifier(
                            parameter.name(),
                            PythonBindingKind::Parameter,
                            PythonTargetForm::Parameter,
                        );
                    }
                }
                self.visit_expr(&node.body);
                self.leave_scope();
            }
            Expr::Named(node) => {
                self.bind_target(
                    &node.target,
                    PythonBindingKind::Walrus,
                    PythonTargetForm::NamedExpression,
                );
                self.visit_expr(&node.value);
            }
            Expr::ListComp(node) => self.collect_comprehension(
                node.range,
                GeneratorKind::ListComprehension,
                &node.generators,
                Some(&node.elt),
                None,
            ),
            Expr::SetComp(node) => self.collect_comprehension(
                node.range,
                GeneratorKind::SetComprehension,
                &node.generators,
                Some(&node.elt),
                None,
            ),
            Expr::DictComp(node) => self.collect_comprehension(
                node.range,
                GeneratorKind::DictComprehension,
                &node.generators,
                node.key.as_deref(),
                Some(&node.value),
            ),
            Expr::Generator(node) => self.collect_comprehension(
                node.range,
                GeneratorKind::Generator,
                &node.generators,
                Some(&node.elt),
                None,
            ),
            _ => visitor::walk_expr(self, expr),
        }
    }

    fn visit_annotation(&mut self, expr: &'a Expr) {
        self.collect_annotation_scope(expr);
    }

    fn visit_except_handler(&mut self, handler: &'a ast::ExceptHandler) {
        let ast::ExceptHandler::ExceptHandler(node) = handler;
        if let Some(name) = &node.name {
            self.bind_identifier(
                name,
                PythonBindingKind::Exception,
                PythonTargetForm::ExceptionTarget,
            );
        }
        visitor::walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        match pattern {
            Pattern::MatchAs(node) => {
                if let Some(name) = &node.name {
                    self.bind_identifier(
                        name,
                        PythonBindingKind::Match,
                        PythonTargetForm::MatchCapture,
                    );
                }
            }
            Pattern::MatchStar(node) => {
                if let Some(name) = &node.name {
                    self.bind_identifier(
                        name,
                        PythonBindingKind::Match,
                        PythonTargetForm::MatchCapture,
                    );
                }
            }
            _ => {}
        }
        visitor::walk_pattern(self, pattern);
    }
}

impl<'a> BindingPass<'a> {
    fn collect_comprehension(
        &mut self,
        range: TextRange,
        generator_kind: GeneratorKind,
        generators: &'a [ast::Comprehension],
        first_result: Option<&'a Expr>,
        second_result: Option<&'a Expr>,
    ) {
        if let Some(first) = generators.first() {
            self.visit_expr(&first.iter);
        }
        let child = self.create_scope(
            range,
            PythonScopeKind::Comprehension,
            None,
            ScopeKind::Generator {
                kind: generator_kind,
                is_async: generators.iter().any(|generator| generator.is_async),
            },
        );
        self.enter_existing(child);
        for (ordinal, generator) in generators.iter().enumerate() {
            if ordinal > 0 {
                self.visit_expr(&generator.iter);
            }
            self.bind_target(
                &generator.target,
                PythonBindingKind::Comprehension,
                PythonTargetForm::ComprehensionTarget,
            );
            for predicate in &generator.ifs {
                self.visit_expr(predicate);
            }
        }
        if let Some(result) = first_result {
            self.visit_expr(result);
        }
        if let Some(result) = second_result {
            self.visit_expr(result);
        }
        self.leave_scope();
    }
}

trait BindingScopeIndex {
    fn scope_id_index(&self, scopes: &[ScopeState]) -> usize;
}

impl BindingScopeIndex for PythonBindingFact {
    fn scope_id_index(&self, scopes: &[ScopeState]) -> usize {
        scopes
            .iter()
            .position(|scope| scope.fact.scope_id == self.scope_id)
            .expect("binding scope exists")
    }
}

fn ruff_import_qualified_names(
    pass: &BindingPass<'_>,
    module_name: &str,
) -> BTreeMap<PythonSemanticId, String> {
    let module_path = module_name
        .split('.')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    pass.binding_by_ruff
        .iter()
        .filter_map(|(ruff_id, binding_index)| {
            let qualified_name = match &pass.semantic.binding(*ruff_id).kind {
                BindingKind::Import(Import { qualified_name })
                | BindingKind::SubmoduleImport(SubmoduleImport { qualified_name }) => {
                    Some(qualified_name.to_string())
                }
                BindingKind::FromImport(FromImport { qualified_name }) => {
                    if qualified_name.segments().first().copied() == Some(".") {
                        from_relative_import(&module_path, qualified_name.segments(), &[])
                            .map(|name| name.to_string())
                    } else {
                        Some(qualified_name.to_string())
                    }
                }
                _ => None,
            }?;
            Some((pass.bindings[*binding_index].binding_id, qualified_name))
        })
        .collect()
}

fn simple_ruff_binding_kind<'a>(
    kind: PythonBindingKind,
    form: PythonTargetForm,
) -> BindingKind<'a> {
    match (kind, form) {
        (PythonBindingKind::Parameter, _) => BindingKind::Argument,
        (PythonBindingKind::Walrus, _) => BindingKind::NamedExprAssignment,
        (PythonBindingKind::TypeParameter, _) => BindingKind::TypeParam,
        (PythonBindingKind::Loop | PythonBindingKind::Comprehension, _) => BindingKind::LoopVar,
        (PythonBindingKind::With, _) => BindingKind::WithItemVar,
        (PythonBindingKind::Exception, _) => BindingKind::BoundException,
        (_, PythonTargetForm::AnnotatedAssignment) => BindingKind::Annotation,
        _ => BindingKind::Assignment,
    }
}

#[derive(Clone, Copy)]
enum ReferenceMode {
    Normal,
    Type,
    Call,
    ReadWrite,
}

struct ReferencePass<'a, 'model> {
    semantic: &'model mut SemanticModel<'a>,
    fingerprint: &'a str,
    scopes: &'model [ScopeState],
    scope_by_key: &'model HashMap<ScopeKey, usize>,
    scope_stack: Vec<usize>,
    bindings: &'model mut Vec<PythonBindingFact>,
    binding_by_ruff: &'model mut HashMap<BindingId, usize>,
    references: Vec<PythonReferenceFact>,
    qualified_names: BTreeMap<PythonSemanticId, String>,
    unknown_symbols: Vec<PythonUnknownSymbolFact>,
    edges: &'model mut Vec<PythonSemanticEdge>,
    mode: ReferenceMode,
    visited_nodes: u64,
}

impl<'a, 'model> ReferencePass<'a, 'model> {
    fn new(pass: &'model mut BindingPass<'a>) -> Self {
        pass.semantic.scope_id = ScopeId::global();
        Self {
            semantic: &mut pass.semantic,
            fingerprint: pass.fingerprint,
            scopes: &pass.scopes,
            scope_by_key: &pass.scope_by_key,
            scope_stack: vec![0],
            bindings: &mut pass.bindings,
            binding_by_ruff: &mut pass.binding_by_ruff,
            references: Vec::new(),
            qualified_names: BTreeMap::new(),
            unknown_symbols: Vec::new(),
            edges: &mut pass.edges,
            mode: ReferenceMode::Normal,
            visited_nodes: 0,
        }
    }

    fn scope_index(&self) -> usize {
        *self.scope_stack.last().expect("module active")
    }

    fn enter(&mut self, range: TextRange, kind: PythonScopeKind) {
        let index = self.scope_by_key[&ScopeKey::new(range, kind)];
        self.scope_stack.push(index);
        self.semantic.scope_id = self.scopes[index].ruff_id;
    }

    fn leave(&mut self) {
        self.scope_stack.pop();
        self.semantic.scope_id = self.scopes[self.scope_index()].ruff_id;
    }

    fn with_mode(&mut self, mode: ReferenceMode, visit: impl FnOnce(&mut Self)) {
        let previous = self.mode;
        self.mode = mode;
        visit(self);
        self.mode = previous;
    }

    fn app_binding(&mut self, ruff_id: BindingId, name: &str) -> usize {
        if let Some(index) = self.binding_by_ruff.get(&ruff_id).copied() {
            return index;
        }
        let module = &self.scopes[0].fact;
        let id = semantic_id(self.fingerprint, "builtin", 0, 0, name, 0);
        let index = self.bindings.len();
        self.bindings.push(PythonBindingFact {
            binding_id: id,
            scope_id: module.scope_id,
            name: name.to_owned(),
            kind: PythonBindingKind::Builtin,
            target_form: PythonTargetForm::Assignment,
            start_byte: 0,
            end_byte: 0,
        });
        self.binding_by_ruff.insert(ruff_id, index);
        index
    }

    fn emit_unknown(
        &mut self,
        name: &str,
        range: TextRange,
        class: PythonReferenceClass,
        resolution: PythonResolution,
        reason: &'static str,
    ) {
        let scope_id = self.scopes[self.scope_index()].fact.scope_id;
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let unknown_id = semantic_id(self.fingerprint, "unknown", start, end, name, 0);
        if !self
            .unknown_symbols
            .iter()
            .any(|unknown| unknown.unknown_symbol_id == unknown_id)
        {
            self.unknown_symbols.push(PythonUnknownSymbolFact {
                unknown_symbol_id: unknown_id,
                scope_id,
                name: name.to_owned(),
                reason_code: reason.to_owned(),
            });
        }
        let reference_id =
            semantic_id(self.fingerprint, "reference", start, end, name, class as u8);
        self.references.push(PythonReferenceFact {
            reference_id,
            scope_id,
            name: name.to_owned(),
            class,
            resolution,
            target_id: unknown_id,
            start_byte: start,
            end_byte: end,
            unknown_reason_code: Some(reason.to_owned()),
        });
        self.edges.push(PythonSemanticEdge {
            subject_id: reference_id,
            object_id: unknown_id,
            kind: if resolution == PythonResolution::MayReferTo {
                PythonSemanticEdgeKind::MayReferTo
            } else {
                PythonSemanticEdgeKind::RefersTo
            },
        });
    }

    fn emit_name(&mut self, name: &'a ast::ExprName) -> Option<usize> {
        let class = match self.mode {
            ReferenceMode::Type => PythonReferenceClass::TypeReference,
            ReferenceMode::Call => PythonReferenceClass::CallReference,
            ReferenceMode::ReadWrite => PythonReferenceClass::ReadWrite,
            ReferenceMode::Normal => match name.ctx {
                ExprContext::Load | ExprContext::Invalid => PythonReferenceClass::Read,
                ExprContext::Store => PythonReferenceClass::Write,
                ExprContext::Del => PythonReferenceClass::Delete,
            },
        };
        if name.ctx.is_store() && !matches!(self.mode, ReferenceMode::ReadWrite) {
            return None;
        }
        if name.ctx.is_del() {
            self.semantic.resolve_del(name.id.as_str(), name.range);
        }
        let result = if name.ctx.is_load() || matches!(self.mode, ReferenceMode::ReadWrite) {
            self.semantic.resolve_load(name)
        } else {
            ReadResult::NotFound
        };
        match result {
            ReadResult::Resolved(ruff_id) => {
                let binding_index = self.app_binding(ruff_id, name.id.as_str());
                self.emit_resolved(
                    name.id.as_str(),
                    name.range,
                    class,
                    binding_index,
                    PythonResolution::Resolved,
                    None,
                );
                Some(binding_index)
            }
            ReadResult::UnboundLocal(ruff_id) => {
                let binding_index = self.app_binding(ruff_id, name.id.as_str());
                self.emit_resolved(
                    name.id.as_str(),
                    name.range,
                    class,
                    binding_index,
                    PythonResolution::UnboundLocal,
                    Some("UNBOUND_LOCAL"),
                );
                Some(binding_index)
            }
            ReadResult::WildcardImport => {
                self.emit_unknown(
                    name.id.as_str(),
                    name.range,
                    class,
                    PythonResolution::MayReferTo,
                    "STAR_IMPORT_EXPORTS_UNKNOWN",
                );
                None
            }
            ReadResult::ImplicitGlobal => {
                self.emit_unknown(
                    name.id.as_str(),
                    name.range,
                    class,
                    PythonResolution::UnknownSymbol,
                    "IMPLICIT_GLOBAL",
                );
                None
            }
            ReadResult::NotFound => {
                self.emit_unknown(
                    name.id.as_str(),
                    name.range,
                    class,
                    PythonResolution::UnknownSymbol,
                    "UNRESOLVED_LEXICAL_NAME",
                );
                None
            }
        }
    }

    fn emit_resolved(
        &mut self,
        name: &str,
        range: TextRange,
        class: PythonReferenceClass,
        binding_index: usize,
        resolution: PythonResolution,
        unknown_reason: Option<&str>,
    ) {
        let scope_id = self.scopes[self.scope_index()].fact.scope_id;
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let reference_id =
            semantic_id(self.fingerprint, "reference", start, end, name, class as u8);
        let target_id = self.bindings[binding_index].binding_id;
        self.references.push(PythonReferenceFact {
            reference_id,
            scope_id,
            name: name.to_owned(),
            class,
            resolution,
            target_id,
            start_byte: start,
            end_byte: end,
            unknown_reason_code: unknown_reason.map(str::to_owned),
        });
        self.edges.push(PythonSemanticEdge {
            subject_id: reference_id,
            object_id: target_id,
            kind: PythonSemanticEdgeKind::RefersTo,
        });
        let target_scope = self.bindings[binding_index].scope_id;
        if target_scope != scope_id && target_scope != self.scopes[0].fact.scope_id {
            self.edges.push(PythonSemanticEdge {
                subject_id: scope_id,
                object_id: target_id,
                kind: PythonSemanticEdgeKind::Captures,
            });
            self.edges.push(PythonSemanticEdge {
                subject_id: target_id,
                object_id: scope_id,
                kind: PythonSemanticEdgeKind::CapturedFrom,
            });
        }
    }

    fn emit_import(&mut self, name: &str, range: TextRange) {
        let scope = self.scope_index();
        if let Some(binding_index) = self.scopes[scope].bindings.get(name).copied() {
            self.emit_resolved(
                name,
                range,
                PythonReferenceClass::ImportReference,
                binding_index,
                PythonResolution::Resolved,
                None,
            );
        }
    }
}

impl<'a> Visitor<'a> for ReferencePass<'a, '_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        self.visited_nodes = self.visited_nodes.saturating_add(1);
        match stmt {
            Stmt::FunctionDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                if let Some(params) = &node.type_params {
                    self.enter(params.range, PythonScopeKind::TypeParameter);
                    visitor::walk_type_params(self, params);
                    self.leave();
                }
                for parameter in &node.parameters {
                    if let Some(annotation) = parameter.annotation() {
                        self.visit_annotation(annotation);
                    }
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
                if let Some(returns) = &node.returns {
                    self.visit_annotation(returns);
                }
                self.enter(node.range, PythonScopeKind::Function);
                visitor::walk_body(self, &node.body);
                self.leave();
            }
            Stmt::ClassDef(node) => {
                for decorator in &node.decorator_list {
                    self.visit_expr(&decorator.expression);
                }
                if let Some(arguments) = &node.arguments {
                    self.visit_arguments(arguments);
                }
                if let Some(params) = &node.type_params {
                    self.enter(params.range, PythonScopeKind::TypeParameter);
                    visitor::walk_type_params(self, params);
                    self.leave();
                }
                self.enter(node.range, PythonScopeKind::Class);
                visitor::walk_body(self, &node.body);
                self.leave();
            }
            Stmt::AugAssign(node) => {
                self.with_mode(ReferenceMode::ReadWrite, |this| {
                    this.visit_expr(&node.target);
                });
                self.visit_expr(&node.value);
            }
            Stmt::Import(node) => {
                for alias in &node.names {
                    let name = alias.asname.as_ref().map_or_else(
                        || {
                            alias
                                .name
                                .as_str()
                                .split('.')
                                .next()
                                .unwrap_or(alias.name.as_str())
                        },
                        ast::Identifier::as_str,
                    );
                    self.emit_import(name, alias.range);
                }
            }
            Stmt::ImportFrom(node) => {
                for alias in &node.names {
                    if alias.name.as_str() != "*" {
                        let identifier = alias.asname.as_ref().unwrap_or(&alias.name);
                        self.emit_import(identifier.as_str(), alias.range);
                    }
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        self.visited_nodes = self.visited_nodes.saturating_add(1);
        match expr {
            Expr::Name(name) => {
                if let Some(binding_index) = self.emit_name(name)
                    && let Some(qualified_name) = self.semantic.resolve_qualified_name(expr)
                {
                    self.qualified_names.insert(
                        self.bindings[binding_index].binding_id,
                        qualified_name.to_string(),
                    );
                }
            }
            Expr::Lambda(node) => {
                self.enter(node.range, PythonScopeKind::Lambda);
                self.visit_expr(&node.body);
                self.leave();
            }
            Expr::Call(node) => {
                self.with_mode(ReferenceMode::Call, |this| this.visit_expr(&node.func));
                self.visit_arguments(&node.arguments);
            }
            Expr::ListComp(node) => {
                self.visit_comprehension_scope(node.range, &node.generators, Some(&node.elt), None);
            }
            Expr::SetComp(node) => {
                self.visit_comprehension_scope(node.range, &node.generators, Some(&node.elt), None);
            }
            Expr::DictComp(node) => self.visit_comprehension_scope(
                node.range,
                &node.generators,
                node.key.as_deref(),
                Some(&node.value),
            ),
            Expr::Generator(node) => {
                self.visit_comprehension_scope(node.range, &node.generators, Some(&node.elt), None);
            }
            _ => visitor::walk_expr(self, expr),
        }
    }

    fn visit_annotation(&mut self, expr: &'a Expr) {
        self.enter(expr.range(), PythonScopeKind::Annotation);
        self.with_mode(ReferenceMode::Type, |this| visitor::walk_expr(this, expr));
        self.leave();
    }
}

impl<'a> ReferencePass<'a, '_> {
    fn visit_comprehension_scope(
        &mut self,
        range: TextRange,
        generators: &'a [ast::Comprehension],
        first_result: Option<&'a Expr>,
        second_result: Option<&'a Expr>,
    ) {
        if let Some(first) = generators.first() {
            self.visit_expr(&first.iter);
        }
        self.enter(range, PythonScopeKind::Comprehension);
        for (ordinal, generator) in generators.iter().enumerate() {
            if ordinal > 0 {
                self.visit_expr(&generator.iter);
            }
            for predicate in &generator.ifs {
                self.visit_expr(predicate);
            }
        }
        if let Some(result) = first_result {
            self.visit_expr(result);
        }
        if let Some(result) = second_result {
            self.visit_expr(result);
        }
        self.leave();
    }
}

#[allow(clippy::too_many_lines)] // The binding/traversal/cleanup transaction remains auditable in one function.
pub(super) fn project_python_semantics<'a>(
    source: &'a str,
    suite: &'a [Stmt],
    module_name: &'a str,
    module_path: &'a Path,
    provider_image_fingerprint: &'a str,
    inject_cleanup_failure: bool,
) -> Result<PythonFrontendBatch, PythonSemanticError> {
    let binding_started = Instant::now();
    let source_len = suite
        .last()
        .map_or(0, |statement| u64::from(u32::from(statement.end())));
    let mut pass = BindingPass::new(
        suite,
        module_name,
        module_path,
        provider_image_fingerprint,
        source_len,
    );
    visitor::walk_body(&mut pass, suite);
    let binding_pass_duration = binding_started.elapsed();

    let traversal_started = Instant::now();
    let (references, unknown_symbols, reference_qualified_names, reference_visited) = {
        let mut references = ReferencePass::new(&mut pass);
        visitor::walk_body(&mut references, suite);
        (
            references.references,
            references.unknown_symbols,
            references.qualified_names,
            references.visited_nodes,
        )
    };
    let traversal_pass_duration = traversal_started.elapsed();

    let cleanup_started = Instant::now();
    if inject_cleanup_failure {
        return Err(PythonSemanticError::CleanupFailure);
    }

    // Every binding occurrence is also a WRITE reference. This includes
    // parameters, imports, captures, and declarative forms that do not appear as
    // ExprName nodes in Ruff's AST.
    let mut all_references = references;
    for binding in &pass.bindings {
        let reference_id = semantic_id(
            provider_image_fingerprint,
            "write-reference",
            binding.start_byte,
            binding.end_byte,
            &binding.name,
            binding.target_form as u8,
        );
        all_references.push(PythonReferenceFact {
            reference_id,
            scope_id: binding.scope_id,
            name: binding.name.clone(),
            class: if binding.target_form == PythonTargetForm::AugmentedAssignment {
                PythonReferenceClass::ReadWrite
            } else if binding.target_form == PythonTargetForm::ImportAlias {
                PythonReferenceClass::ImportReference
            } else {
                PythonReferenceClass::Write
            },
            resolution: PythonResolution::Resolved,
            target_id: binding.binding_id,
            start_byte: binding.start_byte,
            end_byte: binding.end_byte,
            unknown_reason_code: None,
        });
        pass.edges.push(PythonSemanticEdge {
            subject_id: reference_id,
            object_id: binding.binding_id,
            kind: PythonSemanticEdgeKind::RefersTo,
        });
    }
    add_declaration_resolution_edges(&pass.scopes, &pass.bindings, &mut pass.edges);
    let mut ruff_qualified_names = ruff_import_qualified_names(&pass, module_name);
    // A real reference resolution is the strongest local signal because it has
    // passed through `SemanticModel::resolve_qualified_name`; retain it over the
    // equivalent import-binding metadata when both are present.
    ruff_qualified_names.extend(reference_qualified_names);

    let mut scopes = pass
        .scopes
        .into_iter()
        .map(|scope| scope.fact)
        .collect::<Vec<_>>();
    scopes.sort_by_key(|scope| (scope.start_byte, scope.end_byte, scope.kind));
    pass.bindings.sort_by_key(|binding| {
        (
            binding.start_byte,
            binding.end_byte,
            binding.name.clone(),
            binding.target_form,
        )
    });
    all_references.sort_by_key(|reference| {
        (
            reference.start_byte,
            reference.end_byte,
            reference.name.clone(),
            reference.class,
        )
    });
    let mut unknown_symbols = unknown_symbols;
    unknown_symbols.sort_by_key(|unknown| (unknown.name.clone(), unknown.unknown_symbol_id));
    pass.edges
        .sort_by_key(|edge| (edge.kind, edge.subject_id, edge.object_id));
    pass.edges.dedup();
    let cleanup_duration = cleanup_started.elapsed();
    let unresolved_reference_count = u64::try_from(unknown_symbols.len()).unwrap_or(u64::MAX);
    let import_projection = super::imports::project_python_imports(
        suite,
        module_name,
        provider_image_fingerprint,
        &scopes,
        &pass.bindings,
        &ruff_qualified_names,
    );
    let callable_projection = super::callables::project_python_callables(
        source,
        suite,
        module_name,
        provider_image_fingerprint,
        import_projection.module_id,
        &scopes,
        &pass.bindings,
        &all_references,
    );
    let metrics = PythonSemanticMetrics {
        binding_pass_duration,
        traversal_pass_duration,
        cleanup_duration,
        visited_nodes: pass.visited_nodes.saturating_add(reference_visited),
        scope_count: u64::try_from(scopes.len()).unwrap_or(u64::MAX),
        binding_count: u64::try_from(pass.bindings.len()).unwrap_or(u64::MAX),
        reference_count: u64::try_from(all_references.len()).unwrap_or(u64::MAX),
        unresolved_reference_count,
        import_count: u64::try_from(import_projection.imports.len()).unwrap_or(u64::MAX),
        export_count: u64::try_from(import_projection.exports.len()).unwrap_or(u64::MAX),
        unresolved_module_count: u64::try_from(
            import_projection
                .imports
                .iter()
                .filter(|fact| fact.resolution != PythonResolution::Resolved)
                .count(),
        )
        .unwrap_or(u64::MAX),
        callable_count: u64::try_from(callable_projection.callables.len()).unwrap_or(u64::MAX),
        parameter_count: u64::try_from(callable_projection.parameters.len()).unwrap_or(u64::MAX),
        call_site_count: u64::try_from(callable_projection.call_sites.len()).unwrap_or(u64::MAX),
        call_argument_count: u64::try_from(callable_projection.arguments.len()).unwrap_or(u64::MAX),
        member_count: u64::try_from(callable_projection.members.len()).unwrap_or(u64::MAX),
    };
    Ok(PythonFrontendBatch {
        module_id: import_projection.module_id,
        module_name: module_name.to_owned(),
        provider_image_fingerprint: provider_image_fingerprint.to_owned(),
        scopes,
        bindings: pass.bindings,
        references: all_references,
        unknown_symbols,
        edges: pass.edges,
        imports: import_projection.imports,
        exports: import_projection.exports,
        export_status: import_projection.export_status,
        callables: callable_projection.callables,
        parameters: callable_projection.parameters,
        callable_syntax: callable_projection.syntax,
        call_sites: callable_projection.call_sites,
        call_arguments: callable_projection.arguments,
        unknown_argument_sets: callable_projection.unknown_argument_sets,
        members: callable_projection.members,
        call_diagnostics: callable_projection.diagnostics,
        metrics,
        terminal: PythonSemanticTerminal {
            pass_id: "PASS_RUFF_SEMANTIC_TRAVERSAL_V1",
            provider_id: "ruff-python",
            terminal_state: "completed",
            failure_code: None,
        },
    })
}

fn add_declaration_resolution_edges(
    scopes: &[ScopeState],
    bindings: &[PythonBindingFact],
    edges: &mut Vec<PythonSemanticEdge>,
) {
    for binding in bindings {
        let kind = match binding.kind {
            PythonBindingKind::Global => PythonSemanticEdgeKind::GlobalResolution,
            PythonBindingKind::Nonlocal => PythonSemanticEdgeKind::NonlocalResolution,
            _ => continue,
        };
        let Some(scope_index) = scopes
            .iter()
            .position(|scope| scope.fact.scope_id == binding.scope_id)
        else {
            continue;
        };
        let target = if binding.kind == PythonBindingKind::Global {
            scopes[0].bindings.get(&binding.name).copied()
        } else {
            let mut cursor = scopes[scope_index].parent_index;
            let mut found = None;
            while let Some(index) = cursor {
                if index != 0 {
                    found = scopes[index].bindings.get(&binding.name).copied();
                }
                if found.is_some() {
                    break;
                }
                cursor = scopes[index].parent_index;
            }
            found
        };
        if let Some(target) =
            target.filter(|target| bindings[*target].binding_id != binding.binding_id)
        {
            edges.push(PythonSemanticEdge {
                subject_id: binding.binding_id,
                object_id: bindings[target].binding_id,
                kind,
            });
        }
    }
}

pub(super) fn semantic_id(
    fingerprint: &str,
    domain: &str,
    start: u64,
    end: u64,
    name: &str,
    discriminator: u8,
) -> PythonSemanticId {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-semantic-id.v1");
    hasher.update(fingerprint.as_bytes());
    hasher.update(domain.as_bytes());
    hasher.update(&start.to_be_bytes());
    hasher.update(&end.to_be_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&[discriminator]);
    let mut id = [0_u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id
}

#[cfg(test)]
mod tests {
    use ruff_python_parser::{ParseOptions, parse_unchecked};

    use super::super::callables::{
        PythonArgumentBindingStatus, PythonArgumentSpreadKind, PythonCallableSyntaxRole,
        PythonMemberKind, PythonParameterKind,
    };
    use super::*;

    fn analyze(
        source: &str,
        cleanup_failure: bool,
    ) -> Result<PythonFrontendBatch, PythonSemanticError> {
        let parsed = parse_unchecked(
            source,
            ParseOptions::from(PySourceType::Python).with_target_version(PythonVersion::PY314),
        )
        .try_into_module()
        .expect("module parse");
        project_python_semantics(
            source,
            parsed.syntax().body.as_slice(),
            "fixture",
            Path::new("fixture.py"),
            "b3:fixture",
            cleanup_failure,
        )
    }

    fn inspect_non_adapter_rust(path: &Path, violations: &mut Vec<String>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some("ruff_adapter") {
                    inspect_non_adapter_rust(&path, violations);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && path.file_name().and_then(|name| name.to_str()) != Some("ruff_adapter.rs")
            {
                let source = std::fs::read_to_string(&path).unwrap();
                if source.contains("ruff_python_semantic") {
                    violations.push(path.display().to_string());
                }
            }
        }
    }

    #[test]
    fn py_scope_binding_fixture_conformance() {
        let source = r"
x = 1
class C[T]:
    field: T
    def method(self, arg: T):
        local = arg
        return lambda q: [local + q for q in range(2)]
def outer():
    captured = 1
    def inner():
        nonlocal captured
        captured += missing
        return captured
    return inner
";
        let first = analyze(source, false).unwrap();
        let second = analyze(source, false).unwrap();
        let mut normalized_first = first.clone();
        let mut normalized_second = second.clone();
        normalized_first.metrics = PythonSemanticMetrics::default();
        normalized_second.metrics = PythonSemanticMetrics::default();
        assert_eq!(normalized_first, normalized_second);
        for required in [
            PythonScopeKind::Module,
            PythonScopeKind::Function,
            PythonScopeKind::Class,
            PythonScopeKind::Lambda,
            PythonScopeKind::Comprehension,
            PythonScopeKind::Annotation,
            PythonScopeKind::TypeParameter,
        ] {
            assert!(
                first.scopes.iter().any(|scope| scope.kind == required),
                "{required:?}"
            );
        }
        assert!(
            first
                .bindings
                .iter()
                .any(|binding| binding.kind == PythonBindingKind::Nonlocal)
        );
        assert!(
            first
                .edges
                .iter()
                .any(|edge| edge.kind == PythonSemanticEdgeKind::Captures)
        );
        assert_eq!(first.terminal.terminal_state, "completed");

        #[cfg(feature = "daemon")]
        {
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let projection = project_ruff_semantic_batch(
                FactScope {
                    workspace_id: [1; 16],
                    analysis_context_id: [2; 16],
                    source_generation: 3,
                    owner_id: [4; 16],
                },
                [5; 16],
                &first,
            )
            .unwrap();
            assert_eq!(
                projection.batch(200).unwrap().num_rows(),
                first.scopes.len()
            );
            assert_eq!(
                projection.batch(210).unwrap().num_rows(),
                first.bindings.len()
            );
            assert_eq!(
                projection.batch(220).unwrap().num_rows(),
                first.references.len()
            );
            assert_eq!(
                projection.batch(100).unwrap().num_rows(),
                first.scopes.len()
                    + first.bindings.len()
                    + first.references.len()
                    + first.unknown_symbols.len()
                    + 1
                    + first.exports.len()
                    + first.callables.len()
                    + first.parameters.len()
                    + first.callable_syntax.len()
                    + first.call_sites.len()
                    + first.call_arguments.len()
                    + first.unknown_argument_sets.len()
                    + first.members.len()
            );
        }
    }

    #[test]
    fn py_unresolved_reference_unknown_falsification() {
        let batch = analyze(
            "from dynamic import *\nvalue = absent()\nexec(payload)\nobj = getattr(target, key)\n",
            false,
        )
        .unwrap();
        let absent = batch
            .references
            .iter()
            .find(|reference| reference.name == "absent")
            .unwrap();
        assert_eq!(absent.resolution, PythonResolution::MayReferTo);
        assert_eq!(
            absent.unknown_reason_code.as_deref(),
            Some("STAR_IMPORT_EXPORTS_UNKNOWN")
        );
        assert!(
            batch
                .unknown_symbols
                .iter()
                .any(|unknown| unknown.unknown_symbol_id == absent.target_id)
        );
        assert!(batch.edges.iter().any(
            |edge| edge.subject_id == absent.reference_id && edge.object_id == absent.target_id
        ));
        for name in ["payload", "target", "key"] {
            let reference = batch
                .references
                .iter()
                .find(|reference| reference.name == name)
                .unwrap();
            assert_ne!(reference.resolution, PythonResolution::Resolved);
            assert!(reference.unknown_reason_code.is_some());
            assert!(batch.unknown_symbols.iter().any(|unknown| {
                unknown.unknown_symbol_id == reference.target_id && unknown.name == name
            }));
        }

        #[cfg(feature = "daemon")]
        {
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let scope = FactScope {
                workspace_id: [6; 16],
                analysis_context_id: [7; 16],
                source_generation: 8,
                owner_id: [9; 16],
            };
            let projection = project_ruff_semantic_batch(scope, [10; 16], &batch).unwrap();
            assert!(projection.batch(100).unwrap().num_rows() > batch.references.len());
            let mut absent_edge = batch.clone();
            absent_edge
                .edges
                .retain(|edge| edge.subject_id != absent.reference_id);
            assert!(project_ruff_semantic_batch(scope, [10; 16], &absent_edge).is_err());
        }
    }

    #[test]
    fn py_import_export_fixture_conformance() {
        let batch = analyze(
            r#"
import os.path as osp
from .pkg import item as alias
from package import *
import importlib
dynamic = importlib.import_module(target)
__all__ = ["alias"] + ("osp",)
"#,
            false,
        )
        .unwrap();

        assert_eq!(batch.export_status, PythonExportStatus::Complete);
        assert_eq!(
            batch
                .imports
                .iter()
                .filter(|fact| fact.kind != PythonImportKind::Dynamic)
                .count(),
            4
        );
        let relative = batch
            .imports
            .iter()
            .find(|fact| fact.imported_name.as_deref() == Some("item"))
            .unwrap();
        assert_eq!(relative.relative_level, Some(1));
        assert_eq!(relative.target_module_name.as_deref(), Some("pkg"));
        assert_eq!(relative.ruff_qualified_name.as_deref(), Some("pkg.item"));
        assert_eq!(relative.resolution, PythonResolution::Resolved);
        assert_eq!(relative.alias_name.as_deref(), Some("alias"));
        assert!(relative.local_binding_id.is_some());
        assert!(batch.imports.iter().any(|fact| {
            fact.star_import
                && fact.resolution == PythonResolution::MayReferTo
                && fact.unknown_reason_code.as_deref() == Some("STAR_IMPORT_TARGET_SOURCE_DECLARED")
        }));
        assert!(
            batch
                .exports
                .iter()
                .any(|fact| fact.name == "alias" && fact.reexport)
        );
        assert!(
            batch
                .exports
                .iter()
                .any(|fact| fact.name == "osp" && fact.reexport)
        );

        #[cfg(feature = "daemon")]
        {
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let projection = project_ruff_semantic_batch(
                FactScope {
                    workspace_id: [31; 16],
                    analysis_context_id: [32; 16],
                    source_generation: 33,
                    owner_id: [34; 16],
                },
                [35; 16],
                &batch,
            )
            .unwrap();
            assert_eq!(
                projection.batch(230).unwrap().num_rows(),
                batch.imports.len()
            );
            assert_eq!(projection.batch(9).unwrap().num_rows(), 2);
        }
    }

    #[test]
    fn py_import_syntax_semantic_distinction_parity() {
        let batch = analyze("from package import symbol as local\n", false).unwrap();
        let import = batch.imports.first().unwrap();
        let binding = import.local_binding_id.unwrap();
        let symbol = import.imported_entity_id.unwrap();
        let distinct = [import.import_id, binding, import.target_module_id, symbol]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(distinct.len(), 4);
        assert!(batch.bindings.iter().any(|fact| {
            fact.binding_id == binding
                && fact.name == "local"
                && fact.target_form == PythonTargetForm::ImportAlias
        }));
        assert_eq!(import.target_module_name.as_deref(), Some("package"));
        assert_eq!(
            import.ruff_qualified_name.as_deref(),
            Some("package.symbol")
        );
        assert_eq!(import.resolution, PythonResolution::Resolved);
        assert_eq!(import.source_name, "package:symbol");
        assert_eq!(import.imported_name.as_deref(), Some("symbol"));
    }

    #[test]
    fn py_dynamic_export_unknown_falsification() {
        let batch = analyze(
            "__all__ = build_exports()\nmodule = __import__(module_name)\n",
            false,
        )
        .unwrap();
        assert_eq!(batch.export_status, PythonExportStatus::IncompleteDynamic);
        let dynamic = batch
            .imports
            .iter()
            .find(|fact| fact.kind == PythonImportKind::Dynamic)
            .unwrap();
        assert_eq!(dynamic.target_module_name, None);
        assert_eq!(dynamic.ruff_qualified_name, None);
        assert_eq!(dynamic.resolution, PythonResolution::UnknownSymbol);
        assert_eq!(
            dynamic.unknown_reason_code.as_deref(),
            Some("DYNAMIC_IMPORT_TARGET")
        );
        let implementation = include_str!("imports.rs");
        for prohibited in [
            "std::process",
            "tokio::process",
            "Command::new",
            "pyo3",
            "PyImport",
        ] {
            assert!(
                !implementation.contains(prohibited),
                "runtime import machinery escaped into the adapter: {prohibited}"
            );
        }

        #[cfg(feature = "daemon")]
        {
            use arrow_array::{Array as _, Int16Array};

            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let projection = project_ruff_semantic_batch(
                FactScope {
                    workspace_id: [41; 16],
                    analysis_context_id: [42; 16],
                    source_generation: 43,
                    owner_id: [44; 16],
                },
                [45; 16],
                &batch,
            )
            .unwrap();
            let capability = projection.batch(9).unwrap().batch();
            let completeness = capability
                .column_by_name("completeness_state_code")
                .unwrap()
                .as_any()
                .downcast_ref::<Int16Array>()
                .unwrap();
            assert!(completeness.iter().flatten().any(|code| code == 20));
            let details = projection.batch(230).unwrap().batch();
            assert!(
                details
                    .column_by_name("unknown_reason_code")
                    .unwrap()
                    .null_count()
                    < details.num_rows()
            );
        }
    }

    #[test]
    fn py_module_fact_replacement_gate() {
        #[cfg(feature = "daemon")]
        {
            use arrow_array::{Array as _, Int32Array};

            use crate::fabric::batch_checksum;
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let owner = FactScope {
                workspace_id: [51; 16],
                analysis_context_id: [52; 16],
                source_generation: 53,
                owner_id: [54; 16],
            };
            let before = project_ruff_semantic_batch(
                owner,
                [55; 16],
                &analyze("import alpha\n", false).unwrap(),
            )
            .unwrap();
            let after = project_ruff_semantic_batch(
                owner,
                [55; 16],
                &analyze("import alpha\nimport beta\n", false).unwrap(),
            )
            .unwrap();
            let stable_scope = FactScope {
                owner_id: [56; 16],
                ..owner
            };
            let stable_before = project_ruff_semantic_batch(
                stable_scope,
                [57; 16],
                &analyze("import stable\n", false).unwrap(),
            )
            .unwrap();
            let stable_after = project_ruff_semantic_batch(
                stable_scope,
                [57; 16],
                &analyze("import stable\n", false).unwrap(),
            )
            .unwrap();
            assert_ne!(
                batch_checksum(before.batch(230).unwrap().batch()).unwrap(),
                batch_checksum(after.batch(230).unwrap().batch()).unwrap()
            );
            assert_eq!(
                batch_checksum(stable_before.batch(230).unwrap().batch()).unwrap(),
                batch_checksum(stable_after.batch(230).unwrap().batch()).unwrap()
            );
            let relations = after.batch(110).unwrap().batch();
            let relation_kinds = relations
                .column_by_name("relation_kind_code")
                .unwrap()
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap();
            assert!(relation_kinds.iter().flatten().any(|code| code == 250));
        }
    }

    #[test]
    fn py_callable_call_site_fixture_conformance() {
        use super::super::callables::{CALLABLE_FLAG_ASYNC, CALLABLE_FLAG_GENERATOR};

        let batch = analyze(
            r#"
def deco(fn):
    return fn

@deco
async def target(a, /, b: int = 2, *args, c, d=4, **kwargs) -> int:
    yield a

class C:
    field = 1
    @property
    def prop(self):
        return self._value
    class Nested:
        pass
    def set(self, value):
        self.item = value
        def inner():
            return value
        return inner

result = target(1, 3, *[4, *[5]], c=6, **{"extra": 0, "extra": 7})
"#,
            false,
        )
        .unwrap();

        let target = batch
            .callables
            .iter()
            .find(|fact| fact.name == "target")
            .unwrap();
        assert_ne!(target.flags & CALLABLE_FLAG_ASYNC, 0);
        assert_ne!(target.flags & CALLABLE_FLAG_GENERATOR, 0);
        let parameters = batch
            .parameters
            .iter()
            .filter(|fact| fact.callable_id == target.callable_id)
            .collect::<Vec<_>>();
        assert_eq!(parameters.len(), 6);
        assert_eq!(
            parameters.iter().map(|fact| fact.kind).collect::<Vec<_>>(),
            vec![
                PythonParameterKind::PositionalOnly,
                PythonParameterKind::PositionalOrKeyword,
                PythonParameterKind::VarPositional,
                PythonParameterKind::KeywordOnly,
                PythonParameterKind::KeywordOnly,
                PythonParameterKind::VarKeyword,
            ]
        );
        assert!(parameters[1].annotation_syntax_id.is_some());
        assert!(parameters[1].default_syntax_id.is_some());
        assert!(batch.callable_syntax.iter().any(|fact| {
            fact.owner_id == target.callable_id
                && fact.role == PythonCallableSyntaxRole::Decorator
                && fact.text == "deco"
        }));
        assert!(batch.callable_syntax.iter().any(|fact| {
            fact.owner_id == target.callable_id
                && fact.role == PythonCallableSyntaxRole::ReturnAnnotation
                && fact.text == "int"
        }));

        let call = batch
            .call_sites
            .iter()
            .find(|fact| fact.declared_target_id == Some(target.callable_id))
            .unwrap();
        assert_eq!(call.resolved_target_count, 0);
        let arguments = batch
            .call_arguments
            .iter()
            .filter(|fact| fact.call_site_id == call.call_site_id)
            .collect::<Vec<_>>();
        assert_eq!(arguments.len(), 7);
        assert_eq!(
            arguments
                .iter()
                .filter(|fact| fact.binding_status == PythonArgumentBindingStatus::Bound)
                .count(),
            6
        );
        assert!(arguments.iter().any(|fact| {
            fact.binding_status == PythonArgumentBindingStatus::Defaulted
                && fact.parameter_id == Some(parameters[4].parameter_id)
        }));
        assert_eq!(
            arguments
                .iter()
                .filter(|fact| fact.spread_kind == PythonArgumentSpreadKind::PositionalStatic)
                .count(),
            2
        );
        assert!(arguments.iter().any(|fact| {
            fact.spread_kind == PythonArgumentSpreadKind::KeywordStatic
                && fact.keyword_name.as_deref() == Some("extra")
        }));

        let member_kinds = batch
            .members
            .iter()
            .map(|fact| fact.kind)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            member_kinds,
            std::collections::BTreeSet::from([
                PythonMemberKind::Method,
                PythonMemberKind::ClassVariable,
                PythonMemberKind::PropertyCandidate,
                PythonMemberKind::NestedType,
                PythonMemberKind::InstanceVariable,
            ])
        );
        assert!(!batch.members.iter().any(|fact| fact.name == "inner"));
    }

    #[test]
    fn py_call_site_first_class_parity() {
        let batch = analyze(
            "def f(x):\n    return x\ndef g():\n    return f(f(1))\ng()\n",
            false,
        )
        .unwrap();
        let call_syntax_count = batch
            .callable_syntax
            .iter()
            .filter(|fact| fact.role == PythonCallableSyntaxRole::CallExpression)
            .count();
        assert_eq!(call_syntax_count, 3);
        assert_eq!(batch.call_sites.len(), call_syntax_count);
        assert!(batch.call_sites.iter().all(|call| {
            batch
                .callables
                .iter()
                .any(|caller| caller.callable_id == call.caller_id)
                && batch.callable_syntax.iter().any(|syntax| {
                    syntax.syntax_id == call.syntax_id
                        && syntax.owner_id == call.call_site_id
                        && syntax.role == PythonCallableSyntaxRole::CallExpression
                })
        }));

        #[cfg(feature = "daemon")]
        {
            use arrow_array::Int32Array;

            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let projection = project_ruff_semantic_batch(
                FactScope {
                    workspace_id: [61; 16],
                    analysis_context_id: [62; 16],
                    source_generation: 63,
                    owner_id: [64; 16],
                },
                [65; 16],
                &batch,
            )
            .unwrap();
            assert_eq!(projection.batch(260).unwrap().num_rows(), 3);
            assert_eq!(
                projection.batch(270).unwrap().num_rows(),
                batch.call_arguments.len()
            );
            let relations = projection.batch(110).unwrap().batch();
            let kinds = relations
                .column_by_name("relation_kind_code")
                .unwrap()
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap();
            let kinds = kinds.iter().flatten().collect::<Vec<_>>();
            assert_eq!(kinds.iter().filter(|code| **code == 360).count(), 3);
            assert_eq!(kinds.iter().filter(|code| **code == 320).count(), 3);
            assert!(!kinds.contains(&50), "WP05 must not invent CALLS edges");
        }
    }

    #[test]
    fn py_dynamic_splat_unknown_argument_falsification() {
        let batch = analyze(
            "def target(a, /, b):\n    return a\ndef poskw(a, /, **kwargs):\n    return kwargs\ntarget(1, *values, **mapping)\ntarget(1, 2, b=3)\nposkw(a=1)\n",
            false,
        )
        .unwrap();
        assert_eq!(batch.call_sites.len(), 3);
        assert_eq!(batch.unknown_argument_sets.len(), 2);
        let unknown_ids = batch
            .unknown_argument_sets
            .iter()
            .map(|fact| fact.unknown_argument_set_id)
            .collect::<std::collections::BTreeSet<_>>();
        let dynamic = batch
            .call_arguments
            .iter()
            .filter(|fact| {
                matches!(
                    fact.spread_kind,
                    PythonArgumentSpreadKind::PositionalDynamic
                        | PythonArgumentSpreadKind::KeywordDynamic
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(dynamic.len(), 2);
        assert!(dynamic.iter().all(|fact| {
            fact.binding_status == PythonArgumentBindingStatus::UnknownArgumentSet
                && fact
                    .parameter_id
                    .is_some_and(|id| unknown_ids.contains(&id))
        }));
        assert!(batch.call_diagnostics.iter().any(|fact| {
            fact.code == "DUPLICATE_ARGUMENT"
                && batch
                    .call_arguments
                    .iter()
                    .any(|argument| argument.argument_id == fact.argument_id)
        }));
        let poskw = batch
            .callables
            .iter()
            .find(|fact| fact.name == "poskw")
            .unwrap();
        let poskw_call = batch
            .call_sites
            .iter()
            .find(|fact| fact.declared_target_id == Some(poskw.callable_id))
            .unwrap();
        let poskw_parameters = batch
            .parameters
            .iter()
            .filter(|fact| fact.callable_id == poskw.callable_id)
            .collect::<Vec<_>>();
        let poskw_arguments = batch
            .call_arguments
            .iter()
            .filter(|fact| fact.call_site_id == poskw_call.call_site_id)
            .collect::<Vec<_>>();
        assert!(poskw_arguments.iter().any(|argument| {
            argument.keyword_name.as_deref() == Some("a")
                && argument.parameter_id == Some(poskw_parameters[1].parameter_id)
                && argument.binding_status == PythonArgumentBindingStatus::Bound
        }));
        assert!(poskw_arguments.iter().any(|argument| {
            argument.parameter_id == Some(poskw_parameters[0].parameter_id)
                && argument.binding_status == PythonArgumentBindingStatus::MissingRequired
        }));
        assert!(!batch.call_diagnostics.iter().any(|fact| {
            fact.call_site_id == poskw_call.call_site_id && fact.code == "POSITIONAL_ONLY_KEYWORD"
        }));
        let dynamic_call = batch
            .call_sites
            .iter()
            .find(|fact| {
                batch
                    .call_arguments
                    .iter()
                    .filter(|argument| argument.call_site_id == fact.call_site_id)
                    .any(|argument| {
                        argument.binding_status == PythonArgumentBindingStatus::UnknownArgumentSet
                    })
            })
            .unwrap();
        assert!(!batch.call_arguments.iter().any(|argument| {
            argument.call_site_id == dynamic_call.call_site_id
                && matches!(
                    argument.binding_status,
                    PythonArgumentBindingStatus::Defaulted
                        | PythonArgumentBindingStatus::MissingRequired
                )
        }));
    }

    #[test]
    fn py_callable_contract_replacement_gate() {
        #[cfg(feature = "daemon")]
        {
            use crate::fabric::batch_checksum;
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;

            let owner = FactScope {
                workspace_id: [71; 16],
                analysis_context_id: [72; 16],
                source_generation: 73,
                owner_id: [74; 16],
            };
            let before = project_ruff_semantic_batch(
                owner,
                [75; 16],
                &analyze("def target(a, b):\n    return a\ntarget(1)\n", false).unwrap(),
            )
            .unwrap();
            let after = project_ruff_semantic_batch(
                owner,
                [75; 16],
                &analyze("def target(a, c):\n    return a\ntarget(1)\n", false).unwrap(),
            )
            .unwrap();
            let stable_scope = FactScope {
                owner_id: [76; 16],
                ..owner
            };
            let stable_before = project_ruff_semantic_batch(
                stable_scope,
                [77; 16],
                &analyze("def stable(a):\n    return a\nstable(1)\n", false).unwrap(),
            )
            .unwrap();
            let stable_after = project_ruff_semantic_batch(
                stable_scope,
                [77; 16],
                &analyze("def stable(a):\n    return a\nstable(1)\n", false).unwrap(),
            )
            .unwrap();
            assert_ne!(
                batch_checksum(before.batch(250).unwrap().batch()).unwrap(),
                batch_checksum(after.batch(250).unwrap().batch()).unwrap()
            );
            assert_ne!(
                batch_checksum(before.batch(270).unwrap().batch()).unwrap(),
                batch_checksum(after.batch(270).unwrap().batch()).unwrap()
            );
            for table_code in [240, 250, 260, 270] {
                assert_eq!(
                    batch_checksum(stable_before.batch(table_code).unwrap().batch()).unwrap(),
                    batch_checksum(stable_after.batch(table_code).unwrap().batch()).unwrap(),
                    "unchanged owner table {table_code}"
                );
            }
        }
    }

    #[test]
    fn ruff_semantic_isolation_parity() {
        let error = analyze("x = 1\n", true).unwrap_err();
        assert_eq!(error, PythonSemanticError::CleanupFailure);
        assert!(error.to_string().contains("terminal_state=failed"));
        assert!(error.to_string().contains("failure_code=cleanup-failure"));

        let mut violations = Vec::new();
        inspect_non_adapter_rust(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
            &mut violations,
        );
        assert!(
            violations.is_empty(),
            "Ruff semantic types escaped: {violations:?}"
        );

        #[cfg(feature = "daemon")]
        {
            use std::io::Cursor;

            use crate::fabric::batch_checksum;
            use crate::fact_ingest::FactScope;
            use crate::python_semantic::project_ruff_semantic_batch;
            use arrow::ipc::reader::StreamReader;
            use arrow::ipc::writer::StreamWriter;

            let batch = analyze("x = 1\nprint(x)\n", false).unwrap();
            let projection = project_ruff_semantic_batch(
                FactScope {
                    workspace_id: [11; 16],
                    analysis_context_id: [12; 16],
                    source_generation: 13,
                    owner_id: [14; 16],
                },
                [15; 16],
                &batch,
            )
            .unwrap();
            for table_code in [200, 210, 220] {
                let original = projection.batch(table_code).unwrap().batch();
                let mut bytes = Vec::new();
                {
                    let mut writer =
                        StreamWriter::try_new(&mut bytes, original.schema().as_ref()).unwrap();
                    writer.write(original).unwrap();
                    writer.finish().unwrap();
                }
                let mut reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
                let decoded = reader.next().unwrap().unwrap();
                assert_eq!(original.schema(), decoded.schema());
                assert_eq!(
                    batch_checksum(original).unwrap(),
                    batch_checksum(&decoded).unwrap(),
                    "table {table_code} Arrow round trip"
                );
                assert!(reader.next().is_none());
            }
        }
    }
}
