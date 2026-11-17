//! Refuses, before any database write, a pending migration that changes a
//! schema object another member owns, or whose effect the parser cannot
//! read.
//!
//! A table is owned by the member whose declarative schema or migration
//! creates it, or that `schema_table_owners` records for it; the kernel's
//! ledger tables ([`KERNEL_LEDGERS`]) are owned by the `database` member. A
//! member whose schema or migration creates a table another member creates
//! or is recorded for is refused ([`MigrationOwnershipError::SharedTable`]),
//! so a `CREATE TABLE IF NOT EXISTS` never makes a peer's table its own.
//!
//! The judged statements, refused when their table is another member's
//! ([`MigrationOwnershipError::ForeignTable`]): `ALTER TABLE` (including
//! `SET SCHEMA`), `DROP TABLE`, `TRUNCATE`, `RENAME` of a table, column,
//! constraint, trigger, policy or rule, `CREATE INDEX`, `CREATE TRIGGER`,
//! `CREATE RULE`, `CREATE POLICY`, `ALTER POLICY`, `DROP TRIGGER|POLICY|RULE
//! ... ON`, and `GRANT`/`REVOKE` on a table. `DROP INDEX`/`ALTER INDEX` and
//! a `DROP` or `SET SCHEMA` of a type, routine, sequence or view are judged
//! against the member whose SQL creates the object
//! ([`MigrationOwnershipError::ForeignObject`]). Schema-level statements
//! (`DROP SCHEMA`, `ALTER SCHEMA ... RENAME`, `GRANT ... ON ALL ... IN
//! SCHEMA`, `ALTER TABLE ALL IN TABLESPACE`) are refused for an install
//! member, and for a core member unless they name a non-`public` schema its
//! own SQL creates ([`MigrationOwnershipError::SchemaLevel`]). An
//! `INSERT`/`UPDATE`/`DELETE`/`MERGE`/`COPY` into a kernel ledger by any
//! other member is refused ([`MigrationOwnershipError::LedgerWrite`]); other
//! data writes, reads, `COMMENT ON` and foreign-key `REFERENCES` are not
//! judged.
//!
//! For an install member a pending migration is also refused
//! ([`MigrationOwnershipError::Unreadable`], [`MigrationOwnershipError::UnownedObject`])
//! when it holds SQL the parser cannot attribute: `DO`, `CALL`, `EXECUTE`,
//! a top-level `SELECT` that calls a routine, `COPY` from a file or
//! program, a routine in a language other than `sql` (a `sql` routine's body
//! is read as part of the migration), any statement kind the reader does not
//! classify, a `DROP INDEX` of an index its own SQL does not create, and a
//! `DROP ... CASCADE` of an object its own SQL does not create. A core
//! member's statements of these kinds are logged at `warn` as unjudged.
//!
//! Only migrations that will run are judged: a migration recorded as applied
//! is history, and a fresh database stamps its migrations without running
//! them. A `cross_extension_tables` declaration admits a table mutation,
//! except an install member's mutation of a core member's table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;

use crate::lifecycle::schema_member::SchemaMember;

mod claims;
mod drops;
mod judge;
mod kinds;
mod ledger;
mod objects;
mod routines;
mod statements;

pub use kinds::{HiddenSql, TableMutation};

pub const KERNEL_LEDGERS: [&str; 6] = [
    "public.extension_migrations",
    "public.extension_migration_compat",
    "public.schema_table_owners",
    "public.schema_purge_records",
    "public.schema_install_fingerprint",
    "public.schema_trigger_digests",
];

pub const KERNEL_LEDGER_OWNER: &str = "database";

/// One table's owner as `schema_table_owners` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTable {
    pub table: String,
    pub owner: ExtensionId,
    pub standing: StandingTag,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationOwnershipError {
    #[error(
        "member {member} migration {migration}: {statement} of table {table}, owned by {owner}"
    )]
    ForeignTable {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        table: String,
        owner: ExtensionId,
    },

    #[error(
        "member {member} {source_sql} creates table {table}, which member {owner} creates or is \
         recorded for"
    )]
    SharedTable {
        member: ExtensionId,
        source_sql: String,
        table: String,
        owner: ExtensionId,
    },

    #[error(
        "member {member} migration {migration}: {statement} of {object}, created by member {owner}"
    )]
    ForeignObject {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        object: String,
        owner: ExtensionId,
    },

    #[error(
        "install member {member} migration {migration}: {statement} of {object}, which its own \
         SQL does not create"
    )]
    UnownedObject {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        object: String,
    },

    #[error("member {member} migration {migration}: {statement} of schema {schema}")]
    SchemaLevel {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        schema: String,
    },

    #[error(
        "member {member} migration {migration}: {statement} into the kernel ledger {table}"
    )]
    LedgerWrite {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        table: String,
    },

    #[error(
        "install member {member} migration {migration}: a {statement} statement hides what it \
         changes"
    )]
    Unreadable {
        member: ExtensionId,
        migration: String,
        statement: HiddenSql,
    },

    #[error("member {member} migration {migration} could not be parsed")]
    Parse {
        member: ExtensionId,
        migration: String,
        #[source]
        source: pg_query::Error,
    },
}

impl MigrationOwnershipError {
    #[must_use]
    pub const fn member(&self) -> &ExtensionId {
        match self {
            Self::ForeignTable { member, .. }
            | Self::SharedTable { member, .. }
            | Self::ForeignObject { member, .. }
            | Self::UnownedObject { member, .. }
            | Self::SchemaLevel { member, .. }
            | Self::LedgerWrite { member, .. }
            | Self::Unreadable { member, .. }
            | Self::Parse { member, .. } => member,
        }
    }
}

/// The members an install judges and the ones it only reads for ownership.
#[derive(Clone, Copy)]
pub struct MigrationMembers<'a> {
    pub judged: &'a [Arc<dyn SchemaMember>],
    pub held_back: &'a [Arc<dyn SchemaMember>],
}

impl<'a> MigrationMembers<'a> {
    #[must_use]
    pub const fn new(
        judged: &'a [Arc<dyn SchemaMember>],
        held_back: &'a [Arc<dyn SchemaMember>],
    ) -> Self {
        Self { judged, held_back }
    }
}

impl std::fmt::Debug for MigrationMembers<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationMembers")
            .field("judged", &self.judged.len())
            .field("held_back", &self.held_back.len())
            .finish()
    }
}

pub fn check_migration_ownership(
    members: MigrationMembers<'_>,
    recorded: &[RecordedTable],
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Result<(), MigrationOwnershipError> {
    migration_ownership_findings(members, recorded, pending)
        .into_iter()
        .next()
        .map_or(Ok(()), Err)
}

#[must_use]
pub fn migration_ownership_findings(
    members: MigrationMembers<'_>,
    recorded: &[RecordedTable],
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Vec<MigrationOwnershipError> {
    judge::findings(members, recorded, pending)
}

fn label(migration: &Migration) -> String {
    format!("{:03}_{}", migration.version, migration.name)
}
