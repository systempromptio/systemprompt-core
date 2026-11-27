//! Evidence against PostgreSQL itself for the spellings the migration reader
//! judges: `ALTER TABLE` renames, moves and re-owns a view, materialized
//! view, sequence and index, and `CREATE TABLE ... AS` creates a table, so
//! each is judged as the object it names. An install records the owner of
//! every non-table object in `schema_object_owners`, so a member outside the
//! composition keeps its objects. An install deletes a
//! judged member's record of an object its SQL no longer creates and an
//! install-standing record, table or object, of a judged owner for a name a
//! core member's SQL creates, on the full and the settled path; a core create
//! of an absent member's recorded table is refused and leaves its record,
//! rows and purge record in place.

use std::sync::Arc;

use systemprompt_database::lifecycle::{MigrationOwnershipError, TableMutation};
use systemprompt_database::{
    MigrationConfig, SchemaMember, SchemaName, SchemaUnit, install_schema_members,
};
use systemprompt_extension::{
    ExtensionMetadata, LoaderError, Migration, SchemaDefinition, StandingTag,
};
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

const OWNER_WITHOUT_FN: &str = "CREATE TABLE IF NOT EXISTS ev_owned (id INT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS ev_owned_idx ON ev_owned (id);";

const CORE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS ev_core_t (id INT PRIMARY KEY);
CREATE OR REPLACE FUNCTION ev_core_fn() RETURNS INT LANGUAGE sql AS $$ SELECT 1 $$;";

async fn rows(
    pool: &systemprompt_database::DbPool,
    sql: &'static str,
) -> Vec<(String, String, String)> {
    sqlx::query_as(sql)
        .fetch_all(pool.write_pool().as_ref())
        .await
        .expect("records read")
}

fn core_member() -> Arc<dyn SchemaMember> {
    unit(
        "ev-core",
        StandingTag::Core,
        Some(CORE_SCHEMA),
        vec![],
        vec![],
    )
}

fn owner_now() -> Arc<dyn SchemaMember> {
    unit(
        "ev-owner",
        StandingTag::Install,
        Some(OWNER_WITHOUT_FN),
        vec![],
        vec![],
    )
}

#[tokio::test]
async fn a_judged_owners_install_record_yields_to_a_core_create() {
    let database = DisposableDb::empty("stale_records").await;
    let pool = database.test_pool().await;
    install(&pool, vec![ledger(), owner()])
        .await
        .expect("the first install records the owner's function");
    sqlx::raw_sql(
        "INSERT INTO schema_object_owners (object_kind, object_name, extension_id, standing) \
         VALUES ('object', 'public.ev_core_fn', 'ev-owner', 'install'), \
                ('object', 'public.ev_kept_fn', 'ev-gone', 'install');
         INSERT INTO schema_table_owners (table_name, extension_id, standing) \
         VALUES ('public.ev_core_t', 'ev-owner', 'install');",
    )
    .execute(pool.write_pool().as_ref())
    .await
    .expect("records written under the earlier ranking");
    install(&pool, vec![ledger(), core_member(), owner_now()])
        .await
        .expect("a judged owner's install-standing record does not refuse a core create");
    assert_eq!(
        rows(
            &pool,
            "SELECT object_name, extension_id, standing FROM schema_object_owners \
             WHERE object_name IN ('public.ev_owned_fn', 'public.ev_core_fn', 'public.ev_kept_fn') \
             ORDER BY object_name",
        )
        .await,
        vec![
            ("public.ev_core_fn".into(), "ev-core".into(), "core".into()),
            (
                "public.ev_kept_fn".into(),
                "ev-gone".into(),
                "install".into()
            ),
        ],
        "the dropped function's record is released, the core function is re-recorded, \
         and the absent member's record stays"
    );
    assert_eq!(
        rows(
            &pool,
            "SELECT table_name, extension_id, standing FROM schema_table_owners \
             WHERE table_name = 'public.ev_core_t'",
        )
        .await,
        vec![("public.ev_core_t".into(), "ev-core".into(), "core".into())]
    );
    database.drop_now().await;
}

#[tokio::test]
async fn a_core_create_of_an_absent_members_table_is_refused_and_changes_nothing() {
    let database = DisposableDb::empty("absent_owner_refusal").await;
    let pool = database.test_pool().await;
    install(&pool, vec![ledger(), owner()])
        .await
        .expect("the first install records the owner");
    sqlx::raw_sql(
        "CREATE TABLE ev_core_t (id INT PRIMARY KEY, user_id TEXT);
         INSERT INTO ev_core_t VALUES (1, 'u1');
         INSERT INTO schema_table_owners (table_name, extension_id, standing) \
         VALUES ('public.ev_core_t', 'ev-gone', 'install');
         INSERT INTO schema_purge_records (extension_id, standing, position, declarations) \
         VALUES ('ev-gone', 'install', 9, '[{\"table\": \"ev_core_t\"}]'::jsonb);",
    )
    .execute(pool.write_pool().as_ref())
    .await
    .expect("a disabled member's table, rows and records");
    let before_purge: Vec<(String, String)> = sqlx::query_as(
        "SELECT extension_id, declarations::text FROM schema_purge_records \
         WHERE extension_id = 'ev-gone'",
    )
    .fetch_all(pool.write_pool().as_ref())
    .await
    .expect("purge record read");

    let error = install(&pool, vec![ledger(), core_member(), owner_now()])
        .await
        .expect_err("a core create of a disabled member's recorded table is refused");
    assert!(
        matches!(
            &error,
            LoaderError::DuplicateTableOwner { table, extension_a, extension_b }
                if table.ends_with("ev_core_t")
                    && extension_a.as_str() == "ev-gone"
                    && extension_b.as_str() == "ev-core"
        ),
        "{error:?}"
    );
    assert_eq!(
        rows(
            &pool,
            "SELECT table_name, extension_id, standing FROM schema_table_owners \
             WHERE table_name = 'public.ev_core_t'",
        )
        .await,
        vec![(
            "public.ev_core_t".into(),
            "ev-gone".into(),
            "install".into()
        )],
        "the record stays the absent member's"
    );
    let live: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ev_core_t")
        .fetch_one(pool.write_pool().as_ref())
        .await
        .expect("rows read");
    assert_eq!(live, 1, "the absent member's rows stay");
    let after_purge: Vec<(String, String)> = sqlx::query_as(
        "SELECT extension_id, declarations::text FROM schema_purge_records \
         WHERE extension_id = 'ev-gone'",
    )
    .fetch_all(pool.write_pool().as_ref())
    .await
    .expect("purge record read");
    assert_eq!(
        after_purge, before_purge,
        "the absent member's purge record stays"
    );

    let report = install_schema_members(
        &[ledger(), owner_now()],
        pool.write(),
        &SchemaName::public(),
        MigrationConfig::default(),
    )
    .await
    .expect("control: without the core create the install succeeds");
    assert!(
        report
            .residue
            .absent_member_tables
            .iter()
            .any(|absent| absent.owner.as_str() == "ev-gone" && absent.table == "ev_core_t"),
        "doctor keeps listing the table under the absent member: {:?}",
        report.residue.absent_member_tables
    );
    database.drop_now().await;
}

const FINGERPRINT_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_install_fingerprint.sql");

fn fingerprinted_ledger() -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id: "database",
            name: "database",
            version: "1.0.0",
        },
        standing: StandingTag::Core,
        schemas: vec![
            SchemaDefinition::new("schema_table_owners", OWNERS_DDL),
            SchemaDefinition::new("schema_object_owners", OBJECT_OWNERS_DDL),
            SchemaDefinition::new("schema_install_fingerprint", FINGERPRINT_DDL),
        ],
        migrations: vec![],
        seeds: vec![],
        retirements: vec![],
        cross_tables: vec![],
        purges: vec![],
        reassignments: vec![],
    })
}

const OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_table_owners.sql");
const OBJECT_OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_object_owners.sql");

#[tokio::test]
async fn a_settled_install_runs_no_ddl_and_releases_a_judged_owners_stale_record() {
    let database = DisposableDb::empty("settled_stale_release").await;
    let pool = database.test_pool().await;
    let members = || vec![fingerprinted_ledger(), core_member(), owner_now()];
    install(&pool, members())
        .await
        .expect("the full install records the fingerprint");
    let stamped: String = sqlx::query_scalar(
        "SELECT fingerprint || '@' || recorded_at::text FROM schema_install_fingerprint",
    )
    .fetch_one(pool.write_pool().as_ref())
    .await
    .expect("fingerprint read");
    sqlx::raw_sql(
        "UPDATE schema_object_owners SET extension_id = 'ev-owner', standing = 'install' \
         WHERE object_name = 'public.ev_core_fn';",
    )
    .execute(pool.write_pool().as_ref())
    .await
    .expect("a record written under the earlier ranking");
    install(&pool, members())
        .await
        .expect("the settled install succeeds");
    let restamped: String = sqlx::query_scalar(
        "SELECT fingerprint || '@' || recorded_at::text FROM schema_install_fingerprint",
    )
    .fetch_one(pool.write_pool().as_ref())
    .await
    .expect("fingerprint read");
    assert_eq!(
        restamped, stamped,
        "the second install took the settled path"
    );
    assert_eq!(
        rows(
            &pool,
            "SELECT object_name, extension_id, standing FROM schema_object_owners \
             WHERE object_name = 'public.ev_core_fn'",
        )
        .await,
        vec![("public.ev_core_fn".into(), "ev-core".into(), "core".into())],
        "the judged owner's stale record is released and the core create re-recorded"
    );
    database.drop_now().await;
}
