//! Unit tests for ExtensionLoader
//!
//! Tests cover:
//! - Extension discovery from extensions directory
//! - Manifest loading and parsing
//! - MCP and CLI extension filtering
//! - Binary path resolution
//! - Binary validation
//! - Binary map building

use std::collections::HashMap;
use systemprompt_loader::ExtensionLoader;
use tempfile::TempDir;

fn create_mcp_manifest(name: &str, binary: &str) -> String {
    format!(
        r#"extension:
  type: mcp
  name: {}
  binary: {}
  description: "Test MCP extension"
  enabled: true
"#,
        name, binary
    )
}

fn create_cli_manifest(name: &str, binary: &str) -> String {
    format!(
        r#"extension:
  type: cli
  name: {}
  binary: {}
  description: "Test CLI extension"
  enabled: true
  commands:
    - name: test-cmd
      description: "Test command"
"#,
        name, binary
    )
}

fn create_disabled_manifest(name: &str) -> String {
    format!(
        r#"extension:
  type: mcp
  name: {}
  binary: {}
  description: "Disabled extension"
  enabled: false
"#,
        name, name
    )
}

// ============================================================================
// Discovery Tests
// ============================================================================

#[test]
fn test_discover_no_extensions_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert!(discovered.is_empty());
}

#[test]
fn test_discover_empty_extensions_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("Failed to create extensions dir");

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert!(discovered.is_empty());
}

#[test]
fn test_discover_single_extension() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("test-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create extension dir");

    std::fs::write(ext_dir.join("manifest.yaml"), create_mcp_manifest("test-ext", "test-bin"))
        .expect("Failed to write manifest");

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].manifest.extension.name, "test-ext");
}

#[test]
fn test_discover_multiple_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    for i in 1..=3 {
        let ext_dir = extensions_dir.join(format!("ext-{}", i));
        std::fs::create_dir_all(&ext_dir).expect("Failed to create extension dir");
        std::fs::write(
            ext_dir.join("manifest.yaml"),
            create_mcp_manifest(&format!("ext-{}", i), &format!("bin-{}", i)),
        )
        .expect("Failed to write manifest");
    }

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert_eq!(discovered.len(), 3);
}

#[test]
fn test_discover_nested_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    let category_dir = extensions_dir.join("mcp-category");
    let nested_ext = category_dir.join("nested-ext");
    std::fs::create_dir_all(&nested_ext).expect("Failed to create nested ext dir");

    std::fs::write(
        nested_ext.join("manifest.yaml"),
        create_mcp_manifest("nested-ext", "nested-bin"),
    )
    .expect("Failed to write manifest");

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].manifest.extension.name, "nested-ext");
}

#[test]
fn test_discover_ignores_invalid_manifest() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    let valid_ext = extensions_dir.join("valid-ext");
    let invalid_ext = extensions_dir.join("invalid-ext");
    std::fs::create_dir_all(&valid_ext).expect("Failed to create valid ext dir");
    std::fs::create_dir_all(&invalid_ext).expect("Failed to create invalid ext dir");

    std::fs::write(
        valid_ext.join("manifest.yaml"),
        create_mcp_manifest("valid-ext", "valid-bin"),
    )
    .expect("Failed to write valid manifest");

    std::fs::write(invalid_ext.join("manifest.yaml"), "invalid: yaml: :")
        .expect("Failed to write invalid manifest");

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].manifest.extension.name, "valid-ext");
}

#[test]
fn test_discover_ignores_non_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("Failed to create extensions dir");

    std::fs::write(extensions_dir.join("not-a-directory.txt"), "just a file")
        .expect("Failed to write file");

    let discovered = ExtensionLoader::discover(temp_dir.path());
    assert!(discovered.is_empty());
}

// ============================================================================
// MCP Extension Filtering Tests
// ============================================================================

#[test]
fn test_get_enabled_mcp_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    let mcp_ext = extensions_dir.join("mcp-ext");
    let cli_ext = extensions_dir.join("cli-ext");
    let disabled_ext = extensions_dir.join("disabled-ext");

    std::fs::create_dir_all(&mcp_ext).expect("Failed to create mcp ext");
    std::fs::create_dir_all(&cli_ext).expect("Failed to create cli ext");
    std::fs::create_dir_all(&disabled_ext).expect("Failed to create disabled ext");

    std::fs::write(mcp_ext.join("manifest.yaml"), create_mcp_manifest("mcp-ext", "mcp-bin"))
        .expect("Failed to write mcp manifest");
    std::fs::write(cli_ext.join("manifest.yaml"), create_cli_manifest("cli-ext", "cli-bin"))
        .expect("Failed to write cli manifest");
    std::fs::write(
        disabled_ext.join("manifest.yaml"),
        create_disabled_manifest("disabled-ext"),
    )
    .expect("Failed to write disabled manifest");

    let mcp_extensions = ExtensionLoader::get_enabled_mcp_extensions(temp_dir.path());
    assert_eq!(mcp_extensions.len(), 1);
    assert_eq!(mcp_extensions[0].manifest.extension.name, "mcp-ext");
}

#[test]
fn test_get_enabled_mcp_extensions_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("Failed to create extensions dir");

    let mcp_extensions = ExtensionLoader::get_enabled_mcp_extensions(temp_dir.path());
    assert!(mcp_extensions.is_empty());
}

// ============================================================================
// CLI Extension Filtering Tests
// ============================================================================

#[test]
fn test_get_enabled_cli_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    let mcp_ext = extensions_dir.join("mcp-ext");
    let cli_ext = extensions_dir.join("cli-ext");

    std::fs::create_dir_all(&mcp_ext).expect("Failed to create mcp ext");
    std::fs::create_dir_all(&cli_ext).expect("Failed to create cli ext");

    std::fs::write(mcp_ext.join("manifest.yaml"), create_mcp_manifest("mcp-ext", "mcp-bin"))
        .expect("Failed to write mcp manifest");
    std::fs::write(cli_ext.join("manifest.yaml"), create_cli_manifest("cli-ext", "cli-bin"))
        .expect("Failed to write cli manifest");

    let cli_extensions = ExtensionLoader::get_enabled_cli_extensions(temp_dir.path());
    assert_eq!(cli_extensions.len(), 1);
    assert_eq!(cli_extensions[0].manifest.extension.name, "cli-ext");
}

// ============================================================================
// Find CLI Extension Tests
// ============================================================================

#[test]
fn test_find_cli_extension_by_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let cli_ext = extensions_dir.join("my-cli");
    std::fs::create_dir_all(&cli_ext).expect("Failed to create cli ext");

    std::fs::write(
        cli_ext.join("manifest.yaml"),
        create_cli_manifest("my-cli", "my-cli-bin"),
    )
    .expect("Failed to write cli manifest");

    let found = ExtensionLoader::find_cli_extension(temp_dir.path(), "my-cli");
    assert!(found.is_some());
    assert_eq!(found.expect("Should find").manifest.extension.name, "my-cli");
}

#[test]
fn test_find_cli_extension_by_binary() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let cli_ext = extensions_dir.join("my-cli");
    std::fs::create_dir_all(&cli_ext).expect("Failed to create cli ext");

    std::fs::write(
        cli_ext.join("manifest.yaml"),
        create_cli_manifest("my-cli", "my-cli-binary"),
    )
    .expect("Failed to write cli manifest");

    let found = ExtensionLoader::find_cli_extension(temp_dir.path(), "my-cli-binary");
    assert!(found.is_some());
}

#[test]
fn test_find_cli_extension_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("Failed to create extensions dir");

    let found = ExtensionLoader::find_cli_extension(temp_dir.path(), "nonexistent");
    assert!(found.is_none());
}

// ============================================================================
// Binary Path Tests
// ============================================================================

#[test]
fn test_get_cli_binary_path_release() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let release_dir = temp_dir.path().join("target").join("release");
    std::fs::create_dir_all(&release_dir).expect("Failed to create release dir");

    let binary_path = release_dir.join("my-binary");
    std::fs::write(&binary_path, "binary content").expect("Failed to write binary");

    let found = ExtensionLoader::get_cli_binary_path(temp_dir.path(), "my-binary");
    assert!(found.is_some());
    let path = found.expect("Should find binary");
    assert!(path.to_string_lossy().contains("release"));
}

#[test]
fn test_get_cli_binary_path_debug_fallback() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let debug_dir = temp_dir.path().join("target").join("debug");
    std::fs::create_dir_all(&debug_dir).expect("Failed to create debug dir");

    let binary_path = debug_dir.join("debug-binary");
    std::fs::write(&binary_path, "binary content").expect("Failed to write binary");

    let found = ExtensionLoader::get_cli_binary_path(temp_dir.path(), "debug-binary");
    assert!(found.is_some());
    let path = found.expect("Should find binary");
    assert!(path.to_string_lossy().contains("debug"));
}

#[test]
fn test_get_cli_binary_path_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let found = ExtensionLoader::get_cli_binary_path(temp_dir.path(), "nonexistent");
    assert!(found.is_none());
}

#[test]
fn test_get_cli_binary_path_prefers_release() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let release_dir = temp_dir.path().join("target").join("release");
    let debug_dir = temp_dir.path().join("target").join("debug");
    std::fs::create_dir_all(&release_dir).expect("Failed to create release dir");
    std::fs::create_dir_all(&debug_dir).expect("Failed to create debug dir");

    std::fs::write(release_dir.join("my-binary"), "release").expect("Failed to write release");
    std::fs::write(debug_dir.join("my-binary"), "debug").expect("Failed to write debug");

    let found = ExtensionLoader::get_cli_binary_path(temp_dir.path(), "my-binary");
    assert!(found.is_some());
    let path = found.expect("Should find binary");
    assert!(
        path.to_string_lossy().contains("release"),
        "Should prefer release over debug"
    );
}

// ============================================================================
// Validate MCP Binaries Tests
// ============================================================================

#[test]
fn test_validate_mcp_binaries_all_present() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let release_dir = temp_dir.path().join("target").join("release");

    let ext_dir = extensions_dir.join("mcp-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");
    std::fs::create_dir_all(&release_dir).expect("Failed to create release dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("mcp-ext", "mcp-binary"),
    )
    .expect("Failed to write manifest");
    std::fs::write(release_dir.join("mcp-binary"), "binary").expect("Failed to write binary");

    let missing = ExtensionLoader::validate_mcp_binaries(temp_dir.path());
    assert!(missing.is_empty());
}

#[test]
fn test_validate_mcp_binaries_missing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("mcp-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("mcp-ext", "missing-binary"),
    )
    .expect("Failed to write manifest");

    let missing = ExtensionLoader::validate_mcp_binaries(temp_dir.path());
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].0, "missing-binary");
}

// ============================================================================
// MCP Binary Names Tests
// ============================================================================

#[test]
fn test_get_mcp_binary_names() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    for i in 1..=3 {
        let ext_dir = extensions_dir.join(format!("ext-{}", i));
        std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");
        std::fs::write(
            ext_dir.join("manifest.yaml"),
            create_mcp_manifest(&format!("ext-{}", i), &format!("binary-{}", i)),
        )
        .expect("Failed to write manifest");
    }

    let names = ExtensionLoader::get_mcp_binary_names(temp_dir.path());
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"binary-1".to_string()));
    assert!(names.contains(&"binary-2".to_string()));
    assert!(names.contains(&"binary-3".to_string()));
}

// ============================================================================
// Build Binary Map Tests
// ============================================================================

#[test]
fn test_build_binary_map() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    let ext1 = extensions_dir.join("ext-1");
    let ext2 = extensions_dir.join("ext-2");
    std::fs::create_dir_all(&ext1).expect("Failed to create ext1");
    std::fs::create_dir_all(&ext2).expect("Failed to create ext2");

    std::fs::write(ext1.join("manifest.yaml"), create_mcp_manifest("ext-1", "bin-1"))
        .expect("Failed to write manifest 1");
    std::fs::write(ext2.join("manifest.yaml"), create_mcp_manifest("ext-2", "bin-2"))
        .expect("Failed to write manifest 2");

    let map = ExtensionLoader::build_binary_map(temp_dir.path());
    assert_eq!(map.len(), 2);
    assert!(map.contains_key("bin-1"));
    assert!(map.contains_key("bin-2"));
    assert_eq!(map["bin-1"].manifest.extension.name, "ext-1");
    assert_eq!(map["bin-2"].manifest.extension.name, "ext-2");
}

#[test]
fn test_build_binary_map_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let map = ExtensionLoader::build_binary_map(temp_dir.path());
    assert!(map.is_empty());
}

// ============================================================================
// Validate Tests
// ============================================================================

#[test]
fn test_validate_returns_result() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("test-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("test-ext", "test-bin"),
    )
    .expect("Failed to write manifest");

    let result = ExtensionLoader::validate(temp_dir.path());
    assert_eq!(result.discovered.len(), 1);
    assert!(!result.missing_binaries.is_empty(), "Binary should be missing");
}

#[test]
fn test_validation_result_is_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let release_dir = temp_dir.path().join("target").join("release");

    let ext_dir = extensions_dir.join("test-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");
    std::fs::create_dir_all(&release_dir).expect("Failed to create release dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("test-ext", "test-bin"),
    )
    .expect("Failed to write manifest");
    std::fs::write(release_dir.join("test-bin"), "binary").expect("Failed to write binary");

    let result = ExtensionLoader::validate(temp_dir.path());
    assert!(result.is_valid());
}

#[test]
fn test_validation_result_format_missing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("test-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("test-ext", "missing-binary"),
    )
    .expect("Failed to write manifest");

    let result = ExtensionLoader::validate(temp_dir.path());
    let formatted = result.format_missing_binaries();
    assert!(formatted.contains("missing-binary"));
    assert!(formatted.contains("✗"));
}
