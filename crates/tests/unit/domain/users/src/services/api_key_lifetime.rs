//! The PAT lifetime ceiling, idempotent issuance and idempotent service
//! accounts.

use chrono::{Duration, TimeDelta, Utc};
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};
use systemprompt_users::{
    ApiKeyLifetime, ApiKeyLimits, ApiKeyService, EnsuredApiKey, IssueApiKeyParams,
    NewServiceAccount, UserError, UserRepository,
};
use uuid::Uuid;

#[test]
fn an_unbounded_lifetime_keeps_the_requested_expiry() {
    let now = Utc::now();
    assert_eq!(ApiKeyLifetime::unbounded().bound(None, now).unwrap(), None);
    let far = now + Duration::days(4000);
    assert_eq!(
        ApiKeyLifetime::unbounded().bound(Some(far), now).unwrap(),
        Some(far)
    );
}

#[test]
fn a_ceiling_defaults_a_missing_expiry_to_the_maximum() {
    let now = Utc::now();
    let bounded = ApiKeyLifetime::from_max_days(Some(90))
        .bound(None, now)
        .unwrap();
    assert_eq!(bounded, Some(now + Duration::days(90)));
}

#[test]
fn a_ceiling_keeps_a_shorter_expiry_and_refuses_a_longer_one() {
    let now = Utc::now();
    let lifetime = ApiKeyLifetime::from_max_days(Some(90));
    let short = now + Duration::days(30);
    assert_eq!(lifetime.bound(Some(short), now).unwrap(), Some(short));
    let long = now + Duration::days(91);
    assert!(matches!(
        lifetime.bound(Some(long), now),
        Err(UserError::ApiKeyLifetimeExceeded { max_days: 90, .. })
    ));
}

struct Ctx {
    service: ApiKeyService,
    repo: UserRepository,
    user_id: UserId,
}

async fn setup(lifetime: ApiKeyLifetime) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("lifetime-{}@key.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &user_id, &email).await.expect("user");
    Ctx {
        service: ApiKeyService::new(Arc::new(UserRepository::new(&pool)), lifetime),
        repo: UserRepository::new(&pool),
        user_id,
    }
}

fn params<'a>(
    user_id: &'a UserId,
    name: &'a str,
    expires_at: Option<chrono::DateTime<Utc>>,
    limits: &'a ApiKeyLimits,
) -> IssueApiKeyParams<'a> {
    IssueApiKeyParams {
        user_id,
        name,
        expires_at,
        limits,
        scopes: &[],
        purpose: None,
    }
}

#[tokio::test]
async fn issuance_without_expiry_is_capped_at_the_maximum() {
    let ctx = setup(ApiKeyLifetime::from_max_days(Some(7))).await;
    let limits = ApiKeyLimits::default();
    let issued = ctx
        .service
        .issue(params(&ctx.user_id, "capped", None, &limits))
        .await
        .expect("issue");
    let expires = issued.record.expires_at.expect("an expiry is always set");
    let expected = Utc::now() + Duration::days(7);
    assert!((expected - expires).num_seconds().abs() < 60);
}

#[tokio::test]
async fn issuance_past_the_maximum_is_refused_and_writes_nothing() {
    let ctx = setup(ApiKeyLifetime::from_max_days(Some(7))).await;
    let limits = ApiKeyLimits::default();
    let result = ctx
        .service
        .issue(params(
            &ctx.user_id,
            "too-long",
            Some(Utc::now() + Duration::days(30)),
            &limits,
        ))
        .await;
    assert!(matches!(
        result,
        Err(UserError::ApiKeyLifetimeExceeded { max_days: 7, .. })
    ));
    assert!(
        ctx.service
            .list_for_user(&ctx.user_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn ensure_issues_once_per_name() {
    let ctx = setup(ApiKeyLifetime::from_max_days(Some(30))).await;
    let limits = ApiKeyLimits::default();
    let first = ctx
        .service
        .ensure(params(&ctx.user_id, "ci", None, &limits), TimeDelta::zero())
        .await
        .expect("first");
    let EnsuredApiKey::Issued(first) = first else {
        panic!("the first call must issue");
    };
    let second = ctx
        .service
        .ensure(params(&ctx.user_id, "ci", None, &limits), TimeDelta::zero())
        .await
        .expect("second");
    let EnsuredApiKey::Existing(existing) = second else {
        panic!("the second call must reuse the live key");
    };
    assert_eq!(existing.id, first.record.id);
    assert_eq!(
        ctx.service.list_for_user(&ctx.user_id).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ensure_renews_a_key_expiring_within_the_renewal_window() {
    let ctx = setup(ApiKeyLifetime::from_max_days(Some(30))).await;
    let limits = ApiKeyLimits::default();
    ctx.service
        .issue(params(
            &ctx.user_id,
            "rotating",
            Some(Utc::now() + Duration::days(2)),
            &limits,
        ))
        .await
        .expect("seed");
    let renewed = ctx
        .service
        .ensure(
            params(&ctx.user_id, "rotating", None, &limits),
            TimeDelta::days(5),
        )
        .await
        .expect("ensure");
    assert!(matches!(renewed, EnsuredApiKey::Issued(_)));
    assert_eq!(
        ctx.service.list_for_user(&ctx.user_id).await.unwrap().len(),
        2,
        "the old key stays live until it is revoked"
    );
}

#[tokio::test]
async fn ensure_ignores_a_revoked_key_with_the_same_name() {
    let ctx = setup(ApiKeyLifetime::unbounded()).await;
    let limits = ApiKeyLimits::default();
    let first = ctx
        .service
        .issue(params(&ctx.user_id, "revoked", None, &limits))
        .await
        .expect("seed");
    ctx.service
        .revoke(&first.record.id, &ctx.user_id)
        .await
        .expect("revoke");
    let ensured = ctx
        .service
        .ensure(
            params(&ctx.user_id, "revoked", None, &limits),
            TimeDelta::zero(),
        )
        .await
        .expect("ensure");
    assert!(matches!(ensured, EnsuredApiKey::Issued(_)));
}

#[tokio::test]
async fn ensuring_a_service_account_twice_returns_the_same_row() {
    let ctx = setup(ApiKeyLifetime::unbounded()).await;
    let email = format!("svc-{}@svc.invalid", Uuid::new_v4().simple());
    let account = || NewServiceAccount {
        name: "ci-bot".to_owned(),
        email: email.clone(),
        created_by: ctx.user_id.clone(),
    };
    let first = ctx.repo.ensure_service_account(account()).await.unwrap();
    let second = ctx.repo.ensure_service_account(account()).await.unwrap();
    assert!(first.created);
    assert!(!second.created);
    assert_eq!(first.user.id, second.user.id);
    assert!(second.user.is_service_account());
}

#[tokio::test]
async fn a_human_email_is_never_converted_into_a_service_account() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let human = UserId::new(Uuid::new_v4().to_string());
    let email = format!("human-{}@key.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &human, &email).await.expect("user");
    let result = UserRepository::new(&pool)
        .ensure_service_account(NewServiceAccount {
            name: "impostor".to_owned(),
            email,
            created_by: human,
        })
        .await;
    assert!(matches!(result, Err(UserError::EmailAlreadyExists(_))));
}
