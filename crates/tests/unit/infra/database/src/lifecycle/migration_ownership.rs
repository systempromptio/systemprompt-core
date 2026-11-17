//! A migration that alters, drops, truncates or renames a table another
//! member owns is refused before any database write; a foreign-key
//! reference, a read and a mutation of the member's own table are not; a
//! migration recorded as applied is not judged; an install member's pending
//! `DO` block is refused and a core member's passes (K-SEC4-4, N4).

use std::sync::Arc;

use systemprompt_database::lifecycle::{
    MigrationOwnershipError, RecordedTable, TableMutation, check_migration_ownership,
    migration_ownership_findings,
};
use systemprompt_database::{
    DbPool, MigrationConfig, SchemaMember, SchemaUnit, install_schema_members, schema_members,
};
use systemprompt_extension::{
    ExtensionMetadata, LoaderError, Migration, SchemaDefinition, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{DisposableDb, fixture_admitted};

const OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_table_owners.sql");
const RECORDS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_purge_records.sql");

fn unit(
    id: &'static str,
    standing: StandingTag,
    schema: Option<&'static str>,
    migrations: Vec<Migration>,
    cross_tables: Vec<&'static str>,
) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id,
            name: id,
            version: "1.0.0",
        },
        standing,
        schemas: schema
            .map(|sql| vec![SchemaDefinition::new(id, sql)])
            .unwrap_or_default(),
        migrations,
        seeds: vec![],
        retirements: vec![],
        cross_tables,
        purges: vec![],
    })
}

const OWNER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mo_owned (id BIGINT PRIMARY KEY);";
const PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);";

fn owner() -> Arc<dyn SchemaMember> {
    unit("mo-owner", StandingTag::Core, Some(OWNER_SCHEMA), vec![], vec![])
}

fn peer(standing: StandingTag, sql: &'static str) -> Arc<dyn SchemaMember> {
    peer_with(standing, sql, vec![])
}

fn peer_with(
    standing: StandingTag,
    sql: &'static str,
    cross: Vec<&'static str>,
) -> Arc<dyn SchemaMember> {
    unit(
        "mo-peer",
        standing,
        Some(PEER_SCHEMA),
        vec![Migration::new(1, "probe", sql)],
        cross,
    )
}

fn all_pending(_: &ExtensionId, _: &Migration) -> bool {
    true
}

fn judged(members: &[Arc<dyn SchemaMember>]) -> Result<(), MigrationOwnershipError> {
    check_migration_ownership(members, &[], &all_pending)
}

fn foreign(outcome: Result<(), MigrationOwnershipError>) -> (TableMutation, String, String) {
    match outcome {
        Err(MigrationOwnershipError::ForeignTable {
            statement,
            table,
            owner,
            member,
            ..
        }) => {
            assert_eq!(member.as_str(), "mo-peer");
            (statement, table, owner.as_str().to_owned())
        },
        other => panic!("expected a foreign-table refusal, got {other:?}"),
    }
}

#[test]
fn every_mutation_kind_of_a_peer_table_is_refused() {
    let cases = [
        ("ALTER TABLE mo_owned ADD COLUMN x TEXT;", TableMutation::Alter),
        ("DROP TABLE mo_owned;", TableMutation::Drop),
        ("DROP TABLE IF EXISTS public.mo_owned;", TableMutation::Drop),
        ("TRUNCATE mo_owned;", TableMutation::Truncate),
        ("ALTER TABLE mo_owned RENAME TO mo_taken;", TableMutation::Rename),
        ("ALTER TABLE mo_owned RENAME COLUMN id TO key;", TableMutation::Rename),
    ];
    for standing in [StandingTag::Install, StandingTag::Core] {
        for (sql, kind) in cases {
            let (statement, table, owner_id) = foreign(judged(&[owner(), peer(standing, sql)]));
            assert_eq!(statement, kind, "{sql}");
            assert_eq!(table, "public.mo_owned", "{sql}");
            assert_eq!(owner_id, "mo-owner", "{sql}");
        }
    }
}

#[test]
fn own_tables_references_and_reads_are_not_mutations() {
    for sql in [
        "ALTER TABLE mo_peer ADD COLUMN x TEXT;",
        "TRUNCATE mo_peer;",
        "CREATE TABLE mo_peer_late (id BIGINT PRIMARY KEY);\nALTER TABLE mo_peer_late ADD COLUMN y TEXT;",
        "ALTER TABLE mo_peer ADD COLUMN owned_id BIGINT REFERENCES mo_owned (id);",
        "CREATE INDEX IF NOT EXISTS mo_peer_idx ON mo_peer (id);",
        "INSERT INTO mo_peer (id) SELECT id FROM mo_owned;",
        "DROP TABLE IF EXISTS mo_nobody;",
    ] {
        judged(&[owner(), peer(StandingTag::Install, sql)])
            .unwrap_or_else(|e| panic!("negative control '{sql}' is admitted: {e}"));
    }
}

#[test]
fn a_recorded_table_of_an_absent_member_is_refused() {
    let recorded = [RecordedTable {
        table: "public.mo_disabled".to_owned(),
        owner: ExtensionId::new("mo-disabled"),
        standing: StandingTag::Install,
    }];
    let members = [peer(StandingTag::Install, "DROP TABLE mo_disabled;")];
    let outcome = check_migration_ownership(&members, &recorded, &all_pending);
    let (statement, table, owner_id) = foreign(outcome);
    assert_eq!(
        (statement, table.as_str(), owner_id.as_str()),
        (TableMutation::Drop, "public.mo_disabled", "mo-disabled")
    );
    check_migration_ownership(&members, &[], &all_pending)
        .expect("negative control: an unrecorded table of no member is not judged");
}

#[test]
fn an_applied_migration_is_not_judged() {
    let members = [owner(), peer(StandingTag::Install, "DROP TABLE mo_owned;")];
    check_migration_ownership(&members, &[], &|_, _| false)
        .expect("a migration recorded as applied is history");
    assert!(check_migration_ownership(&members, &[], &all_pending).is_err());
}

#[test]
fn a_do_block_is_refused_for_an_install_and_passes_for_core() {
    let sql = "DO $$ BEGIN EXECUTE 'DROP TABLE mo_owned'; END $$;";
    assert!(
        matches!(
            judged(&[owner(), peer(StandingTag::Install, sql)]),
            Err(MigrationOwnershipError::Unreadable { statement: "DO", .. })
        ),
        "an install migration's DO block is refused"
    );
    judged(&[owner(), peer(StandingTag::Core, sql)])
        .expect("a core migration's DO block passes, logged");
}

#[test]
fn a_cross_table_declaration_admits_all_but_an_install_on_a_core_table() {
    let sql = "ALTER TABLE mo_owned ADD COLUMN x TEXT;";
    judged(&[owner(), peer_with(StandingTag::Core, sql, vec!["mo_owned"])])
        .expect("a core member's declared cross table is admitted");
    let install_owner = unit("mo-owner", StandingTag::Install, Some(OWNER_SCHEMA), vec![], vec![]);
    judged(&[install_owner, peer_with(StandingTag::Install, sql, vec!["mo_owned"])])
        .expect("an install's declared cross table of an install owner is admitted");
    foreign(judged(&[owner(), peer_with(StandingTag::Install, sql, vec!["mo_owned"])]));
}

#[test]
fn no_core_migration_mutates_another_members_table() {
    let members = schema_members(fixture_admitted().built());
    assert!(members.len() > 10, "the core composition has its schema members");
    let findings: Vec<String> = migration_ownership_findings(&members, &[], &all_pending)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(findings.is_empty(), "{findings:#?}");
}

fn ledger() -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "database",
            name: "database",
            version: "1.0.0",
        },
        standing: StandingTag::Core,
        schemas: vec![
            SchemaDefinition::new("schema_table_owners", OWNERS_DDL),
            SchemaDefinition::new("schema_purge_records", RECORDS_DDL),
        ],
        migrations: vec![],
        seeds: vec![],
        retirements: vec![],
        cross_tables: vec![],
        purges: vec![],
    })
}

fn installed_peer(migrations: Vec<Migration>, schema: &'static str) -> Arc<dyn SchemaMember> {
    unit("mo-peer", StandingTag::Install, Some(schema), migrations, vec![])
}

async fn install(pool: &DbPool, members: Vec<Arc<dyn SchemaMember>>) -> Result<(), LoaderError> {
    install_schema_members(&members, pool.write(), MigrationConfig::default())
        .await
        .map(drop)
}

async fn present(pool: &DbPool, table: &str) -> bool {
    sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(pool.write_pool().as_ref())
        .await
        .expect("catalog read")
}

#[tokio::test]
async fn a_pending_foreign_migration_is_refused_before_any_ddl() {
    let database = DisposableDb::empty("migration_owners").await;
    let pool = database.test_pool().await;
    let first = vec![Migration::new(1, "first", "SELECT 1;")];
    install(&pool, vec![ledger(), owner(), installed_peer(first.clone(), PEER_SCHEMA)])
        .await
        .expect("the first install stamps the peer's migration");

    let grown = "CREATE TABLE IF NOT EXISTS mo_peer (id BIGINT PRIMARY KEY);\n\
                 CREATE TABLE IF NOT EXISTS mo_peer_new (id BIGINT PRIMARY KEY);";
    let mut foreign = first.clone();
    foreign.push(Migration::new(2, "take", "DROP TABLE mo_owned;"));
    let error = install(&pool, vec![ledger(), owner(), installed_peer(foreign, grown)])
        .await
        .expect_err("a pending migration dropping a peer table is refused");
    let refusal = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<MigrationOwnershipError>());
    assert!(
        matches!(
            refusal,
            Some(MigrationOwnershipError::ForeignTable {
                statement: TableMutation::Drop,
                ..
            })
        ),
        "{error:?}"
    );
    assert!(present(&pool, "mo_owned").await, "the peer table is kept");
    assert!(
        !present(&pool, "mo_peer_new").await,
        "nothing of the refused install ran"
    );

    let mut own = first;
    own.push(Migration::new(2, "own", "ALTER TABLE mo_peer ADD COLUMN x TEXT;"));
    install(&pool, vec![ledger(), owner(), installed_peer(own, grown)])
        .await
        .expect("negative control: a pending migration of its own table installs");
    assert!(present(&pool, "mo_peer_new").await);
    database.drop_now().await;
}
