use systemprompt_extension::types::{
    DependencyList, ExtensionMeta, ExtensionType, MissingDependency, NoDependencies,
};

#[derive(Debug, Default)]
struct TestExt;

impl ExtensionType for TestExt {
    const ID: &'static str = "test-ext";
    const NAME: &'static str = "Test Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for TestExt {}

#[derive(Debug, Default)]
struct PriorityExt;

impl ExtensionType for PriorityExt {
    const ID: &'static str = "priority-ext";
    const NAME: &'static str = "Priority Extension";
    const VERSION: &'static str = "2.0.0";
    const PRIORITY: u32 = 50;
}

impl NoDependencies for PriorityExt {}

#[test]
fn extension_type_id() {
    assert_eq!(TestExt::ID, "test-ext");
}

#[test]
fn extension_type_name() {
    assert_eq!(TestExt::NAME, "Test Extension");
}

#[test]
fn extension_type_version() {
    assert_eq!(TestExt::VERSION, "1.0.0");
}

#[test]
fn extension_type_default_priority() {
    assert_eq!(TestExt::PRIORITY, 100);
}

#[test]
fn extension_type_custom_priority() {
    assert_eq!(PriorityExt::PRIORITY, 50);
}

#[test]
fn extension_type_type_id_returns_valid_id() {
    let type_id = TestExt::type_id();
    assert_eq!(type_id, std::any::TypeId::of::<TestExt>());
}

#[test]
fn extension_meta_id_via_trait() {
    let ext = TestExt;
    assert_eq!(ExtensionMeta::id(&ext), "test-ext");
}

#[test]
fn extension_meta_name_via_trait() {
    let ext = TestExt;
    assert_eq!(ExtensionMeta::name(&ext), "Test Extension");
}

#[test]
fn extension_meta_version_via_trait() {
    let ext = TestExt;
    assert_eq!(ExtensionMeta::version(&ext), "1.0.0");
}

#[test]
fn extension_meta_priority_via_trait() {
    let ext = PriorityExt;
    assert_eq!(ExtensionMeta::priority(&ext), 50);
}

#[test]
fn empty_dependency_list_validate_succeeds() {
    let registry = systemprompt_extension::TypedExtensionRegistry::new();
    let result = <() as DependencyList>::validate(&registry);
    assert!(result.is_ok());
}

#[test]
fn empty_dependency_list_ids_empty() {
    let ids = <() as DependencyList>::dependency_ids();
    assert!(ids.is_empty());
}

#[test]
fn missing_dependency_has_fields() {
    let missing = MissingDependency {
        extension_id: "ext-a",
        extension_name: "Extension A",
    };
    assert_eq!(missing.extension_id, "ext-a");
    assert_eq!(missing.extension_name, "Extension A");
}

#[test]
fn missing_dependency_debug_format() {
    let missing = MissingDependency {
        extension_id: "dep-x",
        extension_name: "Dep X",
    };
    let debug = format!("{missing:?}");
    assert!(debug.contains("dep-x"));
}
