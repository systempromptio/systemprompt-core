use std::path::Path;
use systemprompt_cli::shared::project::{ProjectError, ProjectRoot};
use tempfile::tempdir;

use crate::env_lock;

#[test]
fn project_root_discover_succeeds_in_valid_root_with_cargo_toml() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();

    // We cannot reliably mutate CWD in parallel test runs, so we exercise the
    // resolved-path helpers using the AsRef/as_path APIs through a manually
    // wrapped value via Debug formatting.
    let cwd_backup = std::env::current_dir().ok();
    if std::env::set_current_dir(dir.path()).is_ok() {
        let root = ProjectRoot::discover().expect("should discover valid project root");
        let p: &Path = root.as_ref();
        assert!(p.join(".systemprompt").is_dir());
        let _ = root.as_path();
        let _ = format!("{:?}", root);
        if let Some(prev) = cwd_backup {
            let _ = std::env::set_current_dir(prev);
        }
    }
}

#[test]
fn project_root_discover_succeeds_with_services_dir() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    std::fs::create_dir_all(dir.path().join("services")).unwrap();
    let cwd_backup = std::env::current_dir().ok();
    if std::env::set_current_dir(dir.path()).is_ok() {
        let root = ProjectRoot::discover().expect("services dir is sufficient");
        assert!(root.as_path().exists());
        if let Some(prev) = cwd_backup {
            let _ = std::env::set_current_dir(prev);
        }
    }
}

#[test]
fn project_root_error_displays_helpful_message() {
    let err = ProjectError::ProjectNotFound {
        path: "/tmp/x".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Not a systemprompt.io project"));
}

#[test]
fn project_root_path_resolution_error_displays_source() {
    let io_err = std::io::Error::other("disk full");
    let err = ProjectError::PathResolution {
        path: "/tmp/x".into(),
        source: io_err,
    };
    let msg = err.to_string();
    assert!(msg.contains("Failed to resolve path"));
}

#[test]
fn project_root_discover_walks_up_to_parent() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    std::fs::create_dir_all(dir.path().join("storage")).unwrap();

    let nested = dir.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).unwrap();

    let cwd_backup = std::env::current_dir().ok();
    if std::env::set_current_dir(&nested).is_ok() {
        let root = ProjectRoot::discover().expect("should walk up to discover root");
        // The discovered root should be an ancestor of nested.
        assert!(nested.starts_with(root.as_path()));
        if let Some(prev) = cwd_backup {
            let _ = std::env::set_current_dir(prev);
        }
    }
}
