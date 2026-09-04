//! Complete-inventory capture; changed-work lists cannot construct this boundary.

use std::collections::BTreeSet;

use super::{
    CaptureOutcome, CaptureRequest, SourceBlobHolderKind, SourceEncoding, SourceImage,
    SourceImageError, SourceImageStore, SourceLanguage, current_source_generation,
};
use crate::cancellation::Cancellation;
use crate::identity::{WorkspacePath, random_registration_nonce, source_file_identity};
use crate::inventory::{
    CompleteSourceInventory, InclusionState, InventoryReadFailure, SourceInventoryRecord,
};
use crate::operational_store::OperationalStore;
use crate::secure_path::{PlatformPath, SecurePathError, StableReadError};

/// A release/context-selected disposition. Names alone never classify generated/vendor input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSelection {
    Capture(SourceLanguage),
    Generated,
    Vendored,
    Excluded,
    Unsupported,
}

/// Aggregate captured-byte limit, in addition to the store's per-file limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceInventoryCapturePolicy {
    pub maximum_total_bytes: u64,
    pub holder_kind: SourceBlobHolderKind,
}

/// Every member has exactly one outcome. Pending work never counts as closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryCaptureDisposition {
    Captured {
        image_index: usize,
    },
    Pending,
    Deferred,
    /// Includes descriptor-relative path rejection; no fallback read is authorized.
    Unreadable,
    Unsupported,
    UnsupportedEncoding,
    Oversized,
    Binary,
    Generated,
    Vendored,
    Excluded,
}

/// An immutable per-path capture observation, including negative outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryCaptureEntry {
    path: WorkspacePath,
    disposition: InventoryCaptureDisposition,
}

impl InventoryCaptureEntry {
    #[must_use]
    pub const fn path(&self) -> &WorkspacePath {
        &self.path
    }

    #[must_use]
    pub const fn disposition(&self) -> InventoryCaptureDisposition {
        self.disposition
    }
}

/// Validated full inventory plus all capture outcomes. No public mutable certificate fields.
/// Its durable leases must be released by the owning job; abandoned leases have bounded TTLs.
#[derive(Debug)]
pub struct InventoryCaptureBundle {
    inventory: CompleteSourceInventory,
    dispositions: Vec<InventoryCaptureEntry>,
    images: Vec<SourceImage>,
    fence_current: bool,
}

impl InventoryCaptureBundle {
    #[must_use]
    pub const fn inventory(&self) -> &CompleteSourceInventory {
        &self.inventory
    }

    #[must_use]
    pub fn dispositions(&self) -> &[InventoryCaptureEntry] {
        &self.dispositions
    }

    #[must_use]
    pub fn images(&self) -> &[SourceImage] {
        &self.images
    }

    /// Closure does not promote exclusions or unavailable bytes into provider success.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.fence_current
            && self.dispositions.iter().all(|entry| {
                !matches!(
                    entry.disposition,
                    InventoryCaptureDisposition::Pending | InventoryCaptureDisposition::Deferred
                )
            })
    }

    /// Require terminal dispositions for every member under the captured fence.
    ///
    /// # Errors
    /// Rejects deferred or pending work; callers must preserve the dirty scope.
    pub fn require_closed(&self) -> Result<&Self, SourceImageError> {
        if !self.is_closed() {
            return Err(SourceImageError::InventoryCaptureMismatch);
        }
        Ok(self)
    }

    fn validate_bindings(&self) -> Result<(), SourceImageError> {
        let inventory = self.inventory.inventory();
        if self.dispositions.len() != inventory.records.len() {
            return Err(SourceImageError::InventoryCaptureMismatch);
        }
        let mut indices = BTreeSet::new();
        for (entry, record) in self.dispositions.iter().zip(&inventory.records) {
            if entry.path != record.path || entry.path.workspace_id != inventory.workspace_id {
                return Err(SourceImageError::InventoryCaptureMismatch);
            }
            if let InventoryCaptureDisposition::Captured { image_index } = entry.disposition {
                let image = self
                    .images
                    .get(image_index)
                    .ok_or(SourceImageError::InventoryCaptureMismatch)?;
                if !indices.insert(image_index)
                    || image.workspace_id != inventory.workspace_id
                    || image.source_generation != inventory.source_generation
                    || !image_matches_record(image, record)?
                {
                    return Err(SourceImageError::InventoryCaptureMismatch);
                }
            }
        }
        if indices.len() != self.images.len() {
            return Err(SourceImageError::InventoryCaptureMismatch);
        }
        Ok(())
    }
}

fn image_matches_record(
    image: &SourceImage,
    record: &SourceInventoryRecord,
) -> Result<bool, SourceImageError> {
    Ok(image.path == record.path
        && Some(image.file_id) == record.file_id
        && image.file_id == source_file_identity(&record.path)?.id
        && Some(image.digest) == record.content_digest
        && image.digest == crate::integrity::digest_bytes(&image.bytes)
        && image.blob.digest == image.digest
        && image.lease.blob_digest == image.digest
        && image.blob.byte_length == image.byte_length
        && image.metadata.size == image.byte_length
        && image.byte_length == record.byte_length
        && u64::try_from(image.bytes.len()).ok() == Some(record.byte_length))
}

impl SourceImageStore {
    /// Capture every member of a fenced full inventory under a release-selected policy.
    /// Even failed reads retain an outcome. Every holder is fresh so failed jobs cannot
    /// accidentally release an earlier job's resources.
    ///
    /// # Errors
    /// Rejects invalid bounds, workspace/generation changes, failed persistence, or binding
    /// corruption. Releases all acquired holders before returning an error.
    pub fn capture_inventory_with_fence(
        &mut self,
        store: &mut OperationalStore,
        inventory: &CompleteSourceInventory,
        policy: SourceInventoryCapturePolicy,
        cancellation: &Cancellation,
        mut select: impl FnMut(&SourceInventoryRecord) -> SourceSelection,
        mut observe_change_token: impl FnMut() -> Option<u64>,
    ) -> Result<InventoryCaptureBundle, SourceImageError> {
        if policy.maximum_total_bytes == 0 {
            return Err(SourceImageError::InventoryCaptureBound);
        }
        self.refresh_lease_metrics(store)?;
        let mut bundle = InventoryCaptureBundle {
            inventory: inventory.clone(),
            dispositions: Vec::with_capacity(inventory.inventory().records.len()),
            images: Vec::new(),
            fence_current: true,
        };
        let result = self.capture_members(
            store,
            &mut bundle,
            policy,
            cancellation,
            &mut select,
            &mut observe_change_token,
        );
        if let Err(error) = result {
            self.release_images(store, &bundle.images)?;
            return Err(error);
        }
        if !bundle.is_closed() {
            self.release_images(store, &bundle.images)?;
            bundle.images.clear();
            for entry in &mut bundle.dispositions {
                if matches!(
                    entry.disposition,
                    InventoryCaptureDisposition::Captured { .. }
                ) {
                    entry.disposition = InventoryCaptureDisposition::Deferred;
                }
            }
        }
        if let Err(error) = bundle.validate_bindings() {
            self.release_images(store, &bundle.images)?;
            return Err(error);
        }
        Ok(bundle)
    }

    fn capture_members(
        &mut self,
        store: &mut OperationalStore,
        bundle: &mut InventoryCaptureBundle,
        policy: SourceInventoryCapturePolicy,
        cancellation: &Cancellation,
        select: &mut impl FnMut(&SourceInventoryRecord) -> SourceSelection,
        observe_change_token: &mut impl FnMut() -> Option<u64>,
    ) -> Result<(), SourceImageError> {
        let inventory = bundle.inventory.inventory();
        let mut remaining = policy.maximum_total_bytes;
        for record in &inventory.records {
            if current_source_generation(store, inventory.workspace_id)?
                != inventory.source_generation
            {
                return Err(SourceImageError::GenerationChanged);
            }
            let disposition = if cancellation.is_cancelled() {
                InventoryCaptureDisposition::Pending
            } else if let Some(failure) = bundle
                .inventory
                .read_failure(&record.path.raw_relative_path_bytes)
            {
                match failure {
                    InventoryReadFailure::Pending => InventoryCaptureDisposition::Pending,
                    InventoryReadFailure::Unreadable => InventoryCaptureDisposition::Unreadable,
                }
            } else if record.inclusion == InclusionState::ExcludedSizeLimit {
                InventoryCaptureDisposition::Oversized
            } else if record.inclusion != InclusionState::Included {
                InventoryCaptureDisposition::Excluded
            } else {
                match select(record) {
                    SourceSelection::Capture(language) => {
                        if record.byte_length > remaining {
                            return Err(SourceImageError::InventoryCaptureBound);
                        }
                        let request = CaptureRequest {
                            workspace_id: inventory.workspace_id,
                            source_generation: inventory.source_generation,
                            change_token: bundle.inventory.change_token(),
                            path: PlatformPath::from_raw_relative_bytes(
                                record.path.platform_code,
                                record.path.raw_relative_path_bytes.clone(),
                            )?,
                            language,
                            holder_kind: policy.holder_kind,
                            holder_id: random_registration_nonce()?,
                        };
                        let file_limit = self.policy.maximum_bytes;
                        self.policy.maximum_bytes = file_limit.min(remaining);
                        let outcome = self.capture_with_observed_leases(
                            store,
                            &request,
                            &mut *observe_change_token,
                        );
                        self.policy.maximum_bytes = file_limit;
                        self.admit_member(
                            store,
                            record,
                            outcome,
                            &mut bundle.images,
                            &mut remaining,
                        )?
                    }
                    SourceSelection::Generated => InventoryCaptureDisposition::Generated,
                    SourceSelection::Vendored => InventoryCaptureDisposition::Vendored,
                    SourceSelection::Excluded => InventoryCaptureDisposition::Excluded,
                    SourceSelection::Unsupported => InventoryCaptureDisposition::Unsupported,
                }
            };
            bundle.dispositions.push(InventoryCaptureEntry {
                path: record.path.clone(),
                disposition,
            });
        }
        bundle.fence_current = observe_change_token() == Some(bundle.inventory.change_token())
            && current_source_generation(store, inventory.workspace_id)?
                == inventory.source_generation;
        Ok(())
    }

    fn admit_member(
        &mut self,
        store: &mut OperationalStore,
        record: &SourceInventoryRecord,
        outcome: Result<CaptureOutcome, SourceImageError>,
        images: &mut Vec<SourceImage>,
        remaining: &mut u64,
    ) -> Result<InventoryCaptureDisposition, SourceImageError> {
        match outcome {
            Ok(CaptureOutcome::Published(image)) => {
                let disposition = (|| {
                    Ok(if !image_matches_record(&image, record)? {
                        InventoryCaptureDisposition::Pending
                    } else if image.bytes.contains(&0) {
                        InventoryCaptureDisposition::Binary
                    } else if matches!(image.encoding, SourceEncoding::Unsupported { .. }) {
                        InventoryCaptureDisposition::UnsupportedEncoding
                    } else {
                        *remaining = remaining
                            .checked_sub(image.byte_length)
                            .ok_or(SourceImageError::InventoryCaptureBound)?;
                        InventoryCaptureDisposition::Captured {
                            image_index: images.len(),
                        }
                    })
                })();
                if matches!(
                    disposition,
                    Ok(InventoryCaptureDisposition::Captured { .. })
                ) {
                    images.push(*image);
                    return disposition;
                }
                self.release_without_observation(store, image.lease.lease_id)?;
                disposition
            }
            Ok(CaptureOutcome::Deferred) => Ok(InventoryCaptureDisposition::Deferred),
            Ok(CaptureOutcome::Excluded(gap)) => {
                Ok(if gap.observed_bytes == Some(record.byte_length) {
                    InventoryCaptureDisposition::Oversized
                } else {
                    InventoryCaptureDisposition::Pending
                })
            }
            Err(SourceImageError::StableRead(StableReadError::Secure(
                SecurePathError::SourceAccessDenied
                | SecurePathError::OperatingSystem
                | SecurePathError::OutsideAuthorizedRoot,
            ))) => Ok(InventoryCaptureDisposition::Unreadable),
            Err(error) => Err(error),
        }
    }

    /// Release the bundle's acquired holders after provider joins or rejected admission.
    ///
    /// # Errors
    /// Returns a durable-store failure. All holders are attempted even if one release fails.
    pub fn release_inventory_capture(
        &mut self,
        store: &mut OperationalStore,
        bundle: InventoryCaptureBundle,
    ) -> Result<(), SourceImageError> {
        let InventoryCaptureBundle { images, .. } = bundle;
        self.release_images(store, &images)
    }

    fn release_images(
        &mut self,
        store: &mut OperationalStore,
        images: &[SourceImage],
    ) -> Result<(), SourceImageError> {
        let mut failure = None;
        for image in images {
            if let Err(error) = self.release_without_observation(store, image.lease.lease_id) {
                failure = Some(error);
            }
        }
        let observation = self.refresh_lease_metrics(store);
        failure.map_or(observation, Err)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::ffi::OsStringExt as _;
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    use std::path::PathBuf;

    use super::*;
    use crate::inventory::{ChangedSourcePaths, InventoryLimits, InventoryWalker};
    use crate::secure_path::open_workspace_root;
    use crate::source_image::{
        DependencyInputBundle, SourceCapturePolicy, advance_source_generation,
        publish_provider_workspace_view,
    };
    use crate::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

    fn fixture() -> (
        tempfile::TempDir,
        OperationalStore,
        [u8; 16],
        PathBuf,
        SourceImageStore,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let workspace = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        let images = SourceImageStore::open(
            &directory.path().join("blobs"),
            SourceCapturePolicy {
                maximum_bytes: 16,
                ..SourceCapturePolicy::default()
            },
        )
        .unwrap();
        (directory, store, workspace, root, images)
    }

    fn walk(
        store: &mut OperationalStore,
        workspace: [u8; 16],
        generation: u64,
        token: u64,
    ) -> CompleteSourceInventory {
        let root = open_workspace_root(store, workspace).unwrap();
        InventoryWalker::new(InventoryLimits::default())
            .walk_selected_with_fence(
                &root,
                store,
                generation,
                token,
                &Cancellation::default(),
                || Some(token),
            )
            .unwrap()
    }

    const fn policy() -> SourceInventoryCapturePolicy {
        SourceInventoryCapturePolicy {
            maximum_total_bytes: 1_024,
            holder_kind: SourceBlobHolderKind::ProviderRun,
        }
    }

    fn capture(
        store: &mut OperationalStore,
        images: &mut SourceImageStore,
        inventory: &CompleteSourceInventory,
    ) -> InventoryCaptureBundle {
        images
            .capture_inventory_with_fence(
                store,
                inventory,
                policy(),
                &Cancellation::default(),
                |_| SourceSelection::Capture(SourceLanguage::Other),
                || Some(inventory.change_token()),
            )
            .unwrap()
    }

    fn create_disposition_sources(root: &std::path::Path) -> Vec<u8> {
        for (path, content) in [
            ("module.py", b"value = 1\n".as_slice()),
            ("config.toml", b"version = 3\n".as_slice()),
            ("binary.py", b"\0binary".as_slice()),
            ("encoding.rs", b"\xff\xfe".as_slice()),
            ("large.rs", b"012345678901234567890".as_slice()),
            ("generated.py", b"generated".as_slice()),
            ("vendor.py", b"vendor".as_slice()),
            ("excluded.py", b"excluded".as_slice()),
            ("unsupported.zz", b"unsupported".as_slice()),
        ] {
            fs::write(root.join(path), content).unwrap();
        }
        let raw_path = b"non-utf8-\xff.py".to_vec();
        fs::write(
            root.join(std::ffi::OsString::from_vec(raw_path.clone())),
            b"pass\n",
        )
        .unwrap();
        symlink("module.py", root.join("linked.py")).unwrap();
        raw_path
    }

    fn assert_dispositions(bundle: &InventoryCaptureBundle, raw_path: &[u8]) {
        assert!(bundle.is_closed());
        assert_eq!(bundle.images().len(), 3);
        let actual = bundle
            .dispositions()
            .iter()
            .map(|entry| {
                (
                    entry.path().raw_relative_path_bytes.clone(),
                    entry.disposition(),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for (path, disposition) in [
            (b"binary.py".as_slice(), InventoryCaptureDisposition::Binary),
            (
                b"encoding.rs".as_slice(),
                InventoryCaptureDisposition::UnsupportedEncoding,
            ),
            (
                b"large.rs".as_slice(),
                InventoryCaptureDisposition::Oversized,
            ),
            (
                b"generated.py".as_slice(),
                InventoryCaptureDisposition::Generated,
            ),
            (
                b"vendor.py".as_slice(),
                InventoryCaptureDisposition::Vendored,
            ),
            (
                b"excluded.py".as_slice(),
                InventoryCaptureDisposition::Excluded,
            ),
            (
                b"unsupported.zz".as_slice(),
                InventoryCaptureDisposition::Unsupported,
            ),
            (
                b"linked.py".as_slice(),
                InventoryCaptureDisposition::Excluded,
            ),
        ] {
            assert_eq!(actual.get(path), Some(&disposition));
        }
        assert!(
            bundle
                .images()
                .iter()
                .any(|image| image.path.raw_relative_path_bytes == raw_path
                    && image.bytes.as_ref() == b"pass\n")
        );
        assert!(
            bundle
                .images()
                .iter()
                .any(|image| image.path.raw_relative_path_bytes == b"config.toml"
                    && image.language == SourceLanguage::Other)
        );
    }

    #[test]
    fn rt_cpg_wp78_operations() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        let raw_path = create_disposition_sources(&root);
        let inventory = walk(&mut store, workspace, 0, 7);
        assert_eq!(inventory.inventory().records.len(), 11);
        let bundle = images
            .capture_inventory_with_fence(
                &mut store,
                &inventory,
                policy(),
                &Cancellation::default(),
                |record| match record.path.raw_relative_path_bytes.as_slice() {
                    b"generated.py" => SourceSelection::Generated,
                    b"vendor.py" => SourceSelection::Vendored,
                    b"excluded.py" => SourceSelection::Excluded,
                    b"unsupported.zz" => SourceSelection::Unsupported,
                    _ => SourceSelection::Capture(SourceLanguage::Other),
                },
                || Some(7),
            )
            .unwrap();
        assert_dispositions(&bundle, &raw_path);
        images
            .release_inventory_capture(&mut store, bundle)
            .unwrap();
        assert_eq!(images.metrics().live_holders, 0);

        let mut observations = 0;
        let racy = images
            .capture_inventory_with_fence(
                &mut store,
                &inventory,
                policy(),
                &Cancellation::default(),
                |_| SourceSelection::Capture(SourceLanguage::Other),
                || {
                    observations += 1;
                    if observations == 8 {
                        fs::write(root.join("module.py"), b"value = 2\n").unwrap();
                    }
                    if observations < 8 { Some(7) } else { None }
                },
            )
            .unwrap();
        assert!(!racy.is_closed());
        assert!(racy.require_closed().is_err());
        assert_eq!(racy.dispositions().len(), 11);
        assert!(racy.images().is_empty());
        assert_eq!(images.metrics().live_holders, 0);
        images.release_inventory_capture(&mut store, racy).unwrap();

        advance_source_generation(&mut store, workspace, 0).unwrap();
        assert!(matches!(
            images.capture_inventory_with_fence(
                &mut store,
                &inventory,
                policy(),
                &Cancellation::default(),
                |_| SourceSelection::Capture(SourceLanguage::Other),
                || Some(7)
            ),
            Err(SourceImageError::GenerationChanged)
        ));
        let next = walk(&mut store, workspace, 1, 8);
        let current = capture(&mut store, &mut images, &next);
        assert!(current.is_closed());
        assert!(
            current
                .images()
                .iter()
                .all(|image| image.source_generation == 1)
        );
        images
            .release_inventory_capture(&mut store, current)
            .unwrap();
        assert_eq!(images.metrics().live_holders, 0);
    }

    #[test]
    fn rt_cpg_wp78_capture_and_release_observe_leases_once_per_batch() {
        for file_count in [8_u64, 64] {
            let (_directory, mut store, workspace, root, mut images) = fixture();
            for index in 0..file_count {
                fs::write(root.join(format!("module_{index:03}.py")), b"pass\n").unwrap();
            }
            // A rejected image acquires then releases a holder inside the batch.
            fs::write(root.join("binary.py"), b"\0bin").unwrap();
            let inventory = walk(&mut store, workspace, 0, 0);
            let before = images.lease_metric_reads;
            let bundle = capture(&mut store, &mut images, &inventory);
            assert_eq!(bundle.images().len() as u64, file_count);
            assert_eq!(images.lease_metric_reads - before, 1);
            assert_eq!(images.metrics().live_holders, file_count);
            assert_eq!(durable_lease_count(&store), file_count);
            assert_eq!(images.metrics().acquired_leases, file_count + 1);
            assert_eq!(images.metrics().released_leases, 1);
            let before = images.lease_metric_reads;
            images
                .release_inventory_capture(&mut store, bundle)
                .unwrap();
            assert_eq!(images.lease_metric_reads - before, 1);
            assert_eq!(images.metrics().live_holders, 0);
            assert_eq!(durable_lease_count(&store), 0);
            assert_eq!(images.metrics().released_leases, file_count + 1);
        }
    }

    fn durable_lease_count(store: &OperationalStore) -> u64 {
        store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row("SELECT count(*) FROM source_blob_lease", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn rt_cpg_wp78_metric_read_failure_never_strands_batch_holders() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("a.py"), b"pass\n").unwrap();
        fs::write(root.join("b.py"), b"pass\n").unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        images.fail_next_lease_metric_read = true;
        assert!(
            images
                .capture_inventory_with_fence(
                    &mut store,
                    &inventory,
                    policy(),
                    &Cancellation::default(),
                    |_| SourceSelection::Capture(SourceLanguage::Other),
                    || Some(0),
                )
                .is_err()
        );
        assert_eq!(images.metrics().acquired_leases, 0);
        assert_eq!(durable_lease_count(&store), 0);
        let bundle = capture(&mut store, &mut images, &inventory);
        assert_eq!(durable_lease_count(&store), 2);
        images.fail_next_lease_metric_read = true;
        assert!(
            images
                .release_inventory_capture(&mut store, bundle)
                .is_err()
        );
        assert_eq!(
            durable_lease_count(&store),
            0,
            "all durable releases precede fallible observation"
        );
        assert_eq!(images.metrics().live_holders, 0);
    }

    #[test]
    fn rt_cpg_wp78_member_validation_failure_releases_current_holder() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("a.py"), b"pass\n").unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        let record = &inventory.inventory().records[0];
        let request = CaptureRequest {
            workspace_id: workspace,
            source_generation: 0,
            change_token: 0,
            path: PlatformPath::from_raw_relative_bytes(
                record.path.platform_code,
                b"a.py".to_vec(),
            )
            .unwrap(),
            language: SourceLanguage::Python,
            holder_kind: SourceBlobHolderKind::ProviderRun,
            holder_id: random_registration_nonce().unwrap(),
        };
        let outcome = images.capture(&mut store, &request);
        assert_eq!(durable_lease_count(&store), 1);
        let mut accepted = Vec::new();
        assert!(matches!(
            images.admit_member(&mut store, record, outcome, &mut accepted, &mut 0),
            Err(SourceImageError::InventoryCaptureBound)
        ));
        assert!(accepted.is_empty());
        assert_eq!(durable_lease_count(&store), 0);
    }

    #[test]
    fn complete_capture_rejects_missing_member_and_exact_digest_drift() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("a.py"), b"x = 1\n").unwrap();
        fs::write(root.join("b.py"), b"x = 2\n").unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        let mut bundle = capture(&mut store, &mut images, &inventory);
        let removed = bundle.dispositions.pop().unwrap();
        assert!(matches!(
            bundle.validate_bindings(),
            Err(SourceImageError::InventoryCaptureMismatch)
        ));
        bundle.dispositions.push(removed);
        bundle.images[0].digest = [0; 32];
        assert!(matches!(
            bundle.validate_bindings(),
            Err(SourceImageError::InventoryCaptureMismatch)
        ));
        images
            .release_inventory_capture(&mut store, bundle)
            .unwrap();
        assert_eq!(images.metrics().live_holders, 0);

        fs::write(root.join("a.py"), b"x = 9\n").unwrap();
        fs::remove_file(root.join("b.py")).unwrap();
        let stale = capture(&mut store, &mut images, &inventory);
        assert_eq!(stale.dispositions.len(), 2);
        assert_eq!(
            stale.dispositions[0].disposition,
            InventoryCaptureDisposition::Pending
        );
        assert_eq!(
            stale.dispositions[1].disposition,
            InventoryCaptureDisposition::Unreadable
        );
        assert!(!stale.is_closed());
        assert!(stale.images().is_empty());
        assert_eq!(images.metrics().live_holders, 0);
    }

    #[test]
    fn complete_inventory_retains_unreadable_member_and_releases_partial_cancellation() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("a.py"), b"pass\n").unwrap();
        fs::write(root.join("b.py"), b"pass\n").unwrap();
        fs::set_permissions(root.join("b.py"), fs::Permissions::from_mode(0o000)).unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        assert_eq!(inventory.inventory().records.len(), 2);
        assert_eq!(
            inventory.read_failure(b"b.py"),
            Some(InventoryReadFailure::Unreadable)
        );
        let bundle = capture(&mut store, &mut images, &inventory);
        assert_eq!(
            bundle.dispositions()[1].disposition(),
            InventoryCaptureDisposition::Unreadable
        );
        assert_eq!(bundle.images().len(), 1);
        images
            .release_inventory_capture(&mut store, bundle)
            .unwrap();
        fs::set_permissions(root.join("b.py"), fs::Permissions::from_mode(0o600)).unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        let cancellation = Cancellation::default();
        let mut calls = 0;
        let bundle = images
            .capture_inventory_with_fence(
                &mut store,
                &inventory,
                policy(),
                &cancellation,
                |_| {
                    calls += 1;
                    if calls == 1 {
                        cancellation.cancel();
                    }
                    SourceSelection::Capture(SourceLanguage::Other)
                },
                || Some(0),
            )
            .unwrap();
        assert!(!bundle.is_closed());
        assert_eq!(
            bundle.dispositions()[0].disposition(),
            InventoryCaptureDisposition::Deferred
        );
        assert_eq!(
            bundle.dispositions()[1].disposition(),
            InventoryCaptureDisposition::Pending
        );
        assert!(bundle.images().is_empty());
        assert_eq!(images.metrics().live_holders, 0);
    }

    #[test]
    fn complete_capture_bounds_cancellation_and_fence_release_every_holder() {
        let (_directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("a.py"), b"123456").unwrap();
        fs::write(root.join("b.py"), b"123456").unwrap();
        let inventory = walk(&mut store, workspace, 0, 0);
        let limited = SourceInventoryCapturePolicy {
            maximum_total_bytes: 8,
            ..policy()
        };
        assert!(matches!(
            images.capture_inventory_with_fence(
                &mut store,
                &inventory,
                limited,
                &Cancellation::default(),
                |_| SourceSelection::Capture(SourceLanguage::Other),
                || Some(0)
            ),
            Err(SourceImageError::InventoryCaptureBound)
        ));
        assert_eq!(images.metrics().live_holders, 0);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let cancelled = images
            .capture_inventory_with_fence(
                &mut store,
                &inventory,
                policy(),
                &cancellation,
                |_| SourceSelection::Capture(SourceLanguage::Other),
                || Some(0),
            )
            .unwrap();
        assert!(!cancelled.is_closed());
        assert_eq!(cancelled.dispositions.len(), 2);
        assert!(
            cancelled
                .dispositions
                .iter()
                .all(|entry| entry.disposition == InventoryCaptureDisposition::Pending)
        );
        assert_eq!(images.metrics().live_holders, 0);
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let mut count = 0;
        assert!(matches!(
            InventoryWalker::new(InventoryLimits::default()).walk_selected_with_fence(
                &root,
                &mut store,
                0,
                0,
                &Cancellation::default(),
                || {
                    count += 1;
                    if count == 1 { Some(0) } else { Some(1) }
                }
            ),
            Err(crate::inventory::InventoryError::SourceChanged)
        ));
    }

    #[test]
    fn complete_inventory_delete_last_and_empty_provider_view_are_valid() {
        let (directory, mut store, workspace, root, mut images) = fixture();
        fs::write(root.join("last.py"), b"pass\n").unwrap();
        let first = walk(&mut store, workspace, 0, 0);
        let changed = ChangedSourcePaths::try_new(&first, [b"last.py".to_vec()]).unwrap();
        assert_eq!(changed.paths().len(), 1);
        assert_eq!(first.inventory().records.len(), 1);
        fs::remove_file(root.join("last.py")).unwrap();
        advance_source_generation(&mut store, workspace, 0).unwrap();
        let empty = walk(&mut store, workspace, 1, 1);
        let removed =
            ChangedSourcePaths::try_between(&first, &empty, [b"last.py".to_vec()]).unwrap();
        assert!(removed.paths().is_empty());
        assert_eq!(
            removed.removed_paths(),
            &BTreeSet::from([b"last.py".to_vec()])
        );
        assert!(ChangedSourcePaths::try_between(&first, &empty, [b"never.py".to_vec()]).is_err());
        assert!(ChangedSourcePaths::try_new(&empty, [b"last.py".to_vec()]).is_err());
        let bundle = capture(&mut store, &mut images, &empty);
        assert!(bundle.is_closed());
        assert!(bundle.dispositions().is_empty());
        assert!(bundle.images().is_empty());
        let view = publish_provider_workspace_view(
            &directory.path().join("providers"),
            "empty",
            workspace,
            1,
            &[],
            &DependencyInputBundle::empty(),
        )
        .unwrap();
        assert!(view.entries.is_empty());
        assert_eq!(view.workspace_id, workspace);
        assert_eq!(view.source_generation, 1);
        images
            .release_inventory_capture(&mut store, bundle)
            .unwrap();
        fs::write(root.join("last.py"), b"pass\n").unwrap();
        advance_source_generation(&mut store, workspace, 1).unwrap();
        let recreated = walk(&mut store, workspace, 2, 2);
        let bundle = capture(&mut store, &mut images, &recreated);
        assert!(bundle.is_closed());
        assert_eq!(
            bundle.images()[0].file_id,
            first.inventory().records[0].file_id.unwrap()
        );
        assert_eq!(bundle.images()[0].source_generation, 2);
        images
            .release_inventory_capture(&mut store, bundle)
            .unwrap();
    }
}
