//! A data write into a kernel ledger by any member other than `database`
//! is refused, except a `DELETE` of the `extension_migrations` rows of
//! members no longer in the composition (K-SEC5-4). Every other statement on
//! a ledger, a trigger, index, rule or policy included, is refused at either
//! standing, in a migration or an install's declarative schema, and so is a
//! rule whose action writes a ledger; an install member registered as
//! `database` holds none of that member's rights (K-SEC8-1/5/6).

use systemprompt_database::lifecycle::{MigrationOwnershipError, TableMutation};
use systemprompt_extension::{Migration, StandingTag};

use super::migration_ownership::{PEER_SCHEMA, judged, owner, peer, unit};

const LEDGER_DDL: &[(&str, TableMutation, &str)] = &[
    (
        "CREATE TRIGGER forge BEFORE INSERT ON schema_object_owners FOR EACH ROW EXECUTE FUNCTION forge();",
        TableMutation::CreateTrigger,
        "public.schema_object_owners",
    ),
    (
        "CREATE INDEX forge_idx ON schema_table_owners (extension_id);",
        TableMutation::CreateIndex,
        "public.schema_table_owners",
    ),
    (
        "CREATE RULE forge AS ON INSERT TO schema_object_owners DO INSTEAD NOTHING;",
        TableMutation::CreateRule,
        "public.schema_object_owners",
    ),
    (
        "CREATE POLICY forge ON schema_table_owners USING (true);",
        TableMutation::CreatePolicy,
        "public.schema_table_owners",
    ),
    (
        "ALTER TABLE schema_object_owners ENABLE ROW LEVEL SECURITY;",
        TableMutation::Alter,
        "public.schema_object_owners",
    ),
    (
        "DROP TRIGGER IF EXISTS forge ON extension_migrations;",
        TableMutation::DropTrigger,
        "public.extension_migrations",
    ),
];

fn ledger_refusal(
    outcome: &Result<(), MigrationOwnershipError>,
) -> Option<(TableMutation, String)> {
    match outcome {
        Err(
            MigrationOwnershipError::ForeignTable {
                statement, table, ..
            }
            | MigrationOwnershipError::LedgerWrite {
                statement, table, ..
            },
        ) => Some((*statement, table.clone())),
        _ => None,
    }
}

fn schema_peer(sql: &str) -> std::sync::Arc<dyn systemprompt_database::SchemaMember> {
    let schema: &'static str = Box::leak(format!("{PEER_SCHEMA}\n{sql}").into_boxed_str());
    unit(
        "mo-peer",
        StandingTag::Install,
        Some(schema),
        vec![],
        vec![],
    )
}

#[test]
fn ddl_on_a_kernel_ledger_is_refused_in_a_migration_and_a_schema() {
    for (sql, kind, ledger) in LEDGER_DDL {
        let expected = Some((*kind, (*ledger).to_owned()));
        for standing in [StandingTag::Install, StandingTag::Core] {
            let outcome = judged(&[owner(), peer(standing, sql)]);
            assert_eq!(ledger_refusal(&outcome), expected, "{sql}: {outcome:?}");
        }
        let outcome = judged(&[owner(), schema_peer(sql)]);
        assert_eq!(
            ledger_refusal(&outcome),
            expected,
            "schema {sql}: {outcome:?}"
        );
        let database = unit(
            "database",
            StandingTag::Core,
            None,
            vec![Migration::new(1, "own", sql)],
            vec![],
        );
        judged(&[owner(), database])
            .unwrap_or_else(|e| panic!("negative control, the ledger owner runs {sql}: {e}"));
    }
}

#[test]
fn a_rule_whose_action_writes_a_ledger_is_refused() {
    let sql = "CREATE RULE forge AS ON INSERT TO mo_peer DO ALSO \
               DELETE FROM schema_object_owners;";
    let expected = Some((
        TableMutation::Delete,
        "public.schema_object_owners".to_owned(),
    ));
    assert_eq!(
        ledger_refusal(&judged(&[owner(), peer(StandingTag::Install, sql)])),
        expected
    );
    assert_eq!(
        ledger_refusal(&judged(&[owner(), schema_peer(sql)])),
        expected
    );
    judged(&[
        owner(),
        peer(
            StandingTag::Install,
            "CREATE RULE own AS ON INSERT TO mo_peer DO ALSO DELETE FROM mo_peer;",
        ),
    ])
    .expect("negative control: a rule writing the member's own table is admitted");
}

#[test]
fn an_install_registered_as_the_ledger_owner_holds_none_of_its_rights() {
    for sql in [
        "INSERT INTO schema_object_owners (object_kind, object_name, extension_id, standing) \
         VALUES ('object', 'public.x', 'x', 'core');",
        "CREATE INDEX forge_idx ON schema_table_owners (extension_id);",
    ] {
        let impostor = unit(
            "database",
            StandingTag::Install,
            None,
            vec![Migration::new(1, "forge", sql)],
            vec![],
        );
        assert!(
            ledger_refusal(&judged(&[owner(), impostor])).is_some(),
            "{sql}"
        );
    }
}

#[test]
fn a_write_into_a_kernel_ledger_is_refused() {
    let cases = [
        (
            "INSERT INTO extension_migrations (extension_id, version, name, checksum) VALUES ('mo-owner', 999, 'x', 'x');",
            TableMutation::Insert,
            "public.extension_migrations",
        ),
        (
            "UPDATE extension_migrations SET version = 999 WHERE extension_id = 'mo-owner';",
            TableMutation::Update,
            "public.extension_migrations",
        ),
        (
            "DELETE FROM extension_migrations WHERE extension_id = 'mo-owner';",
            TableMutation::Delete,
            "public.extension_migrations",
        ),
        (
            "DELETE FROM extension_migrations;",
            TableMutation::Delete,
            "public.extension_migrations",
        ),
        (
            "WITH gone AS (DELETE FROM schema_table_owners RETURNING 1) SELECT 1 FROM gone;",
            TableMutation::Delete,
            "public.schema_table_owners",
        ),
        (
            "UPDATE schema_purge_records SET extension_id = 'x';",
            TableMutation::Update,
            "public.schema_purge_records",
        ),
        (
            "COPY schema_install_fingerprint FROM STDIN;",
            TableMutation::Copy,
            "public.schema_install_fingerprint",
        ),
        (
            "INSERT INTO public.schema_trigger_digests SELECT * FROM schema_trigger_digests;",
            TableMutation::Insert,
            "public.schema_trigger_digests",
        ),
    ];
    for standing in [StandingTag::Install, StandingTag::Core] {
        for (sql, kind, ledger) in cases {
            let outcome = judged(&[owner(), peer(standing, sql)]);
            assert!(
                matches!(
                    &outcome,
                    Err(MigrationOwnershipError::LedgerWrite { statement, table, .. })
                        if *statement == kind && table == ledger
                ),
                "{sql}: {outcome:?}"
            );
        }
    }
}

#[test]
fn removing_a_departed_members_rows_and_the_ledger_owner_are_admitted() {
    for sql in [
        "DELETE FROM extension_migrations WHERE extension_id = 'mo-removed';",
        "DELETE FROM extension_migrations WHERE extension_id IN ('mo-removed', 'mo-gone');",
        "INSERT INTO mo_peer (id) VALUES (1);",
    ] {
        judged(&[owner(), peer(StandingTag::Install, sql)])
            .unwrap_or_else(|e| panic!("negative control '{sql}' is admitted: {e}"));
    }
    let database = unit(
        "database",
        StandingTag::Core,
        None,
        vec![Migration::new(
            1,
            "own",
            "DELETE FROM extension_migrations WHERE version > 900;",
        )],
        vec![],
    );
    judged(&[owner(), database]).expect("the database member writes its own ledger");
}
