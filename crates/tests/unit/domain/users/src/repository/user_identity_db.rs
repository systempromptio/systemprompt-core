//! DB-backed tests for email canonicalisation, anonymous promotion, the
//! transactional user merge, and federated sign-in account linking.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_users::{UserError, UserService};
use uuid::Uuid;

struct Ctx {
    service: UserService,
    pool: systemprompt_database::DbPool,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let service = UserService::new(&pool).expect("service");
    Some(Ctx { service, pool })
}

impl Ctx {
    fn raw(&self) -> std::sync::Arc<sqlx::PgPool> {
        self.pool.pool_arc().expect("raw pool")
    }

    async fn purge(&self, id: &UserId) {
        let raw = self.raw();
        for stmt in [
            "DELETE FROM mcp_tool_executions WHERE user_id = $1",
            "DELETE FROM logs WHERE user_id = $1",
            "DELETE FROM user_contexts WHERE user_id = $1",
            "DELETE FROM federated_identities WHERE user_id = $1",
            "DELETE FROM users WHERE id = $1",
        ] {
            let _ = sqlx::query(stmt)
                .bind(id.as_str())
                .execute(raw.as_ref())
                .await;
        }
    }
}

fn tag() -> String {
    Uuid::new_v4().simple().to_string()
}

#[tokio::test]
async fn create_stores_the_canonical_lowercased_email() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let user = ctx
        .service
        .create(
            &format!("Norm-{t}"),
            &format!("  MiXeD-{t}@Case.Invalid "),
            None,
            None,
        )
        .await
        .expect("create user");

    assert_eq!(user.email, format!("mixed-{t}@case.invalid"));

    ctx.purge(&user.id).await;
}

#[tokio::test]
async fn find_by_email_is_case_insensitive() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let user = ctx
        .service
        .create(
            &format!("Find-{t}"),
            &format!("find-{t}@case.invalid"),
            None,
            None,
        )
        .await
        .expect("create user");

    let found = ctx
        .service
        .find_by_email(&format!("  FiNd-{t}@Case.INVALID  "))
        .await
        .expect("lookup")
        .expect("a mixed-case lookup must resolve to the stored user");
    assert_eq!(found.id, user.id);

    ctx.purge(&user.id).await;
}

#[tokio::test]
async fn merge_users_moves_audit_rows_and_removes_the_source() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let source = ctx
        .service
        .create(
            &format!("src-{t}"),
            &format!("src-{t}@merge.invalid"),
            None,
            None,
        )
        .await
        .expect("create source");
    let target = ctx
        .service
        .create(
            &format!("tgt-{t}"),
            &format!("tgt-{t}@merge.invalid"),
            None,
            None,
        )
        .await
        .expect("create target");

    let raw = ctx.raw();
    let context_id = format!("mergectx-{t}");
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, created_at, \
         updated_at) VALUES ($1, $2, NULL, 'merge fixture', 'conversation', NOW(), NOW())",
    )
    .bind(&context_id)
    .bind(source.id.as_str())
    .execute(raw.as_ref())
    .await
    .expect("seed context");

    let execution_id = format!("mergeexec-{t}");
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         input, status, user_id, context_id) VALUES ($1, 'merge_tool', 'merge_server', NOW(), \
         '{}', 'success', $2, $3)",
    )
    .bind(&execution_id)
    .bind(source.id.as_str())
    .bind(&context_id)
    .execute(raw.as_ref())
    .await
    .expect("seed tool execution");

    let log_id = format!("mergelog-{t}");
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id) VALUES ($1, NOW(), \
         'INFO', 'merge-test', 'merge fixture', $2)",
    )
    .bind(&log_id)
    .bind(source.id.as_str())
    .execute(raw.as_ref())
    .await
    .expect("seed log");

    let result = ctx
        .service
        .merge_users(&source.id, &target.id)
        .await
        .expect("merge");
    assert!(
        result.total_rows >= 3,
        "context + tool execution + log must all be counted, got {}",
        result.total_rows
    );

    for (label, sql, key) in [
        (
            "user_contexts",
            "SELECT user_id FROM user_contexts WHERE context_id = $1",
            &context_id,
        ),
        (
            "mcp_tool_executions",
            "SELECT user_id FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &execution_id,
        ),
        ("logs", "SELECT user_id FROM logs WHERE id = $1", &log_id),
    ] {
        let owner: String = sqlx::query_scalar(sql)
            .bind(key)
            .fetch_one(raw.as_ref())
            .await
            .unwrap_or_else(|e| panic!("{label} row should still exist after merge: {e}"));
        assert_eq!(
            owner,
            target.id.as_str(),
            "{label} row should now be keyed to the target user"
        );
    }

    assert!(
        ctx.service
            .find_by_id(&source.id)
            .await
            .expect("lookup")
            .is_none(),
        "the source user row must be deleted by the merge"
    );

    ctx.purge(&target.id).await;
}

#[tokio::test]
async fn promote_anonymous_refuses_a_non_anonymous_source() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let source = ctx
        .service
        .create(
            &format!("psrc-{t}"),
            &format!("psrc-{t}@promote.invalid"),
            None,
            None,
        )
        .await
        .expect("create source");
    let target = ctx
        .service
        .create(
            &format!("ptgt-{t}"),
            &format!("ptgt-{t}@promote.invalid"),
            None,
            None,
        )
        .await
        .expect("create target");

    let err = ctx
        .service
        .promote_anonymous(&source.id, &target.id)
        .await
        .expect_err("promoting a registered account must be refused");
    assert!(
        matches!(err, UserError::Validation(_)),
        "unexpected error: {err:?}"
    );
    assert!(
        ctx.service
            .find_by_id(&source.id)
            .await
            .expect("lookup")
            .is_some(),
        "a refused promotion must not delete the source"
    );

    ctx.purge(&source.id).await;
    ctx.purge(&target.id).await;
}

#[tokio::test]
async fn promote_anonymous_refuses_a_self_merge() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let user = ctx
        .service
        .create(
            &format!("pself-{t}"),
            &format!("pself-{t}@promote.invalid"),
            None,
            None,
        )
        .await
        .expect("create user");

    let err = ctx
        .service
        .promote_anonymous(&user.id, &user.id)
        .await
        .expect_err("a self-merge must be refused");
    assert!(
        matches!(err, UserError::Validation(_)),
        "unexpected error: {err:?}"
    );

    ctx.purge(&user.id).await;
}

fn claims(email: Option<&str>, email_verified: bool) -> FederatedIdentityClaims {
    FederatedIdentityClaims {
        email: email.map(ToOwned::to_owned),
        email_verified,
        name: Some("Federated Person".to_owned()),
        preferred_username: None,
        roles: Vec::new(),
    }
}

#[tokio::test]
async fn verified_federated_email_links_to_the_existing_local_account() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let existing = ctx
        .service
        .create(
            &format!("fed-{t}"),
            &format!("existing-{t}@user.invalid"),
            None,
            None,
        )
        .await
        .expect("create existing user");

    let issuer = format!("https://idp-{t}.invalid");
    let external_sub = format!("sub-{t}");
    let linked = ctx
        .service
        .find_or_create_federated(
            &issuer,
            &external_sub,
            &claims(Some(&format!("Existing-{t}@User.Invalid")), true),
        )
        .await
        .expect("federated sign-in");

    assert_eq!(
        linked.id, existing.id,
        "a verified federated email must link to the existing account, not mint a new one"
    );

    let link_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM federated_identities WHERE issuer = $1 AND external_sub = $2 AND \
         user_id = $3",
    )
    .bind(&issuer)
    .bind(&external_sub)
    .bind(existing.id.as_str())
    .fetch_one(ctx.raw().as_ref())
    .await
    .expect("link probe");
    assert_eq!(
        link_count, 1,
        "the federated identity link must be recorded"
    );

    ctx.purge(&existing.id).await;
}

#[tokio::test]
async fn unverified_federated_email_creates_a_separate_synthetic_account() {
    let Some(ctx) = setup().await else {
        return;
    };
    let t = tag();
    let existing = ctx
        .service
        .create(
            &format!("fedu-{t}"),
            &format!("unverified-{t}@user.invalid"),
            None,
            None,
        )
        .await
        .expect("create existing user");

    let issuer = format!("https://hostile-{t}.invalid");
    let created = ctx
        .service
        .find_or_create_federated(
            &issuer,
            &format!("sub-{t}"),
            &claims(Some(&format!("Unverified-{t}@User.Invalid")), false),
        )
        .await
        .expect("federated sign-in");

    assert_ne!(
        created.id, existing.id,
        "an unverified upstream email must not claim an existing account"
    );
    assert!(
        created.email.ends_with(".federated.local"),
        "unverified sign-in should get a synthetic local email, got {}",
        created.email
    );

    ctx.purge(&created.id).await;
    ctx.purge(&existing.id).await;
}
