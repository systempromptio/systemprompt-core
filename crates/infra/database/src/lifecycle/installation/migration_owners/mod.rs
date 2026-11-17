//! Refuses, before any database write, a pending migration that changes a
//! schema object another member owns, or whose effect the parser cannot
//! read.
//!
//! A table is owned by its `schema_table_owners` record, then by the member
//! whose declarative schema or migration creates it, a core member before an
//! install member, a declarative schema before a migration, then composition
//! order; the kernel's ledger tables ([`KERNEL_LEDGERS`]) are owned by the
//! `database` member. A member whose schema or migration creates a table
//! (including `CREATE TABLE ... AS`) another member owns is refused
//! ([`MigrationOwnershipError::SharedTable`]), so a `CREATE TABLE IF NOT
//! EXISTS` never makes a peer's table its own.
//!
//! The judged statements, refused when their table is another member's
//! ([`MigrationOwnershipError::ForeignTable`]): `ALTER TABLE` (including
//! `SET SCHEMA`), `DROP TABLE`, `TRUNCATE`, `RENAME` of a table, column,
//! constraint, trigger, policy or rule, `CREATE RULE`, `CREATE POLICY`, `ALTER
//! POLICY`, `DROP POLICY|RULE ... ON`, and `GRANT`/`REVOKE` on a table.
//!
//! An index, routine, view, materialized view, sequence, type, domain or
//! trigger (keyed by its name and its table) is owned by its
//! `schema_object_owners` record, then by the first member whose SQL creates
//! it and does not drop it again, in the same order as a table. Every member
//! of the composition claims, a held-back member included; a disabled member,
//! absent from the composition, owns what its records name. Every statement
//! that names an object is judged against one resolution of that name across
//! tables, objects, indexes and triggers, so a table spelling (`ALTER TABLE`,
//! `RENAME`, `SET SCHEMA`, `OWNER TO`) of a view, materialized view,
//! sequence or index is judged against that object's owner. For an install
//! member, a create of an object another member owns is refused, in a
//! migration or in its declarative schema, including `CREATE OR REPLACE`,
//! `CREATE ... IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS` and `CREATE OR
//! REPLACE TRIGGER`, and so are `ALTER` (including `RENAME`, `OWNER TO`, a
//! type's `ADD|DROP|ALTER|RENAME ATTRIBUTE` and a domain's `RENAME
//! CONSTRAINT`), `ALTER INDEX`, `ALTER TRIGGER ... RENAME`, `DROP`, `DROP
//! INDEX`, `DROP TRIGGER` and `SET SCHEMA` of it
//! ([`MigrationOwnershipError::ForeignObject`]); naming another member's
//! object in a create never makes it the member's own. A core member's
//! statements of these kinds are logged at `warn` as unjudged. A held-back
//! member's SQL is read for ownership and never judged. Schema-level
//! statements
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
//! ([`MigrationOwnershipError::Unreadable`]) when it holds SQL the parser
//! cannot attribute and no install ships: `CALL`, `EXECUTE`, `COPY` from a
//! file or program, a `sql` routine whose body does not parse (a parseable
//! body is read as part of the migration), and any statement kind the reader
//! does not classify. A core member's statements of these kinds are logged at
//! `warn` as unjudged. At either standing a `DO` block, `CREATE EXTENSION`,
//! a routine in a language other than `sql`, a routine call in a top-level
//! `SELECT` or inside an `INSERT`/`UPDATE`/`DELETE`/`MERGE`, `CREATE INDEX`
//! on another member's table, a `CREATE TRIGGER` of a new trigger or a
//! `DROP TRIGGER` of the member's own trigger on another member's table, and
//! a `DROP INDEX` of an index no member's SQL creates are logged as unjudged,
//! once per kind and migration for hidden SQL; so is every `DROP ... CASCADE`
//! of an install member, and a core member's of an object it does not own.
//! The rule guards against mistakes, not a hostile member: a `DO` block or a
//! routine in another language can run any statement it refuses.
//!
//! A table no member owns (never declared by any member, or retired by the
//! member that created it, see `claims`) has no owner to protect: a
//! mutation of it, including `DROP TABLE`, is admitted at either standing.
//!
//! Only migrations that will run are judged: a migration recorded as applied
//! is history, and a fresh database stamps its migrations without running
//! them. A `cross_extension_tables` declaration admits a table mutation,
//! except an install member's mutation of a core member's table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::Migration;
use systemprompt_identifiers::ExtensionId;


mod claims;
mod drops;
mod judge;
mod kinds;
mod ledger;
mod member_reads;
mod members;
mod object_claims;
mod objects;
mod records;
mod routines;
mod statements;
mod writes;

pub use kinds::{HiddenSql, TableMutation};
pub(super) use member_reads::retired_tables;
pub use members::MigrationMembers;
pub use records::{ObjectKind, Recorded, RecordedObject, RecordedTable};

pub const KERNEL_LEDGERS: [&str; 7] = [
    "public.extension_migrations",
    "public.extension_migration_compat",
    "public.schema_table_owners",
    "public.schema_object_owners",
    "public.schema_purge_records",
    "public.schema_install_fingerprint",
    "public.schema_trigger_digests",
];

pub const KERNEL_LEDGER_OWNER: &str = "database";

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

    #[error("member {member} migration {migration}: {statement} of schema {schema}")]
    SchemaLevel {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        schema: String,
    },

    #[error("member {member} migration {migration}: {statement} into the kernel ledger {table}")]
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
            | Self::SchemaLevel { member, .. }
            | Self::LedgerWrite { member, .. }
            | Self::Unreadable { member, .. }
            | Self::Parse { member, .. } => member,
        }
    }
}

pub fn check_migration_ownership(
    members: MigrationMembers<'_>,
    recorded: &Recorded<'_>,
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
    recorded: &Recorded<'_>,
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Vec<MigrationOwnershipError> {
    judge::findings(members, recorded, pending)
}

#[must_use]
pub(crate) fn owned_objects(
    members: MigrationMembers<'_>,
    recorded: &Recorded<'_>,
) -> Vec<RecordedObject> {
    claims::ownership(members, recorded, &mut Vec::new()).owned_objects(members)
}

fn label(migration: &Migration) -> String {
    format!("{:03}_{}", migration.version, migration.name)
}
