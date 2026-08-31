//! Application-owned Arrow schemas for the rustc relation-stream boundary.
//!
//! This file is compiled into both Cargo roots. It contains no compiler-owned type: the dated
//! nightly adapter may change while these relation identities remain the stable process boundary.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};

/// Exact compiler release bound by `rustc-extractor/rust-toolchain.toml`.
pub(crate) const RUSTC_PUBLIC_RELEASE: &str = "1.100.0-nightly";
/// Exact dated-nightly channel bound by the extractor Cargo root.
pub(crate) const RUSTC_TOOLCHAIN: &str = "nightly-2026-08-18";
/// Version of the application relation contract, independent of rustc's API version.
pub(crate) const RUSTC_RELATION_PROTOCOL_VERSION: u16 = 1;
/// Exact Arrow public type and IPC metadata universe shared with the stable daemon.
pub(crate) const ARROW_TYPE_UNIVERSE: &str =
    "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5";

/// One compiler-native fact relation per Arrow IPC stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum RustcRelation {
    Compilation = 121,
    PublicItem = 122,
    Type = 123,
    Instance = 124,
    MirBody = 125,
    MirBlock = 126,
    MirLocal = 127,
    MirPlace = 128,
    MirOperand = 129,
    MirRvalue = 130,
    MirStatement = 131,
    MirTerminator = 132,
    CfgEdge = 133,
    Call = 134,
    Access = 135,
    Diagnostic = 136,
    Coverage = 137,
    Remainder = 138,
}

impl RustcRelation {
    pub(crate) const ALL: [Self; 18] = [
        Self::Compilation,
        Self::PublicItem,
        Self::Type,
        Self::Instance,
        Self::MirBody,
        Self::MirBlock,
        Self::MirLocal,
        Self::MirPlace,
        Self::MirOperand,
        Self::MirRvalue,
        Self::MirStatement,
        Self::MirTerminator,
        Self::CfgEdge,
        Self::Call,
        Self::Access,
        Self::Diagnostic,
        Self::Coverage,
        Self::Remainder,
    ];

    pub const fn family_code(self) -> u32 {
        self as u32
    }

    pub const fn relation_id(self) -> &'static str {
        match self {
            Self::Compilation => "provider.rustc.compilation.v1",
            Self::PublicItem => "provider.rustc.public_item.v1",
            Self::Type => "provider.rustc.type.v1",
            Self::Instance => "provider.rustc.instance.v1",
            Self::MirBody => "provider.rustc.mir_body.v1",
            Self::MirBlock => "provider.rustc.mir_block.v1",
            Self::MirLocal => "provider.rustc.mir_local.v1",
            Self::MirPlace => "provider.rustc.mir_place.v1",
            Self::MirOperand => "provider.rustc.mir_operand.v1",
            Self::MirRvalue => "provider.rustc.mir_rvalue.v1",
            Self::MirStatement => "provider.rustc.mir_statement.v1",
            Self::MirTerminator => "provider.rustc.mir_terminator.v1",
            Self::CfgEdge => "provider.rustc.cfg_edge.v1",
            Self::Call => "provider.rustc.call.v1",
            Self::Access => "provider.rustc.access.v1",
            Self::Diagnostic => "provider.rustc.diagnostic.v1",
            Self::Coverage => "provider.rustc.coverage.v1",
            Self::Remainder => "provider.rustc.remainder.v1",
        }
    }

    pub(crate) fn from_family_code(code: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.family_code() == code)
    }

    pub(crate) fn schema(self) -> SchemaRef {
        let digest = self.schema_digest();
        let fields = self
            .field_specs()
            .into_iter()
            .enumerate()
            .map(|(ordinal, spec)| {
                Field::new(spec.name, spec.data_type, spec.nullable).with_metadata(
                    [
                        (
                            "codefabric.field_id".to_owned(),
                            format!("{}.{}", self.relation_id(), spec.name),
                        ),
                        ("codefabric.field_ordinal".to_owned(), ordinal.to_string()),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect::<Vec<_>>();
        let metadata = [
            (
                "codefabric.relation_id".to_owned(),
                self.relation_id().to_owned(),
            ),
            ("codefabric.schema_digest".to_owned(), digest),
            (
                "codefabric.relation_protocol_version".to_owned(),
                RUSTC_RELATION_PROTOCOL_VERSION.to_string(),
            ),
            (
                "codefabric.provider_release".to_owned(),
                RUSTC_PUBLIC_RELEASE.to_owned(),
            ),
            (
                "codefabric.provider_toolchain".to_owned(),
                RUSTC_TOOLCHAIN.to_owned(),
            ),
            (
                "codefabric.semantic_encoding".to_owned(),
                "typed-arrow-relation-stream".to_owned(),
            ),
            (
                "codefabric.arrow_type_universe".to_owned(),
                ARROW_TYPE_UNIVERSE.to_owned(),
            ),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        Arc::new(Schema::new_with_metadata(fields, metadata))
    }

    pub(crate) fn schema_digest(self) -> String {
        digest(self.canonical_descriptor().as_bytes())
    }

    fn canonical_descriptor(self) -> String {
        let fields = self
            .field_specs()
            .into_iter()
            .map(|field| {
                format!(
                    "{}:{}:{}",
                    field.name,
                    data_type_name(&field.data_type),
                    if field.nullable {
                        "nullable"
                    } else {
                        "required"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "{}|protocol={}|arrow={}|provider={}@{}|{}",
            self.relation_id(),
            RUSTC_RELATION_PROTOCOL_VERSION,
            ARROW_TYPE_UNIVERSE,
            RUSTC_PUBLIC_RELEASE,
            RUSTC_TOOLCHAIN,
            fields
        )
    }

    #[allow(clippy::too_many_lines)]
    fn field_specs(self) -> Vec<FieldSpec> {
        let mut fields = common_fields();
        let specific = match self {
            Self::Compilation => vec![
                utf8("crate_name", false),
                bool_field("is_local_crate", false),
                u64_field("local_item_count", false),
                u64_field("body_owner_count", false),
                utf8("rustc_release", false),
                utf8("rustc_toolchain", false),
                utf8("stable_identity_authority", false),
                utf8("source_hygiene_authority", false),
            ],
            Self::PublicItem => vec![
                utf8("qualified_name", false),
                utf8("item_kind", false),
                bool_field("has_body", false),
                bool_field("is_foreign_item", false),
                bool_field("requires_monomorphization", false),
                fixed_binary("type_key", 32, false),
                span_file(),
                span_start(),
                span_end(),
                span_start_line(),
                span_end_line(),
                span_start_column(),
                span_end_column(),
                utf8("expansion_kind", false),
                bool_field("in_external_macro", false),
            ],
            Self::Type => vec![
                fixed_binary("type_key", 32, false),
                utf8("type_kind", false),
                utf8("definition_path", true),
                u64_field("definition_stable_crate_id", true),
                fixed_binary("definition_def_path_hash", 16, true),
                utf8("component_role", false),
                u64_field("component_ordinal", false),
                fixed_binary("component_type_key", 32, true),
                utf8("scalar_value", true),
                utf8("mutability", true),
            ],
            Self::Instance => vec![
                fixed_binary("instance_key", 32, false),
                utf8("definition_path", false),
                u64_field("definition_stable_crate_id", false),
                fixed_binary("definition_def_path_hash", 16, false),
                utf8("instance_kind", false),
                u64_field("generic_argument_count", false),
                fixed_binary("specialized_type_key", 32, false),
                bool_field("has_body", false),
                bool_field("is_foreign_item", false),
                utf8("mangled_name", true),
                utf8("resolution_state", false),
            ],
            Self::MirBody => vec![
                u64_field("block_count", false),
                u64_field("local_count", false),
                u64_field("argument_count", false),
                u64_field("debug_variable_count", false),
                u64_field("spread_argument_local", true),
                span_file(),
                span_start(),
                span_end(),
                span_start_line(),
                span_end_line(),
                span_start_column(),
                span_end_column(),
                utf8("expansion_kind", false),
            ],
            Self::MirBlock => vec![
                u64_field("block_index", false),
                u64_field("statement_count", false),
                utf8("terminator_kind", false),
                bool_field("is_entry", false),
            ],
            Self::MirLocal => vec![
                u64_field("local_index", false),
                utf8("local_role", false),
                fixed_binary("type_key", 32, false),
                utf8("mutability", false),
                span_file(),
                span_start(),
                span_end(),
                span_start_line(),
                span_end_line(),
                span_start_column(),
                span_end_column(),
                utf8("expansion_kind", false),
            ],
            Self::MirPlace => vec![
                fixed_binary("place_id", 32, false),
                u64_field("block_index", false),
                utf8("slot_kind", false),
                u64_field("slot_index", false),
                utf8("occurrence_role", false),
                u64_field("occurrence_ordinal", false),
                u64_field("base_local", false),
                u64_field("projection_ordinal", true),
                utf8("projection_kind", false),
                u64_field("projection_local_or_field", true),
                u64_field("offset", true),
                u64_field("min_length", true),
                u64_field("slice_to", true),
                bool_field("from_end", true),
                fixed_binary("projection_type_key", 32, true),
            ],
            Self::MirOperand => vec![
                fixed_binary("operand_id", 32, false),
                u64_field("block_index", false),
                utf8("slot_kind", false),
                u64_field("slot_index", false),
                utf8("parent_role", false),
                u64_field("operand_ordinal", false),
                utf8("operand_kind", false),
                fixed_binary("place_id", 32, true),
                fixed_binary("type_key", 32, true),
                utf8("constant_kind", true),
                utf8("runtime_check_kind", true),
            ],
            Self::MirRvalue => vec![
                u64_field("block_index", false),
                u64_field("statement_index", false),
                utf8("rvalue_kind", false),
                fixed_binary("result_type_key", 32, true),
                utf8("operator_kind", true),
                utf8("cast_kind", true),
                utf8("aggregate_kind", true),
                u64_field("operand_count", false),
                fixed_binary("source_place_id", 32, true),
                utf8("region_kind", true),
                utf8("mutability", true),
            ],
            Self::MirStatement => vec![
                u64_field("block_index", false),
                u64_field("statement_index", false),
                utf8("raw_statement_kind", false),
                utf8("normalized_effect", false),
                u64_field("source_scope", false),
                span_file(),
                span_start(),
                span_end(),
                span_start_line(),
                span_end_line(),
                span_start_column(),
                span_end_column(),
                utf8("expansion_kind", false),
            ],
            Self::MirTerminator => vec![
                u64_field("block_index", false),
                utf8("raw_terminator_kind", false),
                u64_field("source_scope", false),
                u64_field("normal_target_count", false),
                utf8("unwind_action", true),
                utf8("assert_message_kind", true),
                fixed_binary("destination_place_id", 32, true),
                span_file(),
                span_start(),
                span_end(),
                span_start_line(),
                span_end_line(),
                span_start_column(),
                span_end_column(),
                utf8("expansion_kind", false),
            ],
            Self::CfgEdge => vec![
                u64_field("source_block", false),
                u64_field("target_block", false),
                utf8("edge_kind", false),
                utf8("branch_value_u128", true),
                utf8("unwind_action", true),
            ],
            Self::Call => vec![
                u64_field("block_index", false),
                fixed_binary("callable_operand_id", 32, false),
                u64_field("argument_count", false),
                fixed_binary("destination_place_id", 32, false),
                u64_field("normal_target", true),
                u64_field("unwind_target", true),
                utf8("declared_target", true),
                u64_field("declared_stable_crate_id", true),
                fixed_binary("declared_def_path_hash", 16, true),
                fixed_binary("resolved_instance_key", 32, true),
                utf8("dispatch_kind", false),
                utf8("resolution_confidence", false),
            ],
            Self::Access => vec![
                u64_field("block_index", false),
                utf8("slot_kind", false),
                u64_field("slot_index", false),
                u64_field("access_ordinal", false),
                fixed_binary("place_id", 32, false),
                utf8("access_kind", false),
                fixed_binary("type_key", 32, true),
                utf8("structured_evidence", false),
                bool_field("runtime_effect", false),
            ],
            Self::Diagnostic => vec![
                u64_field("diagnostic_ordinal", false),
                utf8("severity", false),
                utf8("reason_code", false),
                utf8("message", false),
                bool_field("structured_compiler_diagnostic", false),
            ],
            Self::Coverage => vec![
                utf8("fact_family", false),
                utf8("authority_surface", false),
                u64_field("requested_units", false),
                u64_field("completed_units", false),
                u64_field("emitted_rows", false),
                utf8("completeness", false),
                u64_field("remainder_count", false),
                bool_field("unknown_semantics", false),
            ],
            Self::Remainder => vec![
                utf8("fact_family", false),
                utf8("reason_code", false),
                utf8("authority_surface", false),
                bool_field("bounded", false),
                utf8("detail", false),
            ],
        };
        fields.extend(specific);
        fields
    }
}

/// Deterministic identity of the complete rustc provider-native schema bundle.
///
/// Both independent Cargo roots compile this source file. The wrapper and daemon therefore
/// derive this value independently from the same closed relation set, including each schema's
/// exact Arrow-universe identity, rather than trusting a caller-supplied opaque digest.
pub(crate) fn schema_bundle_digest() -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.rustc.relation-schema-bundle.v1\0");
    for relation in RustcRelation::ALL {
        let schema_digest = relation.schema_digest();
        for bytes in [relation.relation_id().as_bytes(), schema_digest.as_bytes()] {
            hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
            hasher.update(bytes);
        }
    }
    format!("b3:{}", hasher.finalize().to_hex())
}

struct FieldSpec {
    name: &'static str,
    data_type: DataType,
    nullable: bool,
}

fn common_fields() -> Vec<FieldSpec> {
    vec![
        utf8("provider_run_id", false),
        utf8("compilation_unit_id", false),
        utf8("owner_id", false),
        u64_field("source_generation", false),
        utf8("source_file_id", false),
        fixed_binary("source_content_digest", 32, false),
        u64_field("stable_crate_id", true),
        fixed_binary("def_path_hash", 16, true),
    ]
}

const fn utf8(name: &'static str, nullable: bool) -> FieldSpec {
    FieldSpec {
        name,
        data_type: DataType::Utf8,
        nullable,
    }
}

const fn u64_field(name: &'static str, nullable: bool) -> FieldSpec {
    FieldSpec {
        name,
        data_type: DataType::UInt64,
        nullable,
    }
}

const fn bool_field(name: &'static str, nullable: bool) -> FieldSpec {
    FieldSpec {
        name,
        data_type: DataType::Boolean,
        nullable,
    }
}

const fn fixed_binary(name: &'static str, width: i32, nullable: bool) -> FieldSpec {
    FieldSpec {
        name,
        data_type: DataType::FixedSizeBinary(width),
        nullable,
    }
}

const fn span_file() -> FieldSpec {
    utf8("span_file", false)
}
const fn span_start() -> FieldSpec {
    u64_field("span_start_byte", false)
}
const fn span_end() -> FieldSpec {
    u64_field("span_end_byte", false)
}
const fn span_start_line() -> FieldSpec {
    u64_field("span_start_line", false)
}
const fn span_end_line() -> FieldSpec {
    u64_field("span_end_line", false)
}
const fn span_start_column() -> FieldSpec {
    u64_field("span_start_column", false)
}
const fn span_end_column() -> FieldSpec {
    u64_field("span_end_column", false)
}

fn data_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Utf8 => "utf8",
        DataType::UInt64 => "uint64",
        DataType::Boolean => "boolean",
        DataType::FixedSizeBinary(16) => "fixed_size_binary[16]",
        DataType::FixedSizeBinary(32) => "fixed_size_binary[32]",
        _ => unreachable!("the rustc relation contract uses a closed Arrow type set"),
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn relation_contracts_are_closed_unique_and_non_opaque() {
        let mut codes = BTreeSet::new();
        let mut identifiers = BTreeSet::new();
        let mut digests = BTreeSet::new();
        for relation in RustcRelation::ALL {
            assert!(codes.insert(relation.family_code()));
            assert!(identifiers.insert(relation.relation_id()));
            assert!(digests.insert(relation.schema_digest()));
            let schema = relation.schema();
            assert_eq!(
                schema.metadata().get("codefabric.semantic_encoding"),
                Some(&"typed-arrow-relation-stream".to_owned())
            );
            assert_eq!(
                schema.metadata().get("codefabric.arrow_type_universe"),
                Some(&ARROW_TYPE_UNIVERSE.to_owned())
            );
            assert!(schema.fields().iter().all(|field| {
                !matches!(field.data_type(), DataType::Binary | DataType::LargeBinary)
            }));
        }
        assert!(schema_bundle_digest().starts_with("b3:"));
        assert_eq!(schema_bundle_digest().len(), 67);
    }
}
