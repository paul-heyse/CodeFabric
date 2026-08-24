//! Compiler-private seam that converts `rustc_public` values to owned protocol DTOs.
//!
//! No compiler-owned value crosses the callback. The stable daemon can retain, hash, transport,
//! and compare only the application-owned records below.

use std::ops::ControlFlow;

use rustc_public::mir::{StatementKind, TerminatorKind};
use rustc_public::{CrateDef, CtorKind, ItemKind};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnedMirItem {
    pub name: String,
    pub item_kind: String,
    pub type_description: String,
    pub requires_monomorphization: bool,
    pub basic_block_count: usize,
    pub local_count: usize,
    pub statement_kinds: Vec<String>,
    pub terminator_kinds: Vec<String>,
    pub successor_count: usize,
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

fn item_kind(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Fn => "function",
        ItemKind::Static => "static",
        ItemKind::Const => "constant",
        ItemKind::Ctor(CtorKind::Const) => "constructor-constant",
        ItemKind::Ctor(CtorKind::Fn) => "constructor-function",
    }
}

fn statement_kind(kind: &StatementKind) -> &'static str {
    match kind {
        StatementKind::Assign(..) => "assign",
        StatementKind::FakeRead(..) => "fake-read",
        StatementKind::SetDiscriminant { .. } => "set-discriminant",
        StatementKind::StorageLive(..) => "storage-live",
        StatementKind::StorageDead(..) => "storage-dead",
        StatementKind::PlaceMention(..) => "place-mention",
        StatementKind::AscribeUserType { .. } => "ascribe-user-type",
        StatementKind::Coverage(..) => "coverage",
        StatementKind::Intrinsic(..) => "intrinsic",
        StatementKind::ConstEvalCounter => "const-eval-counter",
        StatementKind::Nop => "nop",
    }
}

fn terminator_kind(kind: &TerminatorKind) -> &'static str {
    match kind {
        TerminatorKind::Goto { .. } => "goto",
        TerminatorKind::SwitchInt { .. } => "switch-int",
        TerminatorKind::Resume => "resume",
        TerminatorKind::Abort => "abort",
        TerminatorKind::Return => "return",
        TerminatorKind::Unreachable => "unreachable",
        TerminatorKind::Drop { .. } => "drop",
        TerminatorKind::Call { .. } => "call",
        TerminatorKind::Assert { .. } => "assert",
        TerminatorKind::InlineAsm { .. } => "inline-asm",
    }
}

fn extract_inside_callback() -> ControlFlow<(), Vec<OwnedMirItem>> {
    let mut output = rustc_public::all_local_items()
        .into_iter()
        .map(|item| {
            let body = item.body();
            let mut statement_kinds = Vec::new();
            let mut terminator_kinds = Vec::new();
            let mut successor_count = 0_usize;
            let (basic_block_count, local_count) = body.as_ref().map_or((0, 0), |body| {
                for block in &body.blocks {
                    statement_kinds.extend(
                        block
                            .statements
                            .iter()
                            .map(|statement| statement_kind(&statement.kind).to_owned()),
                    );
                    terminator_kinds.push(terminator_kind(&block.terminator.kind).to_owned());
                    successor_count += block.terminator.successors().len();
                }
                (body.blocks.len(), body.locals().len())
            });
            OwnedMirItem {
                name: item.name().clone(),
                item_kind: item_kind(item.kind()).to_owned(),
                type_description: format!("{:?}", item.ty()),
                requires_monomorphization: item.requires_monomorphization(),
                basic_block_count,
                local_count,
                statement_kinds,
                terminator_kinds,
                successor_count,
            }
        })
        .collect::<Vec<_>>();
    output.sort_by(|left, right| left.name.cmp(&right.name));
    ControlFlow::Continue(output)
}

#[allow(
    clippy::unnested_or_patterns,
    reason = "the pinned rustc_public::run! macro expands the unnested pattern"
)]
pub(crate) fn extract_owned(rustc_args: &[String]) -> Result<Vec<OwnedMirItem>, String> {
    rustc_public::run!(rustc_args, extract_inside_callback).map_err(|error| format!("{error:?}"))
}
