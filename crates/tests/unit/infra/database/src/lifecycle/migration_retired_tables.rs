//! A table the member that created it names in its retirements has no owner:
//! a peer's `DROP TABLE IF EXISTS` of it is admitted, and the creator's
//! ownership record of it is released at the next install while the table
//! still exists. A retirement never takes a table another member owns, and a
//! table no member declares has no owner at either standing.

use std::sync::Arc;

use systemprompt_database::lifecycle::{
    MigrationOwnershipError, Recorded, RecordedTable, TableMutation, check_migration_ownership,
};
use systemprompt_database::{DbPool, SchemaMember, SchemaUnit};
use systemprompt_extension::{ExtensionMetadata, Migration, SchemaDefinition, StandingTag};
use systemprompt_test_fixtures::DisposableDb;

use super::migration_ownership::{
    OWNER_SCHEMA, PEER_SCHEMA, all_pending, install, ledger, only, owner, peer, present,
};

const RETIRED: &str = "DROP TABLE IF EXISTS mo_retired;";

fn creator(schema: &'static str, retirements: &[&'static str]) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "mo-creator",
            name: "mo-creator",
            version: "1.0.0",
        },
        standing: StandingTag::Core,
        schemas: vec![SchemaDefinition::new("mo-creator", schema)],
        migrations: vec![Migration::new(
            1,
            "create",
            "CREATE TABLE IF NOT EXISTS mo_retired (id BIGINT PRIMARY KEY);",
        )],
        seeds: vec![],
        retirements: retirements
            .iter()
            .map(|sql| SchemaDefinition {
                table: None,
                sql: (*sql).to_owned(),
                required_columns: Vec::new(),
                schema: None,
            })
            .collect(),
        cross_tables: vec![],
        purges: vec![],
    })
}

fn recorded() -> [RecordedTable; 1] {
    [RecordedTable {
        table: "public.mo_retired".to_owned(),
        owner: systemprompt_identifiers::ExtensionId::new("mo-creator"),
        standing: StandingTag::Core,
    }]
}

fn dropped_by_peer(outcome: Result<(), MigrationOwnershipError>) -> Option<String> {
    match outcome {
        Ok(()) => None,
        Err(MigrationOwnershipError::ForeignTable {
            statement: TableMutation::Drop,
            owner,
            ..
        }) => Some(owner.as_str().to_owned()),
        Err(other) => panic!("expected a drop refusal, got {other:?}"),
    }
}

#[test]
fn a_table_its_creator_retires_may_be_dropped_by_an_install() {
    let peer_drop = || peer(StandingTag::Install, "DROP TABLE IF EXISTS mo_retired;");
    let retired = [creator(OWNER_SCHEMA, &[RETIRED]), peer_drop()];
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&retired),
            &Recorded::NONE,
            &all_pending
        )),
        None
    );
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&retired),
            &Recorded::tables(&recorded()),
            &all_pending
        )),
        None,
        "the creator's record of a table it retires is not a claim"
    );

    let kept = [creator(OWNER_SCHEMA, &[]), peer_drop()];
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&kept),
            &Recorded::NONE,
            &all_pending
        ))
        .as_deref(),
        Some("mo-creator"),
        "negative control: without the retirement the creator's migration owns it"
    );
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&kept),
            &Recorded::tables(&recorded()),
            &all_pending
        ))
        .as_deref(),
        Some("mo-creator"),
        "negative control: without the retirement the record owns it"
    );
}

#[test]
fn a_retirement_never_takes_a_table_its_schema_declares_or_another_member_owns() {
    let declares = "CREATE TABLE IF NOT EXISTS mo_retired (id BIGINT PRIMARY KEY);";
    let members = [
        creator(declares, &[RETIRED]),
        peer(StandingTag::Install, "DROP TABLE IF EXISTS mo_retired;"),
    ];
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&members),
            &Recorded::NONE,
            &all_pending
        ))
        .as_deref(),
        Some("mo-creator"),
        "a table the declarative schema still creates stays the member's"
    );

    let stealing = Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "mo-peer",
            name: "mo-peer",
            version: "1.0.0",
        },
        standing: StandingTag::Install,
        schemas: vec![SchemaDefinition::new("mo-peer", PEER_SCHEMA)],
        migrations: vec![Migration::new(1, "probe", "DROP TABLE IF EXISTS mo_owned;")],
        seeds: vec![],
        retirements: vec![SchemaDefinition {
            table: None,
            sql: "DROP TABLE IF EXISTS mo_owned;".to_owned(),
            required_columns: Vec::new(),
            schema: None,
        }],
        cross_tables: vec![],
        purges: vec![],
    }) as Arc<dyn SchemaMember>;
    assert_eq!(
        dropped_by_peer(check_migration_ownership(
            only(&[owner(), stealing]),
            &Recorded::NONE,
            &all_pending
        ))
        .as_deref(),
        Some("mo-owner"),
        "a member's retirement of a peer's table changes nothing"
    );
}

#[test]
fn a_table_no_member_declares_has_no_owner_at_either_standing() {
    for standing in [StandingTag::Install, StandingTag::Core] {
        check_migration_ownership(
            only(&[owner(), peer(standing, "DROP TABLE mo_never_declared;")]),
            &Recorded::NONE,
            &all_pending,
        )
        .unwrap_or_else(|e| panic!("{standing:?}: {e}"));
    }
}

async fn record_of(pool: &DbPool, table: &str) -> Option<String> {
    sqlx::query_scalar("SELECT extension_id FROM schema_table_owners WHERE table_name = $1")
        .bind(table)
        .fetch_optional(pool.write_pool().as_ref())
        .await
        .expect("record read")
}

#[tokio::test]
async fn a_retired_table_keeps_its_rows_and_its_record_is_released() {
    let database = DisposableDb::empty("migration_retired").await;
    let pool = database.test_pool().await;
    let declares = "CREATE TABLE IF NOT EXISTS mo_retired (id BIGINT PRIMARY KEY);\n\
                    CREATE TABLE IF NOT EXISTS mo_kept (id BIGINT PRIMARY KEY);";
    install(&pool, vec![ledger(), creator(declares, &[])])
        .await
        .expect("the first install creates and records the table");
    assert_eq!(
        record_of(&pool, "public.mo_retired").await.as_deref(),
        Some("mo-creator")
    );

    let kept = "CREATE TABLE IF NOT EXISTS mo_kept (id BIGINT PRIMARY KEY);";
    install(&pool, vec![ledger(), creator(kept, &[])])
        .await
        .expect("an undeclared table that still exists installs");
    assert_eq!(
        record_of(&pool, "public.mo_retired").await.as_deref(),
        Some("mo-creator"),
        "negative control: an undeclared table that still exists keeps its record"
    );

    install(&pool, vec![ledger(), creator(kept, &[RETIRED])])
        .await
        .expect("a retired table installs");
    assert!(
        present(&pool, "mo_retired").await,
        "a table retirement is never executed"
    );
    assert_eq!(record_of(&pool, "public.mo_retired").await, None);
    database.drop_now().await;
}
