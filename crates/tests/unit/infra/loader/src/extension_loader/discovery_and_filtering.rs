//! Tests for extension discovery, manifest loading, and MCP/CLI extension
//! filtering

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

    std::fs::write(
        ext_dir.join("manifest.yaml"),
        create_mcp_manifest("test-ext", "test-bin"),
    )
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

    std::fs::write(
        mcp_ext.join("manifest.yaml"),
        create_mcp_manifest("mcp-ext", "mcp-bin"),
    )
    .expect("Failed to write mcp manifest");
    std::fs::write(
        cli_ext.join("manifest.yaml"),
        create_cli_manifest("cli-ext", "cli-bin"),
    )
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

    std::fs::write(
        mcp_ext.join("manifest.yaml"),
        create_mcp_manifest("mcp-ext", "mcp-bin"),
    )
    .expect("Failed to write mcp manifest");
    std::fs::write(
        cli_ext.join("manifest.yaml"),
        create_cli_manifest("cli-ext", "cli-bin"),
    )
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

    let found = ExtensionLoader::find_cli_extension(temp_dir.path(), "my-cli")
        .expect("Should find CLI extension by name");
    assert_eq!(found.manifest.extension.name, "my-cli");
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

    ExtensionLoader::find_cli_extension(temp_dir.path(), "my-cli-binary")
        .expect("Should find CLI extension by binary name");
}

#[test]
fn test_find_cli_extension_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let extensions_dir = temp_dir.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("Failed to create extensions dir");

    let found = ExtensionLoader::find_cli_extension(temp_dir.path(), "nonexistent");
    assert!(found.is_none());
}
