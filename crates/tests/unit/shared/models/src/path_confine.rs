use std::fs;

use systemprompt_models::path_confine::{
    PathConfinementError, confine_within, entries_within, locate_within, read_within,
    single_segment,
};

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
    assert!(
        matches!(err, PathConfinementError::Unresolved { .. }),
        "{err}"
    );
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

#[test]
fn entries_within_lists_every_entry_resolved_inside_the_root() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("b")).unwrap();
    fs::write(root.path().join("a.yaml"), "x").unwrap();

    let entries = entries_within(root.path(), root.path()).unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["a.yaml", "b"]);
    assert!(!entries[0].is_dir);
    assert!(entries[1].is_dir);
    let canonical = root.path().canonicalize().unwrap();
    assert!(entries.iter().all(|e| e.path.starts_with(&canonical)));
}

#[cfg(unix)]
#[test]
fn entries_within_refuses_a_directory_holding_a_link_out_of_the_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("inside")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();

    let err = entries_within(root.path(), root.path()).unwrap_err();
    assert!(matches!(err, PathConfinementError::Escapes { .. }), "{err}");
}

#[cfg(unix)]
#[test]
fn entries_within_refuses_a_dangling_link() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(root.path().join("nowhere"), root.path().join("dangling")).unwrap();

    let err = entries_within(root.path(), root.path()).unwrap_err();
    assert!(
        matches!(err, PathConfinementError::Unresolved { .. }),
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn entries_within_follows_a_link_that_stays_inside_the_root() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("real")).unwrap();
    std::os::unix::fs::symlink(root.path().join("real"), root.path().join("alias")).unwrap();

    let entries = entries_within(root.path(), root.path()).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.is_dir));
}

#[test]
fn locate_within_is_none_for_an_absent_path_and_confines_a_present_one() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("present"), "x").unwrap();

    assert!(locate_within(root.path(), &root.path().join("absent")).unwrap().is_none());
    assert!(locate_within(root.path(), &root.path().join("present")).unwrap().is_some());
}

#[cfg(unix)]
#[test]
fn read_within_and_locate_within_refuse_a_file_link_out_of_the_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), root.path().join("index.md")).unwrap();

    let read = read_within(root.path(), &root.path().join("index.md")).unwrap_err();
    assert!(matches!(read, PathConfinementError::Escapes { .. }), "{read}");
    let located = locate_within(root.path(), &root.path().join("index.md")).unwrap_err();
    assert!(matches!(located, PathConfinementError::Escapes { .. }), "{located}");
}

#[test]
fn read_within_reads_a_file_inside_the_root() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("config.yaml"), "body").unwrap();
    assert_eq!(read_within(root.path(), &root.path().join("config.yaml")).unwrap(), "body");
}
