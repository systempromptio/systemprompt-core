//! A table is the member's own only when no record, kernel ledger or other
//! member's schema claims it first: a pending migration's `CREATE TABLE` of
//! a name another member owns is refused, and a mutation is skipped only for
//! the owner itself (K-SEC5-1).

use systemprompt_database::lifecycle::{
    MigrationOwnershipError, RecordedTable, TableMutation, check_migration_ownership,
    migration_ownership_findings,
};
use systemprompt_extension::{Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::DisposableDb;

use super::migration_ownership::{
    PEER_SCHEMA, all_pending, install, installed_peer, judged, ledger, only, owner, peer,
    present, unit,
};

const EXPLOIT: &str = "CREATE TABLE IF NOT EXISTS mo_owned (id TEXT);\nDROP TABLE mo_owned CASCADE;";

#[test]
fn redeclaring_a_peer_table_then_dropping_it_is_refused() {
    for standing in [StandingTag::Install, StandingTag::Core] {
        let members = [owner(), peer(standing, EXPLOIT)];
        let findings: Vec<_> = migration_ownership_findings(only(&members), &[], &all_pending)
            .into_iter()
            .map(|finding| match finding {
                MigrationOwnershipError::SharedTable { table, owner, .. } => {
                    (String::from("shared"), table, owner)
                },
                MigrationOwnershipError::ForeignTable {
                    table,
                    owner,
                    statement: TableMutation::Drop,
                    ..
                } => (String::from("drop"), table, owner),
                other => panic!("unexpected finding {other}"),
            })
            .collect();
        let owner_id = ExtensionId::new("mo-owner");
        assert_eq!(
            findings,
            vec![
                ("shared".to_owned(), "public.mo_owned".to_owned(), owner_id.clone()),
                ("drop".to_owned(), "public.mo_owned".to_owned(), owner_id),
            ]
        );
    }
    judged(&[
        owner(),
        peer(
            StandingTag::Install,
            "CREATE TABLE IF NOT EXISTS mo_peer_late (id TEXT);\nDROP TABLE mo_peer_late CASCADE;",
        ),
    ])
    .expect("negative control: the same statements over the member's own new table");
}

#[test]
fn a_member_creating_and_later_altering_its_own_table_is_admitted() {
    let member = unit(
        "mo-peer",
        StandingTag::Install,
        None,
        vec![
            Migration::new(1, "create", "CREATE TABLE mo_chain (id BIGINT PRIMARY KEY);"),
            Migration::new(2, "alter", "ALTER TABLE mo_chain ADD COLUMN note TEXT;"),
            Migration::new(3, "drop", "DROP TABLE mo_chain;"),
        ],
        vec![],
    );
    judged(&[owner(), member]).expect("a member's own migration chain is its own");
}

#[test]
fn two_members_whose_migrations_create_one_name_are_refused() {
    let first = unit(
        "mo-first",
        StandingTag::Install,
        None,
        vec![Migration::new(1, "create", "CREATE TABLE IF NOT EXISTS mo_twice (id TEXT);")],
        vec![],
    );
    let second = unit(
        "mo-second",
        StandingTag::Install,
        None,
        vec![Migration::new(1, "create", "CREATE TABLE IF NOT EXISTS mo_twice (id TEXT);")],
        vec![],
    );
    let outcome = judged(&[first.clone(), second]);
    assert!(
        matches!(
            &outcome,
            Err(MigrationOwnershipError::SharedTable { member, table, owner, .. })
                if member.as_str() == "mo-second"
                    && owner.as_str() == "mo-first"
                    && table == "public.mo_twice"
        ),
        "{outcome:?}"
    );
    judged(&[first]).expect("negative control: one member creating the name");
}

#[test]
fn a_pending_create_of_a_recorded_table_is_refused() {
    let recorded = [RecordedTable {
        table: "public.mo_disabled".to_owned(),
        owner: ExtensionId::new("mo-disabled"),
        standing: StandingTag::Core,
    }];
    let members = [peer(
        StandingTag::Install,
        "CREATE TABLE IF NOT EXISTS mo_disabled (id TEXT);",
    )];
    assert!(matches!(
        check_migration_ownership(only(&members), &recorded, &all_pending),
        Err(MigrationOwnershipError::SharedTable { owner, .. }) if owner.as_str() == "mo-disabled"
    ));
    check_migration_ownership(only(&members), &[], &all_pending)
        .expect("negative control: no record and no member claims the name");
}

#[tokio::test]
async fn the_redeclare_exploit_is_refused_before_any_ddl() {
    let database = DisposableDb::empty("migration_shared").await;
    let pool = database.test_pool().await;
    let first = vec![Migration::new(1, "first", "SELECT 1;")];
    install(&pool, vec![ledger(), owner(), installed_peer(first.clone(), PEER_SCHEMA)])
        .await
        .expect("the first install stamps the peer's migration");

    let grown = "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);\n\
                 CREATE TABLE IF NOT EXISTS mo_peer_new (id BIGINT PRIMARY KEY);";
    let mut exploit = first.clone();
    exploit.push(Migration::new(2, "take", EXPLOIT));
    let error = install(&pool, vec![ledger(), owner(), installed_peer(exploit, grown)])
        .await
        .expect_err("re-declaring then dropping a peer table is refused");
    let refusal = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<MigrationOwnershipError>());
    assert!(
        matches!(refusal, Some(MigrationOwnershipError::SharedTable { table, .. }) if table == "public.mo_owned"),
        "{error:?}"
    );
    assert!(present(&pool, "mo_owned").await, "the peer table is kept");
    assert!(!present(&pool, "mo_peer_new").await, "nothing of the refused install ran");

    let mut own = first;
    own.push(Migration::new(
        2,
        "own",
        "CREATE TABLE IF NOT EXISTS mo_peer_late (id TEXT);\nDROP TABLE mo_peer_late CASCADE;",
    ));
    install(&pool, vec![ledger(), owner(), installed_peer(own, grown)])
        .await
        .expect("negative control: the same statements over its own table install");
    assert!(present(&pool, "mo_peer_new").await);
    database.drop_now().await;
}
