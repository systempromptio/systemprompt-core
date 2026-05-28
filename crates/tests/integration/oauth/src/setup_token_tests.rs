//! Integration tests for setup-token persistence and validation.

use crate::{create_test_user, setup_test_db};
use chrono::{Duration, Utc};
use systemprompt_oauth::repository::{
    CreateSetupTokenParams, OAuthRepository, SetupTokenPurpose, TokenValidationResult,
};
use uuid::Uuid;

fn unique_hash() -> String {
    format!("h_{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn setup_token_full_lifecycle() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(&db).expect("repo");

    let hash = unique_hash();
    let _id = repo
        .store_setup_token(CreateSetupTokenParams {
            user_id: user_id.clone(),
            token_hash: hash.clone(),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("store");

    match repo.validate_setup_token(&hash).await.expect("validate") {
        TokenValidationResult::Valid(record) => {
            assert_eq!(record.user_id.as_str(), user_id.as_str());
            assert_eq!(record.purpose, SetupTokenPurpose::CredentialLink);
            assert!(repo.consume_setup_token(&record.id).await.expect("consume"));
            assert!(
                !repo
                    .consume_setup_token(&record.id)
                    .await
                    .expect("re-consume"),
                "second consume returns false"
            );
        },
        other => panic!("expected Valid, got {:?}", other),
    }

    match repo.validate_setup_token(&hash).await.expect("validate") {
        TokenValidationResult::AlreadyUsed => {},
        other => panic!("expected AlreadyUsed after consume, got {:?}", other),
    }
}

#[tokio::test]
async fn validate_unknown_token_returns_not_found() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");

    match repo
        .validate_setup_token("nonexistent-hash")
        .await
        .expect("validate")
    {
        TokenValidationResult::NotFound => {},
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn validate_expired_token_returns_expired() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(&db).expect("repo");

    let hash = unique_hash();
    repo.store_setup_token(CreateSetupTokenParams {
        user_id,
        token_hash: hash.clone(),
        purpose: SetupTokenPurpose::Recovery,
        expires_at: Utc::now() - Duration::hours(1),
    })
    .await
    .expect("store");

    match repo.validate_setup_token(&hash).await.expect("validate") {
        TokenValidationResult::Expired => {},
        other => panic!("expected Expired, got {:?}", other),
    }
}

#[tokio::test]
async fn revoke_user_setup_tokens_marks_unused_tokens_used() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(&db).expect("repo");

    for _ in 0..3 {
        repo.store_setup_token(CreateSetupTokenParams {
            user_id: user_id.clone(),
            token_hash: unique_hash(),
            purpose: SetupTokenPurpose::CredentialLink,
            expires_at: Utc::now() + Duration::hours(1),
        })
        .await
        .expect("store");
    }
    let revoked = repo
        .revoke_user_setup_tokens(&user_id)
        .await
        .expect("revoke");
    assert!(revoked >= 3);

    let again = repo
        .revoke_user_setup_tokens(&user_id)
        .await
        .expect("revoke twice");
    assert_eq!(again, 0, "second revoke is a no-op");
}

#[tokio::test]
async fn cleanup_expired_setup_tokens_returns_count() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let _ = repo.cleanup_expired_setup_tokens().await.expect("cleanup");
}

#[test]
fn purpose_as_str_and_display_match() {
    assert_eq!(
        SetupTokenPurpose::CredentialLink.as_str(),
        "credential_link"
    );
    assert_eq!(SetupTokenPurpose::Recovery.as_str(), "recovery");
    assert_eq!(format!("{}", SetupTokenPurpose::Recovery), "recovery");
}

#[test]
fn purpose_from_str_roundtrip() {
    use std::str::FromStr;
    assert_eq!(
        SetupTokenPurpose::from_str("credential_link").unwrap(),
        SetupTokenPurpose::CredentialLink
    );
    assert_eq!(
        SetupTokenPurpose::from_str("recovery").unwrap(),
        SetupTokenPurpose::Recovery
    );
    assert!(SetupTokenPurpose::from_str("invalid").is_err());
}
