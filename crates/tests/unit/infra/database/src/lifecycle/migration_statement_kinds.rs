//! Every statement kind the reader attributes to a table, an index, an
//! object or a schema is judged against its owner, and an install member's
//! SQL the parser cannot attribute is refused; each refusal has an own-object
//! control (K-SEC5-2).

use std::sync::Arc;

use systemprompt_database::SchemaMember;
use systemprompt_database::lifecycle::{HiddenSql, MigrationOwnershipError, TableMutation};
use systemprompt_extension::{Migration, StandingTag};

use super::migration_ownership::{judged, unit};

const OWNER: &str = "CREATE TABLE IF NOT EXISTS mo_owned (id BIGINT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS mo_owned_idx ON mo_owned (id);
CREATE TYPE mo_owned_kind AS ENUM ('a');
CREATE SCHEMA IF NOT EXISTS mo_zone;
CREATE TABLE IF NOT EXISTS mo_zone.mo_zoned (id BIGINT);";

const PEER: &str = "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS mo_peer_idx ON mo_peer (id);
CREATE TYPE mo_peer_kind AS ENUM ('a');";

fn owner() -> Arc<dyn SchemaMember> {
    unit("mo-owner", StandingTag::Core, Some(OWNER), vec![], vec![])
}

fn peer(standing: StandingTag, sql: &'static str) -> Arc<dyn SchemaMember> {
    unit(
        "mo-peer",
        standing,
        Some(PEER),
        vec![Migration::new(1, "probe", sql)],
        vec![],
    )
}

fn outcome(standing: StandingTag, sql: &'static str) -> Result<(), MigrationOwnershipError> {
    judged(&[owner(), peer(standing, sql)])
}

fn admitted(standing: StandingTag, sql: &'static str) {
    outcome(standing, sql).unwrap_or_else(|e| panic!("negative control '{sql}' is admitted: {e}"));
}

#[test]
fn a_table_statement_on_a_peer_table_is_refused_and_on_its_own_admitted() {
    let cases = [
        (
            "ALTER TABLE mo_owned SET SCHEMA mo_attic;",
            "ALTER TABLE mo_peer SET SCHEMA mo_attic;",
            TableMutation::SetSchema,
        ),
        (
            "CREATE RULE mo_r AS ON INSERT TO mo_owned DO INSTEAD NOTHING;",
            "CREATE RULE mo_r AS ON INSERT TO mo_peer DO INSTEAD NOTHING;",
            TableMutation::CreateRule,
        ),
        (
            "CREATE POLICY mo_p ON mo_owned USING (true);",
            "CREATE POLICY mo_p ON mo_peer USING (true);",
            TableMutation::CreatePolicy,
        ),
        (
            "ALTER POLICY mo_p ON mo_owned USING (false);",
            "ALTER POLICY mo_p ON mo_peer USING (false);",
            TableMutation::AlterPolicy,
        ),
        (
            "DROP POLICY IF EXISTS mo_p ON mo_owned;",
            "DROP POLICY IF EXISTS mo_p ON mo_peer;",
            TableMutation::DropPolicy,
        ),
        (
            "DROP RULE IF EXISTS mo_r ON mo_owned;",
            "DROP RULE IF EXISTS mo_r ON mo_peer;",
            TableMutation::DropRule,
        ),
        (
            "GRANT SELECT ON mo_owned TO PUBLIC;",
            "GRANT SELECT ON mo_peer TO PUBLIC;",
            TableMutation::Grant,
        ),
        (
            "REVOKE ALL ON mo_owned FROM PUBLIC;",
            "REVOKE ALL ON mo_peer FROM PUBLIC;",
            TableMutation::Revoke,
        ),
        (
            "CREATE FUNCTION mo_f() RETURNS void LANGUAGE sql AS 'DROP TABLE mo_owned';",
            "CREATE FUNCTION mo_f() RETURNS void LANGUAGE sql AS 'TRUNCATE mo_peer';",
            TableMutation::Drop,
        ),
    ];
    for standing in [StandingTag::Install, StandingTag::Core] {
        for (refused, control, kind) in cases {
            assert!(
                matches!(
                    outcome(standing, refused),
                    Err(MigrationOwnershipError::ForeignTable { statement, ref table, .. })
                        if statement == kind && table == "public.mo_owned"
                ),
                "{refused}: {:?}",
                outcome(standing, refused)
            );
            admitted(standing, control);
        }
    }
}

#[test]
fn a_schema_level_statement_over_public_or_a_peer_schema_is_refused() {
    let cases = [
        ("DROP SCHEMA public CASCADE;", TableMutation::DropSchema),
        (
            "ALTER SCHEMA public RENAME TO mo_old;",
            TableMutation::RenameSchema,
        ),
        ("DROP SCHEMA mo_zone CASCADE;", TableMutation::DropSchema),
        (
            "GRANT SELECT ON ALL TABLES IN SCHEMA public TO PUBLIC;",
            TableMutation::GrantInSchema,
        ),
        (
            "ALTER TABLE ALL IN TABLESPACE pg_default SET TABLESPACE mo_space;",
            TableMutation::MoveAll,
        ),
    ];
    for standing in [StandingTag::Install, StandingTag::Core] {
        for (sql, kind) in cases {
            assert!(
                matches!(
                    outcome(standing, sql),
                    Err(MigrationOwnershipError::SchemaLevel { statement, .. }) if statement == kind
                ),
                "{sql}"
            );
        }
        admitted(
            standing,
            "CREATE SCHEMA mo_mine;\nDROP SCHEMA mo_mine CASCADE;",
        );
    }
}

#[test]
fn an_install_dropping_a_peer_index_or_object_is_refused() {
    let cases = [
        (
            "DROP INDEX IF EXISTS mo_owned_idx;",
            TableMutation::DropIndex,
            "public.mo_owned_idx",
        ),
        (
            "ALTER INDEX mo_owned_idx RENAME TO mo_taken_idx;",
            TableMutation::AlterIndex,
            "public.mo_owned_idx",
        ),
        (
            "DROP TYPE IF EXISTS mo_owned_kind;",
            TableMutation::DropObject,
            "public.mo_owned_kind",
        ),
    ];
    for (sql, kind, name) in cases {
        assert!(
            matches!(
                outcome(StandingTag::Install, sql),
                Err(MigrationOwnershipError::ForeignObject { statement, ref object, ref owner, .. })
                    if statement == kind && object == name && owner.as_str() == "mo-owner"
            ),
            "{sql}"
        );
        admitted(StandingTag::Core, sql);
    }
    admitted(StandingTag::Install, "DROP INDEX IF EXISTS mo_peer_idx;");
    admitted(
        StandingTag::Install,
        "DROP TYPE IF EXISTS mo_peer_kind CASCADE;",
    );
}

#[test]
fn an_install_migration_hiding_its_sql_is_refused_and_a_core_one_logged() {
    let cases = [
        ("CALL mo_proc();", HiddenSql::Call),
        (
            "PREPARE mo_q AS SELECT 1;\nEXECUTE mo_q;",
            HiddenSql::Unclassified {
                kind: "PrepareStmt".to_owned(),
            },
        ),
        ("COPY mo_peer FROM PROGRAM 'true';", HiddenSql::CopyProgram),
        (
            "CREATE ROLE mo_role;",
            HiddenSql::Unclassified {
                kind: "CreateRoleStmt".to_owned(),
            },
        ),
        (
            "CREATE FUNCTION mo_f() RETURNS void LANGUAGE sql AS 'not sql at all';",
            HiddenSql::UnparseableBody,
        ),
    ];
    for (sql, hidden) in cases {
        assert!(
            matches!(
                outcome(StandingTag::Install, sql),
                Err(MigrationOwnershipError::Unreadable { ref statement, .. }) if *statement == hidden
            ),
            "{sql}: {:?}",
            outcome(StandingTag::Install, sql)
        );
        admitted(StandingTag::Core, sql);
    }
    admitted(StandingTag::Install, "SELECT 1;");
    admitted(
        StandingTag::Install,
        "COMMENT ON TABLE mo_owned IS 'a note';",
    );
}

#[test]
fn the_kinds_installs_ship_are_logged_not_refused() {
    for sql in [
        "CREATE FUNCTION mo_f() RETURNS void LANGUAGE plpgsql AS $$BEGIN END$$;",
        "SELECT mo_f();",
        "CREATE INDEX IF NOT EXISTS mo_peer_on_owned ON mo_owned (id);",
        "CREATE TRIGGER mo_t AFTER INSERT ON mo_owned FOR EACH ROW EXECUTE FUNCTION mo_f();",
        "DROP TRIGGER IF EXISTS mo_t ON mo_owned;",
        "DROP INDEX IF EXISTS mo_nobody_idx;",
        "DROP TABLE IF EXISTS mo_nobody CASCADE;",
    ] {
        admitted(StandingTag::Install, sql);
    }
}
