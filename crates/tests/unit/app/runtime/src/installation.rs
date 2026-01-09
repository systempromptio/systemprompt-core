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
use systemprompt_extension::{SchemaSource, SeedSource};
use systemprompt_runtime::{Module, ModuleSchema, ModuleSeed};

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
fn test_module_creation() {
    let module = create_test_module("test-module", PathBuf::from("/tmp/test"));

    assert_eq!(module.name, "test-module");
    assert_eq!(module.version, "1.0.0");
    assert!(module.enabled);
}

#[test]
fn test_module_name_access() {
    let module = create_test_module("my-module", PathBuf::from("/tmp"));
    assert_eq!(module.name, "my-module");
}

#[test]
fn test_module_path_access() {
    let path = PathBuf::from("/var/modules/test");
    let module = create_test_module("path-test", path.clone());
    assert_eq!(module.path, path);
}

#[test]
fn test_module_enabled_flag() {
    let module = create_test_module("enabled-test", PathBuf::from("/tmp"));
    assert!(module.enabled);
}

#[test]
fn test_module_with_description() {
    let module = Module {
        uuid: "desc-uuid".to_string(),
        name: "desc-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Description Module".to_string(),
        description: Some("This is a detailed description".to_string()),
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

    assert!(module.description.is_some());
    assert!(module.description.unwrap().contains("detailed"));
}

#[test]
fn test_module_with_dependencies() {
    let module = Module {
        uuid: "dep-uuid".to_string(),
        name: "dep-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Dependency Module".to_string(),
        description: None,
        weight: None,
        dependencies: vec!["core".to_string(), "auth".to_string()],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::from("/tmp"),
    };

    assert_eq!(module.dependencies.len(), 2);
    assert!(module.dependencies.contains(&"core".to_string()));
}

#[test]
fn test_module_with_weight() {
    let module = Module {
        uuid: "weight-uuid".to_string(),
        name: "weight-module".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Weight Module".to_string(),
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

#[test]
fn test_module_schema_creation() {
    let schema = ModuleSchema {
        sql: SchemaSource::File(PathBuf::from("schema.sql")),
        table: "users".to_string(),
        required_columns: vec!["id".to_string(), "name".to_string()],
    };

    assert_eq!(schema.table, "users");
    assert_eq!(schema.required_columns.len(), 2);
}

#[test]
fn test_module_schema_empty_columns() {
    let schema = ModuleSchema {
        sql: SchemaSource::File(PathBuf::from("empty.sql")),
        table: "empty_table".to_string(),
        required_columns: vec![],
    };

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_module_schema_single_column() {
    let schema = ModuleSchema {
        sql: SchemaSource::File(PathBuf::from("single.sql")),
        table: "single_table".to_string(),
        required_columns: vec!["id".to_string()],
    };

    assert_eq!(schema.required_columns.len(), 1);
    assert_eq!(schema.required_columns[0], "id");
}

#[test]
fn test_module_schema_multiple_columns() {
    let schema = ModuleSchema {
        sql: SchemaSource::File(PathBuf::from("multi.sql")),
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
fn test_module_schema_with_inline_sql() {
    let schema = ModuleSchema {
        sql: SchemaSource::Inline("CREATE TABLE test (id INT)".to_string()),
        table: "test".to_string(),
        required_columns: vec!["id".to_string()],
    };

    match &schema.sql {
        SchemaSource::Inline(sql) => assert!(sql.contains("CREATE TABLE")),
        SchemaSource::File(_) => panic!("Expected Inline"),
    }
}

#[test]
fn test_module_seed_creation() {
    let seed = ModuleSeed {
        sql: SeedSource::File(PathBuf::from("seed.sql")),
        table: "users".to_string(),
        check_column: "email".to_string(),
        check_value: "admin@example.com".to_string(),
    };

    assert_eq!(seed.table, "users");
    assert_eq!(seed.check_column, "email");
    assert_eq!(seed.check_value, "admin@example.com");
}

#[test]
fn test_module_seed_with_id_check() {
    let seed = ModuleSeed {
        sql: SeedSource::File(PathBuf::from("initial_data.sql")),
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
        sql: SeedSource::File(PathBuf::from("roles.sql")),
        table: "roles".to_string(),
        check_column: "name".to_string(),
        check_value: "admin".to_string(),
    };

    assert_eq!(seed.check_column, "name");
    assert_eq!(seed.check_value, "admin");
}

#[test]
fn test_module_seed_with_inline_sql() {
    let seed = ModuleSeed {
        sql: SeedSource::Inline("INSERT INTO users (id, name) VALUES (1, 'Admin')".to_string()),
        table: "users".to_string(),
        check_column: "id".to_string(),
        check_value: "1".to_string(),
    };

    match &seed.sql {
        SeedSource::Inline(sql) => assert!(sql.contains("INSERT INTO")),
        SeedSource::File(_) => panic!("Expected Inline"),
    }
}

#[test]
fn test_module_seed_empty_check_value() {
    let seed = ModuleSeed {
        sql: SeedSource::File(PathBuf::from("empty.sql")),
        table: "empty".to_string(),
        check_column: "status".to_string(),
        check_value: "".to_string(),
    };

    assert!(seed.check_value.is_empty());
}

#[test]
fn test_module_with_schemas() {
    let schemas = vec![
        ModuleSchema {
            sql: SchemaSource::File(PathBuf::from("schema1.sql")),
            table: "table1".to_string(),
            required_columns: vec!["id".to_string()],
        },
        ModuleSchema {
            sql: SchemaSource::File(PathBuf::from("schema2.sql")),
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

#[test]
fn test_module_with_seeds() {
    let seeds = vec![
        ModuleSeed {
            sql: SeedSource::File(PathBuf::from("seed1.sql")),
            table: "table1".to_string(),
            check_column: "id".to_string(),
            check_value: "1".to_string(),
        },
        ModuleSeed {
            sql: SeedSource::File(PathBuf::from("seed2.sql")),
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

#[test]
fn test_module_complete_configuration() {
    let schemas = vec![ModuleSchema {
        sql: SchemaSource::File(PathBuf::from("schema.sql")),
        table: "complete".to_string(),
        required_columns: vec!["id".to_string()],
    }];

    let seeds = vec![ModuleSeed {
        sql: SeedSource::File(PathBuf::from("seed.sql")),
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
