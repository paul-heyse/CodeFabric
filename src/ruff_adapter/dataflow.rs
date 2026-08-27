//! Application-owned Python value/access inputs and owner-local reaching definitions.
//!
//! Ruff binding identities define variable domains. The WP06 CFG supplies the control
//! topology, but petgraph-local indices never cross this adapter boundary. Every derived
//! output is selected through the governed `PY_OWNER_REACHING_DEFS_V1` registry entry and
//! carries its precision and derivation-bundle identity.

use std::collections::{BTreeMap, BTreeSet};

use crate::registries::DERIVATION_ENTRIES;

use super::callables::{
    PythonCallArgumentFact, PythonCallSiteFact, PythonCallableFact, PythonCallableSyntaxFact,
};
use super::cfg::{PythonCfgEdgeFact, PythonCfgFact, PythonCfgNodeFact, PythonCfgNodeKind};
use super::semantic::{
    PythonBindingFact, PythonBindingKind, PythonReferenceClass, PythonReferenceFact,
    PythonSemanticError, PythonSemanticId, semantic_id,
};

pub const PYTHON_DATAFLOW_DERIVATION_ID: &str = "PY_OWNER_REACHING_DEFS_V1";
pub const PYTHON_DATAFLOW_PRECISION_PROFILE: &str = "PYTHON_LOCAL_REACHING_DEFS_V1";
pub const PYTHON_DATAFLOW_BUNDLE_ID: &str = "codefabric.derivation-bundle.v1.3";
const IMPLEMENTATION_SYMBOL: &str = "crate::ruff_adapter::dataflow::project_python_dataflow";

/// Closed value classification for `value_detail`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonValueKind {
    Literal,
    NameRead,
    AttributeRead,
    SubscriptRead,
    CallReturn,
    OperationResult,
    Container,
    CallableObject,
    AwaitOrYield,
    Merged,
    Unknown,
}

impl PythonValueKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Literal => 10,
            Self::NameRead => 20,
            Self::AttributeRead => 30,
            Self::SubscriptRead => 40,
            Self::CallReturn => 50,
            Self::OperationResult => 60,
            Self::Container => 70,
            Self::CallableObject => 80,
            Self::AwaitOrYield => 90,
            Self::Merged => 100,
            Self::Unknown => 110,
        }
    }
}

/// Closed normalized-operation classification for `operation_detail`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonOperationKind {
    Read,
    Write,
    Call,
    Merge,
    DynamicBarrier,
}

impl PythonOperationKind {
    pub(crate) const fn code(self) -> i32 {
        match self {
            Self::Read => 10,
            Self::Write => 20,
            Self::Call => 30,
            Self::Merge => 40,
            Self::DynamicBarrier => 50,
        }
    }
}

/// Closed definition/use event classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonDataflowEventKind {
    Definition,
    Read,
    Receiver,
    Callee,
    Argument,
    Condition,
    ReturnOrYield,
    Index,
    Decorator,
    EvaluatedAnnotation,
    DynamicUnknown,
}

impl PythonDataflowEventKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Definition => 10,
            Self::Read => 20,
            Self::Receiver => 30,
            Self::Callee => 40,
            Self::Argument => 50,
            Self::Condition => 60,
            Self::ReturnOrYield => 70,
            Self::Index => 80,
            Self::Decorator => 90,
            Self::EvaluatedAnnotation => 100,
            Self::DynamicUnknown => 110,
        }
    }
}

/// Canonical abstract memory/access-path location classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonLocationKind {
    Local,
    Global,
    Cell,
    Field,
    InstanceMember,
    ClassMember,
    Indexed,
    Module,
    Unknown,
}

impl PythonLocationKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Local => 10,
            Self::Global => 20,
            Self::Cell => 30,
            Self::Field => 40,
            Self::InstanceMember => 50,
            Self::ClassMember => 60,
            Self::Indexed => 70,
            Self::Module => 80,
            Self::Unknown => 90,
        }
    }
}

/// Ordered structural projection in one access path.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonAccessProjectionKind {
    Base,
    Field,
    Index,
    Unknown,
}

impl PythonAccessProjectionKind {
    pub(crate) const fn code(self) -> i16 {
        match self {
            Self::Base => 10,
            Self::Field => 20,
            Self::Index => 30,
            Self::Unknown => 40,
        }
    }
}

/// The six governed owner-local relation families produced by this derivation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonDataflowRelationKind {
    ReachingDefinition,
    Reaches,
    DefUse,
    DataDep,
    ValueFlowsTo,
    KillsDefinition,
}

impl PythonDataflowRelationKind {
    pub(crate) const fn canonical_name(self) -> &'static str {
        match self {
            Self::ReachingDefinition => "REACHING_DEFINITION",
            Self::Reaches => "REACHES",
            Self::DefUse => "DEF_USE",
            Self::DataDep => "DATA_DEP",
            Self::ValueFlowsTo => "VALUE_FLOWS_TO",
            Self::KillsDefinition => "KILLS_DEFINITION",
        }
    }

    const fn discriminator(self) -> u8 {
        match self {
            Self::ReachingDefinition => 1,
            Self::Reaches => 2,
            Self::DefUse => 3,
            Self::DataDep => 4,
            Self::ValueFlowsTo => 5,
            Self::KillsDefinition => 6,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonValueFact {
    pub value_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub kind: PythonValueKind,
    pub producer_operation_id: Option<PythonSemanticId>,
    pub syntax_id: Option<PythonSemanticId>,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonOperationFact {
    pub operation_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub cfg_node_id: Option<PythonSemanticId>,
    pub kind: PythonOperationKind,
    pub result_value_id: Option<PythonSemanticId>,
    pub syntax_id: Option<PythonSemanticId>,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonDataflowEventFact {
    pub event_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub cfg_node_id: Option<PythonSemanticId>,
    pub kind: PythonDataflowEventKind,
    pub binding_id: Option<PythonSemanticId>,
    pub value_id: Option<PythonSemanticId>,
    pub location_id: Option<PythonSemanticId>,
    pub syntax_id: Option<PythonSemanticId>,
    pub ordinal: Option<i32>,
    pub start_byte: u64,
    pub end_byte: u64,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonMemoryLocationFact {
    pub location_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub kind: PythonLocationKind,
    pub base_entity_id: Option<PythonSemanticId>,
    pub base_local_id: Option<PythonSemanticId>,
    pub parent_location_id: Option<PythonSemanticId>,
    pub projection_depth: i16,
    pub canonical_path_hash: [u8; 32],
    pub display_path: Option<String>,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonAccessPathComponentFact {
    pub component_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub location_id: PythonSemanticId,
    pub ordinal: i16,
    pub kind: PythonAccessProjectionKind,
    pub field_entity_id: Option<PythonSemanticId>,
    pub index_value_id: Option<PythonSemanticId>,
    pub constant_index: Option<i64>,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonDataflowRelationFact {
    pub relation_id: PythonSemanticId,
    pub owner_id: PythonSemanticId,
    pub source_id: PythonSemanticId,
    pub target_id: PythonSemanticId,
    pub kind: PythonDataflowRelationKind,
    pub flags: i64,
    pub precision_profile_id: &'static str,
    pub derivation_bundle_id: &'static str,
}

pub(super) struct PythonDataflowProjection {
    pub values: Vec<PythonValueFact>,
    pub operations: Vec<PythonOperationFact>,
    pub events: Vec<PythonDataflowEventFact>,
    pub locations: Vec<PythonMemoryLocationFact>,
    pub components: Vec<PythonAccessPathComponentFact>,
    pub relations: Vec<PythonDataflowRelationFact>,
    pub iteration_count: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BindingDomain {
    scope_id: PythonSemanticId,
    name: String,
}

fn selected_derivation() -> Result<(), PythonSemanticError> {
    let selected = DERIVATION_ENTRIES
        .iter()
        .filter(|entry| entry.derivation_id == PYTHON_DATAFLOW_DERIVATION_ID)
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(PythonSemanticError::Invariant(format!(
            "{PYTHON_DATAFLOW_DERIVATION_ID} must resolve to exactly one derivation entry"
        )));
    }
    let entry = selected[0];
    if entry.precision_profile != PYTHON_DATAFLOW_PRECISION_PROFILE
        || entry.derivation_bundle_id != PYTHON_DATAFLOW_BUNDLE_ID
        || entry.implementation_symbol != IMPLEMENTATION_SYMBOL
        || entry.output_fact_families
            != [
                "reaching-definition",
                "reaches",
                "def-use",
                "data-dep",
                "value-flows-to",
                "kills-definition",
            ]
    {
        return Err(PythonSemanticError::Invariant(format!(
            "{PYTHON_DATAFLOW_DERIVATION_ID} registry selection mismatched runtime contract"
        )));
    }
    Ok(())
}

fn hash_path(owner_id: PythonSemanticId, display: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-access-path.v1");
    hasher.update(&owner_id);
    hasher.update(display.as_bytes());
    *hasher.finalize().as_bytes()
}

fn binding_location_kind(kind: PythonBindingKind) -> PythonLocationKind {
    match kind {
        PythonBindingKind::Global | PythonBindingKind::Builtin => PythonLocationKind::Global,
        PythonBindingKind::Free | PythonBindingKind::Cell | PythonBindingKind::Nonlocal => {
            PythonLocationKind::Cell
        }
        PythonBindingKind::ClassAttribute => PythonLocationKind::ClassMember,
        PythonBindingKind::InstanceAttribute => PythonLocationKind::InstanceMember,
        PythonBindingKind::Import => PythonLocationKind::Module,
        _ => PythonLocationKind::Local,
    }
}

fn owner_for_range(
    start: u64,
    end: u64,
    callables: &[PythonCallableFact],
) -> Option<PythonSemanticId> {
    callables
        .iter()
        .filter(|callable| callable.start_byte <= start && end <= callable.end_byte)
        .min_by_key(|callable| callable.end_byte.saturating_sub(callable.start_byte))
        .map(|callable| callable.callable_id)
}

fn node_for_range(
    owner_id: PythonSemanticId,
    start: u64,
    end: u64,
    cfgs: &[PythonCfgFact],
    nodes: &[PythonCfgNodeFact],
) -> Option<PythonSemanticId> {
    let cfg = cfgs.iter().find(|cfg| cfg.owner_id == owner_id)?;
    nodes
        .iter()
        .filter(|node| node.cfg_id == cfg.cfg_id)
        .filter_map(|node| {
            let (node_start, node_end) = (node.start_byte?, node.end_byte?);
            (node_start <= start && end <= node_end).then_some((
                node_end.saturating_sub(node_start),
                node.ordinal,
                node.cfg_node_id,
            ))
        })
        .min()
        .map(|(_, _, id)| id)
        .or(Some(cfg.entry_node_id))
}

fn event_kind(reference: &PythonReferenceFact, node: Option<&PythonCfgNodeFact>) -> PythonDataflowEventKind {
    if reference.class == PythonReferenceClass::CallReference {
        return PythonDataflowEventKind::Callee;
    }
    let label = node.map_or("", |node| node.label);
    if label.contains("condition") || matches!(node.map(|node| node.kind), Some(PythonCfgNodeKind::Branch)) {
        PythonDataflowEventKind::Condition
    } else if label.contains("return") || label.contains("yield") {
        PythonDataflowEventKind::ReturnOrYield
    } else if label.contains("decorator") {
        PythonDataflowEventKind::Decorator
    } else if label.contains("annotation") {
        PythonDataflowEventKind::EvaluatedAnnotation
    } else if label.contains("index") || label.contains("subscript") {
        PythonDataflowEventKind::Index
    } else {
        PythonDataflowEventKind::Read
    }
}

fn relation(
    fingerprint: &str,
    owner_id: PythonSemanticId,
    source_id: PythonSemanticId,
    target_id: PythonSemanticId,
    kind: PythonDataflowRelationKind,
) -> PythonDataflowRelationFact {
    let mut name = String::with_capacity(65);
    name.push_str(&id_text(source_id));
    name.push(':');
    name.push_str(&id_text(target_id));
    PythonDataflowRelationFact {
        relation_id: semantic_id(fingerprint, "dataflow-relation", 0, 0, &name, kind.discriminator()),
        owner_id,
        source_id,
        target_id,
        kind,
        flags: 0,
        precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
        derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
    }
}

fn id_text(id: PythonSemanticId) -> String {
    use std::fmt::Write as _;

    id.iter().fold(String::with_capacity(32), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// Execute the only registered Python owner-local reaching-definitions implementation.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // The closed derivation transaction remains reviewable in one place.
pub(super) fn project_python_dataflow(
    fingerprint: &str,
    bindings: &[PythonBindingFact],
    references: &[PythonReferenceFact],
    callables: &[PythonCallableFact],
    callable_syntax: &[PythonCallableSyntaxFact],
    call_sites: &[PythonCallSiteFact],
    call_arguments: &[PythonCallArgumentFact],
    cfgs: &[PythonCfgFact],
    cfg_nodes: &[PythonCfgNodeFact],
    cfg_edges: &[PythonCfgEdgeFact],
) -> Result<PythonDataflowProjection, PythonSemanticError> {
    selected_derivation()?;
    let nodes_by_id = cfg_nodes
        .iter()
        .map(|node| (node.cfg_node_id, node))
        .collect::<BTreeMap<_, _>>();
    let binding_by_id = bindings
        .iter()
        .map(|binding| (binding.binding_id, binding))
        .collect::<BTreeMap<_, _>>();
    let domain_by_binding = bindings
        .iter()
        .map(|binding| {
            (
                binding.binding_id,
                BindingDomain {
                    scope_id: binding.scope_id,
                    name: binding.name.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut values = Vec::new();
    let mut operations = Vec::new();
    let mut events = Vec::new();
    let mut locations = BTreeMap::new();
    let mut components = BTreeMap::new();

    for binding in bindings {
        let Some(owner_id) = owner_for_range(binding.start_byte, binding.end_byte, callables) else {
            continue;
        };
        let cfg_node_id = node_for_range(owner_id, binding.start_byte, binding.end_byte, cfgs, cfg_nodes);
        let location_id = semantic_id(
            fingerprint,
            "memory-location",
            binding.start_byte,
            binding.end_byte,
            &binding.name,
            binding_location_kind(binding.kind).code() as u8,
        );
        let value_id = semantic_id(
            fingerprint,
            "definition-value",
            binding.start_byte,
            binding.end_byte,
            &id_text(binding.binding_id),
            1,
        );
        let operation_id = semantic_id(
            fingerprint,
            "definition-operation",
            binding.start_byte,
            binding.end_byte,
            &id_text(binding.binding_id),
            1,
        );
        let event_id = semantic_id(
            fingerprint,
            "definition-event",
            binding.start_byte,
            binding.end_byte,
            &id_text(binding.binding_id),
            1,
        );
        locations.entry(location_id).or_insert_with(|| PythonMemoryLocationFact {
            location_id,
            owner_id,
            kind: binding_location_kind(binding.kind),
            base_entity_id: Some(binding.binding_id),
            base_local_id: Some(binding.binding_id),
            parent_location_id: None,
            projection_depth: 0,
            canonical_path_hash: hash_path(owner_id, &binding.name),
            display_path: Some(binding.name.clone()),
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        let component_id = semantic_id(
            fingerprint,
            "access-path-component",
            binding.start_byte,
            binding.end_byte,
            &binding.name,
            0,
        );
        components.entry(component_id).or_insert(PythonAccessPathComponentFact {
            component_id,
            owner_id,
            location_id,
            ordinal: 0,
            kind: PythonAccessProjectionKind::Base,
            field_entity_id: Some(binding.binding_id),
            index_value_id: None,
            constant_index: None,
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        values.push(PythonValueFact {
            value_id,
            owner_id,
            kind: PythonValueKind::OperationResult,
            producer_operation_id: Some(operation_id),
            syntax_id: None,
            start_byte: Some(binding.start_byte),
            end_byte: Some(binding.end_byte),
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        operations.push(PythonOperationFact {
            operation_id,
            owner_id,
            cfg_node_id,
            kind: PythonOperationKind::Write,
            result_value_id: Some(value_id),
            syntax_id: None,
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        events.push(PythonDataflowEventFact {
            event_id,
            owner_id,
            cfg_node_id,
            kind: PythonDataflowEventKind::Definition,
            binding_id: Some(binding.binding_id),
            value_id: Some(value_id),
            location_id: Some(location_id),
            syntax_id: None,
            ordinal: None,
            start_byte: binding.start_byte,
            end_byte: binding.end_byte,
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
    }

    for reference in references.iter().filter(|reference| {
        !matches!(reference.class, PythonReferenceClass::Write | PythonReferenceClass::Delete)
    }) {
        let Some(owner_id) = owner_for_range(reference.start_byte, reference.end_byte, callables) else {
            continue;
        };
        let cfg_node_id = node_for_range(owner_id, reference.start_byte, reference.end_byte, cfgs, cfg_nodes);
        let location_binding = binding_by_id.get(&reference.target_id).copied();
        let location_kind = location_binding.map_or(PythonLocationKind::Unknown, |binding| binding_location_kind(binding.kind));
        let location_id = semantic_id(
            fingerprint,
            "memory-location-reference",
            0,
            0,
            &format!("{}:{}", id_text(owner_id), reference.name),
            location_kind.code() as u8,
        );
        locations.entry(location_id).or_insert_with(|| PythonMemoryLocationFact {
            location_id,
            owner_id,
            kind: location_kind,
            base_entity_id: location_binding.map(|binding| binding.binding_id),
            base_local_id: location_binding.map(|binding| binding.binding_id),
            parent_location_id: None,
            projection_depth: 0,
            canonical_path_hash: hash_path(owner_id, &reference.name),
            display_path: Some(reference.name.clone()),
            flags: i64::from(location_binding.is_none()),
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        let component_id = semantic_id(fingerprint, "access-path-component-reference", 0, 0, &format!("{}:{}", id_text(owner_id), reference.name), 0);
        components.entry(component_id).or_insert(PythonAccessPathComponentFact {
            component_id,
            owner_id,
            location_id,
            ordinal: 0,
            kind: if location_binding.is_some() { PythonAccessProjectionKind::Base } else { PythonAccessProjectionKind::Unknown },
            field_entity_id: location_binding.map(|binding| binding.binding_id),
            index_value_id: None,
            constant_index: None,
            flags: i64::from(location_binding.is_none()),
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        let reference_identity = id_text(reference.reference_id);
        let value_id = semantic_id(
            fingerprint,
            "use-value",
            reference.start_byte,
            reference.end_byte,
            &reference_identity,
            1,
        );
        let operation_id = semantic_id(
            fingerprint,
            "read-operation",
            reference.start_byte,
            reference.end_byte,
            &reference_identity,
            1,
        );
        values.push(PythonValueFact {
            value_id,
            owner_id,
            kind: if location_binding.is_some() { PythonValueKind::NameRead } else { PythonValueKind::Unknown },
            producer_operation_id: Some(operation_id),
            syntax_id: Some(reference.reference_id),
            start_byte: Some(reference.start_byte),
            end_byte: Some(reference.end_byte),
            flags: i64::from(location_binding.is_none()),
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        operations.push(PythonOperationFact {
            operation_id,
            owner_id,
            cfg_node_id,
            kind: PythonOperationKind::Read,
            result_value_id: Some(value_id),
            syntax_id: Some(reference.reference_id),
            flags: 0,
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
        events.push(PythonDataflowEventFact {
            event_id: semantic_id(
                fingerprint,
                "use-event",
                reference.start_byte,
                reference.end_byte,
                &reference_identity,
                1,
            ),
            owner_id,
            cfg_node_id,
            kind: event_kind(reference, cfg_node_id.and_then(|id| nodes_by_id.get(&id).copied())),
            binding_id: location_binding.map(|binding| binding.binding_id),
            value_id: Some(value_id),
            location_id: Some(location_id),
            syntax_id: Some(reference.reference_id),
            ordinal: None,
            start_byte: reference.start_byte,
            end_byte: reference.end_byte,
            flags: i64::from(location_binding.is_none()),
            precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
            derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
        });
    }

    let syntax_by_id = callable_syntax
        .iter()
        .map(|syntax| (syntax.syntax_id, syntax))
        .collect::<BTreeMap<_, _>>();
    for call in call_sites {
        let Some(owner_id) = owner_for_range(call.start_byte, call.end_byte, callables) else {
            continue;
        };
        let cfg_node_id = node_for_range(owner_id, call.start_byte, call.end_byte, cfgs, cfg_nodes);
        let value_id = semantic_id(fingerprint, "call-return-value", call.start_byte, call.end_byte, &id_text(call.call_site_id), 1);
        let operation_id = semantic_id(fingerprint, "call-operation", call.start_byte, call.end_byte, &id_text(call.call_site_id), 1);
        values.push(PythonValueFact { value_id, owner_id, kind: PythonValueKind::CallReturn, producer_operation_id: Some(operation_id), syntax_id: Some(call.syntax_id), start_byte: Some(call.start_byte), end_byte: Some(call.end_byte), flags: 0, precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE, derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID });
        operations.push(PythonOperationFact { operation_id, owner_id, cfg_node_id, kind: PythonOperationKind::Call, result_value_id: Some(value_id), syntax_id: Some(call.syntax_id), flags: 0, precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE, derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID });

        if let Some(callee) = syntax_by_id.get(&call.callee_syntax_id)
            && matches!(callee.text.as_str(), "exec" | "eval" | "getattr" | "setattr" | "__import__")
        {
            let dynamic_name = format!("{}:{}", callee.text, id_text(call.call_site_id));
            let unknown_location_id = semantic_id(
                fingerprint,
                "dynamic-unknown-location",
                call.start_byte,
                call.end_byte,
                &dynamic_name,
                1,
            );
            let unknown_value_id = semantic_id(
                fingerprint,
                "dynamic-unknown-value",
                call.start_byte,
                call.end_byte,
                &dynamic_name,
                1,
            );
            let barrier_operation_id = semantic_id(
                fingerprint,
                "dynamic-barrier-operation",
                call.start_byte,
                call.end_byte,
                &dynamic_name,
                1,
            );
            locations.entry(unknown_location_id).or_insert(PythonMemoryLocationFact {
                location_id: unknown_location_id,
                owner_id,
                kind: PythonLocationKind::Unknown,
                base_entity_id: Some(call.callee_syntax_id),
                base_local_id: None,
                parent_location_id: None,
                projection_depth: 0,
                canonical_path_hash: hash_path(owner_id, &dynamic_name),
                display_path: Some(format!("unknown({})", callee.text)),
                flags: 1,
                precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
                derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
            });
            let component_id = semantic_id(
                fingerprint,
                "dynamic-unknown-component",
                call.start_byte,
                call.end_byte,
                &dynamic_name,
                1,
            );
            components.entry(component_id).or_insert(PythonAccessPathComponentFact {
                component_id,
                owner_id,
                location_id: unknown_location_id,
                ordinal: 0,
                kind: PythonAccessProjectionKind::Unknown,
                field_entity_id: None,
                index_value_id: None,
                constant_index: None,
                flags: 1,
                precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
                derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
            });
            values.push(PythonValueFact {
                value_id: unknown_value_id,
                owner_id,
                kind: PythonValueKind::Unknown,
                producer_operation_id: Some(barrier_operation_id),
                syntax_id: Some(call.syntax_id),
                start_byte: Some(call.start_byte),
                end_byte: Some(call.end_byte),
                flags: 1,
                precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
                derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
            });
            operations.push(PythonOperationFact {
                operation_id: barrier_operation_id,
                owner_id,
                cfg_node_id,
                kind: PythonOperationKind::DynamicBarrier,
                result_value_id: Some(unknown_value_id),
                syntax_id: Some(call.syntax_id),
                flags: 1,
                precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
                derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
            });
            events.push(PythonDataflowEventFact {
                event_id: semantic_id(
                    fingerprint,
                    "dynamic-unknown-event",
                    call.start_byte,
                    call.end_byte,
                    &dynamic_name,
                    1,
                ),
                owner_id,
                cfg_node_id,
                kind: PythonDataflowEventKind::DynamicUnknown,
                binding_id: None,
                value_id: Some(unknown_value_id),
                location_id: Some(unknown_location_id),
                syntax_id: Some(call.syntax_id),
                ordinal: None,
                start_byte: call.start_byte,
                end_byte: call.end_byte,
                flags: 1,
                precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE,
                derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID,
            });
        }

        if let Some(callee) = syntax_by_id.get(&call.callee_syntax_id)
            && let Some((base, field)) = callee.text.rsplit_once('.')
        {
            let display = format!("{base}.{field}");
            let location_id = semantic_id(fingerprint, "member-location", call.start_byte, call.end_byte, &display, 1);
            locations.entry(location_id).or_insert(PythonMemoryLocationFact { location_id, owner_id, kind: PythonLocationKind::InstanceMember, base_entity_id: call.receiver_syntax_id, base_local_id: None, parent_location_id: None, projection_depth: 1, canonical_path_hash: hash_path(owner_id, &display), display_path: Some(display.clone()), flags: 0, precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE, derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID });
            for (ordinal, (kind, text)) in [(PythonAccessProjectionKind::Base, base), (PythonAccessProjectionKind::Field, field)].into_iter().enumerate() {
                let component_id = semantic_id(fingerprint, "member-location-component", call.start_byte, call.end_byte, text, u8::try_from(ordinal).unwrap_or(u8::MAX));
                components.entry(component_id).or_insert(PythonAccessPathComponentFact { component_id, owner_id, location_id, ordinal: i16::try_from(ordinal).unwrap_or(i16::MAX), kind, field_entity_id: (ordinal == 1).then_some(call.callee_syntax_id), index_value_id: None, constant_index: None, flags: 0, precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE, derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID });
            }
        }
    }
    for argument in call_arguments.iter().filter(|argument| argument.start_byte.is_some()) {
        let (start, end) = (argument.start_byte.unwrap_or(0), argument.end_byte.unwrap_or(0));
        if let Some(owner_id) = owner_for_range(start, end, callables) {
            let cfg_node_id = node_for_range(owner_id, start, end, cfgs, cfg_nodes);
            if let Some(event) = events.iter_mut().find(|event| event.owner_id == owner_id && event.start_byte == start && event.end_byte == end) {
                event.kind = PythonDataflowEventKind::Argument;
                event.ordinal = Some(argument.ordinal);
                event.cfg_node_id = cfg_node_id;
            }
        }
    }

    events.sort_by_key(|event| (event.owner_id, event.start_byte, event.end_byte, event.kind, event.event_id));
    events.dedup_by_key(|event| event.event_id);
    for (ordinal, event) in events.iter_mut().enumerate() {
        if event.ordinal.is_none() {
            event.ordinal = Some(i32::try_from(ordinal).unwrap_or(i32::MAX));
        }
    }

    let value_by_event = events
        .iter()
        .filter_map(|event| event.value_id.map(|value| (event.event_id, value)))
        .collect::<BTreeMap<_, _>>();
    let domain_for_event = events
        .iter()
        .filter_map(|event| {
            let binding = event.binding_id?;
            let domain = domain_by_binding.get(&binding)?.clone();
            Some((event.event_id, domain))
        })
        .collect::<BTreeMap<_, _>>();
    let mut events_by_node = BTreeMap::<PythonSemanticId, Vec<&PythonDataflowEventFact>>::new();
    for event in &events {
        if let Some(node) = event.cfg_node_id {
            events_by_node.entry(node).or_default().push(event);
        }
    }
    let mut predecessors = BTreeMap::<PythonSemanticId, BTreeSet<PythonSemanticId>>::new();
    for edge in cfg_edges {
        predecessors.entry(edge.target_node_id).or_default().insert(edge.source_node_id);
    }
    let graph_nodes = cfg_nodes.iter().map(|node| node.cfg_node_id).collect::<Vec<_>>();
    type Reaching = BTreeMap<BindingDomain, BTreeSet<PythonSemanticId>>;
    let mut incoming = graph_nodes.iter().map(|node| (*node, Reaching::new())).collect::<BTreeMap<_, _>>();
    let mut outgoing = incoming.clone();
    let convergence_limit = graph_nodes.len().saturating_mul(events.len().max(1)).saturating_add(1);
    let mut iteration_count = 0_usize;
    loop {
        iteration_count = iteration_count.saturating_add(1);
        let mut changed = false;
        for node in &graph_nodes {
            let mut next_in = Reaching::new();
            for predecessor in predecessors.get(node).into_iter().flatten() {
                for (domain, definitions) in &outgoing[predecessor] {
                    next_in.entry(domain.clone()).or_default().extend(definitions);
                }
            }
            let mut next_out = next_in.clone();
            for event in events_by_node.get(node).into_iter().flatten().filter(|event| event.kind == PythonDataflowEventKind::Definition) {
                if let Some(domain) = domain_for_event.get(&event.event_id) {
                    next_out.insert(domain.clone(), BTreeSet::from([event.event_id]));
                }
            }
            changed |= incoming.get(node) != Some(&next_in) || outgoing.get(node) != Some(&next_out);
            incoming.insert(*node, next_in);
            outgoing.insert(*node, next_out);
        }
        if !changed { break; }
        if iteration_count > convergence_limit {
            return Err(PythonSemanticError::Invariant("PY_OWNER_REACHING_DEFS_V1 did not converge within its finite lattice bound".into()));
        }
    }

    let mut relations = BTreeSet::new();
    let mut merged_values = BTreeMap::<(PythonSemanticId, BindingDomain, BTreeSet<PythonSemanticId>), PythonSemanticId>::new();
    for node in &graph_nodes {
        let mut current = incoming[node].clone();
        for event in events_by_node.get(node).into_iter().flatten() {
            let Some(domain) = domain_for_event.get(&event.event_id) else { continue; };
            if event.kind == PythonDataflowEventKind::Definition {
                for killed in current.get(domain).into_iter().flatten() {
                    if *killed != event.event_id {
                        relations.insert((event.owner_id, event.event_id, *killed, PythonDataflowRelationKind::KillsDefinition));
                    }
                }
                current.insert(domain.clone(), BTreeSet::from([event.event_id]));
                continue;
            }
            let reaching = current.get(domain).cloned().unwrap_or_default();
            for definition in &reaching {
                for kind in [PythonDataflowRelationKind::ReachingDefinition, PythonDataflowRelationKind::Reaches, PythonDataflowRelationKind::DefUse, PythonDataflowRelationKind::DataDep] {
                    relations.insert((event.owner_id, *definition, event.event_id, kind));
                }
                if let (Some(source), Some(target)) = (value_by_event.get(definition), event.value_id) {
                    relations.insert((event.owner_id, *source, target, PythonDataflowRelationKind::ValueFlowsTo));
                }
            }
            if reaching.len() > 1 {
                let key = (*node, domain.clone(), reaching.clone());
                let merged = *merged_values.entry(key).or_insert_with(|| semantic_id(fingerprint, "merged-value", event.start_byte, event.end_byte, &domain.name, 1));
                values.push(PythonValueFact { value_id: merged, owner_id: event.owner_id, kind: PythonValueKind::Merged, producer_operation_id: None, syntax_id: event.syntax_id, start_byte: Some(event.start_byte), end_byte: Some(event.end_byte), flags: 0, precision_profile_id: PYTHON_DATAFLOW_PRECISION_PROFILE, derivation_bundle_id: PYTHON_DATAFLOW_BUNDLE_ID });
                for definition in &reaching {
                    if let Some(source) = value_by_event.get(definition) {
                        relations.insert((event.owner_id, *source, merged, PythonDataflowRelationKind::ValueFlowsTo));
                    }
                }
                if let Some(target) = event.value_id {
                    relations.insert((event.owner_id, merged, target, PythonDataflowRelationKind::ValueFlowsTo));
                }
            }
        }
    }

    values.sort_by_key(|fact| (fact.owner_id, fact.start_byte, fact.end_byte, fact.kind, fact.value_id));
    values.dedup_by_key(|fact| fact.value_id);
    operations.sort_by_key(|fact| (fact.owner_id, fact.cfg_node_id, fact.operation_id));
    operations.dedup_by_key(|fact| fact.operation_id);
    let mut locations = locations.into_values().collect::<Vec<_>>();
    locations.sort_by_key(|fact| (fact.owner_id, fact.display_path.clone(), fact.location_id));
    let mut components = components.into_values().collect::<Vec<_>>();
    components.sort_by_key(|fact| (fact.owner_id, fact.location_id, fact.ordinal, fact.component_id));
    let relations = relations
        .into_iter()
        .map(|(owner, source, target, kind)| relation(fingerprint, owner, source, target, kind))
        .collect();
    Ok(PythonDataflowProjection { values, operations, events, locations, components, relations, iteration_count: u64::try_from(iteration_count).unwrap_or(u64::MAX) })
}
