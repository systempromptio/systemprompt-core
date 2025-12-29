//! Unit tests for module installation types
//!
//! Tests cover:
//! - Module struct creation and field access
//! - ModuleSchema struct creation and validation
//! - ModuleSeed struct creation and validation
//!
//! Note: The async installation functions (install_module, install_module_with_db)
//! require database setup and are tested in integration tests.

use std::path::PathBuf;
use systemprompt_runtime::{Module, ModuleSchema, ModuleSeed};

// ============================================================================
// Module Struct Tests
// ============================================================================

fn create_test_module(name: &str, path: PathBuf) -> Module {
    Module {
        uuid: "test-uuid-12345".to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        display_name: name.to_string(),
        description: Some("Test module description".to_string()),
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path,
    }
}

#[test]
fn test_module_creation_basic() {
    let module = create_test_module("test-module", PathBuf::from("/tmp/test"));

    assert_eq!(module.name, "test-module");
    assert_eq!(module.version, "1.0.0");
    assert!(module.enabled);
}

#[test]
fn test_module_with_uuid() {
    let module = Module {
        uuid: "unique-uuid-67890".to_string(),
        name: "uuid-module".to_string(),
        version: "2.0.0".to_string(),
        display_name: "UUID Module".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/modules/uuid"),
    };

    assert_eq!(module.uuid, "unique-uuid-67890");
}

#[test]
fn test_module_with_description() {
    let module = create_test_module("desc-module", PathBuf::from("/tmp"));

    assert!(module.description.is_some());
    assert_eq!(
        module.description.unwrap(),
        "Test module description".to_string()
    );
}

#[test]
fn test_module_without_description() {
    let module = Module {
        uuid: "no-desc-uuid".to_string(),
        name: "no-desc-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "No Desc".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert!(module.description.is_none());
}

#[test]
fn test_module_with_weight() {
    let module = Module {
        uuid: "weight-uuid".to_string(),
        name: "weight-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Weight".to_string(),
        description: None,
        weight: Some(100),
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert_eq!(module.weight, Some(100));
}

#[test]
fn test_module_with_dependencies() {
    let module = Module {
        uuid: "deps-uuid".to_string(),
        name: "deps-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Deps".to_string(),
        description: None,
        weight: None,
        dependencies: vec!["dep1".to_string(), "dep2".to_string()],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert_eq!(module.dependencies.len(), 2);
    assert!(module.dependencies.contains(&"dep1".to_string()));
    assert!(module.dependencies.contains(&"dep2".to_string()));
}

#[test]
fn test_module_disabled() {
    let module = Module {
        uuid: "disabled-uuid".to_string(),
        name: "disabled-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Disabled".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: false,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert!(!module.enabled);
}

#[test]
fn test_module_path() {
    let path = PathBuf::from("/var/modules/my-module");
    let module = create_test_module("path-module", path.clone());

    assert_eq!(module.path, path);
}

#[test]
fn test_module_with_audience() {
    let module = Module {
        uuid: "audience-uuid".to_string(),
        name: "audience-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Audience".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec!["api".to_string(), "mcp".to_string()],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert_eq!(module.audience.len(), 2);
    assert!(module.audience.contains(&"api".to_string()));
}

// ============================================================================
// ModuleSchema Struct Tests
// ============================================================================

#[test]
fn test_module_schema_creation() {
    let schema = ModuleSchema {
        file: "schema.sql".to_string(),
        table: "users".to_string(),
        required_columns: vec!["id".to_string(), "name".to_string()],
    };

    assert_eq!(schema.file, "schema.sql");
    assert_eq!(schema.table, "users");
    assert_eq!(schema.required_columns.len(), 2);
}

#[test]
fn test_module_schema_empty_columns() {
    let schema = ModuleSchema {
        file: "empty.sql".to_string(),
        table: "empty_table".to_string(),
        required_columns: vec![],
    };

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_module_schema_single_column() {
    let schema = ModuleSchema {
        file: "single.sql".to_string(),
        table: "single_table".to_string(),
        required_columns: vec!["id".to_string()],
    };

    assert_eq!(schema.required_columns.len(), 1);
    assert_eq!(schema.required_columns[0], "id");
}

#[test]
fn test_module_schema_multiple_columns() {
    let schema = ModuleSchema {
        file: "multi.sql".to_string(),
        table: "multi_table".to_string(),
        required_columns: vec![
            "id".to_string(),
            "created_at".to_string(),
            "updated_at".to_string(),
            "deleted_at".to_string(),
        ],
    };

    assert_eq!(schema.required_columns.len(), 4);
}

#[test]
fn test_module_schema_with_path_in_file() {
    let schema = ModuleSchema {
        file: "schemas/v1/create_tables.sql".to_string(),
        table: "nested_table".to_string(),
        required_columns: vec!["id".to_string()],
    };

    assert!(schema.file.contains('/'));
}

// ============================================================================
// ModuleSeed Struct Tests
// ============================================================================

#[test]
fn test_module_seed_creation() {
    let seed = ModuleSeed {
        file: "seed.sql".to_string(),
        table: "users".to_string(),
        check_column: "email".to_string(),
        check_value: "admin@example.com".to_string(),
    };

    assert_eq!(seed.file, "seed.sql");
    assert_eq!(seed.table, "users");
    assert_eq!(seed.check_column, "email");
    assert_eq!(seed.check_value, "admin@example.com");
}

#[test]
fn test_module_seed_with_id_check() {
    let seed = ModuleSeed {
        file: "initial_data.sql".to_string(),
        table: "settings".to_string(),
        check_column: "id".to_string(),
        check_value: "1".to_string(),
    };

    assert_eq!(seed.check_column, "id");
    assert_eq!(seed.check_value, "1");
}

#[test]
fn test_module_seed_with_name_check() {
    let seed = ModuleSeed {
        file: "roles.sql".to_string(),
        table: "roles".to_string(),
        check_column: "name".to_string(),
        check_value: "admin".to_string(),
    };

    assert_eq!(seed.check_column, "name");
    assert_eq!(seed.check_value, "admin");
}

#[test]
fn test_module_seed_with_path_in_file() {
    let seed = ModuleSeed {
        file: "seeds/production/initial.sql".to_string(),
        table: "config".to_string(),
        check_column: "key".to_string(),
        check_value: "initialized".to_string(),
    };

    assert!(seed.file.contains('/'));
}

#[test]
fn test_module_seed_empty_check_value() {
    let seed = ModuleSeed {
        file: "empty.sql".to_string(),
        table: "empty".to_string(),
        check_column: "status".to_string(),
        check_value: "".to_string(),
    };

    assert!(seed.check_value.is_empty());
}

// ============================================================================
// Module with Schemas Tests
// ============================================================================

#[test]
fn test_module_with_schemas() {
    let schemas = vec![
        ModuleSchema {
            file: "schema1.sql".to_string(),
            table: "table1".to_string(),
            required_columns: vec!["id".to_string()],
        },
        ModuleSchema {
            file: "schema2.sql".to_string(),
            table: "table2".to_string(),
            required_columns: vec!["id".to_string(), "name".to_string()],
        },
    ];

    let module = Module {
        uuid: "schema-uuid".to_string(),
        name: "schema-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Schema Module".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: Some(schemas),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert!(module.schemas.is_some());
    assert_eq!(module.schemas.as_ref().unwrap().len(), 2);
}

// ============================================================================
// Module with Seeds Tests
// ============================================================================

#[test]
fn test_module_with_seeds() {
    let seeds = vec![
        ModuleSeed {
            file: "seed1.sql".to_string(),
            table: "table1".to_string(),
            check_column: "id".to_string(),
            check_value: "1".to_string(),
        },
        ModuleSeed {
            file: "seed2.sql".to_string(),
            table: "table2".to_string(),
            check_column: "name".to_string(),
            check_value: "default".to_string(),
        },
    ];

    let module = Module {
        uuid: "seed-uuid".to_string(),
        name: "seed-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Seed Module".to_string(),
        description: None,
        weight: None,
        dependencies: vec![],
        schemas: None,
        seeds: Some(seeds),
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert!(module.seeds.is_some());
    assert_eq!(module.seeds.as_ref().unwrap().len(), 2);
}

// ============================================================================
// Module Complete Configuration Tests
// ============================================================================

#[test]
fn test_module_complete_configuration() {
    let schemas = vec![ModuleSchema {
        file: "schema.sql".to_string(),
        table: "complete".to_string(),
        required_columns: vec!["id".to_string()],
    }];

    let seeds = vec![ModuleSeed {
        file: "seed.sql".to_string(),
        table: "complete".to_string(),
        check_column: "id".to_string(),
        check_value: "1".to_string(),
    }];

    let module = Module {
        uuid: "complete-uuid".to_string(),
        name: "complete-module".to_string(),
        version: "2.1.0".to_string(),
        display_name: "Complete Module".to_string(),
        description: Some("A fully configured module".to_string()),
        weight: Some(50),
        dependencies: vec!["base".to_string()],
        schemas: Some(schemas),
        seeds: Some(seeds),
        permissions: None,
        audience: vec!["api".to_string()],
        enabled: true,
        api: None,
        path: PathBuf::from("/var/modules/complete"),
    };

    assert!(module.schemas.is_some());
    assert!(module.seeds.is_some());
    assert!(module.description.is_some());
    assert!(module.weight.is_some());
    assert!(!module.dependencies.is_empty());
    assert!(!module.audience.is_empty());
}
