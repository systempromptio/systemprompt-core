use systemprompt_database::services::schema_linter::created_table_names;
use systemprompt_loader::ModuleLoader;

#[test]
fn test_extensions_have_required_metadata() {
    let extensions =
        ModuleLoader::discover_extensions().expect("extension discovery should succeed");

    for ext in &extensions {
        assert!(!ext.id().is_empty(), "Extension id should not be empty");
        assert!(!ext.name().is_empty(), "Extension name should not be empty");
        assert!(
            !ext.version().is_empty(),
            "Extension version should not be empty"
        );
    }
}

#[test]
fn test_schemas_have_required_fields() {
    let schemas =
        ModuleLoader::collect_extension_schemas().expect("schema collection should succeed");

    for schema in &schemas {
        assert!(
            !schema.sql.trim().is_empty(),
            "Schema SQL should not be empty"
        );
        if let Some(table) = schema.table.as_deref() {
            assert!(!table.is_empty(), "Named schema table should not be empty");
        }
    }
}

#[test]
fn every_declared_table_is_created_by_its_own_sql() {
    let schemas =
        ModuleLoader::collect_extension_schemas().expect("schema collection should succeed");

    for schema in &schemas {
        let Some(declared) = schema.table.as_deref() else {
            continue;
        };
        let created = created_table_names(&schema.sql);
        assert!(
            created.iter().any(|t| t == declared),
            "declared table '{declared}' is not created by its own SQL, which creates {created:?}"
        );
    }
}

#[test]
fn every_created_table_is_declared_by_some_extension() {
    let schemas =
        ModuleLoader::collect_extension_schemas().expect("schema collection should succeed");

    let declared: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();

    for schema in &schemas {
        for created in created_table_names(&schema.sql) {
            assert!(
                declared.contains(&created.as_str()),
                "table '{created}' is created by declarative schema but no extension declares it, \
                 so `db doctor` would report it as undeclared"
            );
        }
    }
}

#[test]
fn table_less_schemas_declare_no_table() {
    let schemas =
        ModuleLoader::collect_extension_schemas().expect("schema collection should succeed");

    let table_less: Vec<_> = schemas.iter().filter(|s| s.table.is_none()).collect();

    assert!(
        !table_less.is_empty(),
        "the database extension declares its shared trigger functions with no owning table"
    );
    for schema in table_less {
        assert!(
            !schema.sql.to_uppercase().contains("CREATE TABLE"),
            "a table-less schema definition must not create a table"
        );
        assert!(
            schema.required_columns.is_empty(),
            "a table-less schema definition has no columns to validate"
        );
    }
}
