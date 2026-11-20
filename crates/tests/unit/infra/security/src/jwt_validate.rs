use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, RegisteredClaims, TokenType, UserType,
};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{JWT_LEEWAY_SECONDS, ValidationPolicy, decode_rs256_claims};
use systemprompt_test_fixtures::test_signing_keys;

fn make_claims(issuer: &str, aud: Vec<JwtAudience>) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        registered: RegisteredClaims {
            sub: "u1".to_owned(),
            iat: now.timestamp(),
            exp: (now + Duration::hours(1)).timestamp(),
            nbf: Some(now.timestamp()),
            iss: issuer.to_owned(),
            aud,
            jti: "jti1".to_owned(),
        },
        custom: CustomClaims {
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
            purpose: None,
            interaction: None,
            browser_session: false,
            ui_session: false,
        },
    }
}

fn mint_active_kid(claims: &JwtClaims) -> String {
    let keys = test_signing_keys();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    encode(&header, claims, keys.encoding_key()).expect("encode")
}

#[test]
fn session_context_accepts_first_party_audience() {
    let token = mint_active_kid(&make_claims("test", JwtAudience::standard()));
    let claims = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .expect("decode");
    assert_eq!(claims.registered.sub, "u1");
}

#[test]
fn session_context_rejects_hook_only_audience() {
    let token = mint_active_kid(&make_claims("test", vec![JwtAudience::Hook]));
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
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
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
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
    decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .expect("bridge+mcp token intersects the first-party set");
}

#[test]
fn issuer_scoped_rejects_wrong_audience() {
    let token = mint_active_kid(&make_claims("my.issuer", vec![JwtAudience::Web]));
    let auds = [JwtAudience::Mcp];
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::issuer_scoped("my.issuer", &auds),
    )
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
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::issuer_scoped("my.issuer", &auds),
    )
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
    decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::issuer_scoped("my.issuer", &auds),
    )
    .expect("decode");
}

#[test]
fn empty_audience_policy_is_rejected_before_decode() {
    let token = mint_active_kid(&make_claims("test", JwtAudience::standard()));
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::issuer_scoped("test", &[]),
    )
    .unwrap_err();
    assert!(
        matches!(err, AuthError::EmptyAudiencePolicy),
        "expected EmptyAudiencePolicy, got {err:?}"
    );
}

#[test]
fn decode_rejects_non_rs256_token() {
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

    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm { .. }),
        "expected UnsupportedAlgorithm, got {err:?}"
    );
}

#[test]
fn decode_rejects_missing_kid() {
    let claims = make_claims("test", JwtAudience::standard());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = None;
    let token = encode(&header, &claims, test_signing_keys().encoding_key()).unwrap();

    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got {err:?}"
    );
}

#[test]
fn decode_rejects_unknown_kid() {
    let claims = make_claims("test", JwtAudience::standard());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("unknown-kid-xyz".to_owned());
    let token = encode(&header, &claims, test_signing_keys().encoding_key()).unwrap();

    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
    assert!(
        matches!(err, AuthError::UnknownKid(_)),
        "expected UnknownKid, got {err:?}"
    );
}

#[test]
fn leeway_constant_is_thirty_seconds() {
    assert_eq!(JWT_LEEWAY_SECONDS, 30);
}

#[test]
fn session_context_rejects_a_foreign_issuer() {
    let token = mint_active_kid(&make_claims("other.issuer", JwtAudience::standard()));
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "the session policy pins the issuer, got {err:?}"
    );
}

fn ui_claims() -> JwtClaims {
    let mut claims = make_claims("test", vec![JwtAudience::Ui]);
    claims.custom.ui_session = true;
    claims.custom.client_id = Some(systemprompt_identifiers::ClientId::new("desk_ui"));
    claims
}

#[test]
fn a_ui_session_is_refused_by_the_first_party_policy() {
    let token = mint_active_kid(&ui_claims());
    let err = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::session_context("test"),
    )
    .unwrap_err();
    assert!(matches!(err, AuthError::InvalidToken(_)), "{err:?}");
}

#[test]
fn a_ui_session_decodes_where_the_ui_audience_is_admitted() {
    let token = mint_active_kid(&ui_claims());
    let claims = decode_rs256_claims(
        test_signing_keys().keyring(),
        &token,
        &ValidationPolicy::issuer_scoped("test", JwtAudience::FIRST_PARTY_AND_UI),
    )
    .expect("decode");
    assert!(claims.custom.ui_session);
}

#[test]
fn a_ui_session_in_any_other_shape_is_refused() {
    let mut second_audience = ui_claims();
    second_audience.registered.aud.push(JwtAudience::Api);
    let mut clientless = ui_claims();
    clientless.custom.client_id = None;
    let mut purposed = ui_claims();
    purposed.custom.purpose = Some("farm:run-1".to_owned());
    let mut delegated = ui_claims();
    delegated.custom.act = Some(systemprompt_models::auth::ActClaim {
        iss: "test".to_owned(),
        sub: "delegate".to_owned(),
        act: Box::new(None),
    });
    let mut browser = ui_claims();
    browser.custom.browser_session = true;
    let mut claimless = ui_claims();
    claimless.custom.ui_session = false;
    let mut first_party = make_claims("test", JwtAudience::standard());
    first_party.custom.ui_session = true;
    for claims in [
        second_audience,
        clientless,
        purposed,
        delegated,
        browser,
        claimless,
        first_party,
    ] {
        let token = mint_active_kid(&claims);
        let err = decode_rs256_claims(
            test_signing_keys().keyring(),
            &token,
            &ValidationPolicy::issuer_scoped("test", JwtAudience::FIRST_PARTY_AND_UI),
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::UiSessionShape), "{err:?}");
    }
}
