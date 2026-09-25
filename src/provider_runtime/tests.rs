use super::*;
use std::os::unix::{ffi::OsStrExt as _, fs::symlink};

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, ResourceBudget) {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("usr");
    let cache = fixture.path().join("cache");
    fs::create_dir(&source).unwrap();
    private_directory(&cache).unwrap();
    fs::create_dir(source.join("lib")).unwrap();
    fs::write(source.join("lib/runtime"), b"runtime bytes").unwrap();
    (
        fixture,
        source,
        cache,
        crate::fabric::workspace_resources::test_workspace_budget(),
    )
}

fn observed(source: &Path, budget: &ResourceBudget) -> [u8; 32] {
    observation::observe_root(source, budget, &Cancellation::default()).unwrap()
}

#[test]
fn immutable_runtime_preserves_raw_inputs_reopens_and_keeps_leased_images_during_replacement() {
    let (_fixture, source, cache, budget) = fixture();
    let raw = std::ffi::OsStr::from_bytes(b"module-\xff");
    fs::write(source.join(raw), b"raw").unwrap();
    symlink("../absent", source.join("lib/link")).unwrap();
    let cancel = Cancellation::default();
    let mut first = RuntimeCache::default();
    let identity = observed(&source, &budget);
    let old = first
        .acquire_at(&source, &cache, identity, &budget, &cancel)
        .unwrap();
    assert_eq!(fs::read(old.root().join(raw)).unwrap(), b"raw");
    assert_eq!(
        fs::read_link(old.root().join("lib/link")).unwrap(),
        Path::new("../absent")
    );
    assert_eq!(
        fs::metadata(old.root().join("lib/runtime")).unwrap().mode() & 0o222,
        0
    );
    assert_ne!(
        fs::metadata(source.join("lib/runtime")).unwrap().ino(),
        fs::metadata(old.root().join("lib/runtime")).unwrap().ino()
    );
    let old_root = old.root().to_owned();
    let old_digest = old.digest();
    let mut second = RuntimeCache::default();
    let reopened = second
        .acquire_at(&source, &cache, identity, &budget, &cancel)
        .unwrap();
    assert_eq!(reopened, old);
    fs::write(source.join("lib/runtime"), b"changed bytes").unwrap();
    fs::write(source.join("previously-absent"), b"new dependency").unwrap();
    let current = second
        .acquire_at(
            &source,
            &cache,
            observed(&source, &budget),
            &budget,
            &cancel,
        )
        .unwrap();
    assert_ne!(current.digest(), old_digest);
    assert_eq!(
        fs::read(old.root().join("lib/runtime")).unwrap(),
        b"runtime bytes"
    );
    assert!(!old.root().join("previously-absent").exists());
    assert_eq!(
        fs::read(current.root().join("lib/runtime")).unwrap(),
        b"changed bytes"
    );
    drop((old, reopened, first));
    let lock = lock_file(&cache.join("store.lock")).unwrap();
    lock.try_lock().unwrap();
    collect(
        &cache,
        current
            .root()
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        &cancel,
        Instant::now(),
    )
    .unwrap();
    assert!(!old_root.exists(), "unleased predecessor is reclaimable");
    assert!(
        current.root().exists(),
        "current independent lease is protected"
    );
    drop((current, second));
    assert_eq!(budget.observation().used.disk_bytes, 0);
    assert_eq!(budget.observation().used.memory_bytes, 0);
}

#[test]
fn runtime_capture_rejects_changed_inputs_special_files_and_modified_cached_bytes() {
    let (_fixture, source, cache, budget) = fixture();
    let mut owner = RuntimeCache::default();
    let cancel = Cancellation::default();
    let old = observed(&source, &budget);
    fs::write(source.join("new"), b"new").unwrap();
    assert!(
        owner
            .acquire_at(&source, &cache, old, &budget, &cancel)
            .is_err()
    );
    rustix::fs::mknodat(
        rustix::fs::CWD,
        source.join("fifo"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(
        owner
            .acquire_at(
                &source,
                &cache,
                observed(&source, &budget),
                &budget,
                &cancel
            )
            .is_err()
    );
    assert!(!fs::read_dir(&cache).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .starts_with(".stage-")
    }));
    fs::remove_file(source.join("fifo")).unwrap();
    let witness = observed(&source, &budget);
    let image = owner
        .acquire_at(&source, &cache, witness, &budget, &cancel)
        .unwrap();
    let path = image.root().join("lib/runtime");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, b"tampered").unwrap();
    assert!(
        owner
            .acquire_at(&source, &cache, witness, &budget, &cancel)
            .unwrap_err()
            .to_string()
            .contains("modified")
    );
    assert!(
        image.root().exists(),
        "a corrupt leased image is reported, not overwritten"
    );
    drop((owner, image));
    assert_eq!(budget.observation().used.disk_bytes, 0);
}

#[test]
fn runtime_capture_cancellation_and_disk_capacity_release_staging_and_shared_budget() {
    let (fixture, source, cache, budget) = fixture();
    let witness = observed(&source, &budget);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        RuntimeCache::default()
            .acquire_at(&source, &cache, witness, &budget, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    let stage = fixture.path().join("capacity");
    let headroom = LocalDiskHeadroom::open(fixture.path()).unwrap();
    let result = capture::copy(
        &source,
        &stage,
        &budget,
        &headroom,
        &Cancellation::default(),
        Instant::now(),
        1,
    );
    assert!(result.is_err());
    remove_image(&stage).unwrap();
    assert_eq!(budget.observation().used.memory_bytes, 0);
    assert_eq!(headroom.observe().unwrap().in_flight_growth_bytes, 0);
}

/// Actual process-owner assurance uses the same capture and lease constructor as production.
pub(crate) fn image_fixture() -> (tempfile::TempDir, RuntimeImage, ResourceBudget) {
    let (fixture, source, cache, budget) = fixture();
    let image = RuntimeCache::default()
        .acquire_at(
            &source,
            &cache,
            observed(&source, &budget),
            &budget,
            &Cancellation::default(),
        )
        .unwrap();
    (fixture, image, budget)
}
