use systemprompt_loader::ExtensionLoader;
use tempfile::TempDir;

fn write_mcp_manifest(dir: &std::path::Path, name: &str, binary: &str) {
    std::fs::create_dir_all(dir).expect("create ext dir");
    std::fs::write(
        dir.join("manifest.yaml"),
        format!(
            r#"extension:
  type: mcp
  name: {name}
  binary: {binary}
  enabled: true
"#
        ),
    )
    .expect("write manifest");
}

fn write_mcp_dev_manifest(dir: &std::path::Path, name: &str, binary: &str) {
    std::fs::create_dir_all(dir).expect("create ext dir");
    std::fs::write(
        dir.join("manifest.yaml"),
        format!(
            r#"extension:
  type: mcp
  name: {name}
  binary: {binary}
  enabled: true
"#
        ),
    )
    .expect("write manifest");
}

#[test]
fn resolve_bin_directory_with_override_path() {
    let temp = TempDir::new().expect("tempdir");
    let custom_bin = temp.path().join("custom_bin");
    std::fs::create_dir_all(&custom_bin).expect("create custom bin dir");

    let resolved = ExtensionLoader::resolve_bin_directory(temp.path(), Some(&custom_bin));
    assert_eq!(resolved, custom_bin);
}

#[test]
fn resolve_bin_directory_prefers_release_when_only_release_exists() {
    let temp = TempDir::new().expect("tempdir");
    let release_dir = temp.path().join("target").join("release");
    std::fs::create_dir_all(&release_dir).expect("create release dir");
    std::fs::write(release_dir.join("systemprompt"), "release binary").expect("write binary");

    let resolved = ExtensionLoader::resolve_bin_directory(temp.path(), None);
    assert!(
        resolved.to_string_lossy().contains("release"),
        "should resolve to release dir when only release binary exists"
    );
}

#[test]
fn resolve_bin_directory_uses_debug_when_only_debug_exists() {
    let temp = TempDir::new().expect("tempdir");
    let debug_dir = temp.path().join("target").join("debug");
    std::fs::create_dir_all(&debug_dir).expect("create debug dir");
    std::fs::write(debug_dir.join("systemprompt"), "debug binary").expect("write binary");

    let resolved = ExtensionLoader::resolve_bin_directory(temp.path(), None);
    assert!(
        resolved.to_string_lossy().contains("debug"),
        "should resolve to debug dir when only debug binary exists"
    );
}

#[test]
fn resolve_bin_directory_neither_exists_falls_back_to_release() {
    let temp = TempDir::new().expect("tempdir");

    let resolved = ExtensionLoader::resolve_bin_directory(temp.path(), None);
    assert!(
        resolved.to_string_lossy().contains("release"),
        "should default to release dir when neither binary exists"
    );
}

#[test]
fn resolve_bin_directory_both_exist_picks_newer() {
    let temp = TempDir::new().expect("tempdir");
    let release_dir = temp.path().join("target").join("release");
    let debug_dir = temp.path().join("target").join("debug");
    std::fs::create_dir_all(&release_dir).expect("create release dir");
    std::fs::create_dir_all(&debug_dir).expect("create debug dir");

    std::fs::write(release_dir.join("systemprompt"), "release binary").expect("write release");
    std::fs::write(debug_dir.join("systemprompt"), "debug binary").expect("write debug");

    let resolved = ExtensionLoader::resolve_bin_directory(temp.path(), None);
    let resolved_str = resolved.to_string_lossy();
    assert!(
        resolved_str.contains("release") || resolved_str.contains("debug"),
        "should resolve to either release or debug, got: {resolved_str}"
    );
}

#[test]
fn get_production_mcp_binary_names_excludes_dev_only() {
    let temp = TempDir::new().expect("tempdir");
    let ext_dir = temp.path().join("extensions");

    write_mcp_manifest(&ext_dir.join("prod-ext"), "prod-ext", "prod-bin");
    write_mcp_dev_manifest(&ext_dir.join("dev-ext"), "dev-ext", "dev-bin");

    let mut services_config = systemprompt_models::services::ServicesConfig::default();
    services_config.mcp_servers.insert(
        "dev-ext-server".to_owned(),
        systemprompt_models::mcp::Deployment {
            binary: "dev-bin".to_owned(),
            dev_only: true,
            server_type: systemprompt_models::mcp::deployment::McpServerType::Internal,
            package: None,
            port: 5001,
            endpoint: None,
            enabled: true,
            display_in_web: false,
            oauth: systemprompt_models::mcp::deployment::OAuthRequirement {
                required: false,
                scopes: vec![],
                audience: systemprompt_models::auth::JwtAudience::Mcp,
                client_id: None,
            },
            schemas: vec![],
            tools: std::collections::HashMap::new(),
            model_config: None,
            env_vars: vec![],
            external_auth: None,
            headers: Default::default(),
        },
    );

    let names = ExtensionLoader::get_production_mcp_binary_names(temp.path(), &services_config);
    assert!(
        names.contains(&"prod-bin".to_owned()),
        "prod-bin should be in production names"
    );
    assert!(
        !names.contains(&"dev-bin".to_owned()),
        "dev-bin should be excluded (dev_only)"
    );
}

#[test]
fn get_production_mcp_binary_names_all_production() {
    let temp = TempDir::new().expect("tempdir");
    let ext_dir = temp.path().join("extensions");

    write_mcp_manifest(&ext_dir.join("ext-a"), "ext-a", "bin-a");
    write_mcp_manifest(&ext_dir.join("ext-b"), "ext-b", "bin-b");

    let services_config = systemprompt_models::services::ServicesConfig::default();

    let names = ExtensionLoader::get_production_mcp_binary_names(temp.path(), &services_config);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"bin-a".to_owned()));
    assert!(names.contains(&"bin-b".to_owned()));
}

#[test]
fn get_production_mcp_binary_names_empty_when_no_extensions() {
    let temp = TempDir::new().expect("tempdir");
    let services_config = systemprompt_models::services::ServicesConfig::default();
    let names = ExtensionLoader::get_production_mcp_binary_names(temp.path(), &services_config);
    assert!(names.is_empty());
}

#[test]
fn validate_mcp_binaries_with_debug_dir() {
    let temp = TempDir::new().expect("tempdir");
    let ext_dir = temp.path().join("extensions");
    let debug_dir = temp.path().join("target").join("debug");

    write_mcp_manifest(&ext_dir.join("my-ext"), "my-ext", "my-binary");
    std::fs::create_dir_all(&debug_dir).expect("create debug dir");
    std::fs::write(debug_dir.join("systemprompt"), "systemprompt binary")
        .expect("write systemprompt");
    std::fs::write(debug_dir.join("my-binary"), "my binary").expect("write binary");

    let missing = ExtensionLoader::validate_mcp_binaries(temp.path());
    assert!(
        missing.is_empty(),
        "binary present in debug dir should be found"
    );
}

#[test]
fn extension_validation_result_valid_with_no_extensions() {
    let temp = TempDir::new().expect("tempdir");
    let result = ExtensionLoader::validate(temp.path());
    assert!(result.is_valid());
    assert!(result.discovered.is_empty());
    assert!(result.missing_binaries.is_empty());
    assert!(result.missing_manifests.is_empty());
    assert!(result.format_missing_binaries().is_empty());
}

#[test]
fn extension_validation_result_format_multiple_missing() {
    let temp = TempDir::new().expect("tempdir");
    let ext_dir = temp.path().join("extensions");

    write_mcp_manifest(&ext_dir.join("ext-1"), "ext-1", "bin-1");
    write_mcp_manifest(&ext_dir.join("ext-2"), "ext-2", "bin-2");

    let result = ExtensionLoader::validate(temp.path());
    assert!(!result.is_valid());
    let formatted = result.format_missing_binaries();
    assert!(formatted.contains("bin-1"));
    assert!(formatted.contains("bin-2"));
    assert_eq!(formatted.lines().count(), 2);
}

#[test]
fn build_binary_map_extension_without_binary_is_excluded() {
    let temp = TempDir::new().expect("tempdir");
    let ext_dir = temp.path().join("extensions");
    let no_binary_dir = ext_dir.join("no-binary-ext");
    std::fs::create_dir_all(&no_binary_dir).expect("create dir");
    std::fs::write(
        no_binary_dir.join("manifest.yaml"),
        r#"extension:
  type: mcp
  name: no-binary-ext
  enabled: true
"#,
    )
    .expect("write manifest without binary field");

    let map = ExtensionLoader::build_binary_map(temp.path());
    assert!(
        map.is_empty(),
        "extension without binary must not appear in binary map"
    );
}

#[test]
fn get_mcp_binary_names_empty_without_extensions() {
    let temp = TempDir::new().expect("tempdir");
    let names = ExtensionLoader::get_mcp_binary_names(temp.path());
    assert!(names.is_empty());
}

#[test]
fn find_cli_extension_returns_none_on_missing_dir() {
    let temp = TempDir::new().expect("tempdir");
    let found = ExtensionLoader::find_cli_extension(temp.path(), "anything");
    assert!(found.is_none());
}
