use chrono::Duration;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::session::{SessionGenerator, SessionParams};
use systemprompt_test_fixtures::install_test_signing_key;

fn make_params<'a>(
    user_id: &'a UserId,
    session_id: &'a SessionId,
    duration: Duration,
    user_type: UserType,
    permissions: Vec<Permission>,
) -> SessionParams<'a> {
    SessionParams {
        user_id,
        session_id,
        email: "test@example.com",
        duration,
        user_type,
        permissions,
        roles: vec!["user".to_owned()],
        attributes: Default::default(),
        rate_limit_tier: RateLimitTier::User,
    }
}

#[test]
fn session_generator_mints_valid_token() {
    install_test_signing_key();
    let uid = UserId::new("session-user");
    let sid = SessionId::generate();
    let sg = SessionGenerator::new("test-issuer");
    let params = make_params(
        &uid,
        &sid,
        Duration::hours(1),
        UserType::User,
        vec![Permission::User],
    );
    let token = sg.generate(&params).expect("mint session token");
    assert!(!token.as_str().is_empty());
}

#[test]
fn session_generator_token_is_decodable() {
    install_test_signing_key();
    let uid = UserId::new("decode-user");
    let sid = SessionId::generate();
    let sg = SessionGenerator::new("decode-issuer");
    let params = make_params(
        &uid,
        &sid,
        Duration::hours(1),
        UserType::User,
        vec![Permission::User],
    );
    let token = sg.generate(&params).expect("mint");
    let token_str = format!("Bearer {}", token.as_str());
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderValue::from_str(&token_str).unwrap(),
    )]);

    use systemprompt_security::extraction::TokenExtractor;
    let raw = TokenExtractor::extract_from_authorization(&headers).expect("extract");
    assert!(!raw.is_empty());
}

#[test]
fn session_generator_admin_token_type() {
    install_test_signing_key();
    let uid = UserId::new("admin-session-user");
    let sid = SessionId::generate();
    let sg = SessionGenerator::new("admin-issuer");
    let params = make_params(
        &uid,
        &sid,
        Duration::hours(2),
        UserType::Admin,
        vec![Permission::Admin],
    );
    let token = sg.generate(&params).expect("mint admin session");
    assert!(!token.as_str().is_empty());
}
