//! Canonical declarations built from exact provider relations using native DataFusion plans.
//!
//! Source inventories and run scopes are join authorities. Raw provider observations remain
//! available; this layer never treats missing observations as completed semantic coverage.

use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{Array, ArrayRef, FixedSizeBinaryArray, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use datafusion::common::{DFSchema, DataFusionError, ScalarValue, TableReference};
use datafusion::logical_expr::logical_plan::EmptyRelation;
use datafusion::logical_expr::{
    ColumnarValue, Expr, JoinType, LogicalPlan, LogicalPlanBuilder, ScalarUDF, Volatility,
    create_udf,
};
use datafusion::prelude::{col, lit};

use super::{ProductionWorkspaceStartupError, step};
use crate::fabric::epoch_runtime::FABRIC_CATALOG;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::programmatic_schema::{
    ProgrammaticFieldId, ProgrammaticRelationId, ProgrammaticTransformation,
    ProgrammaticTransformationContract, ProgrammaticTransformationId,
    TransformationDeterminismPolicy, TransformationFieldIdentity, TransformationInputs,
    TransformationOrderingPolicy, TransformationOutput, TransformationPlanError,
    TransformationProvenance, TransformationProvenanceIdentity, TransformationRecursionPolicy,
    TransformationReleaseIdentity, TransformationResourceClass, TransformationSemanticVersion,
};
use crate::identity::{self, IdentityDomain, SourceOccurrenceIdentityInput};
use crate::provider_contracts::ProviderSourceInventory;
use crate::provider_native_syntax::NativeSyntaxRelation;
use crate::rustc_relation_schema::RustcRelation;

mod calls;

const SOURCE: &str = "source.code_file";
const DECLARATION: &str = "fact.code_declaration";
const ENTITY: &str = "fact.code_entity";
const SELECTOR: &str = "fact.code_entity_selector";
const REFERENCE: &str = "fact.code_reference";
const INPUT: &str = "source.input_inventory";
const RUN: &str = "system.provider_run_scope";

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventory: &ProviderSourceInventory,
    python: bool,
    rust: bool,
) -> Result<(), ProductionWorkspaceStartupError> {
    for kind in [
        Kind::Source,
        Kind::Declaration { python, rust },
        Kind::Reference { python },
        Kind::CallSite { rust },
        Kind::Entity,
        Kind::EntitySelector,
    ] {
        builder
            .add_transformation(Arc::new(Canonical::new(kind, inventory)))
            .map_err(|error| step("canonical-code-install", error))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Kind {
    Source,
    Declaration { python: bool, rust: bool },
    Reference { python: bool },
    CallSite { rust: bool },
    Entity,
    EntitySelector,
}

struct Canonical {
    kind: Kind,
    workspace: [u8; 16],
    input_set: [u8; 32],
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependencies: Vec<ProgrammaticRelationId>,
}

impl Canonical {
    fn new(kind: Kind, inventory: &ProviderSourceInventory) -> Self {
        let (id, names, mut dependencies) = match kind {
            Kind::Source => (SOURCE, source_fields(), vec![INPUT]),
            Kind::Declaration { python, rust } => (
                DECLARATION,
                declaration_fields(),
                if rust {
                    vec![SOURCE, RUN]
                } else if python {
                    vec![SOURCE]
                } else {
                    vec![]
                },
            ),
            Kind::Entity => (ENTITY, entity_fields(), vec![DECLARATION]),
            Kind::CallSite { rust } => (
                calls::RELATION,
                calls::fields(),
                if rust { calls::dependencies() } else { vec![] },
            ),
            Kind::Reference { python } => (
                REFERENCE,
                reference_fields(),
                if python {
                    vec![
                        SOURCE,
                        DECLARATION,
                        NativeSyntaxRelation::RuffReference.as_str(),
                    ]
                } else {
                    vec![]
                },
            ),
            Kind::EntitySelector => {
                let mut fields = entity_fields();
                fields.push(("selector", DataType::Utf8, false));
                (SELECTOR, fields, vec![ENTITY])
            }
        };
        if let Kind::Declaration { python, rust } = kind {
            if python {
                dependencies.push(NativeSyntaxRelation::RuffBinding.as_str());
            }
            if rust {
                dependencies.push(RustcRelation::PublicItem.relation_id());
            }
        }
        let (schema, table) = id.split_once('.').expect("closed canonical relation");
        let fields = names
            .iter()
            .map(|(name, _, _)| canonical_field_identity(id, name))
            .collect::<Vec<_>>();
        let mut output = TransformationOutput::new(
            ProgrammaticRelationId::new(id),
            TableReference::full(FABRIC_CATALOG, schema, table),
            fields,
        );
        if matches!(kind, Kind::Entity) {
            // Kept distinct from the predecessor Ruff-only role until scoped public queries
            // consume this relation and its processing coverage together.
            output = output.with_semantic_role("canonical.entity-source");
        }
        if matches!(kind, Kind::EntitySelector) {
            output = output.with_semantic_role("canonical.entity-selector");
        }
        if matches!(kind, Kind::Reference { .. }) {
            output = output.with_semantic_role("canonical.reference");
        }
        if matches!(kind, Kind::CallSite { .. }) {
            output = output.with_semantic_role("canonical.call-site");
        }
        let identity =
            *blake3::hash(format!("codefabric.canonical-code.v1:{id}").as_bytes()).as_bytes();
        Self {
            kind,
            workspace: inventory.workspace_id(),
            input_set: inventory.identity(),
            contract: ProgrammaticTransformationContract::new(
                ProgrammaticTransformationId::new(format!("normalize.{id}.v1")),
                TransformationSemanticVersion::new(1, 0, 0),
                TransformationResourceClass::BoundedSpillable {
                    max_rows: 100_000_000,
                    max_memory_bytes: 32 << 30,
                    max_spill_bytes: 128 << 30,
                },
                TransformationDeterminismPolicy::DeterministicSet,
                TransformationOrderingPolicy::Unordered,
                TransformationRecursionPolicy::Forbidden,
                TransformationProvenance::new(
                    TransformationProvenanceIdentity::from_bytes(identity),
                    TransformationReleaseIdentity::from_bytes(identity),
                ),
            ),
            output,
            dependencies: dependencies
                .into_iter()
                .map(ProgrammaticRelationId::new)
                .collect(),
        }
    }

    fn source(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        Ok(LogicalPlanBuilder::from(plan(inputs, INPUT)?)
            .filter(col("input_set_id").eq(fixed_literal(&self.input_set)))?
            .project(source_fields().iter().map(|(name, _, _)| col(*name)))?
            .build()?)
    }

    fn python(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let raw = plan(inputs, NativeSyntaxRelation::RuffBinding.as_str())?;
        let entity = python_entity_id(self.workspace).call(vec![
            col("p.analysis_context_id"),
            col("p.file_id"),
            col("p.scope_id"),
            col("p.name"),
            col("p.binding_kind"),
        ]);
        // File, digest and generation must all match. Equal offsets or names are not a join key.
        Ok(LogicalPlanBuilder::from(raw)
            .alias("p")?
            .filter(col("p.binding_kind").not_eq(lit("builtin")))?
            .join(
                source_alias(inputs)?,
                JoinType::Inner,
                (
                    vec!["p.file_id", "p.content_digest", "p.source_generation"],
                    vec!["s.file_id", "s.content_digest", "s.source_generation"],
                ),
                None,
            )?
            .project(vec![
                entity.alias("entity_id"),
                python_kind()?.alias("entity_kind"),
                col("p.name").alias("name"),
                lit(ScalarValue::Utf8(None)).alias("qualified_name"),
                lit("python").alias("language"),
                col("p.analysis_context_id").alias("context_id"),
                col("p.file_id").alias("file_id"),
                col("p.content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
                col("p.start_byte").alias("start_byte"),
                col("p.end_byte").alias("end_byte"),
                col("p.provider_run_id").alias("provider_run_id"),
                lit("ruff").alias("provider"),
                col("p.binding_kind").alias("raw_kind"),
                col("p.binding_id").alias("provider_observation_id"),
                col("s.workspace_id").alias("workspace_id"),
            ])?
            .build()?)
    }

    fn rust(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let native = plan(inputs, RustcRelation::PublicItem.relation_id())?;
        let fields = native
            .schema()
            .fields()
            .iter()
            .map(|field| col(field.name()))
            .collect::<Vec<_>>();
        let native = LogicalPlanBuilder::from(native)
            .project(
                fields.into_iter().chain([file_id_udf()
                    .call(vec![col("source_file_id")])
                    .alias("file_id")]),
            )?
            .alias("p")?
            .build()?;
        let run = LogicalPlanBuilder::from(plan(inputs, RUN)?)
            .filter(col("provider").eq(lit("rustc")))?
            .alias("r")?
            .build()?;
        let entity = rust_entity_id(self.workspace).call(vec![
            col("r.context_id"),
            col("p.stable_crate_id"),
            col("p.def_path_hash"),
            col("p.item_kind"),
        ]);
        Ok(LogicalPlanBuilder::from(native)
            .join(
                run,
                JoinType::Inner,
                (
                    vec!["p.provider_run_id", "p.source_generation"],
                    vec!["r.provider_run_identity", "r.source_generation"],
                ),
                None,
            )?
            .join(
                source_alias(inputs)?,
                JoinType::Inner,
                (
                    vec![
                        "p.file_id",
                        "p.source_content_digest",
                        "p.source_generation",
                    ],
                    vec!["s.file_id", "s.content_digest", "s.source_generation"],
                ),
                None,
            )?
            .project(vec![
                entity.alias("entity_id"),
                col("p.item_kind").alias("entity_kind"),
                col("p.qualified_name").alias("name"),
                col("p.qualified_name").alias("qualified_name"),
                lit("rust").alias("language"),
                col("r.context_id").alias("context_id"),
                col("p.file_id").alias("file_id"),
                col("p.source_content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
                col("p.span_start_byte").alias("start_byte"),
                col("p.span_end_byte").alias("end_byte"),
                col("r.provider_run_id").alias("provider_run_id"),
                lit("rustc").alias("provider"),
                col("p.item_kind").alias("raw_kind"),
                lit(ScalarValue::FixedSizeBinary(16, None)).alias("provider_observation_id"),
                col("s.workspace_id").alias("workspace_id"),
            ])?
            .build()?)
    }

    fn references(
        &self,
        inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        let raw =
            LogicalPlanBuilder::from(plan(inputs, NativeSyntaxRelation::RuffReference.as_str())?)
                .alias("p")?
                .build()?;
        let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
            .filter(col("language").eq(lit("python")))?
            .alias("d")?
            .build()?;
        let occurrence = source_occurrence_id(
            self.workspace,
            "codefabric_reference_occurrence_id_v1",
            101,
            2,
        )
        .call(vec![
            col("p.file_id"),
            col("p.file_id"),
            col("p.content_digest"),
            col("p.start_byte"),
            col("p.end_byte"),
        ]);
        // A provider's local lookup links observations from the same admitted run. A name or
        // coincident range is never a substitute for the target binding and its exact inputs.
        Ok(LogicalPlanBuilder::from(raw)
            .join(
                source_alias(inputs)?,
                JoinType::Inner,
                (
                    vec!["p.file_id", "p.content_digest", "p.source_generation"],
                    vec!["s.file_id", "s.content_digest", "s.source_generation"],
                ),
                None,
            )?
            .join(
                declarations,
                JoinType::Left,
                (
                    vec![
                        "p.target_id",
                        "p.file_id",
                        "p.analysis_context_id",
                        "p.content_digest",
                        "p.source_generation",
                        "p.provider_run_id",
                    ],
                    vec![
                        "d.provider_observation_id",
                        "d.file_id",
                        "d.context_id",
                        "d.content_digest",
                        "d.source_generation",
                        "d.provider_run_id",
                    ],
                ),
                None,
            )?
            .project(vec![
                occurrence.alias("reference_id"),
                col("d.entity_id").alias("target_entity_id"),
                col("d.declaration_id").alias("target_declaration_id"),
                col("p.name").alias("name"),
                lit("python").alias("language"),
                col("p.reference_class").alias("reference_kind"),
                col("p.resolution").alias("raw_resolution"),
                datafusion::logical_expr::when(col("d.entity_id").is_null(), lit("unknown"))
                    .when(col("p.resolution").eq(lit("resolved")), lit("resolved"))
                    .otherwise(lit("candidate"))?
                    .alias("resolution"),
                datafusion::logical_expr::when(
                    col("p.unknown_reason").is_not_null(),
                    col("p.unknown_reason"),
                )
                .when(
                    col("d.entity_id").is_null(),
                    lit("canonical_target_unavailable"),
                )
                .otherwise(lit(ScalarValue::Utf8(None)))?
                .alias("unknown_reason"),
                lit("lexical").alias("resolution_scope"),
                col("p.analysis_context_id").alias("context_id"),
                col("p.file_id").alias("file_id"),
                col("p.content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
                col("p.start_byte").alias("start_byte"),
                col("p.end_byte").alias("end_byte"),
                col("p.provider_run_id").alias("provider_run_id"),
                col("p.reference_id").alias("provider_observation_id"),
                col("p.target_id").alias("provider_target_observation_id"),
                lit("ruff").alias("provider"),
                col("s.workspace_id").alias("workspace_id"),
            ])?
            .build()?)
    }
}

impl ProgrammaticTransformation for Canonical {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }
    fn output(&self) -> &TransformationOutput {
        &self.output
    }
    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependencies
    }
    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        match self.kind {
            Kind::Source => self.source(inputs),
            Kind::Reference { python: true } => self.references(inputs),
            Kind::Reference { python: false } => empty(reference_fields()),
            Kind::CallSite { rust: true } => calls::build(self.workspace, inputs),
            Kind::CallSite { rust: false } => empty(calls::fields()),
            Kind::EntitySelector => {
                let input = plan(inputs, ENTITY)?;
                let project = |selector: Expr| -> Result<LogicalPlan, TransformationPlanError> {
                    Ok(LogicalPlanBuilder::from(input.clone())
                        .project(
                            entity_fields()
                                .iter()
                                .map(|(name, _, _)| col(*name))
                                .chain([selector.alias("selector")]),
                        )?
                        .build()?)
                };
                let generic = project(col("entity_kind"))?;
                let language = project(datafusion::functions::string::expr_fn::concat(vec![
                    col("language"),
                    lit(":"),
                    col("entity_kind"),
                ]))?;
                Ok(LogicalPlanBuilder::from(generic).union(language)?.build()?)
            }
            Kind::Entity => Ok(LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
                .filter(col("entity_id").is_not_null())?
                .project(entity_fields().iter().map(|(name, _, _)| col(*name)))?
                .distinct()?
                .build()?),
            Kind::Declaration { python, rust } => {
                let mut branches = Vec::new();
                if python {
                    branches.push(self.python(inputs)?);
                }
                if rust {
                    branches.push(self.rust(inputs)?);
                }
                let Some(mut union) = branches.pop() else {
                    return empty(declaration_fields());
                };
                for branch in branches {
                    union = LogicalPlanBuilder::from(union).union(branch)?.build()?;
                }
                let columns = union
                    .schema()
                    .fields()
                    .iter()
                    .map(|f| col(f.name()))
                    .collect::<Vec<_>>();
                let occurrence = declaration_id(self.workspace).call(vec![
                    col("entity_id"),
                    col("file_id"),
                    col("content_digest"),
                    col("start_byte"),
                    col("end_byte"),
                ]);
                Ok(LogicalPlanBuilder::from(union)
                    .project(
                        columns.into_iter().chain([
                            occurrence.alias("declaration_id"),
                            datafusion::logical_expr::when(
                                col("entity_id").is_null(),
                                lit("stable_identity_unavailable"),
                            )
                            .otherwise(lit("canonical"))?
                            .alias("identity_state"),
                            public_entity_id()
                                .call(vec![col("entity_id"), col("entity_kind")])
                                .alias("public_entity_id"),
                            lit("entity").alias("subject_kind"),
                            lit("declarations").alias("fact_family"),
                        ]),
                    )?
                    .build()?)
            }
        }
    }
}

fn canonical_field_identity(id: &str, name: &str) -> TransformationFieldIdentity {
    let field = TransformationFieldIdentity::new(ProgrammaticFieldId::new(format!("{id}.{name}")));
    match name {
        "entity_id" => field.with_semantic_role("semantic.entity.identity"),
        "entity_kind" => field.with_semantic_role("semantic.entity.kind"),
        "name" => field.with_semantic_role("semantic.entity.name"),
        "language" => field.with_semantic_role("semantic.entity.language"),
        "selector" => field.with_semantic_role("semantic.entity.selector"),
        "file_id" => field.with_semantic_role("semantic.provenance.source-file"),
        "context_id" => field.with_semantic_role("semantic.provenance.analysis-context"),
        "source_generation" => field.with_semantic_role("semantic.provenance.source-generation"),
        "start_byte" => field.with_semantic_role("semantic.source.start-byte"),
        "end_byte" => field.with_semantic_role("semantic.source.end-byte"),
        _ => field,
    }
}

fn plan(inputs: &TransformationInputs, id: &str) -> Result<LogicalPlan, TransformationPlanError> {
    inputs.plan(&ProgrammaticRelationId::new(id))
}

fn source_alias(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    Ok(LogicalPlanBuilder::from(plan(inputs, SOURCE)?)
        .filter(col("disposition").eq(lit("captured")))?
        .alias("s")?
        .build()?)
}

fn fixed_literal(bytes: &[u8]) -> Expr {
    lit(ScalarValue::FixedSizeBinary(
        i32::try_from(bytes.len()).expect("fixed identity width"),
        Some(bytes.to_vec()),
    ))
}

type FieldSpec = (&'static str, DataType, bool);
fn source_fields() -> Vec<FieldSpec> {
    vec![
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("source_generation", DataType::UInt64, false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), true),
        ("relative_path", DataType::Binary, false),
        ("byte_length", DataType::UInt64, true),
        ("disposition", DataType::Utf8, false),
    ]
}
fn entity_fields() -> Vec<FieldSpec> {
    vec![
        ("entity_id", DataType::FixedSizeBinary(16), true),
        ("entity_kind", DataType::Utf8, false),
        ("name", DataType::Utf8, false),
        ("qualified_name", DataType::Utf8, true),
        ("language", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
    ]
}
fn reference_fields() -> Vec<FieldSpec> {
    vec![
        ("reference_id", DataType::FixedSizeBinary(16), false),
        ("target_entity_id", DataType::FixedSizeBinary(16), true),
        ("target_declaration_id", DataType::FixedSizeBinary(16), true),
        ("name", DataType::Utf8, false),
        ("language", DataType::Utf8, false),
        ("reference_kind", DataType::Utf8, false),
        ("raw_resolution", DataType::Utf8, false),
        ("resolution", DataType::Utf8, false),
        ("unknown_reason", DataType::Utf8, true),
        ("resolution_scope", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), false),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("start_byte", DataType::UInt64, false),
        ("end_byte", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        (
            "provider_observation_id",
            DataType::FixedSizeBinary(16),
            false,
        ),
        (
            "provider_target_observation_id",
            DataType::FixedSizeBinary(16),
            false,
        ),
        ("provider", DataType::Utf8, false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
    ]
}
fn declaration_fields() -> Vec<FieldSpec> {
    let mut fields = entity_fields();
    fields.pop(); // workspace follows the observation columns in both provider projections.
    fields.extend([
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("start_byte", DataType::UInt64, false),
        ("end_byte", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider", DataType::Utf8, false),
        ("raw_kind", DataType::Utf8, false),
        (
            "provider_observation_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("declaration_id", DataType::FixedSizeBinary(16), true),
        ("identity_state", DataType::Utf8, false),
        ("public_entity_id", DataType::Utf8, true),
        ("subject_kind", DataType::Utf8, false),
        ("fact_family", DataType::Utf8, false),
    ]);
    fields
}
fn empty(fields: Vec<FieldSpec>) -> Result<LogicalPlan, TransformationPlanError> {
    let schema = Schema::new(
        fields
            .into_iter()
            .map(|(name, kind, nullable)| Field::new(name, kind, nullable))
            .collect::<Vec<_>>(),
    );
    Ok(LogicalPlan::EmptyRelation(EmptyRelation {
        produce_one_row: false,
        schema: Arc::new(DFSchema::try_from(schema)?),
    }))
}

// These scalar functions only implement the application's CBEF identity recipes. Joins,
// selection, union and deduplication remain native relational operators.
fn fixed(array: &ArrayRef, row: usize) -> Result<[u8; 16], DataFusionError> {
    array
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or_else(|| invalid("expected ID array"))?
        .value(row)
        .try_into()
        .map_err(|_| invalid("invalid ID width"))
}
fn text(array: &ArrayRef, row: usize) -> Result<&str, DataFusionError> {
    Ok(array
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid("expected text array"))?
        .value(row))
}
fn number(array: &ArrayRef, row: usize) -> Result<u64, DataFusionError> {
    Ok(array
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| invalid("expected u64 array"))?
        .value(row))
}
fn invalid(detail: &str) -> DataFusionError {
    DataFusionError::Execution(detail.to_owned())
}

fn ids(
    values: &[ColumnarValue],
    derive: impl Fn(&[ArrayRef], usize) -> Result<[u8; 16], DataFusionError>,
) -> Result<ColumnarValue, DataFusionError> {
    let arrays = ColumnarValue::values_to_arrays(values)?;
    let mut output = FixedSizeBinaryBuilder::with_capacity(arrays[0].len(), 16);
    for row in 0..arrays[0].len() {
        if arrays.iter().any(|a| a.is_null(row)) {
            output.append_null();
        } else {
            output.append_value(derive(&arrays, row)?)?;
        }
    }
    Ok(ColumnarValue::Array(Arc::new(output.finish())))
}

fn kind_code(kind: &str) -> u16 {
    // Versioned canonical declaration kinds, not provider-local discriminants.
    match kind {
        "function" => 1,
        "static" => 3,
        "constant" => 4,
        "constructor-function" | "constructor-constant" => 5,
        "class" => 6,
        "parameter" => 7,
        "type-alias" => 8,
        "type-parameter" => 9,
        "import" => 10,
        _ => 2,
    }
}

fn python_kind() -> Result<Expr, DataFusionError> {
    datafusion::logical_expr::when(col("p.binding_kind").eq(lit("function")), lit("function"))
        .when(col("p.binding_kind").eq(lit("class")), lit("class"))
        .when(col("p.binding_kind").eq(lit("parameter")), lit("parameter"))
        .when(
            col("p.binding_kind").eq(lit("type-alias")),
            lit("type-alias"),
        )
        .when(
            col("p.binding_kind").eq(lit("type-parameter")),
            lit("type-parameter"),
        )
        .when(col("p.binding_kind").eq(lit("import")), lit("import"))
        .otherwise(lit("binding"))
}

fn python_entity_id(workspace: [u8; 16]) -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_entity_id_v1",
        vec![DataType::FixedSizeBinary(16); 3]
            .into_iter()
            .chain([DataType::Utf8, DataType::Utf8])
            .collect(),
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(move |values| {
            ids(values, |a, r| {
                let context = fixed(&a[0], r)?;
                let mut owner_key = fixed(&a[1], r)?.to_vec();
                owner_key.extend_from_slice(&fixed(&a[2], r)?);
                let owner = identity::semantic_owner_identity(
                    workspace,
                    context,
                    "python-scope",
                    owner_key,
                )
                .map_err(|e| invalid(&e.to_string()))?;
                identity::semantic_entity_identity(
                    workspace,
                    context,
                    kind_code(text(&a[4], r)?),
                    owner.id,
                    text(&a[3], r)?.as_bytes().to_vec(),
                )
                .map(|v| v.id)
                .map_err(|e| invalid(&e.to_string()))
            })
        }),
    ))
}

fn rust_entity_id(workspace: [u8; 16]) -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_rust_entity_id_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::UInt64,
            DataType::FixedSizeBinary(16),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(move |values| {
            ids(values, |a, r| {
                let context = fixed(&a[0], r)?;
                let mut key = number(&a[1], r)?.to_be_bytes().to_vec();
                key.extend_from_slice(&fixed(&a[2], r)?);
                let owner =
                    identity::semantic_owner_identity(workspace, context, "rust-item", key.clone())
                        .map_err(|e| invalid(&e.to_string()))?;
                identity::semantic_entity_identity(
                    workspace,
                    context,
                    kind_code(text(&a[3], r)?),
                    owner.id,
                    key,
                )
                .map(|v| v.id)
                .map_err(|e| invalid(&e.to_string()))
            })
        }),
    ))
}

fn file_id_udf() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_decode_source_file_id_v1",
        vec![DataType::Utf8],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(|values| {
            ids(values, |a, r| {
                identity::decode_public_id(IdentityDomain::SourceFile, None, text(&a[0], r)?)
                    .map_err(|e| invalid(&e.to_string()))
            })
        }),
    ))
}

fn public_entity_id() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_public_entity_id_v1",
        vec![DataType::FixedSizeBinary(16), DataType::Utf8],
        DataType::Utf8,
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let values = (0..arrays[0].len())
                .map(|row| {
                    if arrays[0].is_null(row) || arrays[1].is_null(row) {
                        return Ok(None);
                    }
                    identity::encode_public_id(
                        IdentityDomain::Entity,
                        Some(text(&arrays[1], row)?),
                        fixed(&arrays[0], row)?,
                    )
                    .map(Some)
                    .map_err(|error| invalid(&error.to_string()))
                })
                .collect::<Result<Vec<_>, DataFusionError>>()?;
            Ok(ColumnarValue::Array(Arc::new(StringArray::from(values))))
        }),
    ))
}

fn declaration_id(workspace: [u8; 16]) -> Arc<ScalarUDF> {
    source_occurrence_id(workspace, "codefabric_declaration_id_v1", 100, 1)
}

fn source_occurrence_id(workspace: [u8; 16], name: &str, kind: u16, family: u16) -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        name,
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::UInt64,
            DataType::UInt64,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(move |values| {
            ids(values, |a, r| {
                let digest = a[2]
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .ok_or_else(|| invalid("expected content digest"))?
                    .value(r)
                    .try_into()
                    .map_err(|_| invalid("invalid content digest"))?;
                identity::source_occurrence_identity(SourceOccurrenceIdentityInput {
                    workspace_id: workspace,
                    file_id: fixed(&a[1], r)?,
                    source_digest: digest,
                    start_byte: number(&a[3], r)?,
                    end_byte: number(&a[4], r)?,
                    owner_id: fixed(&a[0], r)?,
                    entity_kind_code: kind,
                    occurrence_family_code: family,
                    normalized_kind_code: 1,
                    parent_id: None,
                    role_code: None,
                    ordinal: 0,
                })
                .map(|v| v.id)
                .map_err(|e| invalid(&e.to_string()))
            })
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig};
    use crate::fabric::programmatic_schema::ProviderInput;
    use crate::provider_contracts::{ProviderInputDisposition, ProviderInventoryMember};
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
    };
    use arrow_array::RecordBatch;
    use datafusion::datasource::MemTable;
    use std::collections::HashMap;

    fn ids16(values: &[u8]) -> ArrayRef {
        let values = values.iter().map(|v| [*v; 16]).collect::<Vec<_>>();
        crate::fabric::id16_array(values.iter().map(Some))
    }
    fn digests(values: &[u8]) -> ArrayRef {
        let values = values.iter().map(|v| [*v; 32]).collect::<Vec<_>>();
        crate::fabric::hash32_array(values.iter().map(Some))
    }
    fn strings(values: &[&str]) -> ArrayRef {
        Arc::new(StringArray::from(values.to_vec()))
    }
    fn numbers(values: &[u64]) -> ArrayRef {
        Arc::new(UInt64Array::from(values.to_vec()))
    }

    fn provider(
        builder: &mut ProgrammaticFabricEpochBuilder,
        id: &str,
        columns: Vec<(&str, ArrayRef)>,
    ) {
        let fields = columns
            .iter()
            .map(|(name, array)| {
                Field::new(*name, array.data_type().clone(), array.null_count() > 0).with_metadata(
                    HashMap::from([(FIELD_ID_METADATA_KEY.to_owned(), format!("{id}.{name}"))]),
                )
            })
            .collect::<Vec<_>>();
        let schema = Arc::new(Schema::new(fields).with_metadata(HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            id.to_owned(),
        )])));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            columns.into_iter().map(|(_, a)| a).collect(),
        )
        .unwrap();
        let reference = TableReference::full(FABRIC_CATALOG, "raw_ruff", id.replace('.', "_"));
        let contract = SchemaContract::try_new(
            "test-canonical-binding",
            reference.clone(),
            Arc::clone(&schema),
            Arc::clone(&schema),
            (0..schema.fields().len())
                .map(|i| FieldIndexMapping::direct(i, i))
                .collect(),
        )
        .unwrap();
        builder
            .register_provider(ProviderInput::new(
                ProgrammaticRelationId::new(id),
                reference,
                Arc::new(contract),
                Arc::new(MemTable::try_new(schema, vec![vec![batch]]).unwrap()),
            ))
            .unwrap();
    }

    async fn fixture(python: bool) -> datafusion::prelude::SessionContext {
        let members = [7, 8]
            .into_iter()
            .map(|file| ProviderInventoryMember {
                relative_path: format!("{file}.py").into_bytes(),
                selected_for_provider: true,
                disposition: ProviderInputDisposition::Captured {
                    file_id: [file; 16],
                    digest: [17; 32],
                    byte_length: 100,
                },
            })
            .collect();
        let inventory = ProviderSourceInventory::try_new(
            [6; 16],
            3,
            [25; 32],
            &[b"7.py".to_vec(), b"8.py".to_vec()],
            members,
            vec![],
            None,
        )
        .unwrap();
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([42; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap();
        super::super::input_observations::install_input_observations(&mut builder, &inventory, &[])
            .unwrap();
        if python {
            provider(
                &mut builder,
                NativeSyntaxRelation::RuffBinding.as_str(),
                vec![
                    ("analysis_context_id", ids16(&[2; 5])),
                    ("file_id", ids16(&[7, 7, 8, 7, 7])),
                    ("scope_id", ids16(&[5; 5])),
                    ("name", strings(&["x", "x", "x", "stale", "int"])),
                    (
                        "binding_kind",
                        strings(&["local", "local", "local", "local", "builtin"]),
                    ),
                    ("content_digest", digests(&[17, 17, 17, 18, 17])),
                    ("source_generation", numbers(&[3; 5])),
                    ("start_byte", numbers(&[0, 10, 0, 20, 0])),
                    ("end_byte", numbers(&[1, 11, 1, 25, 0])),
                    ("provider_run_id", ids16(&[4; 5])),
                    ("binding_id", ids16(&[9, 10, 9, 11, 12])),
                ],
            );
            provider(
                &mut builder,
                NativeSyntaxRelation::RuffReference.as_str(),
                vec![
                    ("analysis_context_id", ids16(&[2, 2, 2, 2, 3])),
                    ("file_id", ids16(&[7, 8, 7, 7, 7])),
                    ("name", strings(&["x", "x", "missing", "stale", "x"])),
                    ("reference_class", strings(&["read"; 5])),
                    (
                        "resolution",
                        strings(&[
                            "resolved",
                            "resolved",
                            "unknown-symbol",
                            "resolved",
                            "resolved",
                        ]),
                    ),
                    ("target_id", ids16(&[9, 9, 99, 9, 9])),
                    (
                        "unknown_reason",
                        Arc::new(StringArray::from(vec![
                            None,
                            None,
                            Some("unbound_name"),
                            None,
                            None,
                        ])),
                    ),
                    ("content_digest", digests(&[17, 17, 17, 18, 17])),
                    ("source_generation", numbers(&[3; 5])),
                    ("start_byte", numbers(&[40, 40, 50, 70, 80])),
                    ("end_byte", numbers(&[41, 41, 57, 75, 81])),
                    ("provider_run_id", ids16(&[4; 5])),
                    ("reference_id", ids16(&[40, 41, 42, 43, 44])),
                ],
            );
        }
        install(&mut builder, &inventory, python, false).unwrap();
        let mut assembly = builder.into_assembly_parts().3;
        assembly.install_transformations().await.unwrap();
        assembly.candidate_context()
    }

    #[tokio::test]
    async fn canonical_bindings_separate_files_and_occurrences_and_reject_stale_bytes() {
        let context = fixture(true).await;
        let entities = context
            .sql("SELECT * FROM fact.code_entity ORDER BY file_id")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let entities = arrow::compute::concat_batches(&entities[0].schema(), &entities).unwrap();
        assert_eq!(
            entities.num_rows(),
            2,
            "two assignments in one scope are one entity; another file is separate"
        );
        let ids = entities
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_ne!(ids.value(0), ids.value(1));
        assert!(
            entities
                .column_by_name("entity_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .iter()
                .all(|v| v == Some("binding"))
        );
        let declarations = context
            .sql("SELECT * FROM fact.code_declaration ORDER BY file_id, start_byte")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let declarations =
            arrow::compute::concat_batches(&declarations[0].schema(), &declarations).unwrap();
        assert_eq!(
            declarations.num_rows(),
            3,
            "stale digest and non-source builtin must not become declarations"
        );
        let entities = declarations
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        let ids = declarations
            .column_by_name("declaration_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(entities.value(0), entities.value(1));
        assert_ne!(ids.value(0), ids.value(1));
        assert_ne!(ids.value(0), entities.value(0));
        let schema = context
            .table("fact.code_entity")
            .await
            .unwrap()
            .schema()
            .clone();
        assert_eq!(
            schema
                .metadata()
                .get(crate::schema_contract::RELATION_SEMANTIC_ROLE_METADATA_KEY)
                .map(String::as_str),
            Some("canonical.entity-source")
        );
    }

    #[tokio::test]
    async fn canonical_references_preserve_unknowns_and_resolve_only_exact_binding_inputs() {
        let context = fixture(true).await;
        let batches = context
            .sql("SELECT * FROM fact.code_reference ORDER BY file_id, start_byte")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let rows = arrow::compute::concat_batches(&batches[0].schema(), &batches).unwrap();
        assert_eq!(
            rows.num_rows(),
            4,
            "the stale source observation is excluded"
        );
        let target = rows
            .column_by_name("target_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        let occurrence = rows
            .column_by_name("reference_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_ne!(
            target.value(0),
            target.value(3),
            "the same raw binding key in two files is not one target"
        );
        assert_ne!(
            occurrence.value(0),
            target.value(0),
            "a reference occurrence is not its target entity"
        );
        assert!(
            target.is_null(1),
            "unresolved symbols must retain unknown targets"
        );
        assert!(
            target.is_null(2),
            "a different analysis context must not bind to a known name"
        );
        let reasons = rows
            .column_by_name("unknown_reason")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(reasons.value(1), "unbound_name");
        assert_eq!(reasons.value(2), "canonical_target_unavailable");
        let bindings = context.sql("SELECT r.name, d.name AS target_name, d.start_byte AS declaration_start FROM fact.code_reference r JOIN fact.code_declaration d ON r.target_declaration_id = d.declaration_id ORDER BY r.file_id")
            .await.unwrap().collect().await.unwrap();
        let bindings = arrow::compute::concat_batches(&bindings[0].schema(), &bindings).unwrap();
        assert_eq!(bindings.num_rows(), 2);
        assert_eq!(
            bindings
                .column_by_name("target_name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "x"
        );
        assert_eq!(
            bindings
                .column_by_name("declaration_start")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(0),
            0,
            "the provider selected the first declaration, not the last textual assignment"
        );
    }

    #[tokio::test]
    async fn canonical_missing_providers_install_empty_relations_without_invented_facts() {
        let context = fixture(false).await;
        for table in [
            "fact.code_entity",
            "fact.code_declaration",
            "fact.code_reference",
            "fact.code_call_site",
        ] {
            let batches = context.table(table).await.unwrap().collect().await.unwrap();
            assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 0);
        }
        let source = context
            .table("source.code_file")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(source.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
    }

    #[tokio::test]
    async fn canonical_compiler_identity_is_contextual_and_missing_keys_stay_unknown() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("context", DataType::FixedSizeBinary(16), false),
            Field::new("crate", DataType::UInt64, true),
            Field::new("key", DataType::FixedSizeBinary(16), false),
            Field::new("kind", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                ids16(&[2, 2, 3, 2]),
                Arc::new(UInt64Array::from(vec![Some(42), Some(42), Some(42), None])),
                ids16(&[7; 4]),
                strings(&["function"; 4]),
            ],
        )
        .unwrap();
        let context = datafusion::prelude::SessionContext::new();
        let batches = context
            .read_batch(batch)
            .unwrap()
            .select(vec![rust_entity_id([6; 16]).call(vec![
                col("context"),
                col("crate"),
                col("key"),
                col("kind"),
            ])])
            .unwrap()
            .collect()
            .await
            .unwrap();
        let ids = batches[0]
            .column(0)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(
            ids.value(0),
            ids.value(1),
            "observation order/run identity is not entity identity"
        );
        assert_ne!(
            ids.value(0),
            ids.value(2),
            "different effective contexts remain distinct"
        );
        assert!(
            ids.is_null(3),
            "a display name cannot replace the missing stable compiler key"
        );
    }
}
