use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use codefabric::git_state::{
    GitBlockingExecutor, GitCancellation, GitHashAlgorithm, GitInventoryClassification,
    GitOperationState, GitStateAdapter, GitStateError, GitStateObservations, GitTrustPolicy,
    GixGitStateAdapter, HeadKind, RegisteredGitIdentity, apply_to_source_inventory,
};
use codefabric::inventory::{InventoryCancellation, InventoryLimits, InventoryWalker};
use codefabric::operational_store::OperationalStore;
use codefabric::secure_path::open_workspace_root;
use codefabric::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

const REGISTERED: RegisteredGitIdentity = RegisteredGitIdentity {
    repository_id: [0x11; 16],
    worktree_id: [0x22; 16],
};
const OBSERVATIONS: GitStateObservations = GitStateObservations {
    inclusion_policy_fingerprint: [0x31; 32],
    attributes_fingerprint: [0x32; 32],
    worktree_inventory_digest: [0x33; 32],
};

fn git(path: &Path, arguments: &[&OsStr]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(arguments)
        .output()
        .expect("run Git fixture command");
    assert!(
        output.status.success(),
        "git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git_input(path: &Path, arguments: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Git fixture command");
    child
        .stdin
        .take()
        .expect("Git fixture stdin")
        .write_all(input)
        .expect("write Git fixture input");
    let output = child.wait_with_output().expect("Git fixture output");
    assert!(
        output.status.success(),
        "git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn args<const N: usize>(values: [&str; N]) -> [&OsStr; N] {
    values.map(OsStr::new)
}

fn init_repository(root: &Path, object_format: Option<&str>) {
    fs::create_dir_all(root).expect("fixture repository root");
    let mut arguments = vec![
        OsStr::new("-c"),
        OsStr::new("init.defaultBranch=main"),
        OsStr::new("init"),
    ];
    let object_format_argument;
    if let Some(format) = object_format {
        object_format_argument = format!("--object-format={format}");
        arguments.push(OsStr::new(&object_format_argument));
    }
    git(root, &arguments);
    git(root, &args(["config", "user.name", "CodeFabric Test"]));
    git(
        root,
        &args(["config", "user.email", "codefabric@example.invalid"]),
    );
    // Git may launch detached auto-maintenance after a fixture mutation. Disable it so
    // the read-only gix oracle measures only the adapter under test.
    git(root, &args(["config", "maintenance.auto", "false"]));
    git(root, &args(["config", "gc.auto", "0"]));
}

fn commit_file(root: &Path, path: &str, bytes: &[u8]) {
    let destination = root.join(path);
    fs::create_dir_all(destination.parent().expect("fixture file parent")).expect("fixture parent");
    fs::write(destination, bytes).expect("fixture file");
    git(root, &args(["add", "--all"]));
    git(root, &args(["commit", "-m", "fixture"]));
}

fn snapshot_path(root: &Path) -> BTreeMap<Vec<u8>, Vec<u8>> {
    fn visit(base: &Path, path: &Path, out: &mut BTreeMap<Vec<u8>, Vec<u8>>) {
        let metadata = fs::symlink_metadata(path).expect("snapshot metadata");
        let relative = path
            .strip_prefix(base)
            .expect("snapshot relative path")
            .as_os_str()
            .as_bytes()
            .to_vec();
        if metadata.file_type().is_symlink() {
            out.insert(
                relative,
                fs::read_link(path)
                    .expect("snapshot symlink")
                    .as_os_str()
                    .as_bytes()
                    .to_vec(),
            );
        } else if metadata.is_file() {
            out.insert(relative, fs::read(path).expect("snapshot file"));
        } else if metadata.is_dir() {
            let mut children = fs::read_dir(path)
                .expect("snapshot directory")
                .map(|entry| entry.expect("snapshot entry").path())
                .collect::<Vec<_>>();
            children.sort_by(|left, right| {
                left.as_os_str()
                    .as_bytes()
                    .cmp(right.as_os_str().as_bytes())
            });
            for child in children {
                visit(base, &child, out);
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one named oracle proves the complete required Git fixture matrix"
)]
fn wp17_behavioral_acceptance() {
    let fixture = tempfile::tempdir().expect("Git fixture root");
    let main = fixture.path().join("main");
    init_repository(&main, None);
    commit_file(&main, "src/lib.rs", b"fn main_fixture() {}\n");
    fs::write(main.join(".gitignore"), b"ignored.bin\n").expect("ignore file");
    fs::write(main.join("ignored.bin"), b"ignored\n").expect("ignored file");
    fs::write(main.join("untracked.txt"), b"untracked\n").expect("untracked file");
    git(&main, &args(["add", ".gitignore"]));
    git(&main, &args(["commit", "-m", "ignore policy"]));

    let head = git(&main, &args(["rev-parse", "HEAD"]));
    let head = String::from_utf8(head)
        .expect("ASCII object ID")
        .trim()
        .to_owned();
    let linked_a = fixture.path().join("linked-a");
    let linked_b = fixture.path().join("linked-b");
    git(
        &main,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-b"),
            OsStr::new("linked-a"),
            linked_a.as_os_str(),
        ],
    );
    git(
        &main,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-b"),
            OsStr::new("linked-b"),
            linked_b.as_os_str(),
        ],
    );

    let adapter = GixGitStateAdapter;
    let policy = GitTrustPolicy::local_read_only();
    let snapshot = adapter
        .open_worktree(&main, REGISTERED, &policy)
        .expect("open main worktree");
    assert_eq!(snapshot.repository.object_format, GitHashAlgorithm::Sha1);
    assert!(snapshot.selected_worktree.is_main_worktree);
    assert_eq!(snapshot.linked_worktrees.len(), 2);
    let linked = adapter
        .open_worktree(&linked_a, REGISTERED, &policy)
        .expect("exact-path linked worktree open");
    assert!(!linked.selected_worktree.is_main_worktree);
    assert_eq!(linked.selected_worktree.administrative_name, b"linked-a");

    git(&main, &args(["checkout", "--detach"]));
    assert_eq!(
        adapter
            .capture_state(&snapshot.selected_worktree, OBSERVATIONS)
            .expect("detached state")
            .head_kind,
        HeadKind::Detached
    );

    // APFS rejects invalid UTF-8 host filenames, but Git's index remains byte-native.
    // Seed the exact byte path directly in the test-only index fixture.
    let non_utf8 = b"non-utf8-\xff.rs".to_vec();
    let blob = git_input(
        &main,
        &["hash-object", "-w", "--stdin"],
        b"fn byte_path() {}\n",
    );
    let mut index_info = b"100644 ".to_vec();
    index_info.extend_from_slice(blob.strip_suffix(b"\n").expect("object ID newline"));
    index_info.push(b'\t');
    index_info.extend_from_slice(&non_utf8);
    index_info.push(b'\n');
    git_input(&main, &["update-index", "--index-info"], &index_info);
    let cache_info = format!("160000,{head},vendor/sub");
    git(
        &main,
        &[
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            OsStr::new(&cache_info),
        ],
    );

    let inventory = adapter
        .inventory(
            &snapshot.selected_worktree,
            OBSERVATIONS,
            &GitCancellation::default(),
        )
        .expect("Git-native inventory");
    let classification = |path: &[u8]| {
        inventory
            .entries
            .iter()
            .find(|entry| entry.repo_path_bytes == path)
            .map(|entry| entry.classification)
    };
    assert_eq!(
        classification(b"src/lib.rs"),
        Some(GitInventoryClassification::Tracked)
    );
    assert_eq!(
        classification(b"ignored.bin"),
        Some(GitInventoryClassification::UntrackedIgnored)
    );
    assert_eq!(
        classification(b"untracked.txt"),
        Some(GitInventoryClassification::UntrackedNotIgnored)
    );
    assert_eq!(
        classification(&non_utf8),
        Some(GitInventoryClassification::Tracked)
    );
    assert_eq!(
        classification(b"vendor/sub"),
        Some(GitInventoryClassification::SubmoduleGitlink)
    );
    assert_eq!(inventory.vector.inclusion_policy_fingerprint, [0x31; 32]);

    fs::write(main.join(".git/MERGE_HEAD"), format!("{head}\n")).expect("merge marker");
    assert_eq!(
        adapter
            .capture_state(&snapshot.selected_worktree, OBSERVATIONS)
            .expect("merge state")
            .repository_state,
        GitOperationState::Merge
    );

    let unborn = fixture.path().join("unborn");
    init_repository(&unborn, None);
    let unborn = adapter
        .open_worktree(&unborn, REGISTERED, &policy)
        .expect("unborn open");
    assert_eq!(
        adapter
            .capture_state(&unborn.selected_worktree, OBSERVATIONS)
            .expect("unborn state")
            .head_kind,
        HeadKind::Unborn
    );

    let sha256 = fixture.path().join("sha256");
    init_repository(&sha256, Some("sha256"));
    commit_file(&sha256, "sha256.txt", b"sha256\n");
    let sha256 = adapter
        .open_worktree(&sha256, REGISTERED, &policy)
        .expect("SHA-256 open");
    assert_eq!(sha256.repository.object_format, GitHashAlgorithm::Sha256);
    assert_eq!(
        adapter
            .capture_state(&sha256.selected_worktree, OBSERVATIONS)
            .expect("SHA-256 state")
            .head_target
            .expect("born SHA-256 HEAD")
            .bytes
            .len(),
        32
    );

    let bare = fixture.path().join("bare.git");
    fs::create_dir(&bare).expect("bare fixture root");
    git(&bare, &args(["init", "--bare"]));
    let bare = adapter
        .open_worktree(&bare, REGISTERED, &policy)
        .expect("bare open");
    assert!(bare.selected_worktree.is_bare);
    assert!(bare.selected_worktree.work_dir.is_none());
}

#[test]
fn wp17_structural_acceptance() {
    let fixture = tempfile::tempdir().expect("structural fixture");
    let root = fixture.path().join("repository");
    init_repository(&root, None);
    commit_file(&root, "tracked.rs", b"fn tracked() {}\n");
    let adapter = GixGitStateAdapter;
    let snapshot = adapter
        .open_worktree(&root, REGISTERED, &GitTrustPolicy::local_read_only())
        .expect("open fixture");
    let vector = adapter
        .capture_state(&snapshot.selected_worktree, OBSERVATIONS)
        .expect("capture complete vector");
    assert_eq!(vector.repository_id, REGISTERED.repository_id);
    assert_eq!(vector.worktree_id, REGISTERED.worktree_id);
    assert!(vector.head_target.is_some());
    assert!(vector.head_tree.is_some());
    assert!(vector.index_fingerprint.is_some());
    assert_eq!(vector.index_entry_count, Some(1));
    assert_eq!(vector.attributes_fingerprint, [0x32; 32]);
    assert_eq!(vector.worktree_inventory_digest, [0x33; 32]);
    assert!(matches!(
        adapter.status_candidates(),
        Err(GitStateError::CandidateDeltasDeferred)
    ));
    assert!(matches!(
        adapter.tree_diff_candidates(),
        Err(GitStateError::CandidateDeltasDeferred)
    ));

    let mut store =
        OperationalStore::open(&fixture.path().join("state.sqlite3")).expect("operational store");
    let workspace = WorkspaceRegistry::new(&mut store)
        .add(
            &root,
            WorkspaceSourceRegistration::NewGitRepository {
                worktree_administrative_key: b"main".to_vec(),
                worktree_kind: "main".to_owned(),
            },
        )
        .expect("registered Git workspace");
    let secure_root =
        open_workspace_root(&mut store, workspace.workspace_id).expect("authorized source root");
    let mut source = InventoryWalker::new(InventoryLimits::default())
        .walk_and_persist(
            &secure_root,
            &mut store,
            0,
            &InventoryCancellation::default(),
        )
        .expect("authoritative inventory");
    let git = adapter
        .inventory(
            &snapshot.selected_worktree,
            OBSERVATIONS,
            &GitCancellation::default(),
        )
        .expect("detached Git classification");
    apply_to_source_inventory(&git, &mut source, &mut store)
        .expect("overlay Git classification without replacing bytes");
    assert!(source.records.iter().all(|record| {
        record.git_repo_path_bytes.is_some()
            && record.content_digest.is_some()
            && record.classification as u16 == GitInventoryClassification::Tracked as u16
    }));
}

#[test]
fn wp17_negative_zero_state() {
    let fixture = tempfile::tempdir().expect("zero-state fixture");
    let root = fixture.path().join("repository");
    init_repository(&root, None);
    commit_file(&root, "tracked.rs", b"fn tracked() {}\n");
    fs::write(root.join("untracked.txt"), b"untracked\n").expect("untracked fixture");
    let before = snapshot_path(&root.join(".git"));
    let adapter = GixGitStateAdapter;
    let snapshot = adapter
        .open_worktree(&root, REGISTERED, &GitTrustPolicy::local_read_only())
        .expect("read-only open");
    adapter
        .capture_state(&snapshot.selected_worktree, OBSERVATIONS)
        .expect("read-only state");
    adapter
        .inventory(
            &snapshot.selected_worktree,
            OBSERVATIONS,
            &GitCancellation::default(),
        )
        .expect("read-only inventory");
    assert_eq!(before, snapshot_path(&root.join(".git")));
    assert!(
        !snapshot_path(&root.join(".git"))
            .keys()
            .any(|path| path.ends_with(b".lock"))
    );

    let source = include_str!("../../src/git_state.rs");
    for forbidden in [
        ["edit", "_reference"].concat(),
        ["write", "_object"].concat(),
        ["check", "out("].concat(),
        ["std::process::", "Command"].concat(),
    ] {
        assert!(
            !source.contains(&forbidden),
            "forbidden production API {forbidden}"
        );
    }
    let mut rejected = GitTrustPolicy::local_read_only();
    rejected.global_configuration = true;
    assert!(matches!(
        adapter.open_worktree(&root, REGISTERED, &rejected),
        Err(GitStateError::TrustPolicy)
    ));
}

#[tokio::test]
async fn wp17_operational_acceptance() {
    let executor = GitBlockingExecutor::new(1);
    let first_cancel = GitCancellation::default();
    let second_cancel = GitCancellation::default();
    let first = executor.run(first_cancel, |_| {
        thread::sleep(Duration::from_millis(25));
        Ok(1_u8)
    });
    let second = executor.run(second_cancel.clone(), move |cancel| {
        while !cancel.is_cancelled() {
            thread::yield_now();
        }
        Err::<u8, _>(GitStateError::Cancelled)
    });
    tokio::pin!(first);
    tokio::pin!(second);
    tokio::task::yield_now().await;
    assert!(executor.metrics().queue_depth <= 1);
    second_cancel.cancel();
    assert_eq!(first.await.expect("first job"), 1);
    assert!(matches!(second.await, Err(GitStateError::Cancelled)));
    let metrics = executor.metrics();
    assert_eq!(metrics.active_jobs, 0);
    assert_eq!(metrics.completed_jobs, 1);
    assert_eq!(metrics.interrupted_jobs, 1);
    assert!(metrics.total_duration_micros > 0);
}
