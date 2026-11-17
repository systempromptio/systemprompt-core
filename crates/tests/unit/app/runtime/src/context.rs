//! Unit tests for AppContextBuilder.

use systemprompt_runtime::AppContextBuilder;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_admitted};

#[test]
fn test_context_builder_with_startup_warnings_true() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted())
        .with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_with_startup_warnings_false() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted())
        .with_startup_warnings(false);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}

#[test]
fn test_context_builder_full_chain() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted())
        .with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_with_migrations() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted())
        .with_migrations(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("install_schemas: true"));
}

#[test]
fn test_context_builder_default_startup_warnings() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted());
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}
