//! Invariant under test: declared `ON DELETE CASCADE` / `ON DELETE SET NULL`
//! foreign keys behave transitively, and audit-shaped tables that do *not*
//! cascade survive the delete of their referenced parent. Soft-delete
//! semantics (`status = 'inactive'` / `'deleted'`) are also pinned: they are
//! state transitions, not row removals, and must not trigger cascades.
//!
//! Each test creates its own ephemeral schema (UUID-suffixed table prefix)
//! so parallel test runs do not collide. The shapes mirror the production
//! domain — `users` is the parent; `user_api_keys`-style children cascade;
//! `user_sessions`-style children set NULL; `governance_decisions`-style
//! audit rows have no FK at all.

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

struct Fixture {
    pool: PgPool,
    suffix: String,
}

impl Fixture {
    async fn new() -> Self {
        let pool = connect_pool().await;
        let suffix = Uuid::new_v4().simple().to_string()[..12].to_string();
        Self { pool, suffix }
    }

    fn t(&self, base: &str) -> String {
        format!("cascade_{}_{}", base, self.suffix)
    }

    async fn exec(&self, sql: &str) {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .unwrap_or_else(|e| panic!("exec failed for {sql}: {e}"));
    }

    async fn drop_all(&self) {
        for base in ["audit", "session", "api_key", "fed_id", "user"] {
            let stmt = format!("DROP TABLE IF EXISTS {} CASCADE", self.t(base));
            let _ = sqlx::query(&stmt).execute(&self.pool).await;
        }
    }

    async fn install_user_schema(&self) {
        let users = self.t("user");
        let api_keys = self.t("api_key");
        let sessions = self.t("session");
        let fed = self.t("fed_id");
        let audit = self.t("audit");

        self.exec(&format!(
            "CREATE TABLE {users} (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'active'
                    CHECK(status IN ('active','inactive','deleted'))
            )"
        ))
        .await;
        self.exec(&format!(
            "CREATE TABLE {api_keys} (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES {users}(id) ON DELETE CASCADE
            )"
        ))
        .await;
        self.exec(&format!(
            "CREATE TABLE {sessions} (
                id TEXT PRIMARY KEY,
                user_id TEXT REFERENCES {users}(id) ON DELETE SET NULL
            )"
        ))
        .await;
        self.exec(&format!(
            "CREATE TABLE {fed} (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES {users}(id) ON DELETE CASCADE,
                provider TEXT NOT NULL,
                UNIQUE (provider, user_id)
            )"
        ))
        .await;
        // Audit-shaped table: no FK to users; user_id is just a recorded
        // attribute. Must survive deletion of the referenced user.
        self.exec(&format!(
            "CREATE TABLE {audit} (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                action TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )"
        ))
        .await;
    }

    async fn count(&self, table: &str, where_clause: &str) -> i64 {
        let q = format!("SELECT COUNT(*)::bigint AS n FROM {table} WHERE {where_clause}");
        let row = sqlx::query(&q).fetch_one(&self.pool).await.expect("count");
        row.try_get::<i64, _>("n").expect("n column")
    }
}

#[tokio::test]
async fn cascade_delete_user_transitively_removes_dependents() {
    let fx = Fixture::new().await;
    fx.install_user_schema().await;

    let users = fx.t("user");
    let api_keys = fx.t("api_key");
    let sessions = fx.t("session");
    let fed = fx.t("fed_id");
    let user_id = format!("u_{}", fx.suffix);

    sqlx::query(&format!("INSERT INTO {users}(id) VALUES ($1)"))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {api_keys}(id, user_id) VALUES ($1, $2), ($3, $2)"
    ))
    .bind(format!("k1_{}", fx.suffix))
    .bind(&user_id)
    .bind(format!("k2_{}", fx.suffix))
    .execute(&fx.pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {sessions}(id, user_id) VALUES ($1, $2)"
    ))
    .bind(format!("s1_{}", fx.suffix))
    .bind(&user_id)
    .execute(&fx.pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {fed}(id, user_id, provider) VALUES ($1, $2, 'github')"
    ))
    .bind(format!("f1_{}", fx.suffix))
    .bind(&user_id)
    .execute(&fx.pool)
    .await
    .unwrap();

    sqlx::query(&format!("DELETE FROM {users} WHERE id = $1"))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();

    assert_eq!(
        fx.count(&api_keys, &format!("user_id = '{user_id}'")).await,
        0,
        "CASCADE child api_keys must be removed when its user is deleted"
    );
    assert_eq!(
        fx.count(&fed, &format!("user_id = '{user_id}'")).await,
        0,
        "CASCADE child federated identities must be removed transitively"
    );
    assert_eq!(
        fx.count(&sessions, "user_id IS NOT NULL").await,
        0,
        "SET NULL must clear the FK column on dependent sessions, not delete them"
    );
    assert_eq!(
        fx.count(&sessions, &format!("id = 's1_{}'", fx.suffix))
            .await,
        1,
        "SET NULL dependent row itself must survive the parent delete"
    );

    fx.drop_all().await;
}

#[tokio::test]
async fn soft_delete_inactive_does_not_cascade() {
    let fx = Fixture::new().await;
    fx.install_user_schema().await;

    let users = fx.t("user");
    let api_keys = fx.t("api_key");
    let sessions = fx.t("session");
    let user_id = format!("u_{}", fx.suffix);

    sqlx::query(&format!("INSERT INTO {users}(id) VALUES ($1)"))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {api_keys}(id, user_id) VALUES ($1, $2)"
    ))
    .bind(format!("k1_{}", fx.suffix))
    .bind(&user_id)
    .execute(&fx.pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {sessions}(id, user_id) VALUES ($1, $2)"
    ))
    .bind(format!("s1_{}", fx.suffix))
    .bind(&user_id)
    .execute(&fx.pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "UPDATE {users} SET status = 'inactive' WHERE id = $1"
    ))
    .bind(&user_id)
    .execute(&fx.pool)
    .await
    .unwrap();

    assert_eq!(
        fx.count(&users, &format!("id = '{user_id}' AND status = 'inactive'"))
            .await,
        1,
        "the user row must persist after a soft delete"
    );
    assert_eq!(
        fx.count(&api_keys, &format!("user_id = '{user_id}'")).await,
        1,
        "soft delete must not cascade to api_keys — only hard DELETE does"
    );
    assert_eq!(
        fx.count(&sessions, &format!("user_id = '{user_id}'")).await,
        1,
        "soft delete must not null out the session FK either"
    );

    fx.drop_all().await;
}

#[tokio::test]
async fn audit_trail_survives_user_cascade_delete() {
    let fx = Fixture::new().await;
    fx.install_user_schema().await;

    let users = fx.t("user");
    let audit = fx.t("audit");
    let user_id = format!("u_{}", fx.suffix);

    sqlx::query(&format!("INSERT INTO {users}(id) VALUES ($1)"))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();
    for n in 0..3 {
        sqlx::query(&format!(
            "INSERT INTO {audit}(id, user_id, action) VALUES ($1, $2, $3)"
        ))
        .bind(format!("a{n}_{}", fx.suffix))
        .bind(&user_id)
        .bind("authz.deny")
        .execute(&fx.pool)
        .await
        .unwrap();
    }

    sqlx::query(&format!("DELETE FROM {users} WHERE id = $1"))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();

    assert_eq!(
        fx.count(&users, &format!("id = '{user_id}'")).await,
        0,
        "the user itself is gone"
    );
    assert_eq!(
        fx.count(&audit, &format!("user_id = '{user_id}'")).await,
        3,
        "audit rows must remain after the referenced user is hard-deleted — compliance requires \
         that historical authz decisions are immutable"
    );

    fx.drop_all().await;
}

#[tokio::test]
async fn no_orphaned_fk_references_after_cascade() {
    let fx = Fixture::new().await;
    fx.install_user_schema().await;

    let users = fx.t("user");
    let api_keys = fx.t("api_key");
    let sessions = fx.t("session");
    let fed = fx.t("fed_id");

    // Two users, each with children.
    for u in ["alpha", "beta"] {
        let user_id = format!("u_{u}_{}", fx.suffix);
        sqlx::query(&format!("INSERT INTO {users}(id) VALUES ($1)"))
            .bind(&user_id)
            .execute(&fx.pool)
            .await
            .unwrap();
        sqlx::query(&format!(
            "INSERT INTO {api_keys}(id, user_id) VALUES ($1, $2)"
        ))
        .bind(format!("k_{u}_{}", fx.suffix))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();
        sqlx::query(&format!(
            "INSERT INTO {sessions}(id, user_id) VALUES ($1, $2)"
        ))
        .bind(format!("s_{u}_{}", fx.suffix))
        .bind(&user_id)
        .execute(&fx.pool)
        .await
        .unwrap();
        sqlx::query(&format!(
            "INSERT INTO {fed}(id, user_id, provider) VALUES ($1, $2, $3)"
        ))
        .bind(format!("f_{u}_{}", fx.suffix))
        .bind(&user_id)
        .bind(u)
        .execute(&fx.pool)
        .await
        .unwrap();
    }

    sqlx::query(&format!("DELETE FROM {users} WHERE id = $1"))
        .bind(format!("u_alpha_{}", fx.suffix))
        .execute(&fx.pool)
        .await
        .unwrap();

    let orphan_keys = fx
        .count(
            &api_keys,
            &format!("user_id NOT IN (SELECT id FROM {users})"),
        )
        .await;
    let orphan_fed = fx
        .count(&fed, &format!("user_id NOT IN (SELECT id FROM {users})"))
        .await;
    let orphan_sessions = fx
        .count(
            &sessions,
            &format!("user_id IS NOT NULL AND user_id NOT IN (SELECT id FROM {users})"),
        )
        .await;

    assert_eq!(orphan_keys, 0, "no dangling api_keys");
    assert_eq!(orphan_fed, 0, "no dangling federated identities");
    assert_eq!(
        orphan_sessions, 0,
        "no dangling sessions (SET NULL nulled the deleted parent's FKs)"
    );

    fx.drop_all().await;
}
