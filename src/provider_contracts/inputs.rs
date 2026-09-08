//! Closed immutable input sets and typed dependency support at the provider boundary.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{ProviderContractError, ProviderFamilyIdentity, ProviderJob, require_nonzero_bytes};

const MAX_INPUT_MEMBERS: usize = 262_144;
const MAX_SUPPORT_PARTITIONS: usize = 1_048_576;
const MAX_SUPPORT_DEPENDENCIES: usize = 262_144;
const MAX_LOOKUP_CANDIDATES: usize = 65_536;
const MAX_SEARCH_ROOTS: usize = 4_096;
const MAX_INPUT_STRING_BYTES: usize = 16_384;
const MAX_INPUT_METADATA_BYTES: usize = 64 * 1024 * 1024;

type WithdrawnMember = ([u8; 16], Vec<u8>);

fn bounded_count(value: usize, maximum: usize) -> Result<(), ProviderContractError> {
    if value > maximum {
        return Err(ProviderContractError::ResourceCeilingExceeded);
    }
    Ok(())
}

fn checked_bytes(total: &mut usize, bytes: usize) -> Result<(), ProviderContractError> {
    *total = total
        .checked_add(bytes)
        .ok_or(ProviderContractError::ResourceOverflow)?;
    bounded_count(*total, MAX_INPUT_METADATA_BYTES)
}

fn validate_word(value: &[u8]) -> Result<(), ProviderContractError> {
    bounded_count(value.len(), MAX_INPUT_STRING_BYTES)?;
    if value.is_empty() || value.contains(&0) {
        return Err(ProviderContractError::SupportMismatch);
    }
    Ok(())
}

/// A source image and a whole context inventory are different identity categories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderSourceSelection {
    File {
        file_id: [u8; 16],
        content_digest: [u8; 32],
    },
    Inventory(crate::resource_budget::ChargedValue<ProviderSourceInventory>),
}

/// Exact qualified module identity consumed by an in-process Python frontend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderModuleBinding {
    pub file_id: [u8; 16],
    pub qualified_name: String,
    pub relative_path: Vec<u8>,
}

pub(super) fn validate_modules(
    modules: &[ProviderModuleBinding],
) -> Result<(), ProviderContractError> {
    bounded_count(modules.len(), MAX_INPUT_MEMBERS)?;
    let mut files = BTreeSet::new();
    for module in modules {
        require_nonzero_bytes(&module.file_id)?;
        validate_path(&module.relative_path)?;
        validate_word(module.qualified_name.as_bytes())?;
        if !files.insert(module.file_id) {
            return Err(ProviderContractError::SupportMismatch);
        }
    }
    module_memory_bytes(modules)?;
    Ok(())
}

fn module_memory_bytes(modules: &[ProviderModuleBinding]) -> Result<usize, ProviderContractError> {
    let mut bytes = 0;
    for module in modules {
        checked_bytes(&mut bytes, std::mem::size_of::<ProviderModuleBinding>())?;
        checked_bytes(&mut bytes, module.qualified_name.capacity())?;
        checked_bytes(&mut bytes, module.relative_path.capacity())?;
    }
    Ok(bytes)
}

/// Terminal input disposition; pending reads cannot enter a closed provider inventory.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ProviderInputDisposition {
    Captured {
        file_id: [u8; 16],
        digest: [u8; 32],
        byte_length: u64,
    },
    ExcludedPolicy,
    ExcludedSpecialFile,
    ExcludedSizeLimit,
    Binary,
    UnsupportedEncoding,
    Unreadable,
    Generated,
    Vendored,
    Unsupported,
}

/// Raw relative paths remain byte-safe and every inventoried member has one disposition.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderInventoryMember {
    pub relative_path: Vec<u8>,
    pub disposition: ProviderInputDisposition,
    /// Provider selection is explicit; captured configuration is not automatically a module.
    pub selected_for_provider: bool,
}

/// Complete independently enumerated inputs, plus distinct changed and withdrawn members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSourceInventory {
    workspace_id: [u8; 16],
    source_generation: u64,
    identity: [u8; 32],
    members: Vec<ProviderInventoryMember>,
    changed: Vec<Vec<u8>>,
    withdrawn: Vec<([u8; 16], Vec<u8>)>,
}

impl ProviderSourceInventory {
    /// Retained container and nested path capacities; source byte bodies are charged separately.
    ///
    /// # Errors
    /// Rejects arithmetic overflow or a metadata envelope violation.
    pub fn memory_bytes(&self) -> Result<u64, ProviderContractError> {
        let mut bytes = std::mem::size_of::<Self>();
        checked_bytes(
            &mut bytes,
            self.members
                .capacity()
                .checked_mul(std::mem::size_of::<ProviderInventoryMember>())
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )?;
        checked_bytes(
            &mut bytes,
            self.changed
                .capacity()
                .checked_mul(std::mem::size_of::<Vec<u8>>())
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )?;
        checked_bytes(
            &mut bytes,
            self.withdrawn
                .capacity()
                .checked_mul(std::mem::size_of::<WithdrawnMember>())
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )?;
        for path in self
            .members
            .iter()
            .map(|member| &member.relative_path)
            .chain(self.changed.iter())
            .chain(self.withdrawn.iter().map(|(_, path)| path))
        {
            checked_bytes(&mut bytes, path.capacity())?;
        }
        Ok(bytes as u64)
    }

    /// Join a disposition set to the independently authorized full path census.
    ///
    /// The daemon supplies these from its sealed capture bundle, not a dirty work list.
    /// A previous inventory is mandatory for withdrawals; an unknown path is not a deletion.
    ///
    /// # Errors
    /// Rejects missing/duplicate members, invalid pins, invented withdrawals, or changed paths
    /// outside the full current inventory. Empty complete inventories are lawful.
    pub fn try_new(
        workspace_id: [u8; 16],
        source_generation: u64,
        identity: [u8; 32],
        authorized_paths: &[Vec<u8>],
        mut members: Vec<ProviderInventoryMember>,
        mut changed: Vec<Vec<u8>>,
        previous: Option<&Self>,
    ) -> Result<Self, ProviderContractError> {
        require_nonzero_bytes(&workspace_id)?;
        require_nonzero_bytes(&identity)?;
        bounded_count(members.len(), MAX_INPUT_MEMBERS)?;
        bounded_count(authorized_paths.len(), MAX_INPUT_MEMBERS)?;
        bounded_count(changed.len(), MAX_INPUT_MEMBERS)?;
        let mut metadata_bytes = 0;
        for path in authorized_paths.iter().chain(changed.iter()) {
            validate_path(path)?;
            checked_bytes(&mut metadata_bytes, path.capacity())?;
        }
        let authorized = authorized_paths.iter().collect::<BTreeSet<_>>();
        if authorized.len() != authorized_paths.len() || members.len() != authorized.len() {
            return Err(ProviderContractError::SourceInventoryMismatch);
        }
        members.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let mut paths = BTreeSet::new();
        let mut files = BTreeSet::new();
        let mut input_bytes = 0_u64;
        for member in &members {
            validate_path(&member.relative_path)?;
            checked_bytes(
                &mut metadata_bytes,
                std::mem::size_of::<ProviderInventoryMember>(),
            )?;
            checked_bytes(&mut metadata_bytes, member.relative_path.capacity())?;
            if !paths.insert(&member.relative_path) || !authorized.contains(&member.relative_path) {
                return Err(ProviderContractError::SourceInventoryMismatch);
            }
            match member.disposition {
                ProviderInputDisposition::Captured {
                    file_id,
                    digest,
                    byte_length,
                } => {
                    require_nonzero_bytes(&file_id)?;
                    require_nonzero_bytes(&digest)?;
                    if byte_length > super::MAX_INPUT_BYTES || !files.insert(file_id) {
                        return Err(ProviderContractError::SourceInventoryMismatch);
                    }
                    input_bytes = input_bytes
                        .checked_add(byte_length)
                        .ok_or(ProviderContractError::ResourceOverflow)?;
                    if input_bytes > super::MAX_INPUT_BYTES {
                        return Err(ProviderContractError::ResourceCeilingExceeded);
                    }
                }
                _ if member.selected_for_provider => {
                    return Err(ProviderContractError::SourceInventoryMismatch);
                }
                _ => {}
            }
        }
        changed.sort();
        if changed.windows(2).any(|pair| pair[0] == pair[1])
            || changed.iter().any(|path| !paths.contains(path))
        {
            return Err(ProviderContractError::SourceInventoryMismatch);
        }
        let withdrawn =
            Self::withdrawal_members(previous, workspace_id, source_generation, &members)?;
        for (_, path) in &withdrawn {
            checked_bytes(
                &mut metadata_bytes,
                std::mem::size_of::<([u8; 16], Vec<u8>)>(),
            )?;
            checked_bytes(&mut metadata_bytes, path.len())?;
        }
        // This is an application input-set identity, not the filesystem enumeration identity.
        // Record arrays are sorted above; the existing JCS boundary owns object key ordering.
        let value = serde_json::json!({
            "profile": "codefabric.provider-input-inventory.v1",
            "workspace": workspace_id,
            "generation": source_generation.to_string(),
            "enumeration": identity,
            "members": members,
            "changed": changed,
            "withdrawn": withdrawn,
            "predecessor": previous.map(Self::identity),
        });
        let canonical = crate::contracts::jcs::canonicalize_value(&value)
            .map_err(|_| ProviderContractError::SourceInventoryMismatch)?;
        let identity = *blake3::hash(&canonical).as_bytes();
        Ok(Self {
            workspace_id,
            source_generation,
            identity,
            members,
            changed,
            withdrawn,
        })
    }

    fn withdrawal_members(
        previous: Option<&Self>,
        workspace_id: [u8; 16],
        source_generation: u64,
        members: &[ProviderInventoryMember],
    ) -> Result<Vec<WithdrawnMember>, ProviderContractError> {
        let mut withdrawn = Vec::new();
        if let Some(previous) = previous {
            if previous.workspace_id != workspace_id
                || previous.source_generation >= source_generation
            {
                return Err(ProviderContractError::SourceInventoryMismatch);
            }
            let files_by_path = members
                .iter()
                .filter_map(|member| match member.disposition {
                    ProviderInputDisposition::Captured { file_id, .. } => {
                        Some((member.relative_path.as_slice(), file_id))
                    }
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            let paths_by_file = files_by_path
                .iter()
                .map(|(path, file_id)| (*file_id, *path))
                .collect::<BTreeMap<_, _>>();
            for member in &previous.members {
                if let ProviderInputDisposition::Captured { file_id, .. } = member.disposition {
                    if files_by_path
                        .get(member.relative_path.as_slice())
                        .is_some_and(|current| *current != file_id)
                        || paths_by_file
                            .get(&file_id)
                            .is_some_and(|current| *current != member.relative_path.as_slice())
                    {
                        return Err(ProviderContractError::SourceInventoryMismatch);
                    }
                    let current = members
                        .binary_search_by(|current| {
                            current.relative_path.cmp(&member.relative_path)
                        })
                        .ok()
                        .map(|index| &members[index]);
                    if member.selected_for_provider
                        && !current.is_some_and(|current| current.selected_for_provider)
                    {
                        withdrawn.push((file_id, member.relative_path.clone()));
                    }
                }
            }
        }
        Ok(withdrawn)
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }
    #[must_use]
    pub const fn source_generation(&self) -> u64 {
        self.source_generation
    }
    #[must_use]
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    #[must_use]
    pub fn members(&self) -> &[ProviderInventoryMember] {
        &self.members
    }
    #[must_use]
    pub fn changed_paths(&self) -> &[Vec<u8>] {
        &self.changed
    }
    #[must_use]
    pub fn withdrawn(&self) -> &[([u8; 16], Vec<u8>)] {
        &self.withdrawn
    }
    pub fn selected_files(&self) -> impl Iterator<Item = ([u8; 16], [u8; 32])> + '_ {
        self.members
            .iter()
            .filter_map(|member| match member.disposition {
                ProviderInputDisposition::Captured {
                    file_id, digest, ..
                } if member.selected_for_provider => Some((file_id, digest)),
                _ => None,
            })
    }
}

fn validate_path(path: &[u8]) -> Result<(), ProviderContractError> {
    bounded_count(path.len(), MAX_INPUT_STRING_BYTES)?;
    if path.is_empty()
        || path.contains(&0)
        || path[0] == b'/'
        || path
            .split(|b| *b == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return Err(ProviderContractError::SourceInventoryMismatch);
    }
    Ok(())
}

/// Distinct closed lookup keys, not a free-form predicate language.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ProviderLookupKey {
    SourceBytes {
        file_id: [u8; 16],
    },
    Configuration {
        relative_path: Vec<u8>,
    },
    BuildInput {
        relative_path: Vec<u8>,
    },
    ProducerRelease,
    /// Typed module/version/context observations, separate from the canonical context ID.
    EffectiveContextInputs,
    PositiveFact {
        relation: String,
        entity_id: [u8; 16],
    },
    Import {
        qualified_name: Vec<u8>,
    },
    Export {
        qualified_name: Vec<u8>,
    },
    Name {
        name: Vec<u8>,
    },
    Member {
        receiver_type: [u8; 16],
        name: Vec<u8>,
    },
    Implementation {
        trait_id: [u8; 16],
        receiver_type: [u8; 16],
    },
    CallableSummary {
        callable_id: [u8; 16],
    },
    GraphProjection {
        projection_id: [u8; 32],
    },
}

/// Root order and selection policy are part of a negative dependency's support.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderSearchScope {
    pub namespace: Vec<u8>,
    pub ordered_roots: Vec<Vec<u8>>,
    pub policy_identity: [u8; 32],
    pub effective_context: [u8; 32],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ProviderLookupOutcome {
    Consumed {
        revision: [u8; 32],
        candidates: Vec<[u8; 16]>,
    },
    Absent {
        closed_universe: [u8; 32],
    },
    Incomplete {
        observed_universe: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderSupportDependency {
    pub key: ProviderLookupKey,
    pub scope: ProviderSearchScope,
    pub outcome: ProviderLookupOutcome,
}

pub(super) fn validate_dependencies(
    dependencies: &[ProviderSupportDependency],
) -> Result<(), ProviderContractError> {
    bounded_count(dependencies.len(), MAX_SUPPORT_DEPENDENCIES)?;
    let mut keys = BTreeSet::new();
    let mut bytes = 0;
    for dependency in dependencies {
        bounded_count(dependency.scope.ordered_roots.len(), MAX_SEARCH_ROOTS)?;
        if let ProviderLookupOutcome::Consumed { candidates, .. } = &dependency.outcome {
            bounded_count(candidates.len(), MAX_LOOKUP_CANDIDATES)?;
        }
        checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
        validate_lookup_key(&dependency.key)?;
        validate_word(&dependency.scope.namespace)?;
        bounded_count(dependency.scope.ordered_roots.len(), MAX_SEARCH_ROOTS)?;
        let mut roots = BTreeSet::new();
        for root in &dependency.scope.ordered_roots {
            if !root.is_empty() {
                validate_path(root)?;
            }
            if !roots.insert(root) {
                return Err(ProviderContractError::SupportMismatch);
            }
        }
        require_nonzero_bytes(&dependency.scope.policy_identity)?;
        require_nonzero_bytes(&dependency.scope.effective_context)?;
        if dependency.scope.namespace.is_empty()
            || !keys.insert((&dependency.key, &dependency.scope))
        {
            return Err(ProviderContractError::SupportMismatch);
        }
        let revision = match dependency.outcome {
            ProviderLookupOutcome::Consumed { revision, .. } => revision,
            ProviderLookupOutcome::Absent { closed_universe } => closed_universe,
            ProviderLookupOutcome::Incomplete { observed_universe } => observed_universe,
        };
        if let ProviderLookupOutcome::Consumed { candidates, .. } = &dependency.outcome {
            bounded_count(candidates.len(), MAX_LOOKUP_CANDIDATES)?;
            let mut unique = BTreeSet::new();
            for candidate in candidates {
                require_nonzero_bytes(candidate)?;
                if !unique.insert(candidate) {
                    return Err(ProviderContractError::SupportMismatch);
                }
            }
        }
        require_nonzero_bytes(&revision)?;
        if matches!(dependency.outcome, ProviderLookupOutcome::Absent { .. })
            && dependency.scope.ordered_roots.is_empty()
        {
            return Err(ProviderContractError::SupportMismatch);
        }
    }
    Ok(())
}

fn validate_lookup_key(key: &ProviderLookupKey) -> Result<(), ProviderContractError> {
    match key {
        ProviderLookupKey::SourceBytes { file_id } => require_nonzero_bytes(file_id),
        ProviderLookupKey::Configuration { relative_path }
        | ProviderLookupKey::BuildInput { relative_path } => validate_path(relative_path),
        ProviderLookupKey::ProducerRelease | ProviderLookupKey::EffectiveContextInputs => Ok(()),
        ProviderLookupKey::PositiveFact {
            relation,
            entity_id,
        } => {
            super::ProviderRelationIdentity::try_new(relation.as_str())?;
            require_nonzero_bytes(entity_id)
        }
        ProviderLookupKey::Import { qualified_name }
        | ProviderLookupKey::Export { qualified_name } => validate_word(qualified_name),
        ProviderLookupKey::Name { name } => validate_word(name),
        ProviderLookupKey::Member {
            receiver_type,
            name,
        } => {
            require_nonzero_bytes(receiver_type)?;
            validate_word(name)
        }
        ProviderLookupKey::Implementation {
            trait_id,
            receiver_type,
        } => {
            require_nonzero_bytes(trait_id)?;
            require_nonzero_bytes(receiver_type)
        }
        ProviderLookupKey::CallableSummary { callable_id } => require_nonzero_bytes(callable_id),
        ProviderLookupKey::GraphProjection { projection_id } => {
            require_nonzero_bytes(projection_id)
        }
    }
}

fn dependency_memory_bytes(
    dependency: &ProviderSupportDependency,
) -> Result<usize, ProviderContractError> {
    let mut bytes = std::mem::size_of::<ProviderSupportDependency>();
    checked_bytes(&mut bytes, dependency.scope.namespace.capacity())?;
    checked_bytes(
        &mut bytes,
        dependency
            .scope
            .ordered_roots
            .capacity()
            .checked_mul(std::mem::size_of::<Vec<u8>>())
            .ok_or(ProviderContractError::ResourceOverflow)?,
    )?;
    for root in &dependency.scope.ordered_roots {
        checked_bytes(&mut bytes, root.capacity())?;
    }
    let key_bytes = match &dependency.key {
        ProviderLookupKey::Configuration { relative_path }
        | ProviderLookupKey::BuildInput { relative_path } => relative_path.capacity(),
        ProviderLookupKey::PositiveFact { relation, .. } => relation.capacity(),
        ProviderLookupKey::Import { qualified_name }
        | ProviderLookupKey::Export { qualified_name } => qualified_name.capacity(),
        ProviderLookupKey::Name { name } | ProviderLookupKey::Member { name, .. } => {
            name.capacity()
        }
        ProviderLookupKey::SourceBytes { .. }
        | ProviderLookupKey::ProducerRelease
        | ProviderLookupKey::EffectiveContextInputs
        | ProviderLookupKey::Implementation { .. }
        | ProviderLookupKey::CallableSummary { .. }
        | ProviderLookupKey::GraphProjection { .. } => 0,
    };
    checked_bytes(&mut bytes, key_bytes)?;
    if let ProviderLookupOutcome::Consumed { candidates, .. } = &dependency.outcome {
        checked_bytes(
            &mut bytes,
            candidates
                .capacity()
                .checked_mul(16)
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )?;
    }
    Ok(bytes)
}

/// Stable identity of a validated dependency set; root order remains semantically significant.
///
/// # Errors
/// Rejects malformed, contradictory or resource-unbounded dependencies.
pub fn provider_support_bundle_identity(
    dependencies: &[ProviderSupportDependency],
) -> Result<[u8; 32], ProviderContractError> {
    validate_dependencies(dependencies)?;
    let mut ordered = dependencies.iter().collect::<Vec<_>>();
    ordered.sort();
    let value = serde_json::json!({"profile": "codefabric.provider-support-bundle.v1", "dependencies": ordered});
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|_| ProviderContractError::SupportMismatch)?;
    Ok(*blake3::hash(&canonical).as_bytes())
}

pub(super) fn effective_context_input_identity(
    context: &super::ProviderContextBinding,
) -> Result<[u8; 32], ProviderContractError> {
    let mut bytes = module_memory_bytes(&context.modules)?;
    for dependency in context.support_obligations() {
        checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
    }
    let mut modules = context.modules.iter().collect::<Vec<_>>();
    modules.sort_by_key(|module| module.file_id);
    let support = provider_support_bundle_identity(context.support_obligations())?;
    let value = serde_json::json!({
        "profile": "codefabric.effective-provider-context-inputs.v1",
        "canonical_context": context.analysis_context_id(),
        "context_manifest": context.context_fingerprint(),
        "semantic_environment": context.semantic_environment_id(),
        "modules": modules, "python_version": context.python_version(), "support_bundle": support,
    });
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|_| ProviderContractError::SupportMismatch)?;
    Ok(*blake3::hash(&canonical).as_bytes())
}

pub(super) fn context_memory_bytes(
    context: &super::ProviderContextBinding,
) -> Result<u64, ProviderContractError> {
    let mut bytes = std::mem::size_of::<super::ProviderContextBinding>();
    checked_bytes(&mut bytes, context.identity.as_str().len())?;
    checked_bytes(&mut bytes, module_memory_bytes(&context.modules)?)?;
    for dependency in context.support_obligations() {
        checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
    }
    Ok(bytes as u64)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderSupportOwner {
    File([u8; 16]),
    Entity([u8; 16]),
    Context([u8; 16]),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderPartitionKey {
    pub owner: ProviderSupportOwner,
    pub family: ProviderFamilyIdentity,
    pub context_id: [u8; 16],
    pub producer_release: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderPartitionAction {
    Replace,
    Withdraw,
}

/// Replacement applies to facts, unknowns, diagnostics, coverage, dependencies and summaries.
/// A zero-row replacement still carries this key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderPartitionObligation {
    pub key: ProviderPartitionKey,
    pub action: ProviderPartitionAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPartitionSupport {
    pub partition: ProviderPartitionObligation,
    pub dependencies: Vec<ProviderSupportDependency>,
    /// False means provider-private support is unavailable: the entire context must invalidate.
    pub support_complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRunSupport {
    /// Shared once per run, not copied into every family/owner replacement.
    pub common_dependencies: Arc<[ProviderSupportDependency]>,
    pub partitions: Vec<ProviderPartitionSupport>,
}

impl super::ProviderRunProvenance {
    /// The producer is the selected behavior-bearing program as well as the library build.
    ///
    /// # Errors
    /// Propagates failure at the existing canonical JSON boundary.
    pub fn producer_release_identity(&self) -> Result<[u8; 32], ProviderContractError> {
        let value = serde_json::json!({
            "profile": "codefabric.provider-producer-release.v1",
            "build": self.provider_build().as_str(),
            "program": self.program().as_str(),
            "policy": self.policy().as_str(),
        });
        let canonical = crate::contracts::jcs::canonicalize_value(&value)
            .map_err(|_| ProviderContractError::SupportMismatch)?;
        Ok(*blake3::hash(&canonical).as_bytes())
    }
}

pub(super) fn validate_job_input_bounds(
    spec: &super::ProviderJobSpec,
) -> Result<(), ProviderContractError> {
    let (selected, withdrawn, source_bytes) = match spec.source.selection() {
        ProviderSourceSelection::File { .. } => (1, 0, 0),
        ProviderSourceSelection::Inventory(inventory) => {
            let bytes = inventory.members().iter().try_fold(0_u64, |sum, member| {
                let length = match member.disposition {
                    ProviderInputDisposition::Captured { byte_length, .. } => byte_length,
                    _ => 0,
                };
                sum.checked_add(length)
                    .ok_or(ProviderContractError::ResourceOverflow)
            })?;
            (
                inventory.selected_files().count(),
                inventory.withdrawn().len() + 1,
                bytes,
            )
        }
    };
    let owners = selected
        .checked_add(withdrawn)
        .ok_or(ProviderContractError::ResourceOverflow)?;
    let partitions = owners
        .checked_mul(spec.requests.len())
        .ok_or(ProviderContractError::ResourceOverflow)?;
    bounded_count(partitions, MAX_SUPPORT_PARTITIONS)?;
    let common_count = spec
        .context
        .support_obligations()
        .len()
        .checked_add(selected)
        .and_then(|count| count.checked_add(2))
        .ok_or(ProviderContractError::ResourceOverflow)?;
    bounded_count(common_count, MAX_SUPPORT_DEPENDENCIES)?;
    let work = partitions
        .checked_add(common_count)
        .ok_or(ProviderContractError::ResourceOverflow)?;
    if source_bytes > spec.ceilings.max_input_bytes()
        || work as u64 > spec.ceilings.max_work_units()
    {
        return Err(ProviderContractError::ResourceCeilingExceeded);
    }
    let mut bytes = std::mem::size_of::<ProviderRunSupport>()
        .checked_add(
            partitions
                .checked_mul(std::mem::size_of::<ProviderPartitionSupport>() + 67)
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )
        .ok_or(ProviderContractError::ResourceOverflow)?;
    for dependency in spec.context.support_obligations() {
        checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
    }
    let source_size = std::mem::size_of::<ProviderSupportDependency>()
        + spec.context.identity().as_str().len()
        + 16;
    checked_bytes(
        &mut bytes,
        (selected + 2)
            .checked_mul(source_size)
            .ok_or(ProviderContractError::ResourceOverflow)?,
    )?;
    if bytes as u64 > spec.ceilings.max_bytes() {
        return Err(ProviderContractError::ResourceCeilingExceeded);
    }
    Ok(())
}

impl ProviderJob {
    #[must_use]
    pub fn replacement_obligations(&self) -> Vec<ProviderPartitionObligation> {
        let mut owners = Vec::new();
        match self.source().selection() {
            ProviderSourceSelection::File { file_id, .. } => owners.push((
                ProviderSupportOwner::File(*file_id),
                ProviderPartitionAction::Replace,
            )),
            ProviderSourceSelection::Inventory(inventory) => {
                owners.extend(inventory.selected_files().map(|(file_id, _)| {
                    (
                        ProviderSupportOwner::File(file_id),
                        ProviderPartitionAction::Replace,
                    )
                }));
                owners.extend(inventory.withdrawn().iter().map(|(file_id, _)| {
                    (
                        ProviderSupportOwner::File(*file_id),
                        ProviderPartitionAction::Withdraw,
                    )
                }));
                // Even genesis-empty has a context partition and a schema-bearing zero-row result.
                owners.push((
                    ProviderSupportOwner::Context(self.context().analysis_context_id()),
                    ProviderPartitionAction::Replace,
                ));
            }
        }
        let mut obligations = Vec::with_capacity(owners.len() * self.requests().len());
        let release = format!(
            "b3:{}",
            blake3::Hash::from(self.producer_release_identity()).to_hex()
        );
        for request in self.requests() {
            for (owner, action) in &owners {
                obligations.push(ProviderPartitionObligation {
                    key: ProviderPartitionKey {
                        owner: owner.clone(),
                        family: request.family().clone(),
                        context_id: self.context().analysis_context_id(),
                        producer_release: release.clone(),
                    },
                    action: *action,
                });
            }
        }
        obligations.sort();
        obligations
    }
}

impl ProviderRunSupport {
    /// Conservatively expose known job inputs while declaring provider-private support unknown.
    /// This is not narrow-reuse authority; admission carries its whole-context invalidation bound.
    #[must_use]
    pub fn conservative(job: &ProviderJob) -> Self {
        Self::from_job_inputs(job, false)
    }

    /// Complete support is permitted only for a provider whose consumed inputs are exactly the job.
    #[must_use]
    pub fn from_job_inputs(job: &ProviderJob, support_complete: bool) -> Self {
        let mut dependencies = job.context().support_obligations().to_vec();
        let scope = ProviderSearchScope {
            namespace: job.context().identity().as_str().as_bytes().to_vec(),
            ordered_roots: Vec::new(),
            policy_identity: *blake3::hash(job.provenance().policy().as_str().as_bytes())
                .as_bytes(),
            effective_context: job.context().context_fingerprint(),
        };
        let sources = match job.source().selection() {
            ProviderSourceSelection::File {
                file_id,
                content_digest,
            } => vec![(*file_id, *content_digest)],
            ProviderSourceSelection::Inventory(inventory) => inventory.selected_files().collect(),
        };
        for (file_id, revision) in sources {
            dependencies.push(ProviderSupportDependency {
                key: ProviderLookupKey::SourceBytes { file_id },
                scope: scope.clone(),
                outcome: ProviderLookupOutcome::Consumed {
                    revision,
                    candidates: vec![file_id],
                },
            });
        }
        dependencies.push(ProviderSupportDependency {
            key: ProviderLookupKey::ProducerRelease,
            scope: scope.clone(),
            outcome: ProviderLookupOutcome::Consumed {
                revision: job.producer_release_identity(),
                candidates: Vec::new(),
            },
        });
        dependencies.push(ProviderSupportDependency {
            key: ProviderLookupKey::EffectiveContextInputs,
            scope,
            outcome: ProviderLookupOutcome::Consumed {
                revision: job.context().effective_input_identity(),
                candidates: Vec::new(),
            },
        });
        Self {
            common_dependencies: dependencies.into(),
            partitions: job
                .replacement_obligations()
                .into_iter()
                .map(|partition| ProviderPartitionSupport {
                    partition,
                    dependencies: Vec::new(),
                    support_complete,
                })
                .collect(),
        }
    }

    pub(super) fn validate_for_job(&self, job: &ProviderJob) -> Result<(), ProviderContractError> {
        let bytes = self.memory_bytes()?;
        let work = self.partitions.iter().try_fold(
            self.partitions.len() + self.common_dependencies.len(),
            |work, partition| {
                work.checked_add(partition.dependencies.len())
                    .ok_or(ProviderContractError::ResourceOverflow)
            },
        )?;
        if bytes > job.ceilings().max_bytes() || work as u64 > job.ceilings().max_work_units() {
            return Err(ProviderContractError::ResourceCeilingExceeded);
        }
        self.validate_consumed_sources(job)?;
        let expected = job.replacement_obligations();
        let actual = self
            .partitions
            .iter()
            .map(|support| &support.partition)
            .collect::<BTreeSet<_>>();
        if actual.len() != self.partitions.len() || actual.into_iter().ne(expected.iter()) {
            return Err(ProviderContractError::SupportMismatch);
        }
        let required = Self::conservative(job);
        let common = self.common_dependencies.iter().collect::<BTreeSet<_>>();
        if required
            .common_dependencies
            .iter()
            .any(|dependency| !common.contains(dependency))
        {
            return Err(ProviderContractError::SupportMismatch);
        }
        let known_scopes = required
            .common_dependencies
            .iter()
            .map(|dependency| &dependency.scope)
            .collect::<BTreeSet<_>>();
        let known_policies = known_scopes
            .iter()
            .map(|scope| scope.policy_identity)
            .collect::<BTreeSet<_>>();
        let known_absences = required
            .common_dependencies
            .iter()
            .filter_map(|dependency| match dependency.outcome {
                ProviderLookupOutcome::Absent { closed_universe } => {
                    Some((&dependency.scope, closed_universe))
                }
                ProviderLookupOutcome::Consumed { .. }
                | ProviderLookupOutcome::Incomplete { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let validate = |dependency: &ProviderSupportDependency| {
            if dependency.scope.effective_context != job.context().context_fingerprint()
                || !known_policies.contains(&dependency.scope.policy_identity)
            {
                return Err(ProviderContractError::SupportMismatch);
            }
            if matches!(dependency.outcome, ProviderLookupOutcome::Incomplete { .. }) {
                return Ok(());
            }
            if !known_scopes.contains(&dependency.scope) {
                return Err(ProviderContractError::SupportMismatch);
            }
            if let ProviderLookupOutcome::Absent { closed_universe } = dependency.outcome
                && !known_absences.contains(&(&dependency.scope, closed_universe))
            {
                return Err(ProviderContractError::SupportMismatch);
            }
            Ok(())
        };
        for dependency in self.common_dependencies.iter() {
            validate(dependency)?;
        }
        let common_keys = self
            .common_dependencies
            .iter()
            .map(|dependency| (&dependency.key, &dependency.scope))
            .collect::<BTreeSet<_>>();
        for support in &self.partitions {
            for dependency in &support.dependencies {
                if common_keys.contains(&(&dependency.key, &dependency.scope)) {
                    return Err(ProviderContractError::SupportMismatch);
                }
                validate(dependency)?;
            }
        }
        Ok(())
    }

    /// Index only already resource-bounded support, not an expanded copy of the inventory.
    fn validate_consumed_sources(&self, job: &ProviderJob) -> Result<(), ProviderContractError> {
        let mut consumed = BTreeMap::new();
        for dependency in self.common_dependencies.iter().chain(
            self.partitions
                .iter()
                .flat_map(|partition| &partition.dependencies),
        ) {
            if let ProviderLookupKey::SourceBytes { file_id } = dependency.key {
                match dependency.outcome {
                    ProviderLookupOutcome::Consumed { revision, .. } => {
                        if consumed
                            .insert(file_id, revision)
                            .is_some_and(|old| old != revision)
                        {
                            return Err(ProviderContractError::SupportMismatch);
                        }
                    }
                    ProviderLookupOutcome::Incomplete { .. } => {}
                    // Unavailable source bytes are incomplete input, never closed absence.
                    ProviderLookupOutcome::Absent { .. } => {
                        return Err(ProviderContractError::SupportMismatch);
                    }
                }
            }
        }
        let mut observe = |file_id, digest| {
            if consumed
                .remove(&file_id)
                .is_some_and(|revision| revision != digest)
            {
                return Err(ProviderContractError::SupportMismatch);
            }
            Ok(())
        };
        match job.source().selection() {
            ProviderSourceSelection::File {
                file_id,
                content_digest,
            } => observe(*file_id, *content_digest)?,
            ProviderSourceSelection::Inventory(inventory) => {
                for member in inventory.members() {
                    if let ProviderInputDisposition::Captured {
                        file_id, digest, ..
                    } = member.disposition
                    {
                        observe(file_id, digest)?;
                    }
                }
            }
        }
        if !consumed.is_empty() {
            return Err(ProviderContractError::SupportMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub fn common_dependencies(&self) -> &[ProviderSupportDependency] {
        &self.common_dependencies
    }

    pub fn dependencies_for<'a>(
        &'a self,
        partition: &'a ProviderPartitionSupport,
    ) -> impl Iterator<Item = &'a ProviderSupportDependency> {
        self.common_dependencies
            .iter()
            .chain(partition.dependencies.iter())
    }

    /// Owned dependency buffers are charged once; common Arrow projections may share this set.
    ///
    /// # Errors
    /// Rejects malformed support and count/byte overflow before admission or further allocation.
    pub fn memory_bytes(&self) -> Result<u64, ProviderContractError> {
        bounded_count(self.partitions.len(), MAX_SUPPORT_PARTITIONS)?;
        validate_dependencies(&self.common_dependencies)?;
        let mut bytes = std::mem::size_of::<Self>();
        checked_bytes(
            &mut bytes,
            self.partitions
                .capacity()
                .checked_mul(std::mem::size_of::<ProviderPartitionSupport>())
                .ok_or(ProviderContractError::ResourceOverflow)?,
        )?;
        for dependency in self.common_dependencies.iter() {
            checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
        }
        let mut count = self.common_dependencies.len();
        for partition in &self.partitions {
            count = count
                .checked_add(partition.dependencies.len())
                .ok_or(ProviderContractError::ResourceOverflow)?;
            bounded_count(count, MAX_SUPPORT_DEPENDENCIES)?;
            validate_dependencies(&partition.dependencies)?;
            validate_word(partition.partition.key.producer_release.as_bytes())?;
            checked_bytes(
                &mut bytes,
                partition.partition.key.producer_release.capacity(),
            )?;
            checked_bytes(
                &mut bytes,
                (partition.dependencies.capacity() - partition.dependencies.len())
                    .checked_mul(std::mem::size_of::<ProviderSupportDependency>())
                    .ok_or(ProviderContractError::ResourceOverflow)?,
            )?;
            for dependency in &partition.dependencies {
                checked_bytes(&mut bytes, dependency_memory_bytes(dependency)?)?;
            }
        }
        Ok(bytes as u64)
    }

    /// Incomplete private/API support or incomplete searches never authorize a narrow cache hit.
    #[must_use]
    pub fn requires_context_invalidation(&self) -> bool {
        self.partitions.is_empty()
            || self.common_dependencies.iter().any(|dependency| {
                matches!(dependency.outcome, ProviderLookupOutcome::Incomplete { .. })
            })
            || self.partitions.iter().any(|support| {
                !support.support_complete
                    || support.dependencies.iter().any(|dependency| {
                        matches!(dependency.outcome, ProviderLookupOutcome::Incomplete { .. })
                    })
            })
    }
}
