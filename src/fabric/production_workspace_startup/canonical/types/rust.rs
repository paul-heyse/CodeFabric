//! Rust type normalization consumes explicit native structure, never the provider's type hash.

use super::normalize::{TypeNode, text};
use crate::identity::{CbefField, CbefValue, TypeConstructor, TypeTerm};

mod graph;
mod relations;
#[cfg(test)]
mod tests;
mod udf;
pub(super) use graph::{GRAPH, build, fields};
pub(super) use relations::{components, observations};

pub(super) struct Node<'a> {
    pub index: u64,
    pub kind: &'a str,
    pub primitive: Option<&'a str>,
    pub definition: Option<[u8; 16]>,
    pub generic_argument_count: u64,
    pub array_length: Option<u64>,
    pub mutability: Option<&'a str>,
    pub region: Option<&'a str>,
    pub bound_variable_count: u64,
    pub function_abi: Option<&'a str>,
    pub function_abi_unwind: Option<bool>,
    pub function_unsafe: Option<bool>,
    pub function_variadic: Option<bool>,
    pub components: Vec<Component<'a>>,
}

pub(super) struct Component<'a> {
    pub role: &'a str,
    pub ordinal: u64,
    pub target: Option<u64>,
}

impl TypeNode for Node<'_> {
    fn index(&self) -> u64 {
        self.index
    }
    fn targets(&self) -> impl Iterator<Item = Option<u64>> {
        self.components.iter().map(|component| component.target)
    }
    fn term(&self, target: impl Fn(u64) -> Option<[u8; 16]>) -> Result<TypeTerm, &'static str> {
        term(self, target)
    }
    fn unknown_reason(&self) -> Option<&'static str> {
        (self.kind == "Ref" && self.region == Some("erased")).then_some("native_region_erased")
    }
}

fn field(tag: u16, value: CbefValue) -> CbefField {
    CbefField { tag, value }
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed Rust constructor mapping owns its versioned CBEF fields and native precision checks"
)]
fn term(
    node: &Node<'_>,
    target: impl Fn(u64) -> Option<[u8; 16]>,
) -> Result<TypeTerm, &'static str> {
    use TypeConstructor as C;
    if node.bound_variable_count != 0 {
        return Err("rust_type_binder_normalization_unavailable");
    }
    let mut components = node.components.iter().collect::<Vec<_>>();
    components.sort_by_key(|component| component.ordinal);
    let roles: &[&str] = match node.kind {
        "Tuple" => &["tuple-element"],
        "Array" | "Slice" | "RawPtr" | "Ref" => &["element"],
        "Adt" | "FnDef" => &["generic-type-argument"],
        "FnPtr" => &["function-input", "function-output"],
        _ => &[],
    };
    for (index, component) in components.iter().enumerate() {
        if component.ordinal != index as u64 + 1 {
            return Err("rust_type_component_ordinal_gap");
        }
        if !roles.contains(&component.role) {
            return Err("rust_type_component_role_unsupported");
        }
    }
    let generics = components
        .iter()
        .filter(|component| component.role == "generic-type-argument")
        .count() as u64;
    if generics != node.generic_argument_count {
        return Err("rust_non_type_generic_arguments_unavailable");
    }
    let children = |role| {
        components
            .iter()
            .filter(|component| component.role == role)
            .map(|component| {
                component
                    .target
                    .and_then(&target)
                    .map(CbefValue::Id)
                    .ok_or("type_component_unavailable")
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let one = |role| {
        let mut children = children(role)?;
        if children.len() == 1 {
            Ok(children.remove(0))
        } else {
            Err("rust_type_component_cardinality")
        }
    };
    let mut fields = vec![field(1, text("rust"))];
    let constructor = match node.kind {
        "Bool" | "Char" | "Str" | "Int" | "Uint" | "Float" => {
            let primitive = node.primitive.ok_or("rust_primitive_kind_unavailable")?;
            let valid = match node.kind {
                "Bool" => primitive == "bool",
                "Char" => primitive == "char",
                "Str" => primitive == "str",
                "Int" => ["i8", "i16", "i32", "i64", "i128", "isize"].contains(&primitive),
                "Uint" => ["u8", "u16", "u32", "u64", "u128", "usize"].contains(&primitive),
                "Float" => ["f16", "f32", "f64", "f128"].contains(&primitive),
                _ => false,
            };
            if !valid {
                return Err("rust_primitive_kind_unknown");
            }
            fields.push(field(2, text(primitive)));
            C::Primitive
        }
        "Never" => C::NeverBottom,
        "Tuple" => {
            fields.push(field(2, CbefValue::OrderedList(children("tuple-element")?)));
            C::Tuple
        }
        "Array" => {
            fields.push(field(2, one("element")?));
            fields.push(field(
                3,
                CbefValue::Unsigned(
                    node.array_length
                        .ok_or("rust_array_length_unavailable")?
                        .to_be_bytes()
                        .to_vec(),
                ),
            ));
            C::Array
        }
        "Slice" => {
            fields.push(field(2, one("element")?));
            C::Slice
        }
        "RawPtr" | "Ref" => {
            fields.push(field(2, one("element")?));
            let mutable = match node.mutability {
                Some("mutable") => true,
                Some("not-mutable") => false,
                _ => return Err("rust_mutability_unavailable"),
            };
            fields.push(field(3, CbefValue::Boolean(mutable)));
            if node.kind == "Ref" {
                let region = node
                    .region
                    .filter(|region| ["static", "erased"].contains(region))
                    .ok_or("rust_region_binder_unavailable")?;
                fields.push(field(4, text(region)));
                C::Reference
            } else {
                C::RawPointer
            }
        }
        "Adt" | "FnDef" => {
            fields.push(field(
                2,
                CbefValue::Id(node.definition.ok_or("nominal_definition_unavailable")?),
            ));
            fields.push(field(
                3,
                CbefValue::OrderedList(children("generic-type-argument")?),
            ));
            if node.kind == "FnDef" {
                C::FunctionDefinition
            } else if generics == 0 {
                C::Nominal
            } else {
                C::Generic
            }
        }
        "FnPtr" => {
            fields.push(field(
                2,
                CbefValue::OrderedList(children("function-input")?),
            ));
            fields.push(field(3, one("function-output")?));
            let abi = node.function_abi.ok_or("rust_function_abi_unavailable")?;
            let explicit_unwind = [
                "c",
                "cdecl",
                "stdcall",
                "fastcall",
                "vectorcall",
                "thiscall",
                "aapcs",
                "win64",
                "sysv64",
                "system",
            ]
            .contains(&abi);
            if explicit_unwind != node.function_abi_unwind.is_some() {
                return Err("rust_function_abi_unwind_unavailable");
            }
            if !explicit_unwind
                && ![
                    "rust",
                    "ptx-kernel",
                    "msp430-interrupt",
                    "x86-interrupt",
                    "gpu-kernel",
                    "efiapi",
                    "avr-interrupt",
                    "avr-non-blocking-interrupt",
                    "c-cmse-non-secure-call",
                    "c-cmse-non-secure-entry",
                    "rust-call",
                    "unadjusted",
                    "rust-cold",
                    "riscv-interrupt-m",
                    "riscv-interrupt-s",
                    "rust-preserve-none",
                    "rust-tail",
                    "swift",
                ]
                .contains(&abi)
            {
                return Err("rust_function_abi_unknown");
            }
            fields.push(field(4, text(abi)));
            fields.push(field(
                5,
                node.function_abi_unwind
                    .map_or(CbefValue::Absent, CbefValue::Boolean),
            ));
            fields.push(field(
                6,
                CbefValue::Boolean(
                    node.function_unsafe
                        .ok_or("rust_function_safety_unavailable")?,
                ),
            ));
            fields.push(field(
                7,
                CbefValue::Boolean(
                    node.function_variadic
                        .ok_or("rust_function_variadic_unavailable")?,
                ),
            ));
            C::FunctionPointer
        }
        _ => return Err("rust_type_constructor_normalization_unavailable"),
    };
    Ok(TypeTerm {
        constructor,
        fields,
    })
}
