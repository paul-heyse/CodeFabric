//! Scalar type distinctions from the public compiler API. No rendered type is decoded here.

use rustc_public::ty::{Abi, AdtKind, RigidTy, TyKind};

use super::{OwnedRow, float_kind, int_kind, region_kind, uint_kind};

pub(super) fn extend(mut row: OwnedRow, kind: &TyKind) -> OwnedRow {
    let (arguments, bound) = match kind {
        TyKind::RigidTy(
            RigidTy::Adt(_, args)
            | RigidTy::FnDef(_, args)
            | RigidTy::Closure(_, args)
            | RigidTy::Coroutine(_, args)
            | RigidTy::CoroutineClosure(_, args)
            | RigidTy::CoroutineWitness(_, args),
        ) => (args.0.len(), 0),
        TyKind::Alias(_, alias) => (alias.args.0.len(), 0),
        TyKind::RigidTy(RigidTy::FnPtr(signature)) => (0, signature.bound_vars.len()),
        _ => (0, 0),
    };
    row = row
        .u64("generic_argument_count", arguments)
        .u64("bound_variable_count", bound);
    match kind {
        TyKind::RigidTy(rigid) => match rigid {
            RigidTy::Bool => row.utf8("primitive_kind", "bool"),
            RigidTy::Char => row.utf8("primitive_kind", "char"),
            RigidTy::Str => row.utf8("primitive_kind", "str"),
            RigidTy::Int(value) => row.utf8("primitive_kind", int_kind(*value)),
            RigidTy::Uint(value) => row.utf8("primitive_kind", uint_kind(*value)),
            RigidTy::Float(value) => row.utf8("primitive_kind", float_kind(*value)),
            RigidTy::Array(_, length) => {
                row.maybe_u64("array_length", length.eval_target_usize().ok())
            }
            RigidTy::Ref(region, _, _) => row.utf8("region_kind", region_kind(region)),
            RigidTy::Adt(definition, _) => row.utf8(
                "definition_kind",
                match definition.kind() {
                    AdtKind::Struct => "struct",
                    AdtKind::Enum => "enum",
                    AdtKind::Union => "union",
                },
            ),
            RigidTy::FnDef(..) => row.utf8("definition_kind", "function"),
            RigidTy::Foreign(..) => row.utf8("definition_kind", "foreign-type"),
            RigidTy::Closure(..) => row.utf8("definition_kind", "closure"),
            RigidTy::Coroutine(..) => row.utf8("definition_kind", "coroutine"),
            RigidTy::CoroutineClosure(..) => row.utf8("definition_kind", "coroutine-closure"),
            RigidTy::FnPtr(signature) => {
                let (abi, unwind) = abi(&signature.value.abi);
                let row = row
                    .utf8("function_abi", abi)
                    .boolean(
                        "function_unsafe",
                        matches!(signature.value.safety, rustc_public::mir::Safety::Unsafe),
                    )
                    .boolean("function_variadic", signature.value.c_variadic);
                match unwind {
                    Some(unwind) => row.boolean("function_abi_unwind", unwind),
                    None => row,
                }
            }
            RigidTy::Pat(..)
            | RigidTy::Slice(..)
            | RigidTy::RawPtr(..)
            | RigidTy::Dynamic(..)
            | RigidTy::Never
            | RigidTy::Tuple(..)
            | RigidTy::CoroutineWitness(..) => row,
        },
        TyKind::Alias(..) | TyKind::Param(_) | TyKind::Bound(..) => row,
    }
}

// Exact ABI discriminants are schema values. The unwind bit is present only for native ABI
// variants that carry it; it is not a conclusion about whether a particular call unwinds.
fn abi(value: &Abi) -> (&'static str, Option<bool>) {
    match value {
        Abi::Rust => ("rust", None),
        Abi::C { unwind } => ("c", Some(*unwind)),
        Abi::Cdecl { unwind } => ("cdecl", Some(*unwind)),
        Abi::Stdcall { unwind } => ("stdcall", Some(*unwind)),
        Abi::Fastcall { unwind } => ("fastcall", Some(*unwind)),
        Abi::Vectorcall { unwind } => ("vectorcall", Some(*unwind)),
        Abi::Thiscall { unwind } => ("thiscall", Some(*unwind)),
        Abi::Aapcs { unwind } => ("aapcs", Some(*unwind)),
        Abi::Win64 { unwind } => ("win64", Some(*unwind)),
        Abi::SysV64 { unwind } => ("sysv64", Some(*unwind)),
        Abi::PtxKernel => ("ptx-kernel", None),
        Abi::Msp430Interrupt => ("msp430-interrupt", None),
        Abi::X86Interrupt => ("x86-interrupt", None),
        Abi::GpuKernel => ("gpu-kernel", None),
        Abi::EfiApi => ("efiapi", None),
        Abi::AvrInterrupt => ("avr-interrupt", None),
        Abi::AvrNonBlockingInterrupt => ("avr-non-blocking-interrupt", None),
        Abi::CCmseNonSecureCall => ("c-cmse-non-secure-call", None),
        Abi::CCmseNonSecureEntry => ("c-cmse-non-secure-entry", None),
        Abi::System { unwind } => ("system", Some(*unwind)),
        Abi::RustCall => ("rust-call", None),
        Abi::Unadjusted => ("unadjusted", None),
        Abi::RustCold => ("rust-cold", None),
        Abi::RiscvInterruptM => ("riscv-interrupt-m", None),
        Abi::RiscvInterruptS => ("riscv-interrupt-s", None),
        Abi::RustPreserveNone => ("rust-preserve-none", None),
        Abi::RustTail => ("rust-tail", None),
        Abi::RustInvalid => ("rust-invalid", None),
        Abi::Custom => ("custom", None),
        Abi::Swift => ("swift", None),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OwnedCell, RustcRelation, extract_owned};
    use std::collections::BTreeSet;

    #[test]
    fn native_types_preserve_array_lengths_abi_safety_and_generic_census() {
        let root =
            std::env::temp_dir().join(format!("codefabric-rust-types-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("types.rs");
        std::fs::write(
            &source,
            r#"
pub struct Wrap<T, const N: usize> { pub values: [T; N] }
pub static REF: &u16 = &0;
pub fn types(a: [u8; 2], b: [u8; 3], r: &'static u16, p: *mut u16,
    f: unsafe extern "C" fn(u8) -> u16,
    g: unsafe extern "C-unwind" fn(u8) -> u16,
    h: fn(u8) -> u16, w: Wrap<u8, 4>,
    higher: for<'a> fn(&'a u8)) -> (bool, char, f32, &'static str) {
    (true, 'x', 1.0, "value")
}
"#,
        )
        .unwrap();
        let sysroot = std::process::Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).unwrap();
        let result = extract_owned(&[
            "rustc".to_owned(),
            source.display().to_string(),
            "--crate-name=type_shapes".to_owned(),
            "--crate-type=lib".to_owned(),
            "--edition=2024".to_owned(),
            "--emit=metadata".to_owned(),
            "--cap-lints=allow".to_owned(),
            format!("--out-dir={}", root.display()),
            format!("--sysroot={}", sysroot.trim()),
        ]);
        assert!(result.compiler_succeeded);
        let rows = result
            .owners
            .iter()
            .flat_map(|owner| &owner.relations)
            .filter(|relation| relation.relation == RustcRelation::Type)
            .flat_map(|relation| &relation.rows)
            .filter(|row| row.0.get("component_role") == Some(&OwnedCell::Utf8("self".to_owned())))
            .collect::<Vec<_>>();
        for length in [2, 3] {
            assert!(
                rows.iter()
                    .any(|row| row.0.get("array_length") == Some(&OwnedCell::UInt64(length)))
            );
        }
        assert!(rows.iter().any(|row| row.0.get("definition_kind")
            == Some(&OwnedCell::Utf8("struct".to_owned()))
            && row.0.get("generic_argument_count") == Some(&OwnedCell::UInt64(2))));
        assert!(
            rows.iter()
                .any(|row| row.0.get("bound_variable_count") == Some(&OwnedCell::UInt64(1)))
        );
        let mut variants = BTreeSet::new();
        let mut static_references = BTreeSet::new();
        let mut erased_references = BTreeSet::new();
        for row in rows {
            if let Some(OwnedCell::Utf8(region)) = row.0.get("region_kind") {
                let Some(OwnedCell::Fixed32(key)) = row.0.get("type_key") else {
                    panic!("missing type key");
                };
                match region.as_str() {
                    "static" => {
                        static_references.insert(*key);
                    }
                    "erased" => {
                        erased_references.insert(*key);
                    }
                    _ => {}
                }
            }
            if let Some(OwnedCell::Utf8(abi)) = row.0.get("function_abi") {
                let unwind = match row.0.get("function_abi_unwind") {
                    Some(OwnedCell::Boolean(value)) => Some(*value),
                    None => None,
                    _ => panic!("invalid unwind flag"),
                };
                let unsafe_fn = row.0.get("function_unsafe") == Some(&OwnedCell::Boolean(true));
                variants.insert((abi.as_str(), unwind, unsafe_fn));
            }
        }
        assert!(!static_references.is_empty());
        assert!(!erased_references.is_empty());
        assert!(
            static_references.is_disjoint(&erased_references),
            "TypeId-style region erasure must not merge native graph nodes"
        );
        for expected in [
            ("c", Some(false), true),
            ("c", Some(true), true),
            ("rust", None, false),
        ] {
            assert!(variants.contains(&expected), "{variants:?}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
