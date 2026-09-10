//! One native HIR traversal exports resolved paths, method references and namespace-aware imports.
//! Compiler indices remain observation-local provenance; no canonical identity is minted here.

use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, Item, ItemKind, Node, Path, QPath, UseKind, UsePath};
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use super::{OwnedRow, OwnerRelations, RustcRelation, owned_span};

pub(super) fn emit(tcx: TyCtxt<'_>, relations: &mut OwnerRelations) {
    let mut visitor = References {
        tcx,
        relations,
        next_reference: 0,
        next_import: 0,
    };
    tcx.hir_visit_all_item_likes_in_crate(&mut visitor);
    if visitor.relations.unresolved_references > 0 {
        visitor.relations.push(
            RustcRelation::Remainder,
            OwnedRow::default()
                .utf8("fact_family", RustcRelation::HirReference.relation_id())
                .utf8("reason_code", "HIR_RESOLUTION_UNAVAILABLE")
                .utf8("authority_surface", "rustc_hir::Res / TyCtxt::typeck")
                .boolean("bounded", true)
                .utf8(
                    "detail",
                    "native reference observations are retained without an invented target",
                ),
        );
    }
}

struct References<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    relations: &'a mut OwnerRelations,
    next_reference: u64,
    next_import: u64,
}

impl References<'_, '_> {
    fn location(&self, span: Span) -> OwnedRow {
        let public = rustc_public::rustc_internal::stable(span);
        OwnedRow::default()
            .span(&owned_span(self.tcx, public))
            .utf8("location_state", "unmapped-path")
    }

    fn key(
        &self,
        row: OwnedRow,
        id: DefId,
        crate_field: &'static str,
        path_field: &'static str,
    ) -> OwnedRow {
        let hash = self.tcx.def_path_hash(id);
        row.u64(crate_field, hash.stable_crate_id().as_u64())
            .fixed16(path_field, hash.to_raw_def_path_hash().0)
    }

    fn target(&mut self, row: OwnedRow, resolution: Res) -> OwnedRow {
        let row = row.utf8(
            "resolution_kind",
            match resolution {
                Res::Def(..) => "definition",
                Res::PrimTy(_) => "primitive-type",
                Res::Local(_) => "local-binding",
                Res::SelfTyParam { .. } => "self-type-parameter",
                Res::SelfTyAlias { .. } => "self-type-alias",
                Res::SelfCtor(_) => "self-constructor",
                Res::ToolMod => "tool-module",
                Res::OpenMod(_) => "open-module",
                Res::NonMacroAttr(_) => "non-macro-attribute",
                Res::Err => "unresolved",
            },
        );
        match resolution {
            Res::Def(kind, id) => self
                .key(row, id, "target_stable_crate_id", "target_def_path_hash")
                .utf8("target_definition_kind", definition_kind(kind))
                .utf8(
                    "target_native_definition_kind",
                    native_definition_kind(kind),
                )
                .boolean("target_is_local", id.is_local()),
            Res::Local(id) => self
                .key(
                    row,
                    id.owner.def_id.to_def_id(),
                    "target_local_owner_stable_crate_id",
                    "target_local_owner_def_path_hash",
                )
                .u64("target_local_index", id.local_id.as_u32()),
            Res::PrimTy(primitive) => row.utf8("primitive_kind", primitive.name_str()),
            Res::SelfTyParam { trait_: id }
            | Res::SelfTyAlias { alias_to: id, .. }
            | Res::SelfCtor(id) => self
                .key(row, id, "target_stable_crate_id", "target_def_path_hash")
                .utf8(
                    "target_definition_kind",
                    definition_kind(self.tcx.def_kind(id)),
                )
                .utf8(
                    "target_native_definition_kind",
                    native_definition_kind(self.tcx.def_kind(id)),
                )
                .boolean("target_is_local", id.is_local()),
            Res::Err => {
                self.relations.unresolved_references += 1;
                row
            }
            Res::ToolMod | Res::OpenMod(_) | Res::NonMacroAttr(_) => row,
        }
    }

    fn reference(
        &mut self,
        id: HirId,
        span: Span,
        name: &str,
        kind: &str,
        resolutions: impl IntoIterator<Item = (Option<&'static str>, Res)>,
    ) -> u64 {
        let ordinal = self.next_reference;
        self.next_reference += 1;
        let base = self
            .key(
                self.location(span),
                id.owner.def_id.to_def_id(),
                "occurrence_owner_stable_crate_id",
                "occurrence_owner_def_path_hash",
            )
            .u64("occurrence_local_index", id.local_id.as_u32())
            .u64("reference_ordinal", ordinal)
            .utf8("name", name)
            .utf8("reference_kind", kind);
        let mut emitted = false;
        for (target_ordinal, (namespace, resolution)) in resolutions.into_iter().enumerate() {
            emitted = true;
            let row = self.target(
                base.clone()
                    .u64("target_ordinal", target_ordinal)
                    .maybe_utf8("target_namespace", namespace),
                resolution,
            );
            self.relations.push(RustcRelation::HirReference, row);
        }
        if !emitted {
            let row = self.target(base, Res::Err);
            self.relations.push(RustcRelation::HirReference, row);
        }
        ordinal
    }

    fn type_dependent(&self, id: HirId) -> Res {
        if self.tcx.has_typeck_results(id.owner.def_id) {
            self.tcx
                .typeck(id.owner.def_id)
                .type_dependent_def(id)
                .map_or(Res::Err, |(kind, id)| Res::Def(kind, id))
        } else {
            Res::Err
        }
    }

    fn import(&mut self, item: &Item<'_>, path: &UsePath<'_>, kind: UseKind) {
        let (kind, alias) = match kind {
            UseKind::Single(ident) => ("single", Some(ident.name.as_str().to_owned())),
            UseKind::Glob => ("glob", None),
            UseKind::ListStem => return, // A lowering artifact, not a separate source import.
        };
        let name = path
            .segments
            .last()
            .map_or("", |segment| segment.ident.name.as_str());
        let reference = self.reference(
            item.hir_id(),
            path.span,
            name,
            "import",
            [
                (Some("type"), path.res.type_ns),
                (Some("value"), path.res.value_ns),
                (Some("macro"), path.res.macro_ns),
            ]
            .into_iter()
            .filter_map(|(namespace, res)| res.map(|res| (namespace, res))),
        );
        let row = self
            .key(
                self.location(item.span),
                item.owner_id.def_id.to_def_id(),
                "import_stable_crate_id",
                "import_def_path_hash",
            )
            .u64("import_ordinal", self.next_import)
            .u64("reference_ordinal", reference)
            .utf8("import_kind", kind)
            .maybe_utf8("alias", alias)
            .utf8(
                "path",
                path.segments
                    .iter()
                    .map(|segment| segment.ident.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
            .boolean(
                "is_public",
                self.tcx.visibility(item.owner_id.def_id).is_public(),
            );
        self.next_import += 1;
        self.relations.push(RustcRelation::HirImport, row);
    }
}

impl<'tcx> Visitor<'tcx> for References<'_, 'tcx> {
    type NestedFilter = rustc_middle::hir::nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx Item<'tcx>) {
        if let ItemKind::Use(path, kind) = item.kind {
            self.import(item, path, kind);
        }
        intravisit::walk_item(self, item);
    }

    fn visit_use(&mut self, _: &'tcx UsePath<'tcx>, _: HirId) {
        // visit_item already emits each namespace; walk_use would duplicate those references.
    }

    fn visit_path(&mut self, path: &Path<'tcx>, id: HirId) {
        let kind = match self.tcx.hir_node(id) {
            Node::Expr(_) => "value-path",
            Node::Ty(_) => "type-path",
            Node::Pat(_) => "pattern-path",
            _ => "path",
        };
        let name = path
            .segments
            .last()
            .map_or("", |segment| segment.ident.name.as_str());
        self.reference(id, path.span, name, kind, [(None, path.res)]);
        intravisit::walk_path(self, path);
    }

    fn visit_qpath(&mut self, path: &'tcx QPath<'tcx>, id: HirId, span: Span) {
        if let QPath::TypeRelative(_, segment) = path {
            self.reference(
                id,
                span,
                segment.ident.name.as_str(),
                "associated-path",
                [(None, self.type_dependent(id))],
            );
        }
        intravisit::walk_qpath(self, path, id);
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, ..) = expr.kind {
            self.reference(
                expr.hir_id,
                segment.ident.span,
                segment.ident.name.as_str(),
                "method-call",
                [(None, self.type_dependent(expr.hir_id))],
            );
        }
        intravisit::walk_expr(self, expr);
    }
}

// Native discriminants remain separate where canonical declarations share a normalized kind.
fn native_definition_kind(kind: DefKind) -> &'static str {
    match kind {
        DefKind::AssocFn => "associated-function",
        DefKind::Const {
            is_type_const: true,
        } => "type-constant",
        DefKind::AssocConst {
            is_type_const: true,
        } => "associated-type-constant",
        DefKind::AssocConst {
            is_type_const: false,
        } => "associated-constant",
        DefKind::Ctor(CtorOf::Struct, CtorKind::Fn) => "struct-function-constructor",
        DefKind::Ctor(CtorOf::Struct, CtorKind::Const) => "struct-constant-constructor",
        DefKind::Ctor(CtorOf::Variant, CtorKind::Fn) => "variant-function-constructor",
        DefKind::Ctor(CtorOf::Variant, CtorKind::Const) => "variant-constant-constructor",
        _ => definition_kind(kind),
    }
}

// Unknown application mappings stay unknown at the later canonical join.
fn definition_kind(kind: DefKind) -> &'static str {
    match kind {
        DefKind::Mod => "module",
        DefKind::Struct => "struct",
        DefKind::Union => "union",
        DefKind::Enum => "enum",
        DefKind::Variant => "variant",
        DefKind::Trait => "trait",
        DefKind::TyAlias => "type-alias",
        DefKind::ForeignTy => "foreign-type",
        DefKind::TraitAlias => "trait-alias",
        DefKind::AssocTy => "associated-type",
        DefKind::TyParam => "type-parameter",
        DefKind::Fn | DefKind::AssocFn => "function",
        DefKind::Const { .. } | DefKind::AssocConst { .. } => "constant",
        DefKind::ConstParam => "const-parameter",
        DefKind::Static { .. } => "static",
        DefKind::Ctor(_, CtorKind::Fn) => "constructor-function",
        DefKind::Ctor(_, CtorKind::Const) => "constructor-constant",
        DefKind::Macro(_) => "macro",
        DefKind::ExternCrate => "extern-crate",
        DefKind::Use => "import",
        DefKind::ForeignMod => "foreign-module",
        DefKind::AnonConst => "anonymous-constant",
        DefKind::OpaqueTy => "opaque-type",
        DefKind::Field => "field",
        DefKind::LifetimeParam => "lifetime-parameter",
        DefKind::GlobalAsm => "global-assembly",
        DefKind::Impl { .. } => "implementation",
        DefKind::Closure => "closure",
        DefKind::SyntheticCoroutineBody => "synthetic-coroutine-body",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OwnedCell, extract_owned};
    use super::RustcRelation;

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real compiler invocation verifies independent import, namespace, method and source expectations"
    )]
    fn native_hir_resolves_aliases_namespaces_methods_and_keeps_local_provenance() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("lib.rs");
        let text = "mod inner;\nuse inner::{target as selected, Token};\npub use inner::target as exported;\nuse inner::*;\npub fn run(value: u8) -> u8 { let local = value; selected(local) + Token::new().method() }\n";
        std::fs::write(&source, text).unwrap();
        std::fs::write(root.path().join("inner.rs"), "pub struct Token;\nimpl Token { pub fn new() -> Self { Token } pub fn method(&self) -> u8 { 2 } }\npub fn target(value: u8) -> u8 { value }\n").unwrap();
        let sysroot = std::process::Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success());
        let result = extract_owned(&[
            "rustc".into(),
            source.display().to_string(),
            "--crate-name=hir_references".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "--emit=metadata".into(),
            "--cap-lints=allow".into(),
            format!("--out-dir={}", root.path().display()),
            format!(
                "--sysroot={}",
                String::from_utf8(sysroot.stdout).unwrap().trim()
            ),
        ]);
        assert!(result.compiler_succeeded);
        let rows = |family| {
            result
                .owners
                .iter()
                .flat_map(|owner| &owner.relations)
                .filter(move |relation| relation.relation == family)
                .flat_map(|relation| &relation.rows)
                .collect::<Vec<_>>()
        };
        let refs = rows(RustcRelation::HirReference);
        let imports = rows(RustcRelation::HirImport);
        let source_ranges = imports
            .iter()
            .filter(|row| {
                row.0.get("expansion_kind") == Some(&OwnedCell::Utf8("source-authored".into()))
            })
            .map(|row| {
                let (OwnedCell::UInt64(start), OwnedCell::UInt64(end)) =
                    (&row.0["span_start_byte"], &row.0["span_end_byte"])
                else {
                    panic!("source range");
                };
                (*start, *end)
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            source_ranges.len(),
            4,
            "each nested import retains a distinct source occurrence"
        );
        assert_eq!(
            imports
                .iter()
                .filter(|row| row.0.get("expansion_kind")
                    == Some(&OwnedCell::Utf8("source-authored".into())))
                .count(),
            4,
            "list stems do not invent source imports; compiler-injected imports remain separate"
        );
        let equals = |row: &&super::OwnedRow, field, value: &str| {
            row.0.get(field) == Some(&OwnedCell::Utf8(value.to_owned()))
        };
        let declaration_key = |suffix: &str| {
            result
                .owners
                .iter()
                .find(|owner| owner.qualified_name.ends_with(suffix))
                .unwrap()
                .compiler_key
                .unwrap()
        };
        for (name, suffix) in [
            ("selected", "inner::target"),
            ("method", "Token::method"),
            ("new", "Token::new"),
        ] {
            let key = declaration_key(suffix);
            let matched = refs
                .iter()
                .filter(|row| equals(row, "name", name))
                .collect::<Vec<_>>();
            assert!(!matched.is_empty(), "missing {name}");
            assert!(matched.iter().any(|row| row.0.get("target_def_path_hash")
                == Some(&OwnedCell::Fixed16(key.def_path_hash))
                && row.0.get("target_stable_crate_id")
                    == Some(&OwnedCell::UInt64(key.stable_crate_id))));
        }
        let token = refs
            .iter()
            .filter(|row| equals(row, "name", "Token") && equals(row, "reference_kind", "import"))
            .collect::<Vec<_>>();
        assert_eq!(
            token.len(),
            2,
            "unit struct imports have distinct type and value resolutions"
        );
        assert_ne!(
            token[0].0["target_namespace"],
            token[1].0["target_namespace"]
        );
        assert_ne!(
            token[0].0["target_def_path_hash"],
            token[1].0["target_def_path_hash"]
        );
        assert!(
            refs.iter()
                .any(|row| equals(row, "resolution_kind", "primitive-type")
                    && equals(row, "primitive_kind", "u8"))
        );
        assert!(refs.iter().any(|row| equals(row, "name", "local")
            && row.0.contains_key("target_local_owner_def_path_hash")
            && row.0.contains_key("target_local_index")));
        let method = refs
            .iter()
            .find(|row| equals(row, "reference_kind", "method-call"))
            .unwrap();
        let (OwnedCell::UInt64(start), OwnedCell::UInt64(end)) =
            (&method.0["span_start_byte"], &method.0["span_end_byte"])
        else {
            panic!("native range");
        };
        assert_eq!(
            &text.as_bytes()[usize::try_from(*start).unwrap()..usize::try_from(*end).unwrap()],
            b"method"
        );
        assert!(imports.iter().any(|row| equals(row, "alias", "exported")
            && row.0["is_public"] == OwnedCell::Boolean(true)));
        assert!(
            imports
                .iter()
                .any(|row| equals(row, "import_kind", "glob") && !row.0.contains_key("alias"))
        );
    }
}
