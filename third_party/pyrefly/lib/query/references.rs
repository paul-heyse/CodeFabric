/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A bulk, typed boundary over the checker's definition resolver. One transaction and AST walk
//! preserve the configured source/stub choice; callers never need one protocol request per use.

use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::symbol_kind::SymbolKind;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{Alias, Expr, ExprAttribute, ExprContext, ExprName, Identifier};
use ruff_text_size::{Ranged, TextRange, TextSize};

use super::{CalleeDefinition, Query};
use crate::state::lsp::{FindPreference, ImportBehavior};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticReferenceKind {
    Read,
    Write,
    Delete,
    Import,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct SemanticReferenceTarget {
    pub definition: CalleeDefinition,
    pub is_module: bool,
    /// Native symbol-kind evidence. The application owns canonical kind and identity.
    pub symbol_kind: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SemanticReference {
    pub start_byte: u32,
    pub end_byte: u32,
    pub name: String,
    pub kind: SemanticReferenceKind,
    pub targets: Vec<SemanticReferenceTarget>,
    pub definition_unavailable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SemanticReferenceResponse {
    pub references: Vec<SemanticReference>,
    /// False means the caller's finite occurrence bound stopped resolution.
    pub complete: bool,
}

impl Query {
    pub fn get_semantic_references_in_file(
        &self,
        name: ModuleName,
        path: ModulePath,
        maximum_occurrences: usize,
    ) -> Option<SemanticReferenceResponse> {
        let handle = self.make_handle(name, path);
        let transaction = self.state.transaction();
        let ast = transaction.get_ast(&handle)?;
        // AST-only state cannot certify a semantic reference census.
        transaction.get_bindings(&handle)?;
        transaction.get_answers(&handle)?;
        let preference = FindPreference {
            import_behavior: ImportBehavior::ResolveDenotations,
            prefer_pyi: true,
            resolve_call_dunders: false,
            disable_style_fallback: true,
        };
        let mut response = SemanticReferenceResponse {
            complete: true,
            ..Default::default()
        };
        let mut resolve = |range: TextRange, name: &str, kind, site: Site<'_>| {
            if response.references.len() >= maximum_occurrences {
                response.complete = false;
                return;
            }
            // For dotted import names, resolve the final identifier, rather than the first prefix.
            let position = TextSize::from(
                range
                    .end()
                    .to_u32()
                    .saturating_sub(1)
                    .max(range.start().to_u32()),
            );
            let resolved = match site {
                Site::Name(name) => {
                    let identifier = Identifier::new(name.id.clone(), name.range());
                    if name.ctx == ExprContext::Store {
                        transaction.find_definition_for_name_def(&handle, &identifier, preference)
                    } else {
                        transaction.find_definition_for_name_use(&handle, &identifier, preference)
                    }
                    .map(|definition| definition.into_iter().collect::<Vec<_>>())
                }
                Site::Attribute(attribute) => transaction
                    .find_definition_for_attribute(
                        &handle,
                        attribute.value.range(),
                        attribute.attr.id(),
                        preference,
                    )
                    .map(|definitions| definitions.into_vec()),
                Site::Import => transaction
                    .find_definition(&handle, position, preference)
                    .map(|definitions| definitions.into_vec()),
            };
            let unavailable = resolved.is_err();
            let mut targets = resolved
                .into_iter()
                .flatten()
                .map(|target| SemanticReferenceTarget {
                    is_module: target.definition_range.is_empty()
                        && target.metadata.symbol_kind() == Some(SymbolKind::Module),
                    definition: CalleeDefinition {
                        path: target.module.path().as_path().to_path_buf(),
                        start_byte: target.definition_range.start().to_u32(),
                        end_byte: target.definition_range.end().to_u32(),
                    },
                    symbol_kind: target
                        .metadata
                        .symbol_kind()
                        .map(|kind| format!("{kind:?}")),
                })
                .collect::<Vec<_>>();
            targets.sort_by(|a, b| {
                (
                    &a.definition.path,
                    a.definition.start_byte,
                    a.definition.end_byte,
                    a.is_module,
                    &a.symbol_kind,
                )
                    .cmp(&(
                        &b.definition.path,
                        b.definition.start_byte,
                        b.definition.end_byte,
                        b.is_module,
                        &b.symbol_kind,
                    ))
            });
            targets.dedup_by(|a, b| {
                a.definition == b.definition
                    && a.is_module == b.is_module
                    && a.symbol_kind == b.symbol_kind
            });
            response.references.push(SemanticReference {
                start_byte: range.start().to_u32(),
                end_byte: range.end().to_u32(),
                name: name.to_owned(),
                kind,
                targets,
                definition_unavailable: unavailable,
            });
        };
        visitor::walk_body(
            &mut References {
                resolve: &mut resolve,
            },
            &ast.body,
        );
        response
            .references
            .sort_by_key(|reference| (reference.start_byte, reference.end_byte));
        Some(response)
    }
}

struct References<'f, F> {
    resolve: &'f mut F,
}

enum Site<'a> {
    Name(&'a ExprName),
    Attribute(&'a ExprAttribute),
    Import,
}

impl<'a, F: FnMut(TextRange, &str, SemanticReferenceKind, Site<'a>)> Visitor<'a>
    for References<'_, F>
{
    fn visit_expr(&mut self, expr: &'a Expr) {
        let kind = |context: ExprContext| match context {
            ExprContext::Load => SemanticReferenceKind::Read,
            ExprContext::Store => SemanticReferenceKind::Write,
            ExprContext::Del => SemanticReferenceKind::Delete,
            ExprContext::Invalid => SemanticReferenceKind::Unknown,
        };
        match expr {
            Expr::Name(name) => (self.resolve)(
                name.range(),
                name.id.as_str(),
                kind(name.ctx),
                Site::Name(name),
            ),
            Expr::Attribute(attribute) => (self.resolve)(
                attribute.attr.range(),
                attribute.attr.as_str(),
                kind(attribute.ctx),
                Site::Attribute(attribute),
            ),
            _ => {}
        }
        visitor::walk_expr(self, expr);
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        (self.resolve)(
            alias.name.range(),
            alias.name.as_str(),
            SemanticReferenceKind::Import,
            Site::Import,
        );
    }
}
