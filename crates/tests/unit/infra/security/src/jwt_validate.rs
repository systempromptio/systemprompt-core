use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_models::auth::{JwtAudience, JwtClaims, TokenType, UserType};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{JWT_LEEWAY_SECONDS, ValidationPolicy, decode_rs256_claims};
use systemprompt_security::keys::authority;
use systemprompt_test_fixtures::install_test_signing_key;

fn make_claims(issuer: &str, aud: Vec<JwtAudience>) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "u1".to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: issuer.to_owned(),
        aud,
        jti: "jti1".to_owned(),
        scope: vec![],
        username: "u@test.com".to_owned(),
        email: "u@test.com".to_owned(),
        user_type: UserType::User,
        roles: vec![],
        attributes: Default::default(),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: None,
        rate_limit_tier: None,
        plugin_id: None,
        act: None,
    }
}

fn mint_active_kid(claims: &JwtClaims) -> String {
    install_test_signing_key();
    let kid = authority::active_kid().expect("kid");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = authority::encoding_key().expect("key");
    encode(&header, claims, key).expect("encode")
}

#[test]
fn session_context_accepts_first_party_audience() {
    let token = mint_active_kid(&make_claims("test", JwtAudience::standard()));
    let claims = decode_rs256_claims(&token, &ValidationPolicy::session_context()).expect("decode");
    assert_eq!(claims.sub, "u1");
}

#[test]
fn session_context_rejects_hook_only_audience() {
    let token = mint_active_kid(&make_claims("test", vec![JwtAudience::Hook]));
    let err = decode_rs256_claims(&token, &ValidationPolicy::session_context()).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "expected InvalidToken for hook-only aud, got {err:?}"
    );
}

#[test]
fn session_context_rejects_custom_resource_audience() {
    let token = mint_active_kid(&make_claims(
        "test",
        vec![JwtAudience::Resource("https://example.com/api".to_owned())],
    ));
    let err = decode_rs256_claims(&token, &ValidationPolicy::session_context()).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "expected InvalidToken for resource-only aud, got {err:?}"
    );
}

#[test]
fn session_context_accepts_mixed_bridge_and_mcp_audience() {
    let token = mint_active_kid(&make_claims(
        "test",
        vec![JwtAudience::Bridge, JwtAudience::Mcp],
    ));
    decode_rs256_claims(&token, &ValidationPolicy::session_context())
        .expect("bridge+mcp token intersects the first-party set");
}

#[test]
fn issuer_scoped_rejects_wrong_audience() {
    let token = mint_active_kid(&make_claims("my.issuer", vec![JwtAudience::Web]));
    let auds = [JwtAudience::Mcp];
    let err = decode_rs256_claims(&token, &ValidationPolicy::issuer_scoped("my.issuer", &auds))
        .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "expected InvalidToken for non-intersecting aud, got {err:?}"
    );
}

#[test]
fn issuer_scoped_rejects_wrong_issuer() {
    let token = mint_active_kid(&make_claims("other.issuer", JwtAudience::standard()));
    let auds = [JwtAudience::Web];
    let err = decode_rs256_claims(&token, &ValidationPolicy::issuer_scoped("my.issuer", &auds))
        .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "expected InvalidToken for wrong issuer, got {err:?}"
    );
}

#[test]
fn issuer_scoped_accepts_matching_issuer_and_audience() {
    let token = mint_active_kid(&make_claims("my.issuer", vec![JwtAudience::Api]));
    let auds = [JwtAudience::Web, JwtAudience::Api];
    decode_rs256_claims(&token, &ValidationPolicy::issuer_scoped("my.issuer", &auds))
        .expect("decode");
}

#[test]
fn empty_audience_policy_is_rejected_before_decode() {
    let token = mint_active_kid(&make_claims("test", JwtAudience::standard()));
    let err =
        decode_rs256_claims(&token, &ValidationPolicy::issuer_scoped("test", &[])).unwrap_err();
    assert!(
        matches!(err, AuthError::EmptyAudiencePolicy),
        "expected EmptyAudiencePolicy, got {err:?}"
    );
}

#[test]
fn decode_rejects_non_rs256_token() {
    install_test_signing_key();
    use jsonwebtoken::EncodingKey;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct MinClaims {
        sub: String,
        exp: i64,
    }

    let secret = b"super-secret-key-that-is-quite-long-enough";
    let claims = MinClaims {
        sub: "u1".to_owned(),
        exp: 9999999999,
    };
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("kid1".to_owned());
    let token = encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap();

    let err = decode_rs256_claims(&token, &ValidationPolicy::session_context()).unwrap_err();
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm { .. }),
        "expected UnsupportedAlgorithm, got {err:?}"
    );
}

#[test]
fn decode_rejects_missing_kid() {
    install_test_signing_key();
    let claims = make_claims("test", JwtAudience::standard());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = None;
    let key = authority::encoding_key().expect("encoding key");
    let token = encode(&header, &claims, key).unwrap();

    let err = decode_rs256_claims(&token, &ValidationPolicy::session_context()).unwrap_err();
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got {err:?}"
    );
}

#[test]
fn decode_rejects_unknown_kid() {
    install_test_signing_key();
    let claims = make_claims("test", JwtAudience::standard());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("unknown-kid-xyz".to_owned());
    let key = authority::encoding_key().expect("encoding key");
    let token = encode(&header, &claims, key).unwrap();

    let err = decode_rs256_claims(&token, &ValidationPolicy::session_context()).unwrap_err();
    assert!(
        matches!(err, AuthError::UnknownKid(_)),
        "expected UnknownKid, got {err:?}"
    );
}

#[test]
fn leeway_constant_is_thirty_seconds() {
    assert_eq!(JWT_LEEWAY_SECONDS, 30);
}
