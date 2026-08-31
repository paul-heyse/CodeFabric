//! Exact dated-nightly compiler seam.
//!
//! `rustc_public` and the narrowly selected `TyCtxt` identity/source helpers are consumed only
//! inside the compiler callback. The callback returns application-owned scalar relation rows;
//! no compiler handle, debug rendering, MIR text, or opaque JSON crosses this boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use rustc_middle::ty::TyCtxt;
use rustc_public::mir::mono::{Instance, InstanceKind};
use rustc_public::mir::{
    AggregateKind, AssertMessage, BinOp, BorrowKind, CastKind, FakeReadCause, Mutability,
    NonDivergingIntrinsic, Operand, Place, ProjectionElem, RawPtrKind, RuntimeChecks, Rvalue,
    StatementKind, TerminatorKind, UnOp, UnwindAction,
};
use rustc_public::ty::{
    AliasKind, FloatTy, GenericArgs, IntTy, Region, RegionKind, RigidTy, Ty, TyKind, UintTy,
};
use rustc_public::{CrateDef, CtorKind, ItemKind};
use rustc_public_bridge::IndexedVal;

use crate::rustc_relation_schema::{RUSTC_PUBLIC_RELEASE, RUSTC_TOOLCHAIN, RustcRelation};

/// Closed scalar set used by the extractor-owned relation rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnedCell {
    Utf8(String),
    UInt64(u64),
    Boolean(bool),
    Fixed16([u8; 16]),
    Fixed32([u8; 32]),
}

/// One schema-keyed row. Missing keys are Arrow nulls and are accepted only for nullable fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OwnedRow(pub(crate) BTreeMap<&'static str, OwnedCell>);

impl OwnedRow {
    fn utf8(mut self, field: &'static str, value: impl Into<String>) -> Self {
        self.0.insert(field, OwnedCell::Utf8(value.into()));
        self
    }

    fn u64(mut self, field: &'static str, value: impl TryInto<u64>) -> Self {
        self.0.insert(
            field,
            OwnedCell::UInt64(value.try_into().unwrap_or(u64::MAX)),
        );
        self
    }

    fn boolean(mut self, field: &'static str, value: bool) -> Self {
        self.0.insert(field, OwnedCell::Boolean(value));
        self
    }

    fn fixed16(mut self, field: &'static str, value: [u8; 16]) -> Self {
        self.0.insert(field, OwnedCell::Fixed16(value));
        self
    }

    fn fixed32(mut self, field: &'static str, value: [u8; 32]) -> Self {
        self.0.insert(field, OwnedCell::Fixed32(value));
        self
    }

    fn maybe_utf8(self, field: &'static str, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(value) => self.utf8(field, value),
            None => self,
        }
    }

    fn maybe_u64(self, field: &'static str, value: Option<impl TryInto<u64>>) -> Self {
        match value {
            Some(value) => self.u64(field, value),
            None => self,
        }
    }

    fn maybe_fixed32(self, field: &'static str, value: Option<[u8; 32]>) -> Self {
        match value {
            Some(value) => self.fixed32(field, value),
            None => self,
        }
    }

    fn span(self, span: &OwnedSpan) -> Self {
        self.utf8("span_file", &span.file)
            .u64("span_start_byte", span.start_byte)
            .u64("span_end_byte", span.end_byte)
            .u64("span_start_line", span.start_line)
            .u64("span_end_line", span.end_line)
            .u64("span_start_column", span.start_column)
            .u64("span_end_column", span.end_column)
            .utf8("expansion_kind", span.expansion_kind)
    }
}

/// One relation-scoped batch belonging to one compiler owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedRustcRelation {
    pub relation: RustcRelation,
    pub rows: Vec<OwnedRow>,
}

/// Stable compiler identity derived by the selected private seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnedCompilerKey {
    pub stable_crate_id: u64,
    pub def_path_hash: [u8; 16],
}

/// One application-owned replacement partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedRustcOwner {
    pub qualified_name: String,
    pub owner_kind: String,
    pub compiler_key: Option<OwnedCompilerKey>,
    pub relations: Vec<OwnedRustcRelation>,
}

/// Complete successful callback output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedRustcExtraction {
    pub owners: Vec<OwnedRustcOwner>,
}

#[derive(Clone, Debug)]
struct OwnedSpan {
    file: String,
    start_byte: u64,
    end_byte: u64,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
    expansion_kind: &'static str,
    in_external_macro: bool,
}

struct OwnerRelations {
    rows: BTreeMap<RustcRelation, Vec<OwnedRow>>,
    requested_native_families: BTreeSet<RustcRelation>,
    unresolved_calls: u64,
    unknown_operand_types: u64,
}

impl OwnerRelations {
    fn requested(families: impl IntoIterator<Item = RustcRelation>) -> Self {
        Self {
            rows: BTreeMap::new(),
            requested_native_families: families.into_iter().collect(),
            unresolved_calls: 0,
            unknown_operand_types: 0,
        }
    }

    fn push(&mut self, relation: RustcRelation, row: OwnedRow) {
        self.rows.entry(relation).or_default().push(row);
    }

    fn finish(mut self) -> Vec<OwnedRustcRelation> {
        let observations = self
            .requested_native_families
            .iter()
            .map(|relation| {
                (
                    *relation,
                    self.rows.get(relation).map_or(0, |rows| rows.len() as u64),
                )
            })
            .collect::<Vec<_>>();
        for (relation, emitted) in observations {
            self.rows.entry(relation).or_default();
            let remainder_count = match relation {
                RustcRelation::Call => self.unresolved_calls,
                RustcRelation::MirOperand => self.unknown_operand_types,
                _ => 0,
            };
            let partial = remainder_count > 0;
            self.push(
                RustcRelation::Coverage,
                OwnedRow::default()
                    .utf8("fact_family", relation.relation_id())
                    .utf8("authority_surface", authority_surface(relation))
                    .u64("requested_units", 1)
                    .u64("completed_units", 1)
                    .u64("emitted_rows", emitted)
                    .utf8(
                        "completeness",
                        if partial {
                            "partial-characterized"
                        } else {
                            "complete"
                        },
                    )
                    .u64("remainder_count", remainder_count)
                    .boolean("unknown_semantics", partial),
            );
        }
        self.rows
            .into_iter()
            .map(|(relation, rows)| OwnedRustcRelation { relation, rows })
            .collect()
    }
}

const COMPILATION_NATIVE_FAMILIES: [RustcRelation; 1] = [RustcRelation::Compilation];

const ITEM_NATIVE_FAMILIES: [RustcRelation; 14] = [
    RustcRelation::PublicItem,
    RustcRelation::Type,
    RustcRelation::Instance,
    RustcRelation::MirBody,
    RustcRelation::MirBlock,
    RustcRelation::MirLocal,
    RustcRelation::MirPlace,
    RustcRelation::MirOperand,
    RustcRelation::MirRvalue,
    RustcRelation::MirStatement,
    RustcRelation::MirTerminator,
    RustcRelation::CfgEdge,
    RustcRelation::Call,
    RustcRelation::Access,
];

struct TypeCollector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    owner: &'a mut OwnerRelations,
    seen: BTreeSet<[u8; 32]>,
}

impl TypeCollector<'_, '_> {
    fn register(&mut self, ty: Ty) -> [u8; 32] {
        let key = type_key(self.tcx, ty);
        if !self.seen.insert(key) {
            return key;
        }
        let kind = ty.kind();
        let kind_name = type_kind_name(&kind);
        let definition = type_definition(self.tcx, &kind);
        let scalar = type_scalar(&kind);
        let mut root = OwnedRow::default()
            .fixed32("type_key", key)
            .utf8("type_kind", kind_name)
            .utf8("component_role", "self")
            .u64("component_ordinal", 0)
            .maybe_utf8("scalar_value", scalar.clone())
            .maybe_utf8("mutability", type_mutability(&kind));
        if let Some((path, compiler_key)) = &definition {
            root = root
                .utf8("definition_path", path)
                .u64("definition_stable_crate_id", compiler_key.stable_crate_id)
                .fixed16("definition_def_path_hash", compiler_key.def_path_hash);
        }
        self.owner.push(RustcRelation::Type, root);

        for (ordinal, (role, child)) in type_components(&kind).into_iter().enumerate() {
            let child_key = self.register(child);
            let mut row = OwnedRow::default()
                .fixed32("type_key", key)
                .utf8("type_kind", kind_name)
                .utf8("component_role", role)
                .u64("component_ordinal", ordinal + 1)
                .fixed32("component_type_key", child_key)
                .maybe_utf8("scalar_value", scalar.clone())
                .maybe_utf8("mutability", type_mutability(&kind));
            if let Some((path, compiler_key)) = &definition {
                row = row
                    .utf8("definition_path", path)
                    .u64("definition_stable_crate_id", compiler_key.stable_crate_id)
                    .fixed16("definition_def_path_hash", compiler_key.def_path_hash);
            }
            self.owner.push(RustcRelation::Type, row);
        }
        key
    }
}

#[derive(Clone, Copy)]
struct Location {
    block: usize,
    slot_kind: &'static str,
    slot_index: usize,
}

pub(crate) fn compiler_surface_smoke() -> usize {
    rustc_public::target::MachineSize::from_bits(usize::BITS as usize).bytes()
}

#[cfg(test)]
fn count_items_inside_callback() -> ControlFlow<(), usize> {
    ControlFlow::Continue(rustc_public::all_local_items().len())
}

#[cfg(test)]
#[allow(
    clippy::unnested_or_patterns,
    reason = "the pinned rustc_public::run! macro expands the unnested pattern"
)]
pub(crate) fn count_local_items(rustc_args: &[String]) -> Result<usize, String> {
    rustc_public::run!(rustc_args, count_items_inside_callback)
        .map_err(|error| format!("{error:?}"))
}

fn framed_hash(domain: &[u8], fields: impl IntoIterator<Item = Vec<u8>>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for field in fields {
        hasher.update(&(field.len() as u64).to_be_bytes());
        hasher.update(&field);
    }
    *hasher.finalize().as_bytes()
}

fn compiler_key<T: CrateDef>(tcx: TyCtxt<'_>, definition: &T) -> OwnedCompilerKey {
    let internal = rustc_public::rustc_internal::internal(tcx, definition.def_id());
    let hash = tcx.def_path_hash(internal);
    OwnedCompilerKey {
        stable_crate_id: hash.stable_crate_id().as_u64(),
        def_path_hash: hash.to_raw_def_path_hash().0,
    }
}

fn type_key(tcx: TyCtxt<'_>, ty: Ty) -> [u8; 32] {
    let internal = rustc_public::rustc_internal::internal(tcx, ty);
    let compiler_hash = tcx.type_id_hash(internal).as_u128().to_le_bytes();
    framed_hash(b"codefabric.rustc.type-id.v1\0", [compiler_hash.to_vec()])
}

fn owned_span(tcx: TyCtxt<'_>, span: rustc_public::ty::Span) -> OwnedSpan {
    let internal = rustc_public::rustc_internal::internal(tcx, span);
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_byte_offset(internal.lo());
    let start_pos = start.sf.start_pos.0;
    let lines = span.get_lines();
    OwnedSpan {
        file: start.sf.name.prefer_remapped_unconditionally().to_string(),
        start_byte: u64::from(internal.lo().0.saturating_sub(start_pos)),
        end_byte: u64::from(internal.hi().0.saturating_sub(start_pos)),
        start_line: lines.start_line as u64,
        start_column: lines.start_col as u64,
        end_line: lines.end_line as u64,
        end_column: lines.end_col as u64,
        expansion_kind: if internal.from_expansion() {
            "macro-expansion"
        } else {
            "source-authored"
        },
        in_external_macro: internal.in_external_macro(source_map),
    }
}

fn item_kind(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Fn => "function",
        ItemKind::Static => "static",
        ItemKind::Const => "constant",
        ItemKind::Ctor(CtorKind::Const) => "constructor-constant",
        ItemKind::Ctor(CtorKind::Fn) => "constructor-function",
    }
}

fn statement_kind(kind: &StatementKind) -> (&'static str, &'static str) {
    match kind {
        StatementKind::Assign(..) => ("Assign", "assignment"),
        StatementKind::FakeRead(..) => ("FakeRead", "compiler-analysis-artifact"),
        StatementKind::SetDiscriminant { .. } => ("SetDiscriminant", "discriminant-write"),
        StatementKind::StorageLive(..) => ("StorageLive", "storage-lifetime"),
        StatementKind::StorageDead(..) => ("StorageDead", "storage-lifetime"),
        StatementKind::PlaceMention(..) => ("PlaceMention", "compiler-analysis-artifact"),
        StatementKind::AscribeUserType { .. } => ("AscribeUserType", "type-ascription"),
        StatementKind::Coverage(..) => ("Coverage", "compiler-coverage-marker"),
        StatementKind::Intrinsic(..) => ("Intrinsic", "non-diverging-intrinsic"),
        StatementKind::ConstEvalCounter => ("ConstEvalCounter", "const-eval-marker"),
        StatementKind::Nop => ("Nop", "no-effect"),
    }
}

fn terminator_kind(kind: &TerminatorKind) -> &'static str {
    match kind {
        TerminatorKind::Goto { .. } => "Goto",
        TerminatorKind::SwitchInt { .. } => "SwitchInt",
        TerminatorKind::Resume => "Resume",
        TerminatorKind::Abort => "Abort",
        TerminatorKind::Return => "Return",
        TerminatorKind::Unreachable => "Unreachable",
        TerminatorKind::Drop { .. } => "Drop",
        TerminatorKind::Call { .. } => "Call",
        TerminatorKind::Assert { .. } => "Assert",
        TerminatorKind::InlineAsm { .. } => "InlineAsm",
    }
}

fn authority_surface(relation: RustcRelation) -> &'static str {
    match relation {
        RustcRelation::Compilation
        | RustcRelation::PublicItem
        | RustcRelation::Type
        | RustcRelation::Instance
        | RustcRelation::MirBody
        | RustcRelation::MirBlock
        | RustcRelation::MirLocal
        | RustcRelation::MirPlace
        | RustcRelation::MirOperand
        | RustcRelation::MirRvalue
        | RustcRelation::MirStatement
        | RustcRelation::MirTerminator
        | RustcRelation::CfgEdge
        | RustcRelation::Call
        | RustcRelation::Access => "rustc_public-1.100.0-nightly",
        RustcRelation::Diagnostic => "rustc-driver-diagnostic-boundary",
        RustcRelation::Coverage | RustcRelation::Remainder => "codefabric-adapter-v1",
    }
}

fn type_kind_name(kind: &TyKind) -> &'static str {
    match kind {
        TyKind::RigidTy(rigid) => match rigid {
            RigidTy::Bool => "Bool",
            RigidTy::Char => "Char",
            RigidTy::Int(_) => "Int",
            RigidTy::Uint(_) => "Uint",
            RigidTy::Float(_) => "Float",
            RigidTy::Adt(..) => "Adt",
            RigidTy::Foreign(..) => "Foreign",
            RigidTy::Str => "Str",
            RigidTy::Array(..) => "Array",
            RigidTy::Pat(..) => "Pattern",
            RigidTy::Slice(..) => "Slice",
            RigidTy::RawPtr(..) => "RawPtr",
            RigidTy::Ref(..) => "Ref",
            RigidTy::FnDef(..) => "FnDef",
            RigidTy::FnPtr(..) => "FnPtr",
            RigidTy::Closure(..) => "Closure",
            RigidTy::Coroutine(..) => "Coroutine",
            RigidTy::CoroutineClosure(..) => "CoroutineClosure",
            RigidTy::Dynamic(..) => "Dynamic",
            RigidTy::Never => "Never",
            RigidTy::Tuple(..) => "Tuple",
            RigidTy::CoroutineWitness(..) => "CoroutineWitness",
        },
        TyKind::Alias(kind, _) => match kind {
            AliasKind::Projection => "AliasProjection",
            AliasKind::Inherent => "AliasInherent",
            AliasKind::Opaque => "AliasOpaque",
            AliasKind::Free => "AliasFree",
        },
        TyKind::Param(_) => "Param",
        TyKind::Bound(..) => "Bound",
    }
}

fn type_scalar(kind: &TyKind) -> Option<String> {
    match kind {
        TyKind::RigidTy(RigidTy::Int(value)) => Some(int_kind(*value).to_owned()),
        TyKind::RigidTy(RigidTy::Uint(value)) => Some(uint_kind(*value).to_owned()),
        TyKind::RigidTy(RigidTy::Float(value)) => Some(float_kind(*value).to_owned()),
        TyKind::RigidTy(RigidTy::Tuple(values)) => Some(values.len().to_string()),
        TyKind::Param(param) => Some(format!("{}:{}", param.index, param.name)),
        TyKind::Bound(level, bound) => Some(format!("{level}:{}", bound.var)),
        _ => None,
    }
}

fn type_mutability(kind: &TyKind) -> Option<&'static str> {
    match kind {
        TyKind::RigidTy(RigidTy::RawPtr(_, value) | RigidTy::Ref(_, _, value)) => {
            Some(mutability(*value))
        }
        _ => None,
    }
}

fn type_components(kind: &TyKind) -> Vec<(&'static str, Ty)> {
    let mut result = Vec::new();
    match kind {
        TyKind::RigidTy(rigid) => match rigid {
            RigidTy::Array(ty, _)
            | RigidTy::Pat(ty, _)
            | RigidTy::Slice(ty)
            | RigidTy::RawPtr(ty, _)
            | RigidTy::Ref(_, ty, _) => result.push(("element", *ty)),
            RigidTy::Tuple(types) => {
                result.extend(types.iter().copied().map(|ty| ("tuple-element", ty)));
            }
            RigidTy::Adt(_, args)
            | RigidTy::FnDef(_, args)
            | RigidTy::Closure(_, args)
            | RigidTy::Coroutine(_, args)
            | RigidTy::CoroutineClosure(_, args)
            | RigidTy::CoroutineWitness(_, args) => {
                result.extend(
                    args.0
                        .iter()
                        .filter_map(|argument| argument.ty().copied())
                        .map(|ty| ("generic-type-argument", ty)),
                );
            }
            RigidTy::FnPtr(signature) => {
                result.extend(
                    signature
                        .value
                        .inputs()
                        .iter()
                        .copied()
                        .map(|ty| ("function-input", ty)),
                );
                result.push(("function-output", signature.value.output()));
            }
            RigidTy::Bool
            | RigidTy::Char
            | RigidTy::Int(_)
            | RigidTy::Uint(_)
            | RigidTy::Float(_)
            | RigidTy::Foreign(_)
            | RigidTy::Str
            | RigidTy::Dynamic(..)
            | RigidTy::Never => {}
        },
        TyKind::Alias(_, alias) => {
            result.extend(
                alias
                    .args
                    .0
                    .iter()
                    .filter_map(|argument| argument.ty().copied())
                    .map(|ty| ("alias-type-argument", ty)),
            );
        }
        TyKind::Param(_) | TyKind::Bound(..) => {}
    }
    result
}

fn type_definition(tcx: TyCtxt<'_>, kind: &TyKind) -> Option<(String, OwnedCompilerKey)> {
    match kind {
        TyKind::RigidTy(RigidTy::Adt(def, _)) => Some((def.name(), compiler_key(tcx, def))),
        TyKind::RigidTy(RigidTy::Foreign(def)) => Some((def.name(), compiler_key(tcx, def))),
        TyKind::RigidTy(RigidTy::FnDef(def, _)) => Some((def.name(), compiler_key(tcx, def))),
        TyKind::RigidTy(RigidTy::Closure(def, _)) => Some((def.name(), compiler_key(tcx, def))),
        TyKind::RigidTy(RigidTy::Coroutine(def, _)) => Some((def.name(), compiler_key(tcx, def))),
        TyKind::RigidTy(RigidTy::CoroutineClosure(def, _)) => {
            Some((def.name(), compiler_key(tcx, def)))
        }
        TyKind::RigidTy(RigidTy::CoroutineWitness(def, _)) => {
            Some((def.name(), compiler_key(tcx, def)))
        }
        TyKind::Alias(_, alias) => Some((alias.def_id.name(), compiler_key(tcx, &alias.def_id))),
        _ => None,
    }
}

const fn int_kind(kind: IntTy) -> &'static str {
    match kind {
        IntTy::Isize => "isize",
        IntTy::I8 => "i8",
        IntTy::I16 => "i16",
        IntTy::I32 => "i32",
        IntTy::I64 => "i64",
        IntTy::I128 => "i128",
    }
}

const fn uint_kind(kind: UintTy) -> &'static str {
    match kind {
        UintTy::Usize => "usize",
        UintTy::U8 => "u8",
        UintTy::U16 => "u16",
        UintTy::U32 => "u32",
        UintTy::U64 => "u64",
        UintTy::U128 => "u128",
    }
}

const fn float_kind(kind: FloatTy) -> &'static str {
    match kind {
        FloatTy::F16 => "f16",
        FloatTy::F32 => "f32",
        FloatTy::F64 => "f64",
        FloatTy::F128 => "f128",
    }
}

const fn mutability(value: Mutability) -> &'static str {
    match value {
        Mutability::Not => "not-mutable",
        Mutability::Mut => "mutable",
    }
}

fn region_kind(region: &Region) -> &'static str {
    match region.kind {
        RegionKind::ReEarlyParam(_) => "early-param",
        RegionKind::ReBound(..) => "bound",
        RegionKind::ReStatic => "static",
        RegionKind::RePlaceholder(_) => "placeholder",
        RegionKind::ReErased => "erased",
    }
}

fn binop_name(value: BinOp) -> &'static str {
    match value {
        BinOp::Add => "Add",
        BinOp::AddUnchecked => "AddUnchecked",
        BinOp::Sub => "Sub",
        BinOp::SubUnchecked => "SubUnchecked",
        BinOp::Mul => "Mul",
        BinOp::MulUnchecked => "MulUnchecked",
        BinOp::Div => "Div",
        BinOp::Rem => "Rem",
        BinOp::BitXor => "BitXor",
        BinOp::BitAnd => "BitAnd",
        BinOp::BitOr => "BitOr",
        BinOp::Shl => "Shl",
        BinOp::ShlUnchecked => "ShlUnchecked",
        BinOp::Shr => "Shr",
        BinOp::ShrUnchecked => "ShrUnchecked",
        BinOp::Eq => "Eq",
        BinOp::Lt => "Lt",
        BinOp::Le => "Le",
        BinOp::Ne => "Ne",
        BinOp::Ge => "Ge",
        BinOp::Gt => "Gt",
        BinOp::Cmp => "Cmp",
        BinOp::Offset => "Offset",
    }
}

fn unop_name(value: UnOp) -> &'static str {
    match value {
        UnOp::Not => "Not",
        UnOp::Neg => "Neg",
        UnOp::PtrMetadata => "PtrMetadata",
    }
}

fn cast_name(value: CastKind) -> &'static str {
    match value {
        CastKind::PointerExposeAddress => "PointerExposeAddress",
        CastKind::PointerWithExposedProvenance => "PointerWithExposedProvenance",
        CastKind::PointerCoercion(_) => "PointerCoercion",
        CastKind::IntToInt => "IntToInt",
        CastKind::FloatToInt => "FloatToInt",
        CastKind::FloatToFloat => "FloatToFloat",
        CastKind::IntToFloat => "IntToFloat",
        CastKind::PtrToPtr => "PtrToPtr",
        CastKind::FnPtrToPtr => "FnPtrToPtr",
        CastKind::Transmute => "Transmute",
        CastKind::BoxDerefTransmute => "BoxDerefTransmute",
        CastKind::Subtype => "Subtype",
    }
}

fn aggregate_name(value: &AggregateKind) -> &'static str {
    match value {
        AggregateKind::Array(_) => "Array",
        AggregateKind::Tuple => "Tuple",
        AggregateKind::Adt(..) => "Adt",
        AggregateKind::Closure(..) => "Closure",
        AggregateKind::Coroutine(..) => "Coroutine",
        AggregateKind::CoroutineClosure(..) => "CoroutineClosure",
        AggregateKind::RawPtr(..) => "RawPtr",
    }
}

fn instance_kind(value: InstanceKind) -> &'static str {
    match value {
        InstanceKind::Item => "Item",
        InstanceKind::Intrinsic => "Intrinsic",
        InstanceKind::LlvmIntrinsic => "LlvmIntrinsic",
        InstanceKind::Virtual { .. } => "Virtual",
        InstanceKind::Shim => "Shim",
    }
}

fn unwind_name(value: &UnwindAction) -> &'static str {
    match value {
        UnwindAction::Continue => "Continue",
        UnwindAction::Unreachable => "Unreachable",
        UnwindAction::Terminate => "Terminate",
        UnwindAction::Cleanup(_) => "Cleanup",
    }
}

fn unwind_target(value: &UnwindAction) -> Option<usize> {
    match value {
        UnwindAction::Cleanup(target) => Some(*target),
        UnwindAction::Continue | UnwindAction::Unreachable | UnwindAction::Terminate => None,
    }
}

fn assert_message_name(value: &AssertMessage) -> &'static str {
    match value {
        AssertMessage::BoundsCheck { .. } => "BoundsCheck",
        AssertMessage::Overflow(..) => "Overflow",
        AssertMessage::OverflowNeg(..) => "OverflowNeg",
        AssertMessage::DivisionByZero(..) => "DivisionByZero",
        AssertMessage::RemainderByZero(..) => "RemainderByZero",
        AssertMessage::ResumedAfterReturn(..) => "ResumedAfterReturn",
        AssertMessage::ResumedAfterPanic(..) => "ResumedAfterPanic",
        AssertMessage::ResumedAfterDrop(..) => "ResumedAfterDrop",
        AssertMessage::MisalignedPointerDereference { .. } => "MisalignedPointerDereference",
        AssertMessage::NullPointerDereference => "NullPointerDereference",
        AssertMessage::NullReferenceConstructed => "NullReferenceConstructed",
        AssertMessage::InvalidEnumConstruction(..) => "InvalidEnumConstruction",
    }
}

struct MirExtractor<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    owner_key: OwnedCompilerKey,
    relations: &'a mut OwnerRelations,
    seen_types: &'a mut BTreeSet<[u8; 32]>,
}

impl MirExtractor<'_, '_> {
    fn register_type(&mut self, ty: Ty) -> [u8; 32] {
        TypeCollector {
            tcx: self.tcx,
            owner: self.relations,
            seen: std::mem::take(self.seen_types),
        }
        .register_and_restore(ty, self.seen_types)
    }

    fn place_id(&self, location: Location, role: &str, ordinal: usize, place: &Place) -> [u8; 32] {
        let mut fields = vec![
            self.owner_key.stable_crate_id.to_be_bytes().to_vec(),
            self.owner_key.def_path_hash.to_vec(),
            location.block.to_be_bytes().to_vec(),
            location.slot_kind.as_bytes().to_vec(),
            location.slot_index.to_be_bytes().to_vec(),
            role.as_bytes().to_vec(),
            ordinal.to_be_bytes().to_vec(),
            place.local.to_be_bytes().to_vec(),
        ];
        for projection in &place.projection {
            let mut value = projection_kind(projection).as_bytes().to_vec();
            match projection {
                ProjectionElem::Field(index, ty) => {
                    value.extend_from_slice(&index.to_be_bytes());
                    value.extend_from_slice(&type_key(self.tcx, *ty));
                }
                ProjectionElem::Index(local) => value.extend_from_slice(&local.to_be_bytes()),
                ProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => {
                    value.extend_from_slice(&offset.to_be_bytes());
                    value.extend_from_slice(&min_length.to_be_bytes());
                    value.push(u8::from(*from_end));
                }
                ProjectionElem::Subslice { from, to, from_end } => {
                    value.extend_from_slice(&from.to_be_bytes());
                    value.extend_from_slice(&to.to_be_bytes());
                    value.push(u8::from(*from_end));
                }
                ProjectionElem::Downcast(index) => {
                    value.extend_from_slice(&index.to_index().to_be_bytes());
                }
                ProjectionElem::OpaqueCast(ty) => value.extend_from_slice(&type_key(self.tcx, *ty)),
                ProjectionElem::Deref => {}
            }
            fields.push(value);
        }
        framed_hash(b"codefabric.rustc.place-occurrence.v1\0", fields)
    }

    fn emit_place(
        &mut self,
        body: &rustc_public::mir::Body,
        location: Location,
        role: &'static str,
        ordinal: usize,
        place: &Place,
    ) -> ([u8; 32], Option<[u8; 32]>) {
        let id = self.place_id(location, role, ordinal, place);
        let place_ty = place
            .ty(body.locals())
            .ok()
            .map(|ty| self.register_type(ty));
        if place.projection.is_empty() {
            self.relations.push(
                RustcRelation::MirPlace,
                place_row(id, location, role, ordinal, place.local)
                    .utf8("projection_kind", "BaseLocal"),
            );
        } else {
            for (projection_ordinal, projection) in place.projection.iter().enumerate() {
                let mut row = place_row(id, location, role, ordinal, place.local)
                    .u64("projection_ordinal", projection_ordinal)
                    .utf8("projection_kind", projection_kind(projection));
                match projection {
                    ProjectionElem::Deref => {}
                    ProjectionElem::Field(index, ty) => {
                        row = row
                            .u64("projection_local_or_field", *index)
                            .fixed32("projection_type_key", self.register_type(*ty));
                    }
                    ProjectionElem::Index(local) => {
                        row = row.u64("projection_local_or_field", *local);
                    }
                    ProjectionElem::ConstantIndex {
                        offset,
                        min_length,
                        from_end,
                    } => {
                        row = row
                            .u64("offset", *offset)
                            .u64("min_length", *min_length)
                            .boolean("from_end", *from_end);
                    }
                    ProjectionElem::Subslice { from, to, from_end } => {
                        row = row
                            .u64("offset", *from)
                            .u64("slice_to", *to)
                            .boolean("from_end", *from_end);
                    }
                    ProjectionElem::Downcast(index) => {
                        row = row.u64("projection_local_or_field", index.to_index());
                    }
                    ProjectionElem::OpaqueCast(ty) => {
                        row = row.fixed32("projection_type_key", self.register_type(*ty));
                    }
                }
                self.relations.push(RustcRelation::MirPlace, row);
            }
        }
        (id, place_ty)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the relation keeps exact MIR location, role, place, and effect dimensions"
    )]
    fn emit_access(
        &mut self,
        body: &rustc_public::mir::Body,
        location: Location,
        ordinal: usize,
        role: &'static str,
        place: &Place,
        kind: &'static str,
        evidence: &'static str,
        runtime_effect: bool,
    ) -> [u8; 32] {
        let (place_id, type_key) = self.emit_place(body, location, role, ordinal, place);
        self.relations.push(
            RustcRelation::Access,
            OwnedRow::default()
                .u64("block_index", location.block)
                .utf8("slot_kind", location.slot_kind)
                .u64("slot_index", location.slot_index)
                .u64("access_ordinal", ordinal)
                .fixed32("place_id", place_id)
                .utf8("access_kind", kind)
                .maybe_fixed32("type_key", type_key)
                .utf8("structured_evidence", evidence)
                .boolean("runtime_effect", runtime_effect),
        );
        place_id
    }

    fn emit_operand(
        &mut self,
        body: &rustc_public::mir::Body,
        location: Location,
        parent_role: &'static str,
        ordinal: usize,
        operand: &Operand,
    ) -> [u8; 32] {
        let type_key = operand
            .ty(body.locals())
            .ok()
            .map(|ty| self.register_type(ty));
        if type_key.is_none() {
            self.relations.unknown_operand_types += 1;
        }
        let (kind, place_id, constant_kind, runtime_check_kind) = match operand {
            Operand::Copy(place) => (
                "Copy",
                Some(self.emit_access(
                    body,
                    location,
                    ordinal,
                    "operand-copy",
                    place,
                    "Copy",
                    "Operand::Copy",
                    true,
                )),
                None,
                None,
            ),
            Operand::Move(place) => (
                "Move",
                Some(self.emit_access(
                    body,
                    location,
                    ordinal,
                    "operand-move",
                    place,
                    "Move",
                    "Operand::Move",
                    true,
                )),
                None,
                None,
            ),
            Operand::Constant(constant) => (
                "Constant",
                None,
                Some(constant_kind(&constant.const_)),
                None,
            ),
            Operand::RuntimeChecks(check) => {
                ("RuntimeChecks", None, None, Some(runtime_check_kind(check)))
            }
        };
        let id = framed_hash(
            b"codefabric.rustc.operand-occurrence.v1\0",
            [
                self.owner_key.stable_crate_id.to_be_bytes().to_vec(),
                self.owner_key.def_path_hash.to_vec(),
                location.block.to_be_bytes().to_vec(),
                location.slot_kind.as_bytes().to_vec(),
                location.slot_index.to_be_bytes().to_vec(),
                parent_role.as_bytes().to_vec(),
                ordinal.to_be_bytes().to_vec(),
                kind.as_bytes().to_vec(),
                type_key.map_or_else(|| b"unknown-type".to_vec(), |key| key.to_vec()),
            ],
        );
        self.relations.push(
            RustcRelation::MirOperand,
            OwnedRow::default()
                .fixed32("operand_id", id)
                .u64("block_index", location.block)
                .utf8("slot_kind", location.slot_kind)
                .u64("slot_index", location.slot_index)
                .utf8("parent_role", parent_role)
                .u64("operand_ordinal", ordinal)
                .utf8("operand_kind", kind)
                .maybe_fixed32("place_id", place_id)
                .maybe_fixed32("type_key", type_key)
                .maybe_utf8("constant_kind", constant_kind)
                .maybe_utf8("runtime_check_kind", runtime_check_kind),
        );
        id
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match mirrors the pinned rustc_public Rvalue sum type"
    )]
    fn emit_rvalue(&mut self, body: &rustc_public::mir::Body, location: Location, rvalue: &Rvalue) {
        let result_type = rvalue
            .ty(body.locals())
            .ok()
            .map(|ty| self.register_type(ty));
        let mut operator = None;
        let mut cast = None;
        let mut aggregate = None;
        let mut source_place = None;
        let mut region = None;
        let mut mutable = None;
        let operand_count;
        match rvalue {
            Rvalue::AddressOf(kind, place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-address-of",
                    place,
                    raw_ptr_access(*kind),
                    "Rvalue::AddressOf",
                    true,
                ));
                mutable = Some(raw_ptr_mutability(*kind));
                operand_count = 0;
            }
            Rvalue::Aggregate(kind, operands) => {
                aggregate = Some(aggregate_name(kind));
                for (ordinal, operand) in operands.iter().enumerate() {
                    self.emit_operand(body, location, "rvalue-aggregate", ordinal, operand);
                }
                operand_count = operands.len();
            }
            Rvalue::BinaryOp(kind, left, right) | Rvalue::CheckedBinaryOp(kind, left, right) => {
                operator = Some(binop_name(*kind));
                self.emit_operand(body, location, "rvalue-binary", 0, left);
                self.emit_operand(body, location, "rvalue-binary", 1, right);
                operand_count = 2;
            }
            Rvalue::Cast(kind, operand, _) => {
                cast = Some(cast_name(*kind));
                self.emit_operand(body, location, "rvalue-cast", 0, operand);
                operand_count = 1;
            }
            Rvalue::CopyForDeref(place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-copy-for-deref",
                    place,
                    "CopyForDeref",
                    "Rvalue::CopyForDeref",
                    true,
                ));
                operand_count = 0;
            }
            Rvalue::Discriminant(place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-discriminant",
                    place,
                    "DiscriminantRead",
                    "Rvalue::Discriminant",
                    true,
                ));
                operand_count = 0;
            }
            Rvalue::Len(place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-len",
                    place,
                    "LengthRead",
                    "Rvalue::Len",
                    true,
                ));
                operand_count = 0;
            }
            Rvalue::Ref(value, borrow, place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-ref",
                    place,
                    borrow_access(*borrow),
                    "Rvalue::Ref",
                    true,
                ));
                region = Some(region_kind(value));
                mutable = Some(borrow_mutability(*borrow));
                operand_count = 0;
            }
            Rvalue::Repeat(operand, _) => {
                self.emit_operand(body, location, "rvalue-repeat", 0, operand);
                operand_count = 1;
            }
            Rvalue::ThreadLocalRef(_) => operand_count = 0,
            Rvalue::UnaryOp(kind, operand) => {
                operator = Some(unop_name(*kind));
                self.emit_operand(body, location, "rvalue-unary", 0, operand);
                operand_count = 1;
            }
            Rvalue::Use(operand, _) => {
                self.emit_operand(body, location, "rvalue-use", 0, operand);
                operand_count = 1;
            }
            Rvalue::Reborrow(_, value, place) => {
                source_place = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "rvalue-reborrow",
                    place,
                    if *value == Mutability::Mut {
                        "ReborrowMut"
                    } else {
                        "ReborrowShared"
                    },
                    "Rvalue::Reborrow",
                    true,
                ));
                mutable = Some(mutability(*value));
                operand_count = 0;
            }
        }
        self.relations.push(
            RustcRelation::MirRvalue,
            OwnedRow::default()
                .u64("block_index", location.block)
                .u64("statement_index", location.slot_index)
                .utf8("rvalue_kind", rvalue_kind(rvalue))
                .maybe_fixed32("result_type_key", result_type)
                .maybe_utf8("operator_kind", operator)
                .maybe_utf8("cast_kind", cast)
                .maybe_utf8("aggregate_kind", aggregate)
                .u64("operand_count", operand_count)
                .maybe_fixed32("source_place_id", source_place)
                .maybe_utf8("region_kind", region)
                .maybe_utf8("mutability", mutable),
        );
    }

    fn emit_statement(
        &mut self,
        body: &rustc_public::mir::Body,
        block: usize,
        statement_index: usize,
        statement: &rustc_public::mir::Statement,
    ) {
        let location = Location {
            block,
            slot_kind: "statement",
            slot_index: statement_index,
        };
        let (raw_kind, effect) = statement_kind(&statement.kind);
        let span = owned_span(self.tcx, statement.source_info.span);
        self.relations.push(
            RustcRelation::MirStatement,
            OwnedRow::default()
                .u64("block_index", block)
                .u64("statement_index", statement_index)
                .utf8("raw_statement_kind", raw_kind)
                .utf8("normalized_effect", effect)
                .u64("source_scope", statement.source_info.scope)
                .span(&span),
        );
        match &statement.kind {
            StatementKind::Assign(destination, rvalue) => {
                self.emit_access(
                    body,
                    location,
                    0,
                    "assignment-destination",
                    destination,
                    "Write",
                    "StatementKind::Assign.destination",
                    true,
                );
                self.emit_rvalue(body, location, rvalue);
            }
            StatementKind::FakeRead(cause, place) => {
                self.emit_access(
                    body,
                    location,
                    0,
                    "fake-read",
                    place,
                    fake_read_access(cause),
                    "StatementKind::FakeRead",
                    false,
                );
            }
            StatementKind::SetDiscriminant { place, .. } => {
                self.emit_access(
                    body,
                    location,
                    0,
                    "set-discriminant",
                    place,
                    "DiscriminantWrite",
                    "StatementKind::SetDiscriminant",
                    true,
                );
            }
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                let place = Place::from(*local);
                self.emit_access(
                    body,
                    location,
                    0,
                    "storage-marker",
                    &place,
                    if matches!(statement.kind, StatementKind::StorageLive(_)) {
                        "StorageLive"
                    } else {
                        "StorageDead"
                    },
                    "StatementKind::StorageMarker",
                    false,
                );
            }
            StatementKind::PlaceMention(place) | StatementKind::AscribeUserType { place, .. } => {
                self.emit_access(
                    body,
                    location,
                    0,
                    "metadata-place",
                    place,
                    "UnknownUse",
                    raw_kind,
                    false,
                );
            }
            StatementKind::Intrinsic(intrinsic) => self.emit_intrinsic(body, location, intrinsic),
            StatementKind::Coverage(_) | StatementKind::ConstEvalCounter | StatementKind::Nop => {}
        }
    }

    fn emit_intrinsic(
        &mut self,
        body: &rustc_public::mir::Body,
        location: Location,
        intrinsic: &NonDivergingIntrinsic,
    ) {
        match intrinsic {
            NonDivergingIntrinsic::Assume(operand) => {
                self.emit_operand(body, location, "intrinsic-assume", 0, operand);
            }
            NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                for (ordinal, operand) in
                    [&copy.src, &copy.dst, &copy.count].into_iter().enumerate()
                {
                    self.emit_operand(
                        body,
                        location,
                        "intrinsic-copy-nonoverlapping",
                        ordinal,
                        operand,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn emit_terminator(
        &mut self,
        body: &rustc_public::mir::Body,
        block: usize,
        terminator: &rustc_public::mir::Terminator,
    ) {
        let location = Location {
            block,
            slot_kind: "terminator",
            slot_index: 0,
        };
        let span = owned_span(self.tcx, terminator.source_info.span);
        let mut normal_count = 0_usize;
        let mut unwind = None;
        let mut assertion = None;
        let mut destination = None;
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                self.emit_cfg(block, *target, "Normal", None, None);
                normal_count = 1;
            }
            TerminatorKind::SwitchInt { discr, targets } => {
                self.emit_operand(body, location, "switch-discriminant", 0, discr);
                for (value, target) in targets.branches() {
                    self.emit_cfg(block, target, "Case", Some(value.to_string()), None);
                    normal_count += 1;
                }
                self.emit_cfg(block, targets.otherwise(), "Default", None, None);
                normal_count += 1;
            }
            TerminatorKind::Resume
            | TerminatorKind::Abort
            | TerminatorKind::Return
            | TerminatorKind::Unreachable => {}
            TerminatorKind::Drop {
                place,
                target,
                unwind: action,
            } => {
                self.emit_access(
                    body,
                    location,
                    0,
                    "drop-place",
                    place,
                    "Drop",
                    "TerminatorKind::Drop",
                    true,
                );
                self.emit_cfg(block, *target, "DropReturn", None, None);
                normal_count = 1;
                self.emit_unwind_cfg(block, action);
                unwind = Some(unwind_name(action));
            }
            TerminatorKind::Call {
                func,
                args,
                destination: place,
                target,
                unwind: action,
            } => {
                let callable = self.emit_operand(body, location, "callable", 0, func);
                for (ordinal, argument) in args.iter().enumerate() {
                    self.emit_operand(body, location, "call-argument", ordinal, argument);
                }
                destination = Some(self.emit_access(
                    body,
                    location,
                    0,
                    "call-destination",
                    place,
                    "Write",
                    "TerminatorKind::Call.destination",
                    true,
                ));
                if let Some(target) = target {
                    self.emit_cfg(block, *target, "CallReturn", None, None);
                    normal_count = 1;
                }
                self.emit_unwind_cfg(block, action);
                unwind = Some(unwind_name(action));
                self.emit_call(
                    body,
                    block,
                    func,
                    callable,
                    args.len(),
                    destination.expect("set above"),
                    *target,
                    action,
                );
            }
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                unwind: action,
            } => {
                self.emit_operand(body, location, "assert-condition", 0, cond);
                self.emit_cfg(
                    block,
                    *target,
                    if *expected {
                        "AssertExpectedTrue"
                    } else {
                        "AssertExpectedFalse"
                    },
                    None,
                    None,
                );
                normal_count = 1;
                self.emit_unwind_cfg(block, action);
                unwind = Some(unwind_name(action));
                assertion = Some(assert_message_name(msg));
            }
            TerminatorKind::InlineAsm {
                operands,
                destination: target,
                unwind: action,
                ..
            } => {
                for (ordinal, operand) in operands.iter().enumerate() {
                    if let Some(input) = &operand.in_value {
                        self.emit_operand(body, location, "inline-asm-input", ordinal, input);
                    }
                    if let Some(output) = &operand.out_place {
                        self.emit_access(
                            body,
                            location,
                            ordinal,
                            "inline-asm-output",
                            output,
                            "Write",
                            "TerminatorKind::InlineAsm.output",
                            true,
                        );
                    }
                }
                if let Some(target) = target {
                    self.emit_cfg(block, *target, "InlineAsmReturn", None, None);
                    normal_count = 1;
                }
                self.emit_unwind_cfg(block, action);
                unwind = Some(unwind_name(action));
            }
        }
        self.relations.push(
            RustcRelation::MirTerminator,
            OwnedRow::default()
                .u64("block_index", block)
                .utf8("raw_terminator_kind", terminator_kind(&terminator.kind))
                .u64("source_scope", terminator.source_info.scope)
                .u64("normal_target_count", normal_count)
                .maybe_utf8("unwind_action", unwind)
                .maybe_utf8("assert_message_kind", assertion)
                .maybe_fixed32("destination_place_id", destination)
                .span(&span),
        );
    }

    fn emit_cfg(
        &mut self,
        source: usize,
        target: usize,
        kind: &'static str,
        branch: Option<String>,
        unwind: Option<&'static str>,
    ) {
        self.relations.push(
            RustcRelation::CfgEdge,
            OwnedRow::default()
                .u64("source_block", source)
                .u64("target_block", target)
                .utf8("edge_kind", kind)
                .maybe_utf8("branch_value_u128", branch)
                .maybe_utf8("unwind_action", unwind),
        );
    }

    fn emit_unwind_cfg(&mut self, source: usize, action: &UnwindAction) {
        if let Some(target) = unwind_target(action) {
            self.emit_cfg(source, target, "Unwind", None, Some(unwind_name(action)));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_call(
        &mut self,
        body: &rustc_public::mir::Body,
        block: usize,
        func: &Operand,
        callable_operand: [u8; 32],
        argument_count: usize,
        destination: [u8; 32],
        target: Option<usize>,
        unwind: &UnwindAction,
    ) {
        let mut declared_path = None;
        let mut declared_key = None;
        let mut resolved_key = None;
        let mut dispatch = "Unknown";
        let mut confidence = "UNRESOLVED";
        if let Ok(ty) = func.ty(body.locals()) {
            match ty.kind() {
                TyKind::RigidTy(RigidTy::FnDef(definition, args)) => {
                    let key = compiler_key(self.tcx, &definition);
                    declared_path = Some(definition.name());
                    declared_key = Some(key);
                    dispatch = "Direct";
                    confidence = "EXACT_DECLARATION_ONLY";
                    match Instance::resolve(definition, &args) {
                        Ok(instance) => {
                            resolved_key = Some(self.emit_instance(definition, &args, instance));
                            confidence = "EXACT_INSTANCE";
                        }
                        Err(_) => self.relations.unresolved_calls += 1,
                    }
                }
                TyKind::RigidTy(RigidTy::FnPtr(_)) => {
                    dispatch = "FunctionPointer";
                    self.relations.unresolved_calls += 1;
                }
                TyKind::RigidTy(RigidTy::Closure(..)) => {
                    dispatch = "Closure";
                    self.relations.unresolved_calls += 1;
                }
                TyKind::RigidTy(RigidTy::Dynamic(..)) => {
                    dispatch = "DynamicTrait";
                    self.relations.unresolved_calls += 1;
                }
                _ => self.relations.unresolved_calls += 1,
            }
        } else {
            self.relations.unresolved_calls += 1;
        }
        let mut row = OwnedRow::default()
            .u64("block_index", block)
            .fixed32("callable_operand_id", callable_operand)
            .u64("argument_count", argument_count)
            .fixed32("destination_place_id", destination)
            .maybe_u64("normal_target", target)
            .maybe_u64("unwind_target", unwind_target(unwind))
            .maybe_utf8("declared_target", declared_path)
            .maybe_fixed32("resolved_instance_key", resolved_key)
            .utf8("dispatch_kind", dispatch)
            .utf8("resolution_confidence", confidence);
        if let Some(key) = declared_key {
            row = row
                .u64("declared_stable_crate_id", key.stable_crate_id)
                .fixed16("declared_def_path_hash", key.def_path_hash);
        }
        self.relations.push(RustcRelation::Call, row);
    }

    fn emit_instance(
        &mut self,
        definition: rustc_public::ty::FnDef,
        args: &GenericArgs,
        instance: Instance,
    ) -> [u8; 32] {
        let definition_key = compiler_key(self.tcx, &definition);
        let specialized_type = self.register_type(instance.ty());
        let key = framed_hash(
            b"codefabric.rustc.instance.v1\0",
            [
                definition_key.stable_crate_id.to_be_bytes().to_vec(),
                definition_key.def_path_hash.to_vec(),
                specialized_type.to_vec(),
                instance_kind(instance.kind).as_bytes().to_vec(),
            ],
        );
        self.relations.push(
            RustcRelation::Instance,
            OwnedRow::default()
                .fixed32("instance_key", key)
                .utf8("definition_path", definition.name())
                .u64("definition_stable_crate_id", definition_key.stable_crate_id)
                .fixed16("definition_def_path_hash", definition_key.def_path_hash)
                .utf8("instance_kind", instance_kind(instance.kind))
                .u64("generic_argument_count", args.0.len())
                .fixed32("specialized_type_key", specialized_type)
                .boolean("has_body", instance.has_body())
                .boolean("is_foreign_item", instance.is_foreign_item())
                .utf8("mangled_name", instance.mangled_name())
                .utf8("resolution_state", "resolved"),
        );
        key
    }
}

impl TypeCollector<'_, '_> {
    fn register_and_restore(mut self, ty: Ty, destination: &mut BTreeSet<[u8; 32]>) -> [u8; 32] {
        let result = self.register(ty);
        *destination = self.seen;
        result
    }
}

fn place_row(
    id: [u8; 32],
    location: Location,
    role: &'static str,
    ordinal: usize,
    base_local: usize,
) -> OwnedRow {
    OwnedRow::default()
        .fixed32("place_id", id)
        .u64("block_index", location.block)
        .utf8("slot_kind", location.slot_kind)
        .u64("slot_index", location.slot_index)
        .utf8("occurrence_role", role)
        .u64("occurrence_ordinal", ordinal)
        .u64("base_local", base_local)
}

fn projection_kind(value: &ProjectionElem) -> &'static str {
    match value {
        ProjectionElem::Deref => "Deref",
        ProjectionElem::Field(..) => "Field",
        ProjectionElem::Index(..) => "Index",
        ProjectionElem::ConstantIndex { .. } => "ConstantIndex",
        ProjectionElem::Subslice { .. } => "Subslice",
        ProjectionElem::Downcast(..) => "Downcast",
        ProjectionElem::OpaqueCast(..) => "OpaqueCast",
    }
}

fn rvalue_kind(value: &Rvalue) -> &'static str {
    match value {
        Rvalue::AddressOf(..) => "AddressOf",
        Rvalue::Aggregate(..) => "Aggregate",
        Rvalue::BinaryOp(..) => "BinaryOp",
        Rvalue::Cast(..) => "Cast",
        Rvalue::CheckedBinaryOp(..) => "CheckedBinaryOp",
        Rvalue::CopyForDeref(..) => "CopyForDeref",
        Rvalue::Discriminant(..) => "Discriminant",
        Rvalue::Len(..) => "Len",
        Rvalue::Ref(..) => "Ref",
        Rvalue::Repeat(..) => "Repeat",
        Rvalue::ThreadLocalRef(..) => "ThreadLocalRef",
        Rvalue::UnaryOp(..) => "UnaryOp",
        Rvalue::Use(..) => "Use",
        Rvalue::Reborrow(..) => "Reborrow",
    }
}

fn constant_kind(value: &rustc_public::ty::MirConst) -> &'static str {
    match value.kind() {
        rustc_public::ty::ConstantKind::Ty(_) => "Ty",
        rustc_public::ty::ConstantKind::Allocated(_) => "Allocated",
        rustc_public::ty::ConstantKind::Unevaluated(_) => "Unevaluated",
        rustc_public::ty::ConstantKind::Param(_) => "Param",
        rustc_public::ty::ConstantKind::ZeroSized => "ZeroSized",
    }
}

fn runtime_check_kind(value: &RuntimeChecks) -> &'static str {
    match value {
        RuntimeChecks::UbChecks => "UbChecks",
        RuntimeChecks::ContractChecks => "ContractChecks",
        RuntimeChecks::OverflowChecks => "OverflowChecks",
    }
}

fn raw_ptr_access(value: RawPtrKind) -> &'static str {
    match value {
        RawPtrKind::Mut => "AddressOfMut",
        RawPtrKind::Const => "AddressOfConst",
        RawPtrKind::FakeForPtrMetadata => "AddressOfMetadata",
    }
}

fn raw_ptr_mutability(value: RawPtrKind) -> &'static str {
    match value {
        RawPtrKind::Mut => "mutable",
        RawPtrKind::Const | RawPtrKind::FakeForPtrMetadata => "not-mutable",
    }
}

fn borrow_access(value: BorrowKind) -> &'static str {
    match value {
        BorrowKind::Shared => "BorrowShared",
        BorrowKind::Fake(_) => "BorrowFake",
        BorrowKind::Mut { .. } => "BorrowMut",
    }
}

fn borrow_mutability(value: BorrowKind) -> &'static str {
    match value {
        BorrowKind::Mut { .. } => "mutable",
        BorrowKind::Shared | BorrowKind::Fake(_) => "not-mutable",
    }
}

fn fake_read_access(value: &FakeReadCause) -> &'static str {
    match value {
        FakeReadCause::ForMatchGuard => "FakeReadMatchGuard",
        FakeReadCause::ForMatchedPlace(_) => "FakeReadMatchedPlace",
        FakeReadCause::ForGuardBinding => "FakeReadGuardBinding",
        FakeReadCause::ForLet(_) => "FakeReadLet",
        FakeReadCause::ForIndex => "FakeReadIndex",
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one callback-local owner builder keeps rustc values from escaping the adapter"
)]
fn build_item_owner(tcx: TyCtxt<'_>, item: rustc_public::CrateItem) -> OwnedRustcOwner {
    let key = compiler_key(tcx, &item);
    let mut relations = OwnerRelations::requested(ITEM_NATIVE_FAMILIES);
    let mut seen_types = BTreeSet::new();
    let item_type_key = TypeCollector {
        tcx,
        owner: &mut relations,
        seen: std::mem::take(&mut seen_types),
    }
    .register_and_restore(item.ty(), &mut seen_types);
    let span = owned_span(tcx, item.span());
    relations.push(
        RustcRelation::PublicItem,
        OwnedRow::default()
            .utf8("qualified_name", item.name())
            .utf8("item_kind", item_kind(item.kind()))
            .boolean("has_body", item.has_body())
            .boolean("is_foreign_item", item.is_foreign_item())
            .boolean(
                "requires_monomorphization",
                item.requires_monomorphization(),
            )
            .fixed32("type_key", item_type_key)
            .span(&span)
            .boolean("in_external_macro", span.in_external_macro),
    );

    if let Some(body) = item.body() {
        let body_span = owned_span(tcx, body.span);
        relations.push(
            RustcRelation::MirBody,
            OwnedRow::default()
                .u64("block_count", body.blocks.len())
                .u64("local_count", body.locals().len())
                .u64("argument_count", body.arg_locals().len())
                .u64("debug_variable_count", body.var_debug_info.len())
                .maybe_u64("spread_argument_local", body.spread_arg())
                .span(&body_span),
        );
        for (local_index, local) in body.local_decls() {
            let local_span = owned_span(tcx, local.span);
            let local_type_key = TypeCollector {
                tcx,
                owner: &mut relations,
                seen: std::mem::take(&mut seen_types),
            }
            .register_and_restore(local.ty, &mut seen_types);
            relations.push(
                RustcRelation::MirLocal,
                OwnedRow::default()
                    .u64("local_index", local_index)
                    .utf8(
                        "local_role",
                        if local_index == 0 {
                            "return"
                        } else if local_index <= body.arg_locals().len() {
                            "argument"
                        } else {
                            "inner"
                        },
                    )
                    .fixed32("type_key", local_type_key)
                    .utf8("mutability", mutability(local.mutability))
                    .span(&local_span),
            );
        }
        for (block_index, block) in body.blocks.iter().enumerate() {
            relations.push(
                RustcRelation::MirBlock,
                OwnedRow::default()
                    .u64("block_index", block_index)
                    .u64("statement_count", block.statements.len())
                    .utf8("terminator_kind", terminator_kind(&block.terminator.kind))
                    .boolean("is_entry", block_index == 0),
            );
            let mut extractor = MirExtractor {
                tcx,
                owner_key: key,
                relations: &mut relations,
                seen_types: &mut seen_types,
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                extractor.emit_statement(&body, block_index, statement_index, statement);
            }
            extractor.emit_terminator(&body, block_index, &block.terminator);
        }
    }

    if relations.unresolved_calls > 0 {
        relations.push(
            RustcRelation::Remainder,
            OwnedRow::default()
                .utf8("fact_family", "provider.rustc.call.v1")
                .utf8("reason_code", "INDIRECT_OR_UNRESOLVED_CALL_TARGET")
                .utf8("authority_surface", "rustc_public::Instance::resolve")
                .boolean("bounded", true)
                .utf8(
                    "detail",
                    "the callable occurrence is retained and no target is invented",
                ),
        );
    }
    if relations.unknown_operand_types > 0 {
        relations.push(
            RustcRelation::Remainder,
            OwnedRow::default()
                .utf8("fact_family", "provider.rustc.mir_operand.v1")
                .utf8("reason_code", "PUBLIC_OPERAND_TYPE_UNAVAILABLE")
                .utf8("authority_surface", "rustc_public::mir::Operand::ty")
                .boolean("bounded", true)
                .utf8(
                    "detail",
                    "the operand occurrence and native variant remain present; no type is invented",
                ),
        );
    }
    OwnedRustcOwner {
        qualified_name: item.name(),
        owner_kind: "MIR_BODY".to_owned(),
        compiler_key: Some(key),
        relations: relations.finish(),
    }
}

fn extract_inside_callback(tcx: TyCtxt<'_>) -> ControlFlow<(), OwnedRustcExtraction> {
    let local_crate = rustc_public::local_crate();
    let mut items = rustc_public::all_local_items();
    items.sort_by_key(CrateDef::name);
    let body_owner_count = items.iter().filter(|item| item.has_body()).count();
    let mut compilation = OwnerRelations::requested(COMPILATION_NATIVE_FAMILIES);
    compilation.push(
        RustcRelation::Compilation,
        OwnedRow::default()
            .utf8("crate_name", &local_crate.name)
            .boolean("is_local_crate", local_crate.is_local)
            .u64("local_item_count", items.len())
            .u64("body_owner_count", body_owner_count)
            .utf8("rustc_release", RUSTC_PUBLIC_RELEASE)
            .utf8("rustc_toolchain", RUSTC_TOOLCHAIN)
            .utf8(
                "stable_identity_authority",
                "TyCtxt::def_path_hash(StableCrateId+DefPathHash)",
            )
            .utf8(
                "source_hygiene_authority",
                "rustc_private Span byte offsets and expansion context",
            ),
    );
    for (family, reason, surface, bounded, detail) in [
        (
            "borrowck-loans",
            "PRIVATE_BORROWCK_ENRICHMENT_NOT_SELECTED",
            "rustc_borrowck consumer API",
            false,
            "raw public MIR borrows are emitted; exact loan/region facts remain an explicit gap",
        ),
        (
            "mono-vtable-closure",
            "FULL_MONO_COLLECTOR_NOT_SELECTED",
            "rustc_monomorphize collector",
            false,
            "direct Instance resolution is emitted; full vtable and mono-use closure is not claimed",
        ),
        (
            "compiler-diagnostics",
            "STRUCTURED_DIAGNOSTIC_SINK_NOT_SELECTED",
            "rustc_driver diagnostic emitter",
            false,
            "terminal compiler status remains control metadata; structured diagnostics are not fabricated",
        ),
        (
            "derived-flow-analyses",
            "APPLICATION_ANALYSIS_OWNED_OUTSIDE_PROVIDER",
            "CodeFabric WP24 analysis release",
            true,
            "reaching definitions, liveness, alias, ownership state, drop/resource, and async analyses are not rustc provider facts",
        ),
    ] {
        compilation.push(
            RustcRelation::Remainder,
            OwnedRow::default()
                .utf8("fact_family", family)
                .utf8("reason_code", reason)
                .utf8("authority_surface", surface)
                .boolean("bounded", bounded)
                .utf8("detail", detail),
        );
    }
    compilation.push(
        RustcRelation::Coverage,
        OwnedRow::default()
            .utf8("fact_family", RustcRelation::Diagnostic.relation_id())
            .utf8(
                "authority_surface",
                authority_surface(RustcRelation::Diagnostic),
            )
            .u64("requested_units", 1)
            .u64("completed_units", 0)
            .u64("emitted_rows", 0)
            .utf8("completeness", "unavailable-characterized")
            .u64("remainder_count", 1)
            .boolean("unknown_semantics", true),
    );
    let mut owners = vec![OwnedRustcOwner {
        qualified_name: local_crate.name.clone(),
        owner_kind: "COMPILATION".to_owned(),
        compiler_key: None,
        relations: compilation.finish(),
    }];
    owners.extend(items.into_iter().map(|item| build_item_owner(tcx, item)));
    ControlFlow::Continue(OwnedRustcExtraction { owners })
}

#[allow(
    clippy::unnested_or_patterns,
    reason = "the pinned rustc_public::run_with_tcx! macro expands the unnested pattern"
)]
pub(crate) fn extract_owned(rustc_args: &[String]) -> Result<OwnedRustcExtraction, String> {
    rustc_public::run_with_tcx!(rustc_args, extract_inside_callback)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_rows_have_no_opaque_payload_cell() {
        let row = OwnedRow::default()
            .utf8("fact_family", "mir")
            .u64("emitted_rows", 1)
            .boolean("unknown_semantics", false)
            .fixed16("def_path_hash", [1; 16])
            .fixed32("type_key", [2; 32]);
        assert_eq!(row.0.len(), 5);
    }

    #[test]
    fn relation_family_is_complete_and_distinct() {
        let codes = RustcRelation::ALL
            .into_iter()
            .map(RustcRelation::family_code)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), RustcRelation::ALL.len());
        assert!(!codes.contains(&120));
    }

    #[test]
    fn zero_fact_native_family_keeps_complete_owner_coverage() {
        let relations = OwnerRelations::requested([RustcRelation::Call]).finish();
        let call = relations
            .iter()
            .find(|relation| relation.relation == RustcRelation::Call)
            .unwrap();
        assert_eq!(call.rows.len(), 0);

        let coverage = relations
            .iter()
            .find(|relation| relation.relation == RustcRelation::Coverage)
            .and_then(|relation| relation.rows.first())
            .unwrap();
        assert_eq!(
            coverage.0.get("fact_family"),
            Some(&OwnedCell::Utf8(
                RustcRelation::Call.relation_id().to_owned()
            ))
        );
        assert_eq!(
            coverage.0.get("requested_units"),
            Some(&OwnedCell::UInt64(1))
        );
        assert_eq!(
            coverage.0.get("completed_units"),
            Some(&OwnedCell::UInt64(1))
        );
        assert_eq!(coverage.0.get("emitted_rows"), Some(&OwnedCell::UInt64(0)));
        assert_eq!(
            coverage.0.get("completeness"),
            Some(&OwnedCell::Utf8("complete".to_owned()))
        );
    }
}
