//! Single private seam around Pyrefly's explicitly unstable Rust API.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use arrow_ipc::writer::FileWriter;
use arrow_schema::{DataType, Field, Schema};
use pyrefly::library::library::library::library::default_config_finder;
use pyrefly::query::Query;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::thread_pool::ThreadCount;
use serde::Serialize;

pub(crate) const OBSERVATION_FAMILY_CODE: u32 = 110;
pub(crate) const SCHEMA_DESCRIPTOR: &str =
    include_str!("../../contracts/schema/provider-observations/pyrefly-module-v1.json");

#[derive(Serialize)]
struct LocatedCallee {
    range: serde_json::Value,
    kind: String,
    target: String,
    class_name: Option<String>,
}

pub(crate) struct ModuleAnalysis {
    pub arrow_ipc: Vec<u8>,
    pub row_count: u64,
    pub schema_digest: String,
    pub module_digest: String,
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
        serde_json::Value::String(text) => {
            if text.contains(source_path) {
                *text = text.replace(source_path, stable_locator);
            }
        }
        _ => {}
    }
}

fn observation_schema() -> Arc<Schema> {
    Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("module_id", DataType::Utf8, false),
            Field::new("module_name", DataType::Utf8, false),
            Field::new("type_table_json", DataType::Binary, false),
            Field::new("callees_json", DataType::Binary, false),
            Field::new("diagnostics_json", DataType::Binary, false),
        ],
        [("codefabric.schema".to_owned(), SCHEMA_DESCRIPTOR.to_owned())]
            .into_iter()
            .collect(),
    ))
}

pub(crate) fn schema_digest() -> String {
    b3(SCHEMA_DESCRIPTOR.as_bytes())
}

pub(crate) fn query_surface_smoke() -> usize {
    let query = Query::new(default_config_finder(None), ThreadCount::Inline);
    size_of_val(&query)
}

pub(crate) fn analyze_module(
    module_id: &str,
    module_name: &str,
    source_path: &Path,
) -> Result<ModuleAnalysis, String> {
    if !source_path.is_absolute() || !source_path.is_file() {
        return Err("Pyrefly source path must be an existing absolute file".to_owned());
    }
    let name = ModuleName::from_str(module_name);
    let path = ModulePath::filesystem(PathBuf::from(source_path));
    let query = Query::new(default_config_finder(None), ThreadCount::Inline);
    let diagnostics = query.add_files(vec![(name, path.clone())]);
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
    let mut diagnostics = serde_json::to_value(&diagnostics)
        .map_err(|error| format!("project Pyrefly diagnostics: {error}"))?;
    normalize_source_locator(&mut type_table, source_path, module_id);
    normalize_source_locator(&mut callees, source_path, module_id);
    normalize_source_locator(&mut diagnostics, source_path, module_id);
    let type_table_json = canonical_json(&type_table)?;
    let callees_json = canonical_json(&callees)?;
    let diagnostics_json = canonical_json(&diagnostics)?;
    let schema = observation_schema();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![module_id])),
        Arc::new(StringArray::from(vec![module_name])),
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
        module_id.as_bytes(),
        module_name.as_bytes(),
        type_table_json.as_slice(),
        callees_json.as_slice(),
        diagnostics_json.as_slice(),
    ]
    .concat()
    .as_slice());
    Ok(ModuleAnalysis {
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
