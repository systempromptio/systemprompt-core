//! Idempotent post-migration data fixtures (`Seed`s).
//!
//! Seeds are runtime-applied SQL fragments that an extension declares via
//! [`crate::Extension::seeds`]. They run on every boot **after** an
//! extension's schemas and migrations have been applied, are **not** tracked
//! in `extension_migrations`, and must be idempotent by contract: the
//! installation linter rejects anything that is not `INSERT … ON CONFLICT`,
//! `UPDATE`, `MERGE`, or a `WITH … INSERT` CTE (no `CREATE`/`ALTER`/`DROP`).
//!
//! This separates *target schema state* (declarative `schema/*.sql`) and
//! *one-shot state transitions* (versioned migrations) from *reference data
//! that should always exist* — the latter being the seed contract.

#[derive(Debug, Clone, Copy)]
pub struct Seed {
    pub id: &'static str,
    pub sql: &'static str,
}

impl Seed {
    #[must_use]
    pub const fn new(id: &'static str, sql: &'static str) -> Self {
        Self { id, sql }
    }
}
