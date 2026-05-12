//! Unit tests for AppContextBuilder.

use systemprompt_extension::ExtensionRegistry;
use systemprompt_runtime::AppContextBuilder;

#[test]
fn test_context_builder_new() {
    let builder = AppContextBuilder::new();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_default() {
    let builder = AppContextBuilder::default();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_with_extensions() {
    let registry = ExtensionRegistry::new();
    let builder = AppContextBuilder::new().with_extensions(registry);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_with_startup_warnings_true() {
    let builder = AppContextBuilder::new().with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_with_startup_warnings_false() {
    let builder = AppContextBuilder::new().with_startup_warnings(false);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}

#[test]
fn test_context_builder_full_chain() {
    let registry = ExtensionRegistry::new();
    let builder = AppContextBuilder::new()
        .with_extensions(registry)
        .with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_with_migrations() {
    let builder = AppContextBuilder::new().with_migrations(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("install_schemas: true"));
}

#[test]
fn test_context_builder_default_startup_warnings() {
    let builder = AppContextBuilder::default();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}
