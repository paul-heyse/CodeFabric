//! Narrow compiler-library seam for the Wave 0 link smoke.
//!
//! Compiler-owned values never leave the `rustc_public` callback. Later
//! extraction packets replace the owned count with versioned application DTOs.

#[cfg(test)]
use std::ops::ControlFlow;

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
