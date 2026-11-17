//! A data write into a kernel ledger by any member other than `database`
//! is refused, except a `DELETE` of the `extension_migrations` rows of
//! members no longer in the composition (K-SEC5-4).

use systemprompt_database::lifecycle::{MigrationOwnershipError, TableMutation};
use systemprompt_extension::{Migration, StandingTag};

use super::migration_ownership::{judged, owner, peer, unit};

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
