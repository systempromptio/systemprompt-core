use std::fs;

use harness_config::Worktree;

#[test]
fn nearest_git_and_chain() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let deep = repo.join("a").join("b");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(&deep).unwrap();
    let tree = Worktree::detect(&deep);
    assert_eq!(tree.root, repo);
    assert!(tree.is_git());
    assert_eq!(
        tree.chain(&deep),
        [repo.clone(), repo.join("a"), deep.clone()]
    );
    assert_eq!(tree.chain(tmp.path()), [tmp.path().to_path_buf()]);
}

#[test]
fn git_file_and_no_git() {
    let tmp = tempfile::tempdir().unwrap();
    let linked = tmp.path().join("linked");
    fs::create_dir_all(&linked).unwrap();
    fs::write(
        linked.join(".git"),
        "gitdir: ../main/.git/worktrees/linked\n",
    )
    .unwrap();
    let tree = Worktree::detect(&linked);
    assert_eq!(tree.root, linked);
    assert_eq!(
        tree.git_dir,
        Some(linked.join("../main/.git/worktrees/linked"))
    );

    let plain = tempfile::tempdir().unwrap();
    let tree = Worktree::detect(plain.path());
    if !tree.is_git() {
        assert_eq!(tree.root, plain.path());
        assert_eq!(tree.chain(plain.path()), [plain.path().to_path_buf()]);
    }
}
