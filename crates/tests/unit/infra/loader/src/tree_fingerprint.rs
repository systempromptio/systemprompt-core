use systemprompt_loader::tree_fingerprint;

#[test]
fn an_unchanged_tree_keeps_its_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::write(dir.path().join("agents/a.yaml"), "name: a\n").unwrap();

    assert_eq!(tree_fingerprint(dir.path()), tree_fingerprint(dir.path()));
}

#[test]
fn an_edited_or_added_file_moves_the_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::write(dir.path().join("agents/a.yaml"), "name: a\n").unwrap();
    let before = tree_fingerprint(dir.path());

    std::fs::write(dir.path().join("agents/a.yaml"), "name: a-renamed\n").unwrap();
    let edited = tree_fingerprint(dir.path());
    assert_ne!(before, edited);

    std::fs::write(dir.path().join("agents/b.yaml"), "name: b\n").unwrap();
    assert_ne!(edited, tree_fingerprint(dir.path()));
}

#[cfg(unix)]
#[test]
fn a_configmap_style_data_link_swap_moves_the_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("mount");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(root.join("..v1")).unwrap();
    std::fs::write(root.join("..v1/plane.yaml"), "rules: []\n").unwrap();
    std::os::unix::fs::symlink("..v1", root.join("..data")).unwrap();
    std::os::unix::fs::symlink("..data/plane.yaml", root.join("plane.yaml")).unwrap();
    let before = tree_fingerprint(&root);

    std::fs::create_dir_all(root.join("..v2")).unwrap();
    std::fs::write(root.join("..v2/plane.yaml"), "rules: [one]\n").unwrap();
    std::fs::remove_file(root.join("..data")).unwrap();
    std::os::unix::fs::symlink("..v2", root.join("..data")).unwrap();
    std::fs::remove_dir_all(root.join("..v1")).unwrap();

    assert_ne!(before, tree_fingerprint(&root));
}
