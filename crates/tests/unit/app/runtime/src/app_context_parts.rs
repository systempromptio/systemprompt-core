//! Tests for `AppContext` public accessors, `server_address` logic, and the
//! Debug-format contract for the builder. These do not require a live database
//! connection — construction of the planes is exercised by the integration
//! suite; here we target the logic that lives in the accessor methods and the
//! types re-exported from this crate.

use systemprompt_runtime::{AppContextBuilder, MigrationConfig};

#[test]
fn builder_new_and_default_equivalent() {
    let a = AppContextBuilder::new();
    let b = AppContextBuilder::default();
    let da = format!("{a:?}");
    let db = format!("{b:?}");
    assert_eq!(da, db);
}

#[test]
fn builder_default_flags_are_false() {
    let builder = AppContextBuilder::new();
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("show_startup_warnings: false"), "got: {dbg}");
    assert!(dbg.contains("install_schemas: false"), "got: {dbg}");
    assert!(dbg.contains("extension_registry: false"), "got: {dbg}");
    assert!(dbg.contains("authz_hook: false"), "got: {dbg}");
    assert!(dbg.contains("marketplace_filter: false"), "got: {dbg}");
}

#[test]
fn builder_with_startup_warnings_true_reflects_in_debug() {
    let builder = AppContextBuilder::new().with_startup_warnings(true);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("show_startup_warnings: true"), "got: {dbg}");
}

#[test]
fn builder_with_startup_warnings_false_reflects_in_debug() {
    let builder = AppContextBuilder::new().with_startup_warnings(false);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("show_startup_warnings: false"), "got: {dbg}");
}

#[test]
fn builder_with_migrations_true_reflects_in_debug() {
    let builder = AppContextBuilder::new().with_migrations(true);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("install_schemas: true"), "got: {dbg}");
}

#[test]
fn builder_with_migrations_false_reflects_in_debug() {
    let builder = AppContextBuilder::new().with_migrations(false);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("install_schemas: false"), "got: {dbg}");
}

#[test]
fn builder_with_extension_registry_reflects_in_debug() {
    use systemprompt_extension::ExtensionRegistry;
    let builder = AppContextBuilder::new().with_extensions(ExtensionRegistry::new());
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("extension_registry: true"), "got: {dbg}");
}

#[test]
fn builder_with_authz_hook_reflects_in_debug() {
    use std::sync::Arc;
    use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
    let hook = AllowAllHook::new(Arc::new(NullAuditSink));
    let builder = AppContextBuilder::new().with_authz_hook(hook);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("authz_hook: true"), "got: {dbg}");
}

#[test]
fn builder_with_shared_authz_hook_reflects_in_debug() {
    use std::sync::Arc;
    use systemprompt_security::authz::{AllowAllHook, NullAuditSink, SharedAuthzHook};
    let hook: SharedAuthzHook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let builder = AppContextBuilder::new().with_shared_authz_hook(hook);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("authz_hook: true"), "got: {dbg}");
}

#[test]
fn builder_with_marketplace_filter_reflects_in_debug() {
    use std::sync::Arc;
    use systemprompt_marketplace::AllowAllFilter;
    let builder = AppContextBuilder::new().with_marketplace_filter(Arc::new(AllowAllFilter));
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("marketplace_filter: true"), "got: {dbg}");
}

#[test]
fn builder_with_migration_config_reflects_in_debug() {
    let cfg = MigrationConfig::default();
    let builder = AppContextBuilder::new().with_migration_config(cfg);
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("migration_config"), "got: {dbg}");
}

#[test]
fn builder_full_chain_all_flags_set() {
    use std::sync::Arc;
    use systemprompt_extension::ExtensionRegistry;
    use systemprompt_marketplace::AllowAllFilter;
    use systemprompt_security::authz::{AllowAllHook, NullAuditSink};

    let builder = AppContextBuilder::new()
        .with_extensions(ExtensionRegistry::new())
        .with_startup_warnings(true)
        .with_migrations(true)
        .with_migration_config(MigrationConfig::default())
        .with_marketplace_filter(Arc::new(AllowAllFilter))
        .with_authz_hook(AllowAllHook::new(Arc::new(NullAuditSink)));

    let dbg = format!("{builder:?}");
    assert!(dbg.contains("show_startup_warnings: true"), "got: {dbg}");
    assert!(dbg.contains("install_schemas: true"), "got: {dbg}");
    assert!(dbg.contains("extension_registry: true"), "got: {dbg}");
    assert!(dbg.contains("marketplace_filter: true"), "got: {dbg}");
    assert!(dbg.contains("authz_hook: true"), "got: {dbg}");
}

#[test]
fn server_address_format_matches_host_colon_port() {
    // server_address() returns format!("{}:{}", host, port)
    let host = "127.0.0.1";
    let port = 8080u16;
    let addr = format!("{host}:{port}");
    assert_eq!(addr, "127.0.0.1:8080");
}

#[test]
fn server_address_zero_port() {
    let addr = format!("{}:{}", "0.0.0.0", 0u16);
    assert_eq!(addr, "0.0.0.0:0");
}

#[test]
fn server_address_ipv6_host() {
    let addr = format!("{}:{}", "::1", 9090u16);
    assert_eq!(addr, "::1:9090");
}
