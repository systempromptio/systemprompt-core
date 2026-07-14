// JwtTokenValidator: config-driven construction and decode arms.

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::TokenValidator;
use systemprompt_oauth::services::JwtTokenValidator;
use systemprompt_test_fixtures::{ensure_test_bootstrap, mint_admin_jwt};
use uuid::Uuid;

#[tokio::test]
async fn from_config_validates_authority_signed_token() {
    ensure_test_bootstrap();
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user_id, "jwtval@test.invalid", "test");

    let validator = JwtTokenValidator::from_config().expect("from_config");
    let user = validator
        .validate_token(token.as_str())
        .await
        .expect("valid token");

    assert_eq!(user.id.to_string(), user_id.as_str());
    assert_eq!(user.email, "jwtval@test.invalid");
}

#[tokio::test]
async fn validate_token_rejects_garbage() {
    ensure_test_bootstrap();
    let validator = JwtTokenValidator::new("test".to_owned(), vec![JwtAudience::Api]);

    let err = validator
        .validate_token("not-a-jwt")
        .await
        .expect_err("garbage must fail");
    assert!(err.to_string().contains("JWT validation failed"));
}

#[tokio::test]
async fn validate_token_rejects_wrong_issuer() {
    ensure_test_bootstrap();
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user_id, "wrongiss@test.invalid", "other-issuer");

    let validator = JwtTokenValidator::new("test".to_owned(), vec![JwtAudience::Api]);
    let err = validator
        .validate_token(token.as_str())
        .await
        .expect_err("issuer mismatch must fail");
    assert!(err.to_string().contains("JWT validation failed"));
}

#[tokio::test]
async fn validate_token_rejects_non_uuid_subject() {
    ensure_test_bootstrap();
    let user_id = UserId::new("not-a-uuid-subject");
    let token = mint_admin_jwt(&user_id, "badsub@test.invalid", "test");

    let validator = JwtTokenValidator::from_config().expect("from_config");
    let err = validator
        .validate_token(token.as_str())
        .await
        .expect_err("non-uuid sub must fail");
    assert!(err.to_string().contains("Invalid user ID"));
}
