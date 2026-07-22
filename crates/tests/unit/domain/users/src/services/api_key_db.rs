//! DB-backed tests for `ApiKeyService` issuance, verification, and revocation.

use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row,
};
use systemprompt_users::{
    API_KEY_PREFIX, ApiKeyService, IssueApiKeyParams, UserError, UserRepository,
};
use uuid::Uuid;

struct Ctx {
    service: ApiKeyService,
    repo: UserRepository,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{prefix}-{}@key.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &user_id, &email).await.expect("user");
    Some(Ctx {
        service: ApiKeyService::new(&pool).expect("service"),
        repo: UserRepository::new(&pool).expect("repo"),
        user_id,
    })
}

#[tokio::test]
async fn issue_then_verify_round_trip_touches_usage() {
    let Some(ctx) = setup("issue").await else {
        return;
    };
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "  primary  ",
            expires_at: None,
        })
        .await
        .expect("issue");

    assert!(minted.secret.starts_with(API_KEY_PREFIX));
    assert_eq!(minted.record.name, "primary");
    assert!(minted.record.last_used_at.is_none());

    let verified = ctx
        .service
        .verify(&minted.secret)
        .await
        .expect("verify")
        .expect("record");
    assert_eq!(verified.id, minted.record.id);
    assert_eq!(verified.user_id, ctx.user_id);

    let listed = ctx.service.list_for_user(&ctx.user_id).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert!(
        listed[0].last_used_at.is_some(),
        "successful verify must record key usage"
    );
}

#[tokio::test]
async fn issue_rejects_blank_name() {
    let Some(ctx) = setup("blank").await else {
        return;
    };
    let result = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "   ",
            expires_at: None,
        })
        .await;
    assert!(matches!(result, Err(UserError::Validation(_))));
}

#[tokio::test]
async fn verify_rejects_malformed_and_mismatched_secrets() {
    let Some(ctx) = setup("reject").await else {
        return;
    };
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "victim",
            expires_at: None,
        })
        .await
        .expect("issue");

    assert!(
        ctx.service
            .verify("not-an-api-key")
            .await
            .expect("no prefix")
            .is_none()
    );
    assert!(
        ctx.service
            .verify(&format!("{API_KEY_PREFIX}deadbeefdead.ffff"))
            .await
            .expect("unknown prefix")
            .is_none()
    );

    let (prefix, _) = minted.secret.split_once('.').expect("secret shape");
    let forged = format!("{prefix}.{}", "0".repeat(40));
    assert!(
        ctx.service
            .verify(&forged)
            .await
            .expect("wrong hash")
            .is_none()
    );
}

#[tokio::test]
async fn expired_key_fails_verification() {
    let Some(ctx) = setup("expired").await else {
        return;
    };
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "short-lived",
            expires_at: Some(Utc::now() - Duration::hours(1)),
        })
        .await
        .expect("issue");

    assert!(
        ctx.service
            .verify(&minted.secret)
            .await
            .expect("verify")
            .is_none()
    );
}

#[tokio::test]
async fn revoke_disables_key_and_is_idempotent() {
    let Some(ctx) = setup("revoke").await else {
        return;
    };
    let minted = ctx
        .service
        .issue(IssueApiKeyParams {
            user_id: &ctx.user_id,
            name: "revocable",
            expires_at: None,
        })
        .await
        .expect("issue");

    assert!(
        ctx.service
            .revoke(&minted.record.id, &ctx.user_id)
            .await
            .expect("revoke")
    );
    assert!(
        !ctx.service
            .revoke(&minted.record.id, &ctx.user_id)
            .await
            .expect("second revoke")
    );
    assert!(
        ctx.service
            .verify(&minted.secret)
            .await
            .expect("verify")
            .is_none()
    );

    let revoked_ids = ctx
        .repo
        .list_revoked_api_key_ids_for_user(&ctx.user_id)
        .await
        .expect("revoked ids");
    assert_eq!(revoked_ids, vec![minted.record.id.to_string()]);
}
