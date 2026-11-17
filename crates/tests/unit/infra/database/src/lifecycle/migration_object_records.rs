//! Evidence against PostgreSQL itself for the spellings the migration reader
//! judges: `ALTER TABLE` renames, moves and re-owns a view, materialized
//! view, sequence and index, and `CREATE TABLE ... AS` creates a table, so
//! each is judged as the object it names. An install records the owner of
//! every non-table object in `schema_object_owners`, so a member outside the
//! composition keeps its objects (K-SEC7-1, K-SEC7-4).

use std::sync::Arc;

use systemprompt_database::SchemaMember;
use systemprompt_database::lifecycle::{MigrationOwnershipError, TableMutation};
use systemprompt_extension::{Migration, StandingTag};
use systemprompt_test_fixtures::DisposableDb;

use super::migration_ownership::{install, ledger, unit};

const RELATION_FORMS: &[&str] = &[
    "ALTER TABLE ev_v RENAME TO ev_v2",
    "ALTER TABLE ev_mv RENAME TO ev_mv2",
    "ALTER TABLE ev_s RENAME TO ev_s2",
    "ALTER TABLE ev_i RENAME TO ev_i2",
    "ALTER TABLE ev_v2 SET SCHEMA ev_other",
    "ALTER TABLE ev_s2 SET SCHEMA ev_other",
    "ALTER TABLE ev_mv2 SET SCHEMA ev_other",
    "ALTER TABLE ev_other.ev_v2 OWNER TO CURRENT_USER",
    "ALTER TABLE ev_other.ev_s2 OWNER TO CURRENT_USER",
    "ALTER VIEW ev_other.ev_v2 RENAME COLUMN id TO ident",
];

#[tokio::test]
async fn postgres_accepts_the_table_spellings_of_a_view_sequence_matview_and_index() {
    let database = DisposableDb::empty("object_spellings").await;
    let pool = database.test_pool().await;
    let write = pool.write_pool();
    sqlx::raw_sql(
        "CREATE SCHEMA ev_other;
         CREATE TABLE ev_t (id INT PRIMARY KEY);
         CREATE INDEX ev_i ON ev_t (id);
         CREATE VIEW ev_v AS SELECT id FROM ev_t;
         CREATE MATERIALIZED VIEW ev_mv AS SELECT id FROM ev_t;
         CREATE SEQUENCE ev_s;
         CREATE TABLE ev_ctas AS SELECT 1 AS x;",
    )
    .execute(write.as_ref())
    .await
    .expect("fixture objects");
    for statement in RELATION_FORMS {
        sqlx::raw_sql(sqlx::AssertSqlSafe(*statement))
            .execute(write.as_ref())
            .await
            .unwrap_or_else(|e| panic!("PostgreSQL accepts {statement}: {e}"));
    }
    let relkind: String = sqlx::query_scalar(
        "SELECT relkind::text FROM pg_class WHERE oid = 'public.ev_ctas'::regclass",
    )
    .fetch_one(write.as_ref())
    .await
    .expect("catalog read");
    assert_eq!(
        relkind, "r",
        "CREATE TABLE ... AS creates an ordinary table"
    );
    assert_eq!(
        systemprompt_extension::created_table_names("CREATE TABLE ev_ctas AS SELECT 1 AS x;")
            .expect("parses"),
        vec!["ev_ctas".to_owned()],
        "the reader names it as a table"
    );
    database.drop_now().await;
}

const OWNER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS ev_owned (id INT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS ev_owned_idx ON ev_owned (id);
CREATE OR REPLACE FUNCTION ev_owned_fn() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

const PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS ev_peer (id INT PRIMARY KEY);
CREATE OR REPLACE FUNCTION ev_peer_fn() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

fn owner() -> Arc<dyn SchemaMember> {
    unit(
        "ev-owner",
        StandingTag::Install,
        Some(OWNER_SCHEMA),
        vec![],
        vec![],
    )
}

fn peer(migrations: Vec<Migration>) -> Arc<dyn SchemaMember> {
    unit(
        "ev-peer",
        StandingTag::Install,
        Some(PEER_SCHEMA),
        migrations,
        vec![],
    )
}

#[tokio::test]
async fn a_member_outside_the_composition_keeps_its_recorded_objects() {
    let database = DisposableDb::empty("object_records").await;
    let pool = database.test_pool().await;
    let first = vec![Migration::new(1, "first", "SELECT 1;")];
    install(&pool, vec![ledger(), owner(), peer(first.clone())])
        .await
        .expect("the first install records every owner");
    let recorded: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT object_kind, object_name, extension_id FROM schema_object_owners \
         WHERE extension_id = 'ev-owner' ORDER BY object_kind, object_name",
    )
    .fetch_all(pool.write_pool().as_ref())
    .await
    .expect("records read");
    assert_eq!(
        recorded,
        vec![
            (
                "index".into(),
                "public.ev_owned_idx".into(),
                "ev-owner".into()
            ),
            (
                "index".into(),
                "public.ev_owned_pkey".into(),
                "ev-owner".into()
            ),
            (
                "object".into(),
                "public.ev_owned_fn".into(),
                "ev-owner".into()
            ),
        ]
    );

    let mut foreign = first.clone();
    foreign.push(Migration::new(2, "take", "DROP FUNCTION ev_owned_fn();"));
    let error = install(&pool, vec![ledger(), peer(foreign)])
        .await
        .expect_err("the absent owner's function stays its own");
    let refusal = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<MigrationOwnershipError>());
    assert!(
        matches!(
            refusal,
            Some(MigrationOwnershipError::ForeignObject {
                statement: TableMutation::DropObject,
                object,
                owner,
                ..
            }) if object == "public.ev_owned_fn" && owner.as_str() == "ev-owner"
        ),
        "{error:?}"
    );

    let mut own = first;
    own.push(Migration::new(2, "own", "DROP FUNCTION ev_peer_fn();"));
    install(&pool, vec![ledger(), peer(own)])
        .await
        .expect("negative control: a drop of the member's own function installs");
    database.drop_now().await;
}
