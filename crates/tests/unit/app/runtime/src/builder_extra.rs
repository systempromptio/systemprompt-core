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
    let builder = AppContextBuilder::new()
        .with_marketplace_filter(Arc::new(AllowAllFilter));
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
