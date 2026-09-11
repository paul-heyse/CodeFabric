//! Complete captured inputs and effective contexts for fresh production preparation.

use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis_context::{
    ContextLookupObservation, ContextSearchRoot, ContextSearchScope, ContextSearchUniverse,
};
use crate::cancellation::Cancellation;
use crate::identity::{IdentityDomain, decode_public_id, encode_public_id};
use crate::inventory::reserve_memory;
use crate::inventory::{InventoryLimits, InventoryWalker};
use crate::operational_store::OperationalStore;
use crate::provider_contracts::{
    ContextIdentity, ProviderContextBinding, ProviderInputDisposition, ProviderInventoryMember,
    ProviderLookupKey, ProviderLookupOutcome, ProviderModuleBinding, ProviderSearchScope,
    ProviderSourceInventory, ProviderSupportDependency,
};
use crate::python_context::{
    PythonAuthorizedRoot, PythonContextDiscoveryProduct, PythonContextDiscoveryRequest,
    PythonDeploymentProfile, PythonDiscoveryFile, PythonRegisteredInputs, discover_python_context,
};
use crate::resource_budget::{
    ChargedValue, ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceScopeKind,
};
use crate::secure_path::open_workspace_root;
use crate::source_image::{
    InventoryCaptureBundle, InventoryCaptureDisposition, SourceBlobHolderKind, SourceCapturePolicy,
    SourceImageStore, SourceInventoryCapturePolicy, SourceLanguage, SourceSelection,
    current_source_generation_from_reader,
};
use crate::workspace_registry::WorkspaceRecord;

use super::{ProductionWorkspaceStartupError, step};

/// Leases survive provider joins, and every early failure releases only this capture's holders.
pub(super) struct PreparedSourceInputs {
    store: CaptureStore,
    image_store: SourceImageStore,
    capture: Option<InventoryCaptureBundle>,
    pub inventory: ChargedValue<ProviderSourceInventory>,
    budget: ResourceBudget,
    cancellation: Cancellation,
}

enum CaptureStore {
    Capturing(OperationalStore),
    Detached {
        database: std::path::PathBuf,
        writer: std::sync::Arc<std::sync::Mutex<()>>,
    },
}

impl PreparedSourceInputs {
    /// Source bytes and blob leases remain owned, but the operational writer is needed only
    /// while capturing or releasing them. Checker/compiler execution uses immutable inputs.
    pub(super) fn detach_writer(
        &mut self,
        database: std::path::PathBuf,
        writer: std::sync::Arc<std::sync::Mutex<()>>,
    ) {
        self.store = CaptureStore::Detached { database, writer };
    }

    fn release_bundle(
        &mut self,
        bundle: InventoryCaptureBundle,
    ) -> Result<(), ProductionWorkspaceStartupError> {
        match &mut self.store {
            CaptureStore::Capturing(store) => self
                .image_store
                .release_inventory_capture(store, bundle)
                .map_err(|error| step("provider-source-release", error)),
            CaptureStore::Detached { database, writer } => {
                let _writer = writer
                    .lock()
                    .map_err(|error| step("source-release-writer", error))?;
                let mut store = OperationalStore::open(database)
                    .map_err(|error| step("source-release-store", error))?;
                self.image_store
                    .release_inventory_capture(&mut store, bundle)
                    .map_err(|error| step("provider-source-release", error))
            }
        }
    }

    pub fn inventory_for_language(
        &self,
        language: SourceLanguage,
    ) -> Result<ChargedValue<ProviderSourceInventory>, ProductionWorkspaceStartupError> {
        let bytes = self
            .inventory
            .memory_bytes()
            .map_err(|error| step("provider-selection-memory", error))?;
        let mut charge = reserve_memory(&self.budget, bytes.saturating_mul(4).saturating_add(8192))
            .map_err(|error| step("provider-selection-memory", error))?;
        let files = self
            .capture()?
            .images()
            .iter()
            .filter(|image| image.language == language)
            .map(|image| image.file_id)
            .collect();
        let inventory = self
            .inventory
            .select_files(&files)
            .map_err(|error| step("provider-input-selection", error))?;
        shrink_to_retained(
            &mut charge,
            inventory
                .memory_bytes()
                .map_err(|error| step("provider-selection-memory", error))?,
        )?;
        Ok(charge.into_charged_value(inventory))
    }

    pub fn budget(&self) -> &ResourceBudget {
        &self.budget
    }

    /// Retained provider state can outlive an unsuccessful publication. Retrying identical
    /// inputs needs a distinct operation owner; stable provider/source IDs are not lifetimes.
    pub fn provider_operation_budget(
        &self,
    ) -> Result<ResourceBudget, ProductionWorkspaceStartupError> {
        let attempt = crate::identity::random_registration_nonce()
            .map_err(|error| step("provider-operation-identity", error))?;
        self.budget
            .operation(attempt, self.budget.policy())
            .map_err(|error| step("provider-operation-owner", error))
    }
    pub fn capture(&self) -> Result<&InventoryCaptureBundle, ProductionWorkspaceStartupError> {
        self.capture
            .as_ref()
            .ok_or_else(|| step("source-capture-ownership", "capture already released"))
    }

    pub fn release(mut self) -> Result<(), ProductionWorkspaceStartupError> {
        if let Some(bundle) = self.capture.take() {
            self.release_bundle(bundle)?;
        }
        Ok(())
    }
}

impl Drop for PreparedSourceInputs {
    fn drop(&mut self) {
        if let Some(bundle) = self.capture.take() {
            // Failure paths cannot publish; lease expiry remains the bounded recovery fallback.
            let _ = self.release_bundle(bundle);
        }
    }
}

pub(super) fn capture_inputs(
    workspace_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    generation: u64,
    budget: ResourceBudget,
    cancellation: Cancellation,
    disk: crate::source_image::SourceBlobDiskLedger,
) -> Result<PreparedSourceInputs, ProductionWorkspaceStartupError> {
    if budget
        .ancestor_owner(ResourceScopeKind::Workspace)
        .is_none_or(|owner| owner.id != record.workspace_id)
    {
        return Err(step(
            "source-input-owner",
            ResourceBudgetError::ForeignOwner,
        ));
    }
    if cancellation.is_cancelled() {
        return Err(step("source-input-cancelled", "cancelled before capture"));
    }
    let mut store = OperationalStore::open(operational_database)
        .map_err(|error| step("source-store-open", error))?;
    let fence_reader = store
        .reader_factory()
        .open()
        .map_err(|error| step("source-fence-reader", error))?;
    let observe = || current_source_generation_from_reader(&fence_reader, record.workspace_id).ok();
    let root = open_workspace_root(&mut store, record.workspace_id)
        .map_err(|error| step("source-root", error))?;
    let inventory = InventoryWalker::new_governed(InventoryLimits::default(), budget.clone())
        .walk_selected_with_fence(
            &root,
            &mut store,
            generation,
            generation,
            &cancellation,
            observe,
        )
        .map_err(|error| inventory_failure("complete-source-inventory", error))?;
    let mut image_store = SourceImageStore::open_governed(
        &workspace_root.join("source-blobs"),
        SourceCapturePolicy::default(),
        budget.clone(),
        disk,
        &cancellation,
    )
    .map_err(|error| step("source-image-store", error))?;
    let capture = image_store
        .capture_inventory_with_fence(
            &mut store,
            &inventory,
            SourceInventoryCapturePolicy {
                maximum_total_bytes: 1024 * 1024 * 1024,
                holder_kind: SourceBlobHolderKind::ProviderRun,
            },
            &cancellation,
            |member| {
                SourceSelection::Capture(match member.language {
                    Some("python") => SourceLanguage::Python,
                    Some("rust") => SourceLanguage::Rust,
                    _ => SourceLanguage::Other,
                })
            },
            observe,
        )
        .map_err(|error| capture_failure("complete-source-capture", error))?;
    // Construct ownership before any later fallible operation so all error paths release leases.
    let input_inventory = match provider_inventory(&capture, &budget) {
        Ok(inventory) => inventory,
        Err(error) => {
            image_store
                .release_inventory_capture(&mut store, capture)
                .map_err(|release| step("failed-source-capture-release", release))?;
            return Err(error);
        }
    };
    let mut owned = PreparedSourceInputs {
        store: CaptureStore::Capturing(store),
        image_store,
        capture: Some(capture),
        inventory: input_inventory,
        budget,
        cancellation,
    };
    owned
        .capture()?
        .require_closed()
        .map_err(|error| super::source_changed("source-capture-pending", error))?;
    // The enclosing update operation also fences watcher events. This independent census
    // detects edits/creation/deletion during capture before any provider consumes the bundle.
    let CaptureStore::Capturing(store) = &mut owned.store else {
        unreachable!("capture writer remains local until input closure")
    };
    let checked = InventoryWalker::new_governed(InventoryLimits::default(), owned.budget.clone())
        .walk_selected_with_fence(
            &root,
            store,
            generation,
            generation,
            &owned.cancellation,
            observe,
        )
        .map_err(|error| inventory_failure("source-capture-reconciliation", error))?;
    if checked.inventory().digest != inventory.inventory().digest
        || checked.inventory().git_context_digest() != inventory.inventory().git_context_digest()
    {
        return Err(super::source_changed(
            "source-capture-reconciliation",
            "source inventory changed during capture",
        ));
    }
    Ok(owned)
}

fn provider_inventory(
    capture: &InventoryCaptureBundle,
    budget: &ResourceBudget,
) -> Result<ChargedValue<ProviderSourceInventory>, ProductionWorkspaceStartupError> {
    capture
        .require_closed()
        .map_err(|error| step("source-disposition-closure", error))?;
    let inventory = capture.inventory().inventory();
    // Four path/vector copies, BTree validation sets, owned members, and canonicalization
    // scratch are admitted from the actual census geometry, before any clone below.
    let bytes = inventory
        .records
        .iter()
        .try_fold(4096_u64, |sum, record| {
            sum.checked_add((record.path.raw_relative_path_bytes.len() as u64 + 512) * 16)
                .ok_or(ResourceBudgetError::Overflow)
        })
        .map_err(|error| step("provider-input-memory", error))?;
    let mut charge =
        reserve_memory(budget, bytes).map_err(|error| step("provider-input-memory", error))?;
    let paths = inventory
        .records
        .iter()
        .map(|record| record.path.raw_relative_path_bytes.clone())
        .collect::<Vec<_>>();
    let members = capture
        .dispositions()
        .iter()
        .map(|entry| {
            let (disposition, selected_for_provider) = match entry.disposition() {
                InventoryCaptureDisposition::Captured { image_index } => {
                    let image = capture.images().get(image_index).ok_or_else(|| {
                        step("source-disposition-index", "missing captured image")
                    })?;
                    (
                        ProviderInputDisposition::Captured {
                            file_id: image.file_id,
                            digest: image.digest,
                            byte_length: image.byte_length,
                        },
                        image.language == SourceLanguage::Python,
                    )
                }
                InventoryCaptureDisposition::Pending | InventoryCaptureDisposition::Deferred => {
                    return Err(super::source_changed(
                        "source-disposition-pending",
                        "source is still dirty",
                    ));
                }
                InventoryCaptureDisposition::Unreadable => {
                    (ProviderInputDisposition::Unreadable, false)
                }
                InventoryCaptureDisposition::Unsupported => {
                    (ProviderInputDisposition::Unsupported, false)
                }
                InventoryCaptureDisposition::UnsupportedEncoding => {
                    (ProviderInputDisposition::UnsupportedEncoding, false)
                }
                InventoryCaptureDisposition::Oversized => {
                    (ProviderInputDisposition::ExcludedSizeLimit, false)
                }
                InventoryCaptureDisposition::Binary => (ProviderInputDisposition::Binary, false),
                InventoryCaptureDisposition::Generated => {
                    (ProviderInputDisposition::Generated, false)
                }
                InventoryCaptureDisposition::Vendored => {
                    (ProviderInputDisposition::Vendored, false)
                }
                InventoryCaptureDisposition::Excluded => {
                    (ProviderInputDisposition::ExcludedPolicy, false)
                }
            };
            Ok(ProviderInventoryMember {
                relative_path: entry.path().raw_relative_path_bytes.clone(),
                disposition,
                selected_for_provider,
            })
        })
        .collect::<Result<Vec<_>, ProductionWorkspaceStartupError>>()?;
    let value = ProviderSourceInventory::try_new(
        inventory.workspace_id,
        inventory.source_generation,
        inventory.digest,
        &paths,
        members,
        paths.clone(),
        None,
    )
    .map_err(|error| step("provider-input-inventory", error))?;
    let retained = value
        .memory_bytes()
        .map_err(|error| step("provider-input-memory", error))?;
    shrink_to_retained(&mut charge, retained)?;
    Ok(charge.into_charged_value(value))
}

pub(super) fn discover_python_inputs(
    inputs: &PreparedSourceInputs,
    record: &WorkspaceRecord,
) -> Result<ChargedValue<PythonContextDiscoveryProduct>, ProductionWorkspaceStartupError> {
    if inputs.cancellation.is_cancelled() {
        return Err(step(
            "context-input-cancelled",
            "cancelled before discovery",
        ));
    }
    let capture = inputs.capture()?;
    // Discovery owns one source copy, bounded TOML parsing state, path/root maps and
    // canonical products. Ordinary source contents never multiply by the file count.
    let (request_bytes, product_bytes) = python_discovery_memory_bounds(capture)?;
    let _request_charge = reserve_memory(&inputs.budget, request_bytes)
        .map_err(|error| step("context-input-memory", error))?;
    let mut product_charge = reserve_memory(&inputs.budget, product_bytes)
        .map_err(|error| step("context-product-memory", error))?;
    let mut files = Vec::with_capacity(capture.images().len());
    let mut roots = BTreeSet::from([".".to_owned()]);
    for image in capture.images() {
        // Configured roots are Unicode TOML strings; source identity remains byte-native.
        // Files below non-Unicode components retain their project-root input binding.
        if let Ok(path) = std::str::from_utf8(&image.path.raw_relative_path_bytes) {
            let mut parent = Path::new(path).parent();
            while let Some(path) = parent {
                if !path.as_os_str().is_empty() {
                    roots.insert(path.to_str().expect("parent of a Unicode path").to_owned());
                }
                parent = path.parent();
            }
        }
        files.push(PythonDiscoveryFile {
            file_id: encode_public_id(IdentityDomain::SourceFile, None, image.file_id)
                .map_err(|error| step("context-file-id", error))?,
            relative_path: image.path.raw_relative_path_bytes.clone(),
            display_path: image.path.display_string.clone(),
            digest: image.digest,
            contents: image.bytes.to_vec(),
        });
    }
    let root_id = "workspace-root".to_owned();
    let scope = ContextSearchScope {
        namespace: b".".to_vec(),
        ordered_roots: vec![ContextSearchRoot {
            root_id: root_id.clone(),
            relative_path: b".".to_vec(),
        }],
        policy_identity: record.authorization_fingerprint,
        universe: if capture.dispositions().iter().any(|entry| {
            !matches!(
                entry.disposition(),
                InventoryCaptureDisposition::Captured { .. }
            )
        }) {
            ContextSearchUniverse::Incomplete {
                observed_identity: inputs.inventory.identity(),
            }
        } else {
            ContextSearchUniverse::Closed {
                inventory_identity: inputs.inventory.identity(),
            }
        },
    };
    let request = PythonContextDiscoveryRequest {
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, record.workspace_id)
            .map_err(|error| step("context-workspace-id", error))?,
        source_generation: inputs.inventory.source_generation(),
        project_root_path: ".".to_owned(),
        project_root_id: root_id.clone(),
        platform_tag: match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            other => other,
        }
        .to_owned(),
        files,
        workspace_profile: None,
        registered: PythonRegisteredInputs {
            authorized_roots: roots
                .into_iter()
                .map(|path| PythonAuthorizedRoot {
                    path_id: if path == "." {
                        root_id.clone()
                    } else {
                        format!("workspace-root/{path}")
                    },
                    relative_path: path,
                })
                .collect(),
            ..PythonRegisteredInputs::default()
        },
        // A release-selected syntax profile, not a host interpreter or checker default.
        deployment: PythonDeploymentProfile {
            supported_python_versions: (7..=15).map(|minor| format!("3.{minor}")).collect(),
            default_python_version: "3.14".to_owned(),
        },
        typeshed_bundle_digest: None,
        pyrefly_bundle_digest: None,
        ruff_bundle_digest: *blake3::hash(
            crate::provider_raw_kinds::RUFF_PYTHON_FRONTEND
                .provider_version
                .as_bytes(),
        )
        .as_bytes(),
        provider_bundle_version: "codefabric-python-syntax-v2".to_owned(),
        search_scope: scope,
    };
    let product = discover_python_context(&request)
        .map_err(|error| step("effective-python-context", error))?;
    if inputs.cancellation.is_cancelled() {
        return Err(step("context-input-cancelled", "cancelled after discovery"));
    }
    shrink_to_retained(&mut product_charge, python_product_memory_bytes(&product))?;
    Ok(product_charge.into_charged_value(product))
}

pub(super) fn provider_context(
    product: &ChargedValue<PythonContextDiscoveryProduct>,
) -> Result<ChargedValue<ProviderContextBinding>, ProductionWorkspaceStartupError> {
    let bytes = (product.canonical_manifest.len() as u64)
        .checked_mul(16)
        .and_then(|n| {
            n.checked_add(
                product.configuration_dependencies.lookup_evidence.len() as u64 * 8192 + 8192,
            )
        })
        .ok_or_else(|| step("provider-context-memory", ResourceBudgetError::Overflow))?;
    let mut charge = reserve_memory(product.reservation().owner(), bytes)
        .map_err(|error| step("provider-context-memory", error))?;
    let settings = product
        .effective_settings()
        .map_err(|error| step("python-effective-settings", error))?;
    let fingerprint = product
        .context
        .fingerprint_bytes()
        .map_err(|error| step("context-fingerprint", error))?;
    let canonical_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &product.context.analysis_context_id,
    )
    .map_err(|error| step("canonical-analysis-context", error))?;
    let dependencies = product
        .configuration_dependencies
        .lookup_evidence
        .iter()
        .map(|lookup| {
            lookup
                .validate()
                .map_err(|error| step("context-support", error))?;
            let revision = match lookup.scope.universe {
                ContextSearchUniverse::Closed { inventory_identity } => inventory_identity,
                ContextSearchUniverse::Incomplete { observed_identity } => observed_identity,
            };
            let outcome = match &lookup.observation {
                ContextLookupObservation::Present { file_id, digest } => {
                    ProviderLookupOutcome::Consumed {
                        revision: *digest,
                        candidates: vec![
                            decode_public_id(IdentityDomain::SourceFile, None, file_id)
                                .map_err(|error| step("context-support-file", error))?,
                        ],
                    }
                }
                ContextLookupObservation::Absent => ProviderLookupOutcome::Absent {
                    closed_universe: revision,
                },
                ContextLookupObservation::Incomplete => ProviderLookupOutcome::Incomplete {
                    observed_universe: revision,
                },
            };
            Ok(ProviderSupportDependency {
                key: ProviderLookupKey::Configuration {
                    relative_path: lookup.relative_path.clone(),
                },
                scope: ProviderSearchScope {
                    namespace: lookup.scope.namespace.clone(),
                    ordered_roots: lookup
                        .scope
                        .ordered_roots
                        .iter()
                        .map(|root| {
                            if root.relative_path == b"." {
                                Vec::new()
                            } else {
                                root.relative_path.clone()
                            }
                        })
                        .collect(),
                    policy_identity: lookup.scope.policy_identity,
                    effective_context: fingerprint,
                },
                outcome,
            })
        })
        .collect::<Result<Vec<_>, ProductionWorkspaceStartupError>>()?;
    let modules = settings
        .module_map
        .iter()
        .map(|module| {
            Ok(ProviderModuleBinding {
                file_id: decode_public_id(IdentityDomain::SourceFile, None, &module.file_id)
                    .map_err(|error| step("context-module-file", error))?,
                qualified_name: module.module_name.clone(),
                relative_path: module.relative_path.clone(),
            })
        })
        .collect::<Result<Vec<_>, ProductionWorkspaceStartupError>>()?;
    let context = ProviderContextBinding::try_new(
        ContextIdentity::try_new(product.context.analysis_context_id.clone())
            .map_err(|error| step("context-identity", error))?,
        canonical_id,
        fingerprint,
        fingerprint,
    )
    .and_then(|context| {
        context.with_python_version(
            settings.language_version.major,
            settings.language_version.minor,
        )
    })
    .and_then(|context| context.with_modules(modules))
    .and_then(|context| context.with_support_obligations(dependencies))
    .map_err(|error| step("provider-effective-context", error))?;
    let retained = context
        .memory_bytes()
        .map_err(|error| step("provider-context-memory", error))?;
    shrink_to_retained(&mut charge, retained)?;
    Ok(charge.into_charged_value(context))
}

fn shrink_to_retained(
    charge: &mut crate::resource_budget::ResourceReservation,
    retained: u64,
) -> Result<(), ProductionWorkspaceStartupError> {
    let reserved = charge.amounts().memory_bytes;
    let freed = reserved.checked_sub(retained).ok_or_else(|| {
        step(
            "input-allocation-bound",
            ResourceBudgetError::UnchargedAllocation {
                required: retained,
                reserved,
            },
        )
    })?;
    charge
        .shrink(ResourceAmounts {
            memory_bytes: freed,
            ..ResourceAmounts::default()
        })
        .map_err(|error| step("input-allocation-transfer", error))
}

fn python_product_memory_bytes(product: &PythonContextDiscoveryProduct) -> u64 {
    let mut bytes = std::mem::size_of_val(product) as u64;
    let manifest = &product.manifest;
    for value in [
        &manifest.python_language_version,
        &manifest.implementation_profile,
        &manifest.platform_tag,
        &manifest.namespace_package_policy,
        &manifest.ruff_bundle_digest,
        &manifest.provider_bundle_version,
        &product.context_manifest_digest,
        &product.context.workspace_id,
        &product.context.analysis_context_id,
        &product.context.context_fingerprint,
        &product.context.provider_bundle_version,
        &product.context.compiler_or_language_version,
        &product.configuration_dependencies.dependency_set_digest,
    ] {
        bytes += value.capacity() as u64;
    }
    for value in [
        &manifest.typeshed_bundle_digest,
        &manifest.pyrefly_bundle_digest,
        &product.context.configuration_manifest_uri,
    ]
    .into_iter()
    .flatten()
    {
        bytes += value.capacity() as u64;
    }
    for values in [
        &manifest.module_roots,
        &manifest.source_roots,
        &manifest.stub_roots,
        &manifest.dependency_roots,
        &manifest.import_precedence,
        &manifest.platforms,
    ] {
        bytes += (values.capacity() * std::mem::size_of::<String>()) as u64;
        bytes += values.iter().map(|v| v.capacity() as u64).sum::<u64>();
    }
    if let Some(values) = &manifest.unapplied_checker_settings {
        bytes += (values.capacity() * std::mem::size_of::<String>()) as u64;
        bytes += values.iter().map(|v| v.capacity() as u64).sum::<u64>();
    }
    for values in [
        &manifest.lockfile_artifacts,
        &manifest.project_config_artifacts,
    ] {
        bytes += (values.capacity()
            * std::mem::size_of::<crate::python_context::PythonContextArtifact>())
            as u64;
        bytes += values
            .iter()
            .map(|v| (v.file_id.capacity() + v.digest.capacity()) as u64)
            .sum::<u64>();
    }
    bytes += (manifest.typing_markers.capacity()
        * std::mem::size_of::<crate::python_context::PythonTypingMarker>()) as u64;
    bytes += manifest
        .typing_markers
        .iter()
        .map(|marker| {
            (marker.relative_path.capacity()
                + marker.digest.capacity()
                + marker.contents.capacity()) as u64
        })
        .sum::<u64>();
    bytes += (manifest.module_map.capacity()
        * std::mem::size_of::<crate::analysis_context::PythonModuleBinding>()) as u64;
    bytes += manifest
        .module_map
        .iter()
        .map(|v| {
            (v.module_name.capacity()
                + v.file_id.capacity()
                + v.relative_path.capacity()
                + v.root_id.capacity()) as u64
        })
        .sum::<u64>();
    for values in [&manifest.root_bindings, &manifest.configuration_roots] {
        bytes += (values.capacity() * std::mem::size_of::<ContextSearchRoot>()) as u64;
        bytes += values
            .iter()
            .map(|v| (v.root_id.capacity() + v.relative_path.capacity()) as u64)
            .sum::<u64>();
    }
    bytes += (manifest.configuration_namespace.capacity() + product.canonical_manifest.capacity())
        as u64;
    let dependencies = &product.configuration_dependencies;
    bytes += (dependencies.dependencies.capacity()
        * std::mem::size_of::<crate::python_context::PythonConfigurationDependency>())
        as u64;
    bytes += dependencies
        .dependencies
        .iter()
        .map(|v| (v.file_id.capacity() + v.digest.capacity()) as u64)
        .sum::<u64>();
    bytes += (dependencies.lookup_evidence.capacity()
        * std::mem::size_of::<crate::analysis_context::ContextLookupEvidence>())
        as u64;
    for lookup in &dependencies.lookup_evidence {
        bytes += (lookup.relative_path.capacity()
            + lookup.scope.namespace.capacity()
            + lookup.scope.ordered_roots.capacity() * std::mem::size_of::<ContextSearchRoot>())
            as u64;
        bytes += lookup
            .scope
            .ordered_roots
            .iter()
            .map(|v| (v.root_id.capacity() + v.relative_path.capacity()) as u64)
            .sum::<u64>();
        if let ContextLookupObservation::Present { file_id, .. } = &lookup.observation {
            bytes += file_id.capacity() as u64;
        }
    }
    bytes += (product.diagnostics.capacity()
        * std::mem::size_of::<crate::python_context::PythonContextDiagnostic>())
        as u64;
    bytes
        + product
            .diagnostics
            .iter()
            .map(|v| v.detail.capacity() as u64)
            .sum::<u64>()
}

fn python_discovery_memory_bounds(
    capture: &InventoryCaptureBundle,
) -> Result<(u64, u64), ProductionWorkspaceStartupError> {
    let mut request = 64 * 1024_u64;
    let mut product = 64 * 1024_u64;
    for image in capture.images() {
        let path = &image.path.raw_relative_path_bytes;
        let depth = path.iter().filter(|byte| **byte == b'/').count() as u64 + 1;
        // At most one derived module binding per file (discover_module_map's bound_files
        // set). Every ancestor may also contribute an authorized-root record.
        let geometry = (path.len() as u64 + 512)
            .checked_mul(depth)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
        request = request
            .checked_add(image.byte_length)
            .and_then(|n| n.checked_add(geometry))
            .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
        product = product
            .checked_add(geometry)
            .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
        if path.ends_with(b"pyproject.toml") || path.ends_with(b"pyrefly.toml") {
            // TOML values/nodes plus selected arrays and JCS validation copies; root
            // membership itself remains bounded by the independently observed namespace.
            let config = image
                .byte_length
                .checked_mul(128)
                .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
            request = request
                .checked_add(config)
                .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
            product = product
                .checked_add(config)
                .ok_or_else(|| step("context-memory-overflow", ResourceBudgetError::Overflow))?;
        }
    }
    Ok((request, product))
}

fn inventory_failure(
    phase: &'static str,
    error: crate::inventory::InventoryError,
) -> ProductionWorkspaceStartupError {
    if matches!(error, crate::inventory::InventoryError::SourceChanged) {
        super::source_changed(phase, error)
    } else {
        step(phase, error)
    }
}

fn capture_failure(
    phase: &'static str,
    error: crate::source_image::SourceImageError,
) -> ProductionWorkspaceStartupError {
    use crate::source_image::SourceImageError;
    if matches!(
        error,
        SourceImageError::GenerationChanged
            | SourceImageError::Inventory(crate::inventory::InventoryError::SourceChanged)
            | SourceImageError::StableRead(crate::secure_path::StableReadError::ChangedDuringRead)
    ) {
        super::source_changed(phase, error)
    } else {
        step(phase, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_image::advance_source_generation;
    use crate::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

    fn capture_fixture(
        root: &Path,
        db: &Path,
        record: &WorkspaceRecord,
        generation: u64,
    ) -> Result<PreparedSourceInputs, ProductionWorkspaceStartupError> {
        let budget = crate::provider_types::source_fixture_budget(record.workspace_id);
        let disk = crate::source_image::SourceBlobDiskLedger::try_new(
            budget.clone(),
            crate::disk_headroom::LocalDiskHeadroom::open(root.parent().unwrap()).unwrap(),
        )
        .unwrap();
        capture_inputs(
            root,
            db,
            record,
            generation,
            budget,
            Cancellation::default(),
            disk,
        )
    }

    #[test]
    fn fresh_capture_context_configuration_is_a_causal_job_input() {
        let directory = tempfile::tempdir().unwrap();
        let source_root = directory.path().join("workspace");
        std::fs::create_dir(&source_root).unwrap();
        std::fs::write(source_root.join("module.py"), b"value = 1\n").unwrap();
        let database = directory.path().join("state.sqlite3");
        let mut store = OperationalStore::open(&database).unwrap();
        let record = WorkspaceRegistry::new(&mut store)
            .add(&source_root, WorkspaceSourceRegistration::Directory)
            .unwrap();
        let fabric_root = directory.path().join("fabric");
        drop(store);
        let shared_budget = crate::provider_types::source_fixture_budget(record.workspace_id);
        let disk = crate::source_image::SourceBlobDiskLedger::try_new(
            shared_budget.clone(),
            crate::disk_headroom::LocalDiskHeadroom::open(directory.path()).unwrap(),
        )
        .unwrap();
        let mut first = capture_inputs(
            &fabric_root,
            &database,
            &record,
            0,
            shared_budget.clone(),
            Cancellation::default(),
            disk.clone(),
        )
        .unwrap();
        first.detach_writer(
            database.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(())),
        );
        let concurrent_writer = OperationalStore::open(&database).unwrap();
        let reader = concurrent_writer.reader_factory().open().unwrap();
        let leases = || {
            reader
                .with_connection(|connection| {
                    connection.query_row(
                        "SELECT COUNT(*) FROM source_blob_lease WHERE state_code=1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                })
                .unwrap()
        };
        assert_eq!(
            leases(),
            1,
            "immutable provider inputs still own their blob lease"
        );
        assert_eq!(
            first.capture().unwrap().images()[0].bytes.as_ref(),
            b"value = 1\n"
        );
        drop(concurrent_writer);
        let initial_product = discover_python_inputs(&first, &record).unwrap();
        let initial = provider_context(&initial_product).unwrap();
        // Exercise real compiled job admission: a resource-only nonce would be rejected
        // because each job must carry exactly its resource owner's operation identity.
        let release = crate::semantic_release::compile_current_v23_release(
            crate::production_provider_recipe::current_v23_provider_program_definition().unwrap(),
        )
        .unwrap();
        let prepare = || {
            super::super::pyrefly::prepare_job(
                &release,
                &first,
                &initial,
                crate::relation_ipc::SourcePin(first.inventory.identity()),
                1,
                Cancellation::default(),
            )
            .unwrap()
        };
        let first_job = prepare();
        let budget = first_job.job().resource_budget();
        let before = budget.observation().used.memory_bytes;
        let retained = budget
            .try_reserve(
                crate::resource_budget::ResourceClass::Data,
                crate::resource_budget::ResourceAmounts {
                    memory_bytes: 1024,
                    ..Default::default()
                },
            )
            .unwrap();
        let retry = prepare();
        assert_ne!(
            first_job.job().run().provider_run_id(),
            retry.job().run().provider_run_id()
        );
        for job in [&first_job, &retry] {
            assert_eq!(
                job.job().run().provider_run_id(),
                job.job().resource_budget().owner().id
            );
        }
        drop(retry);
        assert_eq!(budget.observation().used.memory_bytes, before + 1024);
        drop(retained);
        drop(first_job);
        let retained_budget = first.budget().clone();
        assert_eq!(initial.python_version(), Some((3, 14)));
        let (file_id, digest) = first.inventory.selected_files().next().unwrap();
        assert_eq!(
            initial.module_for_file(file_id).unwrap().qualified_name,
            "module"
        );
        assert!(
            initial
                .support_obligations()
                .iter()
                .any(|dependency| matches!(
                    dependency.outcome,
                    ProviderLookupOutcome::Absent { .. }
                ))
        );
        first.release().unwrap();
        assert_eq!(
            leases(),
            0,
            "provider cleanup releases through the serialized writer"
        );
        drop(reader);
        assert!(retained_budget.observation().used.memory_bytes > 0);

        std::fs::write(
            source_root.join("pyrefly.toml"),
            b"python-version = '3.12'\n",
        )
        .unwrap();
        let mut store = OperationalStore::open(&database).unwrap();
        advance_source_generation(&mut store, record.workspace_id, 0).unwrap();
        drop(store);
        let second = capture_inputs(
            &fabric_root,
            &database,
            &record,
            1,
            shared_budget.clone(),
            Cancellation::default(),
            disk.clone(),
        )
        .unwrap();
        let selected_product = discover_python_inputs(&second, &record).unwrap();
        let selected = provider_context(&selected_product).unwrap();
        assert_eq!(
            second.inventory.selected_files().next(),
            Some((file_id, digest))
        );
        assert_eq!(selected.python_version(), Some((3, 12)));
        assert_ne!(
            initial.analysis_context_id(),
            selected.analysis_context_id()
        );
        assert_ne!(
            initial.effective_input_identity(),
            selected.effective_input_identity()
        );
        assert!(selected.support_obligations().iter().any(|dependency|
            matches!(&dependency.key, ProviderLookupKey::Configuration { relative_path } if relative_path == b"pyrefly.toml")
                && matches!(dependency.outcome, ProviderLookupOutcome::Consumed { .. })));
        second.release().unwrap();
        drop((initial, initial_product));
        assert!(retained_budget.observation().used.memory_bytes > 0);
        drop((selected, selected_product, disk));
        assert_eq!(retained_budget.observation().used.memory_bytes, 0);
    }

    #[test]
    fn fresh_capture_binary_configuration_is_unknown_not_absent() {
        let directory = tempfile::tempdir().unwrap();
        let source_root = directory.path().join("workspace");
        std::fs::create_dir(&source_root).unwrap();
        std::fs::write(source_root.join("pyrefly.toml"), b"\0binary").unwrap();
        let database = directory.path().join("state.sqlite3");
        let mut store = OperationalStore::open(&database).unwrap();
        let record = WorkspaceRegistry::new(&mut store)
            .add(&source_root, WorkspaceSourceRegistration::Directory)
            .unwrap();
        drop(store);
        let captured =
            capture_fixture(&directory.path().join("fabric"), &database, &record, 0).unwrap();
        assert_eq!(captured.inventory.selected_files().count(), 0);
        assert!(
            captured
                .inventory
                .members()
                .iter()
                .any(|member| member.relative_path == b"pyrefly.toml"
                    && member.disposition == ProviderInputDisposition::Binary)
        );
        let product = discover_python_inputs(&captured, &record).unwrap();
        let context = provider_context(&product).unwrap();
        assert!(context.support_obligations().iter().any(|dependency|
            matches!(&dependency.key, ProviderLookupKey::Configuration { relative_path } if relative_path == b"pyrefly.toml")
                && matches!(dependency.outcome, ProviderLookupOutcome::Incomplete { .. })));
        assert!(
            !context
                .support_obligations()
                .iter()
                .any(|dependency| matches!(
                    dependency.outcome,
                    ProviderLookupOutcome::Absent { .. }
                ))
        );
        captured.release().unwrap();
    }
}
