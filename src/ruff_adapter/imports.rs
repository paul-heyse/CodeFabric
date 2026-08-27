//! Ruff AST import, export, and re-export projection.
//!
//! Resolution is intentionally static. This adapter never imports or executes the
//! analyzed module; syntax that requires execution produces explicit incompleteness.

use std::collections::{BTreeMap, BTreeSet};

use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{Expr, Operator, Stmt};
use ruff_text_size::Ranged;

use super::semantic::{
    PythonBindingFact, PythonExportFact, PythonExportStatus, PythonImportFact, PythonImportKind,
    PythonResolution, PythonScopeFact, PythonScopeKind, PythonSemanticId, PythonTargetForm,
    semantic_id,
};

pub(super) struct PythonImportProjection {
    pub module_id: PythonSemanticId,
    pub imports: Vec<PythonImportFact>,
    pub exports: Vec<PythonExportFact>,
    pub export_status: PythonExportStatus,
}

pub(super) fn project_python_imports(
    suite: &[Stmt],
    module_name: &str,
    fingerprint: &str,
    scopes: &[PythonScopeFact],
    bindings: &[PythonBindingFact],
    ruff_qualified_names: &BTreeMap<PythonSemanticId, String>,
) -> PythonImportProjection {
    let module_id = semantic_id(fingerprint, "module", 0, 0, module_name, 0);
    let mut pass = ImportPass {
        module_name,
        fingerprint,
        scopes,
        bindings,
        ruff_qualified_names,
        imports: Vec::new(),
    };
    visitor::walk_body(&mut pass, suite);
    pass.imports
        .sort_by_key(|fact| (fact.start_byte, fact.end_byte, fact.source_name.clone()));

    let (explicit_exports, mut export_status) = evaluate_module_all(suite);
    if explicit_exports.is_none() && pass.imports.iter().any(|fact| fact.star_import) {
        export_status = PythonExportStatus::IncompleteDynamic;
    }
    let module_scope = scopes
        .iter()
        .find(|scope| scope.kind == PythonScopeKind::Module)
        .map(|scope| scope.scope_id);
    let import_by_binding = pass
        .imports
        .iter()
        .filter_map(|fact| fact.local_binding_id.map(|binding| (binding, fact)))
        .collect::<BTreeMap<_, _>>();
    let export_names = explicit_exports.unwrap_or_else(|| {
        bindings
            .iter()
            .filter(|binding| {
                Some(binding.scope_id) == module_scope && !binding.name.starts_with('_')
            })
            .map(|binding| binding.name.clone())
            .collect()
    });
    let mut exports = Vec::new();
    for name in export_names {
        let Some(binding) = bindings
            .iter()
            .find(|binding| Some(binding.scope_id) == module_scope && binding.name == name)
        else {
            export_status = PythonExportStatus::IncompleteDynamic;
            continue;
        };
        let import = import_by_binding.get(&binding.binding_id).copied();
        let target_id = import
            .and_then(|fact| fact.imported_entity_id)
            .unwrap_or(binding.binding_id);
        exports.push(PythonExportFact {
            export_id: semantic_id(
                fingerprint,
                if import.is_some() {
                    "reexport"
                } else {
                    "export"
                },
                binding.start_byte,
                binding.end_byte,
                &name,
                0,
            ),
            name,
            target_id,
            reexport: import.is_some(),
            start_byte: binding.start_byte,
            end_byte: binding.end_byte,
        });
    }
    exports.sort_by_key(|fact| (fact.name.clone(), fact.export_id));
    exports.dedup_by_key(|fact| fact.name.clone());

    PythonImportProjection {
        module_id,
        imports: pass.imports,
        exports,
        export_status,
    }
}

struct ImportPass<'a> {
    module_name: &'a str,
    fingerprint: &'a str,
    scopes: &'a [PythonScopeFact],
    bindings: &'a [PythonBindingFact],
    ruff_qualified_names: &'a BTreeMap<PythonSemanticId, String>,
    imports: Vec<PythonImportFact>,
}

impl ImportPass<'_> {
    fn scope_at(&self, start: u64, end: u64) -> PythonSemanticId {
        self.scopes
            .iter()
            .filter(|scope| scope.start_byte <= start && end <= scope.end_byte)
            .min_by_key(|scope| scope.end_byte.saturating_sub(scope.start_byte))
            .or_else(|| self.scopes.first())
            .expect("semantic projection always creates a module scope")
            .scope_id
    }

    fn binding(&self, scope_id: PythonSemanticId, local_name: &str) -> Option<PythonSemanticId> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.scope_id == scope_id
                    && binding.name == local_name
                    && binding.target_form == PythonTargetForm::ImportAlias
            })
            .min_by_key(|binding| binding.start_byte)
            .map(|binding| binding.binding_id)
    }

    fn resolve_module(&self, module_text: &str, level: u32) -> Option<String> {
        if level == 0 {
            return (!module_text.is_empty()).then(|| module_text.to_owned());
        }
        let mut parts = self.module_name.split('.').collect::<Vec<_>>();
        parts.pop();
        let remove = usize::try_from(level.saturating_sub(1)).ok()?;
        if remove > parts.len() {
            return None;
        }
        parts.truncate(parts.len() - remove);
        if !module_text.is_empty() {
            parts.extend(module_text.split('.'));
        }
        (!parts.is_empty()).then(|| parts.join("."))
    }

    #[allow(clippy::too_many_arguments)] // The arguments are the governed FAB module_import_detail fields.
    fn push_import(
        &mut self,
        kind: PythonImportKind,
        scope_id: PythonSemanticId,
        module_text: &str,
        imported_name: Option<&str>,
        alias_name: Option<&str>,
        level: u32,
        star_import: bool,
        start: u64,
        end: u64,
    ) {
        let source_resolved_module = self.resolve_module(module_text, level);
        let local_name = if star_import {
            None
        } else if let Some(alias) = alias_name {
            Some(alias)
        } else if let Some(name) = imported_name {
            Some(name)
        } else {
            module_text.split('.').next()
        };
        let local_binding_id = local_name.and_then(|name| self.binding(scope_id, name));
        let ruff_qualified_name = local_binding_id
            .and_then(|binding| self.ruff_qualified_names.get(&binding))
            .cloned();
        let ruff_module_name = ruff_qualified_name.as_ref().and_then(|qualified| {
            if imported_name.is_some_and(|name| name != "*") {
                qualified
                    .rsplit_once('.')
                    .map(|(module, _)| module.to_owned())
            } else {
                Some(qualified.clone())
            }
        });
        let resolved_module = ruff_module_name.or(source_resolved_module);
        let resolution = if ruff_qualified_name.is_some() {
            PythonResolution::Resolved
        } else if resolved_module.is_some() {
            PythonResolution::MayReferTo
        } else {
            PythonResolution::UnknownSymbol
        };
        let unknown_reason = match resolution {
            PythonResolution::Resolved => None,
            PythonResolution::MayReferTo if kind == PythonImportKind::Star => {
                Some("STAR_IMPORT_TARGET_SOURCE_DECLARED".to_owned())
            }
            PythonResolution::MayReferTo if kind == PythonImportKind::Dynamic => {
                Some("DYNAMIC_IMPORT_NOT_EXECUTED".to_owned())
            }
            PythonResolution::MayReferTo => Some("RUFF_QUALIFIED_NAME_UNAVAILABLE".to_owned()),
            PythonResolution::UnknownSymbol | PythonResolution::UnboundLocal => {
                Some("UNKNOWN_MODULE".to_owned())
            }
        };
        let target_name = resolved_module.as_deref().unwrap_or(module_text);
        let target_module_id = semantic_id(
            self.fingerprint,
            if resolved_module.is_some() {
                "target-module"
            } else {
                "unknown-module"
            },
            0,
            0,
            target_name,
            u8::try_from(level).unwrap_or(u8::MAX),
        );
        let imported_entity_id = imported_name.filter(|name| *name != "*").map(|name| {
            semantic_id(
                self.fingerprint,
                "imported-symbol",
                0,
                0,
                &format!("{target_name}.{name}"),
                0,
            )
        });
        let source_name = imported_name.map_or_else(
            || module_text.to_owned(),
            |name| format!("{module_text}:{name}"),
        );
        self.imports.push(PythonImportFact {
            import_id: semantic_id(
                self.fingerprint,
                "import-declaration",
                start,
                end,
                &source_name,
                kind as u8,
            ),
            scope_id,
            kind,
            relative_level: (level > 0).then(|| i16::try_from(level).unwrap_or(i16::MAX)),
            source_name,
            alias_name: alias_name.map(str::to_owned),
            star_import,
            target_module_id,
            target_module_name: resolved_module,
            ruff_qualified_name,
            resolution,
            imported_entity_id,
            imported_name: imported_name.map(str::to_owned),
            local_binding_id,
            unknown_reason_code: unknown_reason,
            start_byte: start,
            end_byte: end,
        });
    }
}

impl<'a> Visitor<'a> for ImportPass<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Import(node) => {
                let start = u64::from(u32::from(node.start()));
                let end = u64::from(u32::from(node.end()));
                let scope = self.scope_at(start, end);
                for alias in &node.names {
                    self.push_import(
                        PythonImportKind::Module,
                        scope,
                        alias.name.as_str(),
                        None,
                        alias
                            .asname
                            .as_ref()
                            .map(ruff_python_ast::Identifier::as_str),
                        0,
                        false,
                        u64::from(u32::from(alias.start())),
                        u64::from(u32::from(alias.end())),
                    );
                }
            }
            Stmt::ImportFrom(node) => {
                let start = u64::from(u32::from(node.start()));
                let end = u64::from(u32::from(node.end()));
                let scope = self.scope_at(start, end);
                let module = node
                    .module
                    .as_ref()
                    .map_or("", ruff_python_ast::Identifier::as_str);
                for alias in &node.names {
                    let star = alias.name.as_str() == "*";
                    self.push_import(
                        if star {
                            PythonImportKind::Star
                        } else {
                            PythonImportKind::FromName
                        },
                        scope,
                        module,
                        Some(alias.name.as_str()),
                        alias
                            .asname
                            .as_ref()
                            .map(ruff_python_ast::Identifier::as_str),
                        node.level,
                        star,
                        u64::from(u32::from(alias.start())),
                        u64::from(u32::from(alias.end())),
                    );
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            let dynamic_name = match call.func.as_ref() {
                Expr::Name(name) if name.id.as_str() == "__import__" => Some("__import__"),
                Expr::Attribute(attribute)
                    if attribute.attr.as_str() == "import_module"
                        && matches!(attribute.value.as_ref(), Expr::Name(name) if name.id.as_str() == "importlib") =>
                {
                    Some("importlib.import_module")
                }
                _ => None,
            };
            if let Some(dynamic_name) = dynamic_name {
                let start = u64::from(u32::from(call.start()));
                let end = u64::from(u32::from(call.end()));
                let scope = self.scope_at(start, end);
                let literal_module = call
                    .arguments
                    .find_positional(0)
                    .and_then(Expr::as_string_literal_expr)
                    .map(|literal| literal.value.to_str().to_owned());
                self.push_import(
                    PythonImportKind::Dynamic,
                    scope,
                    literal_module.as_deref().unwrap_or(dynamic_name),
                    None,
                    None,
                    0,
                    false,
                    start,
                    end,
                );
                if literal_module.is_none()
                    && let Some(last) = self.imports.last_mut()
                {
                    last.target_module_name = None;
                    last.ruff_qualified_name = None;
                    last.resolution = PythonResolution::UnknownSymbol;
                    last.target_module_id =
                        semantic_id(self.fingerprint, "unknown-module", 0, 0, dynamic_name, 0);
                    last.unknown_reason_code = Some("DYNAMIC_IMPORT_TARGET".into());
                }
            }
        }
        visitor::walk_expr(self, expr);
    }
}

fn evaluate_module_all(suite: &[Stmt]) -> (Option<Vec<String>>, PythonExportStatus) {
    let mut exports: Option<Vec<String>> = None;
    let mut dynamic = false;
    for statement in suite {
        match statement {
            Stmt::Assign(node)
                if node.targets.iter().any(
                    |target| matches!(target, Expr::Name(name) if name.id.as_str() == "__all__"),
                ) =>
            {
                match literal_strings(&node.value) {
                    Some(values) => exports = Some(values),
                    None => dynamic = true,
                }
            }
            Stmt::AugAssign(node) if matches!(node.target.as_ref(), Expr::Name(name) if name.id.as_str() == "__all__") => {
                if node.op == Operator::Add {
                    match literal_strings(&node.value) {
                        Some(values) => exports.get_or_insert_with(Vec::new).extend(values),
                        None => dynamic = true,
                    }
                } else {
                    dynamic = true;
                }
            }
            _ => {}
        }
    }
    if let Some(values) = &mut exports {
        let mut seen = BTreeSet::new();
        values.retain(|value| seen.insert(value.clone()));
    }
    (
        exports,
        if dynamic {
            PythonExportStatus::IncompleteDynamic
        } else {
            PythonExportStatus::Complete
        },
    )
}

fn literal_strings(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::List(node) => literal_string_elements(&node.elts),
        Expr::Tuple(node) => literal_string_elements(&node.elts),
        Expr::Set(node) => literal_string_elements(&node.elts),
        Expr::BinOp(node) if node.op == Operator::Add => {
            let mut left = literal_strings(&node.left)?;
            left.extend(literal_strings(&node.right)?);
            Some(left)
        }
        _ => None,
    }
}

fn literal_string_elements(elements: &[Expr]) -> Option<Vec<String>> {
    elements
        .iter()
        .map(|element| {
            element
                .as_string_literal_expr()
                .map(|literal| literal.value.to_str().to_owned())
        })
        .collect()
}
