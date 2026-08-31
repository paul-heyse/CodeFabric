//! Application-owned schemas for the Pyrefly relation-stream boundary.
//!
//! This file is compiled into both independent Cargo roots. It intentionally contains only
//! Arrow schema vocabulary and stable relation identities: Pyrefly types never cross it.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};

/// Exact Pyrefly release bound by the sidecar Cargo lock.
pub(crate) const PYREFLY_RELEASE: &str = "1.2.0";
/// Exact Pyrefly source revision bound by the sidecar Cargo lock.
pub(crate) const PYREFLY_REVISION: &str = "1933169ad8ee9e4d4114112eb56ef0811fb0a094";
/// Version of the relation-stream schemas, independent from Pyrefly's unstable API.
pub(crate) const PYREFLY_RELATION_PROTOCOL_VERSION: u16 = 1;
/// Exact Arrow public type and IPC metadata universe shared with the stable daemon.
pub(crate) const ARROW_TYPE_UNIVERSE: &str =
    "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5";

/// One semantic relation per Arrow IPC stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum PyreflyRelation {
    ModuleContext = 111,
    TypeShape = 112,
    TypeComponent = 113,
    TypeTrait = 114,
    LocatedType = 115,
    CallTarget = 116,
    Member = 117,
    Diagnostic = 118,
    AffectedModule = 119,
    Coverage = 120,
}

impl PyreflyRelation {
    pub(crate) const ALL: [Self; 10] = [
        Self::ModuleContext,
        Self::TypeShape,
        Self::TypeComponent,
        Self::TypeTrait,
        Self::LocatedType,
        Self::CallTarget,
        Self::Member,
        Self::Diagnostic,
        Self::AffectedModule,
        Self::Coverage,
    ];

    pub const fn family_code(self) -> u32 {
        self as u32
    }

    pub const fn relation_id(self) -> &'static str {
        match self {
            Self::ModuleContext => "provider.pyrefly.module_context.v1",
            Self::TypeShape => "provider.pyrefly.type_shape.v1",
            Self::TypeComponent => "provider.pyrefly.type_component.v1",
            Self::TypeTrait => "provider.pyrefly.type_trait.v1",
            Self::LocatedType => "provider.pyrefly.located_type.v1",
            Self::CallTarget => "provider.pyrefly.call_target.v1",
            Self::Member => "provider.pyrefly.member.v1",
            Self::Diagnostic => "provider.pyrefly.diagnostic.v1",
            Self::AffectedModule => "provider.pyrefly.affected_module.v1",
            Self::Coverage => "provider.pyrefly.coverage.v1",
        }
    }

    #[allow(dead_code)] // The stable daemon decodes codes; the independent producer does not.
    pub(crate) fn from_family_code(code: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.family_code() == code)
    }

    pub(crate) fn schema(self) -> SchemaRef {
        let descriptor = self.canonical_descriptor();
        let digest = digest(descriptor.as_bytes());
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
                PYREFLY_RELATION_PROTOCOL_VERSION.to_string(),
            ),
            (
                "codefabric.provider_release".to_owned(),
                PYREFLY_RELEASE.to_owned(),
            ),
            (
                "codefabric.provider_revision".to_owned(),
                PYREFLY_REVISION.to_owned(),
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
            PYREFLY_RELATION_PROTOCOL_VERSION,
            ARROW_TYPE_UNIVERSE,
            PYREFLY_RELEASE,
            PYREFLY_REVISION,
            fields
        )
    }

    fn field_specs(self) -> Vec<FieldSpec> {
        let mut fields = common_fields();
        let specific = match self {
            Self::ModuleContext => vec![
                utf8("module_name", false),
                utf8("provider_release", false),
                utf8("provider_revision", false),
                utf8("requested_module_require_tier", false),
                utf8("dependency_require_tier", false),
                u64_field("source_byte_length", false),
                bool_field("long_lived_context", false),
            ],
            Self::TypeShape => vec![
                u64_field("local_type_index", false),
                u64_field("structural_hash", false),
                utf8("shape_kind", false),
                utf8("name", true),
                u64_field("unspecified_type_arg_count", true),
                bool_field("is_staticmethod", true),
            ],
            Self::TypeComponent => vec![
                u64_field("owner_local_type_index", false),
                utf8("component_role", false),
                u64_field("component_ordinal", false),
                u64_field("referenced_local_type_index", false),
            ],
            Self::TypeTrait => vec![
                u64_field("owner_local_type_index", false),
                utf8("trait_kind", false),
            ],
            Self::LocatedType => vec![
                u64_field("occurrence_ordinal", false),
                u64_field("start_byte", false),
                u64_field("end_byte", false),
                u64_field("local_type_index", false),
                utf8("type_role", false),
                u64_field("provider_start_line", false),
                u64_field("provider_start_column", false),
                u64_field("provider_end_line", false),
                u64_field("provider_end_column", false),
            ],
            Self::CallTarget => vec![
                u64_field("call_occurrence_ordinal", false),
                u64_field("start_byte", false),
                u64_field("end_byte", false),
                u64_field("target_ordinal", false),
                utf8("callee_kind", false),
                utf8("qualified_target", false),
                utf8("class_name", true),
                utf8("resolution_state", false),
            ],
            Self::Member => vec![
                utf8("class_name", false),
                u64_field("member_ordinal", false),
                utf8("member_name", false),
                utf8("member_kind", true),
                utf8("annotation_rendering", false),
                utf8("annotation_representation", false),
                bool_field("is_final", false),
                utf8("discovery_basis", false),
            ],
            Self::Diagnostic => vec![
                u64_field("diagnostic_ordinal", false),
                utf8("rendered_text", false),
                bool_field("structured_fields_available", false),
                bool_field("source_locator_redacted", false),
            ],
            Self::AffectedModule => vec![
                utf8("affected_module_id", false),
                utf8("evidence_source", false),
                bool_field("exact_recheck_proven", false),
                utf8("refresh_policy", false),
            ],
            Self::Coverage => vec![
                utf8("fact_family", false),
                utf8("exact_authority_surface", false),
                u64_field("requested_units", false),
                u64_field("completed_units", false),
                u64_field("emitted_rows", false),
                utf8("completeness", false),
                utf8("remainder_reason", true),
                bool_field("unknown_semantics", false),
            ],
        };
        fields.extend(specific);
        fields
    }
}

pub(crate) fn schema_digests() -> Vec<String> {
    PyreflyRelation::ALL
        .into_iter()
        .map(PyreflyRelation::schema_digest)
        .collect()
}

/// Deterministic identity of the complete Pyrefly provider-native schema bundle.
///
/// Both independent Cargo roots compile this source file, so the daemon and sidecar derive the
/// run-level handshake identity independently from the closed relation set and exact Arrow
/// universe instead of accepting an arbitrary caller-supplied digest.
pub(crate) fn schema_bundle_digest() -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.pyrefly.relation-schema-bundle.v1\0");
    for relation in PyreflyRelation::ALL {
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
        utf8("analysis_context_id", false),
        utf8("module_id", false),
        utf8("file_id", false),
        fixed_binary("content_digest", 32, false),
        fixed_binary("semantic_environment_id", 32, false),
        u64_field("source_generation", false),
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

fn data_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Utf8 => "utf8",
        DataType::UInt64 => "uint64",
        DataType::Boolean => "boolean",
        DataType::FixedSizeBinary(32) => "fixed_size_binary[32]",
        _ => unreachable!("the Pyrefly relation contract uses a closed Arrow type set"),
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
    fn relation_contracts_are_closed_unique_and_typed() {
        let mut codes = BTreeSet::new();
        let mut identifiers = BTreeSet::new();
        let mut digests = BTreeSet::new();
        for relation in PyreflyRelation::ALL {
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
        assert_eq!(codes.len(), PyreflyRelation::ALL.len());
        assert!(schema_bundle_digest().starts_with("b3:"));
        assert_eq!(schema_bundle_digest().len(), 67);
    }
}
