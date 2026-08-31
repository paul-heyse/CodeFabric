use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, StringBuilder, UInt32Builder, UInt64Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{Field, Schema};

use super::release::{FabricCompilerRelease, InstalledIntrinsic, IntrinsicInstaller};
use super::schema::{
    BootstrapMetamodel, ModelRelation, ModelRow, ModelRowBuilder, ModelValue, ScalarType,
};
use super::{ModelDataRow, ModelDataRowReference, ModelError, require_identifier};

type RowKey = Vec<ModelValue>;

/// A typed reference to an active row's complete primary key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowReference {
    relation: ModelRelation,
    key: RowKey,
}

impl RowReference {
    #[must_use]
    pub fn new(relation: ModelRelation, key: impl IntoIterator<Item = ModelValue>) -> Self {
        Self {
            relation,
            key: key.into_iter().collect(),
        }
    }

    pub fn for_row(row: &ModelRow, metamodel: &BootstrapMetamodel) -> Result<Self, ModelError> {
        Ok(Self {
            relation: row.relation(),
            key: metamodel.row_key(row)?,
        })
    }

    #[must_use]
    pub const fn relation(&self) -> ModelRelation {
        self.relation
    }

    #[must_use]
    pub fn key(&self) -> &[ModelValue] {
        &self.key
    }
}

/// An immutable reducer operation. Existing event values are never changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelOperation {
    Add(ModelRow),
    Supersede {
        prior: RowReference,
        replacement: ModelRow,
    },
    Retire(RowReference),
    /// Add a row to a relation whose schema is described by the replayed model.
    AddData(ModelDataRow),
    /// Replace one active model-data row without mutating its historical event.
    SupersedeData {
        prior: ModelDataRowReference,
        replacement: ModelDataRow,
    },
    /// Retire one active model-data row by its complete primary key.
    RetireData(ModelDataRowReference),
}

/// Accountable semantic judgment carried by a migration event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDecision {
    decision_id: String,
    owner: String,
    applicability: String,
    rationale: String,
    operations: Vec<ModelOperation>,
}

impl ModelDecision {
    pub fn new(
        decision_id: impl Into<String>,
        owner: impl Into<String>,
        applicability: impl Into<String>,
        rationale: impl Into<String>,
        operations: Vec<ModelOperation>,
    ) -> Result<Self, ModelError> {
        let decision_id = decision_id.into();
        let owner = owner.into();
        let applicability = applicability.into();
        let rationale = rationale.into();
        require_identifier(&decision_id, "model decision")?;
        if owner.trim().is_empty()
            || applicability.trim().is_empty()
            || rationale.trim().is_empty()
            || operations.is_empty()
        {
            return Err(ModelError::InvalidMigrationChain(format!(
                "decision {decision_id} requires owner, applicability, rationale, and operations"
            )));
        }
        Ok(Self {
            decision_id,
            owner,
            applicability,
            rationale,
            operations,
        })
    }

    #[must_use]
    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    #[must_use]
    pub fn operations(&self) -> &[ModelOperation] {
        &self.operations
    }
}

/// One accepted append-only transition in the model chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelMigration {
    migration_id: String,
    predecessor_migration_id: Option<String>,
    predecessor_model_epoch_id: String,
    target_model_epoch_id: String,
    ordinal: u64,
    accepted_by: String,
    decisions: Vec<ModelDecision>,
}

impl ModelMigration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        migration_id: impl Into<String>,
        predecessor_migration_id: Option<String>,
        predecessor_model_epoch_id: impl Into<String>,
        target_model_epoch_id: impl Into<String>,
        ordinal: u64,
        accepted_by: impl Into<String>,
        decisions: Vec<ModelDecision>,
    ) -> Result<Self, ModelError> {
        let migration_id = migration_id.into();
        let predecessor_model_epoch_id = predecessor_model_epoch_id.into();
        let target_model_epoch_id = target_model_epoch_id.into();
        let accepted_by = accepted_by.into();
        require_identifier(&migration_id, "model migration")?;
        require_identifier(&predecessor_model_epoch_id, "predecessor model epoch")?;
        require_identifier(&target_model_epoch_id, "target model epoch")?;
        if let Some(predecessor) = &predecessor_migration_id {
            require_identifier(predecessor, "predecessor migration")?;
        }
        if ordinal == 0
            || accepted_by.trim().is_empty()
            || decisions.is_empty()
            || predecessor_model_epoch_id == target_model_epoch_id
        {
            return Err(ModelError::InvalidMigrationChain(format!(
                "migration {migration_id} has incomplete or self-referential identity"
            )));
        }
        let decision_ids: BTreeSet<_> = decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .collect();
        if decision_ids.len() != decisions.len() {
            return Err(ModelError::InvalidMigrationChain(format!(
                "migration {migration_id} has duplicate decision IDs"
            )));
        }
        Ok(Self {
            migration_id,
            predecessor_migration_id,
            predecessor_model_epoch_id,
            target_model_epoch_id,
            ordinal,
            accepted_by,
            decisions,
        })
    }

    #[must_use]
    pub fn migration_id(&self) -> &str {
        &self.migration_id
    }

    #[must_use]
    pub fn target_model_epoch_id(&self) -> &str {
        &self.target_model_epoch_id
    }
}

/// Explicit handoff between two exact compiler releases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerReleaseMigration {
    migration_id: String,
    from_release_id: String,
    to_release_id: String,
    source_model_epoch_id: String,
    migrations: Vec<ModelMigration>,
}

impl CompilerReleaseMigration {
    pub fn new(
        migration_id: impl Into<String>,
        from_release_id: impl Into<String>,
        to_release_id: impl Into<String>,
        source_model_epoch_id: impl Into<String>,
        migrations: Vec<ModelMigration>,
    ) -> Result<Self, ModelError> {
        let migration_id = migration_id.into();
        let from_release_id = from_release_id.into();
        let to_release_id = to_release_id.into();
        let source_model_epoch_id = source_model_epoch_id.into();
        for (value, context) in [
            (&migration_id, "compiler release migration"),
            (&from_release_id, "source compiler release"),
            (&to_release_id, "target compiler release"),
            (&source_model_epoch_id, "source model epoch"),
        ] {
            require_identifier(value, context)?;
        }
        if from_release_id == to_release_id || migrations.is_empty() {
            return Err(ModelError::ReleaseMigration(
                "release handoff requires distinct releases and explicit model migrations".into(),
            ));
        }
        Ok(Self {
            migration_id,
            from_release_id,
            to_release_id,
            source_model_epoch_id,
            migrations,
        })
    }
}

#[derive(Clone, Debug, Default)]
struct RelationState {
    active: BTreeMap<RowKey, ModelRow>,
    ever_seen: BTreeSet<RowKey>,
}

#[derive(Clone, Debug, Default)]
struct RelationStateData {
    active: BTreeMap<RowKey, ModelDataRow>,
    ever_seen: BTreeSet<RowKey>,
}

#[derive(Clone, Debug)]
struct ReplayState {
    relations: BTreeMap<ModelRelation, RelationState>,
    data_relations: BTreeMap<String, RelationStateData>,
    current_epoch_id: String,
    last_migration_id: Option<String>,
    migration_ordinal: u64,
}

/// Canonically ordered Arrow representation of every model relation.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRelations {
    batches: BTreeMap<ModelRelation, RecordBatch>,
    data_batches: BTreeMap<String, RecordBatch>,
}

impl ModelRelations {
    #[must_use]
    pub fn batch(&self, relation: ModelRelation) -> &RecordBatch {
        &self.batches[&relation]
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (ModelRelation, &RecordBatch)> {
        self.batches
            .iter()
            .map(|(relation, batch)| (*relation, batch))
    }

    /// Resolve one model-defined relation by its canonical relation ID.
    #[must_use]
    pub fn data_batch(&self, relation_id: &str) -> Option<&RecordBatch> {
        self.data_batches.get(relation_id)
    }

    /// Iterate canonically over all model-defined relation batches.
    #[must_use]
    pub fn data_iter(&self) -> impl ExactSizeIterator<Item = (&str, &RecordBatch)> {
        self.data_batches
            .iter()
            .map(|(relation_id, batch)| (relation_id.as_str(), batch))
    }

    /// Logical equality is schema and row equality, never digest equality.
    #[must_use]
    pub fn logically_equals(&self, other: &Self) -> bool {
        self == other
    }
}

/// Reconstructed immutable model epoch.
#[derive(Clone, Debug)]
pub struct ModelEpoch {
    compiler_release: FabricCompilerRelease,
    model_epoch_id: String,
    last_migration_id: Option<String>,
    migration_ordinal: u64,
    relations: ModelRelations,
    rows: Arc<BTreeMap<ModelRelation, Vec<ModelRow>>>,
    data_rows: Arc<BTreeMap<String, Vec<ModelDataRow>>>,
}

impl ModelEpoch {
    #[must_use]
    pub fn compiler_release(&self) -> &FabricCompilerRelease {
        &self.compiler_release
    }

    #[must_use]
    pub fn model_epoch_id(&self) -> &str {
        &self.model_epoch_id
    }

    #[must_use]
    pub fn relations(&self) -> &ModelRelations {
        &self.relations
    }

    /// Canonical content identity of the exact compiler release and replay result.
    ///
    /// The digest is an identity/pin only.  Correctness remains a row-level and
    /// executable-invariant question; callers must never treat digest equality as
    /// semantic acceptance.
    #[must_use]
    pub fn identity_pin(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hash_text(&mut hasher, "codefabric.model-epoch.v1");
        hash_compiler_release(&mut hasher, &self.compiler_release);
        for (relation, rows) in self.rows.as_ref() {
            hash_text(&mut hasher, relation.as_str());
            hash_len(&mut hasher, rows.len());
            for row in rows {
                hash_model_values(&mut hasher, row.values());
            }
        }
        for (relation_id, rows) in self.data_rows.as_ref() {
            hash_text(&mut hasher, relation_id);
            hash_len(&mut hasher, rows.len());
            for row in rows {
                hash_model_values(&mut hasher, row.values());
            }
        }
        *hasher.finalize().as_bytes()
    }
}

fn hash_compiler_release(hasher: &mut blake3::Hasher, release: &FabricCompilerRelease) {
    for value in [
        release.release_id(),
        release.source_identity(),
        release.build_identity(),
        release.intrinsic_package_id(),
        release.policy_schema_identity(),
        release.effective_configuration_identity(),
    ] {
        hash_text(hasher, value);
    }
    for value in [
        release.metamodel_abi(),
        release.reducer_abi(),
        release.logical_algebra_abi(),
    ] {
        hasher.update(&value.to_le_bytes());
    }
    hash_len(hasher, release.dependencies().len());
    for (name, dependency) in release.dependencies() {
        hash_text(hasher, name);
        hash_text(hasher, dependency.identity());
    }
    hash_text_map(hasher, release.provider_schema_versions());
    hash_text_map(hasher, release.toolchains());
    hash_len(hasher, release.released_wire_contracts().len());
    for contract in release.released_wire_contracts() {
        hash_text(hasher, contract);
    }
}

fn hash_text_map(hasher: &mut blake3::Hasher, values: &BTreeMap<String, String>) {
    hash_len(hasher, values.len());
    for (key, value) in values {
        hash_text(hasher, key);
        hash_text(hasher, value);
    }
}

fn hash_model_values(hasher: &mut blake3::Hasher, values: &BTreeMap<String, ModelValue>) {
    hash_len(hasher, values.len());
    for (field, value) in values {
        hash_text(hasher, field);
        match value {
            ModelValue::Null => {
                hasher.update(&[0]);
            }
            ModelValue::Bool(value) => {
                hasher.update(&[1, u8::from(*value)]);
            }
            ModelValue::UInt32(value) => {
                hasher.update(&[2]);
                hasher.update(&value.to_le_bytes());
            }
            ModelValue::UInt64(value) => {
                hasher.update(&[3]);
                hasher.update(&value.to_le_bytes());
            }
            ModelValue::Utf8(value) => {
                hasher.update(&[4]);
                hash_text(hasher, value);
            }
            ModelValue::Binary(value) => {
                hasher.update(&[5]);
                hash_len(hasher, value.len());
                hasher.update(value);
            }
        }
    }
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
}

/// Exact-release pure reducer from migration events to typed Arrow relations.
#[derive(Clone, Debug)]
pub struct ReplayEngine {
    compiler_release: FabricCompilerRelease,
    metamodel: BootstrapMetamodel,
    installer: IntrinsicInstaller,
}

impl ReplayEngine {
    pub fn new(
        compiler_release: FabricCompilerRelease,
        installer: IntrinsicInstaller,
    ) -> Result<Self, ModelError> {
        if compiler_release.metamodel_abi() != 1 {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "this binary implements metamodel ABI 1, not {}",
                compiler_release.metamodel_abi()
            )));
        }
        if compiler_release.reducer_abi() != 1 {
            return Err(ModelError::InvalidCompilerRelease(format!(
                "this binary implements reducer ABI 1, not {}",
                compiler_release.reducer_abi()
            )));
        }
        if compiler_release.intrinsic_package_id() != installer.package_id() {
            return Err(ModelError::InvalidIntrinsicInstallation(format!(
                "release package {} differs from installer {}",
                compiler_release.intrinsic_package_id(),
                installer.package_id()
            )));
        }
        Ok(Self {
            compiler_release,
            metamodel: BootstrapMetamodel::new(),
            installer,
        })
    }

    #[must_use]
    pub fn metamodel(&self) -> &BootstrapMetamodel {
        &self.metamodel
    }

    pub fn replay(&self, migrations: &[ModelMigration]) -> Result<ModelEpoch, ModelError> {
        let mut state = self.bootstrap_state()?;
        self.apply_migrations(&mut state, migrations)?;
        self.finish(state)
    }

    /// Migrate an epoch reconstructed by its old binary into this exact release.
    pub fn migrate_from(
        &self,
        previous: &ModelEpoch,
        release_migration: &CompilerReleaseMigration,
    ) -> Result<ModelEpoch, ModelError> {
        if previous.compiler_release.release_id() != release_migration.from_release_id
            || self.compiler_release.release_id() != release_migration.to_release_id
            || previous.model_epoch_id != release_migration.source_model_epoch_id
        {
            return Err(ModelError::ReleaseMigration(format!(
                "{} does not bind the supplied source epoch and exact releases",
                release_migration.migration_id
            )));
        }
        let relations = rows_to_state(&previous.rows, &self.metamodel)?;
        let data_relations = data_rows_to_state(&relations, &previous.data_rows)?;
        let mut state = ReplayState {
            relations,
            data_relations,
            current_epoch_id: previous.model_epoch_id.clone(),
            last_migration_id: previous.last_migration_id.clone(),
            migration_ordinal: previous.migration_ordinal,
        };
        replace_intrinsic_rows(
            &mut state,
            &self.metamodel,
            self.installer.install(),
            self.compiler_release.release_id(),
        )?;
        self.apply_migrations(&mut state, &release_migration.migrations)?;
        self.finish(state)
    }

    fn bootstrap_state(&self) -> Result<ReplayState, ModelError> {
        let mut relations = ModelRelation::ALL
            .into_iter()
            .map(|relation| (relation, RelationState::default()))
            .collect::<BTreeMap<_, _>>();
        let bootstrap_epoch = format!("model.bootstrap.{}", self.compiler_release.release_id());
        let mut rows = bootstrap_description_rows(&self.metamodel)?;
        rows.push(
            ModelRowBuilder::new(ModelRelation::ModelEpoch)
                .value("model_epoch_id", bootstrap_epoch.clone())?
                .null("predecessor_model_epoch_id")?
                .value(
                    "compiler_release_id",
                    self.compiler_release.release_id().to_owned(),
                )?
                .value("migration_ordinal", 0_u64)?
                .build(&self.metamodel)?,
        );
        rows.extend(intrinsic_rows(
            &self.metamodel,
            self.installer.install(),
            self.compiler_release.release_id(),
        )?);
        for row in rows {
            add_row(&mut relations, &self.metamodel, row, true)?;
        }
        Ok(ReplayState {
            relations,
            data_relations: BTreeMap::new(),
            current_epoch_id: bootstrap_epoch,
            last_migration_id: None,
            migration_ordinal: 0,
        })
    }

    fn apply_migrations(
        &self,
        state: &mut ReplayState,
        migrations: &[ModelMigration],
    ) -> Result<(), ModelError> {
        let mut migration_ids = BTreeSet::new();
        let mut decision_ids = BTreeSet::new();
        for migration in migrations {
            if migration.ordinal != state.migration_ordinal + 1
                || migration.predecessor_migration_id != state.last_migration_id
                || migration.predecessor_model_epoch_id != state.current_epoch_id
                || !migration_ids.insert(migration.migration_id.as_str())
            {
                return Err(ModelError::InvalidMigrationChain(format!(
                    "migration {} does not extend ordinal {}, predecessor {:?}, epoch {} exactly",
                    migration.migration_id,
                    state.migration_ordinal,
                    state.last_migration_id,
                    state.current_epoch_id
                )));
            }
            add_row(
                &mut state.relations,
                &self.metamodel,
                ModelRowBuilder::new(ModelRelation::ModelEpoch)
                    .value("model_epoch_id", migration.target_model_epoch_id.clone())?
                    .value(
                        "predecessor_model_epoch_id",
                        migration.predecessor_model_epoch_id.clone(),
                    )?
                    .value(
                        "compiler_release_id",
                        self.compiler_release.release_id().to_owned(),
                    )?
                    .value("migration_ordinal", migration.ordinal)?
                    .build(&self.metamodel)?,
                true,
            )?;
            add_row(
                &mut state.relations,
                &self.metamodel,
                ModelRowBuilder::new(ModelRelation::ModelMigration)
                    .value("migration_id", migration.migration_id.clone())?
                    .value(
                        "predecessor_migration_id",
                        migration
                            .predecessor_migration_id
                            .clone()
                            .map_or(ModelValue::Null, ModelValue::Utf8),
                    )?
                    .value(
                        "target_model_epoch_id",
                        migration.target_model_epoch_id.clone(),
                    )?
                    .value("ordinal", migration.ordinal)?
                    .value(
                        "compiler_release_id",
                        self.compiler_release.release_id().to_owned(),
                    )?
                    .value("accepted_by", migration.accepted_by.clone())?
                    .build(&self.metamodel)?,
                true,
            )?;
            for decision in &migration.decisions {
                if !decision_ids.insert(decision.decision_id.as_str()) {
                    return Err(ModelError::InvalidMigrationChain(format!(
                        "duplicate decision {} across migration chain",
                        decision.decision_id
                    )));
                }
                add_row(
                    &mut state.relations,
                    &self.metamodel,
                    ModelRowBuilder::new(ModelRelation::ModelDecision)
                        .value("decision_id", decision.decision_id.clone())?
                        .value("migration_id", migration.migration_id.clone())?
                        .value("owner", decision.owner.clone())?
                        .value("applicability", decision.applicability.clone())?
                        .value("rationale", decision.rationale.clone())?
                        .build(&self.metamodel)?,
                    true,
                )?;
                for operation in &decision.operations {
                    apply_operation(
                        &mut state.relations,
                        &mut state.data_relations,
                        &self.metamodel,
                        operation,
                    )?;
                }
            }
            state.current_epoch_id = migration.target_model_epoch_id.clone();
            state.last_migration_id = Some(migration.migration_id.clone());
            state.migration_ordinal = migration.ordinal;
        }
        Ok(())
    }

    fn finish(&self, state: ReplayState) -> Result<ModelEpoch, ModelError> {
        validate_reference_closure(&state.relations, &state.data_relations, &self.metamodel)?;
        validate_bootstrap_closure(&state.relations, &self.metamodel)?;
        let rows = state
            .relations
            .iter()
            .map(|(relation, state)| (*relation, state.active.values().cloned().collect()))
            .collect::<BTreeMap<_, Vec<_>>>();
        let data_rows = state
            .data_relations
            .iter()
            .map(|(relation_id, state)| {
                (
                    relation_id.clone(),
                    state.active.values().cloned().collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let data_batches = data_rows_to_batches(&state.relations, &data_rows)?;
        let mut relations = rows_to_batches(&rows, &self.metamodel)?;
        relations.data_batches = data_batches;
        Ok(ModelEpoch {
            compiler_release: self.compiler_release.clone(),
            model_epoch_id: state.current_epoch_id,
            last_migration_id: state.last_migration_id,
            migration_ordinal: state.migration_ordinal,
            relations,
            rows: Arc::new(rows),
            data_rows: Arc::new(data_rows),
        })
    }
}

fn derived_relation(relation: ModelRelation) -> bool {
    matches!(
        relation,
        ModelRelation::ModelEpoch
            | ModelRelation::ModelMigration
            | ModelRelation::ModelDecision
            | ModelRelation::IntrinsicPrimitive
    )
}

fn apply_operation(
    relations: &mut BTreeMap<ModelRelation, RelationState>,
    data_relations: &mut BTreeMap<String, RelationStateData>,
    metamodel: &BootstrapMetamodel,
    operation: &ModelOperation,
) -> Result<(), ModelError> {
    let relation = match operation {
        ModelOperation::Add(row)
        | ModelOperation::Supersede {
            replacement: row, ..
        } => row.relation(),
        ModelOperation::Retire(reference) => reference.relation,
        ModelOperation::AddData(_)
        | ModelOperation::SupersedeData { .. }
        | ModelOperation::RetireData(_) => {
            return apply_data_operation(relations, data_relations, operation);
        }
    };
    if derived_relation(relation) {
        return Err(ModelError::OperationRejected(format!(
            "{} is emitted by replay or installation and cannot be authored",
            relation.as_str()
        )));
    }
    match operation {
        ModelOperation::Add(row) => add_row(relations, metamodel, row.clone(), false),
        ModelOperation::Retire(reference) => {
            validate_reference_key(reference, metamodel)?;
            let state = relations.get_mut(&reference.relation).ok_or_else(|| {
                ModelError::OperationRejected("unknown relation in retire operation".into())
            })?;
            if state.active.remove(&reference.key).is_none() {
                return Err(ModelError::OperationRejected(format!(
                    "retire target is not active in {}",
                    reference.relation.as_str()
                )));
            }
            Ok(())
        }
        ModelOperation::Supersede { prior, replacement } => {
            if prior.relation != replacement.relation() {
                return Err(ModelError::OperationRejected(
                    "supersede replacement changes relation".into(),
                ));
            }
            validate_reference_key(prior, metamodel)?;
            metamodel.validate_row(replacement)?;
            let replacement_key = metamodel.row_key(replacement)?;
            let state = relations.get_mut(&prior.relation).ok_or_else(|| {
                ModelError::OperationRejected("unknown relation in supersede operation".into())
            })?;
            if state.active.remove(&prior.key).is_none() {
                return Err(ModelError::OperationRejected(format!(
                    "supersede target is not active in {}",
                    prior.relation.as_str()
                )));
            }
            if replacement_key != prior.key && state.ever_seen.contains(&replacement_key) {
                return Err(ModelError::OperationRejected(
                    "supersede replacement reuses a historical key".into(),
                ));
            }
            state.ever_seen.insert(replacement_key.clone());
            state.active.insert(replacement_key, replacement.clone());
            Ok(())
        }
        ModelOperation::AddData(_)
        | ModelOperation::SupersedeData { .. }
        | ModelOperation::RetireData(_) => unreachable!("data operations returned above"),
    }
}

fn validate_reference_key(
    reference: &RowReference,
    metamodel: &BootstrapMetamodel,
) -> Result<(), ModelError> {
    let expected = metamodel
        .relation_spec(reference.relation)
        .primary_key()
        .len();
    if reference.key.len() != expected
        || reference
            .key
            .iter()
            .any(|value| matches!(value, ModelValue::Null))
    {
        return Err(ModelError::OperationRejected(format!(
            "{} row reference has invalid key arity or null",
            reference.relation.as_str()
        )));
    }
    Ok(())
}

fn add_row(
    relations: &mut BTreeMap<ModelRelation, RelationState>,
    metamodel: &BootstrapMetamodel,
    row: ModelRow,
    derived: bool,
) -> Result<(), ModelError> {
    if !derived && derived_relation(row.relation()) {
        return Err(ModelError::OperationRejected(format!(
            "{} is not an authored relation",
            row.relation().as_str()
        )));
    }
    metamodel.validate_row(&row)?;
    let key = metamodel.row_key(&row)?;
    let state = relations.get_mut(&row.relation()).ok_or_else(|| {
        ModelError::OperationRejected(format!("unknown relation {}", row.relation().as_str()))
    })?;
    if state.ever_seen.contains(&key) {
        return Err(ModelError::OperationRejected(format!(
            "add reuses an active or historical key in {}",
            row.relation().as_str()
        )));
    }
    state.ever_seen.insert(key.clone());
    state.active.insert(key, row);
    Ok(())
}

fn rows_to_state(
    rows: &BTreeMap<ModelRelation, Vec<ModelRow>>,
    metamodel: &BootstrapMetamodel,
) -> Result<BTreeMap<ModelRelation, RelationState>, ModelError> {
    let mut relations = ModelRelation::ALL
        .into_iter()
        .map(|relation| (relation, RelationState::default()))
        .collect::<BTreeMap<_, _>>();
    for row in rows.values().flatten() {
        add_row(&mut relations, metamodel, row.clone(), true)?;
    }
    Ok(relations)
}

fn data_rows_to_state(
    relations: &BTreeMap<ModelRelation, RelationState>,
    rows: &BTreeMap<String, Vec<ModelDataRow>>,
) -> Result<BTreeMap<String, RelationStateData>, ModelError> {
    let definitions = data_relation_definitions(relations)?;
    rows.iter()
        .map(|(relation_id, rows)| {
            let definition = definitions.get(relation_id).ok_or_else(|| {
                ModelError::ReferenceClosure(format!(
                    "persisted model data relation {relation_id} has no active schema"
                ))
            })?;
            let active = rows
                .iter()
                .map(|row| Ok((definition.validate_row(row)?, row.clone())))
                .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
            let ever_seen = active.keys().cloned().collect();
            Ok((relation_id.clone(), RelationStateData { active, ever_seen }))
        })
        .collect()
}

#[derive(Clone, Debug)]
struct DataFieldDefinition {
    field_id: String,
    field_name: String,
    scalar_type: ScalarType,
    nullable: bool,
    ordinal: u32,
    semantic_role: String,
}

#[derive(Clone, Debug)]
struct DataRelationDefinition {
    relation_id: String,
    relation_name: String,
    fields: Vec<DataFieldDefinition>,
    primary_key: Vec<String>,
}

impl DataRelationDefinition {
    fn arrow_schema(&self) -> Arc<Schema> {
        let fields = self.fields.iter().map(|field| {
            Field::new(
                &field.field_name,
                field.scalar_type.arrow_data_type(),
                field.nullable,
            )
            .with_metadata(HashMap::from([
                (
                    "codefabric.model_relation_id".to_owned(),
                    self.relation_id.clone(),
                ),
                (
                    "codefabric.model_field_id".to_owned(),
                    field.field_id.clone(),
                ),
                (
                    "codefabric.semantic_role".to_owned(),
                    field.semantic_role.clone(),
                ),
            ]))
        });
        Arc::new(Schema::new_with_metadata(
            fields.collect::<Vec<_>>(),
            HashMap::from([
                ("codefabric.schema_role".to_owned(), "model".to_owned()),
                (
                    "codefabric.model_relation_id".to_owned(),
                    self.relation_id.clone(),
                ),
                (
                    "codefabric.relation_name".to_owned(),
                    self.relation_name.clone(),
                ),
            ]),
        ))
    }

    fn validate_row(&self, row: &ModelDataRow) -> Result<RowKey, ModelError> {
        if row.relation_id() != self.relation_id || row.values().len() != self.fields.len() {
            return Err(ModelError::InvalidRow {
                relation: row.relation_id().to_owned(),
                message: "row relation or field count differs from replayed schema".to_owned(),
            });
        }
        for field in &self.fields {
            let value =
                row.values()
                    .get(&field.field_name)
                    .ok_or_else(|| ModelError::InvalidRow {
                        relation: self.relation_id.clone(),
                        message: format!("missing field {}", field.field_name),
                    })?;
            if matches!(value, ModelValue::Null) {
                if !field.nullable {
                    return Err(ModelError::InvalidRow {
                        relation: self.relation_id.clone(),
                        message: format!("non-nullable field {} is null", field.field_name),
                    });
                }
            } else if value.scalar_type() != Some(field.scalar_type) {
                return Err(ModelError::InvalidRow {
                    relation: self.relation_id.clone(),
                    message: format!("field {} has the wrong scalar type", field.field_name),
                });
            }
        }
        if row.values().keys().any(|field| {
            !self
                .fields
                .iter()
                .any(|expected| expected.field_name == *field)
        }) {
            return Err(ModelError::InvalidRow {
                relation: self.relation_id.clone(),
                message: "row contains an undeclared field".to_owned(),
            });
        }
        self.primary_key
            .iter()
            .map(|field| {
                let value = row
                    .values()
                    .get(field)
                    .expect("primary-key field is declared");
                if matches!(value, ModelValue::Null) {
                    return Err(ModelError::InvalidRow {
                        relation: self.relation_id.clone(),
                        message: format!("primary-key field {field} is null"),
                    });
                }
                Ok(value.clone())
            })
            .collect()
    }
}

fn data_relation_definitions(
    relations: &BTreeMap<ModelRelation, RelationState>,
) -> Result<BTreeMap<String, DataRelationDefinition>, ModelError> {
    let semantic_types = relations[&ModelRelation::SemanticType]
        .active
        .values()
        .map(|row| {
            Ok((
                utf8_value(row, "semantic_type_id")?.to_owned(),
                scalar_type_from_logical(utf8_value(row, "logical_type")?)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ModelError>>()?;

    let mut definitions = BTreeMap::new();
    for relation in relations[&ModelRelation::Relation].active.values() {
        let relation_id = utf8_value(relation, "relation_id")?;
        let schema_name = utf8_value(relation, "schema_name")?;
        let semantic_role = utf8_value(relation, "semantic_role")?;
        if schema_name != "model" || matches!(semantic_role, "metamodel" | "derived-runtime") {
            continue;
        }
        let relation_name = utf8_value(relation, "relation_name")?.to_owned();
        let mut fields =
            relations[&ModelRelation::Field]
                .active
                .values()
                .filter(|field| {
                    field.value("relation_id").and_then(ModelValue::as_utf8) == Some(relation_id)
                })
                .map(|field| {
                    let semantic_type_id = utf8_value(field, "semantic_type_id")?;
                    let scalar_type = semantic_types.get(semantic_type_id).copied().ok_or_else(
                        || {
                            ModelError::ReferenceClosure(format!(
                                "field {} references unsupported semantic type {semantic_type_id}",
                                utf8_value(field, "field_id").unwrap_or("<invalid>")
                            ))
                        },
                    )?;
                    Ok(DataFieldDefinition {
                        field_id: utf8_value(field, "field_id")?.to_owned(),
                        field_name: utf8_value(field, "field_name")?.to_owned(),
                        scalar_type,
                        nullable: bool_value(field, "nullable")?,
                        ordinal: u32_value(field, "ordinal")?,
                        semantic_role: utf8_value(field, "semantic_role")?.to_owned(),
                    })
                })
                .collect::<Result<Vec<_>, ModelError>>()?;
        fields.sort_by_key(|field| field.ordinal);
        if fields.is_empty()
            || fields
                .iter()
                .enumerate()
                .any(|(ordinal, field)| usize::try_from(field.ordinal).ok() != Some(ordinal))
        {
            return Err(ModelError::ReferenceClosure(format!(
                "model data relation {relation_id} has no fields or non-contiguous ordinals"
            )));
        }
        let field_names = fields
            .iter()
            .map(|field| (field.field_id.as_str(), field.field_name.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut primary_rows = relations[&ModelRelation::Key]
            .active
            .values()
            .filter(|key| {
                key.value("relation_id").and_then(ModelValue::as_utf8) == Some(relation_id)
                    && key.value("key_kind").and_then(ModelValue::as_utf8) == Some("primary")
            })
            .collect::<Vec<_>>();
        primary_rows.sort_by_key(|row| u32_value(row, "ordinal").unwrap_or(u32::MAX));
        let key_ids = primary_rows
            .iter()
            .filter_map(|row| row.value("key_id").and_then(ModelValue::as_utf8))
            .collect::<BTreeSet<_>>();
        if primary_rows.is_empty()
            || key_ids.len() != 1
            || primary_rows
                .iter()
                .enumerate()
                .any(|(ordinal, row)| u32_value(row, "ordinal").ok() != u32::try_from(ordinal).ok())
        {
            return Err(ModelError::ReferenceClosure(format!(
                "model data relation {relation_id} requires one contiguous primary key"
            )));
        }
        let primary_key = primary_rows
            .iter()
            .map(|row| {
                let field_id = utf8_value(row, "field_id")?;
                field_names.get(field_id).map(|name| (*name).to_owned()).ok_or_else(|| {
                    ModelError::ReferenceClosure(format!(
                        "model data relation {relation_id} primary key references foreign field {field_id}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let definition = DataRelationDefinition {
            relation_id: relation_id.to_owned(),
            relation_name,
            fields,
            primary_key,
        };
        if definitions
            .insert(relation_id.to_owned(), definition)
            .is_some()
        {
            return Err(ModelError::ReferenceClosure(format!(
                "duplicate model data relation {relation_id}"
            )));
        }
    }
    Ok(definitions)
}

fn scalar_type_from_logical(value: &str) -> Result<ScalarType, ModelError> {
    match value {
        "bool" | "boolean" => Ok(ScalarType::Bool),
        "u32" => Ok(ScalarType::UInt32),
        "u64" => Ok(ScalarType::UInt64),
        "utf8" => Ok(ScalarType::Utf8),
        "binary" => Ok(ScalarType::Binary),
        other => Err(ModelError::ReferenceClosure(format!(
            "model data relation uses unsupported logical type {other:?}"
        ))),
    }
}

fn bool_value(row: &ModelRow, field: &str) -> Result<bool, ModelError> {
    match row.value(field) {
        Some(ModelValue::Bool(value)) => Ok(*value),
        _ => Err(ModelError::ReferenceClosure(format!(
            "{}.{} is not a bool",
            row.relation().as_str(),
            field
        ))),
    }
}

fn u32_value(row: &ModelRow, field: &str) -> Result<u32, ModelError> {
    match row.value(field) {
        Some(ModelValue::UInt32(value)) => Ok(*value),
        _ => Err(ModelError::ReferenceClosure(format!(
            "{}.{} is not a u32",
            row.relation().as_str(),
            field
        ))),
    }
}

fn apply_data_operation(
    relations: &BTreeMap<ModelRelation, RelationState>,
    data_relations: &mut BTreeMap<String, RelationStateData>,
    operation: &ModelOperation,
) -> Result<(), ModelError> {
    let relation_id = match operation {
        ModelOperation::AddData(row) => row.relation_id(),
        ModelOperation::SupersedeData { prior, replacement } => {
            if prior.relation_id() != replacement.relation_id() {
                return Err(ModelError::OperationRejected(
                    "model data supersede replacement changes relation".to_owned(),
                ));
            }
            prior.relation_id()
        }
        ModelOperation::RetireData(reference) => reference.relation_id(),
        _ => unreachable!("only data operations call apply_data_operation"),
    };
    let definitions = data_relation_definitions(relations)?;
    let definition = definitions.get(relation_id).ok_or_else(|| {
        ModelError::OperationRejected(format!(
            "model data relation {relation_id} is not active in the replayed model"
        ))
    })?;
    let state = data_relations.entry(relation_id.to_owned()).or_default();
    match operation {
        ModelOperation::AddData(row) => {
            let key = definition.validate_row(row)?;
            if !state.ever_seen.insert(key.clone()) {
                return Err(ModelError::OperationRejected(format!(
                    "add reuses an active or historical key in model data relation {relation_id}"
                )));
            }
            state.active.insert(key, row.clone());
            Ok(())
        }
        ModelOperation::RetireData(reference) => {
            if reference.key().len() != definition.primary_key.len()
                || state.active.remove(reference.key()).is_none()
            {
                return Err(ModelError::OperationRejected(format!(
                    "retire target is not active in model data relation {relation_id}"
                )));
            }
            Ok(())
        }
        ModelOperation::SupersedeData { prior, replacement } => {
            if prior.key().len() != definition.primary_key.len()
                || state.active.remove(prior.key()).is_none()
            {
                return Err(ModelError::OperationRejected(format!(
                    "supersede target is not active in model data relation {relation_id}"
                )));
            }
            let replacement_key = definition.validate_row(replacement)?;
            if replacement_key.as_slice() != prior.key()
                && state.ever_seen.contains(&replacement_key)
            {
                return Err(ModelError::OperationRejected(format!(
                    "supersede reuses a historical key in model data relation {relation_id}"
                )));
            }
            state.ever_seen.insert(replacement_key.clone());
            state.active.insert(replacement_key, replacement.clone());
            Ok(())
        }
        _ => unreachable!("only data operations call apply_data_operation"),
    }
}

fn replace_intrinsic_rows(
    state: &mut ReplayState,
    metamodel: &BootstrapMetamodel,
    installed: Vec<InstalledIntrinsic>,
    compiler_release_id: &str,
) -> Result<(), ModelError> {
    state
        .relations
        .insert(ModelRelation::IntrinsicPrimitive, RelationState::default());
    for row in intrinsic_rows(metamodel, installed, compiler_release_id)? {
        add_row(&mut state.relations, metamodel, row, true)?;
    }
    Ok(())
}

fn intrinsic_rows(
    metamodel: &BootstrapMetamodel,
    installed: Vec<InstalledIntrinsic>,
    compiler_release_id: &str,
) -> Result<Vec<ModelRow>, ModelError> {
    installed
        .into_iter()
        .map(|intrinsic| {
            ModelRowBuilder::new(ModelRelation::IntrinsicPrimitive)
                .value("primitive_id", intrinsic.primitive_id)?
                .value("signature", intrinsic.signature)?
                .value("semantic_level", intrinsic.semantic_level)?
                .value("implementation_id", intrinsic.implementation_id)?
                .value("package_id", intrinsic.package_id)?
                .value("compiler_release_id", compiler_release_id.to_owned())?
                .build(metamodel)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct BootstrapForeignKey {
    id: &'static str,
    source_relation: ModelRelation,
    source_field: &'static str,
    target_relation: ModelRelation,
    target_field: &'static str,
}

const BOOTSTRAP_FOREIGN_KEYS: &[BootstrapForeignKey] = &[
    BootstrapForeignKey {
        id: "fk.field.relation",
        source_relation: ModelRelation::Field,
        source_field: "relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.field.semantic_type",
        source_relation: ModelRelation::Field,
        source_field: "semantic_type_id",
        target_relation: ModelRelation::SemanticType,
        target_field: "semantic_type_id",
    },
    BootstrapForeignKey {
        id: "fk.key.relation",
        source_relation: ModelRelation::Key,
        source_field: "relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.key.field",
        source_relation: ModelRelation::Key,
        source_field: "field_id",
        target_relation: ModelRelation::Field,
        target_field: "field_id",
    },
    BootstrapForeignKey {
        id: "fk.representation.semantic_type",
        source_relation: ModelRelation::Representation,
        source_field: "semantic_type_id",
        target_relation: ModelRelation::SemanticType,
        target_field: "semantic_type_id",
    },
    BootstrapForeignKey {
        id: "fk.physical_binding.logical_relation",
        source_relation: ModelRelation::PhysicalBinding,
        source_field: "logical_relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.physical_binding.storage_relation",
        source_relation: ModelRelation::PhysicalBinding,
        source_field: "storage_relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.physical_binding.mapping_program",
        source_relation: ModelRelation::PhysicalBinding,
        source_field: "mapping_program_id",
        target_relation: ModelRelation::Program,
        target_field: "program_id",
    },
    BootstrapForeignKey {
        id: "fk.foreign_key.source_relation",
        source_relation: ModelRelation::ForeignKey,
        source_field: "source_relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.foreign_key.source_field",
        source_relation: ModelRelation::ForeignKey,
        source_field: "source_field_id",
        target_relation: ModelRelation::Field,
        target_field: "field_id",
    },
    BootstrapForeignKey {
        id: "fk.foreign_key.target_relation",
        source_relation: ModelRelation::ForeignKey,
        source_field: "target_relation_id",
        target_relation: ModelRelation::Relation,
        target_field: "relation_id",
    },
    BootstrapForeignKey {
        id: "fk.foreign_key.target_field",
        source_relation: ModelRelation::ForeignKey,
        source_field: "target_field_id",
        target_relation: ModelRelation::Field,
        target_field: "field_id",
    },
    BootstrapForeignKey {
        id: "fk.model_migration.target_epoch",
        source_relation: ModelRelation::ModelMigration,
        source_field: "target_model_epoch_id",
        target_relation: ModelRelation::ModelEpoch,
        target_field: "model_epoch_id",
    },
    BootstrapForeignKey {
        id: "fk.model_decision.migration",
        source_relation: ModelRelation::ModelDecision,
        source_field: "migration_id",
        target_relation: ModelRelation::ModelMigration,
        target_field: "migration_id",
    },
    BootstrapForeignKey {
        id: "fk.program_step.program",
        source_relation: ModelRelation::ProgramStep,
        source_field: "program_id",
        target_relation: ModelRelation::Program,
        target_field: "program_id",
    },
    BootstrapForeignKey {
        id: "fk.step_input.step",
        source_relation: ModelRelation::StepInput,
        source_field: "step_id",
        target_relation: ModelRelation::ProgramStep,
        target_field: "step_id",
    },
    BootstrapForeignKey {
        id: "fk.step_output.step",
        source_relation: ModelRelation::StepOutput,
        source_field: "step_id",
        target_relation: ModelRelation::ProgramStep,
        target_field: "step_id",
    },
    BootstrapForeignKey {
        id: "fk.primitive_binding.primitive",
        source_relation: ModelRelation::PrimitiveBinding,
        source_field: "primitive_id",
        target_relation: ModelRelation::IntrinsicPrimitive,
        target_field: "primitive_id",
    },
    BootstrapForeignKey {
        id: "fk.primitive_binding.program",
        source_relation: ModelRelation::PrimitiveBinding,
        source_field: "program_id",
        target_relation: ModelRelation::Program,
        target_field: "program_id",
    },
    BootstrapForeignKey {
        id: "fk.phrase_binding.phrase",
        source_relation: ModelRelation::PhraseBinding,
        source_field: "phrase_id",
        target_relation: ModelRelation::Phrase,
        target_field: "phrase_id",
    },
    BootstrapForeignKey {
        id: "fk.phrase_binding.form",
        source_relation: ModelRelation::PhraseBinding,
        source_field: "query_form_id",
        target_relation: ModelRelation::QueryForm,
        target_field: "query_form_id",
    },
    BootstrapForeignKey {
        id: "fk.state.machine",
        source_relation: ModelRelation::State,
        source_field: "state_machine_id",
        target_relation: ModelRelation::StateMachine,
        target_field: "state_machine_id",
    },
    BootstrapForeignKey {
        id: "fk.transition.machine",
        source_relation: ModelRelation::Transition,
        source_field: "state_machine_id",
        target_relation: ModelRelation::StateMachine,
        target_field: "state_machine_id",
    },
    BootstrapForeignKey {
        id: "fk.transition.from",
        source_relation: ModelRelation::Transition,
        source_field: "from_state_id",
        target_relation: ModelRelation::State,
        target_field: "state_id",
    },
    BootstrapForeignKey {
        id: "fk.transition.to",
        source_relation: ModelRelation::Transition,
        source_field: "to_state_id",
        target_relation: ModelRelation::State,
        target_field: "state_id",
    },
];

fn bootstrap_description_rows(metamodel: &BootstrapMetamodel) -> Result<Vec<ModelRow>, ModelError> {
    let mut rows = Vec::new();
    for scalar_type in [
        ScalarType::Bool,
        ScalarType::UInt32,
        ScalarType::UInt64,
        ScalarType::Utf8,
        ScalarType::Binary,
    ] {
        rows.push(
            ModelRowBuilder::new(ModelRelation::SemanticType)
                .value(
                    "semantic_type_id",
                    format!("bootstrap.scalar.{}", scalar_type.id()),
                )?
                .value("name", scalar_type.id())?
                .value("logical_type", scalar_type.id())?
                .value("allows_null", true)?
                .build(metamodel)?,
        );
    }
    for relation in metamodel.relations() {
        let spec = metamodel.relation_spec(relation);
        let relation_id = format!("bootstrap.relation.{}", relation.as_str());
        rows.push(
            ModelRowBuilder::new(ModelRelation::Relation)
                .value("relation_id", relation_id.clone())?
                .value("schema_name", "model")?
                .value("relation_name", relation.as_str())?
                .value(
                    "semantic_role",
                    if relation == ModelRelation::IntrinsicPrimitive {
                        "derived-runtime"
                    } else {
                        "metamodel"
                    },
                )?
                .build(metamodel)?,
        );
        for (ordinal, field) in spec.fields().iter().enumerate() {
            rows.push(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value(
                        "field_id",
                        format!("bootstrap.field.{}.{}", relation.as_str(), field.name()),
                    )?
                    .value("relation_id", relation_id.clone())?
                    .value("field_name", field.name())?
                    .value(
                        "semantic_type_id",
                        format!("bootstrap.scalar.{}", field.scalar_type().id()),
                    )?
                    .value(
                        "ordinal",
                        u32::try_from(ordinal).map_err(|_| {
                            ModelError::BootstrapClosure("field ordinal exceeds u32".into())
                        })?,
                    )?
                    .value("nullable", field.nullable())?
                    .value("semantic_role", field.semantic_role())?
                    .build(metamodel)?,
            );
        }
        for (ordinal, field_name) in spec.primary_key().iter().enumerate() {
            rows.push(
                ModelRowBuilder::new(ModelRelation::Key)
                    .value("key_id", format!("bootstrap.key.{}", relation.as_str()))?
                    .value("relation_id", relation_id.clone())?
                    .value(
                        "field_id",
                        format!("bootstrap.field.{}.{}", relation.as_str(), field_name),
                    )?
                    .value(
                        "ordinal",
                        u32::try_from(ordinal).map_err(|_| {
                            ModelError::BootstrapClosure("key ordinal exceeds u32".into())
                        })?,
                    )?
                    .value("key_kind", "primary")?
                    .build(metamodel)?,
            );
        }
    }
    for foreign_key in BOOTSTRAP_FOREIGN_KEYS {
        rows.push(
            ModelRowBuilder::new(ModelRelation::ForeignKey)
                .value("foreign_key_id", foreign_key.id)?
                .value(
                    "source_relation_id",
                    format!(
                        "bootstrap.relation.{}",
                        foreign_key.source_relation.as_str()
                    ),
                )?
                .value(
                    "source_field_id",
                    format!(
                        "bootstrap.field.{}.{}",
                        foreign_key.source_relation.as_str(),
                        foreign_key.source_field
                    ),
                )?
                .value(
                    "target_relation_id",
                    format!(
                        "bootstrap.relation.{}",
                        foreign_key.target_relation.as_str()
                    ),
                )?
                .value(
                    "target_field_id",
                    format!(
                        "bootstrap.field.{}.{}",
                        foreign_key.target_relation.as_str(),
                        foreign_key.target_field
                    ),
                )?
                .value("ordinal", 0_u32)?
                .value("on_missing", "reject")?
                .build(metamodel)?,
        );
    }
    Ok(rows)
}

fn validate_bootstrap_closure(
    relations: &BTreeMap<ModelRelation, RelationState>,
    metamodel: &BootstrapMetamodel,
) -> Result<(), ModelError> {
    for expected in bootstrap_description_rows(metamodel)? {
        let key = metamodel.row_key(&expected)?;
        let observed = relations[&expected.relation()].active.get(&key);
        if observed != Some(&expected) {
            return Err(ModelError::BootstrapClosure(format!(
                "{} row {:?} is absent or differs",
                expected.relation().as_str(),
                key
            )));
        }
    }
    Ok(())
}

fn utf8_value<'a>(row: &'a ModelRow, field: &str) -> Result<&'a str, ModelError> {
    row.value(field)
        .and_then(ModelValue::as_utf8)
        .ok_or_else(|| {
            ModelError::ReferenceClosure(format!(
                "{}.{} is not a UTF-8 reference",
                row.relation().as_str(),
                field
            ))
        })
}

fn validate_reference_closure(
    relations: &BTreeMap<ModelRelation, RelationState>,
    data_relations: &BTreeMap<String, RelationStateData>,
    _metamodel: &BootstrapMetamodel,
) -> Result<(), ModelError> {
    let relation_names = relations[&ModelRelation::Relation]
        .active
        .values()
        .map(|row| {
            Ok((
                utf8_value(row, "relation_id")?.to_owned(),
                utf8_value(row, "relation_name")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
    let fields = relations[&ModelRelation::Field]
        .active
        .values()
        .map(|row| {
            Ok((
                utf8_value(row, "field_id")?.to_owned(),
                (
                    utf8_value(row, "relation_id")?.to_owned(),
                    utf8_value(row, "field_name")?.to_owned(),
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
    for row in relations[&ModelRelation::ForeignKey].active.values() {
        let source_relation_id = utf8_value(row, "source_relation_id")?;
        let source_field_id = utf8_value(row, "source_field_id")?;
        let target_relation_id = utf8_value(row, "target_relation_id")?;
        let target_field_id = utf8_value(row, "target_field_id")?;
        let (source_field_relation, source_field_name) =
            fields.get(source_field_id).ok_or_else(|| {
                ModelError::ReferenceClosure(format!("unknown source field {source_field_id}"))
            })?;
        let (target_field_relation, target_field_name) =
            fields.get(target_field_id).ok_or_else(|| {
                ModelError::ReferenceClosure(format!("unknown target field {target_field_id}"))
            })?;
        if source_field_relation != source_relation_id
            || target_field_relation != target_relation_id
        {
            return Err(ModelError::ReferenceClosure(format!(
                "foreign key {} binds a field to the wrong relation",
                utf8_value(row, "foreign_key_id")?
            )));
        }
        let source_values = active_relation_values(
            relations,
            data_relations,
            &relation_names,
            source_relation_id,
            source_field_name,
        )?;
        let target_values = active_relation_values(
            relations,
            data_relations,
            &relation_names,
            target_relation_id,
            target_field_name,
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        for value in source_values {
            if matches!(&value, ModelValue::Null) {
                continue;
            }
            if !target_values.contains(&value) {
                return Err(ModelError::ReferenceClosure(format!(
                    "{source_relation_id}.{source_field_name} value {value:?} has no {target_relation_id}.{target_field_name} target"
                )));
            }
        }
    }
    Ok(())
}

fn active_relation_values(
    relations: &BTreeMap<ModelRelation, RelationState>,
    data_relations: &BTreeMap<String, RelationStateData>,
    relation_names: &BTreeMap<String, String>,
    relation_id: &str,
    field_name: &str,
) -> Result<Vec<ModelValue>, ModelError> {
    if let Some(relation) = relation_names
        .get(relation_id)
        .and_then(|name| ModelRelation::from_name(name))
    {
        return relations[&relation]
            .active
            .values()
            .map(|row| {
                row.value(field_name).cloned().ok_or_else(|| {
                    ModelError::ReferenceClosure(format!(
                        "{} lacks modeled field {field_name}",
                        relation.as_str()
                    ))
                })
            })
            .collect();
    }
    if let Some(state) = data_relations.get(relation_id) {
        return state
            .active
            .values()
            .map(|row| {
                row.value(field_name).cloned().ok_or_else(|| {
                    ModelError::ReferenceClosure(format!(
                        "model data relation {relation_id} lacks field {field_name}"
                    ))
                })
            })
            .collect();
    }
    if relation_names.contains_key(relation_id) {
        // A declared model-data relation with no active rows is still an exact empty
        // relation.  Absence of the schema row, not zero cardinality, is an error.
        Ok(Vec::new())
    } else {
        Err(ModelError::ReferenceClosure(format!(
            "relation {relation_id} has no replayed row authority"
        )))
    }
}

fn rows_to_batches(
    rows: &BTreeMap<ModelRelation, Vec<ModelRow>>,
    metamodel: &BootstrapMetamodel,
) -> Result<ModelRelations, ModelError> {
    let batches = ModelRelation::ALL
        .into_iter()
        .map(|relation| {
            let spec = metamodel.relation_spec(relation);
            let relation_rows = &rows[&relation];
            let columns = spec
                .fields()
                .iter()
                .map(|field| {
                    let values = relation_rows
                        .iter()
                        .map(|row| &row.values()[field.name()])
                        .collect::<Vec<_>>();
                    build_array(field.scalar_type(), &values)
                })
                .collect::<Result<Vec<_>, ModelError>>()?;
            Ok((
                relation,
                RecordBatch::try_new(spec.arrow_schema(), columns)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
    Ok(ModelRelations {
        batches,
        data_batches: BTreeMap::new(),
    })
}

fn data_rows_to_batches(
    relations: &BTreeMap<ModelRelation, RelationState>,
    rows: &BTreeMap<String, Vec<ModelDataRow>>,
) -> Result<BTreeMap<String, RecordBatch>, ModelError> {
    let definitions = data_relation_definitions(relations)?;
    for relation_id in rows.keys() {
        if !definitions.contains_key(relation_id) {
            return Err(ModelError::ReferenceClosure(format!(
                "model data relation {relation_id} has rows but no active schema"
            )));
        }
    }
    definitions
        .into_iter()
        .map(|(relation_id, definition)| {
            let relation_rows = rows.get(&relation_id).map_or(&[][..], Vec::as_slice);
            let keyed = relation_rows
                .iter()
                .map(|row| Ok((definition.validate_row(row)?, row)))
                .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
            if keyed.len() != relation_rows.len() {
                return Err(ModelError::ReferenceClosure(format!(
                    "model data relation {relation_id} has duplicate active keys"
                )));
            }
            let ordered = keyed.into_values().collect::<Vec<_>>();
            let columns = definition
                .fields
                .iter()
                .map(|field| {
                    let values = ordered
                        .iter()
                        .map(|row| {
                            row.values()
                                .get(&field.field_name)
                                .expect("row validation proved every field")
                        })
                        .collect::<Vec<_>>();
                    build_array(field.scalar_type, &values)
                })
                .collect::<Result<Vec<_>, ModelError>>()?;
            let batch = RecordBatch::try_new(definition.arrow_schema(), columns)?;
            Ok((relation_id, batch))
        })
        .collect()
}

fn build_array(scalar_type: ScalarType, values: &[&ModelValue]) -> Result<ArrayRef, ModelError> {
    macro_rules! primitive_array {
        ($builder:ty, $variant:path) => {{
            let mut builder = <$builder>::with_capacity(values.len());
            for value in values {
                match value {
                    $variant(value) => builder.append_value(*value),
                    ModelValue::Null => builder.append_null(),
                    _ => {
                        return Err(ModelError::Arrow(
                            arrow_schema::ArrowError::InvalidArgumentError(
                                "model value changed type after validation".into(),
                            ),
                        ));
                    }
                }
            }
            Arc::new(builder.finish()) as ArrayRef
        }};
    }
    let array = match scalar_type {
        ScalarType::Bool => primitive_array!(BooleanBuilder, ModelValue::Bool),
        ScalarType::UInt32 => primitive_array!(UInt32Builder, ModelValue::UInt32),
        ScalarType::UInt64 => primitive_array!(UInt64Builder, ModelValue::UInt64),
        ScalarType::Utf8 => {
            let mut builder = StringBuilder::new();
            for value in values {
                match value {
                    ModelValue::Utf8(value) => builder.append_value(value),
                    ModelValue::Null => builder.append_null(),
                    _ => {
                        return Err(ModelError::Arrow(
                            arrow_schema::ArrowError::InvalidArgumentError(
                                "model value changed type after validation".into(),
                            ),
                        ));
                    }
                }
            }
            Arc::new(builder.finish())
        }
        ScalarType::Binary => {
            let mut builder = BinaryBuilder::new();
            for value in values {
                match value {
                    ModelValue::Binary(value) => builder.append_value(value),
                    ModelValue::Null => builder.append_null(),
                    _ => {
                        return Err(ModelError::Arrow(
                            arrow_schema::ArrowError::InvalidArgumentError(
                                "model value changed type after validation".into(),
                            ),
                        ));
                    }
                }
            }
            Arc::new(builder.finish())
        }
    };
    Ok(array)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_model::{FabricCompilerRelease, ModelDataRowBuilder};

    fn release(id: &str, package: &str) -> FabricCompilerRelease {
        FabricCompilerRelease::builder(id, format!("source:{id}"), format!("build:{id}"))
            .with_abis(1, 1, 1)
            .with_intrinsic_package(package)
            .add_dependency("arrow", "59.2.0")
            .unwrap()
            .add_dependency("datafusion", "55.0.0")
            .unwrap()
            .add_dependency("deltalake", "43a0cf10")
            .unwrap()
            .add_provider_schema("tree-sitter", "python-0.25.0-rust-0.24.2")
            .unwrap()
            .with_policy_and_configuration("policy-v2", "config-test")
            .add_toolchain("rust", "1.95.0")
            .unwrap()
            .add_wire_contract("codefabric.rpc.cpg-query-service")
            .unwrap()
            .build()
            .unwrap()
    }

    fn state_machine_row(metamodel: &BootstrapMetamodel, id: &str, name: &str) -> ModelRow {
        ModelRowBuilder::new(ModelRelation::StateMachine)
            .value("state_machine_id", id)
            .unwrap()
            .value("name", name)
            .unwrap()
            .build(metamodel)
            .unwrap()
    }

    fn migration(
        metamodel: &BootstrapMetamodel,
        release_id: &str,
        ordinal: u64,
        predecessor_migration: Option<&str>,
        predecessor_epoch: &str,
        target_epoch: &str,
        machine_id: &str,
    ) -> ModelMigration {
        let decision = ModelDecision::new(
            format!("decision.{ordinal}"),
            "independent-model-owner",
            "all-workspaces",
            "install a lifecycle machine",
            vec![ModelOperation::Add(state_machine_row(
                metamodel, machine_id, machine_id,
            ))],
        )
        .unwrap();
        ModelMigration::new(
            format!("migration.{release_id}.{ordinal}"),
            predecessor_migration.map(str::to_owned),
            predecessor_epoch,
            target_epoch,
            ordinal,
            "release-owner",
            vec![decision],
        )
        .unwrap()
    }

    fn model_data_schema_migration(metamodel: &BootstrapMetamodel) -> ModelMigration {
        let relation_id = "model.relation.semantic.operator";
        let mut operations = vec![ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::Relation)
                .value("relation_id", relation_id)
                .unwrap()
                .value("schema_name", "model")
                .unwrap()
                .value("relation_name", "semantic_operator")
                .unwrap()
                .value("semantic_role", "semantic-parameter")
                .unwrap()
                .build(metamodel)
                .unwrap(),
        )];
        for (ordinal, field_id, field_name, semantic_type, nullable, role) in [
            (
                0_u32,
                "model.field.semantic.operator.id",
                "operator_id",
                "bootstrap.scalar.utf8",
                false,
                "canonical-id",
            ),
            (
                1_u32,
                "model.field.semantic.operator.ordinal",
                "ordinal",
                "bootstrap.scalar.u32",
                false,
                "sequence",
            ),
            (
                2_u32,
                "model.field.semantic.operator.note",
                "note",
                "bootstrap.scalar.utf8",
                true,
                "semantic-text",
            ),
        ] {
            operations.push(ModelOperation::Add(
                ModelRowBuilder::new(ModelRelation::Field)
                    .value("field_id", field_id)
                    .unwrap()
                    .value("relation_id", relation_id)
                    .unwrap()
                    .value("field_name", field_name)
                    .unwrap()
                    .value("semantic_type_id", semantic_type)
                    .unwrap()
                    .value("ordinal", ordinal)
                    .unwrap()
                    .value("nullable", nullable)
                    .unwrap()
                    .value("semantic_role", role)
                    .unwrap()
                    .build(metamodel)
                    .unwrap(),
            ));
        }
        operations.push(ModelOperation::Add(
            ModelRowBuilder::new(ModelRelation::Key)
                .value("key_id", "model.key.semantic.operator")
                .unwrap()
                .value("relation_id", relation_id)
                .unwrap()
                .value("field_id", "model.field.semantic.operator.id")
                .unwrap()
                .value("ordinal", 0_u32)
                .unwrap()
                .value("key_kind", "primary")
                .unwrap()
                .build(metamodel)
                .unwrap(),
        ));
        ModelMigration::new(
            "migration.model-data-schema.1",
            None,
            "model.bootstrap.compiler.r1",
            "model.epoch.schema",
            1,
            "model-owner",
            vec![
                ModelDecision::new(
                    "decision.model-data-schema.1",
                    "model-owner",
                    "semantic-programs",
                    "declare a replay-owned semantic parameter relation",
                    operations,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn model_data_row(id: &str, ordinal: u32, note: Option<&str>) -> ModelDataRow {
        let builder = ModelDataRowBuilder::new("model.relation.semantic.operator")
            .unwrap()
            .value("operator_id", id)
            .unwrap()
            .value("ordinal", ordinal)
            .unwrap();
        match note {
            Some(note) => builder.value("note", note).unwrap(),
            None => builder.null("note").unwrap(),
        }
        .build()
        .unwrap()
    }

    #[test]
    fn replay_is_deterministic_logical_arrow_equality() {
        let compiler = release("compiler.r1", "intrinsics.r1");
        let engine = ReplayEngine::new(
            compiler,
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let bootstrap_epoch = "model.bootstrap.compiler.r1";
        let migration = migration(
            engine.metamodel(),
            "r1",
            1,
            None,
            bootstrap_epoch,
            "model.epoch.1",
            "fabric-command",
        );
        let first = engine.replay(std::slice::from_ref(&migration)).unwrap();
        let second = engine.replay(&[migration]).unwrap();
        assert!(first.relations().logically_equals(second.relations()));
        assert_eq!(first.identity_pin(), second.identity_pin());
        assert_eq!(first.model_epoch_id(), "model.epoch.1");
        assert_eq!(
            first
                .relations()
                .batch(ModelRelation::IntrinsicPrimitive)
                .num_rows(),
            13
        );
    }

    #[test]
    fn replay_owns_typed_rows_for_model_defined_relations() {
        let engine = ReplayEngine::new(
            release("compiler.r1", "intrinsics.r1"),
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let schema = model_data_schema_migration(engine.metamodel());
        let data = ModelMigration::new(
            "migration.model-data.2",
            Some("migration.model-data-schema.1".to_owned()),
            "model.epoch.schema",
            "model.epoch.data",
            2,
            "model-owner",
            vec![
                ModelDecision::new(
                    "decision.model-data.2",
                    "model-owner",
                    "semantic-programs",
                    "install independently accepted semantic parameters",
                    vec![
                        ModelOperation::AddData(model_data_row("operator.z", 2, None)),
                        ModelOperation::AddData(model_data_row("operator.a", 1, Some("first"))),
                    ],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let epoch = engine.replay(&[schema, data]).unwrap();
        let batch = epoch
            .relations()
            .data_batch("model.relation.semantic.operator")
            .unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(
            batch.schema().metadata()["codefabric.relation_name"],
            "semantic_operator"
        );
        let ids = batch
            .column_by_name("operator_id")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        assert_eq!((ids.value(0), ids.value(1)), ("operator.a", "operator.z"));
    }

    #[test]
    fn model_defined_relation_rows_fail_closed_on_schema_or_key_mismatch() {
        let engine = ReplayEngine::new(
            release("compiler.r1", "intrinsics.r1"),
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let schema = model_data_schema_migration(engine.metamodel());
        let wrong_type = ModelDataRowBuilder::new("model.relation.semantic.operator")
            .unwrap()
            .value("operator_id", "operator.bad")
            .unwrap()
            .value("ordinal", "not-u32")
            .unwrap()
            .null("note")
            .unwrap()
            .build()
            .unwrap();
        let data = ModelMigration::new(
            "migration.model-data.2",
            Some("migration.model-data-schema.1".to_owned()),
            "model.epoch.schema",
            "model.epoch.data",
            2,
            "model-owner",
            vec![
                ModelDecision::new(
                    "decision.model-data.2",
                    "model-owner",
                    "semantic-programs",
                    "wrong typed fixture",
                    vec![ModelOperation::AddData(wrong_type)],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        assert!(matches!(
            engine.replay(&[schema, data]),
            Err(ModelError::InvalidRow { .. })
        ));
    }

    #[test]
    fn predecessor_and_historical_key_reuse_fail_closed() {
        let engine = ReplayEngine::new(
            release("compiler.r1", "intrinsics.r1"),
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let first = migration(
            engine.metamodel(),
            "r1",
            1,
            None,
            "model.bootstrap.compiler.r1",
            "model.epoch.1",
            "machine",
        );
        let invalid = migration(
            engine.metamodel(),
            "r1",
            2,
            Some("wrong"),
            "model.epoch.1",
            "model.epoch.2",
            "another",
        );
        assert!(matches!(
            engine.replay(&[first.clone(), invalid]),
            Err(ModelError::InvalidMigrationChain(_))
        ));

        let row = state_machine_row(engine.metamodel(), "machine", "replacement");
        let duplicate = ModelDecision::new(
            "decision.2",
            "owner",
            "all",
            "duplicate must fail",
            vec![ModelOperation::Add(row)],
        )
        .unwrap();
        let second = ModelMigration::new(
            "migration.r1.2",
            Some("migration.r1.1".into()),
            "model.epoch.1",
            "model.epoch.2",
            2,
            "owner",
            vec![duplicate],
        )
        .unwrap();
        assert!(matches!(
            engine.replay(&[first, second]),
            Err(ModelError::OperationRejected(_))
        ));
    }

    #[test]
    fn bootstrap_rows_and_installed_intrinsics_cannot_be_authored() {
        let engine = ReplayEngine::new(
            release("compiler.r1", "intrinsics.r1"),
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let row = ModelRowBuilder::new(ModelRelation::IntrinsicPrimitive)
            .value("primitive_id", "invented")
            .unwrap()
            .value("signature", "x->y")
            .unwrap()
            .value("semantic_level", "function")
            .unwrap()
            .value("implementation_id", "fake")
            .unwrap()
            .value("package_id", "fake")
            .unwrap()
            .value("compiler_release_id", "compiler.r1")
            .unwrap()
            .build(engine.metamodel())
            .unwrap();
        let decision = ModelDecision::new(
            "decision.fake",
            "owner",
            "all",
            "attempt duplicate authority",
            vec![ModelOperation::Add(row)],
        )
        .unwrap();
        let migration = ModelMigration::new(
            "migration.fake",
            None,
            "model.bootstrap.compiler.r1",
            "model.fake",
            1,
            "owner",
            vec![decision],
        )
        .unwrap();
        assert!(matches!(
            engine.replay(&[migration]),
            Err(ModelError::OperationRejected(_))
        ));
    }

    #[test]
    fn cross_release_requires_old_reconstruction_and_explicit_migration() {
        let old_engine = ReplayEngine::new(
            release("compiler.r1", "intrinsics.r1"),
            IntrinsicInstaller::new("intrinsics.r1", "impl.r1").unwrap(),
        )
        .unwrap();
        let old_migration = migration(
            old_engine.metamodel(),
            "r1",
            1,
            None,
            "model.bootstrap.compiler.r1",
            "model.epoch.1",
            "machine.r1",
        );
        let old_epoch = old_engine.replay(&[old_migration]).unwrap();
        let old_rows = old_epoch
            .relations()
            .batch(ModelRelation::StateMachine)
            .clone();

        let new_engine = ReplayEngine::new(
            release("compiler.r2", "intrinsics.r2"),
            IntrinsicInstaller::new("intrinsics.r2", "impl.r2").unwrap(),
        )
        .unwrap();
        let new_migration = migration(
            new_engine.metamodel(),
            "r2",
            2,
            Some("migration.r1.1"),
            "model.epoch.1",
            "model.epoch.2",
            "machine.r2",
        );
        let handoff = CompilerReleaseMigration::new(
            "release-migration.r1-r2",
            "compiler.r1",
            "compiler.r2",
            "model.epoch.1",
            vec![new_migration],
        )
        .unwrap();
        let migrated = new_engine.migrate_from(&old_epoch, &handoff).unwrap();
        assert_eq!(migrated.compiler_release().release_id(), "compiler.r2");
        assert_eq!(migrated.model_epoch_id(), "model.epoch.2");
        assert_eq!(
            old_epoch.relations().batch(ModelRelation::StateMachine),
            &old_rows
        );
        assert_eq!(
            migrated
                .relations()
                .batch(ModelRelation::StateMachine)
                .num_rows(),
            2
        );
    }
}
