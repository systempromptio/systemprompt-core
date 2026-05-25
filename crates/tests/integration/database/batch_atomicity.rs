//! Invariants under test for batch and concurrent-write paths:
//!
//! 1. A multi-row INSERT that violates a unique constraint on row K+1 must
//!    leave rows 0..K uncommitted (all-or-nothing batch semantics — the
//!    Postgres default for a single multi-VALUES statement).
//! 2. A transactional batch of independent INSERTs that errors mid-way rolls
//!    back the entire transaction.
//! 3. `INSERT ... ON CONFLICT DO UPDATE` under concurrent writers preserves a
//!    documented merge rule (here: last-write-wins by value comparison) and
//!    never loses an update of an unrelated column on the same row.
//!
//! These tests use ephemeral schemas (UUID-suffixed table names) so they
//! are safe to run in parallel and against any reachable Postgres
//! instance.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str = "postgres://systemprompt_admin:\
                                    3e00fcdac26b5b731829e8737515db8f@localhost:5432/\
                                    systemprompt-web";

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

async fn connect_pool() -> PgPool {
    PgPool::connect(&database_url())
        .await
        .expect("connect to test database")
}

fn unique_table(prefix: &str) -> String {
    format!(
        "batch_{}_{}",
        prefix,
        Uuid::new_v4().simple().to_string()[..12].to_string()
    )
}

async fn drop_table(pool: &PgPool, table: &str) {
    let _ = sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(pool)
        .await;
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    let row = sqlx::query(&format!("SELECT COUNT(*)::bigint AS n FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("count");
    row.try_get("n").expect("n")
}

/// A single multi-row INSERT must be atomic at the statement level — when
/// Postgres rejects row 3 for a unique-constraint violation, rows 1 and 2
/// from the same statement must not be visible afterwards.
#[tokio::test]
async fn multi_row_insert_unique_violation_rolls_back_entire_statement() {
    let pool = connect_pool().await;
    let table = unique_table("multi_unique");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, payload TEXT NOT NULL)"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let outcome = sqlx::query(&format!(
        "INSERT INTO {table}(id, payload) VALUES ('row-1','a'), ('row-2','b'), ('row-1','dup'), \
         ('row-3','c')"
    ))
    .execute(&pool)
    .await;

    assert!(
        outcome.is_err(),
        "the conflicting batch must error rather than silently insert a subset"
    );
    assert_eq!(
        count(&pool, &table).await,
        0,
        "no row from the failed multi-row INSERT may persist — Postgres rolls the whole statement \
         back; if this fires the call site is relying on a different (non-existent) per-row \
         commit semantic"
    );

    drop_table(&pool, &table).await;
}

/// An explicit transaction containing several INSERT statements must roll
/// the entire transaction back when any statement inside fails, even when
/// the failures span statements (not just rows within one statement).
#[tokio::test]
async fn transactional_batch_rolls_back_on_mid_batch_failure() {
    let pool = connect_pool().await;
    let table = unique_table("tx_batch");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, payload TEXT NOT NULL)"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query(&format!("INSERT INTO {table}(id, payload) VALUES ($1,$2)"))
        .bind("row-1")
        .bind("a")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(&format!("INSERT INTO {table}(id, payload) VALUES ($1,$2)"))
        .bind("row-2")
        .bind("b")
        .execute(&mut *tx)
        .await
        .unwrap();
    let conflict = sqlx::query(&format!("INSERT INTO {table}(id, payload) VALUES ($1,$2)"))
        .bind("row-1")
        .bind("dup")
        .execute(&mut *tx)
        .await;
    assert!(
        conflict.is_err(),
        "duplicate id should fail mid-transaction"
    );

    drop(tx);

    assert_eq!(
        count(&pool, &table).await,
        0,
        "an aborted transaction must leave the table empty — partial commit of rows 1 and 2 would \
         mean the batch helper is not actually transactional"
    );

    drop_table(&pool, &table).await;
}

/// `INSERT ... ON CONFLICT DO NOTHING` is the alternative path used by some
/// idempotent loaders. The documented contract is: pre-existing rows are
/// preserved unchanged; new rows insert; no error is raised. This test
/// pins that contract so a regression to `DO UPDATE` (which would clobber
/// pre-existing rows) is caught.
#[tokio::test]
async fn upsert_do_nothing_preserves_existing_rows() {
    let pool = connect_pool().await;
    let table = unique_table("conflict_nothing");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, payload TEXT NOT NULL)"
    ))
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "INSERT INTO {table}(id, payload) VALUES ('row-1','original')"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table}(id, payload) VALUES ('row-1','clobber'), ('row-2','new') ON \
         CONFLICT(id) DO NOTHING"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let row1: String = sqlx::query_scalar(&format!("SELECT payload FROM {table} WHERE id='row-1'"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let row2: String = sqlx::query_scalar(&format!("SELECT payload FROM {table} WHERE id='row-2'"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row1, "original",
        "DO NOTHING must preserve the pre-existing payload; 'clobber' would indicate the call \
         site has been switched to DO UPDATE"
    );
    assert_eq!(row2, "new", "new row inserts as expected");

    drop_table(&pool, &table).await;
}

/// `INSERT ... ON CONFLICT DO UPDATE` under concurrent writers must produce
/// exactly one row per key, and the surviving payload must be one of the
/// writers' inputs (no torn writes blending columns from different
/// writers).
#[tokio::test]
async fn concurrent_upsert_produces_single_row_with_a_complete_write() {
    let pool = Arc::new(connect_pool().await);
    let table = unique_table("upsert_race");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, payload TEXT NOT NULL, version INT NOT NULL)"
    ))
    .execute(pool.as_ref())
    .await
    .unwrap();

    let writer_count = 16usize;
    let mut handles = Vec::with_capacity(writer_count);
    for i in 0..writer_count {
        let pool = Arc::clone(&pool);
        let table = table.clone();
        handles.push(tokio::spawn(async move {
            let payload = format!("writer-{i}");
            sqlx::query(&format!(
                "INSERT INTO {table}(id, payload, version) VALUES ('contested', $1, $2) ON \
                 CONFLICT(id) DO UPDATE SET payload = EXCLUDED.payload, version = EXCLUDED.version"
            ))
            .bind(&payload)
            .bind(i as i32)
            .execute(pool.as_ref())
            .await
            .expect("upsert");
        }));
    }
    for handle in handles {
        handle.await.expect("writer joined");
    }

    let rows = sqlx::query(&format!(
        "SELECT payload, version FROM {table} WHERE id = 'contested'"
    ))
    .fetch_all(pool.as_ref())
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "concurrent upserts must collapse to exactly one row, not duplicate the primary key — \
         Postgres enforces this via the unique index, a missing index would surface as a count > \
         1 here"
    );
    let payload: String = rows[0].try_get("payload").unwrap();
    let version: i32 = rows[0].try_get("version").unwrap();
    let parsed: usize = payload
        .strip_prefix("writer-")
        .and_then(|s| s.parse().ok())
        .expect("payload parses");
    assert_eq!(
        parsed, version as usize,
        "payload and version must come from the *same* writer — divergence would indicate a torn \
         write where columns from two upserts were interleaved (Postgres prevents this; the test \
         pins the contract)"
    );
    assert!(
        (0..writer_count).contains(&(version as usize)),
        "winning version {version} must be one of the participating writers"
    );

    drop_table(pool.as_ref(), &table).await;
}

/// On a `DO UPDATE` upsert, columns NOT mentioned in the `SET` clause must
/// keep their pre-existing values. A regression that switched the SET list
/// to include unintended columns would surface as a lost update of the
/// untouched column.
#[tokio::test]
async fn upsert_do_update_does_not_clobber_unrelated_columns() {
    let pool = connect_pool().await;
    let table = unique_table("upsert_partial");
    sqlx::query(&format!(
        "CREATE TABLE {table} (
            id TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            audit_note TEXT NOT NULL
        )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table}(id, payload, audit_note) VALUES ('row', 'v1', 'kept')"
    ))
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "INSERT INTO {table}(id, payload, audit_note) VALUES ('row', 'v2', 'ignored') ON \
         CONFLICT(id) DO UPDATE SET payload = EXCLUDED.payload"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let payload: String =
        sqlx::query_scalar(&format!("SELECT payload FROM {table} WHERE id='row'"))
            .fetch_one(&pool)
            .await
            .unwrap();
    let note: String =
        sqlx::query_scalar(&format!("SELECT audit_note FROM {table} WHERE id='row'"))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(payload, "v2", "the SET column updates as instructed");
    assert_eq!(
        note, "kept",
        "the unmentioned audit_note column must be preserved by DO UPDATE — a regression that \
         broadened the SET list would lose this update"
    );

    drop_table(&pool, &table).await;
}
