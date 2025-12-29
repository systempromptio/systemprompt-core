//! Unit tests for SecretsLoader
//!
//! Tests cover:
//! - Path resolution with home directory expansion
//! - Path resolution with base directory
//! - Relative and absolute path handling
//! - File loading success and failure cases
//! - File existence checks

use std::path::{Path, PathBuf};
use systemprompt_loader::SecretsLoader;
use tempfile::TempDir;

// ============================================================================
// Path Resolution Tests - Home Directory Expansion
// ============================================================================

#[test]
fn test_resolve_path_absolute_path() {
    let path = SecretsLoader::resolve_path("/absolute/path/to/secrets.json", None);
    assert_eq!(path, PathBuf::from("/absolute/path/to/secrets.json"));
}

#[test]
fn test_resolve_path_relative_path_no_base() {
    let path = SecretsLoader::resolve_path("relative/path/secrets.json", None);
    assert_eq!(path, PathBuf::from("relative/path/secrets.json"));
}

#[test]
fn test_resolve_path_relative_path_with_base() {
    let base = Path::new("/base/dir");
    let path = SecretsLoader::resolve_path("relative/secrets.json", Some(base));
    assert_eq!(path, PathBuf::from("/base/dir/relative/secrets.json"));
}

#[test]
fn test_resolve_path_home_expansion() {
    let path = SecretsLoader::resolve_path("~/secrets.json", None);
    // Should expand to home directory
    assert!(!path.to_string_lossy().starts_with("~/"));
}

#[test]
fn test_resolve_path_home_expansion_with_subdir() {
    let path = SecretsLoader::resolve_path("~/.config/secrets.json", None);
    assert!(!path.to_string_lossy().starts_with("~"));
    assert!(path.to_string_lossy().contains(".config/secrets.json"));
}

#[test]
fn test_resolve_path_absolute_ignores_base() {
    let base = Path::new("/should/be/ignored");
    let path = SecretsLoader::resolve_path("/absolute/secrets.json", Some(base));
    assert_eq!(path, PathBuf::from("/absolute/secrets.json"));
}

// ============================================================================
// File Loading Tests
// ============================================================================

#[test]
fn test_load_from_file_valid_secrets() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters",
        "database_url": "postgres://localhost/test"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    assert!(result.is_ok());

    let secrets = result.expect("Should have loaded secrets");
    assert_eq!(
        secrets.jwt_secret,
        "a_very_long_jwt_secret_that_is_at_least_32_characters"
    );
    assert_eq!(secrets.database_url, "postgres://localhost/test");
}

#[test]
fn test_load_from_file_with_optional_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters",
        "database_url": "postgres://localhost/test",
        "anthropic": "sk-ant-test-key",
        "openai": "sk-openai-test-key"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    assert!(result.is_ok());

    let secrets = result.expect("Should have loaded secrets");
    assert_eq!(secrets.anthropic, Some("sk-ant-test-key".to_string()));
    assert_eq!(secrets.openai, Some("sk-openai-test-key".to_string()));
    assert!(secrets.gemini.is_none());
}

#[test]
fn test_load_from_file_nonexistent() {
    let path = Path::new("/nonexistent/path/secrets.json");
    let result = SecretsLoader::load_from_file(path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Secrets file not found"));
}

#[test]
fn test_load_from_file_invalid_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    std::fs::write(&secrets_path, "{ invalid json }").expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    assert!(result.is_err());
}

#[test]
fn test_load_from_file_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    // Missing database_url
    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    assert!(result.is_err());
}

#[test]
fn test_load_from_file_jwt_too_short() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "short",
        "database_url": "postgres://localhost/test"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    // Should fail due to jwt_secret being too short (minimum 32 characters)
    assert!(result.is_err());
}

// ============================================================================
// Resolve and Load Tests
// ============================================================================

#[test]
fn test_resolve_and_load_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters",
        "database_url": "postgres://localhost/test"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::resolve_and_load("secrets.json", Some(temp_dir.path()));
    assert!(result.is_ok());
}

#[test]
fn test_resolve_and_load_with_subdirectory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("config");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let secrets_path = subdir.join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters",
        "database_url": "postgres://localhost/test"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::resolve_and_load("config/secrets.json", Some(temp_dir.path()));
    assert!(result.is_ok());
}

// ============================================================================
// Exists Tests
// ============================================================================

#[test]
fn test_exists_true() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.json");
    std::fs::write(&file_path, "{}").expect("Failed to write file");

    assert!(SecretsLoader::exists(&file_path));
}

#[test]
fn test_exists_false() {
    let path = Path::new("/nonexistent/path/secrets.json");
    assert!(!SecretsLoader::exists(path));
}

#[test]
fn test_exists_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // exists() returns true for directories too (Path::exists behavior)
    assert!(SecretsLoader::exists(temp_dir.path()));
}

// ============================================================================
// Custom Fields Tests
// ============================================================================

#[test]
fn test_load_from_file_with_custom_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_path = temp_dir.path().join("secrets.json");

    let secrets_content = r#"{
        "jwt_secret": "a_very_long_jwt_secret_that_is_at_least_32_characters",
        "database_url": "postgres://localhost/test",
        "custom_api_key": "my-custom-key",
        "another_secret": "another-value"
    }"#;

    std::fs::write(&secrets_path, secrets_content).expect("Failed to write secrets file");

    let result = SecretsLoader::load_from_file(&secrets_path);
    assert!(result.is_ok());

    let secrets = result.expect("Should have loaded secrets");
    assert_eq!(
        secrets.get("custom_api_key"),
        Some(&"my-custom-key".to_string())
    );
    assert_eq!(
        secrets.get("another_secret"),
        Some(&"another-value".to_string())
    );
}
