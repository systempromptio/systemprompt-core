//! Unit tests for IncludeResolver
//!
//! Tests cover:
//! - String resolution with and without !include prefix
//! - YAML file resolution and parsing
//! - Subpath resolver creation
//! - File existence checks
//! - Base path accessor
//! - File reading functionality

use std::path::PathBuf;
use systemprompt_loader::IncludeResolver;
use tempfile::TempDir;

// ============================================================================
// Constructor Tests
// ============================================================================

#[test]
fn test_new_with_path() {
    let path = PathBuf::from("/some/base/path");
    let resolver = IncludeResolver::new(path.clone());
    assert_eq!(resolver.base_path(), &path);
}

#[test]
fn test_new_with_empty_path() {
    let path = PathBuf::new();
    let resolver = IncludeResolver::new(path);
    assert_eq!(resolver.base_path(), &PathBuf::new());
}

// ============================================================================
// String Resolution Tests
// ============================================================================

#[test]
fn test_resolve_string_no_include() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("regular string value");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should resolve"), "regular string value");
}

#[test]
fn test_resolve_string_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should resolve"), "");
}

#[test]
fn test_resolve_string_with_include() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let include_file = temp_dir.path().join("included.txt");
    std::fs::write(&include_file, "included content").expect("Failed to write include file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("!include included.txt");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should resolve"), "included content");
}

#[test]
fn test_resolve_string_with_include_whitespace() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let include_file = temp_dir.path().join("included.txt");
    std::fs::write(&include_file, "content with whitespace").expect("Failed to write include file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("!include   included.txt  ");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should resolve"), "content with whitespace");
}

#[test]
fn test_resolve_string_include_missing_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("!include nonexistent.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_resolve_string_include_subdirectory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("subdir");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let include_file = subdir.join("nested.txt");
    std::fs::write(&include_file, "nested content").expect("Failed to write nested file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.resolve_string("!include subdir/nested.txt");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should resolve"), "nested content");
}

// ============================================================================
// YAML Resolution Tests
// ============================================================================

#[test]
fn test_resolve_yaml_file_simple() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let yaml_file = temp_dir.path().join("config.yaml");
    std::fs::write(&yaml_file, "key: value\nnumber: 42").expect("Failed to write yaml file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct TestConfig {
        key: String,
        number: i32,
    }

    let result: Result<TestConfig, _> = resolver.resolve_yaml_file("config.yaml");
    assert!(result.is_ok());

    let config = result.expect("Should parse yaml");
    assert_eq!(config.key, "value");
    assert_eq!(config.number, 42);
}

#[test]
fn test_resolve_yaml_file_complex() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let yaml_file = temp_dir.path().join("complex.yaml");
    let yaml_content = r#"
name: test
items:
  - first
  - second
  - third
nested:
  inner: value
"#;
    std::fs::write(&yaml_file, yaml_content).expect("Failed to write yaml file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    #[derive(serde::Deserialize, Debug)]
    struct Nested {
        inner: String,
    }

    #[derive(serde::Deserialize, Debug)]
    struct ComplexConfig {
        name: String,
        items: Vec<String>,
        nested: Nested,
    }

    let result: Result<ComplexConfig, _> = resolver.resolve_yaml_file("complex.yaml");
    assert!(result.is_ok());

    let config = result.expect("Should parse complex yaml");
    assert_eq!(config.name, "test");
    assert_eq!(config.items.len(), 3);
    assert_eq!(config.nested.inner, "value");
}

#[test]
fn test_resolve_yaml_file_missing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    #[derive(serde::Deserialize, Debug)]
    struct AnyConfig {
        #[allow(dead_code)]
        field: String,
    }

    let result: Result<AnyConfig, _> = resolver.resolve_yaml_file("nonexistent.yaml");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_resolve_yaml_file_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let yaml_file = temp_dir.path().join("invalid.yaml");
    std::fs::write(&yaml_file, "not: valid: yaml: syntax").expect("Failed to write yaml file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    #[derive(serde::Deserialize)]
    struct Config {
        #[allow(dead_code)]
        not: String,
    }

    let result: Result<Config, _> = resolver.resolve_yaml_file("invalid.yaml");
    assert!(result.is_err());
}

// ============================================================================
// Subpath Resolver Tests
// ============================================================================

#[test]
fn test_with_subpath() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let sub_resolver = resolver.with_subpath("subdir");
    assert_eq!(sub_resolver.base_path(), &temp_dir.path().join("subdir"));
}

#[test]
fn test_with_subpath_chained() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let sub_resolver = resolver.with_subpath("level1").with_subpath("level2");
    assert_eq!(
        sub_resolver.base_path(),
        &temp_dir.path().join("level1").join("level2")
    );
}

#[test]
fn test_with_subpath_resolves_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("configs");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let file = subdir.join("test.txt");
    std::fs::write(&file, "subdir content").expect("Failed to write file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    let sub_resolver = resolver.with_subpath("configs");

    let result = sub_resolver.read_file("test.txt");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should read file"), "subdir content");
}

// ============================================================================
// Exists Tests
// ============================================================================

#[test]
fn test_exists_true() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("existing.txt");
    std::fs::write(&file_path, "content").expect("Failed to write file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    assert!(resolver.exists("existing.txt"));
}

#[test]
fn test_exists_false() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    assert!(!resolver.exists("nonexistent.txt"));
}

#[test]
fn test_exists_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("subdir");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    assert!(resolver.exists("subdir"));
}

// ============================================================================
// Read File Tests
// ============================================================================

#[test]
fn test_read_file_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("readable.txt");
    std::fs::write(&file_path, "file content here").expect("Failed to write file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    let result = resolver.read_file("readable.txt");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should read file"), "file content here");
}

#[test]
fn test_read_file_missing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());

    let result = resolver.read_file("missing.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_read_file_in_subdirectory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("nested");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let file_path = subdir.join("deep.txt");
    std::fs::write(&file_path, "deep content").expect("Failed to write file");

    let resolver = IncludeResolver::new(temp_dir.path().to_path_buf());
    let result = resolver.read_file("nested/deep.txt");
    assert!(result.is_ok());
    assert_eq!(result.expect("Should read file"), "deep content");
}

// ============================================================================
// Base Path Tests
// ============================================================================

#[test]
fn test_base_path_accessor() {
    let path = PathBuf::from("/test/base/path");
    let resolver = IncludeResolver::new(path.clone());
    assert_eq!(resolver.base_path(), &path);
}

#[test]
fn test_base_path_after_clone() {
    let path = PathBuf::from("/test/path");
    let resolver = IncludeResolver::new(path.clone());
    let cloned = resolver.clone();
    assert_eq!(cloned.base_path(), &path);
}
