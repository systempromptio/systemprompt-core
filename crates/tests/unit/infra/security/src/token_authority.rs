use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, decode_header, encode};
use systemprompt_identifiers::{AccessTokenId, PluginId, SessionId, UserId};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, TokenAuthority, TokenError, UserJwtRequest,
};
use systemprompt_security::DeploymentTokenAuthority;
use systemprompt_test_fixtures::test_signing_keys;

const ISSUER: &str = "test-issuer";

fn authority() -> DeploymentTokenAuthority {
    DeploymentTokenAuthority::new(test_signing_keys(), ISSUER)
}

fn request(audience: Vec<JwtAudience>, permissions: Vec<Permission>) -> UserJwtRequest {
    UserJwtRequest {
        user: AuthenticatedUser::new(
            UserId::new("user-1"),
            "user".to_owned(),
            "user@test.com".to_owned(),
            permissions.clone(),
        ),
        session_id: SessionId::new("sess-1"),
        jti: AccessTokenId::generate(),
        permissions,
        audience,
        ttl: Duration::hours(1),
        client_id: None,
        purpose: Some("farm:run-1".to_owned()),
        plugin_id: None,
    }
}

#[test]
fn mint_user_jwt_round_trips_through_verify_bearer() {
    let authority = authority();
    let req = request(vec![JwtAudience::Mcp], vec![Permission::User]);
    let token = authority.mint_user_jwt(&req).expect("mint");
    let claims = authority
        .verify_bearer(&token, &[JwtAudience::Mcp])
        .expect("verify");
    assert_eq!(claims.registered.sub, "user-1");
    assert_eq!(claims.registered.iss, ISSUER);
    assert_eq!(claims.registered.jti, req.jti.to_string());
    assert_eq!(claims.custom.purpose.as_deref(), Some("farm:run-1"));
    assert_eq!(claims.custom.session_id, Some(SessionId::new("sess-1")));
}

#[test]
fn mint_user_jwt_rejects_a_non_positive_lifetime() {
    let mut req = request(vec![JwtAudience::Mcp], vec![Permission::User]);
    req.ttl = Duration::zero();
    let err = authority().mint_user_jwt(&req).expect_err("lifetime");
    assert!(matches!(err, TokenError::InvalidLifetime { seconds: 0 }));
}

#[test]
fn verify_bearer_rejects_an_audience_outside_the_policy() {
    let authority = authority();
    let token = authority
        .mint_user_jwt(&request(vec![JwtAudience::Mcp], vec![Permission::User]))
        .expect("mint");
    let err = authority
        .verify_bearer(&token, &[JwtAudience::Api])
        .expect_err("audience");
    assert!(matches!(err, TokenError::InvalidAudience));
}

#[test]
fn verify_bearer_rejects_a_foreign_issuer() {
    let minted = authority()
        .mint_user_jwt(&request(vec![JwtAudience::Mcp], vec![Permission::User]))
        .expect("mint");
    let other = DeploymentTokenAuthority::new(test_signing_keys(), "someone-else");
    let err = other
        .verify_bearer(&minted, &[JwtAudience::Mcp])
        .expect_err("issuer");
    assert!(matches!(err, TokenError::InvalidIssuer));
}

#[test]
fn verify_bearer_reports_an_expired_token() {
    let authority = authority();
    let mut claims = systemprompt_security::jwt::user_claims(
        ISSUER,
        &request(vec![JwtAudience::Mcp], vec![Permission::User]),
    )
    .expect("claims");
    claims.registered.exp = (Utc::now() - Duration::hours(1)).timestamp();
    let token =
        systemprompt_security::jwt::encode_rs256(&test_signing_keys(), &claims).expect("encode");
    let err = authority
        .verify_bearer(&token, &[JwtAudience::Mcp])
        .expect_err("expired");
    assert!(matches!(err, TokenError::Expired));
}

#[test]
fn verify_bearer_reports_garbage_as_malformed() {
    let err = authority()
        .verify_bearer("not-a-jwt", &[JwtAudience::Mcp])
        .expect_err("malformed");
    assert!(matches!(err, TokenError::Malformed(_)));
}

#[test]
fn verify_bearer_reports_an_unknown_kid() {
    let authority = authority();
    let claims = systemprompt_security::jwt::user_claims(
        ISSUER,
        &request(vec![JwtAudience::Mcp], vec![Permission::User]),
    )
    .expect("claims");
    let keys = test_signing_keys();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("rotated-away".to_owned());
    let token = encode(&header, &claims, keys.encoding_key()).expect("encode");
    let err = authority
        .verify_bearer(&token, &[JwtAudience::Mcp])
        .expect_err("kid");
    assert!(matches!(err, TokenError::UnknownKid(kid) if kid == "rotated-away"));
}

#[test]
fn verify_bearer_reports_a_missing_kid_as_kid_mismatch() {
    let authority = authority();
    let claims = systemprompt_security::jwt::user_claims(
        ISSUER,
        &request(vec![JwtAudience::Mcp], vec![Permission::User]),
    )
    .expect("claims");
    let token = encode(
        &Header::new(Algorithm::RS256),
        &claims,
        test_signing_keys().encoding_key(),
    )
    .expect("encode");
    let err = authority
        .verify_bearer(&token, &[JwtAudience::Mcp])
        .expect_err("kid");
    assert!(matches!(err, TokenError::KidMismatch));
}

#[test]
fn verify_bearer_reports_a_tampered_signature() {
    let authority = authority();
    let token = authority
        .mint_user_jwt(&request(vec![JwtAudience::Mcp], vec![Permission::User]))
        .expect("mint");
    let (head, _) = token.rsplit_once('.').expect("jwt shape");
    let forged = format!("{head}.AAAA");
    let err = authority
        .verify_bearer(&forged, &[JwtAudience::Mcp])
        .expect_err("signature");
    assert!(matches!(
        err,
        TokenError::InvalidSignature | TokenError::Malformed(_)
    ));
}

#[test]
fn verify_session_cookie_accepts_first_party_and_refuses_hook_audience() {
    let authority = authority();
    let session = authority
        .mint_user_jwt(&request(JwtAudience::standard(), vec![Permission::User]))
        .expect("mint");
    authority.verify_session_cookie(&session).expect("session");

    let hook = authority
        .mint_user_jwt(&request(vec![JwtAudience::Hook], vec![Permission::User]))
        .expect("mint");
    let err = authority.verify_session_cookie(&hook).expect_err("hook");
    assert!(matches!(err, TokenError::InvalidAudience));
}

#[test]
fn verify_session_cookie_refuses_a_forged_user_type() {
    let authority = authority();
    let mut claims = systemprompt_security::jwt::user_claims(
        ISSUER,
        &request(JwtAudience::standard(), vec![Permission::User]),
    )
    .expect("claims");
    claims.custom.user_type = systemprompt_models::auth::UserType::Admin;
    let token =
        systemprompt_security::jwt::encode_rs256(&test_signing_keys(), &claims).expect("encode");
    let err = authority.verify_session_cookie(&token).expect_err("type");
    assert!(matches!(err, TokenError::Rejected(_)));
}

#[test]
fn verify_hook_token_accepts_only_the_hook_audience() {
    let authority = authority();
    let mut req = request(
        vec![JwtAudience::Hook],
        vec![Permission::HookGovern, Permission::HookTrack],
    );
    req.plugin_id = Some(PluginId::new("plugin-a"));
    let hook = authority.mint_user_jwt(&req).expect("mint");
    let claims = authority.verify_hook_token(&hook).expect("hook");
    assert_eq!(claims.custom.plugin_id, Some(PluginId::new("plugin-a")));

    let mcp = authority
        .mint_user_jwt(&request(vec![JwtAudience::Mcp], vec![Permission::User]))
        .expect("mint");
    let err = authority.verify_hook_token(&mcp).expect_err("audience");
    assert!(matches!(err, TokenError::InvalidAudience));
}

#[test]
fn sign_claims_stamps_the_active_kid_and_verifies() {
    let authority = authority();
    let now = Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "user-1",
        "iat": now,
        "exp": now + 600,
        "nbf": now,
        "iss": ISSUER,
        "aud": ["mcp"],
        "jti": "j1",
        "scope": "user",
        "username": "user",
        "email": "user@test.com",
        "user_type": "user",
        "token_type": "Bearer",
        "auth_time": now,
    });
    let token = authority.sign_claims(&claims).expect("sign");
    let header = decode_header(&token).expect("header");
    assert_eq!(header.alg, Algorithm::RS256);
    assert_eq!(
        header.kid.as_deref(),
        Some(test_signing_keys().active_kid())
    );
    let verified = authority
        .verify_bearer(&token, &[JwtAudience::Mcp])
        .expect("verify");
    assert_eq!(verified.registered.jti, "j1");
}
