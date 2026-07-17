//! Database lifecycle: extension schema installation, migrations, and
//! connection/schema validation.
//!
//! Re-exports the schema installers, the [`MigrationService`] and its result
//! and status types, and the standalone validation helpers
//! ([`validate_database_connection`], [`validate_table_exists`],
//! [`validate_column_exists`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod installation;
mod migrations;
mod validation;

pub use installation::{
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config,
};
pub use migrations::{
    AppliedMigration, ChecksumDrift, ExtensionMigrationStatus, MarkAppliedOutcome, MigrationConfig,
    MigrationResult, MigrationService, MigrationStatus, PendingMigration, RepairResult, SquashPlan,
};
pub use validation::{validate_column_exists, validate_database_connection, validate_table_exists};
