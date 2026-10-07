use std::sync::Arc;
use systemprompt_identifiers::{ApiKeyId, ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};
use systemprompt_users::{
    ApiKeyLimits, ApiKeyService, CreateApiKeyParams, IssueApiKeyParams, UserError, UserRepository,
};
use uuid::Uuid;

struct Ctx {
    service: ApiKeyService,
    repo: UserRepository,
    pool: systemprompt_database::DbPool,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{prefix}-{}@limits.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &user_id, &email).await.expect("user");
    Ctx {
        service: ApiKeyService::new(Arc::new(UserRepository::new(&pool))),
        repo: UserRepository::new(&pool),
        pool,
        user_id,
    }
}

fn scope(dimension: &str, value: &str) -> ScopeBinding {
    ScopeBinding {
        dimension: ScopeDimension::try_new(dimension).expect("dimension"),
        value: value.to_owned(),
    }
}

fn limited() -> ApiKeyLimits {
    ApiKeyLimits {
        model_allowlist: Some(vec!["claude-a".to_owned(), "claude-b".to_owned()]),
        budget_microdollars: Some(5_000_000),
        max_requests: Some(100),
        request_window_seconds: Some(3600),
    }
}

#[tokio::test]
async fn limits_and_scopes_round_trip_through_issue_verify_and_list() {
    let ctx = setup("roundtrip").await;
    let scopes = [scope("project", "apollo"), scope("cost_centre", "cc-9")];
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "limited",
            expires_at: None,
            limits: &limited(),
            scopes: &scopes,
        })
        .await
        .expect("issue");
    assert_eq!(minted.record.limits, limited());
    assert_eq!(minted.record.scopes, scopes.to_vec());

    let verified = ctx
        .service
        .verify(&minted.secret)
        .await
        .expect("verify")
        .expect("record");
    assert_eq!(verified.limits, limited());
    assert_eq!(
        verified.scopes,
        vec![scope("cost_centre", "cc-9"), scope("project", "apollo")],
        "scopes read back ordered by dimension"
    );
    assert!(verified.limits.allows_model("claude-b"));
    assert!(!verified.limits.allows_model("claude-z"));

    let listed = ctx.service.list_for_user(&ctx.user_id).await.expect("list");
    assert_eq!(listed[0].scopes.len(), 2);
}

#[tokio::test]
async fn an_unlimited_key_has_no_limits_and_allows_every_model() {
    let ctx = setup("unlimited").await;
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "open",
            expires_at: None,
            limits: &ApiKeyLimits::default(),
            scopes: &[],
        })
        .await
        .expect("issue");
    assert_eq!(minted.record.limits, ApiKeyLimits::default());
    assert!(minted.record.scopes.is_empty());
    assert!(minted.record.limits.allows_model("anything"));
}

async fn rejected(ctx: &Ctx, limits: ApiKeyLimits, scopes: &[ScopeBinding]) -> String {
    match ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "bad",
            expires_at: None,
            limits: &limits,
            scopes,
        })
        .await
    {
        Err(UserError::Validation(message)) => message,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn issue_validates_limits_and_scopes() {
    let ctx = setup("validate").await;
    let no_window = ApiKeyLimits {
        max_requests: Some(5),
        ..ApiKeyLimits::default()
    };
    assert!(
        rejected(&ctx, no_window, &[])
            .await
            .contains("request_window_seconds")
    );
    let empty_allowlist = ApiKeyLimits {
        model_allowlist: Some(Vec::new()),
        ..ApiKeyLimits::default()
    };
    assert!(
        rejected(&ctx, empty_allowlist, &[])
            .await
            .contains("model_allowlist")
    );
    let negative = ApiKeyLimits {
        budget_microdollars: Some(-1),
        request_window_seconds: Some(60),
        ..ApiKeyLimits::default()
    };
    assert!(rejected(&ctx, negative, &[]).await.contains("negative"));
    let twice = [scope("project", "a"), scope("project", "b")];
    assert!(
        rejected(&ctx, ApiKeyLimits::default(), &twice)
            .await
            .contains("more than once")
    );
}

#[tokio::test]
async fn the_database_refuses_a_ceiling_without_a_window() {
    let ctx = setup("check").await;
    let limits = ApiKeyLimits {
        budget_microdollars: Some(1_000),
        ..ApiKeyLimits::default()
    };
    let id = ApiKeyId::generate();
    let prefix = format!("sp-live-{}", &Uuid::new_v4().simple().to_string()[..12]);
    let result = ctx
        .repo
        .create_api_key(CreateApiKeyParams {
            id: &id,
            user_id: &ctx.user_id,
            name: "unchecked",
            key_prefix: &prefix,
            key_hash: "hash",
            expires_at: None,
            limits: &limits,
            scopes: &[],
        })
        .await;
    let error = result.expect_err("the window CHECK refuses the row");
    assert!(
        format!("{error:?}").contains("user_api_keys_window_required"),
        "{error:?}"
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM user_api_keys WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(ctx.pool.pool().as_ref())
        .await
        .expect("count");
    assert_eq!(rows, 0);
}
