//! Independent source examples and executable admission obligations owned by the release.
//!
//! These are expected answers, never provider observations or a mutable capability registry.
//! The candidate validator accepts only the declared temporary gaps for the unreleased Python
//! analyses. The fixture validator separately checks exact source-bound conformance examples;
//! accepting a required gap explicitly does not prove the preregistered target semantics.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{CompiledProofProgram, SemanticReleaseError};

const MAX_FIXTURES: usize = 128;
const MAX_FIXTURE_SOURCE_BYTES: usize = 65_536;
const MAX_FIXTURE_FACTS: usize = 1_024;
const MAX_WITNESS_BYTES: usize = 512;

/// Required Python analyses whose provisional file-sequential implementation is withdrawn.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequiredAnalysisFamily {
    PythonCfgNode,
    PythonCfgEdge,
    PythonEvaluationOrder,
    PythonDefUse,
    PythonReachingDefinition,
    PythonLiveness,
    PythonValueFlow,
}

impl RequiredAnalysisFamily {
    pub const ALL: [Self; 7] = [
        Self::PythonCfgNode,
        Self::PythonCfgEdge,
        Self::PythonEvaluationOrder,
        Self::PythonDefUse,
        Self::PythonReachingDefinition,
        Self::PythonLiveness,
        Self::PythonValueFlow,
    ];

    /// Relation identity selected by this behavior-bearing obligation.
    #[must_use]
    pub const fn relation_identity(self) -> &'static str {
        match self {
            Self::PythonCfgNode => "application.python.cfg_node",
            Self::PythonCfgEdge => "application.python.cfg_edge",
            Self::PythonEvaluationOrder => "application.python.evaluation_order",
            Self::PythonDefUse => "application.python.def_use",
            Self::PythonReachingDefinition => "application.python.reaching_definition",
            Self::PythonLiveness => "application.python.liveness",
            Self::PythonValueFlow => "application.python.value_flow",
        }
    }

    /// Project an observed relation into the required subset; unrelated families are not claims.
    #[must_use]
    pub fn from_relation(relation: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|family| family.relation_identity() == relation)
    }
}

/// Precision of a named application abstraction, never inferred from available inputs.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisPrecision {
    Exact,
    SoundMay,
    SoundMust,
    Bounded { max_steps: u32 },
}

/// Actual reason a required family has no executable current result.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisGapReason {
    AlgorithmUnavailable,
    ProviderUnavailable,
    ResourceLimit,
    Unsupported,
    PrivateCompilerEvidenceUnavailable,
    TypedTransformationAdapterUnavailable,
}

/// A missing implementation cannot be repaired by retrying the same release.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisGapRetryability {
    Retryable,
    RequiresReleaseChange,
    PermanentlyUnsupported,
}

/// Observation projected from an executed producer or explicit remainder, not a caller waiver.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisDisposition {
    Complete {
        precision: AnalysisPrecision,
    },
    KnownEmpty {
        precision: AnalysisPrecision,
    },
    LanguageUncertainty,
    /// Incomplete output with retained unknown evidence, without guessing its semantic cause.
    Incomplete,
    Unavailable {
        reason: AnalysisGapReason,
        retryability: AnalysisGapRetryability,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RequiredAnalysisObservation {
    pub family: RequiredAnalysisFamily,
    pub disposition: AnalysisDisposition,
}

/// A successful admission check is not synonymous with full selected-profile conformance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredAnalysisConformance {
    /// The observed result matches a fully evaluated independent fixture expectation.
    Satisfied,
    /// Truthful required gaps were verified; the selected functionality remains unimplemented.
    PendingImplementation,
}

/// Source-level witness vocabulary for independently authored small examples.
///
/// Witness labels name source occurrences (for example `value@3`) and owners. An executable
/// provider/analysis harness must resolve these against the exact source, not hash the expected
/// labels into fabricated canonical facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FixtureFactKind {
    CfgNode,
    CfgEdge,
    EvaluationOrder,
    DefUse,
    ReachingDefinition,
    LiveAt,
    ValueFlow,
    Owner,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FixtureFact {
    pub kind: FixtureFactKind,
    pub owner: Arc<str>,
    pub source: Arc<str>,
    pub target: Arc<str>,
}

/// Positive and negative target obligations are distinct from current gap acceptance.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FixtureAssertion {
    Present(FixtureFact),
    Absent(FixtureFact),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FixtureUnknownCause {
    DynamicDispatch,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FixtureUnknown {
    pub owner: Arc<str>,
    pub occurrence: Arc<str>,
    pub cause: FixtureUnknownCause,
}

/// Exact observations from executing one released source fixture.
///
/// This is a narrow owned observation DTO, not a proof certificate. Only validation against the
/// separately compiled expectations establishes its conformance, and only for that fixture.
#[derive(Clone, Debug)]
pub struct FixtureResult<'a> {
    pub fixture_id: &'a str,
    pub source: &'a str,
    pub facts: &'a [FixtureFact],
    pub unknowns: &'a [FixtureUnknown],
    pub analyses: &'a [RequiredAnalysisObservation],
}

#[derive(Clone, Debug)]
struct RequiredFamilyObligation {
    family: RequiredAnalysisFamily,
    target_precision: AnalysisPrecision,
    current_disposition: AnalysisDisposition,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FixtureCase {
    Branch,
    Loop,
    Return,
    NestedCallable,
    KnownEmpty,
    DynamicCall,
}

impl FixtureCase {
    const ALL: [Self; 6] = [
        Self::Branch,
        Self::Loop,
        Self::Return,
        Self::NestedCallable,
        Self::KnownEmpty,
        Self::DynamicCall,
    ];

    const fn identity(self) -> &'static str {
        match self {
            Self::Branch => "python.branch-two-reaching-definitions.v1",
            Self::Loop => "python.loop-back-edge.v1",
            Self::Return => "python.return-no-fallthrough.v1",
            Self::NestedCallable => "python.nested-callable-isolation.v1",
            Self::KnownEmpty => "python.known-empty-callable-body-set.v1",
            Self::DynamicCall => "python.admitted-dynamic-call-uncertainty.v1",
        }
    }

    const fn required_families(self) -> &'static [RequiredAnalysisFamily] {
        match self {
            Self::Branch => &RequiredAnalysisFamily::ALL,
            Self::Loop | Self::Return => &[RequiredAnalysisFamily::PythonCfgEdge],
            Self::NestedCallable => &[
                RequiredAnalysisFamily::PythonCfgNode,
                RequiredAnalysisFamily::PythonCfgEdge,
            ],
            Self::KnownEmpty | Self::DynamicCall => &[],
        }
    }
}

/// Immutable example exposed to the execution harness without a mutable expected-answer input.
#[derive(Clone, Debug)]
pub struct ConformanceFixture {
    case: FixtureCase,
    source: Arc<str>,
    required_families: Arc<[RequiredAnalysisFamily]>,
    current_facts: Arc<[FixtureFact]>,
    current_unknowns: Arc<[FixtureUnknown]>,
    target_assertions: Arc<[FixtureAssertion]>,
}

impl ConformanceFixture {
    #[must_use]
    pub fn id(&self) -> &str {
        self.case.identity()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn required_families(&self) -> &[RequiredAnalysisFamily] {
        &self.required_families
    }

    /// Preregistered target examples. Their presence is not evidence they currently pass.
    #[must_use]
    pub fn target_assertions(&self) -> &[FixtureAssertion] {
        &self.target_assertions
    }
}

#[derive(Clone, Debug)]
pub(super) struct IndependentProofDefinition {
    obligations: Vec<RequiredFamilyObligation>,
    fixtures: Vec<ConformanceFixture>,
}

#[derive(Clone, Debug)]
pub(super) struct IndependentProofContract {
    obligations: Arc<[RequiredFamilyObligation]>,
    fixtures: Arc<[ConformanceFixture]>,
}

const fn pending_implementation() -> AnalysisDisposition {
    AnalysisDisposition::Unavailable {
        reason: AnalysisGapReason::AlgorithmUnavailable,
        retryability: AnalysisGapRetryability::RequiresReleaseChange,
    }
}

fn witness(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WITNESS_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_fact(fact: &FixtureFact) -> bool {
    witness(&fact.owner) && witness(&fact.source) && witness(&fact.target)
}

impl IndependentProofContract {
    pub(super) fn compile(
        definition: IndependentProofDefinition,
    ) -> Result<Self, SemanticReleaseError> {
        let fail = SemanticReleaseError::InvalidIndependentProof;
        let mut families = BTreeSet::new();
        for obligation in &definition.obligations {
            if !families.insert(obligation.family)
                || obligation.current_disposition != pending_implementation()
                || obligation.target_precision != target_precision(obligation.family)
            {
                return Err(fail("duplicate or contradictory required family"));
            }
        }
        if families != RequiredAnalysisFamily::ALL.into_iter().collect() {
            return Err(fail("required family expectation is missing"));
        }
        if definition.fixtures.is_empty() || definition.fixtures.len() > MAX_FIXTURES {
            return Err(fail("independent source fixtures are missing or oversized"));
        }
        let mut fixture_cases = BTreeSet::new();
        let mut covered = BTreeSet::new();
        for fixture in &definition.fixtures {
            if !fixture_cases.insert(fixture.case)
                || fixture.source.len() > MAX_FIXTURE_SOURCE_BYTES
                || fixture.target_assertions.len() > MAX_FIXTURE_FACTS
            {
                return Err(fail("invalid or duplicate source fixture"));
            }
            let required = fixture
                .required_families
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if required.len() != fixture.required_families.len()
                || required != fixture.case.required_families().iter().copied().collect()
                || !required.is_subset(&families)
            {
                return Err(fail("fixture family coverage is duplicated or unbound"));
            }
            if !valid_current_expectation(fixture) {
                return Err(fail(
                    "fixture fact or unknown expectation is missing or contradictory",
                ));
            }
            if !required.is_empty()
                && (fixture.target_assertions.is_empty()
                    || !fixture.current_facts.is_empty()
                    || !fixture.current_unknowns.is_empty())
            {
                return Err(fail(
                    "pending implementation cannot claim facts or language uncertainty",
                ));
            }
            let mut assertions = BTreeMap::new();
            for assertion in fixture.target_assertions.iter() {
                let (fact, present) = match assertion {
                    FixtureAssertion::Present(fact) => (fact, true),
                    FixtureAssertion::Absent(fact) => (fact, false),
                };
                if !valid_fact(fact) || assertions.insert(fact.clone(), present).is_some() {
                    return Err(fail(
                        "duplicate, contradictory or malformed target assertion",
                    ));
                }
            }
            covered.extend(required);
        }
        if covered != families || fixture_cases != FixtureCase::ALL.into_iter().collect() {
            return Err(fail("required family lacks an independent source fixture"));
        }
        Ok(Self {
            obligations: definition.obligations.into(),
            fixtures: definition.fixtures.into(),
        })
    }

    /// Canonical typed framing binds every operand; diagnostic formatting is never identity.
    pub(super) fn content_identity(&self) -> [u8; 32] {
        let mut digest = blake3::Hasher::new();
        digest.update(b"codefabric.independent-proof-contract.v1\0");
        let mut obligations = self.obligations.iter().collect::<Vec<_>>();
        obligations.sort_by_key(|obligation| obligation.family);
        hash_count(&mut digest, obligations.len());
        for obligation in obligations {
            hash_frame(
                &mut digest,
                obligation.family.relation_identity().as_bytes(),
            );
            hash_precision(&mut digest, obligation.target_precision);
            hash_disposition(&mut digest, obligation.current_disposition);
        }
        let mut fixtures = self.fixtures.iter().collect::<Vec<_>>();
        fixtures.sort_by_key(|fixture| fixture.case);
        hash_count(&mut digest, fixtures.len());
        for fixture in fixtures {
            hash_frame(&mut digest, fixture.id().as_bytes());
            hash_frame(&mut digest, fixture.source.as_bytes());
            let mut families = fixture.required_families.to_vec();
            families.sort();
            hash_count(&mut digest, families.len());
            for family in families {
                hash_frame(&mut digest, family.relation_identity().as_bytes());
            }
            let mut facts = fixture.current_facts.to_vec();
            facts.sort();
            hash_count(&mut digest, facts.len());
            for fact in &facts {
                hash_fact(&mut digest, fact);
            }
            let mut unknowns = fixture.current_unknowns.to_vec();
            unknowns.sort();
            hash_count(&mut digest, unknowns.len());
            for unknown in &unknowns {
                hash_frame(&mut digest, unknown.owner.as_bytes());
                hash_frame(&mut digest, unknown.occurrence.as_bytes());
                digest.update(&[match unknown.cause {
                    FixtureUnknownCause::DynamicDispatch => 0,
                }]);
            }
            let mut assertions = fixture.target_assertions.to_vec();
            assertions.sort();
            hash_count(&mut digest, assertions.len());
            for assertion in &assertions {
                let (fact, present) = match assertion {
                    FixtureAssertion::Present(fact) => (fact, true),
                    FixtureAssertion::Absent(fact) => (fact, false),
                };
                digest.update(&[u8::from(present)]);
                hash_fact(&mut digest, fact);
            }
        }
        *digest.finalize().as_bytes()
    }

    fn validate_analyses(
        &self,
        expected: &[RequiredAnalysisFamily],
        observations: &[RequiredAnalysisObservation],
    ) -> Result<RequiredAnalysisConformance, SemanticReleaseError> {
        let fail = SemanticReleaseError::InvalidRequiredAnalysisObservation;
        if observations.len() != expected.len() {
            return Err(fail("required family coverage is incomplete or oversized"));
        }
        let mut seen = BTreeSet::new();
        for observed in observations {
            let obligation = self
                .obligations
                .iter()
                .find(|obligation| obligation.family == observed.family)
                .ok_or_else(|| fail("unbound required family"))?;
            if !expected.contains(&observed.family) || !seen.insert(observed.family) {
                return Err(fail("required family is unexpected or duplicated"));
            }
            if observed.disposition != obligation.current_disposition {
                return Err(fail(
                    "unimplemented family must retain its exact release-change gap",
                ));
            }
        }
        Ok(if observations.is_empty() {
            RequiredAnalysisConformance::Satisfied
        } else {
            RequiredAnalysisConformance::PendingImplementation
        })
    }
}

fn valid_current_expectation(fixture: &ConformanceFixture) -> bool {
    if !fixture.current_facts.is_empty() {
        return false;
    }
    match fixture.case {
        FixtureCase::DynamicCall => {
            fixture.target_assertions.is_empty()
                && fixture.current_unknowns.len() == 1
                && fixture
                    .current_unknowns
                    .iter()
                    .all(|unknown| witness(&unknown.owner) && witness(&unknown.occurrence))
        }
        FixtureCase::KnownEmpty => {
            fixture.target_assertions.is_empty() && fixture.current_unknowns.is_empty()
        }
        FixtureCase::Branch
        | FixtureCase::Loop
        | FixtureCase::Return
        | FixtureCase::NestedCallable => fixture.current_unknowns.is_empty(),
    }
}

pub(super) fn hash_count(digest: &mut blake3::Hasher, count: usize) {
    digest.update(
        &u64::try_from(count)
            .expect("proof operand counts fit u64")
            .to_be_bytes(),
    );
}

pub(super) fn hash_frame(digest: &mut blake3::Hasher, bytes: &[u8]) {
    hash_count(digest, bytes.len());
    digest.update(bytes);
}

fn hash_precision(digest: &mut blake3::Hasher, precision: AnalysisPrecision) {
    let (tag, steps) = match precision {
        AnalysisPrecision::Exact => (0, 0),
        AnalysisPrecision::SoundMay => (1, 0),
        AnalysisPrecision::SoundMust => (2, 0),
        AnalysisPrecision::Bounded { max_steps } => (3, max_steps),
    };
    digest.update(&[tag]);
    digest.update(&steps.to_be_bytes());
}

fn hash_disposition(digest: &mut blake3::Hasher, disposition: AnalysisDisposition) {
    match disposition {
        AnalysisDisposition::Complete { precision } => {
            digest.update(&[0]);
            hash_precision(digest, precision);
        }
        AnalysisDisposition::KnownEmpty { precision } => {
            digest.update(&[1]);
            hash_precision(digest, precision);
        }
        AnalysisDisposition::LanguageUncertainty => {
            digest.update(&[2]);
        }
        AnalysisDisposition::Incomplete => {
            digest.update(&[4]);
        }
        AnalysisDisposition::Unavailable {
            reason,
            retryability,
        } => {
            digest.update(&[3]);
            digest.update(&[match reason {
                AnalysisGapReason::AlgorithmUnavailable => 0,
                AnalysisGapReason::ProviderUnavailable => 1,
                AnalysisGapReason::ResourceLimit => 2,
                AnalysisGapReason::Unsupported => 3,
                AnalysisGapReason::PrivateCompilerEvidenceUnavailable => 4,
                AnalysisGapReason::TypedTransformationAdapterUnavailable => 5,
            }]);
            digest.update(&[match retryability {
                AnalysisGapRetryability::Retryable => 0,
                AnalysisGapRetryability::RequiresReleaseChange => 1,
                AnalysisGapRetryability::PermanentlyUnsupported => 2,
            }]);
        }
    }
}

fn hash_fact(digest: &mut blake3::Hasher, fact: &FixtureFact) {
    digest.update(&[match fact.kind {
        FixtureFactKind::CfgNode => 0,
        FixtureFactKind::CfgEdge => 1,
        FixtureFactKind::EvaluationOrder => 2,
        FixtureFactKind::DefUse => 3,
        FixtureFactKind::ReachingDefinition => 4,
        FixtureFactKind::LiveAt => 5,
        FixtureFactKind::ValueFlow => 6,
        FixtureFactKind::Owner => 7,
    }]);
    hash_frame(digest, fact.owner.as_bytes());
    hash_frame(digest, fact.source.as_bytes());
    hash_frame(digest, fact.target.as_bytes());
}

impl CompiledProofProgram {
    /// Check actual candidate observations without applying a fixture's source-specific facts.
    ///
    /// # Errors
    /// Rejects missing/duplicate families, false complete/empty/uncertainty claims, or a gap whose
    /// reason/retryability masks an unimplemented required analysis.
    pub fn validate_required_analysis_observations(
        &self,
        observations: &[RequiredAnalysisObservation],
    ) -> Result<RequiredAnalysisConformance, SemanticReleaseError> {
        self.independent
            .validate_analyses(&RequiredAnalysisFamily::ALL, observations)
    }

    #[must_use]
    pub fn conformance_fixtures(&self) -> &[ConformanceFixture] {
        &self.independent.fixtures
    }

    /// Compare exact source-bound fixture output to independently authored expected values.
    ///
    /// # Errors
    /// Rejects another source or fixture, missing/extra/wrong facts or unknowns, duplicate rows,
    /// malformed witnesses, and false required-family claims. Preregistered target assertions
    /// are never promoted to proved facts by accepting current required gaps.
    pub fn validate_fixture_result(
        &self,
        result: &FixtureResult<'_>,
    ) -> Result<RequiredAnalysisConformance, SemanticReleaseError> {
        let fail = SemanticReleaseError::IndependentFixtureMismatch;
        let fixture = self
            .independent
            .fixtures
            .iter()
            .find(|fixture| fixture.id() == result.fixture_id)
            .ok_or_else(|| fail("unknown fixture identity"))?;
        if result.source != fixture.source()
            || result.facts.len() > MAX_FIXTURE_FACTS
            || result.unknowns.len() > MAX_FIXTURE_FACTS
            || result.facts.iter().any(|fact| !valid_fact(fact))
            || result
                .unknowns
                .iter()
                .any(|unknown| !witness(&unknown.owner) || !witness(&unknown.occurrence))
        {
            return Err(fail("source or bounded observation contract differs"));
        }
        let facts = result.facts.iter().cloned().collect::<BTreeSet<_>>();
        let unknowns = result.unknowns.iter().cloned().collect::<BTreeSet<_>>();
        if facts.len() != result.facts.len()
            || unknowns.len() != result.unknowns.len()
            || facts != fixture.current_facts.iter().cloned().collect()
            || unknowns != fixture.current_unknowns.iter().cloned().collect()
        {
            return Err(fail(
                "observed facts or unknowns differ from independent expectation",
            ));
        }
        self.independent
            .validate_analyses(fixture.required_families(), result.analyses)
    }
}

const fn target_precision(family: RequiredAnalysisFamily) -> AnalysisPrecision {
    match family {
        RequiredAnalysisFamily::PythonCfgNode
        | RequiredAnalysisFamily::PythonReachingDefinition
        | RequiredAnalysisFamily::PythonLiveness
        | RequiredAnalysisFamily::PythonValueFlow => AnalysisPrecision::Exact,
        RequiredAnalysisFamily::PythonCfgEdge
        | RequiredAnalysisFamily::PythonEvaluationOrder
        | RequiredAnalysisFamily::PythonDefUse => AnalysisPrecision::SoundMay,
    }
}

fn fact(kind: FixtureFactKind, owner: &str, source: &str, target: &str) -> FixtureFact {
    FixtureFact {
        kind,
        owner: Arc::from(owner),
        source: Arc::from(source),
        target: Arc::from(target),
    }
}

fn pending_fixture(
    case: FixtureCase,
    source: &str,
    target_assertions: Vec<FixtureAssertion>,
) -> ConformanceFixture {
    ConformanceFixture {
        case,
        source: Arc::from(source),
        required_families: Arc::from(case.required_families()),
        current_facts: Arc::from([]),
        current_unknowns: Arc::from([]),
        target_assertions: target_assertions.into(),
    }
}

/// Literal authored examples: no provider, parser, derived census or candidate supplies these.
pub(super) fn current_definition() -> IndependentProofDefinition {
    use FixtureAssertion::{Absent, Present};
    use FixtureFactKind::{
        CfgEdge, CfgNode, DefUse, EvaluationOrder, LiveAt, Owner, ReachingDefinition, ValueFlow,
    };

    let branch = pending_fixture(
        FixtureCase::Branch,
        "def choose(flag):\n    if flag:\n        value = 1\n    else:\n        value = 2\n    return value\n",
        vec![
            Present(fact(CfgNode, "choose", "if@2", "if@2")),
            Present(fact(CfgEdge, "choose", "if@2", "value@3")),
            Present(fact(CfgEdge, "choose", "if@2", "value@5")),
            Present(fact(EvaluationOrder, "choose", "literal:1@3", "value@3")),
            Present(fact(DefUse, "choose", "value@3", "value@6")),
            Present(fact(DefUse, "choose", "value@5", "value@6")),
            Present(fact(ReachingDefinition, "choose", "value@3", "value@6")),
            Present(fact(ReachingDefinition, "choose", "value@5", "value@6")),
            Present(fact(LiveAt, "choose", "value", "return-entry@6")),
            Present(fact(ValueFlow, "choose", "literal:1@3", "value@6")),
            Present(fact(ValueFlow, "choose", "literal:2@5", "value@6")),
        ],
    );
    let loop_back = pending_fixture(
        FixtureCase::Loop,
        "def count(n):\n    while n:\n        n -= 1\n    return n\n",
        vec![Present(fact(
            CfgEdge,
            "count",
            "assignment-exit@3",
            "while-test@2",
        ))],
    );
    let no_fallthrough = pending_fixture(
        FixtureCase::Return,
        "def stop():\n    return 1\n    unreachable()\n",
        vec![Absent(fact(CfgEdge, "stop", "return@2", "call@3"))],
    );
    let nested = pending_fixture(
        FixtureCase::NestedCallable,
        "def outer():\n    def inner():\n        return 1\n    return inner\n",
        vec![
            Present(fact(Owner, "outer.inner", "return@3", "outer.inner")),
            Absent(fact(Owner, "outer", "return@3", "outer")),
            Absent(fact(CfgEdge, "outer", "def-inner@2", "return@3")),
        ],
    );
    let known_empty = pending_fixture(
        FixtureCase::KnownEmpty,
        "# No callable bodies or dynamic calls.\n",
        vec![],
    );
    let mut uncertainty = pending_fixture(
        FixtureCase::DynamicCall,
        "target = getattr(obj, name)\ntarget()\n",
        vec![],
    );
    uncertainty.current_unknowns = Arc::from([FixtureUnknown {
        owner: Arc::from("module"),
        occurrence: Arc::from("call@2"),
        cause: FixtureUnknownCause::DynamicDispatch,
    }]);
    IndependentProofDefinition {
        obligations: RequiredAnalysisFamily::ALL
            .into_iter()
            .map(|family| RequiredFamilyObligation {
                family,
                target_precision: target_precision(family),
                current_disposition: pending_implementation(),
            })
            .collect(),
        fixtures: vec![
            branch,
            loop_back,
            no_fallthrough,
            nested,
            known_empty,
            uncertainty,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_release::{CompiledSemanticRelease, tests::fixture_definition};

    fn release() -> CompiledSemanticRelease {
        CompiledSemanticRelease::compile(fixture_definition()).unwrap()
    }

    fn observed_required_gaps() -> Vec<RequiredAnalysisObservation> {
        RequiredAnalysisFamily::ALL
            .into_iter()
            .map(|family| RequiredAnalysisObservation {
                family,
                disposition: AnalysisDisposition::Unavailable {
                    reason: AnalysisGapReason::AlgorithmUnavailable,
                    retryability: AnalysisGapRetryability::RequiresReleaseChange,
                },
            })
            .collect()
    }

    #[test]
    fn rt_cpg_wp77_integrity() {
        let release = release();
        let proof = release.proof();
        assert_eq!(proof.conformance_fixtures().len(), 6);
        assert_eq!(
            proof
                .validate_required_analysis_observations(&observed_required_gaps())
                .unwrap(),
            RequiredAnalysisConformance::PendingImplementation
        );
        let fixtures = proof.conformance_fixtures();
        let branch = fixtures
            .iter()
            .find(|fixture| fixture.case == FixtureCase::Branch)
            .unwrap();
        assert_eq!(branch.required_families().len(), 7);
        assert_eq!(
            branch
                .target_assertions()
                .iter()
                .filter(|assertion| {
                    matches!(assertion, FixtureAssertion::Present(fact)
                    if fact.kind == FixtureFactKind::ReachingDefinition)
                })
                .count(),
            2,
            "both branch definitions are independently required, not a latest-definition count"
        );
        for family in RequiredAnalysisFamily::ALL {
            assert_eq!(
                RequiredAnalysisFamily::from_relation(family.relation_identity()),
                Some(family)
            );
        }
        assert_eq!(
            RequiredAnalysisFamily::from_relation("provider.ruff.ast_node"),
            None
        );
        let gaps = observed_required_gaps();
        assert_eq!(
            proof
                .validate_fixture_result(&FixtureResult {
                    fixture_id: branch.id(),
                    source: branch.source(),
                    facts: &[],
                    unknowns: &[],
                    analyses: &gaps,
                })
                .unwrap(),
            RequiredAnalysisConformance::PendingImplementation,
            "registered target assertions do not become proved from an accepted gap"
        );
    }

    fn reject_missing_duplicate_or_contradictory_definition() {
        let mut missing_family = fixture_definition();
        missing_family.proof.independent.obligations.pop();
        assert!(CompiledSemanticRelease::compile(missing_family).is_err());

        let mut duplicate_family = fixture_definition();
        let first = duplicate_family.proof.independent.obligations[0].clone();
        duplicate_family.proof.independent.obligations.push(first);
        assert!(CompiledSemanticRelease::compile(duplicate_family).is_err());

        for missing in 0..FixtureCase::ALL.len() {
            let mut missing_fixture = fixture_definition();
            missing_fixture.proof.independent.fixtures.remove(missing);
            assert!(CompiledSemanticRelease::compile(missing_fixture).is_err());
        }
        let mut duplicate_fixture = fixture_definition();
        let first = duplicate_fixture.proof.independent.fixtures[0].clone();
        duplicate_fixture.proof.independent.fixtures.push(first);
        assert!(CompiledSemanticRelease::compile(duplicate_fixture).is_err());

        let mut missing_unknown = fixture_definition();
        let fixture = missing_unknown
            .proof
            .independent
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.case == FixtureCase::DynamicCall)
            .unwrap();
        fixture.current_unknowns = Arc::from([]);
        assert!(CompiledSemanticRelease::compile(missing_unknown).is_err());

        let mut contradiction = fixture_definition();
        let fixture = &mut contradiction.proof.independent.fixtures[0];
        let mut assertions = fixture.target_assertions.to_vec();
        let FixtureAssertion::Present(first) = assertions[0].clone() else {
            panic!("positive baseline")
        };
        assertions.push(FixtureAssertion::Absent(first));
        fixture.target_assertions = assertions.into();
        assert!(CompiledSemanticRelease::compile(contradiction).is_err());

        let mut wrong_precision = fixture_definition();
        wrong_precision.proof.independent.obligations[0].target_precision =
            AnalysisPrecision::SoundMay;
        assert!(CompiledSemanticRelease::compile(wrong_precision).is_err());
    }

    #[test]
    fn rt_cpg_wp77_faults() {
        reject_missing_duplicate_or_contradictory_definition();
        let release = release();
        let proof = release.proof();
        for disposition in [
            AnalysisDisposition::Complete {
                precision: AnalysisPrecision::Exact,
            },
            AnalysisDisposition::KnownEmpty {
                precision: AnalysisPrecision::SoundMay,
            },
            AnalysisDisposition::LanguageUncertainty,
            AnalysisDisposition::Incomplete,
            AnalysisDisposition::Unavailable {
                reason: AnalysisGapReason::ProviderUnavailable,
                retryability: AnalysisGapRetryability::Retryable,
            },
            AnalysisDisposition::Unavailable {
                reason: AnalysisGapReason::AlgorithmUnavailable,
                retryability: AnalysisGapRetryability::PermanentlyUnsupported,
            },
        ] {
            let mut observed = observed_required_gaps();
            observed[0].disposition = disposition;
            assert!(
                proof
                    .validate_required_analysis_observations(&observed)
                    .is_err()
            );
        }
        let mut missing = observed_required_gaps();
        missing.pop();
        assert!(
            proof
                .validate_required_analysis_observations(&missing)
                .is_err()
        );
        let mut duplicate = observed_required_gaps();
        duplicate[6] = duplicate[0];
        assert!(
            proof
                .validate_required_analysis_observations(&duplicate)
                .is_err()
        );

        let empty = proof
            .conformance_fixtures()
            .iter()
            .find(|fixture| fixture.case == FixtureCase::KnownEmpty)
            .unwrap();
        let wrong = [fact(
            FixtureFactKind::Owner,
            "invented",
            "line@1",
            "invented",
        )];
        let result = FixtureResult {
            fixture_id: empty.id(),
            source: empty.source(),
            facts: &wrong,
            unknowns: &[],
            analyses: &[],
        };
        assert!(matches!(
            proof.validate_fixture_result(&result),
            Err(SemanticReleaseError::IndependentFixtureMismatch(_))
        ));
        assert!(
            proof
                .validate_fixture_result(&FixtureResult {
                    source: "def changed(): pass\n",
                    facts: &[],
                    ..result
                })
                .is_err()
        );
    }

    #[test]
    fn independent_fixture_known_empty_and_language_uncertainty_are_distinct() {
        let release = release();
        let proof = release.proof();
        let empty = proof
            .conformance_fixtures()
            .iter()
            .find(|fixture| fixture.case == FixtureCase::KnownEmpty)
            .unwrap();
        assert_eq!(
            proof
                .validate_fixture_result(&FixtureResult {
                    fixture_id: empty.id(),
                    source: empty.source(),
                    facts: &[],
                    unknowns: &[],
                    analyses: &[],
                })
                .unwrap(),
            RequiredAnalysisConformance::Satisfied
        );

        let dynamic = proof
            .conformance_fixtures()
            .iter()
            .find(|fixture| fixture.case == FixtureCase::DynamicCall)
            .unwrap();
        let observed = [FixtureUnknown {
            owner: Arc::from("module"),
            occurrence: Arc::from("call@2"),
            cause: FixtureUnknownCause::DynamicDispatch,
        }];
        let result = FixtureResult {
            fixture_id: dynamic.id(),
            source: dynamic.source(),
            facts: &[],
            unknowns: &observed,
            analyses: &[],
        };
        assert_eq!(
            proof.validate_fixture_result(&result).unwrap(),
            RequiredAnalysisConformance::Satisfied
        );
        assert!(
            proof
                .validate_fixture_result(&FixtureResult {
                    unknowns: &[],
                    ..result.clone()
                })
                .is_err()
        );
        let wrong = [FixtureUnknown {
            occurrence: Arc::from("call@1"),
            ..observed[0].clone()
        }];
        assert!(
            proof
                .validate_fixture_result(&FixtureResult {
                    unknowns: &wrong,
                    ..result.clone()
                })
                .is_err()
        );
        let duplicated = [observed[0].clone(), observed[0].clone()];
        assert!(
            proof
                .validate_fixture_result(&FixtureResult {
                    unknowns: &duplicated,
                    ..result
                })
                .is_err()
        );
    }

    #[test]
    fn independent_proof_identity_binds_all_behavior_operands() {
        let baseline = release();
        let baseline_identity = baseline.proof().identity();
        assert_eq!(
            &baseline.proof().construct_input().program_identity,
            baseline_identity
        );
        assert_eq!(&baseline.observation().proof_program, baseline_identity);

        let mut source_change = fixture_definition();
        let fixture = &mut source_change.proof.independent.fixtures[0];
        fixture.source = Arc::from(format!("{}# a new exact source revision\n", fixture.source));
        let changed = CompiledSemanticRelease::compile(source_change).unwrap();
        assert_ne!(changed.proof().identity(), baseline_identity);

        let mut expectation_change = fixture_definition();
        let fixture = &mut expectation_change.proof.independent.fixtures[0];
        let mut assertions = fixture.target_assertions.to_vec();
        let FixtureAssertion::Present(fact) = &mut assertions[0] else {
            panic!("positive baseline")
        };
        fact.target = Arc::from("different-program-point");
        fixture.target_assertions = assertions.into();
        assert_ne!(
            CompiledSemanticRelease::compile(expectation_change)
                .unwrap()
                .proof()
                .identity(),
            baseline_identity
        );

        let mut unknown_change = fixture_definition();
        let fixture = unknown_change
            .proof
            .independent
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.case == FixtureCase::DynamicCall)
            .unwrap();
        let mut unknowns = fixture.current_unknowns.to_vec();
        unknowns[0].occurrence = Arc::from("call@1");
        fixture.current_unknowns = unknowns.into();
        assert_ne!(
            CompiledSemanticRelease::compile(unknown_change)
                .unwrap()
                .proof()
                .identity(),
            baseline_identity
        );

        let mut row_expectation_change = fixture_definition();
        row_expectation_change.proof.expectations[0].minimum_rows += 1;
        assert_ne!(
            CompiledSemanticRelease::compile(row_expectation_change)
                .unwrap()
                .proof()
                .identity(),
            baseline_identity
        );

        let mut fault_change = fixture_definition();
        fault_change.proof.faults[0].effect =
            crate::semantic_release::CausalEffect::RejectAdmission;
        assert_ne!(
            CompiledSemanticRelease::compile(fault_change)
                .unwrap()
                .proof()
                .identity(),
            baseline_identity
        );

        let mut reordered = fixture_definition();
        reordered.proof.expectations.reverse();
        reordered.proof.independent.obligations.reverse();
        reordered.proof.independent.fixtures.reverse();
        let fixture = reordered
            .proof
            .independent
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.case == FixtureCase::Branch)
            .unwrap();
        let mut assertions = fixture.target_assertions.to_vec();
        assertions.reverse();
        fixture.target_assertions = assertions.into();
        let mut families = fixture.required_families.to_vec();
        families.reverse();
        fixture.required_families = families.into();
        let reordered = CompiledSemanticRelease::compile(reordered).unwrap();
        assert_eq!(reordered.proof().identity(), baseline_identity);
        assert_eq!(
            reordered.proof().construct_input().expectations,
            baseline.proof().construct_input().expectations,
            "consumers must observe the same canonical operand order that identity binds"
        );
    }
}
