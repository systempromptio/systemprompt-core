//! Integration tests for the systemprompt-database lifecycle layer.
//!
//! These tests exercise the boundary between declarative `schema/*.sql`
//! files and imperative `Extension::migrations()` — specifically the
//! install-order invariant: migrations first, schema second. The schema
//! linter forbids `ALTER`/`DROP`/etc. in schemas, which means any column
//! that the schema references must already exist when the schema runs.

#[cfg(test)]
#[path = "../schema_migration_order.rs"]
mod schema_migration_order;

#[cfg(test)]
#[path = "../down_migration.rs"]
mod down_migration;
