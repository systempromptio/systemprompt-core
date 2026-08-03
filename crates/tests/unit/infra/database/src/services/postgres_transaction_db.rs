//! `PostgresTransaction` through the `DatabaseTransaction` trait object the
//! provider hands back, plus the `Database` handle's own transaction/batch
//! entry points.
//!
//! Every method here is reached only via `begin_transaction()`, which returns
//! a boxed trait object — the concrete type is never named by a caller.

use systemprompt_database::{DatabaseProvider, PostgresProvider};

use crate::services::db_helper::pool;

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn provider() -> Option<(PostgresProvider, sqlx::PgPool)> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some((PostgresProvider::from_pool(pg.clone()), (*pg).clone()))
}

async fn user_exists(pg: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(id)
        .fetch_one(pg)
        .await
        .expect("existence probe")
}

#[tokio::test]
async fn a_committed_transaction_persists_every_statement_it_ran() {
    let Some((provider, pg)) = provider().await else {
        return;
    };
    let id = unique("pgtx_commit");

    let mut tx = provider
        .begin_transaction()
        .await
        .expect("the provider opens a transaction");
    let affected = tx
        .execute(
            &"INSERT INTO users (id, name, email) VALUES ($1, $1, $2)",
            &[&id, &format!("{id}@pgtx.test")],
        )
        .await
        .expect("insert inside the transaction");
    assert_eq!(affected, 1, "the insert must report one affected row");

    tx.commit().await.expect("commit");

    assert!(
        user_exists(&pg, &id).await,
        "a committed transaction must persist its write"
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&pg)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_rolled_back_transaction_persists_nothing() {
    let Some((provider, pg)) = provider().await else {
        return;
    };
    let id = unique("pgtx_rollback");

    let mut tx = provider.begin_transaction().await.expect("begin");
    tx.execute(
        &"INSERT INTO users (id, name, email) VALUES ($1, $1, $2)",
        &[&id, &format!("{id}@pgtx.test")],
    )
    .await
    .expect("insert");
    tx.rollback().await.expect("rollback");

    assert!(
        !user_exists(&pg, &id).await,
        "a rolled-back transaction must leave nothing behind"
    );
}

#[tokio::test]
async fn the_three_fetch_shapes_read_the_transactions_own_uncommitted_write() {
    let Some((provider, pg)) = provider().await else {
        return;
    };
    let id = unique("pgtx_fetch");

    let mut tx = provider.begin_transaction().await.expect("begin");
    tx.execute(
        &"INSERT INTO users (id, name, email) VALUES ($1, $1, $2)",
        &[&id, &format!("{id}@pgtx.test")],
    )
    .await
    .expect("insert");

    let all = tx
        .fetch_all(&"SELECT id FROM users WHERE id = $1", &[&id])
        .await
        .expect("fetch_all");
    assert_eq!(all.len(), 1, "fetch_all must see the uncommitted row");
    assert_eq!(
        all[0].get("id").and_then(serde_json::Value::as_str),
        Some(id.as_str()),
        "the row must decode into JSON with its column names"
    );

    let one = tx
        .fetch_one(&"SELECT id FROM users WHERE id = $1", &[&id])
        .await
        .expect("fetch_one");
    assert_eq!(
        one.get("id").and_then(serde_json::Value::as_str),
        Some(id.as_str())
    );

    let present = tx
        .fetch_optional(&"SELECT id FROM users WHERE id = $1", &[&id])
        .await
        .expect("fetch_optional on a present row");
    assert!(present.is_some());

    let absent = tx
        .fetch_optional(
            &"SELECT id FROM users WHERE id = $1",
            &[&"no-such-user-in-this-tx"],
        )
        .await
        .expect("fetch_optional on an absent row");
    assert!(
        absent.is_none(),
        "a query matching nothing must read as None, not an error"
    );

    tx.rollback().await.expect("rollback");
    assert!(!user_exists(&pg, &id).await);
}

#[tokio::test]
async fn fetch_one_on_an_empty_result_is_an_error_not_an_empty_row() {
    let Some((provider, _pg)) = provider().await else {
        return;
    };

    let mut tx = provider.begin_transaction().await.expect("begin");
    let err = tx
        .fetch_one(
            &"SELECT id FROM users WHERE id = $1",
            &[&"definitely-not-a-user"],
        )
        .await
        .expect_err("fetch_one promises exactly one row");
    assert!(!err.to_string().is_empty());

    tx.rollback().await.expect("rollback");
}

#[tokio::test]
async fn a_transaction_that_is_dropped_without_a_decision_does_not_commit() {
    let Some((provider, pg)) = provider().await else {
        return;
    };
    let id = unique("pgtx_dropped");

    {
        let mut tx = provider.begin_transaction().await.expect("begin");
        tx.execute(
            &"INSERT INTO users (id, name, email) VALUES ($1, $1, $2)",
            &[&id, &format!("{id}@pgtx.test")],
        )
        .await
        .expect("insert");
        // Neither committed nor rolled back: the handle is dropped, which must
        // abort rather than silently persist.
    }

    assert!(
        !user_exists(&pg, &id).await,
        "dropping a transaction handle must abort it, not commit it"
    );
}

#[tokio::test]
async fn the_database_handle_reports_its_pools_and_liveness() {
    let Some(db) = pool().await else {
        return;
    };

    assert!(
        db.pool().is_some(),
        "a postgres-backed handle must expose a read pool"
    );
    assert!(
        db.write_pool().is_some(),
        "a postgres-backed handle must expose a write pool"
    );
    db.pool_arc().expect("read pool arc");
    db.write_pool_arc().expect("write pool arc");

    db.test_connection()
        .await
        .expect("both providers must answer the liveness probe");

    let info = db.get_info().await.expect("database info");
    assert!(
        !info.version.is_empty(),
        "the handle must report the server version"
    );
    assert!(
        info.tables.iter().any(|t| t.name == "extension_migrations"),
        "the info report must list the migrated tables"
    );
}

#[tokio::test]
async fn the_database_handle_runs_a_batch_and_opens_a_plain_transaction() {
    let Some(db) = pool().await else {
        return;
    };
    let table = unique("batch_tbl");
    let pg = db.write_pool_arc().expect("write pool");

    db.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY);"
    ))
    .await
    .expect("batch DDL");

    let mut tx = db.begin().await.expect("the handle opens a transaction");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO \"{table}\" (id) VALUES ('one')"
    )))
    .execute(&mut *tx)
    .await
    .expect("insert");
    tx.commit().await.expect("commit");

    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\""
    )))
    .fetch_one(&*pg)
    .await
    .expect("count");
    assert_eq!(
        count, 1,
        "the batch-created table must hold the committed row"
    );

    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TABLE IF EXISTS \"{table}\""
    )))
    .execute(&*pg)
    .await;
}
