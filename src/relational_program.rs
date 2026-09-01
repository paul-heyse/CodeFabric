//! Typed relational programs compiled directly to DataFusion logical plans.
//!
//! This module is deliberately a compiler, not a second query language. The
//! program algebra is closed and typed, relation and field identities are
//! resolved from executable schema contracts installed in the candidate
//! session, and the only executable result is a native DataFusion
//! [`LogicalPlan`]. SQL text, serialized plans, bytecode, and string-dispatched
//! operation kinds are not accepted at this boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Not;
use std::sync::Arc;

use arrow_schema::{DataType, SchemaRef};
use datafusion::common::{Column, DFSchema, ScalarValue, TableReference};
use datafusion::error::DataFusionError;
use datafusion::functions_aggregate::expr_fn::{avg, count, count_distinct, max, min, sum};
use datafusion::logical_expr::{Expr, ExprSchemable, JoinType, LogicalPlan, LogicalPlanBuilder};

use crate::schema_contract::{SchemaContract, SchemaRole};

/// A stable application-owned relation identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationId(String);

impl RelationId {
    /// Construct a bounded, non-empty relation identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RelationalProgramError> {
        bounded_id(value.into(), "relation").map(Self)
    }

    /// Return the application-owned identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable application-owned field identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldId(String);

impl FieldId {
    /// Construct a bounded, non-empty field identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RelationalProgramError> {
        bounded_id(value.into(), "field").map(Self)
    }

    /// Return the application-owned identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable identifier for an installer-derived intrinsic primitive row.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PrimitiveId(String);

impl PrimitiveId {
    /// Construct a bounded, non-empty primitive identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RelationalProgramError> {
        bounded_id(value.into(), "primitive").map(Self)
    }

    /// Return the runtime primitive identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Compiler primitives that are intrinsically static because each variant is
/// implemented directly by this binary through a native DataFusion API.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum RelationalPrimitive {
    Project,
    Filter,
    Join,
    Aggregate,
    Union,
    ScalarFunction,
    AggregateFunction,
}

impl RelationalPrimitive {
    const ALL: [Self; 7] = [
        Self::Project,
        Self::Filter,
        Self::Join,
        Self::Aggregate,
        Self::Union,
        Self::ScalarFunction,
        Self::AggregateFunction,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Project => "rel.project",
            Self::Filter => "rel.filter",
            Self::Join => "rel.join",
            Self::Aggregate => "rel.aggregate",
            Self::Union => "rel.union",
            Self::ScalarFunction => "fn.scalar",
            Self::AggregateFunction => "fn.aggregate",
        }
    }

    const fn signature(self) -> &'static str {
        match self {
            Self::Project => "relation,expression[]->relation",
            Self::Filter => "relation,predicate->relation",
            Self::Join => "relation,relation,predicate,join_kind->relation",
            Self::Aggregate => "relation,group[],aggregate[]->relation",
            Self::Union => "relation[]->relation",
            Self::ScalarFunction => "scalar[]->scalar",
            Self::AggregateFunction => "scalar[]->scalar",
        }
    }

    const fn semantic_level(self) -> &'static str {
        match self {
            Self::Project | Self::Filter | Self::Join | Self::Aggregate | Self::Union => {
                "logical-plan"
            }
            Self::ScalarFunction | Self::AggregateFunction => "function",
        }
    }
}

const PROGRAM_COMPILER_AUTHORITY: &str =
    "codefabric.relational_program.v2:datafusion=55.0.0:arrow=59.2.0";

fn bounded_id(value: String, kind: &str) -> Result<String, RelationalProgramError> {
    if value.is_empty() || value.len() > 240 {
        return Err(RelationalProgramError::InvalidIdentifier {
            kind: kind.to_owned(),
            value,
        });
    }
    Ok(value)
}

/// One epoch-pinned logical input plan associated with an admitted relation.
#[derive(Clone, Debug)]
pub struct RelationInput {
    /// Relation resolved against the current epoch.
    pub relation_id: RelationId,
    /// Live logical input. Its `DFSchema` is validated before use.
    pub plan: LogicalPlan,
}

/// One session-resolved catalog object required by a relational program.
///
/// The relation identity remains the semantic binding key; the table
/// reference is typed session data used to resolve the concrete provider inside one
/// sealed epoch catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRelationBinding {
    pub relation_id: RelationId,
    pub table_reference: TableReference,
}

/// One exact live-session relation contract used by the relational compiler.
///
/// The Arrow schema inside `contract` is authoritative for field names,
/// types, nullability, and stable field identities. This wrapper contributes
/// no second schema declaration.
#[derive(Clone, Debug)]
pub struct ProgramRelationContract {
    pub relation_id: RelationId,
    pub table_reference: TableReference,
    pub contract: Arc<SchemaContract>,
}

/// One query-local relation contract appended to immutable epoch bindings for a single compile.
///
/// Supplemental relations are never installed in the epoch catalog. Their exact Arrow schema,
/// stable field identities, scan reference, and independently verified content authority travel
/// together so a request-owned input cannot inherit authority merely by matching a schema.
#[derive(Clone, Debug)]
pub struct SupplementalProgramRelationBinding {
    relation_id: RelationId,
    table_reference: TableReference,
    schema: SchemaRef,
    field_ids: Arc<[FieldId]>,
    authority_pin: [u8; 32],
}

impl SupplementalProgramRelationBinding {
    /// Construct one exact supplemental relation binding.
    ///
    /// # Errors
    ///
    /// Rejects an absent authority pin, empty schema, field-count drift, or duplicate field IDs.
    pub fn try_new(
        relation_id: RelationId,
        table_reference: TableReference,
        schema: SchemaRef,
        field_ids: impl Into<Arc<[FieldId]>>,
        authority_pin: [u8; 32],
    ) -> Result<Self, RelationalProgramError> {
        let field_ids = field_ids.into();
        if authority_pin == [0; 32] {
            return Err(RelationalProgramError::InvalidProgram(format!(
                "supplemental relation {} has no content authority",
                relation_id.as_str()
            )));
        }
        if schema.fields().is_empty() || schema.fields().len() != field_ids.len() {
            return Err(RelationalProgramError::InvalidProgram(format!(
                "supplemental relation {} schema/field contract is empty or has different widths",
                relation_id.as_str()
            )));
        }
        let mut unique = BTreeSet::new();
        for field_id in field_ids.iter() {
            if !unique.insert(field_id) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "supplemental relation {} repeats field {}",
                    relation_id.as_str(),
                    field_id.as_str()
                )));
            }
        }
        Ok(Self {
            relation_id,
            table_reference,
            schema,
            field_ids,
            authority_pin,
        })
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn table_reference(&self) -> &TableReference {
        &self.table_reference
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn field_ids(&self) -> &[FieldId] {
        &self.field_ids
    }

    #[must_use]
    pub const fn authority_pin(&self) -> [u8; 32] {
        self.authority_pin
    }
}

/// Closed scalar operations that map to native DataFusion expressions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
    Not,
    Add,
    Subtract,
    Multiply,
    Divide,
    IsNull,
    IsNotNull,
}

/// Closed aggregate operations backed by DataFusion's registered built-ins.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AggregateOperator {
    Count,
    CountDistinct,
    Sum,
    Average,
    Minimum,
    Maximum,
}

/// A scalar expression in the semantic algebra.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ScalarExpression {
    /// A field ID resolved in the current node's live scope.
    Field(FieldId),
    /// A typed Arrow scalar value.
    Literal(ScalarValue),
    /// One native scalar operation. Arity and type are checked during compilation.
    Call {
        operator: ScalarOperator,
        arguments: Vec<ScalarExpression>,
    },
}

/// One named projection output. The output name and type come from `field_id`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NamedExpression {
    pub field_id: FieldId,
    pub expression: ScalarExpression,
}

/// One aggregate call in an aggregate node.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateExpression {
    pub operator: AggregateOperator,
    pub argument: ScalarExpression,
}

/// One named aggregate output. The output name and type come from `field_id`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NamedAggregateExpression {
    pub field_id: FieldId,
    pub expression: AggregateExpression,
}

/// Native join kinds intentionally exposed by the closed algebra.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    LeftSemi,
    RightSemi,
    LeftAnti,
    RightAnti,
}

impl JoinKind {
    const fn datafusion(self) -> JoinType {
        match self {
            Self::Inner => JoinType::Inner,
            Self::Left => JoinType::Left,
            Self::Right => JoinType::Right,
            Self::Full => JoinType::Full,
            Self::LeftSemi => JoinType::LeftSemi,
            Self::RightSemi => JoinType::RightSemi,
            Self::LeftAnti => JoinType::LeftAnti,
            Self::RightAnti => JoinType::RightAnti,
        }
    }

    const fn retains_left(self) -> bool {
        !matches!(self, Self::RightSemi | Self::RightAnti)
    }

    const fn retains_right(self) -> bool {
        !matches!(self, Self::LeftSemi | Self::LeftAnti)
    }
}

/// Set semantics for a native union node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnionKind {
    All,
    Distinct,
}

/// One native sort expression.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SortExpression {
    pub expression: ScalarExpression,
    pub ascending: bool,
    pub nulls_first: bool,
}

/// The small, closed relational algebra accepted by this compiler.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RelationalExpression {
    Input(RelationId),
    Projection {
        input: Box<RelationalExpression>,
        expressions: Vec<NamedExpression>,
    },
    Filter {
        input: Box<RelationalExpression>,
        predicate: ScalarExpression,
    },
    Join {
        left: Box<RelationalExpression>,
        right: Box<RelationalExpression>,
        kind: JoinKind,
        predicates: Vec<ScalarExpression>,
    },
    Union {
        inputs: Vec<RelationalExpression>,
        kind: UnionKind,
    },
    Aggregate {
        input: Box<RelationalExpression>,
        group_by: Vec<NamedExpression>,
        aggregates: Vec<NamedAggregateExpression>,
    },
    Sort {
        input: Box<RelationalExpression>,
        expressions: Vec<SortExpression>,
    },
    Limit {
        input: Box<RelationalExpression>,
        skip: usize,
        fetch: Option<usize>,
    },
}

/// A relational program and its exact application-owned output field contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RelationalProgram {
    pub root: RelationalExpression,
    pub output_fields: Vec<FieldId>,
}

/// A dependency observed while successfully compiling the actual program tree.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilationDependency {
    /// Exact programmatic session/schema authority used for this compilation.
    SessionAuthority(String),
    Relation(RelationId),
    Field(FieldId),
    Primitive {
        primitive_id: PrimitiveId,
        implementation_id: String,
    },
}

/// The native DataFusion logical node selected for a relational operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeLogicalSelection {
    Projection,
    Filter,
    Join(JoinKind),
    Union(UnionKind),
    Aggregate,
    Sort,
    Limit,
}

/// Highest viable execution surface selected by actual compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExtensionSelection {
    NativeLogical(NativeLogicalSelection),
    BuiltInScalar(ScalarOperator),
    BuiltInAggregate(AggregateOperator),
}

/// Causal observations emitted by the successful compilation itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationObservations {
    pub dependencies: BTreeSet<CompilationDependency>,
    pub extension_selections: BTreeSet<ExtensionSelection>,
}

/// A transient native plan and the observations that caused its construction.
#[derive(Clone, Debug)]
pub struct CompiledRelationalProgram {
    pub plan: LogicalPlan,
    pub observations: CompilationObservations,
}

/// Fail-closed binding, typing, and native planning failures.
#[derive(Debug, thiserror::Error)]
pub enum RelationalProgramError {
    #[error("invalid {kind} identifier {value:?}")]
    InvalidIdentifier { kind: String, value: String },
    #[error("duplicate logical input for relation {0}")]
    DuplicateInput(String),
    #[error("relation {0} is unresolved")]
    UnresolvedRelation(String),
    #[error("field {0} is unresolved")]
    UnresolvedField(String),
    #[error("intrinsic primitive {0} is unresolved or incompatible with this compiler")]
    UnresolvedPrimitive(String),
    #[error("relation {relation} has no supplied logical input")]
    MissingInput { relation: String },
    #[error("field {field} is not available in this relational scope")]
    FieldOutOfScope { field: String },
    #[error("invalid {operator} arity: expected {expected}, observed {observed}")]
    InvalidArity {
        operator: String,
        expected: usize,
        observed: usize,
    },
    #[error("expression for {context} has type {observed:?}; expected {expected:?}")]
    TypeMismatch {
        context: String,
        expected: DataType,
        observed: DataType,
    },
    #[error("expression for {context} has nullable={observed}; expected nullable={expected}")]
    NullabilityMismatch {
        context: String,
        expected: bool,
        observed: bool,
    },
    #[error("invalid relational program: {0}")]
    InvalidProgram(String),
    #[error("compiled output schema differs from the declared fields: {0}")]
    OutputSchema(String),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
}

/// Exact DataFusion 55 compiler for the closed relational algebra.
pub struct RelationalProgramCompiler;

impl RelationalProgramCompiler {
    /// Resolve the single live-session relation that owns a program's output fields.
    ///
    /// # Errors
    ///
    /// Rejects an empty/repeated output contract, an unresolved field, or fields owned by more
    /// than one programmatically installed relation.
    pub fn resolve_output_relation_with_bindings(
        bindings: &ProgramBindings,
        program: &RelationalProgram,
    ) -> Result<RelationId, RelationalProgramError> {
        if program.output_fields.is_empty() {
            return Err(RelationalProgramError::InvalidProgram(
                "the output field contract is empty".to_owned(),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut output_relation = None;
        for field_id in &program.output_fields {
            if !seen.insert(field_id) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "output field {} is repeated",
                    field_id.0
                )));
            }
            let relation_id = &bindings.field(field_id)?.relation_id;
            match &output_relation {
                None => output_relation = Some(relation_id.clone()),
                Some(expected) if expected != relation_id => {
                    return Err(RelationalProgramError::InvalidProgram(
                        "declared output fields must belong to one relation".to_owned(),
                    ));
                }
                Some(_) => {}
            }
        }
        output_relation.ok_or_else(|| {
            RelationalProgramError::InvalidProgram("the output field contract is empty".to_owned())
        })
    }

    /// Resolve exactly the catalog relations referenced by a program's input
    /// nodes against the live programmatic session bindings.
    pub fn bind_catalog_inputs_with_bindings(
        bindings: &ProgramBindings,
        program: &RelationalProgram,
    ) -> Result<Vec<CatalogRelationBinding>, RelationalProgramError> {
        let mut required = BTreeSet::new();
        collect_required_relations(&program.root, &mut required);
        required
            .into_iter()
            .map(|relation_id| {
                let definition = bindings.relation(&relation_id)?;
                Ok(CatalogRelationBinding {
                    relation_id,
                    table_reference: definition.table_reference.clone(),
                })
            })
            .collect()
    }

    /// Compile a program against one immutable programmatic session authority
    /// and the native logical input plans obtained from that same session.
    pub fn compile_with_bindings(
        bindings: &ProgramBindings,
        inputs: impl IntoIterator<Item = RelationInput>,
        program: &RelationalProgram,
    ) -> Result<CompiledRelationalProgram, RelationalProgramError> {
        if program.output_fields.is_empty() {
            return Err(RelationalProgramError::InvalidProgram(
                "the output field contract is empty".to_owned(),
            ));
        }
        let mut input_plans = BTreeMap::new();
        for input in inputs {
            let key = input.relation_id.clone();
            if input_plans.insert(key.clone(), input.plan).is_some() {
                return Err(RelationalProgramError::DuplicateInput(key.0));
            }
        }
        let mut state = CompileState {
            bindings: bindings.clone(),
            input_plans,
            observations: CompilationObservations::default(),
        };
        state
            .observations
            .dependencies
            .insert(CompilationDependency::SessionAuthority(
                bindings.authority_id().to_owned(),
            ));
        let bound = state.compile_relational(&program.root)?;
        state.validate_declared_output(&bound, &program.output_fields)?;
        Ok(CompiledRelationalProgram {
            plan: bound.plan,
            observations: state.observations,
        })
    }
}

fn collect_required_relations(
    expression: &RelationalExpression,
    required: &mut BTreeSet<RelationId>,
) {
    match expression {
        RelationalExpression::Input(relation_id) => {
            required.insert(relation_id.clone());
        }
        RelationalExpression::Projection { input, .. }
        | RelationalExpression::Filter { input, .. }
        | RelationalExpression::Aggregate { input, .. }
        | RelationalExpression::Sort { input, .. }
        | RelationalExpression::Limit { input, .. } => {
            collect_required_relations(input, required);
        }
        RelationalExpression::Join { left, right, .. } => {
            collect_required_relations(left, required);
            collect_required_relations(right, required);
        }
        RelationalExpression::Union { inputs, .. } => {
            for input in inputs {
                collect_required_relations(input, required);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct RelationDefinition {
    table_reference: TableReference,
    fields: Vec<FieldId>,
}

#[derive(Clone, Debug)]
struct FieldDefinition {
    relation_id: RelationId,
    name: String,
    data_type: DataType,
    nullable: bool,
}

#[derive(Clone, Debug)]
struct IntrinsicDefinition {
    signature: String,
    semantic_level: String,
    implementation_id: String,
    package_id: String,
    compiler_authority_id: String,
}

/// Immutable relation/field bindings compiled from the exact schema contracts
/// installed in one candidate DataFusion session.
#[derive(Clone, Debug)]
pub struct ProgramBindings {
    authority_id: String,
    relations: BTreeMap<RelationId, RelationDefinition>,
    fields: BTreeMap<FieldId, FieldDefinition>,
    intrinsics: BTreeMap<PrimitiveId, IntrinsicDefinition>,
    expected_intrinsic_package: String,
    expected_compiler_authority: String,
}

impl ProgramBindings {
    /// Compile bindings from the live session's executable schema contracts.
    ///
    /// # Errors
    ///
    /// Rejects empty authority, partial qualifiers, contract/reference drift,
    /// missing or duplicate stable identities, and duplicate catalog objects.
    pub fn try_new(
        authority_id: impl Into<String>,
        contracts: impl IntoIterator<Item = ProgramRelationContract>,
    ) -> Result<Self, RelationalProgramError> {
        let authority_id = bounded_id(authority_id.into(), "session authority")?;
        let mut relations = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let mut table_references = BTreeSet::new();
        for binding in contracts {
            if !matches!(&binding.table_reference, TableReference::Full { .. }) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "relation {} has a non-full catalog reference {}",
                    binding.relation_id.0, binding.table_reference
                )));
            }
            if binding.contract.qualifier() != &binding.table_reference {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "relation {} contract qualifier {} differs from {}",
                    binding.relation_id.0,
                    binding.contract.qualifier(),
                    binding.table_reference
                )));
            }
            let contract_relation_id =
                binding
                    .contract
                    .relation_id(SchemaRole::Logical)
                    .map_err(|error| {
                        RelationalProgramError::InvalidProgram(format!(
                            "relation {} has no executable logical identity: {error}",
                            binding.relation_id.0
                        ))
                    })?;
            if contract_relation_id != binding.relation_id.as_str() {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "relation binding {} differs from schema identity {contract_relation_id}",
                    binding.relation_id.0
                )));
            }
            if !table_references.insert(binding.table_reference.clone()) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "catalog object {} is bound more than once",
                    binding.table_reference
                )));
            }

            let logical_schema = binding.contract.logical_schema();
            let mut relation_fields = Vec::with_capacity(logical_schema.fields().len());
            for (ordinal, field) in logical_schema.fields().iter().enumerate() {
                let field_id = FieldId::new(
                    binding
                        .contract
                        .field_id_at(SchemaRole::Logical, ordinal)
                        .map_err(|error| {
                            RelationalProgramError::InvalidProgram(format!(
                                "relation {} field {ordinal} has no executable identity: {error}",
                                binding.relation_id.0
                            ))
                        })?,
                )?;
                let definition = FieldDefinition {
                    relation_id: binding.relation_id.clone(),
                    name: field.name().clone(),
                    data_type: field.data_type().clone(),
                    nullable: field.is_nullable(),
                };
                if fields.insert(field_id.clone(), definition).is_some() {
                    return Err(RelationalProgramError::InvalidProgram(format!(
                        "field identity {} is bound more than once",
                        field_id.0
                    )));
                }
                relation_fields.push(field_id);
            }
            let definition = RelationDefinition {
                table_reference: binding.table_reference,
                fields: relation_fields,
            };
            if relations
                .insert(binding.relation_id.clone(), definition)
                .is_some()
            {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "relation identity {} is bound more than once",
                    binding.relation_id.0
                )));
            }
        }
        if relations.is_empty() {
            return Err(RelationalProgramError::InvalidProgram(
                "the session contains no executable relation contracts".to_owned(),
            ));
        }

        let expected_intrinsic_package = PROGRAM_COMPILER_AUTHORITY.to_owned();
        let expected_compiler_authority = PROGRAM_COMPILER_AUTHORITY.to_owned();
        let intrinsics = RelationalPrimitive::ALL
            .into_iter()
            .map(|primitive| {
                let id = PrimitiveId::new(primitive.id())?;
                Ok((
                    id,
                    IntrinsicDefinition {
                        signature: primitive.signature().to_owned(),
                        semantic_level: primitive.semantic_level().to_owned(),
                        implementation_id: format!(
                            "{PROGRAM_COMPILER_AUTHORITY}:{}",
                            primitive.id()
                        ),
                        package_id: expected_intrinsic_package.clone(),
                        compiler_authority_id: expected_compiler_authority.clone(),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RelationalProgramError>>()?;
        Ok(Self {
            authority_id,
            relations,
            fields,
            intrinsics,
            expected_intrinsic_package,
            expected_compiler_authority,
        })
    }

    /// Extend these immutable epoch bindings for one compilation with exact query-local inputs.
    ///
    /// The returned binding set has a new authority identity derived from the parent authority and
    /// every supplemental relation/schema/content pin. The parent is not mutated, and supplemental
    /// relations cannot shadow an epoch relation, field, or table reference.
    ///
    /// # Errors
    ///
    /// Rejects duplicate/shadowing relations, fields, table references, ambiguous Arrow field
    /// names, or field ordinals outside `u32`.
    pub fn with_supplemental_relations(
        &self,
        supplemental: impl IntoIterator<Item = SupplementalProgramRelationBinding>,
    ) -> Result<Self, RelationalProgramError> {
        let mut supplemental = supplemental.into_iter().collect::<Vec<_>>();
        if supplemental.is_empty() {
            return Ok(self.clone());
        }
        supplemental.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));

        let authority_id = supplemental_program_authority(&self.authority_id, &supplemental);
        let mut extended = self.clone();
        extended.authority_id = authority_id;
        let mut table_references = extended
            .relations
            .values()
            .map(|relation| relation.table_reference.clone())
            .collect::<BTreeSet<_>>();

        for binding in supplemental {
            if extended.relations.contains_key(&binding.relation_id) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "supplemental relation {} shadows an epoch relation",
                    binding.relation_id.as_str()
                )));
            }
            if !table_references.insert(binding.table_reference.clone()) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "supplemental table reference {} shadows another relation",
                    binding.table_reference
                )));
            }

            let mut relation_fields = Vec::with_capacity(binding.field_ids.len());
            let mut field_names = BTreeSet::new();
            for (field_id, field) in binding.field_ids.iter().zip(binding.schema.fields()) {
                if !field_names.insert(field.name()) {
                    return Err(RelationalProgramError::InvalidProgram(format!(
                        "supplemental relation {} repeats Arrow field name {}",
                        binding.relation_id.as_str(),
                        field.name()
                    )));
                }
                if extended.fields.contains_key(field_id) {
                    return Err(RelationalProgramError::InvalidProgram(format!(
                        "supplemental field {} shadows an epoch or request field",
                        field_id.as_str()
                    )));
                }
                extended.fields.insert(
                    field_id.clone(),
                    FieldDefinition {
                        relation_id: binding.relation_id.clone(),
                        name: field.name().clone(),
                        data_type: field.data_type().clone(),
                        nullable: field.is_nullable(),
                    },
                );
                relation_fields.push(field_id.clone());
            }
            extended.relations.insert(
                binding.relation_id,
                RelationDefinition {
                    table_reference: binding.table_reference,
                    fields: relation_fields,
                },
            );
        }
        Ok(extended)
    }

    /// Exact candidate-session authority represented by these bindings.
    #[must_use]
    pub fn authority_id(&self) -> &str {
        &self.authority_id
    }

    fn relation(&self, id: &RelationId) -> Result<&RelationDefinition, RelationalProgramError> {
        self.relations
            .get(id)
            .ok_or_else(|| RelationalProgramError::UnresolvedRelation(id.0.clone()))
    }

    fn field(&self, id: &FieldId) -> Result<&FieldDefinition, RelationalProgramError> {
        self.fields
            .get(id)
            .ok_or_else(|| RelationalProgramError::UnresolvedField(id.0.clone()))
    }

    fn intrinsic(
        &self,
        primitive: RelationalPrimitive,
    ) -> Result<(PrimitiveId, &IntrinsicDefinition), RelationalProgramError> {
        let id = PrimitiveId::new(primitive.id())?;
        let definition = self
            .intrinsics
            .get(&id)
            .filter(|definition| {
                definition.signature == primitive.signature()
                    && definition.semantic_level == primitive.semantic_level()
                    && definition.package_id == self.expected_intrinsic_package
                    && definition.compiler_authority_id == self.expected_compiler_authority
            })
            .ok_or_else(|| RelationalProgramError::UnresolvedPrimitive(id.0.clone()))?;
        Ok((id, definition))
    }
}

fn supplemental_program_authority(
    parent_authority: &str,
    supplemental: &[SupplementalProgramRelationBinding],
) -> String {
    fn frame(hasher: &mut blake3::Hasher, value: &[u8]) {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }

    let mut hasher = blake3::Hasher::new();
    frame(
        &mut hasher,
        b"codefabric.relational-program.supplemental-authority.v1",
    );
    frame(&mut hasher, parent_authority.as_bytes());
    for binding in supplemental {
        frame(&mut hasher, binding.relation_id.as_str().as_bytes());
        frame(&mut hasher, binding.table_reference.to_string().as_bytes());
        frame(&mut hasher, &binding.authority_pin);
        let mut schema_metadata = binding.schema.metadata().iter().collect::<Vec<_>>();
        schema_metadata.sort_by(|left, right| left.0.cmp(right.0));
        for (key, value) in schema_metadata {
            frame(&mut hasher, key.as_bytes());
            frame(&mut hasher, value.as_bytes());
        }
        for (field_id, field) in binding.field_ids.iter().zip(binding.schema.fields()) {
            frame(&mut hasher, field_id.as_str().as_bytes());
            frame(&mut hasher, field.name().as_bytes());
            frame(&mut hasher, format!("{:?}", field.data_type()).as_bytes());
            frame(&mut hasher, &[u8::from(field.is_nullable())]);
            let mut metadata = field.metadata().iter().collect::<Vec<_>>();
            metadata.sort_by(|left, right| left.0.cmp(right.0));
            for (key, value) in metadata {
                frame(&mut hasher, key.as_bytes());
                frame(&mut hasher, value.as_bytes());
            }
        }
    }
    format!("codefabric.request-bindings:{}", hasher.finalize().to_hex())
}

#[derive(Clone, Debug)]
struct BoundField {
    id: FieldId,
    column: Column,
}

#[derive(Clone, Debug)]
struct BoundPlan {
    plan: LogicalPlan,
    fields: Vec<BoundField>,
}

struct CompileState {
    bindings: ProgramBindings,
    input_plans: BTreeMap<RelationId, LogicalPlan>,
    observations: CompilationObservations,
}

impl CompileState {
    #[allow(clippy::too_many_lines)]
    fn compile_relational(
        &mut self,
        expression: &RelationalExpression,
    ) -> Result<BoundPlan, RelationalProgramError> {
        match expression {
            RelationalExpression::Input(relation_id) => self.compile_input(relation_id),
            RelationalExpression::Projection { input, expressions } => {
                let input = self.compile_relational(input)?;
                if expressions.is_empty() {
                    return Err(RelationalProgramError::InvalidProgram(
                        "projection requires at least one output".to_owned(),
                    ));
                }
                self.require_intrinsic(RelationalPrimitive::Project)?;
                let output_ids = expressions
                    .iter()
                    .map(|named| named.field_id.clone())
                    .collect::<Vec<_>>();
                self.validate_named_outputs(&output_ids)?;
                let projected = expressions
                    .iter()
                    .map(|named| {
                        let expression = self.compile_scalar(
                            &named.expression,
                            &input.fields,
                            input.plan.schema(),
                        )?;
                        self.validate_named_expression(
                            &named.field_id,
                            &expression,
                            input.plan.schema(),
                        )?;
                        Ok(expression.alias(self.bindings.field(&named.field_id)?.name.clone()))
                    })
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                let plan = LogicalPlanBuilder::from(input.plan)
                    .project(projected)?
                    .build()?;
                let bound = self.bind_declared_output(plan, &output_ids)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Projection,
                    ));
                Ok(bound)
            }
            RelationalExpression::Filter { input, predicate } => {
                let input = self.compile_relational(input)?;
                self.require_intrinsic(RelationalPrimitive::Filter)?;
                let predicate =
                    self.compile_scalar(predicate, &input.fields, input.plan.schema())?;
                self.require_boolean(&predicate, input.plan.schema(), "filter predicate")?;
                let plan = LogicalPlanBuilder::from(input.plan)
                    .filter(predicate)?
                    .build()?;
                let bound = rebind_preserved(plan, &input.fields)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Filter,
                    ));
                Ok(bound)
            }
            RelationalExpression::Join {
                left,
                right,
                kind,
                predicates,
            } => {
                if predicates.is_empty() {
                    return Err(RelationalProgramError::InvalidProgram(
                        "join requires at least one typed predicate".to_owned(),
                    ));
                }
                let left = self.compile_relational(left)?;
                let right = self.compile_relational(right)?;
                let mut joined_fields = left.fields.clone();
                for field in &right.fields {
                    if joined_fields.iter().any(|left| left.id == field.id) {
                        return Err(RelationalProgramError::InvalidProgram(format!(
                            "join scope repeats field ID {}; distinct model aliases are required",
                            field.id.0
                        )));
                    }
                    joined_fields.push(field.clone());
                }
                let joined_schema = left.plan.schema().join(right.plan.schema())?;
                let predicates = predicates
                    .iter()
                    .map(|predicate| {
                        let predicate =
                            self.compile_scalar(predicate, &joined_fields, &joined_schema)?;
                        self.require_boolean(&predicate, &joined_schema, "join predicate")?;
                        Ok(predicate)
                    })
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                self.require_intrinsic(RelationalPrimitive::Join)?;
                let plan = LogicalPlanBuilder::from(left.plan)
                    .join_on(right.plan, kind.datafusion(), predicates)?
                    .build()?;
                let retained = joined_fields
                    .into_iter()
                    .filter(|field| {
                        (kind.retains_left() && left.fields.iter().any(|left| left.id == field.id))
                            || (kind.retains_right()
                                && right.fields.iter().any(|right| right.id == field.id))
                    })
                    .collect::<Vec<_>>();
                let bound = rebind_preserved(plan, &retained)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Join(*kind),
                    ));
                Ok(bound)
            }
            RelationalExpression::Union { inputs, kind } => {
                let mut inputs = inputs.iter();
                let first = inputs.next().ok_or_else(|| {
                    RelationalProgramError::InvalidProgram(
                        "union requires at least two inputs".to_owned(),
                    )
                })?;
                let mut accumulated = self.compile_relational(first)?;
                let mut input_count = 1_usize;
                for input in inputs {
                    input_count += 1;
                    let right = self.compile_relational(input)?;
                    validate_union_inputs(&accumulated, &right)?;
                    let builder = LogicalPlanBuilder::from(accumulated.plan);
                    let plan = match kind {
                        UnionKind::All => builder.union(right.plan)?,
                        UnionKind::Distinct => builder.union_distinct(right.plan)?,
                    }
                    .build()?;
                    accumulated = rebind_preserved(plan, &accumulated.fields)?;
                }
                if input_count < 2 {
                    return Err(RelationalProgramError::InvalidProgram(
                        "union requires at least two inputs".to_owned(),
                    ));
                }
                self.require_intrinsic(RelationalPrimitive::Union)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Union(*kind),
                    ));
                Ok(accumulated)
            }
            RelationalExpression::Aggregate {
                input,
                group_by,
                aggregates,
            } => {
                if group_by.is_empty() && aggregates.is_empty() {
                    return Err(RelationalProgramError::InvalidProgram(
                        "aggregate requires a grouping or aggregate expression".to_owned(),
                    ));
                }
                let input = self.compile_relational(input)?;
                let output_ids = group_by
                    .iter()
                    .map(|named| named.field_id.clone())
                    .chain(aggregates.iter().map(|named| named.field_id.clone()))
                    .collect::<Vec<_>>();
                self.validate_named_outputs(&output_ids)?;
                let groups = group_by
                    .iter()
                    .map(|named| {
                        let expression = self.compile_scalar(
                            &named.expression,
                            &input.fields,
                            input.plan.schema(),
                        )?;
                        self.validate_named_expression(
                            &named.field_id,
                            &expression,
                            input.plan.schema(),
                        )?;
                        Ok(expression.alias(self.bindings.field(&named.field_id)?.name.clone()))
                    })
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                let aggregates = aggregates
                    .iter()
                    .map(|named| {
                        let expression = self.compile_aggregate(
                            &named.expression,
                            &input.fields,
                            input.plan.schema(),
                        )?;
                        self.validate_named_expression(
                            &named.field_id,
                            &expression,
                            input.plan.schema(),
                        )?;
                        Ok(expression.alias(self.bindings.field(&named.field_id)?.name.clone()))
                    })
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                self.require_intrinsic(RelationalPrimitive::Aggregate)?;
                let plan = LogicalPlanBuilder::from(input.plan)
                    .aggregate(groups, aggregates)?
                    .build()?;
                let bound = self.bind_declared_output(plan, &output_ids)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Aggregate,
                    ));
                Ok(bound)
            }
            RelationalExpression::Sort { input, expressions } => {
                if expressions.is_empty() {
                    return Err(RelationalProgramError::InvalidProgram(
                        "sort requires at least one expression".to_owned(),
                    ));
                }
                let input = self.compile_relational(input)?;
                let sorts = expressions
                    .iter()
                    .map(|sort| {
                        let expression = self.compile_scalar(
                            &sort.expression,
                            &input.fields,
                            input.plan.schema(),
                        )?;
                        expression.get_type(input.plan.schema())?;
                        Ok(expression.sort(sort.ascending, sort.nulls_first))
                    })
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                let plan = LogicalPlanBuilder::from(input.plan).sort(sorts)?.build()?;
                let bound = rebind_preserved(plan, &input.fields)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Sort,
                    ));
                Ok(bound)
            }
            RelationalExpression::Limit { input, skip, fetch } => {
                if matches!(fetch, Some(0)) {
                    return Err(RelationalProgramError::InvalidProgram(
                        "limit fetch must be positive when present".to_owned(),
                    ));
                }
                let input = self.compile_relational(input)?;
                let plan = LogicalPlanBuilder::from(input.plan)
                    .limit(*skip, *fetch)?
                    .build()?;
                let bound = rebind_preserved(plan, &input.fields)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::NativeLogical(
                        NativeLogicalSelection::Limit,
                    ));
                Ok(bound)
            }
        }
    }

    fn compile_input(
        &mut self,
        relation_id: &RelationId,
    ) -> Result<BoundPlan, RelationalProgramError> {
        let relation = self.bindings.relation(relation_id)?.clone();
        let input = self.input_plans.get(relation_id).cloned().ok_or_else(|| {
            RelationalProgramError::MissingInput {
                relation: relation_id.0.clone(),
            }
        })?;
        let plan = LogicalPlanBuilder::from(input)
            .alias(relation.table_reference.clone())?
            .build()?;
        if plan.schema().fields().len() != relation.fields.len() {
            return Err(RelationalProgramError::OutputSchema(format!(
                "input {} has {} live fields but the model declares {}",
                relation_id.0,
                plan.schema().fields().len(),
                relation.fields.len()
            )));
        }
        let fields = relation
            .fields
            .iter()
            .map(|field_id| {
                let definition = self.bindings.field(field_id)?;
                let column = Column::new(
                    Some(relation.table_reference.clone()),
                    definition.name.clone(),
                );
                let index = plan.schema().index_of_column(&column)?;
                let actual = plan.schema().field(index);
                validate_field_contract(
                    field_id,
                    definition,
                    actual.data_type(),
                    actual.is_nullable(),
                )?;
                self.observations
                    .dependencies
                    .insert(CompilationDependency::Field(field_id.clone()));
                Ok(BoundField {
                    id: field_id.clone(),
                    column,
                })
            })
            .collect::<Result<Vec<_>, RelationalProgramError>>()?;
        self.observations
            .dependencies
            .insert(CompilationDependency::Relation(relation_id.clone()));
        Ok(BoundPlan { plan, fields })
    }

    fn compile_scalar(
        &mut self,
        expression: &ScalarExpression,
        fields: &[BoundField],
        schema: &DFSchema,
    ) -> Result<Expr, RelationalProgramError> {
        match expression {
            ScalarExpression::Field(field_id) => {
                let field = fields
                    .iter()
                    .find(|field| field.id == *field_id)
                    .ok_or_else(|| RelationalProgramError::FieldOutOfScope {
                        field: field_id.0.clone(),
                    })?;
                schema.index_of_column(&field.column)?;
                self.bindings.field(field_id)?;
                self.observations
                    .dependencies
                    .insert(CompilationDependency::Field(field_id.clone()));
                Ok(Expr::Column(field.column.clone()))
            }
            ScalarExpression::Literal(value) => Ok(Expr::Literal(value.clone(), None)),
            ScalarExpression::Call {
                operator,
                arguments,
            } => {
                let expected = scalar_arity(*operator);
                if arguments.len() != expected {
                    return Err(RelationalProgramError::InvalidArity {
                        operator: format!("{operator:?}"),
                        expected,
                        observed: arguments.len(),
                    });
                }
                self.require_intrinsic(RelationalPrimitive::ScalarFunction)?;
                let mut arguments = arguments
                    .iter()
                    .map(|argument| self.compile_scalar(argument, fields, schema))
                    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
                if *operator == ScalarOperator::Not {
                    self.require_boolean(&arguments[0], schema, "NOT argument")?;
                }
                let expression = match operator {
                    ScalarOperator::Equal => arguments.remove(0).eq(arguments.remove(0)),
                    ScalarOperator::NotEqual => arguments.remove(0).not_eq(arguments.remove(0)),
                    ScalarOperator::LessThan => arguments.remove(0).lt(arguments.remove(0)),
                    ScalarOperator::LessThanOrEqual => {
                        arguments.remove(0).lt_eq(arguments.remove(0))
                    }
                    ScalarOperator::GreaterThan => arguments.remove(0).gt(arguments.remove(0)),
                    ScalarOperator::GreaterThanOrEqual => {
                        arguments.remove(0).gt_eq(arguments.remove(0))
                    }
                    ScalarOperator::And => arguments.remove(0).and(arguments.remove(0)),
                    ScalarOperator::Or => arguments.remove(0).or(arguments.remove(0)),
                    ScalarOperator::Not => arguments.remove(0).not(),
                    ScalarOperator::Add => arguments.remove(0) + arguments.remove(0),
                    ScalarOperator::Subtract => arguments.remove(0) - arguments.remove(0),
                    ScalarOperator::Multiply => arguments.remove(0) * arguments.remove(0),
                    ScalarOperator::Divide => arguments.remove(0) / arguments.remove(0),
                    ScalarOperator::IsNull => arguments.remove(0).is_null(),
                    ScalarOperator::IsNotNull => arguments.remove(0).is_not_null(),
                };
                expression.get_type(schema)?;
                expression.nullable(schema)?;
                self.observations
                    .extension_selections
                    .insert(ExtensionSelection::BuiltInScalar(*operator));
                Ok(expression)
            }
        }
    }

    fn compile_aggregate(
        &mut self,
        aggregate: &AggregateExpression,
        fields: &[BoundField],
        schema: &DFSchema,
    ) -> Result<Expr, RelationalProgramError> {
        self.require_intrinsic(RelationalPrimitive::AggregateFunction)?;
        let argument = self.compile_scalar(&aggregate.argument, fields, schema)?;
        let expression = match aggregate.operator {
            AggregateOperator::Count => count(argument),
            AggregateOperator::CountDistinct => count_distinct(argument),
            AggregateOperator::Sum => sum(argument),
            AggregateOperator::Average => avg(argument),
            AggregateOperator::Minimum => min(argument),
            AggregateOperator::Maximum => max(argument),
        };
        expression.get_type(schema)?;
        expression.nullable(schema)?;
        self.observations
            .extension_selections
            .insert(ExtensionSelection::BuiltInAggregate(aggregate.operator));
        Ok(expression)
    }

    fn require_intrinsic(
        &mut self,
        primitive: RelationalPrimitive,
    ) -> Result<(), RelationalProgramError> {
        let (primitive_id, definition) = self.bindings.intrinsic(primitive)?;
        self.observations
            .dependencies
            .insert(CompilationDependency::Primitive {
                primitive_id,
                implementation_id: definition.implementation_id.clone(),
            });
        Ok(())
    }

    fn require_boolean(
        &self,
        expression: &Expr,
        schema: &DFSchema,
        context: &str,
    ) -> Result<(), RelationalProgramError> {
        let observed = expression.get_type(schema)?;
        if observed == DataType::Boolean {
            Ok(())
        } else {
            Err(RelationalProgramError::TypeMismatch {
                context: context.to_owned(),
                expected: DataType::Boolean,
                observed,
            })
        }
    }

    fn validate_named_expression(
        &self,
        field_id: &FieldId,
        expression: &Expr,
        schema: &DFSchema,
    ) -> Result<(), RelationalProgramError> {
        let expected = self.bindings.field(field_id)?;
        let data_type = expression.get_type(schema)?;
        if data_type != expected.data_type {
            return Err(RelationalProgramError::TypeMismatch {
                context: field_id.0.clone(),
                expected: expected.data_type.clone(),
                observed: data_type,
            });
        }
        let nullable = expression.nullable(schema)?;
        if nullable != expected.nullable {
            return Err(RelationalProgramError::NullabilityMismatch {
                context: field_id.0.clone(),
                expected: expected.nullable,
                observed: nullable,
            });
        }
        Ok(())
    }

    fn validate_named_outputs(&self, output_ids: &[FieldId]) -> Result<(), RelationalProgramError> {
        let mut seen = BTreeSet::new();
        let mut relation = None;
        for field_id in output_ids {
            if !seen.insert(field_id) {
                return Err(RelationalProgramError::InvalidProgram(format!(
                    "output field {} is repeated",
                    field_id.0
                )));
            }
            let definition = self.bindings.field(field_id)?;
            match &relation {
                None => relation = Some(definition.relation_id.clone()),
                Some(relation_id) if relation_id != &definition.relation_id => {
                    return Err(RelationalProgramError::InvalidProgram(
                        "named outputs must belong to one relation".to_owned(),
                    ));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    fn bind_declared_output(
        &self,
        plan: LogicalPlan,
        output_ids: &[FieldId],
    ) -> Result<BoundPlan, RelationalProgramError> {
        if output_ids.is_empty() {
            return Err(RelationalProgramError::InvalidProgram(
                "declared output is empty".to_owned(),
            ));
        }
        let relation_id = self.bindings.field(&output_ids[0])?.relation_id.clone();
        let relation = self.bindings.relation(&relation_id)?;
        let plan = LogicalPlanBuilder::from(plan)
            .alias(relation.table_reference.clone())?
            .build()?;
        let fields = output_ids
            .iter()
            .map(|field_id| {
                let definition = self.bindings.field(field_id)?;
                let column = Column::new(
                    Some(relation.table_reference.clone()),
                    definition.name.clone(),
                );
                let index = plan.schema().index_of_column(&column)?;
                let field = plan.schema().field(index);
                validate_field_contract(
                    field_id,
                    definition,
                    field.data_type(),
                    field.is_nullable(),
                )?;
                Ok(BoundField {
                    id: field_id.clone(),
                    column,
                })
            })
            .collect::<Result<Vec<_>, RelationalProgramError>>()?;
        Ok(BoundPlan { plan, fields })
    }

    fn validate_declared_output(
        &self,
        bound: &BoundPlan,
        expected_ids: &[FieldId],
    ) -> Result<(), RelationalProgramError> {
        if bound.fields.len() != expected_ids.len()
            || bound
                .fields
                .iter()
                .zip(expected_ids)
                .any(|(observed, expected)| &observed.id != expected)
        {
            return Err(RelationalProgramError::OutputSchema(format!(
                "field IDs are {:?}, expected {:?}",
                bound
                    .fields
                    .iter()
                    .map(|field| field.id.as_str())
                    .collect::<Vec<_>>(),
                expected_ids.iter().map(FieldId::as_str).collect::<Vec<_>>()
            )));
        }
        if bound.plan.schema().fields().len() != expected_ids.len() {
            return Err(RelationalProgramError::OutputSchema(format!(
                "live plan has {} fields, expected {}",
                bound.plan.schema().fields().len(),
                expected_ids.len()
            )));
        }
        for (index, field_id) in expected_ids.iter().enumerate() {
            let expected = self.bindings.field(field_id)?;
            let actual = bound.plan.schema().field(index);
            if actual.name() != &expected.name {
                return Err(RelationalProgramError::OutputSchema(format!(
                    "field {} is named {}, expected {}",
                    field_id.0,
                    actual.name(),
                    expected.name
                )));
            }
            validate_field_contract(field_id, expected, actual.data_type(), actual.is_nullable())?;
        }
        Ok(())
    }
}

const fn scalar_arity(operator: ScalarOperator) -> usize {
    match operator {
        ScalarOperator::Not | ScalarOperator::IsNull | ScalarOperator::IsNotNull => 1,
        ScalarOperator::Equal
        | ScalarOperator::NotEqual
        | ScalarOperator::LessThan
        | ScalarOperator::LessThanOrEqual
        | ScalarOperator::GreaterThan
        | ScalarOperator::GreaterThanOrEqual
        | ScalarOperator::And
        | ScalarOperator::Or
        | ScalarOperator::Add
        | ScalarOperator::Subtract
        | ScalarOperator::Multiply
        | ScalarOperator::Divide => 2,
    }
}

fn validate_field_contract(
    field_id: &FieldId,
    definition: &FieldDefinition,
    observed_type: &DataType,
    observed_nullable: bool,
) -> Result<(), RelationalProgramError> {
    if observed_type != &definition.data_type {
        return Err(RelationalProgramError::TypeMismatch {
            context: field_id.0.clone(),
            expected: definition.data_type.clone(),
            observed: observed_type.clone(),
        });
    }
    if observed_nullable != definition.nullable {
        return Err(RelationalProgramError::NullabilityMismatch {
            context: field_id.0.clone(),
            expected: definition.nullable,
            observed: observed_nullable,
        });
    }
    Ok(())
}

fn rebind_preserved(
    plan: LogicalPlan,
    prior: &[BoundField],
) -> Result<BoundPlan, RelationalProgramError> {
    let fields = prior
        .iter()
        .map(|prior| {
            let index = match plan.schema().index_of_column(&prior.column) {
                Ok(index) => index,
                Err(qualified_error) => {
                    // Native set operations intentionally strip input qualifiers.
                    // Fall back only when the live schema has one exact name match;
                    // ambiguity remains a hard binding error.
                    let mut matches = plan
                        .schema()
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, field))| field.name() == &prior.column.name)
                        .map(|(index, _)| index);
                    match (matches.next(), matches.next()) {
                        (Some(index), None) => index,
                        _ => return Err(RelationalProgramError::DataFusion(qualified_error)),
                    }
                }
            };
            let (qualifier, field) = plan.schema().qualified_field(index);
            let column = match qualifier {
                Some(qualifier) => Column::new(Some(qualifier.clone()), field.name().clone()),
                None => Column::new_unqualified(field.name().clone()),
            };
            Ok(BoundField {
                id: prior.id.clone(),
                column,
            })
        })
        .collect::<Result<Vec<_>, RelationalProgramError>>()?;
    Ok(BoundPlan { plan, fields })
}

fn validate_union_inputs(
    left: &BoundPlan,
    right: &BoundPlan,
) -> Result<(), RelationalProgramError> {
    if left.fields.len() != right.fields.len()
        || left
            .fields
            .iter()
            .zip(&right.fields)
            .any(|(left, right)| left.id != right.id)
    {
        return Err(RelationalProgramError::InvalidProgram(
            "union inputs have different field-ID contracts".to_owned(),
        ));
    }
    if left.plan.schema().fields().len() != right.plan.schema().fields().len() {
        return Err(RelationalProgramError::InvalidProgram(
            "union inputs have different live widths".to_owned(),
        ));
    }
    for (left, right) in left
        .plan
        .schema()
        .fields()
        .iter()
        .zip(right.plan.schema().fields())
    {
        if left.name() != right.name()
            || left.data_type() != right.data_type()
            || left.is_nullable() != right.is_nullable()
        {
            return Err(RelationalProgramError::InvalidProgram(format!(
                "union fields differ: {left:?} versus {right:?}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow_schema::{Field, Schema};
    use datafusion::logical_expr::logical_plan::builder::LogicalTableSource;

    use super::*;
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY,
    };

    const LEFT: &str = "test.relation.left";
    const RIGHT: &str = "test.relation.right";
    const SUMMARY: &str = "test.relation.summary";
    const LEFT_ID: &str = "test.field.left.id";
    const LEFT_GROUP: &str = "test.field.left.group";
    const LEFT_VALUE: &str = "test.field.left.value";
    const RIGHT_ID: &str = "test.field.right.id";
    const RIGHT_LABEL: &str = "test.field.right.label";
    const SUMMARY_GROUP: &str = "test.field.summary.group";
    const SUMMARY_TOTAL: &str = "test.field.summary.total";

    fn id(value: &str) -> FieldId {
        FieldId::new(value).unwrap()
    }

    fn relation_id(value: &str) -> RelationId {
        RelationId::new(value).unwrap()
    }

    fn scan(name: &str, fields: Vec<Field>) -> LogicalPlan {
        let source = Arc::new(LogicalTableSource::new(Arc::new(Schema::new(fields))));
        LogicalPlanBuilder::scan(name, source, None)
            .unwrap()
            .build()
            .unwrap()
    }

    fn program_contract(
        relation_id: &str,
        table_name: &str,
        fields: Vec<(&str, &str, DataType, bool)>,
    ) -> ProgramRelationContract {
        let fields = fields
            .into_iter()
            .map(|(field_id, name, data_type, nullable)| {
                Field::new(name, data_type, nullable).with_metadata(HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    field_id.to_owned(),
                )]))
            })
            .collect::<Vec<_>>();
        let schema = Arc::new(Schema::new(fields).with_metadata(HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            relation_id.to_owned(),
        )])));
        let qualifier = TableReference::full("codefabric", "test", table_name);
        let mappings = (0..schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect();
        ProgramRelationContract {
            relation_id: RelationId::new(relation_id).unwrap(),
            table_reference: qualifier.clone(),
            contract: Arc::new(
                SchemaContract::try_new(
                    format!("provider:{relation_id}"),
                    qualifier,
                    Arc::clone(&schema),
                    schema,
                    mappings,
                )
                .unwrap(),
            ),
        }
    }

    fn program_bindings() -> ProgramBindings {
        ProgramBindings::try_new(
            "candidate-session:test",
            [
                program_contract(
                    LEFT,
                    "left",
                    vec![
                        (LEFT_ID, "id", DataType::Int64, false),
                        (LEFT_GROUP, "group_name", DataType::Utf8, false),
                        (LEFT_VALUE, "value", DataType::Int64, true),
                    ],
                ),
                program_contract(
                    RIGHT,
                    "right",
                    vec![
                        (RIGHT_ID, "id", DataType::Int64, false),
                        (RIGHT_LABEL, "label", DataType::Utf8, true),
                    ],
                ),
                program_contract(
                    SUMMARY,
                    "summary",
                    vec![
                        (SUMMARY_GROUP, "group_name", DataType::Utf8, false),
                        (SUMMARY_TOTAL, "total", DataType::Int64, true),
                    ],
                ),
            ],
        )
        .unwrap()
    }

    fn inputs() -> Vec<RelationInput> {
        vec![
            RelationInput {
                relation_id: relation_id(LEFT),
                plan: scan(
                    "physical_left",
                    vec![
                        Field::new("id", DataType::Int64, false),
                        Field::new("group_name", DataType::Utf8, false),
                        Field::new("value", DataType::Int64, true),
                    ],
                ),
            },
            RelationInput {
                relation_id: relation_id(RIGHT),
                plan: scan(
                    "physical_right",
                    vec![
                        Field::new("id", DataType::Int64, false),
                        Field::new("label", DataType::Utf8, true),
                    ],
                ),
            },
        ]
    }

    fn field(field: &str) -> ScalarExpression {
        ScalarExpression::Field(id(field))
    }

    fn equals(left: &str, right: &str) -> ScalarExpression {
        ScalarExpression::Call {
            operator: ScalarOperator::Equal,
            arguments: vec![field(left), field(right)],
        }
    }

    #[test]
    fn programmatic_bindings_compile_from_live_schema_contracts_without_model_rows() {
        let program = RelationalProgram {
            root: RelationalExpression::Aggregate {
                input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                group_by: vec![NamedExpression {
                    field_id: id(SUMMARY_GROUP),
                    expression: field(LEFT_GROUP),
                }],
                aggregates: vec![NamedAggregateExpression {
                    field_id: id(SUMMARY_TOTAL),
                    expression: AggregateExpression {
                        operator: AggregateOperator::Sum,
                        argument: field(LEFT_VALUE),
                    },
                }],
            },
            output_fields: vec![id(SUMMARY_GROUP), id(SUMMARY_TOTAL)],
        };
        let bindings = program_bindings();
        let compiled =
            RelationalProgramCompiler::compile_with_bindings(&bindings, inputs(), &program)
                .unwrap();

        assert!(matches!(compiled.plan, LogicalPlan::SubqueryAlias(_)));
        assert_eq!(compiled.plan.schema().field(0).name(), "group_name");
        assert_eq!(compiled.plan.schema().field(1).name(), "total");
        assert!(compiled.observations.dependencies.contains(
            &CompilationDependency::SessionAuthority("candidate-session:test".to_owned())
        ));
    }

    #[test]
    fn compiles_projection_filter_aggregate_sort_and_limit_to_native_nodes() {
        let program = RelationalProgram {
            root: RelationalExpression::Limit {
                input: Box::new(RelationalExpression::Sort {
                    input: Box::new(RelationalExpression::Aggregate {
                        input: Box::new(RelationalExpression::Filter {
                            input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                            predicate: ScalarExpression::Call {
                                operator: ScalarOperator::GreaterThan,
                                arguments: vec![
                                    field(LEFT_VALUE),
                                    ScalarExpression::Literal(ScalarValue::Int64(Some(0))),
                                ],
                            },
                        }),
                        group_by: vec![NamedExpression {
                            field_id: id(SUMMARY_GROUP),
                            expression: field(LEFT_GROUP),
                        }],
                        aggregates: vec![NamedAggregateExpression {
                            field_id: id(SUMMARY_TOTAL),
                            expression: AggregateExpression {
                                operator: AggregateOperator::Sum,
                                argument: field(LEFT_VALUE),
                            },
                        }],
                    }),
                    expressions: vec![SortExpression {
                        expression: field(SUMMARY_TOTAL),
                        ascending: false,
                        nulls_first: false,
                    }],
                }),
                skip: 2,
                fetch: Some(5),
            },
            output_fields: vec![id(SUMMARY_GROUP), id(SUMMARY_TOTAL)],
        };
        let compiled = RelationalProgramCompiler::compile_with_bindings(
            &program_bindings(),
            inputs(),
            &program,
        )
        .unwrap();
        assert!(matches!(compiled.plan, LogicalPlan::Limit(_)));
        for selection in [
            NativeLogicalSelection::Filter,
            NativeLogicalSelection::Aggregate,
            NativeLogicalSelection::Sort,
            NativeLogicalSelection::Limit,
        ] {
            assert!(
                compiled
                    .observations
                    .extension_selections
                    .contains(&ExtensionSelection::NativeLogical(selection))
            );
        }
        assert!(compiled.observations.extension_selections.contains(
            &ExtensionSelection::BuiltInAggregate(AggregateOperator::Sum)
        ));
        assert!(compiled.observations.extension_selections.contains(
            &ExtensionSelection::BuiltInScalar(ScalarOperator::GreaterThan)
        ));
        let expected_aggregate_implementation =
            format!("{PROGRAM_COMPILER_AUTHORITY}:rel.aggregate");
        assert!(compiled.observations.dependencies.iter().any(|dependency| {
            matches!(
                dependency,
                CompilationDependency::Primitive { primitive_id, implementation_id }
                    if primitive_id.as_str() == "rel.aggregate"
                        && implementation_id == &expected_aggregate_implementation
            )
        }));
    }

    #[test]
    fn projection_uses_program_output_identity_and_live_schema() {
        let program = RelationalProgram {
            root: RelationalExpression::Projection {
                input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                expressions: vec![
                    NamedExpression {
                        field_id: id(LEFT_GROUP),
                        expression: field(LEFT_GROUP),
                    },
                    NamedExpression {
                        field_id: id(LEFT_VALUE),
                        expression: field(LEFT_VALUE),
                    },
                ],
            },
            output_fields: vec![id(LEFT_GROUP), id(LEFT_VALUE)],
        };
        let compiled = RelationalProgramCompiler::compile_with_bindings(
            &program_bindings(),
            inputs(),
            &program,
        )
        .unwrap();
        assert!(matches!(compiled.plan, LogicalPlan::SubqueryAlias(_)));
        assert_eq!(compiled.plan.schema().field(0).name(), "group_name");
        assert_eq!(compiled.plan.schema().field(1).name(), "value");
    }

    #[test]
    fn compiles_inner_semi_and_anti_joins_without_custom_nodes() {
        for kind in [JoinKind::Inner, JoinKind::LeftSemi, JoinKind::LeftAnti] {
            let output_fields = if kind == JoinKind::Inner {
                vec![
                    id(LEFT_ID),
                    id(LEFT_GROUP),
                    id(LEFT_VALUE),
                    id(RIGHT_ID),
                    id(RIGHT_LABEL),
                ]
            } else {
                vec![id(LEFT_ID), id(LEFT_GROUP), id(LEFT_VALUE)]
            };
            let program = RelationalProgram {
                root: RelationalExpression::Join {
                    left: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                    right: Box::new(RelationalExpression::Input(relation_id(RIGHT))),
                    kind,
                    predicates: vec![equals(LEFT_ID, RIGHT_ID)],
                },
                output_fields,
            };
            let compiled = RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &program,
            )
            .unwrap();
            assert!(matches!(compiled.plan, LogicalPlan::Join(_)));
            assert!(compiled.observations.extension_selections.contains(
                &ExtensionSelection::NativeLogical(NativeLogicalSelection::Join(kind))
            ));
        }
    }

    #[test]
    fn compiles_union_all_and_distinct_with_exact_field_identity() {
        let right_as_left = || RelationalExpression::Projection {
            input: Box::new(RelationalExpression::Input(relation_id(RIGHT))),
            expressions: vec![
                NamedExpression {
                    field_id: id(LEFT_ID),
                    expression: field(RIGHT_ID),
                },
                NamedExpression {
                    field_id: id(LEFT_GROUP),
                    expression: ScalarExpression::Call {
                        operator: ScalarOperator::IsNull,
                        arguments: vec![field(RIGHT_LABEL)],
                    },
                },
            ],
        };
        // The deliberately Boolean second projection proves type rejection before
        // DataFusion can silently coerce a union input.
        let invalid = RelationalProgram {
            root: RelationalExpression::Union {
                inputs: vec![
                    RelationalExpression::Projection {
                        input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                        expressions: vec![
                            NamedExpression {
                                field_id: id(LEFT_ID),
                                expression: field(LEFT_ID),
                            },
                            NamedExpression {
                                field_id: id(LEFT_GROUP),
                                expression: field(LEFT_GROUP),
                            },
                        ],
                    },
                    right_as_left(),
                ],
                kind: UnionKind::All,
            },
            output_fields: vec![id(LEFT_ID), id(LEFT_GROUP)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &invalid,
            ),
            Err(RelationalProgramError::TypeMismatch { .. })
        ));

        for kind in [UnionKind::All, UnionKind::Distinct] {
            let program = RelationalProgram {
                root: RelationalExpression::Union {
                    inputs: vec![
                        RelationalExpression::Projection {
                            input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                            expressions: vec![NamedExpression {
                                field_id: id(LEFT_ID),
                                expression: field(LEFT_ID),
                            }],
                        },
                        RelationalExpression::Projection {
                            input: Box::new(RelationalExpression::Input(relation_id(RIGHT))),
                            expressions: vec![NamedExpression {
                                field_id: id(LEFT_ID),
                                expression: field(RIGHT_ID),
                            }],
                        },
                    ],
                    kind,
                },
                output_fields: vec![id(LEFT_ID)],
            };
            let compiled = RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &program,
            )
            .unwrap();
            assert!(compiled.observations.extension_selections.contains(
                &ExtensionSelection::NativeLogical(NativeLogicalSelection::Union(kind))
            ));
        }
    }

    #[test]
    fn rejects_unresolved_fields_ill_typed_predicates_and_bad_input_schema() {
        let unresolved = RelationalProgram {
            root: RelationalExpression::Filter {
                input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                predicate: field("test.field.missing"),
            },
            output_fields: vec![id(LEFT_ID), id(LEFT_GROUP), id(LEFT_VALUE)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &unresolved,
            ),
            Err(RelationalProgramError::FieldOutOfScope { .. })
        ));

        let ill_typed = RelationalProgram {
            root: RelationalExpression::Filter {
                input: Box::new(RelationalExpression::Input(relation_id(LEFT))),
                predicate: field(LEFT_GROUP),
            },
            output_fields: vec![id(LEFT_ID), id(LEFT_GROUP), id(LEFT_VALUE)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &ill_typed,
            ),
            Err(RelationalProgramError::TypeMismatch { .. })
        ));

        let wrong_schema = vec![RelationInput {
            relation_id: relation_id(LEFT),
            plan: scan(
                "wrong",
                vec![
                    Field::new("id", DataType::Utf8, false),
                    Field::new("group_name", DataType::Utf8, false),
                    Field::new("value", DataType::Int64, true),
                ],
            ),
        }];
        let input_only = RelationalProgram {
            root: RelationalExpression::Input(relation_id(LEFT)),
            output_fields: vec![id(LEFT_ID), id(LEFT_GROUP), id(LEFT_VALUE)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                wrong_schema,
                &input_only,
            ),
            Err(RelationalProgramError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn rejects_output_reordering_and_duplicate_relation_inputs() {
        let reordered = RelationalProgram {
            root: RelationalExpression::Input(relation_id(LEFT)),
            output_fields: vec![id(LEFT_GROUP), id(LEFT_ID), id(LEFT_VALUE)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                inputs(),
                &reordered,
            ),
            Err(RelationalProgramError::OutputSchema(_))
        ));

        let mut duplicated = inputs();
        duplicated.push(RelationInput {
            relation_id: relation_id(LEFT),
            plan: scan(
                "duplicate",
                vec![
                    Field::new("id", DataType::Int64, false),
                    Field::new("group_name", DataType::Utf8, false),
                    Field::new("value", DataType::Int64, true),
                ],
            ),
        });
        let input_only = RelationalProgram {
            root: RelationalExpression::Input(relation_id(LEFT)),
            output_fields: vec![id(LEFT_ID), id(LEFT_GROUP), id(LEFT_VALUE)],
        };
        assert!(matches!(
            RelationalProgramCompiler::compile_with_bindings(
                &program_bindings(),
                duplicated,
                &input_only,
            ),
            Err(RelationalProgramError::DuplicateInput(_))
        ));
    }

    #[test]
    fn supplemental_request_binding_is_causal_and_compilable_without_mutating_epoch_bindings() {
        let base = program_bindings();
        let request_relation = relation_id("request.input.entities");
        let request_field = id("request.input.entity_id");
        let request_schema = Arc::new(Schema::new(vec![Field::new(
            "entity_id",
            DataType::Utf8,
            false,
        )]));
        let supplemental = SupplementalProgramRelationBinding::try_new(
            request_relation.clone(),
            TableReference::bare("request_input_entities"),
            Arc::clone(&request_schema),
            vec![request_field.clone()],
            [0x51; 32],
        )
        .unwrap();
        let extended = base.with_supplemental_relations([supplemental]).unwrap();
        assert_ne!(extended.authority_id(), base.authority_id());
        assert!(base.relation(&request_relation).is_err());

        let program = RelationalProgram {
            root: RelationalExpression::Input(request_relation.clone()),
            output_fields: vec![request_field.clone()],
        };
        let compiled = RelationalProgramCompiler::compile_with_bindings(
            &extended,
            [RelationInput {
                relation_id: request_relation,
                plan: scan(
                    "request_input_entities",
                    vec![Field::new("entity_id", DataType::Utf8, false)],
                ),
            }],
            &program,
        )
        .unwrap();
        assert_eq!(compiled.plan.schema().fields().len(), 1);
        assert!(compiled.observations.dependencies.contains(
            &CompilationDependency::SessionAuthority(extended.authority_id().to_owned())
        ));
    }

    #[test]
    fn supplemental_bindings_reject_shadowing_and_bind_content_into_authority() {
        let base = program_bindings();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let binding = |relation: &str, field: &str, pin: u8| {
            SupplementalProgramRelationBinding::try_new(
                relation_id(relation),
                TableReference::bare(relation.replace('.', "_")),
                Arc::clone(&schema),
                vec![id(field)],
                [pin; 32],
            )
            .unwrap()
        };
        let first = base
            .with_supplemental_relations([binding(
                "request.input.values",
                "request.field.value",
                1,
            )])
            .unwrap();
        let second = base
            .with_supplemental_relations([binding(
                "request.input.values",
                "request.field.value",
                2,
            )])
            .unwrap();
        assert_ne!(first.authority_id(), second.authority_id());

        let shadow = SupplementalProgramRelationBinding::try_new(
            relation_id(LEFT),
            TableReference::bare("shadow"),
            schema,
            vec![id("request.field.shadow")],
            [3; 32],
        )
        .unwrap();
        assert!(matches!(
            base.with_supplemental_relations([shadow]),
            Err(RelationalProgramError::InvalidProgram(message))
                if message.contains("shadows an epoch relation")
        ));
    }
}
