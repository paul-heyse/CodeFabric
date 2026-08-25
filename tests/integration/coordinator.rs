use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use codefabric::coordinator::{
    CoordinatorError, WorkspaceCoordinatorManager, WorkspaceCoordinatorState,
    persisted_workspace_health,
};
use codefabric::operational_store::OperationalStore;
use codefabric::registries::{
    EventStreamHealth, GitAccelerationStatus, SourceTrustState, WorkspaceRegistryLifecycle,
};
use codefabric::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};
use tokio::sync::Mutex;

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run Git fixture command");
    assert!(
        output.status.success(),
        "Git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_fixture(root: &Path, linked: &Path) {
    fs::create_dir(root).expect("repository root");
    git(root, &["-c", "init.defaultBranch=main", "init"]);
    git(root, &["config", "user.name", "CodeFabric Test"]);
    git(
        root,
        &["config", "user.email", "codefabric@example.invalid"],
    );
    fs::write(root.join("tracked.rs"), b"fn main_branch() {}\n").expect("tracked source");
    git(root, &["add", "--all"]);
    git(root, &["commit", "-m", "fixture"]);
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().expect("UTF-8 test path"),
        ],
    );
}

async fn register_enabled(
    store: &Arc<Mutex<OperationalStore>>,
    root: &Path,
    source: WorkspaceSourceRegistration,
) -> [u8; 16] {
    let mut store = store.lock().await;
    let mut registry = WorkspaceRegistry::new(&mut store);
    let workspace = registry.add(root, source).expect("register workspace");
    registry
        .enable(workspace.workspace_id)
        .expect("enable workspace");
    workspace.workspace_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wp18_behavioral_acceptance() {
    let fixture = tempfile::tempdir().expect("coordinator fixture");
    let directory = fixture.path().join("directory");
    let repository = fixture.path().join("repository");
    let linked = fixture.path().join("linked");
    fs::create_dir(&directory).expect("directory workspace");
    fs::write(directory.join("source.py"), b"before = 1\n").expect("source fixture");
    git_fixture(&repository, &linked);

    let store = Arc::new(Mutex::new(
        OperationalStore::open(&fixture.path().join("operational.sqlite3"))
            .expect("operational store"),
    ));
    let directory_id =
        register_enabled(&store, &directory, WorkspaceSourceRegistration::Directory).await;
    let main_id = register_enabled(
        &store,
        &repository,
        WorkspaceSourceRegistration::NewGitRepository {
            worktree_administrative_key: b"main".to_vec(),
            worktree_kind: "main".to_owned(),
        },
    )
    .await;
    let repository_id = {
        let mut store = store.lock().await;
        WorkspaceRegistry::new(&mut store)
            .show(main_id)
            .expect("main registration")
            .repository_id
            .expect("repository identity")
    };
    let linked_id = register_enabled(
        &store,
        &linked,
        WorkspaceSourceRegistration::ExistingGitRepository {
            repository_id,
            worktree_administrative_key: b"linked".to_vec(),
            worktree_kind: "linked".to_owned(),
        },
    )
    .await;

    let mut manager =
        WorkspaceCoordinatorManager::new(Arc::clone(&store), fixture.path().join("daemon-state"))
            .expect("manager");
    let directory_handle = manager.spawn(directory_id).await.expect("directory actor");
    let changed_path = directory.join("source.py");
    let changed = Arc::new(move || {
        fs::write(&changed_path, b"after = 2\n").expect("G0/G1 source mutation");
    });
    let directory_state = directory_handle
        .bootstrap_with_hook(changed)
        .await
        .expect("reconciled directory bootstrap");
    assert_eq!(directory_state.reconciliation_count, 1);

    let main_state = manager
        .spawn(main_id)
        .await
        .expect("main actor")
        .bootstrap()
        .await
        .expect("main bootstrap");
    let linked_state = manager
        .spawn(linked_id)
        .await
        .expect("linked actor")
        .bootstrap()
        .await
        .expect("linked bootstrap");
    for state in [&directory_state, &main_state, &linked_state] {
        assert_eq!(state.lifecycle, WorkspaceRegistryLifecycle::Bootstrapping);
        assert_eq!(state.source_trust, SourceTrustState::Current);
        assert_eq!(state.event_stream_health, EventStreamHealth::Unavailable);
        assert!(state.inventory_digest.is_some());
        assert!(state.active_snapshot.is_none());
        assert!(!state.is_ready());
    }
    assert_eq!(
        directory_state.git_acceleration,
        GitAccelerationStatus::NotAGitWorktree
    );
    assert_eq!(main_state.git_acceleration, GitAccelerationStatus::GitReady);
    assert_eq!(
        linked_state.git_acceleration,
        GitAccelerationStatus::GitReady
    );
    assert_ne!(main_state.workspace_id, linked_state.workspace_id);

    manager.shutdown_all().await.expect("joined actors");
    let mut restarted =
        WorkspaceCoordinatorManager::new(Arc::clone(&store), fixture.path().join("daemon-state"))
            .expect("restart");
    let restored = restarted
        .restore_and_bootstrap()
        .await
        .expect("restart reverification");
    assert_eq!(restored.len(), 3);
    assert!(restored.iter().all(|state| {
        state.source_generation == 2
            && state.source_trust == SourceTrustState::Current
            && state.active_snapshot.is_none()
    }));
    restarted.shutdown_all().await.expect("restart join");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp18_structural_acceptance() {
    let fixture = tempfile::tempdir().expect("structural fixture");
    let root = fixture.path().join("workspace");
    fs::create_dir(&root).expect("workspace root");
    let store = Arc::new(Mutex::new(
        OperationalStore::open(&fixture.path().join("operational.sqlite3"))
            .expect("operational store"),
    ));
    let workspace_id =
        register_enabled(&store, &root, WorkspaceSourceRegistration::Directory).await;
    let mut manager =
        WorkspaceCoordinatorManager::new(Arc::clone(&store), fixture.path().join("daemon-state"))
            .expect("manager");
    let handle = manager.spawn(workspace_id).await.expect("sole actor");
    assert!(matches!(
        manager.spawn(workspace_id).await,
        Err(CoordinatorError::DuplicateCoordinator)
    ));
    assert_eq!(manager.active_count(), 1);
    assert_eq!(
        handle
            .status()
            .await
            .expect("initial state")
            .mutations_applied,
        0
    );
    handle.bootstrap().await.expect("receiver mutation");
    assert_eq!(
        handle
            .status()
            .await
            .expect("mutated state")
            .mutations_applied,
        1
    );

    let source = include_str!("../../src/coordinator.rs");
    assert_eq!(source.matches("bootstrap_sync(").count(), 2);
    assert!(source.contains("receiver.recv().await"));
    assert!(!include_str!("../../Cargo.toml").contains("loom"));
    manager.shutdown_all().await.expect("joined actor");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp18_negative_zero_state() {
    let fixture = tempfile::tempdir().expect("negative fixture");
    let root = fixture.path().join("workspace");
    fs::create_dir(&root).expect("workspace root");
    let store = Arc::new(Mutex::new(
        OperationalStore::open(&fixture.path().join("operational.sqlite3"))
            .expect("operational store"),
    ));
    let workspace_id =
        register_enabled(&store, &root, WorkspaceSourceRegistration::Directory).await;
    let mut manager =
        WorkspaceCoordinatorManager::new(Arc::clone(&store), fixture.path().join("daemon-state"))
            .expect("manager");
    let handle = manager.spawn(workspace_id).await.expect("actor");
    let restart_state = handle.status().await.expect("restart state");
    assert_eq!(restart_state.source_trust, SourceTrustState::Unverified);
    assert!(restart_state.active_snapshot.is_none());
    assert!(matches!(
        restart_state.require_ready(),
        Err(CoordinatorError::WorkspaceBootstrapping)
    ));

    for lifecycle in [
        WorkspaceRegistryLifecycle::Bootstrapping,
        WorkspaceRegistryLifecycle::Ready,
        WorkspaceRegistryLifecycle::Degraded,
    ] {
        for trust in [
            SourceTrustState::Unverified,
            SourceTrustState::Verifying,
            SourceTrustState::Current,
            SourceTrustState::PotentiallyStale,
        ] {
            for snapshot in [None, Some([0x55; 16])] {
                let state = WorkspaceCoordinatorState {
                    workspace_id,
                    lifecycle,
                    source_trust: trust,
                    event_stream_health: EventStreamHealth::Unavailable,
                    git_acceleration: GitAccelerationStatus::NotAGitWorktree,
                    source_generation: 0,
                    inventory_digest: None,
                    git_state: None,
                    active_snapshot: snapshot,
                    reconciliation_count: 0,
                    mutations_applied: 0,
                };
                assert_eq!(
                    state.is_ready(),
                    lifecycle == WorkspaceRegistryLifecycle::Ready
                        && trust == SourceTrustState::Current
                        && snapshot.is_some()
                );
            }
        }
    }
    manager.shutdown_all().await.expect("joined actor");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp18_operational_acceptance() {
    let fixture = tempfile::tempdir().expect("operational fixture");
    let root = fixture.path().join("workspace-secret-name");
    fs::create_dir(&root).expect("workspace root");
    fs::write(root.join("source.rs"), b"fn source() {}\n").expect("source");
    let store = Arc::new(Mutex::new(
        OperationalStore::open(&fixture.path().join("operational.sqlite3"))
            .expect("operational store"),
    ));
    let workspace_id =
        register_enabled(&store, &root, WorkspaceSourceRegistration::Directory).await;
    let mut manager =
        WorkspaceCoordinatorManager::new(Arc::clone(&store), fixture.path().join("daemon-state"))
            .expect("manager");
    manager
        .spawn(workspace_id)
        .await
        .expect("actor")
        .bootstrap()
        .await
        .expect("bootstrap");
    let health = {
        let store = store.lock().await;
        persisted_workspace_health(&store).expect("persisted health")
    };
    assert_eq!(health.len(), 1);
    assert!(health[0].workspace_id.starts_with("workspace:"));
    assert_eq!(health[0].readiness, "WORKSPACE_BOOTSTRAPPING");
    assert_eq!(health[0].source_trust, "CURRENT");
    assert!(
        health[0]
            .inventory_digest
            .as_deref()
            .is_some_and(|value| value.starts_with("b3:"))
    );
    let json = serde_json::to_string(&health).expect("health JSON");
    for forbidden in [
        "workspace-secret-name",
        "authorization_fingerprint",
        "root_path",
        "credential",
        "config_root",
    ] {
        assert!(!json.contains(forbidden));
    }
    manager.shutdown_all().await.expect("joined actor");
}
