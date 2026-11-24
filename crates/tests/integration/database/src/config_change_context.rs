//! Invariant under test: a config plane write made under
//! `with_change_context` is stamped with that actor, reason and source by
//! `sp_stamp_change()`; a write without it is recorded as unattributed and
//! keeps its own source; a gateway policy refuses a source outside the
//! vocabulary; the settings die with the transaction; and every
//! committed plane write bumps `config_version` strictly upward, also when two
//! transactions write concurrently.
//!
//! Rows use UUID-suffixed entity ids and are deleted afterwards, so parallel
//! test processes never see each other's rows.

use std::time::Duration;

use sqlx::{Connection, PgConnection, PgPool, Row};
use systemprompt_database::with_change_context;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::test_database_url;
use systemprompt_traits::{ChangeActor, ChangeSource};
use uuid::Uuid;

const BOUND: Duration = Duration::from_secs(10);

fn entity_id() -> String {
    format!("test_ctx_{}", Uuid::new_v4().simple())
}

async fn pool() -> PgPool {
    PgPool::connect(&test_database_url())
        .await
        .expect("connect to test database")
}

async fn insert_entity(conn: &mut PgConnection, id: &str) {
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, source) \
         VALUES ('plugin', $1, 'bundle:own')",
    )
    .bind(id)
    .execute(conn)
    .await
    .expect("insert entity");
}

async fn delete_entity(pool: &PgPool, id: &str) {
    sqlx::query(
        "DELETE FROM access_control_entities WHERE entity_type = 'plugin' AND entity_id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("delete entity");
}

async fn provenance(pool: &PgPool, id: &str) -> (Option<String>, Option<String>, String, bool) {
    let row = sqlx::query(
        "SELECT changed_by, reason, source, changed_at IS NOT NULL AS stamped \
         FROM access_control_entities WHERE entity_type = 'plugin' AND entity_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .expect("read provenance");
    (row.get(0), row.get(1), row.get(2), row.get(3))
}

async fn config_version(conn: &mut PgConnection) -> i64 {
    sqlx::query_scalar("SELECT version FROM config_version WHERE id = 1")
        .fetch_one(conn)
        .await
        .expect("config_version row exists after a plane write")
}

#[tokio::test]
async fn a_write_under_the_change_context_is_stamped_with_it() {
    let pool = pool().await;
    let id = entity_id();
    let user = UserId::new("user_ctx");
    let mut tx = pool.begin().await.expect("begin");
    with_change_context(
        &mut tx,
        &ChangeActor::User(user.clone()),
        Some("open the plugin for sales"),
        &ChangeSource::Console(user),
    )
    .await
    .expect("set change context");
    insert_entity(&mut tx, &id).await;
    tx.commit().await.expect("commit");

    let (by, reason, source, stamped) = provenance(&pool, &id).await;
    assert_eq!(by.as_deref(), Some("user_ctx"));
    assert_eq!(reason.as_deref(), Some("open the plugin for sales"));
    assert_eq!(source, "console:user_ctx");
    assert!(stamped);
    delete_entity(&pool, &id).await;
}

#[tokio::test]
async fn a_write_without_context_is_unattributed_and_keeps_its_source() {
    let pool = pool().await;
    let id = entity_id();
    let mut conn = pool.acquire().await.expect("acquire");
    insert_entity(&mut conn, &id).await;

    let (by, reason, source, stamped) = provenance(&pool, &id).await;
    assert_eq!((by, reason), (None, None));
    assert_eq!(source, "bundle:own");
    assert!(stamped, "changed_at is stamped whatever the context");
    delete_entity(&pool, &id).await;
}

#[tokio::test]
async fn the_context_ends_with_its_transaction() {
    let mut conn = PgConnection::connect(&test_database_url())
        .await
        .expect("connect");
    let mut tx = conn.begin().await.expect("begin");
    with_change_context(&mut tx, &ChangeActor::Cli, None, &ChangeSource::Migration)
        .await
        .expect("set change context");
    let inside: String = sqlx::query_scalar("SELECT current_setting('systemprompt.source', true)")
        .fetch_one(&mut *tx)
        .await
        .expect("read inside");
    assert_eq!(inside, "migration");
    tx.commit().await.expect("commit");

    let after: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(current_setting('systemprompt.source', true), '')")
            .fetch_one(&mut conn)
            .await
            .expect("read after");
    assert_eq!(
        after, None,
        "a pooled connection never carries a stale context"
    );
}

#[tokio::test]
async fn concurrent_plane_writers_get_strictly_increasing_versions() {
    let pool = pool().await;
    let (first_id, second_id) = (entity_id(), entity_id());

    let mut first = pool.begin().await.expect("begin first");
    insert_entity(&mut first, &first_id).await;
    let first_version = config_version(&mut first).await;

    let second_pool = pool.clone();
    let second_entity = second_id.clone();
    let second = tokio::spawn(async move {
        let mut tx = second_pool.begin().await.expect("begin second");
        insert_entity(&mut tx, &second_entity).await;
        let version = config_version(&mut tx).await;
        tx.commit().await.expect("commit second");
        version
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !second.is_finished(),
        "the second writer waits on the version row"
    );
    first.commit().await.expect("commit first");

    let second_version = tokio::time::timeout(BOUND, second)
        .await
        .expect("second writer finishes once the first commits")
        .expect("second writer task");
    assert!(
        second_version > first_version,
        "{second_version} must follow {first_version}"
    );
    delete_entity(&pool, &first_id).await;
    delete_entity(&pool, &second_id).await;
}

#[tokio::test]
async fn gateway_policy_source_outside_the_vocabulary_is_refused() {
    let pool = pool().await;
    let name = entity_id();
    let refused = sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, source) VALUES ($1, $1, '{}', 'yaml')",
    )
    .bind(&name)
    .execute(&pool)
    .await;
    assert!(refused.is_err(), "'yaml' is not a change source");

    let mut tx = pool.begin().await.expect("begin");
    with_change_context(
        &mut tx,
        &ChangeActor::Cli,
        Some("seed"),
        &ChangeSource::Code("gateway/policies.yaml".into()),
    )
    .await
    .expect("set change context");
    sqlx::query("INSERT INTO ai_gateway_policies (id, name, spec) VALUES ($1, $1, '{}')")
        .bind(&name)
        .execute(&mut *tx)
        .await
        .expect("insert under context");
    tx.commit().await.expect("commit");
    let source: String =
        sqlx::query_scalar("SELECT source FROM ai_gateway_policies WHERE name = $1")
            .bind(&name)
            .fetch_one(&pool)
            .await
            .expect("read source");
    assert_eq!(source, "code:gateway/policies.yaml");
    sqlx::query("DELETE FROM ai_gateway_policies WHERE name = $1")
        .bind(&name)
        .execute(&pool)
        .await
        .expect("delete policy");
}
