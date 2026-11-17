//! Refuses, before any database write, a pending migration, or a statement
//! of an install member's declarative schema, that changes a schema object
//! another member owns, or whose effect the parser cannot read. A
//! declarative schema runs on every install, so each of its statements goes
//! through the same judgement as a pending migration's (labelled `schema`);
//! a core member's declarative schema is not judged.
//!
//! A table is owned by its live `schema_table_owners` record, then by the
//! member whose declarative schema or migration creates it, a core member
//! before an install member, a declarative schema before a migration, then
//! composition order; the kernel's ledger tables ([`KERNEL_LEDGERS`]) are
//! owned by the core `database` member. A member whose schema or migration
//! creates a table (including `CREATE TABLE ... AS` and `SELECT ... INTO`)
//! another member owns is refused ([`MigrationOwnershipError::SharedTable`]),
//! so a `CREATE TABLE IF NOT EXISTS` never makes a peer's table its own.
//!
//! Every relation a statement changes is judged, not only its main one: the
//! table of `ALTER TABLE` (including `SET SCHEMA` and `ENABLE ROW LEVEL
//! SECURITY`), `DROP TABLE`, `TRUNCATE`, `RENAME` of a table, column,
//! constraint, trigger, policy or rule, `CREATE RULE`, `CREATE POLICY`,
//! `ALTER POLICY`, `DROP POLICY|RULE ... ON` and `GRANT`/`REVOKE` on a
//! table; the parent of `CREATE TABLE ... INHERITS`, `PARTITION OF` and
//! `ALTER TABLE ... INHERIT | NO INHERIT`; the partition of `ALTER TABLE ...
//! ATTACH | DETACH PARTITION`; and each statement a `CREATE RULE` runs. Each
//! is refused when its table is another member's
//! ([`MigrationOwnershipError::ForeignTable`]). A relation a statement only
//! reads or constrains against (`LIKE ... INCLUDING`, a foreign-key
//! `REFERENCES`, a query's `FROM`) is not changed by it and is not judged.
//!
//! An index, routine, view, materialized view, sequence, type, domain or
//! trigger (keyed by its name and its table) is owned by its live
//! `schema_object_owners` record, then by the first member whose SQL creates
//! it and does not drop it again, in the same order as a table. A record is
//! live unless its owner is a judged member whose SQL no longer creates the
//! object, or it carries install standing, its owner is a judged install
//! member, and a core member's SQL creates the same table or object (see
//! `record_claims`). A record of a held-back or absent owner is always live,
//! so a core member's create of its name is refused (`SharedTable`, or
//! `ForeignObject` for an object, a core declarative schema's creates
//! included). Every member of the composition
//! claims, a held-back member included; a disabled member, absent from the
//! composition, owns what its live records name. An install member never
//! owns what a core-standing claim holds, even under the same id. Every
//! statement that names an object is judged against one resolution of that
//! name across tables, objects, indexes and triggers, so a table spelling
//! (`ALTER TABLE`, `RENAME`, `SET SCHEMA`, `OWNER TO`, `GRANT`) of a
//! view, materialized view, sequence or index is judged against that
//! object's owner. For an install member, a create of an object another
//! member owns is refused, including `CREATE OR REPLACE`, `CREATE ... IF NOT
//! EXISTS`, `CREATE INDEX IF NOT EXISTS` and `CREATE OR REPLACE TRIGGER`,
//! and so are `ALTER` (including `RENAME`, `OWNER TO`, a type's
//! `ADD|DROP|ALTER|RENAME ATTRIBUTE` and a domain's `RENAME CONSTRAINT`),
//! `ALTER INDEX`, `ALTER TRIGGER ... RENAME`, `DROP` (with or without `IF
//! EXISTS`), `DROP INDEX`, `DROP TRIGGER`, `SET SCHEMA` and
//! `GRANT`/`REVOKE` on a routine, sequence, type or domain of it
//! ([`MigrationOwnershipError::ForeignObject`]); naming another member's
//! object in a create never makes it the member's own. A core member's
//! statements of these kinds are logged at `warn` as unjudged. A held-back
//! member's SQL is read for ownership and never judged. Schema-level
//! statements (`DROP SCHEMA`, `ALTER SCHEMA ... RENAME`, `GRANT`/`REVOKE
//! ... ON SCHEMA`, `GRANT ... ON ALL ... IN SCHEMA`, `ALTER TABLE ALL IN
//! TABLESPACE`) are refused for an install member, and for a core member
//! unless they name a non-`public` schema its own SQL creates
//! ([`MigrationOwnershipError::SchemaLevel`]). An
//! `INSERT`/`UPDATE`/`DELETE`/`MERGE`/`COPY` into a kernel ledger by any
//! member other than the core `database` member, including inside a rule's
//! actions, is refused ([`MigrationOwnershipError::LedgerWrite`]), and so is
//! every other statement on a ledger by such a member, a trigger, index,
//! rule or policy included (`ForeignTable`). Other data writes, reads,
//! `COMMENT ON`, `LOCK`, `REFRESH MATERIALIZED VIEW`, `VACUUM`, `REINDEX`
//! and foreign-key `REFERENCES` are not judged.
//!
//! One rule decides a statement kind the reader does not classify (any kind
//! outside the lists above, a `RENAME` or `GRANT` of an object kind not
//! listed, an `ALTER TABLE` of an unknown relation kind): for an install
//! member it is refused ([`MigrationOwnershipError::Unreadable`]) whatever
//! it names; a core member's is logged at `warn` as unjudged. Also refused
//! for an install member as `Unreadable`: `CALL`, `EXECUTE`, `COPY` from a
//! file or program, and a `sql` routine whose body does not parse (a
//! parseable body is read as part of the migration). At either standing a
//! `DO` block, `CREATE EXTENSION`, a routine in a language other than
//! `sql`, a routine call in a top-level `SELECT` or inside an
//! `INSERT`/`UPDATE`/`DELETE`/`MERGE`, `CREATE INDEX` on another member's
//! table, a `CREATE TRIGGER` of a new trigger or a `DROP TRIGGER` of the
//! member's own trigger on another member's table (a kernel ledger
//! excepted), and a `DROP INDEX` of an index no member's SQL creates are
//! logged as unjudged, once per kind and migration for hidden SQL; so is
//! every `DROP ... CASCADE` of an install member, and a core member's of an
//! object it does not own. The rule guards against mistakes, not a hostile
//! member: a `DO` block or a routine in another language can run any
//! statement it refuses.
//!
//! A table no member owns (never declared by any member, or retired by the
//! member that created it, see `claims`) has no owner to protect: a
//! mutation of it, including `DROP TABLE`, is admitted at either standing.
//!
//! Only migrations that will run are judged: a migration recorded as applied
//! is history, and a fresh database stamps its migrations without running
//! them. An install member's declarative schema is judged on every full
//! install, and one that does not parse is refused
//! ([`MigrationOwnershipError::Parse`]); a settled install runs no DDL and
//! judges nothing. An install member's view, materialized view or rule that
//! reads a kernel ledger is refused (`ForeignTable`): a simple view is
//! auto-updatable, so a write into it writes the ledger.
//! A `cross_extension_tables` declaration admits a table mutation, except an
//! install member's mutation of a core member's table and any mutation of a
//! kernel ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::Migration;
use systemprompt_identifiers::ExtensionId;


mod claims;
mod drops;
mod judge;
mod judge_objects;
mod kinds;
mod ledger;
mod member_reads;
mod members;
mod object_claims;
mod objects;
mod record_claims;
mod records;
mod relations;
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

pub(crate) use record_claims::Stale;

#[must_use]
pub(crate) fn record_plan(
    members: MigrationMembers<'_>,
    recorded: &Recorded<'_>,
) -> (Vec<RecordedObject>, Stale) {
    let ownership = claims::ownership(members, recorded, &mut Vec::new());
    (ownership.owned_objects(members), ownership.stale)
}

fn label(migration: &Migration) -> String {
    format!("{:03}_{}", migration.version, migration.name)
}
