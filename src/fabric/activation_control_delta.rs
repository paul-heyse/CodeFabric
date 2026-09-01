//! Programmatic Arrow/DataFusion/Delta authority for activation-control rows.
//!
//! The relation contract in this module is intentionally static: the durable
//! event vocabulary, field identities, and Delta representation are public
//! architectural contracts. Runtime state is not static. Every provider is
//! bound to one candidate [`SessionState`], one exact [`ExactDeltaPin`], and one
//! provider/transformation binding. Appends use the shared zero-retry Delta
//! writer and reads use only the already loaded exact snapshot.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use arrow_array::builder::{
    FixedSizeBinaryBuilder, ListBuilder, StringBuilder, StructBuilder, UInt64Builder,
};
use arrow_array::{
    Array as _, ArrayRef, FixedSizeBinaryArray, Int64Array, ListArray, RecordBatch, StringArray,
    StructArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Fields, Schema, SchemaRef};
use datafusion::catalog::{ScanArgs, ScanResult, Session, TableProvider};
use datafusion::common::ScalarValue;
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::execution::SessionState;
use datafusion::logical_expr::{Expr, TableProviderFilterPushDown, TableType};
use datafusion::physical_expr::expressions::{cast, col as physical_col};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_plan::projection::{ProjectionExec, ProjectionExpr};
use datafusion::prelude::{SessionContext, col, lit};
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableBuilder};
use thiserror::Error;
use url::Url;

use super::activation::{
    ActivationChain, ActivationControlRelationPin, ActivationDeltaCommitEvidence,
    ActivationDeltaCommitObservation, ActivationEvent, ActivationEventId, ActivationOrdinal,
    CompatibilityClassRef, DurableActivationCommit, DurableActivationRow, FabricEpochPins,
    SealedActivationControlBinding, TableVersionSet, TableVersionSetError, TableVersionSetRef,
};
use super::activation_transaction::{
    ActivationAcknowledgementMarker, ActivationAppendContract, ActivationAppendOutcome,
    ActivationAppendUnknownReason, ActivationAuthorityPort, ActivationAuthorityRequest,
    ActivationAuthoritySnapshot, ActivationEventPort, ActivationOperationMarkerOutcome,
    ActivationOperationMarkerPort, ActivationOperationMarkerRequest, AuthorityRevalidationOutcome,
};
use super::command::{
    DiagnosticRef, EpochId, ExpectedHead, LeaseId, OperationId, OperationSelectionRef,
    ProofReceiptRef, ReconciliationEvidenceRef, RetentionPolicyRef, SourceGeneration,
    TransactionRef, WorkspaceId, WriterFence, WriterGeneration,
};
use super::command_actor::CommandPortError;
use super::command_runtime_ports::CommandActivationChainPort;
use super::delta_exact::{
    ExactDeltaPin, ExactDeltaProviderError, ExactDeltaStatisticsInspection, ValidatedDeltaSnapshot,
    provider_read_from_validated_snapshot,
};
use super::delta_write::{
    ApplicationTransactionMarker, ControlledDeltaWriteMode, ControlledDeltaWriteOutcome,
    ControlledDeltaWriteSpec, SessionBoundLogicalPlan, readback_exact_delta_commit,
    write_exact_delta_plan,
};
use super::epoch_runtime::{FABRIC_CATALOG, FabricSchemaRole};
use super::production_kernel::SelectedEpochRecord;
use super::programmatic_schema::{
    ProgrammaticRelationId, ProgrammaticSchemaAssembly, ProviderInput,
};
use super::provider::{ProviderContractError, SchemaContractTableProvider};
use super::writer_lease::DurableWriterGenerationPort;
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SEMANTIC_ROLE_METADATA_KEY,
    SchemaCompatibility, SchemaContract,
};

/// Stable logical identity of the append-only activation-event relation.
pub const ACTIVATION_CONTROL_RELATION_ID: &str = "control.activation_event.v3";
/// Stable Delta storage identity for the same logical relation.
pub const ACTIVATION_CONTROL_STORAGE_RELATION_ID: &str = "storage.delta.activation_event.v3";
/// Exact provider/transformation implementation selected for the relation.
pub const ACTIVATION_CONTROL_PROVIDER_BINDING_ID: &str =
    "binding.delta.exact-snapshot.activation-event.v3";
/// Contract identity used by schema/session binding evidence.
pub const ACTIVATION_CONTROL_SCHEMA_IDENTITY: &str =
    "programmatic:control.activation_event.v3:arrow59-delta1";
/// Version of this durable relation contract.
pub const ACTIVATION_CONTROL_SCHEMA_VERSION: u16 = 3;

const ARROW_TYPE_UNIVERSE: &str =
    "arrow-array@59.2.0|arrow-schema@59.2.0|datafusion@55.0.0|deltalake@43a0cf10";
const RELATION_PROTOCOL_KEY: &str = "codefabric.relation_protocol_version";
const PROVIDER_BINDING_KEY: &str = "codefabric.provider_binding_id";
const SCHEMA_DIGEST_KEY: &str = "codefabric.schema_digest";
const FIELD_ORDINAL_KEY: &str = "codefabric.field_ordinal";
const WIDTH_KEY: &str = "codefabric.identity_width_bytes";
const ARROW_UNIVERSE_KEY: &str = "codefabric.arrow_type_universe";
const SEMANTIC_ENCODING_KEY: &str = "codefabric.semantic_encoding";
const APPEND_ONLY_PROPERTY: &str = "delta.appendOnly";
const CDF_PROPERTY: &str = "delta.enableChangeDataFeed";
const STATS_COLUMNS_PROPERTY: &str = "delta.dataSkippingStatsColumns";
const ACTIVATION_STATS_COLUMNS: &str =
    "workspace_id,operation_id,event_id,control_commit_version,ordinal";

const DIAGNOSTIC_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.diagnostic-fact.v1";
const RECONCILIATION_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.reconciliation-fact.v1";
const READBACK_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.control-readback.v1";
const PERSISTED_ROW_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.control-row.v2";

const EVENT_ID: usize = 8;
const WORKSPACE_ID: usize = 9;
const OPERATION_ID: usize = 10;
const PREDECESSOR_EVENT_ID: usize = 11;
const PREDECESSOR_EPOCH: usize = 12;
const ORDINAL: usize = 13;
const LEASE_ID: usize = 14;
const WRITER_GENERATION: usize = 15;
const EPOCH_ID: usize = 16;
const INPUT_RELEASE: usize = 17;
const PROGRAM_RELEASE: usize = 18;
const APPLICATION_RELEASE: usize = 19;
const SOURCE_AUTHORITY: usize = 20;
const SOURCE_GENERATION: usize = 21;
const PROVIDER_RELEASE: usize = 22;
const PROVIDER_SET: usize = 23;
const TABLE_VERSIONS: usize = 24;
const TABLE_VERSION_COMPONENTS: usize = 25;
const OVERLAY_SEGMENTS: usize = 26;
const POLICY_SET: usize = 27;
const RESOURCE_ENVELOPE: usize = 28;
const PROOF_RECEIPT: usize = 29;
const COMPATIBILITY_CLASS: usize = 30;
const RETENTION_POLICY: usize = 31;
const OPERATION_SELECTION: usize = 32;
const TRANSACTION: usize = 33;
const ROW_DIGEST: usize = 34;

const COMPONENT_RELATION_ID: usize = 0;
const COMPONENT_DELTA_ROOT: usize = 1;
const COMPONENT_DELTA_VERSION: usize = 2;

#[derive(Clone)]
struct FieldSpec {
    name: &'static str,
    logical_type: DataType,
    storage_type: DataType,
    nullable: bool,
    semantic_role: &'static str,
    identity_width: Option<usize>,
}

fn binary(
    name: &'static str,
    width: i32,
    nullable: bool,
    semantic_role: &'static str,
) -> FieldSpec {
    FieldSpec {
        name,
        logical_type: DataType::FixedSizeBinary(width),
        storage_type: DataType::Binary,
        nullable,
        semantic_role,
        identity_width: Some(usize::try_from(width).expect("positive identity width")),
    }
}

fn u64_field(name: &'static str, semantic_role: &'static str) -> FieldSpec {
    FieldSpec {
        name,
        logical_type: DataType::UInt64,
        storage_type: DataType::Int64,
        nullable: false,
        semantic_role,
        identity_width: None,
    }
}

fn utf8(name: &'static str, semantic_role: &'static str) -> FieldSpec {
    FieldSpec {
        name,
        logical_type: DataType::Utf8,
        storage_type: DataType::Utf8,
        nullable: false,
        semantic_role,
        identity_width: None,
    }
}

fn table_version_component_field(
    name: &'static str,
    data_type: DataType,
    semantic_role: &'static str,
) -> Arc<Field> {
    Arc::new(
        Field::new(name, data_type, false).with_metadata(HashMap::from([
            (
                FIELD_ID_METADATA_KEY.to_owned(),
                format!("control.table_version_component.v1.{name}"),
            ),
            (
                SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                semantic_role.to_owned(),
            ),
        ])),
    )
}

fn table_version_component_fields(storage: bool) -> Fields {
    Fields::from(vec![
        table_version_component_field("relation_id", DataType::Utf8, "stable_relation_identity"),
        table_version_component_field("delta_root", DataType::Utf8, "canonical_delta_root"),
        table_version_component_field(
            "delta_version",
            if storage {
                DataType::Int64
            } else {
                DataType::UInt64
            },
            "exact_delta_version",
        ),
    ])
}

fn table_version_component_item_field(storage: bool) -> Arc<Field> {
    let field = Field::new(
        if storage { "element" } else { "item" },
        DataType::Struct(table_version_component_fields(storage)),
        false,
    );
    Arc::new(if storage {
        // Delta's list contract canonicalizes the physical child name to
        // `element` and does not retain metadata on that container field.
        field
    } else {
        field.with_metadata(HashMap::from([
            (
                FIELD_ID_METADATA_KEY.to_owned(),
                "control.table_version_component.v1.item".to_owned(),
            ),
            (
                SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                "exact_delta_relation_state".to_owned(),
            ),
        ]))
    })
}

fn table_version_components_field() -> FieldSpec {
    FieldSpec {
        name: "table_version_components",
        logical_type: DataType::List(table_version_component_item_field(false)),
        storage_type: DataType::List(table_version_component_item_field(true)),
        nullable: false,
        semantic_role: "reversible_table_version_set",
        identity_width: None,
    }
}

fn activation_fields() -> Vec<FieldSpec> {
    vec![
        utf8("control_root", "exact_delta_root"),
        u64_field("control_predecessor_version", "exact_delta_predecessor"),
        u64_field("control_commit_version", "exact_delta_commit"),
        utf8("control_session_id", "datafusion_session_identity"),
        utf8(
            "control_provider_binding_id",
            "provider_transformation_binding",
        ),
        binary(
            "control_binding_fingerprint",
            32,
            false,
            "session_schema_binding",
        ),
        binary(
            "logical_schema_digest",
            32,
            false,
            "logical_schema_identity",
        ),
        binary(
            "storage_schema_digest",
            32,
            false,
            "storage_schema_identity",
        ),
        binary("event_id", 32, false, "activation_event_identity"),
        binary("workspace_id", 16, false, "workspace_identity"),
        binary("operation_id", 16, false, "fabric_operation_identity"),
        binary(
            "predecessor_event_id",
            32,
            true,
            "predecessor_activation_event",
        ),
        binary("predecessor_epoch", 16, true, "predecessor_epoch_identity"),
        u64_field("ordinal", "activation_chain_ordinal"),
        binary("lease_id", 16, false, "writer_lease_identity"),
        u64_field("writer_generation", "writer_fence_generation"),
        binary("epoch_id", 16, false, "selected_epoch_identity"),
        binary("input_release", 32, false, "input_release_pin"),
        binary("program_release", 32, false, "program_release_pin"),
        binary("application_release", 32, false, "application_release_pin"),
        binary("source_authority", 32, false, "source_authority_pin"),
        u64_field("source_generation", "source_generation_pin"),
        binary("provider_release", 32, false, "provider_release_pin"),
        binary("provider_set", 32, false, "provider_set_pin"),
        binary("table_versions", 32, false, "table_version_set_pin"),
        table_version_components_field(),
        binary("overlay_segments", 32, false, "overlay_segment_set_pin"),
        binary("policy_set", 32, false, "policy_set_pin"),
        binary("resource_envelope", 32, false, "resource_envelope_pin"),
        binary("proof_receipt", 32, false, "candidate_proof_receipt"),
        binary("compatibility_class", 32, false, "compatibility_class"),
        binary("retention_policy", 32, false, "retention_policy"),
        binary(
            "operation_selection",
            32,
            false,
            "operation_selection_record",
        ),
        binary("transaction", 32, false, "application_transaction"),
        binary("row_digest", 32, false, "durable_row_integrity"),
    ]
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn relation_schema(fields: &[FieldSpec], logical: bool, digest: Option<&str>) -> SchemaRef {
    let relation_id = if logical {
        ACTIVATION_CONTROL_RELATION_ID
    } else {
        ACTIVATION_CONTROL_STORAGE_RELATION_ID
    };
    let arrow_fields = fields
        .iter()
        .enumerate()
        .map(|(ordinal, spec)| {
            let mut metadata = HashMap::from([
                (
                    FIELD_ID_METADATA_KEY.to_owned(),
                    format!("{relation_id}.{}", spec.name),
                ),
                (FIELD_ORDINAL_KEY.to_owned(), ordinal.to_string()),
                (
                    SEMANTIC_ROLE_METADATA_KEY.to_owned(),
                    spec.semantic_role.to_owned(),
                ),
            ]);
            if let Some(width) = spec.identity_width {
                metadata.insert(WIDTH_KEY.to_owned(), width.to_string());
            }
            Field::new(
                spec.name,
                if logical {
                    spec.logical_type.clone()
                } else {
                    spec.storage_type.clone()
                },
                spec.nullable,
            )
            .with_metadata(metadata)
        })
        .collect::<Vec<_>>();
    let mut metadata = HashMap::from([
        (RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned()),
        (
            RELATION_PROTOCOL_KEY.to_owned(),
            ACTIVATION_CONTROL_SCHEMA_VERSION.to_string(),
        ),
        (
            PROVIDER_BINDING_KEY.to_owned(),
            ACTIVATION_CONTROL_PROVIDER_BINDING_ID.to_owned(),
        ),
        (
            ARROW_UNIVERSE_KEY.to_owned(),
            ARROW_TYPE_UNIVERSE.to_owned(),
        ),
        (
            SEMANTIC_ENCODING_KEY.to_owned(),
            if logical {
                "typed-arrow-activation-control"
            } else {
                "delta-binary-activation-control"
            }
            .to_owned(),
        ),
    ]);
    if let Some(digest) = digest {
        metadata.insert(SCHEMA_DIGEST_KEY.to_owned(), digest.to_owned());
    }
    Arc::new(Schema::new_with_metadata(arrow_fields, metadata))
}

fn canonical_schema_digest(fields: &[FieldSpec]) -> Result<String, ActivationControlError> {
    // Derive the contract identity from Arrow's serialized structure rather
    // than `Debug` output. Nested Arrow fields carry `HashMap` metadata whose
    // iteration order is intentionally randomized, while JCS object-key order
    // is deterministic. The digest field itself is absent from this preimage
    // so the final schemas can carry the resulting identity without a cycle.
    let logical = relation_schema(fields, true, None);
    let storage = relation_schema(fields, false, None);
    let value = serde_json::json!({
        "source_schema_identity": ACTIVATION_CONTROL_SCHEMA_IDENTITY,
        "logical_schema": serde_json::to_value(logical.as_ref())
            .map_err(|error| ActivationControlError::CanonicalSchema(error.to_string()))?,
        "storage_schema": serde_json::to_value(storage.as_ref())
            .map_err(|error| ActivationControlError::CanonicalSchema(error.to_string()))?,
    });
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| ActivationControlError::CanonicalSchema(error.to_string()))?;
    Ok(hex(blake3::hash(&canonical).as_bytes()))
}

/// Build the authoritative executable schema contract directly from the
/// provider/transformation contract.
///
/// No bootstrap-model relation is read. The static field vocabulary is the
/// durable API, while every runtime provider/session/pin is supplied later.
pub fn activation_control_schema_contract() -> Result<SchemaContract, ActivationControlError> {
    let fields = activation_fields();
    let digest = canonical_schema_digest(&fields)?;
    let logical = relation_schema(&fields, true, Some(&digest));
    let storage = relation_schema(&fields, false, Some(&digest));
    let mappings = (0..fields.len())
        .map(|index| FieldIndexMapping::direct(index, index))
        .collect();
    SchemaContract::try_new(
        ACTIVATION_CONTROL_SCHEMA_IDENTITY,
        datafusion::common::TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::System.as_str(),
            "activation_event",
        ),
        logical,
        storage,
        mappings,
    )
    .map_err(|error| ActivationControlError::SchemaContract(error.to_string()))
}

/// Initialize the one append-only activation-control Delta history at version
/// zero from its executable Arrow storage contract.
///
/// This is physical table provisioning, not a semantic bootstrap model. It is
/// legal only for a root that does not already contain a Delta table; every
/// subsequent open must use an application-owned exact pin.
pub async fn provision_activation_control_history(
    root: Url,
) -> Result<(ExactDeltaPin, DeltaTable), ActivationControlError> {
    let codec = ActivationControlRowCodec::try_new()?;
    let kernel: deltalake::kernel::StructType = codec
        .contract()
        .storage_schema()
        .as_ref()
        .try_into_kernel()
        .map_err(|source| ActivationControlError::Delta(source.to_string()))?;
    CreateBuilder::new()
        .with_location(root.to_string())
        .with_table_name("activation_control")
        .with_comment("CodeFabric append-only execution-proved activation history")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(kernel.fields().cloned())
        .with_configuration([
            (APPEND_ONLY_PROPERTY, Some("true")),
            (CDF_PROPERTY, Some("true")),
            (STATS_COLUMNS_PROPERTY, Some(ACTIVATION_STATS_COLUMNS)),
            ("delta.enableDeletionVectors", Some("false")),
        ])
        .await
        .map_err(|source| ActivationControlError::Delta(source.to_string()))?;
    let table = DeltaTableBuilder::from_url(root.clone())
        .map_err(|source| ActivationControlError::Delta(source.to_string()))?
        .with_version(0)
        .load()
        .await
        .map_err(|source| ActivationControlError::Delta(source.to_string()))?;
    let pin = ExactDeltaPin::new(&root, 0)
        .map_err(|source| ActivationControlError::ExactDelta(source.to_string()))?;
    validate_activation_control_properties(&table)?;
    Ok((pin, table))
}

/// Durable envelope around one activation row, including the exact Delta
/// predecessor/target and original session/schema binding known before append.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedActivationControlRow {
    row: DurableActivationRow,
    table_versions: Arc<TableVersionSet>,
    control_predecessor: ActivationControlRelationPin,
    control_commit_version: u64,
}

impl PersistedActivationControlRow {
    /// Bind a semantic row to the exact zero-retry Delta target.
    pub fn try_new(
        row: DurableActivationRow,
        table_versions: Arc<TableVersionSet>,
        control_predecessor: ActivationControlRelationPin,
    ) -> Result<Self, ActivationControlError> {
        if row.pins.table_versions != table_versions.reference() {
            return Err(ActivationControlError::TableVersionSetReferenceMismatch);
        }
        let control_commit_version = control_predecessor.table().version().checked_add(1).ok_or(
            ActivationControlError::ControlVersionOverflow(control_predecessor.table().version()),
        )?;
        Ok(Self {
            row,
            table_versions,
            control_predecessor,
            control_commit_version,
        })
    }

    #[must_use]
    pub const fn row(&self) -> DurableActivationRow {
        self.row
    }

    #[must_use]
    pub const fn table_versions(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
    }

    #[must_use]
    pub const fn control_predecessor(&self) -> &ActivationControlRelationPin {
        &self.control_predecessor
    }

    #[must_use]
    pub const fn control_commit_version(&self) -> u64 {
        self.control_commit_version
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut digest = FramedDigest::new(PERSISTED_ROW_DIGEST_DOMAIN);
        digest.frame(
            self.control_predecessor
                .table()
                .canonical_root()
                .as_str()
                .as_bytes(),
        );
        digest.frame(&self.control_predecessor.table().version().to_be_bytes());
        digest.frame(&self.control_commit_version.to_be_bytes());
        digest.frame(self.control_predecessor.binding().session_id().as_bytes());
        digest.frame(
            self.control_predecessor
                .binding()
                .physical_binding_id()
                .as_bytes(),
        );
        digest.frame(self.control_predecessor.binding().fingerprint());
        digest.frame(&self.row.canonical_digest());
        digest.finish()
    }
}

/// Exact codec for the logical Arrow row and Delta storage representation.
#[derive(Clone, Debug)]
pub struct ActivationControlRowCodec {
    contract: Arc<SchemaContract>,
}

impl ActivationControlRowCodec {
    pub fn try_new() -> Result<Self, ActivationControlError> {
        Ok(Self {
            contract: Arc::new(activation_control_schema_contract()?),
        })
    }

    #[must_use]
    pub const fn contract(&self) -> &Arc<SchemaContract> {
        &self.contract
    }

    pub fn encode_logical(
        &self,
        rows: &[PersistedActivationControlRow],
    ) -> Result<RecordBatch, ActivationControlError> {
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| {
                row.control_predecessor.table().canonical_root().as_str()
            }))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter()
                    .map(|row| row.control_predecessor.table().version()),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter()
                    .map(PersistedActivationControlRow::control_commit_version),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter()
                    .map(|row| row.control_predecessor.binding().session_id()),
            )),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| {
                row.control_predecessor.binding().physical_binding_id()
            }))),
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.control_predecessor.binding().fingerprint())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.control_predecessor.binding().logical_schema_digest())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.control_predecessor.binding().storage_schema_digest())),
            )?,
            fixed_array::<32>(rows.iter().map(|row| Some(*row.row.event_id.as_bytes())))?,
            fixed_array::<16>(
                rows.iter()
                    .map(|row| Some(*row.row.workspace_id.as_bytes())),
            )?,
            fixed_array::<16>(
                rows.iter()
                    .map(|row| Some(*row.row.operation_id.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| row.row.predecessor_event_id.map(|value| *value.as_bytes())),
            )?,
            fixed_array::<16>(rows.iter().map(|row| match row.row.predecessor_epoch {
                ExpectedHead::Empty => None,
                ExpectedHead::Epoch(value) => Some(*value.as_bytes()),
            }))?,
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.row.ordinal.get()),
            )),
            fixed_array::<16>(
                rows.iter()
                    .map(|row| Some(*row.row.execution_fence.lease_id.as_bytes())),
            )?,
            Arc::new(UInt64Array::from_iter_values(
                rows.iter()
                    .map(|row| row.row.execution_fence.generation.get()),
            )),
            fixed_array::<16>(rows.iter().map(|row| Some(*row.row.pins.epoch.as_bytes())))?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.input_release.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.program_release.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.application_release.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.source_authority.as_bytes())),
            )?,
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.row.pins.source_generation.get()),
            )),
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.provider_release.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.provider_set.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.table_versions.as_bytes())),
            )?,
            table_version_sets_array(rows)?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.overlay_segments.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.policy_set.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.resource_envelope.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.pins.proof_receipt.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.compatibility.as_bytes())),
            )?,
            fixed_array::<32>(rows.iter().map(|row| Some(*row.row.retention.as_bytes())))?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.commit.operation_selection.as_bytes())),
            )?,
            fixed_array::<32>(
                rows.iter()
                    .map(|row| Some(*row.row.commit.transaction.as_bytes())),
            )?,
            fixed_array::<32>(rows.iter().map(|row| Some(row.canonical_digest())))?,
        ];
        let batch = RecordBatch::try_new(Arc::clone(self.contract.logical_schema()), columns)
            .map_err(|error| ActivationControlError::Arrow(error.to_string()))?;
        self.contract
            .validate_batch(&batch.schema(), &batch, SchemaCompatibility::Exact)
            .map_err(|error| ActivationControlError::SchemaContract(error.to_string()))?;
        Ok(batch)
    }

    pub fn encode_storage(
        &self,
        rows: &[PersistedActivationControlRow],
    ) -> Result<RecordBatch, ActivationControlError> {
        let logical = self.encode_logical(rows)?;
        self.contract
            .adapt_logical_batch_to_storage(&logical)
            .map_err(|error| ActivationControlError::SchemaContract(error.to_string()))
    }

    pub fn decode_logical(
        &self,
        batch: &RecordBatch,
    ) -> Result<Vec<PersistedActivationControlRow>, ActivationControlError> {
        self.contract
            .validate_batch(&batch.schema(), batch, SchemaCompatibility::Exact)
            .map_err(|error| ActivationControlError::SchemaContract(error.to_string()))?;
        (0..batch.num_rows())
            .map(|index| self.decode_logical_row(batch, index))
            .collect()
    }

    pub fn decode_storage(
        &self,
        batch: &RecordBatch,
    ) -> Result<Vec<PersistedActivationControlRow>, ActivationControlError> {
        let logical = self
            .contract
            .restore_storage_batch(batch)
            .map_err(|error| ActivationControlError::SchemaContract(error.to_string()))?;
        self.decode_logical(&logical)
    }

    #[allow(clippy::too_many_lines)]
    fn decode_logical_row(
        &self,
        batch: &RecordBatch,
        index: usize,
    ) -> Result<PersistedActivationControlRow, ActivationControlError> {
        let root = Url::parse(required_string(batch, 0, index)?)
            .map_err(|error| ActivationControlError::InvalidControlRoot(error.to_string()))?;
        let predecessor_version = required_u64(batch, 1, index)?;
        let commit_version = required_u64(batch, 2, index)?;
        let expected_commit = predecessor_version.checked_add(1).ok_or(
            ActivationControlError::ControlVersionOverflow(predecessor_version),
        )?;
        if commit_version != expected_commit {
            return Err(ActivationControlError::CommitVersionMismatch {
                expected: expected_commit,
                observed: commit_version,
            });
        }
        let session_id = required_string(batch, 3, index)?;
        let provider_binding_id = required_string(batch, 4, index)?;
        if provider_binding_id != ACTIVATION_CONTROL_PROVIDER_BINDING_ID {
            return Err(ActivationControlError::ProviderBindingMismatch {
                expected: ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
                observed: provider_binding_id.to_owned(),
            });
        }
        let binding = SealedActivationControlBinding::try_from_recorded_session_and_contract(
            session_id,
            provider_binding_id,
            &self.contract,
        )
        .map_err(|error| ActivationControlError::Binding(error.to_string()))?;
        require_equal_32(
            batch,
            5,
            index,
            binding.fingerprint(),
            "binding fingerprint",
        )?;
        require_equal_32(
            batch,
            6,
            index,
            binding.logical_schema_digest(),
            "logical schema digest",
        )?;
        require_equal_32(
            batch,
            7,
            index,
            binding.storage_schema_digest(),
            "storage schema digest",
        )?;

        let predecessor_event_id = optional_fixed::<32>(batch, PREDECESSOR_EVENT_ID, index)?
            .map(ActivationEventId::from_bytes);
        let predecessor_epoch = optional_fixed::<16>(batch, PREDECESSOR_EPOCH, index)?
            .map_or(ExpectedHead::Empty, |value| {
                ExpectedHead::Epoch(EpochId::from_bytes(value))
            });
        if predecessor_event_id.is_none() != matches!(predecessor_epoch, ExpectedHead::Empty) {
            return Err(ActivationControlError::InconsistentPredecessorNullability);
        }
        let ordinal_value = required_u64(batch, ORDINAL, index)?;
        let ordinal = ActivationOrdinal::new(ordinal_value)
            .ok_or(ActivationControlError::ZeroActivationOrdinal)?;
        let writer_value = required_u64(batch, WRITER_GENERATION, index)?;
        let writer_generation = WriterGeneration::new(writer_value)
            .ok_or(ActivationControlError::ZeroWriterGeneration)?;
        let table_versions = Arc::new(required_table_version_set(
            batch,
            TABLE_VERSION_COMPONENTS,
            index,
        )?);
        let row =
            DurableActivationRow {
                event_id: ActivationEventId::from_bytes(required_fixed(batch, EVENT_ID, index)?),
                workspace_id: WorkspaceId::from_bytes(required_fixed(batch, WORKSPACE_ID, index)?),
                operation_id: OperationId::from_bytes(required_fixed(batch, OPERATION_ID, index)?),
                predecessor_event_id,
                predecessor_epoch,
                ordinal,
                execution_fence: WriterFence {
                    lease_id: LeaseId::from_bytes(required_fixed(batch, LEASE_ID, index)?),
                    generation: writer_generation,
                },
                pins: FabricEpochPins {
                    epoch: EpochId::from_bytes(required_fixed(batch, EPOCH_ID, index)?),
                    input_release: super::command::InputReleaseRef::from_bytes(required_fixed(
                        batch,
                        INPUT_RELEASE,
                        index,
                    )?),
                    program_release: super::command::ProgramReleaseRef::from_bytes(required_fixed(
                        batch,
                        PROGRAM_RELEASE,
                        index,
                    )?),
                    application_release: super::command::ApplicationReleaseRef::from_bytes(
                        required_fixed(batch, APPLICATION_RELEASE, index)?,
                    ),
                    source_authority: super::command::SourceAuthorityRef::from_bytes(
                        required_fixed(batch, SOURCE_AUTHORITY, index)?,
                    ),
                    source_generation: SourceGeneration::new(required_u64(
                        batch,
                        SOURCE_GENERATION,
                        index,
                    )?),
                    provider_release: super::command::ProviderReleaseRef::from_bytes(
                        required_fixed(batch, PROVIDER_RELEASE, index)?,
                    ),
                    provider_set: super::command::ProviderSetRef::from_bytes(required_fixed(
                        batch,
                        PROVIDER_SET,
                        index,
                    )?),
                    table_versions: TableVersionSetRef::from_bytes(required_fixed(
                        batch,
                        TABLE_VERSIONS,
                        index,
                    )?),
                    overlay_segments: super::activation::OverlaySegmentSetRef::from_bytes(
                        required_fixed(batch, OVERLAY_SEGMENTS, index)?,
                    ),
                    policy_set: super::activation::PolicySetRef::from_bytes(required_fixed(
                        batch, POLICY_SET, index,
                    )?),
                    resource_envelope: super::command::ResourceEnvelopeRef::from_bytes(
                        required_fixed(batch, RESOURCE_ENVELOPE, index)?,
                    ),
                    proof_receipt: super::command::ProofReceiptRef::from_bytes(required_fixed(
                        batch,
                        PROOF_RECEIPT,
                        index,
                    )?),
                },
                compatibility: CompatibilityClassRef::from_bytes(required_fixed(
                    batch,
                    COMPATIBILITY_CLASS,
                    index,
                )?),
                retention: RetentionPolicyRef::from_bytes(required_fixed(
                    batch,
                    RETENTION_POLICY,
                    index,
                )?),
                commit: DurableActivationCommit {
                    operation_selection: OperationSelectionRef::from_bytes(required_fixed(
                        batch,
                        OPERATION_SELECTION,
                        index,
                    )?),
                    transaction: TransactionRef::from_bytes(required_fixed(
                        batch,
                        TRANSACTION,
                        index,
                    )?),
                },
            };
        let control_predecessor = ActivationControlRelationPin::new(
            ExactDeltaPin::new(&root, predecessor_version)
                .map_err(|error| ActivationControlError::ExactDelta(error.to_string()))?,
            binding,
        );
        let persisted =
            PersistedActivationControlRow::try_new(row, table_versions, control_predecessor)?;
        require_equal_32(
            batch,
            ROW_DIGEST,
            index,
            &persisted.canonical_digest(),
            "durable row digest",
        )?;
        Ok(persisted)
    }
}

/// Exact, session-bound activation-control provider over one already loaded
/// Delta snapshot.
pub struct ActivationControlDeltaProvider {
    session: Arc<SessionState>,
    table: DeltaTable,
    control_relation: ActivationControlRelationPin,
    codec: ActivationControlRowCodec,
    provider: Arc<dyn TableProvider>,
    statistics: ExactDeltaStatisticsInspection,
}

impl fmt::Debug for ActivationControlDeltaProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationControlDeltaProvider")
            .field("session_id", &self.session.session_id())
            .field("control_pin", self.control_relation.table())
            .finish_non_exhaustive()
    }
}

impl ActivationControlDeltaProvider {
    /// Validate one loaded exact Delta snapshot and bind it to the supplied
    /// candidate session. The table is never refreshed by this object.
    pub async fn try_from_loaded_table(
        session: Arc<SessionState>,
        pin: ExactDeltaPin,
        table: DeltaTable,
    ) -> Result<Self, ActivationControlError> {
        let codec = ActivationControlRowCodec::try_new()?;
        let binding = SealedActivationControlBinding::try_from_session_and_contract(
            session.as_ref(),
            ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
            codec.contract(),
        )
        .map_err(|error| ActivationControlError::Binding(error.to_string()))?;
        let control_relation = ActivationControlRelationPin::new(pin.clone(), binding);
        let snapshot = ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &pin)
            .map_err(|error| ActivationControlError::ExactDelta(error.to_string()))?;
        validate_activation_control_properties(&table)?;
        let read = provider_read_from_validated_snapshot(&pin, snapshot, Arc::clone(&session))
            .await
            .map_err(|error| ActivationControlError::ExactDelta(error.to_string()))?;
        let (raw, statistics) = read.into_parts();
        let restored = Arc::new(ActivationStorageProvider::try_new(
            Arc::clone(codec.contract()),
            raw,
        )?) as Arc<dyn TableProvider>;
        let provider = Arc::new(
            SchemaContractTableProvider::try_new(Arc::clone(codec.contract()), restored)
                .map_err(|error| ActivationControlError::Provider(error.to_string()))?,
        ) as Arc<dyn TableProvider>;
        Ok(Self {
            session,
            table,
            control_relation,
            codec,
            provider,
            statistics,
        })
    }

    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    #[must_use]
    pub const fn contract(&self) -> &Arc<SchemaContract> {
        self.codec.contract()
    }

    #[must_use]
    pub fn provider(&self) -> Arc<dyn TableProvider> {
        Arc::clone(&self.provider)
    }

    /// Non-lossy file/column and optimizer statistics retained from exact
    /// activation-control provider construction.
    #[must_use]
    pub const fn statistics(&self) -> &ExactDeltaStatisticsInspection {
        &self.statistics
    }

    /// Append one row against exactly this provider's loaded predecessor.
    /// The shared writer performs one zero-retry attempt and returns all
    /// conflicts/unknowns to the command reducer.
    pub async fn append_exact(
        &self,
        row: DurableActivationRow,
        table_versions: Arc<TableVersionSet>,
    ) -> Result<ControlledDeltaWriteOutcome, ActivationControlError> {
        if row.operation_id == OperationId::from_bytes([0; 16]) {
            return Err(ActivationControlError::ZeroOperationId);
        }
        let persisted = PersistedActivationControlRow::try_new(
            row,
            table_versions,
            self.control_relation.clone(),
        )?;
        let encoded = self.codec.encode_storage(&[persisted])?;
        let write_schema = self
            .table
            .snapshot()
            .map_err(|error| ActivationControlError::Delta(error.to_string()))?
            .snapshot()
            .arrow_schema();
        let batch = RecordBatch::try_new(write_schema, encoded.columns().to_vec())
            .map_err(|error| ActivationControlError::Arrow(error.to_string()))?;
        let context = SessionContext::new_with_state(self.session.as_ref().clone());
        let dataframe = context
            .read_batch(batch)
            .map_err(|error| ActivationControlError::DataFusion(error.to_string()))?;
        let input =
            SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&self.session), dataframe)
                .map_err(|error| ActivationControlError::PlanBinding(error.to_string()))?;
        let spec = ControlledDeltaWriteSpec::new(
            self.control_relation.table().clone(),
            row.operation_id,
            row.execution_fence.generation,
            ApplicationTransactionMarker::from_transaction_ref(row.commit.transaction),
            ControlledDeltaWriteMode::Append,
        );
        Ok(write_exact_delta_plan(&self.table, &spec, input).await)
    }

    /// Execute a complete scan of this exact historical provider and decode
    /// every row. Completion of `collect` is the read-horizon evidence; partial
    /// streams never construct this value.
    pub async fn read_all(
        &self,
        active_recovery_fence: WriterFence,
    ) -> Result<ActivationControlReadback, ActivationControlError> {
        self.read_scope(active_recovery_fence, ActivationControlReadScope::All)
            .await
    }

    /// Execute a complete predicate-pushed scan for one workspace's chain.
    /// The workspace identity is included in the canonical readback scope, so
    /// a filtered result cannot be mistaken for a complete global history.
    pub async fn read_workspace(
        &self,
        workspace_id: WorkspaceId,
        active_recovery_fence: WriterFence,
    ) -> Result<ActivationControlReadback, ActivationControlError> {
        self.read_scope(
            active_recovery_fence,
            ActivationControlReadScope::Workspace(workspace_id),
        )
        .await
    }

    /// Reconstruct one workspace's canonical activation chain from this exact
    /// completed Delta read horizon.
    pub async fn read_workspace_chain(
        &self,
        workspace_id: WorkspaceId,
        active_fence: WriterFence,
    ) -> Result<ActivationChain, ActivationControlError> {
        let readback = self.read_workspace(workspace_id, active_fence).await?;
        self.reconstruct_workspace_chain(&readback, workspace_id, None)
            .await
    }

    async fn read_scope(
        &self,
        active_recovery_fence: WriterFence,
        scope: ActivationControlReadScope,
    ) -> Result<ActivationControlReadback, ActivationControlError> {
        let context = SessionContext::new_with_state(self.session.as_ref().clone());
        let dataframe = context
            .read_table(Arc::clone(&self.provider))
            .map_err(|error| ActivationControlError::DataFusion(error.to_string()))?;
        let dataframe = match scope {
            ActivationControlReadScope::All => dataframe,
            ActivationControlReadScope::Workspace(workspace_id) => dataframe
                .filter(col("workspace_id").eq(lit(ScalarValue::FixedSizeBinary(
                    16,
                    Some(workspace_id.as_bytes().to_vec()),
                ))))
                .map_err(|error| ActivationControlError::DataFusion(error.to_string()))?,
        };
        let batches = dataframe
            .collect()
            .await
            .map_err(|error| ActivationControlError::DataFusion(error.to_string()))?;
        let mut rows = Vec::new();
        for batch in batches {
            rows.extend(self.codec.decode_logical(&batch)?);
        }
        ActivationControlReadback::try_new(
            self.control_relation.clone(),
            active_recovery_fence,
            scope,
            rows,
        )
    }

    async fn reconstruct_row_evidence(
        &self,
        persisted: &PersistedActivationControlRow,
    ) -> Result<ActivationDeltaCommitEvidence, ActivationControlError> {
        let row = persisted.row;
        let committed = ExactDeltaPin::new(
            persisted.control_predecessor.table().canonical_root(),
            persisted.control_commit_version,
        )
        .map_err(|error| ActivationControlError::ExactDelta(error.to_string()))?;
        let table = DeltaTableBuilder::from_url(committed.canonical_root().clone())
            .map_err(|error| ActivationControlError::Delta(error.to_string()))?
            .with_version(committed.version())
            .load()
            .await
            .map_err(|error| ActivationControlError::Delta(error.to_string()))?;
        let spec = ControlledDeltaWriteSpec::new(
            persisted.control_predecessor.table().clone(),
            row.operation_id,
            row.execution_fence.generation,
            ApplicationTransactionMarker::from_transaction_ref(row.commit.transaction),
            ControlledDeltaWriteMode::Append,
        );
        let write = readback_exact_delta_commit(
            &table,
            &spec,
            persisted.control_predecessor.binding().session_id(),
        )
        .await
        .map_err(|unknown| ActivationControlError::HistoricalCommitReadback {
            event_id: row.event_id,
            detail: unknown.detail().to_owned(),
        })?;
        let observation = ActivationDeltaCommitObservation::from_controlled_write(&write)
            .map_err(|error| ActivationControlError::ActivationEvidence(error.to_string()))?;
        ActivationDeltaCommitEvidence::try_new(
            persisted.control_predecessor.clone(),
            row.commit.transaction,
            row.operation_id,
            row.execution_fence,
            observation,
        )
        .map_err(|error| ActivationControlError::ActivationEvidence(error.to_string()))
    }

    async fn reconstruct_workspace_chain(
        &self,
        readback: &ActivationControlReadback,
        workspace_id: WorkspaceId,
        fresh_commit: Option<(ActivationEventId, &ActivationDeltaCommitEvidence)>,
    ) -> Result<ActivationChain, ActivationControlError> {
        if let ActivationControlReadScope::Workspace(scoped_workspace) = readback.scope
            && scoped_workspace != workspace_id
        {
            return Err(ActivationControlError::ReadScopeMismatch {
                expected: workspace_id,
                observed: readback.scope,
            });
        }
        let mut events = Vec::new();
        for persisted in readback
            .rows
            .iter()
            .filter(|persisted| persisted.row.workspace_id == workspace_id)
        {
            let row = persisted.row;
            let observed_in = ExactDeltaPin::new(
                persisted.control_predecessor.table().canonical_root(),
                persisted.control_commit_version,
            )
            .map_err(|error| ActivationControlError::ExactDelta(error.to_string()))?;
            let event = if let Some((event_id, evidence)) = fresh_commit
                && row.event_id == event_id
            {
                ActivationEvent::try_from_durable_row(row, &observed_in, evidence)
            } else {
                let evidence = self.reconstruct_row_evidence(persisted).await?;
                ActivationEvent::try_from_durable_row(row, &observed_in, &evidence)
            }
            .map_err(|error| ActivationControlError::ActivationEvidence(error.to_string()))?;
            events.push(event);
        }
        ActivationChain::derive(workspace_id, events)
            .map_err(|error| ActivationControlError::ActivationChain(error.to_string()))
    }

    async fn reconciliation_readback(
        &self,
        request: &ActivationOperationMarkerRequest,
    ) -> Result<(ActivationControlReadback, ActivationReconciliationFact), ActivationControlError>
    {
        let marker = ApplicationTransactionMarker::from_transaction_ref(request.transaction);
        let marker_version = self
            .table
            .snapshot()
            .map_err(|error| ActivationControlError::Delta(error.to_string()))?
            .transaction_version(self.table.log_store().as_ref(), marker.application_id())
            .await
            .map_err(|error| ActivationControlError::Delta(error.to_string()))?;
        let readback = self
            .read_workspace(request.workspace_id, request.active_recovery_fence)
            .await?;
        let fact = ActivationReconciliationFact::try_new(request, &readback, marker_version)?;
        Ok((readback, fact))
    }

    /// Query the canonical Delta transaction marker in this exact snapshot and
    /// derive a relational reconciliation fact over the complete readback.
    pub async fn reconcile_operation(
        &self,
        request: &ActivationOperationMarkerRequest,
    ) -> Result<ActivationReconciliationFact, ActivationControlError> {
        self.reconciliation_readback(request)
            .await
            .map(|(_, fact)| fact)
    }
}

/// Fenced semantic reader over one exact activation-control Delta horizon.
///
/// The provider supplies durable relational history while the generation
/// store independently supplies current writer authority. Callers replace
/// this value with a provider loaded at the newly committed control version
/// after activation; this object never performs a hidden refresh or latest
/// lookup.
pub struct DeltaActivationRuntimeAuthority {
    workspace_id: WorkspaceId,
    control: Arc<ActivationControlDeltaProvider>,
    generations: Arc<dyn DurableWriterGenerationPort>,
}

impl fmt::Debug for DeltaActivationRuntimeAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeltaActivationRuntimeAuthority")
            .field("workspace_id", &self.workspace_id)
            .field("control_relation", self.control.control_relation())
            .field("generations", &"installed")
            .finish_non_exhaustive()
    }
}

impl DeltaActivationRuntimeAuthority {
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        control: Arc<ActivationControlDeltaProvider>,
        generations: Arc<dyn DurableWriterGenerationPort>,
    ) -> Self {
        Self {
            workspace_id,
            control,
            generations,
        }
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub fn control_relation(&self) -> &ActivationControlRelationPin {
        self.control.control_relation()
    }

    fn observe_fence(&self) -> Result<WriterFence, Arc<str>> {
        self.generations
            .observe_current(self.workspace_id)
            .map_err(|error| Arc::<str>::from(error.to_string()))?
            .ok_or_else(|| Arc::<str>::from("no durable writer fence exists for workspace"))
    }

    async fn read_current_horizon(
        &self,
    ) -> Result<
        (ActivationControlReadback, ActivationChain),
        DeltaActivationRuntimeAuthoritySnapshotError,
    > {
        let active_fence = self
            .observe_fence()
            .map_err(DeltaActivationRuntimeAuthoritySnapshotError::WriterAuthority)?;
        let readback = self
            .control
            .read_workspace(self.workspace_id, active_fence)
            .await
            .map_err(DeltaActivationRuntimeAuthoritySnapshotError::Control)?;
        let chain = self
            .control
            .reconstruct_workspace_chain(&readback, self.workspace_id, None)
            .await
            .map_err(DeltaActivationRuntimeAuthoritySnapshotError::Control)?;
        Ok((readback, chain))
    }

    /// Reconstruct the durable workspace selection from one complete exact Delta readback.
    ///
    /// An empty scoped chain is a lawful genesis state. A selected state carries the exact event,
    /// its complete reversible table-version vector, the event's writer fence and proof reference,
    /// and the control horizon which produced all of them. The independently observed current
    /// writer guard is used only to authorize recovery; this path never reacquires it.
    pub(crate) async fn current_selection(
        &self,
    ) -> Result<ExactActivationControlSelection, DeltaActivationRuntimeAuthoritySnapshotError> {
        let (readback, chain) = self.read_current_horizon().await?;
        ExactActivationControlSelection::try_from_readback(&readback, &chain, self.workspace_id)
    }

    /// Read the exact activation chain under the independently observed current writer fence.
    ///
    /// This is the production cold-start input to workspace composition. The returned snapshot is
    /// derived from the already exact, non-refreshing Delta provider and the durable generation
    /// store; neither a process cache nor a latest-table lookup participates.
    pub async fn current_snapshot(
        &self,
    ) -> Result<ActivationAuthoritySnapshot, DeltaActivationRuntimeAuthoritySnapshotError> {
        let (readback, chain) = self.read_current_horizon().await?;
        Ok(ActivationAuthoritySnapshot {
            chain,
            active_fence: readback.active_recovery_fence,
        })
    }
}

fn recovery_fence_authorizes(selected: WriterFence, active: WriterFence) -> bool {
    active == selected || active.generation.get() > selected.generation.get()
}

/// Fail-closed cold-start observation failures from the concrete Delta activation authority.
#[derive(Debug, Error)]
pub enum DeltaActivationRuntimeAuthoritySnapshotError {
    #[error("durable writer authority is unavailable: {0}")]
    WriterAuthority(Arc<str>),
    #[error("exact activation-control readback failed: {0}")]
    Control(ActivationControlError),
    #[error("selected activation event {0:?} has no reversible table-version vector")]
    SelectedTableVersionsMissing(ActivationEventId),
    #[error(
        "current writer fence {active:?} does not authorize recovery of selected fence {selected:?}"
    )]
    SelectionFenceNotAuthorized {
        selected: WriterFence,
        active: WriterFence,
    },
}

#[async_trait::async_trait]
impl CommandActivationChainPort for DeltaActivationRuntimeAuthority {
    async fn read_chain(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ActivationChain, CommandPortError> {
        if workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        let fence = self
            .observe_fence()
            .map_err(|_| CommandPortError::ContextUnavailable)?;
        self.control
            .read_workspace_chain(workspace_id, fence)
            .await
            .map_err(|_| CommandPortError::ContextUnavailable)
    }
}

#[async_trait::async_trait]
impl ActivationAuthorityPort for DeltaActivationRuntimeAuthority {
    async fn revalidate(
        &self,
        request: ActivationAuthorityRequest,
    ) -> AuthorityRevalidationOutcome {
        if request.workspace_id != self.workspace_id {
            return AuthorityRevalidationOutcome::Unknown {
                diagnostic: activation_authority_diagnostic_ref(
                    &request,
                    self.control.control_relation(),
                    "activation authority request targets another workspace",
                ),
            };
        }
        let active_fence = match self.observe_fence() {
            Ok(fence) => fence,
            Err(detail) => {
                return AuthorityRevalidationOutcome::Unknown {
                    diagnostic: activation_authority_diagnostic_ref(
                        &request,
                        self.control.control_relation(),
                        &detail,
                    ),
                };
            }
        };
        let chain = match self
            .control
            .read_workspace_chain(self.workspace_id, active_fence)
            .await
        {
            Ok(chain) => chain,
            Err(error) => {
                return AuthorityRevalidationOutcome::Unknown {
                    diagnostic: activation_authority_diagnostic_ref(
                        &request,
                        self.control.control_relation(),
                        &error.to_string(),
                    ),
                };
            }
        };
        let snapshot = ActivationAuthoritySnapshot {
            chain,
            active_fence,
        };
        if snapshot.active_fence == request.execution_fence
            && snapshot.chain.current_head() == request.expected_head
        {
            AuthorityRevalidationOutcome::Valid(snapshot)
        } else {
            AuthorityRevalidationOutcome::Stale(snapshot)
        }
    }
}

#[async_trait::async_trait]
impl ActivationEventPort for DeltaActivationRuntimeAuthority {
    async fn append_and_readback(
        &self,
        contract: ActivationAppendContract,
    ) -> ActivationAppendOutcome {
        self.control.append_and_readback(contract).await
    }
}

#[async_trait::async_trait]
impl ActivationOperationMarkerPort for DeltaActivationRuntimeAuthority {
    async fn read_operation_marker(
        &self,
        request: ActivationOperationMarkerRequest,
    ) -> ActivationOperationMarkerOutcome {
        self.control.read_operation_marker(request).await
    }
}

fn activation_authority_diagnostic_ref(
    request: &ActivationAuthorityRequest,
    control_relation: &ActivationControlRelationPin,
    detail: &str,
) -> DiagnosticRef {
    let mut digest = FramedDigest::new(b"codefabric.activation-authority-diagnostic.v1\0");
    digest.frame(request.workspace_id.as_bytes());
    digest.frame(request.operation_id.as_bytes());
    match request.expected_head {
        ExpectedHead::Empty => digest.frame(&[0]),
        ExpectedHead::Epoch(epoch) => {
            digest.frame(&[1]);
            digest.frame(epoch.as_bytes());
        }
    }
    digest.frame(request.execution_fence.lease_id.as_bytes());
    digest.frame(&request.execution_fence.generation.get().to_be_bytes());
    digest.frame(control_relation.fingerprint());
    digest.frame(detail.as_bytes());
    DiagnosticRef::from_bytes(digest.finish())
}

fn validate_activation_control_properties(
    table: &DeltaTable,
) -> Result<(), ActivationControlError> {
    let snapshot = table
        .snapshot()
        .map_err(|source| ActivationControlError::Delta(source.to_string()))?;
    let configuration = snapshot.metadata().configuration();
    for (key, expected) in [
        (APPEND_ONLY_PROPERTY, "true"),
        (CDF_PROPERTY, "true"),
        (STATS_COLUMNS_PROPERTY, ACTIVATION_STATS_COLUMNS),
    ] {
        let actual = configuration.get(key).cloned();
        if actual.as_deref() != Some(expected) {
            return Err(ActivationControlError::RequiredTableProperty {
                key,
                expected,
                actual,
            });
        }
    }
    if !snapshot.metadata().partition_columns().is_empty() {
        return Err(ActivationControlError::UnexpectedPartitionColumns(
            snapshot.metadata().partition_columns().to_vec(),
        ));
    }
    Ok(())
}

fn activation_diagnostic_ref(
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    transaction: TransactionRef,
    control_relation: &ActivationControlRelationPin,
    stage: ActivationDiagnosticStage,
    detail: impl Into<Arc<str>>,
) -> DiagnosticRef {
    let detail = detail.into();
    let detail = if detail.trim().is_empty() {
        Arc::<str>::from("activation control operation failed without backend detail")
    } else {
        detail
    };
    ActivationDiagnosticFact::try_new(
        workspace_id,
        operation_id,
        transaction,
        control_relation.clone(),
        stage,
        detail,
    )
    .expect("nonempty canonical activation diagnostic inputs")
    .diagnostic()
}

fn append_unknown(
    contract: &ActivationAppendContract,
    reason: ActivationAppendUnknownReason,
    stage: ActivationDiagnosticStage,
    detail: impl Into<Arc<str>>,
) -> ActivationAppendOutcome {
    ActivationAppendOutcome::Unknown {
        reason,
        diagnostic: activation_diagnostic_ref(
            contract.command().ownership.workspace_id,
            contract.command().identity.operation_id,
            contract.transaction(),
            contract.control_relation(),
            stage,
            detail,
        ),
    }
}

#[async_trait::async_trait]
impl ActivationEventPort for ActivationControlDeltaProvider {
    async fn append_and_readback(
        &self,
        contract: ActivationAppendContract,
    ) -> ActivationAppendOutcome {
        if self.control_relation != *contract.control_relation() {
            return append_unknown(
                &contract,
                ActivationAppendUnknownReason::CommitOutcomeUnknown,
                ActivationDiagnosticStage::SchemaBinding,
                "activation append contract is not bound to this exact control provider",
            );
        }
        if let Err(error) = self.control_relation.binding().revalidate(
            self.session.as_ref(),
            ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
            self.codec.contract(),
        ) {
            return append_unknown(
                &contract,
                ActivationAppendUnknownReason::CommitOutcomeUnknown,
                ActivationDiagnosticStage::SchemaBinding,
                error.to_string(),
            );
        }
        let row = match DurableActivationRow::try_from_attempt(
            contract.event_id(),
            contract.attempt(),
            contract.predecessor_event_id(),
            contract.ordinal(),
            contract.pins(),
            contract.compatibility(),
            contract.retention(),
            DurableActivationCommit {
                operation_selection: contract.operation_selection(),
                transaction: contract.transaction(),
            },
        ) {
            Ok(row) => row,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::CommitOutcomeUnknown,
                    ActivationDiagnosticStage::SchemaBinding,
                    error.to_string(),
                );
            }
        };
        let outcome = match self
            .append_exact(row, Arc::clone(contract.table_versions()))
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::CommitOutcomeUnknown,
                    ActivationDiagnosticStage::DeltaAppend,
                    error.to_string(),
                );
            }
        };
        let ControlledDeltaWriteOutcome::Committed(write) = outcome else {
            return append_unknown(
                &contract,
                ActivationAppendUnknownReason::CommitOutcomeUnknown,
                ActivationDiagnosticStage::DeltaAppend,
                format!("controlled Delta append requires reconciliation: {outcome:?}"),
            );
        };
        let observation = match ActivationDeltaCommitObservation::from_controlled_write(&write) {
            Ok(observation) => observation,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        let evidence = match ActivationDeltaCommitEvidence::try_new(
            contract.control_relation().clone(),
            contract.transaction(),
            contract.command().identity.operation_id,
            contract.execution_fence(),
            observation,
        ) {
            Ok(evidence) => evidence,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        let committed_pin = write.committed().clone();
        let committed = match ActivationControlDeltaProvider::try_from_loaded_table(
            Arc::clone(&self.session),
            committed_pin,
            write.into_table(),
        )
        .await
        {
            Ok(committed) => committed,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        let workspace_id = contract.command().ownership.workspace_id;
        let readback = match committed
            .read_workspace(workspace_id, contract.execution_fence())
            .await
        {
            Ok(readback) => readback,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        let chain = match committed
            .reconstruct_workspace_chain(
                &readback,
                workspace_id,
                Some((contract.event_id(), &evidence)),
            )
            .await
        {
            Ok(chain) => chain,
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        let selection = match ExactActivationControlSelection::try_from_readback(
            &readback,
            &chain,
            workspace_id,
        ) {
            Ok(ExactActivationControlSelection::Selected(selection)) => selection,
            Ok(ExactActivationControlSelection::GenesisRequired(_)) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    "committed append produced an empty exact activation horizon",
                );
            }
            Err(error) => {
                return append_unknown(
                    &contract,
                    ActivationAppendUnknownReason::ReadbackUnavailable,
                    ActivationDiagnosticStage::ControlReadback,
                    error.to_string(),
                );
            }
        };
        if selection.event().event_id() != contract.event_id() {
            return append_unknown(
                &contract,
                ActivationAppendUnknownReason::ReadbackUnavailable,
                ActivationDiagnosticStage::ControlReadback,
                "committed append is not the unique selected event at the exact read horizon",
            );
        }
        if selection.table_versions().as_ref() != contract.table_versions().as_ref() {
            return append_unknown(
                &contract,
                ActivationAppendUnknownReason::ReadbackUnavailable,
                ActivationDiagnosticStage::RowDecode,
                "selected event's reversible table-version vector differs from the append contract",
            );
        }
        ActivationAppendOutcome::Committed {
            selection: SelectedEpochRecord::from_exact_readback(&selection),
            chain_after_readback: chain,
        }
    }
}

#[async_trait::async_trait]
impl ActivationOperationMarkerPort for ActivationControlDeltaProvider {
    async fn read_operation_marker(
        &self,
        request: ActivationOperationMarkerRequest,
    ) -> ActivationOperationMarkerOutcome {
        let (readback, fact) = match self.reconciliation_readback(&request).await {
            Ok(value) => value,
            Err(error) => {
                return ActivationOperationMarkerOutcome::Unknown {
                    diagnostic: activation_diagnostic_ref(
                        request.workspace_id,
                        request.operation_id,
                        request.transaction,
                        &request.control_relation,
                        ActivationDiagnosticStage::Reconciliation,
                        error.to_string(),
                    ),
                };
            }
        };
        let chain = match self
            .reconstruct_workspace_chain(&readback, request.workspace_id, None)
            .await
        {
            Ok(chain) => chain,
            Err(error) => {
                return ActivationOperationMarkerOutcome::Unknown {
                    diagnostic: activation_diagnostic_ref(
                        request.workspace_id,
                        request.operation_id,
                        request.transaction,
                        &request.control_relation,
                        ActivationDiagnosticStage::Reconciliation,
                        error.to_string(),
                    ),
                };
            }
        };
        match fact.disposition() {
            ActivationReconciliationDisposition::Selected(event_id) => {
                let selection = match ExactActivationControlSelection::try_from_readback(
                    &readback,
                    &chain,
                    request.workspace_id,
                ) {
                    Ok(ExactActivationControlSelection::Selected(selection)) => selection,
                    Ok(ExactActivationControlSelection::GenesisRequired(_)) => {
                        return ActivationOperationMarkerOutcome::Unknown {
                            diagnostic: activation_diagnostic_ref(
                                request.workspace_id,
                                request.operation_id,
                                request.transaction,
                                &request.control_relation,
                                ActivationDiagnosticStage::Reconciliation,
                                "selected marker resolved to an empty exact activation horizon",
                            ),
                        };
                    }
                    Err(error) => {
                        return ActivationOperationMarkerOutcome::Unknown {
                            diagnostic: activation_diagnostic_ref(
                                request.workspace_id,
                                request.operation_id,
                                request.transaction,
                                &request.control_relation,
                                ActivationDiagnosticStage::Reconciliation,
                                error.to_string(),
                            ),
                        };
                    }
                };
                if selection.event().event_id() != event_id {
                    return ActivationOperationMarkerOutcome::Unknown {
                        diagnostic: activation_diagnostic_ref(
                            request.workspace_id,
                            request.operation_id,
                            request.transaction,
                            &request.control_relation,
                            ActivationDiagnosticStage::Reconciliation,
                            "operation marker does not name the selected event at the exact control horizon",
                        ),
                    };
                }
                ActivationOperationMarkerOutcome::Selected {
                    selection: SelectedEpochRecord::from_exact_readback(&selection),
                    chain_after_readback: chain,
                    acknowledgement: ActivationAcknowledgementMarker::Absent,
                    evidence: fact.evidence(),
                }
            }
            ActivationReconciliationDisposition::ProvedNotSelected => {
                ActivationOperationMarkerOutcome::ProvedNotSelected {
                    unchanged_chain: chain,
                    evidence: fact.evidence(),
                }
            }
        }
    }
}

/// Bind one exact activation-control snapshot to the candidate session already
/// owned by `assembly`, then register its logical provider into that same
/// candidate catalog.
///
/// Ownership of the assembly is consumed. On any schema, snapshot, session,
/// or catalog error, the candidate is dropped rather than exposing a partially
/// registered catalog. The returned authority retains the same cloned
/// [`SessionState`] identity as the assembly; no parallel context is created.
pub async fn register_activation_control_provider(
    mut assembly: ProgrammaticSchemaAssembly,
    pin: ExactDeltaPin,
    table: DeltaTable,
) -> Result<
    (
        ProgrammaticSchemaAssembly,
        Arc<ActivationControlDeltaProvider>,
    ),
    ActivationControlError,
> {
    let authority = Arc::new(
        ActivationControlDeltaProvider::try_from_loaded_table(
            Arc::new(assembly.candidate_state()),
            pin,
            table,
        )
        .await?,
    );
    let contract = Arc::clone(authority.contract());
    assembly
        .register_provider(ProviderInput::new(
            ProgrammaticRelationId::new(ACTIVATION_CONTROL_RELATION_ID),
            contract.qualifier().clone(),
            contract,
            authority.provider(),
        ))
        .map_err(|error| ActivationControlError::ProgrammaticAssembly(error.to_string()))?;
    Ok((assembly, authority))
}

/// Predicate scope whose completed DataFusion scan constitutes the read
/// horizon for one activation-control readback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationControlReadScope {
    All,
    Workspace(WorkspaceId),
}

/// Complete, exact-snapshot activation-control readback for one explicit
/// relational scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationControlReadback {
    control_relation: ActivationControlRelationPin,
    active_recovery_fence: WriterFence,
    scope: ActivationControlReadScope,
    rows: Vec<PersistedActivationControlRow>,
    history_digest: [u8; 32],
}

impl ActivationControlReadback {
    fn try_new(
        control_relation: ActivationControlRelationPin,
        active_recovery_fence: WriterFence,
        scope: ActivationControlReadScope,
        mut rows: Vec<PersistedActivationControlRow>,
    ) -> Result<Self, ActivationControlError> {
        for row in &rows {
            if row.control_predecessor.table().canonical_root()
                != control_relation.table().canonical_root()
                || row.control_commit_version > control_relation.table().version()
            {
                return Err(ActivationControlError::RowOutsideReadHorizon {
                    event_id: row.row.event_id,
                });
            }
            if let ActivationControlReadScope::Workspace(workspace_id) = scope
                && row.row.workspace_id != workspace_id
            {
                return Err(ActivationControlError::RowOutsideWorkspaceScope {
                    expected: workspace_id,
                    observed: row.row.workspace_id,
                    event_id: row.row.event_id,
                });
            }
        }
        rows.sort_by_key(|row| {
            (
                *row.row.workspace_id.as_bytes(),
                row.row.ordinal.get(),
                *row.row.event_id.as_bytes(),
            )
        });
        let mut digest = FramedDigest::new(READBACK_DIGEST_DOMAIN);
        digest.frame(
            control_relation
                .table()
                .canonical_root()
                .as_str()
                .as_bytes(),
        );
        digest.frame(&control_relation.table().version().to_be_bytes());
        digest.frame(control_relation.binding().fingerprint());
        digest.frame(active_recovery_fence.lease_id.as_bytes());
        digest.frame(&active_recovery_fence.generation.get().to_be_bytes());
        match scope {
            ActivationControlReadScope::All => digest.frame(&[0]),
            ActivationControlReadScope::Workspace(workspace_id) => {
                digest.frame(&[1]);
                digest.frame(workspace_id.as_bytes());
            }
        }
        for row in &rows {
            digest.frame(&row.canonical_digest());
        }
        Ok(Self {
            control_relation,
            active_recovery_fence,
            scope,
            rows,
            history_digest: digest.finish(),
        })
    }

    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    #[must_use]
    pub const fn scope(&self) -> ActivationControlReadScope {
        self.scope
    }

    #[must_use]
    pub fn rows(&self) -> &[PersistedActivationControlRow] {
        &self.rows
    }

    /// Resolve the reversible exact-version vector carried by one durable
    /// activation event. This is the cold-restart input for rebuilding the
    /// selected epoch's programmatic session.
    #[must_use]
    pub fn table_versions_for_event(
        &self,
        event_id: ActivationEventId,
    ) -> Option<&Arc<TableVersionSet>> {
        self.rows
            .iter()
            .find(|row| row.row.event_id == event_id)
            .map(PersistedActivationControlRow::table_versions)
    }

    #[must_use]
    pub const fn history_digest(&self) -> &[u8; 32] {
        &self.history_digest
    }
}

/// One completed, exact activation-control read horizon for a workspace.
///
/// The relation pin fixes the Delta root, version, and executable provider/schema binding. The
/// digest additionally binds the workspace scope, independently observed recovery fence, and every
/// decoded row in the completed scan. This is durable-read evidence, not a separately selectable
/// current pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationControlHorizon {
    workspace_id: WorkspaceId,
    control_relation: ActivationControlRelationPin,
    active_recovery_fence: WriterFence,
    history_digest: [u8; 32],
    history_row_count: u64,
}

impl ActivationControlHorizon {
    fn try_from_readback(
        readback: &ActivationControlReadback,
        workspace_id: WorkspaceId,
    ) -> Result<Self, ActivationControlError> {
        if readback.scope != ActivationControlReadScope::Workspace(workspace_id) {
            return Err(ActivationControlError::ReadScopeMismatch {
                expected: workspace_id,
                observed: readback.scope,
            });
        }
        let history_row_count = u64::try_from(readback.rows.len())
            .map_err(|_| ActivationControlError::HistoryRowCountOverflow)?;
        Ok(Self {
            workspace_id,
            control_relation: readback.control_relation.clone(),
            active_recovery_fence: readback.active_recovery_fence,
            history_digest: readback.history_digest,
            history_row_count,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        workspace_id: WorkspaceId,
        control_relation: ActivationControlRelationPin,
        active_recovery_fence: WriterFence,
        history_digest: [u8; 32],
        history_row_count: u64,
    ) -> Self {
        Self {
            workspace_id,
            control_relation,
            active_recovery_fence,
            history_digest,
            history_row_count,
        }
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    #[must_use]
    pub const fn active_recovery_fence(&self) -> WriterFence {
        self.active_recovery_fence
    }

    #[must_use]
    pub const fn history_digest(&self) -> &[u8; 32] {
        &self.history_digest
    }

    #[must_use]
    pub const fn history_row_count(&self) -> u64 {
        self.history_row_count
    }
}

/// Lawful empty-head result from one exact activation-control read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenesisRequiredActivation {
    workspace_id: WorkspaceId,
    writer_fence: WriterFence,
    control_horizon: ActivationControlHorizon,
}

impl GenesisRequiredActivation {
    #[must_use]
    pub(crate) const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub(crate) const fn writer_fence(&self) -> WriterFence {
        self.writer_fence
    }

    #[must_use]
    pub(crate) const fn control_horizon(&self) -> &ActivationControlHorizon {
        &self.control_horizon
    }
}

/// Exact selected activation reconstructed from one completed durable control readback.
///
/// Construction is private to this module so an event, reversible vector, proof reference, fence,
/// or horizon cannot be independently supplied by a caller. The public phase-owned
/// `SelectedEpochRecord` consumes this value rather than accepting those components separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactSelectedActivation {
    event: ActivationEvent,
    table_versions: Arc<TableVersionSet>,
    chain: ActivationChain,
    control_horizon: ActivationControlHorizon,
}

impl ExactSelectedActivation {
    #[must_use]
    pub(crate) const fn event(&self) -> ActivationEvent {
        self.event
    }

    #[must_use]
    pub(crate) const fn table_versions(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
    }

    #[must_use]
    pub(crate) const fn chain(&self) -> &ActivationChain {
        &self.chain
    }

    #[must_use]
    pub(crate) const fn writer_fence(&self) -> WriterFence {
        self.event.execution_fence()
    }

    #[must_use]
    pub(crate) const fn control_horizon(&self) -> &ActivationControlHorizon {
        &self.control_horizon
    }

    #[must_use]
    pub(crate) const fn proof_reference(&self) -> ProofReceiptRef {
        self.event.pins().proof_receipt
    }
}

/// Exact cold-start conclusion from the durable activation-control relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExactActivationControlSelection {
    GenesisRequired(GenesisRequiredActivation),
    Selected(ExactSelectedActivation),
}

impl ExactActivationControlSelection {
    fn try_from_readback(
        readback: &ActivationControlReadback,
        chain: &ActivationChain,
        workspace_id: WorkspaceId,
    ) -> Result<Self, DeltaActivationRuntimeAuthoritySnapshotError> {
        let horizon = ActivationControlHorizon::try_from_readback(readback, workspace_id)
            .map_err(DeltaActivationRuntimeAuthoritySnapshotError::Control)?;
        let Some(event) = chain.head_event().copied() else {
            return Ok(Self::GenesisRequired(GenesisRequiredActivation {
                workspace_id,
                writer_fence: horizon.active_recovery_fence,
                control_horizon: horizon,
            }));
        };
        if !recovery_fence_authorizes(event.execution_fence(), horizon.active_recovery_fence) {
            return Err(
                DeltaActivationRuntimeAuthoritySnapshotError::SelectionFenceNotAuthorized {
                    selected: event.execution_fence(),
                    active: horizon.active_recovery_fence,
                },
            );
        }
        let table_versions = readback
            .table_versions_for_event(event.event_id())
            .cloned()
            .ok_or(
                DeltaActivationRuntimeAuthoritySnapshotError::SelectedTableVersionsMissing(
                    event.event_id(),
                ),
            )?;
        if table_versions.reference() != event.pins().table_versions {
            return Err(DeltaActivationRuntimeAuthoritySnapshotError::Control(
                ActivationControlError::TableVersionSetReferenceMismatch,
            ));
        }
        Ok(Self::Selected(ExactSelectedActivation {
            event,
            table_versions,
            chain: chain.clone(),
            control_horizon: horizon,
        }))
    }
}

/// Stable stage vocabulary for one relational activation diagnostic fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationDiagnosticStage {
    SchemaBinding,
    DeltaAppend,
    DeltaMarkerRead,
    ControlReadback,
    RowDecode,
    Reconciliation,
}

impl ActivationDiagnosticStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaBinding => "schema_binding",
            Self::DeltaAppend => "delta_append",
            Self::DeltaMarkerRead => "delta_marker_read",
            Self::ControlReadback => "control_readback",
            Self::RowDecode => "row_decode",
            Self::Reconciliation => "reconciliation",
        }
    }
}

/// Canonical diagnostic relation row. The reference is derived from the full
/// exact fact; callers cannot supply arbitrary receipt bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationDiagnosticFact {
    diagnostic: DiagnosticRef,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
    stage: ActivationDiagnosticStage,
    detail: Arc<str>,
}

impl ActivationDiagnosticFact {
    pub fn try_new(
        workspace_id: WorkspaceId,
        operation_id: OperationId,
        transaction: TransactionRef,
        control_relation: ActivationControlRelationPin,
        stage: ActivationDiagnosticStage,
        detail: impl Into<Arc<str>>,
    ) -> Result<Self, ActivationControlError> {
        let detail = detail.into();
        if detail.trim().is_empty() {
            return Err(ActivationControlError::EmptyDiagnosticDetail);
        }
        let mut digest = FramedDigest::new(DIAGNOSTIC_DIGEST_DOMAIN);
        digest.frame(workspace_id.as_bytes());
        digest.frame(operation_id.as_bytes());
        digest.frame(transaction.as_bytes());
        digest.frame(control_relation.fingerprint());
        digest.frame(stage.as_str().as_bytes());
        digest.frame(detail.as_bytes());
        Ok(Self {
            diagnostic: DiagnosticRef::from_bytes(digest.finish()),
            workspace_id,
            operation_id,
            transaction,
            control_relation,
            stage,
            detail,
        })
    }

    #[must_use]
    pub const fn diagnostic(&self) -> DiagnosticRef {
        self.diagnostic
    }

    /// Materialize the exact diagnostic relation row to Arrow.
    pub fn to_record_batch(&self) -> Result<RecordBatch, ActivationControlError> {
        let schema = evidence_schema(
            "control.activation_diagnostic.v1",
            vec![
                evidence_field("diagnostic_ref", DataType::FixedSizeBinary(32), false),
                evidence_field("workspace_id", DataType::FixedSizeBinary(16), false),
                evidence_field("operation_id", DataType::FixedSizeBinary(16), false),
                evidence_field("transaction", DataType::FixedSizeBinary(32), false),
                evidence_field("control_root", DataType::Utf8, false),
                evidence_field("control_version", DataType::UInt64, false),
                evidence_field(
                    "control_binding_fingerprint",
                    DataType::FixedSizeBinary(32),
                    false,
                ),
                evidence_field("stage", DataType::Utf8, false),
                evidence_field("detail", DataType::Utf8, false),
            ],
        );
        RecordBatch::try_new(
            schema,
            vec![
                fixed_array::<32>([Some(*self.diagnostic.as_bytes())])?,
                fixed_array::<16>([Some(*self.workspace_id.as_bytes())])?,
                fixed_array::<16>([Some(*self.operation_id.as_bytes())])?,
                fixed_array::<32>([Some(*self.transaction.as_bytes())])?,
                Arc::new(StringArray::from(vec![
                    self.control_relation.table().canonical_root().as_str(),
                ])),
                Arc::new(UInt64Array::from(vec![
                    self.control_relation.table().version(),
                ])),
                fixed_array::<32>([Some(*self.control_relation.fingerprint())])?,
                Arc::new(StringArray::from(vec![self.stage.as_str()])),
                Arc::new(StringArray::from(vec![self.detail.as_ref()])),
            ],
        )
        .map_err(|error| ActivationControlError::Arrow(error.to_string()))
    }
}

/// Exact conclusion encoded by one reconciliation evidence relation row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationReconciliationDisposition {
    Selected(ActivationEventId),
    ProvedNotSelected,
}

impl ActivationReconciliationDisposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Selected(_) => "selected",
            Self::ProvedNotSelected => "proved_not_selected",
        }
    }
}

/// Canonical evidence fact derived from an exact Delta marker lookup and one
/// complete exact-snapshot control scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationFact {
    evidence: ReconciliationEvidenceRef,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    transaction: TransactionRef,
    operation_selection: OperationSelectionRef,
    expected_control: ActivationControlRelationPin,
    observed_control: ActivationControlRelationPin,
    active_recovery_fence: WriterFence,
    marker_version: Option<i64>,
    disposition: ActivationReconciliationDisposition,
    history_digest: [u8; 32],
    history_row_count: u64,
}

impl ActivationReconciliationFact {
    pub fn try_new(
        request: &super::activation_transaction::ActivationOperationMarkerRequest,
        readback: &ActivationControlReadback,
        marker_version: Option<i64>,
    ) -> Result<Self, ActivationControlError> {
        if readback.scope != ActivationControlReadScope::Workspace(request.workspace_id) {
            return Err(ActivationControlError::ReadScopeMismatch {
                expected: request.workspace_id,
                observed: readback.scope,
            });
        }
        if request.active_recovery_fence != readback.active_recovery_fence {
            return Err(ActivationControlError::RecoveryFenceMismatch);
        }
        if request.control_relation.table().canonical_root()
            != readback.control_relation.table().canonical_root()
            || readback.control_relation.table().version()
                < request.control_relation.table().version()
        {
            return Err(ActivationControlError::ReconciliationHorizonMismatch);
        }
        require_same_contract_binding(
            request.control_relation.binding(),
            readback.control_relation.binding(),
        )?;
        let exact_matches = readback
            .rows
            .iter()
            .filter(|candidate| {
                let row = candidate.row;
                row.workspace_id == request.workspace_id
                    && row.operation_id == request.operation_id
                    && row.event_id == request.event_id
                    && row.commit.transaction == request.transaction
                    && row.commit.operation_selection == request.operation_selection
                    && row.execution_fence == request.execution_fence
                    && row.predecessor_epoch == request.expected_head
            })
            .collect::<Vec<_>>();
        let partial_match = readback.rows.iter().any(|candidate| {
            let row = candidate.row;
            row.operation_id == request.operation_id
                || row.event_id == request.event_id
                || row.commit.transaction == request.transaction
                || row.commit.operation_selection == request.operation_selection
        });
        let disposition = match marker_version {
            Some(0) if exact_matches.len() == 1 => {
                ActivationReconciliationDisposition::Selected(request.event_id)
            }
            Some(0) => return Err(ActivationControlError::MarkerRowDisagreement),
            Some(observed) => {
                return Err(ActivationControlError::MarkerVersionMismatch {
                    expected: 0,
                    observed,
                });
            }
            None if exact_matches.is_empty() && !partial_match => {
                ActivationReconciliationDisposition::ProvedNotSelected
            }
            None => return Err(ActivationControlError::MarkerRowDisagreement),
        };
        let history_row_count = u64::try_from(readback.rows.len())
            .map_err(|_| ActivationControlError::HistoryRowCountOverflow)?;
        let mut digest = FramedDigest::new(RECONCILIATION_DIGEST_DOMAIN);
        digest.frame(request.workspace_id.as_bytes());
        digest.frame(request.operation_id.as_bytes());
        digest.frame(request.event_id.as_bytes());
        digest.frame(request.transaction.as_bytes());
        digest.frame(request.operation_selection.as_bytes());
        digest.frame(request.control_relation.fingerprint());
        digest.frame(readback.control_relation.fingerprint());
        digest.frame(request.active_recovery_fence.lease_id.as_bytes());
        digest.frame(&request.active_recovery_fence.generation.get().to_be_bytes());
        match marker_version {
            Some(version) => {
                digest.frame(&[1]);
                digest.frame(&version.to_be_bytes());
            }
            None => digest.frame(&[0]),
        }
        digest.frame(disposition.as_str().as_bytes());
        if let ActivationReconciliationDisposition::Selected(event) = disposition {
            digest.frame(event.as_bytes());
        }
        digest.frame(&readback.history_digest);
        digest.frame(&history_row_count.to_be_bytes());
        Ok(Self {
            evidence: ReconciliationEvidenceRef::from_bytes(digest.finish()),
            workspace_id: request.workspace_id,
            operation_id: request.operation_id,
            transaction: request.transaction,
            operation_selection: request.operation_selection,
            expected_control: request.control_relation.clone(),
            observed_control: readback.control_relation.clone(),
            active_recovery_fence: request.active_recovery_fence,
            marker_version,
            disposition,
            history_digest: readback.history_digest,
            history_row_count,
        })
    }

    #[must_use]
    pub const fn evidence(&self) -> ReconciliationEvidenceRef {
        self.evidence
    }

    #[must_use]
    pub const fn disposition(&self) -> ActivationReconciliationDisposition {
        self.disposition
    }

    /// Materialize the exact reconciliation evidence relation row to Arrow.
    pub fn to_record_batch(&self) -> Result<RecordBatch, ActivationControlError> {
        let schema = evidence_schema(
            "control.activation_reconciliation_evidence.v1",
            vec![
                evidence_field("evidence_ref", DataType::FixedSizeBinary(32), false),
                evidence_field("workspace_id", DataType::FixedSizeBinary(16), false),
                evidence_field("operation_id", DataType::FixedSizeBinary(16), false),
                evidence_field("transaction", DataType::FixedSizeBinary(32), false),
                evidence_field("operation_selection", DataType::FixedSizeBinary(32), false),
                evidence_field("expected_control_root", DataType::Utf8, false),
                evidence_field("expected_control_version", DataType::UInt64, false),
                evidence_field("observed_control_root", DataType::Utf8, false),
                evidence_field("observed_control_version", DataType::UInt64, false),
                evidence_field(
                    "active_recovery_lease_id",
                    DataType::FixedSizeBinary(16),
                    false,
                ),
                evidence_field("active_recovery_generation", DataType::UInt64, false),
                evidence_field("marker_version", DataType::Int64, true),
                evidence_field("disposition", DataType::Utf8, false),
                evidence_field("selected_event_id", DataType::FixedSizeBinary(32), true),
                evidence_field("history_digest", DataType::FixedSizeBinary(32), false),
                evidence_field("history_row_count", DataType::UInt64, false),
            ],
        );
        let selected = match self.disposition {
            ActivationReconciliationDisposition::Selected(event) => Some(*event.as_bytes()),
            ActivationReconciliationDisposition::ProvedNotSelected => None,
        };
        RecordBatch::try_new(
            schema,
            vec![
                fixed_array::<32>([Some(*self.evidence.as_bytes())])?,
                fixed_array::<16>([Some(*self.workspace_id.as_bytes())])?,
                fixed_array::<16>([Some(*self.operation_id.as_bytes())])?,
                fixed_array::<32>([Some(*self.transaction.as_bytes())])?,
                fixed_array::<32>([Some(*self.operation_selection.as_bytes())])?,
                Arc::new(StringArray::from(vec![
                    self.expected_control.table().canonical_root().as_str(),
                ])),
                Arc::new(UInt64Array::from(vec![
                    self.expected_control.table().version(),
                ])),
                Arc::new(StringArray::from(vec![
                    self.observed_control.table().canonical_root().as_str(),
                ])),
                Arc::new(UInt64Array::from(vec![
                    self.observed_control.table().version(),
                ])),
                fixed_array::<16>([Some(*self.active_recovery_fence.lease_id.as_bytes())])?,
                Arc::new(UInt64Array::from(vec![
                    self.active_recovery_fence.generation.get(),
                ])),
                Arc::new(Int64Array::from(vec![self.marker_version])),
                Arc::new(StringArray::from(vec![self.disposition.as_str()])),
                fixed_array::<32>([selected])?,
                fixed_array::<32>([Some(self.history_digest)])?,
                Arc::new(UInt64Array::from(vec![self.history_row_count])),
            ],
        )
        .map_err(|error| ActivationControlError::Arrow(error.to_string()))
    }
}

#[derive(Debug)]
struct ActivationStorageProvider {
    contract: Arc<SchemaContract>,
    inner: Arc<dyn TableProvider>,
}

impl ActivationStorageProvider {
    fn try_new(
        contract: Arc<SchemaContract>,
        inner: Arc<dyn TableProvider>,
    ) -> Result<Self, ActivationControlError> {
        validate_raw_storage_schema(contract.storage_schema(), &inner.schema())?;
        Ok(Self { contract, inner })
    }

    fn projected_logical_schema(
        &self,
        projection: Option<&[usize]>,
    ) -> Result<SchemaRef, datafusion::error::DataFusionError> {
        projection.map_or_else(
            || Ok(Arc::clone(self.contract.logical_schema())),
            |projection| {
                self.contract
                    .project_logical_schema(projection)
                    .map_err(|error| datafusion::error::DataFusionError::External(Box::new(error)))
            },
        )
    }

    fn reattach_plan(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        projection: Option<&[usize]>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        let target = self.projected_logical_schema(projection)?;
        let input = plan.schema();
        if input.fields().len() != target.fields().len() {
            return Err(datafusion::error::DataFusionError::Plan(format!(
                "activation-control projected storage field count {} differs from logical count {}",
                input.fields().len(),
                target.fields().len()
            )));
        }
        let expressions = target
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let expression = physical_col(input.field(index).name(), &input)?;
                let expression = if input.field(index).data_type() == field.data_type() {
                    expression
                } else {
                    cast(expression, &input, field.data_type().clone())?
                };
                Ok(ProjectionExpr {
                    expr: expression,
                    alias: field.name().to_owned(),
                })
            })
            .collect::<datafusion::error::Result<Vec<_>>>()?;
        Ok(Arc::new(ProjectionExec::try_new_with_schema_metadata(
            expressions,
            plan,
            &target,
        )?))
    }

    fn storage_filter(filter: &Expr) -> datafusion::error::Result<Expr> {
        filter
            .clone()
            .transform_down(|expression| match expression {
                Expr::Literal(ScalarValue::FixedSizeBinary(16 | 32, value), metadata) => Ok(
                    Transformed::yes(Expr::Literal(ScalarValue::Binary(value), metadata)),
                ),
                expression => Ok(Transformed::no(expression)),
            })
            .map(|value| value.data)
    }
}

#[async_trait::async_trait]
impl TableProvider for ActivationStorageProvider {
    fn schema(&self) -> SchemaRef {
        Arc::clone(self.contract.logical_schema())
    }

    fn constraints(&self) -> Option<&datafusion::common::Constraints> {
        self.inner.constraints()
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        let storage_filters = filters
            .iter()
            .map(Self::storage_filter)
            .collect::<datafusion::error::Result<Vec<_>>>()?;
        let plan = self
            .inner
            .scan(state, projection, &storage_filters, limit)
            .await?;
        self.reattach_plan(plan, projection.map(Vec::as_slice))
    }

    async fn scan_with_args<'a>(
        &self,
        state: &dyn Session,
        args: ScanArgs<'a>,
    ) -> datafusion::error::Result<ScanResult> {
        let storage_filters = args
            .filters()
            .map(|filters| {
                filters
                    .iter()
                    .map(Self::storage_filter)
                    .collect::<datafusion::error::Result<Vec<_>>>()
            })
            .transpose()?;
        let storage_args = ScanArgs::default()
            .with_projection(args.projection())
            .with_filters(storage_filters.as_deref())
            .with_limit(args.limit())
            .with_statistics_requests(args.statistics_requests());
        let result = self.inner.scan_with_args(state, storage_args).await?;
        Ok(self
            .reattach_plan(result.into_inner(), args.projection())?
            .into())
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
        let storage_filters = filters
            .iter()
            .map(|filter| Self::storage_filter(filter))
            .collect::<datafusion::error::Result<Vec<_>>>()?;
        self.inner
            .supports_filters_pushdown(&storage_filters.iter().collect::<Vec<_>>())
    }

    fn statistics(&self) -> Option<datafusion::common::Statistics> {
        self.inner.statistics()
    }
}

fn validate_raw_storage_schema(
    expected: &SchemaRef,
    actual: &SchemaRef,
) -> Result<(), ActivationControlError> {
    if expected.fields().len() != actual.fields().len() {
        return Err(ActivationControlError::StorageSchemaMismatch(format!(
            "field count {} != {}",
            actual.fields().len(),
            expected.fields().len()
        )));
    }
    for (ordinal, (expected, actual)) in expected.fields().iter().zip(actual.fields()).enumerate() {
        if expected.name() != actual.name()
            || expected.data_type() != actual.data_type()
            || expected.is_nullable() != actual.is_nullable()
            || (!actual.metadata().is_empty() && actual.metadata() != expected.metadata())
        {
            return Err(ActivationControlError::StorageSchemaMismatch(format!(
                "field {ordinal} differs: expected={expected:?}, actual={actual:?}"
            )));
        }
    }
    if !actual.metadata().is_empty() && actual.metadata() != expected.metadata() {
        return Err(ActivationControlError::StorageSchemaMismatch(
            "schema metadata differs from the exact storage contract".to_owned(),
        ));
    }
    Ok(())
}

fn require_same_contract_binding(
    expected: &SealedActivationControlBinding,
    observed: &SealedActivationControlBinding,
) -> Result<(), ActivationControlError> {
    if expected.physical_binding_id() != observed.physical_binding_id()
        || expected.source_schema_identity() != observed.source_schema_identity()
        || expected.qualifier() != observed.qualifier()
        || expected.logical_schema_digest() != observed.logical_schema_digest()
        || expected.storage_schema_digest() != observed.storage_schema_digest()
    {
        return Err(ActivationControlError::ContractBindingMismatch);
    }
    Ok(())
}

fn table_version_sets_array(
    rows: &[PersistedActivationControlRow],
) -> Result<ArrayRef, ActivationControlError> {
    let fields = table_version_component_fields(false);
    let values = StructBuilder::new(
        fields,
        vec![
            Box::new(StringBuilder::new()),
            Box::new(StringBuilder::new()),
            Box::new(UInt64Builder::new()),
        ],
    );
    let mut lists = ListBuilder::new(values).with_field(table_version_component_item_field(false));
    for row in rows {
        for (relation_id, pin) in row.table_versions.components() {
            let values = lists.values();
            values
                .field_builder::<StringBuilder>(COMPONENT_RELATION_ID)
                .ok_or_else(|| {
                    ActivationControlError::Arrow(
                        "table-version relation-id builder type differs".to_owned(),
                    )
                })?
                .append_value(relation_id);
            values
                .field_builder::<StringBuilder>(COMPONENT_DELTA_ROOT)
                .ok_or_else(|| {
                    ActivationControlError::Arrow(
                        "table-version Delta-root builder type differs".to_owned(),
                    )
                })?
                .append_value(pin.canonical_root().as_str());
            values
                .field_builder::<UInt64Builder>(COMPONENT_DELTA_VERSION)
                .ok_or_else(|| {
                    ActivationControlError::Arrow(
                        "table-version Delta-version builder type differs".to_owned(),
                    )
                })?
                .append_value(pin.version());
            values.append(true);
        }
        lists.append(true);
    }
    Ok(Arc::new(lists.finish()))
}

fn required_table_version_set(
    batch: &RecordBatch,
    column: usize,
    row: usize,
) -> Result<TableVersionSet, ActivationControlError> {
    let lists = batch
        .column(column)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "List<Struct<relation_id,delta_root,delta_version>>",
            actual: batch.column(column).data_type().clone(),
        })?;
    if lists.is_null(row) {
        return Err(ActivationControlError::UnexpectedNull { column, row });
    }
    let values = lists.value(row);
    let components = values
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "List<Struct<relation_id,delta_root,delta_version>>",
            actual: values.data_type().clone(),
        })?;
    let relation_ids = components
        .column(COMPONENT_RELATION_ID)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "table-version relation_id Utf8",
            actual: components.column(COMPONENT_RELATION_ID).data_type().clone(),
        })?;
    let roots = components
        .column(COMPONENT_DELTA_ROOT)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "table-version delta_root Utf8",
            actual: components.column(COMPONENT_DELTA_ROOT).data_type().clone(),
        })?;
    let versions = components
        .column(COMPONENT_DELTA_VERSION)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "table-version delta_version UInt64",
            actual: components
                .column(COMPONENT_DELTA_VERSION)
                .data_type()
                .clone(),
        })?;
    let mut decoded = Vec::with_capacity(components.len());
    let mut previous_relation_id: Option<&str> = None;
    for component in 0..components.len() {
        if components.is_null(component)
            || relation_ids.is_null(component)
            || roots.is_null(component)
            || versions.is_null(component)
        {
            return Err(ActivationControlError::NullTableVersionComponent { row, component });
        }
        let relation_id = relation_ids.value(component);
        if previous_relation_id.is_some_and(|previous| previous >= relation_id) {
            return Err(
                ActivationControlError::NonCanonicalTableVersionComponentOrder { row, component },
            );
        }
        previous_relation_id = Some(relation_id);
        let observed_root = roots.value(component);
        let parsed = Url::parse(observed_root).map_err(|error| {
            ActivationControlError::InvalidTableVersionRoot {
                relation_id: relation_id.to_owned(),
                detail: error.to_string(),
            }
        })?;
        let pin = ExactDeltaPin::new(&parsed, versions.value(component)).map_err(|error| {
            ActivationControlError::InvalidTableVersionRoot {
                relation_id: relation_id.to_owned(),
                detail: error.to_string(),
            }
        })?;
        if pin.canonical_root().as_str() != observed_root {
            return Err(ActivationControlError::NonCanonicalTableVersionRoot {
                relation_id: relation_id.to_owned(),
                observed: observed_root.to_owned(),
                canonical: pin.canonical_root().to_string(),
            });
        }
        decoded.push((Arc::<str>::from(relation_id), pin));
    }
    TableVersionSet::try_new(decoded).map_err(ActivationControlError::from)
}

fn fixed_array<const WIDTH: usize>(
    values: impl IntoIterator<Item = Option<[u8; WIDTH]>>,
) -> Result<ArrayRef, ActivationControlError> {
    let mut builder = FixedSizeBinaryBuilder::new(i32::try_from(WIDTH).expect("small width"));
    for value in values {
        if let Some(value) = value {
            builder
                .append_value(value)
                .map_err(|error| ActivationControlError::Arrow(error.to_string()))?;
        } else {
            builder.append_null();
        }
    }
    Ok(Arc::new(builder.finish()))
}

fn fixed_column(
    batch: &RecordBatch,
    column: usize,
) -> Result<&FixedSizeBinaryArray, ActivationControlError> {
    batch
        .column(column)
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "FixedSizeBinary",
            actual: batch.column(column).data_type().clone(),
        })
}

fn required_fixed<const WIDTH: usize>(
    batch: &RecordBatch,
    column: usize,
    row: usize,
) -> Result<[u8; WIDTH], ActivationControlError> {
    optional_fixed(batch, column, row)?
        .ok_or(ActivationControlError::UnexpectedNull { column, row })
}

fn optional_fixed<const WIDTH: usize>(
    batch: &RecordBatch,
    column: usize,
    row: usize,
) -> Result<Option<[u8; WIDTH]>, ActivationControlError> {
    let array = fixed_column(batch, column)?;
    if array.is_null(row) {
        return Ok(None);
    }
    let value: [u8; WIDTH] =
        array
            .value(row)
            .try_into()
            .map_err(|_| ActivationControlError::IdentityWidth {
                column,
                row,
                expected: WIDTH,
                actual: array.value(row).len(),
            })?;
    Ok(Some(value))
}

fn required_u64(
    batch: &RecordBatch,
    column: usize,
    row: usize,
) -> Result<u64, ActivationControlError> {
    let array = batch
        .column(column)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "UInt64",
            actual: batch.column(column).data_type().clone(),
        })?;
    if array.is_null(row) {
        return Err(ActivationControlError::UnexpectedNull { column, row });
    }
    Ok(array.value(row))
}

fn required_string<'a>(
    batch: &'a RecordBatch,
    column: usize,
    row: usize,
) -> Result<&'a str, ActivationControlError> {
    let array = batch
        .column(column)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ActivationControlError::DecodedColumnType {
            column,
            expected: "Utf8",
            actual: batch.column(column).data_type().clone(),
        })?;
    if array.is_null(row) {
        return Err(ActivationControlError::UnexpectedNull { column, row });
    }
    Ok(array.value(row))
}

fn require_equal_32(
    batch: &RecordBatch,
    column: usize,
    row: usize,
    expected: &[u8; 32],
    meaning: &'static str,
) -> Result<(), ActivationControlError> {
    let observed = required_fixed::<32>(batch, column, row)?;
    if &observed != expected {
        return Err(ActivationControlError::DecodedEvidenceMismatch { meaning });
    }
    Ok(())
}

fn evidence_field(name: &str, data_type: DataType, nullable: bool) -> Field {
    Field::new(name, data_type, nullable)
}

fn evidence_schema(relation_id: &str, fields: Vec<Field>) -> SchemaRef {
    Arc::new(Schema::new_with_metadata(
        fields,
        HashMap::from([
            (RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned()),
            (RELATION_PROTOCOL_KEY.to_owned(), "1".to_owned()),
            (
                ARROW_UNIVERSE_KEY.to_owned(),
                ARROW_TYPE_UNIVERSE.to_owned(),
            ),
            (
                SEMANTIC_ENCODING_KEY.to_owned(),
                "typed-arrow-activation-evidence".to_owned(),
            ),
        ]),
    ))
}

struct FramedDigest(blake3::Hasher);

impl FramedDigest {
    fn new(domain: &[u8]) -> Self {
        let mut value = Self(blake3::Hasher::new());
        value.frame(domain);
        value
    }

    fn frame(&mut self, value: &[u8]) {
        self.0.update(&(value.len() as u64).to_be_bytes());
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

/// Fail-closed activation-control schema/provider/codec error.
#[derive(Debug, Error)]
pub enum ActivationControlError {
    #[error("activation-control schema contract failed: {0}")]
    SchemaContract(String),
    #[error("activation-control canonical Arrow schema failed: {0}")]
    CanonicalSchema(String),
    #[error("activation-control session/schema binding failed: {0}")]
    Binding(String),
    #[error("activation-control provider contract failed: {0}")]
    Provider(String),
    #[error("activation-control candidate assembly failed: {0}")]
    ProgrammaticAssembly(String),
    #[error("exact Delta activation-control binding failed: {0}")]
    ExactDelta(String),
    #[error("Delta activation-control observation failed: {0}")]
    Delta(String),
    #[error("DataFusion activation-control operation failed: {0}")]
    DataFusion(String),
    #[error("activation event evidence failed: {0}")]
    ActivationEvidence(String),
    #[error("activation chain reconstruction failed: {0}")]
    ActivationChain(String),
    #[error("activation event {event_id:?} exact commit readback failed: {detail}")]
    HistoricalCommitReadback {
        event_id: ActivationEventId,
        detail: String,
    },
    #[error("session-bound activation append plan failed: {0}")]
    PlanBinding(String),
    #[error("Arrow activation-control row failed: {0}")]
    Arrow(String),
    #[error(transparent)]
    TableVersionSet(#[from] TableVersionSetError),
    #[error("activation row table-version reference differs from its reversible component set")]
    TableVersionSetReferenceMismatch,
    #[error("activation-control Delta storage schema differs: {0}")]
    StorageSchemaMismatch(String),
    #[error(
        "activation-control Delta property {key} differs: expected {expected}, observed {actual:?}"
    )]
    RequiredTableProperty {
        key: &'static str,
        expected: &'static str,
        actual: Option<String>,
    },
    #[error("activation-control Delta history is unexpectedly partitioned by {0:?}")]
    UnexpectedPartitionColumns(Vec<String>),
    #[error("invalid activation-control root: {0}")]
    InvalidControlRoot(String),
    #[error("invalid Delta root for table-version relation {relation_id}: {detail}")]
    InvalidTableVersionRoot { relation_id: String, detail: String },
    #[error(
        "noncanonical Delta root for table-version relation {relation_id}: observed {observed}, canonical {canonical}"
    )]
    NonCanonicalTableVersionRoot {
        relation_id: String,
        observed: String,
        canonical: String,
    },
    #[error("activation row {row} table-version component {component} contains a null")]
    NullTableVersionComponent { row: usize, component: usize },
    #[error(
        "activation row {row} table-version component {component} is not in canonical relation order"
    )]
    NonCanonicalTableVersionComponentOrder { row: usize, component: usize },
    #[error("activation-control predecessor version {0} cannot advance")]
    ControlVersionOverflow(u64),
    #[error("activation-control commit version differs: expected {expected}, observed {observed}")]
    CommitVersionMismatch { expected: u64, observed: u64 },
    #[error(
        "activation-control provider binding differs: expected {expected}, observed {observed}"
    )]
    ProviderBindingMismatch {
        expected: &'static str,
        observed: String,
    },
    #[error("decoded activation-control {meaning} differs from canonical evidence")]
    DecodedEvidenceMismatch { meaning: &'static str },
    #[error("decoded column {column} type differs: expected {expected}, observed {actual}")]
    DecodedColumnType {
        column: usize,
        expected: &'static str,
        actual: DataType,
    },
    #[error("decoded row {row} column {column} is unexpectedly null")]
    UnexpectedNull { column: usize, row: usize },
    #[error(
        "decoded identity width differs at row {row} column {column}: expected {expected}, observed {actual}"
    )]
    IdentityWidth {
        column: usize,
        row: usize,
        expected: usize,
        actual: usize,
    },
    #[error("predecessor event and predecessor epoch nullability disagree")]
    InconsistentPredecessorNullability,
    #[error("activation ordinal zero is invalid")]
    ZeroActivationOrdinal,
    #[error("writer generation zero is invalid")]
    ZeroWriterGeneration,
    #[error("operation ID zero is not an admitted durable operation")]
    ZeroOperationId,
    #[error("activation row {event_id:?} lies outside the exact read horizon")]
    RowOutsideReadHorizon { event_id: ActivationEventId },
    #[error(
        "activation row {event_id:?} belongs to workspace {observed:?}, outside requested workspace {expected:?}"
    )]
    RowOutsideWorkspaceScope {
        expected: WorkspaceId,
        observed: WorkspaceId,
        event_id: ActivationEventId,
    },
    #[error(
        "activation-control read scope differs: expected workspace {expected:?}, observed {observed:?}"
    )]
    ReadScopeMismatch {
        expected: WorkspaceId,
        observed: ActivationControlReadScope,
    },
    #[error("diagnostic detail is empty")]
    EmptyDiagnosticDetail,
    #[error("recovery fence differs from the exact complete control readback")]
    RecoveryFenceMismatch,
    #[error("reconciliation read horizon does not cover the request's exact control pin")]
    ReconciliationHorizonMismatch,
    #[error("reconciliation provider/schema binding differs from the request binding")]
    ContractBindingMismatch,
    #[error("Delta transaction marker and activation-control rows disagree")]
    MarkerRowDisagreement,
    #[error("Delta transaction marker version differs: expected {expected}, observed {observed}")]
    MarkerVersionMismatch { expected: i64, observed: i64 },
    #[error("activation-control history row count exceeds u64")]
    HistoryRowCountOverflow,
}

impl From<ExactDeltaProviderError> for ActivationControlError {
    fn from(error: ExactDeltaProviderError) -> Self {
        Self::ExactDelta(error.to_string())
    }
}

impl From<ProviderContractError> for ActivationControlError {
    fn from(error: ProviderContractError) -> Self {
        Self::Provider(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use datafusion::execution::SessionStateBuilder;
    use deltalake::DeltaTableBuilder;
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::fabric::activation::{ActivationAttempt, OverlaySegmentSetRef, PolicySetRef};
    use crate::fabric::activation_transaction::{
        ActivationAppendContract, ActivationAppendOutcome, ActivationEventPort,
        ActivationOperationMarkerOutcome, ActivationOperationMarkerPort,
    };
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins, ExecutionOwner,
        FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef, PrincipalId,
        ProgramReleaseRef, ProofReceiptRef, ProviderSetRef, ResourceEnvelopeRef,
    };
    use crate::fabric::epoch_runtime::FabricEpochRuntimeConfig;
    use crate::fabric::production_kernel::{ActiveWorkspace, WorkspaceSlot};
    use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
    use crate::fabric::writer_lease::WriterGenerationPortError;

    struct FixedWriterGeneration {
        workspace_id: WorkspaceId,
        fence: WriterFence,
    }

    impl DurableWriterGenerationPort for FixedWriterGeneration {
        fn allocate_next(
            &self,
            _workspace_id: WorkspaceId,
            _lease_id: LeaseId,
        ) -> Result<WriterGeneration, WriterGenerationPortError> {
            Err(WriterGenerationPortError::Corrupt)
        }

        fn observe_current(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<Option<WriterFence>, WriterGenerationPortError> {
            if workspace_id == self.workspace_id {
                Ok(Some(self.fence))
            } else {
                Ok(None)
            }
        }
    }

    fn bytes16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    fn bytes32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(seed)),
            generation: WriterGeneration::new(generation).unwrap(),
        }
    }

    fn table_versions(seed: u8) -> Arc<TableVersionSet> {
        Arc::new(
            TableVersionSet::try_new([
                (
                    Arc::<str>::from("system.programmatic_field_observation"),
                    ExactDeltaPin::new(
                        &Url::parse(&format!(
                            "s3://codefabric-test/table-version-set-{seed}/field"
                        ))
                        .unwrap(),
                        u64::from(seed) + 1,
                    )
                    .unwrap(),
                ),
                (
                    Arc::<str>::from("system.programmatic_relation_observation"),
                    ExactDeltaPin::new(
                        &Url::parse(&format!(
                            "s3://codefabric-test/table-version-set-{seed}/relation"
                        ))
                        .unwrap(),
                        u64::from(seed) + 2,
                    )
                    .unwrap(),
                ),
            ])
            .unwrap(),
        )
    }

    fn row(seed: u8) -> DurableActivationRow {
        let table_versions = table_versions(seed);
        DurableActivationRow {
            event_id: ActivationEventId::from_bytes(bytes32(seed)),
            workspace_id: WorkspaceId::from_bytes(bytes16(seed.wrapping_add(1))),
            operation_id: OperationId::from_bytes(bytes16(seed.wrapping_add(2))),
            predecessor_event_id: None,
            predecessor_epoch: ExpectedHead::Empty,
            ordinal: ActivationOrdinal::new(1).unwrap(),
            execution_fence: fence(seed.wrapping_add(3), 7),
            pins: FabricEpochPins {
                epoch: EpochId::from_bytes(bytes16(seed.wrapping_add(4))),
                input_release: InputReleaseRef::from_bytes(bytes32(seed.wrapping_add(5))),
                program_release: ProgramReleaseRef::from_bytes(bytes32(seed.wrapping_add(6))),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    bytes32(seed.wrapping_add(20)),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(bytes32(
                    seed.wrapping_add(21),
                )),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(bytes32(
                    seed.wrapping_add(22),
                )),
                source_generation: SourceGeneration::new(9),
                provider_set: ProviderSetRef::from_bytes(bytes32(seed.wrapping_add(7))),
                table_versions: table_versions.reference(),
                overlay_segments: OverlaySegmentSetRef::from_bytes(bytes32(seed.wrapping_add(9))),
                policy_set: PolicySetRef::from_bytes(bytes32(seed.wrapping_add(10))),
                resource_envelope: ResourceEnvelopeRef::from_bytes(bytes32(seed.wrapping_add(11))),
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(seed.wrapping_add(12))),
            },
            compatibility: CompatibilityClassRef::from_bytes(bytes32(seed.wrapping_add(13))),
            retention: RetentionPolicyRef::from_bytes(bytes32(seed.wrapping_add(14))),
            commit: DurableActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(bytes32(
                    seed.wrapping_add(15),
                )),
                transaction: TransactionRef::from_bytes(bytes32(seed.wrapping_add(16))),
            },
        }
    }

    fn append_contract(
        row: DurableActivationRow,
        table_versions: Arc<TableVersionSet>,
        control_relation: ActivationControlRelationPin,
    ) -> ActivationAppendContract {
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: row.operation_id,
                idempotency_key: IdempotencyKey::from_bytes(bytes32(0xe1)),
            },
            ownership: CommandOwnership {
                workspace_id: row.workspace_id,
                principal_id: PrincipalId::from_bytes(bytes16(0xe2)),
                authorization: AuthorizationRef::from_bytes(bytes32(0xe3)),
            },
            expected_head: row.predecessor_epoch,
            writer_fence: row.execution_fence,
            pins: CommandPins {
                input_release: row.pins.input_release,
                program_release: row.pins.program_release,
                application_release: row.pins.application_release,
                source_authority: row.pins.source_authority,
                provider_release: row.pins.provider_release,
                source_generation: row.pins.source_generation,
                provider_set: row.pins.provider_set,
            },
            resources: row.pins.resource_envelope,
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: row.pins.epoch,
                proof_receipt: row.pins.proof_receipt,
            },
        };
        let attempt = ActivationAttempt::for_test(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(bytes16(0xe4)),
                fence: row.execution_fence,
            },
        );
        ActivationAppendContract::for_test(attempt, row, table_versions, control_relation)
    }

    fn control(version: u64, session: &SessionState) -> ActivationControlRelationPin {
        let codec = ActivationControlRowCodec::try_new().unwrap();
        let binding = SealedActivationControlBinding::try_from_session_and_contract(
            session,
            ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
            codec.contract(),
        )
        .unwrap();
        ActivationControlRelationPin::new(
            ExactDeltaPin::new(
                &Url::parse("s3://codefabric-test/activation-control").unwrap(),
                version,
            )
            .unwrap(),
            binding,
        )
    }

    #[test]
    fn programmatic_contract_has_stable_relation_and_field_identities() {
        let contract = activation_control_schema_contract().unwrap();
        let independently_reconstructed = activation_control_schema_contract().unwrap();
        assert_eq!(
            contract.logical_schema().as_ref(),
            independently_reconstructed.logical_schema().as_ref(),
            "canonical schema construction must not depend on HashMap iteration order"
        );
        assert_eq!(
            contract.storage_schema().as_ref(),
            independently_reconstructed.storage_schema().as_ref()
        );
        let session = SessionStateBuilder::new().with_default_features().build();
        assert_eq!(
            SealedActivationControlBinding::try_from_session_and_contract(
                &session,
                ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
                &contract,
            )
            .unwrap(),
            SealedActivationControlBinding::try_from_session_and_contract(
                &session,
                ACTIVATION_CONTROL_PROVIDER_BINDING_ID,
                &independently_reconstructed,
            )
            .unwrap()
        );
        assert_eq!(
            contract
                .logical_schema()
                .metadata()
                .get(RELATION_ID_METADATA_KEY)
                .map(String::as_str),
            Some(ACTIVATION_CONTROL_RELATION_ID)
        );
        assert_eq!(
            contract
                .storage_schema()
                .metadata()
                .get(RELATION_ID_METADATA_KEY)
                .map(String::as_str),
            Some(ACTIVATION_CONTROL_STORAGE_RELATION_ID)
        );
        assert_eq!(
            contract.logical_schema().field(EVENT_ID).data_type(),
            &DataType::FixedSizeBinary(32)
        );
        assert_eq!(
            contract.storage_schema().field(EVENT_ID).data_type(),
            &DataType::Binary
        );
        assert_eq!(
            contract
                .logical_schema()
                .field(EVENT_ID)
                .metadata()
                .get(FIELD_ID_METADATA_KEY)
                .map(String::as_str),
            Some("control.activation_event.v3.event_id")
        );
    }

    #[test]
    fn logical_and_delta_storage_codecs_round_trip_exact_rows() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let persisted =
            PersistedActivationControlRow::try_new(row(1), table_versions(1), control(7, &session))
                .unwrap();
        let codec = ActivationControlRowCodec::try_new().unwrap();

        let logical = codec
            .encode_logical(std::slice::from_ref(&persisted))
            .unwrap();
        assert_eq!(
            codec.decode_logical(&logical).unwrap(),
            vec![persisted.clone()]
        );

        let storage = codec
            .encode_storage(std::slice::from_ref(&persisted))
            .unwrap();
        assert_eq!(&storage.schema(), codec.contract().storage_schema());
        assert_eq!(codec.decode_storage(&storage).unwrap(), vec![persisted]);
    }

    #[test]
    fn predecessor_v2_activation_schema_is_not_a_reopen_authority() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let persisted =
            PersistedActivationControlRow::try_new(row(1), table_versions(1), control(7, &session))
                .unwrap();
        let codec = ActivationControlRowCodec::try_new().unwrap();
        let logical = codec.encode_logical(&[persisted]).unwrap();
        let mut predecessor_metadata = logical.schema().metadata().clone();
        predecessor_metadata.insert(
            RELATION_ID_METADATA_KEY.to_owned(),
            "control.activation_event.v2".to_owned(),
        );
        predecessor_metadata.insert(RELATION_PROTOCOL_KEY.to_owned(), "2".to_owned());
        predecessor_metadata.insert(
            PROVIDER_BINDING_KEY.to_owned(),
            "binding.delta.exact-snapshot.activation-event.v2".to_owned(),
        );
        let predecessor_schema = logical
            .schema()
            .as_ref()
            .clone()
            .with_metadata(predecessor_metadata);
        let predecessor =
            RecordBatch::try_new(Arc::new(predecessor_schema), logical.columns().to_vec()).unwrap();

        assert!(matches!(
            codec.decode_logical(&predecessor),
            Err(ActivationControlError::SchemaContract(_))
        ));
    }

    #[test]
    fn reversible_table_version_components_cannot_be_substituted_for_their_digest() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let predecessor = control(7, &session);
        assert!(matches!(
            PersistedActivationControlRow::try_new(row(1), table_versions(2), predecessor.clone()),
            Err(ActivationControlError::TableVersionSetReferenceMismatch)
        ));

        let codec = ActivationControlRowCodec::try_new().unwrap();
        let first =
            PersistedActivationControlRow::try_new(row(1), table_versions(1), predecessor.clone())
                .unwrap();
        let second =
            PersistedActivationControlRow::try_new(row(2), table_versions(2), predecessor).unwrap();
        let first_batch = codec.encode_logical(&[first]).unwrap();
        let second_batch = codec.encode_logical(&[second]).unwrap();
        let mut columns = first_batch.columns().to_vec();
        columns[TABLE_VERSION_COMPONENTS] =
            Arc::clone(second_batch.column(TABLE_VERSION_COMPONENTS));
        let substituted = RecordBatch::try_new(first_batch.schema(), columns).unwrap();
        assert!(matches!(
            codec.decode_logical(&substituted),
            Err(ActivationControlError::TableVersionSetReferenceMismatch)
        ));
    }

    #[test]
    fn codec_rejects_schema_binding_and_row_digest_substitution() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let persisted =
            PersistedActivationControlRow::try_new(row(1), table_versions(1), control(7, &session))
                .unwrap();
        let codec = ActivationControlRowCodec::try_new().unwrap();
        let logical = codec.encode_logical(&[persisted]).unwrap();

        let mut wrong_metadata = logical.schema().metadata().clone();
        wrong_metadata.insert(
            RELATION_ID_METADATA_KEY.to_owned(),
            "control.wrong.v1".to_owned(),
        );
        let wrong_schema = logical
            .schema()
            .as_ref()
            .clone()
            .with_metadata(wrong_metadata);
        let wrong =
            RecordBatch::try_new(Arc::new(wrong_schema), logical.columns().to_vec()).unwrap();
        assert!(matches!(
            codec.decode_logical(&wrong),
            Err(ActivationControlError::SchemaContract(_))
        ));

        let mut columns = logical.columns().to_vec();
        columns[ROW_DIGEST] = fixed_array::<32>([Some(bytes32(0xee))]).unwrap();
        let wrong_digest = RecordBatch::try_new(logical.schema(), columns).unwrap();
        assert!(matches!(
            codec.decode_logical(&wrong_digest),
            Err(ActivationControlError::DecodedEvidenceMismatch {
                meaning: "durable row digest"
            })
        ));
    }

    #[test]
    fn codec_rejects_unknown_provider_binding_and_invalid_target_version() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let persisted =
            PersistedActivationControlRow::try_new(row(1), table_versions(1), control(7, &session))
                .unwrap();
        let codec = ActivationControlRowCodec::try_new().unwrap();
        let logical = codec.encode_logical(&[persisted]).unwrap();

        let mut columns = logical.columns().to_vec();
        columns[4] = Arc::new(StringArray::from(vec!["binding.unknown"]));
        let wrong_binding = RecordBatch::try_new(logical.schema(), columns).unwrap();
        assert!(matches!(
            codec.decode_logical(&wrong_binding),
            Err(ActivationControlError::ProviderBindingMismatch { .. })
        ));

        let mut columns = logical.columns().to_vec();
        columns[2] = Arc::new(UInt64Array::from(vec![10]));
        let wrong_version = RecordBatch::try_new(logical.schema(), columns).unwrap();
        assert_eq!(required_u64(&wrong_version, 2, 0).unwrap(), 10);
        assert!(matches!(
            codec.decode_logical(&wrong_version),
            Err(ActivationControlError::CommitVersionMismatch {
                expected: 8,
                observed: 10
            })
        ));
    }

    #[test]
    fn diagnostic_refs_are_derived_from_exact_relational_facts() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let row = row(1);
        let control = control(7, &session);
        let first = ActivationDiagnosticFact::try_new(
            row.workspace_id,
            row.operation_id,
            row.commit.transaction,
            control.clone(),
            ActivationDiagnosticStage::DeltaAppend,
            "commit outcome not observable",
        )
        .unwrap();
        let second = ActivationDiagnosticFact::try_new(
            row.workspace_id,
            row.operation_id,
            row.commit.transaction,
            control,
            ActivationDiagnosticStage::DeltaAppend,
            "marker read unavailable",
        )
        .unwrap();
        assert_ne!(first.diagnostic(), second.diagnostic());
        let batch = first.to_record_batch().unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(
            batch.schema().metadata().get(RELATION_ID_METADATA_KEY),
            Some(&"control.activation_diagnostic.v1".to_owned())
        );
    }

    #[test]
    fn reconciliation_authority_requires_marker_row_agreement() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let semantic = row(1);
        let predecessor = control(7, &session);
        let persisted = PersistedActivationControlRow::try_new(
            semantic,
            table_versions(1),
            predecessor.clone(),
        )
        .unwrap();
        let observed = ActivationControlRelationPin::new(
            ExactDeltaPin::new(predecessor.table().canonical_root(), 8).unwrap(),
            predecessor.binding().clone(),
        );
        let recovery_fence = fence(50, 8);
        let readback = ActivationControlReadback::try_new(
            observed,
            recovery_fence,
            ActivationControlReadScope::Workspace(semantic.workspace_id),
            vec![persisted],
        )
        .unwrap();
        let request = super::super::activation_transaction::ActivationOperationMarkerRequest {
            workspace_id: semantic.workspace_id,
            operation_id: semantic.operation_id,
            event_id: semantic.event_id,
            expected_head: semantic.predecessor_epoch,
            execution_fence: semantic.execution_fence,
            active_recovery_fence: recovery_fence,
            transaction: semantic.commit.transaction,
            operation_selection: semantic.commit.operation_selection,
            control_relation: predecessor,
        };

        let selected = ActivationReconciliationFact::try_new(&request, &readback, Some(0)).unwrap();
        assert_eq!(
            selected.disposition(),
            ActivationReconciliationDisposition::Selected(semantic.event_id)
        );
        assert_eq!(selected.to_record_batch().unwrap().num_rows(), 1);
        assert!(matches!(
            ActivationReconciliationFact::try_new(&request, &readback, None),
            Err(ActivationControlError::MarkerRowDisagreement)
        ));
        assert!(matches!(
            ActivationReconciliationFact::try_new(&request, &readback, Some(1)),
            Err(ActivationControlError::MarkerVersionMismatch {
                expected: 0,
                observed: 1
            })
        ));
    }

    #[test]
    fn proved_not_selected_requires_complete_empty_match_set_and_absent_marker() {
        let session = SessionStateBuilder::new().with_default_features().build();
        let semantic = row(1);
        let predecessor = control(7, &session);
        let recovery_fence = fence(50, 8);
        let readback = ActivationControlReadback::try_new(
            predecessor.clone(),
            recovery_fence,
            ActivationControlReadScope::Workspace(semantic.workspace_id),
            Vec::new(),
        )
        .unwrap();
        let request = super::super::activation_transaction::ActivationOperationMarkerRequest {
            workspace_id: semantic.workspace_id,
            operation_id: semantic.operation_id,
            event_id: semantic.event_id,
            expected_head: semantic.predecessor_epoch,
            execution_fence: semantic.execution_fence,
            active_recovery_fence: recovery_fence,
            transaction: semantic.commit.transaction,
            operation_selection: semantic.commit.operation_selection,
            control_relation: predecessor,
        };
        let fact = ActivationReconciliationFact::try_new(&request, &readback, None).unwrap();
        assert_eq!(
            fact.disposition(),
            ActivationReconciliationDisposition::ProvedNotSelected
        );
    }

    #[tokio::test]
    async fn wp32_int_exact_delta_append_readback_and_marker_reconciliation_round_trip() {
        let temporary = TempDir::new().unwrap();
        let table_path = temporary.path().join("activation-control");
        fs::create_dir_all(&table_path).unwrap();
        let root = Url::from_directory_path(&table_path).unwrap();
        let (predecessor, table) = provision_activation_control_history(root.clone())
            .await
            .unwrap();
        let epoch_id = EpochId::from_bytes(bytes16(0xa1));
        let selected_epoch =
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
                .unwrap()
                .seal_for_test()
                .await
                .unwrap();
        let builder =
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
                .unwrap();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let candidate_session_id = assembly.candidate_state().session_id().to_owned();
        let (assembly, provider) =
            register_activation_control_provider(assembly, predecessor, table)
                .await
                .unwrap();
        assert_eq!(
            provider.control_relation().binding().session_id(),
            candidate_session_id
        );
        let sealed = assembly.seal(epoch_id).await.unwrap();
        let registered = sealed
            .relation(&ProgrammaticRelationId::new(ACTIVATION_CONTROL_RELATION_ID))
            .unwrap();
        assert!(Arc::ptr_eq(&registered.contract, provider.contract()));
        assert_eq!(sealed.session().state().session_id(), candidate_session_id);
        let session = Arc::new(sealed.session().state());
        let versions = Arc::clone(selected_epoch.table_version_set());
        let mut semantic = row(21);
        semantic.pins.epoch = epoch_id;
        semantic.pins.table_versions = versions.reference();
        let recovery_fence = fence(90, 8);
        let request_control = provider.control_relation().clone();
        let contract = append_contract(semantic, Arc::clone(&versions), request_control.clone());
        let outcome = provider.append_and_readback(contract).await;
        let (event, chain) = match outcome {
            ActivationAppendOutcome::Committed {
                selection,
                chain_after_readback,
            } => {
                assert_eq!(selection.table_versions().as_ref(), versions.as_ref());
                (selection.event(), chain_after_readback)
            }
            outcome => panic!("expected committed activation event, observed {outcome:?}"),
        };
        assert_eq!(event.event_id(), semantic.event_id);
        assert_eq!(chain.head_event(), Some(&event));

        let committed_pin = ExactDeltaPin::new(&root, 1).unwrap();
        let committed_table = DeltaTableBuilder::from_url(root.clone())
            .unwrap()
            .with_version(1)
            .load()
            .await
            .unwrap();
        let committed = Arc::new(
            ActivationControlDeltaProvider::try_from_loaded_table(
                Arc::clone(&session),
                committed_pin,
                committed_table,
            )
            .await
            .unwrap(),
        );
        assert_eq!(
            committed.control_relation().binding(),
            request_control.binding(),
            "the same session and executable contract must reproduce the binding"
        );
        assert_eq!(committed.statistics().add_actions().num_rows(), 1);
        assert_eq!(
            committed
                .statistics()
                .field("partition.event_id")
                .expect("non-partitioned activation column remains inspectable")
                .availability(),
            super::super::delta_exact::ExactDeltaStatisticAvailability::UnknownForFiles {
                file_count: 1,
                unknown_file_count: 1,
            }
        );
        let readback = committed.read_all(recovery_fence).await.unwrap();
        assert_eq!(readback.rows().len(), 1);
        assert_eq!(readback.rows()[0].row(), semantic);
        assert_eq!(
            readback
                .table_versions_for_event(semantic.event_id)
                .unwrap()
                .as_ref(),
            versions.as_ref()
        );
        let workspace_readback = committed
            .read_workspace(semantic.workspace_id, recovery_fence)
            .await
            .unwrap();
        assert_eq!(
            workspace_readback.scope(),
            ActivationControlReadScope::Workspace(semantic.workspace_id)
        );
        assert_eq!(workspace_readback.rows().len(), 1);
        let other_workspace = WorkspaceId::from_bytes(bytes16(0xf1));
        assert!(
            committed
                .read_workspace(other_workspace, recovery_fence)
                .await
                .unwrap()
                .rows()
                .is_empty()
        );

        let authority = DeltaActivationRuntimeAuthority::new(
            semantic.workspace_id,
            Arc::clone(&committed),
            Arc::new(FixedWriterGeneration {
                workspace_id: semantic.workspace_id,
                fence: semantic.execution_fence,
            }),
        );
        let startup_snapshot = authority.current_snapshot().await.unwrap();
        assert_eq!(startup_snapshot.chain, chain);
        assert_eq!(startup_snapshot.active_fence, semantic.execution_fence);
        let selected = match authority.current_selection().await.unwrap() {
            ExactActivationControlSelection::Selected(selected) => selected,
            ExactActivationControlSelection::GenesisRequired(_) => {
                panic!("committed workspace must reconstruct its selected activation")
            }
        };
        assert_eq!(selected.event(), event);
        assert_eq!(selected.chain(), &chain);
        assert_eq!(selected.table_versions().as_ref(), versions.as_ref());
        assert_eq!(selected.writer_fence(), semantic.execution_fence);
        assert_eq!(selected.proof_reference(), semantic.pins.proof_receipt);
        assert_eq!(
            selected.control_horizon().workspace_id(),
            semantic.workspace_id
        );
        assert_eq!(
            selected.control_horizon().control_relation(),
            committed.control_relation()
        );
        assert_eq!(selected.control_horizon().history_row_count(), 1);
        assert_eq!(
            selected.control_horizon().active_recovery_fence(),
            semantic.execution_fence
        );

        let restarted_fence = fence(91, semantic.execution_fence.generation.get() + 1);
        let restarted_authority = DeltaActivationRuntimeAuthority::new(
            semantic.workspace_id,
            Arc::clone(&committed),
            Arc::new(FixedWriterGeneration {
                workspace_id: semantic.workspace_id,
                fence: restarted_fence,
            }),
        );
        let restarted_selection = match restarted_authority.current_selection().await.unwrap() {
            ExactActivationControlSelection::Selected(selected) => selected,
            ExactActivationControlSelection::GenesisRequired(_) => {
                panic!("restart must retain the exact durable selection")
            }
        };
        assert_eq!(restarted_selection.event(), selected.event());
        assert_eq!(
            restarted_selection.table_versions().as_ref(),
            selected.table_versions().as_ref()
        );
        assert_eq!(
            restarted_selection.writer_fence(),
            selected.writer_fence(),
            "the selected fence comes from the durable event, not the new process guard"
        );
        assert_eq!(
            restarted_selection.proof_reference(),
            selected.proof_reference()
        );
        assert_eq!(
            restarted_selection.control_horizon().control_relation(),
            selected.control_horizon().control_relation()
        );
        assert_eq!(
            restarted_selection
                .control_horizon()
                .active_recovery_fence(),
            restarted_fence
        );

        let slot = WorkspaceSlot::empty(semantic.workspace_id);
        let predecessor = Arc::new(ActiveWorkspace::selection_probe(
            SelectedEpochRecord::from_exact_readback(&selected),
        ));
        slot.install_initial(Arc::clone(&predecessor)).unwrap();
        let predecessor_lease = slot.lease().unwrap();
        let successor = Arc::new(ActiveWorkspace::selection_probe(
            SelectedEpochRecord::from_exact_readback(&restarted_selection),
        ));
        let retained = slot.swap(Arc::clone(&successor)).unwrap();
        assert!(Arc::ptr_eq(retained.workspace(), &predecessor));
        assert!(Arc::ptr_eq(predecessor_lease.workspace(), &predecessor));
        assert_eq!(
            predecessor_lease
                .workspace()
                .selection()
                .control_horizon()
                .active_recovery_fence(),
            semantic.execution_fence,
            "an old workspace lease must retain its exact pre-swap horizon"
        );
        let successor_lease = slot.lease().unwrap();
        assert!(Arc::ptr_eq(successor_lease.workspace(), &successor));
        assert_eq!(
            successor_lease
                .workspace()
                .selection()
                .control_horizon()
                .active_recovery_fence(),
            restarted_fence,
            "a new lease must observe only the atomic successor workspace"
        );

        let stale_fence = fence(89, semantic.execution_fence.generation.get() - 1);
        let stale_authority = DeltaActivationRuntimeAuthority::new(
            semantic.workspace_id,
            Arc::clone(&committed),
            Arc::new(FixedWriterGeneration {
                workspace_id: semantic.workspace_id,
                fence: stale_fence,
            }),
        );
        assert!(matches!(
            stale_authority.current_selection().await,
            Err(
                DeltaActivationRuntimeAuthoritySnapshotError::SelectionFenceNotAuthorized {
                    selected,
                    active,
                }
            ) if selected == semantic.execution_fence && active == stale_fence
        ));

        let genesis_fence = fence(92, restarted_fence.generation.get());
        let genesis_authority = DeltaActivationRuntimeAuthority::new(
            other_workspace,
            Arc::clone(&committed),
            Arc::new(FixedWriterGeneration {
                workspace_id: other_workspace,
                fence: genesis_fence,
            }),
        );
        let genesis = match genesis_authority.current_selection().await.unwrap() {
            ExactActivationControlSelection::GenesisRequired(genesis) => genesis,
            ExactActivationControlSelection::Selected(_) => {
                panic!("an exact empty workspace scope must require lawful genesis")
            }
        };
        assert_eq!(genesis.workspace_id(), other_workspace);
        assert_eq!(genesis.writer_fence(), genesis_fence);
        assert_eq!(genesis.control_horizon().workspace_id(), other_workspace);
        assert_eq!(genesis.control_horizon().history_row_count(), 0);
        assert_eq!(
            genesis.control_horizon().control_relation(),
            committed.control_relation()
        );
        let authoritative_chain = authority
            .read_chain(semantic.workspace_id)
            .await
            .expect("exact Delta provider is the command semantic chain authority");
        assert_eq!(authoritative_chain, chain);
        assert!(matches!(
            authority
                .revalidate(ActivationAuthorityRequest {
                    workspace_id: semantic.workspace_id,
                    operation_id: semantic.operation_id,
                    expected_head: ExpectedHead::Epoch(epoch_id),
                    execution_fence: semantic.execution_fence,
                })
                .await,
            AuthorityRevalidationOutcome::Valid(ActivationAuthoritySnapshot {
                active_fence,
                ..
            }) if active_fence == semantic.execution_fence
        ));
        assert!(matches!(
            authority
                .revalidate(ActivationAuthorityRequest {
                    workspace_id: semantic.workspace_id,
                    operation_id: semantic.operation_id,
                    expected_head: ExpectedHead::Empty,
                    execution_fence: semantic.execution_fence,
                })
                .await,
            AuthorityRevalidationOutcome::Stale(_)
        ));

        let request = super::super::activation_transaction::ActivationOperationMarkerRequest {
            workspace_id: semantic.workspace_id,
            operation_id: semantic.operation_id,
            event_id: semantic.event_id,
            expected_head: semantic.predecessor_epoch,
            execution_fence: semantic.execution_fence,
            active_recovery_fence: recovery_fence,
            transaction: semantic.commit.transaction,
            operation_selection: semantic.commit.operation_selection,
            control_relation: request_control,
        };
        let evidence = committed.reconcile_operation(&request).await.unwrap();
        assert_eq!(
            evidence.disposition(),
            ActivationReconciliationDisposition::Selected(semantic.event_id)
        );
        let marker_outcome = committed.read_operation_marker(request).await;
        let recovered_versions = match marker_outcome {
            ActivationOperationMarkerOutcome::Selected {
                selection,
                chain_after_readback,
                acknowledgement: ActivationAcknowledgementMarker::Absent,
                evidence: marker_evidence,
            } => {
                let event = selection.event();
                assert_eq!(event.event_id(), semantic.event_id);
                assert_eq!(selection.table_versions().as_ref(), versions.as_ref());
                assert_eq!(chain_after_readback.head_event(), Some(&event));
                assert_eq!(marker_evidence, evidence.evidence());
                Arc::clone(selection.table_versions())
            }
            outcome => panic!("expected selected marker outcome, observed {outcome:?}"),
        };

        let restarted =
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
                .unwrap()
                .reopen(recovered_versions)
                .await
                .expect("activation-selected Delta vector must rebuild a fresh epoch");
        assert_ne!(
            selected_epoch.context().state().session_id(),
            restarted.context().state().session_id()
        );
        assert_eq!(
            selected_epoch.table_version_set_ref(),
            restarted.table_version_set_ref()
        );
    }
}
