//! Fallible compilation of the behavior-bearing semantic release.
//!
//! This module is deliberately inward of providers, the relational fabric, state, RPC, and the
//! daemon. A closed Rust definition is compiled into immutable programs. Concrete provider
//! modules may contribute only application-owned relation definitions while the migration is in
//! progress; no provider-native or generated transport type crosses this boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_schema::SchemaRef;
use thiserror::Error;

use crate::provider_contracts::{
    AdmittedProviderResult, CancellationProbe, ProviderBuildIdentity, ProviderContextBinding,
    ProviderContractError, ProviderFamilyIdentity, ProviderFamilyRequest, ProviderIdentity,
    ProviderJob, ProviderJobSpec, ProviderLane, ProviderPolicyIdentity, ProviderProgramIdentity,
    ProviderProtocolIdentity, ProviderRelationIdentity, ProviderResourceCeilingSpec,
    ProviderResourceCeilings, ProviderRunBinding, ProviderRunProvenance, ProviderRunResult,
    ProviderSchemaIdentity, ProviderScopeIdentity, ProviderSourceBinding, ProviderTrustPosture,
    SuiteIdentity, admit_provider_result,
};

const CURRENT_SUITE: &str = "codefabric-relational-data-fabric@2.3.0";
const MAX_PROGRAM_IDENTITY_BYTES: usize = 512;
const MAX_PROVIDER_FAMILIES: usize = 4_096;
const MAX_TRANSFORMATIONS: usize = 8_192;
const MAX_PROOF_EXPECTATIONS: usize = 16_384;
const MAX_QUERY_RELATIONS: usize = 4_096;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct BoundedProgramIdentity(Arc<str>);

impl BoundedProgramIdentity {
    fn try_new(value: impl Into<Arc<str>>) -> Result<Self, SemanticReleaseError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_PROGRAM_IDENTITY_BYTES
            || value.trim() != value.as_ref()
            || value.chars().any(char::is_control)
        {
            return Err(SemanticReleaseError::InvalidProgramIdentity);
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! program_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(BoundedProgramIdentity);

        impl $name {
            /// Construct one bounded categorical program identity.
            ///
            /// # Errors
            ///
            /// Rejects an empty, padded, control-bearing, or oversized value.
            pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, SemanticReleaseError> {
                BoundedProgramIdentity::try_new(value).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

program_identity!(TransformationProgramIdentity);
program_identity!(QueryProgramIdentity);
program_identity!(ProofProgramIdentity);
program_identity!(PolicyProgramIdentity);
program_identity!(TransformationIdentity);
program_identity!(QueryIdentity);
program_identity!(ProofExpectationIdentity);
program_identity!(CausalFaultIdentity);

/// The eight released query forms. These values are application program identities, not wire
/// spellings and not executor-generated observations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticQueryForm {
    FindCodeEntities,
    RetrieveFactsAboutCode,
    FollowCodeRelationships,
    FindConnectingFactPaths,
    MatchCodeFactPattern,
    CombineResultSets,
    SummarizeObjectiveFacts,
    RetrieveSourceAndSyntaxContext,
}

impl SemanticQueryForm {
    pub const ALL: [Self; 8] = [
        Self::FindCodeEntities,
        Self::RetrieveFactsAboutCode,
        Self::FollowCodeRelationships,
        Self::FindConnectingFactPaths,
        Self::MatchCodeFactPattern,
        Self::CombineResultSets,
        Self::SummarizeObjectiveFacts,
        Self::RetrieveSourceAndSyntaxContext,
    ];

    /// Return the categorical identity of this released executable program.
    #[must_use]
    pub const fn program_identity(self) -> &'static str {
        match self {
            Self::FindCodeEntities => "program.semantic-query.find-code-entities.v3",
            Self::RetrieveFactsAboutCode => "program.semantic-query.retrieve-facts-about-code.v3",
            Self::FollowCodeRelationships => "program.semantic-query.follow-code-relationships.v3",
            Self::FindConnectingFactPaths => "program.semantic-query.find-connecting-fact-paths.v3",
            Self::MatchCodeFactPattern => "program.semantic-query.match-code-fact-pattern.v3",
            Self::CombineResultSets => "program.semantic-query.combine-result-sets.v3",
            Self::SummarizeObjectiveFacts => "program.semantic-query.summarize-objective-facts.v3",
            Self::RetrieveSourceAndSyntaxContext => {
                "program.semantic-query.retrieve-source-syntax-context.v3"
            }
        }
    }
}

/// Native relational operator selected by a compiled transformation program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformationOperator {
    Identity,
    Project,
    Filter,
    Normalize,
    Join,
    Union,
    Aggregate,
    TransitiveClosure,
}

impl TransformationOperator {
    const fn accepts_input_count(self, count: usize) -> bool {
        match self {
            Self::Identity | Self::Project | Self::Filter | Self::Normalize | Self::Aggregate => {
                count == 1
            }
            Self::Join | Self::Union => count >= 2,
            Self::TransitiveClosure => count == 1,
        }
    }
}

/// One exact relation family owned by a provider lane in a release definition.
#[derive(Clone, Debug)]
pub(crate) struct ProviderFamilyProgramDefinition {
    family: ProviderFamilyIdentity,
    relation: ProviderRelationIdentity,
    schema_identity: ProviderSchemaIdentity,
    schema: SchemaRef,
}

impl ProviderFamilyProgramDefinition {
    pub(crate) fn try_new(
        family: ProviderFamilyIdentity,
        relation: ProviderRelationIdentity,
        schema_identity: ProviderSchemaIdentity,
        schema: SchemaRef,
    ) -> Result<Self, SemanticReleaseError> {
        if schema.fields().is_empty() {
            return Err(SemanticReleaseError::EmptyArrowSchema);
        }
        Ok(Self {
            family,
            relation,
            schema_identity,
            schema,
        })
    }
}

/// One exact provider lane and its categorical execution/provenance pins.
#[derive(Clone, Debug)]
pub(crate) struct ProviderLaneProgramDefinition {
    lane: ProviderLane,
    provider: ProviderIdentity,
    protocol: ProviderProtocolIdentity,
    build: ProviderBuildIdentity,
    trust: ProviderTrustPosture,
    families: Vec<ProviderFamilyProgramDefinition>,
}

impl ProviderLaneProgramDefinition {
    pub(crate) fn try_new(
        lane: ProviderLane,
        provider: ProviderIdentity,
        protocol: ProviderProtocolIdentity,
        build: ProviderBuildIdentity,
        trust: ProviderTrustPosture,
        families: Vec<ProviderFamilyProgramDefinition>,
    ) -> Result<Self, SemanticReleaseError> {
        if families.is_empty() || families.len() > MAX_PROVIDER_FAMILIES {
            return Err(SemanticReleaseError::EmptyOrOversizedProviderProgram);
        }
        Ok(Self {
            lane,
            provider,
            protocol,
            build,
            trust,
            families,
        })
    }
}

/// Closed provider program definition supplied to the release compiler.
#[derive(Clone, Debug)]
pub(crate) struct ProviderProgramDefinition {
    identity: ProviderProgramIdentity,
    lanes: Vec<ProviderLaneProgramDefinition>,
}

impl ProviderProgramDefinition {
    pub(crate) fn try_new(
        identity: ProviderProgramIdentity,
        lanes: Vec<ProviderLaneProgramDefinition>,
    ) -> Result<Self, SemanticReleaseError> {
        if lanes.is_empty() {
            return Err(SemanticReleaseError::EmptyOrOversizedProviderProgram);
        }
        Ok(Self { identity, lanes })
    }
}

/// One typed transformation node. All inputs and the output are application relation identities.
#[derive(Clone, Debug)]
pub(crate) struct TransformationDefinition {
    identity: TransformationIdentity,
    operator: TransformationOperator,
    inputs: Vec<ProviderRelationIdentity>,
    output: ProviderRelationIdentity,
    output_schema_identity: ProviderSchemaIdentity,
    output_schema: SchemaRef,
}

impl TransformationDefinition {
    pub(crate) fn try_new(
        identity: TransformationIdentity,
        operator: TransformationOperator,
        inputs: Vec<ProviderRelationIdentity>,
        output: ProviderRelationIdentity,
        output_schema_identity: ProviderSchemaIdentity,
        output_schema: SchemaRef,
    ) -> Result<Self, SemanticReleaseError> {
        if !operator.accepts_input_count(inputs.len()) {
            return Err(SemanticReleaseError::TransformationArity);
        }
        if output_schema.fields().is_empty() {
            return Err(SemanticReleaseError::EmptyArrowSchema);
        }
        Ok(Self {
            identity,
            operator,
            inputs,
            output,
            output_schema_identity,
            output_schema,
        })
    }
}

/// Closed transformation program definition.
#[derive(Clone, Debug)]
pub(crate) struct TransformationProgramDefinition {
    identity: TransformationProgramIdentity,
    transformations: Vec<TransformationDefinition>,
}

impl TransformationProgramDefinition {
    pub(crate) fn try_new(
        identity: TransformationProgramIdentity,
        transformations: Vec<TransformationDefinition>,
    ) -> Result<Self, SemanticReleaseError> {
        if transformations.is_empty() || transformations.len() > MAX_TRANSFORMATIONS {
            return Err(SemanticReleaseError::EmptyOrOversizedTransformationProgram);
        }
        Ok(Self {
            identity,
            transformations,
        })
    }
}

/// One released query form and the exact relation dependencies it compiles.
#[derive(Clone, Debug)]
pub(crate) struct QueryDefinition {
    identity: QueryIdentity,
    form: SemanticQueryForm,
    required_relations: Vec<ProviderRelationIdentity>,
    maximum_rows: u64,
}

impl QueryDefinition {
    pub(crate) fn try_new(
        identity: QueryIdentity,
        form: SemanticQueryForm,
        required_relations: Vec<ProviderRelationIdentity>,
        maximum_rows: u64,
    ) -> Result<Self, SemanticReleaseError> {
        if required_relations.is_empty()
            || required_relations.len() > MAX_QUERY_RELATIONS
            || maximum_rows == 0
        {
            return Err(SemanticReleaseError::InvalidQueryDefinition);
        }
        Ok(Self {
            identity,
            form,
            required_relations,
            maximum_rows,
        })
    }
}

/// Closed query program definition.
#[derive(Clone, Debug)]
pub(crate) struct QueryProgramDefinition {
    identity: QueryProgramIdentity,
    queries: Vec<QueryDefinition>,
}

impl QueryProgramDefinition {
    pub(crate) fn try_new(
        identity: QueryProgramIdentity,
        queries: Vec<QueryDefinition>,
    ) -> Result<Self, SemanticReleaseError> {
        if queries.is_empty() {
            return Err(SemanticReleaseError::QueryFormCoverage);
        }
        Ok(Self { identity, queries })
    }
}

/// One independently declared proof expectation.
#[derive(Clone, Debug)]
pub(crate) struct ProofExpectationDefinition {
    identity: ProofExpectationIdentity,
    relation: ProviderRelationIdentity,
    minimum_rows: u64,
}

impl ProofExpectationDefinition {
    pub(crate) const fn new(
        identity: ProofExpectationIdentity,
        relation: ProviderRelationIdentity,
        minimum_rows: u64,
    ) -> Self {
        Self {
            identity,
            relation,
            minimum_rows,
        }
    }
}

/// Required causal effect for a release proof fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalEffect {
    RejectAdmission,
    ChangeTransformation,
    ChangeQuery,
    ChangeProofTerminal,
}

/// One independently declared causal fault.
#[derive(Clone, Debug)]
pub(crate) struct CausalFaultDefinition {
    identity: CausalFaultIdentity,
    target_relation: ProviderRelationIdentity,
    effect: CausalEffect,
}

impl CausalFaultDefinition {
    pub(crate) const fn new(
        identity: CausalFaultIdentity,
        target_relation: ProviderRelationIdentity,
        effect: CausalEffect,
    ) -> Self {
        Self {
            identity,
            target_relation,
            effect,
        }
    }
}

/// Closed proof program definition.
#[derive(Clone, Debug)]
pub(crate) struct ProofProgramDefinition {
    identity: ProofProgramIdentity,
    expectations: Vec<ProofExpectationDefinition>,
    faults: Vec<CausalFaultDefinition>,
}

impl ProofProgramDefinition {
    pub(crate) fn try_new(
        identity: ProofProgramIdentity,
        expectations: Vec<ProofExpectationDefinition>,
        faults: Vec<CausalFaultDefinition>,
    ) -> Result<Self, SemanticReleaseError> {
        if expectations.is_empty()
            || expectations.len() > MAX_PROOF_EXPECTATIONS
            || faults.is_empty()
        {
            return Err(SemanticReleaseError::EmptyOrOversizedProofProgram);
        }
        Ok(Self {
            identity,
            expectations,
            faults,
        })
    }
}

/// One lane's compiled policy ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LanePolicyDefinition {
    lane: ProviderLane,
    ceilings: ProviderResourceCeilings,
}

impl LanePolicyDefinition {
    pub(crate) const fn new(lane: ProviderLane, ceilings: ProviderResourceCeilings) -> Self {
        Self { lane, ceilings }
    }
}

/// Closed resource/trust policy definition.
#[derive(Clone, Debug)]
pub(crate) struct PolicyProgramDefinition {
    identity: PolicyProgramIdentity,
    lanes: Vec<LanePolicyDefinition>,
    maximum_query_rows: u64,
}

impl PolicyProgramDefinition {
    pub(crate) fn try_new(
        identity: PolicyProgramIdentity,
        lanes: Vec<LanePolicyDefinition>,
        maximum_query_rows: u64,
    ) -> Result<Self, SemanticReleaseError> {
        if lanes.is_empty() || maximum_query_rows == 0 {
            return Err(SemanticReleaseError::InvalidPolicyProgram);
        }
        Ok(Self {
            identity,
            lanes,
            maximum_query_rows,
        })
    }
}

/// Dependency edge between the five closed program products.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ProgramDependency {
    consumer: ReleaseComponent,
    dependency: ReleaseComponent,
}

impl ProgramDependency {
    pub(crate) const fn new(consumer: ReleaseComponent, dependency: ReleaseComponent) -> Self {
        Self {
            consumer,
            dependency,
        }
    }
}

/// One member of the closed release program graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReleaseComponent {
    Provider,
    Transformation,
    Query,
    Proof,
    Policy,
}

/// Complete closed Rust input to the semantic release compiler.
#[derive(Clone, Debug)]
pub(crate) struct CurrentSemanticReleaseDefinition {
    suite: SuiteIdentity,
    providers: ProviderProgramDefinition,
    transformations: TransformationProgramDefinition,
    queries: QueryProgramDefinition,
    proof: ProofProgramDefinition,
    policy: PolicyProgramDefinition,
    dependencies: Vec<ProgramDependency>,
}

/// Inputs kept together so construction has one explicit boundary.
#[derive(Clone, Debug)]
pub(crate) struct CurrentSemanticReleaseDefinitionParts {
    pub suite: SuiteIdentity,
    pub providers: ProviderProgramDefinition,
    pub transformations: TransformationProgramDefinition,
    pub queries: QueryProgramDefinition,
    pub proof: ProofProgramDefinition,
    pub policy: PolicyProgramDefinition,
    pub dependencies: Vec<ProgramDependency>,
}

impl CurrentSemanticReleaseDefinition {
    pub(crate) fn new(parts: CurrentSemanticReleaseDefinitionParts) -> Self {
        Self {
            suite: parts.suite,
            providers: parts.providers,
            transformations: parts.transformations,
            queries: parts.queries,
            proof: parts.proof,
            policy: parts.policy,
            dependencies: parts.dependencies,
        }
    }
}

/// Immutable behavior-bearing provider program.
#[derive(Clone, Debug)]
pub struct CompiledProviderProgram {
    suite: SuiteIdentity,
    identity: ProviderProgramIdentity,
    lanes: Arc<BTreeMap<ProviderLane, CompiledProviderLaneProgram>>,
    policy_identity: ProviderPolicyIdentity,
}

#[derive(Clone, Debug)]
struct CompiledProviderLaneProgram {
    provider: ProviderIdentity,
    protocol: ProviderProtocolIdentity,
    build: ProviderBuildIdentity,
    trust: ProviderTrustPosture,
    families: BTreeMap<ProviderFamilyIdentity, ProviderFamilyProgramDefinition>,
}

/// Caller-variable inputs from which the release prepares one immutable provider job.
#[derive(Clone, Debug)]
pub struct ProviderJobInput {
    pub lane: ProviderLane,
    pub source: ProviderSourceBinding,
    pub context: ProviderContextBinding,
    pub run: ProviderRunBinding,
    pub scope: ProviderScopeIdentity,
    pub requested_families: Vec<(ProviderFamilyIdentity, u64)>,
    pub operational_ceilings: ProviderResourceCeilings,
    pub deadline: Instant,
    pub cancellation: CancellationProbe,
}

/// A job that can only be minted by its compiled provider program.
#[derive(Clone, Debug)]
pub struct PreparedProviderJob {
    suite: SuiteIdentity,
    provider_program: ProviderProgramIdentity,
    job: ProviderJob,
}

impl PreparedProviderJob {
    #[must_use]
    pub const fn job(&self) -> &ProviderJob {
        &self.job
    }
}

impl CompiledProviderProgram {
    /// Reduce operational ceilings through compiled policy and prepare an exact provider job.
    ///
    /// # Errors
    ///
    /// Rejects an unknown lane/family, a duplicate family, or any invalid effective job value.
    pub fn prepare_job(
        &self,
        policy: &CompiledPolicyProgram,
        input: ProviderJobInput,
    ) -> Result<PreparedProviderJob, SemanticReleaseError> {
        let lane = self
            .lanes
            .get(&input.lane)
            .ok_or(SemanticReleaseError::UnknownProviderLane)?;
        let ceilings = policy.reduce_provider_ceilings(input.lane, input.operational_ceilings)?;
        let mut requested = BTreeSet::new();
        let mut requests = Vec::with_capacity(input.requested_families.len());
        for (family, units) in input.requested_families {
            if !requested.insert(family.clone()) {
                return Err(SemanticReleaseError::DuplicateProviderFamily);
            }
            let definition = lane
                .families
                .get(&family)
                .ok_or(SemanticReleaseError::UnknownProviderFamily)?;
            requests.push(ProviderFamilyRequest::try_new(
                family,
                definition.relation.clone(),
                definition.schema_identity.clone(),
                Arc::clone(&definition.schema),
                input.scope.clone(),
                units,
            )?);
        }
        let provenance = ProviderRunProvenance::new(
            lane.build.clone(),
            self.policy_identity.clone(),
            self.identity.clone(),
        );
        let now = Instant::now();
        let policy_deadline = now
            .checked_add(Duration::from_millis(ceilings.max_wall_millis()))
            .ok_or(ProviderContractError::InvalidResourceCeiling)?;
        let cancellation = input
            .cancellation
            .restricted_to(ceilings.cancellation_poll_work_units())?;
        let job = ProviderJob::try_new(ProviderJobSpec {
            suite: self.suite.clone(),
            provider: lane.provider.clone(),
            protocol: lane.protocol.clone(),
            source: input.source,
            context: input.context,
            run: input.run,
            lane: input.lane,
            trust: lane.trust,
            requests,
            ceilings,
            deadline: input.deadline.min(policy_deadline),
            cancellation,
            provenance,
        })?;
        Ok(PreparedProviderJob {
            suite: self.suite.clone(),
            provider_program: self.identity.clone(),
            job,
        })
    }

    /// Admit a result only through the release-minted job wrapper.
    ///
    /// # Errors
    ///
    /// Rejects a wrapper from another release/program or any generic admission fault.
    pub fn admit(
        &self,
        prepared: PreparedProviderJob,
        result: ProviderRunResult,
    ) -> Result<AdmittedProviderResult, SemanticReleaseError> {
        if prepared.suite != self.suite || prepared.provider_program != self.identity {
            return Err(SemanticReleaseError::ForgedProviderJob);
        }
        Ok(admit_provider_result(prepared.job, result)?)
    }

    #[must_use]
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    #[must_use]
    pub fn relation_count(&self) -> usize {
        self.lanes.values().map(|lane| lane.families.len()).sum()
    }

    /// Return the exact released family inventory for one lane in deterministic identity order.
    ///
    /// # Errors
    ///
    /// Rejects a lane not compiled into this release.
    pub fn families(
        &self,
        lane: ProviderLane,
    ) -> Result<Vec<ProviderFamilyIdentity>, SemanticReleaseError> {
        Ok(self
            .lanes
            .get(&lane)
            .ok_or(SemanticReleaseError::UnknownProviderLane)?
            .families
            .keys()
            .cloned()
            .collect())
    }
}

/// Immutable behavior-bearing transformation program.
#[derive(Clone, Debug)]
pub struct CompiledTransformationProgram {
    identity: TransformationProgramIdentity,
    transformations: Arc<[TransformationDefinition]>,
}

/// Exact native plan selected by the compiled transformation program.
#[derive(Clone, Debug)]
pub struct CompiledTransformationPlan {
    pub identity: TransformationIdentity,
    pub operator: TransformationOperator,
    pub inputs: Arc<[ProviderRelationIdentity]>,
    pub output: ProviderRelationIdentity,
    pub output_schema_identity: ProviderSchemaIdentity,
    pub output_schema: SchemaRef,
}

impl CompiledTransformationProgram {
    /// Return this program's categorical identity for provenance and cross-boundary matching.
    #[must_use]
    pub fn identity(&self) -> &TransformationProgramIdentity {
        &self.identity
    }

    /// Compile one exact transformation plan from the release-owned graph.
    ///
    /// # Errors
    ///
    /// Rejects an output not owned by this release.
    pub fn compile(
        &self,
        output: &ProviderRelationIdentity,
    ) -> Result<CompiledTransformationPlan, SemanticReleaseError> {
        let definition = self
            .transformations
            .iter()
            .find(|definition| &definition.output == output)
            .ok_or(SemanticReleaseError::UnknownTransformation)?;
        Ok(CompiledTransformationPlan {
            identity: definition.identity.clone(),
            operator: definition.operator,
            inputs: Arc::from(definition.inputs.clone()),
            output: definition.output.clone(),
            output_schema_identity: definition.output_schema_identity.clone(),
            output_schema: Arc::clone(&definition.output_schema),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.transformations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transformations.is_empty()
    }
}

/// Immutable behavior-bearing query program.
#[derive(Clone, Debug)]
pub struct CompiledQueryProgram {
    identity: QueryProgramIdentity,
    queries: Arc<BTreeMap<SemanticQueryForm, QueryDefinition>>,
    policy_maximum_rows: u64,
}

/// Exact query plan selected by a released query form.
#[derive(Clone, Debug)]
pub struct CompiledQueryPlan {
    pub identity: QueryIdentity,
    pub form: SemanticQueryForm,
    pub required_relations: Arc<[ProviderRelationIdentity]>,
    pub maximum_rows: u64,
}

impl CompiledQueryProgram {
    /// Compile one of the eight released query forms.
    ///
    /// # Errors
    ///
    /// Rejects a form missing from the compiled release.
    pub fn compile(
        &self,
        form: SemanticQueryForm,
    ) -> Result<CompiledQueryPlan, SemanticReleaseError> {
        let query = self
            .queries
            .get(&form)
            .ok_or(SemanticReleaseError::UnknownQueryForm)?;
        Ok(CompiledQueryPlan {
            identity: query.identity.clone(),
            form,
            required_relations: Arc::from(query.required_relations.clone()),
            maximum_rows: query.maximum_rows.min(self.policy_maximum_rows),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }
}

/// Immutable behavior-bearing proof program.
#[derive(Clone, Debug)]
pub struct CompiledProofProgram {
    identity: ProofProgramIdentity,
    expectations: Arc<[ProofExpectationDefinition]>,
    faults: Arc<[CausalFaultDefinition]>,
}

/// Independent proof input constructed from the compiled release, not provider output.
#[derive(Clone, Debug)]
pub struct CompiledProofInput {
    pub expectations: Arc<[(ProofExpectationIdentity, ProviderRelationIdentity, u64)]>,
    pub faults: Arc<[(CausalFaultIdentity, ProviderRelationIdentity, CausalEffect)]>,
}

impl CompiledProofProgram {
    #[must_use]
    pub fn construct_input(&self) -> CompiledProofInput {
        CompiledProofInput {
            expectations: Arc::from(
                self.expectations
                    .iter()
                    .map(|expectation| {
                        (
                            expectation.identity.clone(),
                            expectation.relation.clone(),
                            expectation.minimum_rows,
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            faults: Arc::from(
                self.faults
                    .iter()
                    .map(|fault| {
                        (
                            fault.identity.clone(),
                            fault.target_relation.clone(),
                            fault.effect,
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        }
    }
}

/// Immutable behavior-bearing policy program.
#[derive(Clone, Debug)]
pub struct CompiledPolicyProgram {
    identity: PolicyProgramIdentity,
    provider_ceilings: Arc<BTreeMap<ProviderLane, ProviderResourceCeilings>>,
    maximum_query_rows: u64,
}

impl CompiledPolicyProgram {
    fn reduce_provider_ceilings(
        &self,
        lane: ProviderLane,
        operational: ProviderResourceCeilings,
    ) -> Result<ProviderResourceCeilings, SemanticReleaseError> {
        let compiled = self
            .provider_ceilings
            .get(&lane)
            .ok_or(SemanticReleaseError::UnknownProviderLane)?;
        Ok(compiled.intersect(operational)?)
    }
}

/// The single immutable result of compiling one closed semantic release definition.
#[derive(Clone, Debug)]
pub struct CompiledSemanticRelease {
    suite: SuiteIdentity,
    providers: CompiledProviderProgram,
    transformations: CompiledTransformationProgram,
    queries: CompiledQueryProgram,
    proof: CompiledProofProgram,
    policy: CompiledPolicyProgram,
    observation: CompiledReleaseObservation,
}

/// Observation derived from the compiled program values. It is not an input registry or semantic
/// digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledReleaseObservation {
    pub suite: SuiteIdentity,
    pub provider_program: ProviderProgramIdentity,
    pub transformation_program: TransformationProgramIdentity,
    pub query_program: QueryProgramIdentity,
    pub proof_program: ProofProgramIdentity,
    pub policy_program: PolicyProgramIdentity,
    pub provider_lanes: usize,
    pub provider_relations: usize,
    pub transformations: usize,
    pub query_forms: usize,
    pub proof_expectations: usize,
    pub causal_faults: usize,
}

impl CompiledSemanticRelease {
    /// Compile and cross-check a complete release definition.
    ///
    /// # Errors
    ///
    /// Rejects stale/conflated identities, an open dependency graph, relation/schema conflicts,
    /// missing provider/policy lanes, incomplete query forms, or incomplete proof coverage.
    pub(crate) fn compile(
        definition: CurrentSemanticReleaseDefinition,
    ) -> Result<Self, SemanticReleaseError> {
        if definition.suite.as_str() != CURRENT_SUITE {
            return Err(SemanticReleaseError::StaleSuiteIdentity);
        }
        validate_program_identities(&definition)?;
        validate_dependencies(&definition.dependencies)?;

        let policy = compile_policy(definition.policy)?;
        let providers = compile_providers(definition.suite.clone(), definition.providers, &policy)?;
        let available = provider_relation_schemas(&providers)?;
        let transformations = compile_transformations(definition.transformations, available)?;
        let all_relations = transformation_relation_schemas(&providers, &transformations)?;
        let queries = compile_queries(definition.queries, &all_relations, &policy)?;
        let proof = compile_proof(definition.proof, &all_relations, &queries)?;
        let observation = CompiledReleaseObservation {
            suite: definition.suite.clone(),
            provider_program: providers.identity.clone(),
            transformation_program: transformations.identity.clone(),
            query_program: queries.identity.clone(),
            proof_program: proof.identity.clone(),
            policy_program: policy.identity.clone(),
            provider_lanes: providers.lane_count(),
            provider_relations: providers.relation_count(),
            transformations: transformations.transformations.len(),
            query_forms: queries.queries.len(),
            proof_expectations: proof.expectations.len(),
            causal_faults: proof.faults.len(),
        };
        Ok(Self {
            suite: definition.suite,
            providers,
            transformations,
            queries,
            proof,
            policy,
            observation,
        })
    }

    #[must_use]
    pub const fn suite(&self) -> &SuiteIdentity {
        &self.suite
    }

    #[must_use]
    pub const fn providers(&self) -> &CompiledProviderProgram {
        &self.providers
    }

    #[must_use]
    pub const fn transformations(&self) -> &CompiledTransformationProgram {
        &self.transformations
    }

    #[must_use]
    pub const fn queries(&self) -> &CompiledQueryProgram {
        &self.queries
    }

    #[must_use]
    pub const fn proof(&self) -> &CompiledProofProgram {
        &self.proof
    }

    #[must_use]
    pub const fn policy(&self) -> &CompiledPolicyProgram {
        &self.policy
    }

    #[must_use]
    pub const fn observation(&self) -> &CompiledReleaseObservation {
        &self.observation
    }
}

fn validate_program_identities(
    definition: &CurrentSemanticReleaseDefinition,
) -> Result<(), SemanticReleaseError> {
    let identities = [
        definition.providers.identity.as_str(),
        definition.transformations.identity.as_str(),
        definition.queries.identity.as_str(),
        definition.proof.identity.as_str(),
        definition.policy.identity.as_str(),
    ];
    if identities.into_iter().collect::<BTreeSet<_>>().len() != identities.len() {
        return Err(SemanticReleaseError::ProgramIdentityConflation);
    }
    Ok(())
}

fn validate_dependencies(dependencies: &[ProgramDependency]) -> Result<(), SemanticReleaseError> {
    let required = BTreeSet::from([
        ProgramDependency::new(ReleaseComponent::Provider, ReleaseComponent::Policy),
        ProgramDependency::new(ReleaseComponent::Transformation, ReleaseComponent::Provider),
        ProgramDependency::new(ReleaseComponent::Query, ReleaseComponent::Transformation),
        ProgramDependency::new(ReleaseComponent::Query, ReleaseComponent::Policy),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Provider),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Transformation),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Query),
    ]);
    let observed = dependencies.iter().copied().collect::<BTreeSet<_>>();
    if observed != required || observed.len() != dependencies.len() {
        return Err(SemanticReleaseError::ProgramDependencyClosure);
    }
    Ok(())
}

fn compile_policy(
    definition: PolicyProgramDefinition,
) -> Result<CompiledPolicyProgram, SemanticReleaseError> {
    let mut provider_ceilings = BTreeMap::new();
    for lane in definition.lanes {
        if provider_ceilings.insert(lane.lane, lane.ceilings).is_some() {
            return Err(SemanticReleaseError::DuplicateProviderLane);
        }
    }
    if provider_ceilings.keys().copied().collect::<BTreeSet<_>>()
        != ProviderLane::ALL.into_iter().collect::<BTreeSet<_>>()
    {
        return Err(SemanticReleaseError::PolicyLaneCoverage);
    }
    Ok(CompiledPolicyProgram {
        identity: definition.identity,
        provider_ceilings: Arc::new(provider_ceilings),
        maximum_query_rows: definition.maximum_query_rows,
    })
}

fn compile_providers(
    suite: SuiteIdentity,
    definition: ProviderProgramDefinition,
    policy: &CompiledPolicyProgram,
) -> Result<CompiledProviderProgram, SemanticReleaseError> {
    let mut lanes = BTreeMap::new();
    let mut relations = BTreeSet::new();
    let mut families = BTreeSet::new();
    for lane in definition.lanes {
        if !policy.provider_ceilings.contains_key(&lane.lane) {
            return Err(SemanticReleaseError::PolicyLaneCoverage);
        }
        let mut lane_families = BTreeMap::new();
        for family in lane.families {
            if !families.insert(family.family.clone()) {
                return Err(SemanticReleaseError::DuplicateProviderFamily);
            }
            if !relations.insert(family.relation.clone()) {
                return Err(SemanticReleaseError::DuplicateRelation);
            }
            if lane_families
                .insert(family.family.clone(), family)
                .is_some()
            {
                return Err(SemanticReleaseError::DuplicateProviderFamily);
            }
        }
        let compiled = CompiledProviderLaneProgram {
            provider: lane.provider,
            protocol: lane.protocol,
            build: lane.build,
            trust: lane.trust,
            families: lane_families,
        };
        if lanes.insert(lane.lane, compiled).is_some() {
            return Err(SemanticReleaseError::DuplicateProviderLane);
        }
    }
    if lanes.keys().copied().collect::<BTreeSet<_>>()
        != ProviderLane::ALL.into_iter().collect::<BTreeSet<_>>()
    {
        return Err(SemanticReleaseError::ProviderLaneCoverage);
    }
    let policy_identity = ProviderPolicyIdentity::try_new(policy.identity.as_str())?;
    Ok(CompiledProviderProgram {
        suite,
        identity: definition.identity,
        lanes: Arc::new(lanes),
        policy_identity,
    })
}

fn provider_relation_schemas(
    providers: &CompiledProviderProgram,
) -> Result<
    BTreeMap<ProviderRelationIdentity, (ProviderSchemaIdentity, SchemaRef)>,
    SemanticReleaseError,
> {
    let mut relations = BTreeMap::new();
    for lane in providers.lanes.values() {
        for family in lane.families.values() {
            if relations
                .insert(
                    family.relation.clone(),
                    (family.schema_identity.clone(), Arc::clone(&family.schema)),
                )
                .is_some()
            {
                return Err(SemanticReleaseError::DuplicateRelation);
            }
        }
    }
    Ok(relations)
}

fn compile_transformations(
    definition: TransformationProgramDefinition,
    mut available: BTreeMap<ProviderRelationIdentity, (ProviderSchemaIdentity, SchemaRef)>,
) -> Result<CompiledTransformationProgram, SemanticReleaseError> {
    let mut identities = BTreeSet::new();
    for transformation in &definition.transformations {
        if !identities.insert(transformation.identity.clone()) {
            return Err(SemanticReleaseError::DuplicateTransformation);
        }
        if transformation
            .inputs
            .iter()
            .any(|input| !available.contains_key(input))
        {
            return Err(SemanticReleaseError::OpenRelationDependency);
        }
        if available
            .insert(
                transformation.output.clone(),
                (
                    transformation.output_schema_identity.clone(),
                    Arc::clone(&transformation.output_schema),
                ),
            )
            .is_some()
        {
            return Err(SemanticReleaseError::DuplicateRelation);
        }
    }
    Ok(CompiledTransformationProgram {
        identity: definition.identity,
        transformations: Arc::from(definition.transformations),
    })
}

fn transformation_relation_schemas(
    providers: &CompiledProviderProgram,
    transformations: &CompiledTransformationProgram,
) -> Result<
    BTreeMap<ProviderRelationIdentity, (ProviderSchemaIdentity, SchemaRef)>,
    SemanticReleaseError,
> {
    let mut available = provider_relation_schemas(providers)?;
    for transformation in transformations.transformations.iter() {
        if available
            .insert(
                transformation.output.clone(),
                (
                    transformation.output_schema_identity.clone(),
                    Arc::clone(&transformation.output_schema),
                ),
            )
            .is_some()
        {
            return Err(SemanticReleaseError::DuplicateRelation);
        }
    }
    Ok(available)
}

fn compile_queries(
    definition: QueryProgramDefinition,
    available: &BTreeMap<ProviderRelationIdentity, (ProviderSchemaIdentity, SchemaRef)>,
    policy: &CompiledPolicyProgram,
) -> Result<CompiledQueryProgram, SemanticReleaseError> {
    let mut queries = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for query in definition.queries {
        if !identities.insert(query.identity.clone()) {
            return Err(SemanticReleaseError::DuplicateQuery);
        }
        if query.maximum_rows > policy.maximum_query_rows
            || query
                .required_relations
                .iter()
                .any(|relation| !available.contains_key(relation))
        {
            return Err(SemanticReleaseError::InvalidQueryDefinition);
        }
        if queries.insert(query.form, query).is_some() {
            return Err(SemanticReleaseError::DuplicateQuery);
        }
    }
    if queries.keys().copied().collect::<BTreeSet<_>>()
        != SemanticQueryForm::ALL.into_iter().collect::<BTreeSet<_>>()
    {
        return Err(SemanticReleaseError::QueryFormCoverage);
    }
    Ok(CompiledQueryProgram {
        identity: definition.identity,
        queries: Arc::new(queries),
        policy_maximum_rows: policy.maximum_query_rows,
    })
}

fn compile_proof(
    definition: ProofProgramDefinition,
    available: &BTreeMap<ProviderRelationIdentity, (ProviderSchemaIdentity, SchemaRef)>,
    queries: &CompiledQueryProgram,
) -> Result<CompiledProofProgram, SemanticReleaseError> {
    let required = queries
        .queries
        .values()
        .flat_map(|query| query.required_relations.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut expectation_ids = BTreeSet::new();
    let mut expected_relations = BTreeSet::new();
    for expectation in &definition.expectations {
        if !expectation_ids.insert(expectation.identity.clone())
            || !expected_relations.insert(expectation.relation.clone())
            || !available.contains_key(&expectation.relation)
        {
            return Err(SemanticReleaseError::InvalidProofProgram);
        }
    }
    if !required.is_subset(&expected_relations) {
        return Err(SemanticReleaseError::ProofCoverage);
    }
    let mut fault_ids = BTreeSet::new();
    for fault in &definition.faults {
        if !fault_ids.insert(fault.identity.clone())
            || !available.contains_key(&fault.target_relation)
        {
            return Err(SemanticReleaseError::InvalidProofProgram);
        }
    }
    Ok(CompiledProofProgram {
        identity: definition.identity,
        expectations: Arc::from(definition.expectations),
        faults: Arc::from(definition.faults),
    })
}

/// Standard v2.3 component dependency graph.
#[must_use]
pub(crate) fn current_dependency_graph() -> Vec<ProgramDependency> {
    vec![
        ProgramDependency::new(ReleaseComponent::Provider, ReleaseComponent::Policy),
        ProgramDependency::new(ReleaseComponent::Transformation, ReleaseComponent::Provider),
        ProgramDependency::new(ReleaseComponent::Query, ReleaseComponent::Transformation),
        ProgramDependency::new(ReleaseComponent::Query, ReleaseComponent::Policy),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Provider),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Transformation),
        ProgramDependency::new(ReleaseComponent::Proof, ReleaseComponent::Query),
    ]
}

/// Construct the sole target suite identity once.
pub(crate) fn current_suite_identity() -> Result<SuiteIdentity, SemanticReleaseError> {
    Ok(SuiteIdentity::try_new(CURRENT_SUITE)?)
}

/// Compile the sole v2.3 release around the exact provider relation/schema program assembled by
/// the current provider-definition module.
///
/// This is a transition constructor, not a runtime selector: callers must explicitly invoke it
/// once and inject the returned value. Every admitted provider relation becomes an explicit
/// identity-preserving release transformation input; the query and proof products are then
/// compiled against those exact relation values.
pub(crate) fn compile_current_v23_release(
    providers: ProviderProgramDefinition,
) -> Result<CompiledSemanticRelease, SemanticReleaseError> {
    let admitted = providers
        .lanes
        .iter()
        .flat_map(|lane| lane.families.iter())
        .enumerate()
        .map(|(ordinal, family)| {
            let output = ProviderRelationIdentity::try_new(format!(
                "released.admitted.{}",
                family.relation.as_str()
            ))?;
            let transformation = TransformationDefinition::try_new(
                TransformationIdentity::try_new(format!(
                    "codefabric.v2.3.admit.{ordinal}.{}",
                    family.relation.as_str()
                ))?,
                TransformationOperator::Identity,
                vec![family.relation.clone()],
                output.clone(),
                ProviderSchemaIdentity::try_new(format!(
                    "released.admitted.schema.v2.3.{}",
                    family.relation.as_str()
                ))?,
                Arc::clone(&family.schema),
            )?;
            Ok((output, transformation))
        })
        .collect::<Result<Vec<_>, SemanticReleaseError>>()?;
    if admitted.is_empty() {
        return Err(SemanticReleaseError::EmptyOrOversizedProviderProgram);
    }
    let admitted_relations = admitted
        .iter()
        .map(|(relation, _)| relation.clone())
        .collect::<Vec<_>>();
    let transformations = admitted
        .into_iter()
        .map(|(_, transformation)| transformation)
        .collect::<Vec<_>>();
    let queries = SemanticQueryForm::ALL
        .into_iter()
        .map(|form| {
            QueryDefinition::try_new(
                QueryIdentity::try_new(form.program_identity())?,
                form,
                admitted_relations.clone(),
                1_000_000,
            )
        })
        .collect::<Result<Vec<_>, SemanticReleaseError>>()?;
    let expectations = admitted_relations
        .iter()
        .enumerate()
        .map(|(ordinal, relation)| {
            Ok(ProofExpectationDefinition::new(
                ProofExpectationIdentity::try_new(format!(
                    "codefabric.proof.v2.3.relation-present.{ordinal}"
                ))?,
                relation.clone(),
                0,
            ))
        })
        .collect::<Result<Vec<_>, SemanticReleaseError>>()?;
    let effects = [
        CausalEffect::RejectAdmission,
        CausalEffect::ChangeTransformation,
        CausalEffect::ChangeQuery,
        CausalEffect::ChangeProofTerminal,
    ];
    let faults = effects
        .into_iter()
        .enumerate()
        .map(|(ordinal, effect)| {
            Ok(CausalFaultDefinition::new(
                CausalFaultIdentity::try_new(format!("codefabric.proof.v2.3.fault.{ordinal}"))?,
                admitted_relations[ordinal % admitted_relations.len()].clone(),
                effect,
            ))
        })
        .collect::<Result<Vec<_>, SemanticReleaseError>>()?;

    let syntax_ceiling = ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
        max_relations: MAX_PROVIDER_FAMILIES,
        max_batches_per_relation: 65_536,
        max_input_bytes: 16_777_216,
        max_rows: 2_000_000,
        max_bytes: 268_435_456,
        max_diagnostics: 10_000,
        max_work_units: 10_000_000,
        max_wall_millis: 30_000,
        max_visited_nodes: 2_000_000,
        max_traversal_depth: 256,
        max_workers: 4,
        max_retained_revisions: 2,
        cancellation_poll_work_units: 1_024,
        cancellation_ack_millis: 2_000,
    })?;
    let semantic_ceiling = |cancellation_ack_millis| {
        ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
            max_relations: MAX_PROVIDER_FAMILIES,
            max_batches_per_relation: 65_536,
            max_input_bytes: 67_108_864,
            max_rows: 4_000_000,
            max_bytes: 536_870_912,
            max_diagnostics: 20_000,
            max_work_units: 20_000_000,
            max_wall_millis: 120_000,
            max_visited_nodes: 4_000_000,
            max_traversal_depth: 512,
            max_workers: 2,
            max_retained_revisions: 1,
            cancellation_poll_work_units: 1_024,
            cancellation_ack_millis,
        })
    };
    let policy = PolicyProgramDefinition::try_new(
        PolicyProgramIdentity::try_new("codefabric.policy-program.v2.3")?,
        vec![
            LanePolicyDefinition::new(ProviderLane::TreeSitter, syntax_ceiling),
            LanePolicyDefinition::new(ProviderLane::Ruff, syntax_ceiling),
            LanePolicyDefinition::new(ProviderLane::Pyrefly, semantic_ceiling(2_000)?),
            LanePolicyDefinition::new(ProviderLane::Rustc, semantic_ceiling(10_000)?),
        ],
        1_000_000,
    )?;
    CompiledSemanticRelease::compile(CurrentSemanticReleaseDefinition::new(
        CurrentSemanticReleaseDefinitionParts {
            suite: current_suite_identity()?,
            providers,
            transformations: TransformationProgramDefinition::try_new(
                TransformationProgramIdentity::try_new("codefabric.transformation-program.v2.3")?,
                transformations,
            )?,
            queries: QueryProgramDefinition::try_new(
                QueryProgramIdentity::try_new("codefabric.query-program.v2.3")?,
                queries,
            )?,
            proof: ProofProgramDefinition::try_new(
                ProofProgramIdentity::try_new("codefabric.proof-program.v2.3")?,
                expectations,
                faults,
            )?,
            policy,
            dependencies: current_dependency_graph(),
        },
    ))
}

/// Release compiler failures. No variant permits fallback to a marker or runtime registry.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum SemanticReleaseError {
    #[error("application provider contract is invalid: {0}")]
    ProviderContract(#[from] ProviderContractError),
    #[error("program identity is empty, padded, control-bearing, or oversized")]
    InvalidProgramIdentity,
    #[error("compiled release suite is not the sole v2.3 suite")]
    StaleSuiteIdentity,
    #[error("categorically distinct program identities share one spelling")]
    ProgramIdentityConflation,
    #[error("release program dependency graph is not exactly closed")]
    ProgramDependencyClosure,
    #[error("provider program is empty or oversized")]
    EmptyOrOversizedProviderProgram,
    #[error("provider program omits one or more required lanes")]
    ProviderLaneCoverage,
    #[error("provider lane occurs more than once")]
    DuplicateProviderLane,
    #[error("provider family occurs more than once")]
    DuplicateProviderFamily,
    #[error("provider relation occurs more than once")]
    DuplicateRelation,
    #[error("provider lane is not compiled into this release")]
    UnknownProviderLane,
    #[error("provider family is not compiled into the selected lane")]
    UnknownProviderFamily,
    #[error("provider job wrapper was not minted by this release")]
    ForgedProviderJob,
    #[error("relation Arrow schema is empty")]
    EmptyArrowSchema,
    #[error("transformation operator has the wrong input arity")]
    TransformationArity,
    #[error("transformation program is empty or oversized")]
    EmptyOrOversizedTransformationProgram,
    #[error("transformation identity occurs more than once")]
    DuplicateTransformation,
    #[error("transformation depends on an unavailable relation")]
    OpenRelationDependency,
    #[error("transformation is not compiled into this release")]
    UnknownTransformation,
    #[error("query definition has empty inputs, a zero bound, an open relation, or exceeds policy")]
    InvalidQueryDefinition,
    #[error("query identity or form occurs more than once")]
    DuplicateQuery,
    #[error("query program does not contain exactly the eight released forms")]
    QueryFormCoverage,
    #[error("query form is not compiled into this release")]
    UnknownQueryForm,
    #[error("proof program is empty or oversized")]
    EmptyOrOversizedProofProgram,
    #[error("proof expectation or fault is duplicated or references an unavailable relation")]
    InvalidProofProgram,
    #[error("proof expectations do not cover every query dependency")]
    ProofCoverage,
    #[error("policy program is empty or invalid")]
    InvalidPolicyProgram,
    #[error("policy program does not cover exactly every provider lane")]
    PolicyLaneCoverage,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    #[cfg(feature = "data-fabric")]
    use datafusion::prelude::SessionContext;

    use super::*;
    use crate::provider_contracts::{
        ContextIdentity, ProviderContextBinding, ProviderCoverage, ProviderCoverageState,
        ProviderRunBinding, ProviderRunIdentity, ProviderRunResultSpec, ProviderSourceBinding,
        ProviderTerminalStatus, ProviderTrustOutcome, SourceIdentity,
    };

    fn schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]))
    }

    fn ceilings(max_relations: usize, max_rows: u64) -> ProviderResourceCeilings {
        ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
            max_relations,
            max_batches_per_relation: 16,
            max_input_bytes: 1 << 20,
            max_rows,
            max_bytes: 1 << 30,
            max_diagnostics: 1_000,
            max_work_units: 1_000_000,
            max_wall_millis: 30_000,
            max_visited_nodes: 1_000_000,
            max_traversal_depth: 256,
            max_workers: 4,
            max_retained_revisions: 2,
            cancellation_poll_work_units: 64,
            cancellation_ack_millis: 2_000,
        })
        .unwrap()
    }

    fn source_binding() -> ProviderSourceBinding {
        ProviderSourceBinding::try_new(
            SourceIdentity::try_new("source-1").unwrap(),
            [1; 16],
            1,
            [2; 32],
        )
        .unwrap()
    }

    fn context_binding() -> ProviderContextBinding {
        ProviderContextBinding::try_new(
            ContextIdentity::try_new("context-1").unwrap(),
            [3; 32],
            [4; 32],
        )
        .unwrap()
    }

    fn run_binding() -> ProviderRunBinding {
        ProviderRunBinding::try_new(ProviderRunIdentity::try_new("run-1").unwrap(), [5; 16])
            .unwrap()
    }

    fn provider_lane(lane: ProviderLane, name: &str) -> ProviderLaneProgramDefinition {
        ProviderLaneProgramDefinition::try_new(
            lane,
            ProviderIdentity::try_new(name).unwrap(),
            ProviderProtocolIdentity::try_new(format!("{name}.protocol.v1")).unwrap(),
            ProviderBuildIdentity::try_new(format!("{name}.build.v1")).unwrap(),
            match lane {
                ProviderLane::TreeSitter | ProviderLane::Ruff => {
                    ProviderTrustPosture::InProcessConstrained
                }
                ProviderLane::Pyrefly => ProviderTrustPosture::LocalSidecarConstrained,
                ProviderLane::Rustc => ProviderTrustPosture::CompilerSubprocessConstrained,
            },
            vec![
                ProviderFamilyProgramDefinition::try_new(
                    ProviderFamilyIdentity::try_new(format!("{name}.family")).unwrap(),
                    ProviderRelationIdentity::try_new(format!("{name}.raw")).unwrap(),
                    ProviderSchemaIdentity::try_new(format!("{name}.raw.schema.v1")).unwrap(),
                    schema(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn fixture_definition() -> CurrentSemanticReleaseDefinition {
        let lanes = vec![
            provider_lane(ProviderLane::TreeSitter, "tree-sitter"),
            provider_lane(ProviderLane::Ruff, "ruff"),
            provider_lane(ProviderLane::Pyrefly, "pyrefly"),
            provider_lane(ProviderLane::Rustc, "rustc"),
        ];
        let raw = lanes
            .iter()
            .map(|lane| lane.families[0].relation.clone())
            .collect::<Vec<_>>();
        let normalized = raw
            .iter()
            .enumerate()
            .map(|(index, input)| {
                TransformationDefinition::try_new(
                    TransformationIdentity::try_new(format!("normalize.{index}")).unwrap(),
                    TransformationOperator::Normalize,
                    vec![input.clone()],
                    ProviderRelationIdentity::try_new(format!("normalized.{index}")).unwrap(),
                    ProviderSchemaIdentity::try_new(format!("normalized.{index}.schema.v1"))
                        .unwrap(),
                    schema(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let normalized_relations = normalized
            .iter()
            .map(|value| value.output.clone())
            .collect::<Vec<_>>();
        let queries = SemanticQueryForm::ALL
            .into_iter()
            .enumerate()
            .map(|(index, form)| {
                QueryDefinition::try_new(
                    QueryIdentity::try_new(format!("query.{index}")).unwrap(),
                    form,
                    vec![normalized_relations[index % normalized_relations.len()].clone()],
                    128,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let expectations = normalized_relations
            .iter()
            .enumerate()
            .map(|(index, relation)| {
                ProofExpectationDefinition::new(
                    ProofExpectationIdentity::try_new(format!("expectation.{index}")).unwrap(),
                    relation.clone(),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let faults = vec![CausalFaultDefinition::new(
            CausalFaultIdentity::try_new("fault.remove.normalized.0").unwrap(),
            normalized_relations[0].clone(),
            CausalEffect::ChangeQuery,
        )];
        let ceilings = ceilings(64, 1_000_000);
        CurrentSemanticReleaseDefinition::new(CurrentSemanticReleaseDefinitionParts {
            suite: current_suite_identity().unwrap(),
            providers: ProviderProgramDefinition::try_new(
                ProviderProgramIdentity::try_new("codefabric.provider-program.v2.3").unwrap(),
                lanes,
            )
            .unwrap(),
            transformations: TransformationProgramDefinition::try_new(
                TransformationProgramIdentity::try_new("codefabric.transformation-program.v2.3")
                    .unwrap(),
                normalized,
            )
            .unwrap(),
            queries: QueryProgramDefinition::try_new(
                QueryProgramIdentity::try_new("codefabric.query-program.v2.3").unwrap(),
                queries,
            )
            .unwrap(),
            proof: ProofProgramDefinition::try_new(
                ProofProgramIdentity::try_new("codefabric.proof-program.v2.3").unwrap(),
                expectations,
                faults,
            )
            .unwrap(),
            policy: PolicyProgramDefinition::try_new(
                PolicyProgramIdentity::try_new("codefabric.policy-program.v2.3").unwrap(),
                ProviderLane::ALL
                    .into_iter()
                    .map(|lane| LanePolicyDefinition::new(lane, ceilings))
                    .collect(),
                128,
            )
            .unwrap(),
            dependencies: current_dependency_graph(),
        })
    }

    fn compiled() -> CompiledSemanticRelease {
        CompiledSemanticRelease::compile(fixture_definition()).unwrap()
    }

    #[test]
    fn compiled_release_program_identity_integrity() {
        let release = compiled();
        assert_eq!(release.suite().as_str(), CURRENT_SUITE);
        assert_eq!(release.observation().provider_lanes, 4);
        assert_eq!(release.observation().provider_relations, 4);
        assert_eq!(release.observation().transformations, 4);
        assert_eq!(release.observation().query_forms, 8);
        assert_eq!(release.observation().proof_expectations, 4);
        assert_eq!(release.observation().causal_faults, 1);
        assert!(!release.transformations().is_empty());
        assert!(!release.queries().is_empty());
    }

    #[test]
    fn compiled_release_operand_causality() {
        let mut missing_lane = fixture_definition();
        missing_lane.providers.lanes.pop();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_lane).unwrap_err(),
            SemanticReleaseError::ProviderLaneCoverage
        );

        let mut missing_transformation = fixture_definition();
        missing_transformation
            .transformations
            .transformations
            .clear();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_transformation).unwrap_err(),
            SemanticReleaseError::InvalidQueryDefinition
        );

        let mut missing_query = fixture_definition();
        missing_query.queries.queries.pop();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_query).unwrap_err(),
            SemanticReleaseError::QueryFormCoverage
        );

        let mut missing_expectation = fixture_definition();
        missing_expectation.proof.expectations.pop();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_expectation).unwrap_err(),
            SemanticReleaseError::ProofCoverage
        );

        let mut missing_policy = fixture_definition();
        missing_policy.policy.lanes.pop();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_policy).unwrap_err(),
            SemanticReleaseError::PolicyLaneCoverage
        );

        let mut missing_edge = fixture_definition();
        missing_edge.dependencies.pop();
        assert_eq!(
            CompiledSemanticRelease::compile(missing_edge).unwrap_err(),
            SemanticReleaseError::ProgramDependencyClosure
        );
    }

    #[test]
    fn compiled_release_forgery_and_conflation_faults() {
        let mut stale = fixture_definition();
        stale.suite = SuiteIdentity::try_new("codefabric-relational-data-fabric@2.1.0").unwrap();
        assert_eq!(
            CompiledSemanticRelease::compile(stale).unwrap_err(),
            SemanticReleaseError::StaleSuiteIdentity
        );

        let mut conflated = fixture_definition();
        conflated.transformations.identity =
            TransformationProgramIdentity::try_new("codefabric.provider-program.v2.3").unwrap();
        assert_eq!(
            CompiledSemanticRelease::compile(conflated).unwrap_err(),
            SemanticReleaseError::ProgramIdentityConflation
        );

        let release = compiled();
        let (_, cancellation) = CancellationProbe::pair(64).unwrap();
        let mut prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::TreeSitter,
                    source: source_binding(),
                    context: context_binding(),
                    run: run_binding(),
                    scope: ProviderScopeIdentity::try_new("workspace").unwrap(),
                    requested_families: vec![(
                        ProviderFamilyIdentity::try_new("tree-sitter.family").unwrap(),
                        2,
                    )],
                    operational_ceilings: ceilings(2, 10),
                    deadline: Instant::now() + Duration::from_secs(5),
                    cancellation,
                },
            )
            .unwrap();
        prepared.provider_program = ProviderProgramIdentity::try_new("forged-program").unwrap();
        let result = tree_sitter_result();
        assert_eq!(
            release.providers().admit(prepared, result).unwrap_err(),
            SemanticReleaseError::ForgedProviderJob
        );
    }

    #[test]
    fn compiled_release_query_program_operations() {
        let release = compiled();
        let mut observed = BTreeSet::new();
        for form in SemanticQueryForm::ALL {
            let plan = release.queries().compile(form).unwrap();
            assert_eq!(plan.form, form);
            assert_eq!(plan.required_relations.len(), 1);
            assert_eq!(plan.maximum_rows, 128);
            observed.insert(plan.identity);
        }
        assert_eq!(observed.len(), SemanticQueryForm::ALL.len());

        let transformation = release
            .transformations()
            .compile(&ProviderRelationIdentity::try_new("normalized.0").unwrap())
            .unwrap();
        assert_eq!(transformation.operator, TransformationOperator::Normalize);
        assert_eq!(transformation.inputs[0].as_str(), "tree-sitter.raw");
        assert_eq!(transformation.output.as_str(), "normalized.0");

        let proof = release.proof().construct_input();
        assert_eq!(proof.expectations.len(), 4);
        assert_eq!(proof.faults.len(), 1);
    }

    #[cfg(feature = "data-fabric")]
    #[tokio::test]
    async fn compiled_release_query_program_executes_datafusion_fixture() {
        let release = compiled();
        let relation_batches = (0_i64..4)
            .map(|value| {
                let schema = schema();
                let batch = RecordBatch::try_new(
                    Arc::clone(&schema),
                    vec![Arc::new(Int64Array::from(vec![value]))],
                )
                .unwrap();
                (
                    ProviderRelationIdentity::try_new(format!("normalized.{value}")).unwrap(),
                    batch,
                )
            })
            .collect::<BTreeMap<_, _>>();

        for (ordinal, form) in SemanticQueryForm::ALL.into_iter().enumerate() {
            let plan = release.queries().compile(form).unwrap();
            let selected = relation_batches.get(&plan.required_relations[0]).unwrap();
            let data = SessionContext::new()
                .read_batch(selected.clone())
                .unwrap()
                .limit(0, Some(usize::try_from(plan.maximum_rows).unwrap()))
                .unwrap()
                .collect()
                .await
                .unwrap();
            let values = data[0]
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            assert_eq!(values.value(0), i64::try_from(ordinal % 4).unwrap());
        }
    }

    #[cfg(feature = "data-fabric")]
    #[tokio::test]
    async fn semantic_release_provider_to_proof_fixture() {
        let target = ProviderRelationIdentity::try_new("normalized.0").unwrap();
        let mut definition = fixture_definition();
        for query in &mut definition.queries.queries {
            query.required_relations = vec![target.clone()];
        }
        definition.proof.expectations = vec![ProofExpectationDefinition::new(
            ProofExpectationIdentity::try_new("expectation.provider-to-proof").unwrap(),
            target.clone(),
            2,
        )];
        let release = CompiledSemanticRelease::compile(definition).unwrap();

        let (_, cancellation) = CancellationProbe::pair(64).unwrap();
        let prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::TreeSitter,
                    source: source_binding(),
                    context: context_binding(),
                    run: run_binding(),
                    scope: ProviderScopeIdentity::try_new("workspace").unwrap(),
                    requested_families: vec![(
                        ProviderFamilyIdentity::try_new("tree-sitter.family").unwrap(),
                        2,
                    )],
                    operational_ceilings: ceilings(2, 10),
                    deadline: Instant::now() + Duration::from_secs(5),
                    cancellation,
                },
            )
            .unwrap();
        let admitted = release
            .providers()
            .admit(prepared, tree_sitter_result())
            .unwrap();
        let raw = &admitted.result().relations()[0];

        let transformation = release.transformations().compile(&target).unwrap();
        assert_eq!(transformation.operator, TransformationOperator::Normalize);
        assert_eq!(transformation.inputs.as_ref(), [raw.relation().clone()]);
        assert_eq!(transformation.output_schema, raw.schema().clone());

        let transformed = SessionContext::new()
            .read_batches(raw.batches().to_vec())
            .unwrap()
            .collect()
            .await
            .unwrap();
        let transformed_rows = transformed.iter().map(RecordBatch::num_rows).sum::<usize>();
        assert_eq!(transformed_rows, 2);

        for form in SemanticQueryForm::ALL {
            let query = release.queries().compile(form).unwrap();
            assert_eq!(query.required_relations.as_ref(), [target.clone()]);
            let result = SessionContext::new()
                .read_batches(transformed.clone())
                .unwrap()
                .limit(0, Some(usize::try_from(query.maximum_rows).unwrap()))
                .unwrap()
                .collect()
                .await
                .unwrap();
            assert_eq!(result.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
        }

        let proof = release.proof().construct_input();
        let proof_passes = |row_count: usize| {
            proof.expectations.iter().all(|(_, relation, minimum)| {
                relation == &target && u64::try_from(row_count).unwrap() >= *minimum
            })
        };
        assert!(proof_passes(transformed_rows));
        assert!(
            !proof_passes(1),
            "a causal provider-row loss must change the independent proof terminal"
        );
    }

    #[test]
    fn release_provider_preparation_and_admission_are_causal() {
        let release = compiled();
        let (_, cancellation) = CancellationProbe::pair(64).unwrap();
        let prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::TreeSitter,
                    source: source_binding(),
                    context: context_binding(),
                    run: run_binding(),
                    scope: ProviderScopeIdentity::try_new("workspace").unwrap(),
                    requested_families: vec![(
                        ProviderFamilyIdentity::try_new("tree-sitter.family").unwrap(),
                        2,
                    )],
                    operational_ceilings: ceilings(2, 10),
                    deadline: Instant::now() + Duration::from_secs(5),
                    cancellation,
                },
            )
            .unwrap();
        let admitted = release
            .providers()
            .admit(prepared, tree_sitter_result())
            .unwrap();
        assert_eq!(admitted.observation().emitted_relations, 1);
    }

    fn tree_sitter_result() -> ProviderRunResult {
        let schema = schema();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![1_i64, 2]))],
        )
        .unwrap();
        ProviderRunResult::try_new(ProviderRunResultSpec {
            suite: current_suite_identity().unwrap(),
            provider: ProviderIdentity::try_new("tree-sitter").unwrap(),
            protocol: ProviderProtocolIdentity::try_new("tree-sitter.protocol.v1").unwrap(),
            source: source_binding(),
            context: context_binding(),
            run: run_binding(),
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new("tree-sitter.build.v1").unwrap(),
                ProviderPolicyIdentity::try_new("codefabric.policy-program.v2.3").unwrap(),
                ProviderProgramIdentity::try_new("codefabric.provider-program.v2.3").unwrap(),
            ),
            relations: vec![
                crate::provider_contracts::ProviderRelationOutput::try_new(
                    ProviderRelationIdentity::try_new("tree-sitter.raw").unwrap(),
                    ProviderSchemaIdentity::try_new("tree-sitter.raw.schema.v1").unwrap(),
                    schema,
                    vec![batch],
                )
                .unwrap(),
            ],
            coverage: vec![ProviderCoverage::new(
                ProviderFamilyIdentity::try_new("tree-sitter.family").unwrap(),
                ProviderCoverageState::Complete { completed_units: 2 },
            )],
            gaps: Vec::new(),
            diagnostics: Vec::new(),
            trust: ProviderTrustOutcome::Trusted,
            terminal: ProviderTerminalStatus::Complete,
        })
        .unwrap()
    }
}
