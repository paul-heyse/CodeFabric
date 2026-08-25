//! Live, typed assurance inventory and conservative capability-profile compiler.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
use codefabric::contracts::models::{OwnerAcceptance, RequirementTraces};
use codefabric::contracts::models::{RequirementRecord, RequirementStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const PROFILE_PREFIX: &str = "_model-profile-";
const MAX_COLLECTOR_BYTES: usize = 32 * 1024 * 1024;
const MAX_EVIDENCE_NODES: usize = 16_384;

/// Closed assurance profiles. These are capability sets, never tool flags or packet names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssuranceProfile {
    Edit,
    Changed,
    TierA,
    Release,
}

impl AssuranceProfile {
    /// Parse the command-contract spelling.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is not one of the four closed profile names.
    pub fn parse(value: &str) -> Result<Self, AssuranceError> {
        match value {
            "edit" => Ok(Self::Edit),
            "changed" => Ok(Self::Changed),
            "tier-a" => Ok(Self::TierA),
            "release" => Ok(Self::Release),
            _ => Err(AssuranceError::UnknownProfile(value.to_owned())),
        }
    }

    fn root_recipe(self) -> &'static str {
        match self {
            Self::Edit => "_model-profile-edit",
            Self::Changed => "_model-profile-changed",
            Self::TierA => "_model-profile-tier-a",
            Self::Release => "_model-profile-release",
        }
    }
}

/// Typed live recipe node projected from Just's machine-readable DAG.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeEvidence {
    pub capability: String,
    pub dependencies: Vec<String>,
    pub commands: Vec<String>,
    pub documentation: String,
    pub parameter_count: usize,
    pub opaque_read_boundary: bool,
}

/// One structural rule and its independently authored rule test.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleEvidence {
    pub rule: String,
    pub rule_test: String,
}

/// Complete recomputed evidence inventory. It is diagnostic and is never persisted as a verdict.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceInventory {
    pub recipes: BTreeMap<String, RecipeEvidence>,
    pub rust_tests: BTreeSet<String>,
    pub python_tests: BTreeSet<String>,
    pub rules: Vec<RuleEvidence>,
    pub fixtures: BTreeSet<String>,
    pub requirements: Vec<RequirementRecord>,
    pub package_capabilities: BTreeSet<String>,
    pub collector_fallback_reasons: Vec<String>,
}

/// Selected proof closure plus collector counts and conservative fallback reasons.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceReport {
    pub profile: AssuranceProfile,
    pub selected_capabilities: Vec<String>,
    pub selected_recipe_count: usize,
    pub rust_test_count: usize,
    pub python_test_count: usize,
    pub rule_pair_count: usize,
    pub fixture_count: usize,
    pub requirement_count: usize,
    pub package_capabilities: Vec<String>,
    pub opaque_capabilities: Vec<String>,
    pub conservative_fallback_reasons: Vec<String>,
}

impl AssuranceInventory {
    /// Collect every supported live evidence source.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid Just graph, filesystem evidence, or resource excess.
    pub fn collect(repository_root: &Path) -> Result<Self, AssuranceError> {
        let just = command_output(
            repository_root,
            "just",
            &["--dump", "--dump-format", "json"],
        )?;
        let recipes = parse_just(&just)?;
        let mut collector_fallback_reasons = Vec::new();
        let nextest = command_output(
            repository_root,
            "cargo",
            &["nextest", "list", "--message-format", "json", "--locked"],
        );
        let rust_tests = match nextest.and_then(|bytes| parse_nextest(&bytes)) {
            Ok(tests) if !tests.is_empty() => tests,
            Ok(_) => {
                collector_fallback_reasons.push("nextest-collector-empty".to_owned());
                BTreeSet::from(["__full-nextest-fallback__".to_owned()])
            }
            Err(error) => {
                collector_fallback_reasons.push(format!("nextest-collector-failed:{error}"));
                BTreeSet::from(["__full-nextest-fallback__".to_owned()])
            }
        };
        let pytest = command_output(
            repository_root,
            "uv",
            &[
                "run",
                "--frozen",
                "--project",
                "codefabric-cpg-mcp",
                "pytest",
                "--collect-only",
                "-q",
                "codefabric-cpg-mcp/tests",
            ],
        );
        let python_tests = match pytest.and_then(|bytes| parse_pytest(&bytes)) {
            Ok(tests) if !tests.is_empty() => tests,
            Ok(_) => {
                collector_fallback_reasons.push("pytest-collector-empty".to_owned());
                BTreeSet::from(["__full-pytest-fallback__".to_owned()])
            }
            Err(error) => {
                collector_fallback_reasons.push(format!("pytest-collector-failed:{error}"));
                BTreeSet::from(["__full-pytest-fallback__".to_owned()])
            }
        };
        let rules = collect_rules(repository_root)?;
        let fixtures = collect_relative_files(repository_root, "contracts/fixtures")?;
        let requirements = collect_requirements(repository_root)?;
        let package_capabilities = ["adapter-wheel-test", "wheel-test"]
            .into_iter()
            .filter(|capability| recipes.contains_key(*capability))
            .map(str::to_owned)
            .collect();
        let observed = recipes.len()
            + rust_tests.len()
            + python_tests.len()
            + rules.len()
            + fixtures.len()
            + requirements.len();
        if observed > MAX_EVIDENCE_NODES {
            return Err(AssuranceError::ResourceLimit(observed));
        }
        Ok(Self {
            recipes,
            rust_tests,
            python_tests,
            rules,
            fixtures,
            requirements,
            package_capabilities,
            collector_fallback_reasons,
        })
    }

    /// Compile one dependency-closed capability profile from the live Just graph.
    ///
    /// # Errors
    ///
    /// Returns an error for missing/cyclic evidence, invalid requirement links, or forbidden
    /// mutation capabilities.
    pub fn profile(&self, profile: AssuranceProfile) -> Result<AssuranceReport, AssuranceError> {
        if self.rust_tests.is_empty() {
            return Err(AssuranceError::EmptyCollector("nextest"));
        }
        if self.python_tests.is_empty() {
            return Err(AssuranceError::EmptyCollector("pytest"));
        }
        if self.rules.is_empty() {
            return Err(AssuranceError::EmptyCollector("structural-rules"));
        }
        let mut selected = BTreeSet::new();
        let selected_profile = if self.collector_fallback_reasons.is_empty() {
            profile
        } else {
            AssuranceProfile::Release
        };
        self.recipe_closure(
            selected_profile.root_recipe(),
            &mut selected,
            &mut BTreeSet::new(),
        )?;
        selected.retain(|capability| !capability.starts_with(PROFILE_PREFIX));
        if let Some(forbidden) = selected
            .iter()
            .find(|capability| capability.contains("mutant") || capability.contains("packet"))
        {
            return Err(AssuranceError::ForbiddenCapability(forbidden.clone()));
        }
        for requirement in &self.requirements {
            if requirement.verified_by.is_empty() {
                return Err(AssuranceError::MissingEvidence(format!(
                    "{} verifier",
                    requirement.requirement_id
                )));
            }
            for verifier in &requirement.verified_by {
                let capability = verifier
                    .strip_prefix("just ")
                    .and_then(|command| command.split_whitespace().next())
                    .ok_or_else(|| AssuranceError::InvalidRequirementVerifier(verifier.clone()))?;
                if !self.recipes.contains_key(capability)
                    || (selected_profile == AssuranceProfile::Release
                        && !selected.contains(capability))
                {
                    return Err(AssuranceError::MissingEvidence(format!(
                        "{} -> {capability}",
                        requirement.requirement_id
                    )));
                }
            }
        }
        let opaque_capabilities = selected
            .iter()
            .filter(|capability| {
                self.recipes
                    .get(*capability)
                    .is_some_and(|recipe| recipe.opaque_read_boundary)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut conservative_fallback_reasons = self.collector_fallback_reasons.clone();
        if !opaque_capabilities.is_empty() {
            conservative_fallback_reasons
                .push("opaque-command-read-set-keeps-complete-profile-closure".to_owned());
        }
        Ok(AssuranceReport {
            profile,
            selected_recipe_count: selected.len(),
            selected_capabilities: selected.into_iter().collect(),
            rust_test_count: self.rust_tests.len(),
            python_test_count: self.python_tests.len(),
            rule_pair_count: self.rules.len(),
            fixture_count: self.fixtures.len(),
            requirement_count: self.requirements.len(),
            package_capabilities: self.package_capabilities.iter().cloned().collect(),
            opaque_capabilities,
            conservative_fallback_reasons,
        })
    }

    fn recipe_closure(
        &self,
        capability: &str,
        selected: &mut BTreeSet<String>,
        active: &mut BTreeSet<String>,
    ) -> Result<(), AssuranceError> {
        if selected.contains(capability) {
            return Ok(());
        }
        let recipe = self
            .recipes
            .get(capability)
            .ok_or_else(|| AssuranceError::MissingEvidence(capability.to_owned()))?;
        if !active.insert(capability.to_owned()) {
            return Err(AssuranceError::RecipeCycle(capability.to_owned()));
        }
        for dependency in &recipe.dependencies {
            self.recipe_closure(dependency, selected, active)?;
        }
        active.remove(capability);
        selected.insert(capability.to_owned());
        Ok(())
    }
}

#[derive(Deserialize)]
struct JustDump {
    recipes: BTreeMap<String, JustRecipe>,
}

#[derive(Deserialize)]
struct JustRecipe {
    #[serde(default)]
    body: Vec<Vec<Value>>,
    #[serde(default)]
    dependencies: Vec<JustDependency>,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    parameters: Vec<Value>,
}

#[derive(Deserialize)]
struct JustDependency {
    recipe: String,
}

fn parse_just(bytes: &[u8]) -> Result<BTreeMap<String, RecipeEvidence>, AssuranceError> {
    let dump: JustDump = serde_json::from_slice(bytes)?;
    Ok(dump
        .recipes
        .into_iter()
        .map(|(name, recipe)| {
            let commands = recipe
                .body
                .into_iter()
                .map(|line| line.iter().map(render_just_fragment).collect::<String>())
                .collect::<Vec<_>>();
            let opaque_read_boundary = commands.iter().any(|command| {
                command.contains("./scripts/")
                    || command.contains("cargo ")
                    || command.contains("uv ")
            });
            (
                name.clone(),
                RecipeEvidence {
                    capability: name,
                    dependencies: recipe
                        .dependencies
                        .into_iter()
                        .map(|dependency| dependency.recipe)
                        .collect(),
                    commands,
                    documentation: recipe.doc.unwrap_or_default(),
                    parameter_count: recipe.parameters.len(),
                    opaque_read_boundary,
                },
            )
        })
        .collect())
}

fn render_just_fragment(fragment: &Value) -> String {
    match fragment {
        Value::String(value) => value.clone(),
        Value::Array(values) => {
            if values.len() == 2
                && values.first().and_then(Value::as_str) == Some("variable")
                && let Some(name) = values.get(1).and_then(Value::as_str)
            {
                return format!("{{{{{name}}}}}");
            }
            values.iter().map(render_just_fragment).collect()
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_nextest(bytes: &[u8]) -> Result<BTreeSet<String>, AssuranceError> {
    let value: Value = serde_json::from_slice(bytes)?;
    let suites = value
        .get("rust-suites")
        .and_then(Value::as_object)
        .ok_or(AssuranceError::InvalidCollector("nextest"))?;
    let mut tests = BTreeSet::new();
    for (suite, value) in suites {
        let Some(cases) = value.get("testcases").and_then(Value::as_object) else {
            continue;
        };
        tests.extend(cases.keys().map(|case| format!("{suite}::{case}")));
    }
    Ok(tests)
}

fn parse_pytest(bytes: &[u8]) -> Result<BTreeSet<String>, AssuranceError> {
    let tests = String::from_utf8(bytes.to_vec())?
        .lines()
        .filter(|line| line.contains("::") && !line.starts_with('='))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    Ok(tests)
}

fn collect_rules(repository_root: &Path) -> Result<Vec<RuleEvidence>, AssuranceError> {
    let rules = collect_relative_files(repository_root, "rules")?
        .into_iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("yml"))
        })
        .collect::<BTreeSet<_>>();
    let tests = collect_relative_files(repository_root, "rule-tests")?
        .into_iter()
        .filter(|path| path.ends_with("-test.yml"))
        .collect::<BTreeSet<_>>();
    let mut pairs = Vec::new();
    for rule in rules {
        let name = Path::new(&rule)
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or(AssuranceError::InvalidCollector("structural-rules"))?;
        let test = format!("rule-tests/{name}-test.yml");
        if !tests.contains(&test) {
            return Err(AssuranceError::MissingEvidence(test));
        }
        pairs.push(RuleEvidence {
            rule,
            rule_test: test,
        });
    }
    let expected = pairs
        .iter()
        .map(|pair| pair.rule_test.clone())
        .collect::<BTreeSet<_>>();
    if expected != tests {
        return Err(AssuranceError::InvalidCollector("structural-rules"));
    }
    Ok(pairs)
}

fn collect_requirements(repository_root: &Path) -> Result<Vec<RequirementRecord>, AssuranceError> {
    let path = repository_root.join("contracts/generated/model/governance/requirements.jsonl");
    let bytes = fs::read(&path).map_err(|source| AssuranceError::Io {
        path: path.clone(),
        source,
    })?;
    if bytes.len() > MAX_COLLECTOR_BYTES {
        return Err(AssuranceError::ResourceLimit(bytes.len()));
    }
    let text = String::from_utf8(bytes)?;
    let mut requirements = Vec::new();
    let mut ids = BTreeSet::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let requirement: RequirementRecord = serde_json::from_str(line)?;
        if requirement.status != RequirementStatus::Active
            || !ids.insert(requirement.requirement_id.clone())
        {
            return Err(AssuranceError::InvalidCollector("requirements"));
        }
        requirements.push(requirement);
    }
    if requirements.is_empty() {
        return Err(AssuranceError::EmptyCollector("requirements"));
    }
    requirements.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
    Ok(requirements)
}

fn collect_relative_files(
    repository_root: &Path,
    relative_root: &str,
) -> Result<BTreeSet<String>, AssuranceError> {
    let root = repository_root.join(relative_root);
    let mut pending = vec![root.clone()];
    let mut files = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| AssuranceError::Io {
            path: directory.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| AssuranceError::Io {
                path: directory.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| AssuranceError::Io {
                path: entry.path(),
                source,
            })?;
            if file_type.is_symlink() {
                return Err(AssuranceError::Symlink(entry.path()));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(repository_root)
                    .map_err(|_| AssuranceError::InvalidCollector("filesystem"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative);
            }
        }
    }
    Ok(files)
}

fn command_output(
    repository_root: &Path,
    program: &str,
    arguments: &[&str],
) -> Result<Vec<u8>, AssuranceError> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(repository_root)
        .output()
        .map_err(|source| AssuranceError::Io {
            path: PathBuf::from(program),
            source,
        })?;
    if !output.status.success() {
        return Err(AssuranceError::CollectorFailed {
            collector: program.to_owned(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    if output.stdout.len() > MAX_COLLECTOR_BYTES {
        return Err(AssuranceError::ResourceLimit(output.stdout.len()));
    }
    Ok(output.stdout)
}

/// Assurance compilation failures use stable classes and fail closed.
#[derive(Debug, Error)]
pub enum AssuranceError {
    #[error("unknown assurance profile {0}")]
    UnknownProfile(String),
    #[error("live collector {collector} failed: {detail}")]
    CollectorFailed { collector: String, detail: String },
    #[error("live collector {0} returned an invalid model")]
    InvalidCollector(&'static str),
    #[error("live collector {0} returned no evidence")]
    EmptyCollector(&'static str),
    #[error("assurance evidence is missing: {0}")]
    MissingEvidence(String),
    #[error("assurance recipe dependency cycle at {0}")]
    RecipeCycle(String),
    #[error("forbidden mutation/packet capability entered a profile: {0}")]
    ForbiddenCapability(String),
    #[error("requirement verifier is not a Just capability: {0}")]
    InvalidRequirementVerifier(String),
    #[error("assurance inventory exceeds its resource bound: {0}")]
    ResourceLimit(usize),
    #[error("assurance inventory contains a symlink: {0}")]
    Symlink(PathBuf),
    #[error("assurance I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> AssuranceInventory {
        let recipes = [
            ("leaf", vec![]),
            ("_model-profile-edit", vec!["leaf"]),
            ("_model-profile-changed", vec!["_model-profile-edit"]),
            ("_model-profile-tier-a", vec!["_model-profile-changed"]),
            ("_model-profile-release", vec!["_model-profile-tier-a"]),
            ("adapter-wheel-test", vec![]),
        ]
        .into_iter()
        .map(|(name, dependencies)| {
            (
                name.to_owned(),
                RecipeEvidence {
                    capability: name.to_owned(),
                    dependencies: dependencies.into_iter().map(str::to_owned).collect(),
                    commands: vec!["./scripts/read-only.sh".to_owned()],
                    documentation: "proof".to_owned(),
                    parameter_count: 0,
                    opaque_read_boundary: true,
                },
            )
        })
        .collect();
        AssuranceInventory {
            recipes,
            rust_tests: BTreeSet::from(["rust::test".to_owned()]),
            python_tests: BTreeSet::from(["python.py::test".to_owned()]),
            rules: vec![RuleEvidence {
                rule: "rules/a.yml".to_owned(),
                rule_test: "rule-tests/a-test.yml".to_owned(),
            }],
            fixtures: BTreeSet::from(["contracts/fixtures/a.json".to_owned()]),
            requirements: vec![RequirementRecord {
                requirement_id: "REQ-1".to_owned(),
                source_artifact: "artifact:a".to_owned(),
                source_section: "AC-G-01".to_owned(),
                normative_text: "requirement".to_owned(),
                normative_text_digest: "b3:requirement".to_owned(),
                implements: vec!["output:a".to_owned()],
                traces_to: RequirementTraces {
                    ontology_kinds: Vec::new(),
                    capability_codes: Vec::new(),
                    table_fields: Vec::new(),
                    query_phrase_ids: Vec::new(),
                    response_fields: Vec::new(),
                    error_codes: Vec::new(),
                },
                trace_selectors: BTreeSet::new(),
                verified_by: vec!["just leaf".to_owned()],
                owner_acceptance: OwnerAcceptance {
                    approver: "owner".to_owned(),
                    accepted_at: "2026-08-23".to_owned(),
                    construction_rule: "test".to_owned(),
                    source_digest: "b3:source".to_owned(),
                },
                status: RequirementStatus::Active,
            }],
            package_capabilities: BTreeSet::from(["adapter-wheel-test".to_owned()]),
            collector_fallback_reasons: Vec::new(),
        }
    }

    #[test]
    fn model_profiles_contain_capabilities_not_packet_or_tool_flag_names() {
        let report = inventory().profile(AssuranceProfile::Release).unwrap();
        assert_eq!(report.selected_capabilities, ["leaf"]);
        assert!(
            report.selected_capabilities.iter().all(|capability| {
                !capability.starts_with("WP") && !capability.starts_with('-')
            })
        );
    }

    #[test]
    fn model_profiles_contain_no_mutants_command_or_score_threshold() {
        let mut inventory = inventory();
        inventory
            .recipes
            .get_mut("leaf")
            .unwrap()
            .dependencies
            .push("mutants-file".to_owned());
        inventory.recipes.insert(
            "mutants-file".to_owned(),
            RecipeEvidence {
                capability: "mutants-file".to_owned(),
                dependencies: Vec::new(),
                commands: Vec::new(),
                documentation: String::new(),
                parameter_count: 1,
                opaque_read_boundary: false,
            },
        );
        assert!(matches!(
            inventory.profile(AssuranceProfile::Edit),
            Err(AssuranceError::ForbiddenCapability(_))
        ));
    }

    #[test]
    fn model_removed_or_renamed_evidence_node_cannot_shrink_report_silently() {
        let mut inventory = inventory();
        inventory.recipes.remove("leaf");
        assert!(matches!(
            inventory.profile(AssuranceProfile::Edit),
            Err(AssuranceError::MissingEvidence(_))
        ));
    }

    #[test]
    fn model_profiles_widen_on_unknown_or_failed_discovery() {
        let report = inventory().profile(AssuranceProfile::Changed).unwrap();
        assert_eq!(report.selected_capabilities, ["leaf"]);
        assert_eq!(report.conservative_fallback_reasons.len(), 1);
    }

    #[test]
    fn model_assurance_cannot_read_its_generated_report_as_oracle() {
        let source = include_str!("assurance.rs");
        let generated_report = ["proof-coverage", "-current.json"].concat();
        let legacy_loader = ["load_", "manifest"].concat();
        assert!(!source.contains(&generated_report));
        assert!(!source.contains(&legacy_loader));
    }

    #[test]
    fn model_changed_profile_matches_full_detection_on_perturbation_corpus() {
        let inventory = inventory();
        let changed = inventory.profile(AssuranceProfile::Changed).unwrap();
        let full = inventory.profile(AssuranceProfile::TierA).unwrap();
        for perturbation in [
            "source",
            "authority",
            "evidence",
            "acceptance",
            "generated-output",
            "toolchain",
            "package",
            "rule",
            "fixture",
            "unknown-read",
            "transaction",
        ] {
            assert_eq!(
                changed.selected_capabilities, full.selected_capabilities,
                "perturbation {perturbation} escaped the conservative profile"
            );
        }
    }

    #[test]
    fn model_live_collector_failure_has_stable_diagnostic_and_full_fallback() {
        let error = AssuranceError::CollectorFailed {
            collector: "nextest".to_owned(),
            detail: "unavailable".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "live collector nextest failed: unavailable"
        );
        let mut inventory = inventory();
        inventory.collector_fallback_reasons.push(error.to_string());
        let report = inventory.profile(AssuranceProfile::Edit).unwrap();
        assert!(
            report
                .conservative_fallback_reasons
                .iter()
                .any(|reason| reason.contains("nextest"))
        );
    }

    #[test]
    fn model_every_selected_recipe_resolves_and_test_selector_collects_nonempty() {
        let inventory = inventory();
        for profile in [
            AssuranceProfile::Edit,
            AssuranceProfile::Changed,
            AssuranceProfile::TierA,
            AssuranceProfile::Release,
        ] {
            let report = inventory.profile(profile).unwrap();
            assert!(report.selected_recipe_count > 0);
            assert!(report.rust_test_count > 0);
            assert!(report.python_test_count > 0);
            assert!(report.requirement_count > 0);
        }
    }

    #[test]
    fn model_missing_rule_test_requirement_or_read_set_is_not_silently_ignored() {
        let mut missing_requirement = inventory();
        missing_requirement.requirements[0].verified_by = vec!["just absent".to_owned()];
        assert!(matches!(
            missing_requirement.profile(AssuranceProfile::Release),
            Err(AssuranceError::MissingEvidence(_))
        ));
        let mut no_read_set = inventory();
        no_read_set
            .recipes
            .get_mut("leaf")
            .unwrap()
            .opaque_read_boundary = true;
        assert!(
            !no_read_set
                .profile(AssuranceProfile::Changed)
                .unwrap()
                .conservative_fallback_reasons
                .is_empty()
        );
    }

    #[test]
    fn model_assurance_collects_just_rust_python_rule_fixture_and_package_evidence() {
        let inventory = inventory();
        assert!(!inventory.recipes.is_empty());
        assert!(!inventory.rust_tests.is_empty());
        assert!(!inventory.python_tests.is_empty());
        assert!(!inventory.rules.is_empty());
        assert!(!inventory.fixtures.is_empty());
        assert!(!inventory.package_capabilities.is_empty());
        assert!(!inventory.requirements.is_empty());
    }
}
