//! Schema installation from compile-time-registered
//! [`SchemaMember`](crate::lifecycle::schema_member::SchemaMember) snapshots of
//! the admitted composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod backfill;
mod classify;
mod extension;
mod fk_deferral;
mod lock_conflict;
mod migration_cost;
mod migration_owners;
mod migration_refs;
mod prepare;
mod report;
mod routine_refs;
mod seeds;
mod undeclared;

pub use backfill::{BackfillStatement, PROJECTION_BACKLOG, backfill_statements, projection_tables};
pub use extension::catalog_digest::catalog_digest as schema_catalog_digest;
pub use extension::current::{SchemaCurrency, schema_currency};
pub use extension::{install_admitted_schemas, install_schema_members};
pub use fk_deferral::{
    DeferredForeignKey, FkDeferralError, SplitCreateTable, split_create_table_foreign_keys,
};
pub use lock_conflict::{RETRYABLE_SQLSTATES, is_retryable_lock_conflict, is_retryable_sqlstate};
pub use migration_cost::{
    ExpensiveStatement, HOT_TABLES, MigrationCost, audit_migration_cost, audit_one,
};
pub use migration_owners::{
    HiddenSql, KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, MigrationOwnershipError,
    ObjectKind, Recorded, RecordedObject, RecordedTable, TableMutation, check_migration_ownership,
    migration_ownership_findings,
};
pub use migration_refs::check_migration_references;
pub(crate) use extension::release_order::refuse_superseded;
pub use extension::release_order::{SCHEMA_INSTALL_RELEASE, SCHEMA_INSTALL_STAMP};
pub use report::{ForeignKeyDrift, SchemaAhead, SchemaInstallReport, SupersededInstall};
pub use routine_refs::check_trigger_routines;
pub use undeclared::{
    AbsentMemberTable, OrphanMigrationLedger, SchemaResidue, UndeclaredTable, audit_schema_residue,
};

pub use extension::lock::{BOOTSTRAP_ADVISORY_LOCK_KEY, BootstrapLockGuard, BootstrapLockTimeout};
