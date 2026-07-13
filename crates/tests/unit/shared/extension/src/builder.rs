use systemprompt_extension::builder::ExtensionBuilder;
use systemprompt_extension::typed::{SchemaDefinitionTyped, SchemaExtensionTyped};
use systemprompt_extension::types::{ExtensionType, NoDependencies};

#[derive(Debug, Default)]
struct ExtA;

impl ExtensionType for ExtA {
    const ID: &'static str = "ext-a";
    const NAME: &'static str = "Extension A";
    const VERSION: &'static str = "1.0.0";
    const PRIORITY: u32 = 10;
}

impl NoDependencies for ExtA {}

#[derive(Debug, Default)]
struct ExtB;

impl ExtensionType for ExtB {
    const ID: &'static str = "ext-b";
    const NAME: &'static str = "Extension B";
    const VERSION: &'static str = "1.0.0";
    const PRIORITY: u32 = 20;
}

impl NoDependencies for ExtB {}

#[derive(Debug, Default)]
struct SchemaExtA;

impl ExtensionType for SchemaExtA {
    const ID: &'static str = "schema-ext-a";
    const NAME: &'static str = "Schema Extension A";
    const VERSION: &'static str = "1.0.0";
    const PRIORITY: u32 = 15;
}

impl NoDependencies for SchemaExtA {}

impl SchemaExtensionTyped for SchemaExtA {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::new(
            "widgets",
            "CREATE TABLE widgets (id TEXT)",
        )]
    }
}

#[test]
fn builder_new_creates_empty() {
    let builder = ExtensionBuilder::new();
    let registry = builder.build().expect("build should succeed");
    assert!(registry.is_empty());
}

#[test]
fn builder_default_creates_empty() {
    let builder = ExtensionBuilder::default();
    let registry = builder.build().expect("build should succeed");
    assert_eq!(registry.len(), 0);
}

#[test]
fn builder_single_extension() {
    let registry = ExtensionBuilder::new()
        .extension(ExtA)
        .build()
        .expect("build should succeed");
    assert_eq!(registry.len(), 1);
    assert!(registry.has("ext-a"));
}

#[test]
fn builder_two_extensions() {
    let registry = ExtensionBuilder::new()
        .extension(ExtA)
        .extension(ExtB)
        .build()
        .expect("build should succeed");
    assert_eq!(registry.len(), 2);
    assert!(registry.has("ext-a"));
    assert!(registry.has("ext-b"));
}

#[test]
fn builder_duplicate_extension_fails() {
    let result = ExtensionBuilder::new()
        .extension(ExtA)
        .extension(ExtA)
        .build();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("ext-a"));
}

#[test]
fn builder_sorts_by_priority() {
    let registry = ExtensionBuilder::new()
        .extension(ExtB)
        .extension(ExtA)
        .build()
        .expect("build should succeed");
    let all: Vec<_> = registry.all_extensions().collect();
    assert_eq!(all[0].id(), "ext-a");
    assert_eq!(all[1].id(), "ext-b");
}

#[test]
fn builder_schema_extension_registers_and_exposes_schema() {
    let registry = ExtensionBuilder::new()
        .schema_extension(SchemaExtA)
        .build()
        .expect("build should succeed");

    assert!(registry.has("schema-ext-a"));

    let schema_exts: Vec<_> = registry.schema_extensions().collect();
    assert_eq!(
        schema_exts.len(),
        1,
        "schema extension must surface through the typed schema-extension view"
    );
    let tables: Vec<_> = schema_exts[0]
        .schemas()
        .into_iter()
        .map(|s| s.table)
        .collect();
    assert_eq!(tables, vec!["widgets"]);
}

#[test]
fn builder_debug_format() {
    let builder = ExtensionBuilder::new().extension(ExtA);
    let debug = format!("{builder:?}");
    assert!(debug.contains("ExtensionBuilder"));
    assert!(debug.contains("extension_count"));
}
