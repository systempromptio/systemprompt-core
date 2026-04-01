//! Tests for binary path resolution, binary validation, and binary map building

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
