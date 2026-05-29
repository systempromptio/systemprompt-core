use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::extract_user_context;
use systemprompt_security::AuthError;
use systemprompt_test_fixtures::install_test_signing_key;

fn mint_custom(
    user_id: &str,
    session_id: Option<SessionId>,
    scope: Vec<Permission>,
    user_type: UserType,
) -> String {
    install_test_signing_key();
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, Algorithm, Header};
    use systemprompt_models::auth::JwtClaims;
    use systemprompt_security::keys::authority;

    let now = Utc::now();
    let claims = JwtClaims {
        sub: user_id.to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: "test".to_owned(),
        aud: JwtAudience::standard(),
        jti: "test-jti".to_owned(),
        scope,
        username: "u@t.com".to_owned(),
        email: "u@t.com".to_owned(),
        user_type,
        roles: vec![],
        attributes: Default::default(),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id,
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    };

    let kid = authority::active_kid().expect("kid");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = authority::encoding_key().expect("key");
    encode(&header, &claims, key).expect("encode")
}

#[test]
fn extract_user_context_success() {
    let sid = SessionId::generate();
    let uid = UserId::new("alice");
    let token = mint_custom(
        uid.as_str(),
        Some(sid.clone()),
        vec![Permission::User],
        UserType::User,
    );

    let ctx = extract_user_context(&token).expect("extract");
    assert_eq!(ctx.user_id.as_str(), "alice");
    assert_eq!(ctx.session_id, sid);
    assert_eq!(ctx.role, Permission::User);
    assert_eq!(ctx.user_type, UserType::User);
}

#[test]
fn extract_user_context_admin() {
    let sid = SessionId::generate();
    let token = mint_custom(
        "admin-user",
        Some(sid),
        vec![Permission::Admin],
        UserType::Admin,
    );

    let ctx = extract_user_context(&token).expect("extract admin");
    assert_eq!(ctx.user_type, UserType::Admin);
    assert_eq!(ctx.role, Permission::Admin);
}

#[test]
fn extract_user_context_missing_session_id() {
    let token = mint_custom("u2", None, vec![Permission::User], UserType::User);
    let err = extract_user_context(&token).unwrap_err();
    assert!(
        matches!(err, AuthError::MissingSessionId),
        "expected MissingSessionId, got {err:?}"
    );
}

#[test]
fn extract_user_context_user_type_mismatch() {
    let sid = SessionId::generate();
    let token = mint_custom(
        "u3",
        Some(sid),
        vec![Permission::Admin],
        UserType::User,
    );
    let err = extract_user_context(&token).unwrap_err();
    assert!(
        matches!(err, AuthError::UserTypeMismatch { .. }),
        "expected UserTypeMismatch, got {err:?}"
    );
}

#[test]
fn extract_user_context_missing_scope() {
    let sid = SessionId::generate();
    let token = mint_custom("u4", Some(sid), vec![], UserType::User);
    let err = extract_user_context(&token).unwrap_err();
    assert!(
        matches!(err, AuthError::MissingScope),
        "expected MissingScope, got {err:?}"
    );
}
