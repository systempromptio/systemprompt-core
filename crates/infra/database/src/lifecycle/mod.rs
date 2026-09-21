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

pub use installation::{
    BOOTSTRAP_ADVISORY_LOCK_KEY, BootstrapLockGuard, DeferredForeignKey, FkDeferralError,
    ForeignKeyDrift, SchemaInstallReport, SplitCreateTable, check_migration_references,
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config, split_create_table_foreign_keys,
};
pub use migrations::{
    AppliedMigration, BaselineStamp, ChecksumDrift, ExtensionMigrationStatus, FreshnessCheck,
    MarkAppliedOutcome, MigrationConfig, MigrationResult, MigrationService, MigrationStatus,
    OrphanedMigration, PendingMigration, RepairResult, SlotCollision, TombstonedSlot,
};
pub use validation::{
    ReplicaStatus, replica_status, validate_column_exists, validate_database_connection,
    validate_table_exists, validate_write_pool_is_primary,
};
