//! Behavioural tests for [`AuthValidationService::validate_request`].
//!
//! Each test forges an RS256 token signed by a process-global test authority
//! (the same key the validator resolves via `kid` lookup) and asserts the
//! real auth outcome: the produced [`RequestContext`] on success, and the
//! specific [`AuthError`] variant on each rejection path (missing header,
//! missing session id, admin elevation, over-deep delegation chain, wrong
//! issuer, wrong audience, non-RS256 algorithm).

use std::collections::BTreeMap;
use std::sync::Once;

use axum::http::{HeaderMap, HeaderValue};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    ActClaim, JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::AuthValidationService;
use systemprompt_security::error::AuthError;
use systemprompt_security::keys::{RsaSigningKey, authority};

static INSTALL: Once = Once::new();

const ISSUER: &str = "https://issuer.test";

fn ensure_authority() -> &'static RsaSigningKey {
    INSTALL.call_once(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("generate test signing key");
        authority::install_for_test(key);
    });
    authority::signing_key().expect("authority installed")
}

fn base_claims() -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "user-auth-1".to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: ISSUER.to_string(),
        aud: vec![JwtAudience::Api],
        jti: "jti-auth-1".to_string(),
        scope: vec![Permission::User],
        username: "u".to_string(),
        email: "u@example.com".to_string(),
        user_type: UserType::User,
        roles: vec!["user".to_string()],
        attributes: BTreeMap::new(),
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("sess-auth-1")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

fn sign(claims: &JwtClaims) -> String {
    let key = ensure_authority();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid().to_string());
    let der = key
        .private_key()
        .to_pkcs1_der()
        .expect("encode private key der");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    encode(&header, claims, &enc).expect("encode rs256 token")
}

fn bearer_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).expect("header value"),
    );
    headers
}

fn service() -> AuthValidationService {
    AuthValidationService::new(ISSUER.to_string(), vec![JwtAudience::Api])
}

#[test]
fn valid_token_yields_context_with_subject_and_session() {
    let token = sign(&base_claims());
    let headers = bearer_headers(&token);

    let ctx = service()
        .validate_request(&headers)
        .expect("valid token accepted");

    assert_eq!(ctx.user_id().as_str(), "user-auth-1");
    assert_eq!(ctx.session_id().as_str(), "sess-auth-1");
    assert_eq!(ctx.jti(), "jti-auth-1");
    assert_eq!(ctx.auth.user_type, UserType::User);
    assert_eq!(ctx.auth_token().as_str(), token);
}

#[test]
fn admin_scope_elevates_user_type_to_admin() {
    let mut claims = base_claims();
    claims.scope = vec![Permission::User, Permission::Admin];
    let headers = bearer_headers(&sign(&claims));

    let ctx = service()
        .validate_request(&headers)
        .expect("admin token accepted");

    assert_eq!(
        ctx.auth.user_type,
        UserType::Admin,
        "Admin permission must override the claim's user_type"
    );
}

#[test]
fn missing_authorization_header_is_rejected() {
    let headers = HeaderMap::new();
    let err = service()
        .validate_request(&headers)
        .expect_err("no authorization header");
    assert!(matches!(err, AuthError::MissingAuthorization));
}

#[test]
fn missing_session_id_is_rejected() {
    let mut claims = base_claims();
    claims.session_id = None;
    let headers = bearer_headers(&sign(&claims));

    let err = service()
        .validate_request(&headers)
        .expect_err("token without session id");
    assert!(matches!(err, AuthError::MissingSessionId));
}

#[test]
fn over_deep_act_chain_is_rejected() {
    fn nest(depth: usize) -> ActClaim {
        let mut node = ActClaim {
            iss: ISSUER.to_string(),
            sub: "deleg-0".to_string(),
            act: Box::new(None),
        };
        for i in 1..depth {
            node = ActClaim {
                iss: ISSUER.to_string(),
                sub: format!("deleg-{i}"),
                act: Box::new(Some(node)),
            };
        }
        node
    }

    let mut claims = base_claims();
    claims.act = Some(nest(20));
    let headers = bearer_headers(&sign(&claims));

    let err = service()
        .validate_request(&headers)
        .expect_err("over-deep delegation chain");
    match err {
        AuthError::ActChainTooDeep { depth, max } => {
            assert!(depth > max, "reported depth must exceed the cap");
        },
        other => panic!("expected ActChainTooDeep, got {other:?}"),
    }
}

#[test]
fn wrong_issuer_is_rejected() {
    let svc =
        AuthValidationService::new("https://other.issuer".to_string(), vec![JwtAudience::Api]);
    let headers = bearer_headers(&sign(&base_claims()));

    let err = svc.validate_request(&headers).expect_err("issuer mismatch");
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "issuer mismatch must fail signature/claim validation, got {err:?}"
    );
}

#[test]
fn wrong_audience_is_rejected() {
    let svc = AuthValidationService::new(ISSUER.to_string(), vec![JwtAudience::Mcp]);
    let headers = bearer_headers(&sign(&base_claims()));

    let err = svc
        .validate_request(&headers)
        .expect_err("audience mismatch");
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "audience mismatch must fail claim validation, got {err:?}"
    );
}

#[test]
fn hs256_token_is_rejected_as_unsupported_algorithm() {
    let _ = ensure_authority();
    let header = Header::new(Algorithm::HS256);
    let token = encode(
        &header,
        &base_claims(),
        &EncodingKey::from_secret(b"shared-secret"),
    )
    .expect("encode hs256");
    let headers = bearer_headers(&token);

    let err = service()
        .validate_request(&headers)
        .expect_err("hs256 rejected");
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm { .. }),
        "HS256 must be rejected as unsupported, got {err:?}"
    );
}
