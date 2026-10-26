//! Tests for AppContextBuilder fluent helpers not covered in context.rs.

use std::sync::Arc;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_runtime::{AppContextBuilder, MigrationConfig};
use systemprompt_test_fixtures::ensure_test_bootstrap;


#[test]
fn chain_combines_all_flags() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone())
        .with_startup_warnings(true)
        .with_migrations(true)
        .with_migration_config(MigrationConfig::default())
        .with_marketplace_filter(Arc::new(AllowAllFilter));
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("show_startup_warnings: true"));
    assert!(dbg.contains("install_schemas: true"));
}

#[test]
fn with_extensions_records_registry() {
    use systemprompt_extension::ExtensionRegistry;
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone())
        .with_extensions(ExtensionRegistry::new());
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("extension_registry: true"));
}

#[test]
fn with_authz_hook_records_hook() {
    use std::sync::Arc;
    use systemprompt_governance::authz::{AllowAllHook, NullAuditSink};
    let hook = AllowAllHook::new(Arc::new(NullAuditSink));
    let builder =
        AppContextBuilder::new(ensure_test_bootstrap().boot.clone()).with_authz_hook(hook);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("authz_hook: true"));
}

#[test]
fn with_shared_authz_hook_records_hook() {
    use std::sync::Arc;
    use systemprompt_governance::authz::{AllowAllHook, NullAuditSink};
    let hook: systemprompt_governance::authz::SharedAuthzHook =
        Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let builder =
        AppContextBuilder::new(ensure_test_bootstrap().boot.clone()).with_shared_authz_hook(hook);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("authz_hook: true"));
}

#[test]
fn with_startup_warnings_false_is_default() {
    let builder =
        AppContextBuilder::new(ensure_test_bootstrap().boot.clone()).with_startup_warnings(false);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("show_startup_warnings: false"));
}

#[test]
fn with_migrations_false_is_default() {
    let builder =
        AppContextBuilder::new(ensure_test_bootstrap().boot.clone()).with_migrations(false);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("install_schemas: false"));
}
