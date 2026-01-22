use systemprompt_loader::ModuleLoader;

#[test]
fn test_discover_extensions_returns_extensions() {
    let extensions = ModuleLoader::discover_extensions();
    assert!(
        !extensions.is_empty(),
        "Should discover registered extensions"
    );
}

#[test]
fn test_extensions_have_required_metadata() {
    let extensions = ModuleLoader::discover_extensions();

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
fn test_collect_extension_schemas() {
    let schemas = ModuleLoader::collect_extension_schemas();
    assert!(
        !schemas.is_empty(),
        "Should collect schemas from extensions"
    );
}

#[test]
fn test_schemas_have_required_fields() {
    let schemas = ModuleLoader::collect_extension_schemas();

    for schema in &schemas {
        assert!(!schema.table.is_empty(), "Schema table should not be empty");
    }
}
