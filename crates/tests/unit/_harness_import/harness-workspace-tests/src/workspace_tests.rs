use std::fs;
use std::time::Duration;

use harness_workspace::{DiffTarget, PrunePolicy, PruneSchedule, Workspace, WorkspaceOptions};

#[tokio::test]
async fn workspace_round_trip() {
    let worktree = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut options = WorkspaceOptions::new(data.path());
    options.prune = Some(PruneSchedule {
        every: Duration::from_secs(3600),
        policy: PrunePolicy::default(),
    });
    let workspace = Workspace::open(worktree.path(), options).unwrap();
    fs::write(worktree.path().join("a.txt"), "one\n").unwrap();
    let base = workspace.track().await.unwrap();
    fs::write(worktree.path().join("a.txt"), "two\n").unwrap();

    let diffs = workspace
        .diff(base.clone(), DiffTarget::Working)
        .await
        .unwrap();
    assert_eq!(diffs.len(), 1);
    let clone = workspace.clone();
    let touched = clone.restore(base).await.unwrap();
    assert_eq!(touched.len(), 1);
    assert_eq!(
        fs::read_to_string(worktree.path().join("a.txt")).unwrap(),
        "one\n"
    );
    assert!(
        workspace
            .vcs()
            .await
            .unwrap()
            .is_none_or(|s| !s.info.root.starts_with(worktree.path()))
    );
}
