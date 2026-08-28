//! Single private seam around Pyrefly's explicitly unstable Rust API.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use arrow_ipc::writer::FileWriter;
use arrow_schema::{DataType, Field, Schema};
use pyrefly::query::Query;
use pyrefly_config::config::{ConfigFile, ConfigSource};
use pyrefly_config::finder::ConfigFinder;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::events::CategorizedEvents;
use pyrefly_util::thread_pool::ThreadCount;
use serde::Serialize;

#[allow(dead_code)]
mod generated_observation_schema {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../src/generated/model_schema_tables.rs"
    ));
}

use generated_observation_schema::{
    PROVIDER_OBSERVATION_SCHEMAS, ProviderObservationLogicalType, ProviderObservationSchema,
};

#[derive(Serialize)]
struct LocatedCallee {
    range: serde_json::Value,
    kind: String,
    target: String,
    class_name: Option<String>,
}

pub(crate) struct ModuleAnalysis {
    pub module_id: String,
    pub arrow_ipc: Vec<u8>,
    pub row_count: u64,
    pub schema_digest: String,
    pub module_digest: String,
}

#[derive(Clone)]
pub(crate) struct ModuleInput {
    pub module_id: String,
    pub module_name: String,
    pub source_path: PathBuf,
    pub source_digest: String,
}

pub(crate) struct ContextAnalysis {
    pub modules: Vec<ModuleAnalysis>,
    pub rechecked_module_ids: Vec<String>,
}

struct ProviderView {
    root: PathBuf,
}

struct LoadedModule {
    module_id: String,
    source_digest: String,
}

/// One long-lived Pyrefly state per negotiated analysis context.
pub(crate) struct SemanticContext {
    view: ProviderView,
    query: Query,
    loaded: BTreeMap<String, LoadedModule>,
}

impl Drop for ProviderView {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn provider_module_path(root: &Path, module_name: &str) -> Result<PathBuf, String> {
    let components = module_name.split('.').collect::<Vec<_>>();
    if components.is_empty()
        || components.iter().any(|component| {
            component.is_empty()
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    {
        return Err(format!("invalid Pyrefly module name {module_name}"));
    }
    let mut path = root.to_owned();
    for component in &components[..components.len() - 1] {
        path.push(component);
    }
    path.push(components[components.len() - 1]);
    path.set_extension("py");
    Ok(path)
}

fn write_provider_source(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "Pyrefly provider target has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Pyrefly module directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".codefabric-pyrefly-{}-{}.tmp",
        std::process::id(),
        target
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("module")
    ));
    std::fs::write(&temporary, bytes).map_err(|error| format!("stage Pyrefly module: {error}"))?;
    std::fs::rename(&temporary, target).map_err(|error| format!("publish Pyrefly module: {error}"))
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| format!("serialize Pyrefly DTO: {error}"))
}

fn normalize_source_locator(value: &mut serde_json::Value, source_path: &Path, module_id: &str) {
    let source_path = source_path.to_string_lossy();
    let stable_locator = format!("codefabric-source://{module_id}");
    normalize_source_locator_text(value, source_path.as_ref(), &stable_locator);
}

fn normalize_source_locator_text(
    value: &mut serde_json::Value,
    source_path: &str,
    stable_locator: &str,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_source_locator_text(value, source_path, stable_locator);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                normalize_source_locator_text(value, source_path, stable_locator);
            }
        }
        serde_json::Value::String(text) if text.contains(source_path) => {
            *text = text.replace(source_path, stable_locator);
        }
        _ => {}
    }
}

fn observation_contract() -> &'static ProviderObservationSchema {
    PROVIDER_OBSERVATION_SCHEMAS
        .iter()
        .find(|schema| schema.provider_id == "pyrefly-python")
        .expect("the model compiler requires the Pyrefly observation schema")
}

pub(crate) fn observation_family_code() -> u32 {
    u32::from(observation_contract().observation_family_code)
}

fn observation_schema() -> Arc<Schema> {
    let contract = observation_contract();
    Arc::new(Schema::new_with_metadata(
        contract
            .fields
            .iter()
            .map(|field| {
                let data_type = match field.logical_type {
                    ProviderObservationLogicalType::Utf8 => DataType::Utf8,
                    ProviderObservationLogicalType::Binary => DataType::Binary,
                    ProviderObservationLogicalType::Boolean => DataType::Boolean,
                    ProviderObservationLogicalType::UInt64 => DataType::UInt64,
                    ProviderObservationLogicalType::Utf8List => {
                        DataType::List(Arc::new(Field::new_list_field(DataType::Utf8, false)))
                    }
                };
                Field::new(field.name, data_type, field.nullable)
            })
            .collect::<Vec<_>>(),
        [(
            "codefabric.schema".to_owned(),
            contract.canonical_descriptor.to_owned(),
        )]
        .into_iter()
        .collect(),
    ))
}

pub(crate) fn schema_digest() -> String {
    observation_contract().schema_digest.to_owned()
}

pub(crate) fn query_surface_smoke() -> usize {
    let mut config = ConfigFile::default();
    config.python_environment.set_empty_to_default();
    config.configure();
    let query = Query::new(
        ConfigFinder::new_constant(ArcId::new(config)),
        ThreadCount::Inline,
    );
    size_of_val(&query)
}

impl SemanticContext {
    pub(crate) fn new(state_root: &Path, context_key: &str) -> Result<Self, String> {
        if !state_root.is_absolute() || context_key.is_empty() {
            return Err("Pyrefly context state root or key is invalid".to_owned());
        }
        std::fs::create_dir_all(state_root)
            .map_err(|error| format!("create Pyrefly context root: {error}"))?;
        let root = state_root.join(format!("context-{}", &b3(context_key.as_bytes())[3..35]));
        if root.exists() {
            std::fs::remove_dir_all(&root)
                .map_err(|error| format!("replace stale Pyrefly context: {error}"))?;
        }
        std::fs::create_dir(&root)
            .map_err(|error| format!("create Pyrefly provider view: {error}"))?;
        let mut config = ConfigFile {
            source: ConfigSource::File(root.join(ConfigFile::PYREFLY_FILE_NAME)),
            enable_fallback_search_path: true,
            ..ConfigFile::default()
        };
        config.python_environment.set_empty_to_default();
        config.interpreters.skip_interpreter_query = true;
        config.configure();
        let query = Query::new(
            ConfigFinder::new_constant(ArcId::new(config)),
            ThreadCount::Inline,
        );
        Ok(Self {
            view: ProviderView { root },
            query,
            loaded: BTreeMap::new(),
        })
    }

    pub(crate) fn analyze_modules(
        &mut self,
        modules: &[ModuleInput],
    ) -> Result<ContextAnalysis, String> {
        if modules.is_empty() {
            return Err("Pyrefly analysis requires at least one module".to_owned());
        }
        if modules.iter().any(|module| {
            !module.source_path.is_absolute()
                || !module.source_path.is_file()
                || b3(&std::fs::read(&module.source_path).unwrap_or_default())
                    != module.source_digest
        }) {
            return Err(
                "Pyrefly source paths and digests must identify existing immutable files"
                    .to_owned(),
            );
        }

        let mut created = Vec::new();
        let mut modified = Vec::new();
        let mut resolved = Vec::with_capacity(modules.len());
        let mut provider_paths = Vec::with_capacity(modules.len());
        for module in modules {
            let target = provider_module_path(&self.view.root, &module.module_name)?;
            let bytes = std::fs::read(&module.source_path)
                .map_err(|error| format!("read admitted Pyrefly source: {error}"))?;
            match self.loaded.get(&module.module_name) {
                Some(loaded) if loaded.module_id != module.module_id => {
                    return Err(format!(
                        "Pyrefly module identity changed for {}",
                        module.module_name
                    ));
                }
                Some(loaded) if loaded.source_digest == module.source_digest => {}
                Some(_) => {
                    write_provider_source(&target, &bytes)?;
                    modified.push(target.clone());
                }
                None => {
                    write_provider_source(&target, &bytes)?;
                    created.push(target.clone());
                }
            }
            self.loaded.insert(
                module.module_name.clone(),
                LoadedModule {
                    module_id: module.module_id.clone(),
                    source_digest: module.source_digest.clone(),
                },
            );
            resolved.push((
                ModuleName::from_str(&module.module_name),
                ModulePath::filesystem(target.clone()),
            ));
            provider_paths.push(target);
        }

        let events = CategorizedEvents {
            created,
            modified,
            ..CategorizedEvents::default()
        };
        if !events.is_empty() {
            self.query.change_files(&events);
        }
        let diagnostics = self.query.add_files(resolved.clone());
        let analyses = modules
            .iter()
            .zip(resolved)
            .zip(provider_paths)
            .map(|((module, (name, path)), provider_path)| {
                analyze_loaded_module(
                    &self.query,
                    &diagnostics,
                    module,
                    &provider_path,
                    name,
                    path,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        let rechecked_module_ids = modules
            .iter()
            .map(|module| module.module_id.clone())
            .collect();
        Ok(ContextAnalysis {
            modules: analyses,
            rechecked_module_ids,
        })
    }
}

fn analyze_loaded_module(
    query: &Query,
    diagnostics: &[String],
    module: &ModuleInput,
    provider_path: &Path,
    name: ModuleName,
    path: ModulePath,
) -> Result<ModuleAnalysis, String> {
    let type_table = query
        .get_type_table_in_file(name, path.clone(), None)
        .ok_or_else(|| "Pyrefly did not return a type table".to_owned())?;
    let callees = query
        .get_callees_with_location(name, path, None)
        .unwrap_or_default()
        .into_iter()
        .map(|(range, callee)| {
            Ok(LocatedCallee {
                range: serde_json::to_value(range)
                    .map_err(|error| format!("serialize Pyrefly range: {error}"))?,
                kind: callee.kind,
                target: callee.target,
                class_name: callee.class_name,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut type_table = serde_json::to_value(&type_table)
        .map_err(|error| format!("project Pyrefly type table: {error}"))?;
    let mut callees = serde_json::to_value(&callees)
        .map_err(|error| format!("project Pyrefly callees: {error}"))?;
    let source_path_text = provider_path.to_string_lossy();
    let module_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.contains(source_path_text.as_ref()))
        .cloned()
        .collect::<Vec<_>>();
    let mut diagnostics = serde_json::to_value(&module_diagnostics)
        .map_err(|error| format!("project Pyrefly diagnostics: {error}"))?;
    normalize_source_locator(&mut type_table, provider_path, &module.module_id);
    normalize_source_locator(&mut callees, provider_path, &module.module_id);
    normalize_source_locator(&mut diagnostics, provider_path, &module.module_id);
    let type_table_json = canonical_json(&type_table)?;
    let callees_json = canonical_json(&callees)?;
    let diagnostics_json = canonical_json(&diagnostics)?;
    let schema = observation_schema();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![module.module_id.as_str()])),
        Arc::new(StringArray::from(vec![module.module_name.as_str()])),
        Arc::new(BinaryArray::from(vec![type_table_json.as_slice()])),
        Arc::new(BinaryArray::from(vec![callees_json.as_slice()])),
        Arc::new(BinaryArray::from(vec![diagnostics_json.as_slice()])),
    ];
    let batch = RecordBatch::try_new(Arc::clone(&schema), columns)
        .map_err(|error| format!("construct Pyrefly Arrow batch: {error}"))?;
    let mut arrow_ipc = Vec::new();
    {
        let mut writer = FileWriter::try_new(&mut arrow_ipc, &schema)
            .map_err(|error| format!("open Pyrefly Arrow IPC: {error}"))?;
        writer
            .write(&batch)
            .map_err(|error| format!("write Pyrefly Arrow IPC: {error}"))?;
        writer
            .finish()
            .map_err(|error| format!("finish Pyrefly Arrow IPC: {error}"))?;
    }
    let module_digest = b3([
        module.module_id.as_bytes(),
        module.module_name.as_bytes(),
        type_table_json.as_slice(),
        callees_json.as_slice(),
        diagnostics_json.as_slice(),
    ]
    .concat()
    .as_slice());
    Ok(ModuleAnalysis {
        module_id: module.module_id.clone(),
        arrow_ipc,
        row_count: 1,
        schema_digest: schema_digest(),
        module_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_source_paths_do_not_escape_the_application_dto() {
        let path = Path::new("/private/tmp/provider-run-42/pkg/module.py");
        let mut value = serde_json::json!({
            "path": path,
            "diagnostic": format!("parse failure at {}:4", path.display()),
            "nested": [path],
        });

        normalize_source_locator(&mut value, path, "module:pkg.module");

        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("/private/tmp/provider-run-42"));
        assert_eq!(
            value["path"],
            serde_json::Value::String("codefabric-source://module:pkg.module".to_owned())
        );
    }
}
