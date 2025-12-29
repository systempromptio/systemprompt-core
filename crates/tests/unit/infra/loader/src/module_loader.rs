//! Unit tests for ModuleLoader
//!
//! Tests cover:
//! - Module YAML parsing
//! - Module existence checks
//! - Category scanning
//! - Module loading with required fields
//! - Module loading with optional fields

use std::path::PathBuf;
use systemprompt_loader::ModuleLoader;
use tempfile::TempDir;

// ============================================================================
// Load Module YAML Tests
// ============================================================================

#[test]
fn test_load_module_yaml_minimal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: test-module
version: "1.0.0"
display_name: Test Module
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert_eq!(module.name, "test-module");
    assert_eq!(module.version, "1.0.0");
    assert_eq!(module.display_name, "Test Module");
    assert!(module.description.is_none());
}

#[test]
fn test_load_module_yaml_full() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: full-module
version: "2.0.0"
display_name: Full Module
description: A complete module with all fields
weight: 50
dependencies:
  - base-module
  - core-module
audience:
  - api
  - mcp
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert_eq!(module.name, "full-module");
    assert_eq!(module.version, "2.0.0");
    assert_eq!(module.display_name, "Full Module");
    assert_eq!(
        module.description,
        Some("A complete module with all fields".to_string())
    );
    assert_eq!(module.weight, Some(50));
    assert_eq!(module.dependencies.len(), 2);
    assert!(module.dependencies.contains(&"base-module".to_string()));
    assert!(module.dependencies.contains(&"core-module".to_string()));
    assert_eq!(module.audience.len(), 2);
}

#[test]
fn test_load_module_yaml_with_schemas() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: schema-module
version: "1.0.0"
display_name: Schema Module
schemas:
  - file: migrations/001_create_users.sql
    table: users
    required_columns:
      - id
      - email
      - created_at
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert!(module.schemas.is_some());
    let schemas = module.schemas.expect("Should have schemas");
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].table, "users");
    assert_eq!(schemas[0].required_columns.len(), 3);
}

#[test]
fn test_load_module_yaml_with_seeds() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: seed-module
version: "1.0.0"
display_name: Seed Module
seeds:
  - file: seeds/default_roles.sql
    table: roles
    check_column: name
    check_value: admin
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert!(module.seeds.is_some());
    let seeds = module.seeds.expect("Should have seeds");
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0].table, "roles");
    assert_eq!(seeds[0].check_column, "name");
    assert_eq!(seeds[0].check_value, "admin");
}

#[test]
fn test_load_module_yaml_with_permissions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: perm-module
version: "1.0.0"
display_name: Permission Module
permissions:
  - name: read_users
    description: Allows reading user data
    resource: users
    action: read
  - name: write_users
    description: Allows writing user data
    resource: users
    action: write
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert!(module.permissions.is_some());
    let permissions = module.permissions.expect("Should have permissions");
    assert_eq!(permissions.len(), 2);
    assert_eq!(permissions[0].name, "read_users");
    assert_eq!(permissions[0].action, "read");
}

#[test]
fn test_load_module_yaml_with_api_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    let module_content = r#"
name: api-module
version: "1.0.0"
display_name: API Module
api:
  enabled: true
  path_prefix: /api/v1
  openapi_path: openapi.yaml
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert!(module.api.is_some());
    let api = module.api.expect("Should have api config");
    assert!(api.enabled);
    assert_eq!(api.path_prefix, Some("/api/v1".to_string()));
    assert_eq!(api.openapi_path, Some("openapi.yaml".to_string()));
}

#[test]
fn test_load_module_yaml_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    // Missing name and version
    let module_content = r#"
display_name: Incomplete Module
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_err());
}

#[test]
fn test_load_module_yaml_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_path = temp_dir.path().join("module.yaml");

    std::fs::write(&module_path, "invalid: yaml: syntax: here").expect("Failed to write file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_err());
}

#[test]
fn test_load_module_yaml_nonexistent() {
    let path = PathBuf::from("/nonexistent/module.yaml");
    let result = ModuleLoader::load_module_yaml(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_module_yaml_sets_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_dir = temp_dir.path().join("my-module");
    std::fs::create_dir(&module_dir).expect("Failed to create module dir");
    let module_path = module_dir.join("module.yaml");

    let module_content = r#"
name: path-test-module
version: "1.0.0"
display_name: Path Test Module
"#;

    std::fs::write(&module_path, module_content).expect("Failed to write module file");

    let result = ModuleLoader::load_module_yaml(&module_path);
    assert!(result.is_ok());

    let module = result.expect("Should load module");
    assert_eq!(module.path, module_dir);
}

// ============================================================================
// Exists Tests
// ============================================================================

#[test]
fn test_exists_true() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let module_file = temp_dir.path().join("module.yaml");
    std::fs::write(&module_file, "name: test\nversion: '1.0'\ndisplay_name: Test")
        .expect("Failed to write file");

    assert!(ModuleLoader::exists(temp_dir.path()));
}

#[test]
fn test_exists_false_no_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    assert!(!ModuleLoader::exists(temp_dir.path()));
}

#[test]
fn test_exists_false_wrong_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let other_file = temp_dir.path().join("config.yaml");
    std::fs::write(&other_file, "key: value").expect("Failed to write file");

    assert!(!ModuleLoader::exists(temp_dir.path()));
}

// ============================================================================
// Scan and Load Tests
// ============================================================================

#[test]
fn test_scan_and_load_empty_crates_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    // Create empty category directories
    for category in &["domain", "app", "infra"] {
        std::fs::create_dir(crates_dir.join(category)).expect("Failed to create category dir");
    }

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());
    assert!(result.expect("Should scan").is_empty());
}

#[test]
fn test_scan_and_load_with_modules() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    // Create domain category with a module
    let domain_dir = crates_dir.join("domain");
    std::fs::create_dir(&domain_dir).expect("Failed to create domain dir");
    let users_dir = domain_dir.join("users");
    std::fs::create_dir(&users_dir).expect("Failed to create users dir");

    let module_content = r#"
name: users
version: "1.0.0"
display_name: Users Module
weight: 10
"#;
    std::fs::write(users_dir.join("module.yaml"), module_content)
        .expect("Failed to write module file");

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());

    let modules = result.expect("Should scan");
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "users");
}

#[test]
fn test_scan_and_load_multiple_categories() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    // Create modules in different categories
    for (category, module_name, weight) in &[
        ("domain", "users", 10),
        ("app", "runtime", 20),
        ("infra", "database", 5),
    ] {
        let category_dir = crates_dir.join(category);
        std::fs::create_dir_all(&category_dir).expect("Failed to create category dir");
        let module_dir = category_dir.join(module_name);
        std::fs::create_dir(&module_dir).expect("Failed to create module dir");

        let module_content = format!(
            r#"
name: {}
version: "1.0.0"
display_name: {} Module
weight: {}
"#,
            module_name,
            module_name.to_uppercase(),
            weight
        );
        std::fs::write(module_dir.join("module.yaml"), module_content)
            .expect("Failed to write module file");
    }

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());

    let modules = result.expect("Should scan");
    assert_eq!(modules.len(), 3);

    // Should be sorted by weight
    assert_eq!(modules[0].name, "database"); // weight 5
    assert_eq!(modules[1].name, "users"); // weight 10
    assert_eq!(modules[2].name, "runtime"); // weight 20
}

#[test]
fn test_scan_and_load_nested_modules() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    // Create nested module structure
    let domain_dir = crates_dir.join("domain");
    std::fs::create_dir(&domain_dir).expect("Failed to create domain dir");
    let nested_dir = domain_dir.join("auth").join("oauth");
    std::fs::create_dir_all(&nested_dir).expect("Failed to create nested dir");

    let module_content = r#"
name: oauth
version: "1.0.0"
display_name: OAuth Module
"#;
    std::fs::write(nested_dir.join("module.yaml"), module_content)
        .expect("Failed to write module file");

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());

    let modules = result.expect("Should scan");
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "oauth");
}

#[test]
fn test_scan_and_load_no_crates_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Crates directory not found"));
}

#[test]
fn test_scan_and_load_skips_invalid_modules() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    let domain_dir = crates_dir.join("domain");
    std::fs::create_dir(&domain_dir).expect("Failed to create domain dir");

    // Valid module
    let valid_dir = domain_dir.join("valid");
    std::fs::create_dir(&valid_dir).expect("Failed to create valid dir");
    std::fs::write(
        valid_dir.join("module.yaml"),
        "name: valid\nversion: '1.0.0'\ndisplay_name: Valid",
    )
    .expect("Failed to write valid module");

    // Invalid module (missing required fields)
    let invalid_dir = domain_dir.join("invalid");
    std::fs::create_dir(&invalid_dir).expect("Failed to create invalid dir");
    std::fs::write(invalid_dir.join("module.yaml"), "display_name: Invalid Only")
        .expect("Failed to write invalid module");

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());

    let modules = result.expect("Should scan");
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "valid");
}

#[test]
fn test_scan_and_load_default_weight() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let crates_dir = temp_dir.path().join("crates");
    std::fs::create_dir(&crates_dir).expect("Failed to create crates dir");

    let domain_dir = crates_dir.join("domain");
    std::fs::create_dir(&domain_dir).expect("Failed to create domain dir");

    // Module without weight (should default to 100)
    let no_weight_dir = domain_dir.join("no-weight");
    std::fs::create_dir(&no_weight_dir).expect("Failed to create dir");
    std::fs::write(
        no_weight_dir.join("module.yaml"),
        "name: no-weight\nversion: '1.0.0'\ndisplay_name: No Weight",
    )
    .expect("Failed to write module");

    // Module with explicit weight
    let with_weight_dir = domain_dir.join("with-weight");
    std::fs::create_dir(&with_weight_dir).expect("Failed to create dir");
    std::fs::write(
        with_weight_dir.join("module.yaml"),
        "name: with-weight\nversion: '1.0.0'\ndisplay_name: With Weight\nweight: 50",
    )
    .expect("Failed to write module");

    let result = ModuleLoader::scan_and_load(temp_dir.path().to_str().expect("Valid path"));
    assert!(result.is_ok());

    let modules = result.expect("Should scan");
    assert_eq!(modules.len(), 2);

    // with-weight (50) should come before no-weight (default 100)
    assert_eq!(modules[0].name, "with-weight");
    assert_eq!(modules[1].name, "no-weight");
}
