use std::fs;

use systemprompt_models::path_confine::{PathConfinementError, confine_within, single_segment};

#[test]
fn confine_within_returns_the_resolved_path_inside_the_root() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("demo")).unwrap();
    fs::write(root.path().join("demo/config.yaml"), "x").unwrap();

    let resolved = confine_within(root.path(), &root.path().join("demo/config.yaml")).unwrap();
    assert!(resolved.starts_with(root.path().canonicalize().unwrap()));
}

#[test]
fn confine_within_refuses_dot_dot_out_of_the_root() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("tree");
    fs::create_dir_all(&root).unwrap();
    fs::write(parent.path().join("secret.yaml"), "x").unwrap();

    let err = confine_within(&root, &root.join("../secret.yaml")).unwrap_err();
    assert!(matches!(err, PathConfinementError::Escapes { .. }), "{err}");
}

#[test]
fn confine_within_refuses_an_absolute_candidate_outside_the_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("index.md"), "x").unwrap();

    let candidate = root.path().join(outside.path().join("index.md"));
    let err = confine_within(root.path(), &candidate).unwrap_err();
    assert!(matches!(err, PathConfinementError::Escapes { .. }), "{err}");
}

#[cfg(unix)]
#[test]
fn confine_within_refuses_a_symlink_that_leaves_the_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("config.yaml"), "x").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();

    let err = confine_within(root.path(), &root.path().join("link/config.yaml")).unwrap_err();
    assert!(matches!(err, PathConfinementError::Escapes { .. }), "{err}");
}

#[test]
fn confine_within_reports_a_missing_candidate_as_not_found() {
    let root = tempfile::tempdir().unwrap();
    let err = confine_within(root.path(), &root.path().join("absent")).unwrap_err();
    assert!(matches!(err, PathConfinementError::Unresolved { .. }), "{err}");
}

#[test]
fn single_segment_accepts_only_one_plain_component() {
    assert_eq!(single_segment("name", "alpha").unwrap(), "alpha");
    for bad in ["", ".", "..", "../x", "a/b", "/abs", "a\\b", "x/"] {
        assert!(
            matches!(
                single_segment("name", bad),
                Err(PathConfinementError::NotASegment { .. })
            ),
            "accepted {bad:?}"
        );
    }
}
