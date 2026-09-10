/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Seed declaration types even when no expression refers to the declaration. Native AST traversal
//! and checker answers share the expression census transaction and the same bounded type graph.

use pyrefly_python::short_identifier::ShortIdentifier;
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::{self, Visitor};
use starlark_map::Hashed;

use crate::binding::binding::Key;

use super::{GraphBuilder, TypeShapeContext};

pub(super) fn collect(context: &TypeShapeContext, body: &[Stmt], graph: &mut GraphBuilder) {
    let Some(bindings) = context.transaction.get_bindings(context.source_handle) else {
        return;
    };
    let Some(answers) = context.transaction.get_answers(context.source_handle) else {
        return;
    };
    let mut record = |statement: &Stmt| {
        let name = match statement {
            Stmt::FunctionDef(function) => &function.name,
            Stmt::ClassDef(class) => &class.name,
            _ => return,
        };
        // Both native function_def and class_object_and_indices bind this exact identifier key.
        // Use the indexed export-boundary answer directly, avoiding a position-based AST search
        // for every declaration and preserving its definition-linked type through decorators.
        let key = Key::Definition(ShortIdentifier::new(name));
        if let Some(index) = bindings.key_to_idx_hashed_opt(Hashed::new(&key))
            && let Some(ty) = answers.get_type_at(index)
        {
            graph.index(context, &ty);
        }
    };
    let mut visitor = Declarations {
        record: &mut record,
    };
    for statement in body {
        visitor.visit_stmt(statement);
    }
}

struct Declarations<'f, F> {
    record: &'f mut F,
}

impl<'a, F: FnMut(&'a Stmt)> Visitor<'a> for Declarations<'_, F> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        (self.record)(statement);
        visitor::walk_stmt(self, statement);
    }
}
