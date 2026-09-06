//! `with_scoped_transaction` / `begin_scoped` against the fixture pool.
//!
//! The scoped forms wrap an ordinary transaction with the request's connection
//! scope applied, so the observable contract is the ordinary one: the closure's
//! value is returned on success, its error propagates, and nothing it wrote
//! survives a failure.

use systemprompt_database::{
    Database, RepositoryError, RequestScope, begin_scoped, with_scoped_transaction,
    with_scoped_transaction_raw,
};

use crate::services::db_helper::pool_or_skip;

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn user_exists(pg: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(id)
        .fetch_one(pg)
        .await
        .expect("existence probe")
}

#[tokio::test]
async fn a_scoped_transaction_commits_and_returns_the_closure_value() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_commit");
    let scope = RequestScope::new();

    let returned: String = with_scoped_transaction(&pg, &scope, |tx| {
        let id = id.clone();
        Box::pin(async move {
            sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
                .bind(&id)
                .bind(format!("{id}@scoped.test"))
                .execute(&mut **tx)
                .await?;
            Ok::<_, RepositoryError>(id)
        })
    })
    .await
    .expect("the scoped transaction commits");

    assert_eq!(returned, id, "the closure's value must be handed back");
    assert!(
        user_exists(&pg, &id).await,
        "a committed scoped transaction must persist its write"
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&*pg)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_failing_scoped_closure_rolls_back_its_write() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_rollback");
    let scope = RequestScope::new();

    let result: Result<(), RepositoryError> = with_scoped_transaction(&pg, &scope, |tx| {
        let id = id.clone();
        Box::pin(async move {
            sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
                .bind(&id)
                .bind(format!("{id}@scoped.test"))
                .execute(&mut **tx)
                .await?;
            Err(RepositoryError::Internal("closure gave up".to_owned()))
        })
    })
    .await;

    assert!(result.is_err(), "the closure's error must propagate");
    assert!(
        !user_exists(&pg, &id).await,
        "a scoped transaction whose closure failed must roll back its write"
    );
}

#[tokio::test]
async fn the_raw_form_behaves_identically_to_the_wrapper() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_raw");
    let scope = RequestScope::new();

    let count: i64 = with_scoped_transaction_raw(&pg, &scope, |tx| {
        let id = id.clone();
        Box::pin(async move {
            sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
                .bind(&id)
                .bind(format!("{id}@scoped.test"))
                .execute(&mut **tx)
                .await?;
            let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = $1")
                .bind(&id)
                .fetch_one(&mut **tx)
                .await?;
            Ok::<_, RepositoryError>(n)
        })
    })
    .await
    .expect("raw scoped transaction commits");

    assert_eq!(count, 1, "the closure sees its own uncommitted write");
    assert!(user_exists(&pg, &id).await);

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&*pg)
        .await
        .unwrap();
}

#[tokio::test]
async fn begin_scoped_hands_back_a_transaction_that_can_be_rolled_back() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_begin");
    let scope = RequestScope::new();

    let mut tx = begin_scoped(&pg, &scope)
        .await
        .expect("begin_scoped opens a transaction");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&id)
        .bind(format!("{id}@scoped.test"))
        .execute(&mut *tx)
        .await
        .expect("write inside the scoped transaction");
    tx.rollback().await.expect("rollback");

    assert!(
        !user_exists(&pg, &id).await,
        "an explicitly rolled-back scoped transaction must leave nothing behind"
    );
}

#[tokio::test]
async fn the_database_handle_exposes_the_same_scoped_begin() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_handle");
    let scope = RequestScope::new();

    let mut tx = Database::begin_scoped(&db, &scope)
        .await
        .expect("Database::begin_scoped opens a transaction");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&id)
        .bind(format!("{id}@scoped.test"))
        .execute(&mut *tx)
        .await
        .expect("write");
    tx.commit().await.expect("commit");

    assert!(
        user_exists(&pg, &id).await,
        "the handle's scoped begin must commit like the free function"
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&*pg)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_scope_carrying_settings_still_commits() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pg = db.write_pool_arc().expect("write pool");
    let id = unique("scoped_settings");

    // With no scope provider registered the entries are inert, but the scoped
    // path must still apply cleanly rather than reject a populated scope.
    let mut scope = RequestScope::new();
    scope.insert("organization", "org-under-test");

    let committed: bool = with_scoped_transaction(&pg, &scope, |tx| {
        let id = id.clone();
        Box::pin(async move {
            sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
                .bind(&id)
                .bind(format!("{id}@scoped.test"))
                .execute(&mut **tx)
                .await?;
            Ok::<_, RepositoryError>(true)
        })
    })
    .await
    .expect("a populated scope must not break the transaction");

    assert!(committed);
    assert!(user_exists(&pg, &id).await);

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&*pg)
        .await
        .unwrap();
}
