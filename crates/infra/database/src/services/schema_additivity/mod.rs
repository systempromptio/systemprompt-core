//! Runner-side enforcement of schema additivity.
//!
//! Schema authors declare columns inside `CREATE TABLE IF NOT EXISTS …`, but
//! that block is a no-op once the table exists. Legacy tenants therefore never
//! get columns introduced after the table first shipped, and the very next
//! `CREATE INDEX … ON tbl(new_col)` in the same schema dies on
//! `column "new_col" does not exist`.
//!
//! Rather than relying on author discipline to remember a defensive
//! `ALTER TABLE … ADD COLUMN IF NOT EXISTS` for every post-initial column,
//! the installer computes the diff itself: parse every `CREATE TABLE` in the
//! combined schema SQL ([`parser`]), diff against `information_schema.columns`,
//! and pre-emit `ALTER TABLE` statements ([`diff`]) for any declared column
//! missing from the live table. The original schema then sees a table that
//! already has the columns it expects, and the indexes that follow succeed.
//!
//! The parser is intentionally minimal: it accepts the dialect we ship
//! (well-formed Postgres DDL written by us), not arbitrary SQL. It emits
//! `ALTER TABLE … ADD COLUMN IF NOT EXISTS <name> <type>` only — without
//! `NOT NULL`, `DEFAULT`, `REFERENCES`, or `CHECK` constraints. A backfill that
//! needs more than that goes in a versioned migration file, not the schema.

mod diff;
mod lexer;
mod parser;

pub use diff::compute_additive_alters;
pub use parser::parse_declared_tables;

/// One column declared inside a `CREATE TABLE` block: `(name, type_text)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredColumn {
    pub name: String,
    pub type_text: String,
}

/// One `CREATE TABLE [IF NOT EXISTS] <name> (...)` block parsed out of a
/// schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredTable {
    pub name: String,
    pub columns: Vec<DeclaredColumn>,
}
