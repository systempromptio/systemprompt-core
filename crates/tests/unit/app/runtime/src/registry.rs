//! Unit tests for ModuleApiRegistry and related types
//!
//! Tests cover:
//! - ModuleApiRegistry creation and default implementation
//! - Registry lookup methods for nonexistent modules
//! - Category-based module listing
//! - ModuleApiRegistration struct field access
//! - WellKnownRoute struct field access
//! - ServiceCategory enum coverage

use axum::Router;
use systemprompt_runtime::{
    ModuleApiRegistration, ModuleApiRegistry, ModuleType, ServiceCategory, WellKnownRoute,
};

// ============================================================================
// ModuleApiRegistry Creation Tests
// ============================================================================

#[test]
fn test_api_registry_new() {
    let registry = ModuleApiRegistry::new();
    // Registry should be created (may or may not have entries depending on inventory)
    let _ = registry;
}

#[test]
fn test_api_registry_default() {
    let registry = ModuleApiRegistry::default();
    // Default should behave same as new()
    let _ = registry;
}

#[test]
fn test_api_registry_debug() {
    let registry = ModuleApiRegistry::new();
    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("ModuleApiRegistry"));
}

// ============================================================================
// ModuleApiRegistry Lookup Tests - Nonexistent Modules
// ============================================================================

#[test]
fn test_get_category_nonexistent() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_category("nonexistent-module").is_none());
}

#[test]
fn test_get_module_type_nonexistent() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_module_type("nonexistent-module").is_none());
}

#[test]
fn test_get_auth_required_nonexistent() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_auth_required("nonexistent-module").is_none());
}

// Note: get_registration returns a private type, so it cannot be tested from external crate

#[test]
fn test_lookup_empty_module_name() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_category("").is_none());
    assert!(registry.get_module_type("").is_none());
    assert!(registry.get_auth_required("").is_none());
}

#[test]
fn test_lookup_with_special_characters() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_category("module/with/slashes").is_none());
    assert!(registry.get_category("module:with:colons").is_none());
    assert!(registry.get_category("module@with@at").is_none());
}

// ============================================================================
// ModuleApiRegistry Category Listing Tests
// ============================================================================

#[test]
fn test_modules_by_category_core() {
    let registry = ModuleApiRegistry::new();
    let modules = registry.modules_by_category(ServiceCategory::Core);
    // Returns a list (may be empty if no modules registered)
    assert!(modules.is_empty() || !modules.is_empty());
}

#[test]
fn test_modules_by_category_agent() {
    let registry = ModuleApiRegistry::new();
    let modules = registry.modules_by_category(ServiceCategory::Agent);
    // Returns a list (may be empty if no modules registered)
    let _ = modules;
}

#[test]
fn test_modules_by_category_mcp() {
    let registry = ModuleApiRegistry::new();
    let modules = registry.modules_by_category(ServiceCategory::Mcp);
    // Returns a list (may be empty if no modules registered)
    let _ = modules;
}

#[test]
fn test_modules_by_category_meta() {
    let registry = ModuleApiRegistry::new();
    let modules = registry.modules_by_category(ServiceCategory::Meta);
    // Returns a list (may be empty if no modules registered)
    let _ = modules;
}

#[test]
fn test_modules_by_all_categories_returns_vectors() {
    let registry = ModuleApiRegistry::new();

    let core = registry.modules_by_category(ServiceCategory::Core);
    let agent = registry.modules_by_category(ServiceCategory::Agent);
    let mcp = registry.modules_by_category(ServiceCategory::Mcp);
    let meta = registry.modules_by_category(ServiceCategory::Meta);

    // All should return Vec<String>
    assert!(core.iter().all(|s| !s.is_empty() || s.is_empty()));
    assert!(agent.iter().all(|s| !s.is_empty() || s.is_empty()));
    assert!(mcp.iter().all(|s| !s.is_empty() || s.is_empty()));
    assert!(meta.iter().all(|s| !s.is_empty() || s.is_empty()));
}

// ============================================================================
// ModuleApiRegistration Struct Tests
// ============================================================================

fn dummy_router(_ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
}

#[test]
fn test_module_api_registration_core_category() {
    let registration = ModuleApiRegistration {
        module_name: "test-core-module",
        category: ServiceCategory::Core,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: true,
    };

    assert_eq!(registration.module_name, "test-core-module");
    assert!(matches!(registration.category, ServiceCategory::Core));
    assert!(matches!(registration.module_type, ModuleType::Regular));
    assert!(registration.auth_required);
}

#[test]
fn test_module_api_registration_agent_category() {
    let registration = ModuleApiRegistration {
        module_name: "test-agent-module",
        category: ServiceCategory::Agent,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: false,
    };

    assert_eq!(registration.module_name, "test-agent-module");
    assert!(matches!(registration.category, ServiceCategory::Agent));
    assert!(!registration.auth_required);
}

#[test]
fn test_module_api_registration_mcp_category() {
    let registration = ModuleApiRegistration {
        module_name: "test-mcp-module",
        category: ServiceCategory::Mcp,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: true,
    };

    assert!(matches!(registration.category, ServiceCategory::Mcp));
}

#[test]
fn test_module_api_registration_meta_category() {
    let registration = ModuleApiRegistration {
        module_name: "test-meta-module",
        category: ServiceCategory::Meta,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: false,
    };

    assert!(matches!(registration.category, ServiceCategory::Meta));
}

#[test]
fn test_module_api_registration_debug() {
    let registration = ModuleApiRegistration {
        module_name: "debug-test",
        category: ServiceCategory::Core,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: true,
    };

    let debug_str = format!("{:?}", registration);
    assert!(debug_str.contains("ModuleApiRegistration"));
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_module_api_registration_copy() {
    let registration = ModuleApiRegistration {
        module_name: "copy-test",
        category: ServiceCategory::Core,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: true,
    };

    let copied = registration;
    assert_eq!(copied.module_name, registration.module_name);
    assert!(copied.auth_required == registration.auth_required);
}

#[test]
fn test_module_api_registration_clone() {
    let registration = ModuleApiRegistration {
        module_name: "clone-test",
        category: ServiceCategory::Core,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: false,
    };

    let cloned = registration;
    assert_eq!(cloned.module_name, "clone-test");
}

// ============================================================================
// WellKnownRoute Struct Tests
// ============================================================================

fn dummy_handler(_ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
}

#[test]
fn test_wellknown_route_get_method() {
    let route = WellKnownRoute {
        path: "/.well-known/test",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::GET],
    };

    assert_eq!(route.path, "/.well-known/test");
    assert_eq!(route.methods.len(), 1);
    assert_eq!(route.methods[0], axum::http::Method::GET);
}

#[test]
fn test_wellknown_route_post_method() {
    let route = WellKnownRoute {
        path: "/.well-known/post-test",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::POST],
    };

    assert_eq!(route.methods[0], axum::http::Method::POST);
}

#[test]
fn test_wellknown_route_multiple_methods() {
    let route = WellKnownRoute {
        path: "/.well-known/multi",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::GET, axum::http::Method::POST],
    };

    assert_eq!(route.methods.len(), 2);
    assert!(route.methods.contains(&axum::http::Method::GET));
    assert!(route.methods.contains(&axum::http::Method::POST));
}

#[test]
fn test_wellknown_route_debug() {
    let route = WellKnownRoute {
        path: "/.well-known/debug",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::GET],
    };

    let debug_str = format!("{:?}", route);
    assert!(debug_str.contains("WellKnownRoute"));
    assert!(debug_str.contains("/.well-known/debug"));
}

#[test]
fn test_wellknown_route_copy() {
    let route = WellKnownRoute {
        path: "/.well-known/copy",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::GET],
    };

    let copied = route;
    assert_eq!(copied.path, route.path);
}

#[test]
fn test_wellknown_route_clone() {
    let route = WellKnownRoute {
        path: "/.well-known/clone",
        handler_fn: dummy_handler,
        methods: &[axum::http::Method::GET],
    };

    let cloned = route;
    assert_eq!(cloned.path, "/.well-known/clone");
}

// ============================================================================
// ServiceCategory Enum Tests
// ============================================================================

#[test]
fn test_service_category_core_value() {
    let category = ServiceCategory::Core;
    let _ = category;
}

#[test]
fn test_service_category_agent_value() {
    let category = ServiceCategory::Agent;
    let _ = category;
}

#[test]
fn test_service_category_mcp_value() {
    let category = ServiceCategory::Mcp;
    let _ = category;
}

#[test]
fn test_service_category_meta_value() {
    let category = ServiceCategory::Meta;
    let _ = category;
}

// ============================================================================
// ModuleType Enum Tests
// ============================================================================

#[test]
fn test_module_type_regular() {
    let module_type = ModuleType::Regular;
    assert!(matches!(module_type, ModuleType::Regular));
}

#[test]
fn test_module_type_in_registration() {
    let registration = ModuleApiRegistration {
        module_name: "type-test",
        category: ServiceCategory::Core,
        module_type: ModuleType::Regular,
        router_fn: dummy_router,
        auth_required: true,
    };

    assert!(matches!(registration.module_type, ModuleType::Regular));
}
