//! Native Cargo's resolved compiler-unit graph, captured as immutable context input. This is
//! distinct from a compiler/fact census: a discovered unit has not necessarily executed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, BinaryArray, BooleanArray, StringArray, UInt64Array};
use serde::Deserialize;

use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{hash32_array, id16_array};
use crate::resource_budget::{ChargedValue, ResourceBudget};

use super::super::{ProductionWorkspaceStartupError, input_observations, step};

#[derive(Debug, Deserialize)]
pub(super) struct UnitGraph {
    pub version: u32,
    pub units: Vec<Unit>,
    pub roots: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Unit {
    pub pkg_id: String,
    pub target: Target,
    // Native effective profiles contain evolving codegen/debug/strip fields. Preserve their
    // complete structured values; do not substitute the requested manifest profile's spelling.
    pub profile: BTreeMap<String, serde_json::Value>,
    pub platform: Option<String>,
    pub mode: String,
    pub features: Vec<String>,
    #[serde(default)]
    pub is_std: bool,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: String,
    #[serde(flatten)]
    pub remaining: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Dependency {
    pub index: usize,
    pub extern_crate_name: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub noprelude: bool,
    #[serde(default)]
    pub nounused: bool,
}

pub(in crate::fabric) struct CapturedUnitGraph {
    pub digest: [u8; 32],
    pub bytes: Vec<u8>,
    graph: UnitGraph,
    roots: BTreeSet<usize>,
}

pub(super) struct SelectedUnitGraph {
    pub captured: ChargedValue<CapturedUnitGraph>,
    pub context: [u8; 16],
    pub run: [u8; 16],
}

/// Retains only the latest pass's immutable dictionaries. Retention never selects a graph
/// for a generation: only a new contained Cargo observation may produce selection rows.
pub(in crate::fabric) struct Cache {
    budget: ResourceBudget,
    artifacts: BTreeMap<[u8; 32], ChargedValue<CapturedUnitGraph>>,
    last_used: Instant,
}

impl Cache {
    pub(in crate::fabric) fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            artifacts: BTreeMap::new(),
            last_used: Instant::now(),
        }
    }

    pub(super) fn clear(&mut self) {
        self.artifacts.clear();
    }

    pub(super) fn unselected(&mut self) -> Vec<ChargedValue<CapturedUnitGraph>> {
        self.evict_idle(Instant::now());
        self.last_used = Instant::now();
        self.artifacts.values().cloned().collect()
    }

    pub(super) fn replace(&mut self, selections: &[SelectedUnitGraph]) {
        self.artifacts = selections
            .iter()
            .map(|selection| (selection.captured.digest, selection.captured.clone()))
            .collect();
        self.last_used = Instant::now();
    }

    pub(in crate::fabric::production_workspace_startup) fn evict_idle(&mut self, now: Instant) {
        let observation = self.budget.observation();
        let headroom = u128::from(
            observation
                .policy
                .limits
                .memory_bytes
                .saturating_sub(observation.policy.control_reserve.memory_bytes),
        )
        .saturating_sub(observation.used.memory_bytes);
        if now.saturating_duration_since(self.last_used) >= Duration::from_secs(600)
            || headroom < 2 * 1024 * 1024 * 1024
        {
            self.clear();
        }
    }
}

pub(super) fn capture(
    bytes: Vec<u8>,
    budget: &ResourceBudget,
) -> Result<ChargedValue<CapturedUnitGraph>, ProductionWorkspaceStartupError> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(step(
            "cargo-unit-graph",
            "native graph exceeds capture limit",
        ));
    }
    let capacity = crate::inventory::reserve_memory(
        budget,
        (bytes.len() as u64)
            .saturating_mul(32)
            .saturating_add(1024 * 1024),
    )
    .map_err(|error| step("cargo-unit-graph-memory", error))?;
    let graph: UnitGraph =
        serde_json::from_slice(&bytes).map_err(|error| step("cargo-unit-graph-json", error))?;
    validate(&graph).map_err(|error| step("cargo-unit-graph-shape", error))?;
    Ok(capacity.into_charged_value(CapturedUnitGraph {
        digest: super::super::digest32(b"codefabric.native-cargo-unit-graph.v1\0", &[&bytes]),
        bytes,
        roots: graph.roots.iter().copied().collect(),
        graph,
    }))
}

fn validate(graph: &UnitGraph) -> Result<(), &'static str> {
    if graph.version != 1
        || graph.units.is_empty()
        || graph.units.len() > 100_000
        || graph.roots.is_empty()
    {
        return Err("unsupported or incomplete unit graph");
    }
    if graph.roots.iter().any(|root| *root >= graph.units.len())
        || graph.roots.iter().copied().collect::<BTreeSet<_>>().len() != graph.roots.len()
    {
        return Err("invalid native root indices");
    }
    let mut edges = 0;
    for (index, unit) in graph.units.iter().enumerate() {
        if unit.pkg_id.is_empty()
            || unit.target.name.is_empty()
            || unit.target.kind.is_empty()
            || unit.target.crate_types.is_empty()
            || !(unit.target.src_path.starts_with("/workspace/")
                || unit.target.src_path.starts_with("/dependencies/"))
            || unit.profile.is_empty()
            || !matches!(
                unit.mode.as_str(),
                "build" | "check" | "test" | "doc" | "doctest" | "run-custom-build"
            )
        {
            return Err("unqualified native unit identity or mode");
        }
        edges += unit.dependencies.len();
        if edges > 1_000_000
            || unit.dependencies.iter().any(|dependency| {
                dependency.index >= graph.units.len()
                    || dependency.index == index
                    || dependency.extern_crate_name.is_empty()
            })
        {
            return Err("invalid native dependency indices");
        }
    }
    Ok(())
}

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    selections: &[SelectedUnitGraph],
    unselected: &[ChargedValue<CapturedUnitGraph>],
    workspace: [u8; 16],
    generation: u64,
) -> Result<(), ProductionWorkspaceStartupError> {
    let artifacts = selections
        .iter()
        .map(|selection| (selection.captured.digest, &*selection.captured))
        .chain(
            unselected
                .iter()
                .map(|artifact| (artifact.digest, &**artifact)),
        )
        .collect::<BTreeMap<_, _>>();
    let identity = super::super::digest32(
        b"codefabric.cargo-unit-graph-artifacts.v1\0",
        &artifacts
            .keys()
            .map(<[u8; 32]>::as_slice)
            .collect::<Vec<_>>(),
    );
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "cargo_unit_graph",
        vec![
            (
                "graph_digest",
                false,
                hash32_array(artifacts.keys().map(Some)),
            ),
            (
                "format_version",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    artifacts
                        .values()
                        .map(|artifact| u64::from(artifact.graph.version)),
                )),
            ),
            (
                "native_graph_json",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    artifacts.values().map(|artifact| artifact.bytes.as_slice()),
                )),
            ),
        ],
        identity,
    )?;
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "cargo_unit_graph_selection",
        vec![
            (
                "workspace_id",
                false,
                id16_array(selections.iter().map(|_| Some(&workspace))),
            ),
            (
                "source_generation",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    selections.iter().map(|_| generation),
                )),
            ),
            (
                "context_id",
                false,
                id16_array(selections.iter().map(|selection| Some(&selection.context))),
            ),
            (
                "provider_run_id",
                false,
                id16_array(selections.iter().map(|selection| Some(&selection.run))),
            ),
            (
                "graph_digest",
                false,
                hash32_array(
                    selections
                        .iter()
                        .map(|selection| Some(&selection.captured.digest)),
                ),
            ),
        ],
    )?;
    // Unit ordinals are local to the immutable native graph. They never become canonical CPG IDs.
    let units = || {
        artifacts.values().flat_map(|artifact| {
            artifact
                .graph
                .units
                .iter()
                .enumerate()
                .map(move |(index, unit)| (artifact, index, unit))
        })
    };
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "cargo_compilation_unit",
        vec![
            (
                "graph_digest",
                false,
                hash32_array(units().map(|(artifact, _, _)| Some(&artifact.digest))),
            ),
            (
                "unit_index",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    units().map(|(_, index, _)| index as u64),
                )),
            ),
            (
                "package_id",
                false,
                Arc::new(StringArray::from_iter(
                    units().map(|(_, _, unit)| Some(unit.pkg_id.as_str())),
                )),
            ),
            (
                "target_name",
                false,
                Arc::new(StringArray::from_iter(
                    units().map(|(_, _, unit)| Some(unit.target.name.as_str())),
                )),
            ),
            (
                "target_kinds",
                false,
                string_lists(units().map(|(_, _, unit)| unit.target.kind.as_slice())),
            ),
            (
                "crate_types",
                false,
                string_lists(units().map(|(_, _, unit)| unit.target.crate_types.as_slice())),
            ),
            (
                "features",
                false,
                string_lists(units().map(|(_, _, unit)| unit.features.as_slice())),
            ),
            (
                "effective_profile_json",
                false,
                Arc::new(StringArray::from_iter(units().map(|(_, _, unit)| {
                    Some(
                        serde_json::to_string(&unit.profile)
                            .expect("parsed profile serialization is infallible"),
                    )
                }))),
            ),
            (
                "target_configuration_json",
                false,
                Arc::new(StringArray::from_iter(units().map(|(_, _, unit)| {
                    Some(
                        serde_json::to_string(&unit.target.remaining)
                            .expect("parsed target serialization is infallible"),
                    )
                }))),
            ),
            (
                "is_standard_library",
                false,
                Arc::new(BooleanArray::from_iter(
                    units().map(|(_, _, unit)| Some(unit.is_std)),
                )),
            ),
            (
                "is_root",
                false,
                Arc::new(BooleanArray::from_iter(units().map(
                    |(artifact, index, _)| Some(artifact.roots.contains(&index)),
                ))),
            ),
            (
                "source_path",
                false,
                Arc::new(BinaryArray::from_iter(
                    units().map(|(_, _, unit)| Some(unit.target.src_path.as_bytes())),
                )),
            ),
            (
                "mode",
                false,
                Arc::new(StringArray::from_iter(
                    units().map(|(_, _, unit)| Some(unit.mode.as_str())),
                )),
            ),
            (
                "platform",
                true,
                Arc::new(StringArray::from_iter(
                    units().map(|(_, _, unit)| unit.platform.as_deref()),
                )),
            ),
        ],
        identity,
    )?;
    let edges = || {
        artifacts.values().flat_map(|artifact| {
            artifact
                .graph
                .units
                .iter()
                .enumerate()
                .flat_map(move |(index, unit)| {
                    unit.dependencies
                        .iter()
                        .map(move |dependency| (artifact, index, dependency))
                })
        })
    };
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "cargo_unit_dependency",
        vec![
            (
                "graph_digest",
                false,
                hash32_array(edges().map(|(artifact, _, _)| Some(&artifact.digest))),
            ),
            (
                "unit_index",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    edges().map(|(_, index, _)| index as u64),
                )),
            ),
            (
                "dependency_index",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    edges().map(|(_, _, dependency)| dependency.index as u64),
                )),
            ),
            (
                "extern_crate_name",
                false,
                Arc::new(StringArray::from_iter(edges().map(|(_, _, dependency)| {
                    Some(dependency.extern_crate_name.as_str())
                }))),
            ),
            (
                "public",
                false,
                Arc::new(BooleanArray::from_iter(
                    edges().map(|(_, _, dependency)| Some(dependency.public)),
                )),
            ),
            (
                "no_prelude",
                false,
                Arc::new(BooleanArray::from_iter(
                    edges().map(|(_, _, dependency)| Some(dependency.noprelude)),
                )),
            ),
            (
                "no_unused",
                false,
                Arc::new(BooleanArray::from_iter(
                    edges().map(|(_, _, dependency)| Some(dependency.nounused)),
                )),
            ),
        ],
        identity,
    )
}

fn string_lists<'a>(values: impl IntoIterator<Item = &'a [String]>) -> ArrayRef {
    let mut builder = ListBuilder::new(StringBuilder::new());
    for values in values {
        for value in values {
            builder.values().append_value(value);
        }
        builder.append(true);
    }
    Arc::new(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_unit_graph_keeps_effective_modes_features_profiles_and_rejects_invalid_indices() {
        let unit = serde_json::json!({
            "pkg_id": "path+file:///workspace#fixture@0.1.0",
            "target": {"name":"build-script-build","kind":["custom-build"],"crate_types":["bin"],"src_path":"/workspace/build.rs","edition":"2024"},
            "profile": {"name":"dev","debuginfo":0,"incremental":true,"strip":{"deferred":"None"}},
            "platform":null,"mode":"build","features":["feature"],"dependencies":[]
        });
        let mut native = serde_json::json!({"version":1,"units":[unit.clone(),unit],"roots":[1]});
        native["units"][1]["mode"] = "run-custom-build".into();
        native["units"][1]["profile"]["incremental"] = false.into();
        native["units"][1]["dependencies"] =
            serde_json::json!([{"index":0,"extern_crate_name":"build_script_build"}]);
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let captured = capture(serde_json::to_vec(&native).unwrap(), &budget).unwrap();
        assert_eq!(captured.graph.units[0].features, ["feature"]);
        assert_eq!(captured.graph.units[1].mode, "run-custom-build");
        assert_eq!(captured.graph.units[1].profile["incremental"], false);
        let mut cache = Cache::new(budget.clone());
        cache.replace(&[SelectedUnitGraph {
            captured: captured.clone(),
            context: [1; 16],
            run: [2; 16],
        }]);
        let leased = cache.unselected();
        assert_eq!(leased[0].digest, captured.digest);
        cache.evict_idle(Instant::now() + Duration::from_secs(601));
        assert!(cache.unselected().is_empty());
        assert_eq!(
            leased[0].bytes, captured.bytes,
            "eviction cannot revoke a publication lease"
        );

        native["units"][1]["dependencies"][0]["index"] = 1.into();
        assert!(capture(serde_json::to_vec(&native).unwrap(), &budget).is_err());
        native["version"] = 2.into();
        assert!(capture(serde_json::to_vec(&native).unwrap(), &budget).is_err());
    }
}
