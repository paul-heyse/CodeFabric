//! Selected context ingress for the pinned checker and its embedded stub bundles.
//! Source lease and per-module digest checks are enforced by the serving boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use pyrefly_config::config::{ConfigFile, ConfigSource};
use pyrefly_python::sys_info::{PythonPlatform, PythonVersion};
use serde::Deserialize;

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ROOTS: usize = 256;
const MAX_MODULES: usize = 16_384;
const IMPORT_PRECEDENCE: [&str; 6] = [
    "explicit-stub-roots",
    "workspace-module-roots",
    "workspace-source-roots",
    "authorized-dependency-roots",
    "typeshed-stdlib",
    "typeshed-third-party",
];

pub(crate) const UNAVAILABLE_DETAILS: &[u8] = b"codefabric.pyrefly.preparation-unavailable.v1";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparationRemainder {
    TypeshedBundleAuthorityUnavailable,
    PyreflyBundleAuthorityUnavailable,
    CheckerConfigurationAuthorityUnavailable,
    DependencyRootAuthorityUnavailable,
    UnsupportedImplementationProfile,
    UnsupportedNamespaceOrImportPolicy,
    UnsupportedPythonVersion,
    UnsupportedPlatforms,
    AmbiguousModuleSelection,
}

#[derive(Debug)]
pub(crate) enum PreparationError {
    InvalidManifest(&'static str),
    Unavailable(Vec<PreparationRemainder>),
}

// This is the closed ingress shape of PythonAnalysisContextManifest, not an opaque
// Value projection. Derive rejects duplicate fields (including nested objects) before
// they can collapse into maps. Unknown fields cannot silently become ineffective settings.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    context_kind: String,
    python_language_version: String,
    implementation_profile: String,
    platform_tag: String,
    module_roots: Vec<String>,
    source_roots: Vec<String>,
    stub_roots: Vec<String>,
    dependency_roots: Vec<String>,
    namespace_package_policy: String,
    import_precedence: Vec<String>,
    #[serde(deserialize_with = "required_nullable_digest")]
    typeshed_bundle_digest: Option<String>,
    lockfile_artifacts: Vec<Artifact>,
    project_config_artifacts: Vec<Artifact>,
    #[serde(default)]
    unapplied_checker_settings: Option<Vec<String>>,
    #[serde(deserialize_with = "required_nullable_digest")]
    pyrefly_bundle_digest: Option<String>,
    ruff_bundle_digest: String,
    provider_bundle_version: String,
    platforms: Vec<String>,
    root_bindings: Vec<Root>,
    module_map: Vec<ModuleBinding>,
    configuration_namespace: Vec<u8>,
    configuration_roots: Vec<Root>,
    configuration_policy_identity: [u8; 32],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    file_id: String,
    digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
    root_id: String,
    relative_path: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleBinding {
    module_name: String,
    file_id: String,
    relative_path: Vec<u8>,
    root_id: String,
    is_stub: bool,
    is_package: bool,
}

fn required_nullable_digest<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("b3:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[derive(Debug)]
pub(crate) struct SelectedPyreflyPreparation {
    manifest: Manifest,
    version: PythonVersion,
    module_indices: BTreeMap<String, usize>,
    #[cfg(test)]
    synthetic_fixture_modules: bool,
}

impl SelectedPyreflyPreparation {
    pub(crate) fn from_manifest(bytes: &[u8]) -> Result<Self, PreparationError> {
        let selected = Self::parse(bytes)?;
        let mut remainders = selected.unsupported_settings();
        // None explicitly selects the pinned checker's embedded defaults. An
        // external digest must match the actual selected bundle, never a claimed
        // source label. Runtime provider identity also binds the checker release.
        if selected
            .manifest
            .typeshed_bundle_digest
            .as_ref()
            .is_some_and(|expected| bundled_typeshed_digest() != Ok(expected))
        {
            remainders.push(PreparationRemainder::TypeshedBundleAuthorityUnavailable);
        }
        if selected
            .manifest
            .pyrefly_bundle_digest
            .as_ref()
            .is_some_and(|expected| {
                expected != &format!("b3:{}", crate::PYREFLY_LOCK_SOURCE_BLAKE3)
            })
        {
            remainders.push(PreparationRemainder::PyreflyBundleAuthorityUnavailable);
        }
        if !selected.configuration_applied() {
            remainders.push(PreparationRemainder::CheckerConfigurationAuthorityUnavailable);
        }
        if !selected.manifest.dependency_roots.is_empty()
            || !selected.manifest.stub_roots.is_empty()
            || !selected.manifest.lockfile_artifacts.is_empty()
        {
            remainders.push(PreparationRemainder::DependencyRootAuthorityUnavailable);
        }
        if remainders.is_empty() {
            Ok(selected)
        } else {
            Err(PreparationError::Unavailable(remainders))
        }
    }

    fn parse(bytes: &[u8]) -> Result<Self, PreparationError> {
        if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
            return Err(PreparationError::InvalidManifest("manifest byte bound"));
        }
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|_| PreparationError::InvalidManifest("closed manifest schema"))?;
        validate_manifest(&manifest)?;
        let (major, minor) = manifest
            .python_language_version
            .split_once('.')
            .filter(|(major, minor)| {
                !major.is_empty()
                    && !minor.is_empty()
                    && major
                        .bytes()
                        .chain(minor.bytes())
                        .all(|b| b.is_ascii_digit())
                    && (major.len() == 1 || !major.starts_with('0'))
                    && (minor.len() == 1 || !minor.starts_with('0'))
            })
            .ok_or(PreparationError::InvalidManifest("Python version spelling"))?;
        let version = PythonVersion {
            major: major
                .parse()
                .map_err(|_| PreparationError::InvalidManifest("Python major"))?,
            minor: minor
                .parse()
                .map_err(|_| PreparationError::InvalidManifest("Python minor"))?,
            micro: 0,
        };
        Ok(Self {
            module_indices: manifest
                .module_map
                .iter()
                .enumerate()
                .map(|(index, module)| (module.file_id.clone(), index))
                .collect(),
            manifest,
            version,
            #[cfg(test)]
            synthetic_fixture_modules: false,
        })
    }

    fn unsupported_settings(&self) -> Vec<PreparationRemainder> {
        let mut result = Vec::new();
        if self.manifest.implementation_profile != "cpython-semantics" {
            result.push(PreparationRemainder::UnsupportedImplementationProfile);
        }
        if self.manifest.namespace_package_policy != "pep420"
            || self.manifest.import_precedence != IMPORT_PRECEDENCE
        {
            result.push(PreparationRemainder::UnsupportedNamespaceOrImportPolicy);
        }
        if self.version.major != 3 || !(8..=14).contains(&self.version.minor) {
            result.push(PreparationRemainder::UnsupportedPythonVersion);
        }
        if self.manifest.platforms.is_empty()
            || self
                .manifest
                .platforms
                .iter()
                .any(|p| !matches!(p.as_str(), "linux" | "darwin" | "win32" | "all"))
            || (self.manifest.platforms.len() > 1
                && self.manifest.platforms.iter().any(|p| p == "all"))
        {
            result.push(PreparationRemainder::UnsupportedPlatforms);
        }
        let mut names = BTreeSet::new();
        if self
            .manifest
            .module_map
            .iter()
            .any(|module| !names.insert(&module.module_name))
        {
            result.push(PreparationRemainder::AmbiguousModuleSelection);
        }
        result
    }

    fn configuration_applied(&self) -> bool {
        match &self.manifest.unapplied_checker_settings {
            Some(settings) => settings.is_empty(),
            None => self.manifest.project_config_artifacts.is_empty(),
        }
    }

    pub(super) fn config_for_root(&self, root: &Path) -> Result<ConfigFile, String> {
        let paths = |ids: &[String]| {
            ids.iter()
                .map(|id| {
                    // All references were resolved by the closed parser.
                    let binding = self
                        .manifest
                        .root_bindings
                        .iter()
                        .find(|binding| binding.root_id == *id)
                        .unwrap();
                    root.join(OsString::from_vec(binding.relative_path.clone()))
                })
                .collect::<Vec<_>>()
        };
        let mut search_path = paths(&self.manifest.stub_roots);
        search_path.extend(paths(&self.manifest.module_roots));
        search_path.extend(paths(&self.manifest.source_roots));
        let dependencies = paths(&self.manifest.dependency_roots);
        for path in search_path.iter().chain(&dependencies) {
            std::fs::create_dir_all(path)
                .map_err(|_| "selected Pyrefly root could not be materialized".to_owned())?;
        }
        let mut config = ConfigFile {
            source: ConfigSource::File(root.join(ConfigFile::PYREFLY_FILE_NAME)),
            search_path_from_args: search_path,
            disable_search_path_heuristics: true,
            disable_project_excludes_heuristics: true,
            enable_fallback_search_path: false,
            ..ConfigFile::default()
        };
        config.python_environment.python_version = Some(self.version);
        config.python_environment.python_platform =
            Some(PythonPlatform::new_many(self.manifest.platforms.clone()));
        // Explicit Some, even when empty, prevents configure() discovering typings/.
        config.python_environment.site_package_path = Some(dependencies);
        config.interpreters.skip_interpreter_query = true;
        let errors = config.configure();
        if !errors.is_empty() {
            return Err("selected Pyrefly configuration could not be installed".to_owned());
        }
        Ok(config)
    }

    pub(super) fn module_path(
        &self,
        root: &Path,
        module: &super::ModuleInput,
    ) -> Result<PathBuf, String> {
        #[cfg(test)]
        if self.synthetic_fixture_modules {
            return super::provider_module_path(root, &module.module_name);
        }
        let binding = self
            .module_indices
            .get(&module.file_id)
            .map(|index| &self.manifest.module_map[*index])
            .filter(|binding| binding.module_name == module.module_name)
            .ok_or_else(|| "Pyrefly module is not in the selected context module map".to_owned())?;
        Ok(root.join(OsString::from_vec(binding.relative_path.clone())))
    }

    #[cfg(test)]
    pub(crate) fn test_only_from_manifest(bytes: &[u8]) -> Result<Self, PreparationError> {
        let selected = Self::parse(bytes)?;
        let remainders = selected.unsupported_settings();
        if !remainders.is_empty() {
            return Err(PreparationError::Unavailable(remainders));
        }
        if !selected.configuration_applied()
            || !selected.manifest.lockfile_artifacts.is_empty()
            || !selected.manifest.stub_roots.is_empty()
            || !selected.manifest.dependency_roots.is_empty()
        {
            return Err(PreparationError::Unavailable(vec![
                PreparationRemainder::CheckerConfigurationAuthorityUnavailable,
            ]));
        }
        Ok(selected)
    }

    #[cfg(test)]
    pub(crate) fn test_only_protocol_fixture() -> Self {
        let mut selected = Self::test_only_from_manifest(&test_manifest("3.13", "linux")).unwrap();
        selected.synthetic_fixture_modules = true;
        selected
    }
}

fn bundled_typeshed_digest() -> Result<&'static String, &'static str> {
    use pyrefly::module::bundled::BundledStub;
    static DIGEST: std::sync::OnceLock<Result<String, &'static str>> = std::sync::OnceLock::new();
    DIGEST
        .get_or_init(|| {
            let stdlib =
                pyrefly::module::typeshed::typeshed().map_err(|_| "bundled stdlib unavailable")?;
            let third_party = pyrefly::module::typeshed_third_party::typeshed_third_party()
                .map_err(|_| "bundled third-party stubs unavailable")?;
            let mut hash = blake3::Hasher::new();
            hash.update(b"codefabric.pyrefly.embedded-typeshed.v1\0");
            for (kind, mut files) in [
                (b"stdlib".as_slice(), stdlib.load_map().collect::<Vec<_>>()),
                (
                    b"third-party".as_slice(),
                    third_party.load_map().collect::<Vec<_>>(),
                ),
            ] {
                files.sort_by_key(|(path, _)| *path);
                hash.update(kind);
                for (path, contents) in files {
                    let name = path.to_string_lossy();
                    hash.update(&(name.len() as u64).to_be_bytes());
                    hash.update(name.as_bytes());
                    hash.update(&(contents.len() as u64).to_be_bytes());
                    hash.update(contents.as_bytes());
                }
            }
            Ok(format!("b3:{}", hash.finalize().to_hex()))
        })
        .as_ref()
        .map_err(|error| *error)
}

fn valid_path(path: &[u8], directory: bool) -> bool {
    (directory && path == b".")
        || (!path.is_empty()
            && !path.contains(&0)
            && path
                .split(|b| *b == b'/')
                .all(|part| !part.is_empty() && part != b"." && part != b".."))
}

fn within(path: &[u8], root: &[u8]) -> bool {
    root == b"."
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with(b"/"))
}

fn validate_roots(roots: &[Root], namespace: &[u8]) -> Result<(), PreparationError> {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    if roots.len() > MAX_ROOTS
        || roots.iter().any(|root| {
            root.root_id.is_empty()
                || !valid_path(&root.relative_path, true)
                || !within(&root.relative_path, namespace)
                || !ids.insert(&root.root_id)
                || !paths.insert(&root.relative_path)
        })
    {
        return Err(PreparationError::InvalidManifest(
            "root identity or containment",
        ));
    }
    Ok(())
}

fn validate_manifest(manifest: &Manifest) -> Result<(), PreparationError> {
    if manifest.context_kind != "python"
        || manifest.platform_tag.is_empty()
        || manifest.provider_bundle_version.is_empty()
        || manifest.configuration_policy_identity == [0; 32]
        || !valid_path(&manifest.configuration_namespace, true)
        || manifest.configuration_roots.is_empty()
        || manifest.root_bindings.is_empty()
        || manifest.module_map.len() > MAX_MODULES
    {
        return Err(PreparationError::InvalidManifest(
            "manifest identity or bounds",
        ));
    }
    validate_roots(
        &manifest.configuration_roots,
        &manifest.configuration_namespace,
    )?;
    validate_roots(&manifest.root_bindings, b".")?;
    for digest in std::iter::once(&manifest.ruff_bundle_digest)
        .chain(manifest.typeshed_bundle_digest.iter())
        .chain(manifest.pyrefly_bundle_digest.iter())
    {
        if !valid_digest(digest) {
            return Err(PreparationError::InvalidManifest("bundle digest framing"));
        }
    }
    for artifacts in [
        &manifest.lockfile_artifacts,
        &manifest.project_config_artifacts,
    ] {
        let mut files = BTreeSet::new();
        if artifacts.len() > MAX_MODULES
            || artifacts.iter().any(|a| {
                a.file_id.is_empty() || !files.insert(&a.file_id) || !valid_digest(&a.digest)
            })
        {
            return Err(PreparationError::InvalidManifest(
                "configuration artifact identity",
            ));
        }
    }
    let roots = manifest
        .root_bindings
        .iter()
        .map(|r| (&r.root_id, r.relative_path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    for selected in [
        &manifest.module_roots,
        &manifest.source_roots,
        &manifest.stub_roots,
        &manifest.dependency_roots,
    ] {
        let mut seen = BTreeSet::new();
        if selected.len() > MAX_ROOTS
            || selected
                .iter()
                .any(|id| !roots.contains_key(id) || !seen.insert(id))
        {
            return Err(PreparationError::InvalidManifest(
                "selected root has no unique binding",
            ));
        }
    }
    validate_modules(&manifest.module_map, &roots)?;
    let mut platforms = BTreeSet::new();
    if manifest.platforms.iter().any(|p| !platforms.insert(p)) {
        return Err(PreparationError::InvalidManifest(
            "duplicate selected platform",
        ));
    }
    Ok(())
}

fn validate_modules(
    modules: &[ModuleBinding],
    roots: &BTreeMap<&String, &[u8]>,
) -> Result<(), PreparationError> {
    let mut files = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for module in modules {
        let root = roots
            .get(&module.root_id)
            .ok_or(PreparationError::InvalidManifest("module root binding"))?;
        let suffix = if module.is_stub {
            b".pyi".as_slice()
        } else {
            b".py".as_slice()
        };
        if module.module_name.is_empty()
            || module.file_id.is_empty()
            || !files.insert(&module.file_id)
            || !paths.insert(&module.relative_path)
            || !valid_path(&module.relative_path, false)
            || !within(&module.relative_path, root)
            || !module.relative_path.ends_with(suffix)
            || (module.is_package
                && !module
                    .relative_path
                    .rsplit(|b| *b == b'/')
                    .next()
                    .is_some_and(|p| p == b"__init__.py" || p == b"__init__.pyi"))
        {
            return Err(PreparationError::InvalidManifest(
                "module identity, kind or containment",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_manifest(version: &str, platform: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "context_kind": "python", "python_language_version": version,
        "implementation_profile": "cpython-semantics", "platform_tag": platform,
        "module_roots": ["workspace"], "source_roots": [], "stub_roots": [], "dependency_roots": [],
        "namespace_package_policy": "pep420", "import_precedence": IMPORT_PRECEDENCE,
        "typeshed_bundle_digest": null, "lockfile_artifacts": [], "project_config_artifacts": [],
        "pyrefly_bundle_digest": null, "ruff_bundle_digest": super::b3(b"fixture-ruff"),
        "provider_bundle_version": "test-only-pyrefly-preparation", "platforms": [platform],
        "root_bindings": [{"root_id": "workspace", "relative_path": [46]}], "module_map": [],
        "configuration_namespace": [46], "configuration_roots": [{"root_id": "workspace", "relative_path": [46]}],
        "configuration_policy_identity": (vec![1; 32])
    })).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_configuration_requires_complete_application_of_checker_settings() {
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&test_manifest("3.14", "linux")).unwrap();
        manifest["project_config_artifacts"] = serde_json::json!([{
            "file_id": "file:config", "digest": super::super::b3(b"python-version='3.14'")
        }]);
        // Older artifacts without a setting census remain explicitly unavailable.
        for settings in [None, Some(serde_json::json!(["pyrefly.toml.unhandled"]))] {
            if let Some(settings) = settings {
                manifest["unapplied_checker_settings"] = settings;
            }
            assert!(matches!(
                SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap()),
                Err(PreparationError::Unavailable(ref reasons)) if reasons.contains(&PreparationRemainder::CheckerConfigurationAuthorityUnavailable)
            ));
        }
        manifest["unapplied_checker_settings"] = serde_json::json!([]);
        let selected =
            SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
        let root = super::super::tests::claim_001_temp_root("captured-configuration");
        std::fs::create_dir_all(&root).unwrap();
        let config = selected.config_for_root(&root).unwrap();
        assert_eq!(
            config.python_environment.python_version,
            Some(PythonVersion {
                major: 3,
                minor: 14,
                micro: 0
            })
        );
        assert_eq!(
            config.python_environment.python_platform,
            Some(PythonPlatform::new("linux"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selected_context_accepts_embedded_bundles_and_rejects_substitution() {
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&test_manifest("3.14", "darwin")).unwrap();
        assert!(
            SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap())
                .is_ok()
        );
        manifest["typeshed_bundle_digest"] = bundled_typeshed_digest().unwrap().clone().into();
        manifest["pyrefly_bundle_digest"] =
            format!("b3:{}", crate::PYREFLY_LOCK_SOURCE_BLAKE3).into();
        assert!(
            SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap())
                .is_ok()
        );
        manifest["typeshed_bundle_digest"] = super::super::b3(b"different-typeshed").into();
        manifest["pyrefly_bundle_digest"] = super::super::b3(b"different-pyrefly").into();
        assert!(
            matches!(SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap()),
            Err(PreparationError::Unavailable(ref reasons)) if reasons.contains(&PreparationRemainder::TypeshedBundleAuthorityUnavailable) && reasons.contains(&PreparationRemainder::PyreflyBundleAuthorityUnavailable))
        );
    }

    #[test]
    fn selected_context_strict_ingress_rejects_malformed_and_duplicate_fields() {
        let fixture = test_manifest("3.14", "linux");
        let text = String::from_utf8(fixture.clone()).unwrap();
        for invalid in [
            b"{}".to_vec(),
            [fixture.clone(), b"{}".to_vec()].concat(),
            text.replacen(
                "\"context_kind\":\"python\"",
                "\"context_kind\":\"python\",\"context_kind\":\"python\"",
                1,
            )
            .into_bytes(),
            text.replacen(
                "\"root_id\":\"workspace\"",
                "\"root_id\":\"workspace\",\"root_id\":\"workspace\"",
                1,
            )
            .into_bytes(),
            vec![b' '; MAX_MANIFEST_BYTES + 1],
        ] {
            assert!(matches!(
                SelectedPyreflyPreparation::from_manifest(&invalid),
                Err(PreparationError::InvalidManifest(_))
            ));
        }
        for version in ["3.14junk", "3.14.1", "03.14", "3.014"] {
            assert!(matches!(
                SelectedPyreflyPreparation::from_manifest(&test_manifest(version, "linux")),
                Err(PreparationError::InvalidManifest(_))
            ));
        }
    }

    #[test]
    fn selected_context_rejects_unbound_roots_escaping_paths_and_digest_drift() {
        let baseline: serde_json::Value =
            serde_json::from_slice(&test_manifest("3.13", "linux")).unwrap();
        let mut unknown = baseline.clone();
        unknown["unexpected_setting"] = true.into();
        let mut missing = baseline.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("typeshed_bundle_digest");
        let mut unbound = baseline.clone();
        unbound["module_roots"] = serde_json::json!(["absent"]);
        let mut escaping = baseline.clone();
        escaping["root_bindings"][0]["relative_path"] = serde_json::json!(b"../outside".as_slice());
        let mut uppercase = baseline.clone();
        uppercase["ruff_bundle_digest"] = format!("b3:{}", "A".repeat(64)).into();
        let mut duplicate = baseline;
        duplicate["platforms"] = serde_json::json!(["linux", "linux"]);
        for manifest in [unknown, missing, unbound, escaping, uppercase, duplicate] {
            assert!(matches!(
                SelectedPyreflyPreparation::from_manifest(&serde_json::to_vec(&manifest).unwrap()),
                Err(PreparationError::InvalidManifest(_))
            ));
        }
    }

    #[test]
    fn selected_config_projection_is_exact_without_ambient_discovery() {
        let root = super::super::tests::claim_001_temp_root("selected-config-projection");
        std::fs::create_dir_all(root.join("typings")).unwrap();
        for (version, platform) in [("3.13", "linux"), ("3.14", "darwin")] {
            let selected =
                SelectedPyreflyPreparation::parse(&test_manifest(version, platform)).unwrap();
            let config = selected.config_for_root(&root).unwrap();
            assert_eq!(
                config.python_environment.python_version,
                Some(selected.version)
            );
            assert_eq!(
                config.python_environment.python_platform,
                Some(PythonPlatform::new(platform))
            );
            assert_eq!(
                config.search_path().cloned().collect::<Vec<_>>(),
                [root.join(".")]
            );
            assert_eq!(
                config.python_environment.site_package_path,
                Some(Vec::new())
            );
            assert!(
                config
                    .python_environment
                    .interpreter_site_package_path
                    .is_empty()
            );
            assert!(config.python_environment.interpreter_stdlib_path.is_empty());
            assert!(config.interpreters.skip_interpreter_query);
            assert!(config.disable_search_path_heuristics);
            assert!(!config.enable_fallback_search_path);
            assert!(matches!(
                config.fallback_search_path,
                pyrefly_config::config::FallbackSearchPath::Empty
            ));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
