//! Single private seam around Pyrefly's explicitly unstable Rust API.
//!
//! The adapter converts the exact pinned `Query` results directly into application-owned,
//! relation-scoped Arrow streams. Provider-local indices are retained only as provenance.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt64Array};
use arrow_ipc::MetadataVersion;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use pyrefly::query::{IndexedTypeShapeKind, Query, TypeShapeTrait, TypeTableResponseData};
use pyrefly_config::config::{ConfigFile, ConfigSource};
use pyrefly_config::finder::ConfigFinder;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::events::CategorizedEvents;
use pyrefly_util::lined_buffer::PythonASTRange;
use pyrefly_util::thread_pool::ThreadCount;

#[path = "../../src/pyrefly_relation_schema.rs"]
mod relation_schema;

use relation_schema::{PYREFLY_RELEASE, PYREFLY_REVISION};
pub(crate) use relation_schema::{PyreflyRelation, schema_bundle_digest, schema_digests};

const MAX_RELATION_ROWS: usize = 1_000_000;
const MAX_RELATION_IPC_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct RelationAnalysis {
    pub relation: PyreflyRelation,
    pub arrow_ipc: Vec<u8>,
    pub row_count: u64,
    pub schema_digest: String,
}

pub(crate) struct ModuleAnalysis {
    pub module_id: String,
    pub relations: Vec<RelationAnalysis>,
    pub module_digest: String,
}

#[derive(Clone)]
pub(crate) struct ModuleInput {
    pub module_id: String,
    pub module_name: String,
    pub file_id: String,
    pub source_path: PathBuf,
    pub source_digest: String,
}

#[derive(Clone)]
pub(crate) struct AnalysisRunIdentity {
    pub provider_run_id: String,
    pub analysis_context_id: String,
    pub semantic_environment_digest: String,
    pub source_generation: u64,
}

pub(crate) struct ContextAnalysis {
    pub modules: Vec<ModuleAnalysis>,
    /// Query 1.2.0 does not expose the actual affected/rechecked set.
    pub proven_rechecked_module_ids: Vec<String>,
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

#[derive(Clone)]
struct CommonIdentity<'a> {
    run: &'a AnalysisRunIdentity,
    module: &'a ModuleInput,
    content_digest: [u8; 32],
    semantic_environment_id: [u8; 32],
}

struct TypeShapeRow {
    local_index: u64,
    structural_hash: u64,
    kind: &'static str,
    name: Option<String>,
    unspecified_type_arg_count: Option<u64>,
    is_staticmethod: Option<bool>,
}

struct TypeComponentRow {
    owner: u64,
    role: &'static str,
    ordinal: u64,
    referenced: u64,
}

struct TypeTraitRow {
    owner: u64,
    kind: &'static str,
}

struct LocatedTypeRow {
    ordinal: u64,
    start_byte: u64,
    end_byte: u64,
    local_type_index: u64,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

struct CallTargetRow {
    occurrence: u64,
    start_byte: u64,
    end_byte: u64,
    target_ordinal: u64,
    kind: String,
    target: String,
    class_name: Option<String>,
}

struct MemberRow {
    class_name: String,
    ordinal: u64,
    name: String,
    kind: Option<String>,
    annotation: String,
    is_final: bool,
}

struct CoverageRow {
    family: &'static str,
    surface: &'static str,
    requested: u64,
    completed: u64,
    emitted: u64,
    completeness: &'static str,
    remainder: Option<&'static str>,
    unknown: bool,
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

fn parse_digest(value: &str) -> Result<[u8; 32], String> {
    let encoded = value
        .strip_prefix("b3:")
        .ok_or_else(|| "digest lacks b3 prefix".to_owned())?;
    if encoded.len() != 64 {
        return Err("digest is not 32 bytes".to_owned());
    }
    let mut result = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0]).ok_or_else(|| "digest is not hexadecimal".to_owned())?;
        let low = hex_nibble(chunk[1]).ok_or_else(|| "digest is not hexadecimal".to_owned())?;
        result[index] = (high << 4) | low;
    }
    Ok(result)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn normalize_diagnostic(text: &str, source_path: &Path, module_id: &str) -> String {
    text.replace(
        source_path.to_string_lossy().as_ref(),
        &format!("codefabric-source://{module_id}"),
    )
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
        run: &AnalysisRunIdentity,
        modules: &[ModuleInput],
    ) -> Result<ContextAnalysis, String> {
        if modules.is_empty() {
            return Err("Pyrefly analysis requires at least one module".to_owned());
        }
        parse_digest(&run.semantic_environment_digest)?;
        if run.provider_run_id.is_empty() || run.analysis_context_id.is_empty() {
            return Err("Pyrefly run identity is incomplete".to_owned());
        }
        if modules.iter().any(|module| {
            module.file_id.is_empty()
                || !module.source_path.is_absolute()
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
                let source = std::fs::read(&module.source_path)
                    .map_err(|error| format!("read admitted Pyrefly source: {error}"))?;
                analyze_loaded_module(
                    &self.query,
                    run,
                    &diagnostics,
                    module,
                    &source,
                    &provider_path,
                    name,
                    path,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(ContextAnalysis {
            modules: analyses,
            // `change_files` and `add_files` do not return the actual affected set. Returning
            // requested modules here would falsely claim recheck evidence.
            proven_rechecked_module_ids: Vec::new(),
        })
    }
}

#[allow(clippy::too_many_lines)]
fn analyze_loaded_module(
    query: &Query,
    run: &AnalysisRunIdentity,
    diagnostics: &[String],
    module: &ModuleInput,
    source: &[u8],
    provider_path: &Path,
    name: ModuleName,
    path: ModulePath,
) -> Result<ModuleAnalysis, String> {
    let common = CommonIdentity {
        run,
        module,
        content_digest: parse_digest(&module.source_digest)?,
        semantic_environment_id: parse_digest(&run.semantic_environment_digest)?,
    };
    let type_table = query.get_type_table_in_file(name, path.clone(), None);
    let callees = query.get_callees_with_location(name, path.clone(), None);

    let (shape_rows, component_rows, trait_rows, located_rows) =
        project_type_table(type_table.as_ref(), source)?;
    let call_rows = project_callees(callees.as_deref(), source)?;
    let member_rows = project_members(query, name, path, type_table.as_ref());
    let source_path_text = provider_path.to_string_lossy();
    let module_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.contains(source_path_text.as_ref()))
        .map(|diagnostic| normalize_diagnostic(diagnostic, provider_path, &module.module_id))
        .collect::<Vec<_>>();
    let coverage_rows = coverage_rows(
        type_table.is_some(),
        callees.is_some(),
        shape_rows.len(),
        located_rows.len(),
        call_rows.len(),
        member_rows.len(),
        module_diagnostics.len(),
    );

    let mut relations = vec![
        encode_relation(
            PyreflyRelation::ModuleContext,
            module_context_batch(&common, source.len())?,
        )?,
        encode_relation(
            PyreflyRelation::TypeShape,
            type_shape_batch(&common, &shape_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::TypeComponent,
            type_component_batch(&common, &component_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::TypeTrait,
            type_trait_batch(&common, &trait_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::LocatedType,
            located_type_batch(&common, &located_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::CallTarget,
            call_target_batch(&common, &call_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::Member,
            member_batch(&common, &member_rows)?,
        )?,
        encode_relation(
            PyreflyRelation::Diagnostic,
            diagnostic_batch(&common, &module_diagnostics)?,
        )?,
        encode_relation(
            PyreflyRelation::AffectedModule,
            affected_module_batch(&common)?,
        )?,
        encode_relation(
            PyreflyRelation::Coverage,
            coverage_batch(&common, &coverage_rows)?,
        )?,
    ];
    relations.sort_by_key(|relation| relation.relation);
    let module_digest = module_digest(module, &relations);
    Ok(ModuleAnalysis {
        module_id: module.module_id.clone(),
        relations,
        module_digest,
    })
}

fn project_type_table(
    response: Option<&TypeTableResponseData>,
    source: &[u8],
) -> Result<
    (
        Vec<TypeShapeRow>,
        Vec<TypeComponentRow>,
        Vec<TypeTraitRow>,
        Vec<LocatedTypeRow>,
    ),
    String,
> {
    let Some(response) = response else {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    };
    let mut shapes = Vec::with_capacity(response.type_table.len());
    let mut components = Vec::new();
    let mut traits = Vec::new();
    for (index, entry) in response.type_table.iter().enumerate() {
        let owner = u64::try_from(index).map_err(|_| "Pyrefly type index exceeds u64")?;
        let (kind, name, unspecified, is_staticmethod) = match &entry.kind {
            IndexedTypeShapeKind::Named {
                name,
                args,
                unspecified_type_arg_count,
                traits: shape_traits,
            } => {
                push_components(&mut components, owner, "argument", args)?;
                for shape_trait in shape_traits {
                    traits.push(TypeTraitRow {
                        owner,
                        kind: match shape_trait {
                            TypeShapeTrait::TypedDict => "typed_dict",
                            TypeShapeTrait::PartialTypedDict => "partial_typed_dict",
                            TypeShapeTrait::Tuple => "tuple",
                        },
                    });
                }
                (
                    "named",
                    Some(name.clone()),
                    unspecified_type_arg_count
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| "Pyrefly unspecified type argument count exceeds u64")?,
                    None,
                )
            }
            IndexedTypeShapeKind::Callable {
                params,
                return_type,
                is_staticmethod,
            } => {
                push_components(&mut components, owner, "parameter", params)?;
                components.push(TypeComponentRow {
                    owner,
                    role: "return",
                    ordinal: 0,
                    referenced: u64::try_from(*return_type)
                        .map_err(|_| "Pyrefly return type index exceeds u64")?,
                });
                ("callable", None, None, Some(*is_staticmethod))
            }
            IndexedTypeShapeKind::TypeVariable { name, bounds } => {
                push_components(&mut components, owner, "bound", bounds)?;
                ("type_variable", Some(name.clone()), None, None)
            }
        };
        shapes.push(TypeShapeRow {
            local_index: owner,
            structural_hash: entry.hash,
            kind,
            name,
            unspecified_type_arg_count: unspecified,
            is_staticmethod,
        });
    }
    let mut located = Vec::with_capacity(response.types.len());
    for (ordinal, occurrence) in response.types.iter().enumerate() {
        if occurrence.type_index >= response.type_table.len() {
            return Err(
                "Pyrefly located type references an absent response-local index".to_owned(),
            );
        }
        let (start_byte, end_byte) = byte_range(source, &occurrence.location)?;
        located.push(LocatedTypeRow {
            ordinal: u64::try_from(ordinal).map_err(|_| "located type ordinal exceeds u64")?,
            start_byte,
            end_byte,
            local_type_index: u64::try_from(occurrence.type_index)
                .map_err(|_| "located type index exceeds u64")?,
            start_line: u64::from(occurrence.location.start_line.get()),
            start_column: u64::from(occurrence.location.start_col),
            end_line: u64::from(occurrence.location.end_line.get()),
            end_column: u64::from(occurrence.location.end_col),
        });
    }
    Ok((shapes, components, traits, located))
}

fn push_components(
    rows: &mut Vec<TypeComponentRow>,
    owner: u64,
    role: &'static str,
    indices: &[usize],
) -> Result<(), String> {
    for (ordinal, index) in indices.iter().enumerate() {
        rows.push(TypeComponentRow {
            owner,
            role,
            ordinal: u64::try_from(ordinal).map_err(|_| "type component ordinal exceeds u64")?,
            referenced: u64::try_from(*index).map_err(|_| "type component index exceeds u64")?,
        });
    }
    Ok(())
}

fn project_callees(
    callees: Option<&[(PythonASTRange, pyrefly::query::Callee)]>,
    source: &[u8],
) -> Result<Vec<CallTargetRow>, String> {
    let Some(callees) = callees else {
        return Ok(Vec::new());
    };
    let mut occurrence_ordinals = BTreeMap::<(u64, u64), u64>::new();
    let mut target_ordinals = BTreeMap::<(u64, u64), u64>::new();
    let mut rows = Vec::with_capacity(callees.len());
    for (range, callee) in callees {
        let key = byte_range(source, range)?;
        let next_occurrence = u64::try_from(occurrence_ordinals.len())
            .map_err(|_| "callee occurrence count exceeds u64")?;
        let occurrence = *occurrence_ordinals.entry(key).or_insert(next_occurrence);
        let ordinal = target_ordinals.entry(key).or_insert(0);
        rows.push(CallTargetRow {
            occurrence,
            start_byte: key.0,
            end_byte: key.1,
            target_ordinal: *ordinal,
            kind: callee.kind.clone(),
            target: callee.target.clone(),
            class_name: callee.class_name.clone(),
        });
        *ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| "callee target ordinal exceeds u64".to_owned())?;
    }
    Ok(rows)
}

fn project_members(
    query: &Query,
    name: ModuleName,
    path: ModulePath,
    response: Option<&TypeTableResponseData>,
) -> Vec<MemberRow> {
    let candidates = response
        .into_iter()
        .flat_map(|response| &response.type_table)
        .filter_map(|entry| match &entry.kind {
            IndexedTypeShapeKind::Named { name, .. } => name
                .rsplit(|character| character == '.' || character == ':')
                .next()
                .filter(|candidate| {
                    !candidate.is_empty()
                        && candidate
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                })
                .map(str::to_owned),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for candidate in candidates {
        let Some(attributes) = query.get_attributes(name, path.clone(), &candidate) else {
            continue;
        };
        for (ordinal, attribute) in attributes.into_iter().enumerate() {
            rows.push(MemberRow {
                class_name: candidate.clone(),
                ordinal: u64::try_from(ordinal).unwrap_or(u64::MAX),
                name: attribute.name,
                kind: attribute.kind,
                annotation: attribute.annotation,
                is_final: attribute.is_final,
            });
        }
    }
    rows
}

fn byte_range(source: &[u8], range: &PythonASTRange) -> Result<(u64, u64), String> {
    let text = std::str::from_utf8(source)
        .map_err(|_| "Pyrefly semantic source is not valid UTF-8".to_owned())?;
    let mut line_starts = vec![0_usize];
    line_starts.extend(
        source
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'\n').then_some(index + 1)),
    );
    let position = |line: u32, column: u32| -> Result<usize, String> {
        let line_index = usize::try_from(line)
            .map_err(|_| "Pyrefly line exceeds usize".to_owned())?
            .checked_sub(1)
            .ok_or_else(|| "Pyrefly line numbers must be one-indexed".to_owned())?;
        let line_start = *line_starts
            .get(line_index)
            .ok_or_else(|| "Pyrefly range line exceeds source".to_owned())?;
        let absolute = line_start
            .checked_add(
                usize::try_from(column).map_err(|_| "Pyrefly column exceeds usize".to_owned())?,
            )
            .ok_or_else(|| "Pyrefly range overflows source coordinates".to_owned())?;
        let line_end = line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(source.len());
        if absolute > line_end || absolute > source.len() || !text.is_char_boundary(absolute) {
            return Err("Pyrefly range is out of bounds or splits UTF-8".to_owned());
        }
        Ok(absolute)
    };
    let start = position(range.start_line.get(), range.start_col)?;
    let end = position(range.end_line.get(), range.end_col)?;
    if start > end {
        return Err("Pyrefly range is reversed".to_owned());
    }
    Ok((
        u64::try_from(start).map_err(|_| "start byte exceeds u64")?,
        u64::try_from(end).map_err(|_| "end byte exceeds u64")?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn coverage_rows(
    types_available: bool,
    callees_available: bool,
    type_shapes: usize,
    located_types: usize,
    calls: usize,
    members: usize,
    diagnostics: usize,
) -> Vec<CoverageRow> {
    vec![
        CoverageRow {
            family: "computed_types",
            surface: "Query::get_type_table_in_file",
            requested: 1,
            completed: u64::from(types_available),
            emitted: as_u64(type_shapes.saturating_add(located_types)),
            completeness: if types_available {
                "complete"
            } else {
                "unknown"
            },
            remainder: (!types_available).then_some("QUERY_RETURNED_NONE"),
            unknown: !types_available,
        },
        CoverageRow {
            family: "call_targets",
            surface: "Query::get_callees_with_location",
            requested: 1,
            completed: u64::from(callees_available),
            emitted: as_u64(calls),
            completeness: if callees_available {
                "partial"
            } else {
                "unknown"
            },
            remainder: Some(if callees_available {
                "NO_STRUCTURAL_CALL_SITE_CENSUS"
            } else {
                "QUERY_RETURNED_NONE"
            }),
            unknown: true,
        },
        CoverageRow {
            family: "members",
            surface: "Query::get_attributes",
            requested: 1,
            completed: u64::from(types_available),
            emitted: as_u64(members),
            completeness: if types_available {
                "partial"
            } else {
                "unknown"
            },
            remainder: Some(if types_available {
                "QUERY_REQUIRES_CLASS_NAME_NO_CLASS_CENSUS"
            } else {
                "TYPE_TABLE_UNAVAILABLE_FOR_MEMBER_CANDIDATES"
            }),
            unknown: true,
        },
        CoverageRow {
            family: "diagnostics",
            surface: "Query::add_files rendered diagnostics",
            requested: 1,
            completed: 1,
            emitted: as_u64(diagnostics),
            completeness: "partial",
            remainder: Some("STRUCTURED_DIAGNOSTIC_API_UNAVAILABLE"),
            unknown: true,
        },
        unsupported_coverage("declared_types", "pinned TSP seam", "NOT_IN_QUERY_SLICE"),
        unsupported_coverage("expected_types", "pinned TSP seam", "NOT_IN_QUERY_SLICE"),
        unsupported_coverage(
            "import_resolution",
            "pinned TSP/module-resolver seam",
            "NOT_IN_QUERY_SLICE",
        ),
        unsupported_coverage(
            "definitions_xrefs",
            "selected pinned Glean/internal seam",
            "NOT_IN_QUERY_SLICE",
        ),
        unsupported_coverage(
            "navigation_fallback",
            "accepted LSP seam",
            "NOT_IN_QUERY_SLICE",
        ),
        CoverageRow {
            family: "affected_modules",
            surface: "Query::change_files/add_files",
            requested: 1,
            completed: 1,
            emitted: 1,
            completeness: "unknown",
            remainder: Some("PINNED_QUERY_EXPOSES_NO_ACTUAL_AFFECTED_SET"),
            unknown: true,
        },
    ]
}

const fn unsupported_coverage(
    family: &'static str,
    surface: &'static str,
    reason: &'static str,
) -> CoverageRow {
    CoverageRow {
        family,
        surface,
        requested: 1,
        completed: 0,
        emitted: 0,
        completeness: "partial",
        remainder: Some(reason),
        unknown: true,
    }
}

fn common_columns(common: &CommonIdentity<'_>, rows: usize) -> Result<Vec<ArrayRef>, String> {
    if rows > MAX_RELATION_ROWS {
        return Err(format!(
            "Pyrefly relation row limit exceeded: {rows} > {MAX_RELATION_ROWS}"
        ));
    }
    let strings = |value: &str| -> ArrayRef {
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            value, rows,
        )))
    };
    let fixed_digest = |value: &[u8; 32], label: &str| {
        let mut builder = FixedSizeBinaryBuilder::with_capacity(rows, 32);
        for _ in 0..rows {
            builder
                .append_value(value)
                .map_err(|error| format!("construct {label} array: {error}"))?;
        }
        Ok::<ArrayRef, String>(Arc::new(builder.finish()))
    };
    let content = fixed_digest(&common.content_digest, "content-digest")?;
    let environment = fixed_digest(&common.semantic_environment_id, "semantic-environment")?;
    Ok(vec![
        strings(&common.run.provider_run_id),
        strings(&common.run.analysis_context_id),
        strings(&common.module.module_id),
        strings(&common.module.file_id),
        content,
        environment,
        Arc::new(UInt64Array::from_value(common.run.source_generation, rows)),
    ])
}

fn record_batch(
    common: &CommonIdentity<'_>,
    relation: PyreflyRelation,
    rows: usize,
    mut specific: Vec<ArrayRef>,
) -> Result<RecordBatch, String> {
    let mut columns = common_columns(common, rows)?;
    columns.append(&mut specific);
    RecordBatch::try_new(relation.schema(), columns)
        .map_err(|error| format!("construct {} batch: {error}", relation.relation_id()))
}

fn module_context_batch(
    common: &CommonIdentity<'_>,
    source_len: usize,
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::ModuleContext,
        1,
        vec![
            Arc::new(StringArray::from(vec![common.module.module_name.as_str()])),
            Arc::new(StringArray::from(vec![PYREFLY_RELEASE])),
            Arc::new(StringArray::from(vec![PYREFLY_REVISION])),
            Arc::new(StringArray::from(vec!["everything"])),
            Arc::new(StringArray::from(vec!["exports"])),
            Arc::new(UInt64Array::from(vec![as_u64(source_len)])),
            Arc::new(BooleanArray::from(vec![true])),
        ],
    )
}

fn type_shape_batch(
    common: &CommonIdentity<'_>,
    rows: &[TypeShapeRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::TypeShape,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.local_index),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.structural_hash),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.kind),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|row| row.name.as_deref()),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter().map(|row| row.unspecified_type_arg_count),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| row.is_staticmethod),
            )),
        ],
    )
}

fn type_component_batch(
    common: &CommonIdentity<'_>,
    rows: &[TypeComponentRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::TypeComponent,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.owner),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.role),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.referenced),
            )),
        ],
    )
}

fn type_trait_batch(
    common: &CommonIdentity<'_>,
    rows: &[TypeTraitRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::TypeTrait,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.owner),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.kind),
            )),
        ],
    )
}

fn located_type_batch(
    common: &CommonIdentity<'_>,
    rows: &[LocatedTypeRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::LocatedType,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.local_type_index),
            )),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                "computed",
                rows.len(),
            ))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_line),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_column),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_line),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_column),
            )),
        ],
    )
}

fn call_target_batch(
    common: &CommonIdentity<'_>,
    rows: &[CallTargetRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::CallTarget,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.occurrence),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end_byte),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.target_ordinal),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.kind.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.target.as_str()),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|row| row.class_name.as_deref()),
            )),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                "resolved",
                rows.len(),
            ))),
        ],
    )
}

fn member_batch(common: &CommonIdentity<'_>, rows: &[MemberRow]) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::Member,
        rows.len(),
        vec![
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.class_name.as_str()),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.name.as_str()),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|row| row.kind.as_deref()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.annotation.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                "provider-display",
                rows.len(),
            ))),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| Some(row.is_final)),
            )),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                "query-type-table-named-candidate",
                rows.len(),
            ))),
        ],
    )
}

fn diagnostic_batch(common: &CommonIdentity<'_>, rows: &[String]) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::Diagnostic,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values((0..rows.len()).map(as_u64))),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(String::as_str),
            )),
            Arc::new(BooleanArray::from(vec![false; rows.len()])),
            Arc::new(BooleanArray::from(vec![true; rows.len()])),
        ],
    )
}

fn affected_module_batch(common: &CommonIdentity<'_>) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::AffectedModule,
        1,
        vec![
            Arc::new(StringArray::from(vec![common.module.module_id.as_str()])),
            Arc::new(StringArray::from(vec![
                "requested-module-analyzed-no-affected-set",
            ])),
            Arc::new(BooleanArray::from(vec![false])),
            Arc::new(StringArray::from(vec![
                "conservative-reverse-importer-refresh-required",
            ])),
        ],
    )
}

fn coverage_batch(
    common: &CommonIdentity<'_>,
    rows: &[CoverageRow],
) -> Result<RecordBatch, String> {
    record_batch(
        common,
        PyreflyRelation::Coverage,
        rows.len(),
        vec![
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.family),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.surface),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.requested),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.completed),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.emitted),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.completeness),
            )),
            Arc::new(StringArray::from_iter(rows.iter().map(|row| row.remainder))),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| Some(row.unknown)),
            )),
        ],
    )
}

fn encode_relation(
    relation: PyreflyRelation,
    batch: RecordBatch,
) -> Result<RelationAnalysis, String> {
    let row_count = u64::try_from(batch.num_rows()).map_err(|_| "relation rows exceed u64")?;
    let schema = relation.schema();
    if batch.schema().as_ref() != schema.as_ref() {
        return Err(format!(
            "{} batch differs from its application-owned schema",
            relation.relation_id()
        ));
    }
    let mut arrow_ipc = Vec::new();
    {
        let options = IpcWriteOptions::try_new(64, false, MetadataVersion::V5)
            .map_err(|error| format!("configure Arrow IPC V5 writer: {error}"))?;
        let mut writer =
            StreamWriter::try_new_with_options(&mut arrow_ipc, schema.as_ref(), options).map_err(
                |error| format!("open {} Arrow stream: {error}", relation.relation_id()),
            )?;
        writer
            .write(&batch)
            .map_err(|error| format!("write {} Arrow stream: {error}", relation.relation_id()))?;
        writer
            .finish()
            .map_err(|error| format!("finish {} Arrow stream: {error}", relation.relation_id()))?;
    }
    if arrow_ipc.len() > MAX_RELATION_IPC_BYTES {
        return Err(format!(
            "{} stream exceeds bounded IPC size: {} > {}",
            relation.relation_id(),
            arrow_ipc.len(),
            MAX_RELATION_IPC_BYTES
        ));
    }
    Ok(RelationAnalysis {
        relation,
        arrow_ipc,
        row_count,
        schema_digest: relation.schema_digest(),
    })
}

fn module_digest(module: &ModuleInput, relations: &[RelationAnalysis]) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(module.module_id.as_bytes());
    bytes.extend_from_slice(module.module_name.as_bytes());
    bytes.extend_from_slice(module.file_id.as_bytes());
    bytes.extend_from_slice(module.source_digest.as_bytes());
    for relation in relations {
        bytes.extend_from_slice(&relation.relation.family_code().to_be_bytes());
        bytes.extend_from_slice(relation.schema_digest.as_bytes());
        bytes.extend_from_slice(b3(&relation.arrow_ipc).as_bytes());
        bytes.extend_from_slice(&relation.row_count.to_be_bytes());
    }
    b3(&bytes)
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_ipc::reader::StreamReader;
    use std::io::Cursor;

    #[test]
    fn operational_source_paths_do_not_escape_diagnostics() {
        let path = Path::new("/private/tmp/provider-run-42/pkg/module.py");
        let normalized = normalize_diagnostic(
            &format!("parse failure at {}:4", path.display()),
            path,
            "module:pkg.module",
        );
        assert!(!normalized.contains("/private/tmp/provider-run-42"));
        assert!(normalized.contains("codefabric-source://module:pkg.module"));
    }

    #[test]
    fn query_slice_emits_typed_relation_streams_and_explicit_remainders() {
        let root = std::env::temp_dir().join(format!(
            "codefabric-pyrefly-relations-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("input.py");
        let source = b"class C:\n    value: int = 1\n\ndef use(c: C) -> int:\n    return c.value\n";
        std::fs::write(&source_path, source).unwrap();
        let run = AnalysisRunIdentity {
            provider_run_id: "0123456789abcdef0123456789abcdef".to_owned(),
            analysis_context_id: "context".to_owned(),
            semantic_environment_digest: b3(b"environment"),
            source_generation: 7,
        };
        let module = ModuleInput {
            module_id: "module:C".to_owned(),
            module_name: "fixture".to_owned(),
            file_id: "file:C".to_owned(),
            source_path,
            source_digest: b3(source),
        };
        let mut context = SemanticContext::new(&root, "fixture-context").unwrap();
        let result = context.analyze_modules(&run, &[module]).unwrap();
        assert!(result.proven_rechecked_module_ids.is_empty());
        let module = &result.modules[0];
        assert_eq!(module.relations.len(), PyreflyRelation::ALL.len());
        for relation in &module.relations {
            let mut reader = StreamReader::try_new(Cursor::new(&relation.arrow_ipc), None).unwrap();
            assert_eq!(
                reader.schema().as_ref(),
                relation.relation.schema().as_ref()
            );
            let batch = reader.next().unwrap().unwrap();
            assert_eq!(
                batch.num_rows(),
                usize::try_from(relation.row_count).unwrap()
            );
            assert!(reader.next().is_none());
        }
        let coverage = module
            .relations
            .iter()
            .find(|relation| relation.relation == PyreflyRelation::Coverage)
            .unwrap();
        let mut reader = StreamReader::try_new(Cursor::new(&coverage.arrow_ipc), None).unwrap();
        let batch = reader.next().unwrap().unwrap();
        let remainders = batch
            .column_by_name("remainder_reason")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert!(
            remainders
                .iter()
                .flatten()
                .any(|reason| { reason == "PINNED_QUERY_EXPOSES_NO_ACTUAL_AFFECTED_SET" })
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
