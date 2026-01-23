//! Unit tests for ExtensionRegistry
//!
//! Tests cover:
//! - Registry building for local and cloud modes
//! - Path resolution for extensions
//! - Extension lookup by binary name
//! - Extension existence checking

use systemprompt_loader::ExtensionRegistry;
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
// Build Registry Tests
// ============================================================================

#[test]
fn test_build_registry_local_mode() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("test-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("test-ext", "test-binary"),
    )
    .expect("Failed to write manifest");

    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    assert!(registry.has_extension("test-binary"));
}

#[test]
fn test_build_registry_cloud_mode() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    std::fs::write(bin_path.join("cloud-binary"), "binary").expect("Failed to write binary");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        true,
        bin_path.to_str().expect("Valid path"),
    );

    assert!(registry.has_extension("cloud-binary"));
}

#[test]
fn test_build_registry_cloud_mode_ignores_local_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("local-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("local-ext", "local-binary"),
    )
    .expect("Failed to write manifest");

    let registry = ExtensionRegistry::build(temp_dir.path(), true, "/usr/local/bin");

    assert!(!registry.has_extension("local-binary"));
}

// ============================================================================
// Get Path Tests
// ============================================================================

#[test]
fn test_get_path_local_mode() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("path-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("path-ext", "path-binary"),
    )
    .expect("Failed to write manifest");

    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");
    let result = registry.get_path("path-binary");

    assert!(result.is_ok());
    let path = result.expect("Should get path");
    assert!(path.to_string_lossy().contains("path-ext"));
}

#[test]
fn test_get_path_local_mode_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    let result = registry.get_path("nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No manifest.yaml found"));
}

#[test]
fn test_get_path_cloud_mode() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("cloud-bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    std::fs::write(bin_path.join("cloud-ext"), "binary").expect("Failed to write binary");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        true,
        bin_path.to_str().expect("Valid path"),
    );

    let result = registry.get_path("cloud-ext");
    assert!(result.is_ok());
}

#[test]
fn test_get_path_cloud_mode_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("empty-bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        true,
        bin_path.to_str().expect("Valid path"),
    );

    let result = registry.get_path("missing");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ============================================================================
// Get Extension Tests
// ============================================================================

#[test]
fn test_get_extension_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("get-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("get-ext", "get-binary"),
    )
    .expect("Failed to write manifest");

    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");
    let ext = registry.get_extension("get-binary");

    assert!(ext.is_some());
    let discovered = ext.expect("Should find extension");
    assert_eq!(discovered.manifest.extension.name, "get-ext");
}

#[test]
fn test_get_extension_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    let ext = registry.get_extension("nonexistent");
    assert!(ext.is_none());
}

#[test]
fn test_get_extension_cloud_mode_returns_none() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    std::fs::write(bin_path.join("cloud-bin"), "binary").expect("Failed to write binary");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        true,
        bin_path.to_str().expect("Valid path"),
    );

    let ext = registry.get_extension("cloud-bin");
    assert!(ext.is_none(), "Cloud mode doesn't store DiscoveredExtension");
}

// ============================================================================
// Has Extension Tests
// ============================================================================

#[test]
fn test_has_extension_local_discovered() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    let ext_dir = extensions_dir.join("has-ext");
    std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("has-ext", "has-binary"),
    )
    .expect("Failed to write manifest");

    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    assert!(registry.has_extension("has-binary"));
}

#[test]
fn test_has_extension_cloud_in_bin_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    std::fs::write(bin_path.join("exists-binary"), "binary").expect("Failed to write binary");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        true,
        bin_path.to_str().expect("Valid path"),
    );

    assert!(registry.has_extension("exists-binary"));
    assert!(!registry.has_extension("not-exists"));
}

#[test]
fn test_has_extension_local_checks_bin_path_too() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bin_path = temp_dir.path().join("bin");
    std::fs::create_dir_all(&bin_path).expect("Failed to create bin dir");

    std::fs::write(bin_path.join("bin-only"), "binary").expect("Failed to write binary");

    let registry = ExtensionRegistry::build(
        temp_dir.path(),
        false,
        bin_path.to_str().expect("Valid path"),
    );

    assert!(
        registry.has_extension("bin-only"),
        "Should find binary in bin_path even in local mode"
    );
}

#[test]
fn test_has_extension_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/nonexistent/bin");

    assert!(!registry.has_extension("anything"));
}

// ============================================================================
// Multiple Extensions Tests
// ============================================================================

#[test]
fn test_registry_with_multiple_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");

    for i in 1..=5 {
        let ext_dir = extensions_dir.join(format!("ext-{}", i));
        std::fs::create_dir_all(&ext_dir).expect("Failed to create ext dir");
        std::fs::write(
            ext_dir.join("manifest.yaml"),
            create_mcp_manifest(&format!("ext-{}", i), &format!("binary-{}", i)),
        )
        .expect("Failed to write manifest");
    }

    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    for i in 1..=5 {
        assert!(registry.has_extension(&format!("binary-{}", i)));
        assert!(registry.get_extension(&format!("binary-{}", i)).is_some());
    }

    assert!(!registry.has_extension("binary-6"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_registry_empty_project() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = ExtensionRegistry::build(temp_dir.path(), false, "/usr/local/bin");

    assert!(!registry.has_extension("any"));
    assert!(registry.get_extension("any").is_none());
    assert!(registry.get_path("any").is_err());
}

#[test]
fn test_registry_with_empty_bin_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = ExtensionRegistry::build(temp_dir.path(), true, "");

    assert!(!registry.has_extension("any"));
}
