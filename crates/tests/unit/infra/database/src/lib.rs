//! Unit tests for systemprompt-core-database crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/infra/database/src/error.rs`
//! - Test: `crates/tests/unit/infra/database/src/error.rs`
//!
//! Tests cover:
//! - RepositoryError construction and variants
//! - DatabaseQuery and QueryResult operations
//! - DatabaseInfo, TableInfo, ColumnInfo, IndexInfo structures
//! - QuerySelector trait implementations
//! - EntityId trait implementations
//! - DatabaseCliDisplay formatting
//! - DatabaseExtension
//! - Migration structs (AppliedMigration, MigrationResult, MigrationStatus)

#[cfg(test)]
mod error;

#[cfg(test)]
mod models;

#[cfg(test)]
mod admin;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;

#[cfg(test)]
mod lifecycle;

#[cfg(test)]
mod extension;
