use std::collections::{BTreeMap, BTreeSet};

use super::{ModelError, require_identifier};

/// One exact dependency or toolchain input to a compiler release.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CompilerDependency {
    name: String,
    identity: String,
}

impl CompilerDependency {
    pub fn new(name: impl Into<String>, identity: impl Into<String>) -> Result<Self, ModelError> {
        let name = name.into();
        let identity = identity.into();
        require_identifier(&name, "compiler dependency name")?;
        if identity.trim().is_empty() || identity.len() > 1024 {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "dependency {name} has an empty or unbounded identity"
            )));
        }
        Ok(Self { name, identity })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

/// Every executable input that makes a replay release-specific.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricCompilerRelease {
    release_id: String,
    source_identity: String,
    build_identity: String,
    metamodel_abi: u32,
    reducer_abi: u32,
    logical_algebra_abi: u32,
    intrinsic_package_id: String,
    dependencies: BTreeMap<String, CompilerDependency>,
    provider_schema_versions: BTreeMap<String, String>,
    policy_schema_identity: String,
    effective_configuration_identity: String,
    toolchains: BTreeMap<String, String>,
    released_wire_contracts: BTreeSet<String>,
}

impl FabricCompilerRelease {
    pub fn builder(
        release_id: impl Into<String>,
        source_identity: impl Into<String>,
        build_identity: impl Into<String>,
    ) -> FabricCompilerReleaseBuilder {
        FabricCompilerReleaseBuilder {
            release_id: release_id.into(),
            source_identity: source_identity.into(),
            build_identity: build_identity.into(),
            metamodel_abi: None,
            reducer_abi: None,
            logical_algebra_abi: None,
            intrinsic_package_id: None,
            dependencies: BTreeMap::new(),
            provider_schema_versions: BTreeMap::new(),
            policy_schema_identity: None,
            effective_configuration_identity: None,
            toolchains: BTreeMap::new(),
            released_wire_contracts: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    #[must_use]
    pub fn source_identity(&self) -> &str {
        &self.source_identity
    }

    #[must_use]
    pub fn build_identity(&self) -> &str {
        &self.build_identity
    }

    #[must_use]
    pub const fn metamodel_abi(&self) -> u32 {
        self.metamodel_abi
    }

    #[must_use]
    pub const fn reducer_abi(&self) -> u32 {
        self.reducer_abi
    }

    #[must_use]
    pub const fn logical_algebra_abi(&self) -> u32 {
        self.logical_algebra_abi
    }

    #[must_use]
    pub fn intrinsic_package_id(&self) -> &str {
        &self.intrinsic_package_id
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<String, CompilerDependency> {
        &self.dependencies
    }

    #[must_use]
    pub fn provider_schema_versions(&self) -> &BTreeMap<String, String> {
        &self.provider_schema_versions
    }

    #[must_use]
    pub fn policy_schema_identity(&self) -> &str {
        &self.policy_schema_identity
    }

    #[must_use]
    pub fn effective_configuration_identity(&self) -> &str {
        &self.effective_configuration_identity
    }

    #[must_use]
    pub fn toolchains(&self) -> &BTreeMap<String, String> {
        &self.toolchains
    }

    #[must_use]
    pub fn released_wire_contracts(&self) -> &BTreeSet<String> {
        &self.released_wire_contracts
    }
}

/// Construction boundary for an immutable [`FabricCompilerRelease`].
#[derive(Clone, Debug)]
pub struct FabricCompilerReleaseBuilder {
    release_id: String,
    source_identity: String,
    build_identity: String,
    metamodel_abi: Option<u32>,
    reducer_abi: Option<u32>,
    logical_algebra_abi: Option<u32>,
    intrinsic_package_id: Option<String>,
    dependencies: BTreeMap<String, CompilerDependency>,
    provider_schema_versions: BTreeMap<String, String>,
    policy_schema_identity: Option<String>,
    effective_configuration_identity: Option<String>,
    toolchains: BTreeMap<String, String>,
    released_wire_contracts: BTreeSet<String>,
}

impl FabricCompilerReleaseBuilder {
    #[must_use]
    pub const fn with_abis(
        mut self,
        metamodel_abi: u32,
        reducer_abi: u32,
        logical_algebra_abi: u32,
    ) -> Self {
        self.metamodel_abi = Some(metamodel_abi);
        self.reducer_abi = Some(reducer_abi);
        self.logical_algebra_abi = Some(logical_algebra_abi);
        self
    }

    #[must_use]
    pub fn with_intrinsic_package(mut self, package_id: impl Into<String>) -> Self {
        self.intrinsic_package_id = Some(package_id.into());
        self
    }

    pub fn add_dependency(
        mut self,
        name: impl Into<String>,
        identity: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let dependency = CompilerDependency::new(name, identity)?;
        if self
            .dependencies
            .insert(dependency.name.clone(), dependency)
            .is_some()
        {
            return Err(ModelError::InvalidCompilerRelease(
                "duplicate dependency identity".into(),
            ));
        }
        Ok(self)
    }

    pub fn add_provider_schema(
        mut self,
        provider_id: impl Into<String>,
        schema_identity: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let provider_id = provider_id.into();
        let schema_identity = schema_identity.into();
        require_identifier(&provider_id, "provider identity")?;
        if schema_identity.trim().is_empty()
            || self
                .provider_schema_versions
                .insert(provider_id.clone(), schema_identity)
                .is_some()
        {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "provider {provider_id} has an empty or duplicate schema identity"
            )));
        }
        Ok(self)
    }

    #[must_use]
    pub fn with_policy_and_configuration(
        mut self,
        policy_schema_identity: impl Into<String>,
        effective_configuration_identity: impl Into<String>,
    ) -> Self {
        self.policy_schema_identity = Some(policy_schema_identity.into());
        self.effective_configuration_identity = Some(effective_configuration_identity.into());
        self
    }

    pub fn add_toolchain(
        mut self,
        name: impl Into<String>,
        identity: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let name = name.into();
        let identity = identity.into();
        require_identifier(&name, "toolchain name")?;
        if identity.trim().is_empty() || self.toolchains.insert(name.clone(), identity).is_some() {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "toolchain {name} has an empty or duplicate identity"
            )));
        }
        Ok(self)
    }

    pub fn add_wire_contract(mut self, artifact_id: impl Into<String>) -> Result<Self, ModelError> {
        let artifact_id = artifact_id.into();
        require_identifier(&artifact_id, "released wire contract")?;
        if !self.released_wire_contracts.insert(artifact_id.clone()) {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "duplicate released wire contract {artifact_id}"
            )));
        }
        Ok(self)
    }

    pub fn build(self) -> Result<FabricCompilerRelease, ModelError> {
        require_identifier(&self.release_id, "compiler release")?;
        if self.source_identity.trim().is_empty() || self.build_identity.trim().is_empty() {
            return Err(ModelError::InvalidCompilerRelease(
                "source and build identities are required".into(),
            ));
        }
        let metamodel_abi = self
            .metamodel_abi
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                ModelError::InvalidCompilerRelease("metamodel ABI must be non-zero".into())
            })?;
        let reducer_abi = self.reducer_abi.filter(|value| *value > 0).ok_or_else(|| {
            ModelError::InvalidCompilerRelease("reducer ABI must be non-zero".into())
        })?;
        let logical_algebra_abi = self
            .logical_algebra_abi
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                ModelError::InvalidCompilerRelease("logical algebra ABI must be non-zero".into())
            })?;
        let intrinsic_package_id = self.intrinsic_package_id.ok_or_else(|| {
            ModelError::InvalidCompilerRelease("intrinsic package identity is required".into())
        })?;
        require_identifier(&intrinsic_package_id, "intrinsic package")?;
        let policy_schema_identity = self.policy_schema_identity.ok_or_else(|| {
            ModelError::InvalidCompilerRelease("policy schema identity is required".into())
        })?;
        let effective_configuration_identity =
            self.effective_configuration_identity.ok_or_else(|| {
                ModelError::InvalidCompilerRelease(
                    "effective configuration identity is required".into(),
                )
            })?;
        if policy_schema_identity.trim().is_empty()
            || effective_configuration_identity.trim().is_empty()
            || self.dependencies.is_empty()
            || self.provider_schema_versions.is_empty()
            || self.toolchains.is_empty()
            || self.released_wire_contracts.is_empty()
        {
            return Err(ModelError::InvalidCompilerRelease(
                "dependency, provider, policy, configuration, toolchain, and wire closure is required"
                    .into(),
            ));
        }
        Ok(FabricCompilerRelease {
            release_id: self.release_id,
            source_identity: self.source_identity,
            build_identity: self.build_identity,
            metamodel_abi,
            reducer_abi,
            logical_algebra_abi,
            intrinsic_package_id,
            dependencies: self.dependencies,
            provider_schema_versions: self.provider_schema_versions,
            policy_schema_identity,
            effective_configuration_identity,
            toolchains: self.toolchains,
            released_wire_contracts: self.released_wire_contracts,
        })
    }
}

/// Closed primitive implementations installed by this compiler binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum IntrinsicPrimitive {
    Project,
    Filter,
    Join,
    Aggregate,
    Window,
    Union,
    RecursiveQuery,
    ScalarFunction,
    AggregateFunction,
    WindowFunction,
    TableFunction,
    LogicalExtension,
    PhysicalExtension,
}

impl IntrinsicPrimitive {
    pub const ALL: [Self; 13] = [
        Self::Project,
        Self::Filter,
        Self::Join,
        Self::Aggregate,
        Self::Window,
        Self::Union,
        Self::RecursiveQuery,
        Self::ScalarFunction,
        Self::AggregateFunction,
        Self::WindowFunction,
        Self::TableFunction,
        Self::LogicalExtension,
        Self::PhysicalExtension,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Project => "rel.project",
            Self::Filter => "rel.filter",
            Self::Join => "rel.join",
            Self::Aggregate => "rel.aggregate",
            Self::Window => "rel.window",
            Self::Union => "rel.union",
            Self::RecursiveQuery => "rel.recursive_query",
            Self::ScalarFunction => "fn.scalar",
            Self::AggregateFunction => "fn.aggregate",
            Self::WindowFunction => "fn.window",
            Self::TableFunction => "fn.table",
            Self::LogicalExtension => "ext.logical",
            Self::PhysicalExtension => "ext.physical",
        }
    }

    #[must_use]
    pub const fn signature(self) -> &'static str {
        match self {
            Self::Project => "relation,expression[]->relation",
            Self::Filter => "relation,predicate->relation",
            Self::Join => "relation,relation,predicate,join_kind->relation",
            Self::Aggregate => "relation,group[],aggregate[]->relation",
            Self::Window => "relation,window_expression[]->relation",
            Self::Union => "relation[]->relation",
            Self::RecursiveQuery => "seed,recursive_term,key,bounds->relation",
            Self::ScalarFunction => "scalar[]->scalar",
            Self::AggregateFunction => "scalar[]->scalar",
            Self::WindowFunction => "relation,window_frame->scalar",
            Self::TableFunction => "scalar[]->relation",
            Self::LogicalExtension => "typed_logical_inputs->relation",
            Self::PhysicalExtension => "typed_physical_inputs->stream",
        }
    }

    #[must_use]
    pub const fn semantic_level(self) -> &'static str {
        match self {
            Self::Project
            | Self::Filter
            | Self::Join
            | Self::Aggregate
            | Self::Window
            | Self::Union
            | Self::RecursiveQuery => "logical-plan",
            Self::ScalarFunction
            | Self::AggregateFunction
            | Self::WindowFunction
            | Self::TableFunction => "function",
            Self::LogicalExtension => "logical-extension",
            Self::PhysicalExtension => "physical-extension",
        }
    }
}

/// One row derived from the exact installer, never an authored model declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledIntrinsic {
    pub primitive: IntrinsicPrimitive,
    pub primitive_id: &'static str,
    pub signature: &'static str,
    pub semantic_level: &'static str,
    pub implementation_id: String,
    pub package_id: String,
}

/// Exact executable primitive package linked into a replay engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntrinsicInstaller {
    package_id: String,
    implementation_release: String,
}

impl IntrinsicInstaller {
    pub fn new(
        package_id: impl Into<String>,
        implementation_release: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let package_id = package_id.into();
        let implementation_release = implementation_release.into();
        require_identifier(&package_id, "intrinsic installer package")?;
        require_identifier(&implementation_release, "intrinsic implementation release")?;
        Ok(Self {
            package_id,
            implementation_release,
        })
    }

    #[must_use]
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    #[must_use]
    pub fn install(&self) -> Vec<InstalledIntrinsic> {
        IntrinsicPrimitive::ALL
            .into_iter()
            .map(|primitive| InstalledIntrinsic {
                primitive,
                primitive_id: primitive.id(),
                signature: primitive.signature(),
                semantic_level: primitive.semantic_level(),
                implementation_id: format!("{}:{}", self.implementation_release, primitive.id()),
                package_id: self.package_id.clone(),
            })
            .collect()
    }
}
