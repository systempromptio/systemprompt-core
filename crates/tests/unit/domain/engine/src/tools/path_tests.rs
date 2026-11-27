use std::path::{Path, PathBuf};

use systemprompt_engine::tools::{display_path, is_within, normalize, resolve};

#[test]
fn normalizes_dots() {
    assert_eq!(
        normalize(Path::new("/a/b/../c/./d")),
        PathBuf::from("/a/c/d")
    );
    assert_eq!(normalize(Path::new("/../x")), PathBuf::from("/x"));
    assert_eq!(normalize(Path::new("../x")), PathBuf::from("../x"));
}

#[test]
fn resolves_relative_to_cwd() {
    assert_eq!(
        resolve(Path::new("/w/p"), "src/../lib.rs"),
        PathBuf::from("/w/p/lib.rs")
    );
    assert_eq!(
        resolve(Path::new("/w/p"), "/etc/hosts"),
        PathBuf::from("/etc/hosts")
    );
}

#[test]
fn within_checks_components() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("proj");
    std::fs::create_dir_all(root.join("src")).unwrap();
    assert!(is_within(&root.join("src/new.rs"), &root));
    assert!(is_within(&root, &root));
    assert!(!is_within(&dir.path().join("proj2/x"), &root));
    assert!(!is_within(&root.join("../other"), &root));
    assert_eq!(display_path(&root.join("src/a.rs"), &root), "src/a.rs");
}
