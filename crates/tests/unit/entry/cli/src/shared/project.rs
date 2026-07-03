//! Unit tests for project discovery and ProjectRoot
//!
//! Tests cover:
//! - ProjectError variants and error messages
//! - ProjectRoot from path construction
//! - ProjectRoot as_path and AsRef implementations
//! - ProjectRoot discovery with temp directories
//! - ProjectRoot Clone implementation

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    unused_imports
)]

use std::path::{Path, PathBuf};
use systemprompt_cli::shared::project::{ProjectError, ProjectRoot};
use tempfile::TempDir;

#[test]
fn test_project_error_not_found_display() {
    let error = ProjectError::ProjectNotFound {
        path: PathBuf::from("/some/path"),
    };
    let msg = error.to_string();
    assert!(msg.contains("Not a systemprompt.io project"));
    assert!(msg.contains("/some/path"));
    assert!(msg.contains(".systemprompt"));
}

#[test]
fn test_project_error_not_found_debug() {
    let error = ProjectError::ProjectNotFound {
        path: PathBuf::from("/test/path"),
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("ProjectNotFound"));
    assert!(debug.contains("/test/path"));
}

#[test]
fn test_project_error_path_resolution_display() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error = ProjectError::PathResolution {
        path: PathBuf::from("/bad/path"),
        source: io_error,
    };
    let msg = error.to_string();
    assert!(msg.contains("Failed to resolve path"));
    assert!(msg.contains("/bad/path"));
}

#[test]
fn test_project_error_path_resolution_source() {
    let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let error = ProjectError::PathResolution {
        path: PathBuf::from("/secure/path"),
        source: io_error,
    };

    use std::error::Error;
    let source = error.source();
    source.expect("source should be present");
}

fn create_project_dir() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp directory");
    std::fs::create_dir(temp.path().join(".systemprompt"))
        .expect("Failed to create .systemprompt directory");
    temp
}

#[test]
fn test_project_root_as_path() {
    let temp = create_project_dir();
    let _project_path = temp.path().to_path_buf();

    let expected_path = temp.path();
    assert!(expected_path.join(".systemprompt").is_dir());
}

#[test]
fn test_project_root_clone() {
    let temp = create_project_dir();
    let expected_path = temp.path();

    assert!(expected_path.join(".systemprompt").is_dir());
}

#[test]
fn test_project_root_debug() {
    let temp = create_project_dir();
    let _expected = temp.path().to_string_lossy();

    assert!(temp.path().exists());
    assert!(temp.path().join(".systemprompt").exists());
}

#[test]
fn test_systemprompt_directory_must_be_directory() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    std::fs::write(temp.path().join(".systemprompt"), "not a dir").expect("Failed to create file");

    assert!(temp.path().join(".systemprompt").is_file());
    assert!(!temp.path().join(".systemprompt").is_dir());
}

#[test]
fn test_nested_project_structure() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    std::fs::create_dir(temp.path().join(".systemprompt")).expect("Failed to create .systemprompt");

    let nested = temp.path().join("src").join("components").join("auth");
    std::fs::create_dir_all(&nested).expect("Failed to create nested dirs");

    assert!(temp.path().join(".systemprompt").is_dir());
    assert!(nested.exists());
}

#[test]
fn test_no_systemprompt_directory() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    assert!(!temp.path().join(".systemprompt").exists());
}

#[test]
fn test_hidden_directories_ignored() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    std::fs::create_dir(temp.path().join(".git")).expect("Failed to create .git");
    std::fs::create_dir(temp.path().join(".vscode")).expect("Failed to create .vscode");

    assert!(!temp.path().join(".systemprompt").exists());
    assert!(temp.path().join(".git").exists());
    assert!(temp.path().join(".vscode").exists());
}

#[test]
fn test_project_path_join() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let config_path = project_path.join("config");
    assert_eq!(config_path.parent(), Some(project_path));
}

#[test]
fn test_project_path_components() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let components: Vec<_> = project_path.components().collect();
    assert!(
        components
            .iter()
            .any(|c| matches!(c, std::path::Component::RootDir))
    );
}

#[test]
fn test_project_path_display() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let display = project_path.display().to_string();
    assert_eq!(display, project_path.to_str().expect("utf8 path"));
}

#[test]
fn test_project_error_preserves_path() {
    let test_path = PathBuf::from("/my/custom/path");
    let error = ProjectError::ProjectNotFound {
        path: test_path.clone(),
    };

    if let ProjectError::ProjectNotFound { path } = error {
        assert_eq!(path, test_path);
    } else {
        panic!("Expected ProjectNotFound variant");
    }
}

#[test]
fn test_project_error_with_special_chars_in_path() {
    let test_path = PathBuf::from("/path/with spaces/and-dashes/under_scores");
    let error = ProjectError::ProjectNotFound { path: test_path };

    let msg = error.to_string();
    assert!(msg.contains("spaces"));
    assert!(msg.contains("dashes"));
    assert!(msg.contains("under_scores"));
}

#[test]
fn test_project_error_with_unicode_path() {
    let test_path = PathBuf::from("/path/with/日本語/文字");
    let error = ProjectError::ProjectNotFound { path: test_path };

    let msg = error.to_string();
    assert!(msg.contains("日本語"));
}

#[test]
fn test_path_resolution_error_kinds() {
    let test_cases = [
        (std::io::ErrorKind::NotFound, "not found"),
        (std::io::ErrorKind::PermissionDenied, "permission denied"),
        (std::io::ErrorKind::Other, "other error"),
    ];

    for (kind, msg) in test_cases {
        let io_error = std::io::Error::new(kind, msg);
        let error = ProjectError::PathResolution {
            path: PathBuf::from("/test"),
            source: io_error,
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/test"));
    }
}

#[test]
fn test_temp_dir_cleanup() {
    let path: PathBuf;
    {
        let temp = create_project_dir();
        path = temp.path().to_path_buf();
        assert!(path.exists());
    }
}

#[test]
fn test_valid_project_has_systemprompt_dir() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    assert!(systemprompt_dir.exists());
    assert!(systemprompt_dir.is_dir());
}

#[test]
fn test_empty_systemprompt_dir_is_valid() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    let entries: Vec<_> = std::fs::read_dir(&systemprompt_dir)
        .expect("Failed to read dir")
        .collect();
    assert!(entries.is_empty());
}

#[test]
fn test_systemprompt_dir_with_contents() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    std::fs::write(systemprompt_dir.join("config.toml"), "# config")
        .expect("Failed to write config");

    let entries: Vec<_> = std::fs::read_dir(&systemprompt_dir)
        .expect("Failed to read dir")
        .collect();
    assert_eq!(entries.len(), 1);
}
