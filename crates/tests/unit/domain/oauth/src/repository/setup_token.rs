// SetupTokenPurpose pure-logic tests + DB-backed setup-token round-trips.

use std::str::FromStr;

use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::{
    CreateSetupTokenParams, OAuthRepository, SetupTokenPurpose, TokenValidationResult,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

#[test]
fn purpose_as_str_round_trips() {
    assert_eq!(
        SetupTokenPurpose::CredentialLink.as_str(),
        "credential_link"
    );
    assert_eq!(SetupTokenPurpose::Recovery.as_str(), "recovery");
    assert_eq!(
        SetupTokenPurpose::from_str("credential_link").expect("parse"),
        SetupTokenPurpose::CredentialLink
    );
    assert_eq!(
        SetupTokenPurpose::from_str("recovery").expect("parse"),
        SetupTokenPurpose::Recovery
    );
}

#[test]
fn purpose_parse_rejects_unknown() {
    let err = SetupTokenPurpose::from_str("bogus").expect_err("reject");
    assert!(err.to_string().contains("bogus"));
}

#[test]
fn purpose_display_matches_as_str() {
    assert_eq!(SetupTokenPurpose::Recovery.to_string(), "recovery");
}

struct Ctx {
    repo: OAuthRepository,
    user_id: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("st");
    seed_user_row(&pool, &user_id, &format!("{}@st.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Some(Ctx { repo, user_id })
}

#[tokio::test]
async fn store_then_validate_valid() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("hash-{}", Uuid::new_v4());
    ctx.repo
        .store_setup_token(CreateSetupTokenParams {
            user_id: ctx.user_id.clone(),
            token_hash: hash.clone(),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("store");

    match ctx
        .repo
        .validate_setup_token(&hash)
        .await
        .expect("validate")
    {
        TokenValidationResult::Valid(rec) => {
            assert_eq!(rec.user_id, ctx.user_id);
            assert_eq!(rec.purpose, SetupTokenPurpose::CredentialLink);
        },
        other => panic!("expected Valid, got {other:?}"),
    }
}

#[tokio::test]
async fn validate_not_found() {
    let Some(ctx) = setup().await else { return };
    let result = ctx
        .repo
        .validate_setup_token(&format!("missing-{}", Uuid::new_v4()))
        .await
        .expect("validate");
    assert!(matches!(result, TokenValidationResult::NotFound));
}

#[tokio::test]
async fn validate_expired() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("hash-{}", Uuid::new_v4());
    ctx.repo
        .store_setup_token(CreateSetupTokenParams {
            user_id: ctx.user_id.clone(),
            token_hash: hash.clone(),
            purpose: SetupTokenPurpose::Recovery,
            expires_at: Utc::now() - Duration::hours(1),
        })
        .await
        .expect("store");
    let result = ctx
        .repo
        .validate_setup_token(&hash)
        .await
        .expect("validate");
    assert!(matches!(result, TokenValidationResult::Expired));
}

#[tokio::test]
async fn consume_then_already_used() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("hash-{}", Uuid::new_v4());
    ctx.repo
        .store_setup_token(CreateSetupTokenParams {
            user_id: ctx.user_id.clone(),
            token_hash: hash.clone(),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("store");

    let token_id = match ctx
        .repo
        .validate_setup_token(&hash)
        .await
        .expect("validate")
    {
        TokenValidationResult::Valid(rec) => rec.id,
        other => panic!("expected Valid, got {other:?}"),
    };

    assert!(
        ctx.repo
            .consume_setup_token(&token_id)
            .await
            .expect("consume")
    );
    // Consuming again is a no-op.
    assert!(
        !ctx.repo
            .consume_setup_token(&token_id)
            .await
            .expect("consume again")
    );

    let result = ctx
        .repo
        .validate_setup_token(&hash)
        .await
        .expect("validate");
    assert!(matches!(result, TokenValidationResult::AlreadyUsed));
}

#[tokio::test]
async fn revoke_user_setup_tokens_marks_used() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("hash-{}", Uuid::new_v4());
    ctx.repo
        .store_setup_token(CreateSetupTokenParams {
            user_id: ctx.user_id.clone(),
            token_hash: hash.clone(),
            purpose: SetupTokenPurpose::Recovery,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("store");

    let revoked = ctx
        .repo
        .revoke_user_setup_tokens(&ctx.user_id)
        .await
        .expect("revoke");
    assert!(revoked >= 1);
    let result = ctx
        .repo
        .validate_setup_token(&hash)
        .await
        .expect("validate");
    assert!(matches!(result, TokenValidationResult::AlreadyUsed));
}

#[tokio::test]
async fn cleanup_expired_setup_tokens_runs() {
    let Some(ctx) = setup().await else { return };
    // Just exercise the path; rows older than 24h are rare in a fresh DB.
    ctx.repo
        .cleanup_expired_setup_tokens()
        .await
        .expect("cleanup");
}
