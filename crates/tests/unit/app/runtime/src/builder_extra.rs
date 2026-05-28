//! Tests for AppContextBuilder fluent helpers not covered in context.rs.

use std::sync::Arc;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_runtime::{AppContextBuilder, MigrationConfig};

#[test]
fn with_migration_config_applies() {
    let cfg = MigrationConfig::default();
    let builder = AppContextBuilder::new().with_migration_config(cfg);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("AppContextBuilder"));
    assert!(dbg.contains("migration_config"));
}

#[test]
fn with_marketplace_filter_applies() {
    let builder = AppContextBuilder::new().with_marketplace_filter(Arc::new(AllowAllFilter));
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("marketplace_filter"));
}

#[test]
fn chain_combines_all_flags() {
    let builder = AppContextBuilder::new()
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
    let builder = AppContextBuilder::new().with_extensions(ExtensionRegistry::new());
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("extension_registry: true"));
}

#[test]
fn with_authz_hook_records_hook() {
    use std::sync::Arc;
    use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
    let hook = AllowAllHook::new(Arc::new(NullAuditSink));
    let builder = AppContextBuilder::new().with_authz_hook(hook);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("authz_hook: true"));
}

#[test]
fn with_shared_authz_hook_records_hook() {
    use std::sync::Arc;
    use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
    let hook: systemprompt_security::authz::SharedAuthzHook =
        Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let builder = AppContextBuilder::new().with_shared_authz_hook(hook);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("authz_hook: true"));
}

#[test]
fn with_startup_warnings_false_is_default() {
    let builder = AppContextBuilder::new().with_startup_warnings(false);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("show_startup_warnings: false"));
}

#[test]
fn with_migrations_false_is_default() {
    let builder = AppContextBuilder::new().with_migrations(false);
    let dbg = format!("{:?}", builder);
    assert!(dbg.contains("install_schemas: false"));
}
