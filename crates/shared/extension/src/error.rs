//! Typed error enums raised by extension registration and configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Extension '{extension}' requires dependency '{dependency}' which is not registered")]
    MissingDependency {
        extension: String,
        dependency: String,
    },

    #[error("Extension with ID '{0}' is already registered")]
    DuplicateExtension(String),

    #[error("Extension '{0}' is required and cannot be disabled")]
    RequiredExtensionDisabled(String),

    #[error(
        "Extension '{dependency}' is disabled but extension '{extension}' depends on it; disable \
         '{extension}' as well or re-enable '{dependency}'"
    )]
    DisabledDependency {
        extension: String,
        dependency: String,
    },

    #[error("Failed to initialize extension '{extension}': {message}")]
    InitializationFailed { extension: String, message: String },

    #[error("Failed to install schema for extension '{extension}': {message}")]
    SchemaInstallationFailed { extension: String, message: String },

    #[error("Migration failed for extension '{extension}': {message}")]
    MigrationFailed { extension: String, message: String },

    #[error(
        "Migration {version} for extension '{extension}' is not reversible (no down SQL provided)"
    )]
    MigrationNotReversible { extension: String, version: u32 },

    #[error(
        "Extension '{extension}' migration slot {version} was applied as '{stored_name}' but the \
         tree now names it '{current_name}'. A migration file was deleted and its number reused; \
         established databases have already spent that slot, so the new SQL would be skipped, \
         never executed. Renumber it above every used slot and leave a \
         `{version:03}_{stored_name}.tombstone` behind so the number cannot be claimed again."
    )]
    MigrationSlotReused {
        extension: String,
        version: u32,
        stored_name: String,
        current_name: String,
    },

    #[error("Configuration validation failed for extension '{extension}': {message}")]
    ConfigValidationFailed { extension: String, message: String },

    #[error("Extension '{extension}' uses reserved API path '{path}'")]
    ReservedPathCollision { extension: String, path: String },

    #[error(
        "Extension '{extension}' has invalid base path '{path}': must be / or start with /api/"
    )]
    InvalidBasePath { extension: String, path: String },

    #[error("Dependency cycle detected while ordering extensions: {chain}")]
    DependencyCycle { chain: String },

    #[error(
        "Extension '{extension}' migration ALTERs table '{table}' but does not create it in its \
         schemas() nor declare it in cross_extension_tables(); cross-extension table mutations \
         must be declared explicitly"
    )]
    CrossExtensionAlterUndeclared { extension: String, table: String },

    #[error(
        "Extension '{extension}' migration {migration} references {kind} '{object}' via {how}, \
         which only a declarative schema file creates; the dependent phase runs after \
         migrations, so a database that has not booted on that schema fails here. Create it in \
         a migration, guard the reference in a DO $$ block that tests pg_trigger/pg_views, or \
         leave it to the declarative schema"
    )]
    MigrationReferencesDeclarativeObject {
        extension: String,
        migration: String,
        kind: String,
        object: String,
        how: String,
    },

    #[error(
        "Extension '{extension}' migration {migration} toggles trigger '{trigger}' on '{table}' \
         by name; that statement fails on any database where the trigger has since been retired. \
         The runner already suspends every row trigger on the tables a migration writes, so \
         delete the toggle, or guard it in a DO $$ block that tests pg_trigger first"
    )]
    MigrationTogglesTriggerByName {
        extension: String,
        migration: String,
        table: String,
        trigger: String,
    },

    #[error(
        "Trigger '{trigger}' on '{table}' runs {function}, which uses '{relation}', and \
         '{relation}' no longer exists: every write to '{table}' would fail. Retire the trigger in \
         its extension's `retirements()` (it runs before any migration), or restore the relation"
    )]
    DanglingTriggerRoutine {
        trigger: String,
        table: String,
        function: String,
        relation: String,
    },

    #[error(
        "Extension '{extension}' migration {version} ('{name}') has been edited since it was \
         applied (stored checksum {stored_checksum}, current {current_checksum}). Refusing to \
         proceed. If the database schema already matches the edited file, run `systemprompt \
         infra db migrate-repair --reconcile-only --apply` to rewrite the stored checksum \
         without executing any SQL. To re-execute the edited migration, run `systemprompt infra \
         db migrate-repair --apply`. Passing --allow-checksum-drift bypasses the check without \
         fixing it."
    )]
    MigrationChecksumDrift {
        extension: String,
        version: u32,
        name: String,
        stored_checksum: String,
        current_checksum: String,
    },

    #[error(
        "Table '{table}' is created by both extension '{extension_a}' and '{extension_b}'; every \
         table must be declared by exactly one extension"
    )]
    DuplicateTableOwner {
        table: String,
        extension_a: String,
        extension_b: String,
    },

    #[error(
        "Extension '{extension}' declares cross_extension_tables() entry '{table}', which is not \
         a table created by any other loaded extension"
    )]
    CrossExtensionTableNotOwned { extension: String, table: String },

    #[error(
        "Extension '{extension}' seed '{seed}' contains forbidden statement '{statement}'; seeds \
         may only contain INSERT … ON CONFLICT, UPDATE, MERGE, or WITH … INSERT"
    )]
    InvalidSeedStatement {
        extension: String,
        seed: String,
        statement: String,
    },

    #[error(
        "Extension '{extension}' seed '{seed}' contains a bare INSERT with no ON CONFLICT clause; \
         seeds run on every boot and must be idempotent — add ON CONFLICT … DO NOTHING/UPDATE"
    )]
    SeedInsertNotIdempotent { extension: String, seed: String },

    #[error("Extension '{extension}' seed '{seed}' failed to parse or apply: {message}")]
    SeedFailed {
        extension: String,
        seed: String,
        message: String,
    },
}

#[derive(Debug, Error)]
pub enum ExtensionConfigError {
    #[error("Configuration key '{0}' not found")]
    NotFound(String),

    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue { key: String, message: String },

    #[error("Failed to parse configuration: {message}")]
    ParseError { message: String },

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}
