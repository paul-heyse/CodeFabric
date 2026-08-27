//! Application-owned Python control-flow graph construction over Ruff's typed AST.
//!
//! `petgraph` indices exist only while a graph is being assembled and validated.
//! Every observation crossing this module boundary is keyed by a deterministic
//! CodeFabric identity.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::Direction;
use petgraph::algo::has_path_connecting;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{
    Comprehension, ExceptHandler, Expr, InterpolatedStringElement, Parameters, Pattern,
    PatternOrKeyword, Stmt, StmtFunctionDef, TypeParam, TypeParams,
};
use ruff_text_size::{Ranged, TextRange};

use super::callables::PythonCallableFact;
use super::semantic::{PythonSemanticError, PythonSemanticId, semantic_id};

const NODE_FLAG_FINALLY_ENTRY: i64 = 1 << 0;
const NODE_FLAG_FINALLY_RESUME: i64 = 1 << 1;
const NODE_FLAG_HANDLER: i64 = 1 << 2;
const NODE_FLAG_SYNTHETIC: i64 = 1 << 3;
const EDGE_FLAG_EXACT_EXCEPTION: i64 = 1 << 0;
const EDGE_FLAG_SUMMARY_EXCEPTION: i64 = 1 << 1;
const EDGE_FLAG_FINALLY_ROUTE: i64 = 1 << 2;
const EDGE_FLAG_FINALLY_RESUME: i64 = 1 << 3;

/// Governed kinds of Python CFG owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonCfgKind {
    Module,
    Function,
    AsyncFunction,
    Lambda,
}

impl PythonCfgKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Module => 10,
            Self::Function => 20,
            Self::AsyncFunction => 30,
            Self::Lambda => 40,
        }
    }

    const fn identity_discriminator(self) -> u8 {
        match self {
            Self::Module => 10,
            Self::Function => 20,
            Self::AsyncFunction => 30,
            Self::Lambda => 40,
        }
    }
}

/// Application-owned CFG node vocabulary from ONT section 15 and GEN section 24.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonCfgNodeKind {
    Entry,
    Exit,
    BasicBlock,
    ExpressionOperation,
    StatementOperation,
    Branch,
    Switch,
    LoopHeader,
    ReturnPoint,
    ExceptionalExit,
    SuspendPoint,
    ResumePoint,
}

impl PythonCfgNodeKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Entry => 10,
            Self::Exit => 20,
            Self::BasicBlock => 30,
            Self::ExpressionOperation => 40,
            Self::StatementOperation => 50,
            Self::Branch => 60,
            Self::Switch => 70,
            Self::LoopHeader => 80,
            Self::ReturnPoint => 90,
            Self::ExceptionalExit => 100,
            Self::SuspendPoint => 110,
            Self::ResumePoint => 120,
        }
    }

    const fn terminal(self) -> bool {
        matches!(self, Self::Exit | Self::ExceptionalExit)
    }
}

/// Normal, branch, loop, exceptional, and suspension relations remain distinct.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonCfgEdgeKind {
    Next,
    True,
    False,
    Case,
    LoopBack,
    Break,
    Continue,
    Return,
    Exception,
    Unwind,
    CallReturn,
    Cleanup,
    Suspend,
    Resume,
}

impl PythonCfgEdgeKind {
    pub(crate) const fn registry_name(self) -> &'static str {
        match self {
            Self::Next => "CFG_NEXT",
            Self::True => "CFG_TRUE",
            Self::False => "CFG_FALSE",
            Self::Case => "CFG_CASE",
            Self::LoopBack => "CFG_LOOP_BACK",
            Self::Break => "CFG_BREAK",
            Self::Continue => "CFG_CONTINUE",
            Self::Return => "CFG_RETURN",
            Self::Exception => "CFG_EXCEPTION",
            Self::Unwind => "CFG_UNWIND",
            Self::CallReturn => "CFG_CALL_RETURN",
            Self::Cleanup => "CFG_CLEANUP",
            Self::Suspend => "CFG_SUSPEND",
            Self::Resume => "CFG_RESUME",
        }
    }

    const fn exceptional(self) -> bool {
        matches!(self, Self::Exception | Self::Unwind)
    }
}

/// One validated CFG header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgFact {
    pub cfg_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub callable_id: PythonSemanticId,
    pub kind: PythonCfgKind,
    pub entry_node_id: PythonSemanticId,
    pub exit_node_id: PythonSemanticId,
    pub exceptional_exit_node_id: PythonSemanticId,
    pub node_count: i32,
    pub edge_count: i32,
    pub flags: i64,
}

/// One persisted CFG node. Source coordinates remain application values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgNodeFact {
    pub cfg_node_id: PythonSemanticId,
    pub cfg_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub kind: PythonCfgNodeKind,
    pub ordinal: i32,
    pub label: &'static str,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
    pub flags: i64,
}

/// One persisted CFG relation and its columnar edge attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgEdgeFact {
    pub relation_id: PythonSemanticId,
    pub cfg_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub source_node_id: PythonSemanticId,
    pub target_node_id: PythonSemanticId,
    pub kind: PythonCfgEdgeKind,
    pub condition_node_id: Option<PythonSemanticId>,
    pub case_value_text: Option<String>,
    pub exception_category: Option<&'static str>,
    pub flags: i64,
}

/// A rejected CFG never reaches canonical ingest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgValidationError {
    pub cfg_id: PythonSemanticId,
    pub message: String,
}

#[derive(Clone)]
struct NodeDraft {
    id: PythonSemanticId,
    kind: PythonCfgNodeKind,
    ordinal: i32,
    label: &'static str,
    range: Option<TextRange>,
    flags: i64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EdgeDraft {
    kind: PythonCfgEdgeKind,
    condition: Option<PythonSemanticId>,
    case_value: Option<String>,
    exception_category: Option<&'static str>,
    flags: i64,
}

#[derive(Clone, Copy)]
struct FinalizerRoutes {
    normal: NodeIndex,
    return_: NodeIndex,
    exception: NodeIndex,
    break_: Option<NodeIndex>,
    continue_: Option<NodeIndex>,
}

#[derive(Clone, Copy)]
struct BuildContext {
    exception_target: NodeIndex,
    finalizer: Option<FinalizerRoutes>,
}

#[derive(Clone, Copy)]
struct LoopTargets {
    break_: NodeIndex,
    continue_: NodeIndex,
}

fn pending_continuation(label: &str) -> Option<PythonCfgEdgeKind> {
    if label.contains("return") {
        Some(PythonCfgEdgeKind::Return)
    } else if label.contains("exception") {
        Some(PythonCfgEdgeKind::Unwind)
    } else if label.contains("break") {
        Some(PythonCfgEdgeKind::Break)
    } else if label.contains("continue") {
        Some(PythonCfgEdgeKind::Continue)
    } else if label.contains("normal") {
        Some(PythonCfgEdgeKind::Next)
    } else {
        None
    }
}

pub(super) struct PythonCfgProjection {
    pub cfgs: Vec<PythonCfgFact>,
    pub nodes: Vec<PythonCfgNodeFact>,
    pub edges: Vec<PythonCfgEdgeFact>,
}

enum Unit<'a> {
    Function(&'a StmtFunctionDef),
    Lambda(&'a ruff_python_ast::ExprLambda),
}

#[derive(Default)]
struct UnitCollector<'a> {
    units: Vec<Unit<'a>>,
}

impl<'a> Visitor<'a> for UnitCollector<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Stmt::FunctionDef(function) = statement {
            self.units.push(Unit::Function(function));
        }
        visitor::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Lambda(lambda) = expression {
            self.units.push(Unit::Lambda(lambda));
        }
        visitor::walk_expr(self, expression);
    }
}

pub(super) fn project_python_cfgs(
    suite: &[Stmt],
    module_name: &str,
    fingerprint: &str,
    callables: &[PythonCallableFact],
) -> Result<PythonCfgProjection, PythonSemanticError> {
    let module = callables
        .iter()
        .find(|callable| callable.flags & (1 << 7) != 0)
        .ok_or_else(|| {
            PythonSemanticError::Invariant("module-body callable absent for CFG".into())
        })?;
    let mut built = Vec::new();
    built.push(
        CfgBuilder::new(
            fingerprint,
            module.callable_id,
            PythonCfgKind::Module,
            0,
            suite
                .last()
                .map_or(0, |statement| u64::from(u32::from(statement.end()))),
            module_name,
        )
        .build_suite_graph(suite)?,
    );

    let mut collector = UnitCollector::default();
    visitor::walk_body(&mut collector, suite);
    for unit in collector.units {
        let (range, kind, body, lambda_body, label) = match unit {
            Unit::Function(function) => (
                function.range,
                if function.is_async {
                    PythonCfgKind::AsyncFunction
                } else {
                    PythonCfgKind::Function
                },
                Some(function.body.as_slice()),
                None,
                function.name.as_str(),
            ),
            Unit::Lambda(lambda) => (
                lambda.range,
                PythonCfgKind::Lambda,
                None,
                Some(lambda.body.as_ref()),
                "<lambda>",
            ),
        };
        let start = u64::from(u32::from(range.start()));
        let end = u64::from(u32::from(range.end()));
        let callable = callables
            .iter()
            .find(|candidate| candidate.start_byte == start && candidate.end_byte == end)
            .ok_or_else(|| {
                PythonSemanticError::Invariant(format!(
                    "CFG owner has no callable fact at {start}..{end}"
                ))
            })?;
        let builder = CfgBuilder::new(fingerprint, callable.callable_id, kind, start, end, label);
        built.push(if let Some(body) = body {
            builder.build_suite_graph(body)?
        } else {
            builder.build_lambda_graph(lambda_body.expect("lambda body selected"))?
        });
    }

    let mut projection = PythonCfgProjection {
        cfgs: Vec::with_capacity(built.len()),
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    for (cfg, nodes, edges) in built {
        projection.cfgs.push(cfg);
        projection.nodes.extend(nodes);
        projection.edges.extend(edges);
    }
    projection.cfgs.sort_by_key(|cfg| (cfg.kind, cfg.cfg_id));
    projection
        .nodes
        .sort_by_key(|node| (node.cfg_id, node.ordinal, node.cfg_node_id));
    projection.edges.sort_by_key(|edge| {
        (
            edge.cfg_id,
            edge.source_node_id,
            edge.target_node_id,
            edge.kind,
            edge.relation_id,
        )
    });
    Ok(projection)
}

struct CfgBuilder<'a> {
    fingerprint: &'a str,
    owner_id: PythonSemanticId,
    cfg_id: PythonSemanticId,
    kind: PythonCfgKind,
    graph: DiGraph<NodeDraft, EdgeDraft>,
    node_by_id: BTreeMap<PythonSemanticId, NodeIndex>,
    edge_keys: BTreeSet<(NodeIndex, NodeIndex, EdgeDraft)>,
    ordinal: i32,
    entry: NodeIndex,
    exit: NodeIndex,
    exceptional_exit: NodeIndex,
    loops: Vec<LoopTargets>,
}

impl<'a> CfgBuilder<'a> {
    fn new(
        fingerprint: &'a str,
        owner_id: PythonSemanticId,
        kind: PythonCfgKind,
        start: u64,
        end: u64,
        label: &str,
    ) -> Self {
        let discriminator = kind.identity_discriminator();
        let cfg_id = semantic_id(fingerprint, "cfg", start, end, label, discriminator);
        let mut graph = DiGraph::new();
        let mut node_by_id = BTreeMap::new();
        let entry_id = semantic_id(
            fingerprint,
            "cfg-node-entry",
            start,
            end,
            label,
            discriminator,
        );
        let exit_id = semantic_id(
            fingerprint,
            "cfg-node-exit",
            start,
            end,
            label,
            discriminator,
        );
        let exceptional_id = semantic_id(
            fingerprint,
            "cfg-node-exceptional-exit",
            start,
            end,
            label,
            discriminator,
        );
        let entry = graph.add_node(NodeDraft {
            id: entry_id,
            kind: PythonCfgNodeKind::Entry,
            ordinal: 0,
            label: "entry",
            range: None,
            flags: NODE_FLAG_SYNTHETIC,
        });
        let exit = graph.add_node(NodeDraft {
            id: exit_id,
            kind: PythonCfgNodeKind::Exit,
            ordinal: 1,
            label: "exit",
            range: None,
            flags: NODE_FLAG_SYNTHETIC,
        });
        let exceptional_exit = graph.add_node(NodeDraft {
            id: exceptional_id,
            kind: PythonCfgNodeKind::ExceptionalExit,
            ordinal: 2,
            label: "exceptional-exit",
            range: None,
            flags: NODE_FLAG_SYNTHETIC,
        });
        node_by_id.insert(entry_id, entry);
        node_by_id.insert(exit_id, exit);
        node_by_id.insert(exceptional_id, exceptional_exit);
        Self {
            fingerprint,
            owner_id,
            cfg_id,
            kind,
            graph,
            node_by_id,
            edge_keys: BTreeSet::new(),
            ordinal: 3,
            entry,
            exit,
            exceptional_exit,
            loops: Vec::new(),
        }
    }

    fn build_suite_graph(
        mut self,
        suite: &[Stmt],
    ) -> Result<
        (
            PythonCfgFact,
            Vec<PythonCfgNodeFact>,
            Vec<PythonCfgEdgeFact>,
        ),
        PythonSemanticError,
    > {
        let context = BuildContext {
            exception_target: self.exceptional_exit,
            finalizer: None,
        };
        let tails = self.build_suite(suite, vec![self.entry], context);
        for tail in tails {
            self.edge(
                tail,
                self.exit,
                PythonCfgEdgeKind::Next,
                None,
                None,
                None,
                0,
            );
        }
        self.finish()
    }

    fn build_lambda_graph(
        mut self,
        expression: &Expr,
    ) -> Result<
        (
            PythonCfgFact,
            Vec<PythonCfgNodeFact>,
            Vec<PythonCfgEdgeFact>,
        ),
        PythonSemanticError,
    > {
        let context = BuildContext {
            exception_target: self.exceptional_exit,
            finalizer: None,
        };
        let tails = self.eval_expr(expression, vec![self.entry], context);
        let return_point = self.node(
            PythonCfgNodeKind::ReturnPoint,
            "lambda-return",
            Some(expression.range()),
            0,
        );
        self.connect(&tails, return_point, PythonCfgEdgeKind::Next);
        self.edge(
            return_point,
            self.exit,
            PythonCfgEdgeKind::Return,
            None,
            None,
            None,
            0,
        );
        self.finish()
    }

    fn finish(
        self,
    ) -> Result<
        (
            PythonCfgFact,
            Vec<PythonCfgNodeFact>,
            Vec<PythonCfgEdgeFact>,
        ),
        PythonSemanticError,
    > {
        validate_graph(
            self.cfg_id,
            &self.graph,
            self.entry,
            self.exit,
            self.exceptional_exit,
        )
        .map_err(|error| PythonSemanticError::Invariant(error.message))?;
        let node_count = i32::try_from(self.graph.node_count()).unwrap_or(i32::MAX);
        let edge_count = i32::try_from(self.graph.edge_count()).unwrap_or(i32::MAX);
        let nodes = self
            .graph
            .node_indices()
            .map(|index| {
                let node = &self.graph[index];
                PythonCfgNodeFact {
                    cfg_node_id: node.id,
                    cfg_id: self.cfg_id,
                    owner_id: self.owner_id,
                    kind: node.kind,
                    ordinal: node.ordinal,
                    label: node.label,
                    start_byte: node.range.map(|range| u64::from(u32::from(range.start()))),
                    end_byte: node.range.map(|range| u64::from(u32::from(range.end()))),
                    flags: node.flags,
                }
            })
            .collect::<Vec<_>>();
        let edges = self
            .graph
            .edge_references()
            .map(|edge| {
                let source = self.graph[edge.source()].id;
                let target = self.graph[edge.target()].id;
                let weight = edge.weight();
                let relation_id =
                    edge_identity(self.fingerprint, self.cfg_id, source, target, weight);
                PythonCfgEdgeFact {
                    relation_id,
                    cfg_id: self.cfg_id,
                    owner_id: self.owner_id,
                    source_node_id: source,
                    target_node_id: target,
                    kind: weight.kind,
                    condition_node_id: weight.condition,
                    case_value_text: weight.case_value.clone(),
                    exception_category: weight.exception_category,
                    flags: weight.flags,
                }
            })
            .collect::<Vec<_>>();
        Ok((
            PythonCfgFact {
                cfg_id: self.cfg_id,
                owner_id: self.owner_id,
                callable_id: self.owner_id,
                kind: self.kind,
                entry_node_id: self.graph[self.entry].id,
                exit_node_id: self.graph[self.exit].id,
                exceptional_exit_node_id: self.graph[self.exceptional_exit].id,
                node_count,
                edge_count,
                flags: 0,
            },
            nodes,
            edges,
        ))
    }

    fn build_suite(
        &mut self,
        suite: &[Stmt],
        mut tails: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        for statement in suite {
            if tails.is_empty() {
                let unreachable = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "unreachable-region",
                    Some(statement.range()),
                    NODE_FLAG_SYNTHETIC,
                );
                tails.push(unreachable);
            }
            tails = self.build_stmt(statement, tails, context);
        }
        tails
    }

    #[allow(clippy::too_many_lines)]
    fn build_stmt(
        &mut self,
        statement: &Stmt,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        match statement {
            Stmt::Expr(node) => self.eval_expr(&node.value, inputs, context),
            Stmt::Return(node) => {
                let tails = if let Some(value) = node.value.as_deref() {
                    self.eval_expr(value, inputs, context)
                } else {
                    inputs
                };
                let return_point = self.node(
                    PythonCfgNodeKind::ReturnPoint,
                    "return",
                    Some(node.range),
                    0,
                );
                self.connect(&tails, return_point, PythonCfgEdgeKind::Next);
                self.route_continuation(
                    return_point,
                    self.exit,
                    PythonCfgEdgeKind::Return,
                    context,
                );
                Vec::new()
            }
            Stmt::Raise(node) => {
                let mut tails = inputs;
                if let Some(exception) = node.exc.as_deref() {
                    tails = self.eval_expr(exception, tails, context);
                }
                if let Some(cause) = node.cause.as_deref() {
                    tails = self.eval_expr(cause, tails, context);
                }
                let raise = self.node(
                    PythonCfgNodeKind::StatementOperation,
                    "raise",
                    Some(node.range),
                    0,
                );
                self.connect(&tails, raise, PythonCfgEdgeKind::Next);
                self.edge(
                    raise,
                    context.exception_target,
                    PythonCfgEdgeKind::Exception,
                    None,
                    None,
                    Some("explicit-raise"),
                    Self::exception_flags(context, EDGE_FLAG_EXACT_EXCEPTION),
                );
                Vec::new()
            }
            Stmt::Break(node) => {
                let op = self.node(
                    PythonCfgNodeKind::StatementOperation,
                    "break",
                    Some(node.range),
                    0,
                );
                self.connect(&inputs, op, PythonCfgEdgeKind::Next);
                if let Some(target) = self.loops.last().copied() {
                    self.route_continuation(op, target.break_, PythonCfgEdgeKind::Break, context);
                }
                Vec::new()
            }
            Stmt::Continue(node) => {
                let op = self.node(
                    PythonCfgNodeKind::StatementOperation,
                    "continue",
                    Some(node.range),
                    0,
                );
                self.connect(&inputs, op, PythonCfgEdgeKind::Next);
                if let Some(target) = self.loops.last().copied() {
                    self.route_continuation(
                        op,
                        target.continue_,
                        PythonCfgEdgeKind::Continue,
                        context,
                    );
                }
                Vec::new()
            }
            Stmt::If(node) => self.build_if(node, inputs, context),
            Stmt::While(node) => self.build_while(node, &inputs, context),
            Stmt::For(node) => self.build_for(node, inputs, context),
            Stmt::Try(node) => self.build_try(node, inputs, context),
            Stmt::With(node) => self.build_with(node, inputs, context),
            Stmt::Match(node) => self.build_match(node, inputs, context),
            Stmt::Assert(node) => self.build_assert(node, inputs, context),
            Stmt::Assign(node) => {
                let mut tails = self.eval_expr(&node.value, inputs, context);
                for target in &node.targets {
                    tails = self.eval_store(target, tails, context);
                }
                tails
            }
            Stmt::AnnAssign(node) => {
                let mut tails = self.eval_expr(&node.annotation, inputs, context);
                if let Some(value) = node.value.as_deref() {
                    tails = self.eval_expr(value, tails, context);
                }
                self.eval_store(&node.target, tails, context)
            }
            Stmt::AugAssign(node) => {
                let tails = self.eval_expr(&node.target, inputs, context);
                let tails = self.eval_expr(&node.value, tails, context);
                self.eval_store(&node.target, tails, context)
            }
            Stmt::Delete(node) => self.eval_exprs(node.targets.iter(), inputs, context),
            Stmt::TypeAlias(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                self.eval_store(&node.name, tails, context)
            }
            Stmt::FunctionDef(node) => self.eval_definition(node, inputs, context),
            Stmt::ClassDef(node) => {
                let mut tails = inputs;
                for decorator in &node.decorator_list {
                    tails = self.eval_expr(&decorator.expression, tails, context);
                }
                if let Some(type_params) = node.type_params.as_deref() {
                    tails = self.eval_type_params(type_params, tails, context);
                }
                if let Some(arguments) = node.arguments.as_deref() {
                    for argument in &arguments.args {
                        tails = self.eval_expr(argument, tails, context);
                    }
                    for keyword in &arguments.keywords {
                        tails = self.eval_expr(&keyword.value, tails, context);
                    }
                }
                let create = self.operation(
                    &tails,
                    PythonCfgNodeKind::StatementOperation,
                    "class-body-execute",
                    node.range,
                    context,
                    Some("class-body"),
                );
                let mut tails = vec![create];
                for _ in node.decorator_list.iter().rev() {
                    let application = self.node(
                        PythonCfgNodeKind::ExpressionOperation,
                        "decorator-apply",
                        Some(node.range),
                        0,
                    );
                    self.connect(&tails, application, PythonCfgEdgeKind::Next);
                    tails = vec![application];
                }
                tails
            }
            Stmt::Import(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "import",
                node.range,
                context,
                Some("import"),
            )],
            Stmt::ImportFrom(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "import-from",
                node.range,
                context,
                Some("import"),
            )],
            Stmt::Pass(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "pass",
                node.range,
                context,
                None,
            )],
            Stmt::Global(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "global-declaration",
                node.range,
                context,
                None,
            )],
            Stmt::Nonlocal(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "nonlocal-declaration",
                node.range,
                context,
                None,
            )],
            Stmt::IpyEscapeCommand(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "ipy-escape",
                node.range,
                context,
                Some("ipy-escape"),
            )],
        }
    }

    fn build_if(
        &mut self,
        node: &ruff_python_ast::StmtIf,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let mut test_inputs = inputs;
        let mut false_tails = Vec::new();
        let mut completed = Vec::new();
        let mut clauses = Vec::with_capacity(node.elif_else_clauses.len() + 1);
        clauses.push((Some(node.test.as_ref()), node.body.as_slice()));
        clauses.extend(
            node.elif_else_clauses
                .iter()
                .map(|clause| (clause.test.as_ref(), clause.body.as_slice())),
        );
        for (test, body) in clauses {
            if let Some(test) = test {
                let test_tails = self.eval_expr(test, test_inputs, context);
                let branch = self.node(
                    PythonCfgNodeKind::Branch,
                    "if-branch",
                    Some(test.range()),
                    0,
                );
                self.connect(&test_tails, branch, PythonCfgEdgeKind::Next);
                let body_entry = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "if-body",
                    body.first().map(Ranged::range),
                    NODE_FLAG_SYNTHETIC,
                );
                self.edge(
                    branch,
                    body_entry,
                    PythonCfgEdgeKind::True,
                    Some(branch),
                    None,
                    None,
                    0,
                );
                completed.extend(self.build_suite(body, vec![body_entry], context));
                let next = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "if-next-clause",
                    None,
                    NODE_FLAG_SYNTHETIC,
                );
                self.edge(
                    branch,
                    next,
                    PythonCfgEdgeKind::False,
                    Some(branch),
                    None,
                    None,
                    0,
                );
                test_inputs = vec![next];
                false_tails.clone_from(&test_inputs);
            } else {
                completed.extend(self.build_suite(body, test_inputs.clone(), context));
                false_tails.clear();
            }
        }
        completed.extend(false_tails);
        completed
    }

    fn build_while(
        &mut self,
        node: &ruff_python_ast::StmtWhile,
        inputs: &[NodeIndex],
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let header = self.node(
            PythonCfgNodeKind::LoopHeader,
            "while-header",
            Some(node.range),
            0,
        );
        self.connect(inputs, header, PythonCfgEdgeKind::Next);
        let test_tails = self.eval_expr(&node.test, vec![header], context);
        let branch = self.node(
            PythonCfgNodeKind::Branch,
            "while-test",
            Some(node.test.range()),
            0,
        );
        self.connect(&test_tails, branch, PythonCfgEdgeKind::Next);
        let body_entry = self.node(
            PythonCfgNodeKind::BasicBlock,
            "while-body",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        let normal_exit = self.node(
            PythonCfgNodeKind::BasicBlock,
            "while-normal-exit",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        let break_exit = self.node(
            PythonCfgNodeKind::BasicBlock,
            "while-break-exit",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        self.edge(
            branch,
            body_entry,
            PythonCfgEdgeKind::True,
            Some(branch),
            None,
            None,
            0,
        );
        self.edge(
            branch,
            normal_exit,
            PythonCfgEdgeKind::False,
            Some(branch),
            None,
            None,
            0,
        );
        self.loops.push(LoopTargets {
            break_: break_exit,
            continue_: header,
        });
        let body_tails = self.build_suite(&node.body, vec![body_entry], context);
        self.loops.pop();
        for tail in body_tails {
            self.edge(
                tail,
                header,
                PythonCfgEdgeKind::LoopBack,
                None,
                None,
                None,
                0,
            );
        }
        let else_tails = self.build_suite(&node.orelse, vec![normal_exit], context);
        else_tails.into_iter().chain([break_exit]).collect()
    }

    fn build_for(
        &mut self,
        node: &ruff_python_ast::StmtFor,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let iter_tails = self.eval_expr(&node.iter, inputs, context);
        let header = self.operation(
            &iter_tails,
            PythonCfgNodeKind::LoopHeader,
            if node.is_async {
                "async-for-next"
            } else {
                "for-next"
            },
            node.iter.range(),
            context,
            Some("iteration"),
        );
        let body_entry = self.node(
            PythonCfgNodeKind::BasicBlock,
            "for-body",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        let normal_exit = self.node(
            PythonCfgNodeKind::BasicBlock,
            "for-normal-exit",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        let break_exit = self.node(
            PythonCfgNodeKind::BasicBlock,
            "for-break-exit",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        self.edge(
            header,
            body_entry,
            PythonCfgEdgeKind::True,
            Some(header),
            None,
            None,
            0,
        );
        self.edge(
            header,
            normal_exit,
            PythonCfgEdgeKind::False,
            Some(header),
            None,
            None,
            0,
        );
        let target_tails = self.eval_store(&node.target, vec![body_entry], context);
        self.loops.push(LoopTargets {
            break_: break_exit,
            continue_: header,
        });
        let body_tails = self.build_suite(&node.body, target_tails, context);
        self.loops.pop();
        for tail in body_tails {
            self.edge(
                tail,
                header,
                PythonCfgEdgeKind::LoopBack,
                None,
                None,
                None,
                0,
            );
        }
        let else_tails = self.build_suite(&node.orelse, vec![normal_exit], context);
        else_tails.into_iter().chain([break_exit]).collect()
    }

    #[allow(clippy::too_many_lines)] // Mirrors the complete try/except/else/finally rule table.
    fn build_try(
        &mut self,
        node: &ruff_python_ast::StmtTry,
        inputs: Vec<NodeIndex>,
        outer: BuildContext,
    ) -> Vec<NodeIndex> {
        let has_finally = !node.finalbody.is_empty();
        let inside_loop = self.loops.last().is_some();
        let finalizer = has_finally.then(|| FinalizerRoutes {
            normal: self.node(
                PythonCfgNodeKind::BasicBlock,
                "finally-normal",
                Some(node.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            return_: self.node(
                PythonCfgNodeKind::BasicBlock,
                "finally-return",
                Some(node.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            exception: self.node(
                PythonCfgNodeKind::BasicBlock,
                "finally-exception",
                Some(node.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            break_: inside_loop.then(|| {
                self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "finally-break",
                    Some(node.range),
                    NODE_FLAG_FINALLY_ENTRY,
                )
            }),
            continue_: inside_loop.then(|| {
                self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "finally-continue",
                    Some(node.range),
                    NODE_FLAG_FINALLY_ENTRY,
                )
            }),
        });
        let handler_dispatch = (!node.handlers.is_empty()).then(|| {
            self.node(
                PythonCfgNodeKind::Switch,
                "exception-handler-dispatch",
                Some(node.range),
                NODE_FLAG_HANDLER,
            )
        });
        let exception_target = handler_dispatch
            .unwrap_or_else(|| finalizer.map_or(outer.exception_target, |routes| routes.exception));
        let protected = BuildContext {
            exception_target,
            finalizer,
        };
        let body_tails = self.build_suite(&node.body, inputs, protected);
        let mut normal_tails = self.build_suite(&node.orelse, body_tails, protected);

        if let Some(dispatch) = handler_dispatch {
            for (ordinal, handler) in node.handlers.iter().enumerate() {
                let ExceptHandler::ExceptHandler(handler) = handler;
                let handler_entry = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "exception-handler",
                    Some(handler.range),
                    NODE_FLAG_HANDLER,
                );
                self.edge(
                    dispatch,
                    handler_entry,
                    PythonCfgEdgeKind::Case,
                    Some(dispatch),
                    Some(format!("handler-{ordinal}")),
                    handler.type_.as_ref().map(|_| "handler-type"),
                    EDGE_FLAG_SUMMARY_EXCEPTION,
                );
                let mut handler_inputs = vec![handler_entry];
                if let Some(type_) = handler.type_.as_deref() {
                    handler_inputs = self.eval_expr(type_, handler_inputs, protected);
                }
                normal_tails.extend(self.build_suite(&handler.body, handler_inputs, protected));
            }
            let unhandled = finalizer.map_or(outer.exception_target, |routes| routes.exception);
            self.edge(
                dispatch,
                unhandled,
                PythonCfgEdgeKind::Unwind,
                None,
                None,
                Some(if node.is_star {
                    "unhandled-exception-group"
                } else {
                    "unhandled-exception"
                }),
                EDGE_FLAG_SUMMARY_EXCEPTION
                    | if finalizer.is_some() {
                        EDGE_FLAG_FINALLY_ROUTE
                    } else {
                        0
                    },
            );
        }

        let Some(routes) = finalizer else {
            return normal_tails;
        };
        for tail in normal_tails {
            self.edge(
                tail,
                routes.normal,
                PythonCfgEdgeKind::Cleanup,
                None,
                None,
                None,
                EDGE_FLAG_FINALLY_ROUTE,
            );
        }
        let merge = self.node(
            PythonCfgNodeKind::BasicBlock,
            "finally-merge",
            Some(node.range),
            NODE_FLAG_FINALLY_RESUME,
        );
        self.run_finalbody(
            routes.normal,
            &node.finalbody,
            outer,
            merge,
            PythonCfgEdgeKind::Next,
        );
        self.run_finalbody(
            routes.return_,
            &node.finalbody,
            outer,
            self.exit,
            PythonCfgEdgeKind::Return,
        );
        self.run_finalbody(
            routes.exception,
            &node.finalbody,
            outer,
            outer.exception_target,
            PythonCfgEdgeKind::Unwind,
        );
        if let (Some(entry), Some(target)) = (
            routes.break_,
            self.loops.last().map(|targets| targets.break_),
        ) {
            self.run_finalbody(
                entry,
                &node.finalbody,
                outer,
                target,
                PythonCfgEdgeKind::Break,
            );
        }
        if let (Some(entry), Some(target)) = (
            routes.continue_,
            self.loops.last().map(|targets| targets.continue_),
        ) {
            self.run_finalbody(
                entry,
                &node.finalbody,
                outer,
                target,
                PythonCfgEdgeKind::Continue,
            );
        }
        vec![merge]
    }

    fn run_finalbody(
        &mut self,
        entry: NodeIndex,
        finalbody: &[Stmt],
        outer: BuildContext,
        target: NodeIndex,
        continuation: PythonCfgEdgeKind,
    ) {
        let tails = self.build_suite(finalbody, vec![entry], outer);
        for tail in tails {
            self.route_continuation_with_flags(
                tail,
                target,
                continuation,
                outer,
                EDGE_FLAG_FINALLY_RESUME,
            );
        }
    }

    fn build_with(
        &mut self,
        node: &ruff_python_ast::StmtWith,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        self.build_with_item(node, 0, inputs, context)
    }

    #[allow(clippy::too_many_lines)] // One recursive item owns all pending cleanup continuations.
    fn build_with_item(
        &mut self,
        node: &ruff_python_ast::StmtWith,
        item_index: usize,
        inputs: Vec<NodeIndex>,
        outer: BuildContext,
    ) -> Vec<NodeIndex> {
        let Some(item) = node.items.get(item_index) else {
            return self.build_suite(&node.body, inputs, outer);
        };
        let mut tails = inputs;
        tails = self.eval_expr(&item.context_expr, tails, outer);
        let enter = self.operation(
            &tails,
            PythonCfgNodeKind::StatementOperation,
            if node.is_async {
                "async-context-enter"
            } else {
                "context-enter"
            },
            item.range,
            outer,
            Some("context-manager"),
        );
        tails = if node.is_async {
            let resumed = self.suspend_resume(
                &[enter],
                item.range,
                "async-context-enter-suspend",
                "async-context-enter-resume",
            );
            for tail in &resumed {
                self.edge(
                    *tail,
                    outer.exception_target,
                    PythonCfgEdgeKind::Exception,
                    None,
                    None,
                    Some("context-manager"),
                    Self::exception_flags(outer, EDGE_FLAG_EXACT_EXCEPTION),
                );
            }
            resumed
        } else {
            vec![enter]
        };
        if let Some(target) = item.optional_vars.as_deref() {
            tails = self.eval_store(target, tails, outer);
        }

        let inside_loop = self.loops.last().is_some();
        let routes = FinalizerRoutes {
            normal: self.node(
                PythonCfgNodeKind::BasicBlock,
                "with-normal-cleanup",
                Some(item.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            return_: self.node(
                PythonCfgNodeKind::BasicBlock,
                "with-return-cleanup",
                Some(item.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            exception: self.node(
                PythonCfgNodeKind::BasicBlock,
                "with-exception-cleanup",
                Some(item.range),
                NODE_FLAG_FINALLY_ENTRY,
            ),
            break_: inside_loop.then(|| {
                self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "with-break-cleanup",
                    Some(item.range),
                    NODE_FLAG_FINALLY_ENTRY,
                )
            }),
            continue_: inside_loop.then(|| {
                self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "with-continue-cleanup",
                    Some(item.range),
                    NODE_FLAG_FINALLY_ENTRY,
                )
            }),
        };
        let protected = BuildContext {
            exception_target: routes.exception,
            finalizer: Some(routes),
        };
        let inner_tails = self.build_with_item(node, item_index + 1, tails, protected);
        for tail in inner_tails {
            self.edge(
                tail,
                routes.normal,
                PythonCfgEdgeKind::Cleanup,
                None,
                None,
                None,
                EDGE_FLAG_FINALLY_ROUTE,
            );
        }

        let merge = self.node(
            PythonCfgNodeKind::BasicBlock,
            "with-cleanup-merge",
            Some(item.range),
            NODE_FLAG_FINALLY_RESUME,
        );
        self.run_context_exit(
            routes.normal,
            item.range,
            node.is_async,
            outer,
            merge,
            PythonCfgEdgeKind::Next,
            None,
        );
        self.run_context_exit(
            routes.return_,
            item.range,
            node.is_async,
            outer,
            self.exit,
            PythonCfgEdgeKind::Return,
            None,
        );
        self.run_context_exit(
            routes.exception,
            item.range,
            node.is_async,
            outer,
            outer.exception_target,
            PythonCfgEdgeKind::Unwind,
            Some(merge),
        );
        if let (Some(entry), Some(target)) = (
            routes.break_,
            self.loops.last().map(|targets| targets.break_),
        ) {
            self.run_context_exit(
                entry,
                item.range,
                node.is_async,
                outer,
                target,
                PythonCfgEdgeKind::Break,
                None,
            );
        }
        if let (Some(entry), Some(target)) = (
            routes.continue_,
            self.loops.last().map(|targets| targets.continue_),
        ) {
            self.run_context_exit(
                entry,
                item.range,
                node.is_async,
                outer,
                target,
                PythonCfgEdgeKind::Continue,
                None,
            );
        }
        vec![merge]
    }

    #[allow(clippy::too_many_arguments)]
    fn run_context_exit(
        &mut self,
        entry: NodeIndex,
        range: TextRange,
        is_async: bool,
        outer: BuildContext,
        target: NodeIndex,
        continuation: PythonCfgEdgeKind,
        suppressed_target: Option<NodeIndex>,
    ) {
        let exit = self.node(
            PythonCfgNodeKind::StatementOperation,
            if is_async {
                "async-context-exit"
            } else {
                "context-exit"
            },
            Some(range),
            0,
        );
        self.edge(entry, exit, PythonCfgEdgeKind::Cleanup, None, None, None, 0);
        self.edge(
            exit,
            outer.exception_target,
            PythonCfgEdgeKind::Exception,
            None,
            None,
            Some("context-manager"),
            Self::exception_flags(outer, EDGE_FLAG_EXACT_EXCEPTION),
        );
        let tails = if is_async {
            let resumed = self.suspend_resume(
                &[exit],
                range,
                "async-context-exit-suspend",
                "async-context-exit-resume",
            );
            for tail in &resumed {
                self.edge(
                    *tail,
                    outer.exception_target,
                    PythonCfgEdgeKind::Exception,
                    None,
                    None,
                    Some("context-manager"),
                    Self::exception_flags(outer, EDGE_FLAG_EXACT_EXCEPTION),
                );
            }
            resumed
        } else {
            vec![exit]
        };
        if let Some(suppressed_target) = suppressed_target {
            let branch = self.node(
                PythonCfgNodeKind::Branch,
                "context-exit-suppression",
                Some(range),
                NODE_FLAG_FINALLY_RESUME,
            );
            self.connect(&tails, branch, PythonCfgEdgeKind::Next);
            self.edge(
                branch,
                target,
                PythonCfgEdgeKind::Unwind,
                Some(branch),
                None,
                Some("unsuppressed-context-exception"),
                Self::exception_flags(outer, EDGE_FLAG_FINALLY_RESUME),
            );
            self.edge(
                branch,
                suppressed_target,
                PythonCfgEdgeKind::True,
                Some(branch),
                None,
                None,
                EDGE_FLAG_FINALLY_RESUME,
            );
        } else {
            for tail in tails {
                self.route_continuation_with_flags(
                    tail,
                    target,
                    continuation,
                    outer,
                    EDGE_FLAG_FINALLY_RESUME,
                );
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Ordered pattern, guard, bind, and fallthrough remain visible together.
    fn build_match(
        &mut self,
        node: &ruff_python_ast::StmtMatch,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let tails = self.eval_expr(&node.subject, inputs, context);
        let switch = self.node(PythonCfgNodeKind::Switch, "match", Some(node.range), 0);
        self.connect(&tails, switch, PythonCfgEdgeKind::Next);
        let mut completed = Vec::new();
        let mut next_case = vec![switch];
        for (ordinal, case) in node.cases.iter().enumerate() {
            let pattern_entry = self.node(
                PythonCfgNodeKind::BasicBlock,
                "match-pattern-entry",
                Some(case.pattern.range()),
                NODE_FLAG_SYNTHETIC,
            );
            for source in next_case {
                self.edge(
                    source,
                    pattern_entry,
                    PythonCfgEdgeKind::Case,
                    Some(switch),
                    Some(ordinal.to_string()),
                    None,
                    0,
                );
            }
            let pattern_tails = self.eval_pattern(&case.pattern, vec![pattern_entry], context);
            let pattern = self.node(
                PythonCfgNodeKind::Branch,
                "match-pattern",
                Some(case.pattern.range()),
                0,
            );
            self.connect(&pattern_tails, pattern, PythonCfgEdgeKind::Next);
            let accepted = self.node(
                PythonCfgNodeKind::BasicBlock,
                "match-pattern-accepted",
                Some(case.pattern.range()),
                NODE_FLAG_SYNTHETIC,
            );
            self.edge(
                pattern,
                accepted,
                PythonCfgEdgeKind::True,
                Some(pattern),
                None,
                None,
                0,
            );
            let failed = self.node(
                PythonCfgNodeKind::BasicBlock,
                "match-next-case",
                Some(case.pattern.range()),
                NODE_FLAG_SYNTHETIC,
            );
            self.edge(
                pattern,
                failed,
                PythonCfgEdgeKind::False,
                Some(pattern),
                None,
                None,
                0,
            );
            let bound = self.operation(
                &[accepted],
                PythonCfgNodeKind::StatementOperation,
                "match-pattern-bind",
                case.pattern.range(),
                context,
                None,
            );
            let mut case_inputs = vec![bound];
            if let Some(guard) = case.guard.as_deref() {
                case_inputs = self.eval_expr(guard, case_inputs, context);
                let guard_branch = self.node(
                    PythonCfgNodeKind::Branch,
                    "match-guard",
                    Some(guard.range()),
                    0,
                );
                self.connect(&case_inputs, guard_branch, PythonCfgEdgeKind::Next);
                let accepted = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "match-case-body",
                    None,
                    NODE_FLAG_SYNTHETIC,
                );
                self.edge(
                    guard_branch,
                    accepted,
                    PythonCfgEdgeKind::True,
                    Some(guard_branch),
                    None,
                    None,
                    0,
                );
                self.edge(
                    guard_branch,
                    failed,
                    PythonCfgEdgeKind::False,
                    Some(guard_branch),
                    None,
                    None,
                    0,
                );
                case_inputs = vec![accepted];
            }
            completed.extend(self.build_suite(&case.body, case_inputs, context));
            next_case = vec![failed];
        }
        let no_match = self.node(
            PythonCfgNodeKind::BasicBlock,
            "match-no-case",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        self.connect(&next_case, no_match, PythonCfgEdgeKind::False);
        completed.push(no_match);
        completed
    }

    fn eval_pattern(
        &mut self,
        pattern: &Pattern,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        match pattern {
            Pattern::MatchValue(pattern) => self.eval_expr(&pattern.value, inputs, context),
            Pattern::MatchMapping(pattern) => {
                let mut tails = self.eval_exprs(pattern.keys.iter(), inputs, context);
                for nested in &pattern.patterns {
                    tails = self.eval_pattern(nested, tails, context);
                }
                tails
            }
            Pattern::MatchClass(pattern) => {
                let mut tails = self.eval_expr(&pattern.cls, inputs, context);
                for argument in pattern.arguments.iter_source_order() {
                    let nested = match argument {
                        PatternOrKeyword::Pattern(nested) => nested,
                        PatternOrKeyword::Keyword(keyword) => &keyword.pattern,
                    };
                    tails = self.eval_pattern(nested, tails, context);
                }
                tails
            }
            Pattern::MatchSequence(pattern) => {
                let mut tails = inputs;
                for nested in &pattern.patterns {
                    tails = self.eval_pattern(nested, tails, context);
                }
                tails
            }
            Pattern::MatchAs(pattern) => {
                pattern.pattern.as_deref().map_or(inputs.clone(), |nested| {
                    self.eval_pattern(nested, inputs, context)
                })
            }
            Pattern::MatchOr(pattern) => {
                let mut tails = inputs;
                for nested in &pattern.patterns {
                    tails = self.eval_pattern(nested, tails, context);
                }
                tails
            }
            Pattern::MatchSingleton(_) | Pattern::MatchStar(_) => inputs,
        }
    }

    fn build_assert(
        &mut self,
        node: &ruff_python_ast::StmtAssert,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let tails = self.eval_expr(&node.test, inputs, context);
        let branch = self.node(
            PythonCfgNodeKind::Branch,
            "assert",
            Some(node.test.range()),
            0,
        );
        self.connect(&tails, branch, PythonCfgEdgeKind::Next);
        let success = self.node(
            PythonCfgNodeKind::BasicBlock,
            "assert-success",
            None,
            NODE_FLAG_SYNTHETIC,
        );
        self.edge(
            branch,
            success,
            PythonCfgEdgeKind::True,
            Some(branch),
            None,
            None,
            0,
        );
        let mut failure = vec![branch];
        if let Some(message) = node.msg.as_deref() {
            failure = self.eval_expr(message, failure, context);
        }
        for tail in failure {
            self.edge(
                tail,
                context.exception_target,
                PythonCfgEdgeKind::Exception,
                Some(branch),
                None,
                Some("assertion"),
                Self::exception_flags(context, EDGE_FLAG_EXACT_EXCEPTION),
            );
        }
        vec![success]
    }

    fn eval_definition(
        &mut self,
        node: &StmtFunctionDef,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let mut tails = inputs;
        for decorator in &node.decorator_list {
            tails = self.eval_expr(&decorator.expression, tails, context);
        }
        if let Some(type_params) = node.type_params.as_deref() {
            tails = self.eval_type_params(type_params, tails, context);
        }
        tails = self.eval_parameter_expressions(&node.parameters, tails, context);
        if let Some(returns) = node.returns.as_deref() {
            tails = self.eval_expr(returns, tails, context);
        }
        let definition = self.operation(
            &tails,
            PythonCfgNodeKind::StatementOperation,
            if node.is_async {
                "async-function-create"
            } else {
                "function-create"
            },
            node.range,
            context,
            None,
        );
        let mut tails = vec![definition];
        for _ in node.decorator_list.iter().rev() {
            let application = self.node(
                PythonCfgNodeKind::ExpressionOperation,
                "decorator-apply",
                Some(node.range),
                0,
            );
            self.connect(&tails, application, PythonCfgEdgeKind::Next);
            tails = vec![application];
        }
        tails
    }

    fn eval_parameter_expressions(
        &mut self,
        parameters: &Parameters,
        mut tails: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        for parameter in parameters.iter_source_order() {
            let parameter = parameter.as_parameter();
            if let Some(annotation) = parameter.annotation.as_deref() {
                tails = self.eval_expr(annotation, tails, context);
            }
        }
        for parameter in parameters.iter_non_variadic_params() {
            if let Some(default) = parameter.default.as_deref() {
                tails = self.eval_expr(default, tails, context);
            }
        }
        tails
    }

    fn eval_type_params(
        &mut self,
        type_params: &TypeParams,
        mut tails: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        for type_param in type_params {
            match type_param {
                TypeParam::TypeVar(type_var) => {
                    if let Some(bound) = type_var.bound.as_deref() {
                        tails = self.eval_expr(bound, tails, context);
                    }
                    if let Some(default) = type_var.default.as_deref() {
                        tails = self.eval_expr(default, tails, context);
                    }
                }
                TypeParam::TypeVarTuple(type_var_tuple) => {
                    if let Some(default) = type_var_tuple.default.as_deref() {
                        tails = self.eval_expr(default, tails, context);
                    }
                }
                TypeParam::ParamSpec(param_spec) => {
                    if let Some(default) = param_spec.default.as_deref() {
                        tails = self.eval_expr(default, tails, context);
                    }
                }
            }
        }
        tails
    }

    #[allow(clippy::too_many_lines)]
    fn eval_expr(
        &mut self,
        expression: &Expr,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        match expression {
            Expr::BoolOp(node) => {
                let Some((first, rest)) = node.values.split_first() else {
                    return inputs;
                };
                let mut current = self.eval_expr(first, inputs, context);
                let merge = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "bool-merge",
                    Some(node.range),
                    NODE_FLAG_SYNTHETIC,
                );
                for value in rest {
                    let branch = self.node(
                        PythonCfgNodeKind::Branch,
                        "bool-short-circuit",
                        Some(value.range()),
                        0,
                    );
                    self.connect(&current, branch, PythonCfgEdgeKind::Next);
                    let evaluate = self.node(
                        PythonCfgNodeKind::BasicBlock,
                        "bool-next",
                        None,
                        NODE_FLAG_SYNTHETIC,
                    );
                    let (continue_kind, short_kind) = if node.op.to_string() == "and" {
                        (PythonCfgEdgeKind::True, PythonCfgEdgeKind::False)
                    } else {
                        (PythonCfgEdgeKind::False, PythonCfgEdgeKind::True)
                    };
                    self.edge(branch, evaluate, continue_kind, Some(branch), None, None, 0);
                    self.edge(branch, merge, short_kind, Some(branch), None, None, 0);
                    current = self.eval_expr(value, vec![evaluate], context);
                }
                self.connect(&current, merge, PythonCfgEdgeKind::Next);
                vec![merge]
            }
            Expr::If(node) => {
                let test = self.eval_expr(&node.test, inputs, context);
                let branch = self.node(
                    PythonCfgNodeKind::Branch,
                    "conditional-expression",
                    Some(node.test.range()),
                    0,
                );
                self.connect(&test, branch, PythonCfgEdgeKind::Next);
                let body_entry = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "conditional-true",
                    None,
                    NODE_FLAG_SYNTHETIC,
                );
                let else_entry = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "conditional-false",
                    None,
                    NODE_FLAG_SYNTHETIC,
                );
                self.edge(
                    branch,
                    body_entry,
                    PythonCfgEdgeKind::True,
                    Some(branch),
                    None,
                    None,
                    0,
                );
                self.edge(
                    branch,
                    else_entry,
                    PythonCfgEdgeKind::False,
                    Some(branch),
                    None,
                    None,
                    0,
                );
                let mut tails = self.eval_expr(&node.body, vec![body_entry], context);
                tails.extend(self.eval_expr(&node.orelse, vec![else_entry], context));
                let merge = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "conditional-merge",
                    Some(node.range),
                    NODE_FLAG_SYNTHETIC,
                );
                self.connect(&tails, merge, PythonCfgEdgeKind::Next);
                vec![merge]
            }
            Expr::Compare(node) => {
                let mut tails = self.eval_expr(&node.left, inputs, context);
                let merge = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "compare-merge",
                    Some(node.range),
                    NODE_FLAG_SYNTHETIC,
                );
                for comparator in &node.comparators {
                    tails = self.eval_expr(comparator, tails, context);
                    let compare = self.operation(
                        &tails,
                        PythonCfgNodeKind::Branch,
                        "compare",
                        comparator.range(),
                        context,
                        Some("comparison"),
                    );
                    let next = self.node(
                        PythonCfgNodeKind::BasicBlock,
                        "compare-next",
                        None,
                        NODE_FLAG_SYNTHETIC,
                    );
                    self.edge(
                        compare,
                        next,
                        PythonCfgEdgeKind::True,
                        Some(compare),
                        None,
                        None,
                        0,
                    );
                    self.edge(
                        compare,
                        merge,
                        PythonCfgEdgeKind::False,
                        Some(compare),
                        None,
                        None,
                        0,
                    );
                    tails = vec![next];
                }
                self.connect(&tails, merge, PythonCfgEdgeKind::Next);
                vec![merge]
            }
            Expr::Call(node) => {
                let mut tails = self.eval_expr(&node.func, inputs, context);
                for argument in &node.arguments.args {
                    tails = self.eval_expr(argument, tails, context);
                }
                for keyword in &node.arguments.keywords {
                    tails = self.eval_expr(&keyword.value, tails, context);
                }
                let call = self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "call",
                    node.range,
                    context,
                    Some("call"),
                );
                let returned = self.node(
                    PythonCfgNodeKind::BasicBlock,
                    "call-return",
                    Some(node.range),
                    NODE_FLAG_SYNTHETIC,
                );
                self.edge(
                    call,
                    returned,
                    PythonCfgEdgeKind::CallReturn,
                    None,
                    None,
                    None,
                    0,
                );
                vec![returned]
            }
            Expr::Attribute(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "attribute",
                    node.range,
                    context,
                    Some("attribute"),
                )]
            }
            Expr::Subscript(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                let tails = self.eval_expr(&node.slice, tails, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "subscript",
                    node.range,
                    context,
                    Some("subscript"),
                )]
            }
            Expr::Await(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                self.suspend_resume(&tails, node.range, "await-suspend", "await-resume")
            }
            Expr::Yield(node) => {
                let tails = if let Some(value) = node.value.as_deref() {
                    self.eval_expr(value, inputs, context)
                } else {
                    inputs
                };
                self.suspend_resume(&tails, node.range, "yield-suspend", "yield-resume")
            }
            Expr::YieldFrom(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                self.suspend_resume(
                    &tails,
                    node.range,
                    "yield-from-suspend",
                    "yield-from-resume",
                )
            }
            Expr::Named(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                self.eval_store(&node.target, tails, context)
            }
            Expr::BinOp(node) => {
                let tails = self.eval_expr(&node.left, inputs, context);
                let tails = self.eval_expr(&node.right, tails, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "binary-operation",
                    node.range,
                    context,
                    Some("operator"),
                )]
            }
            Expr::UnaryOp(node) => {
                let tails = self.eval_expr(&node.operand, inputs, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "unary-operation",
                    node.range,
                    context,
                    Some("operator"),
                )]
            }
            Expr::Lambda(node) => {
                let tails = if let Some(parameters) = node.parameters.as_deref() {
                    self.eval_parameter_expressions(parameters, inputs, context)
                } else {
                    inputs
                };
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "lambda-create",
                    node.range,
                    context,
                    None,
                )]
            }
            Expr::Dict(node) => {
                let mut tails = inputs;
                for item in &node.items {
                    if let Some(key) = item.key.as_ref() {
                        tails = self.eval_expr(key, tails, context);
                    }
                    tails = self.eval_expr(&item.value, tails, context);
                }
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "dict-display",
                    node.range,
                    context,
                    None,
                )]
            }
            Expr::Set(node) => {
                self.finish_collection("set-display", node.range, node.elts.iter(), inputs, context)
            }
            Expr::List(node) => self.finish_collection(
                "list-display",
                node.range,
                node.elts.iter(),
                inputs,
                context,
            ),
            Expr::Tuple(node) => self.finish_collection(
                "tuple-display",
                node.range,
                node.elts.iter(),
                inputs,
                context,
            ),
            Expr::ListComp(node) => self.eval_comprehension(
                &node.generators,
                Some(&node.elt),
                None,
                &inputs,
                context,
                node.range,
            ),
            Expr::SetComp(node) => self.eval_comprehension(
                &node.generators,
                Some(&node.elt),
                None,
                &inputs,
                context,
                node.range,
            ),
            Expr::Generator(node) => self.eval_comprehension(
                &node.generators,
                Some(&node.elt),
                None,
                &inputs,
                context,
                node.range,
            ),
            Expr::DictComp(node) => self.eval_comprehension(
                &node.generators,
                node.key.as_deref(),
                Some(&node.value),
                &inputs,
                context,
                node.range,
            ),
            Expr::Starred(node) => self.eval_expr(&node.value, inputs, context),
            Expr::Slice(node) => {
                let mut tails = inputs;
                if let Some(lower) = node.lower.as_deref() {
                    tails = self.eval_expr(lower, tails, context);
                }
                if let Some(upper) = node.upper.as_deref() {
                    tails = self.eval_expr(upper, tails, context);
                }
                if let Some(step) = node.step.as_deref() {
                    tails = self.eval_expr(step, tails, context);
                }
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "slice",
                    node.range,
                    context,
                    None,
                )]
            }
            Expr::FString(node) => {
                let tails = self.eval_interpolated_elements(node.value.elements(), inputs, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "f-string",
                    node.range,
                    context,
                    Some("interpolation"),
                )]
            }
            Expr::TString(node) => {
                let tails = self.eval_interpolated_elements(node.value.elements(), inputs, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "t-string",
                    node.range,
                    context,
                    Some("interpolation"),
                )]
            }
            Expr::Name(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "name",
                node.range,
                context,
                None,
            )],
            Expr::StringLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "string-literal",
                node.range,
                context,
                None,
            )],
            Expr::BytesLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "bytes-literal",
                node.range,
                context,
                None,
            )],
            Expr::NumberLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "number-literal",
                node.range,
                context,
                None,
            )],
            Expr::BooleanLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "boolean-literal",
                node.range,
                context,
                None,
            )],
            Expr::NoneLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "none-literal",
                node.range,
                context,
                None,
            )],
            Expr::EllipsisLiteral(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "ellipsis-literal",
                node.range,
                context,
                None,
            )],
            Expr::IpyEscapeCommand(node) => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::ExpressionOperation,
                "ipy-escape-expression",
                node.range,
                context,
                Some("ipy-escape"),
            )],
        }
    }

    fn eval_store(
        &mut self,
        expression: &Expr,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        match expression {
            Expr::Attribute(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "attribute-store",
                    node.range,
                    context,
                    Some("attribute"),
                )]
            }
            Expr::Subscript(node) => {
                let tails = self.eval_expr(&node.value, inputs, context);
                let tails = self.eval_expr(&node.slice, tails, context);
                vec![self.operation(
                    &tails,
                    PythonCfgNodeKind::ExpressionOperation,
                    "subscript-store",
                    node.range,
                    context,
                    Some("subscript"),
                )]
            }
            Expr::List(node) => self.eval_exprs(node.elts.iter(), inputs, context),
            Expr::Tuple(node) => self.eval_exprs(node.elts.iter(), inputs, context),
            Expr::Starred(node) => self.eval_store(&node.value, inputs, context),
            _ => vec![self.operation(
                &inputs,
                PythonCfgNodeKind::StatementOperation,
                "bind-target",
                expression.range(),
                context,
                None,
            )],
        }
    }

    fn eval_comprehension(
        &mut self,
        generators: &[Comprehension],
        first_result: Option<&Expr>,
        second_result: Option<&Expr>,
        tails: &[NodeIndex],
        context: BuildContext,
        range: TextRange,
    ) -> Vec<NodeIndex> {
        let create = self.operation(
            tails,
            PythonCfgNodeKind::ExpressionOperation,
            "comprehension-create",
            range,
            context,
            None,
        );
        let complete = self.node(
            PythonCfgNodeKind::BasicBlock,
            "comprehension-complete",
            Some(range),
            NODE_FLAG_SYNTHETIC,
        );
        if generators.is_empty() {
            self.edge(
                create,
                complete,
                PythonCfgEdgeKind::Next,
                None,
                None,
                None,
                0,
            );
        } else {
            self.build_comprehension_generator(
                generators,
                0,
                vec![create],
                complete,
                first_result,
                second_result,
                context,
            );
        }
        vec![complete]
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn build_comprehension_generator(
        &mut self,
        generators: &[Comprehension],
        index: usize,
        inputs: Vec<NodeIndex>,
        exhausted_target: NodeIndex,
        first_result: Option<&Expr>,
        second_result: Option<&Expr>,
        context: BuildContext,
    ) {
        let generator = &generators[index];
        let iter_tails = self.eval_expr(&generator.iter, inputs, context);
        let header = self.operation(
            &iter_tails,
            PythonCfgNodeKind::LoopHeader,
            if generator.is_async {
                "async-comprehension-next"
            } else {
                "comprehension-next"
            },
            generator.range,
            context,
            Some("iteration"),
        );
        let body_entry = self.node(
            PythonCfgNodeKind::BasicBlock,
            "comprehension-iteration",
            Some(generator.range),
            NODE_FLAG_SYNTHETIC,
        );
        self.edge(
            header,
            body_entry,
            PythonCfgEdgeKind::True,
            Some(header),
            None,
            None,
            0,
        );
        self.edge(
            header,
            exhausted_target,
            PythonCfgEdgeKind::False,
            Some(header),
            None,
            None,
            0,
        );
        let mut accepted = self.eval_store(&generator.target, vec![body_entry], context);
        for predicate in &generator.ifs {
            accepted = self.eval_expr(predicate, accepted, context);
            let filter = self.node(
                PythonCfgNodeKind::Branch,
                "comprehension-filter",
                Some(predicate.range()),
                0,
            );
            self.connect(&accepted, filter, PythonCfgEdgeKind::Next);
            let filter_true = self.node(
                PythonCfgNodeKind::BasicBlock,
                "comprehension-filter-true",
                None,
                NODE_FLAG_SYNTHETIC,
            );
            self.edge(
                filter,
                filter_true,
                PythonCfgEdgeKind::True,
                Some(filter),
                None,
                None,
                0,
            );
            self.edge(
                filter,
                header,
                PythonCfgEdgeKind::LoopBack,
                Some(filter),
                None,
                None,
                0,
            );
            accepted = vec![filter_true];
        }
        if index + 1 < generators.len() {
            self.build_comprehension_generator(
                generators,
                index + 1,
                accepted,
                header,
                first_result,
                second_result,
                context,
            );
        } else {
            let mut result_tails = accepted;
            if let Some(result) = first_result {
                result_tails = self.eval_expr(result, result_tails, context);
            }
            if let Some(result) = second_result {
                result_tails = self.eval_expr(result, result_tails, context);
            }
            let result = self.operation(
                &result_tails,
                PythonCfgNodeKind::ExpressionOperation,
                "comprehension-result",
                generator.range,
                context,
                None,
            );
            self.edge(
                result,
                header,
                PythonCfgEdgeKind::LoopBack,
                None,
                None,
                None,
                0,
            );
        }
    }

    fn eval_interpolated_elements<'b>(
        &mut self,
        elements: impl Iterator<Item = &'b InterpolatedStringElement>,
        mut tails: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        for element in elements {
            let InterpolatedStringElement::Interpolation(interpolation) = element else {
                continue;
            };
            tails = self.eval_expr(&interpolation.expression, tails, context);
            if let Some(format_spec) = interpolation.format_spec.as_deref() {
                tails =
                    self.eval_interpolated_elements(format_spec.elements.iter(), tails, context);
            }
        }
        tails
    }

    fn finish_collection<'b>(
        &mut self,
        label: &'static str,
        range: TextRange,
        expressions: impl Iterator<Item = &'b Expr>,
        inputs: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        let tails = self.eval_exprs(expressions, inputs, context);
        vec![self.operation(
            &tails,
            PythonCfgNodeKind::ExpressionOperation,
            label,
            range,
            context,
            None,
        )]
    }

    fn eval_exprs<'b>(
        &mut self,
        expressions: impl Iterator<Item = &'b Expr>,
        mut tails: Vec<NodeIndex>,
        context: BuildContext,
    ) -> Vec<NodeIndex> {
        for expression in expressions {
            tails = self.eval_expr(expression, tails, context);
        }
        tails
    }

    fn suspend_resume(
        &mut self,
        inputs: &[NodeIndex],
        range: TextRange,
        suspend_label: &'static str,
        resume_label: &'static str,
    ) -> Vec<NodeIndex> {
        let suspend = self.node(
            PythonCfgNodeKind::SuspendPoint,
            suspend_label,
            Some(range),
            0,
        );
        let resume = self.node(PythonCfgNodeKind::ResumePoint, resume_label, Some(range), 0);
        self.connect(inputs, suspend, PythonCfgEdgeKind::Suspend);
        self.edge(
            suspend,
            resume,
            PythonCfgEdgeKind::Resume,
            None,
            None,
            None,
            0,
        );
        vec![resume]
    }

    fn operation(
        &mut self,
        inputs: &[NodeIndex],
        kind: PythonCfgNodeKind,
        label: &'static str,
        range: TextRange,
        context: BuildContext,
        may_raise: Option<&'static str>,
    ) -> NodeIndex {
        let operation = self.node(kind, label, Some(range), 0);
        self.connect(inputs, operation, PythonCfgEdgeKind::Next);
        if let Some(category) = may_raise {
            self.edge(
                operation,
                context.exception_target,
                PythonCfgEdgeKind::Exception,
                None,
                None,
                Some(category),
                Self::exception_flags(context, EDGE_FLAG_EXACT_EXCEPTION),
            );
        }
        operation
    }

    fn route_continuation(
        &mut self,
        source: NodeIndex,
        target: NodeIndex,
        kind: PythonCfgEdgeKind,
        context: BuildContext,
    ) {
        self.route_continuation_with_flags(source, target, kind, context, 0);
    }

    fn route_continuation_with_flags(
        &mut self,
        source: NodeIndex,
        target: NodeIndex,
        kind: PythonCfgEdgeKind,
        context: BuildContext,
        flags: i64,
    ) {
        let routed = context.finalizer.and_then(|routes| match kind {
            PythonCfgEdgeKind::Return => Some(routes.return_),
            PythonCfgEdgeKind::Break => routes.break_,
            PythonCfgEdgeKind::Continue => routes.continue_,
            PythonCfgEdgeKind::Exception | PythonCfgEdgeKind::Unwind => Some(routes.exception),
            _ => None,
        });
        if let Some(finally_entry) = routed {
            self.edge(
                source,
                finally_entry,
                PythonCfgEdgeKind::Cleanup,
                None,
                None,
                None,
                flags | EDGE_FLAG_FINALLY_ROUTE,
            );
        } else {
            self.edge(source, target, kind, None, None, None, flags);
        }
    }

    fn exception_flags(context: BuildContext, flags: i64) -> i64 {
        flags
            | if context
                .finalizer
                .is_some_and(|routes| routes.exception == context.exception_target)
            {
                EDGE_FLAG_FINALLY_ROUTE
            } else {
                0
            }
    }

    fn node(
        &mut self,
        kind: PythonCfgNodeKind,
        label: &'static str,
        range: Option<TextRange>,
        flags: i64,
    ) -> NodeIndex {
        let ordinal = self.ordinal;
        self.ordinal = self.ordinal.saturating_add(1);
        let (start, end) = range.map_or((0, 0), |range| {
            (
                u64::from(u32::from(range.start())),
                u64::from(u32::from(range.end())),
            )
        });
        let id = semantic_id(
            self.fingerprint,
            "cfg-node",
            start,
            end,
            label,
            u8::try_from(ordinal.rem_euclid(256)).unwrap_or(0),
        );
        let index = self.graph.add_node(NodeDraft {
            id,
            kind,
            ordinal,
            label,
            range,
            flags,
        });
        assert!(
            self.node_by_id.insert(id, index).is_none(),
            "CFG identity collision"
        );
        index
    }

    fn connect(&mut self, sources: &[NodeIndex], target: NodeIndex, kind: PythonCfgEdgeKind) {
        for source in sources {
            self.edge(*source, target, kind, None, None, None, 0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn edge(
        &mut self,
        source: NodeIndex,
        target: NodeIndex,
        kind: PythonCfgEdgeKind,
        condition: Option<NodeIndex>,
        case_value: Option<String>,
        exception_category: Option<&'static str>,
        flags: i64,
    ) {
        let edge = EdgeDraft {
            kind,
            condition: condition.map(|index| self.graph[index].id),
            case_value,
            exception_category,
            flags,
        };
        if self.edge_keys.insert((source, target, edge.clone())) {
            self.graph.add_edge(source, target, edge);
        }
    }
}

fn edge_identity(
    fingerprint: &str,
    cfg_id: PythonSemanticId,
    source: PythonSemanticId,
    target: PythonSemanticId,
    edge: &EdgeDraft,
) -> PythonSemanticId {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-cfg-edge.v1");
    hasher.update(fingerprint.as_bytes());
    hasher.update(&cfg_id);
    hasher.update(&source);
    hasher.update(&target);
    hasher.update(edge.kind.registry_name().as_bytes());
    if let Some(condition) = edge.condition {
        hasher.update(&condition);
    }
    if let Some(case) = edge.case_value.as_deref() {
        hasher.update(case.as_bytes());
    }
    if let Some(category) = edge.exception_category {
        hasher.update(category.as_bytes());
    }
    hasher.update(&edge.flags.to_be_bytes());
    let mut id = [0_u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id
}

#[allow(clippy::too_many_lines)] // The in-memory validator is intentionally a single invariant census.
fn validate_graph(
    cfg_id: PythonSemanticId,
    graph: &DiGraph<NodeDraft, EdgeDraft>,
    entry: NodeIndex,
    exit: NodeIndex,
    exceptional_exit: NodeIndex,
) -> Result<(), PythonCfgValidationError> {
    let fail = |message: &str| PythonCfgValidationError {
        cfg_id,
        message: message.to_owned(),
    };
    if graph
        .node_weights()
        .filter(|node| node.kind == PythonCfgNodeKind::Entry)
        .count()
        != 1
    {
        return Err(fail("CFG must contain exactly one entry"));
    }
    if graph
        .node_weights()
        .filter(|node| node.kind == PythonCfgNodeKind::Exit)
        .count()
        != 1
    {
        return Err(fail("CFG must contain exactly one normal exit"));
    }
    if graph
        .node_weights()
        .filter(|node| node.kind == PythonCfgNodeKind::ExceptionalExit)
        .count()
        != 1
    {
        return Err(fail("CFG must contain exactly one exceptional exit"));
    }
    if !has_path_connecting(graph, entry, exit, None)
        && !has_path_connecting(graph, entry, exceptional_exit, None)
    {
        return Err(fail("entry reaches neither a normal nor exceptional exit"));
    }
    for index in graph.node_indices() {
        let node = &graph[index];
        let outgoing = graph
            .edges_directed(index, Direction::Outgoing)
            .collect::<Vec<_>>();
        if !node.kind.terminal() && outgoing.is_empty() {
            return Err(fail("nonterminal CFG node has no successor"));
        }
        if node.kind == PythonCfgNodeKind::ReturnPoint
            && outgoing
                .iter()
                .any(|edge| edge.weight().kind == PythonCfgEdgeKind::Next)
        {
            return Err(fail("return point has a fallthrough edge"));
        }
        for edge in outgoing {
            if edge.weight().kind == PythonCfgEdgeKind::Break
                && !matches!(
                    graph[edge.target()].label,
                    "while-break-exit" | "for-break-exit"
                )
            {
                return Err(fail("break edge targets a non-loop exit"));
            }
            if edge.weight().kind == PythonCfgEdgeKind::Continue
                && graph[edge.target()].kind != PythonCfgNodeKind::LoopHeader
            {
                return Err(fail("continue edge targets a non-loop header"));
            }
            let weight = edge.weight();
            if weight.flags & EDGE_FLAG_EXACT_EXCEPTION != 0
                && weight.kind != PythonCfgEdgeKind::Exception
            {
                return Err(fail("exact exceptional flow uses a normal edge kind"));
            }
            if weight.flags & EDGE_FLAG_SUMMARY_EXCEPTION != 0
                && !weight.kind.exceptional()
                && weight.kind != PythonCfgEdgeKind::Case
            {
                return Err(fail("summarized exceptional flow uses a normal edge kind"));
            }
            if weight.exception_category.is_some()
                && !weight.kind.exceptional()
                && weight.kind != PythonCfgEdgeKind::Case
            {
                return Err(fail("exception payload appears on a normal edge kind"));
            }
            if edge.weight().flags & EDGE_FLAG_FINALLY_ROUTE != 0 {
                if graph[edge.target()].flags & NODE_FLAG_FINALLY_ENTRY == 0 {
                    return Err(fail("finally route does not target a finally entry"));
                }
                let resumes = graph.node_indices().any(|candidate| {
                    graph[candidate].flags & NODE_FLAG_FINALLY_RESUME != 0
                        && has_path_connecting(graph, edge.target(), candidate, None)
                }) || graph.edge_references().any(|candidate| {
                    candidate.weight().flags & EDGE_FLAG_FINALLY_RESUME != 0
                        && has_path_connecting(graph, edge.target(), candidate.source(), None)
                });
                if !resumes {
                    return Err(fail("finally route swallows its pending continuation"));
                }
                if let Some(expected) = pending_continuation(graph[edge.target()].label)
                    && !graph.edge_references().any(|candidate| {
                        candidate.weight().flags & EDGE_FLAG_FINALLY_RESUME != 0
                            && candidate.weight().kind == expected
                            && has_path_connecting(graph, edge.target(), candidate.source(), None)
                    })
                {
                    return Err(fail("finally route resumes the wrong pending continuation"));
                }
            }
        }
    }
    Ok(())
}

/// Validate persisted application facts without reconstructing provider state.
///
/// # Errors
///
/// Rejects dangling node/edge keys, malformed entry/exit cardinality, return
/// fallthrough, loop-control misrouting, or incomplete finally continuations.
#[allow(clippy::too_many_lines)] // Persisted parity checks mirror the in-memory invariant census.
pub fn validate_python_cfg(
    cfg: &PythonCfgFact,
    nodes: &[PythonCfgNodeFact],
    edges: &[PythonCfgEdgeFact],
) -> Result<(), PythonCfgValidationError> {
    let fail = |message: &str| PythonCfgValidationError {
        cfg_id: cfg.cfg_id,
        message: message.to_owned(),
    };
    let own_nodes = nodes
        .iter()
        .filter(|node| node.cfg_id == cfg.cfg_id)
        .collect::<Vec<_>>();
    let own_edges = edges
        .iter()
        .filter(|edge| edge.cfg_id == cfg.cfg_id)
        .collect::<Vec<_>>();
    if i32::try_from(own_nodes.len()).unwrap_or(i32::MAX) != cfg.node_count
        || i32::try_from(own_edges.len()).unwrap_or(i32::MAX) != cfg.edge_count
    {
        return Err(fail("CFG header counts do not match detail rows"));
    }
    let by_id = own_nodes
        .iter()
        .map(|node| (node.cfg_node_id, *node))
        .collect::<BTreeMap<_, _>>();
    if own_nodes
        .iter()
        .filter(|node| node.kind == PythonCfgNodeKind::Entry)
        .count()
        != 1
        || own_nodes
            .iter()
            .filter(|node| node.kind == PythonCfgNodeKind::Exit)
            .count()
            != 1
        || own_nodes
            .iter()
            .filter(|node| node.kind == PythonCfgNodeKind::ExceptionalExit)
            .count()
            != 1
    {
        return Err(fail("CFG detail rows have invalid entry/exit cardinality"));
    }
    if by_id.get(&cfg.entry_node_id).map(|node| node.kind) != Some(PythonCfgNodeKind::Entry)
        || by_id.get(&cfg.exit_node_id).map(|node| node.kind) != Some(PythonCfgNodeKind::Exit)
        || by_id
            .get(&cfg.exceptional_exit_node_id)
            .map(|node| node.kind)
            != Some(PythonCfgNodeKind::ExceptionalExit)
    {
        return Err(fail("CFG header entry/exit identities are invalid"));
    }

    let mut graph = DiGraph::<&PythonCfgNodeFact, &PythonCfgEdgeFact>::new();
    let graph_nodes = own_nodes
        .iter()
        .map(|node| (node.cfg_node_id, graph.add_node(*node)))
        .collect::<BTreeMap<_, _>>();
    for edge in &own_edges {
        if !by_id.contains_key(&edge.source_node_id) || !by_id.contains_key(&edge.target_node_id) {
            return Err(fail("CFG edge references an absent node"));
        }
        if edge.owner_id != cfg.owner_id {
            return Err(fail("CFG edge owner differs from graph owner"));
        }
        if edge
            .condition_node_id
            .is_some_and(|condition| !by_id.contains_key(&condition))
        {
            return Err(fail("CFG edge condition references an absent node"));
        }
        if by_id[&edge.source_node_id].kind == PythonCfgNodeKind::ReturnPoint
            && edge.kind == PythonCfgEdgeKind::Next
        {
            return Err(fail("return point has a fallthrough edge"));
        }
        if edge.flags & EDGE_FLAG_FINALLY_ROUTE != 0
            && by_id[&edge.target_node_id].flags & NODE_FLAG_FINALLY_ENTRY == 0
        {
            return Err(fail("finally route does not target a finally entry"));
        }
        if edge.flags & EDGE_FLAG_EXACT_EXCEPTION != 0 && edge.kind != PythonCfgEdgeKind::Exception
        {
            return Err(fail("exact exceptional flow uses a normal edge kind"));
        }
        if edge.flags & EDGE_FLAG_SUMMARY_EXCEPTION != 0
            && !edge.kind.exceptional()
            && edge.kind != PythonCfgEdgeKind::Case
        {
            return Err(fail("summarized exceptional flow uses a normal edge kind"));
        }
        if edge.exception_category.is_some()
            && !edge.kind.exceptional()
            && edge.kind != PythonCfgEdgeKind::Case
        {
            return Err(fail("exception payload appears on a normal edge kind"));
        }
        graph.add_edge(
            graph_nodes[&edge.source_node_id],
            graph_nodes[&edge.target_node_id],
            *edge,
        );
    }

    let entry = graph_nodes[&cfg.entry_node_id];
    let exit = graph_nodes[&cfg.exit_node_id];
    let exceptional_exit = graph_nodes[&cfg.exceptional_exit_node_id];
    if !has_path_connecting(&graph, entry, exit, None)
        && !has_path_connecting(&graph, entry, exceptional_exit, None)
    {
        return Err(fail("persisted CFG entry reaches no terminal exit"));
    }
    for node in &own_nodes {
        if node.owner_id != cfg.owner_id {
            return Err(fail("CFG node owner differs from graph owner"));
        }
        if !node.kind.terminal()
            && !edges
                .iter()
                .any(|edge| edge.cfg_id == cfg.cfg_id && edge.source_node_id == node.cfg_node_id)
        {
            return Err(fail("nonterminal CFG node has no successor"));
        }
    }
    for edge in &own_edges {
        let target = by_id[&edge.target_node_id];
        if edge.kind == PythonCfgEdgeKind::Break
            && !matches!(target.label, "while-break-exit" | "for-break-exit")
        {
            return Err(fail("break edge targets a non-loop exit"));
        }
        if edge.kind == PythonCfgEdgeKind::Continue && target.kind != PythonCfgNodeKind::LoopHeader
        {
            return Err(fail("continue edge targets a non-loop header"));
        }
        if edge.flags & EDGE_FLAG_FINALLY_ROUTE != 0 {
            let route_target = graph_nodes[&edge.target_node_id];
            let reaches_resume_node = graph.node_indices().any(|candidate| {
                graph[candidate].flags & NODE_FLAG_FINALLY_RESUME != 0
                    && has_path_connecting(&graph, route_target, candidate, None)
            });
            let reaches_resume_edge = graph.edge_references().any(|candidate| {
                candidate.weight().flags & EDGE_FLAG_FINALLY_RESUME != 0
                    && has_path_connecting(&graph, route_target, candidate.source(), None)
            });
            if !reaches_resume_node && !reaches_resume_edge {
                return Err(fail("finally route swallows its pending continuation"));
            }
            if let Some(expected) = pending_continuation(target.label)
                && !graph.edge_references().any(|candidate| {
                    candidate.weight().flags & EDGE_FLAG_FINALLY_RESUME != 0
                        && candidate.weight().kind == expected
                        && has_path_connecting(&graph, route_target, candidate.source(), None)
                })
            {
                return Err(fail("finally route resumes the wrong pending continuation"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ruff_python_ast::{PySourceType, PythonVersion};
    use ruff_python_parser::{ParseOptions, parse_unchecked};

    use super::*;

    fn analyze(source: &str) -> super::super::semantic::PythonFrontendBatch {
        let parsed = parse_unchecked(
            source,
            ParseOptions::from(PySourceType::Python).with_target_version(PythonVersion::PY314),
        )
        .try_into_module()
        .expect("module parse");
        super::super::semantic::project_python_semantics(
            source,
            parsed.syntax().body.as_slice(),
            "fixture",
            std::path::Path::new("fixture.py"),
            "b3:cfg-fixture",
            false,
        )
        .expect("semantic batch")
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One functional fixture covers the GEN 24 construct matrix.
    fn py_cfg_fixture_conformance() {
        let batch = analyze(
            r#"
async def workflow(items, manager):
    total = 0
    try:
        async with manager() as resource, manager() as secondary:
            for item in items:
                if item < 0:
                    continue
                match item:
                    case 0:
                        break
                    case _ if item > 10:
                        total += await resource(item)
                assert total >= 0
                message = f"{resource(item)}:{secondary}"
        return total
    except* ValueError as errors:
        raise errors
    finally:
        total = total + 1

values = [f(x) for x in source() if predicate(x)]
"#,
        );
        assert!(batch.cfgs.len() >= 2);
        assert!(
            batch
                .cfg_nodes
                .iter()
                .any(|node| node.kind == PythonCfgNodeKind::SuspendPoint)
        );
        assert!(
            batch
                .cfg_nodes
                .iter()
                .any(|node| node.kind == PythonCfgNodeKind::Switch)
        );
        assert!(
            batch
                .cfg_edges
                .iter()
                .any(|edge| edge.kind == PythonCfgEdgeKind::Exception)
        );
        assert!(
            batch
                .cfg_edges
                .iter()
                .any(|edge| edge.flags & EDGE_FLAG_FINALLY_ROUTE != 0)
        );
        assert!(
            batch
                .cfg_edges
                .iter()
                .any(|edge| edge.kind == PythonCfgEdgeKind::LoopBack)
        );
        assert!(
            batch
                .cfg_edges
                .iter()
                .any(|edge| edge.kind == PythonCfgEdgeKind::CallReturn)
        );
        assert!(
            batch
                .cfg_nodes
                .iter()
                .filter(|node| node.label == "async-context-exit")
                .count()
                >= 4
        );
        assert!(batch.cfg_nodes.iter().any(|node| node.label == "f-string"));
        assert!(
            batch
                .cfg_edges
                .iter()
                .all(|edge| edge.kind.registry_name() != "CFG_NORMAL")
        );
        let comprehension_headers = batch
            .cfg_nodes
            .iter()
            .filter(|node| node.label == "comprehension-next")
            .map(|node| node.cfg_node_id)
            .collect::<BTreeSet<_>>();
        assert!(!comprehension_headers.is_empty());
        for header in comprehension_headers {
            let outgoing = batch
                .cfg_edges
                .iter()
                .filter(|edge| edge.source_node_id == header)
                .map(|edge| edge.kind)
                .collect::<BTreeSet<_>>();
            assert!(outgoing.contains(&PythonCfgEdgeKind::True));
            assert!(outgoing.contains(&PythonCfgEdgeKind::False));
            assert!(batch.cfg_edges.iter().any(|edge| {
                edge.target_node_id == header && edge.kind == PythonCfgEdgeKind::LoopBack
            }));
        }
        #[cfg(feature = "daemon")]
        {
            let projection = crate::python_semantic::project_ruff_semantic_batch(
                crate::fact_ingest::FactScope {
                    workspace_id: [1; 16],
                    analysis_context_id: [2; 16],
                    source_generation: 7,
                    owner_id: [3; 16],
                },
                [4; 16],
                &batch,
            )
            .expect("canonical CFG projection");
            assert_eq!(
                projection.batch(280).expect("cfg_graph").batch().num_rows(),
                batch.cfgs.len()
            );
            assert_eq!(
                projection
                    .batch(290)
                    .expect("cfg_node_detail")
                    .batch()
                    .num_rows(),
                batch.cfg_nodes.len()
            );
            assert_eq!(
                projection
                    .batch(300)
                    .expect("cfg_edge_detail")
                    .batch()
                    .num_rows(),
                batch.cfg_edges.len()
            );
            let relation_batch = projection.batch(110).expect("relation").batch();
            let relation_codes = relation_batch
                .column_by_name("relation_kind_code")
                .expect("relation kind column")
                .as_any()
                .downcast_ref::<arrow::array::Int32Array>()
                .expect("int32 relation kinds");
            assert!(relation_codes.values().contains(&480));
            assert!(relation_codes.values().iter().all(|code| *code != 70));
        }
    }

    #[test]
    fn py_cfg_wellformedness_parity() {
        let batch = analyze("def f(x):\n    return (x and g(x)) if x else 0\n");
        for cfg in &batch.cfgs {
            validate_python_cfg(cfg, &batch.cfg_nodes, &batch.cfg_edges).expect("valid CFG facts");
        }
        assert!(batch.cfg_nodes.iter().all(|node| node.ordinal >= 0));
        #[cfg(feature = "daemon")]
        {
            let projection = crate::python_semantic::project_ruff_semantic_batch(
                crate::fact_ingest::FactScope {
                    workspace_id: [5; 16],
                    analysis_context_id: [6; 16],
                    source_generation: 8,
                    owner_id: [7; 16],
                },
                [8; 16],
                &batch,
            )
            .expect("canonical CFG projection");
            for table_code in [280, 290, 300] {
                assert!(
                    projection
                        .batch(table_code)
                        .expect("CFG table")
                        .batch()
                        .schema()
                        .fields()
                        .iter()
                        .all(|field| !field.name().contains("node_index"))
                );
            }
        }
    }

    #[test]
    fn py_cfg_exceptional_edge_falsification() {
        let batch = analyze(
            "def f(x):\n    try:\n        return x.attr[call()]\n    finally:\n        cleanup()\n",
        );
        let cfg = batch
            .cfgs
            .iter()
            .find(|cfg| cfg.kind == PythonCfgKind::Function)
            .expect("function CFG");
        let mut edges = batch.cfg_edges.clone();
        let route = edges
            .iter_mut()
            .find(|edge| edge.cfg_id == cfg.cfg_id && edge.flags & EDGE_FLAG_FINALLY_ROUTE != 0)
            .expect("finally route");
        route.target_node_id = cfg.exit_node_id;
        assert!(validate_python_cfg(cfg, &batch.cfg_nodes, &edges).is_err());
        let mut aliased_exception = batch.cfg_edges.clone();
        let exceptional = aliased_exception
            .iter_mut()
            .find(|edge| {
                edge.cfg_id == cfg.cfg_id
                    && edge.kind == PythonCfgEdgeKind::Exception
                    && edge.exception_category == Some("call")
            })
            .expect("exact call exception");
        exceptional.kind = PythonCfgEdgeKind::Next;
        assert!(validate_python_cfg(cfg, &batch.cfg_nodes, &aliased_exception).is_err());
        let mut aliased_unwind = batch.cfg_edges.clone();
        let mut rewritten = 0;
        for unwind in aliased_unwind.iter_mut().filter(|edge| {
            edge.cfg_id == cfg.cfg_id
                && edge.kind == PythonCfgEdgeKind::Unwind
                && edge.flags & EDGE_FLAG_FINALLY_RESUME != 0
        }) {
            unwind.kind = PythonCfgEdgeKind::Next;
            rewritten += 1;
        }
        assert!(rewritten > 0, "finally unwind continuation");
        assert!(validate_python_cfg(cfg, &batch.cfg_nodes, &aliased_unwind).is_err());
        assert!(
            batch
                .cfg_edges
                .iter()
                .filter(|edge| edge.cfg_id == cfg.cfg_id)
                .any(|edge| edge.kind == PythonCfgEdgeKind::Exception
                    && edge.exception_category == Some("call"))
        );
    }

    #[test]
    fn py_cfg_owner_invalidation_gate() {
        let before = analyze("def left(x):\n    return x + 1\n\ndef right(y):\n    return y * 2\n");
        let after = analyze("def left(x):\n    return g(x) \n\ndef right(y):\n    return y * 2\n");
        let cfg_for = |batch: &super::super::semantic::PythonFrontendBatch, name: &str| {
            let callable = batch
                .callables
                .iter()
                .find(|callable| callable.name == name)
                .expect("callable");
            batch
                .cfgs
                .iter()
                .find(|cfg| cfg.callable_id == callable.callable_id)
                .expect("CFG")
                .cfg_id
        };
        let signature = |batch: &super::super::semantic::PythonFrontendBatch,
                         cfg_id: PythonSemanticId| {
            let nodes = batch
                .cfg_nodes
                .iter()
                .filter(|node| node.cfg_id == cfg_id)
                .map(|node| (node.kind, node.label, node.start_byte, node.end_byte))
                .collect::<Vec<_>>();
            let edges = batch
                .cfg_edges
                .iter()
                .filter(|edge| edge.cfg_id == cfg_id)
                .map(|edge| (edge.kind, edge.exception_category, edge.flags))
                .collect::<Vec<_>>();
            (nodes, edges)
        };
        let before_right = cfg_for(&before, "right");
        let after_right = cfg_for(&after, "right");
        assert_eq!(
            signature(&before, before_right),
            signature(&after, after_right)
        );
        let before_left = cfg_for(&before, "left");
        let after_left = cfg_for(&after, "left");
        assert_ne!(
            signature(&before, before_left),
            signature(&after, after_left)
        );
    }
}
