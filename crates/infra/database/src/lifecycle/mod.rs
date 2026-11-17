//! Database lifecycle: extension schema installation, migrations, and
//! connection/schema validation.
//!
//! Re-exports the schema installers, the [`MigrationService`] and its result
//! and status types, and the standalone validation helpers
//! ([`validate_database_connection`], [`validate_write_pool_is_primary`],
//! [`validate_table_exists`], [`validate_column_exists`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod installation;
mod migrations;
mod validation;

pub mod schema_member;
pub use installation::{
    AbsentMemberTable, BOOTSTRAP_ADVISORY_LOCK_KEY, BackfillStatement, BootstrapLockGuard,
    BootstrapLockTimeout, DeferredForeignKey, ExpensiveStatement, FkDeferralError, ForeignKeyDrift,
    HOT_TABLES, HiddenSql, KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationCost, MigrationMembers,
    MigrationOwnershipError, ObjectKind, OrphanMigrationLedger, PROJECTION_BACKLOG,
    RETRYABLE_SQLSTATES, Recorded, RecordedObject, RecordedTable, SchemaAhead, SchemaCurrency,
    SchemaInstallReport, SchemaResidue, SplitCreateTable, TableMutation, UndeclaredTable,
    audit_migration_cost, audit_one, audit_schema_residue, backfill_statements,
    check_migration_ownership, check_migration_references, check_trigger_routines,
    install_admitted_schemas, install_schema_members, is_retryable_lock_conflict,
    is_retryable_sqlstate, migration_ownership_findings, projection_tables, schema_catalog_digest,
    schema_currency, split_create_table_foreign_keys,
};
pub use migrations::{
    AppliedMigration, BaselineStamp, ChecksumDrift, ExtensionMigrationStatus, FreshnessCheck,
    MarkAppliedOutcome, MigrationConfig, MigrationResult, MigrationService, MigrationStatus,
    OrphanedMigration, PendingMigration, RepairResult, SlotCollision, TombstonedSlot,
    is_retirement,
};
pub use schema_member::{SchemaMember, SchemaUnit, schema_members};
pub use validation::{
    ReplicaStatus, replica_status, validate_column_exists, validate_database_connection,
    validate_table_exists, validate_write_pool_is_primary,
};
