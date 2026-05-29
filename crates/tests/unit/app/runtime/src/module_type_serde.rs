//! Tests for `ModuleType` serialization / deserialization and the
//! `ModuleApiRegistry::modules_by_category` filter.

use systemprompt_runtime::{ModuleApiRegistry, ModuleType, ServiceCategory};

#[test]
fn module_type_regular_serializes_to_regular() {
    let json = serde_json::to_string(&ModuleType::Regular).expect("serialize");
    assert!(json.contains("Regular"), "got: {json}");
}

#[test]
fn module_type_proxy_serializes_to_proxy() {
    let json = serde_json::to_string(&ModuleType::Proxy).expect("serialize");
    assert!(json.contains("Proxy"), "got: {json}");
}

#[test]
fn module_type_regular_round_trips() {
    let json = serde_json::to_string(&ModuleType::Regular).expect("serialize");
    let back: ModuleType = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, ModuleType::Regular);
}

#[test]
fn module_type_proxy_round_trips() {
    let json = serde_json::to_string(&ModuleType::Proxy).expect("serialize");
    let back: ModuleType = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, ModuleType::Proxy);
}

#[test]
fn module_type_eq_same_variant() {
    assert_eq!(ModuleType::Regular, ModuleType::Regular);
    assert_eq!(ModuleType::Proxy, ModuleType::Proxy);
}

#[test]
fn module_type_ne_different_variants() {
    assert_ne!(ModuleType::Regular, ModuleType::Proxy);
}

#[test]
fn module_type_copy_semantics() {
    let a = ModuleType::Proxy;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn module_type_debug_regular() {
    let dbg = format!("{:?}", ModuleType::Regular);
    assert!(dbg.contains("Regular"), "got: {dbg}");
}

#[test]
fn module_type_debug_proxy() {
    let dbg = format!("{:?}", ModuleType::Proxy);
    assert!(dbg.contains("Proxy"), "got: {dbg}");
}

#[test]
fn modules_by_category_returns_empty_for_unknown_when_no_modules_registered() {
    let registry = ModuleApiRegistry::new();
    let modules = registry.modules_by_category(ServiceCategory::Core);
    // In the test binary no modules are registered via inventory, so all categories are empty.
    assert!(
        modules.is_empty(),
        "expected empty in test binary, got {modules:?}"
    );
}

#[test]
fn modules_by_all_service_categories_return_vec() {
    let registry = ModuleApiRegistry::new();
    for cat in [
        ServiceCategory::Core,
        ServiceCategory::Agent,
        ServiceCategory::Mcp,
        ServiceCategory::Meta,
    ] {
        let modules = registry.modules_by_category(cat);
        let _ = modules; // just verifying no panic
    }
}

#[test]
fn registry_lookup_returns_none_for_unregistered_module() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_category("no-such-module").is_none());
    assert!(registry.get_module_type("no-such-module").is_none());
    assert!(registry.get_auth_required("no-such-module").is_none());
}
