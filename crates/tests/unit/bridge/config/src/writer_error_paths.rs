use std::fs;

use systemprompt_bridge::config::write::{self, ConfigWriteError};

#[test]
fn edit_file_reports_a_regular_parent_as_a_write_context_without_creating_a_config() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("not-a-directory");
    fs::write(&parent, "operator file").unwrap();
    let path = parent.join("bridge.toml");

    let error = write::edit_file(&path, |doc| {
        write::set(doc, &["bridge", "proxy", "port"], 8123)
    })
    .unwrap_err();
    assert!(
        matches!(error, ConfigWriteError::Write { path: ref error_path, .. } if error_path == &path.with_extension("toml.lock"))
    );
    assert_eq!(fs::read_to_string(&parent).unwrap(), "operator file");
    assert!(!path.exists());
}

#[test]
fn edit_file_refuses_a_directory_at_the_config_path_without_mutating_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge.toml");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("operator-note"), "retain").unwrap();

    let error = write::edit_file(&path, |doc| {
        write::set(doc, &["gateway_url"], "https://example.invalid")
    })
    .unwrap_err();
    assert!(
        matches!(error, ConfigWriteError::Read { path: ref error_path, .. } if error_path == &path)
    );
    assert_eq!(
        fs::read_to_string(path.join("operator-note")).unwrap(),
        "retain"
    );
}

#[test]
fn edit_file_reports_a_directory_lock_path_and_leaves_the_config_bytes_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge.toml");
    fs::write(&path, "gateway_url = \"https://operator.example\"\n").unwrap();
    fs::create_dir(path.with_extension("toml.lock")).unwrap();

    let error = write::edit_file(&path, |doc| {
        write::set(doc, &["gateway_url"], "https://new.example")
    })
    .unwrap_err();
    assert!(matches!(error, ConfigWriteError::Write { .. }));
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "gateway_url = \"https://operator.example\"\n"
    );
}

#[test]
fn invalid_empty_and_scalar_nested_paths_preserve_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge.toml");
    let original = "bridge = 1\n";
    fs::write(&path, original).unwrap();

    let empty = write::edit_file(&path, |doc| write::set(doc, &[], "x")).unwrap_err();
    assert!(matches!(empty, ConfigWriteError::InvalidPath { .. }));
    let scalar =
        write::edit_file(&path, |doc| write::remove(doc, &["bridge", "proxy"])).unwrap_err();
    assert!(matches!(scalar, ConfigWriteError::InvalidPath { .. }));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}
