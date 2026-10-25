//! `config migrate` renames a plugin descriptor's retired `hooks.evaluation`
//! to `hooks.judge`, keeps comments, and leaves current descriptors alone.

use std::path::Path;

use systemprompt_cli::commands::admin::config::migrate::hooks::{migrate, pending};

fn write(root: &Path, id: &str, body: &str) -> std::path::PathBuf {
    let path = root.join("plugins").join(id).join("config.yaml");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("write");
    path
}

#[test]
fn the_retired_evaluation_key_becomes_judge() {
    let root = tempfile::tempdir().expect("tempdir");
    let old = write(
        root.path(),
        "commons",
        "plugin:\n  id: commons\n  hooks:\n    governance: true\n    evaluation: true # the judge\n  skills:\n    evaluation: not-a-hook\n",
    );
    let current = write(
        root.path(),
        "other",
        "plugin:\n  id: other\n  hooks:\n    judge: true\n",
    );
    assert!(pending(root.path()));

    let moved = migrate(root.path()).expect("migrate");

    assert_eq!(moved, vec![old.clone()]);
    assert_eq!(
        std::fs::read_to_string(old).expect("read"),
        "plugin:\n  id: commons\n  hooks:\n    governance: true\n    judge: true # the judge\n  skills:\n    evaluation: not-a-hook\n"
    );
    assert_eq!(
        std::fs::read_to_string(current).expect("read"),
        "plugin:\n  id: other\n  hooks:\n    judge: true\n"
    );
    assert!(!pending(root.path()));
}
