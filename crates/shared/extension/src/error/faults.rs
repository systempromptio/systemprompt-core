//! Typed causes of a schema installation or migration failure, carried as
//! the source of `LoaderError::SchemaInstallationStepFailed` and
//! `LoaderError::MigrationStepFailed` where no lower-level error exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationFault {
    #[error("migration history changed concurrently")]
    ConcurrentHistoryChange,

    #[error("extension_migrations row has a malformed `{column}` column")]
    MalformedRow { column: &'static str },

    #[error("migration {version} is not declared in Extension::migrations()")]
    Undeclared { version: u32 },

    #[error("migration {version} ('{name}') is a tombstone: the slot is spent and has no SQL")]
    Tombstoned { version: u32, name: String },

    #[error("migration {version} ('{name}') is already tracked as applied")]
    AlreadyApplied { version: u32, name: String },
}

#[derive(Debug, Error)]
pub enum SchemaFault {
    #[error("required column '{column}' not found in table '{table}'")]
    MissingColumn { column: String, table: String },

    #[error(
        "imperative SQL detected in declarative schema; move offending statements to \
         schema/migrations/NNN_<name>.sql and declare them via Extension::migrations():\n{}",
        .offences.join("\n")
    )]
    ImperativeSql { offences: Vec<String> },

    #[error("only DROP … IF EXISTS is allowed, got `{statement}`")]
    RetirementNotDrop { statement: String },

    #[error("only triggers, functions, procedures and views can be retired, got `{statement}`")]
    RetirementObjectType { statement: String },

    #[error("a retirement must say IF EXISTS: `{statement}`")]
    RetirementWithoutIfExists { statement: String },
}
