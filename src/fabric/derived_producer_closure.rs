//! Execution-proved closure from accepted fact families to derived-analysis producers.
//!
//! All relation, field, authority, semantic-class, and release identities come from the installed
//! application contract. The compiler binds those identities and execution-count evidence to exact Arrow
//! schemas and constructs ordinary DataFusion logical operators. It owns no fact-family registry
//! and no SQL text. A family is closed only by exactly one execution-proved complete,
//! application-owned runtime producer or exactly one application-owned unsupported remainder.
//! Query requirements traverse the same closure and preserve unsupported, unknown, invalid, and
//! missing states.

use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU16, NonZeroUsize};
use std::ops::Not;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow_array::{Array, RecordBatch, StringArray, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::common::{Column, DFSchema, DFSchemaRef, ScalarValue, TableReference};
use datafusion::datasource::cte_worktable::CteWorkTable;
use datafusion::datasource::provider_as_source;
use datafusion::execution::context::SessionContext;
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::expr_fn::{count, count_distinct, min};
use datafusion::logical_expr::logical_plan::EmptyRelation;
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::execute_stream;
use datafusion::prelude::{col, lit};
use futures::StreamExt;
use thiserror::Error;
use tokio::sync::Notify;

use crate::relational_program::{FieldId, RelationId};
use crate::schema_contract::SchemaRole;

use super::epoch_runtime::{FABRIC_CATALOG, FabricSchemaRole};
use super::production_kernel::CompiledProofAuthority;
use super::programmatic_epoch::{
    ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder, ProgrammaticFabricEpochError,
};
use super::programmatic_schema::{
    ProgrammaticFieldId, ProgrammaticRelationId, ProgrammaticTransformation,
    ProgrammaticTransformationContract, ProgrammaticTransformationId,
    TransformationDeterminismPolicy, TransformationFieldIdentity, TransformationInputs,
    TransformationOrderingPolicy, TransformationOutput, TransformationPlanError,
    TransformationProvenance, TransformationProvenanceIdentity, TransformationRecursionPolicy,
    TransformationReleaseIdentity, TransformationResourceClass, TransformationSemanticVersion,
};

const ACCEPTED_FACT_FAMILY_RELATION_ID: &str = "runtime.accepted_fact_family";
const RUNTIME_PRODUCER_RELATION_ID: &str = "runtime.derived_producer";
const QUERY_FAMILY_REQUIREMENT_RELATION_ID: &str = "runtime.query_family_requirement";
const UNSUPPORTED_REMAINDER_RELATION_ID: &str = "runtime.unsupported_remainder";
const FAMILY_CLOSURE_RELATION_ID: &str = "derived.accepted_family_producer_closure";
const QUERY_REQUIREMENT_CLOSURE_RELATION_ID: &str = "derived.query_family_requirement_closure";
const PRODUCER_CLOSURE_VIOLATION_RELATION_ID: &str = "proof.derived_producer_closure_violation";
const PRODUCER_CLOSURE_OPERATION_ID: &str = "operation.derived-producer-closure.v2";
const PRODUCER_CLOSURE_IMPLEMENTATION_RELEASE: &str = "derived-producer-closure@1.0.0";
const APPLICATION_DERIVED_AUTHORITY_ID: &str = "authority.application-derived.v2";
const FACTUAL_SEMANTIC_CLASS_ID: &str = "semantic.fact.v2";

const ACCEPTED_FAMILY_FIELD_ID: &str = "accepted_family_id";
const ACCEPTED_SEMANTIC_CLASS_FIELD_ID: &str = "accepted_semantic_class_id";
const PRODUCER_FAMILY_FIELD_ID: &str = "producer_family_id";
const RUNTIME_PRODUCER_FIELD_ID: &str = "runtime_producer_id";
const RUNTIME_AUTHORITY_FIELD_ID: &str = "runtime_authority_id";
const ALGORITHM_RELEASE_FIELD_ID: &str = "algorithm_release_pin";
const PRECISION_PROFILE_FIELD_ID: &str = "precision_profile_id";
const PRODUCER_INPUT_FIELD_ID: &str = "producer_input_pin";
const INVALIDATION_POLICY_FIELD_ID: &str = "invalidation_policy_pin";
const MATERIALIZATION_POLICY_FIELD_ID: &str = "materialization_policy_pin";
const PRODUCER_REQUESTED_UNITS_FIELD_ID: &str = "producer_requested_unit_count";
const PRODUCER_COMPLETED_UNITS_FIELD_ID: &str = "producer_completed_unit_count";
const PRODUCER_REMAINDER_UNITS_FIELD_ID: &str = "producer_remainder_unit_count";
const PRODUCER_UNKNOWN_UNITS_FIELD_ID: &str = "producer_unknown_unit_count";
const PRODUCER_COMPLETENESS_PROOF_FIELD_ID: &str = "producer_completeness_proof_pin";
const PRODUCER_EXECUTION_PROOF_FIELD_ID: &str = "producer_execution_proof_pin";
const QUERY_FAMILY_FIELD_ID: &str = "query_family_id";
const QUERY_REQUIRED_FAMILY_FIELD_ID: &str = "query_required_family_id";
const REMAINDER_FAMILY_FIELD_ID: &str = "remainder_family_id";
const UNSUPPORTED_REMAINDER_FIELD_ID: &str = "unsupported_remainder_id";
const REMAINDER_AUTHORITY_FIELD_ID: &str = "remainder_authority_id";
const UNSUPPORTED_REASON_FIELD_ID: &str = "unsupported_reason_id";
const REMAINDER_PROOF_FIELD_ID: &str = "remainder_proof_pin";

const ACCEPTED_ALIAS: &str = "__codefabric_accepted_family";
const PRODUCER_ALIAS: &str = "__codefabric_runtime_producer";
const REMAINDER_ALIAS: &str = "__codefabric_unsupported_remainder";
const FAMILY_ALIAS: &str = "__codefabric_family_closure";
const QUERY_EDGE_ALIAS: &str = "__codefabric_query_edge";
const QUERY_FRONTIER_ALIAS: &str = "__codefabric_query_frontier";
const QUERY_REACH_ALIAS: &str = "__codefabric_query_reach";
const QUERY_SOURCE_ALIAS: &str = "__codefabric_query_source";
const QUERY_RECURSIVE_NAME: &str = "__codefabric_query_requirement_recursive";

const FAMILY: &str = "__cf_family";
const SEMANTIC_CLASS: &str = "__cf_semantic_class";
const SEMANTIC_CLASS_MIN: &str = "__cf_semantic_class_min";
const ACCEPTED_COUNT: &str = "__cf_accepted_count";
const SEMANTIC_CLASS_COUNT: &str = "__cf_semantic_class_count";
const PRODUCER_COUNT: &str = "__cf_producer_count";
const PRODUCER: &str = "__cf_producer";
const PRODUCER_AUTHORITY: &str = "__cf_producer_authority";
const ALGORITHM_RELEASE: &str = "__cf_algorithm_release";
const PRECISION: &str = "__cf_precision";
const INPUT_PIN: &str = "__cf_input_pin";
const INVALIDATION_PIN: &str = "__cf_invalidation_pin";
const MATERIALIZATION_PIN: &str = "__cf_materialization_pin";
const REQUESTED_UNITS: &str = "__cf_requested_units";
const COMPLETED_UNITS: &str = "__cf_completed_units";
const REMAINDER_UNITS: &str = "__cf_remainder_units";
const UNKNOWN_UNITS: &str = "__cf_unknown_units";
const COMPLETENESS_PROOF_PIN: &str = "__cf_completeness_proof_pin";
const PRODUCER_PROOF_PIN: &str = "__cf_producer_proof_pin";
const REMAINDER_COUNT: &str = "__cf_remainder_count";
const REMAINDER: &str = "__cf_remainder";
const REMAINDER_AUTHORITY: &str = "__cf_remainder_authority";
const REMAINDER_REASON: &str = "__cf_remainder_reason";
const REMAINDER_PROOF_PIN: &str = "__cf_remainder_proof_pin";
const CLOSURE_STATE: &str = "__cf_closure_state";
const QUERY_ROOT: &str = "__cf_query_root";
const QUERY_REQUIRED: &str = "__cf_query_required";
const QUERY_DEPTH: &str = "__cf_query_depth";
const QUERY_SOURCE_MARKER: &str = "__cf_query_source_marker";
const QUERY_STATE: &str = "__cf_query_state";
const QUERY_UNKNOWN_CAUSE: &str = "__cf_query_unknown_cause";

const STATE_SUPPORTED: &str = "supported";
const STATE_UNSUPPORTED: &str = "unsupported";
const STATE_UNKNOWN: &str = "unknown";
const STATE_INVALID: &str = "invalid";
const STATE_MISSING: &str = "missing";
const STATE_SATISFIED: &str = "satisfied";

/// One exact contract-bound relation supplied to the closure compiler.
#[derive(Clone, Debug)]
pub struct ProducerClosureRelationInput {
    relation_id: RelationId,
    plan: LogicalPlan,
}

impl ProducerClosureRelationInput {
    #[must_use]
    pub(crate) fn new(relation_id: RelationId, plan: LogicalPlan) -> Self {
        Self { relation_id, plan }
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn plan(&self) -> &LogicalPlan {
        &self.plan
    }
}

/// The four typed relation inputs required by producer closure.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureInputs {
    pub(crate) accepted_fact_family: ProducerClosureRelationInput,
    pub(crate) runtime_producer: ProducerClosureRelationInput,
    pub(crate) query_family_requirement: ProducerClosureRelationInput,
    pub(crate) unsupported_remainder: ProducerClosureRelationInput,
}

/// An application relation plus its exact Arrow contract and role-to-field bindings.
#[derive(Clone, Debug)]
pub struct ProducerClosureRelationContract<F> {
    relation_id: RelationId,
    schema: SchemaRef,
    fields: F,
}

impl<F> ProducerClosureRelationContract<F> {
    #[must_use]
    pub(crate) fn new(relation_id: RelationId, schema: SchemaRef, fields: F) -> Self {
        Self {
            relation_id,
            schema,
            fields,
        }
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn fields(&self) -> &F {
        &self.fields
    }
}

/// Field roles in the `accepted_fact_family` runtime relation.
#[derive(Clone, Debug)]
pub struct AcceptedFactFamilyFields {
    pub family_id: FieldId,
    pub semantic_class_id: FieldId,
}

/// Field roles in the `runtime_producer` runtime relation.
#[derive(Clone, Debug)]
pub struct RuntimeProducerFields {
    pub family_id: FieldId,
    pub producer_id: FieldId,
    pub authority_id: FieldId,
    pub algorithm_release: FieldId,
    pub precision_id: FieldId,
    pub input_pin: FieldId,
    pub invalidation_pin: FieldId,
    pub materialization_pin: FieldId,
    pub requested_unit_count: FieldId,
    pub completed_unit_count: FieldId,
    pub remainder_unit_count: FieldId,
    pub unknown_unit_count: FieldId,
    pub completeness_proof_pin: FieldId,
    pub proof_pin: FieldId,
}

/// Field roles in the `query_family_requirement` runtime relation.
#[derive(Clone, Debug)]
pub struct QueryFamilyRequirementFields {
    pub query_family_id: FieldId,
    pub required_family_id: FieldId,
}

/// Field roles in the `unsupported_remainder` runtime relation.
#[derive(Clone, Debug)]
pub struct UnsupportedRemainderFields {
    pub family_id: FieldId,
    pub remainder_id: FieldId,
    pub authority_id: FieldId,
    pub reason_id: FieldId,
    pub proof_pin: FieldId,
}

/// Field roles in the emitted accepted-family closure relation.
#[derive(Clone, Debug)]
pub struct FamilyClosureFields {
    pub family_id: FieldId,
    pub semantic_class_id: FieldId,
    pub closure_state: FieldId,
    pub producer_id: FieldId,
    pub authority_id: FieldId,
    pub algorithm_release: FieldId,
    pub precision_id: FieldId,
    pub input_pin: FieldId,
    pub invalidation_pin: FieldId,
    pub materialization_pin: FieldId,
    pub requested_unit_count: FieldId,
    pub completed_unit_count: FieldId,
    pub remainder_unit_count: FieldId,
    pub unknown_unit_count: FieldId,
    pub completeness_proof_pin: FieldId,
    pub producer_proof_pin: FieldId,
    pub unsupported_remainder_id: FieldId,
    pub unsupported_reason_id: FieldId,
    pub unsupported_proof_pin: FieldId,
}

/// Field roles in the emitted transitive query-requirement closure relation.
#[derive(Clone, Debug)]
pub struct QueryRequirementClosureFields {
    pub query_family_id: FieldId,
    pub required_family_id: FieldId,
    pub minimum_depth: FieldId,
    pub requirement_state: FieldId,
    pub unknown_cause: FieldId,
}

/// Field roles in the emitted conformance-violation relation.
#[derive(Clone, Debug)]
pub struct ProducerClosureViolationFields {
    pub subject_kind: FieldId,
    pub subject_id: FieldId,
    pub violation_code: FieldId,
    pub related_id: FieldId,
}

/// Application-contract identities whose values execution must read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerClosureSemanticIdentities {
    application_owned_authority_id: Arc<str>,
    factual_semantic_class_id: Arc<str>,
}

impl ProducerClosureSemanticIdentities {
    /// Construct the semantic identities installed in the active fabric epoch.
    ///
    /// # Errors
    ///
    /// Rejects empty or unreasonably large identities.
    pub(crate) fn try_new(
        application_owned_authority_id: impl Into<Arc<str>>,
        factual_semantic_class_id: impl Into<Arc<str>>,
    ) -> Result<Self, DerivedProducerClosureError> {
        let identities = Self {
            application_owned_authority_id: application_owned_authority_id.into(),
            factual_semantic_class_id: factual_semantic_class_id.into(),
        };
        validate_text(
            "application-owned authority",
            &identities.application_owned_authority_id,
        )?;
        validate_text(
            "factual semantic class",
            &identities.factual_semantic_class_id,
        )?;
        Ok(identities)
    }

    #[must_use]
    pub const fn application_owned_authority_id(&self) -> &Arc<str> {
        &self.application_owned_authority_id
    }

    #[must_use]
    pub const fn factual_semantic_class_id(&self) -> &Arc<str> {
        &self.factual_semantic_class_id
    }
}

/// Complete application binding for input, output, and semantic identities.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureBindings {
    operation_id: Arc<str>,
    implementation_release: Arc<str>,
    semantic_identities: ProducerClosureSemanticIdentities,
    accepted_fact_family: ProducerClosureRelationContract<AcceptedFactFamilyFields>,
    runtime_producer: ProducerClosureRelationContract<RuntimeProducerFields>,
    query_family_requirement: ProducerClosureRelationContract<QueryFamilyRequirementFields>,
    unsupported_remainder: ProducerClosureRelationContract<UnsupportedRemainderFields>,
    family_closure: ProducerClosureRelationContract<FamilyClosureFields>,
    query_requirement_closure: ProducerClosureRelationContract<QueryRequirementClosureFields>,
    violation: ProducerClosureRelationContract<ProducerClosureViolationFields>,
}

impl DerivedProducerClosureBindings {
    /// Validate the complete application binding before any executable plan is built.
    ///
    /// # Errors
    ///
    /// Rejects duplicate relations/fields, invalid text identities, and any field type,
    /// nullability, order, or exact-schema mismatch.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        operation_id: impl Into<Arc<str>>,
        implementation_release: impl Into<Arc<str>>,
        semantic_identities: ProducerClosureSemanticIdentities,
        accepted_fact_family: ProducerClosureRelationContract<AcceptedFactFamilyFields>,
        runtime_producer: ProducerClosureRelationContract<RuntimeProducerFields>,
        query_family_requirement: ProducerClosureRelationContract<QueryFamilyRequirementFields>,
        unsupported_remainder: ProducerClosureRelationContract<UnsupportedRemainderFields>,
        family_closure: ProducerClosureRelationContract<FamilyClosureFields>,
        query_requirement_closure: ProducerClosureRelationContract<QueryRequirementClosureFields>,
        violation: ProducerClosureRelationContract<ProducerClosureViolationFields>,
    ) -> Result<Self, DerivedProducerClosureError> {
        let operation_id = operation_id.into();
        let implementation_release = implementation_release.into();
        validate_text("operation", &operation_id)?;
        validate_text("implementation release", &implementation_release)?;

        validate_relation_contracts(
            &accepted_fact_family,
            &runtime_producer,
            &query_family_requirement,
            &unsupported_remainder,
            &family_closure,
            &query_requirement_closure,
            &violation,
        )?;

        Ok(Self {
            operation_id,
            implementation_release,
            semantic_identities,
            accepted_fact_family,
            runtime_producer,
            query_family_requirement,
            unsupported_remainder,
            family_closure,
            query_requirement_closure,
            violation,
        })
    }

    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn implementation_release(&self) -> &Arc<str> {
        &self.implementation_release
    }

    #[must_use]
    pub const fn semantic_identities(&self) -> &ProducerClosureSemanticIdentities {
        &self.semantic_identities
    }
}

/// Request-local execution limits observed by compilation and enforced during streaming.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducerClosureResourceBounds {
    max_query_depth: NonZeroU16,
    max_rows_per_relation: NonZeroUsize,
    max_total_batches: NonZeroUsize,
    max_total_bytes: NonZeroUsize,
}

/// Request-owned cancellation observed while executing the release producer closure.
///
/// Cancellation is monotonic and cloneable so the daemon can retain the handle while the
/// DataFusion execution future owns another clone. Dropping an in-flight physical stream is the
/// cancellation boundary; no partially decoded closure or proof result is returned.
#[derive(Clone, Debug, Default)]
pub struct ProducerClosureCancellation {
    state: Arc<ProducerClosureCancellationState>,
}

#[derive(Debug, Default)]
struct ProducerClosureCancellationState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl ProducerClosureCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark this request cancelled and wake every in-flight closure stream.
    ///
    /// Returns `true` only for the transition from live to cancelled.
    pub fn cancel(&self) -> bool {
        let transitioned = !self.state.cancelled.swap(true, Ordering::AcqRel);
        if transitioned {
            self.state.notify.notify_waiters();
        }
        transitioned
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.state.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// One release-owned accepted-family row installed into the candidate catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseAcceptedFactFamilyRow {
    pub(crate) family_id: Arc<str>,
}

/// One release-owned executable producer row installed into the candidate catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseRuntimeProducerRow {
    pub(crate) family_id: Arc<str>,
    pub(crate) producer_id: Arc<str>,
    pub(crate) algorithm_release: Arc<str>,
    pub(crate) precision_id: Arc<str>,
    pub(crate) input_pin: Arc<str>,
    pub(crate) invalidation_pin: Arc<str>,
    pub(crate) materialization_pin: Arc<str>,
    pub(crate) requested_unit_count: u64,
    pub(crate) completed_unit_count: u64,
    pub(crate) remainder_unit_count: u64,
    pub(crate) unknown_unit_count: u64,
    pub(crate) completeness_proof_pin: Arc<str>,
    pub(crate) proof_pin: Arc<str>,
}

/// One exact compiled query-program dependency edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseQueryFamilyRequirementRow {
    pub(crate) query_family_id: Arc<str>,
    pub(crate) required_family_id: Arc<str>,
}

/// One explicit release-owned unsupported-family remainder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseUnsupportedRemainderRow {
    pub(crate) family_id: Arc<str>,
    pub(crate) remainder_id: Arc<str>,
    pub(crate) reason_id: Arc<str>,
    pub(crate) proof_pin: Arc<str>,
}

/// Closed typed rows from the released analysis and query-program compilers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseProducerClosureCatalog {
    accepted_families: Arc<[ReleaseAcceptedFactFamilyRow]>,
    runtime_producers: Arc<[ReleaseRuntimeProducerRow]>,
    query_requirements: Arc<[ReleaseQueryFamilyRequirementRow]>,
    unsupported_remainders: Arc<[ReleaseUnsupportedRemainderRow]>,
}

impl ReleaseProducerClosureCatalog {
    /// Validate exact-one producer-or-remainder closure before catalog registration.
    pub(crate) fn try_new(
        _authority: &CompiledProofAuthority,
        accepted_families: Vec<ReleaseAcceptedFactFamilyRow>,
        runtime_producers: Vec<ReleaseRuntimeProducerRow>,
        query_requirements: Vec<ReleaseQueryFamilyRequirementRow>,
        unsupported_remainders: Vec<ReleaseUnsupportedRemainderRow>,
    ) -> Result<Self, ReleaseProducerClosureCatalogError> {
        if accepted_families.is_empty() {
            return Err(ReleaseProducerClosureCatalogError::EmptyAcceptedFamilies);
        }
        if query_requirements.is_empty() {
            return Err(ReleaseProducerClosureCatalogError::EmptyQueryRequirements);
        }
        let accepted = unique_family_set(
            "accepted family",
            accepted_families.iter().map(|row| row.family_id.as_ref()),
        )?;
        let producers = unique_family_set(
            "runtime producer",
            runtime_producers.iter().map(|row| row.family_id.as_ref()),
        )?;
        let remainders = unique_family_set(
            "unsupported remainder",
            unsupported_remainders
                .iter()
                .map(|row| row.family_id.as_ref()),
        )?;
        if !producers.is_disjoint(&remainders)
            || producers
                .union(&remainders)
                .copied()
                .collect::<BTreeSet<_>>()
                != accepted
        {
            return Err(ReleaseProducerClosureCatalogError::DispositionClosure);
        }
        for producer in &runtime_producers {
            for (kind, value) in [
                ("producer family", producer.family_id.as_ref()),
                ("producer identity", producer.producer_id.as_ref()),
                ("algorithm release", producer.algorithm_release.as_ref()),
                ("precision", producer.precision_id.as_ref()),
                ("input pin", producer.input_pin.as_ref()),
                ("invalidation pin", producer.invalidation_pin.as_ref()),
                ("materialization pin", producer.materialization_pin.as_ref()),
                (
                    "completeness proof pin",
                    producer.completeness_proof_pin.as_ref(),
                ),
                ("producer proof pin", producer.proof_pin.as_ref()),
            ] {
                validate_catalog_text(kind, value)?;
            }
            if producer.requested_unit_count == 0
                || producer.requested_unit_count != producer.completed_unit_count
                || producer.remainder_unit_count != 0
                || producer.unknown_unit_count != 0
            {
                return Err(
                    ReleaseProducerClosureCatalogError::IncompleteRuntimeProducer(Arc::clone(
                        &producer.family_id,
                    )),
                );
            }
        }
        for remainder in &unsupported_remainders {
            for (kind, value) in [
                ("remainder family", remainder.family_id.as_ref()),
                ("remainder identity", remainder.remainder_id.as_ref()),
                ("remainder reason", remainder.reason_id.as_ref()),
                ("remainder proof pin", remainder.proof_pin.as_ref()),
            ] {
                validate_catalog_text(kind, value)?;
            }
        }
        let mut query_edges = BTreeSet::new();
        for requirement in &query_requirements {
            validate_catalog_text("query family", &requirement.query_family_id)?;
            validate_catalog_text("query required family", &requirement.required_family_id)?;
            if !accepted.contains(requirement.required_family_id.as_ref()) {
                return Err(ReleaseProducerClosureCatalogError::UnknownQueryFamily(
                    Arc::clone(&requirement.required_family_id),
                ));
            }
            if !query_edges.insert((
                requirement.query_family_id.as_ref(),
                requirement.required_family_id.as_ref(),
            )) {
                return Err(ReleaseProducerClosureCatalogError::DuplicateQueryEdge);
            }
        }
        Ok(Self {
            accepted_families: accepted_families.into(),
            runtime_producers: runtime_producers.into(),
            query_requirements: query_requirements.into(),
            unsupported_remainders: unsupported_remainders.into(),
        })
    }
}

fn unique_family_set<'a>(
    kind: &'static str,
    values: impl Iterator<Item = &'a str>,
) -> Result<BTreeSet<&'a str>, ReleaseProducerClosureCatalogError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_catalog_text(kind, value)?;
        if !unique.insert(value) {
            return Err(ReleaseProducerClosureCatalogError::DuplicateFamily {
                kind,
                value: value.to_owned(),
            });
        }
    }
    Ok(unique)
}

fn validate_catalog_text(
    kind: &'static str,
    value: &str,
) -> Result<(), ReleaseProducerClosureCatalogError> {
    if value.trim().is_empty() || value.len() > 240 {
        return Err(ReleaseProducerClosureCatalogError::InvalidText {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// Register the four release producer-closure inputs as native, self-observed transformations.
///
/// The rows come from the release's typed analysis/query compilers. They are not provider inputs,
/// static generated registries, or caller-selected schemas. Registration makes their relation,
/// field, schema, and provenance contracts participate in the candidate's normal fixed-point
/// catalog observation before the closure is compiled back from the sealed session.
pub(crate) fn install_release_producer_closure_catalog(
    _authority: &CompiledProofAuthority,
    mut builder: ProgrammaticFabricEpochBuilder,
    catalog: ReleaseProducerClosureCatalog,
) -> Result<ProgrammaticFabricEpochBuilder, ReleaseProducerClosureCatalogError> {
    let transformations: [Arc<dyn ProgrammaticTransformation>; 4] = [
        Arc::new(ReleaseClosureLiteralTransformation::try_new(
            "runtime.producer-closure.accepted-families.v1",
            ACCEPTED_FACT_FAMILY_RELATION_ID,
            "runtime_accepted_fact_family",
            Arc::new(Schema::new(vec![
                Field::new(ACCEPTED_FAMILY_FIELD_ID, DataType::Utf8, false),
                Field::new(ACCEPTED_SEMANTIC_CLASS_FIELD_ID, DataType::Utf8, false),
            ])),
            catalog
                .accepted_families
                .iter()
                .map(|row| {
                    vec![
                        ScalarValue::Utf8(Some(row.family_id.to_string())),
                        ScalarValue::Utf8(Some(FACTUAL_SEMANTIC_CLASS_ID.to_owned())),
                    ]
                })
                .collect(),
        )?),
        Arc::new(ReleaseClosureLiteralTransformation::try_new(
            "runtime.producer-closure.runtime-producers.v1",
            RUNTIME_PRODUCER_RELATION_ID,
            "runtime_derived_producer",
            Arc::new(Schema::new(vec![
                Field::new(PRODUCER_FAMILY_FIELD_ID, DataType::Utf8, false),
                Field::new(RUNTIME_PRODUCER_FIELD_ID, DataType::Utf8, false),
                Field::new(RUNTIME_AUTHORITY_FIELD_ID, DataType::Utf8, false),
                Field::new(ALGORITHM_RELEASE_FIELD_ID, DataType::Utf8, false),
                Field::new(PRECISION_PROFILE_FIELD_ID, DataType::Utf8, false),
                Field::new(PRODUCER_INPUT_FIELD_ID, DataType::Utf8, false),
                Field::new(INVALIDATION_POLICY_FIELD_ID, DataType::Utf8, false),
                Field::new(MATERIALIZATION_POLICY_FIELD_ID, DataType::Utf8, false),
                Field::new(PRODUCER_REQUESTED_UNITS_FIELD_ID, DataType::UInt64, false),
                Field::new(PRODUCER_COMPLETED_UNITS_FIELD_ID, DataType::UInt64, false),
                Field::new(PRODUCER_REMAINDER_UNITS_FIELD_ID, DataType::UInt64, false),
                Field::new(PRODUCER_UNKNOWN_UNITS_FIELD_ID, DataType::UInt64, false),
                Field::new(PRODUCER_COMPLETENESS_PROOF_FIELD_ID, DataType::Utf8, false),
                Field::new(PRODUCER_EXECUTION_PROOF_FIELD_ID, DataType::Utf8, false),
            ])),
            catalog
                .runtime_producers
                .iter()
                .map(|row| {
                    vec![
                        ScalarValue::Utf8(Some(row.family_id.to_string())),
                        ScalarValue::Utf8(Some(row.producer_id.to_string())),
                        ScalarValue::Utf8(Some(APPLICATION_DERIVED_AUTHORITY_ID.to_owned())),
                        ScalarValue::Utf8(Some(row.algorithm_release.to_string())),
                        ScalarValue::Utf8(Some(row.precision_id.to_string())),
                        ScalarValue::Utf8(Some(row.input_pin.to_string())),
                        ScalarValue::Utf8(Some(row.invalidation_pin.to_string())),
                        ScalarValue::Utf8(Some(row.materialization_pin.to_string())),
                        ScalarValue::UInt64(Some(row.requested_unit_count)),
                        ScalarValue::UInt64(Some(row.completed_unit_count)),
                        ScalarValue::UInt64(Some(row.remainder_unit_count)),
                        ScalarValue::UInt64(Some(row.unknown_unit_count)),
                        ScalarValue::Utf8(Some(row.completeness_proof_pin.to_string())),
                        ScalarValue::Utf8(Some(row.proof_pin.to_string())),
                    ]
                })
                .collect(),
        )?),
        Arc::new(ReleaseClosureLiteralTransformation::try_new(
            "runtime.producer-closure.query-requirements.v1",
            QUERY_FAMILY_REQUIREMENT_RELATION_ID,
            "runtime_query_family_requirement",
            Arc::new(Schema::new(vec![
                Field::new(QUERY_FAMILY_FIELD_ID, DataType::Utf8, false),
                Field::new(QUERY_REQUIRED_FAMILY_FIELD_ID, DataType::Utf8, false),
            ])),
            catalog
                .query_requirements
                .iter()
                .map(|row| {
                    vec![
                        ScalarValue::Utf8(Some(row.query_family_id.to_string())),
                        ScalarValue::Utf8(Some(row.required_family_id.to_string())),
                    ]
                })
                .collect(),
        )?),
        Arc::new(ReleaseClosureLiteralTransformation::try_new(
            "runtime.producer-closure.unsupported-remainders.v1",
            UNSUPPORTED_REMAINDER_RELATION_ID,
            "runtime_unsupported_remainder",
            Arc::new(Schema::new(vec![
                Field::new(REMAINDER_FAMILY_FIELD_ID, DataType::Utf8, false),
                Field::new(UNSUPPORTED_REMAINDER_FIELD_ID, DataType::Utf8, false),
                Field::new(REMAINDER_AUTHORITY_FIELD_ID, DataType::Utf8, false),
                Field::new(UNSUPPORTED_REASON_FIELD_ID, DataType::Utf8, false),
                Field::new(REMAINDER_PROOF_FIELD_ID, DataType::Utf8, false),
            ])),
            catalog
                .unsupported_remainders
                .iter()
                .map(|row| {
                    vec![
                        ScalarValue::Utf8(Some(row.family_id.to_string())),
                        ScalarValue::Utf8(Some(row.remainder_id.to_string())),
                        ScalarValue::Utf8(Some(APPLICATION_DERIVED_AUTHORITY_ID.to_owned())),
                        ScalarValue::Utf8(Some(row.reason_id.to_string())),
                        ScalarValue::Utf8(Some(row.proof_pin.to_string())),
                    ]
                })
                .collect(),
        )?),
    ];
    for transformation in transformations {
        builder.add_transformation(transformation)?;
    }
    Ok(builder)
}

#[derive(Clone, Debug)]
struct ReleaseClosureLiteralTransformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    schema: DFSchemaRef,
    rows: Arc<[Arc<[ScalarValue]>]>,
}

impl ReleaseClosureLiteralTransformation {
    fn try_new(
        semantic_id: &'static str,
        relation_id: &'static str,
        table_name: &'static str,
        arrow_schema: SchemaRef,
        rows: Vec<Vec<ScalarValue>>,
    ) -> Result<Self, ReleaseProducerClosureCatalogError> {
        let schema = Arc::new(DFSchema::try_from(arrow_schema.as_ref().clone())?);
        let provenance = literal_relation_identity(semantic_id, relation_id, &rows);
        let release = literal_relation_identity(
            "codefabric.release-producer-closure-catalog.v1",
            relation_id,
            &[],
        );
        let output = TransformationOutput::new(
            ProgrammaticRelationId::new(relation_id),
            TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::System.as_str(),
                table_name,
            ),
            arrow_schema
                .fields()
                .iter()
                .map(|field| {
                    TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                        field.name().as_str(),
                    ))
                })
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            contract: ProgrammaticTransformationContract::new(
                ProgrammaticTransformationId::new(semantic_id),
                TransformationSemanticVersion::new(1, 0, 0),
                TransformationResourceClass::BoundedInMemory {
                    max_rows: 16_384,
                    max_memory_bytes: 16 * 1024 * 1024,
                },
                TransformationDeterminismPolicy::DeterministicSet,
                TransformationOrderingPolicy::Unordered,
                TransformationRecursionPolicy::Forbidden,
                TransformationProvenance::new(
                    TransformationProvenanceIdentity::from_bytes(provenance),
                    TransformationReleaseIdentity::from_bytes(release),
                ),
            ),
            output,
            schema,
            rows: rows
                .into_iter()
                .map(|row| Arc::<[ScalarValue]>::from(row))
                .collect::<Vec<_>>()
                .into(),
        })
    }
}

impl ProgrammaticTransformation for ReleaseClosureLiteralTransformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &[]
    }

    fn build(
        &self,
        _inputs: &TransformationInputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        if self.rows.is_empty() {
            return Ok(LogicalPlan::EmptyRelation(EmptyRelation {
                produce_one_row: false,
                schema: Arc::clone(&self.schema),
            }));
        }
        Ok(LogicalPlanBuilder::values_with_schema(
            self.rows
                .iter()
                .map(|row| row.iter().cloned().map(lit).collect())
                .collect(),
            &self.schema,
        )?
        .project(
            self.schema
                .fields()
                .iter()
                .enumerate()
                .map(|(ordinal, field)| col(format!("column{}", ordinal + 1)).alias(field.name()))
                .collect::<Vec<_>>(),
        )?
        .build()?)
    }
}

fn literal_relation_identity(
    domain: &str,
    relation_id: &str,
    rows: &[Vec<ScalarValue>],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for value in [domain, relation_id] {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    hasher.update(&(rows.len() as u64).to_be_bytes());
    for row in rows {
        hasher.update(&(row.len() as u64).to_be_bytes());
        for value in row {
            match value {
                ScalarValue::Utf8(Some(value)) => {
                    hasher.update(&[1]);
                    hasher.update(&(value.len() as u64).to_be_bytes());
                    hasher.update(value.as_bytes());
                }
                ScalarValue::UInt64(Some(value)) => {
                    hasher.update(&[2]);
                    hasher.update(&value.to_be_bytes());
                }
                _ => {
                    hasher.update(&[0]);
                }
            }
        }
    }
    *hasher.finalize().as_bytes()
}

impl ProducerClosureResourceBounds {
    /// Construct a non-zero closure resource envelope.
    ///
    /// # Errors
    ///
    /// Rejects zero limits and a row limit that cannot reserve one overflow-probe row.
    pub fn try_new(
        max_query_depth: u16,
        max_rows_per_relation: usize,
        max_total_batches: usize,
        max_total_bytes: usize,
    ) -> Result<Self, DerivedProducerClosureError> {
        let bounds = Self {
            max_query_depth: NonZeroU16::new(max_query_depth).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_query_depth"),
            )?,
            max_rows_per_relation: NonZeroUsize::new(max_rows_per_relation).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_rows_per_relation"),
            )?,
            max_total_batches: NonZeroUsize::new(max_total_batches).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_total_batches"),
            )?,
            max_total_bytes: NonZeroUsize::new(max_total_bytes).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_total_bytes"),
            )?,
        };
        bounds.probe_rows()?;
        Ok(bounds)
    }

    #[must_use]
    pub const fn max_query_depth(self) -> u16 {
        self.max_query_depth.get()
    }

    #[must_use]
    pub const fn max_rows_per_relation(self) -> usize {
        self.max_rows_per_relation.get()
    }

    #[must_use]
    pub const fn max_total_batches(self) -> usize {
        self.max_total_batches.get()
    }

    #[must_use]
    pub const fn max_total_bytes(self) -> usize {
        self.max_total_bytes.get()
    }

    fn probe_rows(self) -> Result<usize, DerivedProducerClosureError> {
        self.max_rows_per_relation
            .get()
            .checked_add(1)
            .ok_or(DerivedProducerClosureError::ResourceProbeOverflow)
    }
}

/// Highest viable extension rung used by this compiler.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureExecutionRung {
    NativeLogicalPlans,
}

/// Native operators causally selected by successful closure compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureNativeOperator {
    Projection,
    Aggregate,
    LeftJoin,
    LeftAntiJoin,
    Filter,
    RecursiveQueryDistinct,
    UnionAll,
    DeterministicSort,
    OutputOverflowProbeLimit,
}

/// Exact application/runtime dependency observed by the compiler that constructed the plans.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureCompilationDependency {
    InputRelation(RelationId),
    InputField(FieldId),
    OutputRelation(RelationId),
    OutputField(FieldId),
    ApplicationOwnedAuthority(Arc<str>),
    FactualSemanticClass(Arc<str>),
    ImplementationRelease(Arc<str>),
    SessionMemoryPool,
    DataFusionExecuteStreamDropAbort,
}

/// Causal evidence for native operator, dependency, and resource selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerClosureCompilationObservation {
    operation_id: Arc<str>,
    rung: ProducerClosureExecutionRung,
    operators: BTreeSet<ProducerClosureNativeOperator>,
    dependencies: BTreeSet<ProducerClosureCompilationDependency>,
    bounds: ProducerClosureResourceBounds,
}

/// One exact producer-family row decoded from the executed Arrow closure relation.
///
/// The fields remain private so callers cannot manufacture semantic closure. The release proof
/// path can only obtain rows through [`DerivedProducerClosureExecution::release_evidence`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseProducerFamilyClosureRow {
    family_id: Arc<str>,
    semantic_class_id: Arc<str>,
    closure_state: Arc<str>,
    producer_id: Option<Arc<str>>,
    authority_id: Option<Arc<str>>,
    algorithm_release: Option<Arc<str>>,
    precision_id: Option<Arc<str>>,
    input_pin: Option<Arc<str>>,
    invalidation_pin: Option<Arc<str>>,
    materialization_pin: Option<Arc<str>>,
    requested_unit_count: Option<u64>,
    completed_unit_count: Option<u64>,
    remainder_unit_count: Option<u64>,
    unknown_unit_count: Option<u64>,
    completeness_proof_pin: Option<Arc<str>>,
    producer_proof_pin: Option<Arc<str>>,
    unsupported_remainder_id: Option<Arc<str>>,
    unsupported_reason_id: Option<Arc<str>>,
    unsupported_proof_pin: Option<Arc<str>>,
}

impl ReleaseProducerFamilyClosureRow {
    #[must_use]
    pub(crate) const fn family_id(&self) -> &Arc<str> {
        &self.family_id
    }

    #[must_use]
    pub(crate) const fn semantic_class_id(&self) -> &Arc<str> {
        &self.semantic_class_id
    }

    #[must_use]
    pub(crate) const fn closure_state(&self) -> &Arc<str> {
        &self.closure_state
    }

    #[must_use]
    pub(crate) const fn authority_id(&self) -> Option<&Arc<str>> {
        self.authority_id.as_ref()
    }

    #[must_use]
    pub(crate) const fn producer_proof_pin(&self) -> Option<&Arc<str>> {
        self.producer_proof_pin.as_ref()
    }

    #[must_use]
    pub(crate) const fn completeness_proof_pin(&self) -> Option<&Arc<str>> {
        self.completeness_proof_pin.as_ref()
    }
}

/// One exact transitive query-requirement row decoded from Arrow execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseQueryRequirementClosureRow {
    query_family_id: Arc<str>,
    required_family_id: Arc<str>,
    minimum_depth: u32,
    requirement_state: Arc<str>,
    unknown_cause: Option<Arc<str>>,
}

impl ReleaseQueryRequirementClosureRow {
    #[must_use]
    pub(crate) const fn query_family_id(&self) -> &Arc<str> {
        &self.query_family_id
    }

    #[must_use]
    pub(crate) const fn required_family_id(&self) -> &Arc<str> {
        &self.required_family_id
    }

    #[must_use]
    pub(crate) const fn requirement_state(&self) -> &Arc<str> {
        &self.requirement_state
    }
}

/// One exact violation row decoded from the executed proof relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseProducerClosureViolationRow {
    subject_kind: Arc<str>,
    subject_id: Arc<str>,
    violation_code: Arc<str>,
    related_id: Option<Arc<str>>,
}

impl ReleaseProducerClosureViolationRow {
    #[must_use]
    pub(crate) const fn subject_id(&self) -> &Arc<str> {
        &self.subject_id
    }

    #[must_use]
    pub(crate) const fn violation_code(&self) -> &Arc<str> {
        &self.violation_code
    }
}

/// Stable structural issue derived from decoded release rows and compiled dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseProducerClosureIssue {
    code: &'static str,
    subject_id: Option<Arc<str>>,
    related_id: Option<Arc<str>>,
}

impl ReleaseProducerClosureIssue {
    fn new(code: &'static str, subject_id: Option<Arc<str>>, related_id: Option<Arc<str>>) -> Self {
        Self {
            code,
            subject_id,
            related_id,
        }
    }

    #[must_use]
    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub(crate) const fn subject_id(&self) -> Option<&Arc<str>> {
        self.subject_id.as_ref()
    }
}

/// Release-owned, row-decoded producer closure consumed by executable proof.
///
/// This is derived from the exact DataFusion results and their compiled dependency observation.
/// It is not constructible from counts, digests, plan text, or caller-authored declarations.
#[derive(Clone, Debug)]
pub(crate) struct ReleaseProducerClosureEvidence {
    operation_id: Arc<str>,
    implementation_release: Arc<str>,
    application_authority_id: Arc<str>,
    factual_semantic_class_id: Arc<str>,
    families: Arc<[ReleaseProducerFamilyClosureRow]>,
    query_requirements: Arc<[ReleaseQueryRequirementClosureRow]>,
    violations: Arc<[ReleaseProducerClosureViolationRow]>,
    issues: Arc<[ReleaseProducerClosureIssue]>,
    dependencies: Arc<[ProducerClosureCompilationDependency]>,
}

impl ReleaseProducerClosureEvidence {
    #[must_use]
    pub(crate) const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub(crate) const fn implementation_release(&self) -> &Arc<str> {
        &self.implementation_release
    }

    #[must_use]
    pub(crate) const fn application_authority_id(&self) -> &Arc<str> {
        &self.application_authority_id
    }

    #[must_use]
    pub(crate) const fn factual_semantic_class_id(&self) -> &Arc<str> {
        &self.factual_semantic_class_id
    }

    #[must_use]
    pub(crate) fn families(&self) -> &[ReleaseProducerFamilyClosureRow] {
        &self.families
    }

    #[must_use]
    pub(crate) fn query_requirements(&self) -> &[ReleaseQueryRequirementClosureRow] {
        &self.query_requirements
    }

    #[must_use]
    pub(crate) fn violations(&self) -> &[ReleaseProducerClosureViolationRow] {
        &self.violations
    }

    #[must_use]
    pub(crate) fn issues(&self) -> &[ReleaseProducerClosureIssue] {
        &self.issues
    }

    #[must_use]
    pub(crate) fn dependencies(&self) -> &[ProducerClosureCompilationDependency] {
        &self.dependencies
    }

    #[must_use]
    pub(crate) fn is_conformant(&self) -> bool {
        self.issues.is_empty()
    }
}

impl ProducerClosureCompilationObservation {
    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn rung(&self) -> ProducerClosureExecutionRung {
        self.rung
    }

    #[must_use]
    pub const fn operators(&self) -> &BTreeSet<ProducerClosureNativeOperator> {
        &self.operators
    }

    #[must_use]
    pub const fn dependencies(&self) -> &BTreeSet<ProducerClosureCompilationDependency> {
        &self.dependencies
    }

    #[must_use]
    pub const fn bounds(&self) -> ProducerClosureResourceBounds {
        self.bounds
    }
}

/// Three optimizer-visible native plans and their exact output contracts.
#[derive(Clone, Debug)]
pub struct CompiledDerivedProducerClosure {
    family_closure_plan: LogicalPlan,
    query_requirement_closure_plan: LogicalPlan,
    violation_plan: LogicalPlan,
    family_closure_schema: SchemaRef,
    query_requirement_closure_schema: SchemaRef,
    violation_schema: SchemaRef,
    input_fields: Arc<[FieldId]>,
    family_closure_fields: FamilyClosureFields,
    query_requirement_closure_fields: QueryRequirementClosureFields,
    violation_fields: ProducerClosureViolationFields,
    semantic_identities: ProducerClosureSemanticIdentities,
    implementation_release: Arc<str>,
    observation: ProducerClosureCompilationObservation,
}

impl CompiledDerivedProducerClosure {
    #[must_use]
    pub const fn family_closure_plan(&self) -> &LogicalPlan {
        &self.family_closure_plan
    }

    #[must_use]
    pub const fn query_requirement_closure_plan(&self) -> &LogicalPlan {
        &self.query_requirement_closure_plan
    }

    #[must_use]
    pub const fn violation_plan(&self) -> &LogicalPlan {
        &self.violation_plan
    }

    #[must_use]
    pub const fn observation(&self) -> &ProducerClosureCompilationObservation {
        &self.observation
    }

    /// Execute all closure plans under one DataFusion session and one shared output budget.
    ///
    /// Empty results retain one zero-row batch with the declared Arrow schema. A non-empty
    /// violation relation is returned as typed evidence and makes `is_conformant` false; it is
    /// never collapsed into a persisted Boolean declaration.
    ///
    /// # Errors
    ///
    /// Returns a typed optimizer, planner, execution, schema, or resource-limit failure.
    pub async fn execute(
        &self,
        context: &SessionContext,
    ) -> Result<DerivedProducerClosureExecution, DerivedProducerClosureError> {
        self.execute_with_cancellation(context, &ProducerClosureCancellation::new())
            .await
    }

    /// Execute under an explicit request cancellation authority.
    ///
    /// A cancellation drops the current DataFusion stream and returns no decoded relation or
    /// proof-capable value. Resource failures have the same atomic behavior.
    ///
    /// # Errors
    ///
    /// Returns a typed cancellation, optimizer, planner, execution, schema, or resource failure.
    pub async fn execute_with_cancellation(
        &self,
        context: &SessionContext,
        cancellation: &ProducerClosureCancellation,
    ) -> Result<DerivedProducerClosureExecution, DerivedProducerClosureError> {
        let mut budget = ExecutionBudget::default();
        let family_closure = execute_bounded(
            context,
            &self.family_closure_plan,
            &self.family_closure_schema,
            self.observation.bounds,
            "family_closure",
            &mut budget,
            cancellation,
        )
        .await?;
        let query_requirement_closure = execute_bounded(
            context,
            &self.query_requirement_closure_plan,
            &self.query_requirement_closure_schema,
            self.observation.bounds,
            "query_requirement_closure",
            &mut budget,
            cancellation,
        )
        .await?;
        let violations = execute_bounded(
            context,
            &self.violation_plan,
            &self.violation_schema,
            self.observation.bounds,
            "violations",
            &mut budget,
            cancellation,
        )
        .await?;

        let release_evidence = decode_release_producer_closure_evidence(
            &family_closure,
            &self.family_closure_fields,
            &query_requirement_closure,
            &self.query_requirement_closure_fields,
            &violations,
            &self.violation_fields,
            &self.input_fields,
            &self.semantic_identities,
            &self.implementation_release,
            &self.observation,
        )?;

        Ok(DerivedProducerClosureExecution {
            family_closure_schema: Arc::clone(&self.family_closure_schema),
            family_closure,
            query_requirement_closure_schema: Arc::clone(&self.query_requirement_closure_schema),
            query_requirement_closure,
            violation_schema: Arc::clone(&self.violation_schema),
            violations,
            family_closure_fields: self.family_closure_fields.clone(),
            observation: self.observation.clone(),
            release_evidence,
        })
    }
}

/// Exact-schema closure output emitted after bounded DataFusion execution.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureExecution {
    family_closure_schema: SchemaRef,
    family_closure: Vec<RecordBatch>,
    query_requirement_closure_schema: SchemaRef,
    query_requirement_closure: Vec<RecordBatch>,
    violation_schema: SchemaRef,
    violations: Vec<RecordBatch>,
    family_closure_fields: FamilyClosureFields,
    observation: ProducerClosureCompilationObservation,
    release_evidence: ReleaseProducerClosureEvidence,
}

impl DerivedProducerClosureExecution {
    #[must_use]
    pub const fn family_closure_schema(&self) -> &SchemaRef {
        &self.family_closure_schema
    }

    #[must_use]
    pub fn family_closure(&self) -> &[RecordBatch] {
        &self.family_closure
    }

    #[must_use]
    pub const fn query_requirement_closure_schema(&self) -> &SchemaRef {
        &self.query_requirement_closure_schema
    }

    #[must_use]
    pub fn query_requirement_closure(&self) -> &[RecordBatch] {
        &self.query_requirement_closure
    }

    #[must_use]
    pub const fn violation_schema(&self) -> &SchemaRef {
        &self.violation_schema
    }

    #[must_use]
    pub fn violations(&self) -> &[RecordBatch] {
        &self.violations
    }

    #[must_use]
    pub(crate) const fn family_closure_fields(&self) -> &FamilyClosureFields {
        &self.family_closure_fields
    }

    #[must_use]
    pub const fn observation(&self) -> &ProducerClosureCompilationObservation {
        &self.observation
    }

    /// Borrow the exact decoded rows and their release-owned conformance result.
    #[must_use]
    pub(crate) const fn release_evidence(&self) -> &ReleaseProducerClosureEvidence {
        &self.release_evidence
    }

    #[must_use]
    pub fn is_conformant(&self) -> bool {
        self.release_evidence.is_conformant()
    }
}

/// Compile the release-owned producer closure from the four exact relations sealed in one epoch.
///
/// Relation identities, field roles, schemas, output contracts, semantic identities, and the
/// implementation release are compiled here. The caller can vary only the sealed epoch and
/// bounded execution resources; it cannot substitute a plan, schema, field map, or producer
/// binding.
///
/// # Errors
///
/// Rejects a missing/renamed relation, relation-metadata drift, field identity/type/nullability
/// drift, provider resolution failure, or native logical-plan construction failure.
pub(crate) async fn compile_release_owned_derived_producer_closure(
    compiled_release: &CompiledProofAuthority,
    epoch: &ProgrammaticFabricEpoch,
    bounds: ProducerClosureResourceBounds,
) -> Result<CompiledDerivedProducerClosure, DerivedProducerClosureError> {
    let accepted_fields = AcceptedFactFamilyFields {
        family_id: compiled_field_id("accepted_family_id")?,
        semantic_class_id: compiled_field_id("accepted_semantic_class_id")?,
    };
    let producer_fields = RuntimeProducerFields {
        family_id: compiled_field_id("producer_family_id")?,
        producer_id: compiled_field_id("runtime_producer_id")?,
        authority_id: compiled_field_id("runtime_authority_id")?,
        algorithm_release: compiled_field_id("algorithm_release_pin")?,
        precision_id: compiled_field_id("precision_profile_id")?,
        input_pin: compiled_field_id("producer_input_pin")?,
        invalidation_pin: compiled_field_id("invalidation_policy_pin")?,
        materialization_pin: compiled_field_id("materialization_policy_pin")?,
        requested_unit_count: compiled_field_id("producer_requested_unit_count")?,
        completed_unit_count: compiled_field_id("producer_completed_unit_count")?,
        remainder_unit_count: compiled_field_id("producer_remainder_unit_count")?,
        unknown_unit_count: compiled_field_id("producer_unknown_unit_count")?,
        completeness_proof_pin: compiled_field_id("producer_completeness_proof_pin")?,
        proof_pin: compiled_field_id("producer_execution_proof_pin")?,
    };
    let query_fields = QueryFamilyRequirementFields {
        query_family_id: compiled_field_id("query_family_id")?,
        required_family_id: compiled_field_id("query_required_family_id")?,
    };
    let remainder_fields = UnsupportedRemainderFields {
        family_id: compiled_field_id("remainder_family_id")?,
        remainder_id: compiled_field_id("unsupported_remainder_id")?,
        authority_id: compiled_field_id("remainder_authority_id")?,
        reason_id: compiled_field_id("unsupported_reason_id")?,
        proof_pin: compiled_field_id("remainder_proof_pin")?,
    };

    let (accepted_input, accepted_schema) = resolve_release_input(
        epoch,
        "accepted_fact_family",
        ACCEPTED_FACT_FAMILY_RELATION_ID,
        &[
            (&accepted_fields.family_id, DataType::Utf8, false),
            (&accepted_fields.semantic_class_id, DataType::Utf8, false),
        ],
    )
    .await?;
    let (producer_input, producer_schema) = resolve_release_input(
        epoch,
        "runtime_producer",
        RUNTIME_PRODUCER_RELATION_ID,
        &[
            (&producer_fields.family_id, DataType::Utf8, false),
            (&producer_fields.producer_id, DataType::Utf8, false),
            (&producer_fields.authority_id, DataType::Utf8, false),
            (&producer_fields.algorithm_release, DataType::Utf8, false),
            (&producer_fields.precision_id, DataType::Utf8, false),
            (&producer_fields.input_pin, DataType::Utf8, false),
            (&producer_fields.invalidation_pin, DataType::Utf8, false),
            (&producer_fields.materialization_pin, DataType::Utf8, false),
            (
                &producer_fields.requested_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer_fields.completed_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer_fields.remainder_unit_count,
                DataType::UInt64,
                false,
            ),
            (&producer_fields.unknown_unit_count, DataType::UInt64, false),
            (
                &producer_fields.completeness_proof_pin,
                DataType::Utf8,
                false,
            ),
            (&producer_fields.proof_pin, DataType::Utf8, false),
        ],
    )
    .await?;
    let (query_input, query_schema) = resolve_release_input(
        epoch,
        "query_family_requirement",
        QUERY_FAMILY_REQUIREMENT_RELATION_ID,
        &[
            (&query_fields.query_family_id, DataType::Utf8, false),
            (&query_fields.required_family_id, DataType::Utf8, false),
        ],
    )
    .await?;
    let (remainder_input, remainder_schema) = resolve_release_input(
        epoch,
        "unsupported_remainder",
        UNSUPPORTED_REMAINDER_RELATION_ID,
        &[
            (&remainder_fields.family_id, DataType::Utf8, false),
            (&remainder_fields.remainder_id, DataType::Utf8, false),
            (&remainder_fields.authority_id, DataType::Utf8, false),
            (&remainder_fields.reason_id, DataType::Utf8, false),
            (&remainder_fields.proof_pin, DataType::Utf8, false),
        ],
    )
    .await?;

    let bindings = release_owned_bindings(
        accepted_schema,
        accepted_fields,
        producer_schema,
        producer_fields,
        query_schema,
        query_fields,
        remainder_schema,
        remainder_fields,
    )?;
    let inputs = DerivedProducerClosureInputs {
        accepted_fact_family: accepted_input,
        runtime_producer: producer_input,
        query_family_requirement: query_input,
        unsupported_remainder: remainder_input,
    };
    compile_derived_producer_closure(compiled_release, inputs, &bindings, bounds)
}

async fn resolve_release_input(
    epoch: &ProgrammaticFabricEpoch,
    role: &'static str,
    relation_identity: &'static str,
    expected_fields: &[(&FieldId, DataType, bool)],
) -> Result<(ProducerClosureRelationInput, SchemaRef), DerivedProducerClosureError> {
    let epoch_relation_id = ProgrammaticRelationId::new(relation_identity);
    if epoch.relation(&epoch_relation_id).is_none() {
        return Err(DerivedProducerClosureError::MissingReleaseInputRelation {
            relation: relation_identity,
        });
    }
    let (table_reference, provider, contract, _) = epoch
        .resolve_sealed_relation(&epoch_relation_id)
        .await
        .map_err(
            |error| DerivedProducerClosureError::ReleaseInputResolution {
                relation: relation_identity,
                detail: error.to_string(),
            },
        )?;
    let observed_relation = contract.relation_id(SchemaRole::Logical).map_err(|error| {
        DerivedProducerClosureError::ReleaseInputResolution {
            relation: relation_identity,
            detail: error.to_string(),
        }
    })?;
    if observed_relation != relation_identity {
        return Err(DerivedProducerClosureError::ReleaseInputRelationIdentity {
            expected: relation_identity,
            actual: observed_relation.to_owned(),
        });
    }
    let schema = Arc::clone(contract.logical_schema());
    validate_exact_fields(role, &schema, expected_fields)?;
    for (ordinal, (expected, _, _)) in expected_fields.iter().enumerate() {
        let observed = contract
            .field_id_at(SchemaRole::Logical, ordinal)
            .map_err(
                |error| DerivedProducerClosureError::ReleaseInputResolution {
                    relation: relation_identity,
                    detail: error.to_string(),
                },
            )?;
        if observed != expected.as_str() {
            return Err(DerivedProducerClosureError::ReleaseInputFieldIdentity {
                relation: relation_identity,
                ordinal,
                expected: expected.as_str().to_owned(),
                actual: observed.to_owned(),
            });
        }
    }
    let relation_id = compiled_relation_id(relation_identity)?;
    let plan =
        LogicalPlanBuilder::scan(table_reference, provider_as_source(provider), None)?.build()?;
    if plan.schema().as_arrow() != schema.as_ref() {
        return Err(DerivedProducerClosureError::InputSchemaMismatch {
            role,
            expected: Arc::clone(&schema),
            actual: Arc::new(plan.schema().as_arrow().clone()),
        });
    }
    Ok((ProducerClosureRelationInput::new(relation_id, plan), schema))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn release_owned_bindings(
    accepted_schema: SchemaRef,
    accepted_fields: AcceptedFactFamilyFields,
    producer_schema: SchemaRef,
    producer_fields: RuntimeProducerFields,
    query_schema: SchemaRef,
    query_fields: QueryFamilyRequirementFields,
    remainder_schema: SchemaRef,
    remainder_fields: UnsupportedRemainderFields,
) -> Result<DerivedProducerClosureBindings, DerivedProducerClosureError> {
    let family_output_fields = FamilyClosureFields {
        family_id: compiled_field_id("closed_family_id")?,
        semantic_class_id: compiled_field_id("closed_semantic_class_id")?,
        closure_state: compiled_field_id("family_closure_state")?,
        producer_id: compiled_field_id("closed_producer_id")?,
        authority_id: compiled_field_id("closed_authority_id")?,
        algorithm_release: compiled_field_id("closed_algorithm_release")?,
        precision_id: compiled_field_id("closed_precision_id")?,
        input_pin: compiled_field_id("closed_input_pin")?,
        invalidation_pin: compiled_field_id("closed_invalidation_pin")?,
        materialization_pin: compiled_field_id("closed_materialization_pin")?,
        requested_unit_count: compiled_field_id("closed_requested_unit_count")?,
        completed_unit_count: compiled_field_id("closed_completed_unit_count")?,
        remainder_unit_count: compiled_field_id("closed_remainder_unit_count")?,
        unknown_unit_count: compiled_field_id("closed_unknown_unit_count")?,
        completeness_proof_pin: compiled_field_id("closed_completeness_proof_pin")?,
        producer_proof_pin: compiled_field_id("closed_producer_proof_pin")?,
        unsupported_remainder_id: compiled_field_id("closed_unsupported_remainder_id")?,
        unsupported_reason_id: compiled_field_id("closed_unsupported_reason_id")?,
        unsupported_proof_pin: compiled_field_id("closed_unsupported_proof_pin")?,
    };
    let query_output_fields = QueryRequirementClosureFields {
        query_family_id: compiled_field_id("closed_query_family_id")?,
        required_family_id: compiled_field_id("closed_query_required_family_id")?,
        minimum_depth: compiled_field_id("query_requirement_minimum_depth")?,
        requirement_state: compiled_field_id("query_requirement_state")?,
        unknown_cause: compiled_field_id("query_requirement_unknown_cause")?,
    };
    let violation_fields = ProducerClosureViolationFields {
        subject_kind: compiled_field_id("violation_subject_kind")?,
        subject_id: compiled_field_id("violation_subject_id")?,
        violation_code: compiled_field_id("producer_closure_violation_code")?,
        related_id: compiled_field_id("violation_related_id")?,
    };
    let field = |identity: &FieldId, data_type, nullable| {
        Field::new(identity.as_str(), data_type, nullable)
    };
    let family_output_schema = Arc::new(Schema::new(vec![
        field(&family_output_fields.family_id, DataType::Utf8, false),
        field(
            &family_output_fields.semantic_class_id,
            DataType::Utf8,
            false,
        ),
        field(&family_output_fields.closure_state, DataType::Utf8, false),
        field(&family_output_fields.producer_id, DataType::Utf8, true),
        field(&family_output_fields.authority_id, DataType::Utf8, true),
        field(
            &family_output_fields.algorithm_release,
            DataType::Utf8,
            true,
        ),
        field(&family_output_fields.precision_id, DataType::Utf8, true),
        field(&family_output_fields.input_pin, DataType::Utf8, true),
        field(&family_output_fields.invalidation_pin, DataType::Utf8, true),
        field(
            &family_output_fields.materialization_pin,
            DataType::Utf8,
            true,
        ),
        field(
            &family_output_fields.requested_unit_count,
            DataType::UInt64,
            true,
        ),
        field(
            &family_output_fields.completed_unit_count,
            DataType::UInt64,
            true,
        ),
        field(
            &family_output_fields.remainder_unit_count,
            DataType::UInt64,
            true,
        ),
        field(
            &family_output_fields.unknown_unit_count,
            DataType::UInt64,
            true,
        ),
        field(
            &family_output_fields.completeness_proof_pin,
            DataType::Utf8,
            true,
        ),
        field(
            &family_output_fields.producer_proof_pin,
            DataType::Utf8,
            true,
        ),
        field(
            &family_output_fields.unsupported_remainder_id,
            DataType::Utf8,
            true,
        ),
        field(
            &family_output_fields.unsupported_reason_id,
            DataType::Utf8,
            true,
        ),
        field(
            &family_output_fields.unsupported_proof_pin,
            DataType::Utf8,
            true,
        ),
    ]));
    let query_output_schema = Arc::new(Schema::new(vec![
        field(&query_output_fields.query_family_id, DataType::Utf8, false),
        field(
            &query_output_fields.required_family_id,
            DataType::Utf8,
            false,
        ),
        field(&query_output_fields.minimum_depth, DataType::UInt32, false),
        field(
            &query_output_fields.requirement_state,
            DataType::Utf8,
            false,
        ),
        field(&query_output_fields.unknown_cause, DataType::Utf8, true),
    ]));
    let violation_schema = Arc::new(Schema::new(vec![
        field(&violation_fields.subject_kind, DataType::Utf8, false),
        field(&violation_fields.subject_id, DataType::Utf8, false),
        field(&violation_fields.violation_code, DataType::Utf8, false),
        field(&violation_fields.related_id, DataType::Utf8, true),
    ]));

    DerivedProducerClosureBindings::try_new(
        PRODUCER_CLOSURE_OPERATION_ID,
        PRODUCER_CLOSURE_IMPLEMENTATION_RELEASE,
        ProducerClosureSemanticIdentities::try_new(
            APPLICATION_DERIVED_AUTHORITY_ID,
            FACTUAL_SEMANTIC_CLASS_ID,
        )?,
        ProducerClosureRelationContract::new(
            compiled_relation_id(ACCEPTED_FACT_FAMILY_RELATION_ID)?,
            accepted_schema,
            accepted_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(RUNTIME_PRODUCER_RELATION_ID)?,
            producer_schema,
            producer_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(QUERY_FAMILY_REQUIREMENT_RELATION_ID)?,
            query_schema,
            query_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(UNSUPPORTED_REMAINDER_RELATION_ID)?,
            remainder_schema,
            remainder_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(FAMILY_CLOSURE_RELATION_ID)?,
            family_output_schema,
            family_output_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(QUERY_REQUIREMENT_CLOSURE_RELATION_ID)?,
            query_output_schema,
            query_output_fields,
        ),
        ProducerClosureRelationContract::new(
            compiled_relation_id(PRODUCER_CLOSURE_VIOLATION_RELATION_ID)?,
            violation_schema,
            violation_fields,
        ),
    )
}

fn compiled_relation_id(value: &'static str) -> Result<RelationId, DerivedProducerClosureError> {
    RelationId::new(value).map_err(
        |error| DerivedProducerClosureError::InvalidCompiledIdentity {
            kind: "relation",
            value,
            detail: error.to_string(),
        },
    )
}

fn compiled_field_id(value: &'static str) -> Result<FieldId, DerivedProducerClosureError> {
    FieldId::new(value).map_err(
        |error| DerivedProducerClosureError::InvalidCompiledIdentity {
            kind: "field",
            value,
            detail: error.to_string(),
        },
    )
}

/// Compile producer, unsupported-remainder, and transitive query closure as native plans.
///
/// # Errors
///
/// Rejects relation/schema drift and any DataFusion logical-plan construction failure.
pub(crate) fn compile_derived_producer_closure(
    _compiled_release: &CompiledProofAuthority,
    inputs: DerivedProducerClosureInputs,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<CompiledDerivedProducerClosure, DerivedProducerClosureError> {
    validate_input(
        &inputs.accepted_fact_family,
        &bindings.accepted_fact_family,
        "accepted_fact_family",
    )?;
    validate_input(
        &inputs.runtime_producer,
        &bindings.runtime_producer,
        "runtime_producer",
    )?;
    validate_input(
        &inputs.query_family_requirement,
        &bindings.query_family_requirement,
        "query_family_requirement",
    )?;
    validate_input(
        &inputs.unsupported_remainder,
        &bindings.unsupported_remainder,
        "unsupported_remainder",
    )?;

    let accepted = compile_accepted_aggregate(inputs.accepted_fact_family.plan, bindings)?;
    let producers = compile_producer_aggregate(inputs.runtime_producer.plan, bindings)?;
    let remainders = compile_remainder_aggregate(inputs.unsupported_remainder.plan, bindings)?;
    let enriched =
        compile_family_enriched(accepted.clone(), producers.clone(), remainders.clone())?;
    let family_closure_internal = compile_family_closure_internal(enriched.clone(), bindings)?;
    let family_closure_plan =
        compile_family_closure_output(family_closure_internal.clone(), bindings, bounds)?;

    let query_program = compile_query_closure_internal(
        inputs.query_family_requirement.plan,
        family_closure_internal.clone(),
        bindings,
        bounds,
    )?;
    let query_requirement_closure_plan =
        compile_query_closure_output(query_program.closure.clone(), bindings, bounds)?;
    let violation_plan = compile_violations(
        enriched,
        producers,
        remainders,
        accepted,
        query_program.closure,
        query_program.depth_exhaustion,
        bindings,
        bounds,
    )?;

    let family_closure_schema = compiled_output_schema(
        "family_closure",
        &family_closure_plan,
        &bindings.family_closure.schema,
    )?;
    let query_requirement_closure_schema = compiled_output_schema(
        "query_requirement_closure",
        &query_requirement_closure_plan,
        &bindings.query_requirement_closure.schema,
    )?;
    let violation_schema =
        compiled_output_schema("violation", &violation_plan, &bindings.violation.schema)?;

    Ok(CompiledDerivedProducerClosure {
        family_closure_plan,
        query_requirement_closure_plan,
        violation_plan,
        family_closure_schema,
        query_requirement_closure_schema,
        violation_schema,
        input_fields: release_input_field_ids(bindings).into(),
        family_closure_fields: bindings.family_closure.fields.clone(),
        query_requirement_closure_fields: bindings.query_requirement_closure.fields.clone(),
        violation_fields: bindings.violation.fields.clone(),
        semantic_identities: bindings.semantic_identities.clone(),
        implementation_release: Arc::clone(&bindings.implementation_release),
        observation: ProducerClosureCompilationObservation {
            operation_id: Arc::clone(&bindings.operation_id),
            rung: ProducerClosureExecutionRung::NativeLogicalPlans,
            operators: BTreeSet::from([
                ProducerClosureNativeOperator::Projection,
                ProducerClosureNativeOperator::Aggregate,
                ProducerClosureNativeOperator::LeftJoin,
                ProducerClosureNativeOperator::LeftAntiJoin,
                ProducerClosureNativeOperator::Filter,
                ProducerClosureNativeOperator::RecursiveQueryDistinct,
                ProducerClosureNativeOperator::UnionAll,
                ProducerClosureNativeOperator::DeterministicSort,
                ProducerClosureNativeOperator::OutputOverflowProbeLimit,
            ]),
            dependencies: observe_dependencies(bindings),
            bounds,
        },
    })
}

fn compile_accepted_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.accepted_fact_family.fields;
    let aggregated = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.semantic_class_id.as_str()).alias(SEMANTIC_CLASS),
        ])?
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(ACCEPTED_COUNT),
                count_distinct(col(SEMANTIC_CLASS)).alias(SEMANTIC_CLASS_COUNT),
                min(col(SEMANTIC_CLASS)).alias(SEMANTIC_CLASS_MIN),
            ],
        )?
        .build()?;
    Ok(LogicalPlanBuilder::from(aggregated)
        .project([
            col(FAMILY),
            col(ACCEPTED_COUNT),
            col(SEMANTIC_CLASS_COUNT),
            coalesce(vec![col(SEMANTIC_CLASS_MIN), lit("")]).alias(SEMANTIC_CLASS),
        ])?
        .build()?)
}

fn compile_producer_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.runtime_producer.fields;
    let projected = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.producer_id.as_str()).alias(PRODUCER),
            col(fields.authority_id.as_str()).alias(PRODUCER_AUTHORITY),
            col(fields.algorithm_release.as_str()).alias(ALGORITHM_RELEASE),
            col(fields.precision_id.as_str()).alias(PRECISION),
            col(fields.input_pin.as_str()).alias(INPUT_PIN),
            col(fields.invalidation_pin.as_str()).alias(INVALIDATION_PIN),
            col(fields.materialization_pin.as_str()).alias(MATERIALIZATION_PIN),
            col(fields.requested_unit_count.as_str()).alias(REQUESTED_UNITS),
            col(fields.completed_unit_count.as_str()).alias(COMPLETED_UNITS),
            col(fields.remainder_unit_count.as_str()).alias(REMAINDER_UNITS),
            col(fields.unknown_unit_count.as_str()).alias(UNKNOWN_UNITS),
            col(fields.completeness_proof_pin.as_str()).alias(COMPLETENESS_PROOF_PIN),
            col(fields.proof_pin.as_str()).alias(PRODUCER_PROOF_PIN),
        ])?
        .build()?;
    Ok(LogicalPlanBuilder::from(projected)
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(PRODUCER_COUNT),
                min(col(PRODUCER)).alias(PRODUCER),
                min(col(PRODUCER_AUTHORITY)).alias(PRODUCER_AUTHORITY),
                min(col(ALGORITHM_RELEASE)).alias(ALGORITHM_RELEASE),
                min(col(PRECISION)).alias(PRECISION),
                min(col(INPUT_PIN)).alias(INPUT_PIN),
                min(col(INVALIDATION_PIN)).alias(INVALIDATION_PIN),
                min(col(MATERIALIZATION_PIN)).alias(MATERIALIZATION_PIN),
                min(col(REQUESTED_UNITS)).alias(REQUESTED_UNITS),
                min(col(COMPLETED_UNITS)).alias(COMPLETED_UNITS),
                min(col(REMAINDER_UNITS)).alias(REMAINDER_UNITS),
                min(col(UNKNOWN_UNITS)).alias(UNKNOWN_UNITS),
                min(col(COMPLETENESS_PROOF_PIN)).alias(COMPLETENESS_PROOF_PIN),
                min(col(PRODUCER_PROOF_PIN)).alias(PRODUCER_PROOF_PIN),
            ],
        )?
        .build()?)
}

fn compile_remainder_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.unsupported_remainder.fields;
    let projected = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.remainder_id.as_str()).alias(REMAINDER),
            col(fields.authority_id.as_str()).alias(REMAINDER_AUTHORITY),
            col(fields.reason_id.as_str()).alias(REMAINDER_REASON),
            col(fields.proof_pin.as_str()).alias(REMAINDER_PROOF_PIN),
        ])?
        .build()?;
    Ok(LogicalPlanBuilder::from(projected)
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(REMAINDER_COUNT),
                min(col(REMAINDER)).alias(REMAINDER),
                min(col(REMAINDER_AUTHORITY)).alias(REMAINDER_AUTHORITY),
                min(col(REMAINDER_REASON)).alias(REMAINDER_REASON),
                min(col(REMAINDER_PROOF_PIN)).alias(REMAINDER_PROOF_PIN),
            ],
        )?
        .build()?)
}

fn compile_family_enriched(
    accepted: LogicalPlan,
    producers: LogicalPlan,
    remainders: LogicalPlan,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let accepted = LogicalPlanBuilder::from(accepted)
        .alias(ACCEPTED_ALIAS)?
        .build()?;
    let producers = LogicalPlanBuilder::from(producers)
        .alias(PRODUCER_ALIAS)?
        .build()?;
    let accepted_and_producer = LogicalPlanBuilder::from(accepted)
        .join_on(
            producers,
            JoinType::Left,
            [qualified(ACCEPTED_ALIAS, FAMILY).eq(qualified(PRODUCER_ALIAS, FAMILY))],
        )?
        .project([
            qualified(ACCEPTED_ALIAS, FAMILY).alias(FAMILY),
            qualified(ACCEPTED_ALIAS, SEMANTIC_CLASS).alias(SEMANTIC_CLASS),
            qualified(ACCEPTED_ALIAS, ACCEPTED_COUNT).alias(ACCEPTED_COUNT),
            qualified(ACCEPTED_ALIAS, SEMANTIC_CLASS_COUNT).alias(SEMANTIC_CLASS_COUNT),
            coalesce(vec![qualified(PRODUCER_ALIAS, PRODUCER_COUNT), lit(0_i64)])
                .alias(PRODUCER_COUNT),
            qualified(PRODUCER_ALIAS, PRODUCER).alias(PRODUCER),
            qualified(PRODUCER_ALIAS, PRODUCER_AUTHORITY).alias(PRODUCER_AUTHORITY),
            qualified(PRODUCER_ALIAS, ALGORITHM_RELEASE).alias(ALGORITHM_RELEASE),
            qualified(PRODUCER_ALIAS, PRECISION).alias(PRECISION),
            qualified(PRODUCER_ALIAS, INPUT_PIN).alias(INPUT_PIN),
            qualified(PRODUCER_ALIAS, INVALIDATION_PIN).alias(INVALIDATION_PIN),
            qualified(PRODUCER_ALIAS, MATERIALIZATION_PIN).alias(MATERIALIZATION_PIN),
            qualified(PRODUCER_ALIAS, REQUESTED_UNITS).alias(REQUESTED_UNITS),
            qualified(PRODUCER_ALIAS, COMPLETED_UNITS).alias(COMPLETED_UNITS),
            qualified(PRODUCER_ALIAS, REMAINDER_UNITS).alias(REMAINDER_UNITS),
            qualified(PRODUCER_ALIAS, UNKNOWN_UNITS).alias(UNKNOWN_UNITS),
            qualified(PRODUCER_ALIAS, COMPLETENESS_PROOF_PIN).alias(COMPLETENESS_PROOF_PIN),
            qualified(PRODUCER_ALIAS, PRODUCER_PROOF_PIN).alias(PRODUCER_PROOF_PIN),
        ])?
        .alias(FAMILY_ALIAS)?
        .build()?;
    let remainders = LogicalPlanBuilder::from(remainders)
        .alias(REMAINDER_ALIAS)?
        .build()?;

    Ok(LogicalPlanBuilder::from(accepted_and_producer)
        .join_on(
            remainders,
            JoinType::Left,
            [qualified(FAMILY_ALIAS, FAMILY).eq(qualified(REMAINDER_ALIAS, FAMILY))],
        )?
        .project([
            qualified(FAMILY_ALIAS, FAMILY).alias(FAMILY),
            qualified(FAMILY_ALIAS, SEMANTIC_CLASS).alias(SEMANTIC_CLASS),
            qualified(FAMILY_ALIAS, ACCEPTED_COUNT).alias(ACCEPTED_COUNT),
            qualified(FAMILY_ALIAS, SEMANTIC_CLASS_COUNT).alias(SEMANTIC_CLASS_COUNT),
            qualified(FAMILY_ALIAS, PRODUCER_COUNT).alias(PRODUCER_COUNT),
            qualified(FAMILY_ALIAS, PRODUCER).alias(PRODUCER),
            qualified(FAMILY_ALIAS, PRODUCER_AUTHORITY).alias(PRODUCER_AUTHORITY),
            qualified(FAMILY_ALIAS, ALGORITHM_RELEASE).alias(ALGORITHM_RELEASE),
            qualified(FAMILY_ALIAS, PRECISION).alias(PRECISION),
            qualified(FAMILY_ALIAS, INPUT_PIN).alias(INPUT_PIN),
            qualified(FAMILY_ALIAS, INVALIDATION_PIN).alias(INVALIDATION_PIN),
            qualified(FAMILY_ALIAS, MATERIALIZATION_PIN).alias(MATERIALIZATION_PIN),
            qualified(FAMILY_ALIAS, REQUESTED_UNITS).alias(REQUESTED_UNITS),
            qualified(FAMILY_ALIAS, COMPLETED_UNITS).alias(COMPLETED_UNITS),
            qualified(FAMILY_ALIAS, REMAINDER_UNITS).alias(REMAINDER_UNITS),
            qualified(FAMILY_ALIAS, UNKNOWN_UNITS).alias(UNKNOWN_UNITS),
            qualified(FAMILY_ALIAS, COMPLETENESS_PROOF_PIN).alias(COMPLETENESS_PROOF_PIN),
            qualified(FAMILY_ALIAS, PRODUCER_PROOF_PIN).alias(PRODUCER_PROOF_PIN),
            coalesce(vec![
                qualified(REMAINDER_ALIAS, REMAINDER_COUNT),
                lit(0_i64),
            ])
            .alias(REMAINDER_COUNT),
            qualified(REMAINDER_ALIAS, REMAINDER).alias(REMAINDER),
            qualified(REMAINDER_ALIAS, REMAINDER_AUTHORITY).alias(REMAINDER_AUTHORITY),
            qualified(REMAINDER_ALIAS, REMAINDER_REASON).alias(REMAINDER_REASON),
            qualified(REMAINDER_ALIAS, REMAINDER_PROOF_PIN).alias(REMAINDER_PROOF_PIN),
        ])?
        .build()?)
}

fn compile_family_closure_internal(
    enriched: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let semantic = &bindings.semantic_identities;
    let producer_contract_present = all_non_empty([
        PRODUCER,
        ALGORITHM_RELEASE,
        PRECISION,
        INPUT_PIN,
        INVALIDATION_PIN,
        MATERIALIZATION_PIN,
        COMPLETENESS_PROOF_PIN,
        PRODUCER_PROOF_PIN,
    ]);
    let remainder_contract_present =
        all_non_empty([REMAINDER, REMAINDER_REASON, REMAINDER_PROOF_PIN]);
    let accepted_valid = col(ACCEPTED_COUNT)
        .eq(lit(1_i64))
        .and(col(SEMANTIC_CLASS_COUNT).eq(lit(1_i64)))
        .and(col(SEMANTIC_CLASS).eq(lit(semantic.factual_semantic_class_id.as_ref())));
    let producer_owned =
        col(PRODUCER_AUTHORITY).eq(lit(semantic.application_owned_authority_id.as_ref()));
    let remainder_owned =
        col(REMAINDER_AUTHORITY).eq(lit(semantic.application_owned_authority_id.as_ref()));
    let producer_exclusive = col(PRODUCER_COUNT)
        .eq(lit(1_i64))
        .and(col(REMAINDER_COUNT).eq(lit(0_i64)));
    let remainder_exclusive = col(PRODUCER_COUNT)
        .eq(lit(0_i64))
        .and(col(REMAINDER_COUNT).eq(lit(1_i64)));
    let producer_complete = col(REQUESTED_UNITS)
        .eq(col(COMPLETED_UNITS))
        .and(col(REMAINDER_UNITS).eq(lit(0_u64)))
        .and(col(UNKNOWN_UNITS).eq(lit(0_u64)));

    let state = datafusion::logical_expr::expr_fn::when(
        accepted_valid
            .clone()
            .and(producer_exclusive.clone())
            .and(producer_owned.clone())
            .and(producer_contract_present.clone())
            .and(producer_complete.clone()),
        lit(STATE_SUPPORTED),
    )
    .when(
        accepted_valid
            .clone()
            .and(producer_exclusive)
            .and(producer_owned)
            .and(producer_contract_present)
            .and(producer_complete.not()),
        lit(STATE_UNKNOWN),
    )
    .when(
        accepted_valid
            .and(remainder_exclusive)
            .and(remainder_owned)
            .and(remainder_contract_present),
        lit(STATE_UNSUPPORTED),
    )
    .otherwise(lit(STATE_INVALID))?
    .alias(CLOSURE_STATE);

    Ok(LogicalPlanBuilder::from(enriched)
        .project([
            col(FAMILY),
            col(SEMANTIC_CLASS),
            state,
            col(PRODUCER),
            col(PRODUCER_AUTHORITY),
            col(ALGORITHM_RELEASE),
            col(PRECISION),
            col(INPUT_PIN),
            col(INVALIDATION_PIN),
            col(MATERIALIZATION_PIN),
            col(REQUESTED_UNITS),
            col(COMPLETED_UNITS),
            col(REMAINDER_UNITS),
            col(UNKNOWN_UNITS),
            col(COMPLETENESS_PROOF_PIN),
            col(PRODUCER_PROOF_PIN),
            col(REMAINDER),
            col(REMAINDER_AUTHORITY),
            col(REMAINDER_REASON),
            col(REMAINDER_PROOF_PIN),
        ])?
        .build()?)
}

fn compile_family_closure_output(
    closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.family_closure.fields;
    Ok(LogicalPlanBuilder::from(closure)
        .project([
            col(FAMILY).alias(fields.family_id.as_str()),
            col(SEMANTIC_CLASS).alias(fields.semantic_class_id.as_str()),
            col(CLOSURE_STATE).alias(fields.closure_state.as_str()),
            col(PRODUCER).alias(fields.producer_id.as_str()),
            coalesce(vec![col(PRODUCER_AUTHORITY), col(REMAINDER_AUTHORITY)])
                .alias(fields.authority_id.as_str()),
            col(ALGORITHM_RELEASE).alias(fields.algorithm_release.as_str()),
            col(PRECISION).alias(fields.precision_id.as_str()),
            col(INPUT_PIN).alias(fields.input_pin.as_str()),
            col(INVALIDATION_PIN).alias(fields.invalidation_pin.as_str()),
            col(MATERIALIZATION_PIN).alias(fields.materialization_pin.as_str()),
            col(REQUESTED_UNITS).alias(fields.requested_unit_count.as_str()),
            col(COMPLETED_UNITS).alias(fields.completed_unit_count.as_str()),
            col(REMAINDER_UNITS).alias(fields.remainder_unit_count.as_str()),
            col(UNKNOWN_UNITS).alias(fields.unknown_unit_count.as_str()),
            col(COMPLETENESS_PROOF_PIN).alias(fields.completeness_proof_pin.as_str()),
            col(PRODUCER_PROOF_PIN).alias(fields.producer_proof_pin.as_str()),
            col(REMAINDER).alias(fields.unsupported_remainder_id.as_str()),
            col(REMAINDER_REASON).alias(fields.unsupported_reason_id.as_str()),
            col(REMAINDER_PROOF_PIN).alias(fields.unsupported_proof_pin.as_str()),
        ])?
        .sort([col(fields.family_id.as_str()).sort(true, false)])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

struct QueryClosureInternal {
    closure: LogicalPlan,
    depth_exhaustion: LogicalPlan,
}

fn compile_query_closure_internal(
    query_plan: LogicalPlan,
    family_closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<QueryClosureInternal, DerivedProducerClosureError> {
    let fields = &bindings.query_family_requirement.fields;
    let edges = LogicalPlanBuilder::from(query_plan)
        .project([
            col(fields.query_family_id.as_str()).alias(QUERY_ROOT),
            col(fields.required_family_id.as_str()).alias(QUERY_REQUIRED),
        ])?
        .distinct()?
        .alias(QUERY_EDGE_ALIAS)?
        .build()?;
    let seed = LogicalPlanBuilder::from(edges.clone())
        .project([
            qualified(QUERY_EDGE_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_EDGE_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            lit(1_u32).alias(QUERY_DEPTH),
        ])?
        .build()?;
    let work_table = Arc::new(CteWorkTable::new(
        QUERY_RECURSIVE_NAME,
        Arc::new(seed.schema().as_arrow().clone()),
    ));
    let frontier =
        LogicalPlanBuilder::scan(QUERY_RECURSIVE_NAME, provider_as_source(work_table), None)?
            .alias(QUERY_FRONTIER_ALIAS)?
            .build()?;
    let recursive_term = LogicalPlanBuilder::from(frontier)
        .filter(
            qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH)
                .lt(lit(u32::from(bounds.max_query_depth()))),
        )?
        .join_on(
            edges.clone(),
            JoinType::Inner,
            [qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_EDGE_ALIAS, QUERY_ROOT))],
        )?
        .project([
            qualified(QUERY_FRONTIER_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_EDGE_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            (qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH) + lit(1_u32)).alias(QUERY_DEPTH),
        ])?
        .build()?;
    let recursive = LogicalPlanBuilder::from(seed)
        .to_recursive_query(QUERY_RECURSIVE_NAME.to_owned(), recursive_term, true)?
        .build()?;

    let reach = LogicalPlanBuilder::from(recursive.clone())
        .aggregate(
            [col(QUERY_ROOT), col(QUERY_REQUIRED)],
            [min(col(QUERY_DEPTH)).alias(QUERY_DEPTH)],
        )?
        .alias(QUERY_REACH_ALIAS)?
        .build()?;
    let query_sources = LogicalPlanBuilder::from(edges.clone())
        .project([qualified(QUERY_EDGE_ALIAS, QUERY_ROOT).alias(QUERY_SOURCE_MARKER)])?
        .distinct()?
        .alias(QUERY_SOURCE_ALIAS)?
        .build()?;
    let family_closure = LogicalPlanBuilder::from(family_closure)
        .alias(FAMILY_ALIAS)?
        .build()?;
    let reach_and_family = LogicalPlanBuilder::from(reach)
        .join_on(
            family_closure,
            JoinType::Left,
            [qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED).eq(qualified(FAMILY_ALIAS, FAMILY))],
        )?
        .build()?;
    let joined = LogicalPlanBuilder::from(reach_and_family)
        .join_on(
            query_sources,
            JoinType::Left,
            [qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_SOURCE_ALIAS, QUERY_SOURCE_MARKER))],
        )?
        .filter(
            qualified(FAMILY_ALIAS, FAMILY).is_not_null().or(qualified(
                QUERY_SOURCE_ALIAS,
                QUERY_SOURCE_MARKER,
            )
            .is_null()),
        )?
        .build()?;

    let state = datafusion::logical_expr::expr_fn::when(
        qualified(FAMILY_ALIAS, FAMILY).is_null(),
        lit(STATE_MISSING),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_SUPPORTED)),
        lit(STATE_SATISFIED),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNSUPPORTED)),
        lit(STATE_UNSUPPORTED),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNKNOWN)),
        lit(STATE_UNKNOWN),
    )
    .otherwise(lit(STATE_INVALID))?;
    let cause = datafusion::logical_expr::expr_fn::when(
        qualified(FAMILY_ALIAS, FAMILY).is_null(),
        lit("accepted_family_absent"),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNSUPPORTED)),
        qualified(FAMILY_ALIAS, REMAINDER_REASON),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNKNOWN)),
        lit("required_family_incomplete"),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_INVALID)),
        lit("required_family_invalid"),
    )
    .otherwise(lit(ScalarValue::Utf8(None)))?;
    let closure = LogicalPlanBuilder::from(joined)
        .project([
            qualified(QUERY_REACH_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            coalesce(vec![qualified(QUERY_REACH_ALIAS, QUERY_DEPTH), lit(0_u32)])
                .alias(QUERY_DEPTH),
            state.alias(QUERY_STATE),
            cause.alias(QUERY_UNKNOWN_CAUSE),
        ])?
        .build()?;

    let recursive = LogicalPlanBuilder::from(recursive)
        .alias(QUERY_FRONTIER_ALIAS)?
        .build()?;
    let depth_exhaustion = LogicalPlanBuilder::from(recursive)
        .filter(
            qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH)
                .eq(lit(u32::from(bounds.max_query_depth()))),
        )?
        .join_on(
            edges,
            JoinType::Inner,
            [qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_EDGE_ALIAS, QUERY_ROOT))],
        )?
        .project([
            qualified(QUERY_FRONTIER_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
        ])?
        .distinct()?
        .build()?;

    Ok(QueryClosureInternal {
        closure,
        depth_exhaustion,
    })
}

fn compile_query_closure_output(
    closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.query_requirement_closure.fields;
    Ok(LogicalPlanBuilder::from(closure)
        .project([
            col(QUERY_ROOT).alias(fields.query_family_id.as_str()),
            col(QUERY_REQUIRED).alias(fields.required_family_id.as_str()),
            col(QUERY_DEPTH).alias(fields.minimum_depth.as_str()),
            col(QUERY_STATE).alias(fields.requirement_state.as_str()),
            col(QUERY_UNKNOWN_CAUSE).alias(fields.unknown_cause.as_str()),
        ])?
        .sort([
            col(fields.query_family_id.as_str()).sort(true, false),
            col(fields.minimum_depth.as_str()).sort(true, false),
            col(fields.required_family_id.as_str()).sort(true, false),
        ])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

#[allow(clippy::too_many_arguments)]
fn compile_violations(
    enriched: LogicalPlan,
    producers: LogicalPlan,
    remainders: LogicalPlan,
    accepted: LogicalPlan,
    query_closure: LogicalPlan,
    depth_exhaustion: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let semantic = &bindings.semantic_identities;
    let none = || lit(ScalarValue::Utf8(None));
    let mut branches = vec![
        violation_branch(
            enriched.clone(),
            col(ACCEPTED_COUNT).not_eq(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "duplicate_accepted_family",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(SEMANTIC_CLASS_COUNT)
                .not_eq(lit(1_i64))
                .or(col(SEMANTIC_CLASS).not_eq(lit(semantic.factual_semantic_class_id.as_ref()))),
            "accepted_fact_family",
            col(FAMILY),
            "non_fact_semantic_class",
            col(SEMANTIC_CLASS),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT)
                .eq(lit(0_i64))
                .and(col(REMAINDER_COUNT).eq(lit(0_i64))),
            "accepted_fact_family",
            col(FAMILY),
            "missing_producer_or_remainder",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).gt(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "multiple_runtime_producers",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT).gt(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "multiple_unsupported_remainders",
            col(REMAINDER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT)
                .gt(lit(0_i64))
                .and(col(REMAINDER_COUNT).gt(lit(0_i64))),
            "accepted_fact_family",
            col(FAMILY),
            "producer_and_remainder",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                col(PRODUCER_AUTHORITY)
                    .not_eq(lit(semantic.application_owned_authority_id.as_ref())),
            ),
            "runtime_producer",
            col(FAMILY),
            "wrong_runtime_producer_authority",
            col(PRODUCER_AUTHORITY),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                all_non_empty([
                    PRODUCER,
                    ALGORITHM_RELEASE,
                    PRECISION,
                    INPUT_PIN,
                    INVALIDATION_PIN,
                    MATERIALIZATION_PIN,
                    COMPLETENESS_PROOF_PIN,
                    PRODUCER_PROOF_PIN,
                ])
                .not(),
            ),
            "runtime_producer",
            col(FAMILY),
            "missing_runtime_producer_contract_pin",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                col(REQUESTED_UNITS)
                    .not_eq(col(COMPLETED_UNITS))
                    .or(col(REMAINDER_UNITS).not_eq(lit(0_u64)))
                    .or(col(UNKNOWN_UNITS).not_eq(lit(0_u64))),
            ),
            "runtime_producer",
            col(FAMILY),
            "incomplete_runtime_producer",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT).eq(lit(1_i64)).and(
                col(REMAINDER_AUTHORITY)
                    .not_eq(lit(semantic.application_owned_authority_id.as_ref())),
            ),
            "unsupported_remainder",
            col(FAMILY),
            "wrong_unsupported_remainder_authority",
            col(REMAINDER_AUTHORITY),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT)
                .eq(lit(1_i64))
                .and(all_non_empty([REMAINDER, REMAINDER_REASON, REMAINDER_PROOF_PIN]).not()),
            "unsupported_remainder",
            col(FAMILY),
            "incomplete_unsupported_remainder",
            col(REMAINDER),
            bindings,
        )?,
    ];

    let accepted_alias = LogicalPlanBuilder::from(accepted)
        .alias(ACCEPTED_ALIAS)?
        .build()?;
    let orphan_producers = LogicalPlanBuilder::from(producers)
        .alias(PRODUCER_ALIAS)?
        .join_on(
            accepted_alias.clone(),
            JoinType::LeftAnti,
            [qualified(PRODUCER_ALIAS, FAMILY).eq(qualified(ACCEPTED_ALIAS, FAMILY))],
        )?
        .project([
            qualified(PRODUCER_ALIAS, FAMILY).alias(FAMILY),
            qualified(PRODUCER_ALIAS, PRODUCER).alias(PRODUCER),
        ])?
        .build()?;
    branches.push(violation_branch(
        orphan_producers,
        lit(true),
        "runtime_producer",
        col(FAMILY),
        "orphan_runtime_producer",
        col(PRODUCER),
        bindings,
    )?);
    let orphan_remainders = LogicalPlanBuilder::from(remainders)
        .alias(REMAINDER_ALIAS)?
        .join_on(
            accepted_alias,
            JoinType::LeftAnti,
            [qualified(REMAINDER_ALIAS, FAMILY).eq(qualified(ACCEPTED_ALIAS, FAMILY))],
        )?
        .project([
            qualified(REMAINDER_ALIAS, FAMILY).alias(FAMILY),
            qualified(REMAINDER_ALIAS, REMAINDER).alias(REMAINDER),
        ])?
        .build()?;
    branches.push(violation_branch(
        orphan_remainders,
        lit(true),
        "unsupported_remainder",
        col(FAMILY),
        "orphan_unsupported_remainder",
        col(REMAINDER),
        bindings,
    )?);

    for (state, code) in [
        (STATE_MISSING, "query_requirement_missing"),
        (STATE_UNKNOWN, "query_requirement_incomplete"),
        (STATE_INVALID, "query_requirement_invalid"),
    ] {
        branches.push(violation_branch(
            query_closure.clone(),
            col(QUERY_STATE).eq(lit(state)),
            "query_family_requirement",
            col(QUERY_ROOT),
            code,
            col(QUERY_REQUIRED),
            bindings,
        )?);
    }
    branches.push(violation_branch(
        depth_exhaustion,
        lit(true),
        "query_family_requirement",
        col(QUERY_ROOT),
        "query_requirement_depth_exhausted",
        col(QUERY_REQUIRED),
        bindings,
    )?);

    let mut iterator = branches.into_iter();
    let mut union = iterator
        .next()
        .ok_or(DerivedProducerClosureError::InternalNoViolationBranches)?;
    for branch in iterator {
        union = LogicalPlanBuilder::from(union).union(branch)?.build()?;
    }
    let fields = &bindings.violation.fields;
    Ok(LogicalPlanBuilder::from(union)
        .distinct()?
        .sort([
            col(fields.subject_kind.as_str()).sort(true, false),
            col(fields.subject_id.as_str()).sort(true, false),
            col(fields.violation_code.as_str()).sort(true, false),
            col(fields.related_id.as_str()).sort(true, true),
        ])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

fn violation_branch(
    source: LogicalPlan,
    condition: Expr,
    subject_kind: &'static str,
    subject_id: Expr,
    violation_code: &'static str,
    related_id: Expr,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.violation.fields;
    Ok(LogicalPlanBuilder::from(source)
        .filter(condition)?
        .project([
            lit(subject_kind).alias(fields.subject_kind.as_str()),
            subject_id.alias(fields.subject_id.as_str()),
            lit(violation_code).alias(fields.violation_code.as_str()),
            related_id.alias(fields.related_id.as_str()),
        ])?
        .build()?)
}

fn all_non_empty<const N: usize>(fields: [&'static str; N]) -> Expr {
    fields
        .into_iter()
        .map(|field| col(field).not_eq(lit("")))
        .reduce(Expr::and)
        .unwrap_or_else(|| lit(true))
}

fn qualified(alias: &'static str, field: &str) -> Expr {
    Expr::Column(Column::new(
        Some(TableReference::bare(alias)),
        field.to_owned(),
    ))
}

fn validate_relation_contracts(
    accepted: &ProducerClosureRelationContract<AcceptedFactFamilyFields>,
    producer: &ProducerClosureRelationContract<RuntimeProducerFields>,
    query: &ProducerClosureRelationContract<QueryFamilyRequirementFields>,
    remainder: &ProducerClosureRelationContract<UnsupportedRemainderFields>,
    family_output: &ProducerClosureRelationContract<FamilyClosureFields>,
    query_output: &ProducerClosureRelationContract<QueryRequirementClosureFields>,
    violation: &ProducerClosureRelationContract<ProducerClosureViolationFields>,
) -> Result<(), DerivedProducerClosureError> {
    let relation_ids = [
        accepted.relation_id(),
        producer.relation_id(),
        query.relation_id(),
        remainder.relation_id(),
        family_output.relation_id(),
        query_output.relation_id(),
        violation.relation_id(),
    ];
    let mut unique_relations = BTreeSet::new();
    for relation_id in relation_ids {
        if !unique_relations.insert(relation_id.as_str()) {
            return Err(DerivedProducerClosureError::DuplicateRelationId(
                relation_id.as_str().to_owned(),
            ));
        }
    }

    validate_exact_fields(
        "accepted_fact_family",
        &accepted.schema,
        &[
            (&accepted.fields.family_id, DataType::Utf8, false),
            (&accepted.fields.semantic_class_id, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "runtime_producer",
        &producer.schema,
        &[
            (&producer.fields.family_id, DataType::Utf8, false),
            (&producer.fields.producer_id, DataType::Utf8, false),
            (&producer.fields.authority_id, DataType::Utf8, false),
            (&producer.fields.algorithm_release, DataType::Utf8, false),
            (&producer.fields.precision_id, DataType::Utf8, false),
            (&producer.fields.input_pin, DataType::Utf8, false),
            (&producer.fields.invalidation_pin, DataType::Utf8, false),
            (&producer.fields.materialization_pin, DataType::Utf8, false),
            (
                &producer.fields.requested_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer.fields.completed_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer.fields.remainder_unit_count,
                DataType::UInt64,
                false,
            ),
            (&producer.fields.unknown_unit_count, DataType::UInt64, false),
            (
                &producer.fields.completeness_proof_pin,
                DataType::Utf8,
                false,
            ),
            (&producer.fields.proof_pin, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "query_family_requirement",
        &query.schema,
        &[
            (&query.fields.query_family_id, DataType::Utf8, false),
            (&query.fields.required_family_id, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "unsupported_remainder",
        &remainder.schema,
        &[
            (&remainder.fields.family_id, DataType::Utf8, false),
            (&remainder.fields.remainder_id, DataType::Utf8, false),
            (&remainder.fields.authority_id, DataType::Utf8, false),
            (&remainder.fields.reason_id, DataType::Utf8, false),
            (&remainder.fields.proof_pin, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "family_closure",
        &family_output.schema,
        &[
            (&family_output.fields.family_id, DataType::Utf8, false),
            (
                &family_output.fields.semantic_class_id,
                DataType::Utf8,
                false,
            ),
            (&family_output.fields.closure_state, DataType::Utf8, false),
            (&family_output.fields.producer_id, DataType::Utf8, true),
            (&family_output.fields.authority_id, DataType::Utf8, true),
            (
                &family_output.fields.algorithm_release,
                DataType::Utf8,
                true,
            ),
            (&family_output.fields.precision_id, DataType::Utf8, true),
            (&family_output.fields.input_pin, DataType::Utf8, true),
            (&family_output.fields.invalidation_pin, DataType::Utf8, true),
            (
                &family_output.fields.materialization_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.requested_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.completed_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.remainder_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.unknown_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.completeness_proof_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.producer_proof_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_remainder_id,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_reason_id,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_proof_pin,
                DataType::Utf8,
                true,
            ),
        ],
    )?;
    validate_exact_fields(
        "query_requirement_closure",
        &query_output.schema,
        &[
            (&query_output.fields.query_family_id, DataType::Utf8, false),
            (
                &query_output.fields.required_family_id,
                DataType::Utf8,
                false,
            ),
            (&query_output.fields.minimum_depth, DataType::UInt32, false),
            (
                &query_output.fields.requirement_state,
                DataType::Utf8,
                false,
            ),
            (&query_output.fields.unknown_cause, DataType::Utf8, true),
        ],
    )?;
    validate_exact_fields(
        "producer_closure_violation",
        &violation.schema,
        &[
            (&violation.fields.subject_kind, DataType::Utf8, false),
            (&violation.fields.subject_id, DataType::Utf8, false),
            (&violation.fields.violation_code, DataType::Utf8, false),
            (&violation.fields.related_id, DataType::Utf8, true),
        ],
    )?;
    Ok(())
}

fn validate_exact_fields(
    role: &'static str,
    schema: &SchemaRef,
    expected: &[(&FieldId, DataType, bool)],
) -> Result<(), DerivedProducerClosureError> {
    if schema.fields().len() != expected.len() {
        return Err(DerivedProducerClosureError::SchemaFieldCount {
            relation: role,
            expected: expected.len(),
            actual: schema.fields().len(),
        });
    }
    let mut identities = BTreeSet::new();
    for (ordinal, ((field_id, data_type, nullable), actual)) in
        expected.iter().zip(schema.fields()).enumerate()
    {
        if !identities.insert(field_id.as_str()) {
            return Err(DerivedProducerClosureError::DuplicateFieldId {
                relation: role,
                field: field_id.as_str().to_owned(),
            });
        }
        if actual.name() != field_id.as_str()
            || actual.data_type() != data_type
            || actual.is_nullable() != *nullable
        {
            return Err(DerivedProducerClosureError::SchemaFieldMismatch {
                relation: role,
                ordinal,
                expected_name: field_id.as_str().to_owned(),
                expected_type: data_type.clone(),
                expected_nullable: *nullable,
                actual_name: actual.name().clone(),
                actual_type: actual.data_type().clone(),
                actual_nullable: actual.is_nullable(),
            });
        }
    }
    Ok(())
}

fn validate_input<F>(
    input: &ProducerClosureRelationInput,
    contract: &ProducerClosureRelationContract<F>,
    role: &'static str,
) -> Result<(), DerivedProducerClosureError> {
    if input.relation_id != contract.relation_id {
        return Err(DerivedProducerClosureError::InputRelationMismatch {
            role,
            expected: contract.relation_id.as_str().to_owned(),
            actual: input.relation_id.as_str().to_owned(),
        });
    }
    let actual = input.plan.schema().as_arrow();
    if actual != contract.schema.as_ref() {
        return Err(DerivedProducerClosureError::InputSchemaMismatch {
            role,
            expected: Arc::clone(&contract.schema),
            actual: Arc::new(actual.clone()),
        });
    }
    Ok(())
}

fn compiled_output_schema(
    role: &'static str,
    plan: &LogicalPlan,
    expected: &SchemaRef,
) -> Result<SchemaRef, DerivedProducerClosureError> {
    let actual = Arc::new(plan.schema().as_arrow().clone());
    if actual.fields().len() != expected.fields().len() {
        return Err(DerivedProducerClosureError::SchemaFieldCount {
            relation: role,
            expected: expected.fields().len(),
            actual: actual.fields().len(),
        });
    }
    for (ordinal, (expected_field, actual_field)) in
        expected.fields().iter().zip(actual.fields()).enumerate()
    {
        if expected_field.name() != actual_field.name()
            || expected_field.data_type() != actual_field.data_type()
            || expected_field.is_nullable() != actual_field.is_nullable()
        {
            return Err(DerivedProducerClosureError::SchemaFieldMismatch {
                relation: role,
                ordinal,
                expected_name: expected_field.name().clone(),
                expected_type: expected_field.data_type().clone(),
                expected_nullable: expected_field.is_nullable(),
                actual_name: actual_field.name().clone(),
                actual_type: actual_field.data_type().clone(),
                actual_nullable: actual_field.is_nullable(),
            });
        }
    }
    Ok(actual)
}

fn validate_text(kind: &'static str, value: &str) -> Result<(), DerivedProducerClosureError> {
    if value.is_empty() || value.len() > 240 {
        return Err(DerivedProducerClosureError::InvalidText {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn observe_dependencies(
    bindings: &DerivedProducerClosureBindings,
) -> BTreeSet<ProducerClosureCompilationDependency> {
    let mut dependencies = BTreeSet::from([
        ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::clone(
            &bindings.semantic_identities.application_owned_authority_id,
        )),
        ProducerClosureCompilationDependency::FactualSemanticClass(Arc::clone(
            &bindings.semantic_identities.factual_semantic_class_id,
        )),
        ProducerClosureCompilationDependency::ImplementationRelease(Arc::clone(
            &bindings.implementation_release,
        )),
        ProducerClosureCompilationDependency::SessionMemoryPool,
        ProducerClosureCompilationDependency::DataFusionExecuteStreamDropAbort,
    ]);

    macro_rules! relation_dependencies {
        ($contract:expr, [$($field:expr),+ $(,)?]) => {{
            dependencies.insert(ProducerClosureCompilationDependency::InputRelation(
                $contract.relation_id.clone(),
            ));
            $(dependencies.insert(ProducerClosureCompilationDependency::InputField(
                $field.clone(),
            ));)+
        }};
    }
    relation_dependencies!(
        bindings.accepted_fact_family,
        [
            bindings.accepted_fact_family.fields.family_id,
            bindings.accepted_fact_family.fields.semantic_class_id,
        ]
    );
    relation_dependencies!(
        bindings.runtime_producer,
        [
            bindings.runtime_producer.fields.family_id,
            bindings.runtime_producer.fields.producer_id,
            bindings.runtime_producer.fields.authority_id,
            bindings.runtime_producer.fields.algorithm_release,
            bindings.runtime_producer.fields.precision_id,
            bindings.runtime_producer.fields.input_pin,
            bindings.runtime_producer.fields.invalidation_pin,
            bindings.runtime_producer.fields.materialization_pin,
            bindings.runtime_producer.fields.requested_unit_count,
            bindings.runtime_producer.fields.completed_unit_count,
            bindings.runtime_producer.fields.remainder_unit_count,
            bindings.runtime_producer.fields.unknown_unit_count,
            bindings.runtime_producer.fields.completeness_proof_pin,
            bindings.runtime_producer.fields.proof_pin,
        ]
    );
    relation_dependencies!(
        bindings.query_family_requirement,
        [
            bindings.query_family_requirement.fields.query_family_id,
            bindings.query_family_requirement.fields.required_family_id,
        ]
    );
    relation_dependencies!(
        bindings.unsupported_remainder,
        [
            bindings.unsupported_remainder.fields.family_id,
            bindings.unsupported_remainder.fields.remainder_id,
            bindings.unsupported_remainder.fields.authority_id,
            bindings.unsupported_remainder.fields.reason_id,
            bindings.unsupported_remainder.fields.proof_pin,
        ]
    );

    macro_rules! output_dependencies {
        ($contract:expr, [$($field:expr),+ $(,)?]) => {{
            dependencies.insert(ProducerClosureCompilationDependency::OutputRelation(
                $contract.relation_id.clone(),
            ));
            $(dependencies.insert(ProducerClosureCompilationDependency::OutputField(
                $field.clone(),
            ));)+
        }};
    }
    output_dependencies!(
        bindings.family_closure,
        [
            bindings.family_closure.fields.family_id,
            bindings.family_closure.fields.semantic_class_id,
            bindings.family_closure.fields.closure_state,
            bindings.family_closure.fields.producer_id,
            bindings.family_closure.fields.authority_id,
            bindings.family_closure.fields.algorithm_release,
            bindings.family_closure.fields.precision_id,
            bindings.family_closure.fields.input_pin,
            bindings.family_closure.fields.invalidation_pin,
            bindings.family_closure.fields.materialization_pin,
            bindings.family_closure.fields.requested_unit_count,
            bindings.family_closure.fields.completed_unit_count,
            bindings.family_closure.fields.remainder_unit_count,
            bindings.family_closure.fields.unknown_unit_count,
            bindings.family_closure.fields.completeness_proof_pin,
            bindings.family_closure.fields.producer_proof_pin,
            bindings.family_closure.fields.unsupported_remainder_id,
            bindings.family_closure.fields.unsupported_reason_id,
            bindings.family_closure.fields.unsupported_proof_pin,
        ]
    );
    output_dependencies!(
        bindings.query_requirement_closure,
        [
            bindings.query_requirement_closure.fields.query_family_id,
            bindings.query_requirement_closure.fields.required_family_id,
            bindings.query_requirement_closure.fields.minimum_depth,
            bindings.query_requirement_closure.fields.requirement_state,
            bindings.query_requirement_closure.fields.unknown_cause,
        ]
    );
    output_dependencies!(
        bindings.violation,
        [
            bindings.violation.fields.subject_kind,
            bindings.violation.fields.subject_id,
            bindings.violation.fields.violation_code,
            bindings.violation.fields.related_id,
        ]
    );
    dependencies
}

fn release_input_field_ids(bindings: &DerivedProducerClosureBindings) -> Vec<FieldId> {
    vec![
        bindings.accepted_fact_family.fields.family_id.clone(),
        bindings
            .accepted_fact_family
            .fields
            .semantic_class_id
            .clone(),
        bindings.runtime_producer.fields.family_id.clone(),
        bindings.runtime_producer.fields.producer_id.clone(),
        bindings.runtime_producer.fields.authority_id.clone(),
        bindings.runtime_producer.fields.algorithm_release.clone(),
        bindings.runtime_producer.fields.precision_id.clone(),
        bindings.runtime_producer.fields.input_pin.clone(),
        bindings.runtime_producer.fields.invalidation_pin.clone(),
        bindings.runtime_producer.fields.materialization_pin.clone(),
        bindings
            .runtime_producer
            .fields
            .requested_unit_count
            .clone(),
        bindings
            .runtime_producer
            .fields
            .completed_unit_count
            .clone(),
        bindings
            .runtime_producer
            .fields
            .remainder_unit_count
            .clone(),
        bindings.runtime_producer.fields.unknown_unit_count.clone(),
        bindings
            .runtime_producer
            .fields
            .completeness_proof_pin
            .clone(),
        bindings.runtime_producer.fields.proof_pin.clone(),
        bindings
            .query_family_requirement
            .fields
            .query_family_id
            .clone(),
        bindings
            .query_family_requirement
            .fields
            .required_family_id
            .clone(),
        bindings.unsupported_remainder.fields.family_id.clone(),
        bindings.unsupported_remainder.fields.remainder_id.clone(),
        bindings.unsupported_remainder.fields.authority_id.clone(),
        bindings.unsupported_remainder.fields.reason_id.clone(),
        bindings.unsupported_remainder.fields.proof_pin.clone(),
    ]
}

#[allow(clippy::too_many_arguments)]
fn decode_release_producer_closure_evidence(
    family_batches: &[RecordBatch],
    family_fields: &FamilyClosureFields,
    query_batches: &[RecordBatch],
    query_fields: &QueryRequirementClosureFields,
    violation_batches: &[RecordBatch],
    violation_fields: &ProducerClosureViolationFields,
    input_fields: &[FieldId],
    semantic_identities: &ProducerClosureSemanticIdentities,
    implementation_release: &Arc<str>,
    observation: &ProducerClosureCompilationObservation,
) -> Result<ReleaseProducerClosureEvidence, DerivedProducerClosureError> {
    let mut families = decode_family_closure_rows(family_batches, family_fields)?;
    let mut query_requirements = decode_query_requirement_rows(query_batches, query_fields)?;
    let mut violations = decode_violation_rows(violation_batches, violation_fields)?;
    families.sort_by(|left, right| left.family_id.cmp(&right.family_id));
    query_requirements.sort_by(|left, right| {
        (
            &left.query_family_id,
            left.minimum_depth,
            &left.required_family_id,
        )
            .cmp(&(
                &right.query_family_id,
                right.minimum_depth,
                &right.required_family_id,
            ))
    });
    violations.sort_by(|left, right| {
        (
            &left.subject_kind,
            &left.subject_id,
            &left.violation_code,
            &left.related_id,
        )
            .cmp(&(
                &right.subject_kind,
                &right.subject_id,
                &right.violation_code,
                &right.related_id,
            ))
    });

    let mut issues = Vec::new();
    validate_compilation_observation(
        observation,
        family_fields,
        query_fields,
        violation_fields,
        input_fields,
        semantic_identities,
        implementation_release,
        &mut issues,
    )?;
    validate_decoded_families(&families, semantic_identities, &mut issues);
    validate_decoded_queries(&query_requirements, &families, &mut issues);
    for violation in &violations {
        issues.push(ReleaseProducerClosureIssue::new(
            "reported_producer_closure_violation",
            Some(Arc::clone(&violation.subject_id)),
            Some(Arc::clone(&violation.violation_code)),
        ));
    }

    Ok(ReleaseProducerClosureEvidence {
        operation_id: Arc::clone(&observation.operation_id),
        implementation_release: Arc::clone(implementation_release),
        application_authority_id: Arc::clone(semantic_identities.application_owned_authority_id()),
        factual_semantic_class_id: Arc::clone(semantic_identities.factual_semantic_class_id()),
        families: families.into(),
        query_requirements: query_requirements.into(),
        violations: violations.into(),
        issues: issues.into(),
        dependencies: observation
            .dependencies
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .into(),
    })
}

fn decode_family_closure_rows(
    batches: &[RecordBatch],
    fields: &FamilyClosureFields,
) -> Result<Vec<ReleaseProducerFamilyClosureRow>, DerivedProducerClosureError> {
    let mut rows = Vec::new();
    for batch in batches {
        for row in 0..batch.num_rows() {
            rows.push(ReleaseProducerFamilyClosureRow {
                family_id: required_executed_text(batch, &fields.family_id, row, "family_closure")?,
                semantic_class_id: required_executed_text(
                    batch,
                    &fields.semantic_class_id,
                    row,
                    "family_closure",
                )?,
                closure_state: required_executed_text(
                    batch,
                    &fields.closure_state,
                    row,
                    "family_closure",
                )?,
                producer_id: optional_executed_text(
                    batch,
                    &fields.producer_id,
                    row,
                    "family_closure",
                )?,
                authority_id: optional_executed_text(
                    batch,
                    &fields.authority_id,
                    row,
                    "family_closure",
                )?,
                algorithm_release: optional_executed_text(
                    batch,
                    &fields.algorithm_release,
                    row,
                    "family_closure",
                )?,
                precision_id: optional_executed_text(
                    batch,
                    &fields.precision_id,
                    row,
                    "family_closure",
                )?,
                input_pin: optional_executed_text(batch, &fields.input_pin, row, "family_closure")?,
                invalidation_pin: optional_executed_text(
                    batch,
                    &fields.invalidation_pin,
                    row,
                    "family_closure",
                )?,
                materialization_pin: optional_executed_text(
                    batch,
                    &fields.materialization_pin,
                    row,
                    "family_closure",
                )?,
                requested_unit_count: optional_executed_u64(
                    batch,
                    &fields.requested_unit_count,
                    row,
                    "family_closure",
                )?,
                completed_unit_count: optional_executed_u64(
                    batch,
                    &fields.completed_unit_count,
                    row,
                    "family_closure",
                )?,
                remainder_unit_count: optional_executed_u64(
                    batch,
                    &fields.remainder_unit_count,
                    row,
                    "family_closure",
                )?,
                unknown_unit_count: optional_executed_u64(
                    batch,
                    &fields.unknown_unit_count,
                    row,
                    "family_closure",
                )?,
                completeness_proof_pin: optional_executed_text(
                    batch,
                    &fields.completeness_proof_pin,
                    row,
                    "family_closure",
                )?,
                producer_proof_pin: optional_executed_text(
                    batch,
                    &fields.producer_proof_pin,
                    row,
                    "family_closure",
                )?,
                unsupported_remainder_id: optional_executed_text(
                    batch,
                    &fields.unsupported_remainder_id,
                    row,
                    "family_closure",
                )?,
                unsupported_reason_id: optional_executed_text(
                    batch,
                    &fields.unsupported_reason_id,
                    row,
                    "family_closure",
                )?,
                unsupported_proof_pin: optional_executed_text(
                    batch,
                    &fields.unsupported_proof_pin,
                    row,
                    "family_closure",
                )?,
            });
        }
    }
    Ok(rows)
}

fn decode_query_requirement_rows(
    batches: &[RecordBatch],
    fields: &QueryRequirementClosureFields,
) -> Result<Vec<ReleaseQueryRequirementClosureRow>, DerivedProducerClosureError> {
    let mut rows = Vec::new();
    for batch in batches {
        for row in 0..batch.num_rows() {
            rows.push(ReleaseQueryRequirementClosureRow {
                query_family_id: required_executed_text(
                    batch,
                    &fields.query_family_id,
                    row,
                    "query_requirement_closure",
                )?,
                required_family_id: required_executed_text(
                    batch,
                    &fields.required_family_id,
                    row,
                    "query_requirement_closure",
                )?,
                minimum_depth: required_executed_u32(
                    batch,
                    &fields.minimum_depth,
                    row,
                    "query_requirement_closure",
                )?,
                requirement_state: required_executed_text(
                    batch,
                    &fields.requirement_state,
                    row,
                    "query_requirement_closure",
                )?,
                unknown_cause: optional_executed_text(
                    batch,
                    &fields.unknown_cause,
                    row,
                    "query_requirement_closure",
                )?,
            });
        }
    }
    Ok(rows)
}

fn decode_violation_rows(
    batches: &[RecordBatch],
    fields: &ProducerClosureViolationFields,
) -> Result<Vec<ReleaseProducerClosureViolationRow>, DerivedProducerClosureError> {
    let mut rows = Vec::new();
    for batch in batches {
        for row in 0..batch.num_rows() {
            rows.push(ReleaseProducerClosureViolationRow {
                subject_kind: required_executed_text(
                    batch,
                    &fields.subject_kind,
                    row,
                    "producer_closure_violation",
                )?,
                subject_id: required_executed_text(
                    batch,
                    &fields.subject_id,
                    row,
                    "producer_closure_violation",
                )?,
                violation_code: required_executed_text(
                    batch,
                    &fields.violation_code,
                    row,
                    "producer_closure_violation",
                )?,
                related_id: optional_executed_text(
                    batch,
                    &fields.related_id,
                    row,
                    "producer_closure_violation",
                )?,
            });
        }
    }
    Ok(rows)
}

fn executed_string_column<'a>(
    batch: &'a RecordBatch,
    field: &FieldId,
    relation: &'static str,
) -> Result<&'a StringArray, DerivedProducerClosureError> {
    let column = batch.column_by_name(field.as_str()).ok_or_else(|| {
        DerivedProducerClosureError::ExecutedColumnMissing {
            relation,
            field: field.as_str().to_owned(),
        }
    })?;
    column
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| DerivedProducerClosureError::ExecutedColumnType {
            relation,
            field: field.as_str().to_owned(),
            expected: DataType::Utf8,
            actual: column.data_type().clone(),
        })
}

fn required_executed_text(
    batch: &RecordBatch,
    field: &FieldId,
    row: usize,
    relation: &'static str,
) -> Result<Arc<str>, DerivedProducerClosureError> {
    optional_executed_text(batch, field, row, relation)?.ok_or_else(|| {
        DerivedProducerClosureError::ExecutedRequiredValueNull {
            relation,
            field: field.as_str().to_owned(),
            row,
        }
    })
}

fn optional_executed_text(
    batch: &RecordBatch,
    field: &FieldId,
    row: usize,
    relation: &'static str,
) -> Result<Option<Arc<str>>, DerivedProducerClosureError> {
    let values = executed_string_column(batch, field, relation)?;
    if values.is_null(row) {
        return Ok(None);
    }
    let value = values.value(row);
    if value.is_empty()
        || value.len() > 4_096
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(DerivedProducerClosureError::ExecutedInvalidText {
            relation,
            field: field.as_str().to_owned(),
            row,
            value: value.to_owned(),
        });
    }
    Ok(Some(Arc::from(value)))
}

fn optional_executed_u64(
    batch: &RecordBatch,
    field: &FieldId,
    row: usize,
    relation: &'static str,
) -> Result<Option<u64>, DerivedProducerClosureError> {
    let column = batch.column_by_name(field.as_str()).ok_or_else(|| {
        DerivedProducerClosureError::ExecutedColumnMissing {
            relation,
            field: field.as_str().to_owned(),
        }
    })?;
    let values = column
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| DerivedProducerClosureError::ExecutedColumnType {
            relation,
            field: field.as_str().to_owned(),
            expected: DataType::UInt64,
            actual: column.data_type().clone(),
        })?;
    Ok((!values.is_null(row)).then(|| values.value(row)))
}

fn required_executed_u32(
    batch: &RecordBatch,
    field: &FieldId,
    row: usize,
    relation: &'static str,
) -> Result<u32, DerivedProducerClosureError> {
    let column = batch.column_by_name(field.as_str()).ok_or_else(|| {
        DerivedProducerClosureError::ExecutedColumnMissing {
            relation,
            field: field.as_str().to_owned(),
        }
    })?;
    let values = column
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| DerivedProducerClosureError::ExecutedColumnType {
            relation,
            field: field.as_str().to_owned(),
            expected: DataType::UInt32,
            actual: column.data_type().clone(),
        })?;
    if values.is_null(row) {
        return Err(DerivedProducerClosureError::ExecutedRequiredValueNull {
            relation,
            field: field.as_str().to_owned(),
            row,
        });
    }
    Ok(values.value(row))
}

fn validate_decoded_families(
    families: &[ReleaseProducerFamilyClosureRow],
    identities: &ProducerClosureSemanticIdentities,
    issues: &mut Vec<ReleaseProducerClosureIssue>,
) {
    if families.is_empty() {
        issues.push(ReleaseProducerClosureIssue::new(
            "empty_accepted_family_closure",
            None,
            None,
        ));
        return;
    }
    for pair in families.windows(2) {
        if pair[0].family_id == pair[1].family_id {
            issues.push(ReleaseProducerClosureIssue::new(
                "duplicate_decoded_family_closure",
                Some(Arc::clone(&pair[0].family_id)),
                None,
            ));
        }
    }
    for family in families {
        if family.semantic_class_id != *identities.factual_semantic_class_id() {
            issues.push(ReleaseProducerClosureIssue::new(
                "decoded_family_semantic_class_mismatch",
                Some(Arc::clone(&family.family_id)),
                Some(Arc::clone(&family.semantic_class_id)),
            ));
        }
        if family.authority_id.as_ref() != Some(identities.application_owned_authority_id()) {
            issues.push(ReleaseProducerClosureIssue::new(
                "decoded_family_authority_mismatch",
                Some(Arc::clone(&family.family_id)),
                family.authority_id.clone(),
            ));
        }

        let producer_binding_complete = family.producer_id.is_some()
            && family.algorithm_release.is_some()
            && family.precision_id.is_some()
            && family.input_pin.is_some()
            && family.invalidation_pin.is_some()
            && family.materialization_pin.is_some()
            && family.requested_unit_count.is_some()
            && family.completed_unit_count.is_some()
            && family.remainder_unit_count.is_some()
            && family.unknown_unit_count.is_some()
            && family.completeness_proof_pin.is_some()
            && family.producer_proof_pin.is_some();
        let producer_binding_present = family.producer_id.is_some()
            || family.algorithm_release.is_some()
            || family.precision_id.is_some()
            || family.input_pin.is_some()
            || family.invalidation_pin.is_some()
            || family.materialization_pin.is_some()
            || family.requested_unit_count.is_some()
            || family.completed_unit_count.is_some()
            || family.remainder_unit_count.is_some()
            || family.unknown_unit_count.is_some()
            || family.completeness_proof_pin.is_some()
            || family.producer_proof_pin.is_some();
        let remainder_binding_complete = family.unsupported_remainder_id.is_some()
            && family.unsupported_reason_id.is_some()
            && family.unsupported_proof_pin.is_some();
        let remainder_binding_present = family.unsupported_remainder_id.is_some()
            || family.unsupported_reason_id.is_some()
            || family.unsupported_proof_pin.is_some();

        match family.closure_state.as_ref() {
            STATE_SUPPORTED => {
                if !producer_binding_complete || remainder_binding_present {
                    issues.push(ReleaseProducerClosureIssue::new(
                        "decoded_supported_family_binding_mismatch",
                        Some(Arc::clone(&family.family_id)),
                        None,
                    ));
                }
                if family.requested_unit_count == Some(0) {
                    issues.push(ReleaseProducerClosureIssue::new(
                        "empty_runtime_producer_scope",
                        Some(Arc::clone(&family.family_id)),
                        family.producer_id.clone(),
                    ));
                }
                if family.requested_unit_count != family.completed_unit_count
                    || family.remainder_unit_count != Some(0)
                    || family.unknown_unit_count != Some(0)
                {
                    issues.push(ReleaseProducerClosureIssue::new(
                        "decoded_runtime_producer_incomplete",
                        Some(Arc::clone(&family.family_id)),
                        family.producer_id.clone(),
                    ));
                }
            }
            STATE_UNSUPPORTED => {
                if producer_binding_present || !remainder_binding_complete {
                    issues.push(ReleaseProducerClosureIssue::new(
                        "decoded_unsupported_family_binding_mismatch",
                        Some(Arc::clone(&family.family_id)),
                        family.unsupported_remainder_id.clone(),
                    ));
                }
            }
            STATE_UNKNOWN | STATE_INVALID => {
                issues.push(ReleaseProducerClosureIssue::new(
                    "decoded_family_not_closed",
                    Some(Arc::clone(&family.family_id)),
                    Some(Arc::clone(&family.closure_state)),
                ));
            }
            _ => {
                issues.push(ReleaseProducerClosureIssue::new(
                    "decoded_family_state_unknown",
                    Some(Arc::clone(&family.family_id)),
                    Some(Arc::clone(&family.closure_state)),
                ));
            }
        }
    }
}

fn validate_decoded_queries(
    queries: &[ReleaseQueryRequirementClosureRow],
    families: &[ReleaseProducerFamilyClosureRow],
    issues: &mut Vec<ReleaseProducerClosureIssue>,
) {
    if queries.is_empty() {
        issues.push(ReleaseProducerClosureIssue::new(
            "empty_query_requirement_closure",
            None,
            None,
        ));
        return;
    }
    for pair in queries.windows(2) {
        if pair[0].query_family_id == pair[1].query_family_id
            && pair[0].required_family_id == pair[1].required_family_id
        {
            issues.push(ReleaseProducerClosureIssue::new(
                "duplicate_decoded_query_requirement",
                Some(Arc::clone(&pair[0].query_family_id)),
                Some(Arc::clone(&pair[0].required_family_id)),
            ));
        }
    }
    let family_by_id = families
        .iter()
        .map(|family| (family.family_id.as_ref(), family))
        .collect::<BTreeMap<_, _>>();
    for query in queries {
        if query.minimum_depth == 0 {
            issues.push(ReleaseProducerClosureIssue::new(
                "zero_query_requirement_depth",
                Some(Arc::clone(&query.query_family_id)),
                Some(Arc::clone(&query.required_family_id)),
            ));
        }
        let family = family_by_id.get(query.required_family_id.as_ref()).copied();
        let state_matches = match (query.requirement_state.as_ref(), family) {
            (STATE_SATISFIED, Some(family)) => {
                family.closure_state.as_ref() == STATE_SUPPORTED && query.unknown_cause.is_none()
            }
            (STATE_UNSUPPORTED, Some(family)) => {
                family.closure_state.as_ref() == STATE_UNSUPPORTED
                    && query.unknown_cause.as_ref() == family.unsupported_reason_id.as_ref()
            }
            _ => false,
        };
        if !state_matches {
            issues.push(ReleaseProducerClosureIssue::new(
                "decoded_query_requirement_binding_mismatch",
                Some(Arc::clone(&query.query_family_id)),
                Some(Arc::clone(&query.required_family_id)),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_compilation_observation(
    observation: &ProducerClosureCompilationObservation,
    family_fields: &FamilyClosureFields,
    query_fields: &QueryRequirementClosureFields,
    violation_fields: &ProducerClosureViolationFields,
    input_fields: &[FieldId],
    identities: &ProducerClosureSemanticIdentities,
    implementation_release: &Arc<str>,
    issues: &mut Vec<ReleaseProducerClosureIssue>,
) -> Result<(), DerivedProducerClosureError> {
    let dependencies = observation.dependencies();
    let mut required = vec![
        ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::clone(
            identities.application_owned_authority_id(),
        )),
        ProducerClosureCompilationDependency::FactualSemanticClass(Arc::clone(
            identities.factual_semantic_class_id(),
        )),
        ProducerClosureCompilationDependency::ImplementationRelease(Arc::clone(
            implementation_release,
        )),
    ];
    for relation in [
        ACCEPTED_FACT_FAMILY_RELATION_ID,
        RUNTIME_PRODUCER_RELATION_ID,
        QUERY_FAMILY_REQUIREMENT_RELATION_ID,
        UNSUPPORTED_REMAINDER_RELATION_ID,
    ] {
        required.push(ProducerClosureCompilationDependency::InputRelation(
            compiled_relation_id(relation)?,
        ));
    }
    required.extend(
        input_fields
            .iter()
            .cloned()
            .map(ProducerClosureCompilationDependency::InputField),
    );
    for relation in [
        FAMILY_CLOSURE_RELATION_ID,
        QUERY_REQUIREMENT_CLOSURE_RELATION_ID,
        PRODUCER_CLOSURE_VIOLATION_RELATION_ID,
    ] {
        required.push(ProducerClosureCompilationDependency::OutputRelation(
            compiled_relation_id(relation)?,
        ));
    }
    let output_fields = [
        &family_fields.family_id,
        &family_fields.semantic_class_id,
        &family_fields.closure_state,
        &family_fields.producer_id,
        &family_fields.authority_id,
        &family_fields.algorithm_release,
        &family_fields.precision_id,
        &family_fields.input_pin,
        &family_fields.invalidation_pin,
        &family_fields.materialization_pin,
        &family_fields.requested_unit_count,
        &family_fields.completed_unit_count,
        &family_fields.remainder_unit_count,
        &family_fields.unknown_unit_count,
        &family_fields.completeness_proof_pin,
        &family_fields.producer_proof_pin,
        &family_fields.unsupported_remainder_id,
        &family_fields.unsupported_reason_id,
        &family_fields.unsupported_proof_pin,
        &query_fields.query_family_id,
        &query_fields.required_family_id,
        &query_fields.minimum_depth,
        &query_fields.requirement_state,
        &query_fields.unknown_cause,
        &violation_fields.subject_kind,
        &violation_fields.subject_id,
        &violation_fields.violation_code,
        &violation_fields.related_id,
    ];
    required.extend(
        output_fields
            .into_iter()
            .cloned()
            .map(ProducerClosureCompilationDependency::OutputField),
    );
    for dependency in required {
        if !dependencies.contains(&dependency) {
            issues.push(ReleaseProducerClosureIssue::new(
                "missing_compiled_release_dependency",
                None,
                Some(compilation_dependency_identity(&dependency)),
            ));
        }
    }
    for operator in [
        ProducerClosureNativeOperator::Projection,
        ProducerClosureNativeOperator::Aggregate,
        ProducerClosureNativeOperator::LeftJoin,
        ProducerClosureNativeOperator::LeftAntiJoin,
        ProducerClosureNativeOperator::Filter,
        ProducerClosureNativeOperator::RecursiveQueryDistinct,
        ProducerClosureNativeOperator::UnionAll,
        ProducerClosureNativeOperator::DeterministicSort,
        ProducerClosureNativeOperator::OutputOverflowProbeLimit,
    ] {
        if !observation.operators().contains(&operator) {
            issues.push(ReleaseProducerClosureIssue::new(
                "missing_typed_native_operator_observation",
                None,
                Some(Arc::from(native_operator_identity(operator))),
            ));
        }
    }
    Ok(())
}

fn compilation_dependency_identity(dependency: &ProducerClosureCompilationDependency) -> Arc<str> {
    match dependency {
        ProducerClosureCompilationDependency::InputRelation(value) => {
            Arc::from(format!("input-relation:{}", value.as_str()))
        }
        ProducerClosureCompilationDependency::InputField(value) => {
            Arc::from(format!("input-field:{}", value.as_str()))
        }
        ProducerClosureCompilationDependency::OutputRelation(value) => {
            Arc::from(format!("output-relation:{}", value.as_str()))
        }
        ProducerClosureCompilationDependency::OutputField(value) => {
            Arc::from(format!("output-field:{}", value.as_str()))
        }
        ProducerClosureCompilationDependency::ApplicationOwnedAuthority(value) => {
            Arc::from(format!("application-authority:{value}"))
        }
        ProducerClosureCompilationDependency::FactualSemanticClass(value) => {
            Arc::from(format!("factual-semantic-class:{value}"))
        }
        ProducerClosureCompilationDependency::ImplementationRelease(value) => {
            Arc::from(format!("implementation-release:{value}"))
        }
        ProducerClosureCompilationDependency::SessionMemoryPool => Arc::from("session-memory-pool"),
        ProducerClosureCompilationDependency::DataFusionExecuteStreamDropAbort => {
            Arc::from("datafusion-execute-stream-drop-abort")
        }
    }
}

const fn native_operator_identity(operator: ProducerClosureNativeOperator) -> &'static str {
    match operator {
        ProducerClosureNativeOperator::Projection => "projection",
        ProducerClosureNativeOperator::Aggregate => "aggregate",
        ProducerClosureNativeOperator::LeftJoin => "left-join",
        ProducerClosureNativeOperator::LeftAntiJoin => "left-anti-join",
        ProducerClosureNativeOperator::Filter => "filter",
        ProducerClosureNativeOperator::RecursiveQueryDistinct => "recursive-query-distinct",
        ProducerClosureNativeOperator::UnionAll => "union-all",
        ProducerClosureNativeOperator::DeterministicSort => "deterministic-sort",
        ProducerClosureNativeOperator::OutputOverflowProbeLimit => "output-overflow-probe-limit",
    }
}

#[derive(Default)]
struct ExecutionBudget {
    batches: usize,
    bytes: usize,
}

async fn execute_bounded(
    context: &SessionContext,
    plan: &LogicalPlan,
    expected_schema: &SchemaRef,
    bounds: ProducerClosureResourceBounds,
    relation: &'static str,
    budget: &mut ExecutionBudget,
    cancellation: &ProducerClosureCancellation,
) -> Result<Vec<RecordBatch>, DerivedProducerClosureError> {
    if cancellation.is_cancelled() {
        return Err(DerivedProducerClosureError::Cancelled { relation });
    }
    let optimized = context.state().optimize(plan)?;
    let physical = context.state().create_physical_plan(&optimized).await?;
    let mut stream = execute_stream(physical, context.task_ctx())?;
    let mut batches = Vec::new();
    let mut relation_rows = 0_usize;
    loop {
        let next = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                return Err(DerivedProducerClosureError::Cancelled { relation });
            }
            batch = stream.next() => batch,
        };
        let Some(batch) = next else {
            break;
        };
        let batch = batch?;
        if batch.schema_ref().as_ref() != expected_schema.as_ref() {
            return Err(DerivedProducerClosureError::ExecutedSchemaMismatch {
                relation,
                expected: Arc::clone(expected_schema),
                actual: batch.schema(),
            });
        }
        relation_rows = relation_rows
            .checked_add(batch.num_rows())
            .ok_or(DerivedProducerClosureError::ResourceCounterOverflow("rows"))?;
        budget.batches = budget.batches.checked_add(1).ok_or(
            DerivedProducerClosureError::ResourceCounterOverflow("batches"),
        )?;
        budget.bytes = budget
            .bytes
            .checked_add(batch.get_array_memory_size())
            .ok_or(DerivedProducerClosureError::ResourceCounterOverflow(
                "bytes",
            ))?;
        if relation_rows > bounds.max_rows_per_relation() {
            return Err(DerivedProducerClosureError::OutputRowsExceeded {
                relation,
                limit: bounds.max_rows_per_relation(),
                observed: relation_rows,
            });
        }
        if budget.batches > bounds.max_total_batches() {
            return Err(DerivedProducerClosureError::OutputBatchesExceeded {
                limit: bounds.max_total_batches(),
                observed: budget.batches,
            });
        }
        if budget.bytes > bounds.max_total_bytes() {
            return Err(DerivedProducerClosureError::OutputBytesExceeded {
                limit: bounds.max_total_bytes(),
                observed: budget.bytes,
            });
        }
        batches.push(batch);
    }
    drop(stream);
    if batches.is_empty() {
        batches.push(RecordBatch::new_empty(Arc::clone(expected_schema)));
    }
    Ok(batches)
}

/// Fail-closed release-catalog construction and registration errors.
#[derive(Debug, Error)]
pub(crate) enum ReleaseProducerClosureCatalogError {
    #[error("release producer closure has no accepted families")]
    EmptyAcceptedFamilies,
    #[error("release producer closure has no query-family requirements")]
    EmptyQueryRequirements,
    #[error("release producer closure repeats {kind} {value:?}")]
    DuplicateFamily { kind: &'static str, value: String },
    #[error(
        "release producer closure producer/remainder dispositions are not exact and exhaustive"
    )]
    DispositionClosure,
    #[error("release runtime producer for {0:?} is incomplete")]
    IncompleteRuntimeProducer(Arc<str>),
    #[error("release query requirement references unknown family {0:?}")]
    UnknownQueryFamily(Arc<str>),
    #[error("release query requirement repeats an exact edge")]
    DuplicateQueryEdge,
    #[error("release producer-closure {kind} has invalid text {value:?}")]
    InvalidText { kind: &'static str, value: String },
    #[error(transparent)]
    Epoch(#[from] ProgrammaticFabricEpochError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

/// Fail-closed binding, planning, execution, and resource errors.
#[derive(Debug, Error)]
pub enum DerivedProducerClosureError {
    #[error("invalid compiled {kind} identity {value:?}: {detail}")]
    InvalidCompiledIdentity {
        kind: &'static str,
        value: &'static str,
        detail: String,
    },
    #[error("compiled producer-closure input relation {relation} is absent from the sealed epoch")]
    MissingReleaseInputRelation { relation: &'static str },
    #[error("cannot resolve compiled producer-closure input relation {relation}: {detail}")]
    ReleaseInputResolution {
        relation: &'static str,
        detail: String,
    },
    #[error(
        "sealed producer-closure relation identity differs: expected {expected:?}, observed {actual:?}"
    )]
    ReleaseInputRelationIdentity {
        expected: &'static str,
        actual: String,
    },
    #[error(
        "sealed producer-closure relation {relation} field {ordinal} identity differs: expected {expected:?}, observed {actual:?}"
    )]
    ReleaseInputFieldIdentity {
        relation: &'static str,
        ordinal: usize,
        expected: String,
        actual: String,
    },
    #[error("invalid {kind} identity {value:?}")]
    InvalidText { kind: &'static str, value: String },
    #[error("runtime relation identity {0:?} is bound more than once")]
    DuplicateRelationId(String),
    #[error("{relation} binds field identity {field:?} more than once")]
    DuplicateFieldId {
        relation: &'static str,
        field: String,
    },
    #[error("{relation} schema has {actual} fields; expected exactly {expected}")]
    SchemaFieldCount {
        relation: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error(
        "{relation} field {ordinal} mismatch: expected {expected_name:?} {expected_type:?} nullable={expected_nullable}, observed {actual_name:?} {actual_type:?} nullable={actual_nullable}"
    )]
    SchemaFieldMismatch {
        relation: &'static str,
        ordinal: usize,
        expected_name: String,
        expected_type: DataType,
        expected_nullable: bool,
        actual_name: String,
        actual_type: DataType,
        actual_nullable: bool,
    },
    #[error("{role} relation mismatch: expected {expected:?}, observed {actual:?}")]
    InputRelationMismatch {
        role: &'static str,
        expected: String,
        actual: String,
    },
    #[error("{role} input schema differs from the installed application binding")]
    InputSchemaMismatch {
        role: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("compiled {role} schema differs from the installed application binding")]
    CompiledSchemaMismatch {
        role: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("executed {relation} schema differs from the compiled contract")]
    ExecutedSchemaMismatch {
        relation: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("executed {relation} relation is missing compiled field {field:?}")]
    ExecutedColumnMissing {
        relation: &'static str,
        field: String,
    },
    #[error("executed {relation} field {field:?} has type {actual:?}; expected {expected:?}")]
    ExecutedColumnType {
        relation: &'static str,
        field: String,
        expected: DataType,
        actual: DataType,
    },
    #[error("executed {relation} field {field:?} is null at row {row}")]
    ExecutedRequiredValueNull {
        relation: &'static str,
        field: String,
        row: usize,
    },
    #[error("executed {relation} field {field:?} has invalid text {value:?} at row {row}")]
    ExecutedInvalidText {
        relation: &'static str,
        field: String,
        row: usize,
        value: String,
    },
    #[error("resource bound {0} must be non-zero")]
    ZeroResourceBound(&'static str),
    #[error("output-row bound cannot reserve an overflow-probe row")]
    ResourceProbeOverflow,
    #[error("{relation} output rows exceeded {limit}: observed at least {observed}")]
    OutputRowsExceeded {
        relation: &'static str,
        limit: usize,
        observed: usize,
    },
    #[error("total output batches exceeded {limit}: observed at least {observed}")]
    OutputBatchesExceeded { limit: usize, observed: usize },
    #[error("total output bytes exceeded {limit}: observed at least {observed}")]
    OutputBytesExceeded { limit: usize, observed: usize },
    #[error("resource counter overflowed for {0}")]
    ResourceCounterOverflow(&'static str),
    #[error("release producer-closure execution was cancelled while reading {relation}")]
    Cancelled { relation: &'static str },
    #[error("internal producer-closure compiler constructed no violation branches")]
    InternalNoViolationBranches,
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{Array, StringArray, UInt32Array, UInt64Array};
    use arrow_schema::{Field, Schema};
    use datafusion::datasource::MemTable;

    use super::*;
    use crate::fabric::epoch_runtime::{
        FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole,
    };
    use crate::fabric::production_kernel::CompiledSemanticRelease;
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };
    use crate::fabric::programmatic_schema::ProviderInput;
    use crate::fabric::proof::{
        ProofTerminalStatus, ReleaseProducerClosureProofInput, evaluate_release_producer_closure,
    };
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
    };

    const APP_AUTHORITY: &str = "authority.application-derived.v2";
    const PROVIDER_AUTHORITY: &str = "authority.provider-native.v2";
    const FACT_CLASS: &str = "semantic.fact.v2";
    const JUDGMENT_CLASS: &str = "semantic.judgment.v2";

    fn relation_id(value: &str) -> RelationId {
        RelationId::new(value).expect("relation ID")
    }

    fn field_id(value: &str) -> FieldId {
        FieldId::new(value).expect("field ID")
    }

    fn utf8_field(name: &str, nullable: bool) -> Field {
        Field::new(name, DataType::Utf8, nullable)
    }

    fn schema(fields: Vec<Field>) -> SchemaRef {
        Arc::new(Schema::new(fields))
    }

    fn bindings() -> DerivedProducerClosureBindings {
        let accepted_fields = AcceptedFactFamilyFields {
            family_id: field_id("accepted_family_id"),
            semantic_class_id: field_id("accepted_semantic_class_id"),
        };
        let producer_fields = RuntimeProducerFields {
            family_id: field_id("producer_family_id"),
            producer_id: field_id("runtime_producer_id"),
            authority_id: field_id("runtime_authority_id"),
            algorithm_release: field_id("algorithm_release_pin"),
            precision_id: field_id("precision_profile_id"),
            input_pin: field_id("producer_input_pin"),
            invalidation_pin: field_id("invalidation_policy_pin"),
            materialization_pin: field_id("materialization_policy_pin"),
            requested_unit_count: field_id("producer_requested_unit_count"),
            completed_unit_count: field_id("producer_completed_unit_count"),
            remainder_unit_count: field_id("producer_remainder_unit_count"),
            unknown_unit_count: field_id("producer_unknown_unit_count"),
            completeness_proof_pin: field_id("producer_completeness_proof_pin"),
            proof_pin: field_id("producer_execution_proof_pin"),
        };
        let query_fields = QueryFamilyRequirementFields {
            query_family_id: field_id("query_family_id"),
            required_family_id: field_id("query_required_family_id"),
        };
        let remainder_fields = UnsupportedRemainderFields {
            family_id: field_id("remainder_family_id"),
            remainder_id: field_id("unsupported_remainder_id"),
            authority_id: field_id("remainder_authority_id"),
            reason_id: field_id("unsupported_reason_id"),
            proof_pin: field_id("remainder_proof_pin"),
        };
        let family_output_fields = FamilyClosureFields {
            family_id: field_id("closed_family_id"),
            semantic_class_id: field_id("closed_semantic_class_id"),
            closure_state: field_id("family_closure_state"),
            producer_id: field_id("closed_producer_id"),
            authority_id: field_id("closed_authority_id"),
            algorithm_release: field_id("closed_algorithm_release"),
            precision_id: field_id("closed_precision_id"),
            input_pin: field_id("closed_input_pin"),
            invalidation_pin: field_id("closed_invalidation_pin"),
            materialization_pin: field_id("closed_materialization_pin"),
            requested_unit_count: field_id("closed_requested_unit_count"),
            completed_unit_count: field_id("closed_completed_unit_count"),
            remainder_unit_count: field_id("closed_remainder_unit_count"),
            unknown_unit_count: field_id("closed_unknown_unit_count"),
            completeness_proof_pin: field_id("closed_completeness_proof_pin"),
            producer_proof_pin: field_id("closed_producer_proof_pin"),
            unsupported_remainder_id: field_id("closed_unsupported_remainder_id"),
            unsupported_reason_id: field_id("closed_unsupported_reason_id"),
            unsupported_proof_pin: field_id("closed_unsupported_proof_pin"),
        };
        let query_output_fields = QueryRequirementClosureFields {
            query_family_id: field_id("closed_query_family_id"),
            required_family_id: field_id("closed_query_required_family_id"),
            minimum_depth: field_id("query_requirement_minimum_depth"),
            requirement_state: field_id("query_requirement_state"),
            unknown_cause: field_id("query_requirement_unknown_cause"),
        };
        let violation_fields = ProducerClosureViolationFields {
            subject_kind: field_id("violation_subject_kind"),
            subject_id: field_id("violation_subject_id"),
            violation_code: field_id("producer_closure_violation_code"),
            related_id: field_id("violation_related_id"),
        };

        let accepted = ProducerClosureRelationContract::new(
            relation_id("runtime.accepted_fact_family"),
            schema(vec![
                utf8_field(accepted_fields.family_id.as_str(), false),
                utf8_field(accepted_fields.semantic_class_id.as_str(), false),
            ]),
            accepted_fields,
        );
        let producer = ProducerClosureRelationContract::new(
            relation_id("runtime.derived_producer"),
            schema(vec![
                utf8_field(producer_fields.family_id.as_str(), false),
                utf8_field(producer_fields.producer_id.as_str(), false),
                utf8_field(producer_fields.authority_id.as_str(), false),
                utf8_field(producer_fields.algorithm_release.as_str(), false),
                utf8_field(producer_fields.precision_id.as_str(), false),
                utf8_field(producer_fields.input_pin.as_str(), false),
                utf8_field(producer_fields.invalidation_pin.as_str(), false),
                utf8_field(producer_fields.materialization_pin.as_str(), false),
                Field::new(
                    producer_fields.requested_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.completed_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.remainder_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.unknown_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                utf8_field(producer_fields.completeness_proof_pin.as_str(), false),
                utf8_field(producer_fields.proof_pin.as_str(), false),
            ]),
            producer_fields,
        );
        let query = ProducerClosureRelationContract::new(
            relation_id("runtime.query_family_requirement"),
            schema(vec![
                utf8_field(query_fields.query_family_id.as_str(), false),
                utf8_field(query_fields.required_family_id.as_str(), false),
            ]),
            query_fields,
        );
        let remainder = ProducerClosureRelationContract::new(
            relation_id("runtime.unsupported_remainder"),
            schema(vec![
                utf8_field(remainder_fields.family_id.as_str(), false),
                utf8_field(remainder_fields.remainder_id.as_str(), false),
                utf8_field(remainder_fields.authority_id.as_str(), false),
                utf8_field(remainder_fields.reason_id.as_str(), false),
                utf8_field(remainder_fields.proof_pin.as_str(), false),
            ]),
            remainder_fields,
        );
        let family_output = ProducerClosureRelationContract::new(
            relation_id("derived.accepted_family_producer_closure"),
            schema(vec![
                utf8_field(family_output_fields.family_id.as_str(), false),
                utf8_field(family_output_fields.semantic_class_id.as_str(), false),
                utf8_field(family_output_fields.closure_state.as_str(), false),
                utf8_field(family_output_fields.producer_id.as_str(), true),
                utf8_field(family_output_fields.authority_id.as_str(), true),
                utf8_field(family_output_fields.algorithm_release.as_str(), true),
                utf8_field(family_output_fields.precision_id.as_str(), true),
                utf8_field(family_output_fields.input_pin.as_str(), true),
                utf8_field(family_output_fields.invalidation_pin.as_str(), true),
                utf8_field(family_output_fields.materialization_pin.as_str(), true),
                Field::new(
                    family_output_fields.requested_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.completed_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.remainder_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.unknown_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                utf8_field(family_output_fields.completeness_proof_pin.as_str(), true),
                utf8_field(family_output_fields.producer_proof_pin.as_str(), true),
                utf8_field(family_output_fields.unsupported_remainder_id.as_str(), true),
                utf8_field(family_output_fields.unsupported_reason_id.as_str(), true),
                utf8_field(family_output_fields.unsupported_proof_pin.as_str(), true),
            ]),
            family_output_fields,
        );
        let query_output = ProducerClosureRelationContract::new(
            relation_id("derived.query_family_requirement_closure"),
            schema(vec![
                utf8_field(query_output_fields.query_family_id.as_str(), false),
                utf8_field(query_output_fields.required_family_id.as_str(), false),
                Field::new(
                    query_output_fields.minimum_depth.as_str(),
                    DataType::UInt32,
                    false,
                ),
                utf8_field(query_output_fields.requirement_state.as_str(), false),
                utf8_field(query_output_fields.unknown_cause.as_str(), true),
            ]),
            query_output_fields,
        );
        let violation = ProducerClosureRelationContract::new(
            relation_id("proof.derived_producer_closure_violation"),
            schema(vec![
                utf8_field(violation_fields.subject_kind.as_str(), false),
                utf8_field(violation_fields.subject_id.as_str(), false),
                utf8_field(violation_fields.violation_code.as_str(), false),
                utf8_field(violation_fields.related_id.as_str(), true),
            ]),
            violation_fields,
        );

        DerivedProducerClosureBindings::try_new(
            "operation.derived-producer-closure.v2",
            "derived-producer-closure@1.0.0",
            ProducerClosureSemanticIdentities::try_new(APP_AUTHORITY, FACT_CLASS)
                .expect("semantic identities"),
            accepted,
            producer,
            query,
            remainder,
            family_output,
            query_output,
            violation,
        )
        .expect("closure bindings")
    }

    async fn sealed_release_epoch(
        accepted_relation: Option<&str>,
        drift_accepted_field: bool,
    ) -> ProgrammaticFabricEpoch {
        let compiled = bindings();
        let mut relations = vec![
            (
                RUNTIME_PRODUCER_RELATION_ID,
                Arc::clone(&compiled.runtime_producer.schema),
            ),
            (
                QUERY_FAMILY_REQUIREMENT_RELATION_ID,
                Arc::clone(&compiled.query_family_requirement.schema),
            ),
            (
                UNSUPPORTED_REMAINDER_RELATION_ID,
                Arc::clone(&compiled.unsupported_remainder.schema),
            ),
        ];
        if let Some(relation_id) = accepted_relation {
            relations.push((
                relation_id,
                Arc::clone(&compiled.accepted_fact_family.schema),
            ));
        }

        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([0xD3; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("release closure epoch builder");
        for (relation_id, expected_schema) in relations {
            let fields = expected_schema
                .fields()
                .iter()
                .enumerate()
                .map(|(ordinal, expected)| {
                    let name = if drift_accepted_field
                        && relation_id == ACCEPTED_FACT_FAMILY_RELATION_ID
                        && ordinal == 0
                    {
                        "alternate_family_id"
                    } else {
                        expected.name()
                    };
                    Field::new(name, expected.data_type().clone(), expected.is_nullable())
                        .with_metadata(HashMap::from([(
                            FIELD_ID_METADATA_KEY.to_owned(),
                            name.to_owned(),
                        )]))
                })
                .collect::<Vec<_>>();
            let schema = Arc::new(Schema::new_with_metadata(
                fields,
                HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
            ));
            let table_reference = TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::System.as_str(),
                relation_id.replace('.', "_"),
            );
            let contract = Arc::new(
                SchemaContract::try_new(
                    format!("release-closure:{relation_id}:v1"),
                    table_reference.clone(),
                    Arc::clone(&schema),
                    Arc::clone(&schema),
                    (0..schema.fields().len())
                        .map(|ordinal| FieldIndexMapping::direct(ordinal, ordinal))
                        .collect(),
                )
                .expect("exact release input contract"),
            );
            let provider = Arc::new(
                MemTable::try_new(
                    Arc::clone(&schema),
                    vec![vec![RecordBatch::new_empty(Arc::clone(&schema))]],
                )
                .expect("release input provider"),
            );
            builder
                .register_provider(ProviderInput::new(
                    ProgrammaticRelationId::new(relation_id),
                    table_reference,
                    contract,
                    provider,
                ))
                .expect("register release input");
        }
        builder.seal_for_test().await.expect("sealed release epoch")
    }

    #[tokio::test]
    async fn release_owned_compiler_resolves_exact_epoch_relations_and_fields() {
        let epoch = sealed_release_epoch(Some(ACCEPTED_FACT_FAMILY_RELATION_ID), false).await;
        let release = CompiledSemanticRelease::current();
        let compiled = compile_release_owned_derived_producer_closure(
            release.proof_authority(),
            &epoch,
            bounds(),
        )
        .await
        .expect("exact sealed relations compile without caller bindings");

        let dependencies = compiled.observation().dependencies();
        for relation in [
            ACCEPTED_FACT_FAMILY_RELATION_ID,
            RUNTIME_PRODUCER_RELATION_ID,
            QUERY_FAMILY_REQUIREMENT_RELATION_ID,
            UNSUPPORTED_REMAINDER_RELATION_ID,
        ] {
            assert!(
                dependencies.contains(&ProducerClosureCompilationDependency::InputRelation(
                    relation_id(relation),
                ))
            );
        }
        assert!(dependencies.contains(
            &ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::from(
                APPLICATION_DERIVED_AUTHORITY_ID,
            )),
        ));
    }

    #[tokio::test]
    async fn release_owned_compiler_rejects_alternate_missing_and_drifted_inputs() {
        let release = CompiledSemanticRelease::current();
        let alternate =
            sealed_release_epoch(Some("runtime.accepted_fact_family.alternate"), false).await;
        assert!(matches!(
            compile_release_owned_derived_producer_closure(
                release.proof_authority(),
                &alternate,
                bounds(),
            )
            .await,
            Err(DerivedProducerClosureError::MissingReleaseInputRelation {
                relation: ACCEPTED_FACT_FAMILY_RELATION_ID,
            })
        ));

        let missing = sealed_release_epoch(None, false).await;
        assert!(matches!(
            compile_release_owned_derived_producer_closure(
                release.proof_authority(),
                &missing,
                bounds(),
            )
            .await,
            Err(DerivedProducerClosureError::MissingReleaseInputRelation {
                relation: ACCEPTED_FACT_FAMILY_RELATION_ID,
            })
        ));

        let drifted = sealed_release_epoch(Some(ACCEPTED_FACT_FAMILY_RELATION_ID), true).await;
        assert!(matches!(
            compile_release_owned_derived_producer_closure(
                release.proof_authority(),
                &drifted,
                bounds(),
            )
            .await,
            Err(DerivedProducerClosureError::SchemaFieldMismatch {
                relation: "accepted_fact_family",
                ordinal: 0,
                ..
            })
        ));
    }

    fn relation_batch(schema: SchemaRef, rows: &[Vec<&str>]) -> RecordBatch {
        let columns = (0..schema.fields().len())
            .map(|column| match schema.field(column).data_type() {
                DataType::Utf8 => Arc::new(StringArray::from(
                    rows.iter().map(|row| row[column]).collect::<Vec<_>>(),
                )) as arrow_array::ArrayRef,
                DataType::UInt64 => Arc::new(UInt64Array::from(
                    rows.iter()
                        .map(|row| row[column].parse::<u64>().expect("u64 fixture"))
                        .collect::<Vec<_>>(),
                )),
                other => panic!("unsupported fixture type {other:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).expect("string relation")
    }

    fn relation_input<F>(
        contract: &ProducerClosureRelationContract<F>,
        batch: RecordBatch,
    ) -> ProducerClosureRelationInput {
        let provider = Arc::new(
            MemTable::try_new(Arc::clone(&contract.schema), vec![vec![batch]]).expect("MemTable"),
        );
        let plan = LogicalPlanBuilder::scan(
            contract.relation_id.as_str(),
            provider_as_source(provider),
            None,
        )
        .expect("scan")
        .build()
        .expect("input plan");
        ProducerClosureRelationInput::new(contract.relation_id.clone(), plan)
    }

    type ProducerRow<'a> = [&'a str; 14];
    type RemainderRow<'a> = [&'a str; 5];

    fn inputs(
        bindings: &DerivedProducerClosureBindings,
        accepted: &[(&str, &str)],
        producers: &[ProducerRow<'_>],
        queries: &[(&str, &str)],
        remainders: &[RemainderRow<'_>],
    ) -> DerivedProducerClosureInputs {
        let accepted_rows = accepted
            .iter()
            .map(|row| vec![row.0, row.1])
            .collect::<Vec<_>>();
        let producer_rows = producers.iter().map(|row| row.to_vec()).collect::<Vec<_>>();
        let query_rows = queries
            .iter()
            .map(|row| vec![row.0, row.1])
            .collect::<Vec<_>>();
        let remainder_rows = remainders
            .iter()
            .map(|row| row.to_vec())
            .collect::<Vec<_>>();
        DerivedProducerClosureInputs {
            accepted_fact_family: relation_input(
                &bindings.accepted_fact_family,
                relation_batch(
                    Arc::clone(&bindings.accepted_fact_family.schema),
                    &accepted_rows,
                ),
            ),
            runtime_producer: relation_input(
                &bindings.runtime_producer,
                relation_batch(
                    Arc::clone(&bindings.runtime_producer.schema),
                    &producer_rows,
                ),
            ),
            query_family_requirement: relation_input(
                &bindings.query_family_requirement,
                relation_batch(
                    Arc::clone(&bindings.query_family_requirement.schema),
                    &query_rows,
                ),
            ),
            unsupported_remainder: relation_input(
                &bindings.unsupported_remainder,
                relation_batch(
                    Arc::clone(&bindings.unsupported_remainder.schema),
                    &remainder_rows,
                ),
            ),
        }
    }

    fn producer<'a>(family: &'a str, producer: &'a str) -> ProducerRow<'a> {
        [
            family,
            producer,
            APP_AUTHORITY,
            "algorithm@1",
            "precision.sound-bounded",
            "input:b3:11",
            "invalidation:b3:22",
            "materialization:b3:33",
            "1",
            "1",
            "0",
            "0",
            "completeness-proof:b3:40",
            "proof:b3:44",
        ]
    }

    fn remainder(family: &str) -> RemainderRow<'_> {
        [
            family,
            "remainder.dynamic-dispatch",
            APP_AUTHORITY,
            "unknown.dynamic-dispatch",
            "proof:b3:55",
        ]
    }

    fn bounds() -> ProducerClosureResourceBounds {
        ProducerClosureResourceBounds::try_new(16, 4_096, 256, 16 * 1024 * 1024).expect("bounds")
    }

    async fn execute(
        bindings: &DerivedProducerClosureBindings,
        inputs: DerivedProducerClosureInputs,
    ) -> DerivedProducerClosureExecution {
        let release = super::super::production_kernel::CompiledSemanticRelease::current();
        let compiled =
            compile_derived_producer_closure(release.proof_authority(), inputs, bindings, bounds())
                .expect("compile closure");
        compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute closure")
    }

    fn string_values(batches: &[RecordBatch], field: &str) -> Vec<Option<String>> {
        let mut values = Vec::new();
        for batch in batches {
            let column = batch
                .column_by_name(field)
                .expect("named string output")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("string output");
            values.extend(
                (0..column.len())
                    .map(|index| (!column.is_null(index)).then(|| column.value(index).to_owned())),
            );
        }
        values
    }

    fn u32_values(batches: &[RecordBatch], field: &str) -> Vec<u32> {
        let mut values = Vec::new();
        for batch in batches {
            let column = batch
                .column_by_name(field)
                .expect("named u32 output")
                .as_any()
                .downcast_ref::<UInt32Array>()
                .expect("u32 output");
            values.extend(column.values().iter().copied());
        }
        values
    }

    fn violation_codes(
        execution: &DerivedProducerClosureExecution,
        bindings: &DerivedProducerClosureBindings,
    ) -> BTreeSet<String> {
        string_values(
            execution.violations(),
            bindings.violation.fields.violation_code.as_str(),
        )
        .into_iter()
        .flatten()
        .collect()
    }

    fn stable_rows(batches: &[RecordBatch]) -> Vec<Vec<Option<String>>> {
        let mut rows = Vec::new();
        for batch in batches {
            for row in 0..batch.num_rows() {
                let mut rendered = Vec::new();
                for column in batch.columns() {
                    if let Some(strings) = column.as_any().downcast_ref::<StringArray>() {
                        rendered
                            .push((!strings.is_null(row)).then(|| strings.value(row).to_owned()));
                    } else if let Some(values) = column.as_any().downcast_ref::<UInt32Array>() {
                        rendered.push(Some(values.value(row).to_string()));
                    } else if let Some(values) = column.as_any().downcast_ref::<UInt64Array>() {
                        rendered
                            .push((!values.is_null(row)).then(|| values.value(row).to_string()));
                    } else {
                        panic!("unexpected output type")
                    }
                }
                rows.push(rendered);
            }
        }
        rows
    }

    #[tokio::test]
    async fn wp35_int_decoded_release_rows_bind_compiled_dependencies_and_provenance() {
        let bindings = bindings();
        let producer = producer("family.control-dependence", "producer.common-cdg@1");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.control-dependence", FACT_CLASS)],
                &[producer],
                &[("query.graph", "family.control-dependence")],
                &[],
            ),
        )
        .await;

        assert!(execution.is_conformant());
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_SUPPORTED.to_owned())]
        );
        assert!(
            execution
                .observation()
                .operators()
                .contains(&ProducerClosureNativeOperator::RecursiveQueryDistinct)
        );
        assert!(execution.observation().dependencies().contains(
            &ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::from(
                APP_AUTHORITY,
            ))
        ));
        for field in release_input_field_ids(&bindings) {
            assert!(
                execution
                    .observation()
                    .dependencies()
                    .contains(&ProducerClosureCompilationDependency::InputField(field),)
            );
        }

        let evidence = execution.release_evidence();
        assert_eq!(evidence.families().len(), 1);
        assert_eq!(
            evidence.families()[0].family_id().as_ref(),
            "family.control-dependence",
        );
        assert_eq!(
            evidence.families()[0].semantic_class_id().as_ref(),
            FACT_CLASS
        );
        assert_eq!(
            evidence.families()[0].closure_state().as_ref(),
            STATE_SUPPORTED
        );
        assert_eq!(
            evidence.families()[0]
                .authority_id()
                .map(|value| value.as_ref()),
            Some(APP_AUTHORITY),
        );
        assert_eq!(
            evidence.families()[0]
                .producer_proof_pin()
                .map(|value| value.as_ref()),
            Some("proof:b3:44"),
        );
        assert_eq!(
            evidence.families()[0]
                .completeness_proof_pin()
                .map(|value| value.as_ref()),
            Some("completeness-proof:b3:40"),
        );
        assert_eq!(evidence.query_requirements().len(), 1);
        assert_eq!(
            evidence.query_requirements()[0].query_family_id().as_ref(),
            "query.graph",
        );
        assert_eq!(
            evidence.query_requirements()[0]
                .required_family_id()
                .as_ref(),
            "family.control-dependence",
        );
        assert_eq!(
            evidence.query_requirements()[0]
                .requirement_state()
                .as_ref(),
            STATE_SATISFIED,
        );
        assert!(evidence.violations().is_empty());
        assert!(evidence.issues().is_empty());

        let release = CompiledSemanticRelease::current();
        let proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                release.proof_authority(),
                &execution,
            )
            .expect("bind exact executed closure"),
        );
        assert_eq!(proof.terminal(), ProofTerminalStatus::Pass);
        assert_eq!(proof.operation_id(), evidence.operation_id());
        assert_eq!(
            proof.implementation_release(),
            evidence.implementation_release()
        );
        assert_eq!(
            proof.application_authority_id(),
            evidence.application_authority_id()
        );
        assert_eq!(
            proof.factual_semantic_class_id(),
            evidence.factual_semantic_class_id(),
        );
        assert_eq!(proof.families(), evidence.families());
        assert_eq!(proof.query_requirements(), evidence.query_requirements());
        assert_eq!(proof.violations(), evidence.violations());
        assert_eq!(proof.issues(), evidence.issues());
        assert_eq!(proof.dependencies(), evidence.dependencies());
    }

    #[tokio::test]
    async fn wp35_int_explicit_remainder_closes_family_and_downgrades_query() {
        let bindings = bindings();
        let remainder = remainder("family.dynamic-call-target");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.dynamic-call-target", FACT_CLASS)],
                &[],
                &[("query.callers", "family.dynamic-call-target")],
                &[remainder],
            ),
        )
        .await;

        assert!(execution.is_conformant());
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_UNSUPPORTED.to_owned())]
        );
        assert_eq!(
            string_values(
                execution.query_requirement_closure(),
                bindings
                    .query_requirement_closure
                    .fields
                    .requirement_state
                    .as_str(),
            ),
            vec![Some(STATE_UNSUPPORTED.to_owned())]
        );
        assert_eq!(
            string_values(
                execution.query_requirement_closure(),
                bindings
                    .query_requirement_closure
                    .fields
                    .unknown_cause
                    .as_str(),
            ),
            vec![Some("unknown.dynamic-dispatch".to_owned())]
        );
        assert_eq!(
            execution.release_evidence().families()[0]
                .authority_id()
                .map(|value| value.as_ref()),
            Some(APP_AUTHORITY),
        );
    }

    #[tokio::test]
    async fn wp35_neg_zero_multiple_and_both_are_independent_decoded_violations() {
        let bindings = bindings();
        let producer_a = producer("family.multiple", "producer.a@1");
        let producer_b = producer("family.multiple", "producer.b@1");
        let producer_both = producer("family.both", "producer.both@1");
        let remainder_both = remainder("family.both");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.zero", FACT_CLASS),
                    ("family.multiple", FACT_CLASS),
                    ("family.both", FACT_CLASS),
                ],
                &[producer_a, producer_b, producer_both],
                &[("query.zero", "family.zero")],
                &[remainder_both],
            ),
        )
        .await;
        let codes = violation_codes(&execution, &bindings);

        assert!(!execution.is_conformant());
        assert!(codes.contains("missing_producer_or_remainder"));
        assert!(codes.contains("multiple_runtime_producers"));
        assert!(codes.contains("producer_and_remainder"));
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![
                Some(STATE_INVALID.to_owned()),
                Some(STATE_INVALID.to_owned()),
                Some(STATE_INVALID.to_owned()),
            ]
        );
        assert!(execution.release_evidence().violations().iter().any(|row| {
            row.violation_code().as_ref() == "missing_producer_or_remainder"
                && row.subject_id().as_ref() == "family.zero"
        }));
    }

    #[tokio::test]
    async fn transitive_gap_and_incomplete_family_propagate_to_queries() {
        let bindings = bindings();
        let mut partial = producer("family.partial", "producer.partial@1");
        partial[9] = "0";
        partial[11] = "1";
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.partial", FACT_CLASS)],
                &[partial],
                &[
                    ("query.root", "query.intermediate"),
                    ("query.intermediate", "family.absent"),
                    ("query.partial", "family.partial"),
                ],
                &[],
            ),
        )
        .await;
        let query_fields = &bindings.query_requirement_closure.fields;
        let roots = string_values(
            execution.query_requirement_closure(),
            query_fields.query_family_id.as_str(),
        );
        let required = string_values(
            execution.query_requirement_closure(),
            query_fields.required_family_id.as_str(),
        );
        let states = string_values(
            execution.query_requirement_closure(),
            query_fields.requirement_state.as_str(),
        );
        let depths = u32_values(
            execution.query_requirement_closure(),
            query_fields.minimum_depth.as_str(),
        );
        let rows = roots
            .into_iter()
            .zip(required)
            .zip(states)
            .zip(depths)
            .map(|(((root, required), state), depth)| {
                (root.unwrap(), required.unwrap(), state.unwrap(), depth)
            })
            .collect::<Vec<_>>();

        assert!(rows.contains(&(
            "query.root".to_owned(),
            "family.absent".to_owned(),
            STATE_MISSING.to_owned(),
            2,
        )));
        assert!(rows.contains(&(
            "query.partial".to_owned(),
            "family.partial".to_owned(),
            STATE_UNKNOWN.to_owned(),
            1,
        )));
        let codes = violation_codes(&execution, &bindings);
        assert!(codes.contains("query_requirement_missing"));
        assert!(codes.contains("query_requirement_incomplete"));
        assert!(codes.contains("incomplete_runtime_producer"));
    }

    #[tokio::test]
    async fn provider_authority_and_judgment_semantics_are_rejected() {
        let bindings = bindings();
        let mut provider = producer("family.refactor-risk", "provider.raw-risk@1");
        provider[2] = PROVIDER_AUTHORITY;
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.refactor-risk", JUDGMENT_CLASS)],
                &[provider],
                &[],
                &[],
            ),
        )
        .await;
        let codes = violation_codes(&execution, &bindings);

        assert!(!execution.is_conformant());
        assert!(codes.contains("wrong_runtime_producer_authority"));
        assert!(codes.contains("non_fact_semantic_class"));
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_INVALID.to_owned())]
        );
    }

    #[tokio::test]
    async fn input_permutation_does_not_change_output() {
        let bindings = bindings();
        let producer_a = producer("family.a", "producer.a@1");
        let producer_b = producer("family.b", "producer.b@1");
        let remainder_c = remainder("family.c");
        let left = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.c", FACT_CLASS),
                    ("family.a", FACT_CLASS),
                    ("family.b", FACT_CLASS),
                ],
                &[producer_b, producer_a],
                &[("query.all", "family.c"), ("query.all", "family.a")],
                &[remainder_c],
            ),
        )
        .await;
        let producer_a = producer("family.a", "producer.a@1");
        let producer_b = producer("family.b", "producer.b@1");
        let remainder_c = remainder("family.c");
        let right = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.b", FACT_CLASS),
                    ("family.a", FACT_CLASS),
                    ("family.c", FACT_CLASS),
                ],
                &[producer_a, producer_b],
                &[("query.all", "family.a"), ("query.all", "family.c")],
                &[remainder_c],
            ),
        )
        .await;

        assert_eq!(
            stable_rows(left.family_closure()),
            stable_rows(right.family_closure())
        );
        assert_eq!(
            stable_rows(left.query_requirement_closure()),
            stable_rows(right.query_requirement_closure())
        );
        assert_eq!(
            stable_rows(left.violations()),
            stable_rows(right.violations())
        );
    }

    #[tokio::test]
    async fn wp35_neg_empty_release_closure_preserves_schema_but_never_conforms() {
        let bindings = bindings();
        let execution = execute(&bindings, inputs(&bindings, &[], &[], &[], &[])).await;

        assert!(!execution.is_conformant());
        let issue_codes = execution
            .release_evidence()
            .issues()
            .iter()
            .map(ReleaseProducerClosureIssue::code)
            .collect::<BTreeSet<_>>();
        assert!(issue_codes.contains("empty_accepted_family_closure"));
        assert!(issue_codes.contains("empty_query_requirement_closure"));
        assert_eq!(execution.family_closure().len(), 1);
        assert_eq!(execution.family_closure()[0].num_rows(), 0);
        assert_eq!(
            execution.family_closure()[0].schema_ref(),
            execution.family_closure_schema()
        );
        assert_eq!(execution.query_requirement_closure().len(), 1);
        assert_eq!(execution.query_requirement_closure()[0].num_rows(), 0);
        assert_eq!(
            execution.query_requirement_closure()[0].schema_ref(),
            execution.query_requirement_closure_schema()
        );
        assert_eq!(execution.violations().len(), 1);
        assert_eq!(execution.violations()[0].num_rows(), 0);
        assert_eq!(
            execution.violations()[0].schema_ref(),
            execution.violation_schema()
        );

        let release = CompiledSemanticRelease::current();
        let proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                release.proof_authority(),
                &execution,
            )
            .expect("bind empty executed closure as negative evidence"),
        );
        assert_eq!(proof.terminal(), ProofTerminalStatus::Fail);
    }

    #[tokio::test]
    async fn wp35_neg_zero_requested_producer_scope_cannot_be_false_success() {
        let bindings = bindings();
        let mut zero_scope = producer("family.empty-scope", "producer.empty-scope@1");
        zero_scope[8] = "0";
        zero_scope[9] = "0";
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.empty-scope", FACT_CLASS)],
                &[zero_scope],
                &[("query.empty-scope", "family.empty-scope")],
                &[],
            ),
        )
        .await;

        assert!(
            execution
                .violations()
                .iter()
                .all(|batch| batch.num_rows() == 0)
        );
        assert!(!execution.is_conformant());
        assert!(execution.release_evidence().issues().iter().any(|issue| {
            issue.code() == "empty_runtime_producer_scope"
                && issue.subject_id().map(|value| value.as_ref()) == Some("family.empty-scope")
        }));

        let release = CompiledSemanticRelease::current();
        let proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                release.proof_authority(),
                &execution,
            )
            .expect("bind zero-scope execution"),
        );
        assert_eq!(proof.terminal(), ProofTerminalStatus::Fail);
        assert!(proof.violations().is_empty());
    }

    #[tokio::test]
    async fn wp35_neg_missing_compiled_input_field_dependency_fails_row_proof() {
        let bindings = bindings();
        let producer = producer("family.dependency", "producer.dependency@1");
        let release = CompiledSemanticRelease::current();
        let mut compiled = compile_derived_producer_closure(
            release.proof_authority(),
            inputs(
                &bindings,
                &[("family.dependency", FACT_CLASS)],
                &[producer],
                &[("query.dependency", "family.dependency")],
                &[],
            ),
            &bindings,
            bounds(),
        )
        .expect("compile closure");
        let missing = ProducerClosureCompilationDependency::InputField(
            bindings
                .accepted_fact_family
                .fields
                .semantic_class_id
                .clone(),
        );
        assert!(compiled.observation.dependencies.remove(&missing));
        let execution = compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute closure with incomplete observation");

        assert!(!execution.is_conformant());
        assert!(execution.release_evidence().issues().iter().any(|issue| {
            issue.code() == "missing_compiled_release_dependency" && issue.subject_id().is_none()
        }));
        let proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                release.proof_authority(),
                &execution,
            )
            .expect("bind incomplete-dependency execution"),
        );
        assert_eq!(proof.terminal(), ProofTerminalStatus::Fail);
        assert!(!proof.dependencies().contains(&missing));
    }

    #[tokio::test]
    async fn wp35_beh_authority_mutation_changes_decoded_proof_result() {
        let bindings = bindings();
        let valid = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.authority", FACT_CLASS)],
                &[producer("family.authority", "producer.authority@1")],
                &[("query.authority", "family.authority")],
                &[],
            ),
        )
        .await;
        let mut mutated_producer = producer("family.authority", "producer.authority@1");
        mutated_producer[2] = PROVIDER_AUTHORITY;
        let mutated = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.authority", FACT_CLASS)],
                &[mutated_producer],
                &[("query.authority", "family.authority")],
                &[],
            ),
        )
        .await;
        let release = CompiledSemanticRelease::current();
        let valid_proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(release.proof_authority(), &valid)
                .expect("bind valid execution"),
        );
        let mutated_proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                release.proof_authority(),
                &mutated,
            )
            .expect("bind mutated execution"),
        );

        assert_eq!(valid_proof.terminal(), ProofTerminalStatus::Pass);
        assert_eq!(mutated_proof.terminal(), ProofTerminalStatus::Fail);
        assert!(mutated_proof.violations().iter().any(|row| {
            row.violation_code().as_ref() == "wrong_runtime_producer_authority"
                && row.subject_id().as_ref() == "family.authority"
        }));
        assert_eq!(
            mutated_proof.families()[0]
                .authority_id()
                .map(|value| value.as_ref()),
            Some(PROVIDER_AUTHORITY),
        );
    }
}
