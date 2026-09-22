//! Schema installation from compile-time-registered
//! [`systemprompt_extension::Extension`] instances.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extension;
mod fk_deferral;
mod migration_cost;
mod migration_refs;
mod prepare;
mod report;
mod seeds;
mod undeclared;

pub use extension::{
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config,
};
pub use fk_deferral::{
    DeferredForeignKey, FkDeferralError, SplitCreateTable, split_create_table_foreign_keys,
};
pub use migration_cost::{
    ExpensiveStatement, HOT_TABLES, MigrationCost, audit_migration_cost, audit_one,
};
pub use migration_refs::check_migration_references;
pub use report::{ForeignKeyDrift, SchemaInstallReport};
pub use undeclared::{OrphanMigrationLedger, SchemaResidue, UndeclaredTable, audit_schema_residue};

pub use extension::lock::{BOOTSTRAP_ADVISORY_LOCK_KEY, BootstrapLockGuard};
